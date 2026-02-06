use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use x07_contracts::X07C_REPORT_SCHEMA_VERSION;
use x07_worlds::WorldId;
use x07c::diagnostics;
use x07c::lint;
use x07c::project;
use x07c::x07ast;

use crate::repair::{RepairArgs, RepairMode};

#[derive(Debug, Clone, Args)]
pub struct FmtArgs {
    #[arg(long)]
    pub input: PathBuf,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub write: bool,
    #[arg(long)]
    pub report_json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct LintArgs {
    #[arg(long)]
    pub input: PathBuf,
    /// Lint world gating (advanced; the public surface defaults to `run-os`).
    #[arg(long, value_enum, default_value_t = WorldId::RunOs, hide = true)]
    pub world: WorldId,
    #[arg(long)]
    pub report_json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct FixArgs {
    #[arg(long)]
    pub input: PathBuf,
    /// Fix world gating (advanced; the public surface defaults to `run-os`).
    #[arg(long, value_enum, default_value_t = WorldId::RunOs, hide = true)]
    pub world: WorldId,
    #[arg(long)]
    pub write: bool,
    #[arg(long)]
    pub report_json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct BuildArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: PathBuf,

    /// Write generated C source to a file (default: stdout).
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,

    /// Emit the runtime C header (requires `emit_main=false`; use `--freestanding` for embedding).
    #[arg(long, value_name = "PATH")]
    pub emit_c_header: Option<PathBuf>,

    /// Build in freestanding mode for library embedding (exports `x07_solve_v2`; no `main()`).
    #[arg(long)]
    pub freestanding: bool,

    /// Override the generated C source size budget (in bytes).
    #[arg(long, value_name = "BYTES")]
    pub max_c_bytes: Option<usize>,

    #[command(flatten)]
    pub repair: RepairArgs,
}

#[derive(Debug, Serialize)]
struct X07cToolReport {
    schema_version: &'static str,
    command: &'static str,
    ok: bool,
    r#in: String,
    diagnostics_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<diagnostics::Diagnostic>,
    exit_code: u8,
}

pub fn cmd_fmt(args: FmtArgs) -> Result<std::process::ExitCode> {
    if args.check == args.write {
        if args.report_json {
            let report = X07cToolReport {
                schema_version: X07C_REPORT_SCHEMA_VERSION,
                command: "fmt",
                ok: false,
                r#in: args.input.display().to_string(),
                diagnostics_count: 1,
                diagnostics: vec![diagnostic_error(
                    "X07-CLI-ARGS-0001",
                    diagnostics::Stage::Parse,
                    "set exactly one of --check or --write",
                )],
                exit_code: 2,
            };
            print_json(&report)?;
            return Ok(std::process::ExitCode::from(2));
        }
        anyhow::bail!("set exactly one of --check or --write");
    }

    let bytes = match std::fs::read(&args.input) {
        Ok(bytes) => bytes,
        Err(err) => {
            if args.report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fmt",
                    ok: false,
                    r#in: args.input.display().to_string(),
                    diagnostics_count: 1,
                    diagnostics: vec![diagnostic_error(
                        "X07-IO-READ-0001",
                        diagnostics::Stage::Parse,
                        &format!("read input {}: {err}", args.input.display()),
                    )],
                    exit_code: 2,
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(2));
            }
            return Err(err).with_context(|| format!("read input: {}", args.input.display()));
        }
    };

    let mut file = match x07ast::parse_x07ast_json(&bytes) {
        Ok(file) => file,
        Err(err) => {
            if args.report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fmt",
                    ok: false,
                    r#in: args.input.display().to_string(),
                    diagnostics_count: 1,
                    diagnostics: vec![diagnostic_error(
                        "X07-X07AST-PARSE-0001",
                        diagnostics::Stage::Parse,
                        &err.to_string(),
                    )],
                    exit_code: 2,
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(2));
            }
            return Err(anyhow::anyhow!("{err}"));
        }
    };

    x07ast::canonicalize_x07ast_file(&mut file);
    let mut v = x07ast::x07ast_file_to_value(&file);
    x07ast::canon_value_jcs(&mut v);
    let formatted = serde_json::to_string(&v)? + "\n";

    if args.check && bytes != formatted.as_bytes() {
        if args.report_json {
            let report = X07cToolReport {
                schema_version: X07C_REPORT_SCHEMA_VERSION,
                command: "fmt",
                ok: false,
                r#in: args.input.display().to_string(),
                diagnostics_count: 1,
                diagnostics: vec![diagnostic_error(
                    "X07-FMT-0001",
                    diagnostics::Stage::Rewrite,
                    &format!("file is not formatted: {}", args.input.display()),
                )],
                exit_code: 1,
            };
            print_json(&report)?;
            return Ok(std::process::ExitCode::from(1));
        }
        anyhow::bail!("file is not formatted: {}", args.input.display());
    }

    if args.write && bytes != formatted.as_bytes() {
        if let Err(err) = std::fs::write(&args.input, formatted.as_bytes()) {
            if args.report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fmt",
                    ok: false,
                    r#in: args.input.display().to_string(),
                    diagnostics_count: 1,
                    diagnostics: vec![diagnostic_error(
                        "X07-IO-WRITE-0001",
                        diagnostics::Stage::Rewrite,
                        &format!("write {}: {err}", args.input.display()),
                    )],
                    exit_code: 2,
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(2));
            }
            return Err(err).with_context(|| format!("write: {}", args.input.display()));
        }
    }

    if args.report_json {
        let report = X07cToolReport {
            schema_version: X07C_REPORT_SCHEMA_VERSION,
            command: "fmt",
            ok: true,
            r#in: args.input.display().to_string(),
            diagnostics_count: 0,
            diagnostics: Vec::new(),
            exit_code: 0,
        };
        print_json(&report)?;
    }

    Ok(std::process::ExitCode::SUCCESS)
}

