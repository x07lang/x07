// CSV parser for X07 (x07import-compatible Rust subset).
//
// Supported:
// - Comma delimiter
// - Line endings: LF (\n), CRLF (\r\n), CR (\r)
// - Quoted fields with "" escaping and newlines inside quotes
// - Optional spaces/tabs after a closing quote before delimiter/newline
//
// Packed output format:
//   Error:   [0x00][u32_le code][u32_le msg_len=0]
//   Success: [0x01][u32_le row_count][rows...]
//     Row:   [u32_le col_count][cols...]
//     Col:   [u32_le len][bytes]
//
// Error codes:
//   1 = unclosed quote
//   2 = invalid char after closing quote

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

fn _emit_field(row_data: VecU8, field: VecU8) -> VecU8 {
    let mut out = row_data;
    let fv = vec_u8_as_view(field);
    let len = view_len(fv);
    out = _push_u32_le(out, len);
    out = vec_u8_extend_bytes_range(out, fv, 0, len);
    out
}

fn _emit_row(table: VecU8, row_data: VecU8, col_count: i32) -> VecU8 {
    let mut out = table;
    let rv = vec_u8_as_view(row_data);
    let len = view_len(rv);
    out = _push_u32_le(out, col_count);
    out = vec_u8_extend_bytes_range(out, rv, 0, len);
    out
}

fn _is_ws_after_quote(c: i32) -> bool {
    c == 32 || c == 9
}

pub fn csv_is_err(doc: BytesView) -> i32 {
    if view_len(doc) < 1 {
        return 1;
    }
    if view_get_u8(doc, 0) == 0 {
        1
    } else {
        0
    }
}

