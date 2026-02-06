#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use std::net::IpAddr;
use std::sync::Mutex;

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

pub const EV_TRAP_DB_INTERNAL: i32 = 9400;

pub fn trap(code: i32) -> ! {
    unsafe { ev_trap(code) }
}

pub fn trap_db_internal() -> ! {
    trap(EV_TRAP_DB_INTERNAL)
}

pub const DB_ERR_POLICY_DENIED: u32 = 53_249;
pub const DB_ERR_BAD_REQ: u32 = 53_250;
pub const DB_ERR_BAD_CONN: u32 = 53_251;
pub const DB_ERR_TOO_LARGE: u32 = 53_760;

pub const OP_OPEN_V1: u32 = 1;
pub const OP_EXEC_V1: u32 = 2;
pub const OP_QUERY_V1: u32 = 3;
pub const OP_CLOSE_V1: u32 = 4;

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

pub fn env_list(name: &str, sep: char) -> Vec<String> {
    let Ok(v) = std::env::var(name) else {
        return vec![];
    };
    v.split(sep)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

pub fn env_list_u16(name: &str, sep: char) -> Vec<u16> {
    env_list(name, sep)
        .into_iter()
        .filter_map(|s| s.parse::<u16>().ok())
        .collect()
}

pub unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    if b.len == 0 || b.ptr.is_null() {
        return &[];
    }
    std::slice::from_raw_parts(b.ptr as *const u8, b.len as usize)
}

pub fn alloc_return_bytes(payload: &[u8]) -> ev_bytes {
    unsafe {
        let out = ev_bytes_alloc(payload.len() as u32);
        if out.len != payload.len() as u32 {
            ev_trap(EV_TRAP_DB_INTERNAL);
        }
        if out.len != 0 && out.ptr.is_null() {
            ev_trap(EV_TRAP_DB_INTERNAL);
        }
        if !payload.is_empty() {
            std::ptr::copy_nonoverlapping(payload.as_ptr(), out.ptr, payload.len());
        }
        out
    }
}

pub fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    let slice = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[derive(Debug, Clone, Copy)]
pub struct DbCapsV1 {
    pub connect_timeout_ms: u32,
    pub query_timeout_ms: u32,
    pub max_rows: u32,
    pub max_resp_bytes: u32,
}

pub fn parse_db_caps_v1(b: &[u8]) -> Result<DbCapsV1, u32> {
    if b.len() != 24 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &b[0..4] != b"X7DC" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(b, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    Ok(DbCapsV1 {
        connect_timeout_ms: read_u32_le(b, 8).ok_or(DB_ERR_BAD_REQ)?,
        query_timeout_ms: read_u32_le(b, 12).ok_or(DB_ERR_BAD_REQ)?,
        max_rows: read_u32_le(b, 16).ok_or(DB_ERR_BAD_REQ)?,
        max_resp_bytes: read_u32_le(b, 20).ok_or(DB_ERR_BAD_REQ)?,
    })
}

pub fn effective_max(policy_max: u32, caps_max: u32) -> u32 {
    if caps_max == 0 {
        policy_max
    } else if policy_max == 0 {
        caps_max
    } else {
        policy_max.min(caps_max)
    }
}

pub fn effective_connect_timeout_ms(policy_max_connect_timeout_ms: u32, caps: DbCapsV1) -> u32 {
    effective_max(policy_max_connect_timeout_ms, caps.connect_timeout_ms)
}

pub fn effective_query_timeout_ms(policy_max_query_timeout_ms: u32, caps: DbCapsV1) -> u32 {
    effective_max(policy_max_query_timeout_ms, caps.query_timeout_ms)
}

pub fn evict_conn_slot<T>(table: &Mutex<Vec<Option<T>>>, conn_id: u32) {
    let Ok(mut table) = table.lock() else {
        return;
    };
    let Some(slot) = table.get_mut(conn_id as usize) else {
        return;
    };
    *slot = None;
}

pub fn evdb_ok(op: u32, ok_payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(20 + ok_payload.len());
    out.extend_from_slice(b"X7DB");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(ok_payload.len() as u32).to_le_bytes());
    out.extend_from_slice(ok_payload);
    out
}

pub fn evdb_err(op: u32, err_code: u32, msg: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + msg.len());
    out.extend_from_slice(b"X7DB");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&err_code.to_le_bytes());
    out.extend_from_slice(&(msg.len() as u32).to_le_bytes());
    out.extend_from_slice(msg);
    out
}

pub fn dm_value_null() -> Vec<u8> {
    vec![0]
}

pub fn dm_value_bool(v: bool) -> Vec<u8> {
    vec![1, if v { 1 } else { 0 }]
}

pub fn dm_value_number_ascii(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + s.len());
    out.push(2);
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s);
    out
}

pub fn dm_value_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + bytes.len());
    out.push(3);
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    out
}

pub fn dm_value_seq(values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(4);
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for v in values {
        out.extend_from_slice(v);
    }
    out
}

pub fn dm_value_map(mut entries: Vec<(Vec<u8>, Vec<u8>)>) -> Result<Vec<u8>, u32> {
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for i in 1..entries.len() {
        if entries[i - 1].0 == entries[i].0 {
            return Err(DB_ERR_BAD_REQ);
        }
    }

    let mut out = Vec::new();
    out.push(5);
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (k, v) in entries {
        out.extend_from_slice(&(k.len() as u32).to_le_bytes());
        out.extend_from_slice(&k);
        out.extend_from_slice(&v);
    }
    Ok(out)
}

