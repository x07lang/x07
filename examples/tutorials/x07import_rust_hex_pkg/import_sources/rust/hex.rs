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

pub fn hex_encode(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(n * 2);
    for i in 0..n {
        let c = view_get_u8(b, i);
        out = vec_u8_push(out, _hex_digit(c / 16));
        out = vec_u8_push(out, _hex_digit(c % 16));
    }
    vec_u8_into_bytes(out)
}

pub fn hex_decode(s: BytesView) -> Bytes {
    let n = view_len(s);
    if n % 2 != 0 {
        return _make_err(1);
    }

    let mut out = vec_u8_with_capacity(1 + (n / 2));
    out = vec_u8_push(out, 1);

    let mut i = 0;
    for _ in 0..n {
        if lt_u(i, n) {
            let hi = _from_hex_digit(view_get_u8(s, i));
            let lo = _from_hex_digit(view_get_u8(s, i + 1));
            if hi < 0 || lo < 0 {
                return _make_err(2);
            }
            out = vec_u8_push(out, (hi << 4) + lo);
            i = i + 2;
        }
    }

    vec_u8_into_bytes(out)
}

pub fn hex_is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn hex_err_code(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn hex_get_bytes(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 1 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 1, n - 1))
}
