#![allow(clippy::missing_safety_doc)]

use dbcore::{
    alloc_return_bytes, bytes_as_slice, dm_doc_ok, dm_value_bool, dm_value_map, dm_value_null,
    dm_value_number_ascii, dm_value_seq, dm_value_string, effective_connect_timeout_ms,
    effective_max, effective_query_timeout_ms, evdb_err, evdb_ok, parse_db_caps_v1,
    parse_ipnet_list, read_u32_le, DB_ERR_BAD_CONN, DB_ERR_BAD_REQ, DB_ERR_POLICY_DENIED,
    DB_ERR_TOO_LARGE, OP_CLOSE_V1, OP_OPEN_V1, OP_QUERY_V1,
};
use once_cell::sync::OnceCell;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, Error as RustlsError, SignatureScheme};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, BufStream,
};
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::runtime::Runtime;
use tokio_rustls::TlsConnector;
use x07_ext_db_native_core as dbcore;

const DB_ERR_REDIS_CONNECT: u32 = 53_552;
const DB_ERR_REDIS_CMD: u32 = 53_553;
const DB_ERR_REDIS_PROTOCOL: u32 = 53_554;
const DB_ERR_REDIS_TLS: u32 = 53_555;
const DB_ERR_REDIS_SERVER: u32 = 53_556;

trait AsyncReadWrite: AsyncRead + AsyncWrite {}
impl<T: AsyncRead + AsyncWrite + ?Sized> AsyncReadWrite for T {}

type DynStream = Pin<Box<dyn AsyncReadWrite + Send>>;
type RedisConnHandle = Arc<tokio::sync::Mutex<RedisConn>>;
type RedisConnTable = Vec<Option<RedisConnHandle>>;
type Resp3ReadFuture<'a> = Pin<Box<dyn Future<Output = Result<Resp3, (u32, Vec<u8>)>> + 'a>>;

#[derive(Debug, Clone)]
struct Policy {
    sandboxed: bool,
    enabled: bool,
    redis_enabled: bool,
    allow_dns: Vec<String>,
    allow_cidrs: Vec<dbcore::IpNet>,
    allow_ports: Vec<u16>,
    require_tls: bool,
    require_verify: bool,
    max_live_conns: u32,
    max_queries: u32,
    max_connect_timeout_ms: u32,
    max_query_timeout_ms: u32,
    max_resp_bytes: u32,
    max_req_bytes: u32,
}

static POLICY: OnceCell<Policy> = OnceCell::new();
static RT: OnceCell<Runtime> = OnceCell::new();
static CONNS: OnceCell<Mutex<RedisConnTable>> = OnceCell::new();
static QUERIES: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
struct AcceptAllVerifier;

impl ServerCertVerifier for AcceptAllVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::ED25519,
        ]
    }
}

fn tls_config_webpki_roots() -> ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}

fn tls_config_no_verify() -> ClientConfig {
    let mut cfg = ClientConfig::builder()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
    cfg.dangerous()
        .set_certificate_verifier(Arc::new(AcceptAllVerifier));
    cfg
}

fn runtime() -> &'static Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|_| dbcore::trap_db_internal())
    })
}

fn conns() -> &'static Mutex<RedisConnTable> {
    CONNS.get_or_init(|| Mutex::new(vec![None; 4096]))
}

