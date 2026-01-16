# x07import (Phase H0)

`x07import` is a deterministic importer that translates a restricted subset of Rust/C into X07 module files (`.x07.json`, x07AST JSON).

Memory model note (ABI v2):
- Prefer `bytes_view` for read-only byte inputs in imported signatures (borrowed, zero-copy), and return owned `bytes` (drop-clean).
- Finalize `vec_u8` builders with `std.vec.as_bytes` (wraps `vec_u8.into_bytes`).

## Repo layout

- Import sources (dev-only fixtures): `tests/x07import/fixtures/import_sources/`
  - Manifest: `tests/x07import/fixtures/import_sources/manifest.json`
  - Rust refs: `tests/x07import/fixtures/import_sources/rust/**`
  - C refs: `tests/x07import/fixtures/import_sources/c/**`
- Generated modules (checked in): `stdlib/std/0.1.1/modules/**` (only for modules listed in the manifest)
  - Current manifest modules: `std.text.ascii`, `std.text.utf8`, `std.regex-lite`, `std.codec`, `std.fmt`

If `tests/x07import/fixtures/import_sources/` is moved out of this repo (for example, into a separate repo or a submodule), CI supports an override:
- `X07IMPORT_MANIFEST=/abs/path/to/manifest.json ./scripts/ci/check_x07import_generated.sh`

## Commands

- Regenerate all manifest entries:
  - `cargo run -p x07import-cli -- batch --manifest tests/x07import/fixtures/import_sources/manifest.json`
- Drift check (no writes):
  - `cargo run -p x07import-cli -- batch --manifest tests/x07import/fixtures/import_sources/manifest.json --check`
  - `./scripts/ci/check_x07import_generated.sh`
- Diagnostics catalog:
  - `cargo run -p x07import-core --bin gen_diagnostics_md > docs/x07import/diagnostics.md`
  - `./scripts/ci/check_x07import_diagnostics_sync.sh`
- Stdlib lockfiles:
  - `./scripts/ci/check_stdlib_lock.sh`

## Frontends

- Rust: `syn::parse_file` subset (see `crates/x07import-core/src/rust/mod.rs`).
- C: parsed via `clang -Xclang -ast-dump=json` subset (see `crates/x07import-core/src/c/`).
  - Supported sources must use `static inline` functions and avoid preprocessor directives beyond `#include <stdint.h>` / `<stddef.h>` / `<stdbool.h>`.
  - Requires `clang` in `PATH`.
