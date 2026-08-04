use soroban_sdk::{contracttype, Address, Env, String, Vec};

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
    Oracle,
    Verification(u64, Address),
    MinReward,
    MaxReward,
    VerificationList,
    TotalPaid,
    Paused,
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

pub fn write_oracle(e: &Env, oracle: &Address) {
    let key = DataKey::Oracle;
    e.storage().instance().set(&key, oracle);
}

pub fn read_oracle(e: &Env) -> Address {
    let key = DataKey::Oracle;
    e.storage().instance().get(&key).unwrap()
}

pub fn write_verification(e: &Env, task_id: u64, user: &Address, v: &Verification) {
    let key = DataKey::Verification(task_id, user.clone());
    e.storage().persistent().set(&key, v);
}

pub fn read_verification(e: &Env, task_id: u64, user: &Address) -> Option<Verification> {
    let key = DataKey::Verification(task_id, user.clone());
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

pub fn set_paused(e: &Env, paused: bool) {
    e.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn is_paused(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}