fn load_policy() -> Policy {
    let sandboxed = dbcore::env_bool("X07_OS_SANDBOXED", false);
    let enabled = dbcore::env_bool("X07_OS_DB", !sandboxed);
    let redis_enabled = dbcore::env_bool("X07_OS_DB_REDIS", !sandboxed);

    let allow_dns = dbcore::env_list("X07_OS_DB_NET_ALLOW_DNS", ';');
    let allow_cidrs_s = dbcore::env_list("X07_OS_DB_NET_ALLOW_CIDRS", ';');
    let allow_cidrs = parse_ipnet_list(&allow_cidrs_s);
    let allow_ports = dbcore::env_list_u16("X07_OS_DB_NET_ALLOW_PORTS", ',');

    Policy {
        sandboxed,
        enabled,
        redis_enabled,
        allow_dns,
        allow_cidrs,
        allow_ports,
        require_tls: dbcore::env_bool("X07_OS_DB_NET_REQUIRE_TLS", true),
        require_verify: dbcore::env_bool("X07_OS_DB_NET_REQUIRE_VERIFY", true),
        max_live_conns: dbcore::env_u32_nonzero("X07_OS_DB_MAX_LIVE_CONNS", 8),
        max_queries: dbcore::env_u32_nonzero("X07_OS_DB_MAX_QUERIES", 1000),
        max_connect_timeout_ms: dbcore::env_u32_nonzero("X07_OS_DB_MAX_CONNECT_TIMEOUT_MS", 30_000),
        max_query_timeout_ms: dbcore::env_u32_nonzero("X07_OS_DB_MAX_QUERY_TIMEOUT_MS", 60_000),
        max_resp_bytes: dbcore::env_u32_nonzero("X07_OS_DB_MAX_RESP_BYTES", 32 * 1024 * 1024),
        max_req_bytes: dbcore::env_u32_nonzero("X07_OS_DB_MAX_SQL_BYTES", 1024 * 1024),
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

fn open_slot(conn: RedisConn, pol: &Policy) -> Option<u32> {
    let mut table = conns().lock().ok()?;
    if pol.max_live_conns != 0 {
        let live = table.iter().skip(1).filter(|s| s.is_some()).count();
        if live >= pol.max_live_conns as usize {
            return None;
        }
    }
    for (idx, slot) in table.iter_mut().enumerate().skip(1) {
        if slot.is_none() {
            *slot = Some(Arc::new(tokio::sync::Mutex::new(conn)));
            return Some(idx as u32);
        }
    }
    None
}

fn take_conn(conn_id: u32) -> Option<RedisConnHandle> {
    let mut table = conns().lock().ok()?;
    let slot = table.get_mut(conn_id as usize)?;
    slot.take()
}

fn get_conn(conn_id: u32) -> Option<RedisConnHandle> {
    let table = conns().lock().ok()?;
    table.get(conn_id as usize).cloned().flatten()
}

enum RedisAddr<'a> {
    Tcp { host: &'a [u8], port: u16 },
    Unix { path: &'a [u8] },
}

struct RedisOpenReq<'a> {
    flags: u32,
    addr: RedisAddr<'a>,
    user: &'a [u8],
    pass: &'a [u8],
    db: u32,
}

fn parse_evro_open_req(req: &[u8]) -> Result<RedisOpenReq<'_>, u32> {
    if req.len() < 20 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &req[0..4] != b"X7RO" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(req, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let flags = read_u32_le(req, 8).ok_or(DB_ERR_BAD_REQ)?;
    let kind = read_u32_le(req, 12).ok_or(DB_ERR_BAD_REQ)?;

    let mut off = 16usize;
    let addr = match kind {
        1 => {
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
            RedisAddr::Tcp {
                host,
                port: port_u32 as u16,
            }
        }
        2 => {
            let path_len = read_u32_le(req, off).ok_or(DB_ERR_BAD_REQ)? as usize;
            off += 4;
            let path_end = off.checked_add(path_len).ok_or(DB_ERR_BAD_REQ)?;
            let path = req.get(off..path_end).ok_or(DB_ERR_BAD_REQ)?;
            off = path_end;
            RedisAddr::Unix { path }
        }
        _ => return Err(DB_ERR_BAD_REQ),
    };

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

    let db = read_u32_le(req, off).ok_or(DB_ERR_BAD_REQ)?;
    off += 4;

    if off != req.len() {
        return Err(DB_ERR_BAD_REQ);
    }

    Ok(RedisOpenReq {
        flags,
        addr,
        user,
        pass,
        db,
    })
}

fn parse_evrq_cmd_req(req: &[u8]) -> Result<(u32, u32, &[u8]), u32> {
    if req.len() < 20 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &req[0..4] != b"X7RQ" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(req, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let _flags = read_u32_le(req, 8).ok_or(DB_ERR_BAD_REQ)?;
    let conn_id = read_u32_le(req, 12).ok_or(DB_ERR_BAD_REQ)?;
    let argv_len = read_u32_le(req, 16).ok_or(DB_ERR_BAD_REQ)? as usize;
    let off = 20usize;
    let argv_end = off.checked_add(argv_len).ok_or(DB_ERR_BAD_REQ)?;
    if argv_end != req.len() {
        return Err(DB_ERR_BAD_REQ);
    }
    let argv = req.get(off..argv_end).ok_or(DB_ERR_BAD_REQ)?;
    Ok((conn_id, _flags, argv))
}

fn parse_evrx_close_req(req: &[u8]) -> Result<u32, u32> {
    if req.len() != 16 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &req[0..4] != b"X7RX" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(req, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let _flags = read_u32_le(req, 8).ok_or(DB_ERR_BAD_REQ)?;
    let conn_id = read_u32_le(req, 12).ok_or(DB_ERR_BAD_REQ)?;
    Ok(conn_id)
}

fn parse_evrv_argv<'a>(argv: &'a [u8]) -> Result<Vec<&'a [u8]>, u32> {
    if argv.len() < 12 {
        return Err(DB_ERR_BAD_REQ);
    }
    if &argv[0..4] != b"X7RV" {
        return Err(DB_ERR_BAD_REQ);
    }
    let ver = read_u32_le(argv, 4).ok_or(DB_ERR_BAD_REQ)?;
    if ver != 1 {
        return Err(DB_ERR_BAD_REQ);
    }
    let count = read_u32_le(argv, 8).ok_or(DB_ERR_BAD_REQ)? as usize;
    let mut off = 12usize;
    let mut out: Vec<&'a [u8]> = Vec::with_capacity(count);
    for _ in 0..count {
        let len = read_u32_le(argv, off).ok_or(DB_ERR_BAD_REQ)? as usize;
        off += 4;
        let end = off.checked_add(len).ok_or(DB_ERR_BAD_REQ)?;
        let slice = argv.get(off..end).ok_or(DB_ERR_BAD_REQ)?;
        out.push(slice);
        off = end;
    }
    if off != argv.len() {
        return Err(DB_ERR_BAD_REQ);
    }
    Ok(out)
}

fn redis_host_port_allowed(pol: &Policy, host: &str, port: u16) -> bool {
    if !pol.sandboxed {
        return true;
    }
    if !dbcore::db_host_allowed(host, &pol.allow_dns, &pol.allow_cidrs) {
        return false;
    }
    pol.allow_ports.contains(&port)
}

fn bytes_to_utf8_path(b: &[u8]) -> Result<PathBuf, u32> {
    let s = std::str::from_utf8(b).map_err(|_| DB_ERR_BAD_REQ)?;
    if s.contains('\0') {
        return Err(DB_ERR_BAD_REQ);
    }
    Ok(PathBuf::from(s))
}

struct RedisConn {
    io: BufStream<DynStream>,
}

enum Resp3 {
    Null,
    Bool(bool),
    Number(Vec<u8>),
    String(Vec<u8>),
    Seq(Vec<Resp3>),
    Map(Vec<(Resp3, Resp3)>),
    Set(Vec<Resp3>),
    Error(Vec<u8>),
}

async fn read_line_crlf(io: &mut BufStream<DynStream>) -> Result<Vec<u8>, (u32, Vec<u8>)> {
    let mut line: Vec<u8> = Vec::new();
    io.read_until(b'\n', &mut line)
        .await
        .map_err(|e| (DB_ERR_REDIS_PROTOCOL, e.to_string().into_bytes()))?;
    if line.len() < 2 || line[line.len() - 2] != b'\r' || line[line.len() - 1] != b'\n' {
        return Err((DB_ERR_REDIS_PROTOCOL, Vec::new()));
    }
    line.truncate(line.len() - 2);
    Ok(line)
}

fn read_resp3<'a>(io: &'a mut BufStream<DynStream>, depth: usize) -> Resp3ReadFuture<'a> {
    Box::pin(async move {
        if depth == 0 {
            return Err((DB_ERR_REDIS_PROTOCOL, Vec::new()));
        }
        let mut prefix = [0u8; 1];
        io.read_exact(&mut prefix)
            .await
            .map_err(|e| (DB_ERR_REDIS_PROTOCOL, e.to_string().into_bytes()))?;
        match prefix[0] {
            b'+' => Ok(Resp3::String(read_line_crlf(io).await?)),
            b':' | b',' | b'(' => Ok(Resp3::Number(read_line_crlf(io).await?)),
            b'_' => {
                let _ = read_line_crlf(io).await?;
                Ok(Resp3::Null)
            }
            b'#' => {
                let b = read_line_crlf(io).await?;
                match b.first().copied() {
                    Some(b't') => Ok(Resp3::Bool(true)),
                    Some(b'f') => Ok(Resp3::Bool(false)),
                    _ => Err((DB_ERR_REDIS_PROTOCOL, Vec::new())),
                }
            }
            b'$' | b'=' | b'!' => {
                let len_b = read_line_crlf(io).await?;
                let len_s =
                    std::str::from_utf8(&len_b).map_err(|_| (DB_ERR_REDIS_PROTOCOL, Vec::new()))?;
                let len_i = len_s
                    .parse::<i64>()
                    .map_err(|_| (DB_ERR_REDIS_PROTOCOL, Vec::new()))?;
                if len_i < 0 {
                    if prefix[0] == b'!' {
                        return Err((DB_ERR_REDIS_PROTOCOL, Vec::new()));
                    }
                    return Ok(Resp3::Null);
                }
                let len = len_i as usize;
                let mut payload = vec![0u8; len];
                io.read_exact(&mut payload)
                    .await
                    .map_err(|e| (DB_ERR_REDIS_PROTOCOL, e.to_string().into_bytes()))?;
                let mut crlf = [0u8; 2];
                io.read_exact(&mut crlf)
                    .await
                    .map_err(|e| (DB_ERR_REDIS_PROTOCOL, e.to_string().into_bytes()))?;
                if crlf != [b'\r', b'\n'] {
                    return Err((DB_ERR_REDIS_PROTOCOL, Vec::new()));
                }
                if prefix[0] == b'!' {
                    Ok(Resp3::Error(payload))
                } else {
                    Ok(Resp3::String(payload))
                }
            }
            b'-' => Ok(Resp3::Error(read_line_crlf(io).await?)),
            b'*' => {
                let n_b = read_line_crlf(io).await?;
                let n_s =
                    std::str::from_utf8(&n_b).map_err(|_| (DB_ERR_REDIS_PROTOCOL, Vec::new()))?;
                let n_i = n_s
                    .parse::<i64>()
                    .map_err(|_| (DB_ERR_REDIS_PROTOCOL, Vec::new()))?;
                if n_i < 0 {
                    return Ok(Resp3::Null);
                }
                let n = n_i as usize;
                let mut out: Vec<Resp3> = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(read_resp3(io, depth - 1).await?);
                }
                Ok(Resp3::Seq(out))
            }
            b'%' | b'|' => {
                let n_b = read_line_crlf(io).await?;
                let n_s =
                    std::str::from_utf8(&n_b).map_err(|_| (DB_ERR_REDIS_PROTOCOL, Vec::new()))?;
                let n_i = n_s
                    .parse::<i64>()
                    .map_err(|_| (DB_ERR_REDIS_PROTOCOL, Vec::new()))?;
                if n_i < 0 {
                    return Ok(Resp3::Null);
                }
                let n = n_i as usize;
                let mut entries: Vec<(Resp3, Resp3)> = Vec::with_capacity(n);
                for _ in 0..n {
                    let k = read_resp3(io, depth - 1).await?;
                    let v = read_resp3(io, depth - 1).await?;
                    entries.push((k, v));
                }
                if prefix[0] == b'|' {
                    read_resp3(io, depth - 1).await
                } else {
                    Ok(Resp3::Map(entries))
                }
            }
            b'~' => {
                let n_b = read_line_crlf(io).await?;
                let n_s =
                    std::str::from_utf8(&n_b).map_err(|_| (DB_ERR_REDIS_PROTOCOL, Vec::new()))?;
                let n_i = n_s
                    .parse::<i64>()
                    .map_err(|_| (DB_ERR_REDIS_PROTOCOL, Vec::new()))?;
                if n_i < 0 {
                    return Ok(Resp3::Null);
                }
                let n = n_i as usize;
                let mut out: Vec<Resp3> = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(read_resp3(io, depth - 1).await?);
                }
                Ok(Resp3::Set(out))
            }
            _ => Err((DB_ERR_REDIS_PROTOCOL, Vec::new())),
        }
    })
}

