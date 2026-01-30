#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use globset::{Glob, GlobMatcher};
use once_cell::sync::OnceCell;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

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

const EV_TRAP_FS_INTERNAL: i32 = 9300;

// -------------------------
// Error code space (FS v1)
// -------------------------

const FS_ERR_POLICY_DENY: i32 = 60001;
const FS_ERR_DISABLED: i32 = 60002;
const FS_ERR_BAD_PATH: i32 = 60003;
const FS_ERR_BAD_CAPS: i32 = 60004;
const FS_ERR_BAD_HANDLE: i32 = 60005;

const FS_ERR_NOT_FOUND: i32 = 60010;
const FS_ERR_ALREADY_EXISTS: i32 = 60011;
const FS_ERR_NOT_DIR: i32 = 60012;
const FS_ERR_IS_DIR: i32 = 60013;
const FS_ERR_PERMISSION: i32 = 60014;
const FS_ERR_IO: i32 = 60015;
const FS_ERR_TOO_LARGE: i32 = 60016;
const FS_ERR_TOO_MANY_ENTRIES: i32 = 60017;
const FS_ERR_DEPTH_EXCEEDED: i32 = 60018;
const FS_ERR_SYMLINK_DENIED: i32 = 60019;
const FS_ERR_UNSUPPORTED: i32 = 60020;

// -------------------------
// Caps decoding (FsCapsV1)
// -------------------------

#[derive(Clone, Copy, Debug)]
struct CapsV1 {
    max_read_bytes: u32,
    max_write_bytes: u32,
    max_entries: u32,
    max_depth: u32,
    flags: u32,
}

const CAP_ALLOW_SYMLINKS: u32 = 1 << 0;
const CAP_ALLOW_HIDDEN: u32 = 1 << 1;
const CAP_CREATE_PARENTS: u32 = 1 << 2;
const CAP_OVERWRITE: u32 = 1 << 3;
const CAP_ATOMIC_WRITE: u32 = 1 << 4;

fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    let slice = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn parse_caps_v1(caps: &[u8]) -> Result<CapsV1, i32> {
    if caps.len() != 24 {
        return Err(FS_ERR_BAD_CAPS);
    }
    let version = read_u32_le(caps, 0).ok_or(FS_ERR_BAD_CAPS)?;
    if version != 1 {
        return Err(FS_ERR_BAD_CAPS);
    }
    Ok(CapsV1 {
        max_read_bytes: read_u32_le(caps, 4).ok_or(FS_ERR_BAD_CAPS)?,
        max_write_bytes: read_u32_le(caps, 8).ok_or(FS_ERR_BAD_CAPS)?,
        max_entries: read_u32_le(caps, 12).ok_or(FS_ERR_BAD_CAPS)?,
        max_depth: read_u32_le(caps, 16).ok_or(FS_ERR_BAD_CAPS)?,
        flags: read_u32_le(caps, 20).ok_or(FS_ERR_BAD_CAPS)?,
    })
}

fn cap_allow_symlinks(c: CapsV1) -> bool {
    (c.flags & CAP_ALLOW_SYMLINKS) != 0
}
fn cap_allow_hidden(c: CapsV1) -> bool {
    (c.flags & CAP_ALLOW_HIDDEN) != 0
}
fn cap_create_parents(c: CapsV1) -> bool {
    (c.flags & CAP_CREATE_PARENTS) != 0
}
fn cap_overwrite(c: CapsV1) -> bool {
    (c.flags & CAP_OVERWRITE) != 0
}
fn cap_atomic_write(c: CapsV1) -> bool {
    (c.flags & CAP_ATOMIC_WRITE) != 0
}

fn effective_max(policy_max: u32, caps_max: u32) -> u32 {
    if caps_max == 0 {
        policy_max
    } else {
        policy_max.min(caps_max)
    }
}

// -------------------------
// Streaming write handles (FS v1)
// -------------------------

#[derive(Debug)]
struct WriterHandleV1 {
    file: Option<std::fs::File>,
    final_path: PathBuf,
    tmp_path: Option<PathBuf>,
    max_write_bytes: u32,
    written: u32,
}

static WRITERS: OnceCell<Mutex<Vec<Option<WriterHandleV1>>>> = OnceCell::new();

fn writers() -> &'static Mutex<Vec<Option<WriterHandleV1>>> {
    WRITERS.get_or_init(|| Mutex::new(Vec::new()))
}

fn writer_idx(handle: i32) -> Option<usize> {
    if handle <= 0 {
        None
    } else {
        Some((handle as usize).saturating_sub(1))
    }
}

