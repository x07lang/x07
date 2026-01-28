use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use jsonschema::Draft;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07c::project;

use crate::util;

const X07CLI_SPECROWS_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07cli.specrows.schema.json");
const X07CLI_SPECROWS_CANON_ENTRY_BYTES: &[u8] =
    include_bytes!("assets/x07cli_specrows_canon_v1.x07.json");
const X07CLI_SPECROWS_COMPILE_ENTRY_BYTES: &[u8] =
    include_bytes!("assets/x07cli_specrows_compile_v1.x07.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct X07CliDiagnostic {
    severity: String,
    code: String,
    scope: String,
    row_index: i32,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct X07CliDiagnosticsFile {
    diagnostics: Vec<X07CliDiagnostic>,
}

#[derive(Debug, Args)]
pub struct CliArgs {
    #[command(subcommand)]
    pub cmd: Option<CliCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum CliCommand {
    /// Work with the CLI specrows schema format.
    Spec(SpecArgs),
}

#[derive(Debug, Args)]
pub struct SpecArgs {
    #[command(subcommand)]
    pub cmd: Option<SpecCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum SpecCommand {
    /// Canonically format a specrows JSON file.
    Fmt(SpecFmtArgs),
    /// Validate a specrows JSON file and emit diagnostics.
    Check(SpecCheckArgs),
    /// Compile a specrows JSON file into specbin.
    Compile(SpecCompileArgs),
}

#[derive(Debug, Args)]
pub struct SpecFmtArgs {
    #[arg(long, value_name = "PATH")]
    r#in: PathBuf,

    #[arg(long, value_name = "PATH")]
    out: Option<PathBuf>,

    #[arg(long)]
    write: bool,
}

#[derive(Debug, Args)]
pub struct SpecCheckArgs {
    #[arg(long, value_name = "PATH")]
    r#in: PathBuf,

    #[arg(long, value_name = "PATH")]
    diag_out: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SpecCompileArgs {
    #[arg(long, value_name = "PATH")]
    r#in: PathBuf,

    #[arg(long, value_name = "PATH")]
    out: PathBuf,
}

pub fn cmd_cli(args: CliArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing cli subcommand (try --help)");
    };
    match cmd {
        CliCommand::Spec(args) => cmd_cli_spec(args),
    }
}

fn cmd_cli_spec(args: SpecArgs) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing cli spec subcommand (try --help)");
    };
    match cmd {
        SpecCommand::Fmt(args) => cmd_cli_spec_fmt(args),
        SpecCommand::Check(args) => cmd_cli_spec_check(args),
        SpecCommand::Compile(args) => cmd_cli_spec_compile(args),
    }
}

#[derive(Debug, Serialize)]
struct SpecCheckReport {
    ok: bool,
    r#in: String,
    diag_out: Option<String>,
    diagnostics_count: usize,
    diagnostics: Vec<X07CliDiagnostic>,
}

fn cmd_cli_spec_check(args: SpecCheckArgs) -> Result<std::process::ExitCode> {
    let in_path = util::resolve_existing_path_upwards(&args.r#in);

    let bytes = fs::read(&in_path).with_context(|| format!("read: {}", in_path.display()))?;
    let doc: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(err) => {
            let report = SpecCheckReport {
                ok: false,
                r#in: in_path.display().to_string(),
                diag_out: args.diag_out.as_ref().map(|p| p.display().to_string()),
                diagnostics_count: 1,
                diagnostics: vec![X07CliDiagnostic {
                    severity: "error".to_string(),
                    code: "ECLI_JSON_PARSE".to_string(),
                    scope: String::new(),
                    row_index: -1,
                    message: err.to_string(),
                }],
            };
            write_diagnostics_if_requested(args.diag_out.as_ref(), &report.diagnostics)?;
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };

    let schema_diags = validate_specrows_schema_value(&doc)?;
    if !schema_diags.is_empty() {
        let report = SpecCheckReport {
            ok: false,
            r#in: in_path.display().to_string(),
            diag_out: args.diag_out.as_ref().map(|p| p.display().to_string()),
            diagnostics_count: schema_diags.len(),
            diagnostics: schema_diags,
        };
        write_diagnostics_if_requested(args.diag_out.as_ref(), &report.diagnostics)?;
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let module_roots = infer_cli_module_roots_for_spec(&in_path)?;
    let mut diagnostics: Vec<X07CliDiagnostic> = Vec::new();
    let ok = match canon_specrows_v1(&module_roots, &bytes)? {
        SpecrowsCanonOutput::Ok(_) => true,
        SpecrowsCanonOutput::ErrCode(code) => {
            diagnostics.push(X07CliDiagnostic {
                severity: "error".to_string(),
                code: "ECLI_SEMANTIC".to_string(),
                scope: String::new(),
                row_index: -1,
                message: format!("ext.cli.specrows.compile failed (code={code})"),
            });
            false
        }
    };
    let report = SpecCheckReport {
        ok,
        r#in: in_path.display().to_string(),
        diag_out: args.diag_out.as_ref().map(|p| p.display().to_string()),
        diagnostics_count: diagnostics.len(),
        diagnostics,
    };
    write_diagnostics_if_requested(args.diag_out.as_ref(), &report.diagnostics)?;
    println!("{}", serde_json::to_string(&report)?);

    Ok(if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(20)
    })
}

fn write_diagnostics_if_requested(
    diag_out: Option<&PathBuf>,
    diagnostics: &[X07CliDiagnostic],
) -> Result<()> {
    let Some(diag_out) = diag_out else {
        return Ok(());
    };
    if diag_out.as_os_str() == "-" {
        anyhow::bail!("--diag-out '-' is not supported (stdout is reserved for the report)");
    }

    let payload = X07CliDiagnosticsFile {
        diagnostics: diagnostics.to_vec(),
    };
    let mut out = serde_json::to_vec(&payload)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    fs::write(diag_out, &out).with_context(|| format!("write diagnostics: {}", diag_out.display()))
}

fn cmd_cli_spec_fmt(args: SpecFmtArgs) -> Result<std::process::ExitCode> {
    if args.write && args.out.is_some() {
        anyhow::bail!("set at most one of --out or --write");
    }

    let in_path = util::resolve_existing_path_upwards(&args.r#in);
    let bytes = fs::read(&in_path).with_context(|| format!("read: {}", in_path.display()))?;
    let doc: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(err) => {
            let diags = vec![X07CliDiagnostic {
                severity: "error".to_string(),
                code: "ECLI_JSON_PARSE".to_string(),
                scope: String::new(),
                row_index: -1,
                message: err.to_string(),
            }];
            write_diags_stderr(&diags)?;
            return Ok(std::process::ExitCode::from(20));
        }
    };
    let schema_diags = validate_specrows_schema_value(&doc)?;
    if !schema_diags.is_empty() {
        write_diags_stderr(&schema_diags)?;
        return Ok(std::process::ExitCode::from(20));
    }

    let module_roots = match infer_cli_module_roots_for_spec(&in_path) {
        Ok(r) => r,
        Err(err) => {
            let diags = vec![X07CliDiagnostic {
                severity: "error".to_string(),
                code: "ECLI_TOOL".to_string(),
                scope: String::new(),
                row_index: -1,
                message: err.to_string(),
            }];
            write_diags_stderr(&diags)?;
            return Ok(std::process::ExitCode::from(1));
        }
    };
    let canon_json = match canon_specrows_v1(&module_roots, &bytes)? {
        SpecrowsCanonOutput::Ok(b) => b,
        SpecrowsCanonOutput::ErrCode(code) => {
            let diags = vec![X07CliDiagnostic {
                severity: "error".to_string(),
                code: "ECLI_SEMANTIC".to_string(),
                scope: String::new(),
                row_index: -1,
                message: format!("ext.cli.specrows.compile failed (code={code})"),
            }];
            write_diags_stderr(&diags)?;
            return Ok(std::process::ExitCode::from(20));
        }
    };

    match (args.write, args.out.as_ref()) {
        (true, _) => write_bytes(&in_path, &canon_json)?,
        (false, None) => {
            std::io::stdout().write_all(&canon_json)?;
        }
        (false, Some(out)) if out.as_os_str() == "-" => {
            std::io::stdout().write_all(&canon_json)?;
        }
        (false, Some(out)) => write_bytes(out, &canon_json)?,
    }

    Ok(std::process::ExitCode::SUCCESS)
}

#[derive(Debug, Serialize)]
struct SpecCompileReport {
    ok: bool,
    r#in: String,
    out: String,
    sha256: String,
    diagnostics_count: usize,
    diagnostics: Vec<X07CliDiagnostic>,
    tool_error: Option<String>,
}

fn cmd_cli_spec_compile(args: SpecCompileArgs) -> Result<std::process::ExitCode> {
    let in_path = util::resolve_existing_path_upwards(&args.r#in);
    let out_path = args.out;

    let bytes = fs::read(&in_path).with_context(|| format!("read: {}", in_path.display()))?;
    let doc: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(err) => {
            let report = SpecCompileReport {
                ok: false,
                r#in: in_path.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
                diagnostics_count: 1,
                diagnostics: vec![X07CliDiagnostic {
                    severity: "error".to_string(),
                    code: "ECLI_JSON_PARSE".to_string(),
                    scope: String::new(),
                    row_index: -1,
                    message: err.to_string(),
                }],
                tool_error: None,
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };
    let schema_diags = validate_specrows_schema_value(&doc)?;
    if !schema_diags.is_empty() {
        let report = SpecCompileReport {
            ok: false,
            r#in: in_path.display().to_string(),
            out: out_path.display().to_string(),
            sha256: String::new(),
            diagnostics_count: schema_diags.len(),
            diagnostics: schema_diags,
            tool_error: None,
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let module_roots = match infer_cli_module_roots_for_spec(&in_path) {
        Ok(r) => r,
        Err(err) => {
            let report = SpecCompileReport {
                ok: false,
                r#in: in_path.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
                diagnostics_count: 0,
                diagnostics: Vec::new(),
                tool_error: Some(err.to_string()),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let canon_json = match canon_specrows_v1(&module_roots, &bytes)? {
        SpecrowsCanonOutput::Ok(b) => b,
        SpecrowsCanonOutput::ErrCode(code) => {
            let diagnostics = vec![X07CliDiagnostic {
                severity: "error".to_string(),
                code: "ECLI_SEMANTIC".to_string(),
                scope: String::new(),
                row_index: -1,
                message: format!("ext.cli.specrows.compile failed (code={code})"),
            }];
            let report = SpecCompileReport {
                ok: false,
                r#in: in_path.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
                diagnostics_count: diagnostics.len(),
                diagnostics,
                tool_error: None,
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(20));
        }
    };

    if canon_json.is_empty() {
        let report = SpecCompileReport {
            ok: false,
            r#in: in_path.display().to_string(),
            out: out_path.display().to_string(),
            sha256: String::new(),
            diagnostics_count: 0,
            diagnostics: Vec::new(),
            tool_error: None,
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let compiled = match compile_specrows_v1(&module_roots, &canon_json) {
        Ok(SpecrowsCompileOutput::Ok(b)) => b,
        Ok(SpecrowsCompileOutput::ErrCode(code)) => {
            let diagnostics = vec![X07CliDiagnostic {
                severity: "error".to_string(),
                code: "ECLI_COMPILE_FAILED".to_string(),
                scope: String::new(),
                row_index: -1,
                message: format!("ext.cli.specrows.compile failed (code={code})"),
            }];
            let report = SpecCompileReport {
                ok: false,
                r#in: in_path.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
                diagnostics_count: diagnostics.len(),
                diagnostics,
                tool_error: None,
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(if code == 1099 {
                1
            } else {
                20
            }));
        }
        Err(err) => {
            let report = SpecCompileReport {
                ok: false,
                r#in: in_path.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
                diagnostics_count: 0,
                diagnostics: Vec::new(),
                tool_error: Some(err.to_string()),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(1));
        }
    };

    write_bytes(&out_path, &compiled)?;
    let report = SpecCompileReport {
        ok: true,
        r#in: in_path.display().to_string(),
        out: out_path.display().to_string(),
        sha256: util::sha256_hex(&compiled),
        diagnostics_count: 0,
        diagnostics: Vec::new(),
        tool_error: None,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(std::process::ExitCode::SUCCESS)
}

enum SpecrowsCompileOutput {
    Ok(Vec<u8>),
    ErrCode(i32),
}

fn compile_specrows_v1(
    module_roots: &[PathBuf],
    spec_json: &[u8],
) -> Result<SpecrowsCompileOutput> {
    let config = x07_host_runner::RunnerConfig {
        world: x07_worlds::WorldId::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_rr_index: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 50_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 64 * 1024 * 1024,
        cpu_time_limit_seconds: 30,
        debug_borrow_checks: false,
    };

    let compile_options = x07_host_runner::compile_options_for_world(
        x07_worlds::WorldId::SolvePure,
        module_roots.to_vec(),
    )?;
    let result = x07_host_runner::compile_and_run_with_options(
        X07CLI_SPECROWS_COMPILE_ENTRY_BYTES,
        &config,
        spec_json,
        None,
        &compile_options,
    )?;

    if !result.compile.ok {
        let msg = result
            .compile
            .compile_error
            .as_deref()
            .unwrap_or("compile failed");
        anyhow::bail!("{msg}");
    }

    let Some(run) = result.solve else {
        anyhow::bail!("internal error: compile succeeded but solve did not run");
    };
    if !run.ok || run.exit_status != 0 {
        anyhow::bail!(
            "internal error: compile helper failed (ok={} exit_status={})",
            run.ok,
            run.exit_status
        );
    }

    parse_compile_helper_output(&run.solve_output)
}

fn parse_compile_helper_output(output: &[u8]) -> Result<SpecrowsCompileOutput> {
    let Some((&tag, payload)) = output.split_first() else {
        anyhow::bail!("internal error: empty compile helper output");
    };

    match tag {
        1 => Ok(SpecrowsCompileOutput::Ok(payload.to_vec())),
        0 => {
            if payload.len() < 4 {
                anyhow::bail!("internal error: truncated compile error output");
            }
            let code = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as i32;
            Ok(SpecrowsCompileOutput::ErrCode(code))
        }
        _ => anyhow::bail!("internal error: unknown compile helper tag={tag}"),
    }
}

fn validate_specrows_schema_value(doc: &Value) -> Result<Vec<X07CliDiagnostic>> {
    let schema_json: Value =
        serde_json::from_slice(X07CLI_SPECROWS_SCHEMA_BYTES).context("parse SpecRows schema")?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema_json)
        .context("build SpecRows schema validator")?;

    let mut out = Vec::new();
    for error in validator.iter_errors(doc) {
        out.push(X07CliDiagnostic {
            severity: "error".to_string(),
            code: "ECLI_SCHEMA_INVALID".to_string(),
            scope: String::new(),
            row_index: -1,
            message: format!("{} ({})", error, error.instance_path()),
        });
    }

    out.sort_by(|a, b| {
        (a.code.as_str(), a.message.as_str()).cmp(&(b.code.as_str(), b.message.as_str()))
    });
    Ok(out)
}

fn write_diags_stderr(diags: &[X07CliDiagnostic]) -> Result<()> {
    let payload = X07CliDiagnosticsFile {
        diagnostics: diags.to_vec(),
    };
    let mut out = serde_json::to_vec(&payload)?;
    out.push(b'\n');
    std::io::stderr().write_all(&out)?;
    Ok(())
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("write: {}", path.display()))?;
    Ok(())
}

fn infer_cli_module_roots_for_spec(spec_path: &Path) -> Result<Vec<PathBuf>> {
    let toolchain_roots = toolchain_cli_spec_module_roots().ok();
    let start_dir = spec_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let Some(project_path) = crate::run::discover_project_manifest(start_dir)? else {
        return toolchain_roots
            .ok_or_else(|| anyhow::anyhow!("could not find toolchain packages/ext directory"));
    };

    let manifest =
        project::load_project_manifest(&project_path).context("load project manifest")?;
    let lock_path = project::default_lockfile_path(&project_path, &manifest);

    if lock_path.is_file() {
        let lock_bytes = std::fs::read(&lock_path)
            .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
        let lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
            .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;
        if project::verify_lockfile(&project_path, &manifest, &lock).is_ok() {
            if let Ok(roots) = project::collect_module_roots(&project_path, &manifest, &lock) {
                return Ok(merge_roots(toolchain_roots, roots));
            }
        }
    }

    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut roots: Vec<PathBuf> = Vec::new();
    for r in &manifest.module_roots {
        roots.push(base.join(r));
    }
    for dep in &manifest.dependencies {
        let dep_dir = base.join(&dep.path);
        let (pkg, _, _) = project::load_package_manifest(&dep_dir).with_context(|| {
            format!(
                "load package manifest for {:?}@{:?} from {}",
                dep.name,
                dep.version,
                dep_dir.display()
            )
        })?;
        roots.push(dep_dir.join(pkg.module_root));
    }

    Ok(merge_roots(toolchain_roots, roots))
}

fn merge_roots(toolchain: Option<Vec<PathBuf>>, project: Vec<PathBuf>) -> Vec<PathBuf> {
    let Some(mut out) = toolchain else {
        return project;
    };
    for r in project {
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

fn toolchain_cli_spec_module_roots() -> Result<Vec<PathBuf>> {
    let Some(ext_dir) = crate::pkg::official_ext_packages_dir() else {
        anyhow::bail!("could not find toolchain packages/ext directory");
    };

    let mut roots: Vec<PathBuf> = Vec::new();
    for name in ["ext-cli", "ext-data-model", "ext-json-rs", "ext-unicode-rs"] {
        let versions_dir = ext_dir.join(format!("x07-{name}"));
        let version = best_semver_version_dir(&versions_dir)
            .with_context(|| format!("find installed {name} versions"))?;
        let dep_dir = versions_dir.join(&version);
        if !dep_dir.is_dir() {
            anyhow::bail!(
                "toolchain package is missing on disk: {name}@{version} (expected {})",
                dep_dir.display()
            );
        }
        let (pkg, _, _) = project::load_package_manifest(&dep_dir)
            .with_context(|| format!("load package manifest: {}", dep_dir.display()))?;
        roots.push(dep_dir.join(pkg.module_root));
    }

    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn best_semver_version_dir(dir: &Path) -> Result<String> {
    let mut best: Option<((u64, u64, u64), String)> = None;
    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir: {}", dir.display()))? {
        let entry = entry.with_context(|| format!("read dir entry: {}", dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type: {}", entry.path().display()))?;
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(v) = parse_semver3(&name) else {
            continue;
        };
        match &best {
            None => best = Some((v, name)),
            Some((best_v, _)) if v > *best_v => best = Some((v, name)),
            _ => {}
        }
    }
    best.map(|(_, name)| name)
        .ok_or_else(|| anyhow::anyhow!("no semver directories under {}", dir.display()))
}

fn parse_semver3(s: &str) -> Option<(u64, u64, u64)> {
    let mut it = s.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

enum SpecrowsCanonOutput {
    Ok(Vec<u8>),
    ErrCode(i32),
}

fn canon_specrows_v1(module_roots: &[PathBuf], spec_json: &[u8]) -> Result<SpecrowsCanonOutput> {
    let config = x07_host_runner::RunnerConfig {
        world: x07_worlds::WorldId::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_rr_index: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 50_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 64 * 1024 * 1024,
        cpu_time_limit_seconds: 30,
        debug_borrow_checks: false,
    };

    let compile_options = x07_host_runner::compile_options_for_world(
        x07_worlds::WorldId::SolvePure,
        module_roots.to_vec(),
    )?;
    let result = x07_host_runner::compile_and_run_with_options(
        X07CLI_SPECROWS_CANON_ENTRY_BYTES,
        &config,
        spec_json,
        None,
        &compile_options,
    )?;

    if !result.compile.ok {
        let msg = result
            .compile
            .compile_error
            .as_deref()
            .unwrap_or("compile failed");
        anyhow::bail!("{msg}");
    }

    let Some(run) = result.solve else {
        anyhow::bail!("internal error: compile succeeded but solve did not run");
    };
    if !run.ok || run.exit_status != 0 {
        anyhow::bail!(
            "internal error: canon helper failed (ok={} exit_status={})",
            run.ok,
            run.exit_status
        );
    }

    parse_canon_helper_output(&run.solve_output)
}

fn parse_canon_helper_output(output: &[u8]) -> Result<SpecrowsCanonOutput> {
    let Some((&tag, payload)) = output.split_first() else {
        anyhow::bail!("internal error: empty canon helper output");
    };

    match tag {
        1 => Ok(SpecrowsCanonOutput::Ok(payload.to_vec())),
        0 => {
            if payload.len() < 4 {
                anyhow::bail!("internal error: truncated canon error output");
            }
            let code = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as i32;
            Ok(SpecrowsCanonOutput::ErrCode(code))
        }
        _ => anyhow::bail!("internal error: unknown canon helper tag={tag}"),
    }
}
