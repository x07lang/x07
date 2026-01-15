use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use clap::Args;
use jsonschema::Draft;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::util;

const X07CLI_SPECROWS_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07cli.specrows.schema.json");
const X07CLI_SPECROWS_COMPILE_ENTRY_BYTES: &[u8] =
    include_bytes!("assets/x07cli_specrows_compile_v1.x07.json");

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

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
    Spec(SpecArgs),
}

#[derive(Debug, Args)]
pub struct SpecArgs {
    #[command(subcommand)]
    pub cmd: Option<SpecCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum SpecCommand {
    Fmt(SpecFmtArgs),
    Check(SpecCheckArgs),
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

    let (schema_diags, _) = validate_specrows_schema_file(&in_path)?;
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

    let semantic = run_semantic_check(&in_path)?;
    let ok = !has_errors(&semantic.diagnostics);
    let report = SpecCheckReport {
        ok,
        r#in: in_path.display().to_string(),
        diag_out: args.diag_out.as_ref().map(|p| p.display().to_string()),
        diagnostics_count: semantic.diagnostics.len(),
        diagnostics: semantic.diagnostics,
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
    let (schema_diags, _) = validate_specrows_schema_file(&in_path)?;
    if !schema_diags.is_empty() {
        write_diags_stderr(&schema_diags)?;
        return Ok(std::process::ExitCode::from(20));
    }

    let fmt = run_semantic_fmt(&in_path)?;
    let has_errors = has_errors(&fmt.diagnostics);
    if has_errors {
        write_diags_stderr(&fmt.diagnostics)?;
    }

    match (args.write, args.out.as_ref()) {
        (true, _) => write_bytes(&in_path, &fmt.canon_json)?,
        (false, None) => {
            std::io::stdout().write_all(&fmt.canon_json)?;
        }
        (false, Some(out)) if out.as_os_str() == "-" => {
            std::io::stdout().write_all(&fmt.canon_json)?;
        }
        (false, Some(out)) => write_bytes(out, &fmt.canon_json)?,
    }

    Ok(if has_errors {
        std::process::ExitCode::from(20)
    } else {
        std::process::ExitCode::SUCCESS
    })
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

    let (schema_diags, _) = validate_specrows_schema_file(&in_path)?;
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

    let fmt = run_semantic_fmt(&in_path)?;
    if has_errors(&fmt.diagnostics) {
        let report = SpecCompileReport {
            ok: false,
            r#in: in_path.display().to_string(),
            out: out_path.display().to_string(),
            sha256: String::new(),
            diagnostics_count: fmt.diagnostics.len(),
            diagnostics: fmt.diagnostics,
            tool_error: None,
        };
        println!("{}", serde_json::to_string(&report)?);
        return Ok(std::process::ExitCode::from(20));
    }

    let module_roots = match default_cli_module_roots() {
        Ok(r) => r,
        Err(err) => {
            let report = SpecCompileReport {
                ok: false,
                r#in: in_path.display().to_string(),
                out: out_path.display().to_string(),
                sha256: String::new(),
                diagnostics_count: fmt.diagnostics.len(),
                diagnostics: fmt.diagnostics,
                tool_error: Some(err.to_string()),
            };
            println!("{}", serde_json::to_string(&report)?);
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let compiled = match compile_specrows_v1(&module_roots, &fmt.canon_json) {
        Ok(SpecrowsCompileOutput::Ok(b)) => b,
        Ok(SpecrowsCompileOutput::ErrCode(code)) => {
            let mut diagnostics = fmt.diagnostics;
            diagnostics.push(X07CliDiagnostic {
                severity: "error".to_string(),
                code: "ECLI_COMPILE_FAILED".to_string(),
                scope: String::new(),
                row_index: -1,
                message: format!("ext.cli.specrows.compile failed (code={code})"),
            });
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
                diagnostics_count: fmt.diagnostics.len(),
                diagnostics: fmt.diagnostics,
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
        diagnostics_count: fmt.diagnostics.len(),
        diagnostics: fmt.diagnostics,
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

#[derive(Debug)]
struct SemanticRun {
    diagnostics: Vec<X07CliDiagnostic>,
    canon_json: Vec<u8>,
}

#[derive(Debug)]
struct SemanticCheck {
    diagnostics: Vec<X07CliDiagnostic>,
}

fn run_semantic_check(spec_path: &Path) -> Result<SemanticCheck> {
    let python = python_bin();
    let script = semantic_script_path()?;

    let output = Command::new(python)
        .arg(script)
        .arg("check")
        .arg(spec_path)
        .arg("--diag-out")
        .arg("-")
        .output()
        .with_context(|| format!("run semantic validator: {}", spec_path.display()))?;

    if !output.status.success() && output.status.code() != Some(1) {
        anyhow::bail!(
            "semantic validator failed unexpectedly (exit={:?})",
            output.status.code()
        );
    }

    let diags: X07CliDiagnosticsFile = serde_json::from_slice(&output.stdout)
        .with_context(|| "parse semantic diagnostics JSON")?;
    Ok(SemanticCheck {
        diagnostics: diags.diagnostics,
    })
}

fn run_semantic_fmt(spec_path: &Path) -> Result<SemanticRun> {
    let python = python_bin();
    let script = semantic_script_path()?;

    let diag_path = tmp_path("x07cli_semantic", "diag.json");
    let output = Command::new(python)
        .arg(script)
        .arg("fmt")
        .arg(spec_path)
        .arg("--diag-out")
        .arg(&diag_path)
        .arg("--out")
        .arg("-")
        .output()
        .with_context(|| format!("run semantic formatter: {}", spec_path.display()))?;

    if !output.status.success() && output.status.code() != Some(1) {
        anyhow::bail!(
            "semantic formatter failed unexpectedly (exit={:?})",
            output.status.code()
        );
    }

    let diag_bytes = fs::read(&diag_path)
        .with_context(|| format!("read semantic diagnostics: {}", diag_path.display()))?;
    let diags: X07CliDiagnosticsFile =
        serde_json::from_slice(&diag_bytes).with_context(|| "parse semantic diagnostics JSON")?;

    Ok(SemanticRun {
        diagnostics: diags.diagnostics,
        canon_json: output.stdout,
    })
}

fn has_errors(diags: &[X07CliDiagnostic]) -> bool {
    diags.iter().any(|d| d.severity == "error")
}

fn validate_specrows_schema_file(path: &Path) -> Result<(Vec<X07CliDiagnostic>, Value)> {
    let bytes = fs::read(path).with_context(|| format!("read: {}", path.display()))?;
    let doc: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(err) => {
            return Ok((
                vec![X07CliDiagnostic {
                    severity: "error".to_string(),
                    code: "ECLI_JSON_PARSE".to_string(),
                    scope: String::new(),
                    row_index: -1,
                    message: err.to_string(),
                }],
                Value::Null,
            ));
        }
    };
    let diags = validate_specrows_schema_value(&doc)?;
    Ok((diags, doc))
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

fn python_bin() -> String {
    let configured = std::env::var("X07_PYTHON").unwrap_or_default();
    if !configured.trim().is_empty() {
        return configured;
    }
    let venv = util::resolve_existing_path_upwards(Path::new(".venv/bin/python"));
    if venv.exists() {
        return venv.display().to_string();
    }
    "python3".to_string()
}

fn semantic_script_path() -> Result<PathBuf> {
    let path =
        util::resolve_existing_path_upwards(Path::new("scripts/check_x07cli_specrows_semantic.py"));
    if !path.exists() {
        anyhow::bail!("missing semantic validator: {}", path.display());
    }
    Ok(path)
}

fn default_cli_module_roots() -> Result<Vec<PathBuf>> {
    let candidates = [
        PathBuf::from("stdlib/std/0.1.1/modules"),
        PathBuf::from("packages/ext/x07-ext-cli/0.1.0/modules"),
        PathBuf::from("packages/ext/x07-ext-data-model/0.1.0/modules"),
        PathBuf::from("packages/ext/x07-ext-json-rs/0.1.0/modules"),
    ];

    let mut out = Vec::new();
    for path in candidates {
        let resolved = util::resolve_existing_path_upwards(&path);
        if !resolved.exists() {
            anyhow::bail!("missing module root: {}", path.display());
        }
        out.push(resolved);
    }
    Ok(out)
}

fn tmp_path(prefix: &str, file_name: &str) -> PathBuf {
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = String::new();
    name.push_str(prefix);
    name.push('_');
    name.push_str(&pid.to_string());
    name.push('_');
    name.push_str(&n.to_string());
    name.push('_');
    name.push_str(file_name);
    std::env::temp_dir().join(name)
}
