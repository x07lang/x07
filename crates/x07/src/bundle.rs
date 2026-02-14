use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use base64::Engine;
use clap::Args;
use serde::Serialize;
use serde_json::Value;
use x07_contracts::{
    PROJECT_LOCKFILE_SCHEMA_VERSION, X07_BUNDLE_REPORT_SCHEMA_VERSION,
    X07_HOST_RUNNER_REPORT_SCHEMA_VERSION,
};
use x07_host_runner::{apply_cc_profile, CcProfile, NativeCliWrapperOpts, NativeToolchainConfig};
use x07_runner_common::sandbox_backend::{
    resolve_sandbox_backend, EffectiveSandboxBackend, SandboxBackend,
};
use x07_runner_common::{auto_ffi, os_env, os_paths, os_policy};
use x07_vm::{
    default_cleanup_ms, default_grace_ms, firecracker_ctr_config_from_env,
    resolve_sibling_or_path as resolve_sibling_or_path_vm, resolve_vm_backend, LimitsSpec,
    MountSpec, NetworkMode, RunSpec, VmBackend,
};
use x07_worlds::WorldId;
use x07c::project;

use crate::policy_overrides::{PolicyOverrides, PolicyResolution};
use crate::repair::RepairArgs;

const DEFAULT_SOLVE_FUEL: u64 = 50_000_000;
const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;

static VM_RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Args)]
pub struct BundleArgs {
    /// Project manifest path (`x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Compile a single `*.x07.json` file (expert mode; requires --module-root).
    #[arg(long, value_name = "PATH")]
    pub program: Option<PathBuf>,

    /// Run profile name (resolved from `x07.json.profiles`).
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Override the resolved world (advanced; prefer `--profile`).
    #[arg(long, value_enum, hide = true)]
    pub world: Option<WorldId>,

    /// Policy JSON (required for `run-os-sandboxed`; not a hardened sandbox).
    #[arg(long, value_name = "PATH")]
    pub policy: Option<PathBuf>,

    /// Sandbox backend selection (run-os-sandboxed defaults to "vm").
    #[arg(long, value_enum)]
    pub sandbox_backend: Option<SandboxBackend>,

    /// Required to bundle run-os-sandboxed without a VM boundary.
    #[arg(long)]
    pub i_accept_weaker_isolation: bool,

    /// Append network destinations to the sandbox policy allowlist (repeatable).
    #[arg(long, value_name = "HOST:PORT[,PORT...]")]
    pub allow_host: Vec<String>,

    /// Read allow-host entries from a file (repeatable).
    #[arg(long, value_name = "PATH")]
    pub allow_host_file: Vec<PathBuf>,

    /// Remove network destinations from the sandbox policy allowlist (repeatable; deny wins).
    #[arg(long, value_name = "HOST:*|HOST:PORT[,PORT...]")]
    pub deny_host: Vec<String>,

    /// Read deny-host entries from a file (repeatable).
    #[arg(long, value_name = "PATH")]
    pub deny_host_file: Vec<PathBuf>,

    /// Append a sandbox filesystem read root (repeatable).
    #[arg(long, value_name = "PATH")]
    pub allow_read_root: Vec<String>,

    /// Append a sandbox filesystem write root (repeatable).
    #[arg(long, value_name = "PATH")]
    pub allow_write_root: Vec<String>,

    #[arg(long, value_enum)]
    pub cc_profile: Option<CcProfile>,

    /// Override the generated C source budget (in bytes).
    #[arg(long, value_name = "BYTES")]
    pub max_c_bytes: Option<usize>,

    #[arg(long, value_name = "BYTES")]
    pub max_memory_bytes: Option<usize>,

    #[arg(long, value_name = "BYTES")]
    pub max_output_bytes: Option<usize>,

    #[arg(long, value_name = "N")]
    pub cpu_time_limit_seconds: Option<u64>,

    #[arg(long)]
    pub debug_borrow_checks: bool,

    /// For OS worlds: collect and apply C FFI flags from dependency packages.
    #[arg(long, conflicts_with = "no_auto_ffi")]
    pub auto_ffi: bool,

    /// For OS worlds: disable automatic C FFI collection.
    #[arg(long, conflicts_with = "auto_ffi")]
    pub no_auto_ffi: bool,

    /// Emit intermediate C sources and report JSON for debugging/CI.
    #[arg(long, value_name = "DIR", alias = "emit")]
    pub emit_dir: Option<PathBuf>,

    /// Module root directory for resolving module ids (required for --program).
    /// May be passed multiple times.
    #[arg(long, value_name = "DIR")]
    pub module_root: Vec<PathBuf>,

    #[command(flatten)]
    pub repair: RepairArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Project,
    Program,
}

impl TargetKind {
    fn as_str(self) -> &'static str {
        match self {
            TargetKind::Project => "project",
            TargetKind::Program => "program",
        }
    }
}

