use crate::storage;
use ecotask_types::{Task, TaskStatus};
use soroban_sdk::{
    contract, contractevent, contractimpl, vec, Address, BytesN, Env, IntoVal, String, Symbol, Val,
};
pub use storage::{Verification, VerificationStatus};

/// Maximum allowed length for a proof CID string (covers CIDv1 + multihash).
const MAX_CID_LEN: u32 = 128;

/// Fetches the task from the registry and enforces that it is active and not
/// expired. Returns the task so the caller can inspect `reward_amount`.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task to fetch and validate
///
/// # Returns
///
/// The Task struct if valid.
///
/// # Panics
///
/// * Panics if the task is not Active
/// * Panics if the task has expired
fn require_active_task(e: &Env, task_id: u64) -> Task {
    let registry_id = storage::read_registry(e);
    let task: Task = e.invoke_contract(
        &registry_id,
        &Symbol::new(e, "get_task"),
        vec![e, task_id.into_val(e)],
    );
    if task.status != TaskStatus::Active {
        panic!("engine: task is not active");
    }
    // Semantics: a task is live when expires_at >= now (i.e. it expires
    // only after the timestamp strictly exceeds expires_at). This must
    // match the registry's complete_task exactly.
    if task.expires_at < e.ledger().timestamp() {
        panic!("engine: task has expired");
    }
    task
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofSubmittedEvent {
    #[topic]
    pub oracle: Address,
    #[topic]
    pub user: Address,
    pub task_id: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardPaidEvent {
    #[topic]
    pub oracle: Address,
    #[topic]
    pub user: Address,
    pub task_id: u64,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofRejectedEvent {
    #[topic]
    pub oracle: Address,
    #[topic]
    pub user: Address,
    pub task_id: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeRaisedEvent {
    #[topic]
    pub user: Address,
    pub task_id: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeResolvedEvent {
    #[topic]
    pub user: Address,
    pub task_id: u64,
    pub approved: bool,
    pub reward_amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleAddedEvent {
    #[topic]
    pub oracle: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleRemovedEvent {
    #[topic]
    pub oracle: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminProposedEvent {
    #[topic]
    pub current_admin: Address,
    #[topic]
    pub proposed_admin: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminAcceptedEvent {
    #[topic]
    pub new_admin: Address,
}

#[contract]
pub struct RewardEngine;

/// Panics if the engine is paused.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Panics
///
/// Panics if the contract is paused.
fn require_not_paused(e: &Env) {
    if storage::is_paused(e) {
        panic!("engine: contract is paused");
    }
}

/// Panics unless `addr` is one of the registered oracles.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `addr` - The address to check
///
/// # Panics
///
/// Panics if the address is not a registered oracle.
fn require_oracle(e: &Env, addr: &Address) {
    if !storage::is_registered_oracle(e, addr) {
        panic!("engine: unauthorized");
    }
}

/// Panics unless `addr` is the admin.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `addr` - The address to check
///
/// # Panics
///
/// Panics if the address is not the admin.
fn require_admin(e: &Env, addr: &Address) {
    let admin = storage::read_admin(e);
    if addr != &admin {
        panic!("engine: unauthorized");
    }
}

/// Panics if `user` was last rewarded fewer than the configured cooldown
/// ledgers ago. A cooldown of 0 (the default) disables this check.
fn require_cooldown_elapsed(e: &Env, user: &Address) {
    let cooldown = storage::read_user_cooldown(e);
    if cooldown == 0 {
        return;
    }
    if let Some(last) = storage::read_last_reward_ledger(e, user) {
        let current_ledger = e.ledger().sequence() as u64;
        if current_ledger.saturating_sub(last) < cooldown {
            panic!("engine: user cooldown active");
        }
    }
}

/// Records the ledger at which `user` most recently received a reward, so
/// future calls to `require_cooldown_elapsed` can rate-limit them.
fn record_reward_ledger(e: &Env, user: &Address) {
    let current_ledger = e.ledger().sequence() as u64;
    storage::write_last_reward_ledger(e, user, current_ledger);
}

/// Collects up to `limit` pending verifications whose sequence number is
/// greater than `cursor` from the *pending-only* linked list.
///
/// Every verification is assigned an immutable, monotonically-increasing
/// `seq` at submit time. `cursor` is the sequence number of the last
/// verification already returned (0 for the first page), so a page returns
/// every pending verification with `seq > cursor`. Because `seq` values
/// never change, resolving (approving, rejecting, or disputing) an entry
/// between pages can never shift another entry past the cursor: the same
/// pending verification is never skipped or returned twice, no matter how
/// many resolutions happen mid-pagination.
///
/// Resolved verifications are removed from the pending list entirely, so
/// walking it never scans resolved records: each result costs O(1) storage
/// reads regardless of how many verifications have ever been submitted or
/// resolved.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `cursor` - The last sequence number already consumed (0 for the first page)
/// * `limit` - The maximum number of verifications to return
///
/// # Returns
///
/// A vector of pending verification records with `seq > cursor`.
fn collect_pending(e: &Env, cursor: u64, limit: u32) -> soroban_sdk::Vec<Verification> {
    let mut result: soroban_sdk::Vec<Verification> = soroban_sdk::Vec::new(e);
    let state = storage::read_pending_list(e);
    let mut current = state.head;
    let mut collected: u32 = 0;
    while collected < limit {
        let key = match current {
            Some(k) => k,
            None => break,
        };
        // Stable cursor: skip verifications already returned (seq <= cursor).
        // A node's seq is assigned once at submit time and never changes, so
        // entries removed by resolution never shift others past the cursor.
        if let Some(v) = storage::read_verification(e, key.task_id, &key.user) {
            if v.seq > cursor {
                result.push_back(v);
                collected += 1;
            }
        }
        current = storage::read_pending_node_next(e, &key);
    }
    result
}

/// Shared payout tail for `approve_proof` and the approve path of
/// `resolve_dispute`.
///
/// # Ordering (atomicity) rationale
///
/// This function follows the pattern *read-all → validate-all → call
/// external contracts → write local state*. Every validation (reward bounds,
/// task budget, free completion slot, user cooldown) runs before any storage
/// mutation in this contract, and the `Approved` verification record is only
/// written *after* both `complete_task` and `mint` have returned.
///
/// Soroban rolls back the entire invocation tree when a sub-invocation
/// panics, but the engine never relies on that: by deferring the local write
/// until after the external calls succeed, a panic in either call can never
/// leave an orphaned `Approved` record, a consumed completion slot, or a
/// `total_paid`/token-supply discrepancy — regardless of sub-call rollback
/// semantics. The registry's own double-claim and max-completions guards
/// remain the authoritative checks; the `completions < max_completions`
/// check here is defensive pre-validation that fails fast before any state
/// is touched (e.g. two oracles racing on a task's last completion slot).
fn approve_and_pay(
    e: &Env,
    user: &Address,
    task_id: u64,
    reward_amount: i128,
    verification: &mut Verification,
) {
    if reward_amount <= 0 {
        panic!("engine: reward amount must be positive");
    }
    if let Some(min) = storage::read_min_reward(e) {
        if reward_amount < min {
            panic!("engine: reward below minimum");
        }
    }
    if let Some(max) = storage::read_max_reward(e) {
        if reward_amount > max {
            panic!("engine: reward exceeds maximum");
        }
    }

    let task = require_active_task(e, task_id);
    if reward_amount > task.reward_amount {
        panic!("engine: reward exceeds task budget");
    }
    // Defensive pre-validation mirroring registry::complete_task: never
    // consume a completion slot (or pay out) once the task's completion
    // budget is exhausted. The authoritative guard lives in the registry;
    // this check fails fast so no local state is written first.
    if task.completions >= task.max_completions {
        panic!("engine: task max completions reached");
    }

    require_cooldown_elapsed(e, user);

    // Cross-contract calls first — see the doc comment above for why the
    // local state mutations are deferred until both have succeeded.
    let registry_id = storage::read_registry(e);
    e.invoke_contract::<Val>(
        &registry_id,
        &Symbol::new(e, "complete_task"),
        vec![
            &e,
            e.current_contract_address().into_val(e),
            task_id.into_val(e),
            user.clone().into_val(e),
        ],
    );

    let token_id = storage::read_token(e);
    e.invoke_contract::<Val>(
        &token_id,
        &Symbol::new(e, "mint"),
        vec![&e, user.clone().into_val(e), reward_amount.into_val(e)],
    );

    // Both external calls succeeded: now record the payout locally.
    verification.status = VerificationStatus::Approved;
    verification.reward_amount = reward_amount;
    verification.resolved_at = Some(e.ledger().timestamp());
    storage::write_verification(e, task_id, user, verification);
    storage::remove_verification_key(e, task_id, user);

    storage::add_total_paid(e, reward_amount);
    record_reward_ledger(e, user);
}

#[contractimpl]
impl RewardEngine {
    /// Initializes the reward engine contract with core addresses.
    ///
    /// # Arguments
    ///
    /// * `admin` - The initial administrator address
    /// * `token` - The address of the token contract
    /// * `registry` - The address of the task registry contract
    /// * `oracle` - The initial oracle address
    ///
    /// # Panics
    ///
    /// * Panics if the contract has already been initialized
    /// * Panics if `admin == oracle` (separation of duties requirement)
    ///
    /// # Auth
    ///
    /// No authentication required. Can only be called once during deployment.
    pub fn initialize(e: Env, admin: Address, token: Address, registry: Address, oracle: Address) {
        if storage::has_admin(&e) {
            panic!("engine: already initialized");
        }
        if admin == oracle {
            panic!("engine: oracle must be different from admin");
        }
        storage::write_admin(&e, &admin);
        storage::write_token(&e, &token);
        storage::write_registry(&e, &registry);
        storage::write_oracles(&e, &vec![&e, oracle]);
    }

    /// Replaces the entire oracle roster with a single oracle.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `new_oracle` - The new oracle address to set
    ///
    /// # Panics
    ///
    /// * Panics if caller is not the admin
    /// * Panics if `new_oracle == caller` (oracle must differ from admin)
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn set_oracle(e: Env, caller: Address, new_oracle: Address) {
        caller.require_auth();
        require_admin(&e, &caller);
        if new_oracle == caller {
            panic!("engine: oracle must be different from admin");
        }
        storage::write_oracles(&e, &vec![&e, new_oracle]);
    }

    /// Registers an additional oracle. Any registered oracle may submit,
    /// approve, or reject proofs.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `new_oracle` - The new oracle address to add
    ///
    /// # Panics
    ///
    /// * Panics if caller is not the admin
    /// * Panics if `new_oracle == admin` (oracle must differ from admin)
    /// * Panics if `new_oracle` is already registered
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn add_oracle(e: Env, caller: Address, new_oracle: Address) {
        caller.require_auth();
        require_admin(&e, &caller);
        let admin = storage::read_admin(&e);
        if new_oracle == admin {
            panic!("engine: oracle must be different from admin");
        }
        if storage::is_registered_oracle(&e, &new_oracle) {
            panic!("engine: oracle already registered");
        }
        storage::push_oracle(&e, &new_oracle);
        OracleAddedEvent { oracle: new_oracle }.publish(&e);
    }

    /// Removes a registered oracle. The last remaining oracle cannot be
    /// removed, so the engine always keeps at least one active oracle.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `oracle` - The oracle address to remove
    ///
    /// # Panics
    ///
    /// * Panics if caller is not the admin
    /// * Panics if `oracle` is not registered
    /// * Panics if `oracle` is the last remaining oracle
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn remove_oracle(e: Env, caller: Address, oracle: Address) {
        caller.require_auth();
        require_admin(&e, &caller);
        if !storage::is_registered_oracle(&e, &oracle) {
            panic!("engine: oracle not registered");
        }
        if storage::read_oracles(&e).len() <= 1 {
            panic!("engine: cannot remove the last oracle");
        }
        storage::remove_oracle_from_list(&e, &oracle);
        OracleRemovedEvent { oracle }.publish(&e);
    }

    /// Returns the full roster of registered oracles.
    ///
    /// # Returns
    ///
    /// A vector of all registered oracle addresses.
    pub fn get_oracles(e: Env) -> soroban_sdk::Vec<Address> {
        storage::read_oracles(&e)
    }

    /// Returns true if `addr` is a registered oracle.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to check
    ///
    /// # Returns
    ///
    /// true if the address is a registered oracle, false otherwise.
    pub fn is_oracle(e: Env, addr: Address) -> bool {
        storage::is_registered_oracle(&e, &addr)
    }

    /// Sets the token contract address.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `new_token` - The new token contract address
    ///
    /// # Panics
    ///
    /// Panics if caller is not the admin.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn set_token(e: Env, caller: Address, new_token: Address) {
        caller.require_auth();
        require_admin(&e, &caller);
        storage::write_token(&e, &new_token);
    }

    /// Sets the registry contract address.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `new_registry` - The new registry contract address
    ///
    /// # Panics
    ///
    /// Panics if caller is not the admin.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn set_registry(e: Env, caller: Address, new_registry: Address) {
        caller.require_auth();
        require_admin(&e, &caller);
        storage::write_registry(&e, &new_registry);
    }

    /// Sets platform-wide reward bounds for all payouts.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `min_reward` - The minimum allowed reward amount (must be positive)
    /// * `max_reward` - The maximum allowed reward amount (must be >= min_reward)
    ///
    /// # Panics
    ///
    /// * Panics if caller is not the admin
    /// * Panics if `min_reward <= 0`
    /// * Panics if `max_reward < min_reward`
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn set_reward_range(e: Env, caller: Address, min_reward: i128, max_reward: i128) {
        caller.require_auth();
        require_admin(&e, &caller);
        if min_reward <= 0 {
            panic!("engine: min reward must be positive");
        }
        if max_reward < min_reward {
            panic!("engine: max reward must be >= min reward");
        }
        storage::write_reward_range(&e, min_reward, max_reward);
    }

    /// Pauses the contract, blocking all proof operations.
    ///
    /// This is an emergency function to halt operations in case of an exploit.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    ///
    /// # Panics
    ///
    ///     /// Sets the minimum number of ledgers a user must wait between reward
    /// approvals. 0 disables the cooldown.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn set_user_cooldown(e: Env, caller: Address, min_ledgers_between_rewards: u64) {
        caller.require_auth();
        require_admin(&e, &caller);
        storage::write_user_cooldown(&e, min_ledgers_between_rewards);
    }

    /// Panics if caller is not the admin.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn pause(e: Env, caller: Address) {
        caller.require_auth();
        require_admin(&e, &caller);
        storage::set_paused(&e, true);
    }

    /// Resumes contract operations after a pause.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    ///
    /// # Panics
    ///
    /// Panics if caller is not the admin.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn unpause(e: Env, caller: Address) {
        caller.require_auth();
        require_admin(&e, &caller);
        storage::set_paused(&e, false);
    }

    /// Checks if the contract is currently paused.
    ///
    /// # Returns
    ///
    /// true if the contract is paused, false otherwise.
    pub fn is_paused(e: Env) -> bool {
        storage::is_paused(&e)
    }

    /// Submits a proof of task completion for verification.
    ///
    /// This records a user's proof CID for a specific task. The proof must be
    /// verified by an oracle before any reward is paid.
    ///
    /// # Arguments
    ///
    /// * `oracle` - The oracle address submitting the proof (must authorize and be registered)
    /// * `user` - The user address that completed the task
    /// * `task_id` - The ID of the task that was completed
    /// * `proof_cid` - The IPFS CID string of the proof
    ///
    /// # Panics
    ///
    /// * Panics if the contract is paused
    /// * Panics if oracle is not authorized
    /// * Panics if oracle is not a registered oracle
    /// * Panics if a verification already exists for this (task_id, user) pair
    /// * Panics if the proof CID has already been submitted
    ///
    /// # Auth
    ///
    /// Requires authentication from a registered oracle address.
    pub fn submit_proof(e: Env, oracle: Address, user: Address, task_id: u64, proof_cid: String) {
        require_not_paused(&e);
        oracle.require_auth();
        require_oracle(&e, &oracle);

        if proof_cid.is_empty() {
            panic!("engine: proof cid must not be empty");
        }
        if proof_cid.len() > MAX_CID_LEN {
            panic!("engine: proof cid too long");
        }

        if storage::read_verification(&e, task_id, &user).is_some() {
            panic!("engine: proof already submitted");
        }

        let cid_hash = e.crypto().sha256(&proof_cid.to_bytes()).to_bytes();
        if storage::read_cid_index(&e, &cid_hash).is_some() {
            panic!("engine: proof cid already submitted");
        }

        let verification = Verification {
            task_id,
            user: user.clone(),
            proof_cid,
            reward_amount: 0,
            status: VerificationStatus::Pending,
            submitted_at: e.ledger().timestamp(),
            resolved_at: None,
            oracle: oracle.clone(),
            seq: storage::next_verification_seq(&e),
        };

        storage::write_verification(&e, task_id, &user, &verification);
        storage::write_cid_index(
            &e,
            &cid_hash,
            &storage::VerificationKey {
                task_id,
                user: user.clone(),
            },
        );
        storage::push_verification_key(&e, task_id, &user);
        storage::push_user_verification_key(&e, &user, task_id);

        ProofSubmittedEvent {
            oracle,
            user,
            task_id,
        }
        .publish(&e);
    }

    /// Approves a proof and triggers reward payout.
    ///
    /// This validates the proof, marks it as approved, calls the registry to
    /// record the task completion, and mints the reward tokens to the user.
    ///
    /// # Arguments
    ///
    /// * `oracle` - The oracle address approving the proof (must authorize and be registered)
    /// * `user` - The user address that completed the task
    /// * `task_id` - The ID of the task that was completed
    /// * `reward_amount` - The amount of ECO tokens to reward (must be positive)
    ///
    /// # Panics
    ///
    /// * Panics if the contract is paused
    /// * Panics if oracle is not authorized
    /// * Panics if oracle is not a registered oracle
    /// * Panics if no verification exists for this (task_id, user) pair
    /// * Panics if the verification is not in Pending status
    /// * Panics if `reward_amount <= 0`
    /// * Panics if `reward_amount` is below the minimum reward bound (if set)
    /// * Panics if `reward_amount` exceeds the maximum reward bound (if set)
    /// * Panics if the task is not active or has expired (via require_active_task)
    /// * Panics if `reward_amount` exceeds the task's declared reward budget
    /// * Panics if the task has reached its max completions
    ///
    /// # Auth
    ///
    /// Requires authentication from a registered oracle address.
    pub fn approve_proof(
        e: Env,
        oracle: Address,
        user: Address,
        task_id: u64,
        reward_amount: i128,
    ) {
        require_not_paused(&e);
        oracle.require_auth();
        require_oracle(&e, &oracle);

        let mut verification = match storage::read_verification(&e, task_id, &user) {
            Some(v) => v,
            None => panic!("engine: verification not found"),
        };

        if verification.status != VerificationStatus::Pending {
            panic!("engine: verification is not pending");
        }

        // See `approve_and_pay` for the ordering that keeps this payout
        // atomic: all validation runs first, then the cross-contract calls
        // (complete_task, mint), and only then is the verification marked
        // Approved locally. A panic in any sub-call therefore leaves no
        // orphaned Approved record, consumed completion slot, or total_paid
        // drift.
        approve_and_pay(&e, &user, task_id, reward_amount, &mut verification);

        RewardPaidEvent {
            oracle,
            user: user.clone(),
            task_id,
            amount: reward_amount,
        }
        .publish(&e);
    }

    /// Rejects a proof without payout.
    ///
    /// This marks a verification as rejected, meaning the user will not receive
    /// any reward for this proof submission.
    ///
    /// # Arguments
    ///
    /// * `oracle` - The oracle address rejecting the proof (must authorize and be registered)
    /// * `user` - The user address that submitted the proof
    /// * `task_id` - The ID of the task
    ///
    /// # Panics
    ///
    /// * Panics if the contract is paused
    /// * Panics if oracle is not authorized
    /// * Panics if oracle is not a registered oracle
    /// * Panics if no verification exists for this (task_id, user) pair
    /// * Panics if the verification is not in Pending status
    ///
    /// # Auth
    ///
    /// Requires authentication from a registered oracle address.
    pub fn reject_proof(e: Env, oracle: Address, user: Address, task_id: u64) {
        require_not_paused(&e);
        oracle.require_auth();
        require_oracle(&e, &oracle);

        let mut verification = match storage::read_verification(&e, task_id, &user) {
            Some(v) => v,
            None => panic!("engine: verification not found"),
        };

        if verification.status != VerificationStatus::Pending {
            panic!("engine: verification is not pending");
        }

        verification.status = VerificationStatus::Rejected;
        verification.resolved_at = Some(e.ledger().timestamp());
        storage::write_verification(&e, task_id, &user, &verification);
        storage::remove_verification_key(&e, task_id, &user);

        ProofRejectedEvent {
            oracle,
            user,
            task_id,
        }
        .publish(&e);
    }

    /// Escalates a pending or rejected proof to dispute status.
    ///
    /// Disputed proofs require admin resolution before any payout can occur.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `user` - The user address that submitted the proof
    /// * `task_id` - The ID of the task
    ///
    /// # Panics
    ///
    /// * Panics if the contract is paused
    /// * Panics if caller is not the admin
    /// * Panics if no verification exists for this (task_id, user) pair
    /// * Panics if the verification is not in Pending or Rejected status
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn dispute_proof(e: Env, caller: Address, user: Address, task_id: u64) {
        require_not_paused(&e);
        caller.require_auth();
        require_admin(&e, &caller);

        let mut verification = match storage::read_verification(&e, task_id, &user) {
            Some(v) => v,
            None => panic!("engine: verification not found"),
        };

        if verification.status != VerificationStatus::Pending
            && verification.status != VerificationStatus::Rejected
        {
            panic!("engine: verification is not disputable");
        }

        verification.status = VerificationStatus::Disputed;
        storage::write_verification(&e, task_id, &user, &verification);
        storage::remove_verification_key(&e, task_id, &user);

        DisputeRaisedEvent { user, task_id }.publish(&e);
    }

    /// Resolves a disputed proof, either approving with payout or rejecting.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `user` - The user address that submitted the proof
    /// * `task_id` - The ID of the task
    /// * `approve` - true to approve and pay, false to reject
    /// * `reward_amount` - The reward amount if approving (must be positive)
    ///
    /// # Panics
    ///
    /// * Panics if the contract is paused
    /// * Panics if caller is not the admin
    /// * Panics if no verification exists for this (task_id, user) pair
    /// * Panics if the verification is not in Disputed status
    /// * Panics if `approve` is true and `reward_amount <= 0`
    /// * Panics if `approve` is true and `reward_amount` is below the minimum (if set)
    /// * Panics if `approve` is true and `reward_amount` exceeds the maximum (if set)
    /// * Panics if `approve` is true and the task is not active or has expired
    /// * Panics if `approve` is true and `reward_amount` exceeds the task's budget
    /// * Panics if `approve` is true and the task has reached its max completions
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn resolve_dispute(
        e: Env,
        caller: Address,
        user: Address,
        task_id: u64,
        approve: bool,
        reward_amount: i128,
    ) {
        require_not_paused(&e);
        caller.require_auth();
        require_admin(&e, &caller);

        let mut verification = match storage::read_verification(&e, task_id, &user) {
            Some(v) => v,
            None => panic!("engine: verification not found"),
        };

        if verification.status != VerificationStatus::Disputed {
            panic!("engine: verification is not disputed");
        }

        if approve {
            // Same atomic ordering as `approve_proof`: validation, then the
            // cross-contract calls, then the local Approved write (see
            // `approve_and_pay`). A mint failure here leaves the verification
            // Disputed — not Approved — with no completed task or payout.
            approve_and_pay(&e, &user, task_id, reward_amount, &mut verification);
        } else {
            verification.status = VerificationStatus::Rejected;
            verification.resolved_at = Some(e.ledger().timestamp());
            storage::write_verification(&e, task_id, &user, &verification);
        }

        DisputeResolvedEvent {
            user,
            task_id,
            approved: approve,
            reward_amount: if approve { reward_amount } else { 0 },
        }
        .publish(&e);
    }

    /// Retrieves a verification record for a specific user and task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task
    /// * `user` - The user address
    ///
    /// # Returns
    ///
    /// The Verification struct for the requested (task_id, user) pair.
    ///
    /// # Panics
    ///
    /// Panics if no verification exists for this pair.
    pub fn get_verification(e: Env, task_id: u64, user: Address) -> Verification {
        match storage::read_verification(&e, task_id, &user) {
            Some(v) => v,
            None => panic!("engine: verification not found"),
        }
    }

    /// Retrieves a verification record by its proof CID hash.
    ///
    /// # Arguments
    ///
    /// * `cid_hash` - The SHA-256 hash of the proof CID
    ///
    /// # Returns
    ///
    /// The Verification struct for the proof with the given CID hash.
    ///
    /// # Panics
    ///
    /// Panics if no verification exists for this CID hash.
    pub fn get_verification_by_cid_hash(e: Env, cid_hash: BytesN<32>) -> Verification {
        let key = match storage::read_cid_index(&e, &cid_hash) {
            Some(key) => key,
            None => panic!("engine: not found"),
        };
        match storage::read_verification(&e, key.task_id, &key.user) {
            Some(verification) => verification,
            None => panic!("engine: not found"),
        }
    }

    /// Returns up to `limit` pending verifications whose sequence number is
    /// greater than `cursor`.
    ///
    /// Every verification carries an immutable, monotonically-increasing
    /// `seq` assigned at submit time. `cursor` is the sequence number of the
    /// last verification already returned (pass 0 for the first page); fetch
    /// the next page with the `seq` of the last returned verification.
    /// Because `seq` values are immutable and unique, resolving an entry
    /// between pages can never shift or reorder the remaining entries — each
    /// pending verification is returned exactly once across the whole
    /// pagination, even when approvals, rejections, or disputes happen
    /// mid-pagination.
    ///
    /// # Migration note (breaking API change)
    ///
    /// `cursor` was previously a zero-based *offset* into the pending-only
    /// set. An offset cursor is unstable: when an entry is resolved it is
    /// removed from the pending list, every later entry shifts left by one,
    /// and a caller resuming from a saved offset silently skips an entry.
    /// The cursor is now a sequence number. Persisted offset cursors are
    /// invalid and must be re-anchored — restart from 0, or read `seq` off
    /// the last record already processed.
    ///
    /// # Arguments
    ///
    /// * `cursor` - The sequence number of the last verification already
    ///   returned; 0 returns from the beginning
    /// * `limit` - The maximum number of verifications to return
    ///
    /// # Returns
    ///
    /// A vector of pending verification records with `seq > cursor`.
    pub fn get_pending_verifications_paged(
        e: Env,
        cursor: u64,
        limit: u32,
    ) -> soroban_sdk::Vec<Verification> {
        collect_pending(&e, cursor, limit)
    }

    /// Returns all pending verifications.
    ///
    /// # Returns
    ///
    /// A vector of all pending verification records.
    pub fn get_pending_verifications(e: Env) -> soroban_sdk::Vec<Verification> {
        collect_pending(&e, 0, u32::MAX)
    }

    /// Pageable history of a single user's verifications across all tasks,
    /// ordered by submission. Reads only the requested page of the user's
    /// sequence index — never the user's full history.
    ///
    /// # Arguments
    ///
    /// * `user` - The user address to query
    /// * `cursor` - The starting offset (zero-based)
    /// * `limit` - The maximum number of verifications to return
    ///
    /// # Returns
    ///
    /// A vector of verification records for the specified user.
    pub fn get_verifications_by_user(
        e: Env,
        user: Address,
        cursor: u32,
        limit: u32,
    ) -> soroban_sdk::Vec<Verification> {
        let count = storage::read_user_verification_count(&e, &user);
        let start = (cursor as u64).min(count);
        let end = start.saturating_add(limit as u64).min(count);
        let mut result: soroban_sdk::Vec<Verification> = soroban_sdk::Vec::new(&e);
        let mut seq = start;
        while seq < end {
            if let Some(task_id) = storage::read_user_verification_task(&e, &user, seq) {
                if let Some(v) = storage::read_verification(&e, task_id, &user) {
                    result.push_back(v);
                }
            }
            seq += 1;
        }
        result
    }

    /// Returns the total amount of ECO tokens paid out by this engine.
    ///
    /// # Returns
    ///
    /// The cumulative sum of all approved rewards as an i128.
    pub fn total_paid(e: Env) -> i128 {
        storage::read_total_paid(&e)
    }

    /// Transfers the admin role to a new address.
    ///
    /// # Arguments
    ///
    /// * `current_admin` - The current admin address (must authorize)
    /// * `new_admin` - The new admin address to transfer control to
    ///
    /// # Panics
    ///
    /// * Panics if `current_admin` is not the stored admin
    /// * Panics if `new_admin == current_admin`
    ///
    /// # Auth
    ///
    /// Requires authentication from the current admin address.
    pub fn propose_admin(e: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();
        require_admin(&e, &current_admin);
        if new_admin == current_admin {
            panic!("engine: new admin must be different");
        }
        storage::write_pending_admin(&e, &new_admin);
        AdminProposedEvent {
            current_admin,
            proposed_admin: new_admin,
        }
        .publish(&e);
    }

    pub fn accept_admin(e: Env, pending_admin: Address) {
        pending_admin.require_auth();
        let proposed =
            storage::read_pending_admin(&e).unwrap_or_else(|| panic!("engine: no pending admin"));
        if pending_admin != proposed {
            panic!("engine: unauthorized pending admin");
        }
        storage::write_admin(&e, &pending_admin);
        storage::remove_pending_admin(&e);
        AdminAcceptedEvent {
            new_admin: pending_admin,
        }
        .publish(&e);
    }

    // DEPRECATED: use propose_admin. This alias preserves the existing ABI while
    // requiring the proposed admin to accept before gaining control.
    pub fn transfer_admin(e: Env, current_admin: Address, new_admin: Address) {
        Self::propose_admin(e, current_admin, new_admin);
    }
}

#[cfg(test)]
mod test {
    use crate::{RewardEngine, RewardEngineClient, VerificationStatus};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::BytesN;
    use soroban_sdk::testutils::Ledger as _;
    use soroban_sdk::{Address, Env, String};

    fn deploy_token(e: &Env, admin: &Address) -> Address {
        let token_id = e.register(eco_token::TokenContract, ());
        let token_client = eco_token::TokenContractClient::new(e, &token_id);
        token_client.initialize(
            admin,
            &String::from_str(e, "ECO"),
            &String::from_str(e, "ECO"),
            &7,
        );
        token_id
    }

    fn deploy_registry(e: &Env, admin: &Address) -> Address {
        let reg_id = e.register(task_registry::RegistryContract, ());
        let reg_client = task_registry::RegistryContractClient::new(e, &reg_id);
        reg_client.initialize(admin);
        reg_id
    }

    fn setup() -> (
        Env,
        Address,
        Address,
        Address,
        u64,
        RewardEngineClient<'static>,
    ) {
        let e = Env::default();
        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let user = Address::generate(&e);

        let token_id = deploy_token(&e, &admin);
        let reg_id = deploy_registry(&e, &admin);

        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        e.mock_all_auths_allowing_non_root_auth();
        let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
        reg_client.add_sponsor(&admin, &engine_id);

        engine_client.initialize(&admin, &token_id, &reg_id, &oracle);

        let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
        let task_id = reg_client.create_task(
            &admin,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 10000),
        );

        (e, admin, oracle, user, task_id, engine_client)
    }

    fn setup_two_tasks() -> (
        Env,
        Address,
        Address,
        Address,
        u64,
        u64,
        RewardEngineClient<'static>,
    ) {
        let e = Env::default();
        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let first_user = Address::generate(&e);
        let second_user = Address::generate(&e);
        let token_id = deploy_token(&e, &admin);
        let reg_id = deploy_registry(&e, &admin);
        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        e.mock_all_auths_allowing_non_root_auth();
        let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
        reg_client.add_sponsor(&admin, &engine_id);
        engine_client.initialize(&admin, &token_id, &reg_id, &oracle);

        let expires_at = e.ledger().timestamp() + 10000;
        let first_task = reg_client.create_task(
            &admin,
            &String::from_str(&e, "first-task"),
            &soroban_sdk::BytesN::<32>::random(&e),
            &1000,
            &1,
            &expires_at,
        );
        let second_task = reg_client.create_task(
            &admin,
            &String::from_str(&e, "second-task"),
            &soroban_sdk::BytesN::<32>::random(&e),
            &1000,
            &1,
            &expires_at,
        );

        (
            e,
            oracle,
            first_user,
            second_user,
            first_task,
            second_task,
            engine_client,
        )
    }

    #[test]
    fn test_lookup_by_cid_hash() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        let proof_cid = String::from_str(&e, "QmLookupProof");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);

        let cid_hash = e.crypto().sha256(&proof_cid.to_bytes()).to_bytes();
        let verification = client.get_verification_by_cid_hash(&cid_hash);

        assert_eq!(verification.task_id, task_id);
        assert_eq!(verification.user, user);
        assert_eq!(verification.proof_cid, proof_cid);
    }

    #[test]
    fn test_duplicate_cid_across_tasks_rejected() {
        let (e, oracle, first_user, second_user, first_task, second_task, client) =
            setup_two_tasks();
        let proof_cid = String::from_str(&e, "QmDuplicateProof");
        client.submit_proof(&oracle, &first_user, &first_task, &proof_cid);

        let result = client.try_submit_proof(&oracle, &second_user, &second_task, &proof_cid);
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_and_approve() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmTest123");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);

        client.approve_proof(&oracle, &user, &task_id, &1000);

        let verification = client.get_verification(&task_id, &user);
        assert_eq!(verification.status, VerificationStatus::Approved);
        assert_eq!(verification.reward_amount, 1000);
        assert!(verification.resolved_at.is_some());
    }

    #[test]
    fn test_submit_and_reject() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmTest456");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);

        client.reject_proof(&oracle, &user, &task_id);

        let verification = client.get_verification(&task_id, &user);
        assert_eq!(verification.status, VerificationStatus::Rejected);
        assert!(verification.resolved_at.is_some());
    }

    #[test]
    #[should_panic(expected = "engine: oracle must be different from admin")]
    fn test_initialize_oracle_same_as_admin() {
        let e = Env::default();
        let admin = Address::generate(&e);
        let token = Address::generate(&e);
        let registry = Address::generate(&e);

        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        engine_client.initialize(&admin, &token, &registry, &admin);
    }

    #[test]
    #[should_panic(expected = "engine: unauthorized")]
    fn test_unauthorized_oracle_cannot_submit() {
        let (e, _admin, _oracle, user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let fake_oracle = Address::generate(&e);
        let proof_cid = String::from_str(&e, "QmBad");
        client.submit_proof(&fake_oracle, &user, &1, &proof_cid);
    }

    #[test]
    #[should_panic(expected = "engine: verification is not pending")]
    fn test_double_approve_fails() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmTest789");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.approve_proof(&oracle, &user, &task_id, &1000);
        client.approve_proof(&oracle, &user, &task_id, &1000);
    }

    #[test]
    fn test_dispute_flow() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmDispute");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);

        client.dispute_proof(&admin, &user, &task_id);

        let verification = client.get_verification(&task_id, &user);
        assert_eq!(verification.status, VerificationStatus::Disputed);
    }

    #[test]
    fn test_resolve_dispute_approve() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmResDispute");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.dispute_proof(&admin, &user, &task_id);

        client.resolve_dispute(&admin, &user, &task_id, &true, &1000);

        let verification = client.get_verification(&task_id, &user);
        assert_eq!(verification.status, VerificationStatus::Approved);
        assert_eq!(verification.reward_amount, 1000);
    }

    #[test]
    fn test_resolve_dispute_reject() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmResReject");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.dispute_proof(&admin, &user, &task_id);

        client.resolve_dispute(&admin, &user, &task_id, &false, &0);

        let verification = client.get_verification(&task_id, &user);
        assert_eq!(verification.status, VerificationStatus::Rejected);
    }

    #[test]
    #[should_panic(expected = "engine: verification is not disputed")]
    fn test_resolve_non_disputed_fails() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmNotDisputed");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);

        client.resolve_dispute(&admin, &user, &task_id, &true, &1000);
    }

    #[test]
    #[should_panic(expected = "engine: verification is not disputable")]
    fn test_dispute_approved_fails() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmApproved");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.approve_proof(&oracle, &user, &task_id, &500);

        client.dispute_proof(&admin, &user, &task_id);
    }

    #[test]
    #[should_panic(expected = "engine: verification is not disputable")]
    fn test_dispute_already_disputed_fails() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmAlreadyDisputed");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.dispute_proof(&admin, &user, &task_id);

        client.dispute_proof(&admin, &user, &task_id);
    }

    #[test]
    fn test_dispute_rejected_succeeds() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmRejectedDispute");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.reject_proof(&oracle, &user, &task_id);

        let verification = client.get_verification(&task_id, &user);
        assert_eq!(verification.status, VerificationStatus::Rejected);

        client.dispute_proof(&admin, &user, &task_id);

        let verification = client.get_verification(&task_id, &user);
        assert_eq!(verification.status, VerificationStatus::Disputed);
    }

    #[test]
    fn test_full_integration() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let user = Address::generate(&e);

        let token_id = deploy_token(&e, &admin);
        let reg_id = deploy_registry(&e, &admin);

        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
        reg_client.add_sponsor(&admin, &engine_id);

        engine_client.initialize(&admin, &token_id, &reg_id, &oracle);
        let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
        let task_id = reg_client.create_task(
            &admin,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 10000),
        );

        let proof_cid = String::from_str(&e, "QmIntegration");
        engine_client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        engine_client.approve_proof(&oracle, &user, &task_id, &1000);

        let token_client = eco_token::TokenContractClient::new(&e, &token_id);
        assert_eq!(token_client.balance(&user), 1000);

        assert!(reg_client.is_task_completed(&task_id, &user));
    }

    #[test]
    fn test_approve_at_exact_expiry_boundary() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let user = Address::generate(&e);

        let token_id = deploy_token(&e, &admin);
        let reg_id = deploy_registry(&e, &admin);

        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
        reg_client.add_sponsor(&admin, &engine_id);

        engine_client.initialize(&admin, &token_id, &reg_id, &oracle);

        // Create task with expiry at exactly timestamp 2000.
        e.ledger().set_timestamp(1000);
        let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
        let task_id = reg_client.create_task(
            &admin,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &2000,
        );

        // At expires_at == 2000 the task is still live (< semantics).
        e.ledger().set_timestamp(2000);
        let proof_cid = String::from_str(&e, "QmBoundary");
        engine_client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        engine_client.approve_proof(&oracle, &user, &task_id, &1000);

        let token_client = eco_token::TokenContractClient::new(&e, &token_id);
        assert_eq!(token_client.balance(&user), 1000);
        assert!(reg_client.is_task_completed(&task_id, &user));

        // One second later the task is expired.
        e.ledger().set_timestamp(2001);
        let user2 = Address::generate(&e);
        let proof_cid2 = String::from_str(&e, "QmAfterExpiry");
        engine_client.submit_proof(&oracle, &user2, &task_id, &proof_cid2);
        let result = engine_client.try_approve_proof(&oracle, &user2, &task_id, &1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_reward_range_enforced() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        // Set allowed range: 500 - 2000
        client.set_reward_range(&admin, &500, &2000);

        let proof_cid = String::from_str(&e, "QmRangeOk");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        // 1000 is within range - should succeed
        client.approve_proof(&oracle, &user, &task_id, &1000);

        let v = client.get_verification(&task_id, &user);
        assert_eq!(v.reward_amount, 1000);
    }

    #[test]
    #[should_panic(expected = "engine: reward below minimum")]
    fn test_reward_below_minimum() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.set_reward_range(&admin, &500, &2000);

        let proof_cid = String::from_str(&e, "QmTooLow");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.approve_proof(&oracle, &user, &task_id, &100);
    }

    #[test]
    #[should_panic(expected = "engine: reward exceeds maximum")]
    fn test_reward_above_maximum() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.set_reward_range(&admin, &500, &2000);

        let proof_cid = String::from_str(&e, "QmTooHigh");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.approve_proof(&oracle, &user, &task_id, &9999);
    }

    #[test]
    #[should_panic(expected = "engine: max reward must be >= min reward")]
    fn test_set_invalid_reward_range() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.set_reward_range(&admin, &2000, &500);
    }

    #[test]
    fn test_get_pending_verifications() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmPending1");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);

        let pending = client.get_pending_verifications();
        assert_eq!(pending.len(), 1);

        client.approve_proof(&oracle, &user, &task_id, &1000);

        let pending = client.get_pending_verifications();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_get_pending_verifications_multiple() {
        let (e, _admin, oracle, user1, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let user2 = Address::generate(&e);

        let proof1 = String::from_str(&e, "QmPend1");
        client.submit_proof(&oracle, &user1, &task_id, &proof1);

        let proof2 = String::from_str(&e, "QmPend2");
        client.submit_proof(&oracle, &user2, &task_id, &proof2);

        let pending = client.get_pending_verifications();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_resolved_not_in_pending_list() {
        let (e, admin, oracle, user1, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let user2 = Address::generate(&e);
        let user3 = Address::generate(&e);

        let proof1 = String::from_str(&e, "QmRes1");
        client.submit_proof(&oracle, &user1, &task_id, &proof1);
        assert_eq!(client.get_pending_verifications().len(), 1);

        client.approve_proof(&oracle, &user1, &task_id, &1000);
        assert_eq!(client.get_pending_verifications().len(), 0);

        let proof2 = String::from_str(&e, "QmRes2");
        client.submit_proof(&oracle, &user2, &task_id, &proof2);
        assert_eq!(client.get_pending_verifications().len(), 1);

        client.reject_proof(&oracle, &user2, &task_id);
        assert_eq!(client.get_pending_verifications().len(), 0);

        let proof3 = String::from_str(&e, "QmRes3");
        client.submit_proof(&oracle, &user3, &task_id, &proof3);
        assert_eq!(client.get_pending_verifications().len(), 1);

        client.dispute_proof(&admin, &user3, &task_id);
        assert_eq!(client.get_pending_verifications().len(), 0);
    }

    #[test]
    fn test_transfer_admin() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let new_admin = Address::generate(&e);
        client.transfer_admin(&admin, &new_admin);
        client.accept_admin(&new_admin);

        let new_oracle = Address::generate(&e);
        client.set_oracle(&new_admin, &new_oracle);
    }

    #[test]
    fn test_propose_admin_overwrite() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();
        let first = Address::generate(&e);
        let second = Address::generate(&e);
        client.propose_admin(&admin, &first);
        client.propose_admin(&admin, &second);
        client.accept_admin(&second);
        let new_oracle = Address::generate(&e);
        client.set_oracle(&second, &new_oracle);
    }

    #[test]
    #[should_panic(expected = "engine: unauthorized pending admin")]
    fn test_accept_admin_wrong_address() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();
        client.propose_admin(&admin, &Address::generate(&e));
        client.accept_admin(&Address::generate(&e));
    }

    #[test]
    #[should_panic(expected = "engine: no pending admin")]
    fn test_accept_admin_without_proposal() {
        let (e, _admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();
        client.accept_admin(&Address::generate(&e));
    }

    #[test]
    #[should_panic(expected = "engine: unauthorized")]
    fn test_transfer_admin_unauthorized() {
        let (e, _admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let attacker = Address::generate(&e);
        let new_admin = Address::generate(&e);
        client.transfer_admin(&attacker, &new_admin);
    }

    #[test]
    #[should_panic(expected = "engine: new admin must be different")]
    fn test_transfer_admin_same_address() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.transfer_admin(&admin, &admin);
    }

    #[test]
    fn test_set_token() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let new_token = Address::generate(&e);
        client.set_token(&admin, &new_token);
    }

    #[test]
    #[should_panic(expected = "engine: unauthorized")]
    fn test_set_token_unauthorized() {
        let (e, _admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let attacker = Address::generate(&e);
        let new_token = Address::generate(&e);
        client.set_token(&attacker, &new_token);
    }

    #[test]
    fn test_set_registry() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let new_registry = Address::generate(&e);
        client.set_registry(&admin, &new_registry);
    }

    #[test]
    #[should_panic(expected = "engine: unauthorized")]
    fn test_set_registry_unauthorized() {
        let (e, _admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let attacker = Address::generate(&e);
        let new_registry = Address::generate(&e);
        client.set_registry(&attacker, &new_registry);
    }

    #[test]
    #[should_panic(expected = "engine: reward exceeds task budget")]
    fn test_approve_exceeds_task_budget() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmBudget");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.approve_proof(&oracle, &user, &task_id, &5000);
    }

    #[test]
    #[should_panic(expected = "engine: task has expired")]
    fn test_approve_expired_task_fails() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let user = Address::generate(&e);

        let token_id = deploy_token(&e, &admin);
        let reg_id = deploy_registry(&e, &admin);

        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
        reg_client.add_sponsor(&admin, &engine_id);

        engine_client.initialize(&admin, &token_id, &reg_id, &oracle);

        let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
        let task_id = reg_client.create_task(
            &admin,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 100),
        );

        e.ledger().set_timestamp(e.ledger().timestamp() + 200);

        let proof_cid = String::from_str(&e, "QmExpired");
        engine_client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        engine_client.approve_proof(&oracle, &user, &task_id, &1000);
    }

    #[test]
    #[should_panic(expected = "engine: task is not active")]
    fn test_approve_cancelled_task_fails() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let user = Address::generate(&e);

        let token_id = deploy_token(&e, &admin);
        let reg_id = deploy_registry(&e, &admin);

        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
        reg_client.add_sponsor(&admin, &engine_id);

        engine_client.initialize(&admin, &token_id, &reg_id, &oracle);

        let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
        let task_id = reg_client.create_task(
            &admin,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 10000),
        );

        reg_client.cancel_task(&admin, &task_id);

        let proof_cid = String::from_str(&e, "QmCancelled");
        engine_client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        engine_client.approve_proof(&oracle, &user, &task_id, &1000);
    }

    #[test]
    #[should_panic(expected = "engine: reward exceeds task budget")]
    fn test_resolve_dispute_approve_over_budget() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmOverBudget");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.dispute_proof(&admin, &user, &task_id);
        client.resolve_dispute(&admin, &user, &task_id, &true, &9999);
    }

    #[test]
    fn test_total_paid_after_approve() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        assert_eq!(client.total_paid(), 0);

        let proof_cid = String::from_str(&e, "QmTotal1");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.approve_proof(&oracle, &user, &task_id, &1000);

        assert_eq!(client.total_paid(), 1000);
    }

    #[test]
    fn test_total_paid_after_dispute_resolve() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        assert_eq!(client.total_paid(), 0);

        let proof_cid = String::from_str(&e, "QmTotalDispute");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.dispute_proof(&admin, &user, &task_id);
        client.resolve_dispute(&admin, &user, &task_id, &true, &1000);

        assert_eq!(client.total_paid(), 1000);
    }

    #[test]
    fn test_total_paid_unaffected_by_rejection() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmTotalRej");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.reject_proof(&oracle, &user, &task_id);

        assert_eq!(client.total_paid(), 0);
    }

    #[test]
    fn test_pause_blocks_submit_proof() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.pause(&admin);
        assert!(client.is_paused());

        let proof_cid = String::from_str(&e, "QmPauseSubmit");
        let result = client.try_submit_proof(&oracle, &user, &task_id, &proof_cid);
        assert!(result.is_err());
    }

    #[test]
    fn test_pause_blocks_approve_proof() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmPauseApprove");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);

        client.pause(&admin);
        let result = client.try_approve_proof(&oracle, &user, &task_id, &50);
        assert!(result.is_err());
    }

    #[test]
    fn test_pause_blocks_dispute_proof() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmPauseDispute");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);

        client.pause(&admin);
        let result = client.try_dispute_proof(&admin, &user, &task_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_pause_blocks_resolve_dispute() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "QmPauseResolve");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        client.dispute_proof(&admin, &user, &task_id);

        client.pause(&admin);
        let result = client.try_resolve_dispute(&admin, &user, &task_id, &true, &50);
        assert!(result.is_err());
    }

    #[test]
    fn test_unpause_allows_operations() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.pause(&admin);
        assert!(client.is_paused());

        client.unpause(&admin);
        assert!(!client.is_paused());

        let proof_cid = String::from_str(&e, "QmUnpause");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        let verification = client.get_verification(&task_id, &user);
        assert_eq!(verification.status, VerificationStatus::Pending);
    }

    #[test]
    fn test_pause_unpause_only_admin() {
        let (e, _admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let someone = Address::generate(&e);
        let result = client.try_pause(&someone);
        assert!(result.is_err());

        let result = client.try_unpause(&someone);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_oracle_and_submit() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let second_oracle = Address::generate(&e);
        client.add_oracle(&admin, &second_oracle);

        assert_eq!(client.get_oracles().len(), 2);
        assert!(client.is_oracle(&oracle));
        assert!(client.is_oracle(&second_oracle));

        // A second registered oracle can submit and approve independently.
        let proof_cid = String::from_str(&e, "QmSecondOracle");
        client.submit_proof(&second_oracle, &user, &task_id, &proof_cid);
        client.approve_proof(&second_oracle, &user, &task_id, &1000);

        let v = client.get_verification(&task_id, &user);
        assert_eq!(v.status, VerificationStatus::Approved);
        assert_eq!(v.oracle, second_oracle);
    }

    #[test]
    #[should_panic(expected = "engine: oracle already registered")]
    fn test_add_duplicate_oracle_fails() {
        let (e, admin, oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.add_oracle(&admin, &oracle);
    }

    #[test]
    #[should_panic(expected = "engine: oracle must be different from admin")]
    fn test_add_admin_as_oracle_fails() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.add_oracle(&admin, &admin);
    }

    #[test]
    #[should_panic(expected = "engine: unauthorized")]
    fn test_add_oracle_non_admin_fails() {
        let (e, _admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let attacker = Address::generate(&e);
        let second_oracle = Address::generate(&e);
        client.add_oracle(&attacker, &second_oracle);
    }

    #[test]
    fn test_remove_oracle_revokes_access() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let second_oracle = Address::generate(&e);
        client.add_oracle(&admin, &second_oracle);
        assert!(client.is_oracle(&second_oracle));

        client.remove_oracle(&admin, &second_oracle);
        assert!(!client.is_oracle(&second_oracle));
        assert_eq!(client.get_oracles().len(), 1);

        // The removed oracle can no longer submit proofs.
        let proof_cid = String::from_str(&e, "QmRemovedOracle");
        let result = client.try_submit_proof(&second_oracle, &user, &task_id, &proof_cid);
        assert!(result.is_err());

        // The original oracle still works.
        let proof_cid = String::from_str(&e, "QmOriginal");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
    }

    #[test]
    #[should_panic(expected = "engine: cannot remove the last oracle")]
    fn test_remove_last_oracle_fails() {
        let (e, admin, oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        client.remove_oracle(&admin, &oracle);
    }

    #[test]
    #[should_panic(expected = "engine: oracle not registered")]
    fn test_remove_unregistered_oracle_fails() {
        let (e, admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let stranger = Address::generate(&e);
        client.remove_oracle(&admin, &stranger);
    }

    #[test]
    fn test_get_pending_verifications_paged() {
        let (e, _admin, oracle, user1, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let user2 = Address::generate(&e);
        let user3 = Address::generate(&e);

        let p1 = String::from_str(&e, "QmPage1");
        client.submit_proof(&oracle, &user1, &task_id, &p1);

        let p2 = String::from_str(&e, "QmPage2");
        client.submit_proof(&oracle, &user2, &task_id, &p2);

        let p3 = String::from_str(&e, "QmPage3");
        client.submit_proof(&oracle, &user3, &task_id, &p3);

        // First page of 2, starting from seq 0 (the beginning).
        let page0 = client.get_pending_verifications_paged(&0, &2);
        assert_eq!(page0.len(), 2);
        assert_eq!(page0.get(0).unwrap().seq, 1);
        assert_eq!(page0.get(1).unwrap().seq, 2);

        // Resume from the last returned seq (2) -> only seq 3 remains.
        let page1 = client.get_pending_verifications_paged(&2, &2);
        assert_eq!(page1.len(), 1);
        assert_eq!(page1.get(0).unwrap().seq, 3);

        // Cursor past the highest seq -> empty.
        let page2 = client.get_pending_verifications_paged(&10, &2);
        assert_eq!(page2.len(), 0);
    }

    #[test]
    fn test_get_pending_verifications_paged_skips_resolved() {
        let (e, _admin, oracle, user1, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let user2 = Address::generate(&e);

        let p1 = String::from_str(&e, "QmResolved1");
        client.submit_proof(&oracle, &user1, &task_id, &p1);
        client.approve_proof(&oracle, &user1, &task_id, &1000);

        let p2 = String::from_str(&e, "QmStillPending");
        client.submit_proof(&oracle, &user2, &task_id, &p2);

        // The resolved entry is skipped, only the pending one is returned.
        let page = client.get_pending_verifications_paged(&0, &10);
        assert_eq!(page.len(), 1);
        assert_eq!(page.get(0).unwrap().user, user2);
    }

    #[test]
    fn test_pending_list_removal_surgery_preserves_pagination() {
        let (e, _admin, oracle, user1, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let user2 = Address::generate(&e);
        let user3 = Address::generate(&e);
        let user4 = Address::generate(&e);

        let p1 = String::from_str(&e, "QmSurgery1");
        client.submit_proof(&oracle, &user1, &task_id, &p1);
        let p2 = String::from_str(&e, "QmSurgery2");
        client.submit_proof(&oracle, &user2, &task_id, &p2);
        let p3 = String::from_str(&e, "QmSurgery3");
        client.submit_proof(&oracle, &user3, &task_id, &p3);
        let p4 = String::from_str(&e, "QmSurgery4");
        client.submit_proof(&oracle, &user4, &task_id, &p4);

        // Remove a middle node (user2) and the tail node (user4) via
        // rejection; the remaining list [user1, user3] must stay connected
        // and in submission order.
        client.reject_proof(&oracle, &user2, &task_id);
        client.reject_proof(&oracle, &user4, &task_id);

        let pending = client.get_pending_verifications();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending.get(0).unwrap().user, user1);
        assert_eq!(pending.get(1).unwrap().user, user3);

        // Seq-based pagination into the pending-only set still works:
        // cursor 1 (last seen seq) skips user1 (seq 1) and returns user3
        // (seq 3).
        let page = client.get_pending_verifications_paged(&1, &5);
        assert_eq!(page.len(), 1);
        assert_eq!(page.get(0).unwrap().user, user3);
    }

    #[test]
    fn test_pagination_stable_across_resolutions() {
        let (e, admin, oracle, user1, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let users = [
            user1,
            Address::generate(&e),
            Address::generate(&e),
            Address::generate(&e),
            Address::generate(&e),
            Address::generate(&e),
        ];
        let cids = [
            "QmStable0",
            "QmStable1",
            "QmStable2",
            "QmStable3",
            "QmStable4",
            "QmStable5",
        ];

        // Submit 6 proofs; they receive seqs 1..=6 in submission order.
        for (i, user) in users.iter().enumerate() {
            let cid = String::from_str(&e, cids[i]);
            client.submit_proof(&oracle, user, &task_id, &cid);
        }

        // Page 0 returns seqs 1 and 2; resume from the last returned seq.
        let page0 = client.get_pending_verifications_paged(&0, &2);
        assert_eq!(page0.len(), 2);
        assert_eq!(page0.get(0).unwrap().seq, 1);
        assert_eq!(page0.get(1).unwrap().seq, 2);
        let mut cursor = page0.get(1).unwrap().seq;

        // Resolve two entries between pages: reject the middle one (seq 3)
        // and dispute the tail one (seq 6). A raw index cursor would shift
        // seqs 4 and 5 left and silently skip seq 4; the seq cursor must be
        // unaffected by either resolution.
        client.reject_proof(&oracle, &users[2], &task_id);
        client.dispute_proof(&admin, &users[5], &task_id);

        // Page 1 resumes at seq 2: it must return seqs 4 and 5 — exactly
        // once, with no skips caused by the mid-pagination resolutions.
        let page1 = client.get_pending_verifications_paged(&cursor, &2);
        assert_eq!(page1.len(), 2);
        assert_eq!(page1.get(0).unwrap().seq, 4);
        assert_eq!(page1.get(1).unwrap().seq, 5);
        cursor = page1.get(1).unwrap().seq;

        // Page 2: nothing left (seq 6 was disputed and removed from the
        // pending set).
        let page2 = client.get_pending_verifications_paged(&cursor, &2);
        assert_eq!(page2.len(), 0);

        // The unbounded view returns exactly the remaining pending set, in
        // submission order (seqs 1, 2, 4, 5 — the rejected and disputed
        // entries are gone).
        let pending = client.get_pending_verifications();
        assert_eq!(pending.len(), 4);
        assert_eq!(pending.get(0).unwrap().seq, 1);
        assert_eq!(pending.get(1).unwrap().seq, 2);
        assert_eq!(pending.get(2).unwrap().seq, 4);
        assert_eq!(pending.get(3).unwrap().seq, 5);
    }

    #[test]
    fn test_get_verifications_by_user() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let user = Address::generate(&e);

        let token_id = deploy_token(&e, &admin);
        let reg_id = deploy_registry(&e, &admin);

        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
        reg_client.add_sponsor(&admin, &engine_id);

        engine_client.initialize(&admin, &token_id, &reg_id, &oracle);

        let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
        let first_task = reg_client.create_task(
            &admin,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 10000),
        );
        let loc_hash2 = soroban_sdk::BytesN::<32>::random(&e);
        let second_task = reg_client.create_task(
            &admin,
            &String::from_str(&e, "recycling"),
            &loc_hash2,
            &500,
            &1,
            &(e.ledger().timestamp() + 10000),
        );

        let p1 = String::from_str(&e, "QmFirst");
        engine_client.submit_proof(&oracle, &user, &first_task, &p1);
        let p2 = String::from_str(&e, "QmSecond");
        engine_client.submit_proof(&oracle, &user, &second_task, &p2);

        let all = engine_client.get_verifications_by_user(&user, &0, &10);
        assert_eq!(all.len(), 2);
        assert_eq!(all.get(0).unwrap().task_id, first_task);
        assert_eq!(all.get(1).unwrap().task_id, second_task);

        let page0 = engine_client.get_verifications_by_user(&user, &0, &1);
        assert_eq!(page0.len(), 1);
        assert_eq!(page0.get(0).unwrap().task_id, first_task);

        let page1 = engine_client.get_verifications_by_user(&user, &1, &1);
        assert_eq!(page1.len(), 1);
        assert_eq!(page1.get(0).unwrap().task_id, second_task);

        let empty = engine_client.get_verifications_by_user(&user, &5, &2);
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_verifications_by_user_empty_for_stranger() {
        let (e, _admin, _oracle, _user, _task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let stranger = Address::generate(&e);
        let result = client.get_verifications_by_user(&stranger, &0, &10);
        assert_eq!(result.len(), 0);
    }

    /// Shared fixture for cooldown tests: one admin/oracle/user plus two
    /// separate tasks so the same user can be rewarded twice.
    fn setup_cooldown() -> (
        Env,
        Address,
        Address,
        Address,
        u64,
        u64,
        RewardEngineClient<'static>,
    ) {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&e);
        let oracle = Address::generate(&e);
        let user = Address::generate(&e);

        let token_id = deploy_token(&e, &admin);
        let reg_id = deploy_registry(&e, &admin);
        let engine_id = e.register(RewardEngine, ());
        let engine_client = RewardEngineClient::new(&e, &engine_id);

        let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
        reg_client.add_sponsor(&admin, &engine_id);
        engine_client.initialize(&admin, &token_id, &reg_id, &oracle);

        let expires_at = e.ledger().timestamp() + 10000;
        let task1 = reg_client.create_task(
            &admin,
            &String::from_str(&e, "cooldown-task-one"),
            &soroban_sdk::BytesN::<32>::random(&e),
            &1000,
            &1,
            &expires_at,
        );
        let task2 = reg_client.create_task(
            &admin,
            &String::from_str(&e, "cooldown-task-two"),
            &soroban_sdk::BytesN::<32>::random(&e),
            &1000,
            &1,
            &expires_at,
        );

        (e, admin, oracle, user, task1, task2, engine_client)
    }

    #[test]
    #[should_panic(expected = "engine: user cooldown active")]
    fn test_cooldown_blocks_rapid_claims() {
        let (e, admin, oracle, user, task1, task2, client) = setup_cooldown();

        client.set_user_cooldown(&admin, &10);

        let p1 = String::from_str(&e, "QmCooldownBlock1");
        client.submit_proof(&oracle, &user, &task1, &p1);
        client.approve_proof(&oracle, &user, &task1, &500);

        // Still within the cooldown window: this must panic.
        let p2 = String::from_str(&e, "QmCooldownBlock2");
        client.submit_proof(&oracle, &user, &task2, &p2);
        client.approve_proof(&oracle, &user, &task2, &500);
    }

    #[test]
    fn test_cooldown_resets_after_interval() {
        let (e, admin, oracle, user, task1, task2, client) = setup_cooldown();

        client.set_user_cooldown(&admin, &10);

        let p1 = String::from_str(&e, "QmCooldownReset1");
        client.submit_proof(&oracle, &user, &task1, &p1);
        client.approve_proof(&oracle, &user, &task1, &500);

        // Advance past the cooldown window before claiming again.
        let next = e.ledger().sequence() + 10;
        e.ledger().set_sequence_number(next);

        let p2 = String::from_str(&e, "QmCooldownReset2");
        client.submit_proof(&oracle, &user, &task2, &p2);
        client.approve_proof(&oracle, &user, &task2, &500);

        let v = client.get_verification(&task2, &user);
        assert_eq!(v.status, VerificationStatus::Approved);
    }

    #[test]
    fn test_cooldown_zero_disabled() {
        let (e, _admin, oracle, user, task1, task2, client) = setup_cooldown();

        // Default cooldown is 0 (disabled) — back-to-back rewards succeed.
        let p1 = String::from_str(&e, "QmCooldownDisabled1");
        client.submit_proof(&oracle, &user, &task1, &p1);
        client.approve_proof(&oracle, &user, &task1, &500);

        let p2 = String::from_str(&e, "QmCooldownDisabled2");
        client.submit_proof(&oracle, &user, &task2, &p2);
        client.approve_proof(&oracle, &user, &task2, &500);

        let v = client.get_verification(&task2, &user);
        assert_eq!(v.status, VerificationStatus::Approved);
    }

    #[test]
    #[should_panic(expected = "engine: proof cid must not be empty")]
    fn test_submit_empty_cid_fails() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        let proof_cid = String::from_str(&e, "");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
    }

    #[test]
    #[should_panic(expected = "engine: proof cid too long")]
    fn test_submit_oversized_cid_fails() {
        let (e, _admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        // 129 bytes exceeds MAX_CID_LEN (128)
        let long_cid = "a".repeat(129);
        let proof_cid = String::from_str(&e, &long_cid);
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
    }
}
