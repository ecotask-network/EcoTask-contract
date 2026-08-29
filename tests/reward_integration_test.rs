use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::BytesN;
use soroban_sdk::{Address, Env, String};

fn deploy_token(e: &Env, admin: &Address) -> Address {
    let token_id = e.register(eco_token::TokenContract, ());
    let client = eco_token::TokenContractClient::new(e, &token_id);
    client.initialize(
        admin,
        &String::from_str(e, "ECO"),
        &String::from_str(e, "ECO"),
        &7,
    );
    token_id
}

fn deploy_registry(e: &Env, admin: &Address) -> Address {
    let reg_id = e.register(task_registry::RegistryContract, ());
    let client = task_registry::RegistryContractClient::new(e, &reg_id);
    client.initialize(admin);
    reg_id
}

fn deploy_engine(
    e: &Env,
    admin: &Address,
    token_id: &Address,
    reg_id: &Address,
    oracle: &Address,
) -> Address {
    let engine_id = e.register(reward_engine::RewardEngine, ());
    let engine_client = reward_engine::RewardEngineClient::new(e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(e, reg_id);
    reg_client.add_sponsor(admin, &engine_id);
    engine_client.initialize(admin, token_id, reg_id, oracle);
    engine_id
}

#[test]
fn test_full_payout_lifecycle() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
    let token_client = eco_token::TokenContractClient::new(&e, &token_id);

    let loc_hash1 = soroban_sdk::BytesN::<32>::random(&e);
    let task_id1 = reg_client.create_task(
        &admin,
        &String::from_str(&e, "tree-planting"),
        &loc_hash1,
        &500,
        &2,
        &(e.ledger().timestamp() + 10000),
    );

    let loc_hash2 = soroban_sdk::BytesN::<32>::random(&e);
    let task_id2 = reg_client.create_task(
        &admin,
        &String::from_str(&e, "ocean-cleanup"),
        &loc_hash2,
        &300,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof1 = String::from_str(&e, "QmUser1Proof");
    engine_client.submit_proof(&oracle, &user1, &task_id1, &proof1);
    engine_client.approve_proof(&oracle, &user1, &task_id1, &500);

    let proof2 = String::from_str(&e, "QmUser2Proof");
    engine_client.submit_proof(&oracle, &user2, &task_id2, &proof2);
    engine_client.approve_proof(&oracle, &user2, &task_id2, &300);

    assert_eq!(token_client.balance(&user1), 500);
    assert_eq!(token_client.balance(&user2), 300);
    assert_eq!(token_client.total_supply(), 800);
    assert_eq!(engine_client.total_paid(), 800);

    let task1 = reg_client.get_task(&task_id1);
    assert_eq!(task1.completions, 1);
    assert_eq!(task1.status, task_registry::TaskStatus::Active);

    let task2 = reg_client.get_task(&task_id2);
    assert_eq!(task2.completions, 1);
    assert_eq!(task2.status, task_registry::TaskStatus::Completed);
}

#[test]
fn test_dispute_reject_then_resolve() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "river-cleanup"),
        &loc_hash,
        &750,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof = String::from_str(&e, "QmDisputed");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);
    engine_client.reject_proof(&oracle, &user, &task_id);

    let v = engine_client.get_verification(&task_id, &user);
    assert_eq!(v.status, reward_engine::VerificationStatus::Rejected);

    engine_client.dispute_proof(&admin, &user, &task_id);
    let v = engine_client.get_verification(&task_id, &user);
    assert_eq!(v.status, reward_engine::VerificationStatus::Disputed);

    engine_client.resolve_dispute(&admin, &user, &task_id, &true, &750);

    let token_client = eco_token::TokenContractClient::new(&e, &token_id);
    assert_eq!(token_client.balance(&user), 750);
    assert_eq!(engine_client.total_paid(), 750);
}

