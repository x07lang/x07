#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use core::str;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_bytes {
    pub ptr: *mut u8,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ev_result_bytes_payload {
    pub ok: ev_bytes,
    pub err: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_result_bytes {
    pub tag: u32, // 1 = ok, 0 = err
    pub payload: ev_result_bytes_payload,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ev_result_i32_payload {
    pub ok: u32,  // i32 bits
    pub err: u32, // error code
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_result_i32 {
    pub tag: u32, // 1 = ok, 0 = err
    pub payload: ev_result_i32_payload,
}

extern "C" {
    // Provided by the X07 runtime (generated C).
    fn ev_bytes_alloc(len: u32) -> ev_bytes;

    // Must not return.
    fn ev_trap(code: i32) -> !;
}

// Keep these in sync with the header.
const EV_TRAP_MATH_BADLEN_F64: i32 = 9100;
const EV_TRAP_MATH_BADLEN_U32: i32 = 9101;
const EV_TRAP_MATH_INTERNAL: i32 = 9102;

// SPEC_ERR space (math package) — mirrored from docs/math/math-v1.md.
const SPEC_ERR_F64_PARSE_INVALID: u32 = 40001;
const SPEC_ERR_F64_PARSE_OVERFLOW: u32 = 40002;
const SPEC_ERR_F64_PARSE_UNDERFLOW: u32 = 40003;
const SPEC_ERR_F64_TO_I32_NAN_INF: u32 = 40020;
const SPEC_ERR_F64_TO_I32_RANGE: u32 = 40021;

#[inline]
unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    core::slice::from_raw_parts(b.ptr as *const u8, b.len as usize)
}

#[inline]
unsafe fn bytes_as_mut_slice<'a>(b: ev_bytes) -> &'a mut [u8] {
    core::slice::from_raw_parts_mut(b.ptr, b.len as usize)
}

#[inline]
fn canonicalize_nan(x: f64) -> f64 {
    if x.is_nan() {
        // Canonical quiet NaN payload.
        f64::from_bits(0x7ff8_0000_0000_0000)
    } else {
        x
    }
}

#[inline]
unsafe fn read_f64_le(b: ev_bytes) -> f64 {
    if b.len != 8 {
        ev_trap(EV_TRAP_MATH_BADLEN_F64);
    }
    let s = bytes_as_slice(b);
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&s[0..8]);
    f64::from_bits(u64::from_le_bytes(arr))
}

#[inline]
unsafe fn write_f64_le(dst: ev_bytes, x: f64) {
    if dst.len != 8 {
        ev_trap(EV_TRAP_MATH_BADLEN_F64);
    }
    let s = bytes_as_mut_slice(dst);
    let bits = canonicalize_nan(x).to_bits().to_le_bytes();
    s[0..8].copy_from_slice(&bits);
}

#[inline]
unsafe fn alloc_bytes(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        // A conservative check: runtime should return exactly requested length.
        // If it doesn't, fail fast.
        ev_trap(EV_TRAP_MATH_INTERNAL);
    }
    out
}

#[inline]
fn ok_bytes(b: ev_bytes) -> ev_result_bytes {
    ev_result_bytes {
        tag: 1,
        payload: ev_result_bytes_payload { ok: b },
    }
}

#[inline]
fn err(code: u32) -> ev_result_bytes {
    ev_result_bytes {
        tag: 0,
        payload: ev_result_bytes_payload { err: code },
    }
}

#[inline]
fn ok_i32(x: i32) -> ev_result_i32 {
    ev_result_i32 {
        tag: 1,
        payload: ev_result_i32_payload { ok: x as u32 },
    }
}

#[inline]
fn err_i32(code: u32) -> ev_result_i32 {
    ev_result_i32 {
        tag: 0,
        payload: ev_result_i32_payload { err: code },
    }
}

// --- f64 arithmetic ---

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_add_v1(a: ev_bytes, b: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let y = read_f64_le(b);
    let out = alloc_bytes(8);
    write_f64_le(out, x + y);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_sub_v1(a: ev_bytes, b: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let y = read_f64_le(b);
    let out = alloc_bytes(8);
    write_f64_le(out, x - y);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_mul_v1(a: ev_bytes, b: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let y = read_f64_le(b);
    let out = alloc_bytes(8);
    write_f64_le(out, x * y);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_div_v1(a: ev_bytes, b: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let y = read_f64_le(b);
    let out = alloc_bytes(8);
    write_f64_le(out, x / y);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_neg_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, -x);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_abs_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, x.abs());
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_min_v1(a: ev_bytes, b: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let y = read_f64_le(b);
    let out = alloc_bytes(8);
    // IEEE 754 minNum behavior isn't the same as Rust's min when NaNs are involved.
    // We define v1 as: if either is NaN => canonical NaN; else smaller.
    let r = if x.is_nan() || y.is_nan() {
        f64::NAN
    } else {
        x.min(y)
    };
    write_f64_le(out, r);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_max_v1(a: ev_bytes, b: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let y = read_f64_le(b);
    let out = alloc_bytes(8);
    let r = if x.is_nan() || y.is_nan() {
        f64::NAN
    } else {
        x.max(y)
    };
    write_f64_le(out, r);
    out
}

// --- f64 libm-ish ---

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_sqrt_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::sqrt(x));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_sin_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::sin(x));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_cos_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::cos(x));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_exp_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::exp(x));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_ln_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::log(x));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_tan_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::tan(x));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_pow_v1(a: ev_bytes, b: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let y = read_f64_le(b);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::pow(x, y));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_atan2_v1(y: ev_bytes, x: ev_bytes) -> ev_bytes {
    let yy = read_f64_le(y);
    let xx = read_f64_le(x);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::atan2(yy, xx));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_floor_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::floor(x));
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_ceil_v1(a: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let out = alloc_bytes(8);
    write_f64_le(out, libm::ceil(x));
    out
}