#[derive(Debug, Serialize)]
struct BundleTarget {
    kind: &'static str,
    path: String,
    project_root: Option<String>,
    lockfile: Option<String>,
    resolved_module_roots: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BundleAbi {
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct BundlePolicy {
    base_policy: String,
    effective_policy: String,
    embedded_env_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BundleSection {
    out: String,
    name: String,
    abi: BundleAbi,
    policy: Option<BundlePolicy>,
    emit_dir: Option<String>,
    kind: &'static str,
    guest: Option<BundleGuest>,
}

#[derive(Debug, Serialize)]
struct BundleGuestPlatform {
    os: &'static str,
    arch: &'static str,
}

#[derive(Debug, Serialize)]
struct BundleGuestImage {
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    ref_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout: Option<String>,
}

#[derive(Debug, Serialize)]
struct BundleGuestMounts {
    job_in: String,
    job_out: String,
}

#[derive(Debug, Serialize)]
struct BundleGuest {
    contract: &'static str,
    platform: BundleGuestPlatform,
    image: BundleGuestImage,
    entrypoint: Vec<String>,
    workdir: String,
    mounts: BundleGuestMounts,
}

#[derive(Debug, Serialize)]
struct BundleReport {
    schema_version: &'static str,
    runner: &'static str,
    world: &'static str,
    target: BundleTarget,
    bundle: BundleSection,
    report: Value,
}

#[derive(Debug)]
struct PreparedTarget {
    program_bytes: Vec<u8>,
    lockfile: Option<PathBuf>,
    module_roots: Vec<PathBuf>,
    extra_cc_args: Vec<String>,
}

#[derive(Debug)]
struct ResolvedPolicyEnv {
    base_policy: Option<PathBuf>,
    env_pairs: Vec<(String, String)>,
    embedded_env_keys: Vec<String>,
    allow_unsafe: Option<bool>,
    allow_ffi: Option<bool>,
}

pub fn cmd_bundle(
    machine: &crate::reporting::MachineArgs,
    args: BundleArgs,
) -> Result<std::process::ExitCode> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let (target_kind, target_path, project_manifest) = resolve_target(&cwd, &args)?;

    if !args.module_root.is_empty() && target_kind != TargetKind::Program {
        anyhow::bail!("--module-root is only valid with --program");
    }
    if target_kind == TargetKind::Program && args.module_root.is_empty() {
        anyhow::bail!("--program requires explicit --module-root");
    }

    let project_root = project_manifest.as_deref().map(|manifest| {
        let abs = if manifest.is_absolute() {
            manifest.to_path_buf()
        } else {
            cwd.join(manifest)
        };
        abs.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(&cwd)
            .to_path_buf()
    });
    let policy_root = project_root.as_deref().unwrap_or(&cwd);

    let profiles_file = match project_manifest.as_deref() {
        Some(path) => Some(crate::run::load_project_profiles(path)?),
        None => None,
    };
    let selected_profile = crate::run::resolve_selected_profile(
        project_manifest.as_deref(),
        profiles_file.as_ref(),
        args.profile.as_deref(),
    )?;

    let world = resolve_world(
        &args,
        project_manifest.as_deref(),
        selected_profile.as_ref(),
    )?;

    if world != WorldId::RunOsSandboxed
        && (args.sandbox_backend.is_some() || args.i_accept_weaker_isolation)
    {
        anyhow::bail!(
            "--sandbox-backend/--i-accept-weaker-isolation are only supported for --world run-os-sandboxed"
        );
    }

    let sandbox_backend = if world == WorldId::RunOsSandboxed {
        Some(resolve_sandbox_backend(
            world,
            args.sandbox_backend,
            args.i_accept_weaker_isolation,
        )?)
    } else {
        None
    };

    let cc_profile = args
        .cc_profile
        .or(selected_profile.as_ref().and_then(|p| p.cc_profile))
        .unwrap_or(CcProfile::Default);
    apply_cc_profile(cc_profile);

    if let Some(max_c_bytes) = args.max_c_bytes {
        std::env::set_var("X07_MAX_C_BYTES", max_c_bytes.to_string());
    }

    let overrides = PolicyOverrides {
        allow_host: args.allow_host.clone(),
        allow_host_file: args.allow_host_file.clone(),
        deny_host: args.deny_host.clone(),
        deny_host_file: args.deny_host_file.clone(),
        allow_read_root: args.allow_read_root.clone(),
        allow_write_root: args.allow_write_root.clone(),
    };

    let profile_policy = selected_profile.as_ref().and_then(|p| p.policy.clone());
    let policy_resolution = crate::policy_overrides::resolve_policy_for_world(
        world,
        policy_root,
        args.policy.clone(),
        profile_policy.clone(),
        &overrides,
    )?;
    let effective_policy = match &policy_resolution {
        PolicyResolution::None => None,
        PolicyResolution::Base(p) => Some(p.clone()),
        PolicyResolution::Derived { derived, .. } => Some(derived.clone()),
        PolicyResolution::SchemaInvalid(errors) => {
            crate::policy_overrides::print_policy_schema_x07diag_stderr(errors.clone());
            return Ok(std::process::ExitCode::from(3));
        }
    };

    if let PolicyResolution::Derived { derived, .. } = &policy_resolution {
        let msg = format!("x07 bundle: using derived policy {}\n", derived.display());
        let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
    }

    let ResolvedPolicyEnv {
        base_policy,
        env_pairs: policy_env_pairs,
        embedded_env_keys: policy_embedded_keys,
        allow_unsafe: policy_allow_unsafe,
        allow_ffi: policy_allow_ffi,
    } = resolve_policy_env(
        world,
        policy_root,
        args.policy.as_deref(),
        profile_policy.as_deref(),
        effective_policy.as_deref(),
    )?;

    let out = machine
        .out
        .as_ref()
        .context("missing --out <PATH> for output executable")?;
    let out_path = resolve_out_path(out, target_kind, &project_root);
    let out_path = normalize_exe_extension(out_path);
    let bundle_name = out_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let PreparedTarget {
        program_bytes,
        lockfile,
        module_roots,
        extra_cc_args,
    } = match target_kind {
        TargetKind::Project => {
            prepare_project_target(&target_path, world, &args, selected_profile.as_ref())?
        }
        TargetKind::Program => prepare_program_target(&target_path, world, &args)?,
    };

    let resolved_module_roots = module_roots
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>();

    let solve_fuel = selected_profile
        .as_ref()
        .and_then(|p| p.solve_fuel)
        .unwrap_or(DEFAULT_SOLVE_FUEL);
    let max_memory_bytes = args
        .max_memory_bytes
        .or(selected_profile.as_ref().and_then(|p| p.max_memory_bytes))
        .unwrap_or(DEFAULT_MAX_MEMORY_BYTES);
    let max_output_bytes = args
        .max_output_bytes
        .or(selected_profile.as_ref().and_then(|p| p.max_output_bytes));
    let cpu_time_limit_seconds = args.cpu_time_limit_seconds.or(selected_profile
        .as_ref()
        .and_then(|p| p.cpu_time_limit_seconds));

    if sandbox_backend == Some(EffectiveSandboxBackend::Vm) {
        let base_policy = base_policy
            .as_deref()
            .context("internal error: base_policy missing")?;
        let effective_policy = effective_policy
            .as_deref()
            .context("internal error: effective_policy missing")?;

        return cmd_bundle_vm(CmdBundleVmParams {
            args: &args,
            cwd: &cwd,
            world,
            target_kind,
            target_path: &target_path,
            project_root: project_root.as_deref(),
            out_path: &out_path,
            bundle_name: &bundle_name,
            base_policy,
            effective_policy,
            embedded_env_keys: &policy_embedded_keys,
            resolved_module_roots: &resolved_module_roots,
            lockfile: lockfile.as_deref(),
            max_memory_bytes,
            max_output_bytes,
            cpu_time_limit_seconds,
        });
    }

    let mut compile_options = x07c::world_config::compile_options_for_world(world, module_roots);
    compile_options.arch_root = project_root.clone();
    if world == WorldId::RunOsSandboxed {
        compile_options.allow_unsafe = policy_allow_unsafe;
        compile_options.allow_ffi = policy_allow_ffi;
    }

    let toolchain = NativeToolchainConfig {
        world_tag: world.as_str().to_string(),
        fuel_init: solve_fuel,
        mem_cap_bytes: max_memory_bytes,
        debug_borrow_checks: args.debug_borrow_checks,
        enable_fs: compile_options.enable_fs,
        enable_rr: compile_options.enable_rr,
        enable_kv: compile_options.enable_kv,
        extra_cc_args,
    };

    let wrapper = NativeCliWrapperOpts {
        argv0: bundle_name.clone(),
        env: policy_env_pairs,
        max_output_bytes: max_output_bytes.and_then(|v| u32::try_from(v).ok()),
        cpu_time_limit_seconds,
    };

    let compile_out = x07_host_runner::compile_bundle_exe(
        &program_bytes,
        &compile_options,
        &toolchain,
        &out_path,
        &wrapper,
    )?;

    let host_report_mode = match target_kind {
        TargetKind::Project => "project-compile",
        TargetKind::Program => "compile",
    };
    let host_report = host_compile_report_json(host_report_mode, &compile_out.compile)?;

    let report = BundleReport {
        schema_version: X07_BUNDLE_REPORT_SCHEMA_VERSION,
        runner: "host",
        world: world.as_str(),
        target: BundleTarget {
            kind: target_kind.as_str(),
            path: target_path.display().to_string(),
            project_root: project_root.as_ref().map(|p| p.display().to_string()),
            lockfile: lockfile.as_ref().map(|p| p.display().to_string()),
            resolved_module_roots,
        },
        bundle: BundleSection {
            out: out_path.display().to_string(),
            name: bundle_name,
            abi: BundleAbi { kind: "argv_v1" },
            policy: base_policy.map(|base_policy| BundlePolicy {
                base_policy: base_policy.display().to_string(),
                effective_policy: effective_policy
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| base_policy.display().to_string()),
                embedded_env_keys: policy_embedded_keys,
            }),
            emit_dir: args.emit_dir.as_ref().map(|p| p.display().to_string()),
            kind: "native",
            guest: None,
        },
        report: host_report,
    };

    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');

    if let Some(dir) = &args.emit_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("create emit dir: {}", dir.display()))?;

