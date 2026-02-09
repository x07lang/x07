use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use sha2::{Digest, Sha256};

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

pub(crate) fn safe_artifact_dir_name(id: &str) -> String {
    format!("id_{}", sha256_hex(id.as_bytes()))
}

pub fn resolve_existing_path_upwards(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_existing_path_upwards_from(&cwd, path)
}

pub fn resolve_existing_path_upwards_from(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let mut dir: Option<&Path> = Some(base_dir);
    while let Some(d) = dir {
        let cand = d.join(path);
        if cand.exists() {
            return cand;
        }
        dir = d.parent();
    }
    path.to_path_buf()
}

pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(nybble_to_hex((b >> 4) & 0x0f));
        out.push(nybble_to_hex(b & 0x0f));
    }
    out
}

pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = temp_path_next_to(path);
    std::fs::write(&tmp, contents)?;

    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp, path)?;
            Ok(())
        }
    }
}

pub fn canonical_jcs_bytes(v: &serde_json::Value) -> Result<Vec<u8>> {
    let mut v = v.clone();
    x07c::x07ast::canon_value_jcs(&mut v);
    Ok(serde_json::to_vec(&v)?)
}

pub(crate) fn resolve_sibling_or_path(name: &str) -> PathBuf {
    let Ok(exe) = std::env::current_exe() else {
        return PathBuf::from(name);
    };
    let Some(dir) = exe.parent() else {
        return PathBuf::from(name);
    };

    let sibling = dir.join(name);
    if sibling.is_file() {
        return sibling;
    }
    if dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "deps")
    {
        if let Some(parent) = dir.parent() {
            let sibling = parent.join(name);
            if sibling.is_file() {
                return sibling;
            }
        }
    }

    PathBuf::from(name)
}

fn temp_path_next_to(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{file_name}.{pid}.{n}.tmp"))
}

fn nybble_to_hex(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => '0',
    }
}
