# Civil Time v1 (ext.time.civil)

This document pins **CivilDocV1**: a compact, deterministic binary encoding for a *civil* (calendar) timestamp:
year/month/day + hour/minute/second + nanos + UTC offset, plus the corresponding Unix seconds (u64 split into lo/hi).

The goal is to make civil time computations **agent-friendly**:
agents call helpers/accessors instead of hand-slicing offsets or inventing ad-hoc encodings.

## Module

- Module: `ext.time.civil`
- Version: `ext.time.civil.ver` returns `1`

## Encoding: CivilDocV1

All integers are **u32 little-endian** (`codec.write_u32_le` / `codec.read_u32_le`), interpreted as `i32` when needed.

### Error doc (length 9)

If the operation fails, functions return:

```
byte 0: tag = 0
bytes 1..4: err_code (u32 LE)
bytes 5..8: reserved = 0
```

### Ok doc (length 41)

If the operation succeeds, functions return:

```
byte 0: tag = 1

bytes  1.. 4: year_i32       (e.g. 2020)
bytes  5.. 8: month_i32      (1..12)
bytes  9..12: day_i32        (1..31)
bytes 13..16: hour_i32       (0..23)
bytes 17..20: minute_i32     (0..59)
bytes 21..24: second_i32     (0..59)
bytes 25..28: nanos_u32      (0..999_999_999)
bytes 29..32: offset_s_i32   (UTC offset seconds, typically multiple of 60)

bytes 33..36: unix_s_lo_u32  (lower 32 bits of Unix seconds)
bytes 37..40: unix_s_hi_u32  (upper 32 bits of Unix seconds)
```

**Interpretation:** the civil fields represent the local time in the zone defined by `offset_s`.
The Unix seconds represent the corresponding absolute instant.

## Error codes (CivilDocV1)

`ext.time.civil.err_code(v)` returns the code from an error doc.

- `code_invalid_doc = 1` — input doc malformed (wrong tag/length)
- `code_invalid_param = 2` — inputs out of range (month/day/hms/nanos/offset)
- `code_out_of_range = 3` — unix seconds outside supported range (currently `year >= 1970 and year <= 9999`)
- `code_before_epoch = 4` — conversion would require dates before 1970-01-01 (not supported in v1)

## API (v1)

### Constructors / converters

- `ext.time.civil.from_unix_s_v1(unix_lo, unix_hi, offset_s, nanos) -> CivilDocV1`
- `ext.time.civil.from_ts_v1(ts_doc_view) -> CivilDocV1`
  - `ts_doc_view` must be an **ok** `ext.time.rfc3339.parse_v1` document (same bytes layout as `rfc3339-v1.md`).
- `ext.time.civil.make_v1(year, month, day, hour, minute, second, nanos, offset_s) -> CivilDocV1`

### Converting CivilDocV1 back to a timestamp doc

- `ext.time.civil.to_ts_v1(civ_doc_view) -> bytes`
  - Returns an `ext.time.rfc3339`-compatible timestamp doc with:
    - `tzid_len = 0` and no tzid bytes
    - `unix_s` / `offset_s` / `nanos` filled from the civil doc
  - On error, returns an error timestamp doc (tag 0 + civil err_code).

### Accessors (no manual slicing)

All accessors take a `bytes_view`.

- `ext.time.civil.is_err(v)` / `ext.time.civil.is_ok(v)`
- `ext.time.civil.err_code(v)`
- `ext.time.civil.year_i32(v)`
- `ext.time.civil.month_i32(v)`
- `ext.time.civil.day_i32(v)`
- `ext.time.civil.hour_i32(v)`
- `ext.time.civil.minute_i32(v)`
- `ext.time.civil.second_i32(v)`
- `ext.time.civil.nanos_u32(v)`
- `ext.time.civil.offset_s(v)`
- `ext.time.civil.unix_s_lo_u32(v)`
- `ext.time.civil.unix_s_hi_u32(v)`

### Derived helpers

- `ext.time.civil.weekday_iso_v1(v) -> i32`
  - Returns ISO weekday in range `1..7` (Mon=1 … Sun=7).
  - Returns `0` if `v` is an error doc.

## Determinism

All functions are pure (no OS world), deterministic, and return bytes that must match this encoding exactly.
