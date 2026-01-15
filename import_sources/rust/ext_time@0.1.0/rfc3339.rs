fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

pub fn _u64_add_lo(a_lo: i32, b_lo: i32) -> i32 {
    a_lo + b_lo
}

pub fn _u64_add_hi(a_lo: i32, a_hi: i32, b_lo: i32, b_hi: i32, sum_lo: i32) -> i32 {
    let carry = if lt_u(sum_lo, a_lo) { 1 } else { 0 };
    a_hi + b_hi + carry
}

pub fn _u64_sub_lo(a_lo: i32, b_lo: i32) -> i32 {
    a_lo - b_lo
}

pub fn _u64_sub_hi(a_lo: i32, a_hi: i32, b_lo: i32, b_hi: i32, diff_lo: i32) -> i32 {
    let borrow = if lt_u(a_lo, b_lo) { 1 } else { 0 };
    a_hi - b_hi - borrow
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

pub fn _u64_mul_u32_lo(lo: i32, hi: i32, k: i32) -> i32 {
    lo * k
}

pub fn _u64_mul_u32_hi(lo: i32, hi: i32, k: i32) -> i32 {
    (hi * k) + _mul_u32_hi(lo, k)
}

pub fn _u64_div_mod_u32_small(lo: i32, hi: i32, d: i32) -> Bytes {
    if d <= 0 {
        return _make_err(3);
    }
    let mut q_lo = 0;
    let mut q_hi = 0;
    let mut r = 0;

    for idx in 0..64 {
        let bit = 63 - idx;
        let bitv = if lt_u(bit, 32) {
            (lo >> bit) & 1
        } else {
            (hi >> (bit - 32)) & 1
        };

        r = (r << 1) | bitv;
        if r >= d {
            r = r - d;
            if lt_u(bit, 32) {
                q_lo = q_lo | (1 << bit);
            } else {
                q_hi = q_hi | (1 << (bit - 32));
            }
        }
    }

    let mut out = vec_u8_with_capacity(13);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(q_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(q_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(r));
    vec_u8_into_bytes(out)
}

fn _digit(c: i32) -> i32 {
    if ge_u(c, 48) && lt_u(c, 58) {
        return c - 48;
    }
    0 - 1
}

fn _parse_2d(b: BytesView, off: i32) -> i32 {
    let d0 = _digit(view_get_u8(b, off));
    let d1 = _digit(view_get_u8(b, off + 1));
    if d0 < 0 || d1 < 0 {
        return 0 - 1;
    }
    (d0 * 10) + d1
}

fn _parse_4d(b: BytesView, off: i32) -> i32 {
    let d0 = _digit(view_get_u8(b, off));
    let d1 = _digit(view_get_u8(b, off + 1));
    let d2 = _digit(view_get_u8(b, off + 2));
    let d3 = _digit(view_get_u8(b, off + 3));
    if d0 < 0 || d1 < 0 || d2 < 0 || d3 < 0 {
        return 0 - 1;
    }
    (((d0 * 10) + d1) * 100) + ((d2 * 10) + d3)
}

pub fn _is_leap_year(year: i32) -> bool {
    if (year % 4) != 0 {
        return false;
    }
    if (year % 100) != 0 {
        return true;
    }
    (year % 400) == 0
}

pub fn _days_in_month(year: i32, month: i32) -> i32 {
    if month == 1 {
        return 31;
    }
    if month == 2 {
        return if _is_leap_year(year) { 29 } else { 28 };
    }
    if month == 3 {
        return 31;
    }
    if month == 4 {
        return 30;
    }
    if month == 5 {
        return 31;
    }
    if month == 6 {
        return 30;
    }
    if month == 7 {
        return 31;
    }
    if month == 8 {
        return 31;
    }
    if month == 9 {
        return 30;
    }
    if month == 10 {
        return 31;
    }
    if month == 11 {
        return 30;
    }
    if month == 12 {
        return 31;
    }
    0
}

pub fn _days_since_epoch_1970(year: i32, month: i32, day: i32) -> i32 {
    let mut days = 0;
    let mut y = 1970;
    for _ in 1970..year {
        days = days + if _is_leap_year(y) { 366 } else { 365 };
        y = y + 1;
    }
    for m in 1..month {
        days = days + _days_in_month(year, m);
    }
    days + (day - 1)
}

fn _pow10(n: i32) -> i32 {
    if n == 0 {
        return 1;
    }
    if n == 1 {
        return 10;
    }
    if n == 2 {
        return 100;
    }
    if n == 3 {
        return 1000;
    }
    if n == 4 {
        return 10000;
    }
    if n == 5 {
        return 100000;
    }
    if n == 6 {
        return 1000000;
    }
    if n == 7 {
        return 10000000;
    }
    if n == 8 {
        return 100000000;
    }
    1000000000
}

fn _push_2d(mut out: VecU8, x: i32) -> VecU8 {
    let tens = x / 10;
    let ones = x % 10;
    out = vec_u8_push(out, 48 + tens);
    vec_u8_push(out, 48 + ones)
}

fn _push_4d(mut out: VecU8, x: i32) -> VecU8 {
    let d0 = x / 1000;
    let d1 = (x / 100) % 10;
    let d2 = (x / 10) % 10;
    let d3 = x % 10;
    out = vec_u8_push(out, 48 + d0);
    out = vec_u8_push(out, 48 + d1);
    out = vec_u8_push(out, 48 + d2);
    vec_u8_push(out, 48 + d3)
}

fn _push_nanos_9(mut out: VecU8, nanos: i32) -> VecU8 {
    let mut rem = nanos;
    let d0 = rem / 100000000;
    rem = rem % 100000000;
    let d1 = rem / 10000000;
    rem = rem % 10000000;
    let d2 = rem / 1000000;
    rem = rem % 1000000;
    let d3 = rem / 100000;
    rem = rem % 100000;
    let d4 = rem / 10000;
    rem = rem % 10000;
    let d5 = rem / 1000;
    rem = rem % 1000;
    let d6 = rem / 100;
    rem = rem % 100;
    let d7 = rem / 10;
    let d8 = rem % 10;
    out = vec_u8_push(out, 48 + d0);
    out = vec_u8_push(out, 48 + d1);
    out = vec_u8_push(out, 48 + d2);
    out = vec_u8_push(out, 48 + d3);
    out = vec_u8_push(out, 48 + d4);
    out = vec_u8_push(out, 48 + d5);
    out = vec_u8_push(out, 48 + d6);
    out = vec_u8_push(out, 48 + d7);
    vec_u8_push(out, 48 + d8)
}

pub fn is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn err_code(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn unix_s_u32(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn offset_s(doc: BytesView) -> i32 {
    if view_len(doc) < 9 {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }
    codec_read_u32_le(doc, 5)
}

pub fn nanos_u32(doc: BytesView) -> i32 {
    if view_len(doc) < 13 {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }
    codec_read_u32_le(doc, 9)
}

pub fn tzid(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 17 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    let len = codec_read_u32_le(doc, 13);
    if lt_u(n, 17 + len) {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 17, len))
}

fn _parse_unix_s_u64(ts: BytesView) -> Bytes {
    // Codes:
    //  1 = SPEC_ERR_TS_INVALID
    //  2 = SPEC_ERR_TS_RANGE
    //  3 = SPEC_ERR_TS_BAD_TZID
    //  4 = SPEC_ERR_TS_TRUNCATED
    let n = view_len(ts);
    if lt_u(n, 20) {
        return _make_err(4);
    }

    if view_get_u8(ts, 4) != 45 || view_get_u8(ts, 7) != 45 {
        return _make_err(1);
    }
    let t = view_get_u8(ts, 10);
    if t != 84 && t != 116 {
        return _make_err(1);
    }
    if view_get_u8(ts, 13) != 58 || view_get_u8(ts, 16) != 58 {
        return _make_err(1);
    }

    let year = _parse_4d(ts, 0);
    let month = _parse_2d(ts, 5);
    let day = _parse_2d(ts, 8);
    let hour = _parse_2d(ts, 11);
    let minute = _parse_2d(ts, 14);
    let second = _parse_2d(ts, 17);
    if year < 0
        || month < 0
        || day < 0
        || hour < 0
        || minute < 0
        || second < 0
    {
        return _make_err(1);
    }

    if month < 1 || month > 12 {
        return _make_err(2);
    }
    let dim = _days_in_month(year, month);
    if day < 1 || day > dim {
        return _make_err(2);
    }
    if hour > 23 || minute > 59 {
        return _make_err(2);
    }
    if second == 60 {
        return _make_err(1);
    }
    if second > 59 {
        return _make_err(2);
    }

    let mut i = 19;
    let mut nanos = 0;
    if lt_u(i, n) && view_get_u8(ts, i) == 46 {
        i = i + 1;
        if !lt_u(i, n) {
            return _make_err(4);
        }

        let mut digits = 0;
        let mut done = 0;
        for _ in 0..9 {
            if done == 0 {
                if lt_u(i, n) {
                    let d = _digit(view_get_u8(ts, i));
                    if d >= 0 {
                        nanos = (nanos * 10) + d;
                        digits = digits + 1;
                        i = i + 1;
                    } else {
                        done = 1;
                    }
                } else {
                    done = 1;
                }
            }
        }
        if digits == 0 {
            return _make_err(1);
        }
        if done == 0 {
            if lt_u(i, n) {
                let d = _digit(view_get_u8(ts, i));
                if d >= 0 {
                    return _make_err(1);
                }
            }
        }
        nanos = nanos * _pow10(9 - digits);
    }

    if !lt_u(i, n) {
        return _make_err(4);
    }

    let mut offset_s = 0;
    let mut tz_end = 0;
    let tz = view_get_u8(ts, i);
    if tz == 90 || tz == 122 {
        tz_end = i + 1;
        offset_s = 0;
    } else if tz == 43 || tz == 45 {
        if lt_u(i + 5, n) == false {
            return _make_err(4);
        }
        let hh = _parse_2d(ts, i + 1);
        let mm = _parse_2d(ts, i + 4);
        if hh < 0 || mm < 0 {
            return _make_err(1);
        }
        if view_get_u8(ts, i + 3) != 58 {
            return _make_err(1);
        }
        if hh > 23 || mm > 59 {
            return _make_err(2);
        }
        let abs = (hh * 3600) + (mm * 60);
        offset_s = if tz == 45 { 0 - abs } else { abs };
        tz_end = i + 6;
    } else {
        return _make_err(1);
    }

    let mut tzid_len = 0;
    let mut tzid_start = 0;
    if lt_u(tz_end, n) {
        if view_get_u8(ts, tz_end) != 91 {
            return _make_err(3);
        }
        tzid_start = tz_end + 1;
        let mut found = 0 - 1;
        for j in tzid_start..n {
            if found < 0 {
                if view_get_u8(ts, j) == 93 {
                    found = j;
                }
            }
        }
        if found < 0 {
            return _make_err(3);
        }
        tzid_len = found - tzid_start;
        if tzid_len <= 0 {
            return _make_err(3);
        }
        if found + 1 != n {
            return _make_err(3);
        }
    } else if tz_end != n {
        return _make_err(4);
    }

    if nanos >= 1000000000 {
        return _make_err(1);
    }

    if year < 1970 {
        return _make_err(2);
    }

    let days = _days_since_epoch_1970(year, month, day);
    if days < 0 {
        return _make_err(2);
    }

    let sec_of_day = ((hour * 3600) + (minute * 60)) + second;
    let local_lo = _u64_mul_u32_lo(days, 0, 86400);
    let local_hi = _u64_mul_u32_hi(days, 0, 86400);
    let sum_lo = _u64_add_lo(local_lo, sec_of_day);
    let sum_hi = _u64_add_hi(local_lo, local_hi, sec_of_day, 0, sum_lo);

    let mut unix_lo = 0;
    let mut unix_hi = 0;
    if offset_s >= 0 {
        if sum_hi == 0 && lt_u(sum_lo, offset_s) {
            return _make_err(2);
        }
        let diff_lo = _u64_sub_lo(sum_lo, offset_s);
        let diff_hi = _u64_sub_hi(sum_lo, sum_hi, offset_s, 0, diff_lo);
        unix_lo = diff_lo;
        unix_hi = diff_hi;
    } else {
        let off = 0 - offset_s;
        let add_lo = _u64_add_lo(sum_lo, off);
        let add_hi = _u64_add_hi(sum_lo, sum_hi, off, 0, add_lo);
        unix_lo = add_lo;
        unix_hi = add_hi;
    }

    let mut out = vec_u8_with_capacity(21 + tzid_len);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(unix_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(unix_hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(offset_s));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(nanos));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(tzid_len));
    if tzid_len > 0 {
        out = vec_u8_extend_bytes_range(out, ts, tzid_start, tzid_len);
    }
    vec_u8_into_bytes(out)
}

pub fn unix_s_i64_hi(doc: BytesView) -> i32 {
    let n = view_len(doc);
    if lt_u(n, 17) {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }

    let tzid_len = codec_read_u32_le(doc, 13);
    if tzid_len < 0 {
        return 0;
    }
    if lt_u(n, 17 + tzid_len) {
        return 0;
    }
    if lt_u(n, 21 + tzid_len) {
        return 0;
    }
    codec_read_u32_le(doc, 17 + tzid_len)
}

pub fn parse_v1(ts: BytesView) -> Bytes {
    let doc = _parse_unix_s_u64(ts);
    if is_err(bytes_view(doc)) {
        return doc;
    }
    let docv = bytes_view(doc);
    if view_len(docv) < 21 {
        return _make_err(4);
    }

    let unix_lo = codec_read_u32_le(docv, 1);
    let unix_hi = codec_read_u32_le(docv, 5);
    let offset_s = codec_read_u32_le(docv, 9);
    let nanos = codec_read_u32_le(docv, 13);
    let tzid_len = codec_read_u32_le(docv, 17);
    if tzid_len < 0 {
        return _make_err(4);
    }
    if lt_u(view_len(docv), 21 + tzid_len) {
        return _make_err(4);
    }

    let mut out = vec_u8_with_capacity(21 + tzid_len);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(unix_lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(offset_s));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(nanos));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(tzid_len));
    if tzid_len > 0 {
        out = vec_u8_extend_bytes_range(out, docv, 21, tzid_len);
    }
    out = vec_u8_extend_bytes(out, codec_write_u32_le(unix_hi));
    vec_u8_into_bytes(out)
}

