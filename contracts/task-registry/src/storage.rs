use ecotask_types::Task;
use soroban_sdk::{contracttype, Address, Env, Vec};

/// Persistent storage TTL: re-bump on every touch; 4,096 ledgers (~5.7 days)
/// provides ample headroom since entries are refreshed on every interaction.
pub const PERSISTENT_TTL_THRESHOLD: u32 = 100;
pub const PERSISTENT_TTL_EXTEND_TO: u32 = 4_096;

/// Instance storage TTL: must survive the longest realistic quiet period
/// (holiday, upstream outage). 535,680 ledgers = 31 days at ~5 s/ledger.
pub const INSTANCE_TTL_THRESHOLD: u32 = 100;
pub const INSTANCE_TTL_EXTEND_TO: u32 = 535_680;

/// Extends the TTL of the contract instance (and code) to
/// `INSTANCE_TTL_EXTEND_TO` ledgers when it is within
/// `INSTANCE_TTL_THRESHOLD` ledgers of expiring.
///
/// Called as the first operation of every public entry point so that any
/// interaction with the registry keeps its configuration alive.
pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
}

#[derive(Clone, Debug)]
#[contracttype]
pub enum DataKey {
    Task(u64),
    TaskCount,
    Admin,
    Sponsor(Address),
    Completion(u64, Address),
    /// Deprecated: old unbounded Vec layout (no longer written; retained in enum
    /// solely to prevent storage-key collisions with any existing deployed data).
    /// New code uses `CreatorTaskCount` + `CreatorTask` instead.
    CreatorTasks(Address),
    /// Number of tasks recorded for a given creator address. Used together with
    /// `CreatorTask(Address, index)` to implement O(1) per-task writes.
    CreatorTaskCount(Address),
    /// The task id stored at position `index` (0-based) in a creator's task list.
    /// Index is in range `0 .. CreatorTaskCount(creator)`.
    CreatorTask(Address, u64),
    PendingAdmin,
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
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
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

pub fn write_pending_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&DataKey::PendingAdmin, admin);
}

pub fn read_pending_admin(e: &Env) -> Option<Address> {
    e.storage().instance().get(&DataKey::PendingAdmin)
}

pub fn remove_pending_admin(e: &Env) {
    e.storage().instance().remove(&DataKey::PendingAdmin);
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
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
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
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_EXTEND_TO);
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

/// Returns the number of tasks created by `creator`.
///
/// This is O(1): it reads one storage entry regardless of how many tasks the
/// creator has accumulated.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `creator` - The address whose task count is requested
///
/// # Returns
///
/// The number of tasks registered for this creator (0 if none).
pub fn read_creator_task_count(e: &Env, creator: &Address) -> u64 {
    let key = DataKey::CreatorTaskCount(creator.clone());
    e.storage().persistent().get(&key).unwrap_or(0)
}

/// Appends `task_id` to `creator`'s indexed task list in O(1) storage work.
///
/// The new entry is written at index `CreatorTaskCount(creator)` and the counter
/// is then incremented by one. The total work is exactly:
///   1 persistent read  (CreatorTaskCount)
///   1 persistent write (CreatorTask(creator, index))
///   1 persistent write (CreatorTaskCount)
///
/// This is independent of how many tasks the creator already has, which fixes
/// the unbounded read-modify-write that previously affected the old
/// `CreatorTasks(creator) -> Vec<u64>` layout.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `creator` - The address of the task creator
/// * `task_id` - The ID of the task to append
pub fn push_creator_task(e: &Env, creator: &Address, task_id: u64) {
    let count_key = DataKey::CreatorTaskCount(creator.clone());
    let index: u64 = e.storage().persistent().get(&count_key).unwrap_or(0);

    let entry_key = DataKey::CreatorTask(creator.clone(), index);
    e.storage().persistent().set(&entry_key, &task_id);
    e.storage().persistent().extend_ttl(
        &entry_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );

    e.storage().persistent().set(&count_key, &(index + 1));
    e.storage().persistent().extend_ttl(
        &count_key,
        PERSISTENT_TTL_THRESHOLD,
        PERSISTENT_TTL_EXTEND_TO,
    );
}

/// Reads up to `limit` task IDs for `creator` starting at `offset` (0-based).
///
/// Only the storage entries needed for the requested page are read; the entire
/// creator history is never deserialized. Reading stops when the end of the
/// list is reached even if `limit` has not been met.
///
/// # Arguments
///
/// * `e` - The Soroban environment
/// * `creator` - The address of the creator to query
/// * `offset` - Zero-based starting position in the creator's task list
/// * `limit` - Maximum number of entries to return
///
/// # Returns
///
/// A `Vec<u64>` of up to `limit` task IDs.
pub fn read_creator_tasks_paged(e: &Env, creator: &Address, offset: u64, limit: u64) -> Vec<u64> {
    let count = read_creator_task_count(e, creator);
    let mut result: Vec<u64> = Vec::new(e);

    if offset >= count || limit == 0 {
        return result;
    }

    let end = (offset + limit).min(count);
    let mut index = offset;
    while index < end {
        let entry_key = DataKey::CreatorTask(creator.clone(), index);
        if let Some(task_id) = e.storage().persistent().get::<DataKey, u64>(&entry_key) {
            result.push_back(task_id);
        }
        index += 1;
    }

    result
}
