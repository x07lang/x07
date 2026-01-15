// TOML parser for X07 (x07import-compatible Rust subset)
// Supports: key = "string", key = integer, key = true/false, [section] headers
//
// Packed output format:
//   Error:   [0x00][u32_le code][u32_le msg_len][msg_bytes]
//   Success: [0x01][u32_le entry_count][entries...]
//     Entry: [u32_le key_len][key_bytes][u8 type][value_data]
//       Type 0x01 (string): [u32_le len][bytes]
//       Type 0x02 (i32):    [i32_le value]
//       Type 0x03 (bool):   [u8 0/1]
//
// Tags: 0=0, 1=1
// Types: 1=1, 2=2, 3=3

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

fn _make_error(code: i32, msg: BytesView) -> Bytes {
    let msg_len = view_len(msg);
    let mut out = vec_u8_with_capacity(9 + msg_len);
    out = vec_u8_push(out, 0);
    out = _push_u32_le(out, code);
    out = _push_u32_le(out, msg_len);
    for i in 0..msg_len {
        out = vec_u8_push(out, view_get_u8(msg, i));
    }
    vec_u8_into_bytes(out)
}

fn _is_whitespace(c: i32) -> bool {
    c == 32 || c == 9
}

fn _is_digit(c: i32) -> bool {
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return true;
        }
    }
    false
}

fn _is_alpha(c: i32) -> bool {
    if ge_u(c, 65) {
        if lt_u(c, 91) {
            return true;
        }
    }
    if ge_u(c, 97) {
        if lt_u(c, 123) {
            return true;
        }
    }
    false
}

fn _is_key_char(c: i32) -> bool {
    if _is_alpha(c) {
        return true;
    }
    if _is_digit(c) {
        return true;
    }
    if c == 95 || c == 45 {
        return true;
    }
    false
}

fn _skip_whitespace(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    let mut i = start;
    for _ in start..n {
        if lt_u(i, n) {
            let c = view_get_u8(b, i);
            if _is_whitespace(c) {
                i = i + 1;
            }
        }
    }
    i
}

