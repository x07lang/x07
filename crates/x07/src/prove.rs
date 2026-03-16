use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde_json::Value;

use crate::{report_common, reporting, util, verify};

#[derive(Debug, Clone, Args)]
#[command(subcommand_required = true)]
pub struct ProveArgs {
    #[command(subcommand)]
    pub cmd: ProveCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProveCommand {
    /// Check a proof object independently of the original prove run.
    Check(ProveCheckArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ProveCheckArgs {
    /// Proof object emitted by `x07 verify --prove --emit-proof`.
    #[arg(long, value_name = "PATH")]
    pub proof: PathBuf,
}

pub fn cmd_prove(
    machine: &crate::reporting::MachineArgs,
    args: ProveArgs,
) -> Result<std::process::ExitCode> {
    match args.cmd {
        ProveCommand::Check(args) => cmd_prove_check(machine, args),
    }
}

fn cmd_prove_check(
    machine: &crate::reporting::MachineArgs,
    args: ProveCheckArgs,
) -> Result<std::process::ExitCode> {
    let report = verify::check_proof_object_path(&args.proof)?;
    let value = serde_json::to_value(&report).context("serialize proof-check report JSON")?;
    write_machine_json(
        machine,
        &value,
        if report.ok { 0 } else { 20 },
        &format!("prove check: result={}", report.result),
    )
}

fn write_machine_json(
    machine: &crate::reporting::MachineArgs,
    value: &Value,
    exit_code: u8,
    text_fallback: &str,
) -> Result<std::process::ExitCode> {
    let bytes = report_common::canonical_pretty_json_bytes(value)?;
    if let Some(path) = machine.out.as_deref() {
        util::write_atomic(path, &bytes)
            .with_context(|| format!("write output: {}", path.display()))?;
    }
    if let Some(path) = machine.report_out.as_deref() {
        reporting::write_bytes(path, &bytes)?;
    }
    if machine.quiet_json {
        return Ok(std::process::ExitCode::from(exit_code));
    }
    if matches!(machine.json, Some(crate::reporting::JsonArg::Off)) {
        println!("{text_fallback}");
    } else {
        std::io::stdout()
            .write_all(&bytes)
            .context("write stdout")?;
    }
    Ok(std::process::ExitCode::from(exit_code))
}
