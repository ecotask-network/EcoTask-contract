use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::BytesN;
use soroban_sdk::{Address, Env, String};
use soroban_sdk::Symbol;

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

#[test]
fn test_set_oracle_evicts_existing_oracles_with_events() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle1 = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle1);
    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);

    // Add a second oracle
    let oracle2 = Address::generate(&e);
    engine_client.add_oracle(&admin, &oracle2);

    let new_oracle = Address::generate(&e);

    let events_before = e.events().all().count();

    // Call set_oracle
    engine_client.set_oracle(&admin, &new_oracle);

    let events = e.events().all().collect::<Vec<_>>();
    let new_events = &events[events_before..];

    // Expect 2 removals + 1 addition
    assert_eq!(new_events.len(), 3);

    // Filter removal events by topic
    let removal_events: Vec<_> = new_events.iter()
        .filter(|ev| {
            if let Some(topic) = ev.topics().first() {
                topic.clone().try_into().map(|s: Symbol| s == Symbol::new(&e, "OracleRemovedEvent")).unwrap_or(false)
            } else {
                false
            }
        })
        .collect();
    let addition_events: Vec<_> = new_events.iter()
        .filter(|ev| {
            if let Some(topic) = ev.topics().first() {
                topic.clone().try_into().map(|s: Symbol| s == Symbol::new(&e, "OracleAddedEvent")).unwrap_or(false)
            } else {
                false
            }
        })
        .collect();

    assert_eq!(removal_events.len(), 2);
    assert_eq!(addition_events.len(), 1);

    // Check evicted oracles
    let removed_addresses: Vec<Address> = removal_events.iter()
        .map(|ev| ev.topics().get(1).unwrap().clone().try_into().unwrap())
        .collect();
    assert!(removed_addresses.contains(&oracle1));
    assert!(removed_addresses.contains(&oracle2));

    // Check added oracle
    let added_address = addition_events[0].topics().get(1).unwrap().clone().try_into().unwrap();
    assert_eq!(added_address, new_oracle);

    // Verify final roster
    let oracles = engine_client.get_oracles();
    assert_eq!(oracles.len(), 1);
    assert_eq!(oracles.get(0).unwrap(), new_oracle);
}

#[test]
fn test_set_oracle_with_existing_oracle_removes_others() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let oracle1 = Address::generate(&e);
    let user = Address::generate(&e);

    let token_id = deploy_token(&e, &admin);
    let reg_id = deploy_registry(&e, &admin);
    let engine_id = deploy_engine(&e, &admin, &token_id, &reg_id, &oracle1);
    let engine_client = reward_engine::RewardEngineClient::new(&e, &engine_id);

    let oracle2 = Address::generate(&e);
    engine_client.add_oracle(&admin, &oracle2);

    let events_before = e.events().all().count();

    // Set to oracle1 (already present)
    engine_client.set_oracle(&admin, &oracle1);

    let events = e.events().all().collect::<Vec<_>>();
    let new_events = &events[events_before..];

    // Expect 1 removal (oracle2) + 1 addition (oracle1)
    assert_eq!(new_events.len(), 2);

    let removal_events: Vec<_> = new_events.iter()
        .filter(|ev| {
            if let Some(topic) = ev.topics().first() {
                topic.clone().try_into().map(|s: Symbol| s == Symbol::new(&e, "OracleRemovedEvent")).unwrap_or(false)
            } else {
                false
            }
        })
        .collect();
    let addition_events: Vec<_> = new_events.iter()
        .filter(|ev| {
            if let Some(topic) = ev.topics().first() {
                topic.clone().try_into().map(|s: Symbol| s == Symbol::new(&e, "OracleAddedEvent")).unwrap_or(false)
            } else {
                false
            }
        })
        .collect();

    assert_eq!(removal_events.len(), 1);
    assert_eq!(addition_events.len(), 1);

    let removed_address = removal_events[0].topics().get(1).unwrap().clone().try_into().unwrap();
    assert_eq!(removed_address, oracle2);
    let added_address = addition_events[0].topics().get(1).unwrap().clone().try_into().unwrap();
    assert_eq!(added_address, oracle1);

    let oracles = engine_client.get_oracles();
    assert_eq!(oracles.len(), 1);
    assert_eq!(oracles.get(0).unwrap(), oracle1);
}
