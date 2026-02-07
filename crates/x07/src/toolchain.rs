use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
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
}

#[derive(Debug, Clone, Args)]
pub struct LintArgs {
    #[arg(long)]
    pub input: PathBuf,
    /// Lint world gating (advanced; the public surface defaults to `run-os`).
    #[arg(long, value_enum, default_value_t = WorldId::RunOs, hide = true)]
    pub world: WorldId,
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
}

#[derive(Debug, Clone, Args)]
pub struct BuildArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: PathBuf,

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

pub fn cmd_fmt(
    _machine: &crate::reporting::MachineArgs,
    args: FmtArgs,
) -> Result<std::process::ExitCode> {
    if args.check == args.write {
        anyhow::bail!("set exactly one of --check or --write");
    }

    let bytes = std::fs::read(&args.input)
        .with_context(|| format!("read input: {}", args.input.display()))?;

    let mut file = match x07ast::parse_x07ast_json(&bytes) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow::anyhow!("{err}"));
        }
    };

    x07ast::canonicalize_x07ast_file(&mut file);
    let mut v = x07ast::x07ast_file_to_value(&file);
    x07ast::canon_value_jcs(&mut v);
    let formatted = serde_json::to_string(&v)? + "\n";

    if args.check && bytes != formatted.as_bytes() {
        eprintln!("file is not formatted: {}", args.input.display());
        return Ok(std::process::ExitCode::from(1));
    }

    if args.write && bytes != formatted.as_bytes() {
        std::fs::write(&args.input, formatted.as_bytes())
            .with_context(|| format!("write: {}", args.input.display()))?;
    }

    Ok(std::process::ExitCode::SUCCESS)
}

pub fn cmd_lint(
    machine: &crate::reporting::MachineArgs,
    args: LintArgs,
) -> Result<std::process::ExitCode> {
    let bytes = std::fs::read(&args.input)
        .with_context(|| format!("read input: {}", args.input.display()))?;

    let mut file = match x07ast::parse_x07ast_json(&bytes) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow::anyhow!("{err}"));
        }
    };

    x07ast::canonicalize_x07ast_file(&mut file);
    let lint_options = x07c::world_config::lint_options_for_world(args.world);
    let report = lint::lint_file(&file, lint_options);

    let out = serde_json::to_string(&report)? + "\n";
    if let Some(path) = machine.out.as_deref() {
        crate::reporting::write_bytes(path, out.as_bytes())?;
    } else {
        print!("{out}");
    }

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

pub fn cmd_fix(
    machine: &crate::reporting::MachineArgs,
    args: FixArgs,
) -> Result<std::process::ExitCode> {
    if args.write && machine.out.is_some() {
        anyhow::bail!("--out cannot be combined with --write");
    }

    let bytes = std::fs::read(&args.input)
        .with_context(|| format!("read input: {}", args.input.display()))?;

    let mut doc: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(doc) => doc,
        Err(err) => {
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
    if remaining_errors > 0 {
        eprintln!(
            "x07 fix: {remaining_errors} error(s) remain after auto-fix. \
             Run `x07 build` to see codegen-stage errors."
        );
    }

    if args.write {
        std::fs::write(&args.input, formatted.as_bytes())
            .with_context(|| format!("write: {}", args.input.display()))?;
    } else {
        match machine.out.as_deref() {
            Some(path) => crate::reporting::write_bytes(path, formatted.as_bytes())?,
            None => print!("{formatted}"),
        }
    }

    Ok(if final_report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

pub fn cmd_build(
    machine: &crate::reporting::MachineArgs,
    args: BuildArgs,
) -> Result<std::process::ExitCode> {
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
    match machine.out.as_ref() {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create output dir: {}", parent.display()))?;
            }
            std::fs::write(path, c.as_bytes())
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
