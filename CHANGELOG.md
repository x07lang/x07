# Changelog

All notable user-facing changes to the X07 toolchain are documented in this file.

## Unreleased

### Fixed

- An unresolved call head that reaches codegen — typically a stdlib/package
  function whose module was not imported (e.g. `std.bytes.copy` without `import
  std.bytes`) — now reports `X07-TYPE-CALL-0001` (unknown callee) with the callee
  and an import hint, instead of `X07-INTERNAL-0001` (an internal-error code) with
  no guidance. `x07 check --project` now flags it cleanly in the loop.
- `X07-TYPE-UNIFY-0001` type errors now name the mismatched types — `type
  mismatch: expected \`<T>\`, got \`<U>\`` — instead of a bare "unification
  failure". The most common type-error class is now actionable without
  binary-searching the expression for the conflicting types.
- The `unsupported schema_version` parse error now detects when a file targets a
  **newer** x07AST schema than the toolchain supports and points at updating the
  toolchain (`x07up`, or raise the channel/version pin in `x07-toolchain.toml`),
  instead of the misleading "if this comes from a dependency package, upgrade the
  package" hint — the toolchain is what's behind, not a dependency.

## v0.2.16

### Fixed

- `try` on a `result_*` value whose result kind differs from the enclosing
  function's now reports a precise, actionable error (which result type to
  declare, or to handle the value explicitly with `is_ok`/`unwrap_or`) instead of
  a bare `X07-TYPE-UNIFY-0001: unification failure` with no pointer to `try`. This
  is the common failure when obtaining a validated string
  (`try (std.str.from_bytes_v1 …)`) inside a `result_i32` function.
- The `std.str` example in `docs/language/types-memory.md` returned `result_i32`
  while `try`-ing a `result_bytes` value — a pattern that does not compile. It now
  returns `result_bytes`, with a note documenting the `try` result-kind rule.
- `tests/smoke_str.x07.json` used the same non-compiling pattern (and was masked
  by suite fail-fast); rewritten as a `result_bytes` validator probe consumed by a
  `result_i32` assertion entry, so the canonical `std.str` test compiles and runs.

### Changed

- `x07 guide` now documents the `f64` builtins (`f64.of_i32`, `f64.to_i32_trunc`,
  `f64.add`/`sub`/`mul`/`div`) and that f64 v1 is a transient compute scalar (no
  literal, comparison, serialization, or record/enum field).

## v0.2.15

### Added

- RFC 0002 expressiveness floor, opt-in via the additive schema
  `x07.x07ast@0.9.0` (concrete-only `0.8.0` programs stay valid unchanged):
  - `f64`: IEEE-754 double scalar with explicit conversions (`f64.of_i32`,
    `f64.to_i32_trunc`) and arithmetic (`f64.add`/`sub`/`mul`/`div`); strict,
    deterministic floating point (no fast-math, no FMA contraction).
  - `defrecord`: nominal product types lowered to fixed-layout branded `bytes`
    (generated `<Record>.make` constructor and `<Record>.<field>` accessors).
  - `defenum` + `match`: nominal tagged unions (`[u32 tag][payload?]` branded
    `bytes`) consumed by an exhaustive `match` form with payload binding.
  - Validated UTF-8 strings via the new `std.str` stdlib module — a string is
    `bytes` branded `std.str.utf8_v1` (`from_bytes_v1`, `as_bytes`, `len`,
    `char_count`, `slice_v1`, `to_lower_ascii`/`to_upper_ascii`).
- RFC 0001 (x07text surface syntax) promoted to **Accepted**.
- Drift gate for documented diagnostic-catalog size: prose that states the
  catalog code count (e.g. `docs/why-x07.md`) is now checked against
  `catalog/diagnostics.json` in the canary gate, so the hand-written count
  can no longer rot silently.

### Changed

- `x07 init` for `sandbox`-default templates (`fs-tool`, `http-client`,
  `web-service`, `sqlite-app`, …) now suggests `x07 run --profile os` (the
  unsandboxed quick-run path) and notes that the default sandboxed run needs a
  VZ guest bundle (`X07_VM_VZ_GUEST_BUNDLE`), instead of a bare `x07 run` that
  fails out of the box without the sandbox guest bundle.

### Fixed

