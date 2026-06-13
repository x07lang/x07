# Generics (monomorphization)

As of x07AST `schema_version` **`x07.x07ast@0.4.0`** (and later), X07 supports bounded generics via **compile-time monomorphization**.

- Generics are compile-time only: each `tapp` instantiation produces a specialized copy.
- The program given to the existing optimizer + C backend is fully monomorphic (no `tapp` / `ty.*` remains).

## At a glance: define and call (both surfaces)

X07 has two equivalent surfaces — the x07AST JSON and its x07text (s-expression)
projection. Generics look like this in each:

**Define** a generic `defn` / `defasync` — declare `type_params`, then refer to a
type variable as `(t A)` / `["t","A"]`:

```clojure
; x07text
{:kind defn :name main.id
 :type_params ({:name A :bound any})
 :params ({:name x :ty (t A)})
 :result (t A)
 :body x}
```

```json
{"kind":"defn","name":"main.id",
 "type_params":[{"name":"A","bound":"any"}],
 "params":[{"name":"x","ty":["t","A"]}],
 "result":["t","A"],
 "body":"x"}
```

**Call** a generic by passing type arguments with `tapp` + `tys`:

```clojure
; x07text
(tapp main.id (tys u32) n)
(tapp std.btree_map.put (tys u32 u32) m 1 v)
```

```json
["tapp","main.id",["tys","u32"],"n"]
["tapp","std.btree_map.put",["tys","u32","u32"],"m",1,"v"]
```

The sections below give the JSON form in full; see also
[x07text projection](x07text.md).

## Schema + compatibility

- Use `x07.x07ast@0.8.0` for new files (including any use of: `type_params`, structured `type_ref`, `tapp`, `ty.*`, contracts, or `decreases`).
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

### Bounds → which `ty.*` intrinsics each permits

A bound restricts which concrete types may be substituted for a type variable.
The `ty.*` intrinsics (see below) are lowered per concrete type, and each
intrinsic only accepts a fixed set of concrete types. So a `ty.*` op is usable
inside a generic body exactly when the type variable's bound only admits
concrete types that the op accepts.

The concrete-type acceptance of each intrinsic is:

| Intrinsic | Accepts |
|---|---|
| `ty.size_bytes` / `ty.size` | `i32`, `u32`, `bytes`, `bytes_view` |
| `ty.read_le_at` | `i32`, `u32` only |
| `ty.write_le_at` | `i32`, `u32` only |
| `ty.push_le` | `i32`, `u32` only |
| `ty.lt` | `i32`, `u32`, `bytes`, `bytes_view` |
| `ty.cmp` | `i32`, `u32`, `bytes`, `bytes_view` |
| `ty.eq` | `i32`, `u32`, `bytes`, `bytes_view` |
| `ty.hash32` | `i32`, `u32`, `bytes`, `bytes_view` |
| `ty.clone` | `i32`, `u32`, `bytes`, `bytes_view` |
| `ty.drop` | `i32`, `u32`, `bytes`, `bytes_view` |
| `ty.add` / `ty.sub` / `ty.mul` | `i32`, `u32` only |

Combined with the bounds above, this gives the practical mapping:

- `num_like` (`i32`, `u32`) — **all** `ty.*` intrinsics are usable (including
  the `i32`/`u32`-only `ty.read_le_at` / `ty.write_le_at` / `ty.push_le` and the
  arithmetic ops `ty.add` / `ty.sub` / `ty.mul`).
- `value` / `hashable` / `orderable` (`i32`, `u32`, `bytes`, `bytes_view`) —
  `ty.size_bytes`, `ty.size`, `ty.lt`, `ty.cmp`, `ty.eq`, `ty.hash32`,
  `ty.clone`, `ty.drop` are usable. The LE-codec ops
  (`ty.read_le_at` / `ty.write_le_at` / `ty.push_le`) are **not**, because they
  reject `bytes` / `bytes_view`.
- `bytes_like` (`bytes`, `bytes_view`) — same usable set as `value` (size,
  ordering, equality, hashing, clone, drop); the LE-codec ops reject these
  types.
