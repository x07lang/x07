use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde_json::Value;
use x07_contracts::{X07DIAG_SCHEMA_VERSION, X07_AGENT_CONTEXT_SCHEMA_VERSION};
use x07c::project;

use crate::ast_slice_engine;
use crate::reporting;
use crate::util;
use crate::x07ast_util::canonicalize_x07ast_bytes_to_value;

#[derive(Debug, Clone, Args)]
#[command(subcommand_required = false)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub cmd: Option<AgentCommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AgentCommand {
    /// Emit a deterministic agent context pack (`x07.agent.context@0.1.0`).
    Context(AgentContextArgs),
}

#[derive(Debug, Clone, Args)]
pub struct AgentContextArgs {
    /// Diagnostic input (`x07.x07diag@0.1.0` or `x07.tool.*.report@0.1.0`).
    #[arg(long, value_name = "PATH")]
    pub diag: PathBuf,

    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: PathBuf,

    #[arg(long, value_enum, default_value = "decl")]
    pub enclosure: ast_slice_engine::SliceEnclosure,

    #[arg(long, value_enum, default_value = "all")]
    pub closure: ast_slice_engine::SliceClosure,

    /// Max number of decls in `ast.slice_ast.decls` (hard bound; deterministic truncation).
    #[arg(long, value_name = "N")]
    pub max_nodes: Option<usize>,

    /// Max canonical JSON byte length of `ast.slice_ast` (hard bound; deterministic truncation).
    #[arg(long, value_name = "BYTES")]
    pub max_bytes: Option<usize>,
}

pub fn cmd_agent(
    machine: &crate::reporting::MachineArgs,
    args: AgentArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing agent subcommand (try --help)");
    };

    match cmd {
        AgentCommand::Context(args) => cmd_context(machine, args),
    }
}

fn cmd_context(
    machine: &crate::reporting::MachineArgs,
    args: AgentContextArgs,
) -> Result<std::process::ExitCode> {
    let out_path = machine.out.as_ref();
    if let Some(out_path) = out_path {
        if out_path.as_os_str() == "-" {
            anyhow::bail!("--out '-' is not supported");
        }
    }

    let diag_doc = load_diag_doc(&args.diag)?;
    let (focus_diag_code, focus_ptr) = select_focus(&diag_doc)?;

    let project_path = util::resolve_existing_path_upwards(&args.project);
    let manifest = project::load_project_manifest(&project_path)?;
    let project_root = project_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let entry_path = project_root.join(&manifest.entry);

    let entry_bytes = std::fs::read(&entry_path)
        .with_context(|| format!("read entry: {}", entry_path.display()))?;
    let entry_doc = canonicalize_x07ast_bytes_to_value(&entry_bytes)
        .with_context(|| format!("parse entry x07ast: {}", entry_path.display()))?;
    let entry_schema_version = entry_doc
        .get("schema_version")
        .and_then(Value::as_str)
        .context("entry x07ast missing schema_version")?
        .to_string();

    let project_world = manifest.world.clone();
    let project_entry = manifest.entry.clone();

    let slice_req = ast_slice_engine::SliceRequest {
        ptr: focus_ptr.clone(),
        enclosure: args.enclosure,
        closure: args.closure,
        max_nodes: args.max_nodes,
        max_bytes: args.max_bytes,
    };
    let slice_outcome = ast_slice_engine::slice_x07ast(&entry_doc, &slice_req)
        .with_context(|| format!("slice entry at {focus_ptr:?}"))?;

    let inputs = vec![
        reporting::file_digest(&args.diag)?,
        reporting::file_digest(&project_path)?,
        reporting::file_digest(&entry_path)?,
    ];

    let context_pack = serde_json::json!({
        "schema_version": X07_AGENT_CONTEXT_SCHEMA_VERSION,
        "toolchain": {
            "name": "x07",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "project": {
            "root": project_root.display().to_string(),
            "world": project_world,
            "entry": project_entry.clone(),
        },
        "focus": {
            "diag_code": focus_diag_code,
            "loc_ptr": focus_ptr,
        },
        "diagnostics": diag_doc,
        "ast": {
            "source_path": project_entry,
            "source_schema_version": entry_schema_version,
            "slice_ast": slice_outcome.slice_ast,
            "slice_meta": slice_outcome.slice_meta,
        },
        "env": {},
        "digests": {
            "inputs": inputs,
            "outputs": [],
        }
    });

    let bytes = reporting::canonical_json_bytes(&context_pack)?;
    if let Some(out_path) = out_path {
        util::write_atomic(out_path, &bytes)
            .with_context(|| format!("write: {}", out_path.display()))?;
        return Ok(std::process::ExitCode::SUCCESS);
    }

    std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn load_diag_doc(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).with_context(|| format!("read diag: {}", path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse diag JSON: {}", path.display()))?;

    let schema_version = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if schema_version == X07DIAG_SCHEMA_VERSION {
        return Ok(doc);
    }

    if schema_version.starts_with("x07.tool.") && schema_version.ends_with("@0.1.0") {
        let ok = doc.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let diags = doc
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        return Ok(serde_json::json!({
            "schema_version": X07DIAG_SCHEMA_VERSION,
            "ok": ok,
            "diagnostics": diags,
            "meta": {},
        }));
    }

    anyhow::bail!("unsupported diag schema_version: {schema_version:?}");
}

fn select_focus(diag_doc: &Value) -> Result<(String, String)> {
    let diags = diag_doc
        .get("diagnostics")
        .and_then(Value::as_array)
        .context("diag JSON missing diagnostics[]")?;

    if diags.is_empty() {
        anyhow::bail!("diag JSON has no diagnostics");
    }

    let mut focus = None;
    for d in diags {
        let sev = d.get("severity").and_then(Value::as_str).unwrap_or("");
        if sev == "error" {
            focus = Some(d);
            break;
        }
    }
    let focus = focus
        .or_else(|| diags.first())
        .context("missing diagnostics")?;

    let diag_code = focus
        .get("code")
        .and_then(Value::as_str)
        .context("diagnostic missing code")?
        .to_string();

    let loc = focus
        .get("loc")
        .and_then(Value::as_object)
        .context("focus diagnostic missing loc")?;
    let kind = loc.get("kind").and_then(Value::as_str).unwrap_or("");
    if kind != "x07ast" {
        anyhow::bail!("focus diagnostic loc.kind is not x07ast (got {kind:?}); re-run producing x07ast pointers");
    }
    let ptr = loc
        .get("ptr")
        .and_then(Value::as_str)
        .context("focus diagnostic loc.ptr missing")?
        .to_string();

    Ok((diag_code, ptr))
}
