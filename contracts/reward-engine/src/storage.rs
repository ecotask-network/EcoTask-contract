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
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct VerificationKey {
    pub task_id: u64,
    pub user: Address,
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
        .extend_ttl(&key, CID_INDEX_TTL_THRESHOLD, CID_INDEX_TTL_EXTEND_TO);
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

/// Adds a verification key to the global verification list.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task
/// * `user` - The user address
pub fn push_verification_key(e: &Env, task_id: u64, user: &Address) {
    let key = DataKey::VerificationList;
    let mut list: Vec<VerificationKey> = e.storage().instance().get(&key).unwrap_or(Vec::new(e));
    list.push_back(VerificationKey {
        task_id,
        user: user.clone(),
    });
    e.storage().instance().set(&key, &list);
}

/// Removes a verification key from the global verification list.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task
/// * `user` - The user address
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

/// Reads all verification keys from the global verification list.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// A vector of all verification keys, or an empty vector if none exist.
pub fn read_verification_keys(e: &Env) -> Vec<VerificationKey> {
    let key = DataKey::VerificationList;
    e.storage().instance().get(&key).unwrap_or(Vec::new(e))
}

/// Adds a task ID to a user's list of verification tasks.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `user` - The user address
/// * `task_id` - The ID of the task to add
pub fn push_user_verification_key(e: &Env, user: &Address, task_id: u64) {
    let key = DataKey::UserVerifications(user.clone());
    let mut list: Vec<u64> = e.storage().persistent().get(&key).unwrap_or(Vec::new(e));
    list.push_back(task_id);
    e.storage().persistent().set(&key, &list);
}

/// Reads all task IDs for a user's verifications.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `user` - The user address to query
///
/// # Returns
///
/// A vector of task IDs, or an empty vector if the user has no verifications.
pub fn read_user_verification_tasks(e: &Env, user: &Address) -> Vec<u64> {
    let key = DataKey::UserVerifications(user.clone());
    e.storage().persistent().get(&key).unwrap_or(Vec::new(e))
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
