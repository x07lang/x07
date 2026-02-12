#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use std::io::{BufRead as _, Write as _};

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

#[repr(C)]
#[derive(Copy, Clone)]
pub union ev_result_i32_payload {
    pub ok: u32,  // i32 bits
    pub err: u32, // error code
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ev_result_i32 {
    pub tag: u32, // 1 = ok, 0 = err
    pub payload: ev_result_i32_payload,
}

extern "C" {
    fn ev_bytes_alloc(len: u32) -> ev_bytes;
    fn ev_trap(code: i32) -> !;
}

const EV_TRAP_STDIO_INTERNAL: i32 = 9600;

const STDIO_ERR_DISABLED_V1: u32 = 60101;
#[allow(dead_code)]
const STDIO_ERR_POLICY_DENY_V1: u32 = 60102;
const STDIO_ERR_BAD_CAPS_V1: u32 = 60104;
const STDIO_ERR_IO_V1: u32 = 60115;
const STDIO_ERR_TOO_LARGE_V1: u32 = 60116;
const STDIO_ERR_EOF_V1: u32 = 60121;

const POLICY_MAX_READ_BYTES: u32 = 16 * 1024 * 1024;
const POLICY_MAX_WRITE_BYTES: u32 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct CapsV1 {
    max_read_bytes: u32,
    max_write_bytes: u32,
    flags: u32,
}

fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    let slice = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn parse_caps_v1(caps: &[u8]) -> Result<CapsV1, u32> {
    if caps.len() != 16 {
        return Err(STDIO_ERR_BAD_CAPS_V1);
    }
    let version = read_u32_le(caps, 0).ok_or(STDIO_ERR_BAD_CAPS_V1)?;
    if version != 1 {
        return Err(STDIO_ERR_BAD_CAPS_V1);
    }
    Ok(CapsV1 {
        max_read_bytes: read_u32_le(caps, 4).ok_or(STDIO_ERR_BAD_CAPS_V1)?,
        max_write_bytes: read_u32_le(caps, 8).ok_or(STDIO_ERR_BAD_CAPS_V1)?,
        flags: read_u32_le(caps, 12).ok_or(STDIO_ERR_BAD_CAPS_V1)?,
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

fn ok_i32(x: i32) -> ev_result_i32 {
    ev_result_i32 {
        tag: 1,
        payload: ev_result_i32_payload { ok: x as u32 },
    }
}

fn err_i32(code: u32) -> ev_result_i32 {
    ev_result_i32 {
        tag: 0,
        payload: ev_result_i32_payload { err: code },
    }
}

unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    std::slice::from_raw_parts(b.ptr, b.len as usize)
}

unsafe fn alloc_bytes(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        ev_trap(EV_TRAP_STDIO_INTERNAL);
    }
    out
}

unsafe fn ok_bytes_vec(v: Vec<u8>) -> ev_result_bytes {
    let len = v.len();
    if len > (u32::MAX as usize) {
        return err_bytes(STDIO_ERR_TOO_LARGE_V1);
    }
    let out = alloc_bytes(len as u32);
    if len != 0 {
        std::ptr::copy_nonoverlapping(v.as_ptr(), out.ptr, len);
    }
    ok_bytes(out)
}

#[no_mangle]
pub extern "C" fn x07_ext_stdio_read_line_v1(caps: ev_bytes) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };
        if caps.flags != 0 {
            return err_bytes(STDIO_ERR_BAD_CAPS_V1);
        }

        let max_read = effective_max(POLICY_MAX_READ_BYTES, caps.max_read_bytes) as usize;
        if max_read == 0 {
            return err_bytes(STDIO_ERR_DISABLED_V1);
        }

        let mut stdin = std::io::stdin().lock();
        let mut out: Vec<u8> = Vec::new();

