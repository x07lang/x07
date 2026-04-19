# XTAL (intent → spec)

XTAL provides a spec-first surface for X07 projects:

- `*.x07spec.json` modules are the canonical intent/behavior interface.
- Optional `*.x07spec.examples.jsonl` example cases provide a minimum semantic oracle.
- A deterministic generator converts examples and declared properties into normal X07 tests under `gen/xtal/`.

Example projects:

- Smallest end-to-end: `docs/examples/agent-gate/xtal/toy-sorter/`
- Branded multi-operation library surface: `docs/examples/agent-gate/xtal/workflow-graph/`

## Artifacts

### XTAL manifest (`x07.xtal.manifest@0.1.0`)

- Location: `arch/xtal/xtal.json`
- Schema: `x07.xtal.manifest@0.1.0` (see `docs/spec/schemas/x07.xtal.manifest@0.1.0.schema.json`)
- Provides:
  - `entrypoints[]` for certification targets
  - `trust.cert_profile` and `trust.review_gates[]` for certification review gates
  - `autonomy.agent_write_paths[]` and `autonomy.agent_write_specs|agent_write_arch` for repair boundaries
- See also: `docs/toolchain/xtal-targets.md`.

### Spec modules (`x07.x07spec@0.1.0`)

- Location: `spec/<module_id>.x07spec.json` (recommended)
- Schema: `x07.x07spec@0.1.0` (see `docs/spec/schemas/x07.x07spec@0.1.0.schema.json`)

### Example cases (`x07.x07spec_examples@0.1.0`, JSONL)

- Location: `spec/<module_id>.x07spec.examples.jsonl` (optional)
- Schema: `x07.x07spec_examples@0.1.0` (see `docs/spec/schemas/x07.x07spec_examples@0.1.0.schema.json`)
- One JSON object per line.

Minimum value encodings:

- `bytes` / `bytes_view`: `{"kind":"bytes_b64","b64":"..."}`
- `i32`: a JSON integer (and the object form `{"kind":"i32","i32":123}` / `{"kind":"i32","value":123}` is accepted)

### Properties (`ensures_props`)

Each operation can declare `ensures_props[]` entries that reference a property function.

- The property function is executed under property-based testing (PBT).
- The property function MUST return a `bytes_status_v1` payload (see `std.test.status_ok` / `std.test.status_fail`).
- `ensures_props[*].args[]` selects which operation parameters are passed to the property function.
- `x07 xtal impl check` enforces that the referenced function exists, is exported, has a compatible signature for the selected args (including brands), and returns `bytes`.

## Branded bytes (canonical example)

If you use brands on public byte boundaries, keep the brand ids aligned across:

- `spec/*.x07spec.json`: `operations[*].params[*].brand` and `operations[*].result_brand`
- `src/*.x07.json`: `defn.params[*].brand` and `defn.result_brand`
- generated property wrappers: `gen/xtal/**/tests.x07.json` (generated from spec)
- boundary index: `arch/boundaries/index.x07boundary.json`

Concrete reference: `docs/examples/agent-gate/xtal/workflow-graph/`.

Key files:

- Spec brands: `docs/examples/agent-gate/xtal/workflow-graph/spec/workflow.graph.x07spec.json`
- Impl brands: `docs/examples/agent-gate/xtal/workflow-graph/src/workflow/graph.x07.json`
- Generated wrappers: `docs/examples/agent-gate/xtal/workflow-graph/gen/xtal/workflow/graph/tests.x07.json`
- Boundary brands: `docs/examples/agent-gate/xtal/workflow-graph/arch/boundaries/index.x07boundary.json`

## Commands

### Authoring

- `x07 xtal spec scaffold --module-id <id> --op <local_name> --param <name:ty> --result <ty> [--examples] [--out-path <path>]`
- `x07 xtal spec fmt --input <spec.x07spec.json> --write [--inject-ids]`
- `x07 xtal spec extract --project x07.json (--module-id <id> | --impl-path <path>) (--write | --patchset-out <path>)`

### Validation

- `x07 xtal spec lint --input <spec.x07spec.json>`
- `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`
  - Validates op ids, signatures, contract clause type/purity checks, and example cases.

