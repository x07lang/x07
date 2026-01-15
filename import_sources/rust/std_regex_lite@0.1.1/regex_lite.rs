pub fn find_literal(hay: BytesView, needle: BytesView) -> i32 {
    let nh = bytes_len(hay);
    let nn = bytes_len(needle);
    if nn == 0 {
        return 0;
    }
    if lt_u(nh, nn) {
        return -1;
    }
    let last = nh - nn;
    for i in 0..(last + 1) {
        let mut ok = true;
        for j in 0..nn {
            if ok {
                if bytes_get_u8(hay, i + j) == bytes_get_u8(needle, j) {
                } else {
                    ok = false;
                }
            }
        }
        if ok {
            return i;
        }
    }
    -1
}

pub fn is_match_literal(hay: BytesView, needle: BytesView) -> bool {
    let idx = find_literal(hay, needle);
    if idx < 0 {
        false
    } else {
        true
    }
}

fn _is_letter(c: i32) -> bool {
    if ge_u(c, 65) && lt_u(c, 91) {
        return true;
    }
    if ge_u(c, 97) && lt_u(c, 123) {
        return true;
    }
    false
}

fn _count_tokens(pat: BytesView, pat_len: i32) -> i32 {
    if pat_len == 0 {
        return 0;
    }
    let mut i = 0;
    let mut k = 0;
    let mut done = false;
    for _ in 0..(pat_len + 1) {
        if done {
        } else if ge_u(i, pat_len) {
            done = true;
        } else {
            let c = bytes_get_u8(pat, i);
            if c == 42 {
                return -1;
            }
            if c == 46 {
            } else if _is_letter(c) {
            } else {
                return -1;
            }
            k = k + 1;
            if lt_u(i + 1, pat_len) {
                if bytes_get_u8(pat, i + 1) == 42 {
                    i = i + 2;
                } else {
                    i = i + 1;
                }
            } else {
                i = i + 1;
            }
        }
    }
    k
}

fn _eps_closure(mut states: Bytes, star: BytesView, k: i32) -> Bytes {
    for i in 0..k {
        if bytes_get_u8(states, i) == 0 {
        } else if bytes_get_u8(star, i) == 0 {
        } else {
            states = bytes_set_u8(states, i + 1, 1);
            let _unit = 0;
        }
    }
    states
}

fn _match_longest_from(
    b: BytesView,
    pos: i32,
    text_end: i32,
    tok: BytesView,
    star: BytesView,
    k: i32,
) -> i32 {
    let mut cur = bytes_alloc(k + 1);
    cur = bytes_set_u8(cur, 0, 1);
    cur = _eps_closure(cur, star, k);

    let mut last = if bytes_get_u8(cur, k) == 0 { -1 } else { 0 };

    let mut next = bytes_alloc(k + 1);
    let max = text_end - pos;
    let mut done = false;
    for t in 0..max {
        if done {
        } else {
            for i in 0..(k + 1) {
                next = bytes_set_u8(next, i, 0);
            }

            let c = bytes_get_u8(b, pos + t);
            let mut any = false;
            for i in 0..k {
                if bytes_get_u8(cur, i) == 0 {
                } else {
                    let pc = bytes_get_u8(tok, i);
                    let mut m = false;
                    if pc == 46 {
                        m = true;
                    } else if pc == c {
                        m = true;
                    }
                    if m {
                        if bytes_get_u8(star, i) == 0 {
                            next = bytes_set_u8(next, i + 1, 1);
                            any = true;
                        } else {
                            next = bytes_set_u8(next, i, 1);
                            any = true;
                        }
                    }
                }
            }

            if any {
                next = _eps_closure(next, star, k);
                if bytes_get_u8(next, k) == 0 {
                } else {
                    last = t + 1;
                }

                for i in 0..(k + 1) {
                    cur = bytes_set_u8(cur, i, bytes_get_u8(next, i));
                }
            } else {
                done = true;
            }
        }
    }
    last
}

pub fn count_matches_u32le(b: BytesView) -> Bytes {
    let n = bytes_len(b);
    let mut sep = n;
    let mut found = false;
    for i in 0..n {
        if found {
        } else if bytes_get_u8(b, i) == 0 {
            sep = i;
            found = true;
        }
    }

    let pat_len = sep;
    let text_start = if lt_u(sep, n) { sep + 1 } else { n };
    let text_end = n;

    let k = _count_tokens(b, pat_len);
    if k < 1 {
        return codec_write_u32_le(0);
    }

    let mut tok = bytes_alloc(k);
    let mut star = bytes_alloc(k);

    let mut pi = 0;
    let mut ti = 0;
    let mut done = false;
    for _ in 0..(pat_len + 1) {
        if done {
        } else if ge_u(pi, pat_len) {
            done = true;
        } else {
            let c = bytes_get_u8(b, pi);
            tok = bytes_set_u8(tok, ti, c);

            let mut is_star = 0;
            if lt_u(pi + 1, pat_len) {
                if bytes_get_u8(b, pi + 1) == 42 {
                    is_star = 1;
                    pi = pi + 2;
                } else {
                    pi = pi + 1;
                }
            } else {
                pi = pi + 1;
            }

            star = bytes_set_u8(star, ti, is_star);
            ti = ti + 1;
        }
    }

    let tok_v = bytes_view(tok);
    let star_v = bytes_view(star);

    let mut pos = text_start;
    let mut count = 0;
    done = false;
    for _ in 0..(n + 1) {
        if done {
        } else if ge_u(pos, text_end) {
            done = true;
        } else {
            let m = _match_longest_from(b, pos, text_end, tok_v, star_v, k);
            if m < 0 {
                pos = pos + 1;
            } else {
                count = count + 1;
                if m == 0 {
                    pos = pos + 1;
                } else {
                    pos = pos + m;
                }
            }
        }
    }

    codec_write_u32_le(count)
}
