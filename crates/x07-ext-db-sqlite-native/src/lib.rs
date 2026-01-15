#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use dbcore::{
    alloc_return_bytes, bytes_as_slice, dm_doc_ok, dm_value_map, dm_value_null,
    dm_value_number_ascii, dm_value_seq, dm_value_string, effective_connect_timeout_ms,
    effective_max, effective_query_timeout_ms, env_bool, env_u32_nonzero, evdb_err, evdb_ok,
    parse_db_caps_v1, parse_params_doc_v1, read_u32_le, DmScalar, DB_ERR_BAD_CONN, DB_ERR_BAD_REQ,
    DB_ERR_POLICY_DENIED, DB_ERR_TOO_LARGE, OP_CLOSE_V1, OP_EXEC_V1, OP_OPEN_V1, OP_QUERY_V1,
};
use libsqlite3_sys as sqlite;
use once_cell::sync::OnceCell;
use std::ffi::{c_char, c_int, CStr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use x07_ext_db_native_core as dbcore;

const DB_ERR_SQLITE_OPEN: u32 = 53_504;
const DB_ERR_SQLITE_PREP: u32 = 53_505;
const DB_ERR_SQLITE_STEP: u32 = 53_506;
type ev_bytes = dbcore::ev_bytes;

const SQLITE_OK: c_int = sqlite::SQLITE_OK as c_int;
const SQLITE_ROW: c_int = sqlite::SQLITE_ROW as c_int;
const SQLITE_DONE: c_int = sqlite::SQLITE_DONE as c_int;

const OPEN_FLAG_READONLY_V1: u32 = 1 << 0;
const OPEN_FLAG_CREATE_V1: u32 = 1 << 1;

#[derive(Debug, Clone)]
struct Policy {
    sandboxed: bool,
    enabled: bool,
    sqlite_enabled: bool,
    sqlite_readonly_only: bool,
    sqlite_allow_create: bool,
    sqlite_allow_in_memory: bool,
    sqlite_allow_paths: Vec<PathBuf>,
    max_live_conns: u32,
    max_queries: u32,
    max_connect_timeout_ms: u32,
    max_query_timeout_ms: u32,
    max_rows: u32,
    max_resp_bytes: u32,
    max_sql_bytes: u32,
}

static POLICY: OnceCell<Policy> = OnceCell::new();

fn canonicalize_best_effort(p: &Path) -> PathBuf {
    if p.is_absolute() {
        return p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    }
    let abs = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(p);
    abs.canonicalize().unwrap_or(abs)
}

fn env_paths(name: &str) -> Vec<PathBuf> {
    let Ok(v) = std::env::var(name) else {
        return vec![];
    };
    v.split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| canonicalize_best_effort(Path::new(s)))
        .collect()
}

fn load_policy() -> Policy {
    let sandboxed = env_bool("X07_OS_SANDBOXED", false);
    let enabled = env_bool("X07_OS_DB", !sandboxed);
    let sqlite_enabled = env_bool("X07_OS_DB_SQLITE", !sandboxed);
    let sqlite_readonly_only = env_bool("X07_OS_DB_SQLITE_READONLY_ONLY", sandboxed);
    let sqlite_allow_create = env_bool("X07_OS_DB_SQLITE_ALLOW_CREATE", !sandboxed);
    let sqlite_allow_in_memory = env_bool("X07_OS_DB_SQLITE_ALLOW_IN_MEMORY", !sandboxed);
    let sqlite_allow_paths = env_paths("X07_OS_DB_SQLITE_ALLOW_PATHS");

    Policy {
        sandboxed,
        enabled,
        sqlite_enabled,
        sqlite_readonly_only,
        sqlite_allow_create,
        sqlite_allow_in_memory,
        sqlite_allow_paths,
        max_live_conns: env_u32_nonzero("X07_OS_DB_MAX_LIVE_CONNS", 8),
        max_queries: env_u32_nonzero("X07_OS_DB_MAX_QUERIES", 1000),
        max_connect_timeout_ms: env_u32_nonzero("X07_OS_DB_MAX_CONNECT_TIMEOUT_MS", 30_000),
        max_query_timeout_ms: env_u32_nonzero("X07_OS_DB_MAX_QUERY_TIMEOUT_MS", 60_000),
        max_rows: env_u32_nonzero("X07_OS_DB_MAX_ROWS", 10_000),
        max_resp_bytes: env_u32_nonzero("X07_OS_DB_MAX_RESP_BYTES", 32 * 1024 * 1024),
        max_sql_bytes: env_u32_nonzero("X07_OS_DB_MAX_SQL_BYTES", 1024 * 1024),
    }
}

