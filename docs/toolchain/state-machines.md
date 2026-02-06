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

- `gen/sm/.../<machine_tail>_v<version>.x07.json`
  - module id: `gen.sm.<machine_id>_v<version>` (dots in `machine_id` become directories)
- `gen/sm/.../<machine_tail>_v<version>/tests.x07.json`
  - module id: `gen.sm.<machine_id>_v<version>.tests`
- `tests.manifest.json`
  - schema: `x07.tests_manifest@0.1.0`

Example (`machine_id="app.minimal_fsm"`, `--out gen/sm`):

- `gen/sm/app/minimal_fsm_v1.x07.json`
- `gen/sm/app/minimal_fsm_v1/tests.x07.json`
- `gen/sm/tests.manifest.json`

Run the generated tests:

```bash
x07 test --manifest gen/sm/tests.manifest.json
```

## Generated API (v1)

The generated machine module exports:

- `init_v1() -> bytes` (initial snapshot)
- `step_v1(snapshot: bytes_view, event: bytes_view) -> result_bytes`

The step result is a deterministic doc in bytes form that includes the next snapshot (or a stable error code).

## Recipe: SM + arch contracts

This section shows the canonical project layout and commands when combining state machines with `x07 arch check`.

### Project layout

```
my-project/
├── arch/
│   ├── manifest.x07arch.json
│   ├── manifest.lock.json
│   └── sm/
│       ├── index.x07sm.json
│       └── specs/
│           └── order_fsm.sm.json
├── gen/
│   └── sm/
│       └── app/
│           ├── order_fsm_v1.x07.json
│           └── order_fsm_v1/
│               └── tests.x07.json
├── src/
│   └── app.x07.json
└── x07.json
```

### Canonical SM generation

Always run from the **project root** with repo-relative paths:

```bash
x07 sm gen --input arch/sm/specs/order_fsm.sm.json --out gen/sm --write
```

The SM generator embeds `meta.source_contract_path` in outputs — running from a different directory produces a different (wrong) embedded path, which causes drift checks to fail.

### Canonical arch check

```bash
x07 arch check --write-lock
```

### externals.allowed_import_prefixes

When using generated SM modules under `gen/`, the arch manifest must allow imports with the `gen.` prefix. In `arch/manifest.x07arch.json`:

```json
{
  "externals": {
    "allowed_import_prefixes": ["std.", "ext.", "gen."]
  }
}
```

Without `"gen."`, imports of `gen.sm.app.order_fsm_v1` produce `E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED`.

### gen_paths in the SM index

`gen_paths` in `arch/sm/index.x07sm.json` should list only the generated `*.x07.json` module files — not `tests.manifest.json` or test modules. The arch checker validates these paths for drift detection.

### Module scan and gen/

By default, `gen/` is in the arch checker's exclude list. To include generated SM modules in the module graph, either:

- Remove `gen/` from `module_scan.exclude_globs` in the lock file, or
- Add `"gen."` to `externals.allowed_import_prefixes` (treats them as external — simpler for most projects)
