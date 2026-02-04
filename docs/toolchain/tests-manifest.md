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