        std::fs::write(dir.join("report.json"), &bytes)
            .with_context(|| format!("write emit report.json under {}", dir.display()))?;
        std::fs::write(
            dir.join("program.freestanding.c"),
            compile_out.freestanding_c.as_bytes(),
        )
        .with_context(|| format!("write emit program.freestanding.c under {}", dir.display()))?;
        std::fs::write(dir.join("wrapper.main.c"), compile_out.wrapper_c.as_bytes())
            .with_context(|| format!("write emit wrapper.main.c under {}", dir.display()))?;
        std::fs::write(
            dir.join("bundle.combined.c"),
            compile_out.combined_c.as_bytes(),
        )
        .with_context(|| format!("write emit bundle.combined.c under {}", dir.display()))?;

        if world == WorldId::RunOsSandboxed {
            if let Some(effective) = effective_policy.as_ref() {
                let contents = std::fs::read(effective)
                    .with_context(|| format!("read policy: {}", effective.display()))?;
                std::fs::write(dir.join("policy.used.json"), contents).with_context(|| {
                    format!("write emit policy.used.json under {}", dir.display())
                })?;
            }
        }
    }

    std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;

    let exit_code: u8 = if compile_out.compile.ok { 0 } else { 1 };
    Ok(std::process::ExitCode::from(exit_code))
}

