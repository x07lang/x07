# Test manifest (`x07.tests_manifest@0.1.0`)

`x07 test` is driven by a JSON manifest, usually at `tests/tests.json`.

## Top-level shape

```json
{
  "schema_version": "x07.tests_manifest@0.1.0",
  "tests": []
}
```

## Test entries (`tests[]`)

Each entry in `tests[]` is an object with:

- `id` (required): ASCII-printable string, unique across the manifest.
- `world` (required): `run-os` or `run-os-sandboxed`.
- `entry` (required): `module.symbol` (must contain `.`).
- `expect` (optional): `pass`, `fail`, or `skip` (default: `pass`).
- `returns` (optional): `result_i32` or `bytes_status_v1` (default: `result_i32`).
- `timeout_ms` (optional): integer `>= 1` (converted to a per-test CPU time limit, ceiling to seconds).
- `fixture_root` (optional): must not be set for OS worlds (rejected by `x07 test`).

For integration-style checks that need filesystem/network access, prefer running `x07 run` (or a bundled executable via `x07 bundle`) in the sandbox profile and asserting on the runner report.

## Diagnostics

Manifest validation errors surface as `ETEST_*` diagnostics in the `x07test` report.

See: [Diagnostic codes](diagnostic-codes.md).
