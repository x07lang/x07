Spec-ID: x07.spec.abi.abi-v1@0.1.0
Status: stable
Applies-to: toolchain >= v0.0.95
Related schemas: []

# X07 ABI v1 (value layouts)

ABI v1 is a legacy spec kept for reference. The current active ABI is ABI v2 (`docs/spec/abi/abi-v2.md`).

This document specifies **ABI v1** value layouts used by the X07 C backend for **generated code** and imported stdlib modules.

Scope:
- **Value layouts** for core runtime types (`bytes`, `bytes_view`, `vec_u8`, interface records, Option/Result encodings).
- Not the solver executable I/O ABI (`stdin`/`stdout`), which is specified in `docs/spec/internal/x07-c-backend.md`.

Non-goals:
- A stable ABI across **debug vs release** builds when `X07_DEBUG_BORROW` changes layouts. ABI v1 is defined for release layouts; debug-only fields are explicitly guarded by `#ifdef X07_DEBUG_BORROW` in the C header.

## Scalar conventions

- `i32` is represented as an **unsigned** 32-bit integer in C (`uint32_t`) with two’s-complement interpretation where needed.
- `bool` is represented as `uint32_t` (`0` = false, non-zero = true).

## Raw pointers (Phase H4, standalone-only)

Raw pointer types map directly to C pointer types:

- `ptr_const_u8` → `const uint8_t*`
- `ptr_mut_u8` → `uint8_t*`
- `ptr_const_void` → `const void*`
- `ptr_mut_void` → `void*`
- `ptr_const_i32` → `const uint32_t*` (because `i32` is represented as `uint32_t`)
- `ptr_mut_i32` → `uint32_t*`

These types and their associated operations are only permitted in standalone worlds (`run-os` / `run-os-sandboxed`), gated by the world capability model and explicit `unsafe` blocks.

## `bytes`

`bytes` is a non-owning byte slice (fat pointer) into the invocation arena.

Layout:
- `ptr`: `uint8_t*`
- `len`: `uint32_t`

## `bytes_view`

`bytes_view` is a non-owning byte slice used for zero-copy scan/slice with (optional) debug borrow instrumentation.

Layout (release):
- `ptr`: `uint8_t*`
- `len`: `uint32_t`

Debug-only fields (only when `X07_DEBUG_BORROW` is defined):
- `aid`: allocation id (u64)
- `bid`: borrow id (u64)
- `off_bytes`: byte offset within the allocation (u32)

## `vec_u8`

`vec_u8` is a mutable, growable byte buffer.

Layout (release):
- `data`: `uint8_t*`
- `len`: `uint32_t`
- `cap`: `uint32_t`

Debug-only fields (only when `X07_DEBUG_BORROW` is defined):
- `dbg_aid`: allocation id for the backing buffer (u64)

Invariants:
- `len <= cap`
- If `cap == 0`, `data` may be null and `len` must be `0`.

## `box_bytes`

`box_bytes` is an owning bytes allocation used for move-only ownership patterns in X07 code.

ABI v1 defines `box_bytes` as layout-identical to `bytes`.

## Interface records

Interface records are ABI-stable “fat handles”:
- `data`: opaque handle (u32)
- `vtable`: opaque handle (u32)

The initial use is `iface(io.reader)` for Phase G2 streaming readers.

## Option / Result

ABI v1 uses tagged layouts by default.

Option:
- `tag`: `uint32_t` (`0` = none, `1` = some)
- `payload`: `T` (only meaningful when `tag == 1`)

Result:
- `tag`: `uint32_t` (`0` = err, `1` = ok)
- `payload`: a union `{ ok: T, err: E }` (selected by `tag`)

Concrete v1 instantiations used by the built-in stdlib and compiler:
- `option_i32`: `Option<i32>`
- `option_bytes`: `Option<bytes>`
- `result_i32`: `Result<i32, i32>` (err payload is an error code)
- `result_bytes`: `Result<bytes, i32>` (err payload is an error code)

Future optimization (documented but not required for v1 correctness):
- `Option<Box<T>>` may use a null-pointer niche (NPO) when `Box<T>` is represented as a nullable pointer.