#[derive(Debug, Serialize)]
struct VmBundleManifest {
    schema_version: &'static str,
    backend: String,
    guest_image: String,
    guest_digest: String,
    payload: &'static str,
    workdir: &'static str,
    policy: &'static str,
}

struct CmdBundleVmParams<'a> {
    args: &'a BundleArgs,
    cwd: &'a Path,
    world: WorldId,
    target_kind: TargetKind,
    target_path: &'a Path,
    project_root: Option<&'a Path>,
    out_path: &'a Path,
    bundle_name: &'a str,
    base_policy: &'a Path,
    effective_policy: &'a Path,
    embedded_env_keys: &'a [String],
    resolved_module_roots: &'a [String],
    lockfile: Option<&'a Path>,
    max_memory_bytes: usize,
    max_output_bytes: Option<usize>,
    cpu_time_limit_seconds: Option<u64>,
}

fn cmd_bundle_vm(params: CmdBundleVmParams<'_>) -> Result<std::process::ExitCode> {
    let CmdBundleVmParams {
        args,
        cwd,
        world,
        target_kind,
        target_path,
        project_root,
        out_path,
        bundle_name,
        base_policy,
        effective_policy,
        embedded_env_keys,
        resolved_module_roots,
        lockfile,
        max_memory_bytes,
        max_output_bytes,
        cpu_time_limit_seconds,
    } = params;
    if world != WorldId::RunOsSandboxed {
        anyhow::bail!("VM bundles are only supported for run-os-sandboxed");
    }

    if !cfg!(any(target_os = "macos", target_os = "linux")) {
        anyhow::bail!("VM bundles are not supported on this platform");
    }

    let sidecar = vm_sidecar_dir_for_out(out_path);
    if sidecar.exists() && !sidecar.is_dir() {
        anyhow::bail!(
            "vm sidecar exists but is not a directory: {}",
            sidecar.display()
        );
    }
    if sidecar.is_dir() {
        std::fs::remove_dir_all(&sidecar)
            .with_context(|| format!("remove vm sidecar dir: {}", sidecar.display()))?;
    }
    std::fs::create_dir_all(sidecar.join("deps"))
        .with_context(|| format!("create vm sidecar deps dir: {}", sidecar.display()))?;

    let policy_bytes = std::fs::read(effective_policy)
        .with_context(|| format!("read effective policy: {}", effective_policy.display()))?;
    let policy: os_policy::Policy = serde_json::from_slice(&policy_bytes)
        .with_context(|| format!("parse effective policy: {}", effective_policy.display()))?;
    policy
        .validate_basic()
        .map_err(|e| anyhow::anyhow!("policy invalid: {e}"))?;

    let backend = resolve_vm_backend()?;

    let guest_image =
        std::env::var("X07_VM_GUEST_IMAGE").unwrap_or_else(|_| default_vm_guest_image());

    let payload_dst = sidecar.join("payload");
    let policy_dst = sidecar.join("policy.json");
    crate::util::write_atomic(&policy_dst, &policy_bytes)
        .with_context(|| format!("write vm bundle policy: {}", policy_dst.display()))?;

    let launcher_src = resolve_sibling_or_path_vm("x07-vm-launcher");
    if !launcher_src.is_file() {
        anyhow::bail!(
            "missing x07-vm-launcher binary (expected next to x07 binary or in PATH): {}",
            launcher_src.display()
        );
    }
    copy_executable_atomic(&launcher_src, out_path)?;

    let reaper_src = resolve_sibling_or_path_vm("x07-vm-reaper");
    if !reaper_src.is_file() {
        anyhow::bail!(
            "missing x07-vm-reaper binary (expected next to x07 binary or in PATH): {}",
            reaper_src.display()
        );
    }
    copy_executable_atomic(&reaper_src, &sidecar.join("deps").join("x07-vm-reaper"))?;

    let guest_payload = build_vm_payload_bundle(VmPayloadBundleParams {
        args,
        cwd,
        target_kind,
        target_path,
        policy_bytes: &policy_bytes,
        guest_image: &guest_image,
        policy: &policy,
        max_memory_bytes,
        max_output_bytes,
        cpu_time_limit_seconds,
        emit_dir: args.emit_dir.as_deref(),
    })?;

    copy_executable_atomic(&guest_payload.payload_path, &payload_dst)?;

    let guest_report = guest_payload.report;

    let firecracker_cfg = if backend == VmBackend::FirecrackerCtr {
        Some(firecracker_ctr_config_from_env())
    } else {
        None
    };

    let guest_digest = if backend == VmBackend::Vz {
        let bundle_dir = std::env::var(x07_vm::ENV_VZ_GUEST_BUNDLE).unwrap_or_default();
        if bundle_dir.trim().is_empty() {
            anyhow::bail!(
                "missing VZ guest bundle directory (set {}=/path/to/guest.bundle)",
                x07_vm::ENV_VZ_GUEST_BUNDLE
            );
        }
        x07_vm::resolve_vm_guest_digest(backend, &bundle_dir, None)?
    } else {
        x07_vm::resolve_vm_guest_digest(backend, &guest_image, firecracker_cfg.as_ref())?
    };

    let manifest = VmBundleManifest {
        schema_version: "x07.vm.bundle.manifest@0.2.0",
        backend: backend.to_string(),
        guest_image: guest_image.clone(),
        guest_digest: guest_digest.clone(),
        payload: "payload",
        workdir: "/x07/work",
        policy: "policy.json",
    };
    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    manifest_bytes.push(b'\n');
    crate::util::write_atomic(&sidecar.join("manifest.json"), &manifest_bytes)
        .with_context(|| format!("write vm bundle manifest under {}", sidecar.display()))?;

    let guest_arch = guest_platform_arch()?;
    let report = BundleReport {
        schema_version: X07_BUNDLE_REPORT_SCHEMA_VERSION,
        runner: "host",
        world: world.as_str(),
        target: BundleTarget {
            kind: target_kind.as_str(),
            path: target_path.display().to_string(),
            project_root: project_root.map(|p| p.display().to_string()),
            lockfile: lockfile.map(|p| p.display().to_string()),
            resolved_module_roots: resolved_module_roots.to_vec(),
        },
        bundle: BundleSection {
            out: out_path.display().to_string(),
            name: bundle_name.to_string(),
            abi: BundleAbi { kind: "argv_v1" },
            policy: Some(BundlePolicy {
                base_policy: base_policy.display().to_string(),
                effective_policy: effective_policy.display().to_string(),
                embedded_env_keys: embedded_env_keys.to_vec(),
            }),
            emit_dir: args.emit_dir.as_ref().map(|p| p.display().to_string()),
            kind: "vm",
            guest: Some(BundleGuest {
                contract: "x07.vm.exec@0.1.0",
                platform: BundleGuestPlatform {
                    os: "linux",
                    arch: guest_arch,
                },
                image: BundleGuestImage {
                    digest: Some(guest_digest),
                    ref_: if backend == VmBackend::Vz {
                        None
                    } else {
                        Some(guest_image.clone())
                    },
                    layout: None,
                },
                entrypoint: vec!["/x07/bundle/payload".to_string()],
                workdir: "/x07/work".to_string(),
                mounts: BundleGuestMounts {
                    job_in: "/x07/in".to_string(),
                    job_out: "/x07/out".to_string(),
                },
            }),
        },
        report: guest_report,
    };

    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');

    if let Some(dir) = &args.emit_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("create emit dir: {}", dir.display()))?;
        std::fs::write(dir.join("report.json"), &bytes)
            .with_context(|| format!("write emit report.json under {}", dir.display()))?;
        std::fs::write(dir.join("policy.used.json"), &policy_bytes)
            .with_context(|| format!("write emit policy.used.json under {}", dir.display()))?;
    }

    std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;

    let ok = report
        .report
        .get("compile")
        .and_then(|v| v.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let exit_code: u8 = if ok { 0 } else { 1 };
    Ok(std::process::ExitCode::from(exit_code))
}

