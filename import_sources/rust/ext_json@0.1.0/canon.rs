// JSON canonicalization for X07 (x07import-compatible Rust subset).
//
// Produces a whitespace-free JSON encoding with object members sorted by their
// raw key bytes (as encoded in the source JSON).

fn _is_ws(c: i32) -> bool {
    c == 32 || c == 9 || c == 10 || c == 13
}

fn _is_digit(c: i32) -> bool {
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return true;
        }
    }
    false
}

fn _is_hex_digit(c: i32) -> bool {
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return true;
        }
    }
    if ge_u(c, 65) {
        if lt_u(c, 71) {
            return true;
        }
    }
    if ge_u(c, 97) {
        if lt_u(c, 103) {
            return true;
        }
    }
    false
}

fn _skip_ws(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    let mut i = start;
    for _ in start..n {
        if lt_u(i, n) {
            if _is_ws(view_get_u8(b, i)) {
                i = i + 1;
            }
        }
    }
    i
}

// start: position after the opening quote.
// Returns index of the closing quote on success, or -1 on error.
fn _string_end_or_err(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    let mut i = start;
    for _ in start..n {
        if lt_u(i, n) {
            let c = view_get_u8(b, i);
            if c == 34 {
                return i;
            }
            if c == 92 {
                if !lt_u(i + 1, n) {
                    return 0 - 1;
                }
                let esc = view_get_u8(b, i + 1);
                if esc == 34
                    || esc == 92
                    || esc == 47
                    || esc == 98
                    || esc == 102
                    || esc == 110
                    || esc == 114
                    || esc == 116
                {
                    i = i + 2;
                } else if esc == 117 {
                    if !lt_u(i + 5, n) {
                        return 0 - 1;
                    }
                    if !_is_hex_digit(view_get_u8(b, i + 2))
                        || !_is_hex_digit(view_get_u8(b, i + 3))
                        || !_is_hex_digit(view_get_u8(b, i + 4))
                        || !_is_hex_digit(view_get_u8(b, i + 5))
                    {
                        return 0 - 1;
                    }
                    i = i + 6;
                } else {
                    return 0 - 1;
                }
            } else if lt_u(c, 32) {
                return 0 - 1;
            } else {
                i = i + 1;
            }
        }
    }
    0 - 1
}

fn _skip_string(b: BytesView, off: i32) -> i32 {
    let n = view_len(b);
    if ge_u(off, n) {
        return 0 - 1;
    }
    if view_get_u8(b, off) != 34 {
        return 0 - 1;
    }
    let end_quote = _string_end_or_err(b, off + 1);
    if end_quote < 0 {
        0 - 1
    } else {
        end_quote + 1
    }
}

// Returns end offset (exclusive), or -1 on error.
fn _skip_number(b: BytesView, off: i32) -> i32 {
    let n = view_len(b);
    let mut i = off;

    if ge_u(i, n) {
        return 0 - 1;
    }

    if view_get_u8(b, i) == 45 {
        i = i + 1;
        if ge_u(i, n) {
            return 0 - 1;
        }
    }

    if ge_u(i, n) {
        return 0 - 1;
    }

    let d0 = view_get_u8(b, i);
    if !_is_digit(d0) {
        return 0 - 1;
    }

    if d0 == 48 {
        i = i + 1;
        if lt_u(i, n) && _is_digit(view_get_u8(b, i)) {
            return 0 - 1;
        }
    } else {
        i = i + 1;
        let mut done = false;
        for _ in 0..n {
            if !done {
                if ge_u(i, n) {
                    done = true;
                } else if _is_digit(view_get_u8(b, i)) {
                    i = i + 1;
                } else {
                    done = true;
                }
            }
        }
    }

    // Fractional part
    if lt_u(i, n) && view_get_u8(b, i) == 46 {
        i = i + 1;
        if ge_u(i, n) {
            return 0 - 1;
        }
        if !_is_digit(view_get_u8(b, i)) {
            return 0 - 1;
        }
        i = i + 1;
        let mut done = false;
        for _ in 0..n {
            if !done {
                if ge_u(i, n) {
                    done = true;
                } else if _is_digit(view_get_u8(b, i)) {
                    i = i + 1;
                } else {
                    done = true;
                }
            }
        }
    }

    // Exponent part
    if lt_u(i, n) {
        let e = view_get_u8(b, i);
        if e == 101 || e == 69 {
            i = i + 1;
            if ge_u(i, n) {
                return 0 - 1;
            }
            let sign = view_get_u8(b, i);
            if sign == 43 || sign == 45 {
                i = i + 1;
                if ge_u(i, n) {
                    return 0 - 1;
                }
            }
            if !_is_digit(view_get_u8(b, i)) {
                return 0 - 1;
            }
            i = i + 1;
            let mut done = false;
            for _ in 0..n {
                if !done {
                    if ge_u(i, n) {
                        done = true;
                    } else if _is_digit(view_get_u8(b, i)) {
                        i = i + 1;
                    } else {
                        done = true;
                    }
                }
            }
        }
    }

    i
}