        loop {
            let consume_n: usize;
            let mut saw_newline: bool = false;
            let mut too_large: bool = false;

            {
                let buf = match stdin.fill_buf() {
                    Ok(buf) => buf,
                    Err(_) => return err_bytes(STDIO_ERR_IO_V1),
                };
                if buf.is_empty() {
                    if out.is_empty() {
                        return err_bytes(STDIO_ERR_EOF_V1);
                    }
                    break;
                }

                if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    saw_newline = true;
                    consume_n = pos + 1;
                    if out.len().saturating_add(pos) > max_read {
                        too_large = true;
                    } else {
                        out.extend_from_slice(&buf[..pos]);
                    }
                } else if out.len().saturating_add(buf.len()) > max_read {
                    consume_n = buf.len();
                    too_large = true;
                } else {
                    consume_n = buf.len();
                    out.extend_from_slice(buf);
                }
            }

            stdin.consume(consume_n);

            if too_large {
                if !saw_newline {
                    loop {
                        let consume_n: usize;
                        let mut saw_newline: bool = false;
                        {
                            let buf = match stdin.fill_buf() {
                                Ok(buf) => buf,
                                Err(_) => return err_bytes(STDIO_ERR_IO_V1),
                            };
                            if buf.is_empty() {
                                break;
                            }
                            if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                                consume_n = pos + 1;
                                saw_newline = true;
                            } else {
                                consume_n = buf.len();
                            }
                        }
                        stdin.consume(consume_n);
                        if saw_newline {
                            break;
                        }
                    }
                }
                return err_bytes(STDIO_ERR_TOO_LARGE_V1);
            }

            if saw_newline {
                break;
            }
        }

        if out.last() == Some(&b'\r') {
            out.pop();
        }
        ok_bytes_vec(out)
    })
    .unwrap_or_else(|_| err_bytes(STDIO_ERR_IO_V1))
}

#[no_mangle]
pub extern "C" fn x07_ext_stdio_write_stdout_v1(data: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };
        if caps.flags != 0 {
            return err_i32(STDIO_ERR_BAD_CAPS_V1);
        }

        let max_write = effective_max(POLICY_MAX_WRITE_BYTES, caps.max_write_bytes) as usize;
        if max_write == 0 {
            return err_i32(STDIO_ERR_DISABLED_V1);
        }

        let data = bytes_as_slice(data);
        if data.len() > max_write {
            return err_i32(STDIO_ERR_TOO_LARGE_V1);
        }

        let mut stdout = std::io::stdout().lock();
        if stdout.write_all(data).is_err() {
            return err_i32(STDIO_ERR_IO_V1);
        }
        ok_i32(data.len() as i32)
    })
    .unwrap_or_else(|_| err_i32(STDIO_ERR_IO_V1))
}

#[no_mangle]
pub extern "C" fn x07_ext_stdio_write_stderr_v1(data: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };
        if caps.flags != 0 {
            return err_i32(STDIO_ERR_BAD_CAPS_V1);
        }

        let max_write = effective_max(POLICY_MAX_WRITE_BYTES, caps.max_write_bytes) as usize;
        if max_write == 0 {
            return err_i32(STDIO_ERR_DISABLED_V1);
        }

        let data = bytes_as_slice(data);
        if data.len() > max_write {
            return err_i32(STDIO_ERR_TOO_LARGE_V1);
        }

        let mut stderr = std::io::stderr().lock();
        if stderr.write_all(data).is_err() {
            return err_i32(STDIO_ERR_IO_V1);
        }
        ok_i32(data.len() as i32)
    })
    .unwrap_or_else(|_| err_i32(STDIO_ERR_IO_V1))
}

#[no_mangle]
pub extern "C" fn x07_ext_stdio_flush_stdout_v1() -> ev_result_i32 {
    std::panic::catch_unwind(|| {
        let mut stdout = std::io::stdout().lock();
        if stdout.flush().is_err() {
            return err_i32(STDIO_ERR_IO_V1);
        }
        ok_i32(0)
    })
    .unwrap_or_else(|_| err_i32(STDIO_ERR_IO_V1))
}

#[no_mangle]
pub extern "C" fn x07_ext_stdio_flush_stderr_v1() -> ev_result_i32 {
    std::panic::catch_unwind(|| {
        let mut stderr = std::io::stderr().lock();
        if stderr.flush().is_err() {
            return err_i32(STDIO_ERR_IO_V1);
        }
        ok_i32(0)
    })
    .unwrap_or_else(|_| err_i32(STDIO_ERR_IO_V1))
}
