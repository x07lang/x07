#![allow(clippy::missing_safety_doc)]

use dbcore::{
    alloc_return_bytes, bytes_as_slice, dm_doc_ok, dm_value_map, dm_value_null,
    dm_value_number_ascii, dm_value_seq, dm_value_string, effective_connect_timeout_ms,
    effective_max, effective_query_timeout_ms, evdb_err, evdb_ok, parse_db_caps_v1,
    parse_ipnet_list, parse_params_doc_v1, read_u32_le, DmScalar, DB_ERR_BAD_CONN, DB_ERR_BAD_REQ,
    DB_ERR_POLICY_DENIED, DB_ERR_TOO_LARGE, OP_CLOSE_V1, OP_EXEC_V1, OP_OPEN_V1, OP_QUERY_V1,
};
use futures_util::{pin_mut, TryStreamExt as _};
use once_cell::sync::OnceCell;
use rustls_tokio_postgres::{config_no_verify, config_webpki_roots, MakeRustlsConnect};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio_postgres::types::{ToSql, Type};
use tokio_postgres::{Client, Config, NoTls};
use x07_ext_db_native_core as dbcore;

const DB_ERR_PG_CONNECT: u32 = 53_520;
const DB_ERR_PG_QUERY: u32 = 53_521;
const DB_ERR_PG_EXEC: u32 = 53_522;
const DB_ERR_PG_TLS: u32 = 53_523;

#[derive(Debug, Clone)]
struct Policy {
    sandboxed: bool,
    enabled: bool,
    pg_enabled: bool,
    allow_dns: Vec<String>,
    allow_cidrs: Vec<dbcore::IpNet>,
    allow_ports: Vec<u16>,
    require_tls: bool,
    require_verify: bool,
    max_live_conns: u32,
    max_queries: u32,
    max_connect_timeout_ms: u32,
    max_query_timeout_ms: u32,
    max_rows: u32,
    max_resp_bytes: u32,
    max_sql_bytes: u32,
}

static POLICY: OnceCell<Policy> = OnceCell::new();
static RT: OnceCell<Runtime> = OnceCell::new();
static CONNS: OnceCell<Mutex<Vec<Option<Arc<Client>>>>> = OnceCell::new();
static QUERIES: AtomicU32 = AtomicU32::new(0);

fn runtime() -> &'static Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| dbcore::trap_db_internal())
    })
}

fn conns() -> &'static Mutex<Vec<Option<Arc<Client>>>> {
    CONNS.get_or_init(|| Mutex::new(vec![None; 4096]))
}

fn load_policy() -> Policy {
    let sandboxed = dbcore::env_bool("X07_OS_SANDBOXED", false);
    let enabled = dbcore::env_bool("X07_OS_DB", !sandboxed);
    let pg_enabled = dbcore::env_bool("X07_OS_DB_PG", !sandboxed);

    let allow_dns = dbcore::env_list("X07_OS_DB_NET_ALLOW_DNS", ';');
    let allow_cidrs_s = dbcore::env_list("X07_OS_DB_NET_ALLOW_CIDRS", ';');
    let allow_cidrs = parse_ipnet_list(&allow_cidrs_s);
    let allow_ports = dbcore::env_list_u16("X07_OS_DB_NET_ALLOW_PORTS", ',');

    Policy {
        sandboxed,
        enabled,
        pg_enabled,
        allow_dns,
        allow_cidrs,
        allow_ports,
        require_tls: dbcore::env_bool("X07_OS_DB_NET_REQUIRE_TLS", true),
        require_verify: dbcore::env_bool("X07_OS_DB_NET_REQUIRE_VERIFY", true),
        max_live_conns: dbcore::env_u32_nonzero("X07_OS_DB_MAX_LIVE_CONNS", 8),
        max_queries: dbcore::env_u32_nonzero("X07_OS_DB_MAX_QUERIES", 1000),
        max_connect_timeout_ms: dbcore::env_u32_nonzero("X07_OS_DB_MAX_CONNECT_TIMEOUT_MS", 30_000),
        max_query_timeout_ms: dbcore::env_u32_nonzero("X07_OS_DB_MAX_QUERY_TIMEOUT_MS", 60_000),
        max_rows: dbcore::env_u32_nonzero("X07_OS_DB_MAX_ROWS", 10_000),
        max_resp_bytes: dbcore::env_u32_nonzero("X07_OS_DB_MAX_RESP_BYTES", 32 * 1024 * 1024),
        max_sql_bytes: dbcore::env_u32_nonzero("X07_OS_DB_MAX_SQL_BYTES", 1024 * 1024),
    }
}