struct VmPayloadBundleOut {
    report: Value,
    payload_path: PathBuf,
}

struct VmPayloadBundleParams<'a> {
    args: &'a BundleArgs,
    cwd: &'a Path,
    target_kind: TargetKind,
    target_path: &'a Path,
    policy_bytes: &'a [u8],
    guest_image: &'a str,
    policy: &'a os_policy::Policy,
    max_memory_bytes: usize,
    max_output_bytes: Option<usize>,
    cpu_time_limit_seconds: Option<u64>,
    emit_dir: Option<&'a Path>,
}

fn build_vm_payload_bundle(params: VmPayloadBundleParams<'_>) -> Result<VmPayloadBundleOut> {
    let VmPayloadBundleParams {
        args,
        cwd,
        target_kind,
        target_path,
        policy_bytes,
        guest_image,
        policy,
        max_memory_bytes,
        max_output_bytes,
        cpu_time_limit_seconds,
        emit_dir,
    } = params;
    if args.i_accept_weaker_isolation {
        std::env::set_var(x07_vm::ENV_ACCEPT_WEAKER_ISOLATION, "1");
    }

    let backend = resolve_vm_backend()?;

    let created_unix_ms = now_unix_ms()?;
    let run_id = {
        let pid = std::process::id();
        let n = VM_RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{created_unix_ms}-{pid}-{n}")
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

    std::fs::write(job_in.join("policy.json"), policy_bytes).context("write policy.json")?;

    let guest_target_args: Vec<String> = match target_kind {
        TargetKind::Program => {
            let bytes = std::fs::read(target_path)
                .with_context(|| format!("read program: {}", target_path.display()))?;
            let program_dir = job_in.join("program");
            std::fs::create_dir_all(&program_dir)
                .with_context(|| format!("create program dir: {}", program_dir.display()))?;
            std::fs::write(program_dir.join("main.x07.json"), bytes)
                .with_context(|| format!("write program to {}", program_dir.display()))?;

            let mut guest_target_args = vec![
                "--program".to_string(),
                "/x07/in/program/main.x07.json".to_string(),
            ];

            let module_roots_dir = job_in.join("module_roots");
            std::fs::create_dir_all(&module_roots_dir).with_context(|| {
                format!("create module_roots dir: {}", module_roots_dir.display())
            })?;

            for (idx, root) in args.module_root.iter().enumerate() {
                let root_abs = if root.is_absolute() {
                    root.to_path_buf()
                } else {
                    cwd.join(root)
                };
                let dst = module_roots_dir.join(idx.to_string());
                x07_vm::copy_dir_recursive(&root_abs, &dst).with_context(|| {
                    format!(
                        "copy module root {} -> {}",
                        root_abs.display(),
                        dst.display()
                    )
                })?;

                guest_target_args.push("--module-root".to_string());
                guest_target_args.push(format!("/x07/in/module_roots/{idx}"));
            }

            guest_target_args
        }

        TargetKind::Project => {
            let manifest_abs = if target_path.is_absolute() {
                target_path.to_path_buf()
            } else {
                cwd.join(target_path)
            };
            let base = manifest_abs
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .context("project manifest has no parent dir")?
                .to_path_buf();

            let project_dst = job_in.join("project");
            x07_vm::copy_dir_recursive(&base, &project_dst).with_context(|| {
                format!(
                    "copy project dir {} -> {}",
                    base.display(),
                    project_dst.display()
                )
            })?;

            let file_name = manifest_abs
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("x07.json"));
            let guest_project_path = PathBuf::from("/x07/in/project").join(file_name);

            vec![
                "--project".to_string(),
                guest_project_path.display().to_string(),
            ]
        }
    };

    let mut guest_argv: Vec<String> = vec!["x07".to_string(), "bundle".to_string()];

    guest_argv.extend(guest_target_args);

    if let Some(profile) = args.profile.as_ref() {
        guest_argv.push("--profile".to_string());
        guest_argv.push(profile.clone());
    }

    guest_argv.push("--world".to_string());
    guest_argv.push(WorldId::RunOsSandboxed.as_str().to_string());

    guest_argv.push("--sandbox-backend".to_string());
    guest_argv.push("os".to_string());
    guest_argv.push("--i-accept-weaker-isolation".to_string());

    guest_argv.push("--policy".to_string());
    guest_argv.push("/x07/in/policy.json".to_string());

    guest_argv.push("--out".to_string());
    guest_argv.push("/x07/out/payload".to_string());

    guest_argv.push("--max-memory-bytes".to_string());
    guest_argv.push(max_memory_bytes.to_string());

    if let Some(v) = max_output_bytes {
        guest_argv.push("--max-output-bytes".to_string());
        guest_argv.push(v.to_string());
    }
    if let Some(v) = cpu_time_limit_seconds {
        guest_argv.push("--cpu-time-limit-seconds".to_string());
        guest_argv.push(v.to_string());
    }
    if args.debug_borrow_checks {
        guest_argv.push("--debug-borrow-checks".to_string());
    }
    if let Some(v) = args.max_c_bytes {
        guest_argv.push("--max-c-bytes".to_string());
        guest_argv.push(v.to_string());
    }
    if let Some(cc_profile) = args.cc_profile {
        guest_argv.push("--cc-profile".to_string());
        guest_argv.push(
            match cc_profile {
                CcProfile::Default => "default",
                CcProfile::Size => "size",
            }
            .to_string(),
        );
    }
    if args.auto_ffi {
        guest_argv.push("--auto-ffi".to_string());
    }
    if args.no_auto_ffi {
        guest_argv.push("--no-auto-ffi".to_string());
    }

    if emit_dir.is_some() {
        guest_argv.push("--emit-dir".to_string());
        guest_argv.push("/x07/out/emit".to_string());
    }

    let mounts: Vec<MountSpec> = vec![
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
    ];

    let limits = LimitsSpec {
        wall_ms,
        grace_ms,
        cleanup_ms,
        mem_bytes: Some(policy.limits.mem_bytes),
        vcpus: None,
        max_stdout_bytes: 16 * 1024 * 1024,
        max_stderr_bytes: 16 * 1024 * 1024,
        network: NetworkMode::None,
    };

    let spec = RunSpec {
        run_id: run_id.clone(),
        backend,
        image: if backend == VmBackend::Vz {
            std::env::var(x07_vm::ENV_VZ_GUEST_BUNDLE).unwrap_or_default()
        } else {
            guest_image.to_string()
        },
        image_digest: None,
        argv: guest_argv,
        env: BTreeMap::new(),
        mounts,
        workdir: Some(PathBuf::from("/opt/x07")),
        limits,
    };

    let firecracker_cfg = if backend == VmBackend::FirecrackerCtr {
        Some(firecracker_ctr_config_from_env())
    } else {
        None
    };

    let reaper_bin = resolve_sibling_or_path_vm("x07-vm-reaper");
    let out = x07_vm::run_vm_job(
        &spec,
        x07_vm::VmJobRunParams {
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

    let report_json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .with_context(|| "guest x07 bundle did not emit valid JSON")?;

    let payload_path = job_out.join("payload");
    if !payload_path.is_file() {
        anyhow::bail!(
            "guest x07 bundle did not produce expected payload at {}",
            payload_path.display()
        );
    }

    if let Some(dir) = emit_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("create emit dir: {}", dir.display()))?;
        let guest_emit = job_out.join("emit");
        if guest_emit.is_dir() {
            let _ = std::fs::remove_dir_all(dir.join("guest"));
            x07_vm::copy_dir_recursive(&guest_emit, &dir.join("guest")).with_context(|| {
                format!(
                    "copy guest emit dir {} -> {}",
                    guest_emit.display(),
                    dir.display()
                )
            })?;
        }
    }

    let runner_report = report_json
        .get("report")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    Ok(VmPayloadBundleOut {
        report: runner_report,
        payload_path,
    })
}

