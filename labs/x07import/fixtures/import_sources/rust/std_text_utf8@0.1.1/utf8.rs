fn _is_cont(c: i32) -> bool {
    if ge_u(c, 128) {
        if lt_u(c, 192) {
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn _cont_range(c: i32, lo: i32, hi: i32) -> bool {
    if ge_u(c, lo) {
        lt_u(c, hi + 1)
    } else {
        false
    }
}

fn _next_index_or_neg1(b: BytesView, i: i32, n: i32) -> i32 {
    let b1 = bytes_get_u8(b, i);
    if lt_u(b1, 128) {
        return i + 1;
    }

    if lt_u(b1, 194) {
        return -1;
    }

    if lt_u(b1, 224) {
        if ge_u(i + 1, n) {
            return -1;
        }
        let b2 = bytes_get_u8(b, i + 1);
        if _is_cont(b2) {
            return i + 2;
        }
        return -1;
    }

    if ge_u(i + 2, n) {
        return -1;
    }
    let b2 = bytes_get_u8(b, i + 1);
    let b3 = bytes_get_u8(b, i + 2);

    if b1 == 224 {
        if _cont_range(b2, 160, 191) {
            if _is_cont(b3) {
                return i + 3;
            }
            return -1;
        }
        return -1;
    }

    if lt_u(b1, 237) {
        if _is_cont(b2) {
            if _is_cont(b3) {
                return i + 3;
            }
            return -1;
        }
        return -1;
    }

    if b1 == 237 {
        if _cont_range(b2, 128, 159) {
            if _is_cont(b3) {
                return i + 3;
            }
            return -1;
        }
        return -1;
    }

    if lt_u(b1, 240) {
        if _is_cont(b2) {
            if _is_cont(b3) {
                return i + 3;
            }
            return -1;
        }
        return -1;
    }

    if ge_u(i + 3, n) {
        return -1;
    }
    let b4 = bytes_get_u8(b, i + 3);

    if b1 == 240 {
        if _cont_range(b2, 144, 191) {
            if _is_cont(b3) {
                if _is_cont(b4) {
                    return i + 4;
                }
                return -1;
            }
            return -1;
        }
        return -1;
    }

    if lt_u(b1, 244) {
        if _is_cont(b2) {
            if _is_cont(b3) {
                if _is_cont(b4) {
                    return i + 4;
                }
                return -1;
            }
            return -1;
        }
        return -1;
    }

    if b1 == 244 {
        if _cont_range(b2, 128, 143) {
            if _is_cont(b3) {
                if _is_cont(b4) {
                    return i + 4;
                }
                return -1;
            }
            return -1;
        }
        return -1;
    }

    -1
}

pub fn is_valid(b: BytesView) -> bool {
    let n = bytes_len(b);
    let mut i = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return true;
        }
        let next = _next_index_or_neg1(b, i, n);
        if next < 0 {
            return false;
        }
        i = next;
    }
    true
}

pub fn validate_or_empty(b: BytesView) -> Bytes {
    if is_valid(b) {
        view_to_bytes(b)
    } else {
        bytes_alloc(0)
    }
}

pub fn count_codepoints_or_neg1(b: BytesView) -> i32 {
    let n = bytes_len(b);
    let mut i = 0;
    let mut count = 0;
    for _ in 0..n {
        if ge_u(i, n) {
            return count;
        }
        let next = _next_index_or_neg1(b, i, n);
        if next < 0 {
            return -1;
        }
        i = next;
        count = count + 1;
    }
    count
}
