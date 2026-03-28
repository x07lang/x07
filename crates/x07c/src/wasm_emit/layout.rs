pub const WASM_PAGE_SIZE_BYTES: u64 = 65536;

pub fn bytes_to_pages_exact(bytes: u64) -> Option<u32> {
    if !bytes.is_multiple_of(WASM_PAGE_SIZE_BYTES) {
        return None;
    }
    let pages = bytes / WASM_PAGE_SIZE_BYTES;
    u32::try_from(pages).ok()
}
