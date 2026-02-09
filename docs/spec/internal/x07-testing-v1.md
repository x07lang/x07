Spec-ID: x07.spec.internal.x07-testing-v1@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# X07 Testing v1 (Deterministic harness)

X07’s built-in test harness is a toolchain feature designed for agentic coding loops: deterministic discovery, deterministic execution, and a strict machine-readable report.

The harness lives in the `x07` CLI (`crates/x07`) and is intentionally not “language semantics”.

## Concepts

### Tests manifest (`tests.json`)

Tests are declared in a JSON manifest (default: `tests/tests.json`):

```json
{
  "schema_version": "x07.tests_manifest@0.2.0",
  "tests": [
    {"id":"smoke/pure_i32_eq", "world":"solve-pure", "entry":"smoke_pure.pure_i32_eq"},
    {"id":"smoke/fs_read_hello", "world":"solve-fs", "entry":"smoke_fs.fs_read_hello", "fixture_root":"fixtures/fs_smoke/root"},
    {"id":"smoke/os_smoke", "world":"run-os", "entry":"smoke_os.check"}
  ]
}
```

Each test entry:

- `id` (string, REQUIRED): stable identifier (ASCII printable, unique)
- `world` (string, REQUIRED): one of:
  - deterministic fixture worlds: `solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, `solve-full`
  - OS worlds: `run-os`, `run-os-sandboxed`
- `entry` (string, REQUIRED): fully-qualified function name (example: `smoke_pure.pure_i32_eq`)
- `expect` (string, OPTIONAL): `pass` (default), `fail` (XFAIL), `skip`
- `fixture_root` (string, OPTIONAL): required when `world == "solve-fs"`; relative to the manifest directory
- `returns` (string, OPTIONAL): `result_i32` (default) or `bytes_status_v1`
- `timeout_ms` (int, OPTIONAL): rounded up to seconds for the runner wall/CPU gate
- `input_b64` / `input_path` (OPTIONAL, `x07.tests_manifest@0.2.0` only): raw input bytes for deterministic `solve-*` worlds
- `pbt` (OPTIONAL): property-based testing config (runs only with `x07 test --pbt` / `--all`)

### Test entry return contract

Preferred entry signature:

- `(defn <entry> () result_i32 ...)`
  - `Ok(_)` => PASS
  - `Err(code)` => FAIL with numeric `code`

Alternate entry signature:

- `(defn <entry> () bytes ...)` returning `X7TEST_STATUS_V1` (see below)
  - select this mode with `returns: "bytes_status_v1"`

### `X7TEST_STATUS_V1` (5 bytes)

- Byte 0: `tag` (`1` = pass, `0` = fail, `2` = skip)
- Bytes 1..4: `u32_le(code)` (0 for pass/skip)

The harness uses `std.test.status_from_result_i32` to convert `result_i32` tests into this record.

## CLI

Run tests:

- `cargo run -p x07 -- test`
- or `target/debug/x07 test`

Key flags:

- `--manifest PATH` (default `tests/tests.json`)
- `--module-root DIR` (default: manifest directory)
- `--filter SUBSTR` / `--exact`
- `--pbt` (run property-based tests only)
- `--all` (run unit tests + property-based tests)
- `--pbt-repro PATH` (replay one counterexample; requires `--pbt`)
- `--list`
- `--repeat N` (determinism gate: outputs + metrics must match)
- `--jobs N` (requires `--no-fail-fast`)
- `--no-run` (compile wrappers only)
- `--keep-artifacts --artifact-dir DIR` (stores generated drivers and executables)
- `--report-out PATH` (writes report file; still emits JSON to stdout)

Exit codes:

- `0`: all tests matched expectations
- `10`: unexpected test result(s)
- `11`: compile failure(s)
- `12`: infrastructure failure(s) (runner/manifest I/O/fixture errors)

## Reporting (`x07test`)

`x07 test` emits a JSON report with stable ordering by `id`.

Schema: `spec/x07test.schema.json` (`schema_version = x07.x07test@0.3.0`).

Contract violations:

- If a test fails due to a runtime contract violation, the per-test result includes:
  - `failure_kind: "contract_violation"`
  - `contract_repro_path` (path to an `x07.contract.repro@0.1.0` JSON artifact)

## Stdlib: `std.test`

The harness expects `std.test` to exist in the embedded stdlib (`stdlib/std/0.1.1/`), providing:

- Assertions returning `result_i32`: `std.test.assert_true`, `std.test.assert_i32_eq`, `std.test.assert_bytes_eq`, `std.test.assert_view_eq`
- Stable numeric codes as functions:
  - `std.test.code_assert_true` = 1001
  - `std.test.code_assert_i32_eq` = 1002
  - `std.test.code_assert_bytes_eq` = 1003
  - `std.test.code_assert_view_eq` = 1004
- Status encoder: `std.test.status_from_result_i32` -> `X7TEST_STATUS_V1` bytes

## Repo smoke suite

The repo includes a minimal end-to-end smoke suite:

- Manifest: `tests/tests.json`
- Pure tests module: `tests/smoke_pure.x07.json`
- Fixture-FS test module: `tests/smoke_fs.x07.json`
- Fixture root: `tests/fixtures/fs_smoke/root/hello.txt`
