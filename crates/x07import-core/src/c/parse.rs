use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;

pub fn parse_translation_unit(src_path: &Path) -> Result<Value> {
    let mut cmd = Command::new("clang");
    cmd.arg("-Xclang")
        .arg("-ast-dump=json")
        .arg("-fsyntax-only")
        .arg("-fno-color-diagnostics")
        .arg("-std=c11")
        .arg(src_path);

    let out = cmd
        .output()
        .with_context(|| format!("run clang: {}", src_path.display()))?;
    if !out.status.success() {
        anyhow::bail!(
            "clang parse failed for {}:\n{}",
            src_path.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let v: Value = serde_json::from_slice(&out.stdout)
        .with_context(|| format!("parse clang AST JSON: {}", src_path.display()))?;
    Ok(v)
}
