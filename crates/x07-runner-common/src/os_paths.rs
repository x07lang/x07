use std::path::{Path, PathBuf};

use anyhow::Result;

const OS_MODULES_REL: &str = "stdlib/os/0.2.0/modules";

pub fn default_os_module_roots() -> Result<Vec<PathBuf>> {
    default_os_module_roots_from_exe(std::env::current_exe().ok().as_deref())
}

pub fn default_os_module_roots_from_exe(exe: Option<&Path>) -> Result<Vec<PathBuf>> {
    let mut checked: Vec<PathBuf> = Vec::new();

    let rel = PathBuf::from(OS_MODULES_REL);
    checked.push(rel.clone());
    if rel.is_dir() {
        return Ok(vec![rel]);
    }

    if let Some(exe) = exe {
        if let Some(exe_dir) = exe.parent() {
            for base in [Some(exe_dir), exe_dir.parent()] {
                let Some(base) = base else { continue };
                let cand = base.join(OS_MODULES_REL);
                checked.push(cand.clone());
                if cand.is_dir() {
                    return Ok(vec![cand]);
                }
            }
        }
    }

    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(workspace_root) = crate_dir.parent().and_then(|p| p.parent()) {
        let abs = workspace_root.join(OS_MODULES_REL);
        checked.push(abs.clone());
        if abs.is_dir() {
            return Ok(vec![abs]);
        }
    }

    let checked = checked
        .into_iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");

    anyhow::bail!(
        "could not locate stdlib/os module root (expected {OS_MODULES_REL})\n\nlooked for:\n{checked}\n\nfix:\n  - install an official toolchain archive (it must include {OS_MODULES_REL}), or\n  - run the tool from the x07 repo root"
    );
}

pub fn default_os_module_roots_best_effort_from_exe(exe: Option<&Path>) -> Vec<PathBuf> {
    default_os_module_roots_from_exe(exe).unwrap_or_default()
}