fn writer_insert(table: &mut Vec<Option<WriterHandleV1>>, w: WriterHandleV1) -> Result<i32, i32> {
    for (idx, slot) in table.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(w);
            let h = idx + 1;
            if h > (i32::MAX as usize) {
                *slot = None;
                return Err(FS_ERR_UNSUPPORTED);
            }
            return Ok(h as i32);
        }
    }
    table.push(Some(w));
    let h = table.len();
    if h > (i32::MAX as usize) {
        table.pop();
        return Err(FS_ERR_UNSUPPORTED);
    }
    Ok(h as i32)
}

fn open_atomic_tmp_best_effort(
    path: &Path,
    overwrite: bool,
) -> Result<(std::fs::File, PathBuf), i32> {
    let Some(parent) = path.parent() else {
        return Err(FS_ERR_BAD_PATH);
    };
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return Err(FS_ERR_BAD_PATH);
    };

    if path.exists() {
        match std::fs::metadata(path) {
            Ok(m) if m.is_dir() => return Err(FS_ERR_IS_DIR),
            Ok(_) if !overwrite => return Err(FS_ERR_ALREADY_EXISTS),
            Ok(_) => {}
            Err(e) => return Err(map_io_err(&e)),
        }
    }

    let mut counter: u32 = 0;
    loop {
        let candidate = parent.join(format!("{name}.x07_tmp_{counter}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(f) => return Ok((f, candidate)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                counter = counter.wrapping_add(1);
                continue;
            }
            Err(e) => return Err(map_io_err(&e)),
        }
    }
}

// -------------------------
// Policy env plumbing (runner)
// -------------------------

#[derive(Clone, Debug)]
struct Policy {
    sandboxed: bool,
    enabled: bool,
    deny_hidden: bool,

    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,

    allow_symlinks: bool,
    allow_mkdir: bool,
    allow_remove: bool,
    allow_rename: bool,
    allow_walk: bool,
    allow_glob: bool,

    max_read_bytes: u32,
    max_write_bytes: u32,
    max_entries: u32,
    max_depth: u32,
}

static POLICY: OnceCell<Policy> = OnceCell::new();

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|v| match v.as_str() {
            "1" | "true" | "TRUE" | "yes" | "YES" => Some(true),
            "0" | "false" | "FALSE" | "no" | "NO" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn env_u32_nonzero(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|&v| v != 0)
        .unwrap_or(default)
}

fn canonicalize_best_effort(p: &Path) -> PathBuf {
    if p.is_absolute() {
        return p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    }
    let abs = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(p);
    abs.canonicalize().unwrap_or(abs)
}

fn env_roots(name: &str) -> Vec<PathBuf> {
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
    let enabled = env_bool("X07_OS_FS", !sandboxed);
    let deny_hidden = env_bool("X07_OS_DENY_HIDDEN", sandboxed);

    let read_roots = env_roots("X07_OS_FS_READ_ROOTS");
    let write_roots = env_roots("X07_OS_FS_WRITE_ROOTS");

    Policy {
        sandboxed,
        enabled,
        deny_hidden,
        read_roots,
        write_roots,
        allow_symlinks: env_bool("X07_OS_FS_ALLOW_SYMLINKS", !sandboxed),
        allow_mkdir: env_bool("X07_OS_FS_ALLOW_MKDIR", !sandboxed),
        allow_remove: env_bool("X07_OS_FS_ALLOW_REMOVE", !sandboxed),
        allow_rename: env_bool("X07_OS_FS_ALLOW_RENAME", !sandboxed),
        allow_walk: env_bool("X07_OS_FS_ALLOW_WALK", !sandboxed),
        allow_glob: env_bool("X07_OS_FS_ALLOW_GLOB", !sandboxed),
        max_read_bytes: env_u32_nonzero("X07_OS_FS_MAX_READ_BYTES", 16 * 1024 * 1024),
        max_write_bytes: env_u32_nonzero("X07_OS_FS_MAX_WRITE_BYTES", 16 * 1024 * 1024),
        max_entries: env_u32_nonzero("X07_OS_FS_MAX_ENTRIES", 10_000),
        max_depth: env_u32_nonzero("X07_OS_FS_MAX_DEPTH", 64),
    }
}

fn policy() -> &'static Policy {
    POLICY.get_or_init(load_policy)
}

// -------------------------
// Path parsing & enforcement
// -------------------------

fn bytes_to_utf8(b: &[u8]) -> Result<&str, i32> {
    std::str::from_utf8(b).map_err(|_| FS_ERR_BAD_PATH)
}