fn policy() -> &'static Policy {
    POLICY.get_or_init(load_policy)
}

fn is_sqlite_path_allowed(path: &Path) -> bool {
    let pol = policy();
    if !pol.sandboxed {
        return true;
    }
    let cand = canonicalize_best_effort(path);
    pol.sqlite_allow_paths.iter().any(|p| p == &cand)
}

#[derive(Copy, Clone)]
struct SqliteConn(*mut sqlite::sqlite3);

unsafe impl Send for SqliteConn {}

static CONNS: OnceCell<Mutex<Vec<Option<SqliteConn>>>> = OnceCell::new();
static QUERIES: AtomicU32 = AtomicU32::new(0);

fn conns() -> &'static Mutex<Vec<Option<SqliteConn>>> {
    CONNS.get_or_init(|| Mutex::new(vec![None; 4096]))
}

unsafe fn sqlite_last_errmsg(db: *mut sqlite::sqlite3) -> Vec<u8> {
    if db.is_null() {
        return Vec::new();
    }
    let msg = sqlite::sqlite3_errmsg(db);
    if msg.is_null() {
        return Vec::new();
    }
    CStr::from_ptr(msg).to_bytes().to_vec()
}

unsafe fn bind_params(stmt: *mut sqlite::sqlite3_stmt, params_doc: &[u8]) -> Result<(), u32> {
    if params_doc.is_empty() {
        return Ok(());
    }
    let params = parse_params_doc_v1(params_doc)?;
    for (idx, param) in params.into_iter().enumerate() {
        let i = (idx + 1) as c_int;
        let rc = match param {
            DmScalar::Null => sqlite::sqlite3_bind_null(stmt, i),
            DmScalar::Bool(v) => sqlite::sqlite3_bind_int(stmt, i, if v { 1 } else { 0 }),
            DmScalar::NumberAscii(s) => {
                let s_txt = std::str::from_utf8(s).map_err(|_| DB_ERR_BAD_REQ)?;
                if s_txt.contains(['.', 'e', 'E']) {
                    let v = s_txt.parse::<f64>().map_err(|_| DB_ERR_BAD_REQ)?;
                    sqlite::sqlite3_bind_double(stmt, i, v)
                } else {
                    let v = s_txt.parse::<i64>().map_err(|_| DB_ERR_BAD_REQ)?;
                    sqlite::sqlite3_bind_int64(stmt, i, v)
                }
            }
            DmScalar::String(s) => sqlite::sqlite3_bind_text(
                stmt,
                i,
                s.as_ptr() as *const c_char,
                s.len() as c_int,
                sqlite::SQLITE_TRANSIENT(),
            ),
        };
        if rc != SQLITE_OK {
            return Err(DB_ERR_BAD_REQ);
        }
    }
    Ok(())
}

