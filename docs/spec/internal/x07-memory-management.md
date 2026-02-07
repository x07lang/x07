Spec-ID: x07.spec.internal.x07-memory-management@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# Memory management (native C backend)

## Core model (ABI v2)

- `bytes` is an **owned** heap allocation (move-only) that is dropped automatically by compiler-emitted drop glue.
- `vec_u8` is an **owned** growable heap allocation (move-only) that is dropped automatically; finalize with `std.vec.as_bytes` (consumes the vec, wraps `vec_u8.into_bytes`).
- `bytes_view` is a **borrowed** read-only view into `bytes` / `vec_u8` / runtime input buffers.
  - Copy a view into owned bytes with `view.to_bytes`.

`input` is a `bytes_view`. `solve` must return owned `bytes`.

## Allocator

- All allocations go through a fixed-capacity deterministic heap (`X07_MEM_CAP`) that supports `alloc`/`free` with coalescing.
- Allocations are zero-initialized.
- `realloc` is an “allocate-new” operation; callers preserve old contents explicitly so `mem_stats.memcpy_bytes` is deterministic.

## Metrics and leak gates

- The runtime resets `mem_stats` after reading the input payload (epoch reset), so input bytes and runtime init are excluded from memory gates/scoring.
- Suites can require:
  - `assertions.mem_stats_required` (must emit `mem_stats`)
  - `assertions.leak_free_required` (enforced as `mem_stats.live_bytes == 0 && live_allocs == 0` at exit)

## Debug borrow checks

- Debug builds can enable runtime borrow checks (`--debug-borrow-checks`) and emit `debug_stats.borrow_violations`.
- In release builds, borrow rules are enforced statically by the compiler (lexical borrows).

## Related

- ABI and execution model: `docs/spec/internal/x07-c-backend.md`
- Allocator interface: `docs/spec/abi/allocator-v1.md`
- Sanitizer gate: `./scripts/ci/check_asan_c_backend.sh`
