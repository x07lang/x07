Spec-ID: x07.spec.internal.types.type-system-v1@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# Type System v1

Type System v1 defines a small, explicit set of concrete types with ABI-stable layouts.

## Where types appear

- `defn` decl objects: `{"kind":"defn","name":"main.f","params":[{"name":"arg1","ty":"ty1"}],"result":"ret_ty","body":<expr>}`
- `defasync` decl objects: `{"kind":"defasync","name":"main.f","params":[{"name":"arg1","ty":"ty1"}],"result":"bytes","body":<expr>}`
  - `defasync` return type is currently restricted to `bytes`.

In the v1 monomorphic ABI subset, type names are identifiers.

As of `x07.x07ast@0.4.0`, signature types are `type_ref` (string or structured type expression), and `defn`/`defasync` may declare `type_params` for compile-time monomorphized generics.

## Supported types

- `i32`: 32-bit integer (used for arithmetic, indices, booleans-as-0/1, and error codes)
- `bytes`: owned byte buffer (`ptr + len`, dropped automatically)
- `bytes_view`: borrowed byte view (`ptr + len` + debug-only borrow tracking fields)
- `vec_u8`: owned, growable byte vector value (`data + len + cap`, dropped automatically)
- `option_i32`, `option_bytes`, `option_bytes_view`: tagged options
- `result_i32`, `result_bytes`, `result_bytes_view`, `result_result_bytes`: tagged results with deterministic error codes
- `iface`: interface record (`data + vtable`) used for streaming readers
- Standalone-only raw pointers: `ptr_const_u8`, `ptr_mut_u8`, `ptr_const_void`, `ptr_mut_void`, `ptr_const_i32`, `ptr_mut_i32`

## Branded bytes

Bytes-like types may carry an optional compile-time brand:

- params: `{"name":"x","ty":"bytes","brand":"my.encoding_v1"}`
- return values: `{"result":"bytes","result_brand":"my.encoding_v1", ...}`

Brands exist only in typing and are erased during lowering (no runtime representation changes).

## ABI and lowering

The C ABI for these types is defined by:

- `docs/spec/abi/abi-v2.md`
- `crates/x07c/include/x07_abi_v2.h`

`iface` is specified under `docs/spec/abi/interface-records-v1.md`.

## Typed builtins

- Options: `option_i32.*`, `option_bytes.*`
- Results: `result_i32.*`, `result_bytes.*`
- Propagation: `["try", r]`
  - `try` is only valid inside a `defn` whose return type is `result_i32` or `result_bytes`.
