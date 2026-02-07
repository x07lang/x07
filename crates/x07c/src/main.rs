use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;

use x07_contracts::X07C_REPORT_SCHEMA_VERSION;
use x07_worlds::WorldId;
use x07c::compile;
use x07c::diagnostics;
use x07c::json_patch;
use x07c::language;
use x07c::lint;
use x07c::project;
use x07c::x07ast;

#[derive(Parser)]
#[command(name = "x07c")]
#[command(about = "X07 compiler (X07 -> C).", long_about = None)]
#[command(subcommand_required = false)]
struct Cli {
    #[arg(long, global = true)]
    cli_specrows: bool,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    LangId,
    Guide,
    Fmt {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        write: bool,
        #[arg(long)]
        report_json: bool,
    },
    Lint {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = WorldId::SolvePure)]
        world: WorldId,
        #[arg(long)]
        report_json: bool,
    },
    Fix {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = WorldId::SolvePure)]
        world: WorldId,
        #[arg(long)]
        write: bool,
        #[arg(long)]
        report_json: bool,
    },
    Lock {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Build {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, value_name = "PATH")]
        emit_mono_map: Option<PathBuf>,
        #[arg(long)]
        emit_c_header: Option<PathBuf>,
        #[arg(long)]
        freestanding: bool,
        #[arg(long, value_name = "BYTES")]
        max_c_bytes: Option<usize>,
    },
    Compile {
        #[arg(long)]
        program: PathBuf,
        #[arg(long, value_enum, default_value_t = WorldId::SolvePure)]
        world: WorldId,
        #[arg(long)]
        module_root: Vec<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, value_name = "PATH")]
        emit_mono_map: Option<PathBuf>,
        #[arg(long)]
        emit_c_header: Option<PathBuf>,
        #[arg(long)]
        freestanding: bool,
        #[arg(long, value_name = "BYTES")]
        max_c_bytes: Option<usize>,
    },
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

fn main() -> std::process::ExitCode {
    match try_main() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:#}");
            std::process::ExitCode::from(2)
        }
    }
}

