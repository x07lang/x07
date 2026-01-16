fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

fn _is_digit(c: i32) -> bool {
    if ge_u(c, 48) {
        if lt_u(c, 58) {
            return true;
        }
    }
    false
}

fn _is_alpha(c: i32) -> bool {
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
    false
}

fn _is_ident_char(c: i32) -> bool {
    if _is_digit(c) {
        return true;
    }
    if _is_alpha(c) {
        return true;
    }
    c == 45
}

fn _parse_u31(b: BytesView, start: i32, end: i32) -> i32 {
    if end <= start {
        return 0 - 1;
    }
    let len = end - start;
    if len > 1 {
        if view_get_u8(b, start) == 48 {
            return 0 - 1;
        }
    }

    let mut val: i32 = 0;
    for i in start..end {
        let c = view_get_u8(b, i);
        if !_is_digit(c) {
            return 0 - 1;
        }
        let digit = c - 48;

        if val > 214748364 {
            return 0 - 1;
        }
        if val == 214748364 {
            if digit > 7 {
                return 0 - 1;
            }
        }

        val = (val * 10) + digit;
    }

    val
}

fn _find_dot_or_end(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    for i in start..n {
        if view_get_u8(b, i) == 46 {
            return i;
        }
    }
    n
}

fn _find_plus_or_end(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    for i in start..n {
        if view_get_u8(b, i) == 43 {
            return i;
        }
    }
    n
}

fn _find_end_of_patch(b: BytesView, start: i32) -> i32 {
    let n = view_len(b);
    for i in start..n {
        let c = view_get_u8(b, i);
        if c == 45 || c == 43 {
            return i;
        }
        if !_is_digit(c) {
            return 0 - 1;
        }
    }
    n
}

fn _validate_ident(b: BytesView, start: i32, end: i32, disallow_leading_zeros_numeric: i32) -> i32 {
    if end <= start {
        return 0;
    }

    let mut is_numeric = 1;
    for i in start..end {
        let c = view_get_u8(b, i);
        if !_is_ident_char(c) {
            return 0;
        }
        if !_is_digit(c) {
            is_numeric = 0;
        }
    }

    if disallow_leading_zeros_numeric == 1 && is_numeric == 1 {
        if (end - start) > 1 {
            if view_get_u8(b, start) == 48 {
                return 0;
            }
        }
    }

    1
}

fn _validate_dot_separated(b: BytesView, start: i32, end: i32, disallow_leading_zeros_numeric: i32) -> i32 {
    if end <= start {
        return 0;
    }

    let mut seg_start = start;
    for i in start..end {
        if view_get_u8(b, i) == 46 {
            if _validate_ident(b, seg_start, i, disallow_leading_zeros_numeric) == 0 {
                return 0;
            }
            seg_start = i + 1;
        }
    }

    if _validate_ident(b, seg_start, end, disallow_leading_zeros_numeric) == 0 {
        return 0;
    }

    1
}

fn _is_numeric_ident(b: BytesView, start: i32, end: i32) -> i32 {
    if end <= start {
        return 0;
    }
    for i in start..end {
        if !_is_digit(view_get_u8(b, i)) {
            return 0;
        }
    }
    1
}

fn _cmp_bytes(a: BytesView, a_start: i32, a_end: i32, b: BytesView, b_start: i32, b_end: i32) -> i32 {
    let a_len = a_end - a_start;
    let b_len = b_end - b_start;
    let min_len = if a_len < b_len { a_len } else { b_len };
    for i in 0..min_len {
        let ac = view_get_u8(a, a_start + i);
        let bc = view_get_u8(b, b_start + i);
        if ac < bc {
            return 0 - 1;
        }
        if ac > bc {
            return 1;
        }
    }
    if a_len < b_len {
        0 - 1
    } else if a_len > b_len {
        1
    } else {
        0
    }
}

