# Native ext-regex backend (libx07_ext_regex)

This crate builds `libx07_ext_regex.a` (or the platform-equivalent) and exposes a
small C ABI used by `x07c`-generated code to implement `ext.regex` with a native backend.

Build + stage into `deps/`:

- `./scripts/build_ext_regex.sh`