fn default_vm_guest_image() -> String {
    format!(
        "ghcr.io/x07lang/x07-guest-runner:{}",
        env!("CARGO_PKG_VERSION")
    )
}

fn vm_sidecar_dir_for_out(out_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.vm", out_path.display()))
}

fn guest_platform_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        other => anyhow::bail!("unsupported guest arch: {other}"),
    }
}

fn copy_executable_atomic(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir: {}", parent.display()))?;
    }

    let file_name = dst
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let pid = std::process::id();
    let n = VM_RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dst.with_file_name(format!(".{file_name}.{pid}.{n}.tmp"));

    std::fs::copy(src, &tmp)
        .with_context(|| format!("copy file {} -> {}", src.display(), tmp.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }

    match std::fs::rename(&tmp, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::fs::remove_file(dst);
            std::fs::rename(&tmp, dst)?;
            Ok(())
        }
    }
}

fn now_unix_ms() -> Result<u64> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(d.as_millis().try_into().unwrap_or(u64::MAX))
}

fn resolve_target(cwd: &Path, args: &BundleArgs) -> Result<(TargetKind, PathBuf, Option<PathBuf>)> {
    let mut count = 0;
    if args.project.is_some() {
        count += 1;
    }
    if args.program.is_some() {
        count += 1;
    }
    if count > 1 {
        anyhow::bail!("set exactly one of --project or --program");
    }

    if let Some(path) = &args.project {
        return Ok((
            TargetKind::Project,
            path.to_path_buf(),
            Some(path.to_path_buf()),
        ));
    }
    if let Some(path) = &args.program {
        let base = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let project_manifest = crate::run::discover_project_manifest(base)?;
        return Ok((TargetKind::Program, path.to_path_buf(), project_manifest));
    }

    let found = crate::run::discover_project_manifest(cwd)?
        .context("no project found (pass --project or --program)")?;
    Ok((TargetKind::Project, found.clone(), Some(found)))
}