fn parse_evso_open_req(req: &[u8]) -> Result<(u32, Vec<u8>), u32> {
    if req.len() < 16 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &req[0..4] != b"X7SO" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(req, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let flags = read_u32_le(req, 8).ok_or(DB_ERR_BAD_REQ)?;
    let path_len = read_u32_le(req, 12).ok_or(DB_ERR_BAD_REQ)? as usize;
    if req.len() != 16 + path_len {
        return Err(DB_ERR_BAD_REQ);
    }
    Ok((flags, req[16..].to_vec()))
}

fn parse_evsq_req(req: &[u8], magic: &[u8; 4]) -> Result<(u32, u32, Vec<u8>, Vec<u8>), u32> {
    if req.len() < 24 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &req[0..4] != magic {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(req, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let conn_id = read_u32_le(req, 8).ok_or(DB_ERR_BAD_REQ)?;
    let flags = read_u32_le(req, 12).ok_or(DB_ERR_BAD_REQ)?;
    let sql_len = read_u32_le(req, 16).ok_or(DB_ERR_BAD_REQ)? as usize;
    if req.len() < 20 + sql_len + 4 {
        return Err(DB_ERR_BAD_REQ);
    }
    let sql_start = 20;
    let sql_end = sql_start + sql_len;
    let sql = req[sql_start..sql_end].to_vec();
    let params_len = read_u32_le(req, sql_end).ok_or(DB_ERR_BAD_REQ)? as usize;
    let params_start = sql_end + 4;
    let params_end = params_start + params_len;
    if req.len() != params_end {
        return Err(DB_ERR_BAD_REQ);
    }
    let params = req[params_start..params_end].to_vec();
    Ok((conn_id, flags, sql, params))
}

fn parse_evsc_close_req(req: &[u8]) -> Result<u32, u32> {
    if req.len() != 12 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &req[0..4] != b"X7SC" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(req, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let conn_id = read_u32_le(req, 8).ok_or(DB_ERR_BAD_REQ)?;
    Ok(conn_id)
}

fn open_slot(db: *mut sqlite::sqlite3, pol: &Policy) -> Option<u32> {
    let mut table = conns().lock().ok()?;
    if pol.max_live_conns != 0 {
        let live = table.iter().skip(1).filter(|s| s.is_some()).count();
        if live >= pol.max_live_conns as usize {
            return None;
        }
    }
    for (idx, slot) in table.iter_mut().enumerate().skip(1) {
        if slot.is_none() {
            *slot = Some(SqliteConn(db));
            return Some(idx as u32);
        }
    }
    None
}

fn take_conn(conn_id: u32) -> Option<*mut sqlite::sqlite3> {
    let mut table = conns().lock().ok()?;
    let slot = table.get_mut(conn_id as usize)?;
    slot.take().map(|c| c.0)
}

fn get_conn(conn_id: u32) -> Option<*mut sqlite::sqlite3> {
    let table = conns().lock().ok()?;
    table.get(conn_id as usize).copied().flatten().map(|c| c.0)
}

unsafe fn bytes_to_utf8_path(b: &[u8]) -> Result<PathBuf, u32> {
    let s = std::str::from_utf8(b).map_err(|_| DB_ERR_BAD_REQ)?;
    if s.contains('\0') {
        return Err(DB_ERR_BAD_REQ);
    }
    Ok(PathBuf::from(s))
}

fn count_query_or_deny(pol: &Policy, op: u32) -> Result<(), ev_bytes> {
    if pol.max_queries == 0 {
        return Ok(());
    }
    let prev = QUERIES.fetch_add(1, Ordering::Relaxed);
    if prev >= pol.max_queries {
        return Err(alloc_return_bytes(&evdb_err(op, DB_ERR_POLICY_DENIED, &[])));
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn x07_ext_db_sqlite_open_v1(req: ev_bytes, caps: ev_bytes) -> ev_bytes {
    let req = unsafe { bytes_as_slice(req) };
    let caps_raw = unsafe { bytes_as_slice(caps) };

    let pol = policy();
    if !pol.enabled || !pol.sqlite_enabled {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let caps = match parse_db_caps_v1(caps_raw) {
        Ok(c) => c,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &[])),
    };

    let (open_flags, path_bytes) = match parse_evso_open_req(req) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &[])),
    };

    if open_flags & !(OPEN_FLAG_READONLY_V1 | OPEN_FLAG_CREATE_V1) != 0 {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_BAD_REQ, &[]));
    }
    if (open_flags & OPEN_FLAG_CREATE_V1) != 0 && !pol.sqlite_allow_create {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_POLICY_DENIED, &[]));
    }
    if pol.sqlite_readonly_only && (open_flags & OPEN_FLAG_READONLY_V1) == 0 {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let is_memory = path_bytes == b":memory:";
    if is_memory && pol.sandboxed && !pol.sqlite_allow_in_memory {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let path = match unsafe { bytes_to_utf8_path(&path_bytes) } {
        Ok(p) => p,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &[])),
    };

    if !is_memory && !is_sqlite_path_allowed(&path) {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let cpath = match std::ffi::CString::new(path_bytes) {
        Ok(s) => s,
        Err(_) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_BAD_REQ, &[])),
    };

    let mut db: *mut sqlite::sqlite3 = std::ptr::null_mut();
    let flags = if (open_flags & OPEN_FLAG_READONLY_V1) != 0 {
        sqlite::SQLITE_OPEN_READONLY
    } else if (open_flags & OPEN_FLAG_CREATE_V1) != 0 {
        sqlite::SQLITE_OPEN_READWRITE | sqlite::SQLITE_OPEN_CREATE
    } else {
        sqlite::SQLITE_OPEN_READWRITE
    };

    let rc = unsafe { sqlite::sqlite3_open_v2(cpath.as_ptr(), &mut db, flags, std::ptr::null()) };
    if rc != SQLITE_OK || db.is_null() {
        let msg = unsafe { sqlite_last_errmsg(db) };
        if !db.is_null() {
            unsafe {
                let _ = sqlite::sqlite3_close(db);
            }
        }
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_SQLITE_OPEN, &msg));
    }

    let connect_timeout_ms = effective_connect_timeout_ms(pol.max_connect_timeout_ms, caps);
    if connect_timeout_ms != 0 {
        let timeout_i = connect_timeout_ms.min(c_int::MAX as u32) as c_int;
        unsafe {
            let _ = sqlite::sqlite3_busy_timeout(db, timeout_i);
        }
    }

    let Some(conn_id) = open_slot(db, pol) else {
        unsafe {
            let _ = sqlite::sqlite3_close(db);
        }
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_TOO_LARGE, &[]));
    };

    alloc_return_bytes(&evdb_ok(OP_OPEN_V1, &conn_id.to_le_bytes()))
}

