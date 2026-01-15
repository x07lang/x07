fn _make_err(code: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(9);
    out = vec_u8_push(out, 0);
    out = vec_u8_extend_bytes(out, codec_write_u32_le(code));
    out = vec_u8_extend_bytes(out, codec_write_u32_le(0));
    vec_u8_into_bytes(out)
}

fn _oct_digit(c: i32) -> i32 {
    if ge_u(c, 48) && lt_u(c, 56) {
        return c - 48;
    }
    0 - 1
}

fn _parse_octal_u32(header: BytesView, off: i32, len: i32) -> i32 {
    let mut i = off;
    let end = off + len;
    let mut val = 0;
    let mut started = 0;
    let mut done = 0;
    for _ in off..end {
        if done == 0 {
            let c = view_get_u8(header, i);
            if started == 0 {
                if c == 0 || c == 32 {
                    i = i + 1;
                } else {
                    started = 1;
                }
            }
            if started != 0 && done == 0 {
                if c == 0 || c == 32 {
                    done = 1;
                } else {
                    let d = _oct_digit(c);
                    if d < 0 {
                        return 0 - 1;
                    }
                    val = (val * 8) + d;
                    if val < 0 {
                        return 0 - 1;
                    }
                    i = i + 1;
                }
            }
        }
    }
    val
}

fn _field_len_until_nul(header: BytesView, off: i32, max_len: i32) -> i32 {
    let mut i = 0;
    let mut done = 0;
    for _ in 0..max_len {
        if done == 0 {
            let c = view_get_u8(header, off + i);
            if c == 0 {
                done = 1;
            } else {
                i = i + 1;
            }
        }
    }
    i
}

fn _match_name(header: BytesView, target: BytesView) -> bool {
    let name_off = 0;
    let prefix_off = 345;
    let name_len = _field_len_until_nul(header, name_off, 100);
    let prefix_len = _field_len_until_nul(header, prefix_off, 155);
    let tlen = view_len(target);

    if prefix_len == 0 {
        if tlen != name_len {
            return false;
        }
        for i in 0..name_len {
            if view_get_u8(target, i) != view_get_u8(header, name_off + i) {
                return false;
            }
        }
        return true;
    }

    let expected = prefix_len + 1 + name_len;
    if tlen != expected {
        return false;
    }
    for i in 0..prefix_len {
        if view_get_u8(target, i) != view_get_u8(header, prefix_off + i) {
            return false;
        }
    }
    if view_get_u8(target, prefix_len) != 47 {
        return false;
    }
    for i in 0..name_len {
        if view_get_u8(target, prefix_len + 1 + i) != view_get_u8(header, name_off + i) {
            return false;
        }
    }
    true
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

pub fn get_bytes(doc: BytesView) -> Bytes {
    let n = view_len(doc);
    if n < 1 {
        return bytes_alloc(0);
    }
    if view_get_u8(doc, 0) != 1 {
        return bytes_alloc(0);
    }
    view_to_bytes(view_slice(doc, 1, n - 1))
}

pub fn find_file_v1(tar: BytesView, name: BytesView, max_size: i32) -> Bytes {
    // Error codes:
    //  1 = ERR_NOT_FOUND
    //  2 = ERR_INVALID_ARCHIVE
    //  3 = ERR_TOO_LARGE
    //  4 = ERR_INVALID_PARAM
    if max_size < 0 {
        return _make_err(4);
    }

    let n = view_len(tar);
    let mut pos = 0;
    for _ in 0..(n + 1) {
        if ge_u(pos, n) {
            return _make_err(1);
        }
        if lt_u(n, pos + 512) {
            return _make_err(2);
        }

        let header = view_slice(tar, pos, 512);
        if view_get_u8(header, 0) == 0 {
            return _make_err(1);
        }

        let typeflag = view_get_u8(header, 156);
        let size = _parse_octal_u32(header, 124, 12);
        if size < 0 {
            return _make_err(2);
        }
        let data_start = pos + 512;
        if lt_u(n, data_start + size) {
            return _make_err(2);
        }

        if (typeflag == 0 || typeflag == 48) && _match_name(header, name) {
            if lt_u(max_size, size) {
                return _make_err(3);
            }
            let mut out = vec_u8_with_capacity(1 + size);
            out = vec_u8_push(out, 1);
            out = vec_u8_extend_bytes_range(out, tar, data_start, size);
            return vec_u8_into_bytes(out);
        }

        let padded = ((size + 511) / 512) * 512;
        pos = data_start + padded;
    }

    _make_err(2)
}