fn parse_rel_path_v1(input: &[u8]) -> Result<(PathBuf, bool), i32> {
    let s = bytes_to_utf8(input)?;
    if s.is_empty() {
        return Err(FS_ERR_BAD_PATH);
    }
    if s.as_bytes()[0] == b'/' {
        return Err(FS_ERR_BAD_PATH);
    }
    if s.as_bytes().iter().any(|&b| b == 0 || b == b'\\') {
        return Err(FS_ERR_BAD_PATH);
    }

    let mut segs: Vec<&str> = Vec::new();
    let mut hidden = false;
    for raw in s.split('/') {
        if raw.is_empty() {
            return Err(FS_ERR_BAD_PATH);
        }
        if raw == "." {
            continue;
        }
        if raw == ".." {
            return Err(FS_ERR_BAD_PATH);
        }
        if raw.starts_with('.') {
            hidden = true;
        }
        segs.push(raw);
    }
    if segs.is_empty() {
        return Err(FS_ERR_BAD_PATH);
    }
    let mut pb = PathBuf::new();
    for seg in segs {
        pb.push(seg);
    }
    Ok((pb, hidden))
}

fn canonicalize_existing_prefix(path: &Path) -> PathBuf {
    let mut cur = path.to_path_buf();
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    while !cur.exists() {
        let Some(name) = cur.file_name() else {
            break;
        };
        missing.push(name.to_os_string());
        let Some(parent) = cur.parent() else {
            break;
        };
        cur = parent.to_path_buf();
    }

    let mut base = canonicalize_best_effort(&cur);
    for part in missing.iter().rev() {
        base.push(part);
    }
    base
}

fn is_allowed_by_roots(abs_path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|r| abs_path.starts_with(r))
}

fn enforce_read_path(caps: CapsV1, path_bytes: &[u8]) -> Result<PathBuf, i32> {
    let pol = policy();
    if !pol.enabled {
        return Err(FS_ERR_DISABLED);
    }

    let (rel, hidden) = parse_rel_path_v1(path_bytes)?;
    if pol.deny_hidden && hidden && !cap_allow_hidden(caps) {
        return Err(FS_ERR_POLICY_DENY);
    }

    if !pol.sandboxed {
        return Ok(rel);
    }
    if pol.read_roots.is_empty() {
        return Err(FS_ERR_POLICY_DENY);
    }

    let abs = canonicalize_existing_prefix(&canonicalize_best_effort(&rel));
    if !is_allowed_by_roots(&abs, &pol.read_roots) {
        return Err(FS_ERR_POLICY_DENY);
    }
    Ok(abs)
}

fn enforce_write_path(caps: CapsV1, path_bytes: &[u8]) -> Result<PathBuf, i32> {
    let pol = policy();
    if !pol.enabled {
        return Err(FS_ERR_DISABLED);
    }

    let (rel, hidden) = parse_rel_path_v1(path_bytes)?;
    if pol.deny_hidden && hidden && !cap_allow_hidden(caps) {
        return Err(FS_ERR_POLICY_DENY);
    }

    if !pol.sandboxed {
        return Ok(rel);
    }
    if pol.write_roots.is_empty() {
        return Err(FS_ERR_POLICY_DENY);
    }

    let abs = canonicalize_existing_prefix(&canonicalize_best_effort(&rel));
    if !is_allowed_by_roots(&abs, &pol.write_roots) {
        return Err(FS_ERR_POLICY_DENY);
    }
    Ok(abs)
}

// -------------------------
// Result helpers
// -------------------------

unsafe fn bytes_as_slice<'a>(b: ev_bytes) -> &'a [u8] {
    core::slice::from_raw_parts(b.ptr as *const u8, b.len as usize)
}

fn ok_bytes_vec(v: Vec<u8>) -> ev_result_bytes {
    unsafe {
        let out = alloc_bytes(v.len() as u32);
        if !v.is_empty() {
            core::ptr::copy_nonoverlapping(v.as_ptr(), out.ptr, v.len());
        }
        ev_result_bytes {
            tag: 1,
            payload: ev_result_bytes_payload { ok: out },
        }
    }
}

fn err_bytes(code: i32) -> ev_result_bytes {
    ev_result_bytes {
        tag: 0,
        payload: ev_result_bytes_payload { err: code as u32 },
    }
}

fn ok_i32(v: i32) -> ev_result_i32 {
    ev_result_i32 {
        tag: 1,
        payload: ev_result_i32_payload { ok: v as u32 },
    }
}