pub fn cmd_lint(args: LintArgs) -> Result<std::process::ExitCode> {
    let bytes = match std::fs::read(&args.input) {
        Ok(bytes) => bytes,
        Err(err) => {
            if args.report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "lint",
                    ok: false,
                    r#in: args.input.display().to_string(),
                    diagnostics_count: 1,
                    diagnostics: vec![diagnostic_error(
                        "X07-IO-READ-0001",
                        diagnostics::Stage::Parse,
                        &format!("read input {}: {err}", args.input.display()),
                    )],
                    exit_code: 2,
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(2));
            }
            return Err(err).with_context(|| format!("read input: {}", args.input.display()));
        }
    };

    let mut file = match x07ast::parse_x07ast_json(&bytes) {
        Ok(file) => file,
        Err(err) => {
            if args.report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "lint",
                    ok: false,
                    r#in: args.input.display().to_string(),
                    diagnostics_count: 1,
                    diagnostics: vec![diagnostic_error(
                        "X07-X07AST-PARSE-0001",
                        diagnostics::Stage::Parse,
                        &err.to_string(),
                    )],
                    exit_code: 2,
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(2));
            }
            return Err(anyhow::anyhow!("{err}"));
        }
    };

    x07ast::canonicalize_x07ast_file(&mut file);
    let lint_options = x07c::world_config::lint_options_for_world(args.world);
    let report = lint::lint_file(&file, lint_options);

    if args.report_json {
        let tool_report = X07cToolReport {
            schema_version: X07C_REPORT_SCHEMA_VERSION,
            command: "lint",
            ok: report.ok,
            r#in: args.input.display().to_string(),
            diagnostics_count: report.diagnostics.len(),
            diagnostics: report.diagnostics,
            exit_code: if report.ok { 0 } else { 1 },
        };
        print_json(&tool_report)?;
        return Ok(std::process::ExitCode::from(tool_report.exit_code));
    }

    println!("{}", serde_json::to_string(&report)?);
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

