# Contracts-by-example (copy/paste)

This folder contains a minimal, schema-valid `arch/` tree you can copy into an X07 project.

Workflow:

1. Copy `arch/` into your project root.
2. Run `x07 arch check --write-lock`.
3. Commit:
   - `arch/manifest.lock.json`
   - `arch/contracts.lock.json`

Notes:

- RR policies are consumed by `std.rr.with_policy_v1` and friends.
- SM specs are consumed by `x07 sm check` / `x07 sm gen` and can be pinned via `arch/sm/index.x07sm.json`.
- Archive policies are consumed by archive helpers like `std.archive.*_from_arch_v1`.
- DB migration/query contracts are consumed by `std.db.migrate.*_from_arch_v1` and `std.db.rr`.
- Crypto contracts are consumed by helpers like `std.auth.jwt.*_from_arch_v1` (and other `std.crypto.*_from_arch_v1` APIs).
