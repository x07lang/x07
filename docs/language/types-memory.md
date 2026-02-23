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

```json
["let","b",["bytes.lit","hello"]],
["let","v",["bytes.view","b"]]
```

For string literals, you can use `bytes.view_lit` to produce a `bytes_view` directly:

```json
["let","v",["bytes.view_lit","hello"]]
```

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