fn key_bytes(v: Resp3) -> Result<Vec<u8>, u32> {
    match v {
        Resp3::String(b) => Ok(b),
        Resp3::Number(b) => Ok(b),
        Resp3::Bool(b) => Ok(if b {
            b"true".to_vec()
        } else {
            b"false".to_vec()
        }),
        _ => Err(DB_ERR_REDIS_PROTOCOL),
    }
}

fn resp_to_dm_value(v: Resp3) -> Result<Vec<u8>, u32> {
    match v {
        Resp3::Null => Ok(dm_value_null()),
        Resp3::Bool(b) => Ok(dm_value_bool(b)),
        Resp3::Number(b) => Ok(dm_value_number_ascii(&b)),
        Resp3::String(b) => Ok(dm_value_string(&b)),
        Resp3::Seq(items) => {
            let mut vals: Vec<Vec<u8>> = Vec::with_capacity(items.len());
            for it in items {
                vals.push(resp_to_dm_value(it)?);
            }
            Ok(dm_value_seq(&vals))
        }
        Resp3::Map(entries) => {
            let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                let kb = key_bytes(k)?;
                let vb = resp_to_dm_value(v)?;
                out.push((kb, vb));
            }
            dm_value_map(out)
        }
        Resp3::Set(items) => {
            let mut vals: Vec<Vec<u8>> = Vec::with_capacity(items.len());
            for it in items {
                vals.push(resp_to_dm_value(it)?);
            }
            vals.sort();
            Ok(dm_value_seq(&vals))
        }
        Resp3::Error(_) => Err(DB_ERR_REDIS_PROTOCOL),
    }
}

