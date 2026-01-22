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

- `examples/tutorials/x07import_rust_memchr_pkg/`
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

## Example A: Rust → x07 (memchr)

This example imports a minimal “find byte in buffer” routine.

### A1. Create the skeleton

```bash
mkdir ext-memchr-rs-demo
cd ext-memchr-rs-demo
x07 --init --package
rm -rf src
mkdir -p import_sources/rust modules/ext/memchr tests
```

### A2. Add a restricted Rust source file

Create `import_sources/rust/memchr.rs`:

```rust
pub fn memchr(hay: BytesView, needle: i32) -> i32 {
    let n = view_len(hay);
    for i in 0..n {
        if view_get_u8(hay, i) == needle {
            return i;
        }
    }
    -1
}
```

### A3. Run the importer

```bash
x07import-cli rust \
  --in import_sources/rust/memchr.rs \
  --module-id ext.memchr \
  --out modules
```

Expected output:

- `modules/ext/memchr.x07.json`

### A4. Add a handwritten tests module

Create `modules/ext/memchr/tests.x07.json`:

```json
{
  "schema_version": "x07.x07ast@0.1.0",
  "kind": "module",
  "module_id": "ext.memchr.tests",
  "imports": ["ext.memchr", "std.test"],
  "decls": [
    { "kind": "export", "names": ["ext.memchr.tests.finds_byte"] },
    {
      "kind": "defn",
      "name": "ext.memchr.tests.finds_byte",
      "params": [],
      "result": "result_i32",
      "body": [
        "begin",
        ["let", "s", ["bytes.lit", "abc"]],
        ["let", "idx", ["ext.memchr.memchr", ["bytes.view", "s"], 98]],
        ["try", ["std.test.assert_i32_eq", "idx", 1, ["std.test.code_assert_i32_eq"]]],
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
  "name": "ext-memchr-rs-demo",
  "version": "0.1.0",
  "description": "Demo package: memchr imported via x07import (restricted Rust subset).",
  "docs": "Imported module: ext.memchr\nTests module: ext.memchr.tests\n",
  "module_root": "modules",
  "modules": ["ext.memchr", "ext.memchr.tests"],
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
  "entry": "modules/ext/memchr/tests.x07.json",
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
    { "id": "finds_byte", "world": "solve-pure", "entry": "ext.memchr.tests.finds_byte", "expect": "pass" }
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
