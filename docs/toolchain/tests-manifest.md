# Test manifest (`x07.tests_manifest@0.2.0`)

`x07 test` is driven by a JSON manifest, usually at `tests/tests.json`.

Supported schema versions:

- `x07.tests_manifest@0.1.0`
- `x07.tests_manifest@0.2.0` (adds `input_b64` / `input_path`)

## Top-level shape

```json
{
  "schema_version": "x07.tests_manifest@0.2.0",
  "tests": []
}
```

## Test entries (`tests[]`)

Each entry in `tests[]` is an object with:

- `id` (required): ASCII-printable string, unique across the manifest.
- `world` (required): one of:
  - deterministic fixture worlds: `solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, `solve-full`
  - OS worlds: `run-os`, `run-os-sandboxed`
- `entry` (required): `module.symbol` (must contain `.`).
- `expect` (optional): `pass`, `fail`, or `skip` (default: `pass`).
- `returns` (optional): `result_i32` or `bytes_status_v1` (default: `result_i32`).
- `timeout_ms` (optional): integer `>= 1` (converted to a per-test CPU time limit, ceiling to seconds).
- `fixture_root` (optional/required depending on world):
  - forbidden for `solve-pure` and OS worlds
  - required for `solve-fs`, `solve-rr`, `solve-kv`
  - required for `solve-full` (must contain `fs/`, `rr/`, and `kv/` subdirectories)
- `policy_json` (optional/required depending on world):
  - required for `run-os-sandboxed`
  - forbidden for all other worlds

## Property-based tests (`pbt`)

If a test entry includes a `pbt` object, it is a property-based test (PBT).

Selection:

- `x07 test` runs unit tests only (entries where `pbt` is not set).
- `x07 test --pbt` runs PBT tests only.
- `x07 test --all` runs both.

Constraints:

- PBT tests are only supported for deterministic `solve-*` worlds.
- PBT tests must use `returns: "bytes_status_v1"`.
- PBT tests must not set `input_b64` / `input_path` (inputs are generated).

See: [Property-based testing](pbt.md).

## Test inputs (v0.2.0)

In `x07.tests_manifest@0.2.0`, each test entry may optionally specify raw input bytes (passed to the solver as the built-in `input` `bytes_view`).

Exactly one of:

- `input_b64` (optional): base64-encoded raw input bytes
- `input_path` (optional): path to a file to read as raw input bytes (resolved relative to the manifest directory)

Inputs are only supported for deterministic `solve-*` worlds.

For integration-style checks that need filesystem/network access, prefer running `x07 run` (or a bundled executable via `x07 bundle`) in the sandbox profile and asserting on the runner report.

## Module roots

`x07 test` resolves modules using `--module-root` directories.

Defaults:

- if a project `x07.json` exists (searched upwards from the manifest directory), it uses:
  - the project module roots
  - dependency module roots from `x07.lock.json`
  - the project root (directory containing `x07.json`) to support generated modules (for example under `gen/`)
  - the manifest directory (as a fallback)
- otherwise: it uses the manifest directory

## Diagnostics

Manifest validation errors surface as `ETEST_*` diagnostics in the `x07test` report.

See: [Diagnostic codes](diagnostic-codes.md).