async fn write_argv(io: &mut BufStream<DynStream>, argv: &[&[u8]]) -> Result<(), (u32, Vec<u8>)> {
    io.write_all(format!("*{}\r\n", argv.len()).as_bytes())
        .await
        .map_err(|e| (DB_ERR_REDIS_CMD, e.to_string().into_bytes()))?;
    for arg in argv {
        io.write_all(format!("${}\r\n", arg.len()).as_bytes())
            .await
            .map_err(|e| (DB_ERR_REDIS_CMD, e.to_string().into_bytes()))?;
        io.write_all(arg)
            .await
            .map_err(|e| (DB_ERR_REDIS_CMD, e.to_string().into_bytes()))?;
        io.write_all(b"\r\n")
            .await
            .map_err(|e| (DB_ERR_REDIS_CMD, e.to_string().into_bytes()))?;
    }
    io.flush()
        .await
        .map_err(|e| (DB_ERR_REDIS_CMD, e.to_string().into_bytes()))?;
    Ok(())
}

async fn cmd_simple(
    conn: &mut RedisConn,
    argv: &[&[u8]],
    depth: usize,
) -> Result<Resp3, (u32, Vec<u8>)> {
    write_argv(&mut conn.io, argv).await?;
    read_resp3(&mut conn.io, depth).await
}

