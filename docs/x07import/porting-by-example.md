# Porting Rust/C code into X07 packages with `x07import` (by example)

This tutorial shows how to translate small, deterministic Rust/C sources into **pure X07 modules** using `x07import-cli`, then package them for publishing.

Key idea:

- `x07import` is for a **restricted subset** of Rust/C that can be translated into x07AST JSON.
- The result is a normal X07 package: `x07-package.json` + `modules/*.x07.json`.
- If you need native bindings (libcurl/sqlite/openssl), that’s a different pattern: OS-world-only FFI packages. See [OS worlds](../worlds/os-worlds.md) and existing packages like `ext-curl-c`.

## Prerequisites

- Toolchain installed: `x07 --help`
- Importer installed: `x07import-cli --help`

For C import:

- `clang` in `PATH` (the importer uses libclang)

Companion examples (in the `x07` repo):

- `examples/tutorials/x07import_rust_hex_pkg/`
- `examples/tutorials/x07import_c_smoke_pkg/`

## Canonical layout for import-based packages

Recommended repo structure (generated modules are committed):

```text
my-package/
  x07-package.json
  x07.json                 # local dev + tests (recommended)
  import_sources/
    rust/
      lib.rs
    c/
      lib.c
  modules/
    ext/
      mypkg.x07.json       # generated (commit)
      mypkg/
        tests.x07.json     # handwritten (commit)
  tests/
    tests.json             # CI manifest (commit)
```

## Example A: Rust → x07 (hex)

This example imports a small, deterministic hex encoder/decoder.

### A1. Create the skeleton

```bash
mkdir ext-hex-rs-demo
cd ext-hex-rs-demo
x07 init --package
mkdir -p import_sources/rust

# This tutorial uses `ext.hex` / `ext.hex.tests` as the module IDs.
# Replace the default generated module files from `x07 init --package`.
rm -rf modules
mkdir -p modules/ext/hex
```

### A2. Add a restricted Rust source file

Create `import_sources/rust/hex.rs`:

```rust
fn _hex_digit(n: i32) -> i32 {
    if lt_u(n, 10) {
        return 48 + n;
    }
    87 + n
}

fn _from_hex_digit(c: i32) -> i32 {
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return c - 48;
        }
    }
    if ge_u(c, 65) {
        if lt_u(c, 71) {
            return (c - 65) + 10;
        }
    }
    if ge_u(c, 97) {
        if lt_u(c, 103) {
            return (c - 97) + 10;
        }
    }
    0 - 1
}

fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

pub fn hex_encode(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(n * 2);
    for i in 0..n {
        let c = view_get_u8(b, i);
        out = vec_u8_push(out, _hex_digit(c / 16));
        out = vec_u8_push(out, _hex_digit(c % 16));
    }
    vec_u8_into_bytes(out)
}

pub fn hex_decode(s: BytesView) -> Bytes {
    let n = view_len(s);
    if n % 2 != 0 {
        return _make_err(1);
    }

    let mut out = vec_u8_with_capacity(1 + (n / 2));
    out = vec_u8_push(out, 1);

    let mut i = 0;
    for _ in 0..n {
        if lt_u(i, n) {
            let hi = _from_hex_digit(view_get_u8(s, i));
            let lo = _from_hex_digit(view_get_u8(s, i + 1));
            if hi < 0 || lo < 0 {
                return _make_err(2);
            }
            out = vec_u8_push(out, (hi << 4) + lo);
            i = i + 2;
        }
    }

    vec_u8_into_bytes(out)
}

pub fn hex_is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn hex_err_code(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn hex_get_bytes(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 1 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 1, n - 1))
}
```

### A3. Run the importer

```bash
x07import-cli rust \
  --in import_sources/rust/hex.rs \
  --module-id ext.hex \
  --out modules
```

Expected output:

- `modules/ext/hex.x07.json`

### A4. Add a handwritten tests module

