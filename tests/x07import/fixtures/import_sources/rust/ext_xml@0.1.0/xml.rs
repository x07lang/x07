// XML parser for X07 (x07import-compatible Rust subset).
//
// Supported (well-formed subset):
// - Elements, attributes, text nodes
// - Self-closing tags: <a/>
// - XML declaration / processing instructions: skipped (<? ... ?>)
// - Comments: skipped (<!-- ... -->)
// - CDATA: treated as text (<![CDATA[ ... ]]>)
// - Entity decoding in text + attribute values:
//   &lt; &gt; &amp; &apos; &quot; and numeric &#...; / &#x...;
//
// Not supported (returns error):
// - DOCTYPE / DTD (<!DOCTYPE ...>)
//
// Error format (matches ext.data_model doc error layout):
//   [0x00][u32_le code][u32_le msg_len=0]
//
// Events format (Success):
//   [0x01][u32_le event_count][events payload bytes]
//
// Events payload layout:
// - Start: [u8 kind=1][u32_le name_len][name bytes][u32_le attr_count][attrs...]
// - End:   [u8 kind=2][u32_le name_len][name bytes]
// - Text:  [u8 kind=3][u32_le text_len][text bytes]
//
// Attr layout (repeated attr_count times):
//   [u32_le key_len][key bytes][u32_le val_len][val bytes]
//
// Tree format (Success):
//   [0x01][u32_le node_count][u32_le root_idx][u32_le strings_len][u32_le attrs_count=0][u32_le children_count]
//   [nodes table: node_count * 40 bytes]
//   [strings blob: strings_len bytes] (the events payload bytes)
//   [children table: children_count * 4 bytes]
//
// Table layouts:
// - Event record (28 bytes):
//   [u8 kind][u8 pad][u8 pad][u8 pad]
//   [u32 name_start][u32 name_len][u32 attr_start_idx][u32 attr_count]
//   [u32 text_start][u32 text_len]
// - Attr record (16 bytes):
//   [u32 key_start][u32 key_len][u32 val_start][u32 val_len]
// - Node record (40 bytes):
//   [u8 kind][u8 pad][u8 pad][u8 pad]
//   [u32 parent_idx]
//   [u32 name_start][u32 name_len]
//   [u32 attr_bytes_start][u32 attr_count]
//   [u32 child_start_idx][u32 child_count]
//   [u32 text_start][u32 text_len]
//
// Kinds:
// - Events: 1=start, 2=end, 3=text
// - Nodes:  1=element, 2=text
//
// Error codes:
//   1 = no_root
//   2 = malformed
//   3 = mismatched_end_tag
//   4 = unexpected_eof
//   5 = doctype_unsupported
//   6 = entity_error
//   7 = duplicate_attribute
//   8 = invalid_name

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

fn _write_u32_le_at_bytes(b: Bytes, offset: i32, val: i32) -> Bytes {
    let mut out = b;
    out = bytes_set_u8(out, offset, val & 255);
    out = bytes_set_u8(out, offset + 1, (val >> 8) & 255);
    out = bytes_set_u8(out, offset + 2, (val >> 16) & 255);
    out = bytes_set_u8(out, offset + 3, (val >> 24) & 255);
    out
}

fn _empty_bytes() -> Bytes {
    vec_u8_into_bytes(vec_u8_with_capacity(0))
}

fn _res_err0() -> Bytes {
    let mut out = vec_u8_with_capacity(1);
    out = vec_u8_push(out, 0);
    vec_u8_into_bytes(out)
}

fn _make_error_code(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = _push_u32_le(out, code);
    out = _push_u32_le(out, 0);
    vec_u8_into_bytes(out)
}

fn _is_ws(c: i32) -> bool {
    c == 32 || c == 9 || c == 10 || c == 13
}

fn _is_name_start(c: i32) -> bool {
    if c == 58 || c == 95 {
        return true;
    }
    if ge_u(c, 65) && lt_u(c, 91) {
        return true;
    }
    if ge_u(c, 97) && lt_u(c, 123) {
        return true;
    }
    ge_u(c, 128)
}

fn _is_name_char(c: i32) -> bool {
    if _is_name_start(c) {
        return true;
    }
    if ge_u(c, 48) && lt_u(c, 58) {
        return true;
    }
    if c == 45 || c == 46 {
        return true;
    }
    false
}

fn _parse_name_end(src: BytesView, start: i32, end: i32) -> i32 {
    if !lt_u(start, end) {
        return 0 - 1;
    }
    let first = view_get_u8(src, start);
    if !_is_name_start(first) {
        return 0 - 1;
    }
    let mut i = start + 1;
    let mut done = false;
    for _ in 0..(end - start) {
        if !done {
            if !lt_u(i, end) {
                done = true;
            } else {
                let c = view_get_u8(src, i);
                if _is_name_char(c) {
                    i = i + 1;
                } else {
                    done = true;
                }
            }
        }
    }
    i
}

fn _skip_ws(src: BytesView, start: i32, end: i32) -> i32 {
    let mut i = start;
    let mut done = false;
    for _ in 0..(end - start + 1) {
        if !done {
            if !lt_u(i, end) {
                done = true;
            } else {
                let c = view_get_u8(src, i);
                if _is_ws(c) {
                    i = i + 1;
                } else {
                    done = true;
                }
            }
        }
    }
    i
}

fn _find_byte(src: BytesView, start: i32, end: i32, needle: i32) -> i32 {
    let mut i = start;
    let mut found = 0 - 1;
    for _ in start..end {
        if lt_u(i, end) {
            if found < 0 {
                if view_get_u8(src, i) == needle {
                    found = i;
                } else {
                    i = i + 1;
                }
            }
        }
    }
    found
}

fn _find_lt(src: BytesView, start: i32, n: i32) -> i32 {
    let mut i = start;
    let mut found = 0 - 1;
    for _ in start..n {
        if lt_u(i, n) {
            if found < 0 {
                if view_get_u8(src, i) == 60 {
                    found = i;
                } else {
                    i = i + 1;
                }
            }
        }
    }
    if found < 0 {
        n
    } else {
        found
    }
}

fn _is_all_ws(src: BytesView, start: i32, end: i32) -> bool {
    let mut i = start;
    let mut ok = true;
    for _ in start..end {
        if ok && lt_u(i, end) {
            if !_is_ws(view_get_u8(src, i)) {
                ok = false;
            }
            i = i + 1;
        }
    }
    ok
}

fn _skip_pi_or_err(src: BytesView, pos: i32, n: i32) -> i32 {
    let mut i = pos + 2;
    let mut found = false;
    let mut done = false;
    for _ in 0..(n - pos) {
        if !done {
            if ge_u(i + 1, n) {
                done = true;
            } else {
                if view_get_u8(src, i) == 63 && view_get_u8(src, i + 1) == 62 {
                    found = true;
                    done = true;
                } else {
                    i = i + 1;
                }
            }
        }
    }
    if found { i + 2 } else { 0 - 1 }
}

fn _skip_comment_or_err(src: BytesView, pos: i32, n: i32) -> i32 {
    let mut i = pos + 4;
    let mut found = false;
    let mut done = false;
    for _ in 0..(n - pos) {
        if !done {
            if ge_u(i + 2, n) {
                done = true;
            } else {
                if view_get_u8(src, i) == 45
                    && view_get_u8(src, i + 1) == 45
                    && view_get_u8(src, i + 2) == 62
                {
                    found = true;
                    done = true;
                } else {
                    i = i + 1;
                }
            }
        }
    }
    if found { i + 3 } else { 0 - 1 }
}

fn _find_cdata_close_or_err(src: BytesView, content_start: i32, n: i32) -> i32 {
    let mut i = content_start;
    let mut found = false;
    let mut done = false;
    for _ in 0..(n - content_start) {
        if !done {
            if ge_u(i + 2, n) {
                done = true;
            } else {
                if view_get_u8(src, i) == 93 && view_get_u8(src, i + 1) == 93 && view_get_u8(src, i + 2) == 62 {
                    found = true;
                    done = true;
                } else {
                    i = i + 1;
                }
            }
        }
    }
    if found { i } else { 0 - 1 }
}

fn _hex_val(c: i32) -> i32 {
    if ge_u(c, 48) && lt_u(c, 58) {
        return c - 48;
    }
    if ge_u(c, 65) && lt_u(c, 71) {
        return c - 55;
    }
    if ge_u(c, 97) && lt_u(c, 103) {
        return c - 87;
    }
    0 - 1
}

