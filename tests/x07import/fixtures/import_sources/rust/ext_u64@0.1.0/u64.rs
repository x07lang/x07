// Minimal 64-bit helpers for x07AST codegen.
//
// Representation: a "u64/i64 value" is carried as a pair of u32 halves:
//   - lo: low 32 bits (u32 stored in i32)
//   - hi: high 32 bits (u32 stored in i32)
//
// This module exports "lo" and "hi" helpers separately because x07AST has no tuple returns.

pub fn add_lo(a_lo: i32, b_lo: i32) -> i32 {
    a_lo + b_lo
}

pub fn add_hi(a_lo: i32, a_hi: i32, b_lo: i32, b_hi: i32, sum_lo: i32) -> i32 {
    let carry = if lt_u(sum_lo, a_lo) { 1 } else { 0 };
    a_hi + b_hi + carry
}

pub fn sub_lo(a_lo: i32, b_lo: i32) -> i32 {
    a_lo - b_lo
}

pub fn sub_hi(a_lo: i32, a_hi: i32, b_lo: i32, b_hi: i32, diff_lo: i32) -> i32 {
    let borrow = if lt_u(a_lo, b_lo) { 1 } else { 0 };
    a_hi - b_hi - borrow
}

pub fn from_i32_lo(x: i32) -> i32 {
    x
}

pub fn from_i32_hi(x: i32) -> i32 {
    if x < 0 { 0 - 1 } else { 0 }
}

pub fn is_neg_hi(hi: i32) -> i32 {
    if ge_u(hi, (-2147483647) - 1) { 1 } else { 0 }
}

pub fn shl_lo(lo: i32, hi: i32, n: i32) -> i32 {
    if n == 0 {
        lo
    } else if lt_u(n, 32) {
        lo << n
    } else if lt_u(n, 64) {
        0
    } else {
        0
    }
}

pub fn shl_hi(lo: i32, hi: i32, n: i32) -> i32 {
    if n == 0 {
        hi
    } else if lt_u(n, 32) {
        (hi << n) | (lo >> (32 - n))
    } else if lt_u(n, 64) {
        lo << (n - 32)
    } else {
        0
    }
}

pub fn shr_u_lo(lo: i32, hi: i32, n: i32) -> i32 {
    if n == 0 {
        lo
    } else if lt_u(n, 32) {
        (lo >> n) | (hi << (32 - n))
    } else if lt_u(n, 64) {
        hi >> (n - 32)
    } else {
        0
    }
}

pub fn shr_u_hi(lo: i32, hi: i32, n: i32) -> i32 {
    if n == 0 {
        hi
    } else if lt_u(n, 32) {
        hi >> n
    } else if lt_u(n, 64) {
        0
    } else {
        0
    }
}

pub fn rotr_lo(lo: i32, hi: i32, n: i32) -> i32 {
    let k = n & 63;
    if k == 0 {
        lo
    } else if lt_u(k, 32) {
        (lo >> k) | (hi << (32 - k))
    } else {
        let kk = k - 32;
        (hi >> kk) | (lo << (32 - kk))
    }
}

pub fn rotr_hi(lo: i32, hi: i32, n: i32) -> i32 {
    let k = n & 63;
    if k == 0 {
        hi
    } else if lt_u(k, 32) {
        (hi >> k) | (lo << (32 - k))
    } else {
        let kk = k - 32;
        (lo >> kk) | (hi << (32 - kk))
    }
}

fn _mul_u32_hi(a: i32, b: i32) -> i32 {
    // Same algorithm shape as std.prng._mul_u32_hi, but standalone.
    let a0 = a & 65535;
    let a1 = (a >> 16) & 65535;
    let b0 = b & 65535;
    let b1 = (b >> 16) & 65535;

    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;

    let mid = p01 + p10;
    let carry_mid = if lt_u(mid, p01) { 1 } else { 0 };

    let mid_low = mid & 65535;
    let mid_high = (mid >> 16) + (carry_mid << 16);

    let sum_low = p00 + (mid_low << 16);
    let carry0 = if lt_u(sum_low, p00) { 1 } else { 0 };

    p11 + mid_high + carry0
}

pub fn mul_u32_lo(a: i32, b: i32) -> i32 {
    a * b
}

pub fn mul_u32_hi(a: i32, b: i32) -> i32 {
    _mul_u32_hi(a, b)
}

pub fn mul_u64_u32_lo(lo: i32, hi: i32, k: i32) -> i32 {
    lo * k
}

pub fn mul_u64_u32_hi(lo: i32, hi: i32, k: i32) -> i32 {
    (hi * k) + _mul_u32_hi(lo, k)
}

pub fn to_bytes_le(lo: i32, hi: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(8);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(hi));
    vec_u8_into_bytes(out)
}

pub fn read_lo_le(b: BytesView, off: i32) -> i32 {
    codec_read_u32_le(b, off)
}

pub fn read_hi_le(b: BytesView, off: i32) -> i32 {
    codec_read_u32_le(b, off + 4)
}
