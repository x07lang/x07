use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use sha2::{Digest, Sha256};

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(crate) const ENV_X07_COMPAT: &str = "X07_COMPAT";
pub(crate) const ENV_X07_OFFLINE: &str = "X07_OFFLINE";

pub(crate) fn parse_env_flag(raw: &str) -> bool {
    let v = raw.trim();
    if v.is_empty() {
        return false;
    }
    match v.to_ascii_lowercase().as_str() {
        "0" | "false" | "no" | "off" => false,
        "1" | "true" | "yes" | "on" => true,
        _ => true,
    }
}

pub(crate) fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|v| parse_env_flag(&v))
}

pub(crate) fn x07_offline_enabled() -> bool {
    env_flag_enabled(ENV_X07_OFFLINE)
}

pub(crate) fn resolve_compat(
    cli: Option<&str>,
    project: Option<&str>,
) -> Result<x07c::compat::Compat> {
    let env = std::env::var(ENV_X07_COMPAT).ok();
    x07c::compat::resolve_compat(cli, env.as_deref(), project)
}

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

pub(crate) fn parse_semver_triplet(v: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = v.trim().split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let major: u32 = parts[0].parse().ok()?;
    let minor: u32 = parts[1].parse().ok()?;
    let patch: u32 = parts[2].parse().ok()?;
    Some((major, minor, patch))
}

pub(crate) fn detect_toolchain_root_best_effort(cwd: &Path) -> Option<PathBuf> {
    let cand = resolve_existing_path_upwards_from(cwd, Path::new("stdlib.lock"));
    if cand.is_file() {
        return cand.parent().map(|p| p.to_path_buf());
    }

    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors() {
            if anc.join("stdlib.lock").is_file() {
                return Some(anc.to_path_buf());
            }
        }
    }

    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let toolchains_dir = home.join(".x07").join("toolchains");
    let mut best: Option<((u32, u32, u32), PathBuf)> = None;
    for entry in std::fs::read_dir(&toolchains_dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name()?.to_string_lossy();
        let dir_name = dir_name.strip_prefix('v').unwrap_or(&dir_name);
        let Some(ver) = parse_semver_triplet(dir_name) else {
            continue;
        };
        if !path.join("stdlib.lock").is_file() {
            continue;
        }
        if best.as_ref().map(|(b, _)| ver > *b).unwrap_or(true) {
            best = Some((ver, path));
        }
    }

    best.map(|(_, p)| p)
}

pub(crate) fn semver_dirs_sorted_desc(base: &Path) -> Vec<PathBuf> {
    let mut out: Vec<((u32, u32, u32), PathBuf)> = Vec::new();
    let Ok(entries) = std::fs::read_dir(base) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(v) = parse_semver_triplet(name) else {
            continue;
        };
        out.push((v, path));
    }
    out.sort_by(|(a, _), (b, _)| b.cmp(a));
    out.into_iter().map(|(_, p)| p).collect()
}

pub(crate) fn toolchain_stdlib_module_roots(toolchain_root: &Path) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let stdlib_dir = toolchain_root.join("stdlib");
    for family in ["os", "std-core", "std"] {
        let base = stdlib_dir.join(family);
        if !base.is_dir() {
            continue;
        }
        if let Some(modules) = semver_dirs_sorted_desc(&base)
            .into_iter()
            .map(|ver| ver.join("modules"))
            .find(|modules| modules.is_dir())
        {
            roots.push(modules);
        }
    }
    roots
}

pub(crate) fn should_walk_dir_entry(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if !entry.file_type().is_dir() {
        return true;
    }
    !matches!(
        name.as_ref(),
        ".git" | ".x07" | "target" | ".agent" | ".claude"
    )
}

pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(nybble_to_hex((b >> 4) & 0x0f));
        out.push(nybble_to_hex(b & 0x0f));
    }
    out
}

pub(crate) fn is_safe_rel_path(raw: &str) -> bool {
    let raw = raw.trim();
    if raw.is_empty() || raw.contains('\\') {
        return false;
    }

    let p = Path::new(raw);
    if p.is_absolute() {
        return false;
    }

    for c in p.components() {
        match c {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
            Component::Normal(_) | Component::CurDir => {}
        }
    }

    true
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_flag_handles_common_forms() {
        assert!(!parse_env_flag(""));
        assert!(!parse_env_flag("  "));

        for v in ["1", "true", "TRUE", "yes", "on", "  on  "] {
            assert!(parse_env_flag(v), "expected {v:?} true");
        }
        for v in ["0", "false", "FALSE", "no", "off", "  off  "] {
            assert!(!parse_env_flag(v), "expected {v:?} false");
        }

        assert!(parse_env_flag("anything"));
        assert!(parse_env_flag("2"));
    }
}