fn _push_utf8(out: VecU8, cp: i32) -> VecU8 {
    let mut o = out;
    if lt_u(cp, 128) {
        return vec_u8_push(o, cp);
    }
    if lt_u(cp, 2048) {
        o = vec_u8_push(o, 192 | (cp >> 6));
        o = vec_u8_push(o, 128 | (cp & 63));
        return o;
    }
    if lt_u(cp, 65536) {
        o = vec_u8_push(o, 224 | (cp >> 12));
        o = vec_u8_push(o, 128 | ((cp >> 6) & 63));
        o = vec_u8_push(o, 128 | (cp & 63));
        return o;
    }
    if lt_u(cp, 1114112) {
        o = vec_u8_push(o, 240 | (cp >> 18));
        o = vec_u8_push(o, 128 | ((cp >> 12) & 63));
        o = vec_u8_push(o, 128 | ((cp >> 6) & 63));
        o = vec_u8_push(o, 128 | (cp & 63));
        return o;
    }
    o
}

fn _decode_entities_status(src: BytesView, start: i32, end: i32) -> Bytes {
    let mut out = vec_u8_with_capacity((end - start) + 1);
    out = vec_u8_push(out, 1);
    let mut i = start;
    let mut done = false;
    for _ in 0..(end - start + 1) {
        if !done {
            if ge_u(i, end) {
                done = true;
            } else {
                let c = view_get_u8(src, i);
                if c != 38 {
                    out = vec_u8_push(out, c);
                    i = i + 1;
                } else {
                    if ge_u(i + 1, end) {
                        return _res_err0();
                    }
                    let c1 = view_get_u8(src, i + 1);

                    // &lt;
                    if c1 == 108 {
                        if ge_u(i + 3, end) {
                            return _res_err0();
                        }
                        if view_get_u8(src, i + 2) == 116 && view_get_u8(src, i + 3) == 59 {
                            out = vec_u8_push(out, 60);
                            i = i + 4;
                        } else {
                            return _res_err0();
                        }
                        0;
                    } else if c1 == 103 {
                        // &gt;
                        if ge_u(i + 3, end) {
                            return _res_err0();
                        }
                        if view_get_u8(src, i + 2) == 116 && view_get_u8(src, i + 3) == 59 {
                            out = vec_u8_push(out, 62);
                            i = i + 4;
                        } else {
                            return _res_err0();
                        }
                        0;
                    } else if c1 == 97 {
                        // &amp; or &apos;
                        if ge_u(i + 4, end) {
                            return _res_err0();
                        }
                        if view_get_u8(src, i + 2) == 109
                            && view_get_u8(src, i + 3) == 112
                            && view_get_u8(src, i + 4) == 59
                        {
                            out = vec_u8_push(out, 38);
                            i = i + 5;
                        } else if ge_u(i + 5, end) {
                            return _res_err0();
                        } else if view_get_u8(src, i + 2) == 112
                            && view_get_u8(src, i + 3) == 111
                            && view_get_u8(src, i + 4) == 115
                            && view_get_u8(src, i + 5) == 59
                        {
                            out = vec_u8_push(out, 39);
                            i = i + 6;
                        } else {
                            return _res_err0();
                        }
                        0;
                    } else if c1 == 113 {
                        // &quot;
                        if ge_u(i + 5, end) {
                            return _res_err0();
                        }
                        if view_get_u8(src, i + 2) == 117
                            && view_get_u8(src, i + 3) == 111
                            && view_get_u8(src, i + 4) == 116
                            && view_get_u8(src, i + 5) == 59
                        {
                            out = vec_u8_push(out, 34);
                            i = i + 6;
                        } else {
                            return _res_err0();
                        }
                        0;
                    } else if c1 == 35 {
                        // numeric char ref
                        if ge_u(i + 2, end) {
                            return _res_err0();
                        }
                        let mut base = 10;
                        let mut j = i + 2;
                        let c2 = view_get_u8(src, j);
                        if c2 == 120 || c2 == 88 {
                            base = 16;
                            j = j + 1;
                        }
                        if ge_u(j, end) {
                            return _res_err0();
                        }
                        let mut cp = 0;
                        let mut any = false;
                        let mut found_semi = false;
                        let mut done2 = false;
                        for _ in 0..(end - j + 1) {
                            if !done2 {
                                if ge_u(j, end) {
                                    done2 = true;
                                } else {
                                    let d = view_get_u8(src, j);
                                    if d == 59 {
                                        found_semi = true;
                                        done2 = true;
                                    } else {
                                        let val = if base == 10 { _hex_val(d) } else { _hex_val(d) };
                                        let ok_digit = if base == 10 {
                                            ge_u(d, 48) && lt_u(d, 58)
                                        } else {
                                            val >= 0
                                        };
                                        if !ok_digit {
                                            return _res_err0();
                                        }
                                        let digit = if base == 10 { d - 48 } else { val };
                                        cp = cp * base + digit;
                                        any = true;
                                        j = j + 1;
                                    }
                                }
                            }
                        }
                        if !found_semi || !any {
                            return _res_err0();
                        }
                        if cp == 0 {
                            return _res_err0();
                        }
                        if ge_u(cp, 55296) && lt_u(cp, 57344) {
                            return _res_err0();
                        }
                        if ge_u(cp, 1114112) {
                            return _res_err0();
                        }
                        out = _push_utf8(out, cp);
                        i = j + 1;
                    } else {
                        return _res_err0();
                    }
                }
            }
        }
    }
    vec_u8_into_bytes(out)
}

fn _eq_bytes_range(a: BytesView, a_start: i32, b: BytesView, b_start: i32, len: i32) -> bool {
    let mut ok = true;
    let mut i = 0;
    for _ in 0..len {
        if ok {
            let ac = view_get_u8(a, a_start + i);
            let bc = view_get_u8(b, b_start + i);
            if ac != bc {
                ok = false;
            }
            i = i + 1;
        }
    }
    ok
}

fn _validate_events_or_code(events: BytesView, event_count: i32, strings: BytesView) -> i32 {
    if event_count < 0 {
        return 2;
    }
    let strings_len = view_len(strings);
    let mut stack = vec_u8_with_capacity(0); // pairs [name_start][name_len] (u32_le)
    let mut depth = 0;
    let mut i = 0;
    for _ in 0..event_count {
        if lt_u(i, event_count) {
            let rec = i * 28;
            if ge_u(rec + 27, view_len(events)) {
                return 2;
            }
            let kind = view_get_u8(events, rec);
            if kind == 1 {
                let name_start = _read_u32_le(events, rec + 4);
                let name_len = _read_u32_le(events, rec + 8);
                if name_start < 0 || name_len < 0 {
                    return 2;
                }
                if ge_u(name_start + name_len, strings_len + 1) {
                    return 2;
                }
                stack = _push_u32_le(stack, name_start);
                stack = _push_u32_le(stack, name_len);
                depth = depth + 1;
            } else if kind == 2 {
                if depth == 0 {
                    return 3;
                }
                let name_start = _read_u32_le(events, rec + 4);
                let name_len = _read_u32_le(events, rec + 8);
                if name_start < 0 || name_len < 0 {
                    return 2;
                }
                if ge_u(name_start + name_len, strings_len + 1) {
                    return 2;
                }

                let stack_v = vec_u8_as_view(stack);
                let top_off = (depth - 1) * 8;
                let top_start = _read_u32_le(stack_v, top_off);
                let top_len = _read_u32_le(stack_v, top_off + 4);
                if top_len != name_len {
                    return 3;
                }
                if !_eq_bytes_range(strings, name_start, strings, top_start, name_len) {
                    return 3;
                }
                depth = depth - 1;
            } else if kind == 3 {
                0;
            } else {
                return 2;
            }
            i = i + 1;
        }
    }
    if depth != 0 {
        return 4;
    }
    0
}

fn _emit_event_record(
    events: VecU8,
    kind: i32,
    name_start: i32,
    name_len: i32,
    attr_start: i32,
    attr_count: i32,
    text_start: i32,
    text_len: i32,
) -> VecU8 {
    let mut e = events;
    e = vec_u8_push(e, kind);
    e = vec_u8_push(e, 0);
    e = vec_u8_push(e, 0);
    e = vec_u8_push(e, 0);
    e = _push_u32_le(e, name_start);
    e = _push_u32_le(e, name_len);
    e = _push_u32_le(e, attr_start);
    e = _push_u32_le(e, attr_count);
    e = _push_u32_le(e, text_start);
    e = _push_u32_le(e, text_len);
    e
}