#[no_mangle]
pub extern "C" fn x07_ext_db_sqlite_close_v1(req: ev_bytes, caps: ev_bytes) -> ev_bytes {
    let _caps_raw = unsafe { bytes_as_slice(caps) };
    let req = unsafe { bytes_as_slice(req) };

    let pol = policy();
    if !pol.enabled || !pol.sqlite_enabled {
        return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let conn_id = match parse_evsc_close_req(req) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, code, &[])),
    };

    let Some(db) = take_conn(conn_id) else {
        return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, DB_ERR_BAD_CONN, &[]));
    };

    let rc = unsafe { sqlite::sqlite3_close(db) };
    if rc != SQLITE_OK {
        return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, DB_ERR_BAD_CONN, &[]));
    }
    alloc_return_bytes(&evdb_ok(OP_CLOSE_V1, &[]))
}

unsafe fn query_rows_doc(
    stmt: *mut sqlite::sqlite3_stmt,
    _db: *mut sqlite::sqlite3,
    max_rows: u32,
) -> Result<Vec<u8>, u32> {
    let col_count = sqlite::sqlite3_column_count(stmt);
    if col_count < 0 {
        return Err(DB_ERR_BAD_REQ);
    }
    let col_count = col_count as usize;

    let mut cols: Vec<Vec<u8>> = Vec::with_capacity(col_count);
    for i in 0..col_count {
        let name = sqlite::sqlite3_column_name(stmt, i as c_int);
        if name.is_null() {
            cols.push(Vec::new());
            continue;
        }
        cols.push(CStr::from_ptr(name).to_bytes().to_vec());
    }

    let cols_value = dm_value_seq(&cols.iter().map(|s| dm_value_string(s)).collect::<Vec<_>>());

    let mut rows: Vec<Vec<u8>> = Vec::new();
    loop {
        let rc = sqlite::sqlite3_step(stmt);
        if rc == SQLITE_DONE {
            break;
        }
        if rc != SQLITE_ROW {
            return Err(DB_ERR_SQLITE_STEP);
        }

        if max_rows != 0 && rows.len() >= max_rows as usize {
            return Err(DB_ERR_TOO_LARGE);
        }

        let mut cells: Vec<Vec<u8>> = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let t = sqlite::sqlite3_column_type(stmt, i as c_int);
            let cell = match t {
                sqlite::SQLITE_NULL => dm_value_null(),
                sqlite::SQLITE_INTEGER => {
                    let v = sqlite::sqlite3_column_int64(stmt, i as c_int);
                    let mut buf = itoa::Buffer::new();
                    dm_value_number_ascii(buf.format(v).as_bytes())
                }
                sqlite::SQLITE_FLOAT => {
                    let v = sqlite::sqlite3_column_double(stmt, i as c_int);
                    let mut buf = ryu::Buffer::new();
                    dm_value_number_ascii(buf.format(v).as_bytes())
                }
                sqlite::SQLITE_TEXT => {
                    let ptr = sqlite::sqlite3_column_text(stmt, i as c_int);
                    let n = sqlite::sqlite3_column_bytes(stmt, i as c_int);
                    if ptr.is_null() || n <= 0 {
                        dm_value_string(&[])
                    } else {
                        let slice = std::slice::from_raw_parts(ptr, n as usize);
                        dm_value_string(slice)
                    }
                }
                sqlite::SQLITE_BLOB => {
                    let ptr = sqlite::sqlite3_column_blob(stmt, i as c_int);
                    let n = sqlite::sqlite3_column_bytes(stmt, i as c_int);
                    if ptr.is_null() || n <= 0 {
                        dm_value_string(&[])
                    } else {
                        let slice = std::slice::from_raw_parts(ptr as *const u8, n as usize);
                        dm_value_string(slice)
                    }
                }
                _ => dm_value_null(),
            };
            cells.push(cell);
        }
        rows.push(dm_value_seq(&cells));
    }

    let rows_value = dm_value_seq(&rows);
    let map_value = dm_value_map(vec![
        (b"cols".to_vec(), cols_value),
        (b"rows".to_vec(), rows_value),
    ])?;
    Ok(dm_doc_ok(&map_value))
}

