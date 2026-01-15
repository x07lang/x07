# ext.regex native backend replacement (no fallback)

This phase replaces the legacy pure X07 regex engine (`ext-regex@0.1.0`) with a native backend, while keeping the v1 API/bytes contracts pinned in `docs/text/regex-v1.md`.

## Versioning

- `packages/ext/x07-ext-regex/0.1.0/` is immutable legacy (pure X07 implementation).
- `packages/ext/x07-ext-regex/0.2.0/` is the replacement (thin wrapper + native backend).

## Architecture

### 1) Thin X07 wrapper module

- Implementation: `packages/ext/x07-ext-regex/0.2.0/modules/ext/regex.x07.json`
- Exports the `ext.regex.*` surface from `docs/text/regex-v1.md`.
- Calls compiler builtins for all “heavy” ops:
  - `regex.compile_opts_v1`
  - `regex.exec_from_v1`
  - `regex.exec_caps_from_v1`
  - `regex.find_all_x7sl_v1`
  - `regex.split_v1`
  - `regex.replace_all_v1`

### 2) Builtins in the C backend (`x07c`)

- Codegen: `crates/x07c/src/c_emit.rs`
- ABI header: `crates/x07c/include/x07_ext_regex_abi_v1.h`

These builtins emit calls to `x07_ext_regex_*` C symbols implemented by the native static library.

### 3) Native static library

- Rust staticlib crate: `crates/x07-ext-regex-native/`
- Exported symbols: `x07_ext_regex_*` (see `crates/x07c/include/x07_ext_regex_abi_v1.h`)
- Output buffers are allocated via the X07 runtime callback `ev_bytes_alloc` (no foreign frees).

### 4) Staging (repo-local “deps”)

To compile programs that use `ext.regex`, you must stage the library + header into `deps/x07/`:

- Build + stage: `./scripts/build_ext_regex.sh`
- Outputs:
  - `deps/x07/libx07_ext_regex.a` (or `deps/x07/x07_ext_regex.lib` on Windows)
  - `deps/x07/include/x07_ext_regex_abi_v1.h`

### 5) No-fallback enforcement

- `crates/x07-host-runner/src/lib.rs` refuses to compile generated C if it references `x07_ext_regex_*` but the staged archive is missing.
- The error message instructs running `./scripts/build_ext_regex.sh`.

## Match semantics (v1): leftmost-longest

`docs/text/regex-v1.md` pins v1 search semantics as **leftmost-longest**.

Rust’s high-level `regex` crate is leftmost-first, so the native backend uses `regex-automata` meta regexes to preserve leftmost-longest deterministically:

1. Search with `MatchKind::LeftmostFirst` to find the earliest start offset `s`.
2. Search with `MatchKind::All`, anchored at `s`, to pick the longest match starting at `s`.

## CI / smoke

- CI runner: `scripts/ci/check_regex_smoke.sh`
- Smoke suite: `benchmarks/smoke/regex-smoke.json`
- Smoke program: `tests/external_pure/regex_smoke/src/main.x07.json`

## Running package tests

```bash
./scripts/build_ext_regex.sh
cargo run -p x07 -- test \
  --manifest packages/ext/x07-ext-regex/0.2.0/tests/tests.json \
  --module-root packages/ext/x07-ext-regex/0.2.0/modules
```
