fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

fn _varint_u32_status(msg: BytesView, off: i32) -> Bytes {
    let n = view_len(msg);
    if ge_u(off, n) {
        return _make_err(1);
    }

    let mut val = 0;
    let mut shift = 0;
    let mut i = off;
    let mut done = 0;
    let mut overflow = 0;
    for _ in 0..10 {
        if done == 0 {
            if ge_u(i, n) {
                return _make_err(1);
            }
            let b = view_get_u8(msg, i);
            let low = b & 127;
            if shift == 28 && (low & 240) != 0 {
                overflow = 1;
            }
            if lt_u(shift, 32) {
                val = val | (low << shift);
            } else {
                overflow = 1;
            }
            i = i + 1;
            if (b & 128) == 0 {
                done = 1;
            }
            shift = shift + 7;
        }
    }
    if done == 0 || overflow != 0 {
        return _make_err(2);
    }

    let len = i - off;
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(val));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(len));
    vec_u8_into_bytes(out)
}

fn _varint_u64_status(msg: BytesView, off: i32) -> Bytes {
    let n = view_len(msg);
    if ge_u(off, n) {
        return _make_err(1);
    }

    let mut lo = 0;
    let mut hi = 0;
    let mut shift = 0;
    let mut i = off;
    let mut done = 0;
    let mut overflow = 0;
    for _ in 0..10 {
        if done == 0 {
            if ge_u(i, n) {
                return _make_err(1);
            }

            let b = view_get_u8(msg, i);
            let low = b & 127;
            if shift == 63 && (low & 126) != 0 {
                overflow = 1;
            }
            if lt_u(shift, 64) {
                if lt_u(shift, 32) {
                    lo = lo | (low << shift);
                    if lt_u(25, shift) {
                        hi = hi | (low >> (32 - shift));
                    }
                } else {
                    hi = hi | (low << (shift - 32));
                }
            } else {
                overflow = 1;
            }
            i = i + 1;
            if (b & 128) == 0 {
                done = 1;
            }
            shift = shift + 7;
        }
    }
    if done == 0 || overflow != 0 {
        return _make_err(2);
    }

    let len = i - off;
    let mut out = vec_u8_with_capacity(13);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(lo));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(hi));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(len));
    vec_u8_into_bytes(out)
}

fn _varint_len_status(msg: BytesView, off: i32) -> Bytes {
    let n = view_len(msg);
    let mut i = off;
    let mut len = 0;
    let mut done = 0;
    for _ in 0..10 {
        if done == 0 {
            if ge_u(i, n) {
                return _make_err(1);
            }
            let b = view_get_u8(msg, i);
            i = i + 1;
            len = len + 1;
            if (b & 128) == 0 {
                done = 1;
            }
        }
    }
    if done == 0 {
        return _make_err(2);
    }
    let mut out = vec_u8_with_capacity(5);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(len));
    vec_u8_into_bytes(out)
}

fn _varint_u32_encode(x: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(5);
    let mut v = x;
    for _ in 0..5 {
        let b = v & 127;
        v = v >> 7;
        if v == 0 {
            out = vec_u8_push(out, b);
            return vec_u8_into_bytes(out);
        }
        out = vec_u8_push(out, b | 128);
    }
    vec_u8_into_bytes(out)
}

pub fn varint_u32_status_v1(msg: BytesView, off: i32) -> Bytes {
    _varint_u32_status(msg, off)
}

pub fn varint_u64_status_v1(msg: BytesView, off: i32) -> Bytes {
    _varint_u64_status(msg, off)
}

