# X7SL v1 — X07 Slice List (canonical bytes encoding)

X7SL is the **canonical** bytes encoding for a list of slices `(start,len)` into some **base** byte buffer (or `bytes_view`).
It is designed to be:
- deterministic (stable across runs/platforms),
- compact (8 bytes per slice),
- and easy for agents to consume without manual offset math (via `std.text.slices` helpers).

This document is **normative** for X7SL v1.

## Byte order and integer types

- All integers are **u32 little-endian** (`u32_le`) encoded as 4 bytes.
- `start` and `len` are interpreted as `u32` offsets into a base buffer.

## Encoding

An X7SL v1 blob is:

- Header (12 bytes)
  1. `magic[4]` = ASCII bytes `X7SL` (`0x45 0x56 0x53 0x4C`)
  2. `version_u32_le` = `1`
  3. `count_u32_le` = number of rows

- Rows (`count` × 8 bytes)
  - `start_u32_le`
  - `len_u32_le`

### Total length

The total byte length **MUST** be exactly:

```
12 + count*8
```

Trailing bytes are **not allowed**.

## Semantics

- Each row describes a slice `[start, start+len)` into some base buffer.
- X7SL does **not** include the base buffer length. Bounds checks require the base length.

### Canonical row order

Producers **SHOULD** emit rows in **ascending `start`** order.
If multiple rows share the same `start`, they **SHOULD** be ordered by ascending `len`.

(Consumers may rely on this for binary-search-based lookups.)

## Validation error codes (std.text.slices.validate_v1)

`std.text.slices.validate_v1(x7sl)` returns `result_i32` with:

- `OK(count)` on success
- `ERR(code)` on failure, using these deterministic codes:

| Code (hex)     | Name                       | Meaning |
|---------------:|----------------------------|---------|
| `0x7E510001`    | `X7SL_ERR_TRUNCATED`       | `bytes.len(x7sl) < 12` |
| `0x7E510002`    | `X7SL_ERR_UNSUPPORTED_VER` | `version != 1` |
| `0x7E510003`    | `X7SL_ERR_LEN_MISMATCH`    | `bytes.len(x7sl) != 12 + count*8` |
| `0x7E510004`    | `X7SL_ERR_BAD_MAGIC`       | first 4 bytes are not `X7SL` |

Notes:
- v1 validation checks **only** the X7SL framing (magic/version/length).
  Checking whether `(start+len) <= base_len` is done by the caller (because base_len is not in X7SL).

## Stdlib helpers

See `stdlib/std/0.1.2/modules/std/text/slices.x07.json`:

- Builder:
  - `std.text.slices.builder_new_v1(cap_hint)`
  - `std.text.slices.builder_push_v1(out,start,len)`
  - `std.text.slices.builder_finish_v1(out,count)` → `bytes@std.text.slices.x7sl_v1`
  - `std.text.slices.cast_bytes_v1(b)` → `result_bytes@std.text.slices.x7sl_v1` (validated cast)

- Accessors:
  - `std.text.slices.count_v1(x7sl)` → `i32` (count; requires branded X7SL)
  - `std.text.slices.start_v1(x7sl,idx)` → `i32`
  - `std.text.slices.len_v1(x7sl,idx)` → `i32`
  - `std.text.slices.view_at_v1(base_view,x7sl,idx)` → `bytes_view`
  - `std.text.slices.copy_at_v1(base_view,x7sl,idx)` → `bytes` (explicit copy)

Type system integration:

- The canonical X7SL brand id is `std.text.slices.x7sl_v1`.
- `std.text.slices.validate_v1(x7sl: bytes_view) -> result_i32` is the validator used for safe casting and stream pipe item validation.
