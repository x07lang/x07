fn _push_u32_le(out: VecU8, x: i32) -> VecU8 {
    vec_u8_extend_bytes(out, codec_write_u32_le(x))
}

fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

fn _is_ows(c: i32) -> bool {
    c == 32 || c == 9
}

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

fn _find_byte(b: BytesView, start: i32, end: i32, needle: i32) -> i32 {
    for i in start..end {
        if view_get_u8(b, i) == needle {
            return i;
        }
    }
    0 - 1
}

fn _find_crlf(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    if ge_u(start + 1, n) {
        return 0 - 1;
    }
    for i in start..(n - 1) {
        if view_get_u8(b, i) == 13 {
            if view_get_u8(b, i + 1) == 10 {
                return i;
            }
        }
    }
    0 - 1
}

fn _skip_ows(b: BytesView, start: i32, end: i32) -> i32 {
    for i in start..end {
        if !_is_ows(view_get_u8(b, i)) {
            return i;
        }
    }
    end
}

fn _trim_end_ows(b: BytesView, start: i32, end: i32) -> i32 {
    if end <= start {
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
                if _is_ows(c) {
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

fn _eq_ascii_case_insensitive(msg: BytesView, msg_start: i32, msg_len: i32, needle: BytesView) -> bool {
    let n = view_len(needle);
    if msg_len != n {
        return false;
    }
    for i in 0..msg_len {
        let a = _ascii_lower(view_get_u8(msg, msg_start + i));
        let b = _ascii_lower(view_get_u8(needle, i));
        if a != b {
            return false;
        }
    }
    true
}

fn _read_u32_le_view(b: BytesView, off: i32) -> i32 {
    let b0 = view_get_u8(b, off);
    let b1 = view_get_u8(b, off + 1);
    let b2 = view_get_u8(b, off + 2);
    let b3 = view_get_u8(b, off + 3);
    b0 + (b1 << 8) + (b2 << 16) + (b3 << 24)
}

// ParseDoc format:
//   Err: [0][u32_le code][u32_le msg_len=0]
//   Ok:
//     [1][u8 kind=1(req)|2(res)][u8 0][u8 0]
//     [u32_le version_start][u32_le version_len]
//     [u32_le method_start][u32_le method_len]
//     [u32_le target_start][u32_le target_len]
//     [u32_le status_code]
//     [u32_le reason_start][u32_le reason_len]
//     [u32_le header_count]
//     header entries: [u32_le name_start][u32_le name_len][u32_le value_start][u32_le value_len]...
//     [u32_le body_start][u32_le body_len]

fn _ok_doc(
    kind: i32,
    version_start: i32,
    version_len: i32,
    method_start: i32,
    method_len: i32,
    target_start: i32,
    target_len: i32,
    status_code: i32,
    reason_start: i32,
    reason_len: i32,
    header_count: i32,
    headers_bytes: Bytes,
    body_start: i32,
    body_len: i32,
) -> Bytes {
    let headers_len = bytes_len(headers_bytes);
    let mut out = vec_u8_with_capacity(52 + headers_len);
    out = vec_u8_push(out, 1);
    out = vec_u8_push(out, kind);
    out = vec_u8_push(out, 0);
    out = vec_u8_push(out, 0);
    out = _push_u32_le(out, version_start);
    out = _push_u32_le(out, version_len);
    out = _push_u32_le(out, method_start);
    out = _push_u32_le(out, method_len);
    out = _push_u32_le(out, target_start);
    out = _push_u32_le(out, target_len);
    out = _push_u32_le(out, status_code);
    out = _push_u32_le(out, reason_start);
    out = _push_u32_le(out, reason_len);
    out = _push_u32_le(out, header_count);
    out = vec_u8_extend_bytes(out, bytes_view(headers_bytes));
    out = _push_u32_le(out, body_start);
    out = _push_u32_le(out, body_len);
    vec_u8_into_bytes(out)
}

fn _parse_headers(msg: BytesView, start: i32) -> Bytes {
    let n = view_len(msg);
    let mut entries = vec_u8_with_capacity(0);
    let mut count: i32 = 0;
    let mut pos = start;
    let mut done = false;

    // First pass: emit entries into entries vec. The caller will compute body_start separately.
    for _ in 0..(n + 1) {
        if done {
            0
        } else if ge_u(pos + 1, n) {
            return _make_err(1);
        } else if view_get_u8(msg, pos) == 13 && view_get_u8(msg, pos + 1) == 10 {
            done = true;
            0
        } else {
            let line_end = _find_crlf(msg, pos);
            if line_end < 0 {
                return _make_err(1);
            }
            let colon = _find_byte(msg, pos, line_end, 58);
            if colon <= pos {
                return _make_err(3);
            }
            let name_start = pos;
            let name_len = colon - pos;
            let mut value_start = colon + 1;
            value_start = _skip_ows(msg, value_start, line_end);
            let value_end = _trim_end_ows(msg, value_start, line_end);
            let value_len = if value_end > value_start { value_end - value_start } else { 0 };
            entries = _push_u32_le(entries, name_start);
            entries = _push_u32_le(entries, name_len);
            entries = _push_u32_le(entries, value_start);
            entries = _push_u32_le(entries, value_len);
            count = count + 1;
            pos = line_end + 2;
        }
    }

    // Return entries as a small doc:
    //   Ok:  [1][u32_le count][entries...]
    //   Err: [0][u32_le code][u32_le msg_len=0]
    let entries_b = vec_u8_into_bytes(entries);
    let entries_v = bytes_view(entries_b);
    let entries_len = view_len(entries_v);
    let mut out = vec_u8_with_capacity(1 + 4 + entries_len);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(count));
    out = vec_u8_extend_bytes(out, entries_v);
    vec_u8_into_bytes(out)
}

pub fn parse_request(msg: BytesView) -> Bytes {
    let n = view_len(msg);
    let line_end = _find_crlf(msg, 0);
    if line_end < 0 {
        return _make_err(1);
    }

    let method_end = _find_byte(msg, 0, line_end, 32);
    if method_end <= 0 {
        return _make_err(2);
    }
    let method_start = 0;
    let method_len = method_end - method_start;

    let mut target_start = _skip_ows(msg, method_end + 1, line_end);
    let target_end = _find_byte(msg, target_start, line_end, 32);
    if target_end <= target_start {
        return _make_err(2);
    }
    let target_len = target_end - target_start;

    let version_start = _skip_ows(msg, target_end + 1, line_end);
    if ge_u(version_start, line_end) {
        return _make_err(2);
    }
    let version_end = _trim_end_ows(msg, version_start, line_end);
    let version_len = if version_end > version_start { version_end - version_start } else { 0 };

    let headers_start = line_end + 2;
    let headers_doc_b = _parse_headers(msg, headers_start);
    let headers_doc_len = bytes_len(headers_doc_b);
    if lt_u(headers_doc_len, 1) {
        return _make_err(1);
    }
    if bytes_get_u8(headers_doc_b, 0) == 0 {
        return headers_doc_b;
    }
    if lt_u(headers_doc_len, 5) {
        return _make_err(1);
    }
    let header_count = _read_u32_le_view(bytes_view(headers_doc_b), 1);

    // Find body_start by scanning for the header terminator.
    let mut pos = headers_start;
    let mut body_start = 0;
    let mut done = false;
    for _ in 0..(n + 1) {
        if done {
            0
        } else if ge_u(pos + 1, n) {
            return _make_err(1);
        } else if view_get_u8(msg, pos) == 13 && view_get_u8(msg, pos + 1) == 10 {
            body_start = pos + 2;
            done = true;
            0
        } else {
            let e = _find_crlf(msg, pos);
            if e < 0 {
                return _make_err(1);
            }
            pos = e + 2;
        }
    }
    if !done {
        return _make_err(1);
    }
    let body_len = if body_start <= n { n - body_start } else { 0 };

    // Strip the [ok][count] prefix from headers_doc, leaving only entry bytes.
    let headers_payload_b = bytes_slice(headers_doc_b, 5, headers_doc_len - 5);

    _ok_doc(
        1,
        version_start,
        version_len,
        method_start,
        method_len,
        target_start,
        target_len,
        0,
        0,
        0,
        header_count,
        headers_payload_b,
        body_start,
        body_len,
    )
}

pub fn parse_response(msg: BytesView) -> Bytes {
    let n = view_len(msg);
    let line_end = _find_crlf(msg, 0);
    if line_end < 0 {
        return _make_err(1);
    }

    let version_end = _find_byte(msg, 0, line_end, 32);
    if version_end <= 0 {
        return _make_err(2);
    }
    let version_start = 0;
    let version_len = version_end - version_start;

    let mut status_start = _skip_ows(msg, version_end + 1, line_end);
    if ge_u(status_start, line_end) {
        return _make_err(2);
    }
    let status_end0 = _find_byte(msg, status_start, line_end, 32);
    let status_end = if status_end0 < 0 { line_end } else { status_end0 };
    if status_end <= status_start {
        return _make_err(4);
    }
    let mut status: i32 = 0;
    for i in status_start..status_end {
        let c = view_get_u8(msg, i);
        if !(ge_u(c, 48) && lt_u(c, 58)) {
            return _make_err(4);
        }
        status = (status * 10) + (c - 48);
    }

    let mut reason_start = status_end;
    if reason_start < line_end {
        reason_start = _skip_ows(msg, status_end + 1, line_end);
    }
    let reason_end = _trim_end_ows(msg, reason_start, line_end);
    let reason_len = if reason_end > reason_start { reason_end - reason_start } else { 0 };

    let headers_start = line_end + 2;
    let headers_doc_b = _parse_headers(msg, headers_start);
    let headers_doc_len = bytes_len(headers_doc_b);
    if lt_u(headers_doc_len, 1) {
        return _make_err(1);
    }
    if bytes_get_u8(headers_doc_b, 0) == 0 {
        return headers_doc_b;
    }
    if lt_u(headers_doc_len, 5) {
        return _make_err(1);
    }
    let header_count = _read_u32_le_view(bytes_view(headers_doc_b), 1);

    let mut pos = headers_start;
    let mut body_start = 0;
    let mut done = false;
    for _ in 0..(n + 1) {
        if done {
            0
        } else if ge_u(pos + 1, n) {
            return _make_err(1);
        } else if view_get_u8(msg, pos) == 13 && view_get_u8(msg, pos + 1) == 10 {
            body_start = pos + 2;
            done = true;
            0
        } else {
            let e = _find_crlf(msg, pos);
            if e < 0 {
                return _make_err(1);
            }
            pos = e + 2;
        }
    }
    if !done {
        return _make_err(1);
    }
    let body_len = if body_start <= n { n - body_start } else { 0 };

    let headers_payload_b = bytes_slice(headers_doc_b, 5, headers_doc_len - 5);

    _ok_doc(
        2,
        version_start,
        version_len,
        0,
        0,
        0,
        0,
        status,
        reason_start,
        reason_len,
        header_count,
        headers_payload_b,
        body_start,
        body_len,
    )
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

pub fn kind(doc: BytesView) -> i32 {
    if view_len(doc) < 2 {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }
    view_get_u8(doc, 1)
}

fn _u32_field_or_0(doc: BytesView, off: i32) -> i32 {
    if lt_u(view_len(doc), off + 4) {
        0
    } else {
        codec_read_u32_le(doc, off)
    }
}

pub fn version(doc: BytesView, msg: BytesView) -> Bytes {
    if kind(doc) == 0 {
        return bytes_alloc(0);
    }
    let start = _u32_field_or_0(doc, 4);
    let len = _u32_field_or_0(doc, 8);
    view_to_bytes(view_slice(msg, start, len))
}

pub fn method(doc: BytesView, msg: BytesView) -> Bytes {
    if kind(doc) != 1 {
        return bytes_alloc(0);
    }
    let start = _u32_field_or_0(doc, 12);
    let len = _u32_field_or_0(doc, 16);
    view_to_bytes(view_slice(msg, start, len))
}

pub fn target(doc: BytesView, msg: BytesView) -> Bytes {
    if kind(doc) != 1 {
        return bytes_alloc(0);
    }
    let start = _u32_field_or_0(doc, 20);
    let len = _u32_field_or_0(doc, 24);
    view_to_bytes(view_slice(msg, start, len))
}

pub fn status_code(doc: BytesView) -> i32 {
    if kind(doc) != 2 {
        return 0;
    }
    _u32_field_or_0(doc, 28)
}

pub fn reason(doc: BytesView, msg: BytesView) -> Bytes {
    if kind(doc) != 2 {
        return bytes_alloc(0);
    }
    let start = _u32_field_or_0(doc, 32);
    let len = _u32_field_or_0(doc, 36);
    view_to_bytes(view_slice(msg, start, len))
}

pub fn header_count(doc: BytesView) -> i32 {
    if kind(doc) == 0 {
        return 0;
    }
    _u32_field_or_0(doc, 40)
}

fn _header_entry_field(doc: BytesView, idx: i32, field: i32) -> i32 {
    let count = header_count(doc);
    if idx < 0 || idx >= count {
        return 0;
    }
    let base = 44 + (idx * 16) + (field * 4);
    _u32_field_or_0(doc, base)
}

pub fn header_name(doc: BytesView, msg: BytesView, idx: i32) -> Bytes {
    if kind(doc) == 0 {
        return bytes_alloc(0);
    }
    let start = _header_entry_field(doc, idx, 0);
    let len = _header_entry_field(doc, idx, 1);
    view_to_bytes(view_slice(msg, start, len))
}

pub fn header_value(doc: BytesView, msg: BytesView, idx: i32) -> Bytes {
    if kind(doc) == 0 {
        return bytes_alloc(0);
    }
    let start = _header_entry_field(doc, idx, 2);
    let len = _header_entry_field(doc, idx, 3);
    view_to_bytes(view_slice(msg, start, len))
}

pub fn header_get(doc: BytesView, msg: BytesView, name: BytesView) -> Bytes {
    if kind(doc) == 0 {
        return bytes_alloc(0);
    }
    let count = header_count(doc);
    for i in 0..count {
        let n_start = _header_entry_field(doc, i, 0);
        let n_len = _header_entry_field(doc, i, 1);
        if _eq_ascii_case_insensitive(msg, n_start, n_len, name) {
            let v_start = _header_entry_field(doc, i, 2);
            let v_len = _header_entry_field(doc, i, 3);
            return view_to_bytes(view_slice(msg, v_start, v_len));
        }
    }
    bytes_alloc(0)
}

pub fn body(doc: BytesView, msg: BytesView) -> Bytes {
    if kind(doc) == 0 {
        return bytes_alloc(0);
    }
    let count = header_count(doc);
    let base = 44 + (count * 16);
    let start = _u32_field_or_0(doc, base);
    let len = _u32_field_or_0(doc, base + 4);
    view_to_bytes(view_slice(msg, start, len))
}
