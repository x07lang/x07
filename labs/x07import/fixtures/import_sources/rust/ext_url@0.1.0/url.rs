fn _is_scheme_char(c: i32, first: bool) -> bool {
    if ge_u(c, 97) {
        if lt_u(c, 123) {
            return true;
        }
    }
    if ge_u(c, 65) {
        if lt_u(c, 91) {
            return true;
        }
    }
    if first {
        return false;
    }
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return true;
        }
    }
    if c == 43 || c == 45 || c == 46 {
        return true;
    }
    false
}

fn _push_u32_le(mut out: VecU8, x: i32) -> VecU8 {
    let b = codec_write_u32_le(x);
    out = vec_u8_extend_bytes_range(out, b, 0, bytes_len(b));
    out
}

fn _find_scheme_end(b: BytesView) -> i32 {
    let n = view_len(b);
    let mut scheme_end = 0;
    let mut found_colon = false;
    for i in 0..n {
        if !found_colon {
            let c = view_get_u8(b, i);
            if c == 58 {
                scheme_end = i;
                found_colon = true;
            } else if !_is_scheme_char(c, i == 0) {
                scheme_end = i;
                found_colon = true;
            } else {
                scheme_end = i + 1;
            }
        }
    }
    scheme_end
}

fn _find_path_start(b: BytesView, authority_start: i32) -> i32 {
    let n = view_len(b);
    for i in authority_start..n {
        let c = view_get_u8(b, i);
        if c == 47 || c == 63 || c == 35 {
            return i;
        }
    }
    n
}

fn _find_host_end_and_port(b: BytesView, authority_start: i32, path_start: i32) -> i32 {
    let mut host_end = authority_start;
    for i in authority_start..path_start {
        let c = view_get_u8(b, i);
        if c == 58 {
            return i;
        }
        host_end = i + 1;
    }
    host_end
}

fn _find_query_start(b: BytesView, path_start: i32) -> i32 {
    let n = view_len(b);
    for i in path_start..n {
        let c = view_get_u8(b, i);
        if c == 63 {
            return i + 1;
        }
        if c == 35 {
            return n;
        }
    }
    n
}

fn _find_frag_start(b: BytesView, query_or_path_start: i32) -> i32 {
    let n = view_len(b);
    for i in query_or_path_start..n {
        if view_get_u8(b, i) == 35 {
            return i + 1;
        }
    }
    n
}

fn _find_path_end(b: BytesView, path_start: i32, query_start: i32, frag_start: i32) -> i32 {
    let n = view_len(b);
    if lt_u(query_start, n) {
        return query_start - 1;
    }
    if lt_u(frag_start, n) {
        return frag_start - 1;
    }
    n
}

pub fn url_parse(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut out = vec_u8_with_capacity(48);

    let scheme_end = _find_scheme_end(b);
    let has_scheme = if lt_u(scheme_end, n) {
        view_get_u8(b, scheme_end) == 58
    } else {
        false
    };

    let scheme_start = 0;
    let scheme_len = if has_scheme { scheme_end } else { 0 };
    out = _push_u32_le(out, scheme_start);
    out = _push_u32_le(out, scheme_len);

    let mut off = if has_scheme { scheme_end + 1 } else { 0 };

    let has_authority = if lt_u(off + 1, n) {
        view_get_u8(b, off) == 47 && view_get_u8(b, off + 1) == 47
    } else {
        false
    };

    if has_authority {
        off = off + 2;
    }

    let authority_start = off;
    let path_start = if has_authority {
        _find_path_start(b, authority_start)
    } else {
        authority_start
    };

    let host_end = if has_authority {
        _find_host_end_and_port(b, authority_start, path_start)
    } else {
        authority_start
    };

    let has_port = if has_authority {
        lt_u(host_end, path_start) && view_get_u8(b, host_end) == 58
    } else {
        false
    };

    let port_start = if has_port { host_end + 1 } else { path_start };

    out = _push_u32_le(out, authority_start);
    out = _push_u32_le(out, host_end - authority_start);

    let port_len = if has_port { path_start - port_start } else { 0 };
    out = _push_u32_le(out, port_start);
    out = _push_u32_le(out, port_len);

    let query_start = _find_query_start(b, path_start);
    let frag_start = _find_frag_start(b, if lt_u(query_start, n) { query_start } else { path_start });
    let path_end = _find_path_end(b, path_start, query_start, frag_start);

    out = _push_u32_le(out, path_start);
    out = _push_u32_le(out, path_end - path_start);

    let query_end = if lt_u(frag_start, n) { frag_start - 1 } else { n };
    let query_len = if lt_u(query_start, n) { query_end - query_start } else { 0 };
    out = _push_u32_le(out, query_start);
    out = _push_u32_le(out, query_len);

    let frag_len = if lt_u(frag_start, n) { n - frag_start } else { 0 };
    out = _push_u32_le(out, frag_start);
    out = _push_u32_le(out, frag_len);

    vec_u8_into_bytes(out)
}

pub fn url_scheme(parsed: BytesView, url: BytesView) -> Bytes {
    if lt_u(view_len(parsed), 8) {
        return bytes_alloc(0);
    }
    let start = codec_read_u32_le(parsed, 0);
    let len = codec_read_u32_le(parsed, 4);
    view_to_bytes(view_slice(url, start, len))
}

pub fn url_host(parsed: BytesView, url: BytesView) -> Bytes {
    if lt_u(view_len(parsed), 16) {
        return bytes_alloc(0);
    }
    let start = codec_read_u32_le(parsed, 8);
    let len = codec_read_u32_le(parsed, 12);
    view_to_bytes(view_slice(url, start, len))
}

pub fn url_port(parsed: BytesView, url: BytesView) -> Bytes {
    if lt_u(view_len(parsed), 24) {
        return bytes_alloc(0);
    }
    let start = codec_read_u32_le(parsed, 16);
    let len = codec_read_u32_le(parsed, 20);
    view_to_bytes(view_slice(url, start, len))
}

pub fn url_path(parsed: BytesView, url: BytesView) -> Bytes {
    if lt_u(view_len(parsed), 32) {
        return bytes_alloc(0);
    }
    let start = codec_read_u32_le(parsed, 24);
    let len = codec_read_u32_le(parsed, 28);
    view_to_bytes(view_slice(url, start, len))
}

pub fn url_query(parsed: BytesView, url: BytesView) -> Bytes {
    if lt_u(view_len(parsed), 40) {
        return bytes_alloc(0);
    }
    let start = codec_read_u32_le(parsed, 32);
    let len = codec_read_u32_le(parsed, 36);
    view_to_bytes(view_slice(url, start, len))
}

pub fn url_fragment(parsed: BytesView, url: BytesView) -> Bytes {
    if lt_u(view_len(parsed), 48) {
        return bytes_alloc(0);
    }
    let start = codec_read_u32_le(parsed, 40);
    let len = codec_read_u32_le(parsed, 44);
    view_to_bytes(view_slice(url, start, len))
}