#[test]
fn test_multi_user_max_completions() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let users: Vec<Address> = (0..5).map(|_| Address::generate(&e)).collect();

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "beach-cleanup"),
        &loc_hash,
        &100,
        &5,
        &(e.ledger().timestamp() + 10000),
    );

    for (index, user) in users.iter().enumerate() {
        let cids = ["QmBeach1", "QmBeach2", "QmBeach3", "QmBeach4", "QmBeach5"];
        let proof = String::from_str(&e, cids[index]);
        engine_client.submit_proof(&oracle, user, &task_id, &proof);
        engine_client.approve_proof(&oracle, user, &task_id, &100);
    }

    let token_client = eco_token::TokenContractClient::new(&e, &token_id);
    for user in users.iter() {
        assert_eq!(token_client.balance(user), 100);
    }
    assert_eq!(token_client.total_supply(), 500);
    assert_eq!(engine_client.total_paid(), 500);

    let task = reg_client.get_task(&task_id);
    assert_eq!(task.completions, 5);
    assert_eq!(task.status, task_registry::TaskStatus::Completed);
}

#[test]
fn test_reward_cap_enforced_cross_contract() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "recycling"),
        &loc_hash,
        &200,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof = String::from_str(&e, "QmCap");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);

    let result = engine_client.try_approve_proof(&oracle, &user, &task_id, &999);
    assert!(result.is_err());
}

#[test]
fn test_minter_is_reward_engine_after_setup() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let token_client = eco_token::TokenContractClient::new(&e, &token_id);

    assert_eq!(token_client.minter(), admin);

    token_client.set_minter(&admin, &engine_id);
    assert_eq!(token_client.minter(), engine_id);
}

#[test]
fn test_admin_cancel_and_payout_rejection() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "composting"),
        &loc_hash,
        &400,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof = String::from_str(&e, "QmAdmin");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);

    reg_client.admin_cancel_task(&admin, &task_id);

    let result = engine_client.try_approve_proof(&oracle, &user, &task_id, &400);
    assert!(result.is_err());
}

#[test]
fn test_emergency_pause_blocks_rewards() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "reforestation"),
        &loc_hash,
        &500,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof = String::from_str(&e, "QmPause");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);

    engine_client.pause(&admin);
    assert!(engine_client.is_paused());

    let result = engine_client.try_approve_proof(&oracle, &user, &task_id, &500);
    assert!(result.is_err());

    let result = engine_client.try_dispute_proof(&admin, &user, &task_id);
    assert!(result.is_err());

    engine_client.unpause(&admin);
    assert!(!engine_client.is_paused());

    engine_client.approve_proof(&oracle, &user, &task_id, &500);
    let token_client = eco_token::TokenContractClient::new(&e, &token_id);
    assert_eq!(token_client.balance(&user), 500);
}

#[test]
fn test_supply_cap_blocks_engine_mint() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
    let token_client = eco_token::TokenContractClient::new(&e, &token_id);

    // Cap the token at 400 ECO: the 500 ECO reward cannot be issued.
    token_client.set_max_supply(&admin, &400);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "recycling"),
        &loc_hash,
        &500,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof = String::from_str(&e, "QmCap");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);

    let result = engine_client.try_approve_proof(&oracle, &user, &task_id, &500);
    assert!(result.is_err());

    // The task is not marked completed and nothing was paid out.
    assert!(!reg_client.is_task_completed(&task_id, &user));
    assert_eq!(engine_client.total_paid(), 0);
    assert_eq!(token_client.balance(&user), 0);

    // Raising the cap unlocks the same payout.
    token_client.set_max_supply(&admin, &1000);
    engine_client.approve_proof(&oracle, &user, &task_id, &500);

    assert_eq!(token_client.balance(&user), 500);
    assert_eq!(engine_client.total_paid(), 500);
    assert!(reg_client.is_task_completed(&task_id, &user));
}

