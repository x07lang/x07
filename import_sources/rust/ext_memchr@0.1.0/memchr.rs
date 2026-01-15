pub fn memchr(hay: BytesView, needle: i32) -> i32 {
    let n = view_len(hay);
    for i in 0..n {
        if view_get_u8(hay, i) == needle {
            return i;
        }
    }
    -1
}

pub fn memrchr(hay: BytesView, needle: i32) -> i32 {
    let n = view_len(hay);
    if n <= 0 {
        return -1;
    }
    let mut i = n - 1;
    let mut done = false;
    for _ in 0..n {
        if done {
        } else if i < 0 {
            done = true;
        } else {
            if view_get_u8(hay, i) == needle {
                return i;
            }
            i = i - 1;
        }
    }
    -1
}

pub fn memchr2(hay: BytesView, needle1: i32, needle2: i32) -> i32 {
    let n = view_len(hay);
    for i in 0..n {
        let c = view_get_u8(hay, i);
        if c == needle1 || c == needle2 {
            return i;
        }
    }
    -1
}

pub fn memchr3(hay: BytesView, needle1: i32, needle2: i32, needle3: i32) -> i32 {
    let n = view_len(hay);
    for i in 0..n {
        let c = view_get_u8(hay, i);
        if c == needle1 || c == needle2 || c == needle3 {
            return i;
        }
    }
    -1
}

pub fn memrchr2(hay: BytesView, needle1: i32, needle2: i32) -> i32 {
    let n = view_len(hay);
    if n <= 0 {
        return -1;
    }
    let mut i = n - 1;
    let mut done = false;
    for _ in 0..n {
        if done {
        } else if i < 0 {
            done = true;
        } else {
            let c = view_get_u8(hay, i);
            if c == needle1 || c == needle2 {
                return i;
            }
            i = i - 1;
        }
    }
    -1
}

pub fn memrchr3(hay: BytesView, needle1: i32, needle2: i32, needle3: i32) -> i32 {
    let n = view_len(hay);
    if n <= 0 {
        return -1;
    }
    let mut i = n - 1;
    let mut done = false;
    for _ in 0..n {
        if done {
        } else if i < 0 {
            done = true;
        } else {
            let c = view_get_u8(hay, i);
            if c == needle1 || c == needle2 || c == needle3 {
                return i;
            }
            i = i - 1;
        }
    }
    -1
}
