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

## Vectors (capacity-planned builders)

Use `vec_u8` for output building:

- `vec_u8.with_capacity(n)` to preallocate
- `vec_u8.extend_bytes(...)` to append chunks
- `vec_u8.as_bytes` to produce final bytes (often as a view)

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