fn policy() -> &'static Policy {
    POLICY.get_or_init(load_policy)
}

fn count_query_or_deny(pol: &Policy, op: u32) -> Result<(), dbcore::ev_bytes> {
    if pol.max_queries == 0 {
        return Ok(());
    }
    let prev = QUERIES.fetch_add(1, Ordering::Relaxed);
    if prev >= pol.max_queries {
        return Err(alloc_return_bytes(&evdb_err(op, DB_ERR_POLICY_DENIED, &[])));
    }
    Ok(())
}

fn open_slot(client: Client, pol: &Policy) -> Option<u32> {
    let mut table = conns().lock().ok()?;
    if pol.max_live_conns != 0 {
        let live = table.iter().skip(1).filter(|s| s.is_some()).count();
        if live >= pol.max_live_conns as usize {
            return None;
        }
    }
    for (idx, slot) in table.iter_mut().enumerate().skip(1) {
        if slot.is_none() {
            *slot = Some(Arc::new(client));
            return Some(idx as u32);
        }
    }
    None
}

fn take_conn(conn_id: u32) -> Option<Arc<Client>> {
    let mut table = conns().lock().ok()?;
    let slot = table.get_mut(conn_id as usize)?;
    slot.take()
}

fn get_conn(conn_id: u32) -> Option<Arc<Client>> {
    let table = conns().lock().ok()?;
    table.get(conn_id as usize).cloned().flatten()
}

struct PgOpenReq<'a> {
    flags: u32,
    host: &'a [u8],
    port: u16,
    user: &'a [u8],
    pass: &'a [u8],
    db: &'a [u8],
}

fn parse_evpo_open_req(req: &[u8]) -> Result<PgOpenReq<'_>, u32> {
    if req.len() < 24 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &req[0..4] != b"X7PO" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(req, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }

    let flags = read_u32_le(req, 8).ok_or(DB_ERR_BAD_REQ)?;
    let mut off = 12usize;

    let host_len = read_u32_le(req, off).ok_or(DB_ERR_BAD_REQ)? as usize;
    off += 4;
    let host_end = off.checked_add(host_len).ok_or(DB_ERR_BAD_REQ)?;
    let host = req.get(off..host_end).ok_or(DB_ERR_BAD_REQ)?;
    off = host_end;

    let port_u32 = read_u32_le(req, off).ok_or(DB_ERR_BAD_REQ)?;
    off += 4;
    if port_u32 == 0 || port_u32 > 65535 {
        return Err(DB_ERR_BAD_REQ);
    }
    let port = port_u32 as u16;

    let user_len = read_u32_le(req, off).ok_or(DB_ERR_BAD_REQ)? as usize;
    off += 4;
    let user_end = off.checked_add(user_len).ok_or(DB_ERR_BAD_REQ)?;
    let user = req.get(off..user_end).ok_or(DB_ERR_BAD_REQ)?;
    off = user_end;

    let pass_len = read_u32_le(req, off).ok_or(DB_ERR_BAD_REQ)? as usize;
    off += 4;
    let pass_end = off.checked_add(pass_len).ok_or(DB_ERR_BAD_REQ)?;
    let pass = req.get(off..pass_end).ok_or(DB_ERR_BAD_REQ)?;
    off = pass_end;

    let db_len = read_u32_le(req, off).ok_or(DB_ERR_BAD_REQ)? as usize;
    off += 4;
    let db_end = off.checked_add(db_len).ok_or(DB_ERR_BAD_REQ)?;
    let db = req.get(off..db_end).ok_or(DB_ERR_BAD_REQ)?;
    off = db_end;

    if off != req.len() {
        return Err(DB_ERR_BAD_REQ);
    }

    Ok(PgOpenReq {
        flags,
        host,
        port,
        user,
        pass,
        db,
    })
}

