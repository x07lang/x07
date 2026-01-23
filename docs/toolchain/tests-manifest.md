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
- `world` (required): `solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, or `solve-full`.
- `entry` (required): `module.symbol` (must contain `.`).
- `expect` (optional): `pass`, `fail`, or `skip` (default: `pass`).
- `returns` (optional): `result_i32` or `bytes_status_v1` (default: `result_i32`).
- `timeout_ms` (optional): integer `>= 1` (converted to a per-test CPU time limit, ceiling to seconds).
- `fixture_root` (conditional):
  - `solve-pure`: must not be set.
  - `solve-fs` / `solve-rr` / `solve-kv`: required; must be an existing directory.
  - `solve-full`: required; must be an existing directory containing `fs/`, `rr/`, and `kv/` subdirectories.

## Path resolution and safety

`fixture_root` is resolved relative to the directory containing the manifest file (for example, relative to `tests/` when the manifest path is `tests/tests.json`).

The toolchain rejects unsafe fixture paths:

- absolute paths
- `..` components
- Windows `\` separators

## Conventional fixture layouts

`x07 test` uses conventional filenames inside each `fixture_root`:

- `solve-fs`: if `root/` exists, it is used as the filesystem root; if `latency.json` exists, it is used for latency modeling.
- `solve-rr`: if `index.json` exists, it is used as the RR fixture index.
- `solve-kv`: if `seed.json` exists, it is used as the KV seed source.
- `solve-full`: the same conventions under `fs/`, `rr/`, and `kv/`.

See: [Fixture formats](../worlds/fixture-formats.md).

## Diagnostics

Manifest validation errors surface as `ETEST_*` diagnostics in the `x07test` report.

See: [Diagnostic codes](diagnostic-codes.md).