fn _skip_lit(b: BytesView, off: i32, c0: i32, c1: i32, c2: i32, c3: i32, c4: i32) -> i32 {
    let n = view_len(b);
    if c4 < 0 {
        // 4-byte literal
        if !lt_u(off + 3, n) {
            return 0 - 1;
        }
        if view_get_u8(b, off) != c0 {
            return 0 - 1;
        }
        if view_get_u8(b, off + 1) != c1 {
            return 0 - 1;
        }
        if view_get_u8(b, off + 2) != c2 {
            return 0 - 1;
        }
        if view_get_u8(b, off + 3) != c3 {
            return 0 - 1;
        }
        return off + 4;
    }

    // 5-byte literal
    if !lt_u(off + 4, n) {
        return 0 - 1;
    }
    if view_get_u8(b, off) != c0 {
        return 0 - 1;
    }
    if view_get_u8(b, off + 1) != c1 {
        return 0 - 1;
    }
    if view_get_u8(b, off + 2) != c2 {
        return 0 - 1;
    }
    if view_get_u8(b, off + 3) != c3 {
        return 0 - 1;
    }
    if view_get_u8(b, off + 4) != c4 {
        return 0 - 1;
    }
    off + 5
}

fn _parse_value(b: BytesView, off: i32) -> Bytes {
    let n = view_len(b);
    let i = _skip_ws(b, off);
    if ge_u(i, n) {
        return bytes_alloc(0);
    }
    let c = view_get_u8(b, i);
    if c == 123 {
        return _parse_object(b, i);
    }
    if c == 91 {
        return _parse_array(b, i);
    }
    if c == 34 {
        return _parse_string(b, i);
    }
    if c == 116 {
        return _parse_lit(b, i, 116, 114, 117, 101, 0 - 1);
    }
    if c == 102 {
        return _parse_lit(b, i, 102, 97, 108, 115, 101);
    }
    if c == 110 {
        return _parse_lit(b, i, 110, 117, 108, 108, 0 - 1);
    }
    if c == 45 || _is_digit(c) {
        return _parse_number(b, i);
    }
    bytes_alloc(0)
}

fn _skip_value(b: BytesView, off: i32) -> i32 {
    let n = view_len(b);
    let i = _skip_ws(b, off);
    if ge_u(i, n) {
        return 0 - 1;
    }
    let c = view_get_u8(b, i);
    if c == 123 {
        return _skip_object(b, i);
    }
    if c == 91 {
        return _skip_array(b, i);
    }
    if c == 34 {
        return _skip_string(b, i);
    }
    if c == 116 {
        return _skip_lit(b, i, 116, 114, 117, 101, 0 - 1);
    }
    if c == 102 {
        return _skip_lit(b, i, 102, 97, 108, 115, 101);
    }
    if c == 110 {
        return _skip_lit(b, i, 110, 117, 108, 108, 0 - 1);
    }
    if c == 45 || _is_digit(c) {
        return _skip_number(b, i);
    }
    0 - 1
}

fn _skip_array(b: BytesView, off: i32) -> i32 {
    let n = view_len(b);
    let mut i = _skip_ws(b, off + 1);
    if ge_u(i, n) {
        return 0 - 1;
    }
    if view_get_u8(b, i) == 93 {
        return i + 1;
    }

    for _ in 0..(n + 1) {
        let end = _skip_value(b, i);
        if end < 0 {
            return 0 - 1;
        }
        i = _skip_ws(b, end);
        if ge_u(i, n) {
            return 0 - 1;
        }
        let c = view_get_u8(b, i);
        if c == 44 {
            i = _skip_ws(b, i + 1);
        } else if c == 93 {
            return i + 1;
        } else {
            return 0 - 1;
        }
    }
    0 - 1
}

