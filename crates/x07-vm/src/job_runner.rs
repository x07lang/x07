use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    apple_container_cleanup, apple_container_hard_kill, container_id_from_run_id, docker_cleanup,
    docker_hard_kill, firecracker_ctr_cleanup, firecracker_ctr_config_from_env,
    firecracker_ctr_hard_kill, podman_cleanup, podman_hard_kill, run_apple_container, run_docker,
    run_firecracker_ctr, run_podman, spawn_reaper, spawn_vz_helper, sweep_orphans_best_effort,
    touch_done_marker, vz_cleanup_scratch, wait_child_output_capped, write_job_file, x07_label_set,
    CtrJob, FirecrackerCtrConfig, RunOutput, RunSpec, VmBackend, VmCaps, VmJob,
};

pub struct VmJobRunParams<'a> {
    pub state_root: &'a Path,
    pub state_dir: &'a Path,
    pub reaper_bin: &'a Path,
    pub created_unix_ms: u64,
    pub deadline_unix_ms: u64,
    pub firecracker_cfg: Option<&'a FirecrackerCtrConfig>,
}

pub trait VmDriver {
    fn run_job(&self, spec: &RunSpec, params: VmJobRunParams<'_>) -> Result<RunOutput>;

    fn capabilities(&self) -> VmCaps;
}

#[derive(Debug, Clone, Copy)]
pub struct DefaultVmDriver {
    backend: VmBackend,
}

impl DefaultVmDriver {
    pub fn new(backend: VmBackend) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> VmBackend {
        self.backend
    }
}

impl Default for DefaultVmDriver {
    fn default() -> Self {
        let backend = if cfg!(target_os = "macos") {
            VmBackend::Vz
        } else if cfg!(target_os = "linux") {
            VmBackend::FirecrackerCtr
        } else {
            VmBackend::Vz
        };
        Self { backend }
    }
}

impl VmDriver for DefaultVmDriver {
    fn run_job(&self, spec: &RunSpec, params: VmJobRunParams<'_>) -> Result<RunOutput> {
        if spec.backend != self.backend {
            anyhow::bail!(
                "DefaultVmDriver backend mismatch: driver={} spec={}",
                self.backend,
                spec.backend
            );
        }
        run_vm_job(spec, params)
    }

