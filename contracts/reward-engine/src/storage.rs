use soroban_sdk::{contracttype, Address, BytesN, Env, String, Vec};

/// Persistent storage TTL: re-bump on every touch; 4,096 ledgers (~5.7 days)
/// provides ample headroom since entries are refreshed on every interaction.
pub const PERSISTENT_TTL_THRESHOLD: u32 = 100;
pub const PERSISTENT_TTL_EXTEND_TO: u32 = 4_096;

/// Instance-storage TTL management.
///
/// The engine's operational state — admin, token/registry addresses, oracle
/// roster, reward bounds, pause flag, cooldown, total paid, and the pending
/// list — all lives in instance storage. Soroban expires instance entries
/// after a short default TTL (~100 ledgers on mainnet, roughly 8 minutes at
/// ~5 s/ledger), and if the contract goes untouched for longer than that
/// every instance entry is evicted: `read_admin`, `read_token`, and
/// `read_registry` all `unwrap()` and the engine becomes permanently
/// non-operational.
///
/// Every public entry point calls `extend_instance_ttl` as its first
/// operation, so any interaction refreshes the clock. `extend_ttl` only
/// rewrites the entry when the remaining TTL is at or below the threshold,
/// so the steady-state cost is a single conditional storage check per call.
///
/// * `INSTANCE_TTL_THRESHOLD`: 100 ledgers (~8 min) — refresh only when the
///   entry is within this many ledgers of expiry (the default TTL), so an
///   active contract bumps roughly once per 100 ledgers instead of on every
///   call.
/// * `INSTANCE_TTL_EXTEND_TO`: 535,680 ledgers = 31 days at ~5 s/ledger
///   (17,280 ledgers/day). The engine must survive the longest realistic
///   quiet period (a holiday break or upstream outage) because expiry is
///   unrecoverable, and a longer target costs nothing extra: the threshold
///   governs how often the extension is actually written.
pub const INSTANCE_TTL_THRESHOLD: u32 = 100;
pub const INSTANCE_TTL_EXTEND_TO: u32 = 535_680;

/// Maximum allowed page size for paginated queries. Must stay well below the
/// Soroban per-transaction ledger-entry footprint limit (100 entries).
pub const MAX_PAGE_SIZE: u32 = 50;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum VerificationStatus {
    Pending,
    Approved,
    Rejected,
    Disputed,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Verification {
    pub task_id: u64,
    pub user: Address,
    pub proof_cid: String,
    pub reward_amount: i128,
    pub status: VerificationStatus,
    pub submitted_at: u64,
    pub resolved_at: Option<u64>,
    /// Records the oracle that submitted the proof (the submitter).
    /// Note: the oracle that approves or rejects the proof may be a different registered oracle,
    /// but this field will not be updated to the approver.
    pub oracle: Address,
    /// Monotonically-increasing sequence number assigned at submit time.
    /// Never changes once set, so it is a stable pagination cursor: resolving
    /// an entry can never shift another entry past a cursor.
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct VerificationKey {
    pub task_id: u64,
    pub user: Address,
}

/// In-memory aggregate of the pending-list instance state (count plus
/// head/tail pointers).
///
/// This type is never serialized: its three fields are stored as three
/// fixed-size instance entries, so the stored state stays constant-size
/// regardless of how many verifications exist. The global pending set is
/// a doubly-linked list so that `push_verification_key` /
/// `remove_verification_key` are O(1) pointer operations and pagination
/// walks only pending entries, never resolved ones. The per-verification
/// prev/next links live in persistent storage.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingListState {
    pub count: u64,
    pub head: Option<VerificationKey>,
    pub tail: Option<VerificationKey>,
}

#[derive(Clone, Debug)]
#[contracttype]
pub enum DataKey {
    Admin,
    Token,
    Registry,
    Oracles,
    Verification(u64, Address),
    CidHash(BytesN<32>),
    MinReward,
    MaxReward,
    PendingListCount,
    PendingListHead,
    PendingListTail,
    VerificationSeq,
    PendingVerificationPrev(VerificationKey),
    PendingVerificationNext(VerificationKey),
    UserVerificationCount(Address),
    UserVerificationIndex(Address, u64),
    TotalPaid,
    Paused,
    UserCooldown,
    LastRewardLedger(Address),
    PendingAdmin,
}

/// Writes the admin address to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `admin` - The admin address to store
pub fn write_admin(e: &Env, admin: &Address) {
    let key = DataKey::Admin;
    e.storage().instance().set(&key, admin);
}

/// Reads the admin address from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The stored admin address.
///
/// # Panics
///
/// Panics if no admin has been set.
pub fn read_admin(e: &Env) -> Address {
    let key = DataKey::Admin;
    e.storage().instance().get(&key).unwrap()
}

/// Checks if an admin address has been set.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// true if an admin exists, false otherwise.
pub fn has_admin(e: &Env) -> bool {
    let key = DataKey::Admin;
    e.storage().instance().has(&key)
}

pub fn write_pending_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&DataKey::PendingAdmin, admin);
}

