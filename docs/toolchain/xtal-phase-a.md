# XTAL Phase A (intent → spec)

XTAL Phase A adds a spec-first surface for X07 projects:

- `*.x07spec.json` modules are the canonical intent/behavior interface.
- Optional `*.x07spec.examples.jsonl` example cases provide a minimum semantic oracle.
- A deterministic generator converts examples into normal X07 unit tests under `gen/xtal/`.

Example project: `docs/examples/agent-gate/xtal-phase-a/toy-sorter/`.

## Artifacts

### Spec modules (`x07.x07spec@0.1.0`)

- Location: `spec/<module_id>.x07spec.json` (recommended)
- Schema: `x07.x07spec@0.1.0` (see `docs/spec/schemas/x07.x07spec@0.1.0.schema.json`)

### Example cases (`x07.x07spec_examples@0.1.0`, JSONL)

- Location: `spec/<module_id>.x07spec.examples.jsonl` (optional)
- Schema: `x07.x07spec_examples@0.1.0` (see `docs/spec/schemas/x07.x07spec_examples@0.1.0.schema.json`)
- One JSON object per line.

Phase A minimum value encodings:

- `bytes` / `bytes_view`: `{"kind":"bytes_b64","b64":"..."}`
- `i32`: a JSON integer (and the object form `{"kind":"i32","i32":123}` / `{"kind":"i32","value":123}` is accepted)

## Commands

### Authoring

- `x07 xtal spec scaffold --module-id <id> --op <local_name> --param <name:ty> --result <ty> [--examples] [--out-path <path>]`
- `x07 xtal spec fmt --input <spec.x07spec.json> --write [--inject-ids]`

### Validation

- `x07 xtal spec lint --input <spec.x07spec.json>`
- `x07 xtal spec check --project x07.json --input <spec.x07spec.json>`
  - Validates op ids, signatures, contract clause type/purity checks, and example cases.
- `x07 xtal dev --phase A`
  - Runs the Phase A spec pipeline (fmt/lint/check).

### Tests generation (examples → unit tests)

- `x07 xtal tests gen-from-spec --project x07.json --write`
  - Writes:
    - `gen/xtal/tests.json` (`x07.tests_manifest@0.2.0`)
    - `gen/xtal/<module_path>/tests.x07.json` (`module_id: gen.xtal.<module_id>.tests`)
- `x07 xtal tests gen-from-spec --project x07.json --check`
  - Fails if any generated output would change (drift check).

### End-to-end wrapper

- `x07 xtal verify`
  - Runs spec checks and generation drift check.
  - Executes `x07 test --manifest gen/xtal/tests.json`.
  - Writes `target/xtal/tests.report.json` (test report).

## Output conventions

- Generated test module id: `gen.xtal.<module_id>.tests`
- Generated test entrypoints: `gen.xtal.<module_id>.tests.ex_0001`, `...ex_0002`, …
- Manifest test ids: `xtal/<module_id>/<op_id>/ex0001`, `.../ex0002`, …
- XTAL reports record deterministic input digests in `meta.spec_digests` and `meta.examples_digests` (sha256 + bytes_len) for review/trust artifacts.