- `x07 run` failures from a fatal sandbox/exec setup error (e.g. a missing VZ
  guest bundle) now populate `compile.compile_error` in the JSON report with
  the reason and fix hint, instead of leaving it null (the message was
  previously only in `stderr_b64`/stderr), so a failed run is self-describing.

## v0.2.14

### Added

- `x07 doctor`: advisory `formal_prover_z3_cbmc` check reporting whether `z3`
  and/or `cbmc` are on `PATH` (the external provers `x07 verify --prove` and
  `x07 trust certify` invoke). Always advisory — when a prover is absent,
  `x07 prove check` still validates the certificate's proof binding structurally.

### Changed

- Official ext packages: the `ext-data-model` / `ext-json-rs`
  reverse-dependency closure is republished with SemVer **range**
  `requires_packages` pins (`name@>=X.Y.Z <UPPER`) instead of exact pins —
  ranges in `requires_packages`, exact versions in lockfiles. Downstream
  projects resolve the latest compatible transitive versions without exact-pin
  cascades. Range `requires_packages` require the v0.2.13+ resolver.

## v0.2.13

### Added

- Generic arithmetic: `ty.add` / `ty.sub` / `ty.mul` intrinsics for the
  `num_like` bound (`i32`/`u32`; lower to `+`/`-`/`*`, wrap modulo 2^32). A
  generic numeric fold/sum/reduce is now expressible; previously only generic
  comparison/ordering and LE (de)serialization were. No generic division.
- `std.bytes.find_sub(hay, needle) -> i32`: substring search (first occurrence
  index, or -1; empty needle returns 0), shipped in std-core 0.1.4. Closes the
  gap that forced hand-rolled O(n·m) scans for `contains`-style filters.
- Lint-stage enforcement of three rules that previously only failed at
  `x07 check` (codegen), so `x07 lint` catches them in the fast loop:
  `X07-TY-0102` (unknown `ty.*` intrinsic, with a did-you-mean), `X07-CONC-0001`
  (`task.scope_v1`/`task.scope.*` inside a plain `defn`; structured concurrency
  is solve/defasync-only), and `X07-IMPORT-0002` (importing a builtin namespace
  such as `std.brand`, with a remove-the-import quickfix).
- `x07 doc` did-you-mean on unknown `ty.*` intrinsics; unknown-module errors
  recognize builtin namespaces and advise removing the import.
- Docs: a generics reference (`docs/language/generics.md`: define/call, the
  bound → `ty.*` map, all intrinsic signatures including arithmetic) and a
  concurrency-and-certification section (kernel/shell: certify the pure kernel,
  keep `task.scope_v1` in the solve shell; static slots vs dynamic channel
  fan-out).

### Fixed

- Module-local typecheck (`x07 lint`) no longer pins an unresolved
  imported-callee result to `bytes_view` at a coercible call-arg position,
  which falsely retyped locals and rejected later owned uses
  (X07-TYPE-SET-0002). (Shipped in the dogfood line; recorded here.)

### CI

- The release `guest-runner-image` job builds per-arch on native runners
  (amd64 + `ubuntu-24.04-arm`) with a registry build cache instead of emulating
  arm64 under QEMU, cutting the job from ~2h toward ~15–20 min.

## v0.2.12

### Added

- x07AST: defn/defasync declarations accept an optional `doc` string,
  preserved through `x07 fmt` and the `x07 ast to-text` / `from-text`
  round-trip; `x07 doc` renders it for project and package symbols. Schemas
  updated additively (`x07.x07ast@0.8.0`).
- `meta.requires_packages` accepts SemVer comparator ranges
  (`name@>=1.2.3 <1.3.0`) alongside exact versions: in-project versions that
  satisfy a range are kept, otherwise the highest satisfying version is
  selected (vendored deps, official tree, then index; never pre-releases) and
  `x07.lock.json` freezes the choice. `x07 pkg tree` resolves range edges;
  `x07 pkg publish` prints the registry's reverse-dependency conflict
  warnings. See the updated `docs/versioning-policy.md` pin policy.
- Run reports gain `trap_help` next to `trap`: fuel exhaustion names the
  configured limit and `--solve-fuel`; `map_u32 full` points at
  `std.hash_map.with_capacity_u32`. Runner report schemas updated additively.
