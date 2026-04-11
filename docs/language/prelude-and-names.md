# Prelude & names

X07 has three “name” categories you’ll see in programs and docs:

1. **Module exports**: fully-qualified `module_id.symbol` names (for example `std.bytes.len`, `ext.net.http.client.get_v1`, `myapp.core.hash`).
2. **Builtin forms**: compiler/runtime primitives that are available without imports (for example `bytes.len`, `view.slice`, `codec.read_u32_le`, `vec_u8.extend_bytes`, `task.scope_v1`, `chan.bytes.recv`, `budget.scope_v1`).
3. **Built-in modules**: a small set of `std.*` modules embedded in the toolchain (these show `file: <builtin>` in `x07 doc`).

## How to inspect names

- Module exports:
  - `x07 doc <module-id>` (example: `x07 doc std.bytes`)
  - `x07 doc <module-id>.<export>` (example: `x07 doc std.bytes.cmp_range`)
- Builtins:
  - `x07 doc <builtin>` (example: `x07 doc bytes.len`)
  - `x07 doc --builtin <builtin>` forces builtin resolution.
- If a name isn’t covered by `x07 doc`, use `x07 guide` for the full reference.

## Guidance

- Prefer fully-qualified `std.*` functions in shared code when they exist (clearer and easier to search).
- Use builtin forms for low-level work and performance-sensitive loops:
  - `bytes_view` + `view.*` for scanning/slicing without copying
  - `vec_u8.*` for building output incrementally (convert once at the end)
- When in doubt, you can always write the fully-qualified `std.*` call even if a shorter builtin exists.