pub fn csv_parse(src: BytesView) -> Bytes {
    let n = view_len(src);
    let mut table = vec_u8_with_capacity(256);
    let mut row_count = 0;

    let mut row_started = false;
    let mut row_data = vec_u8_with_capacity(0);
    let mut col_count = 0;
    let mut field = vec_u8_with_capacity(0);
    let mut in_quotes = false;
    let mut after_quote = false;

    let mut i = 0;
    let mut done = false;
    for _ in 0..(n + 1) {
        if !done {
            if ge_u(i, n) {
                done = true;
                if row_started {
                    if in_quotes {
                        return _make_error_code(1);
                    }
                    row_data = _emit_field(row_data, field);
                    col_count = col_count + 1;
                    table = _emit_row(table, row_data, col_count);
                    row_count = row_count + 1;
                }
            } else {
                let c = view_get_u8(src, i);

                if !row_started {
                    if c == 10 {
                        let mut rd = vec_u8_with_capacity(0);
                        rd = _emit_field(rd, vec_u8_with_capacity(0));
                        table = _emit_row(table, rd, 1);
                        row_count = row_count + 1;
                        i = i + 1;
                        0;
                    } else if c == 13 {
                        let mut advance = 1;
                        if lt_u(i + 1, n) {
                            if view_get_u8(src, i + 1) == 10 {
                                advance = 2;
                            }
                        }
                        let mut rd = vec_u8_with_capacity(0);
                        rd = _emit_field(rd, vec_u8_with_capacity(0));
                        table = _emit_row(table, rd, 1);
                        row_count = row_count + 1;
                        i = i + advance;
                        0;
                    } else {
                        row_started = true;
                        row_data = vec_u8_with_capacity(0);
                        col_count = 0;
                        field = vec_u8_with_capacity(0);
                        in_quotes = false;
                        after_quote = false;
                        0;
                    }
                }

                if row_started {
                    if in_quotes {
                        if c == 34 {
                            if lt_u(i + 1, n) {
                                if view_get_u8(src, i + 1) == 34 {
                                    field = vec_u8_push(field, 34);
                                    i = i + 2;
                                    0;
                                } else {
                                    in_quotes = false;
                                    after_quote = true;
                                    i = i + 1;
                                    0;
                                }
                            } else {
                                in_quotes = false;
                                after_quote = true;
                                i = i + 1;
                                0;
                            }
                        } else {
                            field = vec_u8_push(field, c);
                            i = i + 1;
                            0;
                        }
                    } else if after_quote {
                        if _is_ws_after_quote(c) {
                            i = i + 1;
                            0;
                        } else if c == 44 {
                            row_data = _emit_field(row_data, field);
                            col_count = col_count + 1;
                            field = vec_u8_with_capacity(0);
                            after_quote = false;
                            i = i + 1;
                            0;
                        } else if c == 10 {
                            row_data = _emit_field(row_data, field);
                            col_count = col_count + 1;
                            table = _emit_row(table, row_data, col_count);
                            row_count = row_count + 1;

                            row_started = false;
                            row_data = vec_u8_with_capacity(0);
                            col_count = 0;
                            field = vec_u8_with_capacity(0);
                            after_quote = false;
                            i = i + 1;
                            0;
                        } else if c == 13 {
                            let mut advance = 1;
                            if lt_u(i + 1, n) {
                                if view_get_u8(src, i + 1) == 10 {
                                    advance = 2;
                                }
                            }
                            row_data = _emit_field(row_data, field);
                            col_count = col_count + 1;
                            table = _emit_row(table, row_data, col_count);
                            row_count = row_count + 1;

                            row_started = false;
                            row_data = vec_u8_with_capacity(0);
                            col_count = 0;
                            field = vec_u8_with_capacity(0);
                            after_quote = false;
                            i = i + advance;
                            0;
                        } else {
                            return _make_error_code(2);
                        }
                    } else {
                        let field_len = view_len(vec_u8_as_view(field));
                        if c == 34 && field_len == 0 {
                            in_quotes = true;
                            i = i + 1;
                            0;
                        } else if c == 44 {
                            row_data = _emit_field(row_data, field);
                            col_count = col_count + 1;
                            field = vec_u8_with_capacity(0);
                            i = i + 1;
                            0;
                        } else if c == 10 {
                            row_data = _emit_field(row_data, field);
                            col_count = col_count + 1;
                            table = _emit_row(table, row_data, col_count);
                            row_count = row_count + 1;

                            row_started = false;
                            row_data = vec_u8_with_capacity(0);
                            col_count = 0;
                            field = vec_u8_with_capacity(0);
                            i = i + 1;
                            0;
                        } else if c == 13 {
                            let mut advance = 1;
                            if lt_u(i + 1, n) {
                                if view_get_u8(src, i + 1) == 10 {
                                    advance = 2;
                                }
                            }
                            row_data = _emit_field(row_data, field);
                            col_count = col_count + 1;
                            table = _emit_row(table, row_data, col_count);
                            row_count = row_count + 1;

                            row_started = false;
                            row_data = vec_u8_with_capacity(0);
                            col_count = 0;
                            field = vec_u8_with_capacity(0);
                            i = i + advance;
                            0;
                        } else {
                            field = vec_u8_push(field, c);
                            i = i + 1;
                            0;
                        }
                    }
                }
            }
        }
    }

    let table_b = vec_u8_into_bytes(table);
    let table_v = bytes_view(table_b);
    let table_len = view_len(table_v);
    let mut out = vec_u8_with_capacity(5 + table_len);
    out = vec_u8_push(out, 1);
    out = _push_u32_le(out, row_count);
    out = vec_u8_extend_bytes_range(out, table_v, 0, table_len);
    vec_u8_into_bytes(out)
}

pub fn csv_get_string(doc: BytesView, row_idx: i32, col_idx: i32) -> Bytes {
    let n = view_len(doc);
    if n < 5 {
        return _empty_bytes();
    }
    if view_get_u8(doc, 0) != 1 {
        return _empty_bytes();
    }
    let row_count = _read_u32_le(doc, 1);
    if row_idx < 0 || row_idx >= row_count {
        return _empty_bytes();
    }
    if col_idx < 0 {
        return _empty_bytes();
    }

    let mut pos = 5;
    for r in 0..row_count {
        if !lt_u(pos + 4, n + 1) {
            return _empty_bytes();
        }
        let col_count = _read_u32_le(doc, pos);
        pos = pos + 4;

        if r == row_idx {
            if col_idx >= col_count {
                return _empty_bytes();
            }
            for c in 0..col_count {
                if !lt_u(pos + 4, n + 1) {
                    return _empty_bytes();
                }
                let len = _read_u32_le(doc, pos);
                pos = pos + 4;
                if !lt_u(pos + len, n + 1) {
                    return _empty_bytes();
                }
                if c == col_idx {
                    return view_to_bytes(view_slice(doc, pos, len));
                }
                pos = pos + len;
            }
            return _empty_bytes();
        } else {
            for _ in 0..col_count {
                if !lt_u(pos + 4, n + 1) {
                    return _empty_bytes();
                }
                let len = _read_u32_le(doc, pos);
                pos = pos + 4;
                if !lt_u(pos + len, n + 1) {
                    return _empty_bytes();
                }
                pos = pos + len;
            }
        }
    }

    _empty_bytes()
}