pub fn read_pending_admin(e: &Env) -> Option<Address> {
    e.storage().instance().get(&DataKey::PendingAdmin)
}

pub fn remove_pending_admin(e: &Env) {
    e.storage().instance().remove(&DataKey::PendingAdmin);
}

/// Writes the token contract address to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `token` - The token contract address to store
pub fn write_token(e: &Env, token: &Address) {
    let key = DataKey::Token;
    e.storage().instance().set(&key, token);
}

/// Reads the token contract address from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The stored token contract address.
///
/// # Panics
///
/// Panics if no token has been set.
pub fn read_token(e: &Env) -> Address {
    let key = DataKey::Token;
    e.storage().instance().get(&key).unwrap()
}

/// Writes the registry contract address to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `registry` - The registry contract address to store
pub fn write_registry(e: &Env, registry: &Address) {
    let key = DataKey::Registry;
    e.storage().instance().set(&key, registry);
}

/// Reads the registry contract address from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The stored registry contract address.
///
/// # Panics
///
/// Panics if no registry has been set.
pub fn read_registry(e: &Env) -> Address {
    let key = DataKey::Registry;
    e.storage().instance().get(&key).unwrap()
}

/// Writes the list of oracle addresses to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `oracles` - The vector of oracle addresses to store
pub fn write_oracles(e: &Env, oracles: &Vec<Address>) {
    let key = DataKey::Oracles;
    e.storage().instance().set(&key, oracles);
}

/// Reads the list of oracle addresses from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The vector of registered oracle addresses, or an empty vector if none exist.
pub fn read_oracles(e: &Env) -> Vec<Address> {
    let key = DataKey::Oracles;
    e.storage().instance().get(&key).unwrap_or(Vec::new(e))
}

/// Adds an oracle address to the list of registered oracles.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `oracle` - The oracle address to add
pub fn push_oracle(e: &Env, oracle: &Address) {
    let key = DataKey::Oracles;
    let mut list: Vec<Address> = e.storage().instance().get(&key).unwrap_or(Vec::new(e));
    list.push_back(oracle.clone());
    e.storage().instance().set(&key, &list);
}

/// Removes an oracle address from the list of registered oracles.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `oracle` - The oracle address to remove
pub fn remove_oracle_from_list(e: &Env, oracle: &Address) {
    let key = DataKey::Oracles;
    let list: Vec<Address> = e.storage().instance().get(&key).unwrap_or(Vec::new(e));
    let mut filtered: Vec<Address> = Vec::new(e);
    for o in list.iter() {
        if o != *oracle {
            filtered.push_back(o);
        }
    }
    e.storage().instance().set(&key, &filtered);
}

/// Checks if an address is a registered oracle.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `addr` - The address to check
///
/// # Returns
///
/// true if the address is a registered oracle, false otherwise.
pub fn is_registered_oracle(e: &Env, addr: &Address) -> bool {
    let oracles = read_oracles(e);
    oracles.iter().any(|o| o == *addr)
}

/// Writes a verification record to persistent storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task being verified
/// * `user` - The user address that submitted the proof
/// * `v` - The verification record to store
pub fn write_verification(e: &Env, task_id: u64, user: &Address, v: &Verification) {
    let key = DataKey::Verification(task_id, user.clone());
    e.storage().persistent().set(&key, v);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
}

