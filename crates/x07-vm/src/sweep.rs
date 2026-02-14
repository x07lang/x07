use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::{
    apple_container_cleanup, apple_container_hard_kill, firecracker_ctr_cleanup,
    firecracker_ctr_config_from_env, firecracker_ctr_config_from_job, firecracker_ctr_hard_kill,
    hard_kill_pid_and_group, parse_apple_container_json_owned, parse_ctr_container_info_json_owned,
    vz_cleanup_scratch, FirecrackerCtrConfig, VmBackend, VmJob, X07_LABEL_DEADLINE_UNIX_MS_KEY,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct SweepReport {
    pub state_reaped: usize,
    pub runtime_reaped: usize,
}

pub fn sweep_orphans_best_effort(
    state_root: &Path,
    backend: VmBackend,
    firecracker_cfg: Option<&FirecrackerCtrConfig>,
) -> Result<SweepReport> {
    let now = now_unix_ms()?;

    let state_reaped = sweep_state_dirs_best_effort(state_root, now).unwrap_or(0);
    let runtime_reaped = match backend {
        VmBackend::AppleContainer => sweep_apple_container_runtime_best_effort(now).unwrap_or(0),
        VmBackend::FirecrackerCtr => {
            let cfg = firecracker_cfg
                .cloned()
                .unwrap_or_else(firecracker_ctr_config_from_env);
            sweep_firecracker_runtime_best_effort(now, &cfg).unwrap_or(0)
        }
        VmBackend::Vz | VmBackend::Docker | VmBackend::Podman => 0,
    };

    Ok(SweepReport {
        state_reaped,
        runtime_reaped,
    })
}

fn now_unix_ms() -> Result<u64> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(d.as_millis().try_into().unwrap_or(u64::MAX))
}

