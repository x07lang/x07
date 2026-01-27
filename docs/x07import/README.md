# x07import

`x07import` is a deterministic importer that translates a restricted subset of Rust/C into X07 module files (`.x07.json`, x07AST JSON).

For an end-to-end tutorial that produces a publishable package, see: [Porting by example](porting-by-example.md).

Memory model note (ABI v2):
- Prefer `bytes_view` for read-only byte inputs in imported signatures (borrowed, zero-copy), and return owned `bytes` (drop-clean).
- Finalize `vec_u8` builders with `std.vec.as_bytes` (wraps `vec_u8.into_bytes`).

## Commands

- Import a single file:
  - Rust: `x07import-cli rust --in <src.rs> --module-id <module.id> --out modules/`
  - C: `x07import-cli c --in <src.c> --module-id <module.id> --out modules/`
- Batch drift check (no writes): `x07import-cli batch --manifest <manifest.json> --check`

## Diagnostics

See: [Diagnostics catalog](diagnostics.md).

## Frontends

- Rust: `syn::parse_file` subset (see `crates/x07import-core/src/rust/mod.rs`).
- C: parsed via `clang -Xclang -ast-dump=json` subset (see `crates/x07import-core/src/c/`).
  - Supported sources must use `static inline` functions and avoid preprocessor directives beyond `#include <stdint.h>` / `<stddef.h>` / `<stdbool.h>`.
  - Requires `clang` in `PATH`.
