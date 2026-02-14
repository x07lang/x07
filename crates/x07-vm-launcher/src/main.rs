use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use x07_runner_common::os_policy;
use x07_vm::{
    default_cleanup_ms, default_grace_ms, firecracker_ctr_config_from_env, resolve_sibling_or_path,
    resolve_vm_backend, run_vm_job, LimitsSpec, MountSpec, NetworkMode, RunSpec, VmBackend,
    VmJobRunParams, ENV_VZ_GUEST_BUNDLE,
};

#[derive(Debug, Clone, Deserialize)]
struct VmBundleManifest {
    schema_version: String,
    backend: String,
    guest_image: String,
    guest_digest: String,
    payload: String,
    workdir: String,
    policy: String,
}

fn main() -> std::process::ExitCode {
    match try_main() {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln_stderr(format!("{err:#}\n"));
            std::process::ExitCode::from(2)
        }
    }
}

fn try_main() -> Result<std::process::ExitCode> {
    let exe = std::env::current_exe().context("current_exe")?;
    let sidecar = sidecar_dir_for_exe(&exe);
    let manifest_path = sidecar.join("manifest.json");
    let manifest: VmBundleManifest = serde_json::from_slice(
        &std::fs::read(&manifest_path)
            .with_context(|| format!("read vm bundle manifest: {}", manifest_path.display()))?,
    )
    .with_context(|| format!("parse vm bundle manifest: {}", manifest_path.display()))?;

    if manifest.schema_version != "x07.vm.bundle.manifest@0.2.0" {
        anyhow::bail!(
            "unsupported vm bundle manifest schema_version: {:?}",
            manifest.schema_version
        );
    }

    let policy_path = sidecar.join(&manifest.policy);
    let policy_bytes = std::fs::read(&policy_path)
        .with_context(|| format!("read policy: {}", policy_path.display()))?;
    let policy: os_policy::Policy = serde_json::from_slice(&policy_bytes)
        .with_context(|| format!("parse policy JSON: {}", policy_path.display()))?;
    policy
        .validate_basic()
        .map_err(|e| anyhow::anyhow!("policy invalid: {e}"))?;

    let accept_weaker_isolation = x07_vm::read_accept_weaker_isolation_env().unwrap_or(false);

    let manifest_backend: VmBackend = manifest.backend.parse()?;
    let backend = match std::env::var(x07_vm::ENV_VM_BACKEND).ok() {
        Some(raw) => {
            let b: VmBackend = raw.parse()?;
            if b != manifest_backend && !accept_weaker_isolation {
                anyhow::bail!(
                    "VM bundle requires backend {manifest_backend}, but {}={b} is set.\n\nfix:\n  - unset {}, or\n  - set {}=1 to override backend selection",
                    x07_vm::ENV_VM_BACKEND,
                    x07_vm::ENV_VM_BACKEND,
                    x07_vm::ENV_ACCEPT_WEAKER_ISOLATION,
                );
            }
            b
        }
        None => manifest_backend,
    };
    std::env::set_var(x07_vm::ENV_VM_BACKEND, backend.to_string());
    let backend = resolve_vm_backend()?;

    let guest_image_override = std::env::var("X07_VM_GUEST_IMAGE").ok();
    let guest_image = if backend == VmBackend::Vz {
        std::env::var(ENV_VZ_GUEST_BUNDLE).unwrap_or_default()
    } else if let Some(override_ref) = guest_image_override {
        if !accept_weaker_isolation {
            anyhow::bail!(
                "VM bundle does not allow overriding X07_VM_GUEST_IMAGE without {}=1",
                x07_vm::ENV_ACCEPT_WEAKER_ISOLATION
            );
        }
        override_ref
    } else {
        manifest.guest_image.clone()
    };

    if guest_image.trim().is_empty() {
        anyhow::bail!("missing guest image (bundle manifest and environment are empty)");
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let created_unix_ms = now_unix_ms()?;
    let run_id = {
        let pid = std::process::id();
        format!("{created_unix_ms}-{pid}")
    };

    let wall_ms = policy.limits.wall_ms.max(1);
    let deadline_unix_ms = created_unix_ms.saturating_add(wall_ms);
    let grace_ms = default_grace_ms(wall_ms);
    let cleanup_ms = default_cleanup_ms();

    let state_root = x07_vm::default_vm_state_root()?;
    let state_dir = state_root.join(&run_id);

    let job_in = state_dir.join("in");
    let job_out = state_dir.join("out");
    std::fs::create_dir_all(&job_in)
        .with_context(|| format!("create job input dir: {}", job_in.display()))?;
    std::fs::create_dir_all(&job_out)
        .with_context(|| format!("create job output dir: {}", job_out.display()))?;

    std::fs::write(job_in.join("policy.json"), &policy_bytes).context("write policy.json")?;

    let mut mounts: Vec<MountSpec> = vec![
        MountSpec {
            host_path: job_in.clone(),
            guest_path: PathBuf::from("/x07/in"),
            readonly: true,
        },
        MountSpec {
            host_path: job_out.clone(),
            guest_path: PathBuf::from("/x07/out"),
            readonly: false,
        },
        MountSpec {
            host_path: sidecar.clone(),
            guest_path: PathBuf::from("/x07/bundle"),
            readonly: true,
        },
    ];

    x07_vm::append_root_mounts(
        &mut mounts,
        &policy.fs.read_roots,
        &policy.fs.write_roots,
        &cwd,
        Path::new(&manifest.workdir),
    )?;

    let mut guest_argv: Vec<String> = Vec::new();
    guest_argv.push(
        PathBuf::from("/x07/bundle")
            .join(&manifest.payload)
            .display()
            .to_string(),
    );
    for a in std::env::args().skip(1) {
        guest_argv.push(a);
    }

    let allowlist_requested = policy.net.enabled && !policy.net.allow_hosts.is_empty();
    let network_mode = if allowlist_requested {
        if backend == VmBackend::Vz || accept_weaker_isolation {
            NetworkMode::Default
        } else {
            anyhow::bail!(
                "VM backend {backend} does not yet enforce policy.net.allow_hosts at the VM boundary.\n\nfix:\n  - use the VZ backend (macOS): X07_VM_BACKEND=vz, or\n  - set X07_I_ACCEPT_WEAKER_ISOLATION=1 to allow networking without VM-boundary allowlist enforcement"
            );
        }
    } else {
        NetworkMode::None
    };

    let firecracker_cfg = if backend == VmBackend::FirecrackerCtr {
        Some(firecracker_ctr_config_from_env())
    } else {
        None
    };
    if !accept_weaker_isolation {
        x07_vm::verify_vm_guest_digest(
            backend,
            &guest_image,
            &manifest.guest_digest,
            firecracker_cfg.as_ref(),
        )?;
    }

    let limits = LimitsSpec {
        wall_ms,
        grace_ms,
        cleanup_ms,
        mem_bytes: Some(policy.limits.mem_bytes),
        vcpus: None,
        max_stdout_bytes: 64 * 1024 * 1024,
        max_stderr_bytes: 64 * 1024 * 1024,
        network: network_mode,
    };

    let spec = RunSpec {
        run_id: run_id.clone(),
        backend,
        image: guest_image,
        image_digest: Some(manifest.guest_digest.clone()),
        argv: guest_argv,
        env: BTreeMap::new(),
        mounts,
        workdir: Some(PathBuf::from(&manifest.workdir)),
        limits,
    };

    let reaper_bin = resolve_reaper(&exe, &sidecar);

    let out = run_vm_job(
        &spec,
        VmJobRunParams {
            state_root: &state_root,
            state_dir: &state_dir,
            reaper_bin: &reaper_bin,
            created_unix_ms,
            deadline_unix_ms,
            firecracker_cfg: firecracker_cfg.as_ref(),
        },
    )?;

    if !out.stderr.is_empty() {
        let _ = std::io::Write::write_all(&mut std::io::stderr(), &out.stderr);
    }
    if !out.stdout.is_empty() {
        std::io::Write::write_all(&mut std::io::stdout(), &out.stdout).context("write stdout")?;
    }

    Ok(std::process::ExitCode::from(
        out.exit_status.clamp(0, 255) as u8
    ))
}

fn sidecar_dir_for_exe(exe: &Path) -> PathBuf {
    PathBuf::from(format!("{}.vm", exe.display()))
}

fn resolve_reaper(exe: &Path, sidecar: &Path) -> PathBuf {
    let sibling = resolve_sibling_or_path("x07-vm-reaper");
    if sibling.is_file() {
        return sibling;
    }

    let in_sidecar = sidecar.join("x07-vm-reaper");
    if in_sidecar.is_file() {
        return in_sidecar;
    }

    let deps_sidecar = sidecar.join("deps").join("x07-vm-reaper");
    if deps_sidecar.is_file() {
        return deps_sidecar;
    }

    if let Some(parent) = exe.parent() {
        let deps = parent.join("deps").join("x07-vm-reaper");
        if deps.is_file() {
            return deps;
        }
    }

    PathBuf::from("x07-vm-reaper")
}

fn now_unix_ms() -> Result<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(d.as_millis().try_into().unwrap_or(u64::MAX))
}

fn writeln_stderr(msg: String) -> std::io::Result<()> {
    use std::io::Write;
    let mut stderr = std::io::stderr();
    stderr.write_all(msg.as_bytes())?;
    stderr.flush()
}