// --- f64 cmp ---

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_cmp_v1(a: ev_bytes, b: ev_bytes) -> ev_bytes {
    let x = read_f64_le(a);
    let y = read_f64_le(b);

    let code: u32 = if x.is_nan() || y.is_nan() {
        3 // unordered
    } else if x < y {
        0
    } else if x > y {
        2
    } else {
        1
    };

    let out = alloc_bytes(4);
    if out.len != 4 {
        ev_trap(EV_TRAP_MATH_BADLEN_U32);
    }
    let s = bytes_as_mut_slice(out);
    s[0..4].copy_from_slice(&code.to_le_bytes());
    out
}

// --- f64 conversions ---

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_from_i32_v1(x: i32) -> ev_bytes {
    let out = alloc_bytes(8);
    write_f64_le(out, x as f64);
    out
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_to_i32_trunc_v1(x: ev_bytes) -> ev_result_i32 {
    let v = read_f64_le(x);
    if !v.is_finite() {
        return err_i32(SPEC_ERR_F64_TO_I32_NAN_INF);
    }

    let t = v.trunc();
    if t < (i32::MIN as f64) || t > (i32::MAX as f64) {
        return err_i32(SPEC_ERR_F64_TO_I32_RANGE);
    }

    ok_i32(t as i32)
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_to_bits_u64le_v1(x: ev_bytes) -> ev_bytes {
    if x.len != 8 {
        ev_trap(EV_TRAP_MATH_BADLEN_F64);
    }
    let out = alloc_bytes(8);
    let src = bytes_as_slice(x);
    let dst = bytes_as_mut_slice(out);
    dst.copy_from_slice(&src[0..8]);
    out
}

// --- f64 parse / fmt ---

#[inline]
fn trim_ascii_ws(mut s: &[u8]) -> &[u8] {
    while let Some((&c, rest)) = s.split_first() {
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
            s = rest;
        } else {
            break;
        }
    }
    while let Some((&c, rest)) = s.split_last() {
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
            s = rest;
        } else {
            break;
        }
    }
    s
}

#[inline]
fn parse_f64_lexical(s: &[u8]) -> Result<f64, u32> {
    let (value, used) = match lexical_core::parse_partial::<f64>(s) {
        Ok(v) => v,
        Err(err) => {
            return Err(match err {
                lexical_core::Error::Overflow(_) => SPEC_ERR_F64_PARSE_OVERFLOW,
                lexical_core::Error::Underflow(_) => SPEC_ERR_F64_PARSE_UNDERFLOW,
                _ => SPEC_ERR_F64_PARSE_INVALID,
            });
        }
    };
    if used != s.len() {
        return Err(SPEC_ERR_F64_PARSE_INVALID);
    }
    if !value.is_finite() {
        return Err(SPEC_ERR_F64_PARSE_OVERFLOW);
    }
    Ok(value)
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_parse_v1(s: ev_bytes) -> ev_result_bytes {
    let bs = bytes_as_slice(s);
    let bs = trim_ascii_ws(bs);

    if bs.is_empty() {
        return err(SPEC_ERR_F64_PARSE_INVALID);
    }

    let (sign, rest) = match bs.first().copied() {
        Some(b'+') => (1, &bs[1..]),
        Some(b'-') => (-1, &bs[1..]),
        _ => (1, bs),
    };
    if rest.is_empty() {
        return err(SPEC_ERR_F64_PARSE_INVALID);
    }

    let x = if rest.eq_ignore_ascii_case(b"nan") {
        f64::NAN
    } else if rest.eq_ignore_ascii_case(b"inf") {
        if sign < 0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    } else {
        match parse_f64_lexical(bs) {
            Ok(v) => v,
            Err(code) => return err(code),
        }
    };

    let out = alloc_bytes(8);
    write_f64_le(out, x);
    ok_bytes(out)
}

#[no_mangle]
pub unsafe extern "C" fn ev_math_f64_fmt_shortest_v1(x: ev_bytes) -> ev_bytes {
    let v = read_f64_le(x);
    // Avoid *all* Rust heap allocations: either use a constant string, or ryu's
    // stack buffer for finite numbers.
    let mut buf = ryu::Buffer::new();
    let s: &str = if v.is_nan() {
        "nan"
    } else if v == f64::INFINITY {
        "inf"
    } else if v == f64::NEG_INFINITY {
        "-inf"
    } else {
        buf.format_finite(v)
    };
    let s = s.strip_suffix(".0").unwrap_or(s);

    let out = alloc_bytes(s.len() as u32);
    let dst = bytes_as_mut_slice(out);
    dst.copy_from_slice(s.as_bytes());
    out
}