// Attribute scanner used by xml_events_parse to keep the top-level parser small.
//
// Success payload:
//   [0x01][u32_le new_pos][u8 kind]
//     kind=0: end of attrs (new_pos points at '>' or '/')
//     kind=1: attr parsed:
//       [u32_le key_len][key_bytes][u32_le val_len][val_bytes(decoded)]
// Error payload matches _make_error_code.
fn _scan_attr_or_end(src: BytesView, start: i32, n: i32) -> Bytes {
    let mut i = _skip_ws(src, start, n);
    if ge_u(i, n) {
        return _make_error_code(4);
    }
    let t = view_get_u8(src, i);
    if t == 62 || t == 47 {
        let mut out = vec_u8_with_capacity(6);
        out = vec_u8_push(out, 1);
        out = _push_u32_le(out, i);
        out = vec_u8_push(out, 0);
        return vec_u8_into_bytes(out);
    }

    let key_start_src = i;
    let key_end_src = _parse_name_end(src, key_start_src, n);
    if key_end_src < 0 {
        return _make_error_code(8);
    }
    let key_len = key_end_src - key_start_src;

    i = _skip_ws(src, key_end_src, n);
    if ge_u(i, n) {
        return _make_error_code(4);
    }
    if view_get_u8(src, i) != 61 {
        return _make_error_code(2);
    }
    i = _skip_ws(src, i + 1, n);
    if ge_u(i, n) {
        return _make_error_code(4);
    }
    let q = view_get_u8(src, i);
    if !(q == 34 || q == 39) {
        return _make_error_code(2);
    }
    let val_start_src = i + 1;
    let val_end_src = _find_byte(src, val_start_src, n, q);
    if val_end_src < 0 {
        return _make_error_code(4);
    }

    let decoded = _decode_entities_status(src, val_start_src, val_end_src);
    let dv = bytes_view(decoded);
    if view_len(dv) < 1 {
        return _make_error_code(6);
    }
    if view_get_u8(dv, 0) != 1 {
        return _make_error_code(6);
    }
    let val_len = view_len(dv) - 1;

    let new_pos = val_end_src + 1;
    let mut out = vec_u8_with_capacity(14 + key_len + val_len);
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, new_pos);
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, key_len);
    out = vec_u8_extend_bytes_range(out, src, key_start_src, key_len);
    out = _push_u32_le(out, val_len);
    if val_len > 0 {
        out = vec_u8_extend_bytes_range(out, dv, 1, val_len);
    }
    vec_u8_into_bytes(out)
}

// Start tag scanner used by xml_events_parse to keep the top-level parser within the compiler
// local budget.
//
// Success payload:
//   [0x01][u32_le new_pos][u8 self_closing][u32_le name_len][name_bytes]
//   [u32_le attr_count][attrs...]
//   Attr entry: [u32_le key_len][key_bytes][u32_le val_len][val_bytes(decoded)]
// Error payload matches _make_error_code.
fn _scan_start_tag_or_err(src: BytesView, pos: i32, n: i32) -> Bytes {
    let mut i = pos + 1;
    i = _skip_ws(src, i, n);
    let name_start_src = i;
    let name_end_src = _parse_name_end(src, name_start_src, n);
    if name_end_src < 0 {
        return _make_error_code(8);
    }
    let name_len = name_end_src - name_start_src;
    i = name_end_src;

    let mut attrs_blob = vec_u8_with_capacity(0);
    let mut attr_count = 0;

    let mut done_attrs = false;
    for _ in 0..(n - i + 1) {
        if !done_attrs {
            let step_b = _scan_attr_or_end(src, i, n);
            if view_len(bytes_view(step_b)) < 1 {
                return _make_error_code(2);
            }
            if view_get_u8(bytes_view(step_b), 0) == 0 {
                return step_b;
            }
            let step_v = bytes_view(step_b);
            if view_len(step_v) < 6 {
                return _make_error_code(2);
            }
            let new_i = _read_u32_le(step_v, 1);
            let kind = view_get_u8(step_v, 5);
            if kind == 0 {
                i = new_i;
                done_attrs = true;
            } else if kind == 1 {
                if view_len(step_v) < 10 {
                    return _make_error_code(2);
                }
                let key_len = _read_u32_le(step_v, 6);
                let key_off = 10;
                if key_len < 0 {
                    return _make_error_code(2);
                }
                if ge_u(key_off + key_len, view_len(step_v) + 1) {
                    return _make_error_code(2);
                }

                let val_len_off = key_off + key_len;
                if ge_u(val_len_off + 4, view_len(step_v) + 1) {
                    return _make_error_code(2);
                }
                let val_len = _read_u32_le(step_v, val_len_off);
                let val_off = val_len_off + 4;
                if val_len < 0 {
                    return _make_error_code(2);
                }
                if ge_u(val_off + val_len, view_len(step_v) + 1) {
                    return _make_error_code(2);
                }

                attrs_blob = _push_u32_le(attrs_blob, key_len);
                if key_len > 0 {
                    attrs_blob = vec_u8_extend_bytes_range(attrs_blob, step_v, key_off, key_len);
                }
                attrs_blob = _push_u32_le(attrs_blob, val_len);
                if val_len > 0 {
                    attrs_blob = vec_u8_extend_bytes_range(attrs_blob, step_v, val_off, val_len);
                }

                attr_count = attr_count + 1;
                i = new_i;
            } else {
                return _make_error_code(2);
            }
        }
    }

    i = _skip_ws(src, i, n);
    if ge_u(i, n) {
        return _make_error_code(4);
    }
    let mut self_closing = false;
    if view_get_u8(src, i) == 47 {
        if ge_u(i + 1, n) {
            return _make_error_code(4);
        }
        if view_get_u8(src, i + 1) != 62 {
            return _make_error_code(2);
        }
        self_closing = true;
        i = i + 2;
    } else if view_get_u8(src, i) == 62 {
        i = i + 1;
    } else {
        return _make_error_code(2);
    }

    let attrs_v = vec_u8_as_view(attrs_blob);
    let mut out = vec_u8_with_capacity(14 + name_len + view_len(attrs_v));
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, i);
    out = vec_u8_push(out, if self_closing { 1 } else { 0 });
    out = _push_u32_le(out, name_len);
    if name_len > 0 {
        out = vec_u8_extend_bytes_range(out, src, name_start_src, name_len);
    }
    out = _push_u32_le(out, attr_count);
    out = vec_u8_extend_bytes_range(out, attrs_v, 0, view_len(attrs_v));
    vec_u8_into_bytes(out)
}

pub fn xml_events_is_err(doc: BytesView) -> i32 {
    if view_len(doc) < 1 {
        return 1;
    }
    if view_get_u8(doc, 0) == 0 {
        1
    } else {
        0
    }
}

pub fn xml_events_len(doc: BytesView) -> i32 {
    if xml_events_is_err(doc) == 1 {
        return 0 - 1;
    }
    if view_len(doc) < 5 {
        return 0 - 1;
    }
    _read_u32_le(doc, 1)
}

fn _events_payload_start() -> i32 {
    5
}

fn _events_skip_one(doc: BytesView, off: i32) -> i32 {
    let n = view_len(doc);
    if off < 0 {
        return 0 - 1;
    }
    if ge_u(off, n) {
        return 0 - 1;
    }
    let kind = view_get_u8(doc, off);
    if kind == 1 {
        if ge_u(off + 5, n + 1) {
            return 0 - 1;
        }
        let name_len = _read_u32_le(doc, off + 1);
        if name_len < 0 {
            return 0 - 1;
        }
        let mut p = off + 5;
        if ge_u(p + name_len, n + 1) {
            return 0 - 1;
        }
        p = p + name_len;
        if ge_u(p + 4, n + 1) {
            return 0 - 1;
        }
        let attr_count = _read_u32_le(doc, p);
        if attr_count < 0 {
            return 0 - 1;
        }
        p = p + 4;
        let mut a = 0;
        for _ in 0..attr_count {
            if lt_u(a, attr_count) {
                if ge_u(p + 4, n + 1) {
                    return 0 - 1;
                }
                let key_len = _read_u32_le(doc, p);
                if key_len < 0 {
                    return 0 - 1;
                }
                p = p + 4;
                if ge_u(p + key_len, n + 1) {
                    return 0 - 1;
                }
                p = p + key_len;

                if ge_u(p + 4, n + 1) {
                    return 0 - 1;
                }
                let val_len = _read_u32_le(doc, p);
                if val_len < 0 {
                    return 0 - 1;
                }
                p = p + 4;
                if ge_u(p + val_len, n + 1) {
                    return 0 - 1;
                }
                p = p + val_len;
                a = a + 1;
            }
        }
        p
    } else if kind == 2 || kind == 3 {
        if ge_u(off + 5, n + 1) {
            return 0 - 1;
        }
        let len = _read_u32_le(doc, off + 1);
        if len < 0 {
            return 0 - 1;
        }
        let p = off + 5;
        if ge_u(p + len, n + 1) {
            return 0 - 1;
        }
        p + len
    } else {
        0 - 1
    }
}