/// Reads a verification record from persistent storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task to query
/// * `user` - The user address to query
///
/// # Returns
///
/// The verification record if it exists, or None if no verification exists.
pub fn read_verification(e: &Env, task_id: u64, user: &Address) -> Option<Verification> {
    let key = DataKey::Verification(task_id, user.clone());
    e.storage().persistent().get(&key)
}

/// Writes a CID hash to verification key mapping to persistent storage.
///
/// This prevents duplicate proof submissions across different tasks.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `cid_hash` - The SHA-256 hash of the proof CID
/// * `verification_key` - The verification key (task_id, user) that this CID maps to
pub fn write_cid_index(e: &Env, cid_hash: &BytesN<32>, verification_key: &VerificationKey) {
    let key = DataKey::CidHash(cid_hash.clone());
    e.storage().persistent().set(&key, verification_key);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
}

/// Reads a verification key from the CID index.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `cid_hash` - The SHA-256 hash of the proof CID to look up
///
/// # Returns
///
/// The verification key if the CID has been indexed, or None if not found.
pub fn read_cid_index(e: &Env, cid_hash: &BytesN<32>) -> Option<VerificationKey> {
    let key = DataKey::CidHash(cid_hash.clone());
    e.storage().persistent().get(&key)
}

/// Writes the reward range bounds to instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `min` - The minimum allowed reward amount
/// * `max` - The maximum allowed reward amount
pub fn write_reward_range(e: &Env, min: i128, max: i128) {
    e.storage().instance().set(&DataKey::MinReward, &min);
    e.storage().instance().set(&DataKey::MaxReward, &max);
}

/// Reads the minimum reward bound from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The minimum reward if set, or None if no minimum has been configured.
pub fn read_min_reward(e: &Env) -> Option<i128> {
    e.storage().instance().get(&DataKey::MinReward)
}

/// Reads the maximum reward bound from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The maximum reward if set, or None if no maximum has been configured.
pub fn read_max_reward(e: &Env) -> Option<i128> {
    e.storage().instance().get(&DataKey::MaxReward)
}

/// Reads the fixed-size state of the pending-verification linked list
/// (count plus head/tail pointers) from instance storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The current pending-list state, or an empty state if none exists.
pub fn read_pending_list(e: &Env) -> PendingListState {
    PendingListState {
        count: e
            .storage()
            .instance()
            .get(&DataKey::PendingListCount)
            .unwrap_or(0),
        head: e
            .storage()
            .instance()
            .get(&DataKey::PendingListHead)
            .unwrap_or(None),
        tail: e
            .storage()
            .instance()
            .get(&DataKey::PendingListTail)
            .unwrap_or(None),
    }
}

/// Persists the fixed-size state of the pending-verification linked list.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `state` - The pending-list state to store
fn write_pending_list(e: &Env, state: &PendingListState) {
    e.storage()
        .instance()
        .set(&DataKey::PendingListCount, &state.count);
    e.storage()
        .instance()
        .set(&DataKey::PendingListHead, &state.head);
    e.storage()
        .instance()
        .set(&DataKey::PendingListTail, &state.tail);
}

