# Iteration for collections (emitters v1)

X07 programs are evaluated as pure `bytes -> bytes` functions. For data structures, the missing "iteration" capability is a deterministic way to turn a collection into a canonical `bytes` encoding.

Rather than introducing first-class stateful iterators, Phase H2 uses **emitters**: stdlib functions that serialize a collection into bytes in a stable order.

## Status (implemented)

- Normative spec: `docs/spec/stdlib-emitters-v1.md`
- Canary suite: `benchmarks/solve-pure/emitters-v1-suite.json`
  - Included in: `benchmarks/bundles/phaseH2.json`, `benchmarks/bundles/phaseH1H2.json`
- Implemented emitters (v1):
  - `stdlib/std/0.1.1/modules/std/btree_set.x07.json`: `std.btree_set.emit_u32le`
  - `stdlib/std/0.1.1/modules/std/btree_map.x07.json`: `std.btree_map.emit_kv_u32le_u32le`
  - `stdlib/std/0.1.1/modules/std/hash_set.x07.json`: `std.hash_set.emit_u32le`
  - `stdlib/std/0.1.1/modules/std/hash_map.x07.json`: `std.hash_map.emit_kv_u32le_u32le`
  - `stdlib/std/0.1.1/modules/std/deque_u32.x07.json`: `std.deque_u32.emit_u32le`
  - `stdlib/std/0.1.1/modules/std/heap_u32.x07.json`: `std.heap_u32.emit_u32le`

## Single canonical way

Each collection module exposes exactly one canonical `emit_*` function per supported encoding.

Solver programs should not "iterate manually" for deterministic output. Instead:
- build or update the collection with stdlib helpers,
- call `std.*.emit_*`,
- return the emitted bytes (or parse/process them further).

## Canonical ordering rules (v1)

The spec pins these ordering semantics:

- `std.btree_*`: ascending by key/element (already stored in sorted packed form)
- `std.hash_*`: ascending by key/element (canonicalize by sorting before emitting)
- `std.deque_u32`: front-to-back
- `std.heap_u32`: non-decreasing pop-min order

## Implementation notes

### BTree emitters (identity)

`std.btree_set` and `std.btree_map` already store contents as sorted packed bytes. Their emitters return the underlying representation unchanged.

### Deque emitter (walk ring buffer)

`std.deque_u32.emit_u32le` iterates from the logical head for `len` elements (wrapping at `cap`) and writes `u32le` values into a `vec_u8` builder.

### Heap emitter (drain via pop-min)

`std.heap_u32.emit_u32le` clones the heap bytes and repeatedly calls `pop_min_or` to emit elements in canonical order.

### Hash emitters (dump -> build btree -> emit)

`std.hash_set.emit_u32le` and `std.hash_map.emit_kv_u32le_u32le` canonicalize output by building an intermediate `std.btree_*` structure and emitting from that.

To do this efficiently in the C backend, the stdlib uses internal-only builtins that dump the current contents of the underlying `map_u32` / `set_u32` runtime tables:

- `set_u32.dump_u32le(handle:i32) -> bytes`
- `map_u32.dump_kv_u32le_u32le(handle:i32) -> bytes`

These builtins are internal-only:
- Allowed in compiler-embedded builtin modules (stdlib).
- Rejected in entry programs and filesystem modules at compile time.

## Future direction (Phase H4)

If/when X07 needs streaming iteration to avoid large materializations, the canonical "iterator" should be an `iface` reader (as with `std.io` / `std.io.bufread`).

Emitters remain the canonical materializers for stable bytes fixtures and benchmarks.
