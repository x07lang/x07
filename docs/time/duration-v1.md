# Duration v1 (X7DU)

Module: `ext.time.duration`

## Wire encoding (bytes doc)

A Duration is represented as a small tagged bytes document.

### OK doc (tag = 1)

Byte layout (little-endian):

| Offset | Size | Field | Type | Notes |
|---:|---:|---|---|---|
| 0 | 1 | `tag` | u8 | must be `1` |
| 1 | 1 | `ver` | u8 | must be `1` |
| 2 | 4 | `secs_lo` | u32 | low 32 bits of signed i64 seconds (two’s complement) |
| 6 | 4 | `secs_hi` | u32 | high 32 bits of signed i64 seconds (two’s complement) |
| 10 | 4 | `nanos` | u32 | must be `< 1_000_000_000` |

The value is: `secs_i64 + nanos/1e9`.

### ERR doc (tag = 0)

| Offset | Size | Field | Type | Notes |
|---:|---:|---|---|---|
| 0 | 1 | `tag` | u8 | must be `0` |
| 1 | 4 | `err_code` | u32 | see below |
| 5 | 4 | `reserved` | u32 | must be `0` |

## Error codes (v1)

Pinned numeric meanings:

- `SPEC_ERR_DUR_TRUNCATED = 1` — doc too short
- `SPEC_ERR_DUR_BAD_TAG = 2` — tag is not 0/1
- `SPEC_ERR_DUR_BAD_VER = 3` — ok doc but `ver != 1`
- `SPEC_ERR_DUR_BAD_NANOS = 4` — nanos >= 1e9
- `SPEC_ERR_DUR_RANGE = 5` — i64 overflow/underflow during normalization or arithmetic

## Canonical API (v1)

Constructors:

- `ext.time.duration.from_secs_i32_v1(secs: i32) -> bytes`
- `ext.time.duration.from_secs_u32_v1(secs_u32: i32) -> bytes`
- `ext.time.duration.from_nanos_v1(nanos_u32: i32) -> bytes`
- `ext.time.duration.make_v1(secs_lo: i32, secs_hi: i32, nanos_u32: i32) -> bytes`
  - Normalizes nanos carry/borrow and returns an OK/ERR doc.

Arithmetic:

- `add_v1(a: bytes, b: bytes) -> bytes`
- `sub_v1(a: bytes, b: bytes) -> bytes`

Accessors:

- `is_err_v1(doc: bytes_view) -> i32`
- `err_code_v1(doc: bytes_view) -> i32`
- `ver_v1(doc: bytes_view) -> i32`
- `secs_lo_v1(doc: bytes_view) -> i32`
- `secs_hi_v1(doc: bytes_view) -> i32`
- `nanos_v1(doc: bytes_view) -> i32`

All accessors are **total**: for malformed docs, they return deterministic defaults and/or `SPEC_ERR_*` via `err_code_v1`.
