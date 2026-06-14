# Types & memory model

X07’s memory model is designed to be:

- safe enough for autonomous agents,
- predictable in performance,
- compatible with native C backends and OS interop.

## Bytes vs views (zero-copy)

- `bytes` = owning buffer
- `bytes_view` = borrowed window into an existing buffer

Views are “fat pointers” (pointer + length + metadata). They allow scanning/splitting without copies.

Performance rule:
- Use views for parsing/scanning.
- Convert to owning bytes only when you must persist.

### Creating views (owner bindings)

`bytes.view` requires an identifier owner. You can’t create a view of a temporary expression; bind it first:

```clojure
; x07text
(let b (bytes.lit hello))
(let v (bytes.view b))
```

For string literals, you can use `bytes.view_lit` to produce a `bytes_view` directly:

```clojure
; x07text
(let v (bytes.view_lit hello))
```

The literal argument may include whitespace (same as `bytes.lit`).

## Vectors (capacity-planned builders)

Use `vec_u8` for output building:

- `std.vec.with_capacity(n)` to preallocate
- `std.vec.extend_bytes(v, b)` to append chunks
- `std.vec.as_bytes(v)` to produce final bytes (often without copying)

Note: there are also lower-level `vec_u8.*` builtins; prefer `std.vec.*` in application code.

This reduces:
- `realloc_calls`
- `memcpy_bytes`
- `peak_live_bytes`

## Boxes and moves

`box_bytes` (and later `box<T>`) is move-checked:

- once you “move out”, the original binding becomes invalid
- use-after-move is a compile error

This gives a Rust-like ownership feel without requiring Rust syntax.
If you want the ownership/borrowing mental model, the Rust book is a good reference:
- “References and Borrowing”
- “The Slice Type”

## Option / Result

X07 standardizes:

- `option_*` for absence/presence
- `result_*` for success/error with stable numeric error codes

Agents should prefer:

- results over sentinel bytes (“ERR” strings),
- stable error code spaces per module.

### View-carrying results

For zero-copy pipelines and parsers, X07 also provides view payload variants:

- `option_bytes_view`: `None | Some(bytes_view)`
- `result_bytes_view`: `Err(i32) | Ok(bytes_view)`

These are especially useful with brand-aware casts (`std.brand.cast_view_v1`) and stream pipes, where validation can operate on a view without allocating.

## Floating point (`f64`)

`f64` is an IEEE-754 double scalar, available from `x07.x07ast@0.9.0` (RFC 0002). It is
a by-value scalar like `i32`, lowered to C `double` and compiled with strict, deterministic
floating point (no fast-math, no FMA contraction), so results match across targets.

There is **no implicit numeric tower** — conversions between `i32` and `f64` are always
explicit:

- `f64.of_i32` — widen a signed `i32` to `f64`
- `f64.to_i32_trunc` — truncate an `f64` toward zero into `i32`
- `f64.add` / `f64.sub` / `f64.mul` / `f64.div` — arithmetic on two `f64` values

```clojure
; x07text
{
  :kind entry
  :module_id main
  :schema_version x07.x07ast@0.9.0
  :imports ()
  :decls ()
  :solve (codec.write_u32_le
    (f64.to_i32_trunc (f64.div (f64.of_i32 22) (f64.of_i32 7)))
  )
}
```

`(f64.div (f64.of_i32 22) (f64.of_i32 7))` is real division (≈ 3.142857), so
`f64.to_i32_trunc` yields `3` — not the `i32` result `22 / 7`. Mixing `f64` and `i32`
without an explicit conversion is a type error.

## Records (`defrecord`)

`defrecord` declares a nominal product type, available from `x07.x07ast@0.9.0`
(RFC 0002). A record lowers to a fixed-layout `bytes` value tagged with a brand equal
to the record's name, so it reuses the move-only `bytes` model — no GC, no new runtime
representation. Records v1 support `i32`/`u32` fields (each a 4-byte little-endian slot).

A declaration generates two kinds of operations:

- `<Record>.make` — the constructor, taking one `i32` arg per field in declaration order
  and returning a branded record value.
- `<Record>.<field>` — a field accessor, taking a value of that record and returning the
  field. The accessor borrows its argument (reads do not consume), so several fields can
  be read from the same value.

