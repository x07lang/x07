#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_bytes {
    pub ptr: *mut u8,
    pub len: u32,
}

extern "C" {
    fn ev_bytes_alloc(len: u32) -> ev_bytes;
    fn ev_trap(code: i32) -> !;
}

pub const EV_TRAP_OBJ_INTERNAL: i32 = 9500;
pub const OBJ_ERR_POLICY_DENIED: u32 = 54_249;
pub const OBJ_ERR_BAD_REQ: u32 = 54_250;
pub const OBJ_ERR_IO: u32 = 54_251;
pub const OBJ_ERR_NOT_FOUND: u32 = 54_252;
pub const OBJ_ERR_TOO_LARGE: u32 = 54_760;

pub const OP_HEAD_V1: u32 = 1;
pub const OP_GET_V1: u32 = 2;
pub const OP_PUT_V1: u32 = 3;
pub const OP_DELETE_V1: u32 = 4;
pub const OP_LIST_V1: u32 = 5;

pub fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|v| match v.as_str() {
            "1" | "true" | "TRUE" | "yes" | "YES" => Some(true),
            "0" | "false" | "FALSE" | "no" | "NO" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

pub fn env_u32_nonzero(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|&v| v != 0)
        .unwrap_or(default)
}

pub fn trap(code: i32) -> ! {
    unsafe { ev_trap(code) }
}

pub fn trap_obj_internal() -> ! {
    trap(EV_TRAP_OBJ_INTERNAL)
}

pub fn alloc_return_bytes(payload: &[u8]) -> ev_bytes {
    unsafe {
        let out = ev_bytes_alloc(payload.len() as u32);
        if out.len != payload.len() as u32 {
            ev_trap(EV_TRAP_OBJ_INTERNAL);
        }
        if out.len != 0 && out.ptr.is_null() {
            ev_trap(EV_TRAP_OBJ_INTERNAL);
        }
        if !payload.is_empty() {
            std::ptr::copy_nonoverlapping(payload.as_ptr(), out.ptr, payload.len());
        }
        out
    }
}

pub unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    if b.len == 0 || b.ptr.is_null() {
        return &[];
    }
    std::slice::from_raw_parts(b.ptr as *const u8, b.len as usize)
}

pub fn evobj_ok(op: u32, ok_payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(20 + ok_payload.len());
    out.extend_from_slice(b"X7OB");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(ok_payload.len() as u32).to_le_bytes());
    out.extend_from_slice(ok_payload);
    out
}

pub fn evobj_err(op: u32, err_code: u32, msg: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + msg.len());
    out.extend_from_slice(b"X7OB");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&err_code.to_le_bytes());
    out.extend_from_slice(&(msg.len() as u32).to_le_bytes());
    out.extend_from_slice(msg);
    out
}
