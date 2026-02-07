Spec-ID: x07.spec.abi.interface-records-v1@0.1.0
Status: stable
Applies-to: toolchain >= v0.0.95
Related schemas: []

# Interface records ABI v1

Interface records provide a minimal, deterministic dynamic-dispatch substrate without a full trait solver.

## Layout

An interface record is:
- `data`: opaque handle (u32)
- `vtable`: opaque handle (u32)

See `crates/x07c/include/x07_abi_v2.h` (`ev_iface_v2_t`).

Notes:
- `vtable=1` is the built-in streaming reader vtable used by world adapters and in-memory adapters (e.g. `fs.open_read`, `kv.get_stream`, `io.open_read_bytes`).
- OS-world runtimes may register additional `vtable` IDs for external package adapters (still dispatching through `io.read` deterministically).

## Phase H1 scope

Phase H1 introduces interface records for Phase G2 streaming I/O:
- `iface(io.reader)` values are returned by `fs.open_read`, `kv.get_stream`, and `io.open_read_bytes`.
- `io.read` consumes the interface record and dispatches deterministically.