pub fn varint_u32_encode_v1(x: i32) -> Bytes {
    _varint_u32_encode(x)
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

pub fn decode_v1(msg: BytesView, max_out_bytes: i32) -> Bytes {
    // Error codes:
    //  1 = ERR_TRUNCATED
    //  2 = ERR_VARINT_OVERFLOW
    //  3 = ERR_INVALID_WIRE_TYPE
    //  4 = ERR_OUTPUT_LIMIT
    if max_out_bytes < 0 {
        return _make_err(4);
    }

    let n = view_len(msg);
    let mut out = vec_u8_with_capacity(1 + 4);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));

    let mut off = 0;
    let mut count = 0;
    for _ in 0..(n + 1) {
        if ge_u(off, n) {
            let mut b = vec_u8_into_bytes(out);
            let c = codec_write_u32_le(count);
            b = bytes_set_u8(b, 1, bytes_get_u8(c, 0));
            b = bytes_set_u8(b, 2, bytes_get_u8(c, 1));
            b = bytes_set_u8(b, 3, bytes_get_u8(c, 2));
            b = bytes_set_u8(b, 4, bytes_get_u8(c, 3));
            return b;
        }

        let tag_status = _varint_u32_status(msg, off);
        if bytes_get_u8(tag_status, 0) == 0 {
            return tag_status;
        }
        let tagv = bytes_view(tag_status);
        let tag = codec_read_u32_le(tagv, 1);
        let tag_len = codec_read_u32_le(tagv, 5);
        off = off + tag_len;

        let wire_type = tag & 7;
        let field_no = tag >> 3;
        if field_no == 0 {
            return _make_err(3);
        }
        if wire_type != 0 && wire_type != 1 && wire_type != 2 && wire_type != 5 {
            return _make_err(3);
        }

        let mut value_len = 0;
        let mut value_off = 0;
        if wire_type == 0 {
            let len_status = _varint_len_status(msg, off);
            if bytes_get_u8(len_status, 0) == 0 {
                return len_status;
            }
            let lv = bytes_view(len_status);
            let l = codec_read_u32_le(lv, 1);
            value_off = off;
            value_len = l;
            off = off + l;
        } else if wire_type == 1 {
            if lt_u(n, off + 8) {
                return _make_err(1);
            }
            value_off = off;
            value_len = 8;
            off = off + 8;
        } else if wire_type == 5 {
            if lt_u(n, off + 4) {
                return _make_err(1);
            }
            value_off = off;
            value_len = 4;
            off = off + 4;
        } else {
            let len_status = _varint_u32_status(msg, off);
            if bytes_get_u8(len_status, 0) == 0 {
                return len_status;
            }
            let lv = bytes_view(len_status);
            let l = codec_read_u32_le(lv, 1);
            let l_len = codec_read_u32_le(lv, 5);
            if l < 0 {
                return _make_err(2);
            }
            off = off + l_len;
            if lt_u(n, off + l) {
                return _make_err(1);
            }
            value_off = off;
            value_len = l;
            off = off + l;
        }

        let cur_payload = vec_u8_len(out) - 1;
        let needed = 9 + value_len;
        if lt_u(max_out_bytes, cur_payload) {
            return _make_err(4);
        }
        let remain = max_out_bytes - cur_payload;
        if lt_u(remain, needed) {
            return _make_err(4);
        }

        out = vec_u8_extend_bytes(out, codec_write_u32_le(field_no));
        out = vec_u8_push(out, wire_type);
        out = vec_u8_extend_bytes(out, codec_write_u32_le(value_len));
        out = vec_u8_extend_bytes_range(out, msg, value_off, value_len);
        count = count + 1;
    }

    _make_err(1)
}

pub fn encode_v1(doc: BytesView, max_out_bytes: i32) -> Bytes {
    // Error codes:
    //  1 = ERR_MALFORMED_DOC
    //  2 = ERR_OUTPUT_LIMIT
    //  3 = ERR_INVALID_WIRE_TYPE
    //  4 = ERR_INVALID_FIELD_NO
    //  5 = ERR_INVALID_VALUE_LEN
    if max_out_bytes < 0 {
        return _make_err(2);
    }

    let n = view_len(doc);
    if lt_u(n, 1) {
        return _make_err(1);
    }

    if view_get_u8(doc, 0) == 0 {
        return view_to_bytes(doc);
    }

    if lt_u(n, 5) {
        return _make_err(1);
    }

    let count = codec_read_u32_le(doc, 1);
    if count < 0 {
        return _make_err(1);
    }

    let mut pos = 5;

    let mut out = vec_u8_with_capacity(n);
    out = vec_u8_push(out, 1);
    let mut out_payload_len = 0;

    for _ in 0..count {
        if ge_u(pos + 9, n + 1) {
            return _make_err(1);
        }

        let field_no = codec_read_u32_le(doc, pos);
        pos = pos + 4;
        let wire_type = view_get_u8(doc, pos);
        pos = pos + 1;
        let value_len = codec_read_u32_le(doc, pos);
        pos = pos + 4;

        if field_no == 0 {
            return _make_err(4);
        }
        if wire_type != 0 && wire_type != 1 && wire_type != 2 && wire_type != 5 {
            return _make_err(3);
        }
        if value_len < 0 {
            return _make_err(1);
        }
        if ge_u(pos + value_len, n + 1) {
            return _make_err(1);
        }

        if wire_type == 0 {
            if value_len == 0 || lt_u(10, value_len) {
                return _make_err(5);
            }
        } else if wire_type == 1 {
            if value_len != 8 {
                return _make_err(5);
            }
        } else if wire_type == 5 {
            if value_len != 4 {
                return _make_err(5);
            }
        }

        let value_off = pos;
        pos = pos + value_len;

        let tag = (field_no << 3) | wire_type;
        let tag_bytes = _varint_u32_encode(tag);
        let tag_len = bytes_len(tag_bytes);
        if lt_u(max_out_bytes, out_payload_len + tag_len) {
            return _make_err(2);
        }
        out = vec_u8_extend_bytes(out, bytes_view(tag_bytes));
        out_payload_len = out_payload_len + tag_len;

        if wire_type == 2 {
            let len_bytes = _varint_u32_encode(value_len);
            let len_len = bytes_len(len_bytes);
            if lt_u(max_out_bytes, out_payload_len + len_len) {
                return _make_err(2);
            }
            out = vec_u8_extend_bytes(out, bytes_view(len_bytes));
            out_payload_len = out_payload_len + len_len;
        }

        if lt_u(max_out_bytes, out_payload_len + value_len) {
            return _make_err(2);
        }
        out = vec_u8_extend_bytes_range(out, doc, value_off, value_len);
        out_payload_len = out_payload_len + value_len;
    }

    if pos != n {
        return _make_err(1);
    }

    vec_u8_into_bytes(out)
}