fn _skip_object(b: BytesView, off: i32) -> i32 {
    let n = view_len(b);
    let mut i = _skip_ws(b, off + 1);
    if ge_u(i, n) {
        return 0 - 1;
    }
    if view_get_u8(b, i) == 125 {
        return i + 1;
    }

    for _ in 0..(n + 1) {
        if ge_u(i, n) {
            return 0 - 1;
        }
        if view_get_u8(b, i) != 34 {
            return 0 - 1;
        }
        let key_end = _string_end_or_err(b, i + 1);
        if key_end < 0 {
            return 0 - 1;
        }
        i = key_end + 1;
        i = _skip_ws(b, i);
        if ge_u(i, n) {
            return 0 - 1;
        }
        if view_get_u8(b, i) != 58 {
            return 0 - 1;
        }
        i = _skip_ws(b, i + 1);
        let end = _skip_value(b, i);
        if end < 0 {
            return 0 - 1;
        }
        i = _skip_ws(b, end);
        if ge_u(i, n) {
            return 0 - 1;
        }
        let c = view_get_u8(b, i);
        if c == 44 {
            i = _skip_ws(b, i + 1);
        } else if c == 125 {
            return i + 1;
        } else {
            return 0 - 1;
        }
    }
    0 - 1
}

fn _parse_string(b: BytesView, off: i32) -> Bytes {
    let end_quote = _string_end_or_err(b, off + 1);
    if end_quote < 0 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(b, off, (end_quote - off) + 1))
}

fn _parse_number(b: BytesView, off: i32) -> Bytes {
    let end = _skip_number(b, off);
    if end < 0 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(b, off, end - off))
}

fn _parse_lit(b: BytesView, off: i32, c0: i32, c1: i32, c2: i32, c3: i32, c4: i32) -> Bytes {
    let end = _skip_lit(b, off, c0, c1, c2, c3, c4);
    if end < 0 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(b, off, end - off))
}

fn _cmp_member_key(members: BytesView, offsets: BytesView, a: i32, b: i32) -> i32 {
    let a_off = codec_read_u32_le(offsets, a * 4);
    let b_off = codec_read_u32_le(offsets, b * 4);
    let a_len = codec_read_u32_le(members, a_off);
    let b_len = codec_read_u32_le(members, b_off);
    let a_start = a_off + 4;
    let b_start = b_off + 4;

    let min_len = if lt_u(a_len, b_len) { a_len } else { b_len };
    for i in 0..min_len {
        let ac = view_get_u8(members, a_start + i);
        let bc = view_get_u8(members, b_start + i);
        if ac < bc {
            return 0 - 1;
        }
        if ac > bc {
            return 1;
        }
    }

    if a_len < b_len {
        0 - 1
    } else if a_len > b_len {
        1
    } else {
        0
    }
}

fn _emit_member_kv(members: BytesView, offsets: BytesView, idx: i32, out: VecU8) -> VecU8 {
    let off = codec_read_u32_le(offsets, idx * 4);
    let key_len = codec_read_u32_le(members, off);
    let key_start = off + 4;
    let val_len_off = key_start + key_len;
    let val_len = codec_read_u32_le(members, val_len_off);
    let val_start = val_len_off + 4;

    let mut o = out;
    o = vec_u8_push(o, 34);
    o = vec_u8_extend_bytes_range(o, members, key_start, key_len);
    o = vec_u8_push(o, 34);
    o = vec_u8_push(o, 58);
    o = vec_u8_extend_bytes_range(o, members, val_start, val_len);
    o
}

fn _emit_object_sorted(members: BytesView, offsets: BytesView, count: i32, out: VecU8) -> VecU8 {
    let mut o = out;
    o = vec_u8_push(o, 123);

    let mut used = bytes_alloc(count);
    let mut first = 1;
    for _ in 0..count {
        let mut best = 0 - 1;
        for j in 0..count {
            if bytes_get_u8(used, j) == 1 {
                // already used
            } else if best < 0 {
                best = j;
            } else if _cmp_member_key(members, offsets, j, best) < 0 {
                best = j;
            }
        }

        used = bytes_set_u8(used, best, 1);
        if first == 0 {
            o = vec_u8_push(o, 44);
        } else {
            first = 0;
        }
        o = _emit_member_kv(members, offsets, best, o);
    }

    o = vec_u8_push(o, 125);
    o
}