/// Appends a verification key to the tail of the pending list.
///
/// This is a doubly-linked-list append: it touches only the tail node's
/// links and the fixed-size list state, so its cost is bounded to a
/// constant number of storage operations regardless of how many
/// verifications exist.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task
/// * `user` - The user address
pub fn push_verification_key(e: &Env, task_id: u64, user: &Address) {
    let key = VerificationKey {
        task_id,
        user: user.clone(),
    };
    let mut state = read_pending_list(e);
    match state.tail.clone() {
        None => {
            // First pending verification: it is both head and tail.
            let prev_key = DataKey::PendingVerificationPrev(key.clone());
            e.storage()
                .persistent()
                .set(&prev_key, &None::<VerificationKey>);
            e.storage().persistent().extend_ttl(
                &prev_key,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
            let next_key = DataKey::PendingVerificationNext(key.clone());
            e.storage()
                .persistent()
                .set(&next_key, &None::<VerificationKey>);
            e.storage().persistent().extend_ttl(
                &next_key,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
            state.head = Some(key.clone());
            state.tail = Some(key);
        }
        Some(tail_key) => {
            // Append after the current tail: point the old tail's `next`
            // at the new node and record the new node's `prev` back to it.
            let old_next_key = DataKey::PendingVerificationNext(tail_key.clone());
            e.storage()
                .persistent()
                .set(&old_next_key, &Some(key.clone()));
            e.storage().persistent().extend_ttl(
                &old_next_key,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
            let prev_key = DataKey::PendingVerificationPrev(key.clone());
            e.storage()
                .persistent()
                .set(&prev_key, &Some(tail_key.clone()));
            e.storage().persistent().extend_ttl(
                &prev_key,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
            let next_key = DataKey::PendingVerificationNext(key.clone());
            e.storage()
                .persistent()
                .set(&next_key, &None::<VerificationKey>);
            e.storage().persistent().extend_ttl(
                &next_key,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
            state.tail = Some(key);
        }
    }
    state.count += 1;
    write_pending_list(e, &state);
}

/// Removes a verification key from the pending list via O(1) pointer
/// surgery: it updates the removed node's neighbours and the fixed-size
/// list state without ever deserialising the whole list. If the key is not
/// currently in the pending list (e.g. a dispute already removed it), this
/// is a no-op.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task
/// * `user` - The user address
pub fn remove_verification_key(e: &Env, task_id: u64, user: &Address) {
    let key = VerificationKey {
        task_id,
        user: user.clone(),
    };
    let prev_key = DataKey::PendingVerificationPrev(key.clone());
    if !e.storage().persistent().has(&prev_key) {
        // Not in the pending list: nothing to unlink.
        return;
    }
    let prev: Option<VerificationKey> = e.storage().persistent().get(&prev_key).unwrap_or(None);
    let next: Option<VerificationKey> = e
        .storage()
        .persistent()
        .get(&DataKey::PendingVerificationNext(key.clone()))
        .unwrap_or(None);
    let mut state = read_pending_list(e);
    match &prev {
        Some(prev_key_val) => {
            let next_of_prev = DataKey::PendingVerificationNext(prev_key_val.clone());
            e.storage().persistent().set(&next_of_prev, &next);
            e.storage().persistent().extend_ttl(
                &next_of_prev,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
        }
        None => state.head = next.clone(),
    }
    match &next {
        Some(next_key_val) => {
            let prev_of_next = DataKey::PendingVerificationPrev(next_key_val.clone());
            e.storage().persistent().set(&prev_of_next, &prev);
            e.storage().persistent().extend_ttl(
                &prev_of_next,
                PERSISTENT_TTL_THRESHOLD,
                PERSISTENT_TTL_EXTEND_TO,
            );
        }
        None => state.tail = prev.clone(),
    }
    e.storage().persistent().remove(&prev_key);
    e.storage()
        .persistent()
        .remove(&DataKey::PendingVerificationNext(key.clone()));
    state.count = state.count.saturating_sub(1);
    write_pending_list(e, &state);
}

/// Allocates the next monotonically-increasing verification sequence
/// number. Sequence numbers are assigned once at submit time and never
/// change, which makes them a stable pagination cursor: resolving an entry
/// can never shift another entry past a cursor.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The next sequence number to assign (1, 2, 3, ...).
pub fn next_verification_seq(e: &Env) -> u64 {
    let key = DataKey::VerificationSeq;
    let seq: u64 = e.storage().instance().get(&key).unwrap_or(0);
    let next = seq + 1;
    e.storage().instance().set(&key, &next);
    next
}

/// Returns the key the given pending node links to as `next`, or `None`
/// when the node is the tail or is no longer in the pending list.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `key` - The verification key whose successor to look up
///
/// # Returns
///
/// The next pending verification key, or `None` at the end of the list.
pub fn read_pending_node_next(e: &Env, key: &VerificationKey) -> Option<VerificationKey> {
    e.storage()
        .persistent()
        .get(&DataKey::PendingVerificationNext(key.clone()))
        .unwrap_or(None)
}

/// Appends a task ID to a user's verification history.
///
/// A user's history is a persistent sequence index (a count plus one
/// per-position task ID entry), so appending never deserialises a growing
/// list: it is bounded to a constant number of storage operations.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `user` - The user address
/// * `task_id` - The ID of the task to add
pub fn push_user_verification_key(e: &Env, user: &Address, task_id: u64) {
    let count_key = DataKey::UserVerificationCount(user.clone());
    let count: u64 = e.storage().persistent().get(&count_key).unwrap_or(0);
    let index_key = DataKey::UserVerificationIndex(user.clone(), count);
    e.storage().persistent().set(&index_key, &task_id);
    e.storage().persistent().extend_ttl(
        &index_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );
    e.storage().persistent().set(&count_key, &(count + 1));
    e.storage().persistent().extend_ttl(
        &count_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );
}

/// Returns the number of verifications a user has submitted.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `user` - The user address to query
///
/// # Returns
///
/// The count of the user's verification entries.
pub fn read_user_verification_count(e: &Env, user: &Address) -> u64 {
    e.storage()
        .persistent()
        .get(&DataKey::UserVerificationCount(user.clone()))
        .unwrap_or(0)
}

/// Returns the task ID at position `seq` in a user's verification history.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `user` - The user address to query
/// * `seq` - The zero-based position in the user's history
///
/// # Returns
///
/// The task ID at that position, or `None` if the position is out of range.
pub fn read_user_verification_task(e: &Env, user: &Address, seq: u64) -> Option<u64> {
    e.storage()
        .persistent()
        .get(&DataKey::UserVerificationIndex(user.clone(), seq))
}

/// Adds an amount to the total paid out by this engine.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `amount` - The amount to add to the total
///
/// # Panics
///
/// Panics if the addition would overflow.
pub fn add_total_paid(e: &Env, amount: i128) {
    let key = DataKey::TotalPaid;
    let current: i128 = e.storage().instance().get(&key).unwrap_or(0);
    e.storage().instance().set(
        &key,
        &(current.checked_add(amount).expect("total_paid overflow")),
    );
}

/// Reads the total amount paid out by this engine.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The total paid amount, or 0 if no payments have been made.
pub fn read_total_paid(e: &Env) -> i128 {
    let key = DataKey::TotalPaid;
    e.storage().instance().get(&key).unwrap_or(0)
}

/// Sets the minimum number of ledgers a user must wait between two reward
/// approvals.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `ledgers` - The cooldown length in ledgers; 0 disables the cooldown.
pub fn write_user_cooldown(e: &Env, ledgers: u64) {
    e.storage().instance().set(&DataKey::UserCooldown, &ledgers);
}

/// Reads the configured per-user reward cooldown.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The cooldown in ledgers, or 0 (disabled) if never configured.
pub fn read_user_cooldown(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::UserCooldown)
        .unwrap_or(0)
}

/// Records the ledger at which a user most recently received a reward.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `user` - The user address being rewarded
/// * `ledger` - The current ledger sequence number
pub fn write_last_reward_ledger(e: &Env, user: &Address, ledger: u64) {
    let key = DataKey::LastRewardLedger(user.clone());
    e.storage().persistent().set(&key, &ledger);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
}

/// Reads the ledger at which a user most recently received a reward.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `user` - The user address to query
///
/// # Returns
///
/// `None` if the user has never been rewarded, distinguishing that case
/// from a legitimate ledger value of 0.
pub fn read_last_reward_ledger(e: &Env, user: &Address) -> Option<u64> {
    let key = DataKey::LastRewardLedger(user.clone());
    e.storage().persistent().get(&key)
}

/// Sets the paused state of the contract.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `paused` - true to pause, false to unpause
pub fn set_paused(e: &Env, paused: bool) {
    e.storage().instance().set(&DataKey::Paused, &paused);
}

/// Checks if the contract is currently paused.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// true if the contract is paused, false otherwise.
pub fn is_paused(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

/// Extends the TTL of the contract instance (and code) to
/// `INSTANCE_TTL_EXTEND_TO` ledgers when it is within
/// `INSTANCE_TTL_THRESHOLD` ledgers of expiring.
///
/// Called as the first operation of every public entry point so that any
/// interaction with the engine keeps its configuration alive. See the
/// `INSTANCE_TTL_*` constants at the top of this module for the rationale
/// behind the chosen values.
///
/// # Arguments
///
/// * `e` - The Soroban environment
pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
}