- `x07 doc` text output renders behavioral summaries (module listings and
  symbol view), generic signatures with type params (`std.heap.push[A:
  orderable](...)`) plus a `tapp` call template, and falls back to defn
  `doc` strings; structured `type_ref` params no longer render as empty
  types. Doc report schema reconciled (`summary`, `type_params`).
- `x07 ast from-text` defaults a missing `:decls` to the empty list for
  entry files.
- Stdlib summaries document the `std.hash_map` fixed-capacity contract
  (cap is total open-addressing slots; full table traps `map_u32 full`;
  key `-1` is the reserved empty-slot sentinel), the vec_u8
  accumulate-then-freeze contract, and `std.small_map` bytes-key behavior.
- Docs: program-level performance tuning section (fuel accounting, owned vs
  view discipline, arena patterns, `x07 run` recompile vs `x07 build`),
  six new agent-quickstart pitfalls, requires_packages range guidance.

### Fixed

- Module-local typecheck (lint) no longer pins an unresolved imported-callee
  result meta to `bytes_view` at coercible call-arg positions, which
  falsely retyped locals and rejected later moves with X07-TYPE-SET-0002.
- `if`-branch type mismatch errors from project compile name the branch
  types concisely and suggest `set0` for statement assignments.
- `x07up list` without flags lists installed toolchains instead of erroring.
- Stdlib spec source docs no longer carry `utm_source` link suffixes that
  broke the website link checker.

### Packages

- ext-json-rs 0.1.8: doc-annotated public pointer/data-model API (including
  the empty-bytes missing-path sentinel), SPDX license metadata.
- ext-data-model 0.1.12: requires ext-json-rs@0.1.8, healing the
  latest-with-latest auto-deps conflict.

## v0.2.11

### Added

- Did-you-mean suggestions on unknown callees (`X07-TYPE-CALL-0001` now
  carries ranked `data.suggestions` and a "did you mean" note), including
  unknown dotless heads such as `==` (suggesting `=`), which previously
  surfaced only as an unsupported-head string from codegen.
- `x07 doc` fuzzy lookup: not-found queries return ranked near-matches over
  module ids and builtin stdlib export symbols (for example `split` finds
  `std.text.ascii.split_u8`), and symbol misses inside a found module rank
  that module's exports.
- `x07 doc` behavioral summaries: export rows gain an optional `summary`
  field with one-line behavioral contracts (separators, encodings, error
  codes, move semantics) for 79 stdlib exports, sourced from a
  toolchain-owned sidecar.
- x07text (RFC 0001): `x07 ast to-text` / `x07 ast from-text` provide a
  lossless, deterministic text projection of x07AST JSON; `from-text` output
  is byte-identical to `x07 fmt` canonical bytes; a corpus round-trip gate
  covers the full stdlib and fixture tree. New docs page
  `docs/language/x07text.md`.
- Failed-compile runner reports (`x07-host-runner`, `x07-os-runner`) attach
  structured lint diagnostics (pointer, provenance, quickfix) as
  `compile.diagnostics` alongside `compile_error`; runner report schemas gain
  the optional `diagnostics` property (additive).
- `labs/agent-eval`: comparative agent benchmark harness (python, rust, x07,
  x07text arms; 12 vector-judged tasks) with pilot results and a scaled-run
  RUNBOOK containing a predeclared decision rule for the direct-authoring
  bet.

### Changed

- Project story and roadmap: X07 leads as the deterministic, certifiable
  execution substrate for agent-written software; direct-authoring
  investment (RFC 0002 expressiveness floor) is gated on the comparative
  eval. Active ecosystem scope narrows to `x07`, `x07-mcp`, `x07-registry`,
  `x07-wasm-backend`, and `hardproof`.

### Deprecated

- Device/app delivery surfaces: `x07-device-host`, `x07-web-ui`, the studio
  and demo repos, and the platform control-plane repos are archived
  (2026-06 scope cut). Their bundle/compat entries remain in this release
  for installer continuity and are planned for removal from the release
  train in a future release. Note: external packages currently pin
  `x07c_compat < 0.3.0`, so the next minor requires a coordinated package
  compat-widening train.

### Carried docs notes (pre-0.2.11 unreleased)

### Added

- XTAL verify summaries and proof-timeout diagnostics now record the effective proof solver budget used for the proof lane.
- XTAL verify summaries now include the first proof diagnostic code/message for proof rows that are unsupported, inconclusive, timed out, or missing tools.

### Changed