fn _format_unix_s_u64(
    unix_lo: i32,
    unix_hi: i32,
    offset_s: i32,
    nanos: i32,
    tzid: BytesView,
) -> Bytes {
    // Codes:
    //  1 = SPEC_ERR_TS_INVALID
    //  2 = SPEC_ERR_TS_RANGE
    //  3 = SPEC_ERR_TS_BAD_TZID
    //  4 = SPEC_ERR_TS_TRUNCATED
    if nanos < 0 || nanos >= 1000000000 {
        return _make_err(1);
    }
    let offset_abs = if offset_s < 0 { 0 - offset_s } else { offset_s };
    if offset_abs >= 86400 {
        return _make_err(1);
    }
    if (offset_abs % 60) != 0 {
        return _make_err(1);
    }

    let tzid_len = view_len(tzid);
    if tzid_len > 0 {
        for i in 0..tzid_len {
            let c = view_get_u8(tzid, i);
            if c == 91 || c == 93 {
                return _make_err(3);
            }
        }
    }

    let mut local_lo = 0;
    let mut local_hi = 0;
    if offset_s >= 0 {
        let sum_lo = _u64_add_lo(unix_lo, offset_s);
        let sum_hi = _u64_add_hi(unix_lo, unix_hi, offset_s, 0, sum_lo);
        local_lo = sum_lo;
        local_hi = sum_hi;
    } else {
        let off = 0 - offset_s;
        if unix_hi == 0 && lt_u(unix_lo, off) {
            return _make_err(2);
        }
        let diff_lo = _u64_sub_lo(unix_lo, off);
        let diff_hi = _u64_sub_hi(unix_lo, unix_hi, off, 0, diff_lo);
        local_lo = diff_lo;
        local_hi = diff_hi;
    }

    let div = _u64_div_mod_u32_small(local_lo, local_hi, 86400);
    if is_err(bytes_view(div)) {
        return div;
    }
    let divv = bytes_view(div);
    let days_lo = codec_read_u32_le(divv, 1);
    let days_hi = codec_read_u32_le(divv, 5);
    let sod = codec_read_u32_le(divv, 9);
    if days_hi != 0 {
        return _make_err(2);
    }

    let hour = sod / 3600;
    let rem = sod % 3600;
    let minute = rem / 60;
    let second = rem % 60;

    let mut year = 1970;
    let mut day_rem = days_lo;
    let mut done = 0;
    for _ in 0..10000 {
        if done == 0 {
            let diy = if _is_leap_year(year) { 366 } else { 365 };
            if lt_u(day_rem, diy) {
                done = 1;
            } else {
                day_rem = day_rem - diy;
                year = year + 1;
            }
        }
    }
    if done == 0 || year > 9999 {
        return _make_err(2);
    }

    let mut month = 1;
    done = 0;
    for _ in 0..12 {
        if done == 0 {
            let dim = _days_in_month(year, month);
            if lt_u(day_rem, dim) {
                done = 1;
            } else {
                day_rem = day_rem - dim;
                month = month + 1;
            }
        }
    }
    if done == 0 || month > 12 {
        return _make_err(2);
    }
    let day = day_rem + 1;

    let extra = if nanos == 0 { 0 } else { 10 };
    let tz_extra = if tzid_len == 0 { 0 } else { 2 + tzid_len };
    let cap = 20 + extra + if offset_s == 0 { 1 } else { 6 } + tz_extra;
    let mut out = vec_u8_with_capacity(cap);
    out = _push_4d(out, year);
    out = vec_u8_push(out, 45);
    out = _push_2d(out, month);
    out = vec_u8_push(out, 45);
    out = _push_2d(out, day);
    out = vec_u8_push(out, 84);
    out = _push_2d(out, hour);
    out = vec_u8_push(out, 58);
    out = _push_2d(out, minute);
    out = vec_u8_push(out, 58);
    out = _push_2d(out, second);
    if nanos != 0 {
        out = vec_u8_push(out, 46);
        out = _push_nanos_9(out, nanos);
    }

    if offset_s == 0 {
        out = vec_u8_push(out, 90);
    } else {
        let sign = if offset_s < 0 { 45 } else { 43 };
        let abs = if offset_s < 0 { 0 - offset_s } else { offset_s };
        let hh = abs / 3600;
        let mm = (abs % 3600) / 60;
        out = vec_u8_push(out, sign);
        out = _push_2d(out, hh);
        out = vec_u8_push(out, 58);
        out = _push_2d(out, mm);
    }

    if tzid_len > 0 {
        out = vec_u8_push(out, 91);
        out = vec_u8_extend_bytes_range(out, tzid, 0, tzid_len);
        out = vec_u8_push(out, 93);
    }

    vec_u8_into_bytes(out)
}

pub fn format_v1(
    unix_s_lo: i32,
    unix_s_hi: i32,
    offset_s: i32,
    nanos_u32: i32,
    tzid: Bytes,
) -> Bytes {
    _format_unix_s_u64(unix_s_lo, unix_s_hi, offset_s, nanos_u32, bytes_view(tzid))
}

pub fn format_doc_v1(doc: BytesView) -> Bytes {
    if is_err(doc) {
        return bytes_alloc(0);
    }
    format_v1(
        unix_s_u32(doc),
        unix_s_i64_hi(doc),
        offset_s(doc),
        nanos_u32(doc),
        tzid(doc),
    )
}