#[test]
fn test_partial_failure_leaves_no_orphan_state() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
    let token_client = eco_token::TokenContractClient::new(&e, &token_id);

    // Cap the token below the reward so the mint sub-call panics.
    token_client.set_max_supply(&admin, &400);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "tree-planting"),
        &loc_hash,
        &500,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof = String::from_str(&e, "QmOrphan");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);

    let result = engine_client.try_approve_proof(&oracle, &user, &task_id, &500);
    assert!(result.is_err());

    // No orphaned state: the verification is still Pending (not Approved),
    // the task is not marked completed, and nothing was paid out.
    let v = engine_client.get_verification(&task_id, &user);
    assert_eq!(v.status, reward_engine::VerificationStatus::Pending);
    assert!(!reg_client.is_task_completed(&task_id, &user));
    assert_eq!(engine_client.total_paid(), 0);
    assert_eq!(token_client.balance(&user), 0);

    // The proof is not stuck: raising the cap unlocks the same payout.
    token_client.set_max_supply(&admin, &1000);
    engine_client.approve_proof(&oracle, &user, &task_id, &500);

    let v = engine_client.get_verification(&task_id, &user);
    assert_eq!(v.status, reward_engine::VerificationStatus::Approved);
    assert_eq!(token_client.balance(&user), 500);
    assert_eq!(engine_client.total_paid(), 500);
    assert!(reg_client.is_task_completed(&task_id, &user));
}

#[test]
fn test_resolve_dispute_mint_failure_leaves_no_orphan_state() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
    let token_client = eco_token::TokenContractClient::new(&e, &token_id);

    // Cap the token below the reward so the mint sub-call panics.
    token_client.set_max_supply(&admin, &400);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "ocean-cleanup"),
        &loc_hash,
        &500,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof = String::from_str(&e, "QmOrphanDispute");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);
    engine_client.dispute_proof(&admin, &user, &task_id);

    let result = engine_client.try_resolve_dispute(&admin, &user, &task_id, &true, &500);
    assert!(result.is_err());

    // No orphaned state: the verification is still Disputed (not Approved),
    // the task is not marked completed, and nothing was paid out.
    let v = engine_client.get_verification(&task_id, &user);
    assert_eq!(v.status, reward_engine::VerificationStatus::Disputed);
    assert!(!reg_client.is_task_completed(&task_id, &user));
    assert_eq!(engine_client.total_paid(), 0);
    assert_eq!(token_client.balance(&user), 0);

    // The disputed proof is not stuck: raising the cap unlocks the payout.
    token_client.set_max_supply(&admin, &1000);
    engine_client.resolve_dispute(&admin, &user, &task_id, &true, &500);

    let v = engine_client.get_verification(&task_id, &user);
    assert_eq!(v.status, reward_engine::VerificationStatus::Approved);
    assert_eq!(token_client.balance(&user), 500);
    assert_eq!(engine_client.total_paid(), 500);
    assert!(reg_client.is_task_completed(&task_id, &user));
}

#[test]
fn test_supply_cap_counts_cumulative_emissions() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);
    let token_client = eco_token::TokenContractClient::new(&e, &token_id);

    // Cumulative emissions are capped: two 400 ECO payouts exceed a 600 cap.
    token_client.set_max_supply(&admin, &600);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "beach-cleanup"),
        &loc_hash,
        &400,
        &2,
        &(e.ledger().timestamp() + 10000),
    );

    let proof1 = String::from_str(&e, "QmCap1");
    engine_client.submit_proof(&oracle, &user1, &task_id, &proof1);
    engine_client.approve_proof(&oracle, &user1, &task_id, &400);

    let proof2 = String::from_str(&e, "QmCap2");
    engine_client.submit_proof(&oracle, &user2, &task_id, &proof2);
    let result = engine_client.try_approve_proof(&oracle, &user2, &task_id, &400);
    assert!(result.is_err());

    assert_eq!(token_client.total_supply(), 400);
    assert_eq!(engine_client.total_paid(), 400);
}

#[test]
#[should_panic(expected = "registry: sponsor revoked")]
fn test_oracle_approval_fails_for_desponsored_creator() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let sponsor = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);

    reg_client.add_sponsor(&admin, &sponsor);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &sponsor,
        &String::from_str(&e, "tree-planting"),
        &loc_hash,
        &500,
        &1,
        &(e.ledger().timestamp() + 10000),
    );

    let proof = String::from_str(&e, "QmSponsorRevokedProof");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);

    reg_client.remove_sponsor(&admin, &sponsor);

    engine_client.approve_proof(&oracle, &user, &task_id, &500);
}

