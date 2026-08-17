# Changelog

All notable source-level changes to the EcoTask contracts are documented in
this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and source releases follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Soroban deployments are immutable: a breaking public API or storage layout
change requires a new contract deployment and address, even when the source
package version is bumped.

## [Unreleased]

### Added

#### `reward-engine`

- `set_user_cooldown(caller: Address, min_ledgers_between_rewards: u64)` —
  admin-only; sets the minimum number of ledgers a user must wait between
  reward approvals. `0` disables the cooldown (the default).

Storage layout additions:

| Scope | Key | Value |
|-------|-----|-------|
| Instance | `DataKey::UserCooldown` | `u64` |
| Persistent | `DataKey::LastRewardLedger(Address)` | `u64` |

Error strings:

- `engine: user cooldown active`

## [0.1.0-alpha] - 2026-08-17

This entry records the current source interface as the baseline for future
contract releases. It does not designate any deployed contract address as a
release.

### Added

#### `eco-token`

Public API:

- `initialize(admin: Address, name: String, symbol: String, decimal: u32)`
- `mint(to: Address, amount: i128)`
- `transfer(from: Address, to: Address, amount: i128)`
- `balance(id: Address) -> i128`
- `total_supply() -> i128`
- `max_supply() -> i128`
- `set_max_supply(caller: Address, max_supply: i128)`
- `name() -> String`
- `symbol() -> String`
- `decimal() -> u32`
- `decimals() -> u32`
- `set_metadata(caller: Address, name: String, symbol: String, decimal: u32)`
- `admin() -> Address`
- `transfer_admin(current_admin: Address, new_admin: Address)`
- `minter() -> Address`
- `set_minter(caller: Address, new_minter: Address)`
- `burn(from: Address, amount: i128)`
- `approve(owner: Address, spender: Address, amount: i128, expiration_ledger: u32)`
- `allowance(owner: Address, spender: Address) -> i128`
- `allowance_with_expiry(owner: Address, spender: Address) -> Option<(i128, u32)>`
- `transfer_from(spender: Address, from: Address, to: Address, amount: i128)`

Storage layout:

| Scope | Key | Value |
|-------|-----|-------|
| Instance | `"admin"` | `Address` |
| Instance | `"minter"` | `Address` |
| Instance | `"name"` | `String` |
| Instance | `"symbol"` | `String` |
| Instance | `"decimal"` | `u32` |
| Instance | `"supply"` | `i128` |
| Instance | `"maxsupply"` | `i128` |
| Persistent | `("balance", Address)` | `i128` |
| Persistent | `("allow", owner: Address, spender: Address)` | `Allowance { amount: i128, expiration_ledger: u32 }` |

Error strings:

- `token: allowance expired`
- `token: allowance not found`
- `token: already initialized`
- `token: amount must be non-negative`
- `token: amount must be positive`
- `token: expiration must be in the future`
- `token: insufficient allowance`
- `token: insufficient balance`
- `token: max supply below current supply`
- `token: max supply must be positive`
- `token: new admin must be different`
- `token: supply cap exceeded`
- `token: unauthorized`
- `allowance underflow`
- `balance overflow`
- `balance underflow`
- `supply overflow`
- `supply underflow`

#### `task-registry`

Public API:

- `initialize(admin: Address)`
- `add_sponsor(caller: Address, sponsor: Address)`
- `remove_sponsor(caller: Address, sponsor: Address)`
- `create_task(creator: Address, task_type: String, location_hash: BytesN<32>, reward_amount: i128, max_completions: u32, expires_at: u64) -> u64`
- `get_task(task_id: u64) -> Task`
- `get_task_live_status(task_id: u64) -> Task`
- `complete_task(caller: Address, task_id: u64, user: Address)`
- `expire_task(caller: Address, task_id: u64)`
- `expire_task_permissionless(task_id: u64)`
- `extend_task_expiry(caller: Address, task_id: u64, new_expires_at: u64)`
- `cancel_task(caller: Address, task_id: u64)`
- `admin_cancel_task(caller: Address, task_id: u64)`
- `task_count() -> u64`
- `is_task_completed(task_id: u64, user: Address) -> bool`
- `get_tasks_by_creator(creator: Address) -> Vec<u64>`
- `get_tasks_by_creator_paged(creator: Address, cursor: u32, limit: u32) -> Vec<u64>`
- `list_tasks(cursor: u64, limit: u32) -> Vec<Task>`
- `transfer_admin(current_admin: Address, new_admin: Address)`

Storage layout:

| Scope | Key | Value |
|-------|-----|-------|
| Instance | `DataKey::TaskCount` | `u64` |
| Instance | `DataKey::Admin` | `Address` |
| Persistent | `DataKey::Task(task_id: u64)` | `Task { id, creator, task_type, location_hash, reward_amount, max_completions, completions, status, created_at, expires_at }` |
| Persistent | `DataKey::Sponsor(Address)` | `bool` |
| Persistent | `DataKey::Completion(task_id: u64, user: Address)` | `bool` |
| Persistent | `DataKey::CreatorTasks(Address)` | `Vec<u64>` |

