Spec-ID: x07.spec.abi.allocator-v1@0.1.0
Status: stable
Applies-to: toolchain >= v0.0.95
Related schemas: []

# Allocator ABI v1

Allocator ABI v1 defines a C-friendly interface used by the generated runtime for allocation and deterministic memory accounting.

This ABI is intended to support:
- a deterministic fixed-capacity heap allocator in evaluation mode (default)
- optional alternate allocators in standalone mode (future)

## C interface

See `crates/x07c/include/x07_abi_v2.h` (`ev_allocator_v1_t`).

Notes:
- All sizes and alignments are expressed as 32-bit unsigned integers.
- Implementations must be deterministic for a fixed program + inputs.
- The runtime assumes allocations are **zero-initialized**.
- `realloc` is an “allocate-new” operation for deterministic accounting:
  - It returns a fresh allocation of `new_size` bytes aligned to `align`.
  - The runtime preserves old contents explicitly (so `mem_stats.memcpy_bytes` is deterministic and accurate).
  - Implementations should not perform hidden copies internally.

## Standalone override hook

The generated C runtime contains a weak symbol `x07_custom_allocator()` that can be overridden at link time.
If it returns an `ev_allocator_v1_t` with non-null hooks, the runtime will use it instead of the default heap allocator.
If it returns null hooks, the runtime uses the default deterministic heap allocator.

Note: the generated runtime defines its own internal `allocator_v1_t` typedef with the same field layout as `ev_allocator_v1_t`.
For overrides, use the `ev_allocator_v1_t` layout from `crates/x07c/include/x07_abi_v2.h`.