- Clarified agent-facing XTAL docs for current `x07 init` templates, generated PBT test selection, and project-root-relative XTAL report paths.
- Clarified the XTAL example agent loop so generated `ensures_props` property wrappers are run with `x07 test --all`.
- Added XTAL proof-warning triage guidance for balanced-policy summaries, SMT timeouts, and proof-facing loop design.
- Clarified that proof-facing XTAL operations need a simplified implementation body, not only a narrower public spec surface.
- Clarified the agent pattern for composing owned-byte helpers from reusable public byte params using `bytes.view` plus `view.to_bytes` copies.
- Documented `x07.patchset@0.1.0` / `x07 patch apply` as the deterministic multi-file edit path for XTAL agents.

## v0.2.10

### Added

- `x07 pkg inventory` for emitting an offline inventory of stdlib + official external packages shipped in the current toolchain bundle.
- `x07 init --template xtal-pure` and `x07 init --template xtal-verified` for scaffolding solve-pure XTAL starter projects.
- `x07 verify --z3-timeout-seconds` for bounding SMT/prove solver runtime.
- `x07 verify --z3-memory-mb` for bounding SMT/prove solver memory.
- XTAL docs:
  - `docs/toolchain/xtal-targets.md` (certification target semantics)
  - `docs/toolchain/proof-subset.md` (compact proof-supported subset guide)
  - `docs/packages/inventory.md` (offline package inventory entry point)
  - XTAL example project: `docs/examples/agent-gate/xtal/workflow-graph/` (branded multi-operation pure library surface).

### Changed

- `x07 init` now copies `.agent/docs/` and `.agent/skills/` into the project for portability (instead of creating toolchain-path symlinks).
- `x07 arch check` now accepts `--project <x07.json|dir>` as an alternative to `--repo-root`.
- `x07 trust certify` and `x07 xtal certify` now accept `--no-fail-fast` to preserve full test signal in the certification test lane.
- `x07 trust certify` and `x07 xtal certify` now accept proof-budget overrides (`--unwind`, `--max-bytes-len`, `--input-len-bytes`, `--z3-timeout-seconds`, `--z3-memory-mb`) and forward them to `x07 verify --prove`.
- `x07 xtal verify` now runs the generated test lane with `--no-fail-fast` to preserve full suite signal after the first failure.
- `x07 xtal verify --proof-policy balanced` now uses smaller default proof bounds and a shorter solver timeout (override with `--unwind`, `--max-bytes-len`, `--input-len-bytes`, and `--z3-timeout-seconds`).

### Fixed

- XTAL generated PBT driver generation no longer emits duplicate local bindings for mixed and multi-bytes signatures (for example `(i32, bytes)` and `(i32, bytes, bytes)`).
- Branded PBT inputs are now validated via brand casts and treated as skipped when invalid (instead of being counted as passing cases).
- `x07 xtal verify` now emits a compact per-entry proof support summary (first diagnostic code/message) when proofs are unsupported or inconclusive.
- `x07 xtal dev` now surfaces `x07 xtal verify` warnings in the top-level diagnostics report.
- `x07 verify --prove` no longer rejects `for` loops solely because their bounds are non-literal.
- `x07 trust profile check` and `x07 trust certify` no longer scan toolchain/dependency sources when enforcing language-subset flags (only declared `module_roots`).

## v0.2.9

### Fixed

- Formal verification perf budgets refreshed for `verified_core_fixture.prove` so release-readiness CI stays consistent.
- `labs/scripts/ci/check_formal_verification_perf.py` now supports `--enforce` and `--scenario` for faster local CI reproduction.

## v0.2.8

### Fixed

- Release CI now includes the XTAL improve fixture incident bundle used by `scripts/ci/check_xtal_improve.sh`.
- `scripts/bump_toolchain_version.py` now refreshes `ci/fixtures/**/x07.lock.json` lockfiles during version bumps so canary lock checks stay consistent.
- `scripts/ci/check_threads_smoke.sh` now prints `x07 pkg lock --check` output on failure to speed up debugging.

## v0.2.7

### Fixed

- `docs/examples/**/x07.lock.json` lockfiles are now refreshed during version bumps so `x07 pkg lock --check` stays consistent for docs projects.

## v0.2.6

### Fixed

