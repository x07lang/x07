pub fn is_space_tab(c: i32) -> bool {
    if c == 32 {
        true
    } else if c == 9 {
        true
    } else {
        false
    }
}

fn _is_upper(c: i32) -> bool {
    if ge_u(c, 65) {
        if lt_u(c, 91) {
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn _is_lower(c: i32) -> bool {
    if ge_u(c, 97) {
        if lt_u(c, 123) {
            true
        } else {
            false
        }
    } else {
        false
    }
}

pub fn is_alpha(c: i32) -> bool {
    if _is_upper(c) {
        true
    } else if _is_lower(c) {
        true
    } else {
        false
    }
}

pub fn to_lower_u8(c: i32) -> i32 {
    if _is_upper(c) {
        c + 32
    } else {
        c
    }
}

pub fn to_upper_u8(c: i32) -> i32 {
    if _is_lower(c) {
        c - 32
    } else {
        c
    }
}

fn _trim_start_space_tab(b: BytesView, start: i32, end: i32) -> i32 {
    for i in start..end {
        let c = bytes_get_u8(b, i);
        if is_space_tab(c) {
        } else {
            return i;
        }
    }
    end
}

fn _trim_end_space_tab(b: BytesView, start: i32, end: i32) -> i32 {
    let mut last = start - 1;
    for i in start..end {
        let c = bytes_get_u8(b, i);
        if is_space_tab(c) {
        } else {
            last = i;
        }
    }
    last + 1
}

fn _append_trimmed_line_space_tab(b: BytesView, start: i32, end: i32, mut out: VecU8) -> VecU8 {
    let l = _trim_start_space_tab(b, start, end);
    let r = _trim_end_space_tab(b, l, end);
    if lt_u(l, r) {
        if lt_u(0, vec_u8_len(out)) {
            out = vec_u8_push(out, 10);
        }
        out = vec_u8_extend_bytes_range(out, b, l, r - l);
        out
    } else {
        out
    }
}

pub fn normalize_lines(b: BytesView) -> Bytes {
    let n = bytes_len(b);
    let mut out = vec_u8_with_capacity(n);
    let mut line_start = 0;
    for i in 0..n {
        let c = bytes_get_u8(b, i);
        if c == 10 || c == 13 {
            out = _append_trimmed_line_space_tab(b, line_start, i, out);
            line_start = i + 1;
        }
    }
    out = _append_trimmed_line_space_tab(b, line_start, n, out);
    vec_u8_into_bytes(out)
}

pub fn tokenize_words_lower(b: BytesView) -> Bytes {
    let n = bytes_len(b);
    let mut out = vec_u8_with_capacity(n);
    let mut in_word = false;
    for i in 0..n {
        let c = bytes_get_u8(b, i);
        if is_alpha(c) {
            if !in_word {
                if lt_u(0, vec_u8_len(out)) {
                    out = vec_u8_push(out, 32);
                }
                in_word = true;
            }
            out = vec_u8_push(out, to_lower_u8(c));
        } else {
            in_word = false;
        }
    }
    vec_u8_into_bytes(out)
}

pub fn first_line_view(b: BytesView) -> BytesView {
    let n = view_len(b);
    for i in 0..n {
        if view_get_u8(b, i) == 10 {
            return view_slice(b, 0, i);
        }
    }
    b
}

pub fn last_line_view(b: BytesView) -> BytesView {
    let n = view_len(b);
    if n == 0 {
        return b;
    }

    let mut end = n;
    if view_get_u8(b, n - 1) == 10 {
        end = n - 1;
    }

    let mut start = 0;
    for i in 0..end {
        if view_get_u8(b, i) == 10 {
            start = i + 1;
        }
    }

    if lt_u(start, end) {
        view_slice(b, start, end - start)
    } else {
        view_slice(b, end, 0)
    }
}

pub fn kth_line_view(b: BytesView, k: i32) -> BytesView {
    let n = view_len(b);
    let k0 = if k < 0 { 0 } else { k };

    let mut line_start = 0;
    let mut line = 0;
    for i in 0..n {
        if view_get_u8(b, i) == 10 {
            if line == k0 {
                return view_slice(b, line_start, i - line_start);
            }
            line = line + 1;
            line_start = i + 1;
        }
    }

    if line == k0 {
        view_slice(b, line_start, n - line_start)
    } else {
        view_slice(b, n, 0)
    }
}

fn _push_u32_le(mut out: VecU8, x: i32) -> VecU8 {
    let b = codec_write_u32_le(x);
    out = vec_u8_extend_bytes_range(out, b, 0, bytes_len(b));
    out
}

fn _x7sl_new_v1(count: i32) -> VecU8 {
    let mut out = vec_u8_with_capacity(12 + (count * 8));
    out = vec_u8_push(out, 88);
    out = vec_u8_push(out, 55);
    out = vec_u8_push(out, 83);
    out = vec_u8_push(out, 76);
    out = _push_u32_le(out, 1);
    out = _push_u32_le(out, count);
    out
}

fn _emit_slice(mut out: VecU8, start: i32, len: i32) -> VecU8 {
    out = _push_u32_le(out, start);
    out = _push_u32_le(out, len);
    out
}

pub fn split_u8(b: BytesView, sep: i32) -> Bytes {
    let n = view_len(b);
    let mut count = 1;
    for i in 0..n {
        if view_get_u8(b, i) == sep {
            count = count + 1;
        }
    }

    let mut out = _x7sl_new_v1(count);
    let mut start = 0;
    for i in 0..n {
        if view_get_u8(b, i) == sep {
            out = _emit_slice(out, start, i - start);
            start = i + 1;
        }
    }
    out = _emit_slice(out, start, n - start);
    vec_u8_into_bytes(out)
}

pub fn split_lines_view(b: BytesView) -> Bytes {
    let n = view_len(b);
    let mut nl_count = 0;
    for i in 0..n {
        if view_get_u8(b, i) == 10 {
            nl_count = nl_count + 1;
        }
    }

    let emit_trailing = if n == 0 {
        1
    } else if view_get_u8(b, n - 1) == 10 {
        0
    } else {
        1
    };
    let total_count = nl_count + emit_trailing;

    let mut out = _x7sl_new_v1(total_count);
    let mut start = 0;
    for i in 0..n {
        if view_get_u8(b, i) == 10 {
            let mut end = i;
            if lt_u(start, end) && view_get_u8(b, end - 1) == 13 {
                end = end - 1;
            }
            out = _emit_slice(out, start, end - start);
            start = i + 1;
        }
    }
    if emit_trailing != 0 {
        let mut end = n;
        if lt_u(start, end) && view_get_u8(b, end - 1) == 13 {
            end = end - 1;
        }
        out = _emit_slice(out, start, end - start);
    }
    vec_u8_into_bytes(out)
}
