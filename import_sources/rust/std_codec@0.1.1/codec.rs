pub fn read_u32_le(b: BytesView, off: i32) -> i32 {
    codec_read_u32_le(b, off)
}

pub fn write_u32_le(x: i32) -> Bytes {
    codec_write_u32_le(x)
}