// ---------------------------------------------------------------------------
// Budget / footprint benchmarks
//
// Stellar Soroban per-transaction network limits (Protocol 22 mainnet):
//   CPU instructions : 100,000,000
//   Memory bytes     :  40,971,520  (~40 MB)
//
// These tests assert that the compound operations stay below 50% of each
// limit — a conservative buffer that leaves room for SDK overhead growth
// and future contract additions.
//
// Run with:  make bench
// ---------------------------------------------------------------------------

/// The Stellar Soroban mainnet CPU-instruction limit per transaction
/// (Protocol 22, https://developers.stellar.org/docs/networks/resource-limits-fees).
const MAINNET_CPU_LIMIT: u64 = 100_000_000;

/// The Stellar Soroban mainnet memory-bytes limit per transaction.
const MAINNET_MEM_LIMIT: u64 = 40_971_520;

/// Conservative threshold: 50% of each network limit.
const CPU_THRESHOLD: u64 = MAINNET_CPU_LIMIT / 2; // 50_000_000
const MEM_THRESHOLD: u64 = MAINNET_MEM_LIMIT / 2; //  20_485_760

/// Helper: deploy all three contracts and wire them together.
/// Returns (engine_client, reg_client, task_id).
fn setup_benchmark_env(
    e: &Env,
    admin: &Address,
    oracle: &Address,
    reward_budget: i128,
    task_type: &str,
) -> (
    reward_engine::RewardEngineClient<'static>,
    task_registry::RegistryContractClient<'static>,
    u64,
) {
    let token_id = deploy_token(e, admin);
    let reg_id = deploy_registry(e, admin);
    let engine_id = deploy_engine(e, admin, &token_id, &reg_id, oracle);

    // Point the token minter at the engine so mint() auth is satisfied
    // when the engine makes its cross-contract call during approve_proof.
    let token_client = eco_token::TokenContractClient::new(e, &token_id);
    token_client.set_minter(admin, &engine_id);

    let engine_client = reward_engine::RewardEngineClient::new(e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(e, &reg_id);

    let loc_hash = soroban_sdk::BytesN::<32>::random(e);
    let task_id = reg_client.create_task(
        admin,
        &String::from_str(e, task_type),
        &loc_hash,
        &reward_budget,
        &1,
        &(e.ledger().timestamp() + 10_000),
    );

    (engine_client, reg_client, task_id)
}

/// Builds a deployed engine where `total` distinct users each submit one
/// proof for a single shared task, then resolves the first `resolved` of
/// them through the dispute → reject-resolution path (which leaves the
/// records in the historical log but removes them from the pending set).
///
/// Returns the env, engine client, and task id.
fn build_pending_stress_env(
    total: usize,
    resolved: usize,
) -> (Env, reward_engine::RewardEngineClient<'static>, u64) {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle);

    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);
    let reg_client = task_registry::RegistryContractClient::new(&e, &reg_id);

    let loc_hash = soroban_sdk::BytesN::<32>::random(&e);
    let task_id = reg_client.create_task(
        &admin,
        &String::from_str(&e, "stress-pending"),
        &loc_hash,
        &1000,
        &(total as u32),
        &(e.ledger().timestamp() + 10000),
    );

    for i in 0..total {
        let user = Address::generate(&e);
        let proof = String::from_str(&e, &format!("QmStress{:04}", i));
        engine_client.submit_proof(&oracle, &user, &task_id, &proof);
        if i < resolved {
            engine_client.dispute_proof(&admin, &user, &task_id);
            engine_client.resolve_dispute(&admin, &user, &task_id, &false, &0);
        }
    }

    (e, engine_client, task_id)
}

