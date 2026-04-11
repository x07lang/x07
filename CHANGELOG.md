# Changelog

All notable user-facing changes to the X07 toolchain are documented in this file.

## Unreleased

### Added

- Compat corpus CI gate (`scripts/ci/check_compat_corpus.sh`, `tests/compat_corpus/`) to prevent ecosystem regressions.
- Compatibility contract documentation (`docs/reference/compat.md`).
- Offline workflow guide (`docs/guides/offline.md`) covering lock checks and local index mirrors.
- Project-level compatibility selection: `x07.project@0.5.0` adds `project.compat`, with `--compat` and `X07_COMPAT` overrides for compilation entry points.
- `x07 migrate` for deterministic mechanical rewrites (`--check` / `--write`) targeting `--to 0.5`.
- `x07 project migrate` for migrating `x07.json` manifests from legacy schema lines to `x07.project@0.5.0` (inserts `compat: "0.5"` when upgrading).
- Core control-flow form `while`: `["while", cond, body]` (returns `i32` `0`).
- Project-local `x07 pkg` configuration via `.x07/config.json` or `x07.config.json` (`x07.config@0.1.0`) for `pkg.registry` and `pkg.offline`.
- `x07 pkg list` and `x07 pkg info` for browsing packages via a local `file://` sparse index mirror (and local `.x07/deps` when available).
- `x07 pkg repair --toolchain current` for deterministic lock repair after toolchain upgrades.
- `try_doc` special form: `["try_doc", doc_expr]` for doc-envelope propagation in `bytes`-returning functions.
- Built-in `std.doc` helpers and `docs/reference/doc-envelope.md` describing the stable doc-envelope encoding.
- Built-in `std.view.slice_v1` for clamped `bytes_view` slicing (never traps).
- Safe unsigned decimal parsing helpers in `std.parse`: `u32_status_le` and `u32_status_le_at` (non-trapping status bytes, with optional next-offset reporting).
- Stable encoding helpers in `std.codec`: `base64_encode_v1`, `base64_decode_v1`, `hex_encode_v1`, `hex_decode_v1` (decode returns a doc envelope).
- Iteration helpers in `std.small_map` / `std.small_set`: `iter_init_v1`, `iter_next_v1` (doc envelope results).
- `x07 explain <CODE>` top-level alias for `x07 diag explain <CODE>`.
- `x07 repro compile` for portable compile repro directory bundles.
- Perf canary `canary/ext_json_canonicalize_small` (bench suite now supports per-suite `module_roots` for resolving non-stdlib modules deterministically).
- `x07-agent-context` end-user skill for deterministic repair handoffs (`x07 agent context`).
- Canary gate `scripts/ci/check_doc_examples.sh` that lints `docs/examples/*.x07.json`.

### Changed

- Expanded `docs/versioning-policy.md` to clarify toolchain/package/lockfile versioning and compat guardrails.
- `x07 init` now emits `x07.project@0.5.0` and pins `compat: "0.5"` by default for new projects.
- Contract enforcement now typechecks only contract clauses (requires/ensures/invariant/decreases) instead of full bodies.
- Typechecker now supports call-argument compatibility `bytes -> bytes_view` (call-site-only) to match compiler behavior.
- Improved `if` branch mismatch diagnostics to point at a specific branch and suggest canonical conversions.
- `x07 verify` summaries now emit `source_path` relative to the project root when possible (portable artifacts; no machine-local absolute paths).
- `x07 pkg lock` now enforces package `meta.x07c_compat` when present; official packages ship `meta.x07c_compat` metadata.
- `x07 pkg` now accepts `--registry <URL>` as an alias for `--index <URL>` across subcommands.
- `x07 pkg lock` now supports `--lock-version {0.3|0.4}` (default: `0.4`) and `x07.lock@0.4.0` records toolchain identity and registry provenance.
- Recursion termination evidence (`decreases[]`) is now required only for directly recursive `defn` targets that declare any contract clauses; non-contract recursion no longer requires decreases boilerplate.
- `x07 fix` / `x07 migrate` can now auto-insert `decreases[]` for common recursion patterns (for example `n -> n-1`).
- Built-in stdlib is now split into `std-core@0.1.2` (foundational, pure modules) and `std@0.1.2` (extended modules depending on `std-core`).
- `x07 trust report` includes `std-core` SBOM components when `stdlib.std-core.lock` is present.
- `x07 check` diagnostics now include provenance fields (`module_id`, dependency `package{name,version}` when applicable, and best-effort `dependency_chain`).
- `x07 diag explain` now prints suggested `x07 fix` / `x07 migrate` commands when applicable.
- `x07 doc` now resolves common prelude names (for example `codec.*`, `bytes.get_u8`, `vec_u8.*`, `chan.bytes.*`, `task.scope.*`) and documents them in `docs/language/prelude-and-names.md`.
- `ext.json.canon.canonicalize` now emits canonical JSON without allocating intermediate per-value `bytes` buffers (lower heap and memcpy for nested objects/arrays).
- Getting-started docs and agent skills are now aligned on the canonical compat/migrate/while/try_doc narrative (and document `--compat` and `x07 repro compile` where applicable).

### Breaking changes

- `x07 pkg lock` can now refuse package versions whose `meta.x07c_compat` excludes the running compiler.
- `x07 pkg lock` now writes `x07.lock@0.4.0` by default; external tooling that only supports `x07.lock@0.3.0` must use `--lock-version 0.3`.
- Contract enforcement is now applied to x07AST `v0.7` and `v0.8` as well (and respects the active compat selection); invalid contract clauses that previously slipped through may now fail until fixed.
- Toolchain stdlib inventories are now split across `stdlib.lock` and `stdlib.std-core.lock`; out-of-tree tooling that only reads `stdlib.lock` may need updates.
