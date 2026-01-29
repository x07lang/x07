# X07 external math package — v1 (normative)

This document pins the *v1* contract for `x07-ext-math`.

## Single canonical way (agent-facing)
Agents should only use:
- `std.math` (facade)
- `std.math.f64` (binary64 floats as **F64LE bytes**)
- `std.math.i32` (small integer helpers)

Anything under `std.math._internal.*` is forbidden for production agents.

## Float representation: F64LE
A float value is represented as **8 bytes little-endian** containing IEEE-754 **binary64** bits.

- Bytes `[0..4)` are the low u32 word (`lo`).
- Bytes `[4..8)` are the high u32 word (`hi`).
- In `hi`:
  - sign bit = bit 31
  - exponent = bits 20..30 (11 bits)
  - mantissa_hi = bits 0..19
- `lo` contains mantissa low 32 bits.

The package provides constructors and helpers so agents never build bit patterns manually.

## Determinism policy
- Pure bit-level operations (`is_nan_v1`, `total_cmp_v1`, constants) are implemented in x07AST and deterministic everywhere.
- Arithmetic/libm/text conversion functions are implemented by a pinned native backend (see [vendor guidance](vendor-v1.md)).

## Error codes (SPEC_ERR_MATH_*, numeric)
This package uses a dedicated error space that must not overlap OS/NET/DB errors:

- `SPEC_ERR_MATH_BASE = 40000`
- `SPEC_ERR_F64_BAD_LEN = 40010`
- `SPEC_ERR_F64_PARSE_INVALID = 40001`
- `SPEC_ERR_F64_PARSE_OVERFLOW = 40002`
- `SPEC_ERR_F64_PARSE_UNDERFLOW = 40003`
- `SPEC_ERR_F64_TO_I32_NAN_INF = 40020`
- `SPEC_ERR_F64_TO_I32_RANGE = 40021`

`parse_v1` returns `result_bytes`:
- OK payload: F64LE (8 bytes)
- ERR payload: error code (u32)

## Public API surface (v1)
### `std.math` (facade)
- Re-exports `std.math.f64` and `std.math.i32`.

### `std.math.f64` (F64LE floats)
Constructors:
- `zero_v1() -> bytes`
- `one_v1() -> bytes`
- `two_v1() -> bytes`
- `four_v1() -> bytes`
- `neg_zero_v1() -> bytes`
- `nan_v1() -> bytes` (canonical quiet NaN)
- `inf_v1() -> bytes`
- `neg_inf_v1() -> bytes`

Classification (pure, deterministic):
- `is_nan_v1(x: bytes_view) -> i32`
- `is_inf_v1(x: bytes_view) -> i32`
- `is_finite_v1(x: bytes_view) -> i32`
- `is_neg_zero_v1(x: bytes_view) -> i32`

Ordering (pure, deterministic):
- `total_cmp_v1(a: bytes_view, b: bytes_view) -> i32`  (-1/0/1) with NaNs ordered *after* all numbers
- `cmp_u8_v1(a: bytes_view, b: bytes_view) -> i32` (0/1/2) agent-friendly comparator

Arithmetic/libm (native backend, deterministic if pinned):
- `add_v1/sub_v1/mul_v1/div_v1`
- `abs_v1`, `neg_v1`, `min_v1`, `max_v1`
- `sqrt_v1`, `sin_v1`, `cos_v1`, `tan_v1`, `exp_v1`, `log_v1`, `pow_v1`, `atan2_v1`, `floor_v1`, `ceil_v1`

Text I/O (native backend, deterministic if pinned; see [vendor guidance](vendor-v1.md)):
- `fmt_shortest_v1(x: bytes_view) -> bytes`
- `parse_v1(s: bytes_view) -> result_bytes` (accepts optional sign, trims ASCII whitespace, accepts `nan` / `inf` / `-inf`)

Examples:
- `fmt_shortest_v1(from_i32_v1(42)) -> "42"`
- `fmt_shortest_v1(neg_zero_v1()) -> "-0"`

Conversions:
- `from_i32_v1(x: i32) -> bytes`
- `to_i32_trunc_v1(x: bytes_view) -> result_i32`
- `to_bits_u64le_v1(x: bytes_view) -> bytes`
- `from_bits_u64le_v1(bits_u64le: bytes_view) -> result_bytes`

### `std.math.i32` (minimal helpers)
- `abs_v1`, `min_v1`, `max_v1`, `clamp_v1`