/// Stress test for pending-list pagination: 200 proofs are submitted and
/// 180 are resolved, and a single `get_pending_verifications_paged(0, 20)`
/// call returns exactly the 20 remaining pending verifications without
/// scanning the resolved records.
#[test]
fn test_pending_pagination_does_not_scan_resolved_records() {
    let total = 200usize;
    let resolved = 180usize;
    let pending = (total - resolved) as u32; // 20

    // Resolved-heavy environment: 180 of the 200 verifications resolved.
    let (e_resolved, client_resolved, _task_id) = build_pending_stress_env(total, resolved);
    assert_eq!(
        client_resolved
            .get_pending_verifications_paged(&0, &50)
            .len(),
        pending
    );

    // One paged call returns exactly `pending` entries. The budget is reset
    // immediately beforehand so the measured cost covers only this call.
    e_resolved.cost_estimate().budget().reset_default();
    let page = client_resolved.get_pending_verifications_paged(&0, &pending);
    assert_eq!(page.len(), pending);
    let resolved_mem = e_resolved.cost_estimate().budget().memory_bytes_cost();
    let resolved_cpu = e_resolved.cost_estimate().budget().cpu_instruction_cost();

    println!("\n[test_pending_pagination_does_not_scan_resolved_records]");
    println!("  paged(0, {}) with 180/200 resolved", pending);
    println!(
        "  CPU instructions : {:>12}   Memory bytes : {:>12}",
        resolved_cpu, resolved_mem
    );

    // Baseline environment: all 200 verifications still pending. Serving the
    // same 20-entry page must cost about the same as the resolved-heavy
    // case — pagination walks only the pending list, so resolved history
    // never enters the scan. A log-scanning implementation would read ~200
    // records in the resolved-heavy case (≈10x the baseline), so a 3x bound
    // is a clear regression guard while tolerating measurement noise.
    let (e_clean, client_clean, _task_id) = build_pending_stress_env(total, 0);
    e_clean.cost_estimate().budget().reset_default();
    let _ = client_clean.get_pending_verifications_paged(&0, &pending);
    let clean_mem = e_clean.cost_estimate().budget().memory_bytes_cost();
    let clean_cpu = e_clean.cost_estimate().budget().cpu_instruction_cost();

    println!("  paged(0, {}) with 0/200 resolved (baseline)", pending);
    println!(
        "  CPU instructions : {:>12}   Memory bytes : {:>12}",
        clean_cpu, clean_mem
    );

    assert!(
        resolved_mem <= clean_mem * 3,
        "paged call in the resolved-heavy env used {} memory bytes vs {} in the \
         all-pending baseline — it appears to be scanning resolved records",
        resolved_mem,
        clean_mem
    );
    assert!(
        resolved_cpu <= clean_cpu * 3,
        "paged call in the resolved-heavy env used {} CPU instructions vs {} in \
         the all-pending baseline — it appears to be scanning resolved records",
        resolved_cpu,
        clean_cpu
    );
}

/// Measures CPU instructions and memory bytes consumed by
/// `submit_proof` + `approve_proof` (the hot path) and asserts
/// both are below 50% of the Stellar mainnet per-transaction limits.
///
/// Run with: `make bench`
#[test]
fn test_approve_proof_budget() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let (engine_client, _reg_client, task_id) =
        setup_benchmark_env(&e, &admin, &oracle, 500, "tree-planting");

    // Reset the budget counter here so only the two measured calls are
    // accounted for; setup overhead (deploys, creates) is excluded.
    e.cost_estimate().budget().reset_default();

    let proof = String::from_str(&e, "QmBudgetApproveProof");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);
    engine_client.approve_proof(&oracle, &user, &task_id, &500);

    let cpu = e.cost_estimate().budget().cpu_instruction_cost();
    let mem = e.cost_estimate().budget().memory_bytes_cost();

    println!("\n[test_approve_proof_budget]");
    println!("  submit_proof + approve_proof");
    println!(
        "  CPU instructions : {:>12} / {:>12}  ({:.1}% of 50% threshold, {:.1}% of mainnet limit)",
        cpu,
        CPU_THRESHOLD,
        cpu as f64 / CPU_THRESHOLD as f64 * 100.0,
        cpu as f64 / MAINNET_CPU_LIMIT as f64 * 100.0,
    );
    println!(
        "  Memory bytes     : {:>12} / {:>12}  ({:.1}% of 50% threshold, {:.1}% of mainnet limit)",
        mem,
        MEM_THRESHOLD,
        mem as f64 / MEM_THRESHOLD as f64 * 100.0,
        mem as f64 / MAINNET_MEM_LIMIT as f64 * 100.0,
    );

    assert!(
        cpu < CPU_THRESHOLD,
        "submit_proof + approve_proof used {} CPU instructions — \
         exceeds the 50% safety threshold ({}).\n\
         Stellar mainnet limit: {}. \
         File a follow-up optimisation issue.",
        cpu,
        CPU_THRESHOLD,
        MAINNET_CPU_LIMIT,
    );
    assert!(
        mem < MEM_THRESHOLD,
        "submit_proof + approve_proof used {} memory bytes — \
         exceeds the 50% safety threshold ({}).\n\
         Stellar mainnet limit: {}. \
         File a follow-up optimisation issue.",
        mem,
        MEM_THRESHOLD,
        MAINNET_MEM_LIMIT,
    );
}