fn _parse_object(b: BytesView, off: i32) -> Bytes {
    let n = view_len(b);
    let mut i = _skip_ws(b, off + 1);
    if ge_u(i, n) {
        return bytes_alloc(0);
    }

    if view_get_u8(b, i) == 125 {
        let mut out = vec_u8_with_capacity(2);
        out = vec_u8_push(out, 123);
        out = vec_u8_push(out, 125);
        return vec_u8_into_bytes(out);
    }

    let mut members = vec_u8_with_capacity(0);
    let mut offsets = vec_u8_with_capacity(0);
    let mut count = 0;

    for _ in 0..(n + 1) {
        if ge_u(i, n) {
            return bytes_alloc(0);
        }
        if view_get_u8(b, i) != 34 {
            return bytes_alloc(0);
        }

        let key_start = i + 1;
        let key_end = _string_end_or_err(b, key_start);
        if key_end < 0 {
            return bytes_alloc(0);
        }
        let key_len = key_end - key_start;

        i = key_end + 1;
        i = _skip_ws(b, i);
        if ge_u(i, n) {
            return bytes_alloc(0);
        }
        if view_get_u8(b, i) != 58 {
            return bytes_alloc(0);
        }
        i = i + 1;

        let value = _parse_value(b, i);
        let val_len = bytes_len(value);
        if val_len == 0 {
            return bytes_alloc(0);
        }
        let val_end = _skip_value(b, i);
        if val_end < 0 {
            return bytes_alloc(0);
        }
        i = _skip_ws(b, val_end);

        let m_off = vec_u8_len(members);
        offsets = vec_u8_extend_bytes(offsets, codec_write_u32_le(m_off));

        members = vec_u8_extend_bytes(members, codec_write_u32_le(key_len));
        members = vec_u8_extend_bytes_range(members, b, key_start, key_len);

        members = vec_u8_extend_bytes(members, codec_write_u32_le(val_len));
        members = vec_u8_extend_bytes(members, value);

        count = count + 1;

        if ge_u(i, n) {
            return bytes_alloc(0);
        }
        let c = view_get_u8(b, i);
        if c == 44 {
            i = _skip_ws(b, i + 1);
        } else if c == 125 {
            let members_b = vec_u8_into_bytes(members);
            let offsets_b = vec_u8_into_bytes(offsets);
            let mut out = vec_u8_with_capacity(view_len(b));
            out = _emit_object_sorted(bytes_view(members_b), bytes_view(offsets_b), count, out);
            return vec_u8_into_bytes(out);
        } else {
            return bytes_alloc(0);
        }
    }

    bytes_alloc(0)
}

fn _parse_array(b: BytesView, off: i32) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(n);
    out = vec_u8_push(out, 91);
    let mut i = _skip_ws(b, off + 1);
    if ge_u(i, n) {
        return bytes_alloc(0);
    }
    if view_get_u8(b, i) == 93 {
        out = vec_u8_push(out, 93);
        return vec_u8_into_bytes(out);
    }

    let mut first = 1;
    for _ in 0..(n + 1) {
        if first == 0 {
            out = vec_u8_push(out, 44);
        } else {
            first = 0;
        }

        let value = _parse_value(b, i);
        if bytes_len(value) == 0 {
            return bytes_alloc(0);
        }
        out = vec_u8_extend_bytes(out, value);
        let end = _skip_value(b, i);
        if end < 0 {
            return bytes_alloc(0);
        }
        i = _skip_ws(b, end);
        if ge_u(i, n) {
            return bytes_alloc(0);
        }
        let c = view_get_u8(b, i);
        if c == 44 {
            i = _skip_ws(b, i + 1);
        } else if c == 93 {
            out = vec_u8_push(out, 93);
            return vec_u8_into_bytes(out);
        } else {
            return bytes_alloc(0);
        }
    }

    bytes_alloc(0)
}

pub fn canonicalize(b: BytesView) -> Bytes {
    let end = _skip_value(b, 0);
    if end < 0 {
        return bytes_alloc(0);
    }
    let end2 = _skip_ws(b, end);
    if end2 != view_len(b) {
        return bytes_alloc(0);
    }
    _parse_value(b, 0)
}