fn _cmp_ident(a: BytesView, a_start: i32, a_end: i32, b: BytesView, b_start: i32, b_end: i32) -> i32 {
    let a_num = _is_numeric_ident(a, a_start, a_end);
    let b_num = _is_numeric_ident(b, b_start, b_end);

    if a_num == 1 && b_num == 1 {
        let a_len = a_end - a_start;
        let b_len = b_end - b_start;
        if a_len < b_len {
            return 0 - 1;
        }
        if a_len > b_len {
            return 1;
        }
        return _cmp_bytes(a, a_start, a_end, b, b_start, b_end);
    }

    if a_num == 1 && b_num == 0 {
        return 0 - 1;
    }
    if a_num == 0 && b_num == 1 {
        return 1;
    }

    _cmp_bytes(a, a_start, a_end, b, b_start, b_end)
}

fn _cmp_prerelease(a: BytesView, b: BytesView) -> i32 {
    let a_len = view_len(a);
    let b_len = view_len(b);

    let mut a_pos = 0;
    let mut b_pos = 0;

    for _ in 0..(a_len + b_len + 2) {
        if ge_u(a_pos, a_len) {
            if ge_u(b_pos, b_len) {
                return 0;
            }
            return 0 - 1;
        }
        if ge_u(b_pos, b_len) {
            return 1;
        }

        let a_dot = _find_dot_or_end(a, a_pos);
        let b_dot = _find_dot_or_end(b, b_pos);

        let cmp = _cmp_ident(a, a_pos, a_dot, b, b_pos, b_dot);
        if cmp != 0 {
            return cmp;
        }

        a_pos = if lt_u(a_dot, a_len) { a_dot + 1 } else { a_dot };
        b_pos = if lt_u(b_dot, b_len) { b_dot + 1 } else { b_dot };
    }

    0
}

fn _cmp_docs(a_doc: BytesView, b_doc: BytesView) -> i32 {
    let a_major = codec_read_u32_le(a_doc, 1);
    let b_major = codec_read_u32_le(b_doc, 1);
    if a_major < b_major {
        return 0 - 1;
    }
    if a_major > b_major {
        return 1;
    }

    let a_minor = codec_read_u32_le(a_doc, 5);
    let b_minor = codec_read_u32_le(b_doc, 5);
    if a_minor < b_minor {
        return 0 - 1;
    }
    if a_minor > b_minor {
        return 1;
    }

    let a_patch = codec_read_u32_le(a_doc, 9);
    let b_patch = codec_read_u32_le(b_doc, 9);
    if a_patch < b_patch {
        return 0 - 1;
    }
    if a_patch > b_patch {
        return 1;
    }

    let a_pre_len = codec_read_u32_le(a_doc, 13);
    let b_pre_len = codec_read_u32_le(b_doc, 13);

    if a_pre_len == 0 {
        if b_pre_len == 0 {
            return 0;
        }
        return 1;
    }
    if b_pre_len == 0 {
        return 0 - 1;
    }

    let a_pre = view_slice(a_doc, 17, a_pre_len);
    let b_pre = view_slice(b_doc, 17, b_pre_len);
    _cmp_prerelease(a_pre, b_pre)
}