### Tests generation (examples → unit tests)

- `x07 xtal tests gen-from-spec --project x07.json --write`
  - Writes:
    - `gen/xtal/tests.json` (`x07.tests_manifest@0.2.0`)
    - `gen/xtal/<module_path>/tests.x07.json` (`module_id: gen.xtal.<module_id>.tests`)
  - Generates:
    - unit tests from examples (returns `result_i32`)
    - PBT property wrappers for `ensures_props` (returns `bytes_status_v1`)
- `x07 xtal tests gen-from-spec --project x07.json --check`
  - Fails if any generated output would change (drift check).

### Implementation conformance

- `x07 xtal impl check --project x07.json`
  - Validates that each spec module has a corresponding implementation module under `src/`.
  - Validates exports, signatures, and contract-core clause alignment.
- `x07 xtal impl sync --project x07.json --write`
  - Creates missing modules and stubs, adds missing exports, and syncs contract-core clauses.
  - If `--patchset-out <path>` is provided, emits an `x07.patchset@0.1.0` instead of writing files.
  - Requires deterministic clause ids for contract-core clauses (use `x07 xtal spec fmt --inject-ids --write`).

### End-to-end wrapper

- `x07 xtal dev`
  - Runs spec fmt/lint/check.
  - If `arch/gen/index.x07gen.json` exists (or `--gen-index` is passed), runs `x07 gen verify`.
  - Otherwise, runs `x07 xtal tests gen-from-spec --check`.
  - Runs `x07 xtal impl check`.
  - Unless `--prechecks-only` is set, runs `x07 xtal verify`.
  - If verification fails and `--repair-on-fail` is set, runs `x07 xtal repair --write`.
