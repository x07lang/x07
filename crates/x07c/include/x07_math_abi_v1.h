#ifndef X07_MATH_ABI_V1_H
#define X07_MATH_ABI_V1_H

// X07 Math Backend ABI (v1)
//
// This header is *pinned* and is intended to be included by:
//  - the generated C produced by x07c (call sites)
//  - the native math backend library implementation (libx07_math.a)
//
// Design goals:
//  - Stable C ABI across platforms (Linux/macOS)
//  - Minimal surface: only what the math builtins need
//  - No dependency on Rust layout defaults (everything is explicit)

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// --- Core value types (must match x07c's runtime ABI) ---

// bytes = (ptr, len) where len is the number of bytes.
// The runtime owns the allocation pointed to by ptr.
//
// IMPORTANT: the math backend must allocate outputs using ev_bytes_alloc().
// Do NOT malloc/free directly.
typedef struct {
  uint8_t* ptr;
  uint32_t len;
} ev_bytes;

// result_bytes = either Ok(bytes) or Err(i32)
// tag: 1 = Ok, 0 = Err
typedef struct {
  uint32_t tag;
  union {
    ev_bytes ok;
    uint32_t err;
  } payload;
} ev_result_bytes;

// result_i32 = either Ok(i32) or Err(u32)
// tag: 1 = Ok, 0 = Err
// ok: i32 bits stored in a u32 slot (must match x07c's result_i32_t layout)
typedef struct {
  uint32_t tag;
  union {
    uint32_t ok;
    uint32_t err;
  } payload;
} ev_result_i32;

// --- Runtime hooks required by the math backend ---

// Allocate a bytes buffer of exactly `len` bytes in the X07 runtime allocator.
// This must be provided by the generated runtime.
ev_bytes ev_bytes_alloc(uint32_t len);

// Trap (non-recoverable). This must be provided by the generated runtime.
// The math backend uses traps only for impossible-in-well-typed-programs invariants
// (e.g. wrong-length f64 bytes).
void ev_trap(int32_t code);

// --- Trap codes (reserved range for math backend) ---

// These codes must not overlap other runtime trap codes.
// If you already have a global trap catalog, map these into it.
enum {
  EV_TRAP_MATH_BADLEN_F64 = 9100,  // input bytes len != 8
  EV_TRAP_MATH_BADLEN_U32 = 9101,  // input bytes len != 4
  EV_TRAP_MATH_INTERNAL   = 9102
};

// --- Encoding conventions ---

// f64 values are encoded as 8 bytes, little-endian IEEE-754 binary64 bit patterns.
// u32 values are encoded as 4 bytes, little-endian.
//
// All functions below:
//  - TRAP with EV_TRAP_MATH_BADLEN_* if any input byte string has the wrong length.
//  - Allocate output bytes via ev_bytes_alloc().

// --- f64 arithmetic ---

// (math.f64.add_v1 a b) -> f64_bytes
// (math.f64.sub_v1 a b) -> f64_bytes
// (math.f64.mul_v1 a b) -> f64_bytes
// (math.f64.div_v1 a b) -> f64_bytes

ev_bytes ev_math_f64_add_v1(ev_bytes a, ev_bytes b);
ev_bytes ev_math_f64_sub_v1(ev_bytes a, ev_bytes b);
ev_bytes ev_math_f64_mul_v1(ev_bytes a, ev_bytes b);
ev_bytes ev_math_f64_div_v1(ev_bytes a, ev_bytes b);

// --- f64 unary ops ---

ev_bytes ev_math_f64_neg_v1(ev_bytes x);
ev_bytes ev_math_f64_abs_v1(ev_bytes x);
ev_bytes ev_math_f64_tan_v1(ev_bytes x);
ev_bytes ev_math_f64_sqrt_v1(ev_bytes x);
ev_bytes ev_math_f64_floor_v1(ev_bytes x);
ev_bytes ev_math_f64_ceil_v1(ev_bytes x);

// --- f64 transcendentals ---

ev_bytes ev_math_f64_sin_v1(ev_bytes x);
ev_bytes ev_math_f64_cos_v1(ev_bytes x);
ev_bytes ev_math_f64_exp_v1(ev_bytes x);
ev_bytes ev_math_f64_ln_v1(ev_bytes x);
ev_bytes ev_math_f64_pow_v1(ev_bytes x, ev_bytes y);
ev_bytes ev_math_f64_atan2_v1(ev_bytes y, ev_bytes x);
ev_bytes ev_math_f64_min_v1(ev_bytes a, ev_bytes b);
ev_bytes ev_math_f64_max_v1(ev_bytes a, ev_bytes b);

// --- f64 comparisons (returns i32 as 4-byte LE) ---

// (math.f64.cmp_v1 a b) -> u32_le:
//   0 = less, 1 = equal, 2 = greater, 3 = unordered (NaN involved)
ev_bytes ev_math_f64_cmp_v1(ev_bytes a, ev_bytes b);

// --- f64 text interop ---

// (math.f64.fmt_shortest_v1 x) -> bytes (UTF-8, ASCII subset)
// Canonical formatting:
//  - no leading '+',
//  - 'nan', 'inf', '-inf' for non-finite,
//  - otherwise shortest round-trippable decimal.
ev_bytes ev_math_f64_fmt_shortest_v1(ev_bytes x);

// (math.f64.parse_v1 s) -> result_bytes
// Accepts:
//  - optional leading/trailing ASCII spaces,
//  - optional leading sign,
//  - decimal, optional exponent.
// Returns Err(code) on invalid syntax or overflow.
ev_result_bytes ev_math_f64_parse_v1(ev_bytes s);

// --- f64 conversions ---

// Convert i32 to f64 (exact for all i32 values), returned as f64 bytes.
ev_bytes ev_math_f64_from_i32_v1(int32_t x);

// Truncate f64 toward zero and convert to i32.
// Returns Err(SPEC_ERR_F64_TO_I32_*) on NaN/Inf/out-of-range.
ev_result_i32 ev_math_f64_to_i32_trunc_v1(ev_bytes x);

// Return the raw IEEE-754 binary64 bits as u64le bytes (same 8 bytes as input).
ev_bytes ev_math_f64_to_bits_u64le_v1(ev_bytes x);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_MATH_ABI_V1_H