fn _events_find_event_off(doc: BytesView, idx: i32) -> i32 {
    let count = xml_events_len(doc);
    if count < 0 {
        return 0 - 1;
    }
    if idx < 0 || idx >= count {
        return 0 - 1;
    }
    let mut off = _events_payload_start();
    let mut i = 0;
    for _ in 0..idx {
        if lt_u(i, idx) {
            off = _events_skip_one(doc, off);
            if off < 0 {
                return 0 - 1;
            }
            i = i + 1;
        }
    }
    off
}

fn _events_payload_skip_one(payload: BytesView, off: i32) -> i32 {
    let n = view_len(payload);
    if off < 0 {
        return 0 - 1;
    }
    if ge_u(off, n) {
        return 0 - 1;
    }
    let kind = view_get_u8(payload, off);
    if kind == 1 {
        if ge_u(off + 5, n + 1) {
            return 0 - 1;
        }
        let name_len = _read_u32_le(payload, off + 1);
        if name_len < 0 {
            return 0 - 1;
        }
        let mut p = off + 5;
        if ge_u(p + name_len, n + 1) {
            return 0 - 1;
        }
        p = p + name_len;
        if ge_u(p + 4, n + 1) {
            return 0 - 1;
        }
        let attr_count = _read_u32_le(payload, p);
        if attr_count < 0 {
            return 0 - 1;
        }
        p = p + 4;
        let mut a = 0;
        for _ in 0..attr_count {
            if lt_u(a, attr_count) {
                if ge_u(p + 4, n + 1) {
                    return 0 - 1;
                }
                let key_len = _read_u32_le(payload, p);
                if key_len < 0 {
                    return 0 - 1;
                }
                p = p + 4;
                if ge_u(p + key_len, n + 1) {
                    return 0 - 1;
                }
                p = p + key_len;
                if ge_u(p + 4, n + 1) {
                    return 0 - 1;
                }
                let val_len = _read_u32_le(payload, p);
                if val_len < 0 {
                    return 0 - 1;
                }
                p = p + 4;
                if ge_u(p + val_len, n + 1) {
                    return 0 - 1;
                }
                p = p + val_len;
                a = a + 1;
            }
        }
        p
    } else if kind == 2 || kind == 3 {
        if ge_u(off + 5, n + 1) {
            return 0 - 1;
        }
        let len = _read_u32_le(payload, off + 1);
        if len < 0 {
            return 0 - 1;
        }
        let p = off + 5;
        if ge_u(p + len, n + 1) {
            return 0 - 1;
        }
        p + len
    } else {
        0 - 1
    }
}

fn _validate_events_payload_or_code(payload: BytesView, event_count: i32) -> i32 {
    if event_count < 0 {
        return 2;
    }
    let n = view_len(payload);
    let mut stack = vec_u8_with_capacity(0); // frames: [prev_idx][name_off][name_len] (u32_le)
    let mut stack_top = 0 - 1;
    let mut stack_frames = 0;
    let mut depth = 0;
    let mut off = 0;
    let mut i = 0;
    for _ in 0..event_count {
        if lt_u(i, event_count) {
            if ge_u(off, n) {
                return 2;
            }
            let kind = view_get_u8(payload, off);
            if kind == 1 {
                if ge_u(off + 5, n + 1) {
                    return 2;
                }
                let name_len = _read_u32_le(payload, off + 1);
                if name_len < 0 {
                    return 2;
                }
                let name_off = off + 5;
                if ge_u(name_off + name_len, n + 1) {
                    return 2;
                }
                stack = _push_u32_le(stack, stack_top);
                stack = _push_u32_le(stack, name_off);
                stack = _push_u32_le(stack, name_len);
                stack_top = stack_frames;
                stack_frames = stack_frames + 1;
                depth = depth + 1;
            } else if kind == 2 {
                if depth == 0 {
                    return 3;
                }
                if stack_top < 0 {
                    return 3;
                }
                if ge_u(off + 5, n + 1) {
                    return 2;
                }
                let name_len = _read_u32_le(payload, off + 1);
                if name_len < 0 {
                    return 2;
                }
                let name_off = off + 5;
                if ge_u(name_off + name_len, n + 1) {
                    return 2;
                }

                let stack_v = vec_u8_as_view(stack);
                let top_off = stack_top * 12;
                let top_prev = _read_u32_le(stack_v, top_off);
                let top_name_off = _read_u32_le(stack_v, top_off + 4);
                let top_name_len = _read_u32_le(stack_v, top_off + 8);
                if top_name_len != name_len {
                    return 3;
                }
                if !_eq_bytes_range(payload, name_off, payload, top_name_off, name_len) {
                    return 3;
                }
                stack_top = top_prev;
                depth = depth - 1;
            } else if kind == 3 {
                0;
            } else {
                return 2;
            }

            off = _events_payload_skip_one(payload, off);
            if off < 0 {
                return 2;
            }
            i = i + 1;
        }
    }
    if depth != 0 {
        return 4;
    }
    if off != n {
        return 2;
    }
    0
}

fn _step_ok(new_pos: i32, depth_delta: i32, event_count_delta: i32, payload: VecU8) -> Bytes {
    let pv = vec_u8_as_view(payload);
    let mut out = vec_u8_with_capacity(13 + view_len(pv));
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, new_pos);
    out = _push_u32_le(out, depth_delta);
    out = _push_u32_le(out, event_count_delta);
    out = vec_u8_extend_bytes_range(out, pv, 0, view_len(pv));
    vec_u8_into_bytes(out)
}

fn _step_pi_or_err(src: BytesView, pos: i32, n: i32) -> Bytes {
    let new_pos = _skip_pi_or_err(src, pos, n);
    if new_pos < 0 {
        return _make_error_code(4);
    }
    _step_ok(new_pos, 0, 0, vec_u8_with_capacity(0))
}

fn _step_comment_or_err(src: BytesView, pos: i32, n: i32) -> Bytes {
    let new_pos = _skip_comment_or_err(src, pos, n);
    if new_pos < 0 {
        return _make_error_code(4);
    }
    _step_ok(new_pos, 0, 0, vec_u8_with_capacity(0))
}

fn _step_cdata_or_err(src: BytesView, pos: i32, n: i32, depth: i32) -> Bytes {
    if depth == 0 {
        return _make_error_code(2);
    }
    if ge_u(pos + 8, n) {
        return _make_error_code(4);
    }
    if !(view_get_u8(src, pos + 3) == 67
        && view_get_u8(src, pos + 4) == 68
        && view_get_u8(src, pos + 5) == 65
        && view_get_u8(src, pos + 6) == 84
        && view_get_u8(src, pos + 7) == 65
        && view_get_u8(src, pos + 8) == 91)
    {
        return _make_error_code(2);
    }
    let content_start = pos + 9;
    let content_end = _find_cdata_close_or_err(src, content_start, n);
    if content_end < 0 {
        return _make_error_code(4);
    }
    let text_len = content_end - content_start;
    let new_pos = content_end + 3;
    if text_len <= 0 {
        return _step_ok(new_pos, 0, 0, vec_u8_with_capacity(0));
    }
    let mut payload = vec_u8_with_capacity(5 + text_len);
    payload = vec_u8_push(payload, 3);
    payload = _push_u32_le(payload, text_len);
    payload = vec_u8_extend_bytes_range(payload, src, content_start, text_len);
    _step_ok(new_pos, 0, 1, payload)
}

