#![allow(clippy::missing_safety_doc)]

use x07_ext_obj_native_core as objcore;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn x07_obj_s3_dispatch_v1(
    _op: u32,
    _req_ptr: *const u8,
    _req_len: u32,
) -> objcore::ev_bytes {
    // Stub implementation for the package-line PR.
    // Real S3 behavior lands in a later implementation PR.
    objcore::alloc_return_bytes(&objcore::evobj_err(
        objcore::OP_GET_V1,
        objcore::OBJ_ERR_BAD_REQ,
        b"not implemented",
    ))
}
