#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use chrono::offset::Offset as _;
use chrono::TimeZone as _;
use chrono::Utc;

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

const EV_TRAP_TIME_INTERNAL: i32 = 9200;

const SPEC_ERR_TZDB_INVALID_TZID: u32 = 100;
const SPEC_ERR_TZDB_RANGE: u32 = 101;
const SPEC_ERR_TZDB_INTERNAL: u32 = 102;

#[inline]
unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    core::slice::from_raw_parts(b.ptr as *const u8, b.len as usize)
}

#[inline]
unsafe fn bytes_as_mut_slice<'a>(b: ev_bytes) -> &'a mut [u8] {
    core::slice::from_raw_parts_mut(b.ptr, b.len as usize)
}

#[inline]
unsafe fn alloc_bytes(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        ev_trap(EV_TRAP_TIME_INTERNAL);
    }
    out
}

#[inline]
fn write_u32_le(dst: &mut [u8], x: u32) {
    dst[0..4].copy_from_slice(&x.to_le_bytes());
}

#[inline]
unsafe fn duration_err(code: u32) -> ev_bytes {
    let out = alloc_bytes(9);
    let s = bytes_as_mut_slice(out);
    s[0] = 0;
    write_u32_le(&mut s[1..5], code);
    write_u32_le(&mut s[5..9], 0);
    out
}

#[inline]
unsafe fn duration_ok(secs: i64, nanos: u32) -> ev_bytes {
    let out = alloc_bytes(14);
    let s = bytes_as_mut_slice(out);

    s[0] = 1;
    s[1] = 1;

    let bits = secs as u64;
    let lo = bits as u32;
    let hi = (bits >> 32) as u32;
    write_u32_le(&mut s[2..6], lo);
    write_u32_le(&mut s[6..10], hi);
    write_u32_le(&mut s[10..14], nanos);

    out
}

#[inline]
fn i64_from_lohi(lo: i32, hi: i32) -> i64 {
    let lo_bits = lo as u32 as u64;
    let hi_bits = hi as u32 as u64;
    let bits = lo_bits | (hi_bits << 32);
    bits as i64
}

#[inline]
unsafe fn tzid_as_str<'a>(tzid: ev_bytes) -> Option<&'a str> {
    let s = bytes_as_slice(tzid);
    let s = core::str::from_utf8(s).ok()?;
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[no_mangle]
pub unsafe extern "C" fn ev_time_tzdb_is_valid_tzid_v1(tzid: ev_bytes) -> u32 {
    let Some(s) = tzid_as_str(tzid) else {
        return 0;
    };
    match s.parse::<chrono_tz::Tz>() {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn ev_time_tzdb_offset_duration_v1(
    tzid: ev_bytes,
    unix_s_lo: i32,
    unix_s_hi: i32,
) -> ev_bytes {
    let res = std::panic::catch_unwind(|| {
        let Some(s) = (unsafe { tzid_as_str(tzid) }) else {
            return unsafe { duration_err(SPEC_ERR_TZDB_INVALID_TZID) };
        };

        let tz: chrono_tz::Tz = match s.parse() {
            Ok(v) => v,
            Err(_) => return unsafe { duration_err(SPEC_ERR_TZDB_INVALID_TZID) },
        };

        let unix_s = i64_from_lohi(unix_s_lo, unix_s_hi);
        let Some(utc) = chrono::DateTime::<Utc>::from_timestamp(unix_s, 0) else {
            return unsafe { duration_err(SPEC_ERR_TZDB_RANGE) };
        };

        let offset_s = tz
            .offset_from_utc_datetime(&utc.naive_utc())
            .fix()
            .local_minus_utc();
        unsafe { duration_ok(offset_s as i64, 0) }
    });

    match res {
        Ok(b) => b,
        Err(_) => duration_err(SPEC_ERR_TZDB_INTERNAL),
    }
}

#[no_mangle]
pub unsafe extern "C" fn ev_time_tzdb_snapshot_id_v1() -> ev_bytes {
    const PREFIX: &[u8] = b"tzdb-";
    let version = chrono_tz::IANA_TZDB_VERSION.as_bytes();
    let total_len = PREFIX.len() + version.len();
    if total_len > (u32::MAX as usize) {
        ev_trap(EV_TRAP_TIME_INTERNAL);
    }
    let out = alloc_bytes(total_len as u32);
    let dst = bytes_as_mut_slice(out);
    dst[..PREFIX.len()].copy_from_slice(PREFIX);
    dst[PREFIX.len()..].copy_from_slice(version);
    out
}