fn try_main() -> Result<std::process::ExitCode> {
    let cli = Cli::parse();

    if cli.cli_specrows {
        use clap::CommandFactory as _;
        let root = Cli::command();
        let path: Vec<&str> = match &cli.cmd {
            None => Vec::new(),
            Some(Cmd::LangId) => vec!["lang-id"],
            Some(Cmd::Guide) => vec!["guide"],
            Some(Cmd::Fmt { .. }) => vec!["fmt"],
            Some(Cmd::Lint { .. }) => vec!["lint"],
            Some(Cmd::Fix { .. }) => vec!["fix"],
            Some(Cmd::Lock { .. }) => vec!["lock"],
            Some(Cmd::Build { .. }) => vec!["build"],
            Some(Cmd::Compile { .. }) => vec!["compile"],
        };

        let node = x07c::cli_specrows::find_command(&root, &path).unwrap_or(&root);
        let doc = x07c::cli_specrows::command_to_specrows(node);
        println!("{}", serde_json::to_string(&doc)?);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let Some(cmd) = cli.cmd else {
        anyhow::bail!("missing subcommand (try --help)");
    };

    match cmd {
        Cmd::LangId => {
            println!("{}", language::LANG_ID);
            Ok(std::process::ExitCode::SUCCESS)
        }
        Cmd::Guide => {
            println!("{}", compile::guide_md());
            Ok(std::process::ExitCode::SUCCESS)
        }
        Cmd::Fmt {
            input,
            check,
            write,
            report_json,
        } => {
            if check == write {
                if report_json {
                    let report = X07cToolReport {
                        schema_version: X07C_REPORT_SCHEMA_VERSION,
                        command: "fmt",
                        ok: false,
                        r#in: input.display().to_string(),
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

            let bytes = match std::fs::read(&input) {
                Ok(bytes) => bytes,
                Err(err) => {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "fmt",
                            ok: false,
                            r#in: input.display().to_string(),
                            diagnostics_count: 1,
                            diagnostics: vec![diagnostic_error(
                                "X07-IO-READ-0001",
                                diagnostics::Stage::Parse,
                                &format!("read input {}: {err}", input.display()),
                            )],
                            exit_code: 2,
                        };
                        print_json(&report)?;
                        return Ok(std::process::ExitCode::from(2));
                    }
                    return Err(err).with_context(|| format!("read input: {}", input.display()));
                }
            };

            let mut file = match x07ast::parse_x07ast_json(&bytes) {
                Ok(file) => file,
                Err(err) => {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "fmt",
                            ok: false,
                            r#in: input.display().to_string(),
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

            if check && bytes != formatted.as_bytes() {
                if report_json {
                    let report = X07cToolReport {
                        schema_version: X07C_REPORT_SCHEMA_VERSION,
                        command: "fmt",
                        ok: false,
                        r#in: input.display().to_string(),
                        diagnostics_count: 1,
                        diagnostics: vec![diagnostic_error(
                            "X07-FMT-0001",
                            diagnostics::Stage::Rewrite,
                            &format!("file is not formatted: {}", input.display()),
                        )],
                        exit_code: 1,
                    };
                    print_json(&report)?;
                    return Ok(std::process::ExitCode::from(1));
                }
                anyhow::bail!("file is not formatted: {}", input.display());
            }

            if write && bytes != formatted.as_bytes() {
                if let Err(err) = std::fs::write(&input, formatted.as_bytes()) {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "fmt",
                            ok: false,
                            r#in: input.display().to_string(),
                            diagnostics_count: 1,
                            diagnostics: vec![diagnostic_error(
                                "X07-IO-WRITE-0001",
                                diagnostics::Stage::Rewrite,
                                &format!("write {}: {err}", input.display()),
                            )],
                            exit_code: 2,
                        };
                        print_json(&report)?;
                        return Ok(std::process::ExitCode::from(2));
                    }
                    return Err(err).with_context(|| format!("write: {}", input.display()));
                }
            }

            if report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fmt",
                    ok: true,
                    r#in: input.display().to_string(),
                    diagnostics_count: 0,
                    diagnostics: Vec::new(),
                    exit_code: 0,
                };
                print_json(&report)?;
            }

            Ok(std::process::ExitCode::SUCCESS)
        }
        Cmd::Lint {
            input,
            world,
            report_json,
        } => {
            let bytes = match std::fs::read(&input) {
                Ok(bytes) => bytes,
                Err(err) => {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "lint",
                            ok: false,
                            r#in: input.display().to_string(),
                            diagnostics_count: 1,
                            diagnostics: vec![diagnostic_error(
                                "X07-IO-READ-0001",
                                diagnostics::Stage::Parse,
                                &format!("read input {}: {err}", input.display()),
                            )],
                            exit_code: 2,
                        };
                        print_json(&report)?;
                        return Ok(std::process::ExitCode::from(2));
                    }
                    return Err(err).with_context(|| format!("read input: {}", input.display()));
                }
            };

            let mut file = match x07ast::parse_x07ast_json(&bytes) {
                Ok(file) => file,
                Err(err) => {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "lint",
                            ok: false,
                            r#in: input.display().to_string(),
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
            let lint_options = x07c::world_config::lint_options_for_world(world);
            let report = lint::lint_file(&file, lint_options);

            if report_json {
                let tool_report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "lint",
                    ok: report.ok,
                    r#in: input.display().to_string(),
                    diagnostics_count: report.diagnostics.len(),
                    diagnostics: report.diagnostics,
                    exit_code: if report.ok { 0 } else { 1 },
                };
                print_json(&tool_report)?;
                return Ok(std::process::ExitCode::from(tool_report.exit_code));
            }

            let out = serde_json::to_string(&report)?;
            println!("{out}");
            Ok(if report.ok {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::from(1)
            })
        }
        Cmd::Fix {
            input,
            world,
            write,
            report_json,
        } => {
            if report_json && !write {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fix",
                    ok: false,
                    r#in: input.display().to_string(),
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

            let bytes = match std::fs::read(&input) {
                Ok(bytes) => bytes,
                Err(err) => {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "fix",
                            ok: false,
                            r#in: input.display().to_string(),
                            diagnostics_count: 1,
                            diagnostics: vec![diagnostic_error(
                                "X07-IO-READ-0001",
                                diagnostics::Stage::Parse,
                                &format!("read input {}: {err}", input.display()),
                            )],
                            exit_code: 2,
                        };
                        print_json(&report)?;
                        return Ok(std::process::ExitCode::from(2));
                    }
                    return Err(err).with_context(|| format!("read input: {}", input.display()));
                }
            };

            let mut doc: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(doc) => doc,
                Err(err) => {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "fix",
                            ok: false,
                            r#in: input.display().to_string(),
                            diagnostics_count: 1,
                            diagnostics: vec![diagnostic_error(
                                "X07-JSON-PARSE-0001",
                                diagnostics::Stage::Parse,
                                &format!("parse JSON {}: {err}", input.display()),
                            )],
                            exit_code: 2,
                        };
                        print_json(&report)?;
                        return Ok(std::process::ExitCode::from(2));
                    }
                    return Err(err).with_context(|| format!("parse JSON: {}", input.display()));
                }
            };

            let lint_options = x07c::world_config::lint_options_for_world(world);

            let (final_report, formatted) = match (|| -> Result<(diagnostics::Report, String)> {
                for _pass in 0..5 {
                    let doc_bytes = serde_json::to_vec(&doc)?;
                    let mut file = x07ast::parse_x07ast_json(&doc_bytes)
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    x07ast::canonicalize_x07ast_file(&mut file);
                    let report = lint::lint_file(&file, lint_options);

                    let mut any_applied = false;
                    for d in &report.diagnostics {
                        let Some(q) = &d.quickfix else { continue };
                        if q.kind != diagnostics::QuickfixKind::JsonPatch {
                            continue;
                        }
                        json_patch::apply_patch(&mut doc, &q.patch)
                            .map_err(|e| anyhow::anyhow!("apply patch failed: {e}"))?;
                        any_applied = true;
                    }
                    if !any_applied {
                        break;
                    }
                }

                let doc_bytes = serde_json::to_vec(&doc)?;
                let mut file =
                    x07ast::parse_x07ast_json(&doc_bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
                x07ast::canonicalize_x07ast_file(&mut file);
                let final_report = lint::lint_file(&file, lint_options);

                let mut v = x07ast::x07ast_file_to_value(&file);
                x07ast::canon_value_jcs(&mut v);
                let formatted = serde_json::to_string(&v)? + "\n";
                Ok((final_report, formatted))
            })() {
                Ok(v) => v,
                Err(err) => {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "fix",
                            ok: false,
                            r#in: input.display().to_string(),
                            diagnostics_count: 1,
                            diagnostics: vec![diagnostic_error(
                                "X07-FIX-0003",
                                diagnostics::Stage::Rewrite,
                                &err.to_string(),
                            )],
                            exit_code: 2,
                        };
                        print_json(&report)?;
                        return Ok(std::process::ExitCode::from(2));
                    }
                    return Err(err);
                }
            };

            if write {
                if let Err(err) = std::fs::write(&input, formatted.as_bytes()) {
                    if report_json {
                        let report = X07cToolReport {
                            schema_version: X07C_REPORT_SCHEMA_VERSION,
                            command: "fix",
                            ok: false,
                            r#in: input.display().to_string(),
                            diagnostics_count: 1,
                            diagnostics: vec![diagnostic_error(
                                "X07-IO-WRITE-0001",
                                diagnostics::Stage::Rewrite,
                                &format!("write {}: {err}", input.display()),
                            )],
                            exit_code: 2,
                        };
                        print_json(&report)?;
                        return Ok(std::process::ExitCode::from(2));
                    }
                    return Err(err).with_context(|| format!("write: {}", input.display()));
                }
            } else {
                print!("{formatted}");
            }

            if report_json {
                let report = X07cToolReport {
                    schema_version: X07C_REPORT_SCHEMA_VERSION,
                    command: "fix",
                    ok: final_report.ok,
                    r#in: input.display().to_string(),
                    diagnostics_count: final_report.diagnostics.len(),
                    diagnostics: final_report.diagnostics,
                    exit_code: if final_report.ok { 0 } else { 1 },
                };
                print_json(&report)?;
                return Ok(std::process::ExitCode::from(report.exit_code));
            }

            Ok(std::process::ExitCode::SUCCESS)
        }
        Cmd::Lock {
            project: project_path,
            out,
        } => {
            let manifest = project::load_project_manifest(&project_path)?;
            let lock = project::compute_lockfile(&project_path, &manifest)?;
            let lock_path =
                out.unwrap_or_else(|| project::default_lockfile_path(&project_path, &manifest));
            if let Some(parent) = lock_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create lockfile dir: {}", parent.display()))?;
            }
            std::fs::write(&lock_path, serde_json::to_vec_pretty(&lock)?)
                .with_context(|| format!("write lockfile: {}", lock_path.display()))?;
            Ok(std::process::ExitCode::SUCCESS)
        }
        Cmd::Build {
            project: project_path,
            out: out_path,
            emit_mono_map,
            emit_c_header,
            freestanding,
            max_c_bytes,
        } => {
            if let Some(max_c_bytes) = max_c_bytes {
                std::env::set_var("X07_MAX_C_BYTES", max_c_bytes.to_string());
            }

            let manifest = project::load_project_manifest(&project_path)?;
            let lock_path = project::default_lockfile_path(&project_path, &manifest);
            let lock_bytes = std::fs::read(&lock_path)
                .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
            let lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
                .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;

            project::verify_lockfile(&project_path, &manifest, &lock)?;

            let base = project_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            let program_path = base.join(&manifest.entry);

            let program_bytes = std::fs::read(&program_path)
                .with_context(|| format!("read entry: {}", program_path.display()))?;

            let module_roots = project::collect_module_roots(&project_path, &manifest, &lock)?;
            let world = x07c::world_config::parse_world_id(&manifest.world)
                .with_context(|| format!("invalid project world {:?}", manifest.world))?;
            let mut options = x07c::world_config::compile_options_for_world(world, module_roots);
            options.arch_root = infer_arch_root_from_path(&project_path)
                .or_else(|| Some(base.to_path_buf()))
                .or_else(|| std::env::current_dir().ok());
            if freestanding {
                options.emit_main = false;
                options.freestanding = true;
            } else if emit_c_header.is_some() {
                options.emit_main = false;
            }

            let compile_out = compile::compile_program_to_c_with_meta(&program_bytes, &options)
                .map_err(|e| anyhow::anyhow!("compile failed: {:?}: {}", e.kind, e.message))?;
            match out_path {
                Some(path) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("create output dir: {}", parent.display()))?;
                    }
                    std::fs::write(&path, compile_out.c_src.as_bytes())
                        .with_context(|| format!("write: {}", path.display()))?;
                }
                None => {
                    print!("{}", compile_out.c_src);
                }
            }

            if let Some(path) = emit_mono_map {
                let mono_map = compile_out.mono_map.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("internal error: compile output missing mono_map")
                })?;
                write_canon_json_file(&path, mono_map)
                    .with_context(|| format!("write mono map: {}", path.display()))?;
            }

            if let Some(path) = emit_c_header {
                let h = x07c::c_emit::emit_c_header(&options).map_err(|e| {
                    anyhow::anyhow!("emit header failed: {:?}: {}", e.kind, e.message)
                })?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("create output dir: {}", parent.display()))?;
                }
                std::fs::write(&path, h.as_bytes())
                    .with_context(|| format!("write: {}", path.display()))?;
            }
            Ok(std::process::ExitCode::SUCCESS)
        }
        Cmd::Compile {
            program,
            world,
            module_root,
            out: out_path,
            emit_mono_map,
            emit_c_header,
            freestanding,
            max_c_bytes,
        } => {
            if let Some(max_c_bytes) = max_c_bytes {
                std::env::set_var("X07_MAX_C_BYTES", max_c_bytes.to_string());
            }

            let program_bytes = std::fs::read(&program)
                .with_context(|| format!("read program: {}", program.display()))?;
            let mut options = x07c::world_config::compile_options_for_world(world, module_root);
            options.arch_root =
                infer_arch_root_from_path(&program).or_else(|| std::env::current_dir().ok());
            if freestanding {
                options.emit_main = false;
                options.freestanding = true;
            } else if emit_c_header.is_some() {
                options.emit_main = false;
            }
            let compile_out = compile::compile_program_to_c_with_meta(&program_bytes, &options)
                .map_err(|e| anyhow::anyhow!("compile failed: {:?}: {}", e.kind, e.message))?;
            match out_path {
                Some(path) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("create output dir: {}", parent.display()))?;
                    }
                    std::fs::write(&path, compile_out.c_src.as_bytes())
                        .with_context(|| format!("write: {}", path.display()))?;
                }
                None => {
                    print!("{}", compile_out.c_src);
                }
            }

            if let Some(path) = emit_mono_map {
                let mono_map = compile_out.mono_map.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("internal error: compile output missing mono_map")
                })?;
                write_canon_json_file(&path, mono_map)
                    .with_context(|| format!("write mono map: {}", path.display()))?;
            }

            if let Some(path) = emit_c_header {
                let h = x07c::c_emit::emit_c_header(&options).map_err(|e| {
                    anyhow::anyhow!("emit header failed: {:?}: {}", e.kind, e.message)
                })?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("create output dir: {}", parent.display()))?;
                }
                std::fs::write(&path, h.as_bytes())
                    .with_context(|| format!("write: {}", path.display()))?;
            }
            Ok(std::process::ExitCode::SUCCESS)
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn write_canon_json_file(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
    }

    let mut v = serde_json::to_value(value)?;
    x07ast::canon_value_jcs(&mut v);
    let out = serde_json::to_string(&v)? + "\n";
    std::fs::write(path, out.as_bytes()).with_context(|| format!("write: {}", path.display()))?;
    Ok(())
}

fn infer_arch_root_from_path(start: &Path) -> Option<PathBuf> {
    let start_dir = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent().map(Path::to_path_buf)?
    };
    let start_dir = std::fs::canonicalize(&start_dir).unwrap_or(start_dir);

    let mut dir: Option<&Path> = Some(start_dir.as_path());
    while let Some(d) = dir {
        if d.join("arch").is_dir() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
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
