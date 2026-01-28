use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
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

pub(crate) fn resolve_sibling_or_path(name: &str) -> PathBuf {
    let Ok(exe) = std::env::current_exe() else {
        return PathBuf::from(name);
    };
    let Some(dir) = exe.parent() else {
        return PathBuf::from(name);
    };

    let mut candidates = Vec::new();

    candidates.push(dir.join(name));

    if dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "deps")
    {
        if let Some(parent) = dir.parent() {
            candidates.push(parent.join(name));
        }
    }

    for cand in candidates {
        if cand.is_file() {
            return cand;
        }
    }

    PathBuf::from(name)
}

fn nybble_to_hex(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => '0',
    }
}
