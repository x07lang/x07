use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Parser;
use x07_vm::{
    apple_container_cleanup, apple_container_hard_kill, apple_container_soft_stop, docker_cleanup,
    docker_hard_kill, docker_soft_stop, firecracker_ctr_cleanup, firecracker_ctr_config_from_env,
    firecracker_ctr_config_from_job, firecracker_ctr_hard_kill, firecracker_ctr_soft_stop,
    hard_kill_pid_and_group, podman_cleanup, podman_hard_kill, podman_soft_stop,
    vz_cleanup_scratch, VmBackend, VmJob,
};

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

    let now = now_unix_ms()?;
    let t_soft = job
        .deadline_unix_ms
        .saturating_sub(job.grace_ms)
        .max(job.created_unix_ms);

    if now < t_soft {
        sleep_until_or_done(t_soft, &done_marker)?;
    }
    if done_marker.is_file() {
        return Ok(());
    }

    match job.backend {
        VmBackend::Vz => {}
        VmBackend::AppleContainer => {
            let _ = apple_container_soft_stop(&job.container_id);
        }
        VmBackend::Docker => {
            let _ = docker_soft_stop(&job.container_id, job.grace_ms);
        }
        VmBackend::Podman => {
            let _ = podman_soft_stop(&job.container_id, job.grace_ms);
        }
        VmBackend::FirecrackerCtr => {
            let cfg = job
                .ctr
                .as_ref()
                .map(firecracker_ctr_config_from_job)
                .unwrap_or_else(firecracker_ctr_config_from_env);
            let _ = firecracker_ctr_soft_stop(&cfg, &job.container_id, job.grace_ms);
        }
    }

    sleep_until_or_done(job.deadline_unix_ms, &done_marker)?;
    if done_marker.is_file() {
        return Ok(());
    }

    match job.backend {
        VmBackend::Vz => {
            if let Some(pid) = job.pid {
                hard_kill_pid_and_group(pid);
            }
        }
        VmBackend::AppleContainer => {
            let _ = apple_container_hard_kill(&job.container_id);
        }
        VmBackend::Docker => {
            let _ = docker_hard_kill(&job.container_id);
        }
        VmBackend::Podman => {
            let _ = podman_hard_kill(&job.container_id);
        }
        VmBackend::FirecrackerCtr => {
            let cfg = job
                .ctr
                .as_ref()
                .map(firecracker_ctr_config_from_job)
                .unwrap_or_else(firecracker_ctr_config_from_env);
            let _ = firecracker_ctr_hard_kill(&cfg, &job.container_id);
        }
    }

    let cleanup_deadline = job.deadline_unix_ms.saturating_add(job.cleanup_ms);
    let mut backoff = Duration::from_millis(100);
    loop {
        if done_marker.is_file() {
            return Ok(());
        }

        match job.backend {
            VmBackend::Vz => {
                let _ = vz_cleanup_scratch(state_dir);
            }
            VmBackend::AppleContainer => {
                let _ = apple_container_cleanup(&job.container_id);
            }
            VmBackend::Docker => {
                let _ = docker_cleanup(&job.container_id);
            }
            VmBackend::Podman => {
                let _ = podman_cleanup(&job.container_id);
            }
            VmBackend::FirecrackerCtr => {
                let cfg = job
                    .ctr
                    .as_ref()
                    .map(firecracker_ctr_config_from_job)
                    .unwrap_or_else(firecracker_ctr_config_from_env);
                let _ = firecracker_ctr_cleanup(&cfg, &job.container_id);
            }
        }

        if now_unix_ms()? >= cleanup_deadline {
            break;
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(Duration::from_secs(1));
    }

    let _ = std::fs::write(reaped_marker, b"reaped\n");
    Ok(())
}

fn now_unix_ms() -> Result<u64> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(d.as_millis().try_into().unwrap_or(u64::MAX))
}

fn sleep_until_or_done(deadline_unix_ms: u64, done_marker: &Path) -> Result<()> {
    loop {
        if done_marker.is_file() {
            return Ok(());
        }
        let now = now_unix_ms()?;
        if now >= deadline_unix_ms {
            return Ok(());
        }
        let remaining_ms = deadline_unix_ms - now;
        std::thread::sleep(Duration::from_millis(remaining_ms.min(250)));
    }
}
