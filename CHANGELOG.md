# Changelog

All notable source-level changes to the EcoTask contracts are documented in
this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and source releases follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Soroban deployments are immutable: a breaking public API or storage layout
change requires a new contract deployment and address, even when the source
package version is bumped.

## [Unreleased]

### Security

#### `eco-token`, `task-registry`, and `reward-engine`

- Admin rotation now uses a two-step handover. The current admin proposes a
  successor with `propose_admin`, and the proposed address must authenticate
  and call `accept_admin` before receiving control. The existing
  `transfer_admin` entry point remains as a compatibility alias for the
  proposal step. Successful proposals and acceptances emit
  `AdminProposedEvent` and `AdminAcceptedEvent`, respectively.

### Fixed

#### `eco-token`

- **[#67] Prevent self-transfer allowance drain in `transfer_from` and fix storage re-fetch TOCTOU in `spend_allowance`.**
  `transfer_from` now panics with `"token: cannot transfer to self"` when `from == to`, preventing spenders from burning an owner's allowance without transferring tokens. `spend_allowance` in `storage.rs` now accepts the `&Allowance` struct directly instead of re-fetching from persistent storage with `.unwrap()`, eliminating a potential TOCTOU window.

#### `reward-engine`

- **[#71] Make `approve_proof` and the approve path of `resolve_dispute`
  atomic across contracts.** The verification record was previously written
  as `Approved` *before* the cross-contract `complete_task` and `mint` calls.
  A panic in either sub-invocation (e.g. the token supply cap) could leave
  the verification stuck in `Approved` with no payout, a consumed task
  completion slot, and a `total_paid`/supply discrepancy. Both functions now
  share an `approve_and_pay` tail that validates everything (including a
  defensive `completions < max_completions` pre-check) before any storage
  write, calls out to the registry and token only after validation, and
  writes the local `Approved` state only after both sub-calls succeed.

Error strings:

- `engine: task max completions reached`

- **[#71] Fix `reward-engine` WASM build and share contract types.** The
  `eco-token` and `task-registry` crates are now dev-only dependencies of
  the reward-engine. Their `#[contractimpl]` blocks emit `#[no_mangle]`
  exports, so linking either rlib into the reward-engine WASM binary
  produced duplicate symbols (`accept_admin`, `propose_admin`,
  `transfer_admin`) and `cargo build --target wasm32v1-none --release`
  failed. Production code already calls both contracts via raw
  `invoke_contract`; the shared `Task`/`TaskStatus` wire types now live in
  a new `ecotask-types` crate (no `#[contractimpl]`, so no exports) used
  by both the task-registry and reward-engine. Public contract ABI and
  storage layout are unchanged.

- **[#61] Replace the unbounded `VerificationList` Vec with a doubly-linked
  pending list and the `UserVerifications` Vec with a per-user sequence
  index.** The global pending set previously lived as a single growing
  `Vec<VerificationKey>` in instance storage: every `submit_proof` or
  resolution deserialised and rewrote the entire list,
  `remove_verification_key` was an O(n) scan with no cap, and
  `get_pending_verifications_paged` skipped resolved entries in the
  historical log, so oracle polling degraded with total history. The new
  storage layout is:

  | Scope | Key | Value | Purpose |
  |-------|-----|-------|---------|
  | Instance | `DataKey::PendingListCount` | `u64` | number of pending verifications |
  | Instance | `DataKey::PendingListHead` | `Option<VerificationKey>` | head of the pending list |
  | Instance | `DataKey::PendingListTail` | `Option<VerificationKey>` | tail of the pending list |
  | Persistent | `DataKey::PendingVerificationPrev(VerificationKey)` | `Option<VerificationKey>` | previous node link |
  | Persistent | `DataKey::PendingVerificationNext(VerificationKey)` | `Option<VerificationKey>` | next node link |
  | Persistent | `DataKey::UserVerificationCount(Address)` | `u64` | number of verifications for that user |
  | Persistent | `DataKey::UserVerificationIndex(Address, u64)` | `u64` | task id at the given 0-based index |

  `push_verification_key` / `remove_verification_key` are now O(1) pointer
  operations (a fixed number of storage ops regardless of history size),
  and `get_pending_verifications_paged` walks only pending nodes — resolved
  records are never scanned.

  **Pagination contract change:** `cursor` is now an offset into the
  *pending-only* set instead of an offset into the full verification log.
  **Storage layout change:** existing deployed state requires a fresh
  deployment (testnet only; acceptable per the issue).

- **[#74] Make `get_pending_verifications_paged` cursors stable across
  resolutions.** The cursor was a zero-based offset into the pending-only
  set, which shrinks whenever a verification is resolved:
  `approve_proof`, `reject_proof`, or `dispute_proof` removes the entry,
  shifting every later entry left by one, so an oracle or backend resuming
  from a saved offset silently skipped or re-processed entries whenever a
  resolution happened mid-pagination. Every verification now carries an
  immutable, monotonically-increasing `seq` assigned at submit time, and
  the cursor is "the last seq already returned": a page returns pending
  verifications with `seq > cursor`, which no resolution can shift or
  reorder. The returned `Verification` struct exposes `seq` so callers can
  resume with the last record's value.

  **Pagination contract change (breaking):** `cursor` changed from a
  `u32` offset to a `u64` sequence number, and `Verification` gained a
  `seq: u64` field. Persisted offset cursors are invalid and must be
  re-anchored — restart from 0 or read `seq` off the last record already
  processed.

  New storage:

  | Scope | Key | Value | Purpose |
  |-------|-----|-------|---------|
  | Instance | `DataKey::VerificationSeq` | `u64` | next verification sequence number to assign |

- **[#76] Keep `reward-engine` instance storage alive by refreshing its TTL
  on every entry point.** All operational state (admin, token/registry
  addresses, oracle roster, reward bounds, pause flag, cooldown, total
  paid, pending-list links) lives in instance storage, which Soroban
  expires after a short default TTL (~100 ledgers on mainnet, roughly
  8 minutes at ~5 s/ledger). A quiet period longer than that evicted
  everything: `read_admin`, `read_token`, and `read_registry` all `unwrap()`
  and the engine became permanently non-operational. Every public
  `#[contractimpl]` function now calls `extend_instance_ttl` as its first
  operation, which extends the contract instance to
  `INSTANCE_TTL_EXTEND_TO` (535,680 ledgers ≈ 31 days at ~5 s/ledger)
  whenever it is within `INSTANCE_TTL_THRESHOLD` (100 ledgers) of
  expiring. Because `extend_ttl` only rewrites the entry when it is close
  to expiry, the steady-state cost is one conditional storage check per
  call. Adds `test_instance_ttl_survives_quiet_period`, which advances the
  ledger by `INSTANCE_TTL_EXTEND_TO - 1` and confirms `approve_proof`
  still succeeds.

#### `scripts`

- `deploy.sh` now builds the requested contract first and fails fast if no
  (non-empty) WASM artifact is produced, so a stale or missing build can
  never be deployed.

#### `task-registry`

- **[#52] Unbounded `CreatorTasks` Vec replaced with indexed persistent
  storage.** Each `create_task` call previously read and rewrote an
  ever-growing `Vec<u64>` under `DataKey::CreatorTasks(creator)`. Prolific
  sponsors could drive storage and compute costs up without bound, and the
  serialized Vec would eventually exhaust transaction limits. The storage
  layout is now:

  | Key | Value | Purpose |
  |-----|-------|---------|
  | `DataKey::CreatorTaskCount(Address)` | `u64` | number of tasks for that creator |
  | `DataKey::CreatorTask(Address, u64)` | `u64` | task id at the given 0-based index |

  `push_creator_task` now does one read and two writes regardless of the
  creator's history size (O(1) per task). `get_tasks_by_creator_paged` reads
  only the indexed entries required for the requested page. The unpaged
  `get_tasks_by_creator` is retained for API compatibility but is deprecated
  and hard-capped at 50 entries to stay within the Soroban 100-entry footprint
  budget.

### Added

#### `eco-token`

- **[#41] Added fuzz / property-based arithmetic tests and max supply boundary tests using proptest.**
- `set_minter` now emits a `MinterUpdatedEvent` (`#[contractevent]`) on
  every successful minter rotation, containing the `admin` (topic),
  `previous_minter`, and `new_minter` fields.


### Changed

#### `eco-token`

- `set_minter` now panics with `"token: minter must differ from admin"` when
  the caller attempts to set `minter == admin`, enforcing role separation.

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

#### `reward-engine` (proof CID validation)

- `submit_proof` validates `proof_cid` length, rejecting empty or oversized
  (`> MAX_CID_LEN` bytes) CID strings before hashing/storage.

Error strings:

- `engine: proof cid must not be empty`
- `engine: proof cid too long`

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