- `scripts/bump_toolchain_version.py` now refreshes `docs/_generated/versions.json` during version bumps so `scripts/ci/check_all.sh` stays consistent.

## v0.2.5

### Fixed

- `x07 run` now emits contract repro artifacts for `run-os*` worlds even when the runner trap field is non-contract output (by extracting `X07T_CONTRACT_V1 ...` from captured stderr when needed).

## v0.2.4

### Added

- XTAL tooling:
  - `x07 xtal spec fmt|lint|check|extract|scaffold` for authoring, validating, and extracting `*.x07spec.json` modules and `*.x07spec.examples.jsonl`.
  - `x07 xtal tests gen-from-spec` for generating deterministic unit tests from spec examples and property checks from `ensures_props` under `gen/xtal/`.
  - `x07 xtal impl check|sync` for validating and synchronizing implementation exports/signatures/contracts against specs (including optional patch emission via `impl sync --patchset-out`).
  - `x07 xtal dev` and `x07 xtal verify` wrappers for a single-command XTAL loop (spec checks, generator drift checks, impl conformance checks, verification runs, and test execution), with `x07 xtal dev --prechecks-only` and `x07 xtal dev --repair-on-fail`.
  - `x07 xtal repair` for a bounded repair loop that emits an `x07.patchset@0.1.0` + deterministic review diff under `target/xtal/repair/` (and can optionally emit a spec witness suggestion with `--suggest-spec-patch`).
  - `x07 xtal certify` for producing a manifest-driven certification bundle via `x07 trust certify`, writing a summary under `target/xtal/cert/`.
  - `x07 xtal ingest` for normalizing runtime violation bundles (or contract repros) into a canonical workspace under `target/xtal/ingest/` (and optionally running an improvement loop).
  - `x07 xtal improve` for consuming incidents (violation bundles, contract repros, or recovery event logs) and coordinating a bounded verify/repair/certify run under `target/xtal/`.
  - `x07 xtal tasks run` for executing recovery tasks from `arch/tasks/index.x07tasks.json` for an incident input (and emitting optional recovery events under `target/xtal/events/`).
- Generator determinism gate:
  - `arch/gen/index.x07gen.json` (`x07.arch.gen.index@0.1.0`) for declaring generator outputs and pinned invocations.
  - `x07 gen verify|write` for byte-for-byte drift checks and (optional) double-run determinism verification across declared generators.
- Schemas:
  - `x07.x07spec@0.1.0` and `x07.x07spec_examples@0.1.0` for XTAL spec and example artifacts.
  - `x07.xtal.manifest@0.1.0` for `arch/xtal/xtal.json` (XTAL manifest).
  - `x07.xtal.verify_summary@0.1.0` for aggregate `x07 xtal verify` outputs (`target/xtal/verify/summary.json`).
  - `x07.xtal.repair_summary@0.1.0` for aggregate `x07 xtal repair` outputs (`target/xtal/repair/summary.json`).
  - `x07.xtal.certify_summary@0.1.0` for aggregate `x07 xtal certify` outputs (`target/xtal/cert/summary.json`).
  - `x07.xtal.cert_bundle@0.1.0` for the `x07 xtal certify` bundle manifest (`target/xtal/cert/bundle.json`).
  - `x07.xtal.violation@0.1.0` for runtime contract violation bundles (`target/xtal/violations/<id>/violation.json`).
  - `x07.xtal.ingest_summary@0.1.0` for `x07 xtal ingest` summary outputs (`target/xtal/ingest/summary.json`).
  - `x07.xtal.recovery_event@0.1.0` for recovery event log entries (JSONL; `target/xtal/events/<id>/events.jsonl`).
  - `x07.xtal.improve_summary@0.1.0` for aggregate `x07 xtal improve` outputs (`target/xtal/improve/summary.json`).
  - `x07.arch.tasks.index@0.1.0` for task policy graphs (`arch/tasks/index.x07tasks.json`).
- Formal verification:
  - `x07 verify --input-len-bytes` for overriding the verification input encoding length (advanced; used by wrappers that derive verification inputs).
  - `x07 verify --prove` proof caching keyed by declaration hash + imported proof-summary digests, storing summaries under `.x07/cache/verify/proof_summaries/` and (when `--emit-proof` is used) proof bundles under `.x07/cache/verify/proofs/`.

### Changed

