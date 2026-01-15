// YAML subset parser for X07 (x07import-compatible Rust subset).
//
// Supported:
// - Comments starting with '#' (line start or after space/tab)
// - Block mappings: key: value, key: (nested), key: (null)
// - Block sequences: - value, - (nested), - (null)
// - Scalars: null/~, true/false, JSON-number syntax, or string
//
// Not supported (returns error):
// - Tabs in indentation
// - Multiline scalars (| or >)
// - Flow style ({...} or [...])
// - Multiple documents
//
// Output format:
//   Error:   [0x00][u32_le code][u32_le msg_len=0]
//   Success: [0x01][json_bytes...]

fn _push_u32_le(out: VecU8, val: i32) -> VecU8 {
    let mut o = out;
    o = vec_u8_push(o, val & 255);
    o = vec_u8_push(o, (val >> 8) & 255);
    o = vec_u8_push(o, (val >> 16) & 255);
    o = vec_u8_push(o, (val >> 24) & 255);
    o
}

fn _read_u32_le(b: BytesView, offset: i32) -> i32 {
    let b0 = view_get_u8(b, offset);
    let b1 = view_get_u8(b, offset + 1);
    let b2 = view_get_u8(b, offset + 2);
    let b3 = view_get_u8(b, offset + 3);
    b0 + (b1 << 8) + (b2 << 16) + (b3 << 24)
}

fn _make_error_code(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = _push_u32_le(out, code);
    out = _push_u32_le(out, 0);
    vec_u8_into_bytes(out)
}

fn _empty_bytes() -> Bytes {
    vec_u8_into_bytes(vec_u8_with_capacity(0))
}

fn _find_newline(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    let mut i = start;
    let mut found = 0 - 1;
    for _ in start..n {
        if lt_u(i, n) {
            if found < 0 {
                if view_get_u8(b, i) == 10 {
                    found = i;
                }
                i = i + 1;
            }
        }
    }
    if found < 0 {
        n
    } else {
        found
    }
}

fn _trim_end_space_tab(b: BytesView, start: i32, end: i32) -> i32 {
    if !lt_u(start, end) {
        return end;
    }
    let mut r = end;
    let mut i = end - 1;
    let mut done = false;
    for _ in start..end {
        if !done {
            if i < start {
                done = true;
            } else {
                let c = view_get_u8(b, i);
                if c == 32 || c == 9 || c == 13 {
                    r = i;
                    i = i - 1;
                } else {
                    done = true;
                }
            }
        }
    }
    r
}

fn _count_indent_or_err(b: BytesView, line_start: i32, line_end: i32) -> i32 {
    let mut i = line_start;
    let mut count = 0;
    for _ in line_start..line_end {
        if lt_u(i, line_end) {
            let c = view_get_u8(b, i);
            if c == 32 {
                count = count + 1;
                i = i + 1;
            } else if c == 9 {
                return 0 - 1;
            } else {
                return count;
            }
        }
    }
    count
}