fn err_i32(code: i32) -> ev_result_i32 {
    ev_result_i32 {
        tag: 0,
        payload: ev_result_i32_payload { err: code as u32 },
    }
}

unsafe fn alloc_bytes(len: u32) -> ev_bytes {
    let out = ev_bytes_alloc(len);
    if out.len != len {
        ev_trap(EV_TRAP_FS_INTERNAL);
    }
    out
}

fn map_io_err(e: &io::Error) -> i32 {
    match e.kind() {
        io::ErrorKind::NotFound => FS_ERR_NOT_FOUND,
        io::ErrorKind::AlreadyExists => FS_ERR_ALREADY_EXISTS,
        io::ErrorKind::PermissionDenied => FS_ERR_PERMISSION,
        io::ErrorKind::Unsupported => FS_ERR_UNSUPPORTED,
        _ => FS_ERR_IO,
    }
}

fn join_lines_sorted(mut lines: Vec<String>) -> Vec<u8> {
    lines.sort(); // UTF-8 string order
    let mut out = String::new();
    if lines.is_empty() {
        out.push('\n');
        return out.into_bytes();
    }
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    out.into_bytes()
}

fn build_glob_matcher(glob: &str) -> Result<GlobMatcher, i32> {
    Glob::new(glob)
        .map_err(|_| FS_ERR_BAD_PATH)
        .map(|g| g.compile_matcher())
}

// -------------------------
// Exported C ABI functions
// -------------------------