fn _find_newline(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    let mut i = start;
    let mut found = 0 - 1;
    for _ in start..n {
        if lt_u(i, n) {
            if found < 0 {
                let c = view_get_u8(b, i);
                if c == 10 {
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

fn _parse_key(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    let mut i = start;
    for _ in start..n {
        if lt_u(i, n) {
            let c = view_get_u8(b, i);
            if _is_key_char(c) {
                i = i + 1;
            }
        }
    }
    i
}

fn _parse_quoted_string(b: BytesView, start: i32) -> Bytes {
    let n = view_len(b);
    if ge_u(start, n) {
        return vec_u8_into_bytes(vec_u8_with_capacity(0));
    }
    let first = view_get_u8(b, start);
    if first != 34 {
        return vec_u8_into_bytes(vec_u8_with_capacity(0));
    }
    let mut out = vec_u8_with_capacity(64);
    let mut i = start + 1;
    let mut found_end = 0;
    for _ in (start + 1)..n {
        if lt_u(i, n) {
            if found_end == 0 {
                let c = view_get_u8(b, i);
                if c == 34 {
                    found_end = 1;
                } else if c == 92 {
                    if lt_u(i + 1, n) {
                        let next = view_get_u8(b, i + 1);
                        if next == 110 {
                            out = vec_u8_push(out, 10);
                        } else if next == 116 {
                            out = vec_u8_push(out, 9);
                        } else if next == 114 {
                            out = vec_u8_push(out, 13);
                        } else if next == 92 {
                            out = vec_u8_push(out, 92);
                        } else if next == 34 {
                            out = vec_u8_push(out, 34);
                        } else {
                            out = vec_u8_push(out, next);
                        }
                        i = i + 2;
                    } else {
                        i = i + 1;
                    }
                } else {
                    out = vec_u8_push(out, c);
                    i = i + 1;
                }
            }
        }
    }
    vec_u8_into_bytes(out)
}

fn _find_quote_end(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    if ge_u(start, n) {
        return start;
    }
    let first = view_get_u8(b, start);
    if first != 34 {
        return start;
    }
    let mut i = start + 1;
    let mut found_end = 0 - 1;
    for _ in (start + 1)..n {
        if lt_u(i, n) {
            if found_end < 0 {
                let c = view_get_u8(b, i);
                if c == 34 {
                    found_end = i + 1;
                } else if c == 92 {
                    i = i + 2;
                } else {
                    i = i + 1;
                }
            }
        }
    }
    if found_end < 0 {
        n
    } else {
        found_end
    }
}

fn _parse_integer(b: BytesView, start: i32, line_end: i32) -> i32 {
    let mut i = start;
    let mut neg = 0;
    if lt_u(i, line_end) {
        let c = view_get_u8(b, i);
        if c == 45 {
            neg = 1;
            i = i + 1;
        } else if c == 43 {
            i = i + 1;
        }
    }
    let mut val: i32 = 0;
    for _ in i..line_end {
        if lt_u(i, line_end) {
            let c = view_get_u8(b, i);
            if _is_digit(c) {
                val = val * 10 + (c - 48);
                i = i + 1;
            }
        }
    }
    if neg == 1 {
        0 - val
    } else {
        val
    }
}

fn _check_bool_true(b: BytesView, start: i32) -> bool {
    let n = view_len(b);
    if lt_u(start + 3, n) {
        if view_get_u8(b, start) == 116 {
            if view_get_u8(b, start + 1) == 114 {
                if view_get_u8(b, start + 2) == 117 {
                    if view_get_u8(b, start + 3) == 101 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn _check_bool_false(b: BytesView, start: i32) -> bool {
    let n = view_len(b);
    if lt_u(start + 4, n) {
        if view_get_u8(b, start) == 102 {
            if view_get_u8(b, start + 1) == 97 {
                if view_get_u8(b, start + 2) == 108 {
                    if view_get_u8(b, start + 3) == 115 {
                        if view_get_u8(b, start + 4) == 101 {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

pub fn toml_parse(src: BytesView) -> Bytes {
    let n = view_len(src);
    let mut out = vec_u8_with_capacity(256);
    out = vec_u8_push(out, 1);
    let count_pos = 1;
    out = _push_u32_le(out, 0);

    let mut section = vec_u8_with_capacity(64);
    let mut entry_count: i32 = 0;
    let mut pos: i32 = 0;
    let mut error_flag: i32 = 0;

    for _ in 0..n {
        if lt_u(pos, n) {
            if error_flag == 0 {
                let line_start = pos;
                let line_end = _find_newline(src, pos);
                pos = line_end + 1;

                let trimmed_start = _skip_whitespace(src, line_start);
                if lt_u(trimmed_start, line_end) {
                    let first_char = view_get_u8(src, trimmed_start);

                    if first_char == 35 {
                        // Comment line, skip
                    } else if first_char == 91 {
                        // Section header [section]
                        let section_start = trimmed_start + 1;
                        let mut section_end = section_start;
                        for j in section_start..line_end {
                            if lt_u(j, line_end) {
                                let c = view_get_u8(src, j);
                                if c == 93 {
                                    // Found ]
                                } else if _is_key_char(c) || c == 46 {
                                    section_end = j + 1;
                                }
                            }
                        }
                        section = vec_u8_with_capacity(section_end - section_start + 1);
                        for j in section_start..section_end {
                            section = vec_u8_push(section, view_get_u8(src, j));
                        }
                        section = vec_u8_push(section, 46);
                        0;
                    } else if _is_key_char(first_char) {
                        // Key = value
                        let key_start = trimmed_start;
                        let key_end = _parse_key(src, key_start);

                        let after_key = _skip_whitespace(src, key_end);
                        if lt_u(after_key, line_end) {
                            let eq_char = view_get_u8(src, after_key);
                            if eq_char == 61 {
                                let value_start = _skip_whitespace(src, after_key + 1);
                                if lt_u(value_start, line_end) {
                                    let value_char = view_get_u8(src, value_start);

                                    // Write section + key
                                    let sec_view = vec_u8_as_view(section);
                                    let sec_len = view_len(sec_view);
                                    let key_len = key_end - key_start;
                                    let full_key_len = sec_len + key_len;
                                    out = _push_u32_le(out, full_key_len);
                                    for j in 0..sec_len {
                                        out = vec_u8_push(out, view_get_u8(sec_view, j));
                                    }
                                    for j in key_start..key_end {
                                        out = vec_u8_push(out, view_get_u8(src, j));
                                    }

                                    if value_char == 34 {
                                        // String value
                                        let str_val = _parse_quoted_string(src, value_start);
                                        let str_view = bytes_view(str_val);
                                        let str_len = view_len(str_view);
                                        out = vec_u8_push(out, 1);
                                        out = _push_u32_le(out, str_len);
                                        for j in 0..str_len {
                                            out = vec_u8_push(out, view_get_u8(str_view, j));
                                        }
                                        entry_count = entry_count + 1;
                                    } else if _is_digit(value_char) || value_char == 45 || value_char == 43 {
                                        // Integer value
                                        let int_val = _parse_integer(src, value_start, line_end);
                                        out = vec_u8_push(out, 2);
                                        out = _push_u32_le(out, int_val);
                                        entry_count = entry_count + 1;
                                    } else if _check_bool_true(src, value_start) {
                                        out = vec_u8_push(out, 3);
                                        out = vec_u8_push(out, 1);
                                        entry_count = entry_count + 1;
                                    } else if _check_bool_false(src, value_start) {
                                        out = vec_u8_push(out, 3);
                                        out = vec_u8_push(out, 0);
                                        entry_count = entry_count + 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Patch entry count
    let final_bytes = vec_u8_into_bytes(out);
    let final_view = bytes_view(final_bytes);
    let final_len = view_len(final_view);
    let mut result = vec_u8_with_capacity(final_len);
    result = vec_u8_push(result, 1);
    result = _push_u32_le(result, entry_count);
    for i in 5..final_len {
        result = vec_u8_push(result, view_get_u8(final_view, i));
    }
    vec_u8_into_bytes(result)
}

pub fn toml_is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn toml_get_string(doc: BytesView, key: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 5 {
        return vec_u8_into_bytes(vec_u8_with_capacity(0));
    }
    let tag = view_get_u8(doc, 0);
    if tag != 1 {
        return vec_u8_into_bytes(vec_u8_with_capacity(0));
    }
    let entry_count = _read_u32_le(doc, 1);
    let key_len = view_len(key);

    let mut pos = 5;
    for _ in 0..entry_count {
        if lt_u(pos + 4, n) {
            let ek_len = _read_u32_le(doc, pos);
            pos = pos + 4;
            if lt_u(pos + ek_len, n) {
                let mut match_flag = 1;
                if ek_len != key_len {
                    match_flag = 0;
                }
                if match_flag == 1 {
                    for j in 0..ek_len {
                        if view_get_u8(doc, pos + j) != view_get_u8(key, j) {
                            match_flag = 0;
                        }
                    }
                }
                pos = pos + ek_len;
                if lt_u(pos, n) {
                    let vtype = view_get_u8(doc, pos);
                    pos = pos + 1;
                    if vtype == 1 {
                        if lt_u(pos + 4, n) {
                            let vlen = _read_u32_le(doc, pos);
                            pos = pos + 4;
                            if match_flag == 1 {
                                let mut out = vec_u8_with_capacity(vlen);
                                for j in 0..vlen {
                                    out = vec_u8_push(out, view_get_u8(doc, pos + j));
                                }
                                return vec_u8_into_bytes(out);
                            }
                            pos = pos + vlen;
                        }
                    } else if vtype == 2 {
                        pos = pos + 4;
                    } else if vtype == 3 {
                        pos = pos + 1;
                    }
                }
            }
        }
    }
    vec_u8_into_bytes(vec_u8_with_capacity(0))
}

pub fn toml_get_i32(doc: BytesView, key: BytesView) -> i32 {
    let n = view_len(doc);
    if n < 5 {
        return 0;
    }
    let tag = view_get_u8(doc, 0);
    if tag != 1 {
        return 0;
    }
    let entry_count = _read_u32_le(doc, 1);
    let key_len = view_len(key);

    let mut pos = 5;
    for _ in 0..entry_count {
        if lt_u(pos + 4, n) {
            let ek_len = _read_u32_le(doc, pos);
            pos = pos + 4;
            if lt_u(pos + ek_len, n) {
                let mut match_flag = 1;
                if ek_len != key_len {
                    match_flag = 0;
                }
                if match_flag == 1 {
                    for j in 0..ek_len {
                        if view_get_u8(doc, pos + j) != view_get_u8(key, j) {
                            match_flag = 0;
                        }
                    }
                }
                pos = pos + ek_len;
                if lt_u(pos, n) {
                    let vtype = view_get_u8(doc, pos);
                    pos = pos + 1;
                    if vtype == 1 {
                        if lt_u(pos + 4, n) {
                            let vlen = _read_u32_le(doc, pos);
                            pos = pos + 4 + vlen;
                        }
                    } else if vtype == 2 {
                        if match_flag == 1 {
                            return _read_u32_le(doc, pos);
                        }
                        pos = pos + 4;
                    } else if vtype == 3 {
                        pos = pos + 1;
                    }
                }
            }
        }
    }
    0
}

pub fn toml_get_bool(doc: BytesView, key: BytesView) -> i32 {
    let n = view_len(doc);
    if n < 5 {
        return 0 - 1;
    }
    let tag = view_get_u8(doc, 0);
    if tag != 1 {
        return 0 - 1;
    }
    let entry_count = _read_u32_le(doc, 1);
    let key_len = view_len(key);

    let mut pos = 5;
    for _ in 0..entry_count {
        if lt_u(pos + 4, n) {
            let ek_len = _read_u32_le(doc, pos);
            pos = pos + 4;
            if lt_u(pos + ek_len, n) {
                let mut match_flag = 1;
                if ek_len != key_len {
                    match_flag = 0;
                }
                if match_flag == 1 {
                    for j in 0..ek_len {
                        if view_get_u8(doc, pos + j) != view_get_u8(key, j) {
                            match_flag = 0;
                        }
                    }
                }
                pos = pos + ek_len;
                if lt_u(pos, n) {
                    let vtype = view_get_u8(doc, pos);
                    pos = pos + 1;
                    if vtype == 1 {
                        if lt_u(pos + 4, n) {
                            let vlen = _read_u32_le(doc, pos);
                            pos = pos + 4 + vlen;
                        }
                    } else if vtype == 2 {
                        pos = pos + 4;
                    } else if vtype == 3 {
                        if match_flag == 1 {
                            return view_get_u8(doc, pos);
                        }
                        pos = pos + 1;
                    }
                }
            }
        }
    }
    0 - 1
}