`TaskStatus` variants are `Active`, `Completed`, `Expired`, and `Cancelled`.

Error strings:

- `registry: already completed`
- `registry: already initialized`
- `registry: expiry must be in the future`
- `registry: max completions must be positive`
- `registry: max completions reached`
- `registry: new admin must be different`
- `registry: new expiry must extend the current one`
- `registry: reward must be positive`
- `registry: sponsor revoked`
- `registry: task expired`
- `registry: task is not active`
- `registry: task not found`
- `registry: task not yet expired`
- `registry: task type must not be empty`
- `registry: unauthorized`

#### `reward-engine`

Public API:

- `initialize(admin: Address, token: Address, registry: Address, oracle: Address)`
- `set_oracle(caller: Address, new_oracle: Address)`
- `add_oracle(caller: Address, new_oracle: Address)`
- `remove_oracle(caller: Address, oracle: Address)`
- `get_oracles() -> Vec<Address>`
- `is_oracle(addr: Address) -> bool`
- `set_token(caller: Address, new_token: Address)`
- `set_registry(caller: Address, new_registry: Address)`
- `set_reward_range(caller: Address, min_reward: i128, max_reward: i128)`
- `pause(caller: Address)`
- `unpause(caller: Address)`
- `is_paused() -> bool`
- `submit_proof(oracle: Address, user: Address, task_id: u64, proof_cid: String)`
- `approve_proof(oracle: Address, user: Address, task_id: u64, reward_amount: i128)`
- `reject_proof(oracle: Address, user: Address, task_id: u64)`
- `dispute_proof(caller: Address, user: Address, task_id: u64)`
- `resolve_dispute(caller: Address, user: Address, task_id: u64, approve: bool, reward_amount: i128)`
- `get_verification(task_id: u64, user: Address) -> Verification`
- `get_verification_by_cid_hash(cid_hash: BytesN<32>) -> Verification`
- `get_pending_verifications_paged(cursor: u32, limit: u32) -> Vec<Verification>`
- `get_pending_verifications() -> Vec<Verification>`
- `get_verifications_by_user(user: Address, cursor: u32, limit: u32) -> Vec<Verification>`
- `total_paid() -> i128`
- `transfer_admin(current_admin: Address, new_admin: Address)`

Storage layout:

| Scope | Key | Value |
|-------|-----|-------|
| Instance | `DataKey::Admin` | `Address` |
| Instance | `DataKey::Token` | `Address` |
| Instance | `DataKey::Registry` | `Address` |
| Instance | `DataKey::Oracles` | `Vec<Address>` |
| Instance | `DataKey::MinReward` | `i128` |
| Instance | `DataKey::MaxReward` | `i128` |
| Instance | `DataKey::VerificationList` | `Vec<VerificationKey>` |
| Instance | `DataKey::TotalPaid` | `i128` |
| Instance | `DataKey::Paused` | `bool` |
| Persistent | `DataKey::Verification(task_id: u64, user: Address)` | `Verification { task_id, user, proof_cid, reward_amount, status, submitted_at, resolved_at, oracle }` |
| Persistent | `DataKey::CidHash(BytesN<32>)` | `VerificationKey { task_id, user }` |
| Persistent | `DataKey::UserVerifications(Address)` | `Vec<u64>` |

`VerificationStatus` variants are `Pending`, `Approved`, `Rejected`, and
`Disputed`. CID index entries are extended to 4,096 ledgers when written.

Error strings:

- `engine: already initialized`
- `engine: cannot remove the last oracle`
- `engine: contract is paused`
- `engine: max reward must be >= min reward`
- `engine: min reward must be positive`
- `engine: new admin must be different`
- `engine: not found`
- `engine: oracle already registered`
- `engine: oracle must be different from admin`
- `engine: oracle not registered`
- `engine: proof already submitted`
- `engine: proof cid already submitted`
- `engine: reward amount must be positive`
- `engine: reward below minimum`
- `engine: reward exceeds maximum`
- `engine: reward exceeds task budget`
- `engine: task has expired`
- `engine: task is not active`
- `engine: unauthorized`
- `engine: verification is not disputable`
- `engine: verification is not disputed`
- `engine: verification is not pending`
- `engine: verification not found`
- `total_paid overflow`

[Unreleased]: https://github.com/ecotask-network/EcoTask-contract/compare/c579be9ebcb378005fbdab62939b755d4418935e...HEAD
[0.1.0-alpha]: https://github.com/ecotask-network/EcoTask-contract/tree/c579be9ebcb378005fbdab62939b755d4418935e