struct PgSqlReq<'a> {
    conn_id: u32,
    sql: &'a [u8],
    params_doc: &'a [u8],
}

fn parse_evpq_req<'a>(req: &'a [u8], magic: &[u8; 4]) -> Result<PgSqlReq<'a>, u32> {
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
    if flags != 0 {
        return Err(DB_ERR_BAD_REQ);
    }

    let sql_len = read_u32_le(req, 16).ok_or(DB_ERR_BAD_REQ)? as usize;
    let mut off = 20usize;
    let sql_end = off.checked_add(sql_len).ok_or(DB_ERR_BAD_REQ)?;
    let sql = req.get(off..sql_end).ok_or(DB_ERR_BAD_REQ)?;
    off = sql_end;

    let params_len = read_u32_le(req, off).ok_or(DB_ERR_BAD_REQ)? as usize;
    off += 4;
    let params_end = off.checked_add(params_len).ok_or(DB_ERR_BAD_REQ)?;
    let params = req.get(off..params_end).ok_or(DB_ERR_BAD_REQ)?;
    off = params_end;

    if off != req.len() {
        return Err(DB_ERR_BAD_REQ);
    }

    Ok(PgSqlReq {
        conn_id,
        sql,
        params_doc: params,
    })
}

fn parse_evpc_close_req(req: &[u8]) -> Result<u32, u32> {
    if req.len() != 12 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &req[0..4] != b"X7PC" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(req, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let conn_id = read_u32_le(req, 8).ok_or(DB_ERR_BAD_REQ)?;
    Ok(conn_id)
}

fn pg_host_port_allowed(pol: &Policy, host: &str, port: u16) -> bool {
    if !pol.sandboxed {
        return true;
    }
    if !dbcore::db_host_allowed(host, &pol.allow_dns, &pol.allow_cidrs) {
        return false;
    }
    pol.allow_ports.contains(&port)
}

fn dm_rows_doc_from_pg(
    cols: &[tokio_postgres::Column],
    rows: &[tokio_postgres::Row],
) -> Result<Vec<u8>, u32> {
    let cols_val = dm_value_seq(
        &cols
            .iter()
            .map(|c| dm_value_string(c.name().as_bytes()))
            .collect::<Vec<_>>(),
    );

    let mut rows_vals: Vec<Vec<u8>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut cells: Vec<Vec<u8>> = Vec::with_capacity(cols.len());
        for (i, col) in cols.iter().enumerate() {
            let cell = match *col.type_() {
                Type::BOOL => match row.try_get::<usize, Option<bool>>(i) {
                    Ok(Some(v)) => dm_value_number_ascii(if v { b"1" } else { b"0" }),
                    Ok(None) => dm_value_null(),
                    Err(_) => dm_value_null(),
                },
                Type::INT2 => match row.try_get::<usize, Option<i16>>(i) {
                    Ok(Some(v)) => {
                        let mut buf = itoa::Buffer::new();
                        dm_value_number_ascii(buf.format(v).as_bytes())
                    }
                    Ok(None) => dm_value_null(),
                    Err(_) => dm_value_null(),
                },
                Type::INT4 => match row.try_get::<usize, Option<i32>>(i) {
                    Ok(Some(v)) => {
                        let mut buf = itoa::Buffer::new();
                        dm_value_number_ascii(buf.format(v).as_bytes())
                    }
                    Ok(None) => dm_value_null(),
                    Err(_) => dm_value_null(),
                },
                Type::INT8 => match row.try_get::<usize, Option<i64>>(i) {
                    Ok(Some(v)) => {
                        let mut buf = itoa::Buffer::new();
                        dm_value_number_ascii(buf.format(v).as_bytes())
                    }
                    Ok(None) => dm_value_null(),
                    Err(_) => dm_value_null(),
                },
                Type::FLOAT4 => match row.try_get::<usize, Option<f32>>(i) {
                    Ok(Some(v)) => {
                        let mut buf = ryu::Buffer::new();
                        dm_value_number_ascii(buf.format(v).as_bytes())
                    }
                    Ok(None) => dm_value_null(),
                    Err(_) => dm_value_null(),
                },
                Type::FLOAT8 => match row.try_get::<usize, Option<f64>>(i) {
                    Ok(Some(v)) => {
                        let mut buf = ryu::Buffer::new();
                        dm_value_number_ascii(buf.format(v).as_bytes())
                    }
                    Ok(None) => dm_value_null(),
                    Err(_) => dm_value_null(),
                },
                Type::BYTEA => match row.try_get::<usize, Option<Vec<u8>>>(i) {
                    Ok(Some(v)) => dm_value_string(&v),
                    Ok(None) => dm_value_null(),
                    Err(_) => dm_value_null(),
                },
                _ => match row.try_get::<usize, Option<String>>(i) {
                    Ok(Some(v)) => dm_value_string(v.as_bytes()),
                    Ok(None) => dm_value_null(),
                    Err(_) => dm_value_null(),
                },
            };
            cells.push(cell);
        }
        rows_vals.push(dm_value_seq(&cells));
    }

    let rows_val = dm_value_seq(&rows_vals);
    let map_val = dm_value_map(vec![
        (b"cols".to_vec(), cols_val),
        (b"rows".to_vec(), rows_val),
    ])?;
    Ok(dm_doc_ok(&map_val))
}