fn _step_end_tag_or_err(src: BytesView, pos: i32, n: i32, depth: i32) -> Bytes {
    if depth == 0 {
        return _make_error_code(3);
    }
    let mut i = pos + 2;
    i = _skip_ws(src, i, n);
    let name_start_src = i;
    let name_end = _parse_name_end(src, name_start_src, n);
    if name_end < 0 {
        return _make_error_code(8);
    }
    let name_len = name_end - name_start_src;
    i = _skip_ws(src, name_end, n);
    if ge_u(i, n) {
        return _make_error_code(4);
    }
    if view_get_u8(src, i) != 62 {
        return _make_error_code(2);
    }
    let new_pos = i + 1;
    let mut payload = vec_u8_with_capacity(5 + name_len);
    payload = vec_u8_push(payload, 2);
    payload = _push_u32_le(payload, name_len);
    if name_len > 0 {
        payload = vec_u8_extend_bytes_range(payload, src, name_start_src, name_len);
    }
    _step_ok(new_pos, 0 - 1, 1, payload)
}

fn _step_start_tag_or_err(src: BytesView, pos: i32, n: i32) -> Bytes {
    let scanned_b = _scan_start_tag_or_err(src, pos, n);
    if view_len(bytes_view(scanned_b)) < 1 {
        return _make_error_code(2);
    }
    if view_get_u8(bytes_view(scanned_b), 0) == 0 {
        return scanned_b;
    }
    let scanned_v = bytes_view(scanned_b);
    if view_len(scanned_v) < 14 {
        return _make_error_code(2);
    }

    let new_pos = _read_u32_le(scanned_v, 1);
    let self_closing = view_get_u8(scanned_v, 5);
    let name_len = _read_u32_le(scanned_v, 6);
    if name_len < 0 {
        return _make_error_code(2);
    }
    let name_off = 10;
    if ge_u(name_off + name_len, view_len(scanned_v) + 1) {
        return _make_error_code(2);
    }
    let attr_count_off = name_off + name_len;
    if ge_u(attr_count_off + 4, view_len(scanned_v) + 1) {
        return _make_error_code(2);
    }
    let attr_count = _read_u32_le(scanned_v, attr_count_off);
    if attr_count < 0 {
        return _make_error_code(2);
    }
    let attrs_off = attr_count_off + 4;
    let attrs_len = view_len(scanned_v) - attrs_off;
    if attrs_len < 0 {
        return _make_error_code(2);
    }

    let mut payload = vec_u8_with_capacity(5 + name_len + 4 + attrs_len + 5 + name_len);
    payload = vec_u8_push(payload, 1);
    payload = _push_u32_le(payload, name_len);
    if name_len > 0 {
        payload = vec_u8_extend_bytes_range(payload, scanned_v, name_off, name_len);
    }
    payload = _push_u32_le(payload, attr_count);
    if attrs_len > 0 {
        payload = vec_u8_extend_bytes_range(payload, scanned_v, attrs_off, attrs_len);
    }
    let mut depth_delta = 1;
    let mut count_delta = 1;
    if self_closing == 1 {
        payload = vec_u8_push(payload, 2);
        payload = _push_u32_le(payload, name_len);
        if name_len > 0 {
            payload = vec_u8_extend_bytes_range(payload, scanned_v, name_off, name_len);
        }
        depth_delta = 0;
        count_delta = 2;
    }
    _step_ok(new_pos, depth_delta, count_delta, payload)
}

fn _step_text_or_err(src: BytesView, pos: i32, n: i32, depth: i32) -> Bytes {
    let end = _find_lt(src, pos, n);
    if depth == 0 {
        if !_is_all_ws(src, pos, end) {
            return _make_error_code(2);
        }
        return _step_ok(end, 0, 0, vec_u8_with_capacity(0));
    }
    let decoded = _decode_entities_status(src, pos, end);
    let dv = bytes_view(decoded);
    if view_len(dv) < 1 {
        return _make_error_code(6);
    }
    if view_get_u8(dv, 0) != 1 {
        return _make_error_code(6);
    }
    let text_len = view_len(dv) - 1;
    if text_len <= 0 {
        return _step_ok(end, 0, 0, vec_u8_with_capacity(0));
    }
    let mut payload = vec_u8_with_capacity(5 + text_len);
    payload = vec_u8_push(payload, 3);
    payload = _push_u32_le(payload, text_len);
    payload = vec_u8_extend_bytes_range(payload, dv, 1, text_len);
    _step_ok(end, 0, 1, payload)
}

pub fn xml_events_kind(doc: BytesView, idx: i32) -> i32 {
    let off = _events_find_event_off(doc, idx);
    if off < 0 {
        return 0 - 1;
    }
    view_get_u8(doc, off)
}