    fn capabilities(&self) -> VmCaps {
        VmCaps::for_backend(self.backend)
    }
}

pub fn run_vm_job(spec: &RunSpec, params: VmJobRunParams<'_>) -> Result<RunOutput> {
    let container_id = container_id_from_run_id(&spec.run_id)?;

    let job_file = params.state_dir.join("job.json");
    let done_marker = params.state_dir.join("done");

    let labels = x07_label_set(
        params.state_root,
        &spec.run_id,
        spec.backend,
        params.created_unix_ms,
        params.deadline_unix_ms,
    )?;

    let firecracker_cfg = if spec.backend == VmBackend::FirecrackerCtr {
        Some(
            params
                .firecracker_cfg
                .cloned()
                .unwrap_or_else(firecracker_ctr_config_from_env),
        )
    } else {
        None
    };

    let _ = sweep_orphans_best_effort(params.state_root, spec.backend, firecracker_cfg.as_ref());

    let grace_ms = spec.limits.grace_ms;
    let cleanup_ms = spec.limits.cleanup_ms;

    let out = match spec.backend {
        VmBackend::Vz => {
            let spawned = spawn_vz_helper(spec, params.state_dir)?;

            let job = VmJob {
                schema_version: crate::VM_JOB_SCHEMA_VERSION.to_string(),
                run_id: spec.run_id.clone(),
                backend: spec.backend,
                container_id: container_id.clone(),
                pid: Some(spawned.pid),
                created_unix_ms: params.created_unix_ms,
                deadline_unix_ms: params.deadline_unix_ms,
                grace_ms,
                cleanup_ms,
                ctr: None,
            };
            write_job_file(&job_file, &job)?;
            spawn_reaper(params.reaper_bin, &job_file)?;

            let out = wait_child_output_capped(
                spawned.child,
                spec.limits.wall_ms,
                spec.limits.max_stdout_bytes,
                spec.limits.max_stderr_bytes,
            )?;
            let _ = vz_cleanup_scratch(params.state_dir);
            out
        }

        VmBackend::AppleContainer => {
            let job = VmJob {
                schema_version: crate::VM_JOB_SCHEMA_VERSION.to_string(),
                run_id: spec.run_id.clone(),
                backend: spec.backend,
                container_id: container_id.clone(),
                pid: None,
                created_unix_ms: params.created_unix_ms,
                deadline_unix_ms: params.deadline_unix_ms,
                grace_ms,
                cleanup_ms,
                ctr: None,
            };
            write_job_file(&job_file, &job)?;
            spawn_reaper(params.reaper_bin, &job_file)?;
            run_apple_container(spec, &container_id, &labels)?
        }

        VmBackend::Docker => {
            let job = VmJob {
                schema_version: crate::VM_JOB_SCHEMA_VERSION.to_string(),
                run_id: spec.run_id.clone(),
                backend: spec.backend,
                container_id: container_id.clone(),
                pid: None,
                created_unix_ms: params.created_unix_ms,
                deadline_unix_ms: params.deadline_unix_ms,
                grace_ms,
                cleanup_ms,
                ctr: None,
            };
            write_job_file(&job_file, &job)?;
            spawn_reaper(params.reaper_bin, &job_file)?;
            run_docker(spec, &container_id, &labels)?
        }

        VmBackend::Podman => {
            let job = VmJob {
                schema_version: crate::VM_JOB_SCHEMA_VERSION.to_string(),
                run_id: spec.run_id.clone(),
                backend: spec.backend,
                container_id: container_id.clone(),
                pid: None,
                created_unix_ms: params.created_unix_ms,
                deadline_unix_ms: params.deadline_unix_ms,
                grace_ms,
                cleanup_ms,
                ctr: None,
            };
            write_job_file(&job_file, &job)?;
            spawn_reaper(params.reaper_bin, &job_file)?;
            run_podman(spec, &container_id, &labels)?
        }

        VmBackend::FirecrackerCtr => {
            let cfg = firecracker_cfg
                .as_ref()
                .context("internal error: firecracker cfg missing")?;

            let job = VmJob {
                schema_version: crate::VM_JOB_SCHEMA_VERSION.to_string(),
                run_id: spec.run_id.clone(),
                backend: spec.backend,
                container_id: container_id.clone(),
                pid: None,
                created_unix_ms: params.created_unix_ms,
                deadline_unix_ms: params.deadline_unix_ms,
                grace_ms,
                cleanup_ms,
                ctr: Some(CtrJob {
                    bin: cfg.bin.to_string_lossy().to_string(),
                    address: cfg.address.clone(),
                    namespace: cfg.namespace.clone(),
                }),
            };
            write_job_file(&job_file, &job)?;
            spawn_reaper(params.reaper_bin, &job_file)?;

            run_firecracker_ctr(spec, cfg, &container_id, &labels)?
        }
    };

    if out.timed_out {
        match spec.backend {
            VmBackend::Vz => {
                let _ = vz_cleanup_scratch(params.state_dir);
            }
            VmBackend::AppleContainer => {
                let _ = apple_container_hard_kill(&container_id);
                let _ = apple_container_cleanup(&container_id);
            }
            VmBackend::Docker => {
                let _ = docker_hard_kill(&container_id);
                let _ = docker_cleanup(&container_id);
            }
            VmBackend::Podman => {
                let _ = podman_hard_kill(&container_id);
                let _ = podman_cleanup(&container_id);
            }
            VmBackend::FirecrackerCtr => {
                let cfg = firecracker_cfg
                    .as_ref()
                    .context("internal error: firecracker cfg missing")?;
                let _ = firecracker_ctr_hard_kill(cfg, &container_id);
                let _ = firecracker_ctr_cleanup(cfg, &container_id);
            }
        }
    } else {
        match spec.backend {
            VmBackend::Vz => {
                let _ = vz_cleanup_scratch(params.state_dir);
            }
            VmBackend::AppleContainer => {
                let _ = apple_container_cleanup(&container_id);
            }
            VmBackend::Docker => {
                let _ = docker_cleanup(&container_id);
            }
            VmBackend::Podman => {
                let _ = podman_cleanup(&container_id);
            }
            VmBackend::FirecrackerCtr => {
                let cfg = firecracker_cfg
                    .as_ref()
                    .context("internal error: firecracker cfg missing")?;
                let _ = firecracker_ctr_cleanup(cfg, &container_id);
            }
        }
    }

    touch_done_marker(&done_marker)?;
    Ok(out)
}
