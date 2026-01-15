# x07-time-native

This crate builds **`libx07_time.a`** (or the platform-equivalent) and exposes a small C ABI
used by the X07 C backend for deterministic tzdb offset lookup.

It is staged into `deps/x07/` by `./scripts/build_ext_time.sh`.
