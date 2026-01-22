pub fn memchr(hay: BytesView, needle: i32) -> i32 {
    let n = view_len(hay);
    for i in 0..n {
        if view_get_u8(hay, i) == needle {
            return i;
        }
    }
    -1
}

