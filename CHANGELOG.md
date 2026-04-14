# Changelog

All notable user-facing changes to the X07 toolchain are documented in this file.

## Unreleased

### Added

- `x07 run --offline` (and `X07_OFFLINE=1`) to forbid network access during dependency hydration (the implicit `x07 pkg lock` step).
- `x07 pkg tree` for a deterministic dependency-closure graph from `x07.json` + `x07.lock.json`, including declared and resolved module roots.
- `x07 check --ast` for schema/shape validation + lint only (no typecheck or backend-check), intended for fast x07AST authoring feedback.
- Lint diagnostics for common x07AST authoring mistakes:
  - `X07-ARITY-BINOP-0001` for n-ary uses of binary operators (for example `["+", 1, 2, 3]`).
  - `X07-FOR-0001` when the `for` loop variable is not an identifier.
- Guide + runnable example: `docs/guides/ast-authoring-best-practices.md` and `docs/examples/ast-authoring-best-practices/`.
- Stdlib ergonomics helpers:
  - Decimal parsing: `std.parse.u32_dec` and `std.parse.i32_dec` (both return `result_i32`).
  - Bytes views: `std.bytes.trim_ascii_view`, `std.bytes.strip_prefix_view`, `std.bytes.strip_suffix_view`.
  - JSON: `std.json.encode(json, opts)` (with canonical key ordering via `opts & 1`) and `std.json.pretty_encode(json)`.
  - Paths: `std.path.normalize_posix`, `std.path.is_safe_relative`, `std.path.parent`, `std.path.join_checked`.
- Guide + runnable example: `docs/guides/stdlib-ergonomics.md` and `docs/examples/agent-gate/stdlib-ergonomics/`.
- Safe archive processing via `ext-archive-c@0.1.5`:
  - `std.archive.safe_extract_v1` (tar/tgz/zip) with strict path policies, explicit caps, and structured issues (`x07.archive.issue@0.1.0`).
  - Pinned archive profiles under `arch/archive/profiles/` (`*_extract_safe_v1.archive.json`).
- Archive security corpus + CI gate: `tests/corpora/archive/` and `scripts/ci/check_archive_corpus.sh`.
- Guide + API docs + runnable example: `docs/guides/safe-archives.md`, `docs/archive/archive-v1.md`, and `docs/examples/agent-gate/archive-safe-extract/zip-hello/`.
- Streaming filesystem IO via `ext-fs@0.1.6`:
  - streaming reader/writer handles under `std.os.fs.stream_*_v1`
  - `std.os.fs.copy_file_v1` and `std.os.fs.stream_copy_to_end_v1`
- Streaming archive extract-to-fs via `ext-archive-c@0.1.6` + native `os.archive.*`:
  - `std.archive.extract_os.safe_extract_to_fs_path_v1` and `std.archive.extract_os.extract_to_fs_path_from_arch_v1`
  - Guide + runnable example: `docs/guides/streaming-io.md` and `docs/examples/agent-gate/archive-extract-to-fs/zip-hello/`
- Minimal deterministic profiling via `X07_PROFILE=1` (JSON line `x07.profile.fn@0.1.0` on stderr); docs: `docs/toolchain/profiling.md`.
- CLI specrows tooling:
  - `x07 cli specrows check` and `x07 cli specrows fmt` (alias: `x07 cli spec`) for semantic validation and canonical formatting.
  - `x07 cli specrows compile` for emitting specbin for `ext.cli.parse_compiled*`.
- CLI v2 (`ext-cli`) features:
  - typed options (`U32`, `I32`, `PATH`, `BOOL`, `ENUM`, `BYTES_HEX`)
  - built-in help renderer (`ext.cli.render_help`)
  - stable machine-readable error map (`ext.cli.err_doc_v2`)
- Guide + API docs + runnable example: `docs/guides/cli-patterns.md`, `docs/libraries/ext-cli.md`, and `docs/examples/agent-gate/cli-ext-cli/`.
- Packaging integrity tooling:
  - `x07 pkg verify` to validate sparse-index signatures (ed25519) and clearly report unsigned indices/packages.
  - `x07 pkg check-semver` to detect breaking export changes (removed exports or signature changes) between two package directories.
  - `x07 info` as a top-level alias for `x07 pkg info`.
- Guide + runnable example: `docs/guides/packaging-integrity.md` and `docs/examples/packaging-integrity/`.
- `x07 init --package` now includes `license` and `meta.x07c_compat` in the generated `x07-package.json` template.

### Changed

- Built-in stdlib packages bumped to `stdlib/std-core/0.1.3` and `stdlib/std/0.1.3`.

### Fixed

- Dependency hydration and packaging errors now include more actionable next steps (including `--offline` / `X07_OFFLINE=1` guidance when the index would otherwise be consulted).
- Tool wrapper scope detection now recognizes `pkg tree` as `pkg.tree` (schema discovery and nondeterminism inference).
- `x07 check` backend-check now validates all declarations (including unreachable ones), surfacing latent codegen errors earlier.

## v0.2.2

### Fixed

- `x07 check` now typechecks calls into imported builtin stdlib modules (for example `std.bytes.*`), so many former codegen `X07-INTERNAL-0001` failures become proper type diagnostics.
- Unknown callee typos in imported modules now produce `X07-TYPE-CALL-0001` (type stage) instead of falling through into codegen errors.
- Tool wrapper nondeterminism inference now marks `x07 init --template ...` flows as `meta.nondeterminism.uses_network=true` when they lock dependencies against a non-`file://` package index.

## v0.2.1

### Fixed

- Tool reports no longer emit empty `X07-TOOL-EXEC-0001` messages when a wrapped command fails with empty stderr (now falls back to child JSON `error.message` when present).
- Tool report `meta.nondeterminism.uses_network` is now inferred for `x07 pkg*` scopes (false for `--offline` and `file://` registries).
- `x07 explain` / `x07 diag explain` now finds the diagnostics catalog from an installed toolchain layout (no longer requires running from the repo root).

### Packaging

- Toolchain archives now include `catalog/diagnostics.json` and `stdlib.std-core.lock`.

## v0.2.0

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
