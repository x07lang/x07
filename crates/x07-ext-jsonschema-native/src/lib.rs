#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use core::cmp::Ordering;
use jsonschema::{Draft, Validator};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

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

const EV_TRAP_JSONSCHEMA_INTERNAL: i32 = 9800;

const COMPILED_MAGIC: &[u8; 4] = b"X7JS";
const COMPILED_VERSION: u8 = 1;
const COMPILED_LEN: u32 = 15;

const VALIDATE_MAGIC: &[u8; 4] = b"X7JV";
const VALIDATE_VERSION: u8 = 1;
const VALIDATE_OK_LEN: u32 = 6;

const CODE_COMPILE_SCHEMA_JSON_INVALID: u32 = 1;
const CODE_COMPILE_UNSUPPORTED_DIALECT: u32 = 2;
const CODE_COMPILE_SCHEMA_INVALID: u32 = 3;

const CODE_VALIDATE_ERRORS: u32 = 1;
const CODE_VALIDATE_INVALID_HANDLE: u32 = 2;
const CODE_VALIDATE_INSTANCE_JSON_INVALID: u32 = 3;
const CODE_VALIDATE_INTERNAL: u32 = 4;

#[derive(Clone)]
struct Compiled {
    validator: Arc<Validator>,
}

struct SchemaTable {
    entries: Vec<Option<Compiled>>,
    by_key: HashMap<(u8, u32, u64), u32>,
}

impl SchemaTable {
    fn new() -> Self {
        Self {
            // Handle 0 reserved as invalid.
            entries: vec![None],
            by_key: HashMap::new(),
        }
    }

    fn insert(&mut self, compiled: Compiled) -> Option<u32> {
        // Deterministic handle assignment: first free slot, else append.
        for (i, slot) in self.entries.iter_mut().enumerate().skip(1) {
            if slot.is_none() {
                *slot = Some(compiled);
                return Some(i as u32);
            }
        }
        let h = self.entries.len() as u32;
        self.entries.push(Some(compiled));
        Some(h)
    }

    fn get(&self, h: u32) -> Option<&Compiled> {
        self.entries.get(h as usize)?.as_ref()
    }
}

static TABLE: OnceLock<Mutex<SchemaTable>> = OnceLock::new();

fn table() -> &'static Mutex<SchemaTable> {
    TABLE.get_or_init(|| Mutex::new(SchemaTable::new()))
}

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
        ev_trap(EV_TRAP_JSONSCHEMA_INTERNAL);
    }
    out
}

fn write_u32_le(dst: &mut [u8], x: u32) {
    dst[0] = (x & 0xFF) as u8;
    dst[1] = ((x >> 8) & 0xFF) as u8;
    dst[2] = ((x >> 16) & 0xFF) as u8;
    dst[3] = ((x >> 24) & 0xFF) as u8;
}

