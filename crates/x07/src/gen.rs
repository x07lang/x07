use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use clap::Args;
use serde::Deserialize;
use serde_json::{json, Value};
use walkdir::WalkDir;
use x07c::diagnostics;

use crate::report_common;
use crate::util;

const ARCH_GEN_INDEX_SCHEMA_VERSION: &str = "x07.arch.gen.index@0.1.0";
const ARCH_GEN_INDEX_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-arch.gen.index.schema.json");

static TMP_N: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Args)]
pub struct GenArgs {
    #[command(subcommand)]
    pub cmd: Option<GenCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum GenCommand {
    /// Verify committed generator outputs are up to date (and deterministic).
    Verify(GenVerifyArgs),
    /// Regenerate committed outputs declared in the generator index.
    Write(GenWriteArgs),
}

#[derive(Debug, Args)]
pub struct GenVerifyArgs {
    #[arg(
        long,
        value_name = "PATH",
        default_value = "arch/gen/index.x07gen.json"
    )]
    pub index: PathBuf,
}

#[derive(Debug, Args)]
pub struct GenWriteArgs {
    #[arg(
        long,
        value_name = "PATH",
        default_value = "arch/gen/index.x07gen.json"
    )]
    pub index: PathBuf,
}

pub fn cmd_gen(
    machine: &crate::reporting::MachineArgs,
    args: GenArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing gen subcommand (try --help)");
    };
    match cmd {
        GenCommand::Verify(args) => cmd_gen_verify(machine, args),
        GenCommand::Write(args) => cmd_gen_write(machine, args),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchGenIndex {
    schema_version: String,
    #[serde(default)]
    generators: Vec<ArchGenEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchGenEntry {
    id: String,
    #[serde(default)]
    mode: Option<String>,
    check_argv: Vec<String>,
    write_argv: Vec<String>,
    outputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyMode {
    DoubleRunV1,
    SingleRunV1,
}

impl VerifyMode {
    fn parse(raw: Option<&str>) -> Result<Self> {
        match raw.unwrap_or("double_run_v1").trim() {
            "double_run_v1" => Ok(Self::DoubleRunV1),
            "single_run_v1" => Ok(Self::SingleRunV1),
            other => anyhow::bail!(
                "unsupported generator mode: expected \"double_run_v1\" or \"single_run_v1\" got {other:?}"
            ),
        }
    }
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self> {
        let pid = std::process::id();
        let n = TMP_N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("x07_{prefix}_{pid}_{n}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path)
            .with_context(|| format!("create temp dir: {}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn cmd_gen_verify(
    machine: &crate::reporting::MachineArgs,
    args: GenVerifyArgs,
) -> Result<std::process::ExitCode> {
    let mut diagnostics: Vec<diagnostics::Diagnostic> = Vec::new();

    let index_path = util::resolve_existing_path_upwards(&args.index);
    let index_bytes = match std::fs::read(&index_path) {
        Ok(b) => b,
        Err(err) => {
            diagnostics.push(diag_error(
                "E_GEN_INDEX_IO_READ_FAILED",
                diagnostics::Stage::Parse,
                format!("cannot read index: {}: {err}", index_path.display()),
                None,
            ));
            return write_diag_and_exit(machine, &index_path, None, diagnostics);
        }
    };
    let index_sha256_hex = util::sha256_hex(&index_bytes);
    let index_doc: Value = match serde_json::from_slice(&index_bytes) {
        Ok(v) => v,
        Err(err) => {
            diagnostics.push(diag_error(
                "E_GEN_INDEX_JSON_PARSE",
                diagnostics::Stage::Parse,
                format!("invalid JSON in index: {}: {err}", index_path.display()),
                None,
            ));
            return write_diag_and_exit(machine, &index_path, None, diagnostics);
        }
    };

    let schema_version = index_doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema_version != ARCH_GEN_INDEX_SCHEMA_VERSION {
        diagnostics.push(diag_error(
            "E_GEN_INDEX_SCHEMA_VERSION_UNSUPPORTED",
            diagnostics::Stage::Parse,
            format!(
                "index schema_version mismatch: expected {ARCH_GEN_INDEX_SCHEMA_VERSION:?} got {schema_version:?}"
            ),
            Some(diagnostics::Location::X07Ast {
                ptr: "/schema_version".to_string(),
            }),
        ));
    }

    let schema_diags = report_common::validate_schema(
        ARCH_GEN_INDEX_SCHEMA_BYTES,
        "spec/x07-arch.gen.index.schema.json",
        &index_doc,
    )?;
    for d in schema_diags {
        diagnostics.push(remap_schema_diag(
            "E_GEN_INDEX_SCHEMA_INVALID",
            d,
            Some(index_path.display().to_string()),
        ));
    }

    let index: Option<ArchGenIndex> = match serde_json::from_value(index_doc.clone()) {
        Ok(v) => Some(v),
        Err(err) => {
            diagnostics.push(diag_error(
                "E_GEN_INDEX_SCHEMA_INVALID",
                diagnostics::Stage::Parse,
                format!("index JSON shape is invalid: {err}"),
                None,
            ));
            None
        }
    };

    let project_root = index_path
        .parent()
        .and_then(find_project_root)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let mut generator_results: Vec<Value> = Vec::new();
    if let Some(index) = &index {
        for entry in &index.generators {
            let mode = match VerifyMode::parse(entry.mode.as_deref()) {
                Ok(m) => m,
                Err(err) => {
                    diagnostics.push(diag_error(
                        "E_GEN_ENTRY_MODE_UNSUPPORTED",
                        diagnostics::Stage::Parse,
                        format!("generator {:?}: {err:#}", entry.id),
                        None,
                    ));
                    generator_results.push(json!({"id": entry.id, "status": "error"}));
                    continue;
                }
            };

            let entry_diag_count_before = diagnostics.len();
            validate_entry_argv(entry, &mut diagnostics);
            validate_entry_outputs(entry, &project_root, &mut diagnostics);

            let has_entry_errors = diagnostics[entry_diag_count_before..]
                .iter()
                .any(|d| d.severity == diagnostics::Severity::Error);
            if has_entry_errors {
                generator_results.push(json!({"id": entry.id, "status": "error"}));
                continue;
            }

            let temp_a = TempDirGuard::new("gen_verify_a")?;
            copy_project_tree(&project_root, &temp_a.path)?;
            remove_outputs(&temp_a.path, &entry.outputs);
            let run_a = run_self(&temp_a.path, &entry.write_argv)?;
            if run_a.exit_code != 0 {
                diagnostics.push(diag_error(
                    "E_GEN_RUN_FAILED",
                    diagnostics::Stage::Run,
                    format!(
                        "generator {:?} failed in verify run A (exit_code={}): {}",
                        entry.id,
                        run_a.exit_code,
                        stderr_summary(&run_a.stderr)
                    ),
                    None,
                ));
                generator_results.push(json!({"id": entry.id, "status": "error"}));
                continue;
            }

            let drift = diff_outputs(&project_root, &temp_a.path, &entry.outputs)?;
            if !drift.is_empty() {
                let mut d = diag_error(
                    "E_GEN_DRIFT",
                    diagnostics::Stage::Run,
                    format!(
                        "generator {:?} outputs drifted (re-run `x07 gen write --index {}`)",
                        entry.id,
                        display_rel(&project_root, &index_path)
                    ),
                    None,
                );
                for rel in drift.iter().take(50) {
                    d.notes.push(format!("drifted: {rel}"));
                }
                diagnostics.push(d);
            }

            let mut nondet = Vec::new();
            if mode == VerifyMode::DoubleRunV1 {
                let temp_b = TempDirGuard::new("gen_verify_b")?;
                copy_project_tree(&project_root, &temp_b.path)?;
                remove_outputs(&temp_b.path, &entry.outputs);
                let run_b = run_self(&temp_b.path, &entry.write_argv)?;
                if run_b.exit_code != 0 {
                    diagnostics.push(diag_error(
                        "E_GEN_RUN_FAILED",
                        diagnostics::Stage::Run,
                        format!(
                            "generator {:?} failed in verify run B (exit_code={}): {}",
                            entry.id,
                            run_b.exit_code,
                            stderr_summary(&run_b.stderr)
                        ),
                        None,
                    ));
                } else {
                    nondet = diff_outputs(&temp_a.path, &temp_b.path, &entry.outputs)?;
                    if !nondet.is_empty() {
                        let mut d = diag_error(
                            "E_GEN_NONDETERMINISTIC",
                            diagnostics::Stage::Run,
                            format!("generator {:?} outputs are not deterministic", entry.id),
                            None,
                        );
                        for rel in nondet.iter().take(50) {
                            d.notes.push(format!("nondeterministic: {rel}"));
                        }
                        diagnostics.push(d);
                    }
                }
            }

            let status = if drift.is_empty() && nondet.is_empty() {
                "ok"
            } else {
                "fail"
            };
            generator_results.push(json!({
                "id": entry.id,
                "mode": entry.mode.as_deref().unwrap_or("double_run_v1"),
                "status": status,
                "outputs": entry.outputs,
            }));
        }
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diagnostics);
    report.meta.insert(
        "index".to_string(),
        json!({
            "path": index_path.display().to_string(),
            "sha256_hex": index_sha256_hex,
        }),
    );
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report
        .meta
        .insert("generators".to_string(), Value::Array(generator_results));

    write_report(machine, &report)?;
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_gen_write(
    machine: &crate::reporting::MachineArgs,
    args: GenWriteArgs,
) -> Result<std::process::ExitCode> {
    let index_path = util::resolve_existing_path_upwards(&args.index);
    let index_doc = report_common::read_json_file(&index_path)
        .with_context(|| format!("read index JSON: {}", index_path.display()))?;
    let schema_diags = report_common::validate_schema(
        ARCH_GEN_INDEX_SCHEMA_BYTES,
        "spec/x07-arch.gen.index.schema.json",
        &index_doc,
    )?;
    if !schema_diags.is_empty() {
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(
            schema_diags
                .into_iter()
                .map(|d| {
                    remap_schema_diag(
                        "E_GEN_INDEX_SCHEMA_INVALID",
                        d,
                        Some(index_path.display().to_string()),
                    )
                })
                .collect(),
        );
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let index: ArchGenIndex =
        serde_json::from_value(index_doc).context("parse index JSON (schema-valid)")?;
    if index.schema_version != ARCH_GEN_INDEX_SCHEMA_VERSION {
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(vec![diag_error(
            "E_GEN_INDEX_SCHEMA_VERSION_UNSUPPORTED",
            diagnostics::Stage::Parse,
            format!(
                "index schema_version mismatch: expected {ARCH_GEN_INDEX_SCHEMA_VERSION:?} got {:?}",
                index.schema_version
            ),
            Some(diagnostics::Location::X07Ast {
                ptr: "/schema_version".to_string(),
            }),
        )]);
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let project_root = index_path
        .parent()
        .and_then(find_project_root)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let mut diags = Vec::new();
    for entry in &index.generators {
        if let Err(err) = VerifyMode::parse(entry.mode.as_deref()) {
            diags.push(diag_error(
                "E_GEN_ENTRY_MODE_UNSUPPORTED",
                diagnostics::Stage::Parse,
                format!("generator {:?}: {err:#}", entry.id),
                None,
            ));
        }
        validate_entry_argv(entry, &mut diags);
    }
    if diags
        .iter()
        .any(|d| d.severity == diagnostics::Severity::Error)
    {
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(diags);
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    for entry in &index.generators {
        remove_outputs(&project_root, &entry.outputs);
        let run = run_self(&project_root, &entry.write_argv)?;
        if run.exit_code != 0 {
            let mut report = diagnostics::Report::ok();
            report = report.with_diagnostics(vec![diag_error(
                "E_GEN_RUN_FAILED",
                diagnostics::Stage::Run,
                format!(
                    "generator {:?} failed (exit_code={}): {}",
                    entry.id,
                    run.exit_code,
                    stderr_summary(&run.stderr)
                ),
                None,
            )]);
            write_report(machine, &report)?;
            return Ok(std::process::ExitCode::from(1));
        }
    }

    let report = diagnostics::Report::ok();
    write_report(machine, &report)?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn validate_entry_argv(entry: &ArchGenEntry, diagnostics: &mut Vec<diagnostics::Diagnostic>) {
    if entry.check_argv.is_empty() {
        diagnostics.push(diag_error(
            "E_GEN_ENTRY_CHECK_ARGV_EMPTY",
            diagnostics::Stage::Parse,
            format!("generator {:?} check_argv must be non-empty", entry.id),
            None,
        ));
    }
    if entry.write_argv.is_empty() {
        diagnostics.push(diag_error(
            "E_GEN_ENTRY_WRITE_ARGV_EMPTY",
            diagnostics::Stage::Parse,
            format!("generator {:?} write_argv must be non-empty", entry.id),
            None,
        ));
    }

    if entry.check_argv.iter().any(|s| s == "--write") {
        diagnostics.push(diag_error(
            "E_GEN_ENTRY_CHECK_ARGV_CONTAINS_WRITE",
            diagnostics::Stage::Parse,
            format!(
                "generator {:?} check_argv must not contain --write",
                entry.id
            ),
            None,
        ));
    }
    if entry.write_argv.iter().any(|s| s == "--check") {
        diagnostics.push(diag_error(
            "E_GEN_ENTRY_WRITE_ARGV_CONTAINS_CHECK",
            diagnostics::Stage::Parse,
            format!(
                "generator {:?} write_argv must not contain --check",
                entry.id
            ),
            None,
        ));
    }
}

fn validate_entry_outputs(
    entry: &ArchGenEntry,
    project_root: &Path,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    if entry.outputs.is_empty() {
        diagnostics.push(diag_error(
            "E_GEN_ENTRY_OUTPUTS_EMPTY",
            diagnostics::Stage::Parse,
            format!("generator {:?} outputs must be non-empty", entry.id),
            None,
        ));
        return;
    }
    for out in &entry.outputs {
        let path = project_root.join(out);
        if !path.exists() {
            diagnostics.push(diag_error(
                "E_GEN_MISSING_OUTPUT",
                diagnostics::Stage::Run,
                format!(
                    "generator {:?} missing declared output root: {}",
                    entry.id,
                    display_rel(project_root, &path)
                ),
                None,
            ));
        }
    }
}

#[derive(Debug, Clone)]
struct RunOutcome {
    exit_code: i32,
    stderr: Vec<u8>,
}

fn run_self(cwd: &Path, argv: &[String]) -> Result<RunOutcome> {
    let exe = std::env::current_exe().context("resolve current x07 executable")?;
    let mut cmd = Command::new(exe);
    cmd.current_dir(cwd);
    cmd.env("LC_ALL", "C");
    cmd.env("LANG", "C");
    cmd.env("TZ", "UTC");
    cmd.env("SOURCE_DATE_EPOCH", "0");
    cmd.env("X07_TOOL_API_CHILD", "1");
    cmd.args(argv.iter().map(OsString::from));
    let out = cmd
        .output()
        .with_context(|| format!("run x07 command in {}", cwd.display()))?;
    Ok(RunOutcome {
        exit_code: out.status.code().unwrap_or(-1),
        stderr: out.stderr,
    })
}

fn copy_project_tree(src_root: &Path, dst_root: &Path) -> Result<()> {
    for entry in WalkDir::new(src_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(should_copy_entry)
        .flatten()
    {
        let path = entry.path();
        let rel = match path.strip_prefix(src_root) {
            Ok(p) if !p.as_os_str().is_empty() => p,
            _ => continue,
        };
        let dst = dst_root.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dst).with_context(|| format!("mkdir: {}", dst.display()))?;
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("mkdir: {}", parent.display()))?;
        }
        std::fs::copy(path, &dst)
            .with_context(|| format!("copy {} -> {}", path.display(), dst.display()))?;
    }
    Ok(())
}

fn should_copy_entry(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !matches!(
        name.as_ref(),
        ".git" | ".x07" | "target" | ".agent" | ".claude"
    )
}

fn remove_outputs(project_root: &Path, outputs: &[String]) {
    for rel in outputs {
        let path = project_root.join(rel);
        if !path.exists() {
            continue;
        }
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn diff_outputs(a_root: &Path, b_root: &Path, outputs: &[String]) -> Result<Vec<String>> {
    let a = collect_output_digests(a_root, outputs)?;
    let b = collect_output_digests(b_root, outputs)?;

    let mut diffs = Vec::new();
    let mut all: BTreeSet<String> = BTreeSet::new();
    all.extend(a.keys().cloned());
    all.extend(b.keys().cloned());

    for key in all {
        match (a.get(&key), b.get(&key)) {
            (Some(da), Some(db)) if da == db => {}
            _ => diffs.push(key),
        }
    }

    Ok(diffs)
}

fn collect_output_digests(root: &Path, outputs: &[String]) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for rel in outputs {
        let base = root.join(rel);
        if base.is_file() {
            insert_digest(root, &base, &mut out)?;
            continue;
        }
        if base.is_dir() {
            for entry in WalkDir::new(&base)
                .follow_links(false)
                .into_iter()
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                insert_digest(root, entry.path(), &mut out)?;
            }
            continue;
        }
        // Missing outputs are handled by callers.
    }
    Ok(out)
}

fn insert_digest(root: &Path, path: &Path, out: &mut BTreeMap<String, String>) -> Result<()> {
    let rel = display_rel(root, path);
    let bytes = std::fs::read(path).with_context(|| format!("read: {}", path.display()))?;
    out.insert(rel, util::sha256_hex(&bytes));
    Ok(())
}

fn display_rel(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => path.to_string_lossy().replace('\\', "/"),
    }
}

fn stderr_summary(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr).trim().to_string();
    if text.is_empty() {
        "no stderr output".to_string()
    } else {
        text
    }
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir: Option<&Path> = Some(start);
    while let Some(d) = dir {
        if d.join("x07.json").is_file() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

fn remap_schema_diag(
    code: &str,
    mut d: diagnostics::Diagnostic,
    file: Option<String>,
) -> diagnostics::Diagnostic {
    d.code = code.to_string();
    if let Some(file) = file {
        d.data.insert("file".to_string(), Value::String(file));
    }
    d
}

fn diag_error(
    code: &str,
    stage: diagnostics::Stage,
    message: impl Into<String>,
    loc: Option<diagnostics::Location>,
) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Error,
        stage,
        message: message.into(),
        loc,
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    }
}

fn write_diag_and_exit(
    machine: &crate::reporting::MachineArgs,
    index_path: &Path,
    project_root: Option<&Path>,
    diagnostics: Vec<diagnostics::Diagnostic>,
) -> Result<std::process::ExitCode> {
    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diagnostics);
    report.meta.insert(
        "index".to_string(),
        Value::String(index_path.display().to_string()),
    );
    if let Some(root) = project_root {
        report.meta.insert(
            "project_root".to_string(),
            Value::String(root.display().to_string()),
        );
    }
    write_report(machine, &report)?;
    Ok(std::process::ExitCode::from(1))
}

fn write_report(
    machine: &crate::reporting::MachineArgs,
    report: &diagnostics::Report,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(report)?;
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }

    if let Some(path) = machine.report_out.as_deref() {
        if path.as_os_str() == std::ffi::OsStr::new("-") {
            anyhow::bail!("--report-out '-' is not supported (stdout is reserved for the report)");
        }
        crate::reporting::write_bytes(path, &bytes)?;
    }
    if machine.quiet_json {
        return Ok(());
    }

    std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
    Ok(())
}