fn _skip_blank_and_comment_lines(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    let mut i = start;
    for _ in 0..n {
        if ge_u(i, n) {
            return n;
        }
        let line_end = _find_newline(b, i);
        let trimmed_end = _trim_end_space_tab(b, i, line_end);
        let mut j = i;
        let mut done = false;
        for _ in i..trimmed_end {
            if !done {
                if lt_u(j, trimmed_end) {
                    let c = view_get_u8(b, j);
                    if c == 32 || c == 9 || c == 13 {
                        j = j + 1;
                    } else {
                        done = true;
                    }
                } else {
                    done = true;
                }
            }
        }
        if !lt_u(j, trimmed_end) {
            i = line_end + 1;
        } else if view_get_u8(b, j) == 35 {
            i = line_end + 1;
        } else {
            return i;
        }
    }
    n
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

fn _hex_digit(n: i32) -> i32 {
    if lt_u(n, 10) {
        48 + n
    } else {
        87 + n
    }
}

fn _append_json_escaped(out: VecU8, c: i32) -> VecU8 {
    let mut o = out;
    if c == 34 {
        o = vec_u8_push(o, 92);
        o = vec_u8_push(o, 34);
        return o;
    }
    if c == 92 {
        o = vec_u8_push(o, 92);
        o = vec_u8_push(o, 92);
        return o;
    }
    if c == 8 {
        o = vec_u8_push(o, 92);
        o = vec_u8_push(o, 98);
        return o;
    }
    if c == 12 {
        o = vec_u8_push(o, 92);
        o = vec_u8_push(o, 102);
        return o;
    }
    if c == 10 {
        o = vec_u8_push(o, 92);
        o = vec_u8_push(o, 110);
        return o;
    }
    if c == 13 {
        o = vec_u8_push(o, 92);
        o = vec_u8_push(o, 114);
        return o;
    }
    if c == 9 {
        o = vec_u8_push(o, 92);
        o = vec_u8_push(o, 116);
        return o;
    }
    if lt_u(c, 32) {
        let hi = _hex_digit((c >> 4) & 15);
        let lo = _hex_digit(c & 15);
        o = vec_u8_push(o, 92);
        o = vec_u8_push(o, 117);
        o = vec_u8_push(o, 48);
        o = vec_u8_push(o, 48);
        o = vec_u8_push(o, hi);
        o = vec_u8_push(o, lo);
        return o;
    }
    vec_u8_push(o, c)
}

fn _json_string_end_or_err(b: BytesView, start: i32, end: i32) -> i32 {
    let mut i = start;
    for _ in start..end {
        if lt_u(i, end) {
            let c = view_get_u8(b, i);
            if c == 34 {
                return i;
            }
            if c == 92 {
                if !lt_u(i + 1, end) {
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
                    if !lt_u(i + 5, end) {
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

fn _is_json_number_range(b: BytesView, start: i32, end: i32) -> bool {
    let mut i = start;
    if !lt_u(i, end) {
        return false;
    }
    if view_get_u8(b, i) == 45 {
        i = i + 1;
        if !lt_u(i, end) {
            return false;
        }
    }
    if !lt_u(i, end) {
        return false;
    }
    let d0 = view_get_u8(b, i);
    if !_is_digit(d0) {
        return false;
    }
    if d0 == 48 {
        i = i + 1;
        if lt_u(i, end) && _is_digit(view_get_u8(b, i)) {
            return false;
        }
    } else {
        i = i + 1;
        let mut done = false;
        for _ in 0..end {
            if !done {
                if !lt_u(i, end) {
                    done = true;
                } else if _is_digit(view_get_u8(b, i)) {
                    i = i + 1;
                } else {
                    done = true;
                }
            }
        }
    }
    if lt_u(i, end) && view_get_u8(b, i) == 46 {
        i = i + 1;
        if !lt_u(i, end) {
            return false;
        }
        if !_is_digit(view_get_u8(b, i)) {
            return false;
        }
        i = i + 1;
        let mut done = false;
        for _ in 0..end {
            if !done {
                if !lt_u(i, end) {
                    done = true;
                } else if _is_digit(view_get_u8(b, i)) {
                    i = i + 1;
                } else {
                    done = true;
                }
            }
        }
    }
    if lt_u(i, end) {
        let e = view_get_u8(b, i);
        if e == 101 || e == 69 {
            i = i + 1;
            if !lt_u(i, end) {
                return false;
            }
            let sign = view_get_u8(b, i);
            if sign == 43 || sign == 45 {
                i = i + 1;
                if !lt_u(i, end) {
                    return false;
                }
            }
            if !_is_digit(view_get_u8(b, i)) {
                return false;
            }
            i = i + 1;
            let mut done = false;
            for _ in 0..end {
                if !done {
                    if !lt_u(i, end) {
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
    i == end
}

fn _scalar_json(b: BytesView, start: i32, end: i32) -> Bytes {
    if !lt_u(start, end) {
        let mut o = vec_u8_with_capacity(4);
        o = vec_u8_push(o, 110);
        o = vec_u8_push(o, 117);
        o = vec_u8_push(o, 108);
        o = vec_u8_push(o, 108);
        return vec_u8_into_bytes(o);
    }

    let first = view_get_u8(b, start);
    if first == 124 || first == 62 {
        return _empty_bytes();
    }
    if first == 123 || first == 91 {
        return _empty_bytes();
    }

    if first == 34 {
        let quote = _json_string_end_or_err(b, start + 1, end);
        if quote < 0 {
            return _empty_bytes();
        }
        if quote != end - 1 {
            return _empty_bytes();
        }
        let mut out = vec_u8_with_capacity(end - start);
        out = vec_u8_push(out, 34);
        out = vec_u8_extend_bytes_range(out, b, start + 1, (end - 1) - (start + 1));
        out = vec_u8_push(out, 34);
        return vec_u8_into_bytes(out);
    }

    if first == 39 {
        // YAML single-quoted: '' is an escaped '
        if view_get_u8(b, end - 1) != 39 {
            return _empty_bytes();
        }
        let mut out = vec_u8_with_capacity(end - start + 2);
        out = vec_u8_push(out, 34);
        let mut i = start + 1;
        for _ in start..end {
            if lt_u(i, end - 1) {
                let c = view_get_u8(b, i);
                if c == 39 {
                    if lt_u(i + 1, end - 1) && view_get_u8(b, i + 1) == 39 {
                        out = _append_json_escaped(out, 39);
                        i = i + 2;
                    } else {
                        return _empty_bytes();
                    }
                } else {
                    out = _append_json_escaped(out, c);
                    i = i + 1;
                }
            }
        }
        out = vec_u8_push(out, 34);
        return vec_u8_into_bytes(out);
    }

    // Plain scalar: strip spaces/tabs already done by caller.
    let len = end - start;

    if len == 1 && view_get_u8(b, start) == 126 {
        let mut o = vec_u8_with_capacity(4);
        o = vec_u8_push(o, 110);
        o = vec_u8_push(o, 117);
        o = vec_u8_push(o, 108);
        o = vec_u8_push(o, 108);
        return vec_u8_into_bytes(o);
    }

    if len == 4 {
        if view_get_u8(b, start) == 110
            && view_get_u8(b, start + 1) == 117
            && view_get_u8(b, start + 2) == 108
            && view_get_u8(b, start + 3) == 108
        {
            let mut o = vec_u8_with_capacity(4);
            o = vec_u8_push(o, 110);
            o = vec_u8_push(o, 117);
            o = vec_u8_push(o, 108);
            o = vec_u8_push(o, 108);
            return vec_u8_into_bytes(o);
        }
        if view_get_u8(b, start) == 116
            && view_get_u8(b, start + 1) == 114
            && view_get_u8(b, start + 2) == 117
            && view_get_u8(b, start + 3) == 101
        {
            let mut o = vec_u8_with_capacity(4);
            o = vec_u8_push(o, 116);
            o = vec_u8_push(o, 114);
            o = vec_u8_push(o, 117);
            o = vec_u8_push(o, 101);
            return vec_u8_into_bytes(o);
        }
    }

    if len == 5 {
        if view_get_u8(b, start) == 102
            && view_get_u8(b, start + 1) == 97
            && view_get_u8(b, start + 2) == 108
            && view_get_u8(b, start + 3) == 115
            && view_get_u8(b, start + 4) == 101
        {
            let mut o = vec_u8_with_capacity(5);
            o = vec_u8_push(o, 102);
            o = vec_u8_push(o, 97);
            o = vec_u8_push(o, 108);
            o = vec_u8_push(o, 115);
            o = vec_u8_push(o, 101);
            return vec_u8_into_bytes(o);
        }
    }

    if _is_json_number_range(b, start, end) {
        let mut out = vec_u8_with_capacity(len);
        out = vec_u8_extend_bytes_range(out, b, start, len);
        return vec_u8_into_bytes(out);
    }

    let mut out = vec_u8_with_capacity(len + 2);
    out = vec_u8_push(out, 34);
    for i in start..end {
        out = _append_json_escaped(out, view_get_u8(b, i));
    }
    out = vec_u8_push(out, 34);
    vec_u8_into_bytes(out)
}

fn _comment_cut(b: BytesView, start: i32, end: i32) -> i32 {
    let mut i = start;
    let mut cut = end;
    let mut done = false;
    for _ in start..end {
        if !done {
            if lt_u(i, end) {
                let c = view_get_u8(b, i);
                if c == 35 {
                    if i == start {
                        cut = i;
                        done = true;
                    } else {
                        let prev = view_get_u8(b, i - 1);
                        if prev == 32 || prev == 9 {
                            cut = i;
                            done = true;
                        }
                    }
                }
                i = i + 1;
            } else {
                done = true;
            }
        }
    }
    cut
}

fn _parse_node(b: BytesView, start: i32, indent: i32) -> Bytes {
    let n = view_len(b);
    let mut pos = _skip_blank_and_comment_lines(b, start);
    if ge_u(pos, n) {
        let mut out = vec_u8_with_capacity(9);
        out = vec_u8_push(out, 1);
        out = _push_u32_le(out, n);
        out = vec_u8_push(out, 110);
        out = vec_u8_push(out, 117);
        out = vec_u8_push(out, 108);
        out = vec_u8_push(out, 108);
        return vec_u8_into_bytes(out);
    }

    let line_end = _find_newline(b, pos);
    let trimmed_end0 = _trim_end_space_tab(b, pos, line_end);
    let ind = _count_indent_or_err(b, pos, trimmed_end0);
    if ind < 0 {
        return _make_error_code(2);
    }
    if ind < indent || ind > indent {
        return _make_error_code(2);
    }

    let content_start = pos + indent;
    if !lt_u(content_start, trimmed_end0) {
        return _make_error_code(1);
    }

    let c0 = view_get_u8(b, content_start);
    if c0 == 45 {
        return _parse_seq(b, pos, indent);
    }

    // If the line contains a ':' before any comment, parse as map; otherwise scalar.
    let cut0 = _comment_cut(b, content_start, trimmed_end0);
    let mut has_colon = false;
    for i in content_start..cut0 {
        if view_get_u8(b, i) == 58 {
            has_colon = true;
        }
    }
    if has_colon {
        return _parse_map(b, pos, indent);
    }

    let scalar_end0 = _trim_end_space_tab(b, content_start, cut0);
    let scalar = _scalar_json(b, content_start, scalar_end0);
    if bytes_len(scalar) == 0 {
        return _make_error_code(5);
    }
    let scalar_v = bytes_view(scalar);
    let scalar_n = view_len(scalar_v);
    let mut out = vec_u8_with_capacity(5 + scalar_n);
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, line_end + 1);
    out = vec_u8_extend_bytes_range(out, scalar_v, 0, scalar_n);
    vec_u8_into_bytes(out)
}

fn _emit_json_string_key(out: VecU8, b: BytesView, start: i32, end: i32) -> VecU8 {
    let mut o = out;
    o = vec_u8_push(o, 34);
    for i in start..end {
        o = _append_json_escaped(o, view_get_u8(b, i));
    }
    o = vec_u8_push(o, 34);
    o
}

fn _emit_null(out: VecU8) -> VecU8 {
    let mut o = out;
    o = vec_u8_push(o, 110);
    o = vec_u8_push(o, 117);
    o = vec_u8_push(o, 108);
    o = vec_u8_push(o, 108);
    o
}

fn _parse_map(b: BytesView, start: i32, indent: i32) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(128);
    out = vec_u8_push(out, 123);
    let mut first = 1;
    let mut pos = start;
    let mut done = false;
    for _ in 0..n {
        if !done {
            pos = _skip_blank_and_comment_lines(b, pos);
            if ge_u(pos, n) {
                done = true;
            } else {
                let line_end = _find_newline(b, pos);
                let trimmed_end0 = _trim_end_space_tab(b, pos, line_end);
                let ind = _count_indent_or_err(b, pos, trimmed_end0);
                if ind < 0 {
                    return _make_error_code(2);
                }
                if ind < indent {
                    done = true;
                } else if ind > indent {
                    return _make_error_code(2);
                } else {
                    let key_start = pos + indent;
                    if !lt_u(key_start, trimmed_end0) {
                        return _make_error_code(3);
                    }

                    let cut0 = _comment_cut(b, key_start, trimmed_end0);

                    // Parse key up to ':' (allow spaces before ':').
                    let mut k = key_start;
                    let mut key_end = key_start;
                    let mut saw_key = false;
                    let mut done_key = false;
                    for _ in key_start..cut0 {
                        if !done_key {
                            if lt_u(k, cut0) {
                                let c = view_get_u8(b, k);
                                if c == 58 || c == 32 || c == 9 {
                                    done_key = true;
                                } else {
                                    saw_key = true;
                                    k = k + 1;
                                    key_end = k;
                                }
                            } else {
                                done_key = true;
                            }
                        }
                    }
                    if !saw_key {
                        return _make_error_code(4);
                    }

                    // Skip spaces/tabs to ':'
                    let mut p = key_end;
                    let mut found_colon = false;
                    for _ in p..cut0 {
                        if !found_colon {
                            if lt_u(p, cut0) {
                                let c = view_get_u8(b, p);
                                if c == 58 {
                                    found_colon = true;
                                } else if c == 32 || c == 9 {
                                    p = p + 1;
                                } else {
                                    return _make_error_code(3);
                                }
                            } else {
                                return _make_error_code(3);
                            }
                        }
                    }
                    if !found_colon {
                        return _make_error_code(3);
                    }
                    let mut val_start = p + 1;
                    // Skip spaces/tabs before value
                    let mut done_ws = false;
                    for _ in val_start..cut0 {
                        if !done_ws {
                            if lt_u(val_start, cut0) {
                                let c = view_get_u8(b, val_start);
                                if c == 32 || c == 9 {
                                    val_start = val_start + 1;
                                } else {
                                    done_ws = true;
                                }
                            } else {
                                done_ws = true;
                            }
                        }
                    }

                    if first == 0 {
                        out = vec_u8_push(out, 44);
                    }
                    out = _emit_json_string_key(out, b, key_start, key_end);
                    out = vec_u8_push(out, 58);

                    if lt_u(val_start, cut0) {
                        let val_cut = _comment_cut(b, val_start, trimmed_end0);
                        let val_end = _trim_end_space_tab(b, val_start, val_cut);
                        let val = _scalar_json(b, val_start, val_end);
                        if bytes_len(val) == 0 {
                            return _make_error_code(5);
                        }
                        let valv = bytes_view(val);
                        let valn = view_len(valv);
                        out = vec_u8_extend_bytes_range(out, valv, 0, valn);
                        pos = line_end + 1;
                    } else {
                        let next = _skip_blank_and_comment_lines(b, line_end + 1);
                        if ge_u(next, n) {
                            out = _emit_null(out);
                            pos = next;
                        } else {
                            let next_line_end = _find_newline(b, next);
                            let next_trim_end = _trim_end_space_tab(b, next, next_line_end);
                            let next_ind = _count_indent_or_err(b, next, next_trim_end);
                            if next_ind < 0 {
                                return _make_error_code(2);
                            }
                            if next_ind <= indent {
                                out = _emit_null(out);
                                pos = line_end + 1;
                            } else {
                                let sub = _parse_node(b, next, next_ind);
                                if bytes_len(sub) < 5 {
                                    return _make_error_code(1);
                                }
                                if bytes_get_u8(sub, 0) == 0 {
                                    return sub;
                                }
                                let subv = bytes_view(sub);
                                let end_pos = _read_u32_le(subv, 1);
                                let subn = view_len(subv);
                                out = vec_u8_extend_bytes_range(out, subv, 5, subn - 5);
                                pos = end_pos;
                            }
                        }
                    }

                    first = 0;
                }
            }
        }
    }
    out = vec_u8_push(out, 125);
    let json_b = vec_u8_into_bytes(out);
    let jsonv = bytes_view(json_b);
    let jsonn = view_len(jsonv);
    let mut res = vec_u8_with_capacity(5 + jsonn);
    res = vec_u8_push(res, 1);
    res = _push_u32_le(res, pos);
    res = vec_u8_extend_bytes_range(res, jsonv, 0, jsonn);
    vec_u8_into_bytes(res)
}

fn _parse_seq(b: BytesView, start: i32, indent: i32) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(128);
    out = vec_u8_push(out, 91);
    let mut first = 1;
    let mut pos = start;
    let mut done = false;
    for _ in 0..n {
        if !done {
            pos = _skip_blank_and_comment_lines(b, pos);
            if ge_u(pos, n) {
                done = true;
            } else {
                let line_end = _find_newline(b, pos);
                let trimmed_end0 = _trim_end_space_tab(b, pos, line_end);
                let ind = _count_indent_or_err(b, pos, trimmed_end0);
                if ind < 0 {
                    return _make_error_code(2);
                }
                if ind < indent {
                    done = true;
                } else if ind > indent {
                    return _make_error_code(2);
                } else {
                    let dash = pos + indent;
                    if !lt_u(dash, trimmed_end0) || view_get_u8(b, dash) != 45 {
                        return _make_error_code(1);
                    }
                    let mut item_start = dash + 1;
                    if lt_u(item_start, trimmed_end0) {
                        let c = view_get_u8(b, item_start);
                        if c == 32 || c == 9 {
                            item_start = item_start + 1;
                        }
                    }

                    if first == 0 {
                        out = vec_u8_push(out, 44);
                    }

                    let cut0 = _comment_cut(b, item_start, trimmed_end0);
                    let item_end = _trim_end_space_tab(b, item_start, cut0);
                    if lt_u(item_start, item_end) {
                        let val = _scalar_json(b, item_start, item_end);
                        if bytes_len(val) == 0 {
                            return _make_error_code(5);
                        }
                        let valv = bytes_view(val);
                        let valn = view_len(valv);
                        out = vec_u8_extend_bytes_range(out, valv, 0, valn);
                        pos = line_end + 1;
                    } else {
                        let next = _skip_blank_and_comment_lines(b, line_end + 1);
                        if ge_u(next, n) {
                            out = _emit_null(out);
                            pos = next;
                        } else {
                            let next_line_end = _find_newline(b, next);
                            let next_trim_end = _trim_end_space_tab(b, next, next_line_end);
                            let next_ind = _count_indent_or_err(b, next, next_trim_end);
                            if next_ind < 0 {
                                return _make_error_code(2);
                            }
                            if next_ind <= indent {
                                out = _emit_null(out);
                                pos = line_end + 1;
                            } else {
                                let sub = _parse_node(b, next, next_ind);
                                if bytes_len(sub) < 5 {
                                    return _make_error_code(1);
                                }
                                if bytes_get_u8(sub, 0) == 0 {
                                    return sub;
                                }
                                let subv = bytes_view(sub);
                                let end_pos = _read_u32_le(subv, 1);
                                let subn = view_len(subv);
                                out = vec_u8_extend_bytes_range(out, subv, 5, subn - 5);
                                pos = end_pos;
                            }
                        }
                    }
                    first = 0;
                }
            }
        }
    }
    out = vec_u8_push(out, 93);
    let json_b = vec_u8_into_bytes(out);
    let jsonv = bytes_view(json_b);
    let jsonn = view_len(jsonv);
    let mut res = vec_u8_with_capacity(5 + jsonn);
    res = vec_u8_push(res, 1);
    res = _push_u32_le(res, pos);
    res = vec_u8_extend_bytes_range(res, jsonv, 0, jsonn);
    vec_u8_into_bytes(res)
}

pub fn yaml_is_err(doc: BytesView) -> i32 {
    if view_len(doc) < 1 {
        return 1;
    }
    if view_get_u8(doc, 0) == 0 {
        1
    } else {
        0
    }
}

pub fn yaml_parse(src: BytesView) -> Bytes {
    let n = view_len(src);
    let mut pos = _skip_blank_and_comment_lines(src, 0);
    if ge_u(pos, n) {
        return _make_error_code(1);
    }

    // Optional document start marker: '---'
    let line_end0 = _find_newline(src, pos);
    let trimmed_end0 = _trim_end_space_tab(src, pos, line_end0);
    let len0 = trimmed_end0 - pos;
    if len0 == 3
        && view_get_u8(src, pos) == 45
        && view_get_u8(src, pos + 1) == 45
        && view_get_u8(src, pos + 2) == 45
    {
        pos = _skip_blank_and_comment_lines(src, line_end0 + 1);
    }

    let ind0_line_end = _find_newline(src, pos);
    let ind0_trim_end = _trim_end_space_tab(src, pos, ind0_line_end);
    let ind0 = _count_indent_or_err(src, pos, ind0_trim_end);
    if ind0 < 0 {
        return _make_error_code(2);
    }

    let res = _parse_node(src, pos, ind0);
    if bytes_len(res) < 1 {
        return _make_error_code(1);
    }
    if bytes_get_u8(res, 0) == 0 {
        return res;
    }
    if bytes_len(res) < 5 {
        return _make_error_code(1);
    }
    let resv = bytes_view(res);
    let end_pos = _read_u32_le(resv, 1);
    let after = _skip_blank_and_comment_lines(src, end_pos);
    if lt_u(after, n) {
        return _make_error_code(7);
    }

    let out_len = view_len(resv) - 5;
    let mut out = vec_u8_with_capacity(1 + out_len);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes_range(out, resv, 5, out_len);
    vec_u8_into_bytes(out)
}