pub fn cmd_fix(args: FixArgs) -> Result<std::process::ExitCode> {
    if args.report_json && !args.write {
        let report = X07cToolReport {
            schema_version: X07C_REPORT_SCHEMA_VERSION,
            command: "fix",
            ok: false,
            r#in: args.input.display().to_string(),
            diagnostics_count: 1,
            diagnostics: vec![diagnostic_error(
                "X07-CLI-ARGS-0002",
                diagnostics::Stage::Parse,
                "--report-json requires --write (otherwise stdout would be the fixed x07AST)",
            )],
            exit_code: 2,
        };
        print_json(&report)?;
        return Ok(std::process::ExitCode::from(2));
    }

    let bytes = match std::fs::read(&args.input) {
        Ok(bytes) => bytes,
        Err(err) => {
            if args.report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fix",
                    ok: false,
                    r#in: args.input.display().to_string(),
                    diagnostics_count: 1,
                    diagnostics: vec![diagnostic_error(
                        "X07-IO-READ-0001",
                        diagnostics::Stage::Parse,
                        &format!("read input {}: {err}", args.input.display()),
                    )],
                    exit_code: 2,
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(2));
            }
            return Err(err).with_context(|| format!("read input: {}", args.input.display()));
        }
    };

    let mut doc: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(doc) => doc,
        Err(err) => {
            if args.report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fix",
                    ok: false,
                    r#in: args.input.display().to_string(),
                    diagnostics_count: 1,
                    diagnostics: vec![diagnostic_error(
                        "X07-JSON-PARSE-0001",
                        diagnostics::Stage::Parse,
                        &format!("parse JSON {}: {err}", args.input.display()),
                    )],
                    exit_code: 2,
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(2));
            }
            return Err(err).with_context(|| format!("parse JSON: {}", args.input.display()));
        }
    };

    let repair_mode = if args.write {
        RepairMode::Write
    } else {
        RepairMode::Memory
    };
    let repair_result = crate::repair::repair_x07ast_file_doc(&mut doc, args.world, 5, repair_mode)
        .context("fix")?;
    let formatted = repair_result.formatted;
    let final_report = repair_result.final_report;

    let remaining_errors: usize = final_report
        .diagnostics
        .iter()
        .filter(|d| d.severity == diagnostics::Severity::Error)
        .count();
    if remaining_errors > 0 && !args.report_json {
        eprintln!(
            "x07 fix: {remaining_errors} error(s) remain after auto-fix. \
             Run `x07 build` to see codegen-stage errors."
        );
    }

    if args.write {
        if let Err(err) = std::fs::write(&args.input, formatted.as_bytes()) {
            if args.report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fix",
                    ok: false,
                    r#in: args.input.display().to_string(),
                    diagnostics_count: 1,
                    diagnostics: vec![diagnostic_error(
                        "X07-IO-WRITE-0001",
                        diagnostics::Stage::Rewrite,
                        &format!("write {}: {err}", args.input.display()),
                    )],
                    exit_code: 2,
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(2));
            }
            return Err(err).with_context(|| format!("write: {}", args.input.display()));
        }
    } else {
        print!("{formatted}");
    }

    if args.report_json {
        let report = X07cToolReport {
            schema_version: X07C_REPORT_SCHEMA_VERSION,
            command: "fix",
            ok: final_report.ok,
            r#in: args.input.display().to_string(),
            diagnostics_count: final_report.diagnostics.len(),
            diagnostics: final_report.diagnostics,
            exit_code: if final_report.ok { 0 } else { 1 },
        };
        print_json(&report)?;
        return Ok(std::process::ExitCode::from(report.exit_code));
    }

    Ok(std::process::ExitCode::SUCCESS)
}

pub fn cmd_build(args: BuildArgs) -> Result<std::process::ExitCode> {
    if let Some(max_c_bytes) = args.max_c_bytes {
        std::env::set_var("X07_MAX_C_BYTES", max_c_bytes.to_string());
    }

    let manifest = project::load_project_manifest(&args.project)?;
    let lock_path = project::default_lockfile_path(&args.project, &manifest);
    let lock_bytes = std::fs::read(&lock_path)
        .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
    let lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
        .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;

    project::verify_lockfile(&args.project, &manifest, &lock)?;

    let base = args
        .project
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let program_path = base.join(&manifest.entry);

    let module_roots = project::collect_module_roots(&args.project, &manifest, &lock)?;
    let world = x07c::world_config::parse_world_id(&manifest.world)
        .with_context(|| format!("invalid project world {:?}", manifest.world))?;

    let repair_result = crate::repair::maybe_repair_x07ast_file(&program_path, world, &args.repair)
        .with_context(|| format!("repair entry: {}", program_path.display()))?;
    let program_bytes = if let Some(r) = repair_result {
        r.formatted.into_bytes()
    } else {
        std::fs::read(&program_path)
            .with_context(|| format!("read entry: {}", program_path.display()))?
    };

    let mut options = x07c::world_config::compile_options_for_world(world, module_roots);
    options.arch_root = Some(base.to_path_buf());
    if args.freestanding {
        options.emit_main = false;
        options.freestanding = true;
    } else if args.emit_c_header.is_some() {
        options.emit_main = false;
    }

    let c = x07c::compile::compile_program_to_c(&program_bytes, &options)
        .map_err(|e| anyhow::anyhow!("compile failed: {:?}: {}", e.kind, e.message))?;
    match args.out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create output dir: {}", parent.display()))?;
            }
            std::fs::write(&path, c.as_bytes())
                .with_context(|| format!("write: {}", path.display()))?;
        }
        None => {
            print!("{c}");
        }
    }

    if let Some(path) = args.emit_c_header {
        let h = x07c::c_emit::emit_c_header(&options)
            .map_err(|e| anyhow::anyhow!("emit header failed: {:?}: {}", e.kind, e.message))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create output dir: {}", parent.display()))?;
        }
        std::fs::write(&path, h.as_bytes())
            .with_context(|| format!("write: {}", path.display()))?;
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn diagnostic_error(
    code: &str,
    stage: diagnostics::Stage,
    message: &str,
) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Error,
        stage,
        message: message.to_string(),
        loc: None,
        notes: Vec::new(),
        related: Vec::new(),
        data: Default::default(),
        quickfix: None,
    }
}
