<div align="center">

# ecotask-contracts

**The on-chain heart of EcoTask — smart contracts powering verifiable climate rewards.**

Stellar Soroban contracts written in Rust that handle token issuance, task registration, and trustless reward distribution.

[![Build](https://img.shields.io/badge/Build-Passing-brightgreen)]()
[![Rust](https://img.shields.io/badge/Rust-1.76-orange?logo=rust)](https://www.rust-lang.org)
[![Soroban](https://img.shields.io/badge/Soroban-v26-7B68EE?logo=stellar)](https://soroban.stellar.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)
[![Status](https://img.shields.io/badge/Status-v0.1.0--alpha-blue)]()

</div>

---

## Table of Contents

- [Overview](#overview)
- [How It Works](#how-it-works)
- [Contracts](#contracts)
  - [eco-token](#1-eco-token)
  - [task-registry](#2-task-registry)
  - [reward-engine](#3-reward-engine)
- [Roles](#roles)
- [Architecture](#architecture)
- [Getting Started](#getting-started)
- [Contract API Reference](#contract-api-reference)
- [Testing](#testing)
- [Deployment](#deployment)
- [Security](#security)
- [Troubleshooting](#troubleshooting)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [Ecosystem](#ecosystem)
- [License](#license)

---

## Overview

EcoTask is a platform that rewards people for verified environmental actions — planting trees, cleaning coastlines, reducing waste, and more. These contracts are the trustless backbone: they register tasks, verify proof of completion, and release token rewards automatically on the Stellar blockchain.

**The problem:** Environmental rewards programmes today are opaque, slow, and controlled by intermediaries. Participants don't know if their impact is real, and funds disappear into administrative overhead.

**The solution:** Put the entire reward pipeline on-chain. Tasks are defined with transparent parameters. Proofs are verified by a trusted oracle. Rewards are minted automatically when verification passes. No middleman, no delays, no corruption.

**What this repo contains:**

- `eco-token` — the ECO reward token (SEP-0041 compatible)
- `task-registry` — on-chain task database with sponsor access control
- `reward-engine` — proof verification, dispute resolution, and automatic payout

---

## How It Works

```
1. Admin creates a task (e.g. "plant 10 trees", reward: 500 ECO)
2. A user performs the action off-chain and submits proof (IPFS CID)
3. The oracle verifies the proof against the stored hash
4. On approval, the reward engine:
   a. Validates the task is active and not expired
   b. Validates the reward doesn't exceed the task's declared budget
   c. Calls the registry to mark the task as completed
   d. Calls the token contract to mint ECO to the user's wallet
5. The user receives ECO tokens — fully on-chain, fully auditable
```

---

## Contracts

### 1. `eco-token`

The EcoTask native token contract. Implements the Stellar SEP-0041 token interface.

**Key properties:**
- Token name: `ECO`
- Standard: SEP-0041 compatible
- Minting: restricted to a designated **minter address** (the reward engine)
- Admin: controls minter assignment, can transfer admin role
- Arithmetic: all balance operations use `checked_add` / `checked_sub` to prevent overflow

**Functions:**

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin, name, symbol, decimal)` | — | Deploy once to set admin, metadata, and initial minter |
| `mint(to, amount)` | minter | Mint new ECO tokens to an address |
| `burn(from, amount)` | `from` | Burn ECO tokens from an address |
| `transfer(from, to, amount)` | `from` | Transfer ECO between addresses |
| `approve(owner, spender, amount, expiration_ledger)` | `owner` | Set a spending allowance for another address |
| `transfer_from(from, to, spender, amount)` | `spender` | Transfer using an approved allowance |
| `balance(id)` | — | Read the ECO balance of an address |
| `total_supply()` | — | Read total ECO supply |
| `max_supply()` | — | Hard supply cap (`i128::MAX` if unset) |
| `set_max_supply(caller, max_supply)` | `caller` (admin only) | Set the hard supply cap for future minting |
| `name()`, `symbol()`, `decimal()`, `decimals()` | — | Token metadata (SEP-0041 includes `decimals`) |
| `set_metadata(caller, name, symbol, decimal)` | `caller` (admin only) | Update token metadata (SEP-0041 `set_metadata`) |
| `admin()` | — | Current admin address |
| `minter()` | — | Current minter address |
| `set_minter(caller, new_minter)` | `caller` (admin only) | Assign a new minter |
| `transfer_admin(current_admin, new_admin)` | `current_admin` | Transfer admin role |
| `allowance(owner, spender)` | — | Read current allowance for a spender |

**Input validation:**
- `mint` requires `amount > 0` and, once a cap is set, `supply + amount <= max_supply`
- `burn` requires `amount > 0` and sufficient balance
- `approve` requires `amount >= 0` and `expiration_ledger > current ledger sequence`
- `set_max_supply` requires a positive cap that is not below the current supply
- `set_metadata` is admin-only (SEP-0041)

### 2. `task-registry`

The on-chain task database. Stores task definitions, tracks completions, and enforces sponsor access control.

**Task lifecycle:**

```
Created (Active) ──► Completed  (all slots filled)
       │
       ├──► Expired   (admin or timestamp)
       └──► Cancelled (creator or admin)
```

**Functions:**

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin)` | — | Deploy once to set admin |
| `add_sponsor(caller, sponsor)` | admin | Grant sponsor privileges to an address |
| `remove_sponsor(caller, sponsor)` | admin | Revoke sponsor privileges |
| `create_task(creator, task_type, location_hash, reward_amount, max_completions, expires_at) → u64` | creator (sponsor/admin) | Create a new task, returns task_id |
| `get_task(task_id) → Task` | — | Fetch full task details |
| `complete_task(caller, task_id, user)` | caller (sponsor/admin) | Mark a task as completed for a user |
| `expire_task(caller, task_id)` | admin | Force-expire an active task |
| `extend_task_expiry(caller, task_id, new_expires_at)` | creator or admin | Extend an active task's expiry to a later timestamp |
| `cancel_task(caller, task_id)` | creator | Cancel a task you created |
| `admin_cancel_task(caller, task_id)` | admin | Cancel any active task (governance) |
| `task_count() → u64` | — | Total tasks created (next ID) |
| `list_tasks(cursor, limit) → Vec<Task>` | — | Pageable global task listing, ordered by task id |
| `is_task_completed(task_id, user) → bool` | — | Check if a user completed a task |
| `get_tasks_by_creator(creator) → Vec<u64>` | — | List all task IDs created by an address |
| `get_tasks_by_creator_paged(creator, cursor, limit) → Vec<u64>` | — | Pageable slice of a creator's task IDs |
| `transfer_admin(current_admin, new_admin)` | `current_admin` | Transfer admin role |

**Task struct:**

```rust
pub struct Task {
    pub id: u64,
    pub creator: Address,
    pub task_type: String,          // e.g. "tree-planting", "coastline-cleanup"
    pub location_hash: BytesN<32>,  // SHA-256 of GPS coordinates
    pub reward_amount: i128,        // max ECO per completion
    pub max_completions: u32,       // total allowed completions
    pub completions: u32,           // current completion count
    pub status: TaskStatus,         // Active | Completed | Expired | Cancelled
    pub created_at: u64,            // ledger timestamp
    pub expires_at: u64,            // ledger timestamp
}
```

**Validation on create:**
- `task_type` must not be empty
- `reward_amount` must be positive
- `max_completions` must be positive
- `expires_at` must be in the future

**Access model:** Admins and approved sponsors can create and complete tasks. Only the task creator can cancel their own task. Admins can cancel any task via `admin_cancel_task`. Active tasks can be extended (pushed to a later expiry) by their creator or the admin via `extend_task_expiry` — useful for campaigns that outlive their original deadline without having to recreate and republish the task.

**Querying & pagination:** `list_tasks(cursor, limit)` walks the global task list by id, while `get_tasks_by_creator_paged(creator, cursor, limit)` slices a creator's task list. Both are bounded reads, so off-chain indexers and the backend can paginate without pulling the whole registry in one call.

### 3. `reward-engine`

The verification and payout engine. Receives proof from the off-chain oracle, validates it against the registry, and triggers token minting on success.

**Proof lifecycle:**

```
Submitted (Pending) ──► Approved (mints ECO)
         │
         ├──► Rejected (no payout)
         └──► Disputed ──► Resolved (approve → mint / reject → no payout)
```

**Functions:**

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin, token, registry, oracle)` | — | Deploy once, sets all addresses. Admin and oracle must differ. |
| `submit_proof(oracle, user, task_id, proof_cid)` | oracle | Submit an IPFS proof CID for verification |
| `approve_proof(oracle, user, task_id, reward_amount)` | oracle | Approve a proof and mint ECO reward |
| `reject_proof(oracle, user, task_id)` | oracle | Reject a proof (no payout) |
| `add_oracle(caller, new_oracle)` | admin | Register an additional oracle |
| `remove_oracle(caller, oracle)` | admin | Remove an oracle (last one cannot be removed) |
| `set_oracle(caller, new_oracle)` | admin | Replace the entire oracle roster with a single oracle |
| `get_oracles() → Vec<Address>` | — | List all registered oracles |
| `is_oracle(addr) → bool` | — | Check if an address is a registered oracle |
| `dispute_proof(caller, user, task_id)` | admin | Escalate a pending or rejected proof to dispute |
| `resolve_dispute(caller, user, task_id, approve, reward_amount)` | admin | Resolve a disputed proof |
| `get_verification(task_id, user) → Verification` | — | Fetch verification details |
| `get_pending_verifications() → Vec<Verification>` | — | **Deprecated** — use `get_pending_verifications_paged` instead |
| `get_pending_verifications_paged(cursor: u64, limit) → Vec<Verification>` | — | Pageable list of pending verifications (`cursor` = seq of the last returned item; `0` starts at the beginning) |
| `get_verifications_by_user(user, cursor, limit) → Vec<Verification>` | — | Pageable history of a user's verifications |
| `total_paid() → i128` | — | Cumulative ECO minted through this engine |
| `pause(caller)` | admin | Emergency pause — blocks all proof operations |
| `unpause(caller)` | admin | Resume operations after pause |
| `is_paused() → bool` | — | Check if engine is paused |
| `set_oracle(caller, new_oracle)` | admin | Replace the oracle address |
| `set_token(caller, new_token)` | admin | Replace the token address |
| `set_registry(caller, new_registry)` | admin | Replace the registry address |
| `set_reward_range(caller, min_reward, max_reward)` | admin | Set platform-wide reward bounds |
| `transfer_admin(current_admin, new_admin)` | `current_admin` | Transfer admin role |

**Guards on `approve_proof` and `resolve_dispute` (approve path):**
1. Engine must not be paused
2. Caller must be the registered oracle
3. Verification must be in `Pending` status
4. `reward_amount` must be positive
5. `reward_amount` must be within platform min/max range (if set)
6. Task must be `Active` in the registry (cross-contract call)
7. Task must not be expired (cross-contract check on `expires_at`)
8. `reward_amount` must not exceed the task's declared `reward_amount`

**Verification struct:**

```rust
pub struct Verification {
    pub task_id: u64,
    pub user: Address,
    pub proof_cid: String,              // IPFS CID of the proof
    pub reward_amount: i128,            // 0 until approved
    pub status: VerificationStatus,     // Pending | Approved | Rejected | Disputed
    pub submitted_at: u64,              // ledger timestamp
    pub resolved_at: Option<u64>,       // set on approve/reject/resolve
    pub oracle: Address,                // oracle that submitted the proof
    pub seq: u64,                       // immutable submit-time sequence number; stable pagination cursor
}
```

---

## Roles

The system has four distinct roles. Separation of duties prevents any single party from unilaterally minting tokens or manipulating tasks.

| Role | Assigned To | Powers |
|------|-------------|--------|
| **Admin** | Deployer (transferable) | Configure contracts, manage sponsors, cancel tasks, emergency pause, resolve disputes |
| **Minter** | Reward engine address | Mint new ECO tokens (set by admin via `set_minter`) |
| **Oracle** | Off-chain verification service(s) | Submit proofs, approve/reject proofs and trigger payouts |
| **Sponsor** | NGOs, companies, governments | Create tasks, mark tasks as completed |

**Critical separation:** The admin and oracle must be different addresses. This prevents a single compromised key from both approving proofs and reconfiguring the contract. The minter must be the reward engine — no direct admin minting. The engine supports a **roster of oracles** (`add_oracle` / `remove_oracle`) so verification duties can be shared or rotated without re-deploying; the final oracle can never be removed, so the engine always retains at least one operator.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                         Stellar Network                         │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐     ┌──────────────────┐     ┌──────────────┐  │
│  │  eco-token   │     │   task-registry  │     │ reward-engine│  │
│  │             │     │                  │     │              │  │
│  │ ECO token   │     │ Task definitions │     │ Proof submit │  │
│  │ Mint/Burn   │◄────│ Completion track │────►│ Verify/Pay   │  │
│  │ Allowances  │mint │ Sponsor ACL      │query│ Disputes     │  │
│  │             │     │ Admin governance │     │ Pause        │  │
│  └──────┬──────┘     └──────────────────┘     └──────┬───────┘  │
│         │                                            │          │
│         │                                            │          │
│  ┌──────▼──────┐                            ┌───────▼───────┐  │
│  │ User Wallet │                            │  Off-chain    │  │
│  │ (receives   │                            │  Oracle       │  │
│  │  ECO tokens)│                            │  (verifies    │  │
│  └─────────────┘                            │   proofs)     │  │
│                                             └───────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

**Cross-contract calls:**
- `reward-engine` calls `task-registry.get_task()` to validate task status before payout
- `reward-engine` calls `task-registry.complete_task()` to mark completion
- `reward-engine` calls `eco-token.mint()` to issue reward tokens
- All calls are authenticated: the reward engine must be registered as a sponsor in the registry, and as the minter in the token contract

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) >= 1.76
- [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup) v26+
- WASM target: `rustup target add wasm32v1-none`

```bash
# One-line setup
rustup target add wasm32v1-none && cargo install --locked soroban-cli
```

### Clone and build

```bash
git clone https://github.com/ecotask-network/EcoTask-contract.git
cd EcoTask-contract
make build
```

### Run tests

```bash
make test
```

### Lint

```bash
make lint   # clippy
make fmt    # format check
```

### Available Makefile targets

| Target | Command | Description |
|--------|---------|-------------|
| `build` | `cargo build --target wasm32v1-none --release` | Build all contracts to WASM |
| `test` | `cargo test` | Run all unit and integration tests |
| `lint` | `cargo clippy --all-targets --all-features -- -D warnings` | Lint with zero warnings |
| `fmt` | `cargo fmt --all -- --check` | Check formatting |
| `clean` | `cargo clean` | Remove build artifacts |
| `all` | build + test + lint + fmt | Full CI pipeline |
| `deploy-testnet` | Deploys all 3 contracts | Requires funded Stellar account |

---

## Contract API Reference

### eco-token

```bash
# Initialize (deployer becomes admin and minter)
soroban contract invoke --id <TOKEN_ID> -- initialize \
  --admin <ADMIN_KEY> --name "ECO" --symbol "ECO" --decimal 7

# Assign minter to reward engine
soroban contract invoke --id <TOKEN_ID> -- set_minter \
  --caller <ADMIN_KEY> --new_minter <ENGINE_ID>

# Mint (caller must be minter)
soroban contract invoke --id <TOKEN_ID> -- mint \
  --to <USER_KEY> --amount 500

# Check balance
soroban contract invoke --id <TOKEN_ID> -- balance --id <USER_KEY>

# Check total supply
soroban contract invoke --id <TOKEN_ID> -- total_supply
```

### task-registry

```bash
# Initialize
soroban contract invoke --id <REGISTRY_ID> -- initialize \
  --admin <ADMIN_KEY>

# Add a sponsor
soroban contract invoke --id <REGISTRY_ID> -- add_sponsor \
  --caller <ADMIN_KEY> --sponsor <SPONSOR_KEY>

# Create a task (sponsor or admin)
soroban contract invoke --id <REGISTRY_ID> -- create_task \
  --creator <SPONSOR_KEY> \
  --task_type "tree-planting" \
  --location_hash <32_BYTES_HEX> \
  --reward_amount 500 \
  --max_completions 50 \
  --expires_at 1735689600

# Get task details
soroban contract invoke --id <REGISTRY_ID> -- get_task --task_id 0

# Cancel a task (creator only)
soroban contract invoke --id <REGISTRY_ID> -- cancel_task \
  --caller <CREATOR_KEY> --task_id 0

# Admin cancel (any active task)
soroban contract invoke --id <REGISTRY_ID> -- admin_cancel_task \
  --caller <ADMIN_KEY> --task_id 0
```

### reward-engine

```bash
# Initialize
soroban contract invoke --id <ENGINE_ID> -- initialize \
  --admin <ADMIN_KEY> --token <TOKEN_ID> --registry <REGISTRY_ID> --oracle <ORACLE_KEY>

# Submit proof (oracle)
soroban contract invoke --id <ENGINE_ID> -- submit_proof \
  --oracle <ORACLE_KEY> --user <USER_KEY> --task_id 0 --proof_cid "QmXyz..."

# Approve and mint (oracle)
soroban contract invoke --id <ENGINE_ID> -- approve_proof \
  --oracle <ORACLE_KEY> --user <USER_KEY> --task_id 0 --reward_amount 500

# Reject proof (oracle)
soroban contract invoke --id <ENGINE_ID> -- reject_proof \
  --oracle <ORACLE_KEY> --user <USER_KEY> --task_id 0

# Dispute (admin)
soroban contract invoke --id <ENGINE_ID> -- dispute_proof \
  --caller <ADMIN_KEY> --user <USER_KEY> --task_id 0

# Resolve dispute (admin)
soroban contract invoke --id <ENGINE_ID> -- resolve_dispute \
  --caller <ADMIN_KEY> --user <USER_KEY> --task_id 0 \
  --approve true --reward_amount 500

# Emergency pause / unpause
soroban contract invoke --id <ENGINE_ID> -- pause --caller <ADMIN_KEY>
soroban contract invoke --id <ENGINE_ID> -- unpause --caller <ADMIN_KEY>

# Check total paid
soroban contract invoke --id <ENGINE_ID> -- total_paid
```

---

## Testing

The project has **141 tests** across unit and integration suites:

| Suite | Contract | Tests | Description |
|-------|----------|-------|-------------|
| Unit | eco-token | 40 | Mint, transfer, burn, approve, minter role, supply cap, metadata, input validation |
| Unit | task-registry | 37 | CRUD, sponsors, completions, expiry, pagination, task extension, admin cancel, empty type |
| Unit | reward-engine | 51 | Proofs, disputes, reward guards, multi-oracle, pagination, total paid, pause |
| Integration | Root | 9 | Full lifecycle, dispute flow, multi-user, minter delegation, reward caps, admin cancel, emergency pause, supply cap |

```bash
# Run everything
cargo test --workspace

# Run a specific contract
cargo test -p eco-token
cargo test -p task-registry
cargo test -p reward-engine

# Run a specific test
cargo test -p reward-engine test_pause_blocks_submit_proof
```

Integration tests in `tests/reward_integration_test.rs` deploy all three contracts into a single `Env` and exercise real cross-contract calls (not mocks).

---

## Deployment

### Prerequisites

- A Stellar testnet or mainnet account with sufficient XLM for contract storage
- Soroban CLI configured for the target network

### Step 1: Configure network

```bash
soroban network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

### Step 2: Build WASM

```bash
cargo build --target wasm32v1-none --release
```

### Step 3: Deploy contracts

```bash
# Deploy in order: token → registry → engine
./scripts/deploy.sh eco-token testnet
./scripts/deploy.sh task-registry testnet
./scripts/deploy.sh reward-engine testnet
```

### Step 4: Initialize and wire contracts

```bash
# Initialize token
soroban contract invoke --id <TOKEN_ID> -- --network testnet --source <KEY> \
  initialize --admin <ADMIN> --name "ECO" --symbol "ECO" --decimal 7

# Set minter to reward engine
soroban contract invoke --id <TOKEN_ID> -- --network testnet --source <KEY> \
  set_minter --caller <ADMIN> --new_minter <ENGINE_ID>

# Initialize registry
soroban contract invoke --id <REGISTRY_ID> -- --network testnet --source <KEY> \
  initialize --admin <ADMIN>

# Initialize engine
soroban contract invoke --id <ENGINE_ID> -- --network testnet --source <KEY> \
  initialize --admin <ADMIN> --token <TOKEN_ID> --registry <REGISTRY_ID> --oracle <ORACLE_KEY>

# Add reward engine as sponsor (so it can call complete_task)
soroban contract invoke --id <REGISTRY_ID> -- --network testnet --source <KEY> \
  add_sponsor --caller <ADMIN> --sponsor <ENGINE_ID>
```

### Step 5: Verify deployment

```bash
NETWORK=testnet ./scripts/verify-deploy.sh
```

### Fund test accounts

```bash
./scripts/fund-accounts.sh testnet <ADDRESS1> <ADDRESS2> ...
```

---

## Security

**Before mainnet, all contracts must undergo a formal security audit.**

### Design properties

- **Minting is minter-only.** Only the address stored as `minter` (the reward engine) can mint new ECO tokens. Admin assigns minter via `set_minter()`.
- **Hard supply cap.** The admin can set a `max_supply`; no mint — including reward-engine payouts — can push total supply past it, bounding inflation.
- **Oracle is separated from admin.** The engine enforces that oracle and admin are different addresses at initialization.
- **Double-claim prevention.** The registry records each `(task_id, user)` completion pair and rejects duplicates.
- **Overflow protection.** All balance arithmetic uses `checked_add` / `checked_sub`.
- **Proof immutability.** Once a proof CID is submitted, it cannot be changed — only its status evolves.
- **Reward cap enforcement.** The engine validates every payout against the task's declared budget.
- **Cross-contract validation.** The engine calls the registry to verify task status before every payout.
- **Emergency pause.** Admin can instantly halt all proof operations in case of an exploit.
- **Input validation.** Token `approve` rejects negative amounts and past expirations. Task creation rejects empty types.

### Reporting vulnerabilities

Please see [SECURITY.md](./SECURITY.md) for responsible disclosure instructions. Do **not** open public issues for security vulnerabilities.

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `token: already initialized` | Contract was already initialized. Each contract can only be initialized once. |
| `engine: unauthorized` | Caller is not the expected role (admin, oracle, or minter). Check which role the function requires. |
| `engine: contract is paused` | The engine is paused. Admin must call `unpause` to resume operations. |
| `engine: reward exceeds task budget` | The `reward_amount` you're trying to pay exceeds the task's declared `reward_amount`. Reduce the payout or update the task. |
| `engine: task has expired` | The task's `expires_at` has passed. Create a new task or extend it. |
| `engine: task is not active` | The task status is not `Active`. It may be completed, expired, or cancelled. |
| `registry: task type must not be empty` | `task_type` was an empty string. Provide a non-empty task type. |
| `registry: already completed` | This user has already completed this task. Double-claiming is not allowed. |
| `token: insufficient balance` | Transfer or burn amount exceeds the sender's balance. |
| WASM build fails | Ensure `rustup target add wasm32v1-none` was run and you're using Soroban SDK v26+. |
| `cargo test` fails to compile | Run `cargo update` to refresh `Cargo.lock`, then rebuild. |

---

## Roadmap

| Phase | Milestone | Status |
|-------|-----------|--------|
| **Phase 1** | [Core contracts (token, registry, reward engine)](./CHANGELOG.md#010-alpha---2026-08-17) | Done |
| **Phase 2** | [Token minter role, reward guards, emergency pause, admin governance](./CHANGELOG.md#010-alpha---2026-08-17) | Done |
| **Phase 3** | Formal security audit | Upcoming |
| **Phase 4** | Testnet deployment + backend integration | Upcoming |
| **Phase 5** | Bug bounty programme launch | Planned |
| **Phase 6** | Mainnet launch | Planned |
| **Phase 7** | DAO governance contract | Planned |
| **Phase 8** | Cross-chain reward bridging | Future |

### Near-term priorities

- Formal audit of all three contracts by an independent Soroban auditor
- Testnet deployment with live oracle and backend integration
- On-chain governance for task sponsor verification and platform parameter updates
- Tokenomics modelling for sustainable reward emissions

---

## Contributing

We welcome contributions from everyone — Rust and Soroban experience is helpful but not required. We're happy to mentor.

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development workflow, PR process, and style guide.
See [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) for community standards.

**Quick start:**

```bash
git clone https://github.com/ecotask-network/EcoTask-contract.git
cd EcoTask-contract
make all   # build + test + lint + fmt
```

---

## Ecosystem

This is part of the [EcoTask Network](https://github.com/ecotask-network):

| Repo | Description |
|------|-------------|
| [EcoTask-app](https://github.com/ecotask-network/EcoTask-app) | Mobile dApp |
| [EcoTask-backend](https://github.com/ecotask-network/EcoTask-backend) | Node.js API & verification engine |
| [EcoTask-contracts](https://github.com/ecotask-network/EcoTask-contract) | Stellar Soroban smart contracts |
| [EcoTask-docs](https://github.com/ecotask-network/EcoTask-docs) | Documentation hub |

---

## License

MIT — see [LICENSE](./LICENSE) for details.

---

<div align="center">

*Part of the [EcoTask Network](https://github.com/ecotask-network) — Because the environment deserves an economy.*

</div>
