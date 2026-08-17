# Contributing to EcoTask Contracts

First off, thank you for considering contributing to EcoTask! It's people like you that make EcoTask a great tool for the environmental community.

## Table of Contents

- [Prerequisites](#prerequisites)
- [How Can I Contribute?](#how-can-i-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Pull Requests](#pull-requests)
- [Development Workflow](#development-workflow)
- [Changelog](#changelog)
- [Style Guide](#style-guide)
- [Security](#security)

## Prerequisites

To build and test the smart contracts, you will need:

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version, at least 1.75+)
- [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup#install-the-soroban-cli)
- `wasm32v1-none` target: `rustup target add wasm32v1-none`

> **Note:** This project uses `wasm32v1-none` (not the legacy `wasm32-unknown-unknown`).
> The `wasm32v1-none` target is required by Soroban SDK v20+ and produces smaller WASM
> binaries with lower deployment costs. If you are using an older Soroban CLI (< v20),
> you will need `wasm32-unknown-unknown` instead. CI and local builds must use the same
> target for artifact compatibility.

## How Can I Contribute?

### Reporting Bugs

If you find a bug, please create an issue using the Bug Report template. Include as much detail as possible, such as:

- Steps to reproduce the bug.
- Expected behavior vs actual behavior.
- Version of Rust and Soroban CLI you are using.

### Suggesting Enhancements

Enhancement suggestions are welcome! Please open an issue using the Feature Request template and describe:

- The problem this enhancement solves.
- A clear and concise description of the proposed change.

### Pull Requests

1. Fork the repository and create your branch from `main`.
2. If you've added code that should be tested, add tests.
3. Ensure the test suite passes (`cargo test`).
4. Format your code (`cargo fmt`).
5. Lint your code (`cargo clippy`).
6. Update the documentation if necessary.
7. Submit a pull request.

## Development Workflow

### Building

Build all contracts to WASM:

```bash
cargo build --target wasm32v1-none --release
```

### Testing

Run the full test suite:

```bash
cargo test
```

### Linting & Formatting

We maintain strict linting rules. Ensure your code is clean before submitting:

```bash
# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings
```

## Changelog

Every pull request that changes a public contract function, a storage key or
stored value shape, or an error string must update the `[Unreleased]` section
of [CHANGELOG.md](./CHANGELOG.md). Contract source changes under
`contracts/*/src/*.rs` are checked in CI and cannot merge without a changelog
update.

Use the Keep a Changelog categories `Added`, `Changed`, `Deprecated`,
`Removed`, `Fixed`, and `Security`. Name the affected contract and describe
what integrators must do. Do not edit an existing release entry; release
entries are historical records.

Classify contract changes by their integration impact:

- **API:** Public function additions are backward-compatible. Renaming,
  removing, or changing the parameters or return type of a public function is
  breaking and requires callers to update.
- **Storage:** Adding, removing, renaming, or changing the encoded shape of a
  storage key is breaking for existing state. State whether the change
  requires a migration or a fresh deployment.
- **Behaviour:** A semantic change that keeps the same function signature and
  storage layout must explain the old and new behaviour, including any changed
  error string.

Examples:

```markdown
### Added

- API: `task-registry` adds `get_task_status(task_id) -> TaskStatus`.

### Changed

- Storage: `reward-engine` replaces `DataKey::VerificationList` with a paged
  index. Existing state requires migration or a fresh deployment.
- Behaviour: `eco-token::allowance` now removes expired allowance entries
  while continuing to return `0`.
```

Workspace and contract package versions follow Semantic Versioning for source
releases. Before `1.0.0`, increment the minor version for breaking API or
storage changes and the patch version for backward-compatible fixes. Starting
with `1.0.0`, increment the major version for breaking changes. A source
version does not upgrade a deployed Soroban contract: incompatible API or
storage changes always require a new deployment and contract address unless a
specific migration path is provided.

## Style Guide

- Follow standard [Rust naming conventions](https://rust-lang.github.io/api-guidelines/naming.html).
- Keep functions small and focused.
- Document public functions and complex logic using Doc comments (`///`).
- Soroban-specific: Be mindful of ledger footprint and CPU cycles.

## Security

If you discover a security vulnerability, please do NOT open a public issue. Instead, email us at security@ecotask.network.

---

Thank you for your contributions! 🌍