fn _schema_field_entry_v1(fd_bytes: BytesView, max_out_bytes: i32) -> Bytes {
    let fd_doc = decode_v1(fd_bytes, max_out_bytes);
    let fdv = bytes_view(fd_doc);
    if is_err(fdv) {
        return view_to_bytes(fdv);
    }
    let n = view_len(fdv);
    if lt_u(n, 5) {
        return _make_err(5);
    }
    let count = codec_read_u32_le(fdv, 1);
    if count < 0 {
        return _make_err(5);
    }

    let mut f_name = bytes_alloc(0);
    let mut f_number = 0;
    let mut f_label = 0;
    let mut f_type = 0;
    let mut f_type_name = bytes_alloc(0);

    let mut pos = 5;
    for _ in 0..count {
        if ge_u(pos + 9, n + 1) {
            return _make_err(5);
        }
        let fdf = codec_read_u32_le(fdv, pos);
        pos = pos + 4;
        let fdwt = view_get_u8(fdv, pos);
        pos = pos + 1;
        let vlen = codec_read_u32_le(fdv, pos);
        pos = pos + 4;
        if vlen < 0 {
            return _make_err(5);
        }
        if ge_u(pos + vlen, n + 1) {
            return _make_err(5);
        }

        let v = view_slice(fdv, pos, vlen);
        if fdf == 1 && fdwt == 2 {
            f_name = view_to_bytes(v);
            0
        } else if (fdf == 3 || fdf == 4 || fdf == 5) && fdwt == 0 {
            let st = _varint_u32_status(v, 0);
            let stv = bytes_view(st);
            if is_err(stv) {
                return view_to_bytes(stv);
            }
            let len_used = codec_read_u32_le(stv, 5);
            if len_used != vlen {
                return _make_err(5);
            }
            let vv = codec_read_u32_le(stv, 1);
            if fdf == 3 {
                f_number = vv;
            } else if fdf == 4 {
                f_label = vv;
            } else {
                f_type = vv;
            }
            0
        } else if fdf == 6 && fdwt == 2 {
            let mut tn = v;
            if lt_u(0, view_len(tn)) && view_get_u8(tn, 0) == 46 {
                tn = view_slice(tn, 1, view_len(tn) - 1);
                0
            }
            f_type_name = view_to_bytes(tn);
            0
        }

        pos = pos + vlen;
    }
    if pos != n {
        return _make_err(5);
    }

    if bytes_len(f_name) == 0 || f_number == 0 || f_label == 0 || f_type == 0 {
        return _make_err(5);
    }

    let fn_len = bytes_len(f_name);
    let tn_len = bytes_len(f_type_name);
    let payload_len = 4 + 1 + 1 + 4 + fn_len + 4 + tn_len;
    let mut out = vec_u8_with_capacity(payload_len + 1);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(f_number));
    out = vec_u8_push(out, f_label & 255);
    out = vec_u8_push(out, f_type & 255);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(fn_len));
    out = vec_u8_extend_bytes(out, bytes_view(f_name));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(tn_len));
    out = vec_u8_extend_bytes(out, bytes_view(f_type_name));
    vec_u8_into_bytes(out)
}