fn pg_params_as_unknown_text(params_doc: &[u8]) -> Result<Vec<Option<String>>, u32> {
    if params_doc.is_empty() {
        return Ok(vec![]);
    }
    let params = parse_params_doc_v1(params_doc)?;
    let mut out: Vec<Option<String>> = Vec::with_capacity(params.len());
    for p in params {
        let s = match p {
            DmScalar::Null => None,
            DmScalar::Bool(v) => Some(if v {
                "true".to_string()
            } else {
                "false".to_string()
            }),
            DmScalar::NumberAscii(b) => Some(
                std::str::from_utf8(b)
                    .map_err(|_| DB_ERR_BAD_REQ)?
                    .to_string(),
            ),
            DmScalar::String(b) => Some(
                std::str::from_utf8(b)
                    .map_err(|_| DB_ERR_BAD_REQ)?
                    .to_string(),
            ),
        };
        out.push(s);
    }
    Ok(out)
}

#[no_mangle]
pub extern "C" fn x07_ext_db_pg_open_v1(
    req: dbcore::ev_bytes,
    caps: dbcore::ev_bytes,
) -> dbcore::ev_bytes {
    let req = unsafe { bytes_as_slice(req) };
    let caps_raw = unsafe { bytes_as_slice(caps) };

    let pol = policy();
    if !pol.enabled || !pol.pg_enabled {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let caps = match parse_db_caps_v1(caps_raw) {
        Ok(c) => c,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &[])),
    };

    let open = match parse_evpo_open_req(req) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &[])),
    };

    if open.flags != 0 {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_BAD_REQ, &[]));
    }

    let host = match std::str::from_utf8(open.host) {
        Ok(s) => s,
        Err(_) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_BAD_REQ, &[])),
    };
    if !pg_host_port_allowed(pol, host, open.port) {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let user = std::str::from_utf8(open.user).map_err(|_| DB_ERR_BAD_REQ);
    let pass = std::str::from_utf8(open.pass).map_err(|_| DB_ERR_BAD_REQ);
    let db = std::str::from_utf8(open.db).map_err(|_| DB_ERR_BAD_REQ);
    let (Ok(user), Ok(pass), Ok(db)) = (user, pass, db) else {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_BAD_REQ, &[]));
    };

    let timeout_ms = effective_connect_timeout_ms(pol.max_connect_timeout_ms, caps);

    let client = match runtime().block_on(async {
        let mut cfg = Config::new();
        cfg.host(host);
        cfg.port(open.port);
        if !user.is_empty() {
            cfg.user(user);
        }
        if !pass.is_empty() {
            cfg.password(pass);
        }
        if !db.is_empty() {
            cfg.dbname(db);
        }
        if timeout_ms != 0 {
            cfg.connect_timeout(Duration::from_millis(timeout_ms as u64));
        }

        if pol.sandboxed && pol.require_tls {
            cfg.ssl_mode(tokio_postgres::config::SslMode::Require);
            let tls_cfg = if pol.require_verify {
                config_webpki_roots()
            } else {
                config_no_verify()
            };
            let tls = MakeRustlsConnect::new(tls_cfg);
            let (client, connection) = cfg
                .connect(tls)
                .await
                .map_err(|e| (DB_ERR_PG_TLS, e.to_string().into_bytes()))?;
            tokio::spawn(async move {
                let _ = connection.await;
            });
            Ok::<Client, (u32, Vec<u8>)>(client)
        } else {
            cfg.ssl_mode(tokio_postgres::config::SslMode::Disable);
            let (client, connection) = cfg
                .connect(NoTls)
                .await
                .map_err(|e| (DB_ERR_PG_CONNECT, e.to_string().into_bytes()))?;
            tokio::spawn(async move {
                let _ = connection.await;
            });
            Ok::<Client, (u32, Vec<u8>)>(client)
        }
    }) {
        Ok(v) => v,
        Err((code, msg)) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &msg)),
    };

    let Some(conn_id) = open_slot(client, pol) else {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_TOO_LARGE, &[]));
    };

    alloc_return_bytes(&evdb_ok(OP_OPEN_V1, &conn_id.to_le_bytes()))
}

