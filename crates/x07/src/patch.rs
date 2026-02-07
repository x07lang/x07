use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07_contracts::X07_PATCHSET_SCHEMA_VERSION;
use x07c::diagnostics;

use crate::report_common::canonical_pretty_json_bytes;
use crate::util;

const X07_PATCHSET_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07.patchset.schema.json");

#[derive(Debug, Clone, Args)]
pub struct PatchArgs {
    #[command(subcommand)]
    pub cmd: Option<PatchCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PatchCommand {
    /// Apply a multi-file JSON patchset.
    Apply(PatchApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub struct PatchApplyArgs {
    /// Patchset JSON file path.
    #[arg(long = "in", value_name = "PATH")]
    pub input: PathBuf,

    /// Repository root used to resolve relative target paths.
    #[arg(long, value_name = "DIR", default_value = ".")]
    pub repo_root: PathBuf,

    /// Write patched files. Without this flag, validates and reports only.
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct PatchTarget {
    path: String,
    patch: Vec<diagnostics::PatchOp>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PatchSet {
    schema_version: String,
    patches: Vec<PatchTarget>,
}

#[derive(Debug, Clone, Serialize)]
struct PatchApplyReport {
    schema_version: &'static str,
    command: &'static str,
    ok: bool,
    exit_code: u8,
    diagnostics: Vec<diagnostics::Diagnostic>,
    result: PatchApplyResult,
}

#[derive(Debug, Clone, Serialize)]
struct PatchApplyResult {
    patchset_path: String,
    repo_root: String,
    write: bool,
    changed_paths: Vec<String>,
    created_paths: Vec<String>,
}

pub fn cmd_patch(
    _machine: &crate::reporting::MachineArgs,
    args: PatchArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        bail!("missing subcommand (try: x07 patch apply --in <patchset.json> --write)");
    };
    match cmd {
        PatchCommand::Apply(args) => cmd_patch_apply(args),
    }
}

fn cmd_patch_apply(args: PatchApplyArgs) -> Result<std::process::ExitCode> {
    let repo_root = util::resolve_existing_path_upwards(&args.repo_root);
    let repo_root_abs = std::fs::canonicalize(&repo_root)
        .with_context(|| format!("canonicalize repo root: {}", repo_root.display()))?;
    let patchset_path = util::resolve_existing_path_upwards(&args.input);

    let bytes = std::fs::read(&patchset_path)
        .with_context(|| format!("read patchset: {}", patchset_path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse patchset JSON: {}", patchset_path.display()))?;

    let mut diagnostics = crate::report_common::validate_schema(
        X07_PATCHSET_SCHEMA_BYTES,
        "spec/x07.patchset.schema.json",
        &doc,
    )?;
    if !diagnostics.is_empty() {
        return emit_report(PatchApplyReport {
            schema_version: X07_PATCHSET_SCHEMA_VERSION,
            command: "patch.apply",
            ok: false,
            exit_code: 1,
            diagnostics,
            result: PatchApplyResult {
                patchset_path: patchset_path.display().to_string(),
                repo_root: repo_root_abs.display().to_string(),
                write: args.write,
                changed_paths: Vec::new(),
                created_paths: Vec::new(),
            },
        });
    }

    let patchset: PatchSet = serde_json::from_value(doc)
        .with_context(|| format!("decode patchset JSON: {}", patchset_path.display()))?;
    if patchset.schema_version.trim() != X07_PATCHSET_SCHEMA_VERSION {
        diagnostics.push(diag_error(
            "X07-PATCHSET-0001",
            diagnostics::Stage::Parse,
            &format!(
                "patchset schema_version mismatch: expected {} got {:?}",
                X07_PATCHSET_SCHEMA_VERSION, patchset.schema_version
            ),
        ));
        return emit_report(PatchApplyReport {
            schema_version: X07_PATCHSET_SCHEMA_VERSION,
            command: "patch.apply",
            ok: false,
            exit_code: 1,
            diagnostics,
            result: PatchApplyResult {
                patchset_path: patchset_path.display().to_string(),
                repo_root: repo_root_abs.display().to_string(),
                write: args.write,
                changed_paths: Vec::new(),
                created_paths: Vec::new(),
            },
        });
    }

    let mut changed_paths = Vec::new();
    let mut created_paths = Vec::new();

    for target in patchset.patches {
        let _ = target.note.as_deref();
        let target_path = resolve_path_under_root(&repo_root_abs, Path::new(&target.path))?;
        let created = !target_path.exists();
        let mut doc = if created {
            Value::Object(serde_json::Map::new())
        } else {
            let target_bytes = std::fs::read(&target_path)
                .with_context(|| format!("read: {}", target_path.display()))?;
            serde_json::from_slice(&target_bytes)
                .with_context(|| format!("parse JSON: {}", target_path.display()))?
        };

        x07c::json_patch::apply_patch(&mut doc, &target.patch)
            .with_context(|| format!("apply patch: {}", target_path.display()))?;

        if args.write {
            let out_bytes = canonicalized_output_bytes(&target_path, &doc)?;
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create dir: {}", parent.display()))?;
            }
            util::write_atomic(&target_path, &out_bytes)
                .with_context(|| format!("write patched: {}", target_path.display()))?;
        }

        changed_paths.push(target.path);
        if created {
            created_paths.push(target_path.display().to_string());
        }
    }

    changed_paths.sort();
    changed_paths.dedup();
    created_paths.sort();
    created_paths.dedup();

    emit_report(PatchApplyReport {
        schema_version: X07_PATCHSET_SCHEMA_VERSION,
        command: "patch.apply",
        ok: true,
        exit_code: 0,
        diagnostics: Vec::new(),
        result: PatchApplyResult {
            patchset_path: patchset_path.display().to_string(),
            repo_root: repo_root_abs.display().to_string(),
            write: args.write,
            changed_paths,
            created_paths,
        },
    })
}

fn canonicalized_output_bytes(path: &Path, doc: &Value) -> Result<Vec<u8>> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.ends_with(".x07.json"))
    {
        let mut file = x07c::x07ast::parse_x07ast_json(&serde_json::to_vec(doc)?)
            .map_err(|e| anyhow::anyhow!("x07ast parse after patch: {e}"))?;
        x07c::x07ast::canonicalize_x07ast_file(&mut file);
        let mut out = x07c::x07ast::x07ast_file_to_value(&file);
        x07c::x07ast::canon_value_jcs(&mut out);
        let mut bytes = serde_json::to_vec_pretty(&out)?;
        if bytes.last() != Some(&b'\n') {
            bytes.push(b'\n');
        }
        return Ok(bytes);
    }
    canonical_pretty_json_bytes(doc)
}

fn resolve_path_under_root(repo_root: &Path, raw: &Path) -> Result<PathBuf> {
    let joined = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        repo_root.join(raw)
    };
    let normalized = normalize_path(&joined);
    if !normalized.starts_with(repo_root) {
        bail!(
            "path escapes repo root: {} (root={})",
            raw.display(),
            repo_root.display()
        );
    }
    Ok(normalized)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(seg) => out.push(seg),
        }
    }
    out
}

fn emit_report(report: PatchApplyReport) -> Result<std::process::ExitCode> {
    if report.ok {
        for path in &report.result.changed_paths {
            println!("{path}");
        }
    } else {
        for d in &report.diagnostics {
            eprintln!("{}: {}", d.code, d.message);
        }
    }
    Ok(std::process::ExitCode::from(report.exit_code))
}

fn diag_error(code: &str, stage: diagnostics::Stage, message: &str) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Error,
        stage,
        message: message.to_string(),
        loc: None,
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    }
}
