use once_cell::sync::OnceCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// -------------------------
// Error code space (OS/FS v1)
// -------------------------

pub const FS_ERR_POLICY_DENY: i32 = 60001;
pub const FS_ERR_DISABLED: i32 = 60002;
pub const FS_ERR_BAD_PATH: i32 = 60003;
pub const FS_ERR_BAD_CAPS: i32 = 60004;
pub const FS_ERR_BAD_HANDLE: i32 = 60005;

pub const FS_ERR_NOT_FOUND: i32 = 60010;
pub const FS_ERR_ALREADY_EXISTS: i32 = 60011;
pub const FS_ERR_NOT_DIR: i32 = 60012;
pub const FS_ERR_IS_DIR: i32 = 60013;
pub const FS_ERR_PERMISSION: i32 = 60014;
pub const FS_ERR_IO: i32 = 60015;
pub const FS_ERR_TOO_LARGE: i32 = 60016;
pub const FS_ERR_TOO_MANY_ENTRIES: i32 = 60017;
pub const FS_ERR_DEPTH_EXCEEDED: i32 = 60018;
pub const FS_ERR_SYMLINK_DENIED: i32 = 60019;
pub const FS_ERR_UNSUPPORTED: i32 = 60020;

// -------------------------
// Caps decoding (FsCapsV1)
// -------------------------

#[derive(Clone, Copy, Debug)]
pub struct CapsV1 {
    pub max_read_bytes: u32,
    pub max_write_bytes: u32,
    pub max_entries: u32,
    pub max_depth: u32,
    pub flags: u32,
}

pub const CAP_ALLOW_SYMLINKS: u32 = 1 << 0;
pub const CAP_ALLOW_HIDDEN: u32 = 1 << 1;
pub const CAP_CREATE_PARENTS: u32 = 1 << 2;
pub const CAP_OVERWRITE: u32 = 1 << 3;
pub const CAP_ATOMIC_WRITE: u32 = 1 << 4;

pub fn cap_allow_symlinks(c: CapsV1) -> bool {
    (c.flags & CAP_ALLOW_SYMLINKS) != 0
}

pub fn cap_allow_hidden(c: CapsV1) -> bool {
    (c.flags & CAP_ALLOW_HIDDEN) != 0
}

pub fn cap_create_parents(c: CapsV1) -> bool {
    (c.flags & CAP_CREATE_PARENTS) != 0
}

pub fn cap_overwrite(c: CapsV1) -> bool {
    (c.flags & CAP_OVERWRITE) != 0
}

pub fn cap_atomic_write(c: CapsV1) -> bool {
    (c.flags & CAP_ATOMIC_WRITE) != 0
}

pub fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    let slice = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

