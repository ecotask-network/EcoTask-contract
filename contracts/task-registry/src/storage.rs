use soroban_sdk::{contracttype, Address, BytesN, Env, String, Vec};

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum TaskStatus {
    Active,
    Completed,
    Expired,
    Cancelled,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Task {
    pub id: u64,
    pub creator: Address,
    pub task_type: String,
    pub location_hash: BytesN<32>,
    pub reward_amount: i128,
    pub max_completions: u32,
    pub completions: u32,
    pub status: TaskStatus,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub enum DataKey {
    Task(u64),
    TaskCount,
    Admin,
    Sponsor(Address),
    Completion(u64, Address),
    CreatorTasks(Address),
}

/// Writes a task to persistent storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task` - The task to store
pub fn write_task(e: &Env, task: &Task) {
    let key = DataKey::Task(task.id);
    e.storage().persistent().set(&key, task);
}

/// Reads a task from persistent storage by its ID.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task to retrieve
///
/// # Returns
///
/// The task if it exists, or None if no task with that ID exists.
pub fn read_task(e: &Env, task_id: u64) -> Option<Task> {
    let key = DataKey::Task(task_id);
    e.storage().persistent().get(&key)
}

/// Gets the next available task ID and increments the counter.
///
/// # Arguments
///
/// * `e` - The Soroban environment
///
/// # Returns
///
/// The next task ID (current count before increment).
pub fn next_task_id(e: &Env) -> u64 {
    let key = DataKey::TaskCount;
    let count: u64 = e.storage().instance().get(&key).unwrap_or(0);
    e.storage().instance().set(&key, &(count + 1));
    count
}

/// Reads the number of tasks created so far (the next available task id)
/// without mutating any storage. This is safe to call repeatedly.
pub fn read_task_count(e: &Env) -> u64 {
    let key = DataKey::TaskCount;
    e.storage().instance().get(&key).unwrap_or(0)
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

/// Adds a sponsor address to the approved sponsors list.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `sponsor` - The sponsor address to add
pub fn add_sponsor(e: &Env, sponsor: &Address) {
    let key = DataKey::Sponsor(sponsor.clone());
    e.storage().persistent().set(&key, &true);
}

/// Removes a sponsor address from the approved sponsors list.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `sponsor` - The sponsor address to remove
pub fn remove_sponsor(e: &Env, sponsor: &Address) {
    let key = DataKey::Sponsor(sponsor.clone());
    e.storage().persistent().remove(&key);
}

/// Checks if an address is an approved sponsor.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `addr` - The address to check
///
/// # Returns
///
/// true if the address is a sponsor, false otherwise.
pub fn is_sponsor(e: &Env, addr: &Address) -> bool {
    let key = DataKey::Sponsor(addr.clone());
    e.storage().persistent().has(&key)
}

/// Marks a (task_id, user) pair as completed in storage.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task
/// * `user` - The user address that completed the task
pub fn mark_completed(e: &Env, task_id: u64, user: &Address) {
    let key = DataKey::Completion(task_id, user.clone());
    e.storage().persistent().set(&key, &true);
}

/// Checks if a user has completed a specific task.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `task_id` - The ID of the task to check
/// * `user` - The user address to check
///
/// # Returns
///
/// true if the user has completed the task, false otherwise.
pub fn is_completed(e: &Env, task_id: u64, user: &Address) -> bool {
    let key = DataKey::Completion(task_id, user.clone());
    e.storage().persistent().get(&key).unwrap_or(false)
}

/// Adds a task ID to a creator's list of created tasks.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `creator` - The address of the task creator
/// * `task_id` - The ID of the task to add to the creator's list
pub fn push_creator_task(e: &Env, creator: &Address, task_id: u64) {
    let key = DataKey::CreatorTasks(creator.clone());
    let mut ids: Vec<u64> = e.storage().persistent().get(&key).unwrap_or(Vec::new(e));
    ids.push_back(task_id);
    e.storage().persistent().set(&key, &ids);
}

/// Reads all task IDs created by a specific creator.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `creator` - The address of the creator to query
///
/// # Returns
///
/// A vector of task IDs created by the creator, or an empty vector if none exist.
pub fn read_creator_tasks(e: &Env, creator: &Address) -> Vec<u64> {
    let key = DataKey::CreatorTasks(creator.clone());
    e.storage().persistent().get(&key).unwrap_or(Vec::new(e))
}