#[no_mangle]
pub extern "C" fn x07_ext_db_sqlite_query_v1(req: ev_bytes, caps: ev_bytes) -> ev_bytes {
    let req = unsafe { bytes_as_slice(req) };
    let caps_raw = unsafe { bytes_as_slice(caps) };

    let pol = policy();
    if !pol.enabled || !pol.sqlite_enabled {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_POLICY_DENIED, &[]));
    }
    if let Err(out) = count_query_or_deny(pol, OP_QUERY_V1) {
        return out;
    }

    let caps = match parse_db_caps_v1(caps_raw) {
        Ok(c) => c,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };

    let (conn_id, _flags, sql, params) = match parse_evsq_req(req, b"X7SQ") {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };

    if sql.len() > pol.max_sql_bytes as usize {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_TOO_LARGE, &[]));
    }

    let Some(db) = get_conn(conn_id) else {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_BAD_CONN, &[]));
    };

    let timeout_ms = effective_query_timeout_ms(pol.max_query_timeout_ms, caps);
    if timeout_ms != 0 {
        let timeout_i = timeout_ms.min(c_int::MAX as u32) as c_int;
        unsafe {
            let _ = sqlite::sqlite3_busy_timeout(db, timeout_i);
        }
    }

    let sql_c = match std::ffi::CString::new(sql) {
        Ok(s) => s,
        Err(_) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_BAD_REQ, &[])),
    };

    let mut stmt: *mut sqlite::sqlite3_stmt = std::ptr::null_mut();
    let rc = unsafe {
        sqlite::sqlite3_prepare_v2(db, sql_c.as_ptr(), -1, &mut stmt, std::ptr::null_mut())
    };
    if rc != SQLITE_OK || stmt.is_null() {
        let msg = unsafe { sqlite_last_errmsg(db) };
        if !stmt.is_null() {
            unsafe {
                let _ = sqlite::sqlite3_finalize(stmt);
            }
        }
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_SQLITE_PREP, &msg));
    }

    let bind_res = unsafe { bind_params(stmt, &params) };
    if bind_res.is_err() {
        unsafe {
            let _ = sqlite::sqlite3_finalize(stmt);
        }
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_BAD_REQ, &[]));
    }

    let max_rows = effective_max(pol.max_rows, caps.max_rows);
    let doc = unsafe { query_rows_doc(stmt, db, max_rows) };
    unsafe {
        let _ = sqlite::sqlite3_finalize(stmt);
    }
    let doc = match doc {
        Ok(d) => d,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };

    let max_resp = effective_max(pol.max_resp_bytes, caps.max_resp_bytes);
    if max_resp != 0 && doc.len() > max_resp as usize {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_TOO_LARGE, &[]));
    }

    alloc_return_bytes(&evdb_ok(OP_QUERY_V1, &doc))
}