/// Measures CPU instructions and memory bytes consumed by the full
/// dispute path: `submit_proof` → `reject_proof` → `dispute_proof`
/// → `resolve_dispute` (approve) and asserts both stay below 50% of
/// the Stellar mainnet per-transaction limits.
///
/// Run with: `make bench`
#[test]
fn test_dispute_resolve_budget() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let user = Address::generate(&e);

    let (engine_client, _reg_client, task_id) =
        setup_benchmark_env(&e, &admin, &oracle, 750, "coastline-cleanup");

    // Submit and reject without measuring — these are preconditions for
    // the disputed state, not the operation being benchmarked.
    let proof = String::from_str(&e, "QmBudgetDisputeProof");
    engine_client.submit_proof(&oracle, &user, &task_id, &proof);
    engine_client.reject_proof(&oracle, &user, &task_id);

    // Reset the budget counter: measure only the dispute + resolve path.
    e.cost_estimate().budget().reset_default();

    engine_client.dispute_proof(&admin, &user, &task_id);
    engine_client.resolve_dispute(&admin, &user, &task_id, &true, &750);

    let cpu = e.cost_estimate().budget().cpu_instruction_cost();
    let mem = e.cost_estimate().budget().memory_bytes_cost();

    println!("\n[test_dispute_resolve_budget]");
    println!("  dispute_proof + resolve_dispute (approve path)");
    println!(
        "  CPU instructions : {:>12} / {:>12}  ({:.1}% of 50% threshold, {:.1}% of mainnet limit)",
        cpu,
        CPU_THRESHOLD,
        cpu as f64 / CPU_THRESHOLD as f64 * 100.0,
        cpu as f64 / MAINNET_CPU_LIMIT as f64 * 100.0,
    );
    println!(
        "  Memory bytes     : {:>12} / {:>12}  ({:.1}% of 50% threshold, {:.1}% of mainnet limit)",
        mem,
        MEM_THRESHOLD,
        mem as f64 / MEM_THRESHOLD as f64 * 100.0,
        mem as f64 / MAINNET_MEM_LIMIT as f64 * 100.0,
    );

    assert!(
        cpu < CPU_THRESHOLD,
        "dispute_proof + resolve_dispute used {} CPU instructions — \
         exceeds the 50% safety threshold ({}).\n\
         Stellar mainnet limit: {}. \
         File a follow-up optimisation issue.",
        cpu,
        CPU_THRESHOLD,
        MAINNET_CPU_LIMIT,
    );
    assert!(
        mem < MEM_THRESHOLD,
        "dispute_proof + resolve_dispute used {} memory bytes — \
         exceeds the 50% safety threshold ({}).\n\
         Stellar mainnet limit: {}. \
         File a follow-up optimisation issue.",
        mem,
        MEM_THRESHOLD,
        MAINNET_MEM_LIMIT,
    );
}