Create `modules/ext/hex/tests.x07.json`:

```json
{
  "schema_version": "x07.x07ast@0.1.0",
  "kind": "module",
  "module_id": "ext.hex.tests",
  "imports": ["ext.hex", "std.test"],
  "decls": [
    { "kind": "export", "names": ["ext.hex.tests.test_encode_decode_roundtrip"] },
    {
      "kind": "defn",
      "name": "ext.hex.tests.test_encode_decode_roundtrip",
      "params": [],
      "result": "result_i32",
      "body": [
        "begin",
        ["let", "src_bytes", ["bytes.lit", "hello"]],
        ["let", "hex_bytes", ["ext.hex.hex_encode", ["bytes.view", "src_bytes"]]],
        ["let", "expected_hex", ["bytes.lit", "68656c6c6f"]],
        ["try", ["std.test.assert_bytes_eq", "hex_bytes", "expected_hex", ["std.test.code_assert_bytes_eq"]]],
        ["let", "doc", ["ext.hex.hex_decode", ["bytes.view", "hex_bytes"]]],
        ["let", "doc_view", ["bytes.view", "doc"]],
        ["try", ["std.test.assert_i32_eq", ["ext.hex.hex_is_err", "doc_view"], 0, ["std.test.code_assert_i32_eq"]]],
        ["let", "decoded", ["ext.hex.hex_get_bytes", "doc_view"]],
        ["try", ["std.test.assert_bytes_eq", "decoded", "src_bytes", ["std.test.code_assert_bytes_eq"]]],
        ["std.test.pass"]
      ]
    }
  ]
}
```

### A5. Write `x07-package.json` and `x07.json`

Replace `x07-package.json` with:

```json
{
  "schema_version": "x07.package@0.1.0",
  "name": "ext-hex-rs-demo",
  "version": "0.1.0",
  "description": "Demo package: hex imported via x07import (restricted Rust subset).",
  "docs": "Imported module: ext.hex\nTests module: ext.hex.tests\n",
  "module_root": "modules",
  "modules": ["ext.hex", "ext.hex.tests"],
  "meta": {
    "determinism_tier": "pure",
    "worlds_allowed": ["solve-pure", "solve-fs", "solve-rr", "solve-kv", "solve-full", "run-os", "run-os-sandboxed"],
    "import_mode": "x07import"
  }
}
```

Create `x07.json` for local tests:

```json
{
  "schema_version": "x07.project@0.2.0",
  "world": "solve-pure",
  "entry": "modules/ext/hex/tests.x07.json",
  "module_roots": ["modules"],
  "lockfile": "x07.lock.json",
  "dependencies": []
}
```

### A6. Add a test manifest and run it

Create `tests/tests.json`:

```json
{
  "schema_version": "x07.tests_manifest@0.1.0",
  "tests": [
    {
      "id": "encode_decode_roundtrip",
      "world": "solve-pure",
      "entry": "ext.hex.tests.test_encode_decode_roundtrip",
      "expect": "pass"
    }
  ]
}
```

Run:

```bash
x07 test --manifest tests/tests.json
```

## Example B: C → x07 (clang-backed smoke)

### B1. Add a small C source file

Create `import_sources/c/smoke.c`:

```c
#include <stdint.h>

static inline int32_t ext_smoke_add_i32(int32_t a, int32_t b) { return a + b; }
static inline int32_t ext_smoke_abs_i32(int32_t x) { return x < 0 ? -x : x; }
```

### B2. Run the importer

```bash
mkdir -p modules/ext
x07import-cli c \
  --in import_sources/c/smoke.c \
  --module-id ext.smoke \
  --out modules
```

Then inspect the generated exports in `modules/ext/smoke.x07.json` and write a small `ext.smoke.tests` module that asserts behavior.

## Publishing

Once you have:

- `x07-package.json`
- tests passing (`x07 test`)

…publish using the same flow as [Publishing by example](../packages/publishing-by-example.md).