#[no_mangle]
pub extern "C" fn x07_ext_db_pg_close_v1(
    req: dbcore::ev_bytes,
    caps: dbcore::ev_bytes,
) -> dbcore::ev_bytes {
    let _caps_raw = unsafe { bytes_as_slice(caps) };
    let req = unsafe { bytes_as_slice(req) };

    let pol = policy();
    if !pol.enabled || !pol.pg_enabled {
        return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let conn_id = match parse_evpc_close_req(req) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, code, &[])),
    };

    if take_conn(conn_id).is_none() {
        return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, DB_ERR_BAD_CONN, &[]));
    }

    alloc_return_bytes(&evdb_ok(OP_CLOSE_V1, &[]))
}

#[no_mangle]
pub extern "C" fn x07_ext_db_pg_query_v1(
    req: dbcore::ev_bytes,
    caps: dbcore::ev_bytes,
) -> dbcore::ev_bytes {
    let req = unsafe { bytes_as_slice(req) };
    let caps_raw = unsafe { bytes_as_slice(caps) };

    let pol = policy();
    if !pol.enabled || !pol.pg_enabled {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_POLICY_DENIED, &[]));
    }
    if let Err(out) = count_query_or_deny(pol, OP_QUERY_V1) {
        return out;
    }

    let caps = match parse_db_caps_v1(caps_raw) {
        Ok(c) => c,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };

    let sql_req = match parse_evpq_req(req, b"X7PQ") {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };
    let conn_id = sql_req.conn_id;
    let sql = sql_req.sql;
    let params_doc = sql_req.params_doc;

    if sql.len() > pol.max_sql_bytes as usize {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_TOO_LARGE, &[]));
    }

    let Some(client) = get_conn(conn_id) else {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_BAD_CONN, &[]));
    };

    let sql = match std::str::from_utf8(sql) {
        Ok(s) => s.to_string(),
        Err(_) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_BAD_REQ, &[])),
    };

    let params = match pg_params_as_unknown_text(params_doc) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };

    let max_rows = effective_max(pol.max_rows, caps.max_rows);
    let timeout_ms = effective_query_timeout_ms(pol.max_query_timeout_ms, caps);

    let doc = match runtime().block_on(async move {
        let stmt = client
            .prepare(&sql)
            .await
            .map_err(|e| (DB_ERR_PG_QUERY, e.to_string().into_bytes()))?;

        let stream = client
            .query_raw(&stmt, params.iter().map(|p| p as &dyn ToSql))
            .await
            .map_err(|e| (DB_ERR_PG_QUERY, e.to_string().into_bytes()))?;
        pin_mut!(stream);

        let mut rows: Vec<tokio_postgres::Row> = Vec::new();
        let mut too_many = false;

        loop {
            let next = if timeout_ms != 0 {
                tokio::time::timeout(Duration::from_millis(timeout_ms as u64), stream.try_next())
                    .await
                    .map_err(|_| (DB_ERR_PG_QUERY, b"timeout".to_vec()))?
            } else {
                stream.try_next().await
            };
            let row = next.map_err(|e| (DB_ERR_PG_QUERY, e.to_string().into_bytes()))?;
            let Some(row) = row else {
                break;
            };
            if max_rows != 0 && rows.len() >= max_rows as usize {
                too_many = true;
                continue;
            }
            rows.push(row);
        }

        if too_many {
            return Err((DB_ERR_TOO_LARGE, Vec::new()));
        }

        dm_rows_doc_from_pg(stmt.columns(), &rows).map_err(|code| (code, Vec::new()))
    }) {
        Ok(doc) => doc,
        Err((code, msg)) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &msg)),
    };

    let max_resp = effective_max(pol.max_resp_bytes, caps.max_resp_bytes);
    if max_resp != 0 && doc.len() > max_resp as usize {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_TOO_LARGE, &[]));
    }

    alloc_return_bytes(&evdb_ok(OP_QUERY_V1, &doc))
}

