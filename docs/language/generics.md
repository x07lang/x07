# Generics (monomorphization)

As of x07AST `schema_version` **`x07.x07ast@0.4.0`** (and later), X07 supports bounded generics via **compile-time monomorphization**.

- Generics are compile-time only: each `tapp` instantiation produces a specialized copy.
- The program given to the existing optimizer + C backend is fully monomorphic (no `tapp` / `ty.*` remains).

## Schema + compatibility

- Use `x07.x07ast@0.5.0` for new files (including any use of: `type_params`, structured `type_ref`, `tapp`, `ty.*`).
- The toolchain also accepts legacy `x07.x07ast@0.4.0` for generics-only programs (no contracts).
- The toolchain still accepts legacy `x07.x07ast@0.3.0` for concrete-only programs.

## `type_ref` (structured type expressions)

In `params[].ty` and `result`, a `type_ref` can be either:

- a legacy concrete type token (string), like `"i32"`, `"bytes"`, or
- a structured type expression (array):
  - type variable: `["t","A"]`
  - type application: `["option", ["t","A"]]`, `["result", "bytes_view"]`

## `type_params` (generic declarations)

`defn` / `defasync` may include `type_params`:

```jsonc
{
  "kind": "defn",
  "name": "main.id",
  "type_params": [{"bound":"num_like","name":"A"}],
  "params": [{"name":"x","ty":["t","A"]}],
  "result": ["t","A"],
  "body": "x"
}
```

Bounds are canonical and small:

- `any` (no constraint)
- `bytes_like`
- `num_like`
- `value`
- `hashable`
- `orderable`

## Bounds (currently supported concrete types)

<!-- x07-generics-bounds:begin -->
```json
{
  "any": ["*"],
  "bytes_like": ["bytes", "bytes_view"],
  "num_like": ["i32", "u32"],
  "value": ["i32", "u32", "bytes", "bytes_view"],
  "hashable": ["i32", "u32", "bytes", "bytes_view"],
  "orderable": ["i32", "u32", "bytes", "bytes_view"]
}
```
<!-- x07-generics-bounds:end -->

## `tapp` (generic instantiation at call sites)

Form:

```jsonc
["tapp", "<callee_symbol>", ["tys", <type_arg_0>, <type_arg_1>, ...], <value_arg_0>, ...]
```

Examples:

```jsonc
["tapp","std.heap.with_capacity",["tys","u32"],16]
["tapp","std.heap.push",["tys","u32"],"h",7]
["tapp","std.vec.with_capacity",["tys",["t","A"]],"cap"]
```

## Local type inference + `tapp` elaboration (inside bodies)

When a call targets a generic function but omits `tapp`, `x07 lint` can infer the missing type arguments from local usage and emit a JSON Patch quickfix that rewrites the call into the canonical `tapp` form.

Use `x07 fix --write` to apply these rewrites.

## Examples

- `docs/examples/16_generics_identity.x07.json`
- `docs/examples/17_generics_containers.x07.json`

## Migration pattern (generic base + wrappers)

To migrate existing concrete-only APIs without breaking callers:

- Introduce a new generic base function (for example `pkg.foo` with `type_params: [{ "name": "A" }]`).
- Keep existing concrete entrypoints as thin wrappers that call the base via `tapp` (for example `pkg.foo_u32` → `["tapp","pkg.foo",["tys","u32"], ...]`).

Tooling support:

- `x07 fix --suggest-generics` emits a suggested `x07.patchset@0.1.0` for converting near-identical type-suffixed functions into a generic base plus wrappers.

## `ty.*` intrinsics (type-dependent ops inside generic bodies)

`ty.*` intrinsics are compile-time intrinsics that are lowered during monomorphization.

Supported (v0.4 toolchain):

- `ty.size_bytes(T)` / `ty.size(T)` → `i32` constant (currently: `i32`/`u32` only)
- `ty.read_le_at(T, bytes_view, off)` → `std.u32.read_le_at(...)` (currently: `i32`/`u32` only)
- `ty.write_le_at(T, bytes, off, x)` → `std.u32.write_le_at(...)` (currently: `i32`/`u32` only)
- `ty.push_le(T, vec_u8, x)` → `std.u32.push_le(...)` (currently: `i32`/`u32` only)
- `ty.clone(T, x)` → `T` (does not consume `x`; for `bytes` does a deep clone)
- `ty.drop(T, x)` → `i32` (consumes `x`; for `bytes` runs an explicit drop)
- `ty.lt(T, a, b)` → `<` or `<u` (`bytes`/`bytes_view` are lexicographic by contents)
- `ty.eq(T, a, b)` → `i32` (`bytes`/`bytes_view` compare contents)
- `ty.cmp(T, a, b)` → `i32` (`bytes`/`bytes_view` are lexicographic by contents)
- `ty.hash32(T, x)` → `i32` (`bytes`/`bytes_view` hash contents via `std.hash.fnv1a32_view` + `std.hash.mix32`)

## Budgets / caps

Monomorphization is bounded:

- max specializations: `4096`
- max type expression depth: `64`

When a cap is hit, compilation fails deterministically.

## Monomorphization map (`x07c --emit-mono-map`)

The compiler can emit a machine-readable monomorphization map, describing which generic functions were specialized and where the `tapp` sites occurred.

- `x07c compile --emit-mono-map <PATH> ...`
- `x07c build --emit-mono-map <PATH> ...`

Schema: `x07.mono.map@0.1.0` (`spec/x07-mono.map.schema.json`).