pub fn semver_parse(s: BytesView) -> Bytes {
    let n = view_len(s);
    if n < 5 {
        return _make_err(1);
    }

    let dot1 = _find_dot_or_end(s, 0);
    if dot1 == n {
        return _make_err(1);
    }

    let major = _parse_u31(s, 0, dot1);
    if major < 0 {
        return _make_err(2);
    }

    let minor_start = dot1 + 1;
    if ge_u(minor_start, n) {
        return _make_err(1);
    }
    let dot2 = _find_dot_or_end(s, minor_start);
    if dot2 == n {
        return _make_err(1);
    }

    let minor = _parse_u31(s, minor_start, dot2);
    if minor < 0 {
        return _make_err(2);
    }

    let patch_start = dot2 + 1;
    if ge_u(patch_start, n) {
        return _make_err(1);
    }

    let patch_end = _find_end_of_patch(s, patch_start);
    if patch_end < 0 {
        return _make_err(1);
    }

    let patch = _parse_u31(s, patch_start, patch_end);
    if patch < 0 {
        return _make_err(2);
    }

    let mut pre_start: i32 = 0;
    let mut pre_end: i32 = 0;
    let mut has_pre: i32 = 0;
    let mut build_start: i32 = 0;
    let mut build_end: i32 = 0;
    let mut has_build: i32 = 0;

    if lt_u(patch_end, n) {
        let c = view_get_u8(s, patch_end);
        if c == 45 {
            pre_start = patch_end + 1;
            pre_end = _find_plus_or_end(s, pre_start);
            has_pre = 1;

            if _validate_dot_separated(s, pre_start, pre_end, 1) == 0 {
                return _make_err(3);
            }

            if lt_u(pre_end, n) {
                build_start = pre_end + 1;
                build_end = n;
                has_build = 1;
            }
        } else if c == 43 {
            build_start = patch_end + 1;
            build_end = n;
            has_build = 1;
        } else {
            return _make_err(1);
        }
    }

    if has_build == 1 {
        if _validate_dot_separated(s, build_start, build_end, 0) == 0 {
            return _make_err(4);
        }
    }

    let pre_len = if has_pre == 1 { pre_end - pre_start } else { 0 };
    let build_len = if has_build == 1 { build_end - build_start } else { 0 };

    let mut out = vec_u8_with_capacity(21 + pre_len + build_len);
    out = vec_u8_push(out, 1);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(major));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(minor));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(patch));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(pre_len));
    if pre_len > 0 {
        out = vec_u8_extend_bytes_range(out, s, pre_start, pre_len);
    }
    out = vec_u8_extend_bytes(out, codec_write_u32_le(build_len));
    if build_len > 0 {
        out = vec_u8_extend_bytes_range(out, s, build_start, build_len);
    }
    vec_u8_into_bytes(out)
}

pub fn semver_is_err(doc: BytesView) -> bool {
    if view_len(doc) < 1 {
        return true;
    }
    view_get_u8(doc, 0) == 0
}

pub fn semver_err_code(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 0 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn semver_major(doc: BytesView) -> i32 {
    if view_len(doc) < 5 {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }
    codec_read_u32_le(doc, 1)
}

pub fn semver_minor(doc: BytesView) -> i32 {
    if view_len(doc) < 9 {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }
    codec_read_u32_le(doc, 5)
}

pub fn semver_patch(doc: BytesView) -> i32 {
    if view_len(doc) < 13 {
        return 0;
    }
    if view_get_u8(doc, 0) != 1 {
        return 0;
    }
    codec_read_u32_le(doc, 9)
}

pub fn semver_prerelease(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 17 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    let pre_len = codec_read_u32_le(doc, 13);
    if pre_len <= 0 {
        return bytes_alloc(0);
    }
    if lt_u(n, 17 + pre_len) {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 17, pre_len))
}

pub fn semver_build(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 21 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    let pre_len = codec_read_u32_le(doc, 13);
    let build_len_off = 17 + pre_len;
    if lt_u(n, build_len_off + 4) {
        return bytes_alloc(0);
    }
    let build_len = codec_read_u32_le(doc, build_len_off);
    if build_len <= 0 {
        return bytes_alloc(0);
    }
    let build_start = build_len_off + 4;
    if lt_u(n, build_start + build_len) {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, build_start, build_len))
}

pub fn semver_cmp_str(a: BytesView, b: BytesView) -> i32 {
    let a_bytes = semver_parse(a);
    let a_view = bytes_view(a_bytes);
    if semver_is_err(a_view) {
        return 0 - 2;
    }

    let b_bytes = semver_parse(b);
    let b_view = bytes_view(b_bytes);
    if semver_is_err(b_view) {
        return 0 - 2;
    }

    _cmp_docs(a_view, b_view)
}