#[no_mangle]
pub extern "C" fn x07_ext_fs_read_all_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_read_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_bytes(code),
        };

        if !policy().allow_symlinks && cap_allow_symlinks(caps) {
            return err_bytes(FS_ERR_SYMLINK_DENIED);
        }

        let md = match std::fs::metadata(&pb) {
            Ok(m) => m,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        if md.is_dir() {
            return err_bytes(FS_ERR_IS_DIR);
        }

        let max = effective_max(policy().max_read_bytes, caps.max_read_bytes);
        if md.len() > (max as u64) {
            return err_bytes(FS_ERR_TOO_LARGE);
        }

        let mut f = match std::fs::File::open(&pb) {
            Ok(f) => f,
            Err(e) => return err_bytes(map_io_err(&e)),
        };

        let mut data: Vec<u8> = Vec::with_capacity(md.len().min(max as u64) as usize);
        let mut buf = [0u8; 8192];
        loop {
            let n = match f.read(&mut buf) {
                Ok(n) => n,
                Err(e) => return err_bytes(map_io_err(&e)),
            };
            if n == 0 {
                break;
            }
            if data.len() + n > (max as usize) {
                return err_bytes(FS_ERR_TOO_LARGE);
            }
            data.extend_from_slice(&buf[..n]);
        }
        ok_bytes_vec(data)
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_write_all_v1(
    path: ev_bytes,
    data: ev_bytes,
    caps: ev_bytes,
) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        if cap_create_parents(caps) && !pol.allow_mkdir {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_atomic_write(caps) && !pol.allow_rename {
            return err_i32(FS_ERR_POLICY_DENY);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        let data_bytes = bytes_as_slice(data);

        let max = effective_max(pol.max_write_bytes, caps.max_write_bytes);
        if data_bytes.len() > (max as usize) {
            return err_i32(FS_ERR_TOO_LARGE);
        }

        if cap_create_parents(caps) {
            if let Some(parent) = pb.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return err_i32(map_io_err(&e));
                }
            }
        }

        if !cap_overwrite(caps) {
            match std::fs::metadata(&pb) {
                Ok(m) => {
                    if m.is_dir() {
                        return err_i32(FS_ERR_IS_DIR);
                    }
                    return err_i32(FS_ERR_ALREADY_EXISTS);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return err_i32(map_io_err(&e)),
            }
        }

        if cap_atomic_write(caps) {
            return write_atomic_best_effort(&pb, data_bytes, cap_overwrite(caps));
        }

        if let Err(e) = std::fs::write(&pb, data_bytes) {
            return err_i32(map_io_err(&e));
        }
        ok_i32(data_bytes.len() as i32)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_open_write_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        if cap_create_parents(caps) && !pol.allow_mkdir {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_atomic_write(caps) && !pol.allow_rename {
            return err_i32(FS_ERR_POLICY_DENY);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        let max_write = effective_max(pol.max_write_bytes, caps.max_write_bytes);

        if cap_create_parents(caps) {
            if let Some(parent) = pb.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return err_i32(map_io_err(&e));
                }
            }
        }

        let overwrite = cap_overwrite(caps);

        if cap_atomic_write(caps) {
            let (f, tmp) = match open_atomic_tmp_best_effort(&pb, overwrite) {
                Ok(v) => v,
                Err(code) => return err_i32(code),
            };

            let handle = match writers().lock() {
                Ok(mut table) => writer_insert(
                    &mut table,
                    WriterHandleV1 {
                        file: Some(f),
                        final_path: pb,
                        tmp_path: Some(tmp),
                        max_write_bytes: max_write,
                        written: 0,
                    },
                ),
                Err(_) => Err(FS_ERR_IO),
            };

            return match handle {
                Ok(h) => ok_i32(h),
                Err(code) => err_i32(code),
            };
        }

        if overwrite {
            match std::fs::metadata(&pb) {
                Ok(m) if m.is_dir() => return err_i32(FS_ERR_IS_DIR),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return err_i32(map_io_err(&e)),
            }
        } else {
            match std::fs::metadata(&pb) {
                Ok(m) => {
                    if m.is_dir() {
                        return err_i32(FS_ERR_IS_DIR);
                    }
                    return err_i32(FS_ERR_ALREADY_EXISTS);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return err_i32(map_io_err(&e)),
            }
        }

        let open = if overwrite {
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&pb)
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&pb)
        };

        let f = match open {
            Ok(f) => f,
            Err(e) => return err_i32(map_io_err(&e)),
        };

        let handle = match writers().lock() {
            Ok(mut table) => writer_insert(
                &mut table,
                WriterHandleV1 {
                    file: Some(f),
                    final_path: pb,
                    tmp_path: None,
                    max_write_bytes: max_write,
                    written: 0,
                },
            ),
            Err(_) => Err(FS_ERR_IO),
        };

        match handle {
            Ok(h) => ok_i32(h),
            Err(code) => err_i32(code),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_write_all_v1(
    writer_handle: i32,
    data: ev_bytes,
) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let Ok(mut table) = writers().lock() else {
            return err_i32(FS_ERR_IO);
        };
        let Some(idx) = writer_idx(writer_handle) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(w) = table.get_mut(idx).and_then(|v| v.as_mut()) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(f) = w.file.as_mut() else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };

        let data_bytes = bytes_as_slice(data);
        let Some(rem) = w.max_write_bytes.checked_sub(w.written) else {
            return err_i32(FS_ERR_TOO_LARGE);
        };
        if data_bytes.len() > (rem as usize) {
            return err_i32(FS_ERR_TOO_LARGE);
        }

        if let Err(e) = f.write_all(data_bytes) {
            return err_i32(map_io_err(&e));
        }
        w.written = w.written.saturating_add(data_bytes.len() as u32);

        ok_i32(data_bytes.len() as i32)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_close_v1(writer_handle: i32) -> ev_result_i32 {
    std::panic::catch_unwind(|| {
        let Ok(mut table) = writers().lock() else {
            return err_i32(FS_ERR_IO);
        };
        let Some(idx) = writer_idx(writer_handle) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };
        let Some(w) = table.get_mut(idx).and_then(|v| v.as_mut()) else {
            return err_i32(FS_ERR_BAD_HANDLE);
        };

        // Idempotent close.
        let Some(f) = w.file.take() else {
            return ok_i32(1);
        };
        drop(f);

        if let Some(tmp) = w.tmp_path.take() {
            if let Err(e) = std::fs::rename(&tmp, &w.final_path) {
                let _ = std::fs::remove_file(&tmp);
                w.tmp_path = Some(tmp);
                return err_i32(map_io_err(&e));
            }
        }

        ok_i32(1)
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stream_drop_v1(writer_handle: i32) -> i32 {
    std::panic::catch_unwind(|| {
        let Ok(mut table) = writers().lock() else {
            return 1;
        };
        let Some(idx) = writer_idx(writer_handle) else {
            return 1;
        };
        let Some(w) = table.get_mut(idx).and_then(|v| v.take()) else {
            return 1;
        };

        drop(w.file);
        if let Some(tmp) = w.tmp_path {
            let _ = std::fs::remove_file(&tmp);
        }

        1
    })
    .unwrap_or(1)
}

fn write_atomic_best_effort(path: &Path, data: &[u8], overwrite: bool) -> ev_result_i32 {
    let Some(parent) = path.parent() else {
        return err_i32(FS_ERR_BAD_PATH);
    };
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return err_i32(FS_ERR_BAD_PATH);
    };

    if !overwrite && path.exists() {
        return err_i32(FS_ERR_ALREADY_EXISTS);
    }

    let mut counter: u32 = 0;
    let tmp_path = loop {
        let candidate = parent.join(format!("{name}.x07_tmp_{counter}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut f) => {
                if let Err(e) = f.write_all(data) {
                    let _ = std::fs::remove_file(&candidate);
                    return err_i32(map_io_err(&e));
                }
                let _ = f.sync_all();
                break candidate;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                counter = counter.wrapping_add(1);
                continue;
            }
            Err(e) => return err_i32(map_io_err(&e)),
        }
    };

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return err_i32(map_io_err(&e));
    }
    ok_i32(data.len() as i32)
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_mkdirs_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if !pol.allow_mkdir {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };
        match std::fs::create_dir_all(&pb) {
            Ok(()) => ok_i32(1),
            Err(e) => err_i32(map_io_err(&e)),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_remove_file_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if !pol.allow_remove {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        match std::fs::metadata(&pb) {
            Ok(m) => {
                if m.is_dir() {
                    return err_i32(FS_ERR_IS_DIR);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => return err_i32(FS_ERR_NOT_FOUND),
            Err(e) => return err_i32(map_io_err(&e)),
        }

        match std::fs::remove_file(&pb) {
            Ok(()) => ok_i32(1),
            Err(e) => err_i32(map_io_err(&e)),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_remove_dir_all_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if !pol.allow_remove {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_write_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        match std::fs::metadata(&pb) {
            Ok(m) => {
                if !m.is_dir() {
                    return err_i32(FS_ERR_NOT_DIR);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => return err_i32(FS_ERR_NOT_FOUND),
            Err(e) => return err_i32(map_io_err(&e)),
        }

        match std::fs::remove_dir_all(&pb) {
            Ok(()) => ok_i32(1),
            Err(e) => err_i32(map_io_err(&e)),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_rename_v1(
    src: ev_bytes,
    dst: ev_bytes,
    caps: ev_bytes,
) -> ev_result_i32 {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_i32(code),
        };

        let pol = policy();
        if !pol.allow_rename {
            return err_i32(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_i32(FS_ERR_SYMLINK_DENIED);
        }

        let src_bytes = bytes_as_slice(src);
        let dst_bytes = bytes_as_slice(dst);
        let src_pb = match enforce_write_path(caps, src_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };
        let dst_pb = match enforce_write_path(caps, dst_bytes) {
            Ok(p) => p,
            Err(code) => return err_i32(code),
        };

        match std::fs::rename(&src_pb, &dst_pb) {
            Ok(()) => ok_i32(1),
            Err(e) => err_i32(map_io_err(&e)),
        }
    })
    .unwrap_or_else(|_| err_i32(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_list_dir_sorted_text_v1(
    path: ev_bytes,
    caps: ev_bytes,
) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        let pol = policy();
        if !pol.allow_walk {
            return err_bytes(FS_ERR_POLICY_DENY);
        }
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_bytes(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_read_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_bytes(code),
        };

        let md = match std::fs::metadata(&pb) {
            Ok(m) => m,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        if !md.is_dir() {
            return err_bytes(FS_ERR_NOT_DIR);
        }

        let max = effective_max(pol.max_entries, caps.max_entries) as usize;
        let mut names: Vec<String> = Vec::new();

        let rd = match std::fs::read_dir(&pb) {
            Ok(r) => r,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        for ent in rd {
            let ent = match ent {
                Ok(e) => e,
                Err(e) => return err_bytes(map_io_err(&e)),
            };
            let Ok(name) = ent.file_name().into_string() else {
                continue;
            };
            if pol.deny_hidden && name.starts_with('.') && !cap_allow_hidden(caps) {
                continue;
            }
            names.push(name);
            if names.len() > max {
                return err_bytes(FS_ERR_TOO_MANY_ENTRIES);
            }
        }

        ok_bytes_vec(join_lines_sorted(names))
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_walk_glob_sorted_text_v1(
    root: ev_bytes,
    glob: ev_bytes,
    caps: ev_bytes,
) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        let pol = policy();
        if !pol.allow_walk || !pol.allow_glob {
            return err_bytes(FS_ERR_POLICY_DENY);
        }

        let root_b = bytes_as_slice(root);
        let root_pb = match enforce_read_path(caps, root_b) {
            Ok(p) => p,
            Err(code) => return err_bytes(code),
        };

        let md = match std::fs::metadata(&root_pb) {
            Ok(m) => m,
            Err(e) => return err_bytes(map_io_err(&e)),
        };
        if !md.is_dir() {
            return err_bytes(FS_ERR_NOT_DIR);
        }

        let glob_b = bytes_as_slice(glob);
        let glob_s = match bytes_to_utf8(glob_b) {
            Ok(s) => s,
            Err(code) => return err_bytes(code),
        };
        let matcher = match build_glob_matcher(glob_s) {
            Ok(m) => m,
            Err(code) => return err_bytes(code),
        };

        let follow_links = cap_allow_symlinks(caps) && pol.allow_symlinks;
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_bytes(FS_ERR_SYMLINK_DENIED);
        }

        let max_entries = effective_max(pol.max_entries, caps.max_entries) as usize;
        let max_depth = effective_max(pol.max_depth, caps.max_depth) as usize;

        let walker = WalkDir::new(&root_pb)
            .follow_links(follow_links)
            .max_depth(max_depth.saturating_add(1));

        let mut out: Vec<String> = Vec::new();

        for ent in walker {
            let ent = match ent {
                Ok(e) => e,
                Err(_) => return err_bytes(FS_ERR_IO),
            };
            if ent.depth() > max_depth {
                return err_bytes(FS_ERR_DEPTH_EXCEEDED);
            }
            if ent.file_type().is_dir() {
                continue;
            }
            let rel = match ent.path().strip_prefix(&root_pb) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let Some(rel_s) = rel.to_str() else {
                continue;
            };
            let rel_s = rel_s.replace('\\', "/");
            if pol.deny_hidden
                && !cap_allow_hidden(caps)
                && rel_s.split('/').any(|s| s.starts_with('.'))
            {
                continue;
            }
            if matcher.is_match(rel_s.as_str()) {
                out.push(rel_s);
                if out.len() > max_entries {
                    return err_bytes(FS_ERR_TOO_MANY_ENTRIES);
                }
            }
        }

        ok_bytes_vec(join_lines_sorted(out))
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[no_mangle]
pub extern "C" fn x07_ext_fs_stat_v1(path: ev_bytes, caps: ev_bytes) -> ev_result_bytes {
    std::panic::catch_unwind(|| unsafe {
        let caps = match parse_caps_v1(bytes_as_slice(caps)) {
            Ok(caps) => caps,
            Err(code) => return err_bytes(code),
        };

        let pol = policy();
        if cap_allow_symlinks(caps) && !pol.allow_symlinks {
            return err_bytes(FS_ERR_SYMLINK_DENIED);
        }

        let path_bytes = bytes_as_slice(path);
        let pb = match enforce_read_path(caps, path_bytes) {
            Ok(p) => p,
            Err(code) => return err_bytes(code),
        };

        let md = match std::fs::symlink_metadata(&pb) {
            Ok(m) => m,
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    let mut stat = vec![0u8; 16];
                    stat[0..4].copy_from_slice(&1u32.to_le_bytes()); // version
                    stat[4..8].copy_from_slice(&0u32.to_le_bytes()); // kind=0 missing
                    return ok_bytes_vec(stat);
                }
                return err_bytes(map_io_err(&e));
            }
        };

        let ft = md.file_type();
        let kind: u32 = if ft.is_file() {
            1
        } else if ft.is_dir() {
            2
        } else if ft.is_symlink() {
            3
        } else {
            4
        };
        let size: u32 = if ft.is_file() {
            md.len().min(u32::MAX as u64) as u32
        } else {
            0
        };
        let mtime_s: u32 = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs().min(u32::MAX as u64) as u32)
            .unwrap_or(0);

        let mut stat = vec![0u8; 16];
        stat[0..4].copy_from_slice(&1u32.to_le_bytes());
        stat[4..8].copy_from_slice(&kind.to_le_bytes());
        stat[8..12].copy_from_slice(&size.to_le_bytes());
        stat[12..16].copy_from_slice(&mtime_s.to_le_bytes());
        ok_bytes_vec(stat)
    })
    .unwrap_or_else(|_| err_bytes(FS_ERR_IO))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[no_mangle]
    extern "C" fn ev_bytes_alloc(len: u32) -> ev_bytes {
        let mut v = vec![0u8; len as usize];
        let ptr = v.as_mut_ptr();
        std::mem::forget(v);
        ev_bytes { ptr, len }
    }

    #[no_mangle]
    extern "C" fn ev_trap(code: i32) -> ! {
        panic!("ev_trap({code})")
    }

    fn caps_v1(max_write_bytes: u32, flags: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(24);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // max_read_bytes
        out.extend_from_slice(&max_write_bytes.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // max_entries
        out.extend_from_slice(&0u32.to_le_bytes()); // max_depth
        out.extend_from_slice(&flags.to_le_bytes());
        out
    }

    fn to_ev_bytes(b: &[u8]) -> ev_bytes {
        ev_bytes {
            ptr: b.as_ptr() as *mut u8,
            len: b.len() as u32,
        }
    }

    fn ok_i32(res: ev_result_i32) -> i32 {
        assert_eq!(res.tag, 1, "expected ok, got err={}", unsafe {
            res.payload.err
        });
        unsafe { res.payload.ok as i32 }
    }

    fn err_i32(res: ev_result_i32) -> i32 {
        assert_eq!(res.tag, 0, "expected err");
        unsafe { res.payload.err as i32 }
    }

    #[test]
    fn fs_stream_writer_handle_v1_smoke() {
        std::env::set_var("X07_OS_SANDBOXED", "0");
        std::env::set_var("X07_OS_FS", "1");
        std::env::set_var("X07_OS_FS_ALLOW_MKDIR", "1");
        std::env::set_var("X07_OS_FS_ALLOW_RENAME", "1");
        std::env::set_var("X07_OS_FS_MAX_WRITE_BYTES", "1000000");

        let root = format!("target/x07_ext_fs_stream_test_{}", std::process::id());
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create test dir");

        // Non-atomic writer, max_write_bytes enforced cumulatively.
        let out_path = format!("{root}/out.txt");
        let caps = caps_v1(8, CAP_CREATE_PARENTS | CAP_OVERWRITE);
        let h = ok_i32(x07_ext_fs_stream_open_write_v1(
            to_ev_bytes(out_path.as_bytes()),
            to_ev_bytes(&caps),
        ));
        assert!(h > 0);
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h, to_ev_bytes(b"abc"))),
            3
        );
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h, to_ev_bytes(b"def"))),
            3
        );
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h, to_ev_bytes(b"gh"))),
            2
        );
        assert_eq!(
            err_i32(x07_ext_fs_stream_write_all_v1(h, to_ev_bytes(b"i"))),
            FS_ERR_TOO_LARGE
        );
        assert_eq!(ok_i32(x07_ext_fs_stream_close_v1(h)), 1);
        assert_eq!(x07_ext_fs_stream_drop_v1(h), 1);

        let got = std::fs::read(&out_path).expect("read out.txt");
        assert_eq!(got, b"abcdefgh");

        // Atomic writer commits on close.
        let atomic_path = format!("{root}/atomic.txt");
        let caps_atomic = caps_v1(1024, CAP_CREATE_PARENTS | CAP_OVERWRITE | CAP_ATOMIC_WRITE);
        let h2 = ok_i32(x07_ext_fs_stream_open_write_v1(
            to_ev_bytes(atomic_path.as_bytes()),
            to_ev_bytes(&caps_atomic),
        ));
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h2, to_ev_bytes(b"hi"))),
            2
        );
        assert_eq!(ok_i32(x07_ext_fs_stream_close_v1(h2)), 1);
        assert_eq!(x07_ext_fs_stream_drop_v1(h2), 1);
        let got2 = std::fs::read(&atomic_path).expect("read atomic.txt");
        assert_eq!(got2, b"hi");

        // Dropping without close should clean up the tmp file and not create the final path.
        let atomic_drop_path = format!("{root}/atomic_drop.txt");
        let h3 = ok_i32(x07_ext_fs_stream_open_write_v1(
            to_ev_bytes(atomic_drop_path.as_bytes()),
            to_ev_bytes(&caps_atomic),
        ));
        assert_eq!(
            ok_i32(x07_ext_fs_stream_write_all_v1(h3, to_ev_bytes(b"x"))),
            1
        );
        assert_eq!(x07_ext_fs_stream_drop_v1(h3), 1);
        assert_eq!(err_i32(x07_ext_fs_stream_close_v1(h3)), FS_ERR_BAD_HANDLE);
        assert!(!Path::new(&atomic_drop_path).exists());
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.starts_with("atomic_drop.txt.x07_tmp_"))
            .collect();
        assert_eq!(leftovers, Vec::<String>::new());

        // Invalid handle errors.
        assert_eq!(
            err_i32(x07_ext_fs_stream_write_all_v1(123, to_ev_bytes(b"z"))),
            FS_ERR_BAD_HANDLE
        );
        assert_eq!(err_i32(x07_ext_fs_stream_close_v1(123)), FS_ERR_BAD_HANDLE);

        let _ = std::fs::remove_dir_all(&root);
    }
}
