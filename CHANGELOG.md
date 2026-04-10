# Changelog

All notable user-facing changes to the X07 toolchain are documented in this file.

## Unreleased

### Added

- Compat corpus CI gate (`scripts/ci/check_compat_corpus.sh`, `tests/compat_corpus/`) to prevent ecosystem regressions.
- Compatibility contract documentation (`docs/reference/compat.md`).

### Changed

- Expanded `docs/versioning-policy.md` to clarify toolchain/package/lockfile versioning and compat guardrails.
- Contract enforcement now typechecks only contract clauses (requires/ensures/invariant/decreases) instead of full bodies.
- Typechecker now supports call-argument compatibility `bytes -> bytes_view` (call-site-only) to match compiler behavior.
- Improved `if` branch mismatch diagnostics to point at a specific branch and suggest canonical conversions.

### Breaking changes

- None.