#[no_mangle]
pub extern "C" fn x07_ext_db_redis_open_v1(
    req: dbcore::ev_bytes,
    caps: dbcore::ev_bytes,
) -> dbcore::ev_bytes {
    let req = unsafe { bytes_as_slice(req) };
    let caps_raw = unsafe { bytes_as_slice(caps) };

    let pol = policy();
    if !pol.enabled || !pol.redis_enabled {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let caps = match parse_db_caps_v1(caps_raw) {
        Ok(c) => c,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &[])),
    };

    let open = match parse_evro_open_req(req) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &[])),
    };
    if open.flags != 0 {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_BAD_REQ, &[]));
    }

    let connect_timeout_ms = effective_connect_timeout_ms(pol.max_connect_timeout_ms, caps);
    let connect_code = if pol.sandboxed && pol.require_tls {
        DB_ERR_REDIS_TLS
    } else {
        DB_ERR_REDIS_CONNECT
    };

    let conn = match runtime().block_on(async move {
        let fut = async {
            let stream: DynStream = match open.addr {
                RedisAddr::Tcp { host, port } => {
                    let host_s =
                        std::str::from_utf8(host).map_err(|_| (DB_ERR_BAD_REQ, Vec::new()))?;
                    if !redis_host_port_allowed(pol, host_s, port) {
                        return Err((DB_ERR_POLICY_DENIED, Vec::new()));
                    }
                    let tcp = TcpStream::connect((host_s, port))
                        .await
                        .map_err(|e| (connect_code, e.to_string().into_bytes()))?;
                    if pol.sandboxed && pol.require_tls {
                        let cfg = if pol.require_verify {
                            tls_config_webpki_roots()
                        } else {
                            tls_config_no_verify()
                        };
                        let connector = TlsConnector::from(Arc::new(cfg));
                        let server_name = ServerName::try_from(host_s)
                            .map_err(|_| (DB_ERR_BAD_REQ, Vec::new()))?;
                        let tls = connector
                            .connect(server_name, tcp)
                            .await
                            .map_err(|e| (connect_code, e.to_string().into_bytes()))?;
                        Box::pin(tls)
                    } else {
                        Box::pin(tcp)
                    }
                }
                #[cfg(unix)]
                RedisAddr::Unix { path } => {
                    if pol.sandboxed {
                        return Err((DB_ERR_POLICY_DENIED, Vec::new()));
                    }
                    let p = bytes_to_utf8_path(path).map_err(|code| (code, Vec::new()))?;
                    let unix = UnixStream::connect(p)
                        .await
                        .map_err(|e| (DB_ERR_REDIS_CONNECT, e.to_string().into_bytes()))?;
                    Box::pin(unix)
                }
                #[cfg(not(unix))]
                RedisAddr::Unix { path } => {
                    if pol.sandboxed {
                        return Err((DB_ERR_POLICY_DENIED, Vec::new()));
                    }
                    let _ = bytes_to_utf8_path(path).map_err(|code| (code, Vec::new()))?;
                    Err((
                        DB_ERR_REDIS_CONNECT,
                        b"unix sockets are not supported on this platform".to_vec(),
                    ))?
                }
            };

            let mut conn = RedisConn {
                io: BufStream::with_capacity(8 * 1024, 8 * 1024, stream),
            };

            let hello = cmd_simple(&mut conn, &[b"HELLO", b"3"], 64).await?;
            if let Resp3::Error(msg) = hello {
                return Err((DB_ERR_REDIS_SERVER, msg));
            }

            if !open.user.is_empty() || !open.pass.is_empty() {
                let auth = if open.user.is_empty() {
                    cmd_simple(&mut conn, &[b"AUTH", open.pass], 64).await?
                } else {
                    cmd_simple(&mut conn, &[b"AUTH", open.user, open.pass], 64).await?
                };
                if let Resp3::Error(msg) = auth {
                    return Err((DB_ERR_REDIS_SERVER, msg));
                }
            }

            if open.db != 0 {
                let mut buf = itoa::Buffer::new();
                let db_s = buf.format(open.db);
                let sel = cmd_simple(&mut conn, &[b"SELECT", db_s.as_bytes()], 64).await?;
                if let Resp3::Error(msg) = sel {
                    return Err((DB_ERR_REDIS_SERVER, msg));
                }
            }

            Ok::<RedisConn, (u32, Vec<u8>)>(conn)
        };

        if connect_timeout_ms != 0 {
            tokio::time::timeout(Duration::from_millis(connect_timeout_ms as u64), fut)
                .await
                .map_err(|_| (connect_code, b"timeout".to_vec()))?
        } else {
            fut.await
        }
    }) {
        Ok(v) => v,
        Err((code, msg)) => return alloc_return_bytes(&evdb_err(OP_OPEN_V1, code, &msg)),
    };

    let Some(conn_id) = open_slot(conn, pol) else {
        return alloc_return_bytes(&evdb_err(OP_OPEN_V1, DB_ERR_TOO_LARGE, &[]));
    };
    alloc_return_bytes(&evdb_ok(OP_OPEN_V1, &conn_id.to_le_bytes()))
}