fn resolve_world(
    args: &BundleArgs,
    project_manifest: Option<&Path>,
    profile: Option<&crate::run::ResolvedProfile>,
) -> Result<WorldId> {
    if let Some(world) = args.world {
        return Ok(world);
    }
    if let Some(profile) = profile {
        return Ok(profile.world);
    }
    if let Some(project_path) = project_manifest {
        let manifest = project::load_project_manifest(project_path)?;
        let world = x07c::world_config::parse_world_id(&manifest.world)
            .with_context(|| format!("invalid project world {:?}", manifest.world))?;
        return Ok(world);
    }
    Ok(WorldId::RunOs)
}

fn resolve_out_path(
    out: &Path,
    target_kind: TargetKind,
    project_root: &Option<PathBuf>,
) -> PathBuf {
    let raw = out.as_os_str().to_string_lossy();
    let is_dir_hint = raw.ends_with('/') || raw.ends_with('\\');
    let is_dir = out.is_dir() || is_dir_hint;
    if !is_dir {
        return out.to_path_buf();
    }

    let dir = out;
    let name = match target_kind {
        TargetKind::Project => project_root
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("app")
            .to_string(),
        TargetKind::Program => "app".to_string(),
    };
    dir.join(name)
}

fn normalize_exe_extension(out: PathBuf) -> PathBuf {
    out
}

fn resolve_auto_ffi(args: &BundleArgs, profile: Option<&crate::run::ResolvedProfile>) -> bool {
    if args.no_auto_ffi {
        return false;
    }
    if args.auto_ffi {
        return true;
    }
    if let Some(profile) = profile {
        if let Some(v) = profile.auto_ffi {
            return v;
        }
    }
    true
}

fn prepare_project_target(
    project_path: &Path,
    world: WorldId,
    args: &BundleArgs,
    profile: Option<&crate::run::ResolvedProfile>,
) -> Result<PreparedTarget> {
    let manifest = project::load_project_manifest(project_path)?;
    let base = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut extra_cc_args = manifest.link.cc_args(base);

    let lock_path = project::default_lockfile_path(project_path, &manifest);
    let (lockfile, lock): (Option<PathBuf>, project::Lockfile) = if lock_path.is_file() {
        let lock_bytes = std::fs::read(&lock_path)
            .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
        let lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
            .with_context(|| format!("parse lockfile: {}", lock_path.display()))?;
        (Some(lock_path.clone()), lock)
    } else if manifest.dependencies.is_empty() {
        (
            None,
            project::Lockfile {
                schema_version: PROJECT_LOCKFILE_SCHEMA_VERSION.to_string(),
                dependencies: Vec::new(),
            },
        )
    } else {
        anyhow::bail!(
            "missing lockfile for project with dependencies: {}",
            lock_path.display()
        );
    };
    project::verify_lockfile(project_path, &manifest, &lock)?;

    let entry_path = base.join(&manifest.entry);
    let repair_result = crate::repair::maybe_repair_x07ast_file(&entry_path, world, &args.repair)
        .with_context(|| format!("repair entry: {}", entry_path.display()))?;
    let program = if let Some(r) = repair_result {
        r.formatted.into_bytes()
    } else {
        std::fs::read(&entry_path)
            .with_context(|| format!("read entry: {}", entry_path.display()))?
    };

    let mut module_roots = project::collect_module_roots(project_path, &manifest, &lock)?;
    if world.is_standalone_only() {
        let os_roots = os_paths::default_os_module_roots()?;
        for r in os_roots {
            if !module_roots.contains(&r) {
                module_roots.push(r);
            }
        }
    }

    if world.is_standalone_only() && resolve_auto_ffi(args, profile) {
        extra_cc_args.extend(auto_ffi::collect_auto_ffi_cc_args(&module_roots)?);
    }

    Ok(PreparedTarget {
        program_bytes: program,
        lockfile,
        module_roots,
        extra_cc_args,
    })
}