pub fn parse_caps_v1(caps: &[u8]) -> Result<CapsV1, i32> {
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

pub fn effective_max(policy_max: u32, caps_max: u32) -> u32 {
    if caps_max == 0 {
        policy_max
    } else {
        policy_max.min(caps_max)
    }
}

// -------------------------
// Policy env plumbing (runner)
// -------------------------

#[derive(Clone, Debug)]
pub struct Policy {
    pub sandboxed: bool,
    pub enabled: bool,
    pub deny_hidden: bool,

    pub read_roots: Vec<PathBuf>,
    pub write_roots: Vec<PathBuf>,

    pub allow_symlinks: bool,
    pub allow_mkdir: bool,
    pub allow_remove: bool,
    pub allow_rename: bool,
    pub allow_walk: bool,
    pub allow_glob: bool,

    pub max_read_bytes: u32,
    pub max_write_bytes: u32,
    pub max_entries: u32,
    pub max_depth: u32,
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

pub fn canonicalize_best_effort(p: &Path) -> PathBuf {
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

pub fn policy() -> &'static Policy {
    POLICY.get_or_init(load_policy)
}

// -------------------------
// Path parsing & enforcement
// -------------------------

pub fn bytes_to_utf8(b: &[u8]) -> Result<&str, i32> {
    std::str::from_utf8(b).map_err(|_| FS_ERR_BAD_PATH)
}

pub fn parse_safe_path_v1(input: &[u8]) -> Result<(PathBuf, bool), i32> {
    let s = bytes_to_utf8(input)?;
    if s.is_empty() {
        return Err(FS_ERR_BAD_PATH);
    }
    if s.as_bytes().iter().any(|&b| b == 0 || b == b'\\') {
        return Err(FS_ERR_BAD_PATH);
    }

    let abs = s.as_bytes()[0] == b'/';
    let body = if abs { &s[1..] } else { s };
    if body.is_empty() {
        return Err(FS_ERR_BAD_PATH);
    }

    let mut segs: Vec<&str> = Vec::new();
    let mut hidden = false;
    for raw in body.split('/') {
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
    let mut pb = if abs {
        PathBuf::from("/")
    } else {
        PathBuf::new()
    };
    for seg in segs {
        pb.push(seg);
    }
    Ok((pb, hidden))
}

pub fn canonicalize_existing_prefix(path: &Path) -> PathBuf {
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

pub fn enforce_read_path(caps: CapsV1, path_bytes: &[u8]) -> Result<PathBuf, i32> {
    let pol = policy();
    if !pol.enabled {
        return Err(FS_ERR_DISABLED);
    }

    let (path, hidden) = parse_safe_path_v1(path_bytes)?;
    if pol.deny_hidden && hidden && !cap_allow_hidden(caps) {
        return Err(FS_ERR_POLICY_DENY);
    }

    if !pol.sandboxed {
        return Ok(path);
    }
    if pol.read_roots.is_empty() {
        return Err(FS_ERR_POLICY_DENY);
    }

    let abs = canonicalize_existing_prefix(&canonicalize_best_effort(&path));
    if !is_allowed_by_roots(&abs, &pol.read_roots) {
        return Err(FS_ERR_POLICY_DENY);
    }
    Ok(abs)
}

pub fn enforce_write_path(caps: CapsV1, path_bytes: &[u8]) -> Result<PathBuf, i32> {
    let pol = policy();
    if !pol.enabled {
        return Err(FS_ERR_DISABLED);
    }

    let (path, hidden) = parse_safe_path_v1(path_bytes)?;
    if pol.deny_hidden && hidden && !cap_allow_hidden(caps) {
        return Err(FS_ERR_POLICY_DENY);
    }

    if !pol.sandboxed {
        return Ok(path);
    }
    if pol.write_roots.is_empty() {
        return Err(FS_ERR_POLICY_DENY);
    }

    let abs = canonicalize_existing_prefix(&canonicalize_best_effort(&path));
    if !is_allowed_by_roots(&abs, &pol.write_roots) {
        return Err(FS_ERR_POLICY_DENY);
    }
    Ok(abs)
}

// -------------------------
// IO helpers
// -------------------------

pub fn map_io_err(e: &io::Error) -> i32 {
    match e.kind() {
        io::ErrorKind::NotFound => FS_ERR_NOT_FOUND,
        io::ErrorKind::AlreadyExists => FS_ERR_ALREADY_EXISTS,
        io::ErrorKind::PermissionDenied => FS_ERR_PERMISSION,
        io::ErrorKind::Unsupported => FS_ERR_UNSUPPORTED,
        _ => FS_ERR_IO,
    }
}

pub fn open_atomic_tmp_best_effort(
    path: &Path,
    overwrite: bool,
) -> Result<(fs::File, PathBuf), i32> {
    let Some(parent) = path.parent() else {
        return Err(FS_ERR_BAD_PATH);
    };
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return Err(FS_ERR_BAD_PATH);
    };

    if !overwrite && path.exists() {
        return Err(FS_ERR_ALREADY_EXISTS);
    }

    match fs::metadata(path) {
        Ok(m) if m.is_dir() => return Err(FS_ERR_IS_DIR),
        Ok(_) if !overwrite => return Err(FS_ERR_ALREADY_EXISTS),
        Ok(_) => {}
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                return Err(map_io_err(&e));
            }
        }
    }

    let mut counter: u32 = 0;
    loop {
        let candidate = parent.join(format!("{name}.x07_tmp_{counter}"));
        match fs::OpenOptions::new()
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
