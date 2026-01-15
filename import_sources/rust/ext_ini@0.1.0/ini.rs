// INI parser for X07 (x07import-compatible Rust subset).
//
// Supported:
// - Sections: [section]
// - Entries: key=value (also accepts key: value)
// - Comments: line starting with ';' or '#' after leading whitespace
// - Duplicate keys (including section prefix) are an error
//
// Keys are emitted as:
// - "key" (global keys)
// - "section.key" (sectioned keys)
//
// Packed output format:
//   Error:   [0x00][u32_le code][u32_le msg_len=0]
//   Success: [0x01][u32_le entry_count][entries...]
//     Entry: [u32_le key_len][key_bytes][u32_le val_len][val_bytes]
//
// Error codes:
//   1 = invalid section header
//   2 = invalid entry line
//   3 = invalid quoted value
//   4 = duplicate key

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

fn _is_ws(c: i32) -> bool {
    c == 32 || c == 9 || c == 13
}

fn _skip_ws(b: BytesView, start: i32, end: i32) -> i32 {
    let mut i = start;
    for _ in start..end {
        if lt_u(i, end) {
            if _is_ws(view_get_u8(b, i)) {
                i = i + 1;
            }
        }
    }
    i
}

fn _trim_end_ws(b: BytesView, start: i32, end: i32) -> i32 {
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
                if _is_ws(c) {
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

// Returns [0] on error, or [1][decoded bytes...] on success.
fn _decode_quoted_status(b: BytesView, start: i32, end: i32) -> Bytes {
    if !lt_u(start, end) {
        return vec_u8_into_bytes(vec_u8_push(vec_u8_with_capacity(1), 0));
    }
    if view_get_u8(b, start) != 34 {
        return vec_u8_into_bytes(vec_u8_push(vec_u8_with_capacity(1), 0));
    }
    if !lt_u(start + 1, end) {
        return vec_u8_into_bytes(vec_u8_push(vec_u8_with_capacity(1), 0));
    }
    if view_get_u8(b, end - 1) != 34 {
        return vec_u8_into_bytes(vec_u8_push(vec_u8_with_capacity(1), 0));
    }

    let mut out = vec_u8_with_capacity(1 + (end - start));
    out = vec_u8_push(out, 1);

    let mut i = start + 1;
    let mut done = false;
    let mut ok = true;
    for _ in (start + 1)..end {
        if !done {
            if !ok {
                done = true;
            } else if ge_u(i, end - 1) {
                done = true;
            } else {
                let c = view_get_u8(b, i);
                if c == 92 {
                    if ge_u(i + 1, end - 1) {
                        ok = false;
                    } else {
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
                    }
                } else {
                    out = vec_u8_push(out, c);
                    i = i + 1;
                }
            }
        }
    }

    if ok {
        vec_u8_into_bytes(out)
    } else {
        vec_u8_into_bytes(vec_u8_push(vec_u8_with_capacity(1), 0))
    }
}

fn _has_key(entries: BytesView, key: BytesView, entry_count: i32) -> bool {
    let key_len = view_len(key);
    let mut off = 0;

    for _ in 0..entry_count {
        let k_len = _read_u32_le(entries, off);
        let k_start = off + 4;
        let mut next = k_start + k_len;
        if k_len == key_len {
            let mut match_flag = true;
            for j in 0..k_len {
                if match_flag {
                    if view_get_u8(entries, k_start + j) != view_get_u8(key, j) {
                        match_flag = false;
                    }
                }
            }
            if match_flag {
                return true;
            }
        }
        let v_len = _read_u32_le(entries, next);
        next = next + 4 + v_len;
        off = next;
    }

    false
}

pub fn ini_is_err(doc: BytesView) -> i32 {
    if view_len(doc) < 1 {
        return 1;
    }
    if view_get_u8(doc, 0) == 0 {
        1
    } else {
        0
    }
}

pub fn ini_parse(src: BytesView) -> Bytes {
    let n = view_len(src);
    let mut entries = vec_u8_with_capacity(256);
    let mut count = 0;
    let mut section = vec_u8_with_capacity(0);

    let mut pos = 0;
    let mut done = false;
    for _ in 0..(n + 1) {
        if !done {
            if ge_u(pos, n) {
                done = true;
            } else {
                let line_start = pos;
                let line_end = _find_newline(src, pos);
                pos = if lt_u(line_end, n) { line_end + 1 } else { line_end };

                let trimmed_end = _trim_end_ws(src, line_start, line_end);
                let start = _skip_ws(src, line_start, trimmed_end);
                if !lt_u(start, trimmed_end) {
                    // blank line
                    0;
                } else {
                    let c0 = view_get_u8(src, start);
                    if c0 == 59 || c0 == 35 {
                        // comment line
                        0;
                    } else if c0 == 91 {
                        // section header
                        let mut i = start + 1;
                        let mut close = 0 - 1;
                        for _ in (start + 1)..trimmed_end {
                            if lt_u(i, trimmed_end) {
                                if close < 0 {
                                    if view_get_u8(src, i) == 93 {
                                        close = i;
                                    }
                                    i = i + 1;
                                }
                            }
                        }
                        if close < 0 {
                            return _make_error_code(1);
                        }
                        let sec_start0 = start + 1;
                        let sec_end0 = close;
                        let sec_start = _skip_ws(src, sec_start0, sec_end0);
                        let sec_end = _trim_end_ws(src, sec_start, sec_end0);
                        if !lt_u(sec_start, sec_end) {
                            return _make_error_code(1);
                        }
                        let sec_len = sec_end - sec_start;
                        let mut sec = vec_u8_with_capacity(sec_len + 1);
                        for j in sec_start..sec_end {
                            sec = vec_u8_push(sec, view_get_u8(src, j));
                        }
                        sec = vec_u8_push(sec, 46);
                        section = sec;
                        0;
                    } else {
                        // entry
                        let mut i = start;
                        let mut delim = 0 - 1;
                        for _ in start..trimmed_end {
                            if lt_u(i, trimmed_end) {
                                if delim < 0 {
                                    let c = view_get_u8(src, i);
                                    if c == 61 || c == 58 {
                                        delim = i;
                                    }
                                    i = i + 1;
                                }
                            }
                        }
                        if delim < 0 {
                            return _make_error_code(2);
                        }

                        let key_start0 = start;
                        let key_end0 = delim;
                        let key_start = _skip_ws(src, key_start0, key_end0);
                        let key_end = _trim_end_ws(src, key_start, key_end0);
                        if !lt_u(key_start, key_end) {
                            return _make_error_code(2);
                        }

                        let sec_view = vec_u8_as_view(section);
                        let sec_len = view_len(sec_view);
                        let key_len = key_end - key_start;
                        let full_key_len = sec_len + key_len;

                        let mut key_buf = vec_u8_with_capacity(full_key_len);
                        for j in 0..sec_len {
                            key_buf = vec_u8_push(key_buf, view_get_u8(sec_view, j));
                        }
                        for j in key_start..key_end {
                            key_buf = vec_u8_push(key_buf, view_get_u8(src, j));
                        }
                        let key_view = vec_u8_as_view(key_buf);

                        if _has_key(vec_u8_as_view(entries), key_view, count) {
                            return _make_error_code(4);
                        }

                        let mut val_start0 = delim + 1;
                        val_start0 = _skip_ws(src, val_start0, trimmed_end);
                        let mut val_end0 = trimmed_end;
                        val_end0 = _trim_end_ws(src, val_start0, val_end0);

                        let mut val_bytes = _empty_bytes();
                        let mut val_off = 0;
                        let mut val_len2 = 0;
                        if !lt_u(val_start0, val_end0) {
                            // empty value
                            val_bytes = _empty_bytes();
                            val_off = 0;
                            val_len2 = 0;
                        } else if view_get_u8(src, val_start0) == 34 {
                            let dec = _decode_quoted_status(src, val_start0, val_end0);
                            if bytes_len(dec) < 1 {
                                return _make_error_code(3);
                            }
                            if bytes_get_u8(dec, 0) == 0 {
                                return _make_error_code(3);
                            }
                            val_bytes = dec;
                            val_off = 1;
                            val_len2 = bytes_len(val_bytes) - 1;
                        } else {
                            // plain value (no unescape, no inline comments)
                            let mut out = vec_u8_with_capacity(val_end0 - val_start0);
                            for j in val_start0..val_end0 {
                                out = vec_u8_push(out, view_get_u8(src, j));
                            }
                            val_bytes = vec_u8_into_bytes(out);
                            val_off = 0;
                            val_len2 = bytes_len(val_bytes);
                        }

                        entries = _push_u32_le(entries, full_key_len);
                        entries = vec_u8_extend_bytes_range(entries, key_view, 0, full_key_len);
                        entries = _push_u32_le(entries, val_len2);
                        let vv = bytes_view(val_bytes);
                        entries = vec_u8_extend_bytes_range(entries, vv, val_off, val_len2);

                        count = count + 1;
                        0;
                    }
                }
            }
        }
    }

    let entries_b = vec_u8_into_bytes(entries);
    let entries_v = bytes_view(entries_b);
    let entries_len = view_len(entries_v);
    let mut out = vec_u8_with_capacity(5 + entries_len);
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, count);
    out = vec_u8_extend_bytes_range(out, entries_v, 0, entries_len);
    vec_u8_into_bytes(out)
}

pub fn ini_get_string(doc: BytesView, key: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 5 {
        return _empty_bytes();
    }
    if view_get_u8(doc, 0) != 1 {
        return _empty_bytes();
    }
    let entry_count = _read_u32_le(doc, 1);
    let key_len = view_len(key);
    let mut pos = 5;
    for _ in 0..entry_count {
        if !lt_u(pos + 4, n + 1) {
            return _empty_bytes();
        }
        let ek_len = _read_u32_le(doc, pos);
        pos = pos + 4;
        if !lt_u(pos + ek_len, n + 1) {
            return _empty_bytes();
        }
        let mut match_flag = true;
        if ek_len != key_len {
            match_flag = false;
        }
        if match_flag {
            for j in 0..ek_len {
                if match_flag {
                    if view_get_u8(doc, pos + j) != view_get_u8(key, j) {
                        match_flag = false;
                    }
                }
            }
        }
        pos = pos + ek_len;

        if !lt_u(pos + 4, n + 1) {
            return _empty_bytes();
        }
        let v_len = _read_u32_le(doc, pos);
        pos = pos + 4;
        if !lt_u(pos + v_len, n + 1) {
            return _empty_bytes();
        }
        if match_flag {
            return view_to_bytes(view_slice(doc, pos, v_len));
        }
        pos = pos + v_len;
    }
    _empty_bytes()
}