#[no_mangle]
pub extern "C" fn x07_ext_db_pg_exec_v1(
    req: dbcore::ev_bytes,
    caps: dbcore::ev_bytes,
) -> dbcore::ev_bytes {
    let req = unsafe { bytes_as_slice(req) };
    let caps_raw = unsafe { bytes_as_slice(caps) };

    let pol = policy();
    if !pol.enabled || !pol.pg_enabled {
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_POLICY_DENIED, &[]));
    }
    if let Err(out) = count_query_or_deny(pol, OP_EXEC_V1) {
        return out;
    }

    let caps = match parse_db_caps_v1(caps_raw) {
        Ok(c) => c,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, code, &[])),
    };

    let sql_req = match parse_evpq_req(req, b"X7PE") {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, code, &[])),
    };
    let conn_id = sql_req.conn_id;
    let sql = sql_req.sql;
    let params_doc = sql_req.params_doc;

    if sql.len() > pol.max_sql_bytes as usize {
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_TOO_LARGE, &[]));
    }

    let Some(client) = get_conn(conn_id) else {
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_BAD_CONN, &[]));
    };

    let sql = match std::str::from_utf8(sql) {
        Ok(s) => s.to_string(),
        Err(_) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_BAD_REQ, &[])),
    };

    let params = match pg_params_as_unknown_text(params_doc) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, code, &[])),
    };

    let timeout_ms = effective_query_timeout_ms(pol.max_query_timeout_ms, caps);

    let rows_affected = match runtime().block_on(async move {
        let stmt = client
            .prepare(&sql)
            .await
            .map_err(|e| (DB_ERR_PG_EXEC, e.to_string().into_bytes()))?;

        let stream = client
            .query_raw(&stmt, params.iter().map(|p| p as &dyn ToSql))
            .await
            .map_err(|e| (DB_ERR_PG_EXEC, e.to_string().into_bytes()))?;
        pin_mut!(stream);

        loop {
            let next = if timeout_ms != 0 {
                tokio::time::timeout(Duration::from_millis(timeout_ms as u64), stream.try_next())
                    .await
                    .map_err(|_| (DB_ERR_PG_EXEC, b"timeout".to_vec()))?
            } else {
                stream.try_next().await
            };
            let row = next.map_err(|e| (DB_ERR_PG_EXEC, e.to_string().into_bytes()))?;
            if row.is_none() {
                break;
            }
        }

        Ok::<u64, (u32, Vec<u8>)>(stream.rows_affected().unwrap_or(0))
    }) {
        Ok(v) => v,
        Err((code, msg)) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, code, &msg)),
    };

    let mut entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    entries.push((b"last_insert_id".to_vec(), dm_value_number_ascii(b"0")));
    let mut buf = itoa::Buffer::new();
    entries.push((
        b"rows_affected".to_vec(),
        dm_value_number_ascii(buf.format(rows_affected).as_bytes()),
    ));

    let map_val = match dm_value_map(entries) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_EXEC_V1, code, &[])),
    };
    let doc = dm_doc_ok(&map_val);

    let max_resp = effective_max(pol.max_resp_bytes, caps.max_resp_bytes);
    if max_resp != 0 && doc.len() > max_resp as usize {
        return alloc_return_bytes(&evdb_err(OP_EXEC_V1, DB_ERR_TOO_LARGE, &[]));
    }

    alloc_return_bytes(&evdb_ok(OP_EXEC_V1, &doc))
}