- `x07 xtal verify`
  - Runs spec/gen/impl prechecks.
  - Requires a deterministic `solve-*` world by default (pass `--allow-os-world` to override).
  - Runs formal verification per spec operation entrypoint:
    - `x07 verify --coverage --entry <spec.operations[*].name>`
    - `x07 verify --prove --entry <spec.operations[*].name> --emit-proof <path>`
  - Runs `x07 test --all --no-fail-fast --manifest gen/xtal/tests.json --allow-empty`.
  - Writes:
    - `target/xtal/xtal.verify.diag.json` (wrapper diagnostics report, `x07diag.report@0.3.0`)
    - `target/xtal/verify/summary.json` (aggregate summary, `x07.xtal.verify_summary@0.1.0`; see `docs/spec/schemas/x07.xtal.verify_summary@0.1.0.schema.json`)
    - `target/xtal/tests.report.json` (test report)
    - `target/xtal/verify/_artifacts/` (nested artifacts for `x07 verify` and `x07 test`)
      - `target/xtal/verify/_artifacts/verify/...` (proof counterexamples, solver inputs, CBMC/Z3 intermediates)
      - `target/xtal/verify/_artifacts/test/...` (property-test repro cases and other test artifacts)
    - `target/xtal/verify/coverage/<module_path>/<local>.report.json` (per-entry coverage reports)
    - `target/xtal/verify/prove/<module_path>/<local>.report.json` (per-entry prove reports)
    - `target/xtal/verify/prove/<module_path>/<local>/proof.object.json` (proof objects, when emitted)
  - Proof outcomes are controlled by `--proof-policy {balanced|strict}` (default: `balanced`).
    - Under `balanced`, missing proof tools produce warnings (and verification continues).
    - Under `strict`, only `proven` outcomes pass.
  - When proofs are unsupported or inconclusive, the wrapper emits a compact per-entry reason summary in the XTAL diagnostics (for example: unsupported loop forms or unsupported branded input shapes).
  - Verification bounds can be overridden with `--unwind`, `--max-bytes-len`, and `--input-len-bytes`.
  - Under `--proof-policy balanced`, xtal verify uses conservative proof budgets so the default loop stays bounded.
    - Defaults: `--unwind 1`, `--max-bytes-len 8`, `--z3-timeout-seconds 1`.
  - The Z3 timeout for the proof lane can be overridden with `--z3-timeout-seconds` (otherwise x07's default timeout applies under `--proof-policy strict`).
  - The Z3 solver memory limit can be set with `--z3-memory-mb`.
  - Proof caching is automatic when a project manifest is available:
    - Successful `x07 verify --prove` runs cache proof summaries under `.x07/cache/verify/proof_summaries/`.
    - When proof objects are emitted, proof bundles are cached under `.x07/cache/verify/proofs/` and may be reused on subsequent prove runs (even without solver tools present).
  - Proof runs require external tooling; see `docs/toolchain/formal-verification.md`.
- `x07 xtal certify`
  - Requires `arch/xtal/xtal.json`.
  - Optionally runs `x07 xtal dev` prechecks (skip with `--no-prechecks`).
  - Uses `--spec-dir <dir>` (default: `spec`) to bind spec/example inputs into the certification bundle manifest.
  - Selects entrypoints from the XTAL manifest (pass `--entry` to pick one, or `--all` to certify all).
  - For each entrypoint, runs `x07 trust certify` with:
    - `--profile <trust.cert_profile>`
    - `--baseline <path>` (optional, enables review diff outputs)
    - `--review-fail-on <gate>` for each `trust.review_gates[]` entry
    - `--no-fail-fast` (optional; preserve full test signal after the first failure)
    - Proof budgets forwarded to `x07 verify --prove`: `--unwind`, `--max-bytes-len`, `--input-len-bytes`, `--z3-timeout-seconds`, `--z3-memory-mb`
  - Writes:
    - `target/xtal/xtal.certify.diag.json` (wrapper diagnostics report, `x07diag.report@0.3.0`)
    - `target/xtal/cert/summary.json` (`x07.xtal.certify_summary@0.1.0`; see `docs/spec/schemas/x07.xtal.certify_summary@0.1.0.schema.json`)
    - `target/xtal/cert/bundle.json` (`x07.xtal.cert_bundle@0.1.0`; digests for the certification outputs plus bound spec/example inputs)
    - `target/xtal/cert/<entry>/...` (certificate bundles and trust artifacts from `x07 trust certify`)
- `x07 xtal repair`
  - Reads `target/xtal/verify/summary.json` (baseline) and attempts a bounded repair for one failing entry.
  - Strategies:
    - semantic attempt for `prove.raw == "counterexample"` entries (enumerative single-expression synthesis for `i32` results, pruned by spec examples and validated by targeted `x07 test` then `x07 verify --prove`)
    - quickfix fallback via `x07 fix` when semantic repair is unavailable or fails
  - If `--write` is set, requires `arch/xtal/xtal.json`, enforces `autonomy.agent_write_paths[]` for patch targets, applies the final patchset to the working tree, and reruns `x07 xtal verify`.
  - By default, only edits known stub bodies (pass `--allow-edit-non-stubs` to override).
  - Flags: `--write`, `--max-rounds`, `--max-candidates`, `--semantic-max-depth`, `--semantic-ops`, `--entry`, `--stubs-only`, `--semantic-only`, `--quickfix-only`, `--suggest-spec-patch`.
  - Writes:
    - `target/xtal/xtal.repair.diag.json` (wrapper diagnostics report, `x07diag.report@0.3.0`)
    - `target/xtal/repair/summary.json` (`x07.xtal.repair_summary@0.1.0`; see `docs/spec/schemas/x07.xtal.repair_summary@0.1.0.schema.json`)
    - `target/xtal/repair/patchset.json` (`x07.patchset@0.1.0`)
    - `target/xtal/repair/diff.txt` (deterministic review diff)
    - `target/xtal/repair/attempts/attempt-0001/...` (per-attempt evaluation artifacts)

## Output conventions

- Generated test module id: `gen.xtal.<module_id>.tests`
- Generated unit test entrypoints: `gen.xtal.<module_id>.tests.ex_0001`, `...ex_0002`, …
- Generated property wrapper entrypoints: `gen.xtal.<module_id>.tests.prop_0001`, `...prop_0002`, …
- Manifest unit test ids: `xtal/<module_id>/<op_id>/ex0001`, `.../ex0002`, …
- Manifest property test ids: `xtal/<module_id>/<op_id>/prop0001`, `.../prop0002`, …
- XTAL reports record deterministic input digests in `meta.spec_digests` and `meta.examples_digests` (sha256 + bytes_len) for review/trust artifacts.

## Runtime violations

When a runtime contract violation is detected (for example via `x07 test` or `x07 run`), x07 can write a self-contained violation bundle under `target/xtal/violations/<id>/`.

By default, this is enabled when the project has `arch/xtal/xtal.json`. To override the output directory (or to enable it for non-XTAL projects), set `X07_XTAL_VIOLATIONS_DIR` (absolute, or relative to the project root).

Artifacts:

- `target/xtal/violations/<id>/violation.json` (`x07.xtal.violation@0.1.0`)
- `target/xtal/violations/<id>/repro.json` (`x07.contract.repro@0.1.0`)

OS-world incidents: when a contract violation is captured from `run-os` or `run-os-sandboxed`, `x07 run` rewrites the emitted repro to `world: "solve-rr"` and captures `.x07_rr/` into `.x07/artifacts/contract/<clause_id>/rr/` (referenced via `runner.fixture_rr_dir`).

See: `docs/worlds/record-replay.md`.

## Recovery events

When a runtime violation is recorded, x07 can also emit a structured recovery event stream (JSONL).

By default, this is enabled when the project has `arch/xtal/xtal.json`. To override the output directory (or to enable it for non-XTAL projects), set `X07_XTAL_EVENTS_DIR` (absolute, or relative to the project root).

Artifacts:

- `target/xtal/events/<id>/events.jsonl` (JSONL; each line is `x07.xtal.recovery_event@0.1.0`)

See: [Task policy graph](tasks.md).

## `x07 xtal ingest`

Normalize a violation (`x07.xtal.violation@0.1.0`) or a contract repro (`x07.contract.repro@0.1.0`) into a canonical ingest workspace (and, by default, run an improvement loop).

Ingest validates that `violation.json` matches `repro.json` (content-addressed id, repro sha256, and repro bytes length), and records the check results in the ingest summary.

By default, ingest also runs `x07 xtal improve` on the normalized workspace. To stop after normalization, pass `--normalize-only`.

Artifacts:

- `target/xtal/xtal.ingest.diag.json` (`x07diag.report@0.3.0`)
- `target/xtal/ingest/summary.json` (`x07.xtal.ingest_summary@0.1.0`)
- `target/xtal/ingest/<id>/violation.json` (`x07.xtal.violation@0.1.0`)
- `target/xtal/ingest/<id>/repro.json` (`x07.contract.repro@0.1.0`)
- `target/xtal/ingest/<id>/events.jsonl` (optional; copied when ingesting `events.jsonl`)

## `x07 xtal improve`

Turn incident inputs (violation bundles, contract repros, or recovery event logs) into a bounded improvement run:

- `x07 xtal improve --input <path>`
- Optional: `--reduce-repro`, `--write`, `--allow-spec-change`, `--certify`, `--run-tasks`, `--baseline <cert_dir>`, `--out-dir <dir>`

Artifacts:

- `target/xtal/xtal.improve.diag.json` (`x07diag.report@0.3.0`)
- `target/xtal/improve/summary.json` (`x07.xtal.improve_summary@0.1.0`)
- `target/xtal/improve/<id>/tests.shadow.json` (generated single-case tests manifest)
- `target/xtal/improve/<id>/repro.min.json` and `target/xtal/improve/<id>/reduction.report.json` (optional; when `--reduce-repro` is supported)

## `x07 xtal tasks run`

Execute recovery tasks from `arch/tasks/index.x07tasks.json` for an incident input:

- `x07 xtal tasks run --input target/xtal/violations/<id>/violation.json`

Artifacts:

- `target/xtal/xtal.tasks.diag.json` (`x07diag.report@0.3.0`)
- `target/xtal/tasks/<id>/...` (generated wrappers + per-attempt stderr captures)
- `target/xtal/events/<id>/events.jsonl` (optional; when recovery events are enabled)

Execution conventions:

- Each task must reference a `defn`/`defasync` symbol that returns `bytes`.
- Supported task parameter shapes:
  - 0 params
  - 1 param: `bytes_view` (receives the built-in `input`) or `bytes` (receives `view.to_bytes(input)`).
