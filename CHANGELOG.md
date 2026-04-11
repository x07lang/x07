# Changelog

All notable user-facing changes to the X07 toolchain are documented in this file.

## Unreleased

### Added

- Compat corpus CI gate (`scripts/ci/check_compat_corpus.sh`, `tests/compat_corpus/`) to prevent ecosystem regressions.
- Compatibility contract documentation (`docs/reference/compat.md`).
- Project-level compatibility selection: `x07.project@0.5.0` adds `project.compat`, with `--compat` and `X07_COMPAT` overrides for compilation entry points.
- `x07 migrate` for deterministic mechanical rewrites (`--check` / `--write`) targeting `--to 0.5`.
- Core control-flow form `while`: `["while", cond, body]` (returns `i32` `0`).

### Changed

- Expanded `docs/versioning-policy.md` to clarify toolchain/package/lockfile versioning and compat guardrails.
- `x07 init` now emits `x07.project@0.5.0` and pins `compat: "0.5"` by default for new projects.
- Contract enforcement now typechecks only contract clauses (requires/ensures/invariant/decreases) instead of full bodies.
- Typechecker now supports call-argument compatibility `bytes -> bytes_view` (call-site-only) to match compiler behavior.
- Improved `if` branch mismatch diagnostics to point at a specific branch and suggest canonical conversions.
- `x07 verify` summaries now emit `source_path` relative to the project root when possible (portable artifacts; no machine-local absolute paths).
- `x07 pkg lock` now enforces package `meta.x07c_compat` when present; official packages ship `meta.x07c_compat` metadata.
- Recursion termination evidence (`decreases[]`) is now required only for directly recursive `defn` targets that declare any contract clauses; non-contract recursion no longer requires decreases boilerplate.
- `x07 fix` / `x07 migrate` can now auto-insert `decreases[]` for common recursion patterns (for example `n -> n-1`).

### Breaking changes

- `x07 pkg lock` can now refuse package versions whose `meta.x07c_compat` excludes the running compiler.
- Contract enforcement is now applied to x07AST `v0.7` and `v0.8` as well (and respects the active compat selection); invalid contract clauses that previously slipped through may now fail until fixed.