```clojure
; x07text
{
  :kind module
  :module_id app
  :schema_version x07.x07ast@0.9.0
  :imports ()
  :decls ({:kind defrecord :name app.Order :fields ({:name id :ty u32} {:name total :ty u32})}
    {:kind export :names (app.order_total)}
    {
      :kind defn
      :name app.order_total
      :body (app.Order.total o)
      :params ({:name o :brand app.Order :ty bytes})
      :result i32
    }
  )
}
```

The brand makes records nominal: passing unbranded `bytes`, or a different record, where
an `app.Order` is expected is a type error — `(app.Order.total x)` requires `x` to carry
the `app.Order` brand. Functions accept records by declaring a `bytes` parameter (or result)
with the record name as its `brand`.

## Enums (`defenum`) and `match`

`defenum` declares a nominal tagged-union (sum) type, available from `x07.x07ast@0.9.0`
(RFC 0002). Like a record, an enum value lowers to `bytes` branded with the enum's name, so
it reuses the move-only `bytes` model. The layout is `[u32 tag][payload?]`: a little-endian
tag holding the variant's 0-based declaration index, optionally followed by a 4-byte payload.
Enums v1 support unit variants and variants with a single `i32`/`u32` payload.

Each declared variant becomes a constructor `<Enum>.<Variant>`: a unit variant takes no
arguments, a payload variant takes one `i32`. You consume an enum with `match`, which is
**exhaustive** — every variant must appear exactly once, with no fallthrough or wildcard —
and binds a payload variant's value to a name in that arm:

```clojure
; x07text
{
  :kind module
  :module_id app
  :schema_version x07.x07ast@0.9.0
  :imports ()
  :decls ({:kind export :names (app.area)}
    {
      :kind defn
      :name app.area
      :body (match s (app.Shape.Unit 1) (app.Shape.Square side (* side side)))
      :params ({:name s :brand app.Shape :ty bytes})
      :result i32
    }
    {
      :kind defenum
      :name app.Shape
      :variants ({:name Unit} {:name Square :payload i32})
    }
  )
}
```

A match arm is `(<Enum>.<Variant> <body>)` for a unit variant or
`(<Enum>.<Variant> <bind> <body>)` for a payload variant, where `<bind>` names the payload
inside `<body>`. All arms must agree on a result type, and the compiler rejects a match that
omits a variant (`non-exhaustive match on enum app.Shape; missing arm(s): ...`), repeats one,
or scrutinizes a value that is not branded with an enum. As with records, functions accept an
enum by declaring a `bytes` parameter (or result) whose `brand` is the enum name.

## Branded bytes (typed encodings)

Bytes-like values can carry a nominal **brand** (compile-time only) to represent “validated bytes of encoding X”.

Conceptually:

- `bytes@B`, `bytes_view@B`
- `option_bytes@B`, `result_bytes@B`
- `option_bytes_view@B`, `result_bytes_view@B`

Compatibility:

- branded is assignable to unbranded (`bytes@B` can be passed as `bytes`)
- unbranded is not assignable to branded without an explicit cast/assume

Brand operators live under `std.brand.*`:

- `std.brand.cast_bytes_v1(brand_id, validator_id, b: bytes) -> result_bytes@brand_id`
- `std.brand.cast_view_v1(brand_id, validator_id, v: bytes_view) -> result_bytes_view@brand_id`
- `std.brand.cast_view_copy_v1(brand_id, validator_id, v: bytes_view) -> result_bytes@brand_id` (copy on success)
- `std.brand.assume_bytes_v1(brand_id, b: bytes) -> bytes@brand_id` (unsafe)
- `std.brand.erase_bytes_v1`, `std.brand.erase_view_v1` (drop a brand)
- `std.brand.view_v1(b: bytes@B) -> bytes_view@B` (full view)
- `std.brand.to_bytes_preserve_if_full_v1(v: bytes_view@B) -> bytes` (preserves `B` only when `v` is provably a full view)

Brands are also used by stream pipes as an item-level typecheck rail (see [Streaming pipes](stream-pipes.md)).

## Debug-only safety instrumentation

In debug builds / diagnostic runs, X07 can enable:

- runtime borrow checking (view lifetimes)
- per-allocation tracking tables

These features help agent repair loops find “why” a program failed, not just “that it failed”.

## Resource budgets

X07’s fixture worlds enforce deterministic budgets:

- fuel (instruction-like)
- memory cap
- I/O caps (bytes read, request counts, etc.)

In OS worlds, policies enforce caps for safety, but behavior is not deterministic.

For local (per-region) budgets in code, see: [Budget scopes](budget-scopes.md).