fn _schema_message_entry_v1(msg_bytes: BytesView, package: BytesView, max_out_bytes: i32) -> Bytes {
    let msg_doc = decode_v1(msg_bytes, max_out_bytes);
    let msgv = bytes_view(msg_doc);
    if is_err(msgv) {
        return view_to_bytes(msgv);
    }
    let n = view_len(msgv);
    if lt_u(n, 5) {
        return _make_err(5);
    }
    let count = codec_read_u32_le(msgv, 1);
    if count < 0 {
        return _make_err(5);
    }

    let mut msg_name = bytes_alloc(0);
    let mut fields_out = vec_u8_with_capacity(64);
    let mut field_count = 0;

    let mut pos = 5;
    for _ in 0..count {
        if ge_u(pos + 9, n + 1) {
            return _make_err(5);
        }
        let mno = codec_read_u32_le(msgv, pos);
        pos = pos + 4;
        let mwt = view_get_u8(msgv, pos);
        pos = pos + 1;
        let vlen = codec_read_u32_le(msgv, pos);
        pos = pos + 4;
        if vlen < 0 {
            return _make_err(5);
        }
        if ge_u(pos + vlen, n + 1) {
            return _make_err(5);
        }

        if mno == 1 && mwt == 2 {
            msg_name = view_to_bytes(view_slice(msgv, pos, vlen));
            0
        } else if mno == 2 && mwt == 2 {
            let fd_bytes = view_slice(msgv, pos, vlen);
            let fd_entry = _schema_field_entry_v1(fd_bytes, max_out_bytes);
            let fdv = bytes_view(fd_entry);
            if is_err(fdv) {
                return view_to_bytes(fdv);
            }
            let fdn = view_len(fdv);
            if lt_u(fdn, 2) {
                return _make_err(5);
            }
            fields_out = vec_u8_extend_bytes(fields_out, view_slice(fdv, 1, fdn - 1));
            field_count = field_count + 1;
            0
        }

        pos = pos + vlen;
    }
    if pos != n {
        return _make_err(5);
    }
    if bytes_len(msg_name) == 0 {
        return _make_err(5);
    }

    let pkg_len = view_len(package);
    let name_len = bytes_len(msg_name);
    let full_len = if pkg_len == 0 { name_len } else { pkg_len + 1 + name_len };
    let mut full_name = vec_u8_with_capacity(full_len);
    if pkg_len != 0 {
        full_name = vec_u8_extend_bytes(full_name, package);
        full_name = vec_u8_push(full_name, 46);
        0
    }
    full_name = vec_u8_extend_bytes(full_name, bytes_view(msg_name));
    let full_b = vec_u8_into_bytes(full_name);

    let fields_b = vec_u8_into_bytes(fields_out);
    let fields_len = bytes_len(fields_b);
    let payload_len = 4 + full_len + 4 + fields_len;
    let mut out = vec_u8_with_capacity(payload_len + 1);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(full_len));
    out = vec_u8_extend_bytes(out, bytes_view(full_b));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(field_count));
    out = vec_u8_extend_bytes(out, bytes_view(fields_b));
    vec_u8_into_bytes(out)
}

fn _schema_file_entries_v1(file_bytes: BytesView, max_out_bytes: i32) -> Bytes {
    let file_doc = decode_v1(file_bytes, max_out_bytes);
    let filev = bytes_view(file_doc);
    if is_err(filev) {
        return view_to_bytes(filev);
    }
    let n = view_len(filev);
    if lt_u(n, 5) {
        return _make_err(5);
    }
    let count = codec_read_u32_le(filev, 1);
    if count < 0 {
        return _make_err(5);
    }

    let mut package = bytes_alloc(0);
    let mut msgs_out = vec_u8_with_capacity(64);
    let mut msg_count = 0;

    let mut pos = 5;
    for _ in 0..count {
        if ge_u(pos + 9, n + 1) {
            return _make_err(5);
        }
        let fno = codec_read_u32_le(filev, pos);
        pos = pos + 4;
        let fwt = view_get_u8(filev, pos);
        pos = pos + 1;
        let vlen = codec_read_u32_le(filev, pos);
        pos = pos + 4;
        if vlen < 0 {
            return _make_err(5);
        }
        if ge_u(pos + vlen, n + 1) {
            return _make_err(5);
        }

        if fno == 2 && fwt == 2 {
            package = view_to_bytes(view_slice(filev, pos, vlen));
            0
        } else if fno == 4 && fwt == 2 {
            let msg_bytes = view_slice(filev, pos, vlen);
            let msg_entry = _schema_message_entry_v1(msg_bytes, bytes_view(package), max_out_bytes);
            let mv = bytes_view(msg_entry);
            if is_err(mv) {
                return view_to_bytes(mv);
            }
            let mn = view_len(mv);
            if lt_u(mn, 2) {
                return _make_err(5);
            }
            msgs_out = vec_u8_extend_bytes(msgs_out, view_slice(mv, 1, mn - 1));
            msg_count = msg_count + 1;
            0
        }

        pos = pos + vlen;
    }
    if pos != n {
        return _make_err(5);
    }

    let msgs_b = vec_u8_into_bytes(msgs_out);
    let msgs_len = bytes_len(msgs_b);
    let payload_len = 4 + msgs_len;
    let mut out = vec_u8_with_capacity(payload_len + 1);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(msg_count));
    out = vec_u8_extend_bytes(out, bytes_view(msgs_b));
    vec_u8_into_bytes(out)
}

