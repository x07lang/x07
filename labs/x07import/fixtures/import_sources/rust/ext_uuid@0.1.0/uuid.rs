fn _hex_digit(n: i32) -> i32 {
    if lt_u(n, 10) {
        return 48 + n;
    }
    87 + n
}

fn _from_hex_digit(c: i32) -> i32 {
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return c - 48;
        }
    }
    if ge_u(c, 65) {
        if lt_u(c, 71) {
            return (c - 65) + 10;
        }
    }
    if ge_u(c, 97) {
        if lt_u(c, 103) {
            return (c - 97) + 10;
        }
    }
    0 - 1
}

fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

pub fn uuid_parse(s: BytesView) -> Bytes {
    let n = view_len(s);
    if n != 36 {
        return _make_err(1);
    }

    let mut out = vec_u8_with_capacity(17);
    out = vec_u8_push(out, 1);

    let mut have_hi = 0;
    let mut hi: i32 = 0;

    for i in 0..n {
        let c = view_get_u8(s, i);

        if i == 8 || i == 13 || i == 18 || i == 23 {
            if c != 45 {
                return _make_err(2);
            }
        } else {
            let v = _from_hex_digit(c);
            if v < 0 {
                return _make_err(3);
            }
            if have_hi == 0 {
                hi = v;
                have_hi = 1;
            } else {
                out = vec_u8_push(out, (hi << 4) + v);
                have_hi = 0;
            }
        }
    }

    if have_hi != 0 {
        return _make_err(2);
    }

    vec_u8_into_bytes(out)
}

pub fn uuid_format(uuid: BytesView) -> Bytes {
    let n = view_len(uuid);
    if n != 16 {
        return _make_err(1);
    }

    let mut out = vec_u8_with_capacity(37);
    out = vec_u8_push(out, 1);

    for i in 0..n {
        if i == 4 || i == 6 || i == 8 || i == 10 {
            out = vec_u8_push(out, 45);
        }
        let c = view_get_u8(uuid, i);
        out = vec_u8_push(out, _hex_digit(c / 16));
        out = vec_u8_push(out, _hex_digit(c % 16));
    }

    vec_u8_into_bytes(out)
}

pub fn uuid_is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn uuid_err_code(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn uuid_get_bytes(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 17 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 1, 16))
}

pub fn uuid_get_string(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 37 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 1, 36))
}
