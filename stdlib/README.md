# Stdlib packages

This directory contains versioned, composable stdlib packages used by projects and by benchmark suites.

Built-in package shipped with the compiler:

- `stdlib/std-core/0.1.3/` (modules under `modules/`)
  - foundational, pure modules that define stable primitives (`std.bytes`, `std.view`, `std.codec`, `std.vec`, `std.json`, `std.small_set`, `std.small_map`, `std.doc`, ...)
- `stdlib/std/0.1.3/` (modules under `modules/`)
  - extended modules (text helpers, tests, and world-scoped I/O such as `std.io`, `std.fs`, `std.rr`, `std.kv`)
  - depends on `std-core` via `meta.requires_packages`

See `docs/packages/index.md` for package and workspace basics.

## Stdlib import sources

Some modules are generated deterministically by `x07import` from reference sources in `labs/x07import/fixtures/import_sources/`:

- Manifest: `labs/x07import/fixtures/import_sources/manifest.json`
- Drift check: `./scripts/ci/check_x07import_generated.sh`
- Diagnostics sync: `./scripts/ci/check_x07import_diagnostics_sync.sh`