fn prepare_program_target(
    program_path: &Path,
    world: WorldId,
    args: &BundleArgs,
) -> Result<PreparedTarget> {
    if !program_path
        .as_os_str()
        .to_string_lossy()
        .ends_with(".x07.json")
    {
        anyhow::bail!(
            "--program must be an x07AST JSON file (*.x07.json), got {}",
            program_path.display()
        );
    }
    let repair_result = crate::repair::maybe_repair_x07ast_file(program_path, world, &args.repair)
        .with_context(|| format!("repair program: {}", program_path.display()))?;
    let program = if let Some(r) = repair_result {
        r.formatted.into_bytes()
    } else {
        std::fs::read(program_path)
            .with_context(|| format!("read program: {}", program_path.display()))?
    };

    let mut module_roots = args.module_root.clone();
    if world.is_standalone_only() {
        let os_roots = os_paths::default_os_module_roots()?;
        for r in os_roots {
            if !module_roots.contains(&r) {
                module_roots.push(r);
            }
        }
    }

    let mut extra_cc_args = Vec::new();
    if world.is_standalone_only() && resolve_auto_ffi(args, None) {
        extra_cc_args.extend(auto_ffi::collect_auto_ffi_cc_args(&module_roots)?);
    }

    Ok(PreparedTarget {
        program_bytes: program,
        lockfile: None,
        module_roots,
        extra_cc_args,
    })
}

fn resolve_policy_env(
    world: WorldId,
    policy_root: &Path,
    cli_policy: Option<&Path>,
    profile_policy: Option<&Path>,
    effective_policy: Option<&Path>,
) -> Result<ResolvedPolicyEnv> {
    if world != WorldId::RunOsSandboxed {
        return Ok(ResolvedPolicyEnv {
            base_policy: None,
            env_pairs: Vec::new(),
            embedded_env_keys: Vec::new(),
            allow_unsafe: None,
            allow_ffi: None,
        });
    }

    let base_policy = cli_policy
        .map(PathBuf::from)
        .or_else(|| profile_policy.map(PathBuf::from))
        .context("run-os-sandboxed requires a policy file (--policy or profile policy)")?;
    let base_policy = if base_policy.is_absolute() {
        base_policy
    } else {
        policy_root.join(base_policy)
    };

    let effective_policy = effective_policy.unwrap_or(base_policy.as_path());
    let bytes = std::fs::read(effective_policy)
        .with_context(|| format!("read policy: {}", effective_policy.display()))?;
    let pol: os_policy::Policy = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse policy JSON: {}", effective_policy.display()))?;
    pol.validate_basic()
        .map_err(|e| anyhow::anyhow!("invalid policy: {e}"))?;

    let mut env_pairs: Vec<(String, String)> = Vec::new();
    env_pairs.push(("X07_WORLD".to_string(), world.as_str().to_string()));
    env_pairs.push(("X07_OS_SANDBOXED".to_string(), "1".to_string()));
    env_pairs.extend(os_env::policy_to_env(&pol));
    let keys = env_pairs.iter().map(|(k, _)| k.clone()).collect();

    Ok(ResolvedPolicyEnv {
        base_policy: Some(base_policy),
        env_pairs,
        embedded_env_keys: keys,
        allow_unsafe: Some(pol.language.allow_unsafe),
        allow_ffi: Some(pol.language.allow_ffi),
    })
}

fn host_compile_report_json(
    mode: &str,
    compile: &x07_host_runner::CompilerResult,
) -> Result<Value> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let exit_code: u8 = if compile.ok { 0 } else { 1 };
    Ok(serde_json::json!({
        "schema_version": X07_HOST_RUNNER_REPORT_SCHEMA_VERSION,
        "mode": mode,
        "exit_code": exit_code,
        "compile": {
            "ok": compile.ok,
            "exit_status": compile.exit_status,
            "lang_id": compile.lang_id,
            "native_requires": compile.native_requires,
            "c_source_size": compile.c_source_size,
            "compiled_exe": compile.compiled_exe.as_ref().map(|p| p.display().to_string()),
            "compiled_exe_size": compile.compiled_exe_size,
            "compile_error": compile.compile_error,
            "stdout_b64": b64.encode(&compile.stdout),
            "stderr_b64": b64.encode(&compile.stderr),
            "fuel_used": compile.fuel_used,
            "trap": compile.trap,
        },
        "solve": serde_json::Value::Null,
    }))
}
