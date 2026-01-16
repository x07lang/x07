fn _is_upper_ascii(c: i32) -> bool {
    ge_u(c, 65) && lt_u(c, 91)
}

fn _ascii_lower(c: i32) -> i32 {
    if _is_upper_ascii(c) {
        c + 32
    } else {
        c
    }
}

fn _append_http_11(mut out: VecU8) -> VecU8 {
    out = vec_u8_push(out, 72);
    out = vec_u8_push(out, 84);
    out = vec_u8_push(out, 84);
    out = vec_u8_push(out, 80);
    out = vec_u8_push(out, 47);
    out = vec_u8_push(out, 49);
    out = vec_u8_push(out, 46);
    out = vec_u8_push(out, 49);
    out
}

fn _append_crlf(mut out: VecU8) -> VecU8 {
    out = vec_u8_push(out, 13);
    out = vec_u8_push(out, 10);
    out
}

fn _ends_with_crlf(b: BytesView) -> bool {
    let n = view_len(b);
    if lt_u(n, 2) {
        return false;
    }
    view_get_u8(b, n - 2) == 13 && view_get_u8(b, n - 1) == 10
}

fn _ends_with_crlfcrlf(b: BytesView) -> bool {
    let n = view_len(b);
    if lt_u(n, 4) {
        return false;
    }
    view_get_u8(b, n - 4) == 13
        && view_get_u8(b, n - 3) == 10
        && view_get_u8(b, n - 2) == 13
        && view_get_u8(b, n - 1) == 10
}

pub fn lowercase_ascii(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(n);
    for i in 0..n {
        let c = view_get_u8(b, i);
        out = vec_u8_push(out, _ascii_lower(c));
    }
    vec_u8_into_bytes(out)
}

pub fn header_line(name: BytesView, value: BytesView) -> Bytes {
    let n_len = view_len(name);
    let v_len = view_len(value);
    let mut out = vec_u8_with_capacity(n_len + v_len + 4);
    out = vec_u8_extend_bytes(out, name);
    out = vec_u8_push(out, 58);
    out = vec_u8_push(out, 32);
    out = vec_u8_extend_bytes(out, value);
    out = _append_crlf(out);
    vec_u8_into_bytes(out)
}

pub fn build_request(method: BytesView, target: BytesView, headers: BytesView, body: BytesView) -> Bytes {
    let method_len = view_len(method);
    let target_len = view_len(target);
    let headers_len = view_len(headers);
    let body_len = view_len(body);
    let mut out = vec_u8_with_capacity(method_len + target_len + headers_len + body_len + 32);

    out = vec_u8_extend_bytes(out, method);
    out = vec_u8_push(out, 32);
    out = vec_u8_extend_bytes(out, target);
    out = vec_u8_push(out, 32);
    out = _append_http_11(out);
    out = _append_crlf(out);

    out = vec_u8_extend_bytes(out, headers);
    if headers_len == 0 {
        out = _append_crlf(out);
        0
    } else if _ends_with_crlfcrlf(headers) {
        0
    } else if _ends_with_crlf(headers) {
        out = _append_crlf(out);
        0
    } else {
        out = _append_crlf(out);
        out = _append_crlf(out);
        0
    }

    out = vec_u8_extend_bytes(out, body);
    vec_u8_into_bytes(out)
}

pub fn build_response(status: i32, reason: BytesView, headers: BytesView, body: BytesView) -> Bytes {
    let status_b = fmt_u32_to_dec(status);
    let status_v = bytes_view(status_b);

    let reason_len = view_len(reason);
    let headers_len = view_len(headers);
    let body_len = view_len(body);
    let mut out = vec_u8_with_capacity(reason_len + headers_len + body_len + 48);

    out = _append_http_11(out);
    out = vec_u8_push(out, 32);
    out = vec_u8_extend_bytes(out, status_v);
    out = vec_u8_push(out, 32);
    out = vec_u8_extend_bytes(out, reason);
    out = _append_crlf(out);

    out = vec_u8_extend_bytes(out, headers);
    if headers_len == 0 {
        out = _append_crlf(out);
        0
    } else if _ends_with_crlfcrlf(headers) {
        0
    } else if _ends_with_crlf(headers) {
        out = _append_crlf(out);
        0
    } else {
        out = _append_crlf(out);
        out = _append_crlf(out);
        0
    }

    out = vec_u8_extend_bytes(out, body);
    vec_u8_into_bytes(out)
}