fn read_u32_le(src: &[u8]) -> u32 {
    u32::from_le_bytes([src[0], src[1], src[2], src[3]])
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn make_compile_ok(handle: u32, dialect_id: u8, schema_hash: u32) -> ev_bytes {
    unsafe {
        let out = alloc_bytes(COMPILED_LEN);
        let b = bytes_as_mut_slice(out);
        b[0] = 1;
        b[1..5].copy_from_slice(COMPILED_MAGIC);
        b[5] = COMPILED_VERSION;
        write_u32_le(&mut b[6..10], handle);
        b[10] = dialect_id;
        write_u32_le(&mut b[11..15], schema_hash);
        out
    }
}

fn make_compile_err(code: u32, msg: &str) -> ev_bytes {
    let msg_b = msg.as_bytes();
    let msg_len = msg_b.len().min(u32::MAX as usize) as u32;
    let total_len = 14u32.saturating_add(msg_len);

    unsafe {
        let out = alloc_bytes(total_len);
        let b = bytes_as_mut_slice(out);
        b[0] = 0;
        b[1..5].copy_from_slice(COMPILED_MAGIC);
        b[5] = COMPILED_VERSION;
        write_u32_le(&mut b[6..10], code);
        write_u32_le(&mut b[10..14], msg_len);
        if msg_len != 0 {
            core::ptr::copy_nonoverlapping(msg_b.as_ptr(), b[14..].as_mut_ptr(), msg_len as usize);
        }
        out
    }
}

fn make_validate_ok() -> ev_bytes {
    unsafe {
        let out = alloc_bytes(VALIDATE_OK_LEN);
        let b = bytes_as_mut_slice(out);
        b[0] = 1;
        b[1..5].copy_from_slice(VALIDATE_MAGIC);
        b[5] = VALIDATE_VERSION;
        out
    }
}

fn make_validate_err(code: u32, errors_json: &[u8]) -> ev_bytes {
    let errors_len = errors_json.len().min(u32::MAX as usize) as u32;
    let total_len = 14u32.saturating_add(errors_len);

    unsafe {
        let out = alloc_bytes(total_len);
        let b = bytes_as_mut_slice(out);
        b[0] = 0;
        b[1..5].copy_from_slice(VALIDATE_MAGIC);
        b[5] = VALIDATE_VERSION;
        write_u32_le(&mut b[6..10], code);
        write_u32_le(&mut b[10..14], errors_len);
        if errors_len != 0 {
            core::ptr::copy_nonoverlapping(
                errors_json.as_ptr(),
                b[14..].as_mut_ptr(),
                errors_len as usize,
            );
        }
        out
    }
}

fn compile_dialect(schema: &Value) -> Result<(Draft, u8), (u32, String)> {
    let obj = schema.as_object();
    let Some(obj) = obj else {
        return Ok((Draft::Draft202012, 1));
    };

    let schema_uri = match obj.get("$schema") {
        None => return Ok((Draft::Draft202012, 1)),
        Some(v) => v.as_str().ok_or_else(|| {
            (
                CODE_COMPILE_SCHEMA_INVALID,
                "schema $schema must be a string".to_string(),
            )
        })?,
    };

    // Normalize common variants (with/without trailing '#').
    let uri = schema_uri.trim_end_matches('#');
    let uri = uri.strip_suffix('/').unwrap_or(uri);

    let (draft, dialect_id) =
        match uri {
            "https://json-schema.org/draft/2020-12/schema"
            | "http://json-schema.org/draft/2020-12/schema" => (Draft::Draft202012, 1),
            "https://json-schema.org/draft/2019-09/schema"
            | "http://json-schema.org/draft/2019-09/schema" => (Draft::Draft201909, 2),
            "https://json-schema.org/draft-07/schema"
            | "http://json-schema.org/draft-07/schema" => (Draft::Draft7, 7),
            "https://json-schema.org/draft-06/schema"
            | "http://json-schema.org/draft-06/schema" => (Draft::Draft6, 6),
            "https://json-schema.org/draft-04/schema"
            | "http://json-schema.org/draft-04/schema" => (Draft::Draft4, 4),
            _ => {
                return Err((
                    CODE_COMPILE_UNSUPPORTED_DIALECT,
                    format!("unsupported $schema dialect: {schema_uri}"),
                ));
            }
        };
    Ok((draft, dialect_id))
}

fn keyword_from_schema_path(schema_path: &str) -> &str {
    if schema_path.is_empty() {
        return "";
    }
    schema_path.rsplit('/').next().unwrap_or("")
}

#[derive(Serialize)]
struct ErrorEntry<'a> {
    #[serde(rename = "jsonPointer")]
    json_pointer: &'a str,
    keyword: &'a str,
    message: &'a str,
}

fn errors_to_json<'a>(errors: impl Iterator<Item = jsonschema::ValidationError<'a>>) -> Vec<u8> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for err in errors {
        let json_pointer = err.instance_path.to_string();
        let schema_path = err.schema_path.to_string();
        let keyword = keyword_from_schema_path(&schema_path).to_string();
        let message = err.to_string();
        out.push((json_pointer, keyword, message));
    }

    out.sort_by(|a, b| {
        let (ap, ak, am) = a;
        let (bp, bk, bm) = b;
        match ap.cmp(bp) {
            Ordering::Equal => match ak.cmp(bk) {
                Ordering::Equal => am.cmp(bm),
                other => other,
            },
            other => other,
        }
    });

    let entries: Vec<ErrorEntry<'_>> = out
        .iter()
        .map(|(p, k, m)| ErrorEntry {
            json_pointer: p.as_str(),
            keyword: k.as_str(),
            message: m.as_str(),
        })
        .collect();

    serde_json::to_vec(&entries).unwrap_or_else(|_| b"[]".to_vec())
}

fn single_error_json(json_pointer: &str, keyword: &str, message: &str) -> Vec<u8> {
    let entries = vec![ErrorEntry {
        json_pointer,
        keyword,
        message,
    }];
    serde_json::to_vec(&entries).unwrap_or_else(|_| b"[]".to_vec())
}

