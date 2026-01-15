fn _b64_char(n: i32) -> i32 {
    if lt_u(n, 26) {
        return 65 + n;
    }
    if lt_u(n, 52) {
        return 97 + (n - 26);
    }
    if lt_u(n, 62) {
        return 48 + (n - 52);
    }
    if n == 62 {
        return 43;
    }
    47
}

fn _b64_val(c: i32) -> i32 {
    if ge_u(c, 65) {
        if lt_u(c, 91) {
            return c - 65;
        }
    }
    if ge_u(c, 97) {
        if lt_u(c, 123) {
            return (c - 97) + 26;
        }
    }
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return (c - 48) + 52;
        }
    }
    if c == 43 {
        return 62;
    }
    if c == 47 {
        return 63;
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

pub fn base64_encode(b: BytesView) -> Bytes {
    let n = view_len(b);
    let out_len = ((n + 2) / 3) * 4;
    let mut out = vec_u8_with_capacity(out_len);

    let mut i = 0;
    for _ in 0..n {
        if lt_u(i, n) {
            let rem = n - i;
            if rem >= 3 {
                let b0 = view_get_u8(b, i);
                let b1 = view_get_u8(b, i + 1);
                let b2 = view_get_u8(b, i + 2);

                let c0 = (b0 >> 2) & 63;
                let c1 = (((b0 & 3) << 4) | ((b1 >> 4) & 15)) & 63;
                let c2 = (((b1 & 15) << 2) | ((b2 >> 6) & 3)) & 63;
                let c3 = b2 & 63;

                out = vec_u8_push(out, _b64_char(c0));
                out = vec_u8_push(out, _b64_char(c1));
                out = vec_u8_push(out, _b64_char(c2));
                out = vec_u8_push(out, _b64_char(c3));

                i = i + 3;
            } else if rem == 2 {
                let b0 = view_get_u8(b, i);
                let b1 = view_get_u8(b, i + 1);

                let c0 = (b0 >> 2) & 63;
                let c1 = (((b0 & 3) << 4) | ((b1 >> 4) & 15)) & 63;
                let c2 = ((b1 & 15) << 2) & 63;

                out = vec_u8_push(out, _b64_char(c0));
                out = vec_u8_push(out, _b64_char(c1));
                out = vec_u8_push(out, _b64_char(c2));
                out = vec_u8_push(out, 61);

                i = n;
            } else {
                let b0 = view_get_u8(b, i);

                let c0 = (b0 >> 2) & 63;
                let c1 = ((b0 & 3) << 4) & 63;

                out = vec_u8_push(out, _b64_char(c0));
                out = vec_u8_push(out, _b64_char(c1));
                out = vec_u8_push(out, 61);
                out = vec_u8_push(out, 61);

                i = n;
            }
        }
    }

    vec_u8_into_bytes(out)
}

pub fn base64_decode(s: BytesView) -> Bytes {
    let n = view_len(s);
    if n % 4 != 0 {
        return _make_err(1);
    }
    if n == 0 {
        let mut out = vec_u8_with_capacity(1);
        out = vec_u8_push(out, 1);
        return vec_u8_into_bytes(out);
    }

    let mut pad = 0;
    if view_get_u8(s, n - 1) == 61 {
        pad = 1;
        if view_get_u8(s, n - 2) == 61 {
            pad = 2;
        }
    }

    let out_len = ((n / 4) * 3) - pad;
    let mut out = vec_u8_with_capacity(1 + out_len);
    out = vec_u8_push(out, 1);

    let groups = n / 4;
    let mut off = 0;
    for _ in 0..groups {
        if lt_u(off + 3, n) {
            let c0 = view_get_u8(s, off);
            let c1 = view_get_u8(s, off + 1);
            let c2 = view_get_u8(s, off + 2);
            let c3 = view_get_u8(s, off + 3);

            let is_last = off == (n - 4);

            if c2 == 61 {
                if !is_last {
                    return _make_err(3);
                }
                if c3 != 61 {
                    return _make_err(3);
                }
                let v0 = _b64_val(c0);
                let v1 = _b64_val(c1);
                if v0 < 0 || v1 < 0 {
                    return _make_err(2);
                }
                if (v1 & 15) != 0 {
                    return _make_err(3);
                }
                out = vec_u8_push(out, ((v0 << 2) | (v1 >> 4)) & 255);
            } else if c3 == 61 {
                if !is_last {
                    return _make_err(3);
                }
                let v0 = _b64_val(c0);
                let v1 = _b64_val(c1);
                let v2 = _b64_val(c2);
                if v0 < 0 || v1 < 0 || v2 < 0 {
                    return _make_err(2);
                }
                if (v2 & 3) != 0 {
                    return _make_err(3);
                }
                out = vec_u8_push(out, ((v0 << 2) | (v1 >> 4)) & 255);
                out = vec_u8_push(out, (((v1 & 15) << 4) | (v2 >> 2)) & 255);
            } else {
                let v0 = _b64_val(c0);
                let v1 = _b64_val(c1);
                let v2 = _b64_val(c2);
                let v3 = _b64_val(c3);
                if v0 < 0 || v1 < 0 || v2 < 0 || v3 < 0 {
                    return _make_err(2);
                }
                out = vec_u8_push(out, ((v0 << 2) | (v1 >> 4)) & 255);
                out = vec_u8_push(out, (((v1 & 15) << 4) | (v2 >> 2)) & 255);
                out = vec_u8_push(out, (((v2 & 3) << 6) | v3) & 255);
            }

            off = off + 4;
        }
    }

    vec_u8_into_bytes(out)
}

pub fn base64_is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn base64_err_code(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn base64_get_bytes(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 1 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 1, n - 1))
}
