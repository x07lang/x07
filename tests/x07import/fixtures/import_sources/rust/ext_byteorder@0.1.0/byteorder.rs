pub fn byteorder_read_u16_le_or(b: BytesView, off: i32, default: i32) -> i32 {
    if off < 0 {
        return default;
    }
    let n = view_len(b);
    if lt_u(off + 1, n) {
        let b0 = view_get_u8(b, off);
        let b1 = view_get_u8(b, off + 1);
        return b0 + (b1 << 8);
    }
    default
}

pub fn byteorder_read_u16_be_or(b: BytesView, off: i32, default: i32) -> i32 {
    if off < 0 {
        return default;
    }
    let n = view_len(b);
    if lt_u(off + 1, n) {
        let b0 = view_get_u8(b, off);
        let b1 = view_get_u8(b, off + 1);
        return (b0 << 8) + b1;
    }
    default
}

pub fn byteorder_read_u32_le_or(b: BytesView, off: i32, default: i32) -> i32 {
    if off < 0 {
        return default;
    }
    let n = view_len(b);
    if lt_u(off + 3, n) {
        let b0 = view_get_u8(b, off);
        let b1 = view_get_u8(b, off + 1);
        let b2 = view_get_u8(b, off + 2);
        let b3 = view_get_u8(b, off + 3);
        return b0 + (b1 << 8) + (b2 << 16) + (b3 << 24);
    }
    default
}

pub fn byteorder_read_u32_be_or(b: BytesView, off: i32, default: i32) -> i32 {
    if off < 0 {
        return default;
    }
    let n = view_len(b);
    if lt_u(off + 3, n) {
        let b0 = view_get_u8(b, off);
        let b1 = view_get_u8(b, off + 1);
        let b2 = view_get_u8(b, off + 2);
        let b3 = view_get_u8(b, off + 3);
        return (b0 << 24) + (b1 << 16) + (b2 << 8) + b3;
    }
    default
}

pub fn byteorder_write_u16_le(x: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(2);
    out = vec_u8_push(out, x & 255);
    out = vec_u8_push(out, (x >> 8) & 255);
    vec_u8_into_bytes(out)
}

pub fn byteorder_write_u16_be(x: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(2);
    out = vec_u8_push(out, (x >> 8) & 255);
    out = vec_u8_push(out, x & 255);
    vec_u8_into_bytes(out)
}

pub fn byteorder_write_u32_le(x: i32) -> Bytes {
    codec_write_u32_le(x)
}

pub fn byteorder_write_u32_be(x: i32) -> Bytes {
    let mut out = vec_u8_with_capacity(4);
    out = vec_u8_push(out, (x >> 24) & 255);
    out = vec_u8_push(out, (x >> 16) & 255);
    out = vec_u8_push(out, (x >> 8) & 255);
    out = vec_u8_push(out, x & 255);
    vec_u8_into_bytes(out)
}
