use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;
use serde::Serialize;
use serde_json::Value;
use x07_contracts::{
    PROJECT_MANIFEST_SCHEMA_VERSION, PROJECT_MANIFEST_SCHEMA_VERSIONS_SUPPORTED,
    PROJECT_MANIFEST_SCHEMA_VERSION_V0_5_0,
};
use x07c::project;

use crate::util;

#[derive(Debug, Args)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub cmd: Option<ProjectCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum ProjectCommand {
    /// Migrate a project manifest (`x07.json`) to the current schema line.
    Migrate(ProjectMigrateArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ProjectMigrateArgs {
    /// Project manifest path (`x07.json`) or a directory containing it.
    #[arg(long, value_name = "PATH", default_value = "x07.json")]
    pub project: PathBuf,

    /// Check whether a migration is required (default when neither flag is set).
    #[arg(long)]
    pub check: bool,

    /// Write the migrated manifest to `--out` (or in-place when `--out` is omitted).
    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectMigrateChange {
    path: String,
    from: String,
    to: String,
}

#[derive(Debug, Serialize)]
struct ProjectMigrateReport {
    ok: bool,
    command: &'static str,
    project: String,
    out: String,
    check: bool,
    write: bool,
    changed: bool,
    wrote: bool,
    schema_before: String,
    schema_after: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    changes: Vec<ProjectMigrateChange>,
    sha256: String,
    bytes_len: usize,
}

pub fn cmd_project(
    machine: &crate::reporting::MachineArgs,
    args: ProjectArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing project subcommand (try --help)");
    };
    match cmd {
        ProjectCommand::Migrate(args) => cmd_project_migrate(machine, args),
    }
}

fn cmd_project_migrate(
    machine: &crate::reporting::MachineArgs,
    mut args: ProjectMigrateArgs,
) -> Result<std::process::ExitCode> {
    if args.check && args.write {
        anyhow::bail!("--check cannot be combined with --write");
    }
    if !args.check && !args.write {
        args.check = true;
    }

    let project_path = resolve_project_manifest_path(&args.project);
    let out_path = machine.out.clone().unwrap_or_else(|| project_path.clone());

    let input_bytes = match std::fs::read(&project_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            let report = ProjectMigrateReport {
                ok: false,
                command: "project.migrate",
                project: project_path.display().to_string(),
                out: out_path.display().to_string(),
                check: args.check,
                write: args.write,
                changed: false,
                wrote: false,
                schema_before: String::new(),
                schema_after: PROJECT_MANIFEST_SCHEMA_VERSION.to_string(),
                changes: Vec::new(),
                sha256: String::new(),
                bytes_len: 0,
            };
            print_json(&report)?;
            eprintln!("{err}");
            return Ok(std::process::ExitCode::from(2));
        }
    };

    let mut doc: Value = match serde_json::from_slice(&input_bytes) {
        Ok(doc) => doc,
        Err(err) => {
            let report = ProjectMigrateReport {
                ok: false,
                command: "project.migrate",
                project: project_path.display().to_string(),
                out: out_path.display().to_string(),
                check: args.check,
                write: args.write,
                changed: false,
                wrote: false,
                schema_before: String::new(),
                schema_after: PROJECT_MANIFEST_SCHEMA_VERSION.to_string(),
                changes: Vec::new(),
                sha256: String::new(),
                bytes_len: 0,
            };
            print_json(&report)?;
            eprintln!("{err}");
            return Ok(std::process::ExitCode::from(2));
        }
    };

    let Some(obj) = doc.as_object() else {
        let report = ProjectMigrateReport {
            ok: false,
            command: "project.migrate",
            project: project_path.display().to_string(),
            out: out_path.display().to_string(),
            check: args.check,
            write: args.write,
            changed: false,
            wrote: false,
            schema_before: String::new(),
            schema_after: PROJECT_MANIFEST_SCHEMA_VERSION.to_string(),
            changes: Vec::new(),
            sha256: String::new(),
            bytes_len: 0,
        };
        print_json(&report)?;
        eprintln!("project must be a JSON object");
        return Ok(std::process::ExitCode::from(20));
    };

    let schema_before = obj
        .get("schema_version")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if schema_before.is_empty() {
        let report = ProjectMigrateReport {
            ok: false,
            command: "project.migrate",
            project: project_path.display().to_string(),
            out: out_path.display().to_string(),
            check: args.check,
            write: args.write,
            changed: false,
            wrote: false,
            schema_before: String::new(),
            schema_after: PROJECT_MANIFEST_SCHEMA_VERSION.to_string(),
            changes: Vec::new(),
            sha256: String::new(),
            bytes_len: 0,
        };
        print_json(&report)?;
        eprintln!("project.schema_version must be a non-empty string");
        return Ok(std::process::ExitCode::from(20));
    }

    if !PROJECT_MANIFEST_SCHEMA_VERSIONS_SUPPORTED
        .iter()
        .any(|v| *v == schema_before.as_str())
    {
        let report = ProjectMigrateReport {
            ok: false,
            command: "project.migrate",
            project: project_path.display().to_string(),
            out: out_path.display().to_string(),
            check: args.check,
            write: args.write,
            changed: false,
            wrote: false,
            schema_before: schema_before.clone(),
            schema_after: PROJECT_MANIFEST_SCHEMA_VERSION.to_string(),
            changes: Vec::new(),
            sha256: String::new(),
            bytes_len: 0,
        };
        print_json(&report)?;
        eprintln!(
            "unsupported project schema_version {:?} (supported: {:?})",
            schema_before, PROJECT_MANIFEST_SCHEMA_VERSIONS_SUPPORTED
        );
        return Ok(std::process::ExitCode::from(20));
    }

    let schema_after = PROJECT_MANIFEST_SCHEMA_VERSION;
    let is_upgrading_schema = schema_before != schema_after;

    let mut changed = false;
    let mut changes: Vec<ProjectMigrateChange> = Vec::new();

    if let Some(obj) = doc.as_object_mut() {
        if is_upgrading_schema {
            obj.insert(
                "schema_version".to_string(),
                Value::String(schema_after.to_string()),
            );
            changed = true;
            changes.push(ProjectMigrateChange {
                path: "/schema_version".to_string(),
                from: schema_before.clone(),
                to: schema_after.to_string(),
            });
        }

        if is_upgrading_schema && schema_after == PROJECT_MANIFEST_SCHEMA_VERSION_V0_5_0 {
            let compat_before = obj
                .get("compat")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .to_string();
            if compat_before.is_empty() {
                let compat_after = x07c::compat::Compat::CURRENT.to_string_lossy();
                obj.insert("compat".to_string(), Value::String(compat_after.clone()));
                changed = true;
                changes.push(ProjectMigrateChange {
                    path: "/compat".to_string(),
                    from: compat_before,
                    to: compat_after,
                });
            }
        }
    }

    let out_bytes = pretty_json_bytes(&doc)?;

    if let Err(err) = project::parse_project_manifest_bytes(&out_bytes, &project_path) {
        let report = ProjectMigrateReport {
            ok: false,
            command: "project.migrate",
            project: project_path.display().to_string(),
            out: out_path.display().to_string(),
            check: args.check,
            write: args.write,
            changed,
            wrote: false,
            schema_before: schema_before.to_string(),
            schema_after: schema_after.to_string(),
            changes,
            sha256: String::new(),
            bytes_len: 0,
        };
        print_json(&report)?;
        eprintln!("{err:#}");
        return Ok(std::process::ExitCode::from(20));
    }

    let mut wrote = false;
    if args.write && (changed || out_path != project_path) {
        if let Err(err) = util::write_atomic(&out_path, &out_bytes) {
            let report = ProjectMigrateReport {
                ok: false,
                command: "project.migrate",
                project: project_path.display().to_string(),
                out: out_path.display().to_string(),
                check: args.check,
                write: args.write,
                changed,
                wrote: false,
                schema_before: schema_before.to_string(),
                schema_after: schema_after.to_string(),
                changes,
                sha256: String::new(),
                bytes_len: 0,
            };
            print_json(&report)?;
            eprintln!("{err}");
            return Ok(std::process::ExitCode::from(2));
        }
        wrote = true;
    }

    let ok = if args.check { !changed } else { true };
    let report = ProjectMigrateReport {
        ok,
        command: "project.migrate",
        project: project_path.display().to_string(),
        out: out_path.display().to_string(),
        check: args.check,
        write: args.write,
        changed,
        wrote,
        schema_before: schema_before.to_string(),
        schema_after: schema_after.to_string(),
        changes,
        sha256: util::sha256_hex(&out_bytes),
        bytes_len: out_bytes.len(),
    };
    print_json(&report)?;

    Ok(if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn resolve_project_manifest_path(raw: &Path) -> PathBuf {
    let path = match std::fs::metadata(raw) {
        Ok(meta) if meta.is_dir() => raw.join("x07.json"),
        _ => raw.to_path_buf(),
    };
    util::resolve_existing_path_upwards(&path)
}

fn pretty_json_bytes(doc: &Value) -> Result<Vec<u8>> {
    let mut out = serde_json::to_vec_pretty(doc)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}
