# XTAL (intent → spec)

XTAL provides a spec-first surface for X07 projects:

- `*.x07spec.json` modules are the canonical intent/behavior interface.
- Optional `*.x07spec.examples.jsonl` example cases provide a minimum semantic oracle.
- A deterministic generator converts examples and declared properties into normal X07 tests under `gen/xtal/`.

Example project: `docs/examples/agent-gate/xtal/toy-sorter/`.

## Artifacts

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
- `x07 xtal verify`
  - Runs `dev` prechecks.
  - Runs formal verification per spec operation entrypoint:
    - `x07 verify --coverage --entry <spec.operations[*].name>`
    - `x07 verify --prove --entry <spec.operations[*].name> --emit-proof <path>`
  - Runs `x07 test --all --manifest gen/xtal/tests.json`.
  - Writes:
    - `target/xtal/xtal.verify.diag.json` (wrapper diagnostics report, `x07diag.report@0.3.0`)
    - `target/xtal/verify/summary.json` (aggregate summary, `x07.xtal.verify_summary@0.1.0`; see `docs/spec/schemas/x07.xtal.verify_summary@0.1.0.schema.json`)
    - `target/xtal/tests.report.json` (test report)
    - `target/xtal/verify/coverage/<module_path>/<local>.report.json` (per-entry coverage reports)
    - `target/xtal/verify/prove/<module_path>/<local>.report.json` (per-entry prove reports)
    - `target/xtal/verify/prove/<module_path>/<local>.proof.json` (proof objects, when emitted)
  - Proof outcomes are controlled by `--proof-policy {balanced|strict}` (default: `balanced`).
  - Verification bounds can be overridden with `--unwind`, `--max-bytes-len`, and `--input-len-bytes`.
  - Proof runs require external tooling; see `docs/toolchain/formal-verification.md`.

## Output conventions

- Generated test module id: `gen.xtal.<module_id>.tests`
- Generated unit test entrypoints: `gen.xtal.<module_id>.tests.ex_0001`, `...ex_0002`, …
- Generated property wrapper entrypoints: `gen.xtal.<module_id>.tests.prop_0001`, `...prop_0002`, …
- Manifest unit test ids: `xtal/<module_id>/<op_id>/ex0001`, `.../ex0002`, …
- Manifest property test ids: `xtal/<module_id>/<op_id>/prop0001`, `.../prop0002`, …
- XTAL reports record deterministic input digests in `meta.spec_digests` and `meta.examples_digests` (sha256 + bytes_len) for review/trust artifacts.
