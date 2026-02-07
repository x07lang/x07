# Stdlib packages

This directory contains versioned, composable stdlib packages used by projects and by benchmark suites.

Built-in package shipped with the compiler:

- `stdlib/std/0.1.2/` (modules under `modules/`)
  - includes pure helpers (`std.vec`, `std.slice`, `std.bytes`, `std.codec`, `std.parse`, `std.fmt`, `std.prng`, `std.text.ascii`, `std.text.utf8`, `std.regex-lite`, `std.json`, `std.csv`, `std.map`, `std.set`, `std.result`, `std.option`, `std.path`) and world-scoped I/O (`std.io`, `std.io.bufread`, `std.fs`, `std.rr`, `std.kv`)

See `docs/packages/index.md` for package and workspace basics.

## Stdlib import sources

Some modules are generated deterministically by `x07import` from reference sources in `labs/x07import/fixtures/import_sources/`:

- Manifest: `labs/x07import/fixtures/import_sources/manifest.json`
- Drift check: `./scripts/ci/check_x07import_generated.sh`
- Diagnostics sync: `./scripts/ci/check_x07import_diagnostics_sync.sh`
