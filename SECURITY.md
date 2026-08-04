# Security Policy

## Supported Versions

We actively maintain security fixes for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | ✅ Yes             |
| < 0.1   | ❌ No              |

---

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

If you discover a security issue in any `ecotask-contracts` contract, please
report it responsibly by emailing:

**security@ecotask.network**

Include as much detail as possible:

- A description of the vulnerability
- The affected contract(s) and function(s)
- Steps to reproduce or a proof-of-concept (PoC)
- Potential impact and severity assessment
- Any suggested mitigations

We aim to acknowledge reports within **48 hours** and provide an initial
assessment within **7 days**.

---

## Disclosure Policy

We follow a coordinated disclosure process:

1. You report the vulnerability privately.
2. We confirm and reproduce the issue.
3. We develop and test a fix internally.
4. We release the fix and credit you (unless you prefer to remain anonymous).
5. A public post-mortem is published after the fix is deployed.

We ask that you give us a reasonable window — typically **90 days** — before
any public disclosure.

---

## Scope

The following are **in scope** for security reports:

- `contracts/eco-token` — token minting, burning, and admin controls
- `contracts/task-registry` — task creation, completion, and access control
- `contracts/reward-engine` — proof submission, approval, and payout logic
- Cross-contract interaction vulnerabilities (re-entrancy, privilege escalation)
- Storage key collisions or data corruption vectors

The following are **out of scope**:

- Issues in upstream dependencies (report those to the respective maintainer)
- Theoretical vulnerabilities with no practical exploit path
- Issues in third-party deployment infrastructure

---

## Security Design Notes

Key security properties the contracts are designed to uphold:

- **Minting is minter-only.** Only the address stored as `minter` in the
  eco-token contract can mint new ECO tokens. The admin assigns the minter
  via `set_minter()`. In production, the minter is the reward engine contract.
- **Oracle is separated from admin.** The reward-engine enforces that the
  oracle and admin must be different addresses at initialization time.
- **Double-claim prevention.** The task-registry records each
  (task_id, user) completion pair and rejects duplicate claims.
- **Overflow protection.** All arithmetic on balances and supply uses
  `checked_add` / `checked_sub` to prevent wrapping.
- **Proof immutability.** Once a proof CID is submitted it cannot be changed;
  only its status (pending → approved / rejected / disputed) evolves.
- **Reward range guards.** Admins can set a platform-wide min/max reward
  band to limit oracle payout discretion.

All contracts are intended for formal audit before mainnet deployment.

---

## Bug Bounty

A bug bounty programme is planned for the mainnet launch. Details will be
published on [ecotask.network](https://ecotask.network) closer to launch.

---

## Contact

| Channel | Address |
|---------|---------|
| Security email | security@ecotask.network |
| General contact | hello@ecotask.network |
| GitHub | [@ecotask-network](https://github.com/ecotask-network) |