pub fn descriptor_set_to_schema_v1(desc: BytesView, max_out_bytes: i32) -> Bytes {
    // SchemaDocV1 layout (ok):
    //  [1][ver=1][msg_count u32le]
    //  repeated msg:
    //    [name_len u32le][name bytes]
    //    [field_count u32le]
    //    repeated field:
    //      [field_no u32le][label u8][type u8]
    //      [field_name_len u32le][field_name bytes]
    //      [type_name_len u32le][type_name bytes]   // leading '.' stripped
    //
    // Error codes (in addition to ext.pb.wire.decode_v1 errors):
    //  5 = ERR_BAD_DESCRIPTOR
    if max_out_bytes < 0 {
        return _make_err(4);
    }

    let raw = decode_v1(desc, max_out_bytes);
    let rawv = bytes_view(raw);
    if is_err(rawv) {
        return view_to_bytes(rawv);
    }
    let raw_len = view_len(rawv);
    if lt_u(raw_len, 5) {
        return _make_err(5);
    }
    let count = codec_read_u32_le(rawv, 1);
    if count < 0 {
        return _make_err(5);
    }

    let mut out = vec_u8_with_capacity(64);
    out = vec_u8_push(out, 1);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    let mut out_payload_len = 5;

    let mut msg_count = 0;

    let mut pos = 5;
    for _ in 0..count {
        if ge_u(pos + 9, raw_len + 1) {
            return _make_err(5);
        }
        let field_no = codec_read_u32_le(rawv, pos);
        pos = pos + 4;
        let wire_type = view_get_u8(rawv, pos);
        pos = pos + 1;
        let vlen = codec_read_u32_le(rawv, pos);
        pos = pos + 4;
        if vlen < 0 {
            return _make_err(5);
        }
        if ge_u(pos + vlen, raw_len + 1) {
            return _make_err(5);
        }

        if field_no == 1 && wire_type == 2 {
            let file_bytes = view_slice(rawv, pos, vlen);
            let file_doc = _schema_file_entries_v1(file_bytes, max_out_bytes);
            let fv = bytes_view(file_doc);
            if is_err(fv) {
                return view_to_bytes(fv);
            }
            let fnn = view_len(fv);
            if lt_u(fnn, 5) {
                return _make_err(5);
            }
            let file_msg_count = codec_read_u32_le(fv, 1);
            if file_msg_count < 0 {
                return _make_err(5);
            }

            let entries_len = fnn - 5;
            if entries_len != 0 {
                if lt_u(max_out_bytes, out_payload_len + entries_len) {
                    return _make_err(4);
                }
                out = vec_u8_extend_bytes(out, view_slice(fv, 5, entries_len));
                out_payload_len = out_payload_len + entries_len;
            }
            msg_count = msg_count + file_msg_count;
        }

        pos = pos + vlen;
    }
    if pos != raw_len {
        return _make_err(5);
    }

    let mut b = vec_u8_into_bytes(out);
    let c = codec_write_u32_le(msg_count);
    b = bytes_set_u8(b, 2, bytes_get_u8(c, 0));
    b = bytes_set_u8(b, 3, bytes_get_u8(c, 1));
    b = bytes_set_u8(b, 4, bytes_get_u8(c, 2));
    b = bytes_set_u8(b, 5, bytes_get_u8(c, 3));
    b
}
