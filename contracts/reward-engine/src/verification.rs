use crate::storage;
use soroban_sdk::{
    contract, contractevent, contractimpl, vec, Address, Env, IntoVal, String, Symbol, Val,
};
pub use storage::{Verification, VerificationStatus};
use task_registry::{Task, TaskStatus};

/// Fetches the task from the registry and enforces that it is active and not
/// expired. Returns the task so the caller can inspect `reward_amount`.
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

#[contract]
pub struct RewardEngine;

/// Panics if the engine is paused.
fn require_not_paused(e: &Env) {
    if storage::is_paused(e) {
        panic!("engine: contract is paused");
    }
}

#[contractimpl]
impl RewardEngine {
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
        storage::write_oracle(&e, &oracle);
    }

    pub fn set_oracle(e: Env, caller: Address, new_oracle: Address) {
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("engine: unauthorized");
        }
        storage::write_oracle(&e, &new_oracle);
    }

    pub fn set_token(e: Env, caller: Address, new_token: Address) {
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("engine: unauthorized");
        }
        storage::write_token(&e, &new_token);
    }

    pub fn set_registry(e: Env, caller: Address, new_registry: Address) {
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("engine: unauthorized");
        }
        storage::write_registry(&e, &new_registry);
    }

    pub fn set_reward_range(e: Env, caller: Address, min_reward: i128, max_reward: i128) {
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("engine: unauthorized");
        }
        if min_reward <= 0 {
            panic!("engine: min reward must be positive");
        }
        if max_reward < min_reward {
            panic!("engine: max reward must be >= min reward");
        }
        storage::write_reward_range(&e, min_reward, max_reward);
    }

    pub fn pause(e: Env, caller: Address) {
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("engine: unauthorized");
        }
        storage::set_paused(&e, true);
    }

    pub fn unpause(e: Env, caller: Address) {
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("engine: unauthorized");
        }
        storage::set_paused(&e, false);
    }

    pub fn is_paused(e: Env) -> bool {
        storage::is_paused(&e)
    }

    pub fn submit_proof(e: Env, oracle: Address, user: Address, task_id: u64, proof_cid: String) {
        require_not_paused(&e);
        oracle.require_auth();
        let stored_oracle = storage::read_oracle(&e);
        if oracle != stored_oracle {
            panic!("engine: unauthorized");
        }

        if storage::read_verification(&e, task_id, &user).is_some() {
            panic!("engine: proof already submitted");
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
        };

        storage::write_verification(&e, task_id, &user, &verification);
        storage::push_verification_key(&e, task_id, &user);

        ProofSubmittedEvent {
            oracle,
            user,
            task_id,
        }
        .publish(&e);
    }

    pub fn approve_proof(
        e: Env,
        oracle: Address,
        user: Address,
        task_id: u64,
        reward_amount: i128,
    ) {
        require_not_paused(&e);
        oracle.require_auth();
        let stored_oracle = storage::read_oracle(&e);
        if oracle != stored_oracle {
            panic!("engine: unauthorized");
        }

        let mut verification = match storage::read_verification(&e, task_id, &user) {
            Some(v) => v,
            None => panic!("engine: verification not found"),
        };

        if verification.status != VerificationStatus::Pending {
            panic!("engine: verification is not pending");
        }

        if reward_amount <= 0 {
            panic!("engine: reward amount must be positive");
        }
        if let Some(min) = storage::read_min_reward(&e) {
            if reward_amount < min {
                panic!("engine: reward below minimum");
            }
        }
        if let Some(max) = storage::read_max_reward(&e) {
            if reward_amount > max {
                panic!("engine: reward exceeds maximum");
            }
        }

        let task = require_active_task(&e, task_id);
        if reward_amount > task.reward_amount {
            panic!("engine: reward exceeds task budget");
        }

        verification.status = VerificationStatus::Approved;
        verification.reward_amount = reward_amount;
        verification.resolved_at = Some(e.ledger().timestamp());
        storage::write_verification(&e, task_id, &user, &verification);
        storage::remove_verification_key(&e, task_id, &user);

        let registry_id = storage::read_registry(&e);
        e.invoke_contract::<Val>(
            &registry_id,
            &Symbol::new(&e, "complete_task"),
            vec![
                &e,
                e.current_contract_address().into_val(&e),
                task_id.into_val(&e),
                user.clone().into_val(&e),
            ],
        );

        let token_id = storage::read_token(&e);
        e.invoke_contract::<Val>(
            &token_id,
            &Symbol::new(&e, "mint"),
            vec![&e, user.clone().into_val(&e), reward_amount.into_val(&e)],
        );

        RewardPaidEvent {
            oracle,
            user,
            task_id,
            amount: reward_amount,
        }
        .publish(&e);

        storage::add_total_paid(&e, reward_amount);
    }

    pub fn reject_proof(e: Env, oracle: Address, user: Address, task_id: u64) {
        require_not_paused(&e);
        oracle.require_auth();
        let stored_oracle = storage::read_oracle(&e);
        if oracle != stored_oracle {
            panic!("engine: unauthorized");
        }

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

    pub fn dispute_proof(e: Env, caller: Address, user: Address, task_id: u64) {
        require_not_paused(&e);
        caller.require_auth();
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("engine: unauthorized");
        }

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
        let admin = storage::read_admin(&e);
        if caller != admin {
            panic!("engine: unauthorized");
        }

        let mut verification = match storage::read_verification(&e, task_id, &user) {
            Some(v) => v,
            None => panic!("engine: verification not found"),
        };

        if verification.status != VerificationStatus::Disputed {
            panic!("engine: verification is not disputed");
        }

        if approve {
            if reward_amount <= 0 {
                panic!("engine: reward amount must be positive");
            }
            if let Some(min) = storage::read_min_reward(&e) {
                if reward_amount < min {
                    panic!("engine: reward below minimum");
                }
            }
            if let Some(max) = storage::read_max_reward(&e) {
                if reward_amount > max {
                    panic!("engine: reward exceeds maximum");
                }
            }

            let task = require_active_task(&e, task_id);
            if reward_amount > task.reward_amount {
                panic!("engine: reward exceeds task budget");
            }

            verification.status = VerificationStatus::Approved;
            verification.reward_amount = reward_amount;
            verification.resolved_at = Some(e.ledger().timestamp());
            storage::write_verification(&e, task_id, &user, &verification);

            let registry_id = storage::read_registry(&e);
            e.invoke_contract::<Val>(
                &registry_id,
                &Symbol::new(&e, "complete_task"),
                vec![
                    &e,
                    e.current_contract_address().into_val(&e),
                    task_id.into_val(&e),
                    user.clone().into_val(&e),
                ],
            );

            let token_id = storage::read_token(&e);
            e.invoke_contract::<Val>(
                &token_id,
                &Symbol::new(&e, "mint"),
                vec![&e, user.clone().into_val(&e), reward_amount.into_val(&e)],
            );

            storage::add_total_paid(&e, reward_amount);
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

    pub fn get_verification(e: Env, task_id: u64, user: Address) -> Verification {
        match storage::read_verification(&e, task_id, &user) {
            Some(v) => v,
            None => panic!("engine: verification not found"),
        }
    }

    pub fn get_pending_verifications(e: Env) -> soroban_sdk::Vec<Verification> {
        let keys = storage::read_verification_keys(&e);
        let mut pending: soroban_sdk::Vec<Verification> = soroban_sdk::Vec::new(&e);
        for key in keys.iter() {
            if let Some(v) = storage::read_verification(&e, key.task_id, &key.user) {
                if v.status == VerificationStatus::Pending {
                    pending.push_back(v);
                }
            }
        }
        pending
    }

    pub fn total_paid(e: Env) -> i128 {
        storage::read_total_paid(&e)
    }

    pub fn transfer_admin(e: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();
        let stored_admin = storage::read_admin(&e);
        if current_admin != stored_admin {
            panic!("engine: unauthorized");
        }
        if new_admin == current_admin {
            panic!("engine: new admin must be different");
        }
        storage::write_admin(&e, &new_admin);
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
    fn test_reward_range_enforced() {
        let (e, admin, oracle, user, task_id, client) = setup();
        e.mock_all_auths_allowing_non_root_auth();

        // Set allowed range: 500 – 2000
        client.set_reward_range(&admin, &500, &2000);

        let proof_cid = String::from_str(&e, "QmRangeOk");
        client.submit_proof(&oracle, &user, &task_id, &proof_cid);
        // 1000 is within range — should succeed
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

        let new_oracle = Address::generate(&e);
        client.set_oracle(&new_admin, &new_oracle);
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
}
