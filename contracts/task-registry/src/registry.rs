use crate::{access, storage};
use soroban_sdk::{contract, contractevent, contractimpl, Address, BytesN, Env, String};
pub use storage::{Task, TaskStatus};

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskCreatedEvent {
    #[topic]
    pub creator: Address,
    pub task_id: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskCompletedEvent {
    #[topic]
    pub user: Address,
    pub task_id: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskExpiredEvent {
    pub task_id: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskCancelledEvent {
    #[topic]
    pub creator: Address,
    pub task_id: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskExtendedEvent {
    #[topic]
    pub creator: Address,
    pub task_id: u64,
    pub new_expires_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SponsorAddedEvent {
    #[topic]
    pub sponsor: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SponsorRemovedEvent {
    #[topic]
    pub sponsor: Address,
}

#[contract]
pub struct RegistryContract;

#[contractimpl]
impl RegistryContract {
    /// Initializes the registry contract with an admin address.
    ///
    /// # Arguments
    ///
    /// * `admin` - The initial administrator address
    ///
    /// # Panics
    ///
    /// Panics if the contract has already been initialized.
    ///
    /// # Auth
    ///
    /// No authentication required. Can only be called once during deployment.
    pub fn initialize(e: Env, admin: Address) {
        if storage::has_admin(&e) {
            panic!("registry: already initialized");
        }
        storage::write_admin(&e, &admin);
    }

    /// Adds a sponsor address to the approved sponsors list.
    ///
    /// Sponsors are authorized to create and complete tasks.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `sponsor` - The address to add as a sponsor
    ///
    /// # Panics
    ///
    /// Panics if caller is not the admin.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn add_sponsor(e: Env, caller: Address, sponsor: Address) {
        caller.require_auth();
        access::require_admin(&e, &caller);
        storage::add_sponsor(&e, &sponsor);
        SponsorAddedEvent { sponsor }.publish(&e);
    }

    /// Removes a sponsor address from the approved sponsors list.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `sponsor` - The address to remove from sponsors
    ///
    /// # Panics
    ///
    /// Panics if caller is not the admin.
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn remove_sponsor(e: Env, caller: Address, sponsor: Address) {
        caller.require_auth();
        access::require_admin(&e, &caller);
        storage::remove_sponsor(&e, &sponsor);
        SponsorRemovedEvent { sponsor }.publish(&e);
    }

    /// Creates a new task in the registry.
    ///
    /// # Arguments
    ///
    /// * `creator` - The address creating the task (must be a sponsor or admin)
    /// * `task_type` - A string describing the type of task (e.g., "tree-planting")
    /// * `location_hash` - SHA-256 hash of the task location (32 bytes)
    /// * `reward_amount` - The maximum ECO reward per completion (must be positive)
    /// * `max_completions` - The maximum number of times this task can be completed (must be positive)
    /// * `expires_at` - The ledger timestamp when this task expires (must be in the future)
    ///
    /// # Returns
    ///
    /// The ID of the newly created task.
    ///
    /// # Panics
    ///
    /// * Panics if `task_type` is empty
    /// * Panics if `reward_amount <= 0`
    /// * Panics if `max_completions == 0`
    /// * Panics if `expires_at` is in the past
    /// * Panics if creator is not a sponsor or admin
    ///
    /// # Auth
    ///
    /// Requires authentication from the creator address, which must be a sponsor or admin.
    pub fn create_task(
        e: Env,
        creator: Address,
        task_type: String,
        location_hash: BytesN<32>,
        reward_amount: i128,
        max_completions: u32,
        expires_at: u64,
    ) -> u64 {
        creator.require_auth();
        access::require_sponsor(&e, &creator);

        if task_type.is_empty() {
            panic!("registry: task type must not be empty");
        }
        if reward_amount <= 0 {
            panic!("registry: reward must be positive");
        }
        if max_completions == 0 {
            panic!("registry: max completions must be positive");
        }
        if expires_at <= e.ledger().timestamp() {
            panic!("registry: expiry must be in the future");
        }

        let task_id = storage::next_task_id(&e);

        let task = Task {
            id: task_id,
            creator: creator.clone(),
            task_type,
            location_hash,
            reward_amount,
            max_completions,
            completions: 0,
            status: TaskStatus::Active,
            created_at: e.ledger().timestamp(),
            expires_at,
        };

        storage::write_task(&e, &task);

        storage::push_creator_task(&e, &creator, task_id);

        TaskCreatedEvent { creator, task_id }.publish(&e);

        task_id
    }

    /// Retrieves a task by its ID.
    ///
    /// The returned status is the last value written to storage. An active task whose
    /// deadline has passed still reads as active until an expiry function persists the
    /// change. Use `get_task_live_status` to obtain its effective status.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to retrieve
    ///
    /// # Returns
    ///
    /// The task with its stored status.
    ///
    /// # Panics
    ///
    /// Panics if no task exists with the given ID.
    ///
    /// # Auth
    ///
    /// No authentication is required.
    pub fn get_task(e: Env, task_id: u64) -> Task {
        match storage::read_task(&e, task_id) {
            Some(task) => task,
            None => panic!("registry: task not found"),
        }
    }

    /// Retrieves a task with its effective status at the current ledger timestamp.
    ///
    /// Unlike `get_task`, this reports an active task as expired after its deadline,
    /// even if the stored status has not been updated. This function is read-only; use
    /// `expire_task_permissionless` to persist the expired status.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to retrieve
    ///
    /// # Returns
    ///
    /// The task with its effective status.
    ///
    /// # Panics
    ///
    /// Panics if no task exists with the given ID.
    ///
    /// # Auth
    ///
    /// No authentication is required.
    pub fn get_task_live_status(e: Env, task_id: u64) -> Task {
        let mut task = match storage::read_task(&e, task_id) {
            Some(task) => task,
            None => panic!("registry: task not found"),
        };

        if task.status == TaskStatus::Active && task.expires_at < e.ledger().timestamp() {
            task.status = TaskStatus::Expired;
        }

        task
    }

    /// Marks a task as completed for a specific user.
    ///
    /// This records that the user has successfully completed the task and increments
    /// the task's completion count. If the task reaches its max completions, its
    /// status is changed to Completed.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address marking the task as complete (must be a sponsor or admin)
    /// * `task_id` - The ID of the task to mark as complete
    /// * `user` - The user address that completed the task
    ///
    /// # Panics
    ///
    /// * Panics if the task does not exist
    /// * Panics if the task creator's sponsor status has been revoked
    /// * Panics if the task is not Active
    /// * Panics if the task has expired
    /// * Panics if the user has already completed this task (double-claim prevention)
    /// * Panics if the task has reached its max completions
    ///
    /// # Auth
    ///
    /// Requires authentication from the caller address, which must be a sponsor or admin.
    pub fn complete_task(e: Env, caller: Address, task_id: u64, user: Address) {
        caller.require_auth();
        access::require_sponsor(&e, &caller);

        let mut task = match storage::read_task(&e, task_id) {
            Some(task) => task,
            None => panic!("registry: task not found"),
        };

        let admin = storage::read_admin(&e);
        if task.creator != admin && !storage::is_sponsor(&e, &task.creator) {
            panic!("registry: sponsor revoked");
        }

        if task.status != TaskStatus::Active {
            panic!("registry: task is not active");
        }
        if task.expires_at < e.ledger().timestamp() {
            panic!("registry: task expired");
        }
        if storage::is_completed(&e, task_id, &user) {
            panic!("registry: already completed");
        }
        if task.completions >= task.max_completions {
            panic!("registry: max completions reached");
        }

        task.completions += 1;
        if task.completions >= task.max_completions {
            task.status = TaskStatus::Completed;
        }

        storage::write_task(&e, &task);
        storage::mark_completed(&e, task_id, &user);

        TaskCompletedEvent { user, task_id }.publish(&e);
    }

    /// Force-expires an active task.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `task_id` - The ID of the task to expire
    ///
    /// # Panics
    ///
    /// * Panics if the task does not exist
    /// * Panics if the task is not Active
    /// * Panics if caller is not the admin
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn expire_task(e: Env, caller: Address, task_id: u64) {
        caller.require_auth();
        access::require_admin(&e, &caller);

        let mut task = match storage::read_task(&e, task_id) {
            Some(task) => task,
            None => panic!("registry: task not found"),
        };

        if task.status != TaskStatus::Active {
            panic!("registry: task is not active");
        }

        task.status = TaskStatus::Expired;
        storage::write_task(&e, &task);
    }

    /// Persists an active task as expired after its deadline has passed.
    ///
    /// This permissionless operation allows any indexer or user to synchronize the
    /// stored status without waiting for the administrator to call `expire_task`.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to expire
    ///
    /// # Panics
    ///
    /// * Panics if the task does not exist
    /// * Panics if the task is not Active
    /// * Panics if the task's deadline has not passed
    ///
    /// # Auth
    ///
    /// No authentication is required.
    pub fn expire_task_permissionless(e: Env, task_id: u64) {
        let mut task = match storage::read_task(&e, task_id) {
            Some(task) => task,
            None => panic!("registry: task not found"),
        };

        if task.status != TaskStatus::Active {
            panic!("registry: task is not active");
        }
        if task.expires_at >= e.ledger().timestamp() {
            panic!("registry: task not yet expired");
        }

        task.status = TaskStatus::Expired;
        storage::write_task(&e, &task);

        TaskExpiredEvent { task_id }.publish(&e);
    }

    /// Extends the expiry of an active task. Callable by the task creator or
    /// the admin. The new expiry must be strictly later than the current one
    /// (i.e. a genuine extension) and still in the future.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be task creator or admin)
    /// * `task_id` - The ID of the task to extend
    /// * `new_expires_at` - The new expiry timestamp (must be > current expiry and in the future)
    ///
    /// # Panics
    ///
    /// * Panics if the task does not exist
    /// * Panics if caller is not the task creator or admin
    /// * Panics if the task is not Active
    /// * Panics if `new_expires_at` is in the past
    /// * Panics if `new_expires_at` does not extend the current expiry
    ///
    /// # Auth
    ///
    /// Requires authentication from the caller address.
    pub fn extend_task_expiry(e: Env, caller: Address, task_id: u64, new_expires_at: u64) {
        caller.require_auth();
        let admin = storage::read_admin(&e);

        let mut task = match storage::read_task(&e, task_id) {
            Some(task) => task,
            None => panic!("registry: task not found"),
        };

        if caller != admin && task.creator != caller {
            panic!("registry: unauthorized");
        }
        if task.status != TaskStatus::Active {
            panic!("registry: task is not active");
        }
        if new_expires_at <= e.ledger().timestamp() {
            panic!("registry: expiry must be in the future");
        }
        if new_expires_at <= task.expires_at {
            panic!("registry: new expiry must extend the current one");
        }

        task.expires_at = new_expires_at;
        storage::write_task(&e, &task);

        TaskExtendedEvent {
            creator: task.creator,
            task_id,
            new_expires_at,
        }
        .publish(&e);
    }

    /// Cancels an active task. Only the task creator can cancel their own task.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be the task creator)
    /// * `task_id` - The ID of the task to cancel
    ///
    /// # Panics
    ///
    /// * Panics if the task does not exist
    /// * Panics if caller is not the task creator
    /// * Panics if the task is not Active
    ///
    /// # Auth
    ///
    /// Requires authentication from the caller address.
    pub fn cancel_task(e: Env, caller: Address, task_id: u64) {
        caller.require_auth();

        let mut task = match storage::read_task(&e, task_id) {
            Some(task) => task,
            None => panic!("registry: task not found"),
        };

        if task.creator != caller {
            panic!("registry: unauthorized");
        }
        if task.status != TaskStatus::Active {
            panic!("registry: task is not active");
        }

        task.status = TaskStatus::Cancelled;
        storage::write_task(&e, &task);

        TaskCancelledEvent {
            creator: task.creator,
            task_id,
        }
        .publish(&e);
    }

    /// Cancels any active task. Admin-only governance function.
    ///
    /// # Arguments
    ///
    /// * `caller` - The address invoking the function (must be admin)
    /// * `task_id` - The ID of the task to cancel
    ///
    /// # Panics
    ///
    /// * Panics if the task does not exist
    /// * Panics if the task is not Active
    /// * Panics if caller is not the admin
    ///
    /// # Auth
    ///
    /// Requires authentication from the admin address.
    pub fn admin_cancel_task(e: Env, caller: Address, task_id: u64) {
        caller.require_auth();
        access::require_admin(&e, &caller);

        let mut task = match storage::read_task(&e, task_id) {
            Some(task) => task,
            None => panic!("registry: task not found"),
        };

        if task.status != TaskStatus::Active {
            panic!("registry: task is not active");
        }

        task.status = TaskStatus::Cancelled;
        storage::write_task(&e, &task);

        TaskCancelledEvent {
            creator: task.creator,
            task_id,
        }
        .publish(&e);
    }

    /// Returns the total number of tasks created.
    ///
    /// # Returns
    ///
    /// The count of tasks created (also the next available task ID).
    pub fn task_count(e: Env) -> u64 {
        storage::read_task_count(&e)
    }

    /// Checks if a specific user has completed a specific task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to check
    /// * `user` - The user address to check
    ///
    /// # Returns
    ///
    /// true if the user has completed the task, false otherwise.
    pub fn is_task_completed(e: Env, task_id: u64, user: Address) -> bool {
        storage::is_completed(&e, task_id, &user)
    }

    /// Returns task IDs created by a specific creator, capped at
    /// `MAX_CREATOR_TASKS_QUERY` entries.
    ///
    /// # Deprecation notice
    ///
    /// This function is retained for API compatibility but is **deprecated**.
    /// Callers should migrate to `get_tasks_by_creator_paged`, which reads only
    /// the entries needed for each page and scales correctly for any number of
    /// tasks. This unpaged variant returns at most `MAX_CREATOR_TASKS_QUERY`
    /// (50) results; creators with more tasks than that cap will not see their
    /// full history through this call.
    ///
    /// The cap is intentionally conservative. Soroban enforces a hard limit of
    /// 100 total ledger-entry footprint entries per transaction. Reading more
    /// than ~98 indexed `CreatorTask` entries in a single call would breach
    /// that limit on a real network. The 50-entry cap gives ample headroom
    /// while still being useful for small creator histories. Use
    /// `get_tasks_by_creator_paged` to retrieve larger histories safely.
    ///
    /// # Arguments
    ///
    /// * `creator` - The address of the creator to query
    ///
    /// # Returns
    ///
    /// Up to `MAX_CREATOR_TASKS_QUERY` task IDs created by the specified creator.
    pub fn get_tasks_by_creator(e: Env, creator: Address) -> soroban_sdk::Vec<u64> {
        /// Hard cap on the unpaged creator-task query. Must stay well below the
        /// Soroban per-transaction ledger-entry footprint limit (100 entries).
        /// Use `get_tasks_by_creator_paged` for creators with more than this
        /// many tasks.
        const MAX_CREATOR_TASKS_QUERY: u64 = 50;
        storage::read_creator_tasks_paged(&e, &creator, 0, MAX_CREATOR_TASKS_QUERY)
    }

    /// Pageable slice of the task IDs created by `creator`.
    ///
    /// `cursor` is the zero-based offset into the creator's indexed task list
    /// and `limit` caps the number of IDs returned. Only the storage entries
    /// for the requested page are read; the full creator history is never
    /// loaded. This is the canonical way to enumerate a creator's tasks.
    ///
    /// # Arguments
    ///
    /// * `creator` - The address of the creator to query
    /// * `cursor` - The zero-based offset into the creator's task list
    /// * `limit` - The maximum number of task IDs to return
    ///
    /// # Returns
    ///
    /// A vector of up to `limit` task IDs, starting from `cursor`.
    pub fn get_tasks_by_creator_paged(
        e: Env,
        creator: Address,
        cursor: u32,
        limit: u32,
    ) -> soroban_sdk::Vec<u64> {
        storage::read_creator_tasks_paged(&e, &creator, cursor as u64, limit as u64)
    }

    /// Pageable listing of every task in the registry ordered by id.
    /// `cursor` is the lowest task id to include (inclusive) and `limit` caps
    /// the number of tasks returned. Safe for off-chain indexers to paginate
    /// through without pulling the entire registry in one call.
    ///
    /// # Arguments
    ///
    /// * `cursor` - The starting task ID (inclusive)
    /// * `limit` - The maximum number of tasks to return
    ///
    /// # Returns
    ///
    /// A vector of up to `limit` Task structs, starting from `cursor`.
    pub fn list_tasks(e: Env, cursor: u64, limit: u32) -> soroban_sdk::Vec<Task> {
        let count = storage::read_task_count(&e);
        let mut tasks: soroban_sdk::Vec<Task> = soroban_sdk::Vec::new(&e);
        let mut current = cursor;
        let mut remaining = limit;
        while current < count && remaining > 0 {
            if let Some(task) = storage::read_task(&e, current) {
                tasks.push_back(task);
            }
            current += 1;
            remaining -= 1;
        }
        tasks
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
    pub fn transfer_admin(e: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();
        let stored_admin = storage::read_admin(&e);
        if current_admin != stored_admin {
            panic!("registry: unauthorized");
        }
        if new_admin == current_admin {
            panic!("registry: new admin must be different");
        }
        storage::write_admin(&e, &new_admin);
    }
}

#[cfg(test)]
mod test {
    use crate::{RegistryContract, RegistryContractClient, TaskStatus};
    use soroban_sdk::testutils::{Address as _, BytesN as _, Ledger as _};
    use soroban_sdk::{Address, BytesN, Env, String, Vec};

    fn setup() -> (Env, Address, RegistryContractClient<'static>) {
        let e = Env::default();
        let admin = Address::generate(&e);
        let contract_id = e.register(RegistryContract, ());
        let client = RegistryContractClient::new(&e, &contract_id);

        client.initialize(&admin);
        (e, admin, client)
    }

    fn create_test_task(
        client: &RegistryContractClient<'static>,
        creator: &Address,
        task_type: &String,
        max_completions: u32,
        expires_in: u64,
    ) -> u64 {
        let loc_hash: BytesN<32> = BytesN::random(&client.env);
        client.create_task(
            creator,
            task_type,
            &loc_hash,
            &1000,
            &max_completions,
            &(client.env.ledger().timestamp() + expires_in),
        )
    }

    #[test]
    fn test_create_and_get_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let task_id = create_test_task(&client, &admin, &task_type, 1, 1000);

        let task = client.get_task(&task_id);
        assert_eq!(task.id, task_id);
        assert_eq!(task.creator, admin);
        assert_eq!(task.task_type, task_type);
        assert_eq!(task.reward_amount, 1000);
        assert_eq!(task.max_completions, 1);
        assert_eq!(task.completions, 0);
        assert_eq!(task.status, TaskStatus::Active);
    }

    #[test]
    fn test_complete_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let user = Address::generate(&e);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "trash-collection"),
            1,
            1000,
        );

        client.complete_task(&admin, &task_id, &user);

        let task = client.get_task(&task_id);
        assert_eq!(task.completions, 1);
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(client.is_task_completed(&task_id, &user));
    }

    #[test]
    #[should_panic(expected = "registry: already completed")]
    fn test_double_claim_prevention() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let user = Address::generate(&e);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "ocean-cleanup"),
            2,
            1000,
        );

        client.complete_task(&admin, &task_id, &user);
        client.complete_task(&admin, &task_id, &user);
    }

    #[test]
    fn test_max_completions() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let user1 = Address::generate(&e);
        let user2 = Address::generate(&e);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            2,
            1000,
        );

        client.complete_task(&admin, &task_id, &user1);
        let task = client.get_task(&task_id);
        assert_eq!(task.completions, 1);
        assert_eq!(task.status, TaskStatus::Active);

        client.complete_task(&admin, &task_id, &user2);
        let task = client.get_task(&task_id);
        assert_eq!(task.completions, 2);
        assert_eq!(task.status, TaskStatus::Completed);
    }

    #[test]
    fn test_expire_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.expire_task(&admin, &task_id);

        let task = client.get_task(&task_id);
        assert_eq!(task.status, TaskStatus::Expired);
    }

    #[test]
    #[should_panic(expected = "registry: unauthorized")]
    fn test_unauthorized_creator() {
        let (e, _admin, client) = setup();
        e.mock_all_auths();

        let attacker = Address::generate(&e);

        let loc_hash: BytesN<32> = BytesN::random(&e);
        client.create_task(
            &attacker,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 1000),
        );
    }

    #[test]
    #[should_panic(expected = "registry: unauthorized")]
    fn test_unauthorized_expire() {
        let (e, _admin, client) = setup();
        e.mock_all_auths();

        let attacker = Address::generate(&e);
        client.expire_task(&attacker, &0);
    }

    #[test]
    fn test_permissionless_expire_past_deadline() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        e.ledger().set_timestamp(1000);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            500,
        );

        e.ledger().set_timestamp(2000);
        client.expire_task_permissionless(&task_id);

        let task = client.get_task(&task_id);
        assert_eq!(task.status, TaskStatus::Expired);
    }

    #[test]
    #[should_panic(expected = "registry: task not yet expired")]
    fn test_permissionless_expire_not_yet_expired() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.expire_task_permissionless(&task_id);
    }

    #[test]
    fn test_get_task_live_status_reflects_expiry() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        e.ledger().set_timestamp(1000);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            500,
        );

        e.ledger().set_timestamp(2000);

        // Storage is untouched until expire_task_permissionless runs.
        let stored = client.get_task(&task_id);
        assert_eq!(stored.status, TaskStatus::Active);

        // The live-status view reports the effective status instead.
        let live = client.get_task_live_status(&task_id);
        assert_eq!(live.status, TaskStatus::Expired);
    }

    #[test]
    fn test_add_sponsor() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let sponsor = Address::generate(&e);
        client.add_sponsor(&admin, &sponsor);

        let loc_hash: BytesN<32> = BytesN::random(&e);
        let task_id = client.create_task(
            &sponsor,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 1000),
        );

        let task = client.get_task(&task_id);
        assert_eq!(task.creator, sponsor);
    }

    #[test]
    fn test_remove_sponsor() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let sponsor = Address::generate(&e);
        client.add_sponsor(&admin, &sponsor);

        let loc_hash: BytesN<32> = BytesN::random(&e);
        client.create_task(
            &sponsor,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 1000),
        );

        client.remove_sponsor(&admin, &sponsor);
    }

    #[test]
    #[should_panic(expected = "registry: unauthorized")]
    fn test_removed_sponsor_cannot_create_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let sponsor = Address::generate(&e);
        client.add_sponsor(&admin, &sponsor);
        client.remove_sponsor(&admin, &sponsor);

        let loc_hash: BytesN<32> = BytesN::random(&e);
        client.create_task(
            &sponsor,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 1000),
        );
    }

    #[test]
    #[should_panic(expected = "registry: unauthorized")]
    fn test_remove_sponsor_non_admin() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let sponsor = Address::generate(&e);
        client.add_sponsor(&admin, &sponsor);

        let non_admin = Address::generate(&e);
        client.remove_sponsor(&non_admin, &sponsor);
    }

    #[test]
    #[should_panic(expected = "registry: task is not active")]
    fn test_expire_already_expired_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.expire_task(&admin, &task_id);
        client.expire_task(&admin, &task_id);
    }

    #[test]
    #[should_panic(expected = "registry: task expired")]
    fn test_expired_task_cannot_be_completed() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let user = Address::generate(&e);

        e.ledger().set_timestamp(1000);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        e.ledger().set_timestamp(3000);
        client.complete_task(&admin, &task_id, &user);
    }

    #[test]
    fn test_cancel_task_by_creator() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.cancel_task(&admin, &task_id);

        let task = client.get_task(&task_id);
        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    #[test]
    #[should_panic(expected = "registry: unauthorized")]
    fn test_cancel_task_not_creator() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        let other = Address::generate(&e);
        client.cancel_task(&other, &task_id);
    }

    #[test]
    #[should_panic(expected = "registry: task is not active")]
    fn test_cancel_already_completed_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let user = Address::generate(&e);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.complete_task(&admin, &task_id, &user);
        client.cancel_task(&admin, &task_id);
    }

    #[test]
    #[should_panic(expected = "registry: task is not active")]
    fn test_complete_cancelled_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let user = Address::generate(&e);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            2,
            1000,
        );

        client.cancel_task(&admin, &task_id);
        client.complete_task(&admin, &task_id, &user);
    }

    #[test]
    #[should_panic(expected = "registry: task is not active")]
    fn test_expire_cancelled_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.cancel_task(&admin, &task_id);
        client.expire_task(&admin, &task_id);
    }

    #[test]
    #[should_panic(expected = "registry: task type must not be empty")]
    fn test_create_task_empty_type() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let loc_hash: BytesN<32> = BytesN::random(&e);
        client.create_task(
            &admin,
            &String::from_str(&e, ""),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 1000),
        );
    }

    #[test]
    fn test_get_tasks_by_creator() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let id0 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let id1 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let id2 = create_test_task(&client, &admin, &task_type, 1, 1000);

        let ids = client.get_tasks_by_creator(&admin);
        assert_eq!(ids.len(), 3);
        assert_eq!(ids.get(0).unwrap(), id0);
        assert_eq!(ids.get(1).unwrap(), id1);
        assert_eq!(ids.get(2).unwrap(), id2);

        // A different creator should have an empty list
        let other = Address::generate(&e);
        let empty = client.get_tasks_by_creator(&other);
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_get_tasks_by_creator_paged() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let mut ids = Vec::new(&e);
        for _ in 0..5 {
            ids.push_back(create_test_task(&client, &admin, &task_type, 1, 1000));
        }

        // First page: 2 items starting at offset 0
        let page0 = client.get_tasks_by_creator_paged(&admin, &0, &2);
        assert_eq!(page0.len(), 2);
        assert_eq!(page0.get(0).unwrap(), ids.get(0).unwrap());
        assert_eq!(page0.get(1).unwrap(), ids.get(1).unwrap());

        // Second page: 2 items starting at offset 2
        let page1 = client.get_tasks_by_creator_paged(&admin, &2, &2);
        assert_eq!(page1.len(), 2);
        assert_eq!(page1.get(0).unwrap(), ids.get(2).unwrap());
        assert_eq!(page1.get(1).unwrap(), ids.get(3).unwrap());

        // Last page: remaining 1 item, then empty once the cursor passes the end
        let page2 = client.get_tasks_by_creator_paged(&admin, &4, &2);
        assert_eq!(page2.len(), 1);
        assert_eq!(page2.get(0).unwrap(), ids.get(4).unwrap());

        let empty = client.get_tasks_by_creator_paged(&admin, &10, &2);
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_list_tasks_pagination() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let mut ids = Vec::new(&e);
        for _ in 0..4 {
            ids.push_back(create_test_task(&client, &admin, &task_type, 1, 1000));
        }

        // Page 1: tasks 0..2
        let page0 = client.list_tasks(&0, &2);
        assert_eq!(page0.len(), 2);
        assert_eq!(page0.get(0).unwrap().id, ids.get(0).unwrap());
        assert_eq!(page0.get(1).unwrap().id, ids.get(1).unwrap());

        // Page 2: tasks 2..4
        let page1 = client.list_tasks(&2, &2);
        assert_eq!(page1.len(), 2);
        assert_eq!(page1.get(0).unwrap().id, ids.get(2).unwrap());
        assert_eq!(page1.get(1).unwrap().id, ids.get(3).unwrap());

        // Cursor past the end returns an empty page
        let page2 = client.list_tasks(&4, &10);
        assert_eq!(page2.len(), 0);

        // A limit of 0 returns an empty page without panicking
        let empty = client.list_tasks(&0, &0);
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_list_tasks_full_scan() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let id0 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let id1 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let id2 = create_test_task(&client, &admin, &task_type, 1, 1000);

        let all = client.list_tasks(&0, &u32::MAX);
        assert_eq!(all.len(), 3);
        assert_eq!(all.get(0).unwrap().id, id0);
        assert_eq!(all.get(1).unwrap().id, id1);
        assert_eq!(all.get(2).unwrap().id, id2);
    }

    #[test]
    fn test_task_count_is_stable() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        assert_eq!(client.task_count(), 0);

        let task_type = String::from_str(&e, "tree-planting");
        let id0 = create_test_task(&client, &admin, &task_type, 1, 1000);
        assert_eq!(id0, 0);
        assert_eq!(client.task_count(), 1);

        // Repeated reads must not advance the counter or corrupt task ids.
        assert_eq!(client.task_count(), 1);
        assert_eq!(client.task_count(), 1);

        let id1 = create_test_task(&client, &admin, &task_type, 1, 1000);
        assert_eq!(id1, 1);
        assert_eq!(client.task_count(), 2);
    }

    #[test]
    fn test_extend_task_expiry() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let now = e.ledger().timestamp();
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        let task = client.get_task(&task_id);
        assert_eq!(task.expires_at, now + 1000);

        client.extend_task_expiry(&admin, &task_id, &(now + 5000));

        let task = client.get_task(&task_id);
        assert_eq!(task.expires_at, now + 5000);
        assert_eq!(task.status, TaskStatus::Active);
    }

    #[test]
    fn test_extend_task_expiry_by_creator_sponsor() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let sponsor = Address::generate(&e);
        client.add_sponsor(&admin, &sponsor);

        let now = e.ledger().timestamp();
        let loc_hash: BytesN<32> = BytesN::random(&e);
        let task_id = client.create_task(
            &sponsor,
            &String::from_str(&e, "ocean-cleanup"),
            &loc_hash,
            &1000,
            &1,
            &(now + 1000),
        );

        client.extend_task_expiry(&sponsor, &task_id, &(now + 9000));

        let task = client.get_task(&task_id);
        assert_eq!(task.expires_at, now + 9000);
    }

    #[test]
    #[should_panic(expected = "registry: unauthorized")]
    fn test_extend_task_expiry_unauthorized() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let now = e.ledger().timestamp();
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        let attacker = Address::generate(&e);
        client.extend_task_expiry(&attacker, &task_id, &(now + 9000));
    }

    #[test]
    #[should_panic(expected = "registry: new expiry must extend the current one")]
    fn test_extend_task_expiry_must_increase() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let now = e.ledger().timestamp();
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.extend_task_expiry(&admin, &task_id, &(now + 500));
    }

    #[test]
    #[should_panic(expected = "registry: task is not active")]
    fn test_extend_expired_task_fails() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let now = e.ledger().timestamp();
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.expire_task(&admin, &task_id);
        client.extend_task_expiry(&admin, &task_id, &(now + 9000));
    }

    #[test]
    #[should_panic(expected = "registry: task not found")]
    fn test_extend_missing_task_fails() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        client.extend_task_expiry(&admin, &999, &(e.ledger().timestamp() + 9000));
    }

    #[test]
    fn test_task_survives_ledger_advancement() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let task_id = create_test_task(&client, &admin, &task_type, 2, 100_000);

        let user1 = Address::generate(&e);
        client.complete_task(&admin, &task_id, &user1);

        // Advance the ledger well past the default instance storage TTL
        // (instance TTL is ~100 ledgers; persistent TTL is ~4096).
        e.ledger().set_sequence_number(5000);

        let task = client.get_task(&task_id);
        assert_eq!(task.id, task_id);
        assert_eq!(task.creator, admin);
        assert_eq!(task.task_type, task_type);
        assert_eq!(task.completions, 1);
        assert_eq!(task.status, TaskStatus::Active);
        assert!(client.is_task_completed(&task_id, &user1));

        let ids = client.get_tasks_by_creator(&admin);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids.get(0).unwrap(), task_id);
    }

    #[test]
    fn test_transfer_admin() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let new_admin = Address::generate(&e);
        client.transfer_admin(&admin, &new_admin);

        let loc_hash: BytesN<32> = BytesN::random(&e);
        let task_id = client.create_task(
            &new_admin,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 1000),
        );

        let task = client.get_task(&task_id);
        assert_eq!(task.creator, new_admin);
    }

    #[test]
    #[should_panic(expected = "registry: unauthorized")]
    fn test_transfer_admin_unauthorized() {
        let (e, _admin, client) = setup();
        e.mock_all_auths();

        let attacker = Address::generate(&e);
        let new_admin = Address::generate(&e);
        client.transfer_admin(&attacker, &new_admin);
    }

    #[test]
    #[should_panic(expected = "registry: new admin must be different")]
    fn test_transfer_admin_same_address() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        client.transfer_admin(&admin, &admin);
    }

    #[test]
    fn test_admin_cancel_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.admin_cancel_task(&admin, &task_id);

        let task = client.get_task(&task_id);
        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    #[test]
    #[should_panic(expected = "registry: unauthorized")]
    fn test_non_admin_cannot_admin_cancel() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        let other = Address::generate(&e);
        client.admin_cancel_task(&other, &task_id);
    }

    #[test]
    #[should_panic(expected = "registry: task is not active")]
    fn test_admin_cancel_completed_task() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let user = Address::generate(&e);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.complete_task(&admin, &task_id, &user);
        client.admin_cancel_task(&admin, &task_id);
    }

    #[test]
    #[should_panic(expected = "registry: sponsor revoked")]
    fn test_complete_task_desponsored_creator_fails() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let sponsor = Address::generate(&e);
        client.add_sponsor(&admin, &sponsor);

        let loc_hash: BytesN<32> = BytesN::random(&e);
        let task_id = client.create_task(
            &sponsor,
            &String::from_str(&e, "tree-planting"),
            &loc_hash,
            &1000,
            &1,
            &(e.ledger().timestamp() + 1000),
        );

        client.remove_sponsor(&admin, &sponsor);

        let user = Address::generate(&e);
        client.complete_task(&admin, &task_id, &user);
    }

    #[test]
    fn test_complete_task_admin_creator_unaffected() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let user = Address::generate(&e);
        let task_id = create_test_task(
            &client,
            &admin,
            &String::from_str(&e, "tree-planting"),
            1,
            1000,
        );

        client.complete_task(&admin, &task_id, &user);
        let task = client.get_task(&task_id);
        assert_eq!(task.status, TaskStatus::Completed);
    }

    // =========================================================================
    // Issue #52 regression tests — indexed CreatorTask storage
    // =========================================================================

    /// Regression test: creating 500 tasks for one creator must not panic or
    /// fail. Before fix #52 the unbounded `CreatorTasks` Vec would grow with
    /// every task, making each creation O(n) in storage reads/writes. This
    /// test verifies that the 500th creation succeeds with the new indexed
    /// storage layout.
    #[test]
    fn test_500_task_regression_creation_succeeds() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let mut last_id: u64 = 0;

        for i in 0u32..500 {
            last_id = create_test_task(&client, &admin, &task_type, 1, 1_000_000 + i as u64);
        }

        // The 500th creation (index 499) must have succeeded.
        assert_eq!(last_id, 499);

        // The task is readable through the normal path.
        let task = client.get_task(&last_id);
        assert_eq!(task.id, last_id);
        assert_eq!(task.creator, admin);
        assert_eq!(task.status, TaskStatus::Active);
    }

    /// Regression test: the indexed creator task count reaches 500 after
    /// creating 500 tasks, and all IDs are retrievable through pagination.
    #[test]
    fn test_500_task_creator_count_and_full_retrieval() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "coastline-cleanup");
        let mut expected_ids: Vec<u64> = Vec::new(&e);

        for i in 0u32..500 {
            let id = create_test_task(&client, &admin, &task_type, 1, 1_000_000 + i as u64);
            expected_ids.push_back(id);
        }

        // Pagination: first page
        let page0 = client.get_tasks_by_creator_paged(&admin, &0, &10);
        assert_eq!(page0.len(), 10);
        for i in 0..10u32 {
            assert_eq!(page0.get(i).unwrap(), expected_ids.get(i).unwrap());
        }

        // Pagination: middle page at offset 100, limit 50
        let page_mid = client.get_tasks_by_creator_paged(&admin, &100, &50);
        assert_eq!(page_mid.len(), 50);
        for i in 0..50u32 {
            assert_eq!(page_mid.get(i).unwrap(), expected_ids.get(100 + i).unwrap());
        }

        // Pagination: near-end page at offset 450, limit 50
        let page_near_end = client.get_tasks_by_creator_paged(&admin, &450, &50);
        assert_eq!(page_near_end.len(), 50);
        for i in 0..50u32 {
            assert_eq!(
                page_near_end.get(i).unwrap(),
                expected_ids.get(450 + i).unwrap()
            );
        }

        // Pagination: partial final page at offset 490, limit 20 — only 10 remain
        let page_partial = client.get_tasks_by_creator_paged(&admin, &490, &20);
        assert_eq!(page_partial.len(), 10);
        for i in 0..10u32 {
            assert_eq!(
                page_partial.get(i).unwrap(),
                expected_ids.get(490 + i).unwrap()
            );
        }

        // Pagination: offset == count → empty
        let page_at_end = client.get_tasks_by_creator_paged(&admin, &500, &20);
        assert_eq!(page_at_end.len(), 0);

        // Pagination: offset > count → empty
        let page_beyond = client.get_tasks_by_creator_paged(&admin, &600, &20);
        assert_eq!(page_beyond.len(), 0);
    }

    /// Verifies that creator task count increments correctly for each new task.
    #[test]
    fn test_creator_task_count_increments() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");

        // No tasks yet — empty page
        let empty = client.get_tasks_by_creator_paged(&admin, &0, &10);
        assert_eq!(empty.len(), 0);

        // One task
        let id0 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let one = client.get_tasks_by_creator_paged(&admin, &0, &10);
        assert_eq!(one.len(), 1);
        assert_eq!(one.get(0).unwrap(), id0);

        // Two tasks
        let id1 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let two = client.get_tasks_by_creator_paged(&admin, &0, &10);
        assert_eq!(two.len(), 2);
        assert_eq!(two.get(0).unwrap(), id0);
        assert_eq!(two.get(1).unwrap(), id1);

        // Three tasks
        let id2 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let three = client.get_tasks_by_creator_paged(&admin, &0, &10);
        assert_eq!(three.len(), 3);
        assert_eq!(three.get(0).unwrap(), id0);
        assert_eq!(three.get(1).unwrap(), id1);
        assert_eq!(three.get(2).unwrap(), id2);
    }

    /// Verifies that multiple creators each maintain independent indexed lists.
    #[test]
    fn test_multiple_creators_independent_indexes() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let sponsor1 = Address::generate(&e);
        let sponsor2 = Address::generate(&e);
        client.add_sponsor(&admin, &sponsor1);
        client.add_sponsor(&admin, &sponsor2);

        let task_type = String::from_str(&e, "tree-planting");

        // sponsor1 creates 3 tasks
        let s1_id0 = create_test_task(&client, &sponsor1, &task_type, 1, 1000);
        let s1_id1 = create_test_task(&client, &sponsor1, &task_type, 1, 1000);
        let s1_id2 = create_test_task(&client, &sponsor1, &task_type, 1, 1000);

        // sponsor2 creates 2 tasks
        let s2_id0 = create_test_task(&client, &sponsor2, &task_type, 1, 1000);
        let s2_id1 = create_test_task(&client, &sponsor2, &task_type, 1, 1000);

        // sponsor1's list must contain exactly the 3 tasks, in order
        let s1_ids = client.get_tasks_by_creator_paged(&sponsor1, &0, &10);
        assert_eq!(s1_ids.len(), 3);
        assert_eq!(s1_ids.get(0).unwrap(), s1_id0);
        assert_eq!(s1_ids.get(1).unwrap(), s1_id1);
        assert_eq!(s1_ids.get(2).unwrap(), s1_id2);

        // sponsor2's list must contain exactly the 2 tasks, in order
        let s2_ids = client.get_tasks_by_creator_paged(&sponsor2, &0, &10);
        assert_eq!(s2_ids.len(), 2);
        assert_eq!(s2_ids.get(0).unwrap(), s2_id0);
        assert_eq!(s2_ids.get(1).unwrap(), s2_id1);

        // admin's list is empty (admin created no tasks in this test)
        let admin_ids = client.get_tasks_by_creator_paged(&admin, &0, &10);
        assert_eq!(admin_ids.len(), 0);
    }

    /// Edge case: limit == 0 returns an empty vector without panicking.
    #[test]
    fn test_paged_limit_zero_returns_empty() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        create_test_task(&client, &admin, &task_type, 1, 1000);

        let result = client.get_tasks_by_creator_paged(&admin, &0, &0);
        assert_eq!(result.len(), 0);
    }

    /// Edge case: limit == 1 returns exactly one entry.
    #[test]
    fn test_paged_limit_one() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let id0 = create_test_task(&client, &admin, &task_type, 1, 1000);
        create_test_task(&client, &admin, &task_type, 1, 1000);

        let result = client.get_tasks_by_creator_paged(&admin, &0, &1);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get(0).unwrap(), id0);
    }

    /// Edge case: limit larger than remaining tasks returns only the
    /// remaining entries (partial final page).
    #[test]
    fn test_paged_limit_larger_than_remaining() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let id0 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let id1 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let id2 = create_test_task(&client, &admin, &task_type, 1, 1000);

        // 5 tasks requested but only 2 remain after offset 1
        let result = client.get_tasks_by_creator_paged(&admin, &1, &5);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(0).unwrap(), id1);
        assert_eq!(result.get(1).unwrap(), id2);

        // Also check id0 isn't lost
        let first = client.get_tasks_by_creator_paged(&admin, &0, &1);
        assert_eq!(first.get(0).unwrap(), id0);
    }

    /// Verifies that the deprecated unpaged `get_tasks_by_creator` still
    /// works correctly for creators with fewer than the 200-entry cap.
    #[test]
    fn test_get_tasks_by_creator_below_cap() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let id0 = create_test_task(&client, &admin, &task_type, 1, 1000);
        let id1 = create_test_task(&client, &admin, &task_type, 1, 1000);

        let ids = client.get_tasks_by_creator(&admin);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids.get(0).unwrap(), id0);
        assert_eq!(ids.get(1).unwrap(), id1);
    }

    /// Verifies that the deprecated unpaged `get_tasks_by_creator` caps at
    /// 50 entries for a creator with exactly 51 tasks.
    #[test]
    fn test_get_tasks_by_creator_hard_cap_at_50() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        for i in 0u32..51 {
            create_test_task(&client, &admin, &task_type, 1, 1_000_000 + i as u64);
        }

        // Unpaged call must be capped at 50.
        let ids = client.get_tasks_by_creator(&admin);
        assert_eq!(ids.len(), 50);

        // The 51st task IS accessible through pagination.
        let paged = client.get_tasks_by_creator_paged(&admin, &50, &10);
        assert_eq!(paged.len(), 1);
    }

    /// Verifies no duplicate task IDs appear anywhere in a creator's indexed
    /// list when many tasks are created.
    #[test]
    fn test_no_duplicate_ids_in_creator_list() {
        let (e, admin, client) = setup();
        e.mock_all_auths();

        let task_type = String::from_str(&e, "tree-planting");
        let n: u32 = 50;

        for i in 0u32..n {
            create_test_task(&client, &admin, &task_type, 1, 1_000_000 + i as u64);
        }

        // Retrieve all entries and check for duplicates.
        let all = client.get_tasks_by_creator_paged(&admin, &0, &n);
        assert_eq!(all.len(), n);

        // Verify each expected ID appears exactly once.
        for i in 0u32..n {
            let id = all.get(i).unwrap();
            assert_eq!(id, i as u64); // tasks were created 0..n in order
        }
    }
}
