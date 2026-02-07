Spec-ID: x07.spec.abi.abi-v2@0.1.0
Status: stable
Applies-to: toolchain >= v0.0.95
Related schemas: []

# X07 ABI v2 (value layouts + ownership semantics)

This document specifies **ABI v2** value layouts used by the X07 C backend for **generated code** and imported stdlib modules.

ABI v2 is a **breaking** change from ABI v1: `bytes` becomes an **owning** heap allocation and `input` becomes a `bytes_view` (borrowed).

Scope:
- **Value layouts** for core runtime types (`bytes`, `bytes_view`, `vec_u8`, interface records, Option/Result encodings).
- Not the solver executable I/O ABI (`stdin`/`stdout`), which is specified in `docs/spec/internal/x07-c-backend.md`.

Non-goals:
- Stable ABI across **debug vs release** builds when `X07_DEBUG_BORROW` changes layouts. ABI v2 is defined for release layouts; debug-only fields are explicitly guarded by `#ifdef X07_DEBUG_BORROW` in the C header.

## Scalar conventions

- `i32` is represented as an **unsigned** 32-bit integer in C (`uint32_t`) with two’s-complement interpretation where needed.
- `bool` is represented as `uint32_t` (`0` = false, non-zero = true).

## `bytes` (owned)

`bytes` is an **owning** byte buffer.

Layout:
- `ptr`: `uint8_t*`
- `len`: `uint32_t`

Invariants:
- If `len == 0`, `ptr` is a non-null sentinel (implementation-defined; do not dereference).
- If `len > 0`, `ptr` is non-null and points to a heap allocation of exactly `len` bytes.

Ownership:
- `bytes` is move-only (affine). Drops free the underlying allocation.
- Borrowed reads/slices must use `bytes_view`.

## `bytes_view` (borrowed)

`bytes_view` is a **borrowed** byte slice used for zero-copy scan/slice.

Layout (release):
- `ptr`: `uint8_t*`
- `len`: `uint32_t`

Debug-only fields (only when `X07_DEBUG_BORROW` is defined):
- `aid`: allocation id (u64)
- `bid`: borrow id (u64)
- `off_bytes`: byte offset within the allocation (u32)

Borrowing rules:
- Enforced statically by the compiler in release builds (lexical borrows initially).
- Enforced dynamically in debug builds (deterministic borrow table).

## `vec_u8` (owned)

`vec_u8` is an owning, growable byte buffer.

Layout (release):
- `data`: `uint8_t*`
- `len`: `uint32_t`
- `cap`: `uint32_t`

Debug-only fields (only when `X07_DEBUG_BORROW` is defined):
- `dbg_aid`: allocation id for the backing buffer (u64)

Invariants:
- `len <= cap`
- If `cap == 0`, `data` is a non-null sentinel (implementation-defined; do not dereference) and `len` must be `0`.
- If `cap > 0`, `data` is non-null and points to a heap allocation of `cap` bytes.

Ownership:
- `vec_u8` is move-only (affine). Drops free the backing allocation.
- Finalizing a builder transfers ownership without copying (`vec_u8.into_bytes`).

## Interface records

Interface records are ABI-stable “fat handles”:
- `data`: opaque handle (u32)
- `vtable`: opaque handle (u32)

The initial use is `iface(io.reader)` for Phase G2 streaming readers.

## Option / Result

ABI v2 uses tagged layouts by default.

Option:
- `tag`: `uint32_t` (`0` = none, `1` = some)
- `payload`: `T` (only meaningful when `tag == 1`)

Result:
- `tag`: `uint32_t` (`0` = err, `1` = ok)
- `payload`: a union `{ ok: T, err: E }` (selected by `tag`)

Concrete v2 instantiations used by the built-in stdlib and compiler:
- `option_i32`: `Option<i32>`
- `option_bytes`: `Option<bytes>`
- `option_bytes_view`: `Option<bytes_view>`
- `result_i32`: `Result<i32, i32>` (err payload is an error code)
- `result_bytes`: `Result<bytes, i32>` (err payload is an error code)
- `result_bytes_view`: `Result<bytes_view, i32>` (err payload is an error code)
- `result_result_bytes`: `Result<result_bytes, i32>` (err payload is an error code)

Notes:
- Brands (for bytes-like types) are compile-time only and do not affect ABI layouts.
