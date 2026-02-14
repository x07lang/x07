use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use x07_vm::{enforce_kill_plan_for_job, KillResult, VmJob};

#[derive(Parser)]
#[command(name = "x07-vm-reaper")]
#[command(about = "Watchdog for VM-backed sandbox jobs.", long_about = None)]
struct Cli {
    #[arg(long, value_name = "PATH")]
    job: PathBuf,
}

fn main() -> std::process::ExitCode {
    match try_main() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::from(2),
    }
}

fn try_main() -> Result<()> {
    let cli = Cli::parse();

    let bytes =
        std::fs::read(&cli.job).with_context(|| format!("read job file: {}", cli.job.display()))?;
    let job: VmJob = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse job JSON: {}", cli.job.display()))?;

    if job.schema_version != x07_vm::VM_JOB_SCHEMA_VERSION {
        anyhow::bail!(
            "job.schema_version mismatch: expected {} got {:?}",
            x07_vm::VM_JOB_SCHEMA_VERSION,
            job.schema_version
        );
    }

    let state_dir = cli
        .job
        .parent()
        .context("job file has no parent directory")?;
    let done_marker = state_dir.join("done");
    let reaped_marker = state_dir.join("reaped");

    let res = enforce_kill_plan_for_job(&job, state_dir, &done_marker)?;
    if res == KillResult::CompletedBeforeDeadline || done_marker.is_file() {
        return Ok(());
    }

    let _ = std::fs::write(reaped_marker, b"reaped\n");
    Ok(())
}