pub fn dm_doc_ok(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + value.len());
    out.push(1);
    out.extend_from_slice(value);
    out
}

#[derive(Debug, Clone, Copy)]
pub enum DmScalar<'a> {
    Null,
    Bool(bool),
    NumberAscii(&'a [u8]),
    String(&'a [u8]),
}

fn dm_skip_value(b: &[u8], off: usize) -> Option<usize> {
    if off >= b.len() {
        return None;
    }
    let tag = b[off];
    match tag {
        0 => Some(off + 1),
        1 => (off + 2 <= b.len()).then_some(off + 2),
        2 | 3 => {
            let len = read_u32_le(b, off + 1)? as usize;
            let end = off + 5 + len;
            (end <= b.len()).then_some(end)
        }
        4 => {
            let count = read_u32_le(b, off + 1)? as usize;
            let mut pos = off + 5;
            for _ in 0..count {
                pos = dm_skip_value(b, pos)?;
            }
            Some(pos)
        }
        5 => {
            let count = read_u32_le(b, off + 1)? as usize;
            let mut pos = off + 5;
            for _ in 0..count {
                let key_len = read_u32_le(b, pos)? as usize;
                pos = pos.checked_add(4 + key_len)?;
                pos = dm_skip_value(b, pos)?;
            }
            Some(pos)
        }
        _ => None,
    }
}

pub fn parse_params_doc_v1(doc: &[u8]) -> Result<Vec<DmScalar<'_>>, u32> {
    if doc.is_empty() || doc[0] != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let root_off = 1usize;
    if root_off >= doc.len() || doc[root_off] != 4 {
        return Err(DB_ERR_BAD_REQ);
    }
    let count = read_u32_le(doc, root_off + 1).ok_or(DB_ERR_BAD_REQ)? as usize;
    let mut pos = root_off + 5;
    let mut out: Vec<DmScalar<'_>> = Vec::with_capacity(count);
    for _ in 0..count {
        if pos >= doc.len() {
            return Err(DB_ERR_BAD_REQ);
        }
        let tag = doc[pos];
        let end = dm_skip_value(doc, pos).ok_or(DB_ERR_BAD_REQ)?;
        match tag {
            0 => out.push(DmScalar::Null),
            1 => {
                let v = doc.get(pos + 1).copied().unwrap_or(0) != 0;
                out.push(DmScalar::Bool(v));
            }
            2 | 3 => {
                let len = read_u32_le(doc, pos + 1).ok_or(DB_ERR_BAD_REQ)? as usize;
                if end != pos + 5 + len {
                    return Err(DB_ERR_BAD_REQ);
                }
                let payload = &doc[pos + 5..pos + 5 + len];
                if tag == 2 {
                    out.push(DmScalar::NumberAscii(payload));
                } else {
                    out.push(DmScalar::String(payload));
                }
            }
            _ => return Err(DB_ERR_BAD_REQ),
        }
        pos = end;
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
pub struct IpNet {
    net: IpAddr,
    prefix: u8,
}

impl IpNet {
    pub fn parse(s: &str) -> Option<IpNet> {
        let (ip_s, prefix_s) = s.split_once('/')?;
        let ip = ip_s.parse::<IpAddr>().ok()?;
        let prefix = prefix_s.parse::<u8>().ok()?;
        let net = match ip {
            IpAddr::V4(v4) => {
                if prefix > 32 {
                    return None;
                }
                let addr_u32 = u32::from(v4);
                let masked = addr_u32 & mask_u32(prefix);
                IpAddr::V4(masked.into())
            }
            IpAddr::V6(v6) => {
                if prefix > 128 {
                    return None;
                }
                let addr_u128 = u128::from(v6);
                let masked = addr_u128 & mask_u128(prefix);
                IpAddr::V6(masked.into())
            }
        };
        Some(IpNet { net, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.net, ip) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let net_u32 = u32::from(net);
                let ip_u32 = u32::from(ip);
                (ip_u32 & mask_u32(self.prefix)) == net_u32
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let net_u128 = u128::from(net);
                let ip_u128 = u128::from(ip);
                (ip_u128 & mask_u128(self.prefix)) == net_u128
            }
            _ => false,
        }
    }
}

fn mask_u32(prefix: u8) -> u32 {
    if prefix == 0 {
        return 0;
    }
    (!0u32) << (32 - prefix)
}

fn mask_u128(prefix: u8) -> u128 {
    if prefix == 0 {
        return 0;
    }
    (!0u128) << (128 - prefix)
}

pub fn parse_ipnet_list(items: &[String]) -> Vec<IpNet> {
    items.iter().filter_map(|s| IpNet::parse(s)).collect()
}

pub fn db_host_allowed(host: &str, allow_dns: &[String], allow_cidrs: &[IpNet]) -> bool {
    if allow_dns.iter().any(|h| h == host) {
        return true;
    }
    let Ok(ip) = host.parse::<IpAddr>() else {
        return false;
    };
    allow_cidrs.iter().any(|net| net.contains(ip))
}
