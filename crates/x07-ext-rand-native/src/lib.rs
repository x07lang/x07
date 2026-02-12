#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_bytes {
    pub ptr: *mut u8,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union ev_result_bytes_payload {
    pub ok: ev_bytes,
    pub err: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_result_bytes {
    pub tag: u32, // 1 = ok, 0 = err
    pub payload: ev_result_bytes_payload,
}

extern "C" {
    fn ev_bytes_alloc(len: u32) -> ev_bytes;
    fn ev_trap(code: i32) -> !;
}

const EV_TRAP_RAND_INTERNAL: i32 = 9700;

const RAND_ERR_DISABLED_V1: u32 = 60201;
#[allow(dead_code)]
const RAND_ERR_POLICY_DENY_V1: u32 = 60202;
const RAND_ERR_BAD_CAPS_V1: u32 = 60204;
const RAND_ERR_BAD_ARG_V1: u32 = 60205;
const RAND_ERR_IO_V1: u32 = 60215;

const POLICY_MAX_BYTES_PER_CALL: u32 = 65536;

#[derive(Clone, Copy, Debug)]
struct CapsV1 {
    max_bytes_per_call: u32,
    flags: u32,
}

fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    let slice = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn parse_caps_v1(caps: &[u8]) -> Result<CapsV1, u32> {
    if caps.len() != 12 {
        return Err(RAND_ERR_BAD_CAPS_V1);
    }
    let version = read_u32_le(caps, 0).ok_or(RAND_ERR_BAD_CAPS_V1)?;
    if version != 1 {
        return Err(RAND_ERR_BAD_CAPS_V1);
    }
    Ok(CapsV1 {
        max_bytes_per_call: read_u32_le(caps, 4).ok_or(RAND_ERR_BAD_CAPS_V1)?,
        flags: read_u32_le(caps, 8).ok_or(RAND_ERR_BAD_CAPS_V1)?,
    })
}

fn effective_max(policy_max: u32, caps_max: u32) -> u32 {
    if caps_max == 0 {
        policy_max
    } else {
        policy_max.min(caps_max)
    }
}

fn ok_bytes(out: ev_bytes) -> ev_result_bytes {
    ev_result_bytes {
        tag: 1,
        payload: ev_result_bytes_payload { ok: out },
    }
}

fn err_bytes(code: u32) -> ev_result_bytes {
    ev_result_bytes {
        tag: 0,
        payload: ev_result_bytes_payload { err: code },
    }
}

unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    std::slice::from_raw_parts(b.ptr, b.len as usize)
}

unsafe fn alloc_bytes(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        ev_trap(EV_TRAP_RAND_INTERNAL);
    }
    out
}

unsafe fn rand_bytes_impl_v1(n: u32, caps: CapsV1) -> ev_result_bytes {
    if caps.flags != 0 {
        return err_bytes(RAND_ERR_BAD_CAPS_V1);
    }

    let max = effective_max(POLICY_MAX_BYTES_PER_CALL, caps.max_bytes_per_call);
    if max == 0 {
        return err_bytes(RAND_ERR_DISABLED_V1);
    }
    if n > max {
        return err_bytes(RAND_ERR_BAD_ARG_V1);
    }

    let out = alloc_bytes(n);
    if n != 0 {
        let out_slice = std::slice::from_raw_parts_mut(out.ptr, n as usize);
        if getrandom::getrandom(out_slice).is_err() {
            return err_bytes(RAND_ERR_IO_V1);
        }
    }
    ok_bytes(out)
}

#[no_mangle]
pub extern "C" fn x07_ext_rand_bytes_v1(n: i32, caps: ev_bytes) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        if n < 0 {
            return err_bytes(RAND_ERR_BAD_ARG_V1);
        }
        rand_bytes_impl_v1(n as u32, caps)
    })
    .unwrap_or_else(|_| err_bytes(RAND_ERR_IO_V1))
}

#[no_mangle]
pub extern "C" fn x07_ext_rand_u64_v1(caps: ev_bytes) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };
        rand_bytes_impl_v1(8, caps)
    })
    .unwrap_or_else(|_| err_bytes(RAND_ERR_IO_V1))
}
