# x07-math-native

This crate builds **`libx07_math.a`** (or the platform-equivalent) and exposes the
pinned C ABI declared in:

- `crates/x07c/include/x07_math_abi_v1.h`

It is intended to be linked into binaries emitted by `x07c` whenever the
external `x07-ext-math` package is used.

## Determinism notes

- Formatting uses the `ryu` algorithm (shortest, correctly-rounded) so output is stable.
- Math functions use the pure-Rust `libm` implementations.
- Parsing uses `lexical-core` (byte-based, locale-independent) and supports `nan` / `inf`.

## Building

```bash
cargo build -p x07-math-native --release
```

Then run:

```bash
scripts/build_ext_math.sh
```

which copies the header + staticlib into `deps/x07/` for toolchain linking.