fn parse_compiled(doc: &[u8]) -> Result<u32, (u32, &'static str)> {
    if doc.len() < COMPILED_LEN as usize {
        return Err((CODE_VALIDATE_INVALID_HANDLE, "compiled handle too short"));
    }
    if doc[0] == 0 {
        return Err((CODE_VALIDATE_INVALID_HANDLE, "compiled handle is an error"));
    }
    if &doc[1..5] != COMPILED_MAGIC {
        return Err((
            CODE_VALIDATE_INVALID_HANDLE,
            "compiled handle magic mismatch",
        ));
    }
    if doc[5] != COMPILED_VERSION {
        return Err((
            CODE_VALIDATE_INVALID_HANDLE,
            "compiled handle version mismatch",
        ));
    }
    let handle = read_u32_le(&doc[6..10]);
    Ok(handle)
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_jsonschema_compile_v1(schema_json: ev_bytes) -> ev_bytes {
    std::panic::catch_unwind(|| {
        let schema_bytes = unsafe { bytes_as_slice(schema_json) };
        let schema_hash = fnv1a32(schema_bytes);
        let schema_hash64 = fnv1a64(schema_bytes);
        let schema_str = match core::str::from_utf8(schema_bytes) {
            Ok(s) => s,
            Err(_) => {
                return make_compile_err(
                    CODE_COMPILE_SCHEMA_JSON_INVALID,
                    "schema_json must be utf-8",
                );
            }
        };

        let schema_val: Value = match serde_json::from_str(schema_str) {
            Ok(v) => v,
            Err(e) => {
                return make_compile_err(
                    CODE_COMPILE_SCHEMA_JSON_INVALID,
                    &format!("invalid schema_json: {e}"),
                );
            }
        };

        let (draft, dialect_id) = match compile_dialect(&schema_val) {
            Ok(x) => x,
            Err((code, msg)) => return make_compile_err(code, &msg),
        };

        let cache_key = (dialect_id, schema_json.len, schema_hash64);
        {
            let guard = table().lock().unwrap();
            if let Some(handle) = guard.by_key.get(&cache_key).copied() {
                if guard.get(handle).is_some() {
                    return make_compile_ok(handle, dialect_id, schema_hash);
                }
            }
        }

        let schema = match jsonschema::options().with_draft(draft).build(&schema_val) {
            Ok(s) => s,
            Err(e) => {
                return make_compile_err(CODE_COMPILE_SCHEMA_INVALID, &e.to_string());
            }
        };

        let compiled = Compiled {
            validator: Arc::new(schema),
        };
        let mut guard = table().lock().unwrap();
        if let Some(handle) = guard.by_key.get(&cache_key).copied() {
            if guard.get(handle).is_some() {
                return make_compile_ok(handle, dialect_id, schema_hash);
            }
        }
        let Some(handle) = guard.insert(compiled) else {
            return make_compile_err(
                CODE_COMPILE_SCHEMA_INVALID,
                "failed to allocate schema handle",
            );
        };
        guard.by_key.insert(cache_key, handle);
        drop(guard);

        make_compile_ok(handle, dialect_id, schema_hash)
    })
    .unwrap_or_else(|_| make_compile_err(CODE_COMPILE_SCHEMA_INVALID, "compile panicked"))
}

#[no_mangle]
pub unsafe extern "C" fn x07_ext_jsonschema_validate_v1(
    compiled: ev_bytes,
    instance_json: ev_bytes,
) -> ev_bytes {
    std::panic::catch_unwind(|| {
        let compiled_bytes = unsafe { bytes_as_slice(compiled) };
        let handle = match parse_compiled(compiled_bytes) {
            Ok(v) => v,
            Err((code, msg)) => {
                return make_validate_err(code, &single_error_json("", "internal", msg))
            }
        };

        let compiled = {
            let guard = table().lock().unwrap();
            let Some(c) = guard.get(handle).cloned() else {
                drop(guard);
                return make_validate_err(
                    CODE_VALIDATE_INVALID_HANDLE,
                    &single_error_json("", "internal", "compiled schema handle not found"),
                );
            };
            c
        };

        let instance_bytes = unsafe { bytes_as_slice(instance_json) };
        let instance_str = match core::str::from_utf8(instance_bytes) {
            Ok(s) => s,
            Err(_) => {
                return make_validate_err(
                    CODE_VALIDATE_INSTANCE_JSON_INVALID,
                    &single_error_json("", "json", "instance_json must be utf-8"),
                );
            }
        };

        let instance_val: Value = match serde_json::from_str(instance_str) {
            Ok(v) => v,
            Err(e) => {
                return make_validate_err(
                    CODE_VALIDATE_INSTANCE_JSON_INVALID,
                    &single_error_json("", "json", &format!("invalid instance_json: {e}")),
                );
            }
        };

        let out = match compiled.validator.validate(&instance_val) {
            Ok(()) => make_validate_ok(),
            Err(iter) => {
                let errors_json = errors_to_json(iter);
                make_validate_err(CODE_VALIDATE_ERRORS, &errors_json)
            }
        };
        out
    })
    .unwrap_or_else(|_| {
        make_validate_err(
            CODE_VALIDATE_INTERNAL,
            &single_error_json("", "internal", "validate panicked"),
        )
    })
}
