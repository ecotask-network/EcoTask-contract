# Fix Report: Distinguish "expired allowance" from "no allowance" (eco-token)

**Repo:** `https://github.com/felladaniel36-hash/EcoTask-contract.git`
**Area:** `contracts/eco-token` (SEP-0041 token)
**Labels addressed:** `type: bug`, `type: api-design`, `area: eco-token`

## Problem
The `allowance(owner, spender)` read conflates two distinct states by returning `0`
in both cases:
- **No allowance exists** (storage key absent), and
- **Allowance expired** (key exists but `expiration_ledger` has passed).

A frontend or integrating contract calling `allowance()` before `transfer_from()`
cannot tell the user whether to show "Please approve" or "Your approval expired —
please re-approve". `transfer_from()` already returns distinct error strings
(`allowance not found` vs `allowance expired`), but the read path could not.

## Fix (acceptance criteria implemented)
1. **New API** — `allowance_with_expiry(owner, spender) -> Option<(i128, u32)>`:
   - `None` → no allowance exists.
   - `Some((0, expiration_ledger))` → allowance was set but has expired.
   - `Some((amount, expiration_ledger))` → live allowance.
   `allowance()` return type is unchanged (still `i128`, still `0` for both states)
   so SEP-0041 compliance is preserved.
2. **Lazy cleanup** — when `allowance()` is called on an *expired* entry, the
   storage key is removed (`remove_allowance`), reclaiming ledger rent while
   keeping the return value `0`.
3. **Tests** — cover live / expired / none for **both** `allowance()` and
   `allowance_with_expiry()`, plus a test proving the lazy cleanup removes the
   expired key.

## Files modified (2, no files created)
| File | Change |
|------|--------|
| `contracts/eco-token/src/storage.rs` | Added `remove_allowance(e, owner, spender)` to delete an allowance storage key. |
| `contracts/eco-token/src/token.rs` | `allowance()` now lazily removes expired keys; added `allowance_with_expiry()`; added 4 new unit tests. |

## Validation
- `cargo test -p eco-token` → **44 passed** (40 existing + 4 new). Existing allowance
  tests pass **unchanged**.
- `cargo test --workspace` → all crates pass (eco-token 44, reward-engine 53, its
  lifecycle 4, task-registry 39, reward integration 10).
- `cargo fmt --all -- --check` → passes.
- `cargo clippy --all-targets --all-features -- -D warnings` → passes.
- `cargo build --target wasm32v1-none --release -p eco-token` → succeeds.

## Notes
- Out-of-scope items from the issue were not touched: `approve`/`transfer_from`
  error messages and SEP-0041 certification are unchanged.
- Generated `test_snapshots/*` files are git-ignored per the repo `.gitignore`
  (`**/test_snapshots/`) and do not appear in the diff; unrelated snapshot churn in
  other crates from running the full suite was reverted to keep the diff focused.