fn sweep_state_dirs_best_effort(state_root: &Path, now_unix_ms: u64) -> Result<usize> {
    let mut reaped: usize = 0;

    let entries = match std::fs::read_dir(state_root) {
        Ok(v) => v,
        Err(_) => return Ok(0),
    };

    for entry in entries {
        let entry = match entry {
            Ok(v) => v,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let done_marker = path.join("done");
        if done_marker.is_file() {
            continue;
        }

        let job_file = path.join("job.json");
        if !job_file.is_file() {
            continue;
        }

        let bytes = match std::fs::read(&job_file) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let job: VmJob = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if job.schema_version != crate::VM_JOB_SCHEMA_VERSION {
            continue;
        }

        if now_unix_ms < job.deadline_unix_ms {
            continue;
        }

        let _ = reap_job_best_effort(&job, &path);
        let _ = std::fs::write(path.join("reaped"), b"reaped\n");
        reaped += 1;
    }

    Ok(reaped)
}

fn reap_job_best_effort(job: &VmJob, state_dir: &Path) -> Result<()> {
    match job.backend {
        VmBackend::Vz => {
            if let Some(pid) = job.pid {
                hard_kill_pid_and_group(pid);
            }
            let _ = vz_cleanup_scratch(state_dir);
        }
        VmBackend::AppleContainer => {
            let _ = apple_container_hard_kill(&job.container_id);
            let _ = apple_container_cleanup(&job.container_id);
        }
        VmBackend::Docker => {
            let _ = crate::docker_hard_kill(&job.container_id);
            let _ = crate::docker_cleanup(&job.container_id);
        }
        VmBackend::Podman => {
            let _ = crate::podman_hard_kill(&job.container_id);
            let _ = crate::podman_cleanup(&job.container_id);
        }
        VmBackend::FirecrackerCtr => {
            let cfg = job
                .ctr
                .as_ref()
                .map(firecracker_ctr_config_from_job)
                .unwrap_or_else(firecracker_ctr_config_from_env);
            let _ = firecracker_ctr_hard_kill(&cfg, &job.container_id);
            let _ = firecracker_ctr_cleanup(&cfg, &job.container_id);
        }
    }
    Ok(())
}

fn sweep_apple_container_runtime_best_effort(now_unix_ms: u64) -> Result<usize> {
    if !cfg!(target_os = "macos") {
        return Ok(0);
    }

    let mut cmd = std::process::Command::new("container");
    cmd.args(["list", "--all", "--format", "json"]);
    let out = crate::run_command_capped(cmd, 2_000, 256 * 1024, 256 * 1024)?;
    if out.timed_out || out.exit_status != 0 {
        return Ok(0);
    }

    let s = String::from_utf8_lossy(&out.stdout);
    let owned = match parse_apple_container_json_owned(&s) {
        Ok(v) => v,
        Err(_) => return Ok(0),
    };

    let mut reaped: usize = 0;
    for c in owned {
        let Some(deadline_ms) = parse_deadline_label(&c.labels) else {
            continue;
        };
        if now_unix_ms < deadline_ms {
            continue;
        }

        let _ = apple_container_hard_kill(&c.id);
        let _ = apple_container_cleanup(&c.id);
        reaped += 1;
    }

    Ok(reaped)
}

fn sweep_firecracker_runtime_best_effort(
    now_unix_ms: u64,
    cfg: &FirecrackerCtrConfig,
) -> Result<usize> {
    if !cfg!(target_os = "linux") {
        return Ok(0);
    }

    let mut cmd = std::process::Command::new(&cfg.bin);
    cmd.args(crate::ctr_base_args(cfg));
    cmd.arg("--timeout").arg("2s");
    cmd.args(["containers", "list", "-q"]);
    let out = crate::run_command_capped(cmd, 2_000, 256 * 1024, 256 * 1024)?;
    if out.timed_out || out.exit_status != 0 {
        return Ok(0);
    }

    let ids = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .take(512)
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    let mut reaped: usize = 0;
    for id in ids {
        let mut info_cmd = std::process::Command::new(&cfg.bin);
        info_cmd.args(crate::ctr_base_args(cfg));
        info_cmd.arg("--timeout").arg("2s");
        info_cmd.args(["containers", "info"]);
        info_cmd.arg(&id);
        let info = crate::run_command_capped(info_cmd, 2_000, 256 * 1024, 256 * 1024)?;
        if info.timed_out || info.exit_status != 0 {
            continue;
        }

        let s = String::from_utf8_lossy(&info.stdout);
        let owned = match parse_ctr_container_info_json_owned(&s) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(owned) = owned else {
            continue;
        };
        let Some(deadline_ms) = parse_deadline_label(&owned.labels) else {
            continue;
        };
        if now_unix_ms < deadline_ms {
            continue;
        }

        let _ = firecracker_ctr_hard_kill(cfg, &id);
        let _ = firecracker_ctr_cleanup(cfg, &id);
        reaped += 1;
    }

    Ok(reaped)
}

fn parse_deadline_label(labels: &crate::Labels) -> Option<u64> {
    labels
        .get(X07_LABEL_DEADLINE_UNIX_MS_KEY)
        .and_then(|v| v.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let base = std::env::temp_dir();
            let pid = std::process::id();

            for _ in 0..256 {
                let attempt_id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time since epoch")
                    .as_nanos();

                let mut path = base.clone();
                path.push(format!("{prefix}_{pid}_{nanos}_{attempt_id}"));

                match std::fs::create_dir(&path) {
                    Ok(()) => return Self { path },
                    Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                    Err(e) => panic!("create temp dir {path:?}: {e}"),
                }
            }

            panic!("failed to create unique temp dir");
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn sweep_reaps_expired_job_state_dir() {
        let tmp = TempDir::new("x07_vm_sweep");
        let state_root = &tmp.path;

        let now = now_unix_ms().unwrap();
        let job_dir = state_root.join("job1");
        std::fs::create_dir_all(&job_dir).unwrap();

        let job = VmJob {
            schema_version: crate::VM_JOB_SCHEMA_VERSION.to_string(),
            run_id: "job1".to_string(),
            backend: VmBackend::Vz,
            container_id: "x07-job1".to_string(),
            pid: None,
            created_unix_ms: now.saturating_sub(10_000),
            deadline_unix_ms: now.saturating_sub(1),
            grace_ms: 1,
            cleanup_ms: 1,
            ctr: None,
        };

        let mut bytes = serde_json::to_vec_pretty(&job).unwrap();
        bytes.push(b'\n');
        std::fs::write(job_dir.join("job.json"), bytes).unwrap();

        let report = sweep_orphans_best_effort(state_root, VmBackend::Vz, None).unwrap();
        assert_eq!(report.state_reaped, 1);
        assert!(job_dir.join("reaped").is_file());
    }

    #[test]
    fn sweep_skips_unexpired_job_state_dir() {
        let tmp = TempDir::new("x07_vm_sweep");
        let state_root = &tmp.path;

        let now = now_unix_ms().unwrap();
        let job_dir = state_root.join("job2");
        std::fs::create_dir_all(&job_dir).unwrap();

        let job = VmJob {
            schema_version: crate::VM_JOB_SCHEMA_VERSION.to_string(),
            run_id: "job2".to_string(),
            backend: VmBackend::Vz,
            container_id: "x07-job2".to_string(),
            pid: None,
            created_unix_ms: now,
            deadline_unix_ms: now.saturating_add(60_000),
            grace_ms: 1,
            cleanup_ms: 1,
            ctr: None,
        };

        let mut bytes = serde_json::to_vec_pretty(&job).unwrap();
        bytes.push(b'\n');
        std::fs::write(job_dir.join("job.json"), bytes).unwrap();

        let report = sweep_orphans_best_effort(state_root, VmBackend::Vz, None).unwrap();
        assert_eq!(report.state_reaped, 0);
        assert!(!job_dir.join("reaped").exists());
    }
}