fn _slice_from_strings(doc: BytesView, strings_start: i32, start: i32, len: i32) -> Bytes {
    if start < 0 || len < 0 {
        return _empty_bytes();
    }
    let abs = strings_start + start;
    if ge_u(abs, view_len(doc)) {
        return _empty_bytes();
    }
    if ge_u(abs + len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    view_to_bytes(view_slice(doc, abs, len))
}

pub fn xml_events_name(doc: BytesView, idx: i32) -> Bytes {
    let off = _events_find_event_off(doc, idx);
    if off < 0 {
        return _empty_bytes();
    }
    let kind = view_get_u8(doc, off);
    if !(kind == 1 || kind == 2) {
        return _empty_bytes();
    }
    if ge_u(off + 5, view_len(doc) + 1) {
        return _empty_bytes();
    }
    let name_len = _read_u32_le(doc, off + 1);
    if name_len < 0 {
        return _empty_bytes();
    }
    let name_off = off + 5;
    if ge_u(name_off + name_len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    view_to_bytes(view_slice(doc, name_off, name_len))
}

pub fn xml_events_text(doc: BytesView, idx: i32) -> Bytes {
    let off = _events_find_event_off(doc, idx);
    if off < 0 {
        return _empty_bytes();
    }
    let kind = view_get_u8(doc, off);
    if kind != 3 {
        return _empty_bytes();
    }
    if ge_u(off + 5, view_len(doc) + 1) {
        return _empty_bytes();
    }
    let text_len = _read_u32_le(doc, off + 1);
    if text_len < 0 {
        return _empty_bytes();
    }
    let text_off = off + 5;
    if ge_u(text_off + text_len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    view_to_bytes(view_slice(doc, text_off, text_len))
}

pub fn xml_events_attr_count(doc: BytesView, idx: i32) -> i32 {
    let off = _events_find_event_off(doc, idx);
    if off < 0 {
        return 0 - 1;
    }
    let kind = view_get_u8(doc, off);
    if kind != 1 {
        return 0 - 1;
    }
    if ge_u(off + 5, view_len(doc) + 1) {
        return 0 - 1;
    }
    let name_len = _read_u32_le(doc, off + 1);
    if name_len < 0 {
        return 0 - 1;
    }
    let mut p = off + 5;
    if ge_u(p + name_len, view_len(doc) + 1) {
        return 0 - 1;
    }
    p = p + name_len;
    if ge_u(p + 4, view_len(doc) + 1) {
        return 0 - 1;
    }
    _read_u32_le(doc, p)
}

fn _attr_entry_off(doc: BytesView, start: i32, attr_count: i32, attr_idx: i32) -> i32 {
    if start < 0 {
        return 0 - 1;
    }
    if attr_count < 0 {
        return 0 - 1;
    }
    if attr_idx < 0 || attr_idx >= attr_count {
        return 0 - 1;
    }
    let mut p = start;
    let mut a = 0;
    for _ in 0..attr_idx {
        if lt_u(a, attr_idx) {
            if ge_u(p + 4, view_len(doc) + 1) {
                return 0 - 1;
            }
            let key_len = _read_u32_le(doc, p);
            if key_len < 0 {
                return 0 - 1;
            }
            p = p + 4;
            if ge_u(p + key_len, view_len(doc) + 1) {
                return 0 - 1;
            }
            p = p + key_len;
            if ge_u(p + 4, view_len(doc) + 1) {
                return 0 - 1;
            }
            let val_len = _read_u32_le(doc, p);
            if val_len < 0 {
                return 0 - 1;
            }
            p = p + 4;
            if ge_u(p + val_len, view_len(doc) + 1) {
                return 0 - 1;
            }
            p = p + val_len;
            a = a + 1;
        }
    }
    p
}

fn _events_attr_entry_off(doc: BytesView, idx: i32, attr_idx: i32) -> i32 {
    let off = _events_find_event_off(doc, idx);
    if off < 0 {
        return 0 - 1;
    }
    if view_get_u8(doc, off) != 1 {
        return 0 - 1;
    }
    if ge_u(off + 5, view_len(doc) + 1) {
        return 0 - 1;
    }
    let name_len = _read_u32_le(doc, off + 1);
    if name_len < 0 {
        return 0 - 1;
    }
    let mut p = off + 5;
    if ge_u(p + name_len, view_len(doc) + 1) {
        return 0 - 1;
    }
    p = p + name_len;
    if ge_u(p + 4, view_len(doc) + 1) {
        return 0 - 1;
    }
    let attr_count = _read_u32_le(doc, p);
    if attr_count < 0 {
        return 0 - 1;
    }
    _attr_entry_off(doc, p + 4, attr_count, attr_idx)
}

pub fn xml_events_attr_key(doc: BytesView, idx: i32, attr_idx: i32) -> Bytes {
    let off = _events_attr_entry_off(doc, idx, attr_idx);
    if off < 0 {
        return _empty_bytes();
    }
    if ge_u(off + 4, view_len(doc) + 1) {
        return _empty_bytes();
    }
    let key_len = _read_u32_le(doc, off);
    if key_len < 0 {
        return _empty_bytes();
    }
    let key_off = off + 4;
    if ge_u(key_off + key_len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    view_to_bytes(view_slice(doc, key_off, key_len))
}

pub fn xml_events_attr_value(doc: BytesView, idx: i32, attr_idx: i32) -> Bytes {
    let off = _events_attr_entry_off(doc, idx, attr_idx);
    if off < 0 {
        return _empty_bytes();
    }
    if ge_u(off + 4, view_len(doc) + 1) {
        return _empty_bytes();
    }
    let key_len = _read_u32_le(doc, off);
    if key_len < 0 {
        return _empty_bytes();
    }
    let mut p = off + 4;
    if ge_u(p + key_len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    p = p + key_len;
    if ge_u(p + 4, view_len(doc) + 1) {
        return _empty_bytes();
    }
    let val_len = _read_u32_le(doc, p);
    if val_len < 0 {
        return _empty_bytes();
    }
    let val_off = p + 4;
    if ge_u(val_off + val_len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    view_to_bytes(view_slice(doc, val_off, val_len))
}

pub fn xml_events_parse(src: BytesView) -> Bytes {
    let n = view_len(src);
    let mut events = vec_u8_with_capacity(n);
    let mut event_count = 0;

    let mut depth = 0;
    let mut seen_root = false;
    let mut after_root = false;

    let mut pos = 0;
    let mut done = false;
    for _ in 0..(n + 1) {
        if !done {
            if ge_u(pos, n) {
                done = true;
            } else {
                let c = view_get_u8(src, pos);
                let step_b = if c == 60 {
                    if ge_u(pos + 1, n) {
                        return _make_error_code(4);
                    }
                    let c1 = view_get_u8(src, pos + 1);
                    if c1 == 63 {
                        _step_pi_or_err(src, pos, n)
                    } else if c1 == 33 {
                        if ge_u(pos + 2, n) {
                            return _make_error_code(4);
                        }
                        let c2 = view_get_u8(src, pos + 2);
                        if c2 == 45 {
                            if ge_u(pos + 3, n) {
                                return _make_error_code(4);
                            }
                            if view_get_u8(src, pos + 3) != 45 {
                                return _make_error_code(2);
                            }
                            _step_comment_or_err(src, pos, n)
                        } else if c2 == 91 {
                            _step_cdata_or_err(src, pos, n, depth)
                        } else {
                            if ge_u(pos + 8, n) {
                                return _make_error_code(4);
                            }
                            if view_get_u8(src, pos + 2) == 68
                                && view_get_u8(src, pos + 3) == 79
                                && view_get_u8(src, pos + 4) == 67
                                && view_get_u8(src, pos + 5) == 84
                                && view_get_u8(src, pos + 6) == 89
                                && view_get_u8(src, pos + 7) == 80
                                && view_get_u8(src, pos + 8) == 69
                            {
                                return _make_error_code(5);
                            }
                            return _make_error_code(2);
                        }
                    } else if c1 == 47 {
                        _step_end_tag_or_err(src, pos, n, depth)
                    } else {
                        if depth == 0 {
                            if after_root || seen_root {
                                return _make_error_code(2);
                            }
                            seen_root = true;
                        }
                        _step_start_tag_or_err(src, pos, n)
                    }
                } else {
                    _step_text_or_err(src, pos, n, depth)
                };

                if view_len(bytes_view(step_b)) < 1 {
                    return _make_error_code(2);
                }
                if view_get_u8(bytes_view(step_b), 0) == 0 {
                    return step_b;
                }
                let step_v = bytes_view(step_b);
                if view_len(step_v) < 13 {
                    return _make_error_code(2);
                }
                let new_pos = _read_u32_le(step_v, 1);
                let depth_delta = _read_u32_le(step_v, 5);
                let count_delta = _read_u32_le(step_v, 9);
                let payload_off = 13;
                let payload_len = view_len(step_v) - payload_off;
                if payload_len > 0 {
                    events = vec_u8_extend_bytes_range(events, step_v, payload_off, payload_len);
                }
                event_count = event_count + count_delta;
                depth = depth + depth_delta;
                pos = new_pos;
                if seen_root && depth == 0 {
                    after_root = true;
                }
            }
        }
    }

    if depth != 0 {
        return _make_error_code(4);
    }
    if !seen_root {
        return _make_error_code(1);
    }

    let validate_code = _validate_events_payload_or_code(vec_u8_as_view(events), event_count);
    if validate_code != 0 {
        return _make_error_code(validate_code);
    }

    let events_v = vec_u8_as_view(events);
    let mut out = vec_u8_with_capacity(5 + view_len(events_v));
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, event_count);
    out = vec_u8_extend_bytes_range(out, events_v, 0, view_len(events_v));
    vec_u8_into_bytes(out)
}

pub fn xml_tree_is_err(doc: BytesView) -> i32 {
    if view_len(doc) < 1 {
        return 1;
    }
    if view_get_u8(doc, 0) == 0 {
        1
    } else {
        0
    }
}

fn _tree_header_node_count(doc: BytesView) -> i32 {
    if view_len(doc) < 21 {
        return 0 - 1;
    }
    _read_u32_le(doc, 1)
}

fn _tree_header_root(doc: BytesView) -> i32 {
    if view_len(doc) < 21 {
        return 0 - 1;
    }
    _read_u32_le(doc, 5)
}

fn _tree_header_strings_len(doc: BytesView) -> i32 {
    if view_len(doc) < 21 {
        return 0 - 1;
    }
    _read_u32_le(doc, 9)
}

fn _tree_header_attrs_count(doc: BytesView) -> i32 {
    if view_len(doc) < 21 {
        return 0 - 1;
    }
    _read_u32_le(doc, 13)
}

fn _tree_header_children_count(doc: BytesView) -> i32 {
    if view_len(doc) < 21 {
        return 0 - 1;
    }
    _read_u32_le(doc, 17)
}

fn _tree_nodes_start() -> i32 {
    21
}

fn _tree_strings_start(doc: BytesView) -> i32 {
    let node_count = _tree_header_node_count(doc);
    let strings_len = _tree_header_strings_len(doc);
    if node_count < 0 || strings_len < 0 {
        return 0 - 1;
    }
    let start = _tree_nodes_start() + node_count * 40;
    if ge_u(start + strings_len, view_len(doc) + 1) {
        return 0 - 1;
    }
    start
}

fn _tree_attrs_start(doc: BytesView) -> i32 {
    let strings_start = _tree_strings_start(doc);
    if strings_start < 0 {
        return 0 - 1;
    }
    let strings_len = _tree_header_strings_len(doc);
    let attrs_count = _tree_header_attrs_count(doc);
    if strings_len < 0 || attrs_count < 0 {
        return 0 - 1;
    }
    strings_start + strings_len
}

fn _tree_children_start(doc: BytesView) -> i32 {
    let attrs_start = _tree_attrs_start(doc);
    if attrs_start < 0 {
        return 0 - 1;
    }
    let attrs_count = _tree_header_attrs_count(doc);
    if attrs_count < 0 {
        return 0 - 1;
    }
    attrs_start + attrs_count * 16
}

fn _tree_node_record_off(doc: BytesView, node_idx: i32) -> i32 {
    let count = _tree_header_node_count(doc);
    if count < 0 {
        return 0 - 1;
    }
    if node_idx < 0 || node_idx >= count {
        return 0 - 1;
    }
    _tree_nodes_start() + node_idx * 40
}

pub fn xml_tree_node_count(doc: BytesView) -> i32 {
    if xml_tree_is_err(doc) == 1 {
        return 0 - 1;
    }
    _tree_header_node_count(doc)
}

pub fn xml_tree_root(doc: BytesView) -> i32 {
    if xml_tree_is_err(doc) == 1 {
        return 0 - 1;
    }
    _tree_header_root(doc)
}

pub fn xml_tree_node_kind(doc: BytesView, node_idx: i32) -> i32 {
    let off = _tree_node_record_off(doc, node_idx);
    if off < 0 {
        return 0 - 1;
    }
    view_get_u8(doc, off)
}

pub fn xml_tree_node_parent(doc: BytesView, node_idx: i32) -> i32 {
    let off = _tree_node_record_off(doc, node_idx);
    if off < 0 {
        return 0 - 1;
    }
    _read_u32_le(doc, off + 4)
}

pub fn xml_tree_node_name(doc: BytesView, node_idx: i32) -> Bytes {
    let strings_start = _tree_strings_start(doc);
    if strings_start < 0 {
        return _empty_bytes();
    }
    let off = _tree_node_record_off(doc, node_idx);
    if off < 0 {
        return _empty_bytes();
    }
    let kind = view_get_u8(doc, off);
    if kind != 1 {
        return _empty_bytes();
    }
    let name_start = _read_u32_le(doc, off + 8);
    let name_len = _read_u32_le(doc, off + 12);
    _slice_from_strings(doc, strings_start, name_start, name_len)
}

pub fn xml_tree_node_text(doc: BytesView, node_idx: i32) -> Bytes {
    let strings_start = _tree_strings_start(doc);
    if strings_start < 0 {
        return _empty_bytes();
    }
    let off = _tree_node_record_off(doc, node_idx);
    if off < 0 {
        return _empty_bytes();
    }
    let kind = view_get_u8(doc, off);
    if kind != 2 {
        return _empty_bytes();
    }
    let text_start = _read_u32_le(doc, off + 32);
    let text_len = _read_u32_le(doc, off + 36);
    _slice_from_strings(doc, strings_start, text_start, text_len)
}

pub fn xml_tree_attr_count(doc: BytesView, node_idx: i32) -> i32 {
    let off = _tree_node_record_off(doc, node_idx);
    if off < 0 {
        return 0 - 1;
    }
    let kind = view_get_u8(doc, off);
    if kind != 1 {
        return 0 - 1;
    }
    _read_u32_le(doc, off + 20)
}

fn _tree_attr_entry_off(doc: BytesView, node_idx: i32, attr_idx: i32) -> i32 {
    let strings_start = _tree_strings_start(doc);
    if strings_start < 0 {
        return 0 - 1;
    }
    let off = _tree_node_record_off(doc, node_idx);
    if off < 0 {
        return 0 - 1;
    }
    let kind = view_get_u8(doc, off);
    if kind != 1 {
        return 0 - 1;
    }
    let attr_bytes_start = _read_u32_le(doc, off + 16);
    let attr_count = _read_u32_le(doc, off + 20);
    let start = strings_start + attr_bytes_start;
    _attr_entry_off(doc, start, attr_count, attr_idx)
}

pub fn xml_tree_attr_key(doc: BytesView, node_idx: i32, attr_idx: i32) -> Bytes {
    let off = _tree_attr_entry_off(doc, node_idx, attr_idx);
    if off < 0 {
        return _empty_bytes();
    }
    if ge_u(off + 4, view_len(doc) + 1) {
        return _empty_bytes();
    }
    let key_len = _read_u32_le(doc, off);
    if key_len < 0 {
        return _empty_bytes();
    }
    let key_off = off + 4;
    if ge_u(key_off + key_len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    view_to_bytes(view_slice(doc, key_off, key_len))
}

pub fn xml_tree_attr_value(doc: BytesView, node_idx: i32, attr_idx: i32) -> Bytes {
    let off = _tree_attr_entry_off(doc, node_idx, attr_idx);
    if off < 0 {
        return _empty_bytes();
    }
    if ge_u(off + 4, view_len(doc) + 1) {
        return _empty_bytes();
    }
    let key_len = _read_u32_le(doc, off);
    if key_len < 0 {
        return _empty_bytes();
    }
    let mut p = off + 4;
    if ge_u(p + key_len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    p = p + key_len;
    if ge_u(p + 4, view_len(doc) + 1) {
        return _empty_bytes();
    }
    let val_len = _read_u32_le(doc, p);
    if val_len < 0 {
        return _empty_bytes();
    }
    let val_off = p + 4;
    if ge_u(val_off + val_len, view_len(doc) + 1) {
        return _empty_bytes();
    }
    view_to_bytes(view_slice(doc, val_off, val_len))
}

pub fn xml_tree_child_count(doc: BytesView, node_idx: i32) -> i32 {
    let off = _tree_node_record_off(doc, node_idx);
    if off < 0 {
        return 0 - 1;
    }
    let kind = view_get_u8(doc, off);
    if kind != 1 {
        return 0 - 1;
    }
    _read_u32_le(doc, off + 28)
}

pub fn xml_tree_child_at(doc: BytesView, node_idx: i32, child_idx: i32) -> i32 {
    let children_start = _tree_children_start(doc);
    if children_start < 0 {
        return 0 - 1;
    }
    let off = _tree_node_record_off(doc, node_idx);
    if off < 0 {
        return 0 - 1;
    }
    let kind = view_get_u8(doc, off);
    if kind != 1 {
        return 0 - 1;
    }
    let child_start_idx = _read_u32_le(doc, off + 24);
    let child_count = _read_u32_le(doc, off + 28);
    if child_idx < 0 || child_idx >= child_count {
        return 0 - 1;
    }
    let idx = child_start_idx + child_idx;
    let children_count = _tree_header_children_count(doc);
    if children_count < 0 {
        return 0 - 1;
    }
    if idx < 0 || idx >= children_count {
        return 0 - 1;
    }
    _read_u32_le(doc, children_start + idx * 4)
}

fn _tree_count_or_err(doc: BytesView, event_count: i32) -> Bytes {
    if event_count < 0 {
        return _make_error_code(2);
    }
    let mut node_total = 0;
    let mut children_total = 0;
    let mut depth0 = 0;
    let mut off0 = _events_payload_start();
    let mut i = 0;
    for _ in 0..event_count {
        if lt_u(i, event_count) {
            if off0 < 0 {
                return _make_error_code(2);
            }
            let kind = view_get_u8(doc, off0);
            if kind == 1 {
                if depth0 > 0 {
                    children_total = children_total + 1;
                }
                node_total = node_total + 1;
                depth0 = depth0 + 1;
            } else if kind == 2 {
                if depth0 == 0 {
                    return _make_error_code(3);
                }
                depth0 = depth0 - 1;
            } else if kind == 3 {
                if depth0 == 0 {
                    return _make_error_code(2);
                }
                children_total = children_total + 1;
                node_total = node_total + 1;
            } else {
                return _make_error_code(2);
            }
            off0 = _events_skip_one(doc, off0);
            i = i + 1;
        }
    }
    if depth0 != 0 {
        return _make_error_code(4);
    }
    if node_total == 0 {
        return _make_error_code(1);
    }
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, node_total);
    out = _push_u32_le(out, children_total);
    vec_u8_into_bytes(out)
}

fn _tree_inc_child_count(nodes_b: Bytes, parent: i32) -> Bytes {
    let mut b = nodes_b;
    let parent_off = parent * 40;
    let prev = _read_u32_le(bytes_view(b), parent_off + 28);
    b = _write_u32_le_at_bytes(b, parent_off + 28, prev + 1);
    b
}

fn _tree_write_elem_node(
    nodes_b: Bytes,
    node_idx: i32,
    parent: i32,
    name_start: i32,
    name_len: i32,
    attr_start_idx: i32,
    attr_count: i32,
    child_start_idx: i32,
) -> Bytes {
    let mut b = nodes_b;
    let off_node = node_idx * 40;

    b = bytes_set_u8(b, off_node, 1);
    b = bytes_set_u8(b, off_node + 1, 0);
    b = bytes_set_u8(b, off_node + 2, 0);
    b = bytes_set_u8(b, off_node + 3, 0);

    b = _write_u32_le_at_bytes(b, off_node + 4, parent);
    b = _write_u32_le_at_bytes(b, off_node + 8, name_start);
    b = _write_u32_le_at_bytes(b, off_node + 12, name_len);
    b = _write_u32_le_at_bytes(b, off_node + 16, attr_start_idx);
    b = _write_u32_le_at_bytes(b, off_node + 20, attr_count);
    b = _write_u32_le_at_bytes(b, off_node + 24, child_start_idx);
    b = _write_u32_le_at_bytes(b, off_node + 28, 0);
    b = _write_u32_le_at_bytes(b, off_node + 32, 0);
    b = _write_u32_le_at_bytes(b, off_node + 36, 0);
    b
}

fn _tree_write_text_node(
    nodes_b: Bytes,
    node_idx: i32,
    parent: i32,
    text_start: i32,
    text_len: i32,
) -> Bytes {
    let mut b = nodes_b;
    let off_node = node_idx * 40;

    b = bytes_set_u8(b, off_node, 2);
    b = bytes_set_u8(b, off_node + 1, 0);
    b = bytes_set_u8(b, off_node + 2, 0);
    b = bytes_set_u8(b, off_node + 3, 0);

    b = _write_u32_le_at_bytes(b, off_node + 4, parent);
    b = _write_u32_le_at_bytes(b, off_node + 8, 0);
    b = _write_u32_le_at_bytes(b, off_node + 12, 0);
    b = _write_u32_le_at_bytes(b, off_node + 16, 0);
    b = _write_u32_le_at_bytes(b, off_node + 20, 0);
    b = _write_u32_le_at_bytes(b, off_node + 24, 0);
    b = _write_u32_le_at_bytes(b, off_node + 28, 0);
    b = _write_u32_le_at_bytes(b, off_node + 32, text_start);
    b = _write_u32_le_at_bytes(b, off_node + 36, text_len);
    b
}

fn _tree_build_or_err(doc: BytesView, event_count: i32, node_total: i32, children_total: i32) -> Bytes {
    if event_count < 0 || node_total < 0 || children_total < 0 {
        return _make_error_code(2);
    }

    let mut nodes_b = bytes_alloc(node_total * 40);
    let mut children_b = bytes_alloc(children_total * 4);
    let mut child_write = 0;

    let mut stack = vec_u8_with_capacity(0); // frames: [prev_idx][node_idx] (u32_le)
    let mut stack_top = 0 - 1;
    let mut stack_frames = 0;
    let mut depth = 0;
    let mut node_idx = 0;
    let mut root_idx = 0 - 1;

    let mut off = _events_payload_start();
    let mut i = 0;
    for _ in 0..event_count {
        if lt_u(i, event_count) {
            if off < 0 {
                return _make_error_code(2);
            }
            let kind = view_get_u8(doc, off);
            if kind == 1 {
                if ge_u(off + 5, view_len(doc) + 1) {
                    return _make_error_code(2);
                }
                let name_len = _read_u32_le(doc, off + 1);
                if name_len < 0 {
                    return _make_error_code(2);
                }
                let name_off = off + 5;
                if ge_u(name_off + name_len, view_len(doc) + 1) {
                    return _make_error_code(2);
                }
                let mut p = name_off + name_len;
                if ge_u(p + 4, view_len(doc) + 1) {
                    return _make_error_code(2);
                }
                let attr_count = _read_u32_le(doc, p);
                if attr_count < 0 {
                    return _make_error_code(2);
                }
                p = p + 4;
                let attr_bytes_start = p - _events_payload_start();

                let parent = if stack_top < 0 {
                    0 - 1
                } else {
                    let stack_v = vec_u8_as_view(stack);
                    _read_u32_le(stack_v, stack_top * 8 + 4)
                };

                if parent < 0 {
                    if root_idx < 0 {
                        root_idx = node_idx;
                    } else {
                        return _make_error_code(2);
                    }
                } else {
                    children_b = _write_u32_le_at_bytes(children_b, child_write * 4, node_idx);
                    child_write = child_write + 1;
                    nodes_b = _tree_inc_child_count(nodes_b, parent);
                }

                // Strings blob in the tree document is the events payload bytes.
                // Name bytes for this event start at payload offset `off`.
                let name_start = off;

                let child_start_idx = child_write;
                nodes_b = _tree_write_elem_node(
                    nodes_b,
                    node_idx,
                    parent,
                    name_start,
                    name_len,
                    attr_bytes_start,
                    attr_count,
                    child_start_idx,
                );

                stack = _push_u32_le(stack, stack_top);
                stack = _push_u32_le(stack, node_idx);
                stack_top = stack_frames;
                stack_frames = stack_frames + 1;
                depth = depth + 1;
                node_idx = node_idx + 1;
            } else if kind == 2 {
                if depth == 0 {
                    return _make_error_code(3);
                }
                if stack_top < 0 {
                    return _make_error_code(3);
                }
                let stack_v = vec_u8_as_view(stack);
                stack_top = _read_u32_le(stack_v, stack_top * 8);
                depth = depth - 1;
            } else if kind == 3 {
                if depth == 0 {
                    return _make_error_code(2);
                }
                if ge_u(off + 5, view_len(doc) + 1) {
                    return _make_error_code(2);
                }
                let text_len = _read_u32_le(doc, off + 1);
                if text_len < 0 {
                    return _make_error_code(2);
                }
                let text_off = off + 5;
                if ge_u(text_off + text_len, view_len(doc) + 1) {
                    return _make_error_code(2);
                }
                if stack_top < 0 {
                    return _make_error_code(2);
                }
                let parent = _read_u32_le(vec_u8_as_view(stack), stack_top * 8 + 4);

                children_b = _write_u32_le_at_bytes(children_b, child_write * 4, node_idx);
                child_write = child_write + 1;
                nodes_b = _tree_inc_child_count(nodes_b, parent);

                // Text bytes for this event start at payload offset `off`.
                let text_start = off;
                nodes_b = _tree_write_text_node(nodes_b, node_idx, parent, text_start, text_len);

                node_idx = node_idx + 1;
            } else {
                return _make_error_code(2);
            }

            off = _events_skip_one(doc, off);
            i = i + 1;
        }
    }

    if depth != 0 {
        return _make_error_code(4);
    }
    if root_idx < 0 {
        return _make_error_code(1);
    }
    if node_idx != node_total {
        return _make_error_code(2);
    }
    if child_write != children_total {
        return _make_error_code(2);
    }

    let nodes_v = bytes_view(nodes_b);
    let children_v = bytes_view(children_b);

    let payload_start = _events_payload_start();
    let strings_len = view_len(doc) - payload_start;
    if strings_len < 0 {
        return _make_error_code(2);
    }
    let strings_blob = view_slice(doc, payload_start, strings_len);
    let attrs_count = 0;

    let mut out = vec_u8_with_capacity(
        21 + view_len(nodes_v) + strings_len + view_len(children_v),
    );
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, node_total);
    out = _push_u32_le(out, root_idx);
    out = _push_u32_le(out, strings_len);
    out = _push_u32_le(out, attrs_count);
    out = _push_u32_le(out, children_total);
    out = vec_u8_extend_bytes_range(out, nodes_v, 0, view_len(nodes_v));
    out = vec_u8_extend_bytes_range(out, strings_blob, 0, strings_len);
    out = vec_u8_extend_bytes_range(out, children_v, 0, view_len(children_v));
    vec_u8_into_bytes(out)
}

pub fn xml_tree_parse(src: BytesView) -> Bytes {
    let events_b = xml_events_parse(src);
    if xml_events_is_err(bytes_view(events_b)) == 1 {
        return events_b;
    }
    let doc = bytes_view(events_b);

    let event_count = xml_events_len(doc);
    if event_count < 0 {
        return _make_error_code(2);
    }

    let counts_b = _tree_count_or_err(doc, event_count);
    if view_get_u8(bytes_view(counts_b), 0) == 0 {
        return counts_b;
    }
    let counts_v = bytes_view(counts_b);
    if view_len(counts_v) < 9 {
        return _make_error_code(2);
    }
    let node_total = _read_u32_le(counts_v, 1);
    let children_total = _read_u32_le(counts_v, 5);
    _tree_build_or_err(doc, event_count, node_total, children_total)
}
