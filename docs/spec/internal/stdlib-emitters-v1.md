Spec-ID: x07.spec.internal.stdlib-emitters-v1@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# Stdlib emitters v1 (normative)

This document defines **canonical byte encodings** (“emitters”) for X07 stdlib data structures.

Emitters exist so:
- programs can serialize collections deterministically,
- benchmarks can assert exact outputs,
- and fixture-world programs can share stable binary formats.

See also: `labs/benchmarks/solve-pure/emitters-v1-suite.json`.

## General rules

### Naming

- Sets: `std.<set_module>.emit_<ENC>(...) -> bytes`
- Maps: `std.<map_module>.emit_kv_<KENC>_<VENC>(...) -> bytes`
- Sequences: `std.<seq_module>.emit_<ENC>(...) -> bytes`

### Output format

- Emitters return a `bytes` value containing **only** the concatenated encoded elements/records.
- Emitters MUST NOT include a top-level count prefix (length is implicit by EOF).
- Empty collections emit empty bytes.

### Determinism

For the same logical contents, emitters MUST return identical bytes regardless of insertion order.

## Primitive encodings

### `u32le`

4 bytes, little-endian unsigned 32-bit.

### `emit_kv_u32le_u32le`

A sequence of `(key u32le)(value u32le)` records:

```
[k0][v0][k1][v1]...[k{n-1}][v{n-1}]
```

## Collection emitters (v1 required surface)

### Ordered collections (BTree)

Canonical order is ascending by key/element.

- `std.btree_set.emit_u32le(set_bytes) -> bytes`
- `std.btree_map.emit_kv_u32le_u32le(map_bytes) -> bytes`

### Hash collections (Hash)

Canonical order is ascending by key/element (sort before emitting).

- `std.hash_set.emit_u32le(set_handle) -> bytes`
- `std.hash_map.emit_kv_u32le_u32le(map_handle) -> bytes`

### Deque (sequence)

Canonical order is front-to-back.

- `std.deque_u32.emit_u32le(dq_bytes) -> bytes`

### Heap (priority queue)

Canonical order is non-decreasing “pop-min” order (duplicates preserved).

- `std.heap_u32.emit_u32le(heap_bytes) -> bytes`
  - Consumes `heap_bytes` (drains the heap).

## Duplicate handling

- Sets: duplicates collapse (unique elements).
- Maps: duplicate keys resolve deterministically (last write wins for `set/put` style APIs).

## Compatibility / versioning

This document is **ABI/stability critical** for benchmarks and fixtures.

Any incompatible change to encodings or canonical order MUST introduce a new document
`stdlib-emitters-v2.md` and a new set of emitter functions (or versioned modules),
keeping v1 behavior available.