#[no_mangle]
pub extern "C" fn x07_ext_db_redis_close_v1(
    req: dbcore::ev_bytes,
    _caps: dbcore::ev_bytes,
) -> dbcore::ev_bytes {
    let req = unsafe { bytes_as_slice(req) };

    let pol = policy();
    if !pol.enabled || !pol.redis_enabled {
        return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, DB_ERR_POLICY_DENIED, &[]));
    }

    let conn_id = match parse_evrx_close_req(req) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, code, &[])),
    };

    if take_conn(conn_id).is_none() {
        return alloc_return_bytes(&evdb_err(OP_CLOSE_V1, DB_ERR_BAD_CONN, &[]));
    }

    alloc_return_bytes(&evdb_ok(OP_CLOSE_V1, &[]))
}

#[no_mangle]
pub extern "C" fn x07_ext_db_redis_cmd_v1(
    req: dbcore::ev_bytes,
    caps: dbcore::ev_bytes,
) -> dbcore::ev_bytes {
    let req = unsafe { bytes_as_slice(req) };
    let caps_raw = unsafe { bytes_as_slice(caps) };

    let pol = policy();
    if !pol.enabled || !pol.redis_enabled {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_POLICY_DENIED, &[]));
    }
    if let Err(out) = count_query_or_deny(pol, OP_QUERY_V1) {
        return out;
    }

    let caps = match parse_db_caps_v1(caps_raw) {
        Ok(c) => c,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };

    let (conn_id, _flags, argv_bytes) = match parse_evrq_cmd_req(req) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };

    if argv_bytes.len() > pol.max_req_bytes as usize {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_TOO_LARGE, &[]));
    }

    let argv = match parse_evrv_argv(argv_bytes) {
        Ok(v) => v,
        Err(code) => return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &[])),
    };

    let Some(conn) = get_conn(conn_id) else {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_BAD_CONN, &[]));
    };

    let timeout_ms = effective_query_timeout_ms(pol.max_query_timeout_ms, caps);

    let doc = match runtime().block_on(async move {
        let fut = async {
            let mut conn = conn.lock().await;
            let resp = cmd_simple(&mut conn, &argv, 64).await?;
            if let Resp3::Error(msg) = resp {
                return Err((DB_ERR_REDIS_SERVER, msg));
            }
            let value = resp_to_dm_value(resp).map_err(|code| (code, Vec::new()))?;
            Ok::<Vec<u8>, (u32, Vec<u8>)>(dm_doc_ok(&value))
        };

        if timeout_ms != 0 {
            tokio::time::timeout(Duration::from_millis(timeout_ms as u64), fut)
                .await
                .map_err(|_| (DB_ERR_REDIS_CMD, b"timeout".to_vec()))?
        } else {
            fut.await
        }
    }) {
        Ok(v) => v,
        Err((code, msg)) => {
            if msg.as_slice() == b"timeout" {
                dbcore::evict_conn_slot(conns(), conn_id);
            }
            return alloc_return_bytes(&evdb_err(OP_QUERY_V1, code, &msg));
        }
    };

    let max_resp = effective_max(pol.max_resp_bytes, caps.max_resp_bytes);
    if max_resp != 0 && doc.len() > max_resp as usize {
        return alloc_return_bytes(&evdb_err(OP_QUERY_V1, DB_ERR_TOO_LARGE, &[]));
    }

    alloc_return_bytes(&evdb_ok(OP_QUERY_V1, &doc))
}
