# State machines (`x07 sm`)

`x07 sm` validates and generates X07 modules from a pinned state machine spec.

This is the “state machines as data” workflow: you edit a declarative spec, and the toolchain generates a deterministic `step(state,event)` function plus tests.

## Commands

- Validate a spec (machine-readable report by default):
  - `x07 sm check --input path/to/machine.json`
- Generate outputs:
  - `x07 sm gen --input path/to/machine.json --out gen/sm --write`
- Drift check (CI-friendly):
  - `x07 sm gen --input path/to/machine.json --out gen/sm --check`

Exit codes:

- `0`: ok
- `2`: validation failed / drift detected

## Input spec

Schema: `spec/x07-sm.spec.schema.json` (`schema_version: "x07.sm.spec@0.1.0"`).

Key fields:

- `machine_id`: stable id (`"app.order_fsm"`)
- `version`: integer (`1` → `_v1`)
- `world`: one of X07’s worlds (generated tests run in this world)
- `states[]`, `events[]`, `transitions[]`
- optional:
  - `brand`: attach a bytes brand to snapshots/step outputs
  - `context`: codec + size cap for additional state
  - `budgets`: step-level caps (v1 is intentionally small)

Example spec (copy/paste):

- `docs/examples/contracts_project/arch/sm/specs/minimal.sm.json`

## Outputs

`x07 sm gen` writes three files under `--out`:

- `<machine_id>_v<version>.x07.json`
  - module id: `gen.sm.<machine_id>_v<version>`
- `<machine_id>_v<version>.tests.x07.json`
  - module id: `gen.sm.<machine_id>_v<version>.tests`
- `tests.manifest.json`
  - schema: `x07.tests_manifest@0.1.0`

Run the generated tests:

```bash
x07 test --manifest gen/sm/tests.manifest.json
```

## Generated API (v1)

The generated machine module exports:

- `init_v1() -> bytes` (initial snapshot)
- `step_v1(snapshot: bytes_view, event: bytes_view) -> result_bytes`

The step result is a deterministic doc in bytes form that includes the next snapshot (or a stable error code).