- `x07 xtal dev` now runs `x07 xtal verify` by default (pass `--prechecks-only` to stop after spec/gen/impl checks).
- `x07 xtal ingest` now runs `x07 xtal improve` by default (pass `--normalize-only` to stop after normalization), and accepts `--improve-out-dir` to control improvement artifacts.
- `x07 xtal verify` now runs `x07 verify --coverage` and `x07 verify --prove` for each spec operation entrypoint (and records results under `target/xtal/verify/`).
- `x07 xtal verify` now routes nested verification and test artifacts under `target/xtal/verify/_artifacts/` (and enforces solve-world determinism by default).
- `x07 xtal verify --proof-policy balanced` now treats missing proof tools as warnings (and verification continues); `--proof-policy strict` requires proven outcomes.
- `x07 xtal verify` now writes per-entry proof bundles under `target/xtal/verify/prove/<module>/<local>/` to avoid proof object collisions across modules.
- `x07 xtal impl check` now enforces that `ensures_props[*].prop` symbols exist, are exported, and have compatible signatures for the selected args.
- `x07 xtal repair --write` now requires `arch/xtal/xtal.json` and enforces `autonomy.agent_write_paths[]` boundaries for patch targets.
- `x07 xtal certify` now accepts `--spec-dir` and writes a certification bundle manifest to `target/xtal/cert/bundle.json` (binds output digests plus spec/example digests).
- `x07 xtal ingest` now validates `violation.json` ↔ `repro.json` integrity and records contract/source/tool metadata (and can ingest `events.jsonl` inputs).
- `x07 trust certify` now supports `--fail-on` (trust report gates) and `--review-fail-on` (review diff gates) for CI posture enforcement, and writes `review.diff.txt` when a baseline is provided.
- Contract repros emitted from `x07 run` in `run-os*` worlds now prefer replayable `solve-rr` repros by capturing record/replay fixtures under `.x07/artifacts/contract/<id>/rr`.

### Fixed

- `x07 prove check` no longer spuriously rejects proofs when the proof run requires a larger verification-input encoding (now recorded and validated consistently during replay).
- `x07 test --allow-empty` now accepts manifests with an empty `tests[]` array (useful for generated test manifests that intentionally select 0 tests).
- `x07 xtal repair` no longer fails candidate evaluation with ambiguous module roots when overlaying patched modules.
- `x07 xtal repair` semantic repair no longer performs unbounded expression enumeration at default budgets (avoids hangs when examples are weak).

## v0.2.3

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
- JSON reporting reference implementation:
  - Guide + API docs: `docs/guides/json-reporting.md`, `docs/libraries/ext-data-model.md`, `docs/libraries/ext-json-rs.md`
  - Runnable example: `docs/examples/agent-gate/json-report/`
  - Template: `x07 init --template json-report`
- Packaging integrity tooling:
  - `x07 pkg verify` to validate sparse-index signatures (ed25519) and clearly report unsigned indices/packages.
  - `x07 pkg check-semver` to detect breaking export changes (removed exports or signature changes) between two package directories.
  - `x07 info` as a top-level alias for `x07 pkg info`.
  - `x07 pkg pack` / `x07 pkg publish` now validate required `x07-package.json` metadata (`description`, `docs`, `license`, `meta.x07c_compat`) before producing archives.
  - Package archives now include `ffi/` contents when present (for FFI-backed packages).
- Guide + runnable example: `docs/guides/packaging-integrity.md` and `docs/examples/packaging-integrity/`.
- `x07 init --package` now includes `license` and `meta.x07c_compat` in the generated `x07-package.json` template.

### Changed

- Built-in stdlib packages bumped to `stdlib/std-core/0.1.3` and `stdlib/std/0.1.3`.

### Fixed

- Dependency hydration and packaging errors now include more actionable next steps (including `--offline` / `X07_OFFLINE=1` guidance when the index would otherwise be consulted).
- Tool wrapper scope detection now recognizes `pkg tree` as `pkg.tree` (schema discovery and nondeterminism inference).
- `x07 check` backend-check now validates all declarations (including unreachable ones), surfacing latent codegen errors earlier.
- `x07 init --template fs-tool` now exercises sandboxed filesystem caps (read from `src/`, write to `out/`) instead of only echoing input bytes.
- Agent-gate CI now runs `x07 test` for example projects that include `tests/tests.json` and adds the `json-report` example to the gate.

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