- `any` (admits any concrete type) — no `ty.*` op is guaranteed; only
  structural moves (passing the value through, as in the identity example)
  are safe. A `ty.*` op under `any` compiles only for the specific
  instantiations you actually use, and fails on any instantiation outside the
  intrinsic's accepted set.

For `bytes` / `bytes_view`, the comparison and hashing intrinsics operate on
contents: `ty.lt` / `ty.cmp` are lexicographic, `ty.eq` compares contents, and
`ty.hash32` hashes contents. This is what makes a **generic `max` / `min` /
sort** work uniformly across numbers and byte strings.

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
- `ty.add(T, a, b)` / `ty.sub(T, a, b)` / `ty.mul(T, a, b)` → `T` (`i32`/`u32` only; wrap modulo 2^32)

The complete `ty.*` set is exactly the fourteen intrinsics above:
`ty.size_bytes`, `ty.size`, `ty.read_le_at`, `ty.write_le_at`, `ty.push_le`,
`ty.lt`, `ty.cmp`, `ty.eq`, `ty.hash32`, `ty.clone`, `ty.drop`, `ty.add`,
`ty.sub`, `ty.mul`. Signature forms (with `T` written as a type variable
`["t","A"]`):

```jsonc
["ty.size_bytes", ["t","A"]]               // -> i32 constant
["ty.size",       ["t","A"]]               // -> i32 constant
["ty.read_le_at", ["t","A"], bytes_view, off]  // -> A   (i32/u32 only)
["ty.write_le_at",["t","A"], bytes, off, x]    // -> bytes (i32/u32 only)
["ty.push_le",    ["t","A"], vec_u8, x]        // -> vec  (i32/u32 only)
["ty.lt",         ["t","A"], a, b]         // -> i32  (1 if a < b else 0)
["ty.cmp",        ["t","A"], a, b]         // -> i32  (-1 / 0 / 1)
["ty.eq",         ["t","A"], a, b]         // -> i32  (0/1)
["ty.hash32",     ["t","A"], x]            // -> i32
["ty.clone",      ["t","A"], x]            // -> A    (does not consume x)
["ty.drop",       ["t","A"], x]            // -> i32  (consumes x)
["ty.add",        ["t","A"], a, b]         // -> A    (i32/u32 only; wraps)
["ty.sub",        ["t","A"], a, b]         // -> A    (i32/u32 only; wraps)
["ty.mul",        ["t","A"], a, b]         // -> A    (i32/u32 only; wraps)
```

## Generic arithmetic (`num_like` only)

Generic add / subtract / multiply are available via `ty.add` / `ty.sub` /
`ty.mul`, restricted to the `num_like` bound (`i32` / `u32`). They lower to the
ordinary `+` / `-` / `*` operators for the substituted width and wrap modulo
2^32, so a generic numeric **fold / sum / reduce** is expressible:

```jsonc
{
  "kind": "defn",
  "name": "pkg.sum2",
  "type_params": [{"name": "A", "bound": "num_like"}],
  "params": [{"name": "a", "ty": ["t","A"]}, {"name": "b", "ty": ["t","A"]}],
  "result": ["t","A"],
  "body": ["ty.add", ["t","A"], "a", "b"]
}
```

Boundaries:

- There is **no generic division or modulo** intrinsic — integer division
  differs by signedness and traps on zero, so it stays a monomorphic concern
  (the `ty.*` arithmetic set is add / subtract / multiply only).
- The bare `+` / `-` / `*` operators still reject a type variable: `["+","x","x"]`
  on `x: (t A)` is a `X07-TYPE-UNIFY-0001` unification failure (the operators
  unify operands with the concrete `i32`). Inside a generic body, use the
  `ty.*` arithmetic intrinsics, not the bare operators.
- `ty.add` / `ty.sub` / `ty.mul` reject `bytes` / `bytes_view`, so they are only
  usable under `num_like` (not `value` / `orderable` / `bytes_like` / `any`).

A generic **`max` / `min` / sort** works through `ty.lt` / `ty.cmp` / `ty.eq`,
which are defined for every type in the `orderable` / `value` set — including
`bytes` and `bytes_view`, compared lexicographically by contents.

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
