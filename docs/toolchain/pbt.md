# Property-based testing (`x07 test --pbt`)

Property-based tests (PBT) run a property function against many generated inputs, stop at the first failure, and then deterministically shrink the counterexample.

In X07, PBT is designed for agent loops:

- deterministic seeds
- budgeted per-case execution
- structured failure artifacts + replay (`x07.pbt.repro@0.1.0`)

## Enabling PBT in the tests manifest

A test entry becomes a PBT test when it has a `pbt` object.
PBT tests are only supported for deterministic `solve-*` worlds.

Example entry:

```jsonc
{
  "id": "prop/bytes_roundtrip",
  "world": "solve-pure",
  "entry": "app.prop_bytes_roundtrip",
  "expect": "pass",
  "returns": "bytes_status_v1",
  "pbt": {
    "cases": 200,
    "max_shrinks": 4096,
    "params": [
      { "name": "b", "gen": { "kind": "bytes", "max_len": 1024 } }
    ],
    "case_budget": {
      "fuel": 200000,
      "timeout_ms": 250,
      "max_mem_bytes": 67108864,
      "max_output_bytes": 1048576
    }
  }
}
```

### Property function signature

For a PBT test, `entry` must point at a function whose parameters match `pbt.params` order and types.

Supported parameter types (v0.1):

- `i32`
- `bytes`

Return type must be `bytes` encoding `std.test.status_v1` (5 bytes).
In the manifest, use `returns: "bytes_status_v1"`.

### Generators (`pbt.params[].gen`)

Supported generator kinds (v0.1):

- `i32`
  - optional `min` / `max`
- `bytes`
  - `max_len` required

### Per-case budgets (`pbt.case_budget`)

Each generated case run is executed with caps:

- `fuel`
- `timeout_ms`
- `max_mem_bytes`
- `max_output_bytes`

These are tool-level (runner) limits, so traps/timeouts/fuel exhaustion are treated as first-class failures.

### Optional in-language budget scope (`pbt.budget_scope`)

If `pbt.budget_scope` is set, `x07 test --pbt` wraps the property call in `budget.scope_v1` (mode `trap_v1`) with the configured caps.

Example:

```jsonc
"pbt": {
  "params": [
    { "name": "b", "gen": { "kind": "bytes", "max_len": 64 } }
  ],
  "budget_scope": {
    "alloc_bytes": 65536,
    "memcpy_bytes": 1048576
  }
}
```

Supported caps (v0.1): `alloc_bytes`, `alloc_calls`, `realloc_calls`, `memcpy_bytes`, `sched_ticks`.

## CLI usage

- `x07 test` runs unit tests only (tests where `pbt` is not set).
- `x07 test --pbt` runs PBT tests only.
- `x07 test --all` runs both.

Common flags:

- `--pbt-seed <u64>` (default: `0`)
- `--pbt-cases <n>`
- `--pbt-max-shrinks <n>`
- per-case caps:
  - `--pbt-case-fuel <fuel>`
  - `--pbt-case-timeout-ms <ms>` (rounded up to seconds)
  - `--pbt-case-mem-bytes <bytes>`
  - `--pbt-case-output-bytes <bytes>`

### Replay a counterexample (`--pbt-repro`)

`x07 test --pbt --pbt-repro <path/to/repro.json>` runs exactly one test + one case from a repro artifact.

## Failure artifacts (`x07.pbt.repro@0.1.0`)

On failure, `x07 test --pbt` writes a repro artifact:

- `<artifact_dir>/pbt/id_<sha256(test.id)>/repro.json`

It also attaches a `X07T_EPBT_FAIL` diagnostic to the failing test case, with `diag.details` set to the full repro object.

Schema files:

- `spec/x07.pbt.repro@0.1.0.schema.json`
- `spec/x07.pbt.params@0.1.0.schema.json`

Next: [Convert a repro into a regression test](pbt-fix-from-repro.md).
