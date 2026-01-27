fn _is_unreserved(c: i32) -> bool {
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
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return true;
        }
    }
    if c == 45 || c == 46 || c == 95 || c == 126 {
        return true;
    }
    false
}

fn _hex_digit(n: i32) -> i32 {
    if lt_u(n, 10) {
        48 + n
    } else {
        65 + n - 10
    }
}

fn _from_hex_digit(c: i32) -> i32 {
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return c - 48;
        }
    }
    if ge_u(c, 65) {
        if lt_u(c, 71) {
            return c - 65 + 10;
        }
    }
    if ge_u(c, 97) {
        if lt_u(c, 103) {
            return c - 97 + 10;
        }
    }
    0 - 1
}

pub fn percent_encode(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(n * 3);
    for i in 0..n {
        let c = view_get_u8(b, i);
        if _is_unreserved(c) {
            out = vec_u8_push(out, c);
        } else {
            out = vec_u8_push(out, 37);
            out = vec_u8_push(out, _hex_digit(c / 16));
            out = vec_u8_push(out, _hex_digit(c % 16));
        }
    }
    vec_u8_into_bytes(out)
}

pub fn percent_decode(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(n);
    let mut i = 0;
    for _ in 0..n {
        if lt_u(i, n) {
            let c = view_get_u8(b, i);
            if c == 37 {
                if lt_u(i + 2, n) {
                    let hi = _from_hex_digit(view_get_u8(b, i + 1));
                    let lo = _from_hex_digit(view_get_u8(b, i + 2));
                    if hi >= 0 && lo >= 0 {
                        out = vec_u8_push(out, hi * 16 + lo);
                        i = i + 3;
                    } else {
                        out = vec_u8_push(out, c);
                        i = i + 1;
                    }
                } else {
                    out = vec_u8_push(out, c);
                    i = i + 1;
                }
            } else {
                out = vec_u8_push(out, c);
                i = i + 1;
            }
        }
    }
    vec_u8_into_bytes(out)
}
