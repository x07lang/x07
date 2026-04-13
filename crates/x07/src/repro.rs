use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolInfo {
    pub(crate) x07_version: String,
    pub(crate) x07c_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) git_sha: Option<String>,
}

pub(crate) fn tool_info() -> ToolInfo {
    ToolInfo {
        x07_version: env!("CARGO_PKG_VERSION").to_string(),
        x07c_version: x07c::X07C_VERSION.to_string(),
        git_sha: std::env::var("X07_GIT_SHA").ok(),
    }
}

#[derive(Debug, Args)]
pub struct ReproArgs {
    #[command(subcommand)]
    pub cmd: Option<ReproCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum ReproCommand {
    /// Generate a self-contained compile repro bundle for a project.
    Compile(ReproCompileArgs),
}

#[derive(Debug, Args)]
pub struct ReproCompileArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: PathBuf,

    /// Override the language/toolchain compatibility mode.
    #[arg(long, value_name = "COMPAT")]
    pub compat: Option<String>,

    /// Output directory to write.
    ///
    /// If omitted, defaults to `<project_root>/.x07/artifacts/repro/compile/<unix_ms>/`.
    #[arg(long, value_name = "DIR")]
    pub out_dir: Option<PathBuf>,
}

pub fn cmd_repro(
    _machine: &crate::reporting::MachineArgs,
    args: ReproArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing repro subcommand (try --help)");
    };
    match cmd {
        ReproCommand::Compile(args) => cmd_repro_compile(args),
    }
}

fn cmd_repro_compile(args: ReproCompileArgs) -> Result<std::process::ExitCode> {
    let project_path = crate::util::resolve_existing_path_upwards(&args.project);
    let ctx = crate::project_ctx::load_project_ctx(&project_path, true).context("load project")?;

    let out_dir = match args.out_dir.as_ref() {
        Some(dir) if dir.is_absolute() => dir.clone(),
        Some(dir) => ctx.base.join(dir),
        None => {
            let unix_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            ctx.base
                .join(".x07")
                .join("artifacts")
                .join("repro")
                .join("compile")
                .join(unix_ms.to_string())
        }
    };

    if out_dir.exists() {
        anyhow::bail!("output directory already exists: {}", out_dir.display());
    }
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("create out dir: {}", out_dir.display()))?;

    let project_file_name = project_path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("x07.json"));

    let bundle_project_path = out_dir.join(project_file_name);
    let bundle_lock_path = out_dir.join(
        ctx.lock_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("x07.lock.json")),
    );

    let project_bytes = std::fs::read(&project_path)
        .with_context(|| format!("read project: {}", project_path.display()))?;
    let lock_bytes = std::fs::read(&ctx.lock_path)
        .with_context(|| format!("read lock: {}", ctx.lock_path.display()))?;
    crate::util::write_atomic(&bundle_project_path, &project_bytes)
        .with_context(|| format!("write project: {}", bundle_project_path.display()))?;
    crate::util::write_atomic(&bundle_lock_path, &lock_bytes)
        .with_context(|| format!("write lock: {}", bundle_lock_path.display()))?;

    let entry_dst = out_dir.join(&ctx.manifest.entry);
    let entry_bytes = std::fs::read(&ctx.program_path)
        .with_context(|| format!("read entry: {}", ctx.program_path.display()))?;
    crate::util::write_atomic(&entry_dst, &entry_bytes)
        .with_context(|| format!("write entry: {}", entry_dst.display()))?;

    for raw_root in &ctx.manifest.module_roots {
        let raw_root = raw_root.trim();
        if raw_root.starts_with("$workspace") {
            anyhow::bail!(
                "x07 repro compile does not currently support $workspace module_roots (got {:?})",
                raw_root
            );
        }
        if !crate::util::is_safe_rel_path(raw_root) || raw_root == "." {
            anyhow::bail!("unsafe module_root path in project: {:?}", raw_root);
        }
        let src = ctx.base.join(raw_root);
        let dst = out_dir.join(raw_root);
        x07_vm::copy_dir_recursive(&src, &dst).with_context(|| {
            format!(
                "copy module_root {:?}: {} -> {}",
                raw_root,
                src.display(),
                dst.display()
            )
        })?;
    }

    for dep in &ctx.lock.dependencies {
        let raw_path = dep.path.trim();
        if raw_path.starts_with("$workspace") {
            anyhow::bail!(
                "x07 repro compile does not currently support $workspace dependency paths (got {:?})",
                raw_path
            );
        }
        if !crate::util::is_safe_rel_path(raw_path) {
            anyhow::bail!("unsafe dependency path in lock: {:?}", raw_path);
        }
        let src = ctx.base.join(raw_path);
        let dst = out_dir.join(raw_path);
        x07_vm::copy_dir_recursive(&src, &dst).with_context(|| {
            format!(
                "copy dep {:?}: {} -> {}",
                raw_path,
                src.display(),
                dst.display()
            )
        })?;
    }

    let tool_json = serde_json::to_value(tool_info()).context("serialize tool info")?;
    let tool_bytes =
        crate::report_common::canonical_pretty_json_bytes(&tool_json).context("canon tool.json")?;
    let tool_path = out_dir.join("tool.json");
    crate::util::write_atomic(&tool_path, &tool_bytes)
        .with_context(|| format!("write tool.json: {}", tool_path.display()))?;

    let diag_out = out_dir.join("diagnostics.json");
    let machine = crate::reporting::MachineArgs {
        out: Some(diag_out.clone()),
        json: None,
        jsonl: false,
        json_schema: false,
        json_schema_id: false,
        report_out: None,
        quiet_json: false,
    };
    let _ = crate::toolchain::cmd_check(
        &machine,
        crate::toolchain::CheckArgs {
            project: bundle_project_path.clone(),
            compat: args.compat.clone(),
            ast: false,
        },
    )
    .context("run x07 check on repro bundle")?;

    println!("wrote repro bundle: {}", out_dir.display());
    println!("project: {}", bundle_project_path.display());
    println!("diagnostics: {}", diag_out.display());
    Ok(std::process::ExitCode::SUCCESS)
}
