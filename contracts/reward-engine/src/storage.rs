use soroban_sdk::{contracttype, Address, BytesN, Env, String, Vec};

const CID_INDEX_TTL_THRESHOLD: u32 = 100;
const CID_INDEX_TTL_EXTEND_TO: u32 = 4096;

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
    pub oracle: Address,
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
    VerificationList,
    UserVerifications(Address),
    TotalPaid,
    Paused,
    UserCooldown,
    LastRewardLedger(Address),
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct VerificationKey {
    pub task_id: u64,
    pub user: Address,
}

pub fn write_admin(e: &Env, admin: &Address) {
    let key = DataKey::Admin;
    e.storage().instance().set(&key, admin);
}

pub fn read_admin(e: &Env) -> Address {
    let key = DataKey::Admin;
    e.storage().instance().get(&key).unwrap()
}

pub fn has_admin(e: &Env) -> bool {
    let key = DataKey::Admin;
    e.storage().instance().has(&key)
}

pub fn write_token(e: &Env, token: &Address) {
    let key = DataKey::Token;
    e.storage().instance().set(&key, token);
}

pub fn read_token(e: &Env) -> Address {
    let key = DataKey::Token;
    e.storage().instance().get(&key).unwrap()
}

pub fn write_registry(e: &Env, registry: &Address) {
    let key = DataKey::Registry;
    e.storage().instance().set(&key, registry);
}

pub fn read_registry(e: &Env) -> Address {
    let key = DataKey::Registry;
    e.storage().instance().get(&key).unwrap()
}

pub fn write_oracles(e: &Env, oracles: &Vec<Address>) {
    let key = DataKey::Oracles;
    e.storage().instance().set(&key, oracles);
}

pub fn read_oracles(e: &Env) -> Vec<Address> {
    let key = DataKey::Oracles;
    e.storage().instance().get(&key).unwrap_or(Vec::new(e))
}

pub fn push_oracle(e: &Env, oracle: &Address) {
    let key = DataKey::Oracles;
    let mut list: Vec<Address> = e.storage().instance().get(&key).unwrap_or(Vec::new(e));
    list.push_back(oracle.clone());
    e.storage().instance().set(&key, &list);
}

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

pub fn is_registered_oracle(e: &Env, addr: &Address) -> bool {
    let oracles = read_oracles(e);
    oracles.iter().any(|o| o == *addr)
}

pub fn write_verification(e: &Env, task_id: u64, user: &Address, v: &Verification) {
    let key = DataKey::Verification(task_id, user.clone());
    e.storage().persistent().set(&key, v);
}

pub fn read_verification(e: &Env, task_id: u64, user: &Address) -> Option<Verification> {
    let key = DataKey::Verification(task_id, user.clone());
    e.storage().persistent().get(&key)
}

pub fn write_cid_index(e: &Env, cid_hash: &BytesN<32>, verification_key: &VerificationKey) {
    let key = DataKey::CidHash(cid_hash.clone());
    e.storage().persistent().set(&key, verification_key);
    e.storage()
        .persistent()
        .extend_ttl(&key, CID_INDEX_TTL_THRESHOLD, CID_INDEX_TTL_EXTEND_TO);
}

pub fn read_cid_index(e: &Env, cid_hash: &BytesN<32>) -> Option<VerificationKey> {
    let key = DataKey::CidHash(cid_hash.clone());
    e.storage().persistent().get(&key)
}

pub fn write_reward_range(e: &Env, min: i128, max: i128) {
    e.storage().instance().set(&DataKey::MinReward, &min);
    e.storage().instance().set(&DataKey::MaxReward, &max);
}

pub fn read_min_reward(e: &Env) -> Option<i128> {
    e.storage().instance().get(&DataKey::MinReward)
}

pub fn read_max_reward(e: &Env) -> Option<i128> {
    e.storage().instance().get(&DataKey::MaxReward)
}

pub fn push_verification_key(e: &Env, task_id: u64, user: &Address) {
    let key = DataKey::VerificationList;
    let mut list: Vec<VerificationKey> = e.storage().instance().get(&key).unwrap_or(Vec::new(e));
    list.push_back(VerificationKey {
        task_id,
        user: user.clone(),
    });
    e.storage().instance().set(&key, &list);
}

pub fn remove_verification_key(e: &Env, task_id: u64, user: &Address) {
    let key = DataKey::VerificationList;
    let list: Vec<VerificationKey> = e.storage().instance().get(&key).unwrap_or(Vec::new(e));
    let mut filtered: Vec<VerificationKey> = Vec::new(e);
    for item in list.iter() {
        if item.task_id != task_id || item.user != *user {
            filtered.push_back(item);
        }
    }
    e.storage().instance().set(&key, &filtered);
}

pub fn read_verification_keys(e: &Env) -> Vec<VerificationKey> {
    let key = DataKey::VerificationList;
    e.storage().instance().get(&key).unwrap_or(Vec::new(e))
}

pub fn push_user_verification_key(e: &Env, user: &Address, task_id: u64) {
    let key = DataKey::UserVerifications(user.clone());
    let mut list: Vec<u64> = e.storage().persistent().get(&key).unwrap_or(Vec::new(e));
    list.push_back(task_id);
    e.storage().persistent().set(&key, &list);
}

pub fn read_user_verification_tasks(e: &Env, user: &Address) -> Vec<u64> {
    let key = DataKey::UserVerifications(user.clone());
    e.storage().persistent().get(&key).unwrap_or(Vec::new(e))
}

pub fn add_total_paid(e: &Env, amount: i128) {
    let key = DataKey::TotalPaid;
    let current: i128 = e.storage().instance().get(&key).unwrap_or(0);
    e.storage().instance().set(
        &key,
        &(current.checked_add(amount).expect("total_paid overflow")),
    );
}

pub fn read_total_paid(e: &Env) -> i128 {
    let key = DataKey::TotalPaid;
    e.storage().instance().get(&key).unwrap_or(0)
}

/// Sets the minimum number of ledgers a user must wait between two reward
/// approvals. A value of 0 disables the cooldown entirely.
pub fn write_user_cooldown(e: &Env, ledgers: u64) {
    e.storage().instance().set(&DataKey::UserCooldown, &ledgers);
}

pub fn read_user_cooldown(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::UserCooldown)
        .unwrap_or(0)
}

pub fn write_last_reward_ledger(e: &Env, user: &Address, ledger: u64) {
    let key = DataKey::LastRewardLedger(user.clone());
    e.storage().persistent().set(&key, &ledger);
}

/// Returns `None` if the user has never received a reward, distinguishing
/// that case from a legitimate ledger value of 0.
pub fn read_last_reward_ledger(e: &Env, user: &Address) -> Option<u64> {
    let key = DataKey::LastRewardLedger(user.clone());
    e.storage().persistent().get(&key)
}

pub fn set_paused(e: &Env, paused: bool) {
    e.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn is_paused(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}