#[no_mangle]
pub extern "C" fn x07_ext_db_sqlite_exec_v1(req: ev_bytes, caps: ev_bytes) -> ev_bytes {
    let req = unsafe { bytes_as_slice(req) };
    let caps_raw = unsafe { bytes_as_slice(caps) };

    let pol = policy();
    if !pol.enabled || !pol.sqlite_enabled {
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_POLICY_DENIED, &[]));
    }
    if let Err(out) = count_query_or_deny(pol, OP_EXEC_V1) {
        return out;
    }

    let caps = match parse_db_caps_v1(caps_raw) {
        Ok(c) => c,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, code, &[])),
    };

    let (conn_id, _flags, sql, params) = match parse_evsq_req(req, b"X7SE") {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, code, &[])),
    };

    if sql.len() > pol.max_sql_bytes as usize {
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_TOO_LARGE, &[]));
    }

    let Some(db) = get_conn(conn_id) else {
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_BAD_CONN, &[]));
    };

    let timeout_ms = effective_query_timeout_ms(pol.max_query_timeout_ms, caps);
    if timeout_ms != 0 {
        let timeout_i = timeout_ms.min(c_int::MAX as u32) as c_int;
        unsafe {
            let _ = sqlite::sqlite3_busy_timeout(db, timeout_i);
        }
    }

    let sql_c = match std::ffi::CString::new(sql) {
        Ok(s) => s,
        Err(_) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_BAD_REQ, &[])),
    };

    let mut stmt: *mut sqlite::sqlite3_stmt = std::ptr::null_mut();
    let rc = unsafe {
        sqlite::sqlite3_prepare_v2(db, sql_c.as_ptr(), -1, &mut stmt, std::ptr::null_mut())
    };
    if rc != SQLITE_OK || stmt.is_null() {
        let msg = unsafe { sqlite_last_errmsg(db) };
        if !stmt.is_null() {
            unsafe {
                let _ = sqlite::sqlite3_finalize(stmt);
            }
        }
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_SQLITE_PREP, &msg));
    }

    let bind_res = unsafe { bind_params(stmt, &params) };
    if bind_res.is_err() {
        unsafe {
            let _ = sqlite::sqlite3_finalize(stmt);
        }
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_BAD_REQ, &[]));
    }

    loop {
        let rc = unsafe { sqlite::sqlite3_step(stmt) };
        if rc == SQLITE_DONE {
            break;
        }
        if rc == SQLITE_ROW {
            continue;
        }
        unsafe {
            let _ = sqlite::sqlite3_finalize(stmt);
        }
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_SQLITE_STEP, &[]));
    }

    unsafe {
        let _ = sqlite::sqlite3_finalize(stmt);
    }

    let rows_affected = unsafe { sqlite::sqlite3_changes(db) };
    let last_id = unsafe { sqlite::sqlite3_last_insert_rowid(db) };

    let mut entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut buf = itoa::Buffer::new();
    entries.push((
        b"last_insert_id".to_vec(),
        dm_value_number_ascii(buf.format(last_id).as_bytes()),
    ));
    let mut buf2 = itoa::Buffer::new();
    entries.push((
        b"rows_affected".to_vec(),
        dm_value_number_ascii(buf2.format(rows_affected).as_bytes()),
    ));

    let map_value = match dm_value_map(entries) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, code, &[])),
    };
    let doc = dm_doc_ok(&map_value);

    let max_resp = effective_max(pol.max_resp_bytes, caps.max_resp_bytes);
    if max_resp != 0 && doc.len() > max_resp as usize {
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_TOO_LARGE, &[]));
    }

    alloc_return_bytes(&evdb_ok(OP_EXEC_V1, &doc))
}
