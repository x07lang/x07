use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use x07_contracts::X07_OS_RUNNER_REPORT_SCHEMA_VERSION;

mod caps;
mod digest;
mod inspect_parsers;
mod job_runner;
mod kill_plan;
mod labels;
mod reaper_joiner;
mod sweep;

pub use caps::VmCaps;
pub use digest::{resolve_vm_guest_digest, verify_vm_guest_digest};
pub use inspect_parsers::{
    is_owned_by_x07, parse_apple_container_json_owned, parse_ctr_container_info_json_owned, Labels,
    OwnedContainer, ParseError,
};
pub use job_runner::{run_vm_job, DefaultVmDriver, VmDriver, VmJobRunParams};
pub use kill_plan::{
    enforce_kill_plan, enforce_kill_plan_for_job, CommandSpec, ExecResult, KillBackend, KillPlan,
    KillResult, RetryPolicy, Signal, TargetRef,
};
pub use labels::{
    read_or_create_runner_instance_id, LabelError, X07LabelSet, X07_LABEL_BACKEND_KEY,
    X07_LABEL_CREATED_UNIX_MS_KEY, X07_LABEL_DEADLINE_UNIX_MS_KEY, X07_LABEL_IMAGE_DIGEST_KEY,
    X07_LABEL_JOB_ID_KEY, X07_LABEL_RUNNER_INSTANCE_KEY, X07_LABEL_RUN_ID_KEY,
    X07_LABEL_SCHEMA_KEY, X07_LABEL_SCHEMA_VALUE,
};
pub use sweep::{sweep_orphans_best_effort, SweepReport};

pub const VM_JOB_SCHEMA_VERSION: &str = "x07.vm.job@0.1.0";

pub const ENV_VM_BACKEND: &str = "X07_VM_BACKEND";
pub const ENV_VM_STATE_DIR: &str = "X07_VM_STATE_DIR";
pub const ENV_ACCEPT_WEAKER_ISOLATION: &str = "X07_I_ACCEPT_WEAKER_ISOLATION";

pub const ENV_VZ_HELPER_BIN: &str = "X07_VM_VZ_HELPER_BIN";
pub const ENV_VZ_GUEST_BUNDLE: &str = "X07_VM_VZ_GUEST_BUNDLE";
pub const ENV_VM_GUEST_IMAGE_DIGEST: &str = "X07_VM_GUEST_IMAGE_DIGEST";

pub const DEFAULT_VZ_HELPER_BIN: &str = "x07-vz-helper";

pub const ENV_FIRECRACKER_CTR_BIN: &str = "X07_VM_FIRECRACKER_CTR_BIN";
pub const ENV_FIRECRACKER_CONTAINERD_SOCK: &str = "X07_VM_FIRECRACKER_CONTAINERD_SOCK";
pub const ENV_FIRECRACKER_SNAPSHOTTER: &str = "X07_VM_FIRECRACKER_SNAPSHOTTER";
pub const ENV_CONTAINERD_NAMESPACE: &str = "X07_VM_CONTAINERD_NAMESPACE";

pub const DEFAULT_FIRECRACKER_CTR_BIN: &str = "firecracker-ctr";
pub const DEFAULT_FIRECRACKER_CONTAINERD_SOCK: &str = "/run/firecracker-containerd/containerd.sock";
pub const DEFAULT_FIRECRACKER_RUNTIME: &str = "aws.firecracker";
pub const DEFAULT_FIRECRACKER_SNAPSHOTTER: &str = "devmapper";
pub const DEFAULT_CONTAINERD_NAMESPACE: &str = "x07";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkMode {
    None,
    Default,
}

#[derive(Debug, Clone)]
pub struct MountSpec {
    pub host_path: PathBuf,
    pub guest_path: PathBuf,
    pub readonly: bool,
}

#[derive(Debug, Clone)]
pub struct LimitsSpec {
    pub wall_ms: u64,
    pub grace_ms: u64,
    pub cleanup_ms: u64,
    pub mem_bytes: Option<u64>,
    pub vcpus: Option<u32>,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub network: NetworkMode,
}

#[derive(Debug, Clone)]
pub struct RunSpec {
    pub run_id: String,
    pub backend: VmBackend,
    pub image: String,
    pub image_digest: Option<String>,
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub mounts: Vec<MountSpec>,
    pub workdir: Option<PathBuf>,
    pub limits: LimitsSpec,
}

#[derive(Debug)]
pub struct RunOutput {
    pub exit_status: i32,
    pub timed_out: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VmBackend {
    AppleContainer,
    Vz,
    Docker,
    Podman,
    FirecrackerCtr,
}

impl std::fmt::Display for VmBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmBackend::AppleContainer => f.write_str("apple-container"),
            VmBackend::Vz => f.write_str("vz"),
            VmBackend::Docker => f.write_str("docker"),
            VmBackend::Podman => f.write_str("podman"),
            VmBackend::FirecrackerCtr => f.write_str("firecracker-ctr"),
        }
    }
}

impl std::str::FromStr for VmBackend {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim().to_ascii_lowercase();
        match s.as_str() {
            "apple-container" | "container" => Ok(VmBackend::AppleContainer),
            "vz" => Ok(VmBackend::Vz),
            "docker" => Ok(VmBackend::Docker),
            "podman" => Ok(VmBackend::Podman),
            "firecracker-ctr" | "firecracker" => Ok(VmBackend::FirecrackerCtr),
            other => anyhow::bail!(
                "invalid {ENV_VM_BACKEND}={other:?} (expected one of: apple-container, vz, docker, podman, firecracker-ctr)"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmJob {
    pub schema_version: String,
    pub run_id: String,
    pub backend: VmBackend,
    pub container_id: String,
    pub pid: Option<u32>,
    pub created_unix_ms: u64,
    pub deadline_unix_ms: u64,
    pub grace_ms: u64,
    pub cleanup_ms: u64,
    pub ctr: Option<CtrJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtrJob {
    pub bin: String,
    pub address: String,
    pub namespace: String,
}

#[derive(Debug, Clone)]
pub struct FirecrackerCtrConfig {
    pub bin: OsString,
    pub address: String,
    pub namespace: String,
    pub runtime: String,
    pub snapshotter: String,
}

pub fn firecracker_ctr_config_from_env() -> FirecrackerCtrConfig {
    let bin = std::env::var_os(ENV_FIRECRACKER_CTR_BIN)
        .unwrap_or_else(|| OsString::from(DEFAULT_FIRECRACKER_CTR_BIN));
    let address = std::env::var(ENV_FIRECRACKER_CONTAINERD_SOCK)
        .unwrap_or_else(|_| DEFAULT_FIRECRACKER_CONTAINERD_SOCK.to_string());
    let namespace = std::env::var(ENV_CONTAINERD_NAMESPACE)
        .unwrap_or_else(|_| DEFAULT_CONTAINERD_NAMESPACE.to_string());
    let snapshotter = std::env::var(ENV_FIRECRACKER_SNAPSHOTTER)
        .unwrap_or_else(|_| DEFAULT_FIRECRACKER_SNAPSHOTTER.to_string());

    FirecrackerCtrConfig {
        bin,
        address,
        namespace,
        runtime: DEFAULT_FIRECRACKER_RUNTIME.to_string(),
        snapshotter,
    }
}

pub fn firecracker_ctr_config_from_job(job: &CtrJob) -> FirecrackerCtrConfig {
    FirecrackerCtrConfig {
        bin: OsString::from(job.bin.clone()),
        address: job.address.clone(),
        namespace: job.namespace.clone(),
        runtime: DEFAULT_FIRECRACKER_RUNTIME.to_string(),
        snapshotter: DEFAULT_FIRECRACKER_SNAPSHOTTER.to_string(),
    }
}

fn resolve_executable(bin: &OsString) -> Option<PathBuf> {
    let bin_path = PathBuf::from(bin);
    if bin_path.components().count() > 1 {
        return if is_executable(&bin_path) {
            Some(bin_path)
        } else {
            None
        };
    }

    let path_env = std::env::var_os("PATH")?;
    for p in std::env::split_paths(&path_env) {
        let cand = p.join(&bin_path);
        if is_executable(&cand) {
            return Some(cand);
        }
    }
    None
}

fn resolve_vz_helper_bin() -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        anyhow::bail!("vz helper is only supported on macOS");
    }

    if let Some(raw) = std::env::var_os(ENV_VZ_HELPER_BIN) {
        let Some(path) = resolve_executable(&raw) else {
            anyhow::bail!(
                "missing VZ helper binary {:?} (set {ENV_VZ_HELPER_BIN} to override)",
                raw
            );
        };
        return Ok(path);
    }

    let sibling = resolve_sibling_or_path(DEFAULT_VZ_HELPER_BIN);
    if is_executable(&sibling) {
        return Ok(sibling);
    }

    let Some(path) = resolve_executable(&OsString::from(DEFAULT_VZ_HELPER_BIN)) else {
        anyhow::bail!(
            "missing VZ helper binary {DEFAULT_VZ_HELPER_BIN:?} (expected next to x07 binaries or in PATH; set {ENV_VZ_HELPER_BIN} to override)"
        );
    };
    Ok(path)
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if let Ok(meta) = std::fs::metadata(path) {
            return meta.permissions().mode() & 0o111 != 0;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        return true;
    }
    false
}

fn preflight_macos_vm_backend(backend: VmBackend) -> Result<()> {
    let mut cmd = match backend {
        VmBackend::AppleContainer => {
            let mut c = Command::new("container");
            c.args(["system", "info"]);
            c
        }
        VmBackend::Vz => {
            let helper = resolve_vz_helper_bin()?;
            let mut c = Command::new(helper);
            c.arg("preflight");
            c
        }
        VmBackend::Podman => {
            let mut c = Command::new("podman");
            c.arg("info");
            c
        }
        VmBackend::Docker => {
            let mut c = Command::new("docker");
            c.arg("info");
            c
        }
        VmBackend::FirecrackerCtr => anyhow::bail!("preflight_macos_vm_backend: invalid backend"),
    };

    cmd.stdin(Stdio::null());
    let out = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("preflight {backend}"))?;
    if out.timed_out {
        anyhow::bail!("preflight {backend} timed out");
    }
    if out.exit_status != 0 {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("preflight {backend} failed: {stderr}");
    }
    Ok(())
}

fn preflight_linux_firecracker_backend(cfg: &FirecrackerCtrConfig) -> Result<()> {
    let Some(_) = resolve_executable(&cfg.bin) else {
        anyhow::bail!(
            "missing firecracker-ctr binary {:?} (set {ENV_FIRECRACKER_CTR_BIN} to override)",
            cfg.bin
        );
    };

    if !Path::new(&cfg.address).exists() {
        anyhow::bail!(
            "missing firecracker-containerd socket at {:?} (set {ENV_FIRECRACKER_CONTAINERD_SOCK} to override)",
            cfg.address
        );
    }

    if !Path::new("/dev/kvm").exists() {
        anyhow::bail!("missing /dev/kvm (Firecracker requires KVM)");
    }

    Ok(())
}

pub fn resolve_vm_backend() -> Result<VmBackend> {
    if let Ok(raw) = std::env::var(ENV_VM_BACKEND) {
        let backend = VmBackend::from_str(&raw)?;
        if cfg!(target_os = "macos") {
            if matches!(backend, VmBackend::FirecrackerCtr) {
                anyhow::bail!("unsupported {ENV_VM_BACKEND}={backend} on macOS");
            }
            preflight_macos_vm_backend(backend)?;
            return Ok(backend);
        }
        if cfg!(target_os = "linux") {
            if backend != VmBackend::FirecrackerCtr {
                anyhow::bail!(
                    "unsupported {ENV_VM_BACKEND}={backend} on Linux (expected firecracker-ctr)"
                );
            }
            let cfg = firecracker_ctr_config_from_env();
            preflight_linux_firecracker_backend(&cfg)?;
            return Ok(backend);
        }
        anyhow::bail!("VM backend is not supported on this platform");
    }

    if cfg!(target_os = "macos") {
        let accept_weaker_isolation = read_accept_weaker_isolation_env().unwrap_or(false);

        let macos_major = macos_product_major_version().unwrap_or(0);
        if macos_major >= 26 && preflight_macos_vm_backend(VmBackend::AppleContainer).is_ok() {
            return Ok(VmBackend::AppleContainer);
        }

        if preflight_macos_vm_backend(VmBackend::Vz).is_ok() {
            return Ok(VmBackend::Vz);
        }

        if accept_weaker_isolation {
            for backend in [VmBackend::Podman, VmBackend::Docker] {
                if preflight_macos_vm_backend(backend).is_ok() {
                    return Ok(backend);
                }
            }
        }

        anyhow::bail!(
            "no supported VM backend found on macOS\n\nfix:\n  - install the signed {DEFAULT_VZ_HELPER_BIN} helper + provide a VZ guest bundle ({ENV_VZ_GUEST_BUNDLE}), or\n  - on macOS 26+: install and start Apple container, or\n  - (weaker isolation) set {ENV_ACCEPT_WEAKER_ISOLATION}=1 and use Docker Desktop / Podman"
        );
    }

    if cfg!(target_os = "linux") {
        let cfg = firecracker_ctr_config_from_env();
        preflight_linux_firecracker_backend(&cfg)?;
        return Ok(VmBackend::FirecrackerCtr);
    }

    anyhow::bail!("VM backend is not supported on this platform");
}

fn parse_bool_env(name: &str, raw: &str) -> Result<bool> {
    match raw.trim() {
        "1" | "true" | "TRUE" | "yes" | "YES" => Ok(true),
        "0" | "false" | "FALSE" | "no" | "NO" => Ok(false),
        other => anyhow::bail!(
            "invalid environment variable {name}={other:?} (expected one of: 1, 0, true, false, yes, no)"
        ),
    }
}

pub fn read_accept_weaker_isolation_env() -> Result<bool> {
    match std::env::var(ENV_ACCEPT_WEAKER_ISOLATION) {
        Ok(raw) => parse_bool_env(ENV_ACCEPT_WEAKER_ISOLATION, &raw),
        Err(_) => Ok(false),
    }
}

#[cfg(target_os = "macos")]
fn macos_product_major_version() -> Option<u32> {
    let out = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let s = s.trim();
    let major = s.split('.').next()?.parse().ok()?;
    Some(major)
}

#[cfg(not(target_os = "macos"))]
fn macos_product_major_version() -> Option<u32> {
    None
}

pub fn default_vm_state_root() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var(ENV_VM_STATE_DIR) {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("create {ENV_VM_STATE_DIR}: {}", dir.display()))?;
        return Ok(dir);
    }

    if let Ok(home) = std::env::var("HOME") {
        let dir = PathBuf::from(home).join(".x07").join("vm").join("jobs");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("create vm state dir: {}", dir.display()))?;
        return Ok(dir);
    }

    let dir = std::env::temp_dir().join("x07").join("vm").join("jobs");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create vm state dir: {}", dir.display()))?;
    Ok(dir)
}

pub fn default_grace_ms(wall_ms: u64) -> u64 {
    let wall_ms = wall_ms.max(1);
    let tenth = wall_ms / 10;
    let cap = 2_000u64.min(tenth.max(1));
    1_000u64.min(cap).max(1)
}

pub fn default_cleanup_ms() -> u64 {
    30_000
}

pub fn container_id_from_run_id(run_id: &str) -> Result<String> {
    let id = format!("x07-{run_id}");
    validate_container_id(&id)?;
    Ok(id)
}

pub fn validate_container_id(id: &str) -> Result<()> {
    if id.is_empty() {
        anyhow::bail!("container id is empty");
    }
    if !id.is_ascii() {
        anyhow::bail!("container id must be ASCII");
    }
    if id.len() > 128 {
        anyhow::bail!("container id must be <= 128 bytes");
    }
    let first = id.as_bytes()[0] as char;
    if !matches!(first, 'A'..='Z' | 'a'..='z' | '0'..='9') {
        anyhow::bail!("container id must start with [A-Za-z0-9]");
    }
    for b in id.bytes() {
        let c = b as char;
        if !matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '.' | '-') {
            anyhow::bail!("container id contains invalid character {c:?}");
        }
    }
    Ok(())
}

pub fn x07_label_set(
    state_root: &Path,
    run_id: &str,
    backend: VmBackend,
    created_unix_ms: u64,
    deadline_unix_ms: u64,
    image_digest: Option<&str>,
) -> Result<BTreeMap<String, String>> {
    let runner_instance = read_or_create_runner_instance_id(state_root)?;
    let set = X07LabelSet::new(run_id, runner_instance, deadline_unix_ms)
        .with_job_id(run_id)
        .with_backend(format!("vm.{backend}"))
        .with_created_unix_ms(created_unix_ms);
    let set = if let Some(d) = image_digest {
        set.with_image_digest(d)
    } else {
        set
    };
    set.to_btreemap().map_err(anyhow::Error::new)
}

pub fn write_job_file(path: &Path, job: &VmJob) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create job dir: {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(job)?;
    bytes.push(b'\n');
    std::fs::write(path, &bytes).with_context(|| format!("write job file: {}", path.display()))?;
    Ok(())
}

pub fn resolve_sibling_or_path(name: &str) -> PathBuf {
    let Ok(exe) = std::env::current_exe() else {
        return PathBuf::from(name);
    };
    let Some(dir) = exe.parent() else {
        return PathBuf::from(name);
    };

    let sibling = dir.join(name);
    if sibling.is_file() {
        return sibling;
    }
    if dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "deps")
    {
        if let Some(parent) = dir.parent() {
            let sibling = parent.join(name);
            if sibling.is_file() {
                return sibling;
            }
        }
    }

    PathBuf::from(name)
}

pub fn touch_done_marker(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create done dir: {}", parent.display()))?;
    }
    std::fs::write(path, b"done\n")
        .with_context(|| format!("write done marker: {}", path.display()))?;
    Ok(())
}

pub fn spawn_reaper(reaper_bin: &Path, job_file: &Path) -> Result<()> {
    let mut cmd = Command::new(reaper_bin);
    cmd.arg("--job").arg(job_file);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    if libc::setpgid(0, 0) == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                } else {
                    let _ = libc::setpgid(0, 0);
                }
                Ok(())
            });
        }
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("spawn reaper: {}", reaper_bin.display()))?;

    reaper_joiner::register(child);
    Ok(())
}

#[derive(Debug)]
pub struct SpawnedChild {
    pub pid: u32,
    pub child: std::process::Child,
}

#[derive(Debug, Serialize)]
struct GuestRequestJson {
    schema_version: &'static str,
    run_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mounts: Vec<GuestMountJson>,
    exec: GuestExecJson,
    expect: GuestExpectJson,
    limits: GuestLimitsJson,
}

#[derive(Debug, Serialize)]
struct GuestMountJson {
    tag: String,
    guest_path: String,
    readonly: bool,
}

#[derive(Debug, Serialize)]
struct GuestExecJson {
    path: String,
    argv: Vec<String>,
    env: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct GuestExpectJson {
    stdout_schema_version: &'static str,
    stdout_is_single_json_object: bool,
}

#[derive(Debug, Serialize)]
struct GuestLimitsJson {
    wall_ms: u64,
    stdout_max_bytes: u64,
    stderr_max_bytes: u64,
}

fn resolve_vz_guest_bundle(spec_image: &str) -> Result<PathBuf> {
    if let Ok(raw) = std::env::var(ENV_VZ_GUEST_BUNDLE) {
        let p = PathBuf::from(raw);
        if !p.is_dir() {
            anyhow::bail!(
                "{ENV_VZ_GUEST_BUNDLE} points to a non-directory path: {}",
                p.display()
            );
        }
        return Ok(p);
    }

    if !spec_image.trim().is_empty() {
        let p = PathBuf::from(spec_image);
        if p.is_dir() {
            return Ok(p);
        }
    }

    anyhow::bail!(
        "missing VZ guest bundle directory\n\nfix:\n  - set {ENV_VZ_GUEST_BUNDLE}=/path/to/guest.bundle (see scripts/build_vz_guest_bundle.sh)"
    )
}

fn resolve_guest_exec_path(argv0: &str) -> Result<String> {
    if argv0.starts_with('/') {
        return Ok(argv0.to_string());
    }
    match argv0 {
        "x07" => Ok("/usr/local/bin/x07".to_string()),
        "x07-os-runner" => Ok("/usr/local/bin/x07-os-runner".to_string()),
        "x07-guestd" => Ok("/usr/local/bin/x07-guestd".to_string()),
        other => {
            anyhow::bail!("vz backend requires argv[0] to be an absolute path (got {other:?})")
        }
    }
}

fn write_guest_request_json(job_in: &Path, req: &GuestRequestJson) -> Result<()> {
    let path = job_in.join("request.json");
    let mut bytes = serde_json::to_vec_pretty(req)?;
    bytes.push(b'\n');
    std::fs::write(&path, &bytes)
        .with_context(|| format!("write request.json: {}", path.display()))?;
    Ok(())
}

pub fn vz_scratch_rootfs_path(state_dir: &Path) -> PathBuf {
    state_dir.join("rootfs.cow.img")
}

pub fn vz_cleanup_scratch(state_dir: &Path) -> Result<()> {
    let p = vz_scratch_rootfs_path(state_dir);
    if p.is_file() {
        std::fs::remove_file(&p)
            .with_context(|| format!("remove vz scratch image: {}", p.display()))?;
    }
    Ok(())
}

pub fn spawn_vz_helper(spec: &RunSpec, state_dir: &Path) -> Result<SpawnedChild> {
    if spec.backend != VmBackend::Vz {
        anyhow::bail!("spawn_vz_helper: backend mismatch (expected vz)");
    }

    if !cfg!(target_os = "macos") {
        anyhow::bail!("vz backend is only supported on macOS");
    }

    let bundle_dir = resolve_vz_guest_bundle(&spec.image)?;
    if !bundle_dir.join("manifest.json").is_file() {
        anyhow::bail!(
            "invalid VZ guest bundle (missing manifest.json): {}",
            bundle_dir.display()
        );
    }

    let job_in_guest_path = Path::new("/x07/in");
    let job_out_guest_path = Path::new("/x07/out");

    let job_in = spec
        .mounts
        .iter()
        .find(|m| m.guest_path == job_in_guest_path)
        .map(|m| m.host_path.clone())
        .context("vz backend requires a /x07/in mount")?;
    let job_out = spec
        .mounts
        .iter()
        .find(|m| m.guest_path == job_out_guest_path)
        .map(|m| m.host_path.clone())
        .context("vz backend requires a /x07/out mount")?;

    let extra_mounts: Vec<&MountSpec> = spec
        .mounts
        .iter()
        .filter(|m| m.guest_path != job_in_guest_path && m.guest_path != job_out_guest_path)
        .collect();
    if extra_mounts.len() > 64 {
        anyhow::bail!(
            "vz backend supports at most 64 extra mounts, got {}",
            extra_mounts.len()
        );
    }

    let exec_path =
        resolve_guest_exec_path(spec.argv.first().map(|s| s.as_str()).unwrap_or_default())?;

    let mut env = spec.env.clone();
    if let Some(wd) = spec.workdir.as_ref() {
        env.insert("X07_GUESTD_WORKDIR".to_string(), wd.display().to_string());
    }
    env.entry("PATH".to_string()).or_insert_with(|| {
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()
    });
    env.entry("HOME".to_string())
        .or_insert_with(|| "/root".to_string());
    env.entry("LC_ALL".to_string())
        .or_insert_with(|| "C".to_string());

    let mut req_mounts: Vec<GuestMountJson> = Vec::new();
    let mut shares: Vec<(String, PathBuf, bool)> = Vec::new();
    shares.push(("x07in".to_string(), job_in.clone(), true));
    shares.push(("x07out".to_string(), job_out.clone(), false));

    for (idx, m) in extra_mounts.iter().enumerate() {
        let tag = format!("x07m{idx}");
        req_mounts.push(GuestMountJson {
            tag: tag.clone(),
            guest_path: m.guest_path.display().to_string(),
            readonly: m.readonly,
        });
        shares.push((tag, m.host_path.clone(), m.readonly));
    }

    let req = GuestRequestJson {
        schema_version: "x07.guest.request@1",
        run_id: spec.run_id.clone(),
        mounts: req_mounts,
        exec: GuestExecJson {
            path: exec_path,
            argv: spec.argv.clone(),
            env,
        },
        expect: GuestExpectJson {
            stdout_schema_version: X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
            stdout_is_single_json_object: true,
        },
        limits: GuestLimitsJson {
            wall_ms: spec.limits.wall_ms.max(1),
            stdout_max_bytes: spec.limits.max_stdout_bytes.try_into().unwrap_or(u64::MAX),
            stderr_max_bytes: spec.limits.max_stderr_bytes.try_into().unwrap_or(u64::MAX),
        },
    };
    write_guest_request_json(&job_in, &req)?;

    let helper = resolve_vz_helper_bin()?;

    let mut cmd = Command::new(helper);
    cmd.arg("run");
    cmd.arg("--run-id").arg(&spec.run_id);
    cmd.arg("--bundle").arg(bundle_dir);
    cmd.arg("--state-dir").arg(state_dir);

    let mem_bytes = spec.limits.mem_bytes.unwrap_or(512 * 1024 * 1024);
    cmd.arg("--mem-bytes").arg(mem_bytes.to_string());
    if let Some(v) = spec.limits.vcpus {
        cmd.arg("--cpus").arg(v.to_string());
    }

    cmd.arg("--net").arg(match spec.limits.network {
        NetworkMode::None => "none",
        NetworkMode::Default => "nat",
    });

    cmd.arg("--wall-ms")
        .arg(spec.limits.wall_ms.max(1).to_string());
    cmd.arg("--grace-ms")
        .arg(spec.limits.grace_ms.max(1).to_string());

    for (tag, path, readonly) in shares {
        cmd.arg("--share");
        cmd.arg(tag);
        cmd.arg(path);
        cmd.arg(if readonly { "ro" } else { "rw" });
    }

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 && libc::setpgid(0, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let child = cmd.spawn().context("spawn vz helper")?;
    let pid = child.id();

    Ok(SpawnedChild { pid, child })
}

pub fn hard_kill_pid_and_group(pid: u32) {
    #[cfg(unix)]
    {
        let Ok(pid) = i32::try_from(pid) else {
            return;
        };
        unsafe {
            let _ = libc::kill(-pid, libc::SIGKILL);
            let _ = libc::kill(pid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

fn validate_mount_kv_string_safe(path: &Path, label: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        for &bad in [b',', b'\0', b'\n', b'\r'].iter() {
            if path.as_os_str().as_bytes().contains(&bad) {
                anyhow::bail!(
                    "{label} mount path contains disallowed byte {bad:?}: {}",
                    path.display()
                );
            }
        }
    }
    #[cfg(not(unix))]
    {
        let s = path.as_os_str().to_string_lossy();
        for bad in [",", "\0", "\n", "\r"] {
            if s.contains(bad) {
                anyhow::bail!(
                    "{label} mount path contains disallowed sequence {bad:?}: {}",
                    path.display()
                );
            }
        }
    }

    Ok(())
}

fn run_docker_like(
    bin: &str,
    spec: &RunSpec,
    container_id: &str,
    labels: &BTreeMap<String, String>,
    include_annotations: bool,
) -> Result<RunOutput> {
    let mut cmd = Command::new(bin);
    cmd.arg("run");
    cmd.arg("--rm");
    cmd.arg("--name").arg(container_id);

    for (k, v) in labels {
        cmd.arg("--label").arg(format!("{k}={v}"));
        if include_annotations {
            cmd.arg("--annotation").arg(format!("{k}={v}"));
        }
    }

    if let Some(mem_bytes) = spec.limits.mem_bytes {
        // Docker expects a human-ish unit; round up to MiB.
        let mib = (mem_bytes.saturating_add(1024 * 1024 - 1)) / (1024 * 1024);
        cmd.arg("--memory").arg(format!("{mib}m"));
    }
    if let Some(vcpus) = spec.limits.vcpus {
        cmd.arg("--cpus").arg(vcpus.to_string());
    }

    match spec.limits.network {
        NetworkMode::None => {
            cmd.arg("--network").arg("none");
        }
        NetworkMode::Default => {}
    }

    if let Some(workdir) = spec.workdir.as_ref() {
        cmd.arg("--workdir").arg(workdir);
    }

    for (k, v) in &spec.env {
        cmd.arg("--env").arg(format!("{k}={v}"));
    }

    for m in &spec.mounts {
        validate_mount_kv_string_safe(&m.host_path, "host")?;
        validate_mount_kv_string_safe(&m.guest_path, "guest")?;

        let mut mount = format!(
            "type=bind,source={},target={}",
            m.host_path.display(),
            m.guest_path.display()
        );
        if m.readonly {
            mount.push_str(",readonly");
        }
        cmd.arg("--mount").arg(mount);
    }

    cmd.arg(&spec.image);
    for a in &spec.argv {
        cmd.arg(a);
    }

    run_command_capped(
        cmd,
        spec.limits.wall_ms,
        spec.limits.max_stdout_bytes,
        spec.limits.max_stderr_bytes,
    )
}

pub fn run_docker(
    spec: &RunSpec,
    container_id: &str,
    labels: &BTreeMap<String, String>,
) -> Result<RunOutput> {
    run_docker_like("docker", spec, container_id, labels, false)
}

pub fn run_podman(
    spec: &RunSpec,
    container_id: &str,
    labels: &BTreeMap<String, String>,
) -> Result<RunOutput> {
    run_docker_like("podman", spec, container_id, labels, true)
}

pub fn run_apple_container(
    spec: &RunSpec,
    container_id: &str,
    labels: &BTreeMap<String, String>,
) -> Result<RunOutput> {
    let mut cmd = Command::new("container");
    cmd.arg("run");
    cmd.arg("--name").arg(container_id);
    cmd.arg("--rm");

    for (k, v) in labels {
        cmd.arg("--label").arg(format!("{k}={v}"));
    }

    if let Some(mem_bytes) = spec.limits.mem_bytes {
        cmd.arg("--memory").arg(mem_bytes.to_string());
    }
    if let Some(vcpus) = spec.limits.vcpus {
        cmd.arg("--cpus").arg(vcpus.to_string());
    }

    match spec.limits.network {
        NetworkMode::None => {
            cmd.arg("--network").arg("none");
        }
        NetworkMode::Default => {
            cmd.arg("--network").arg("default");
        }
    }

    if let Some(workdir) = spec.workdir.as_ref() {
        cmd.arg("--workdir").arg(workdir);
    }

    for (k, v) in &spec.env {
        cmd.arg("--env").arg(format!("{k}={v}"));
    }

    for m in &spec.mounts {
        validate_mount_kv_string_safe(&m.host_path, "host")?;
        validate_mount_kv_string_safe(&m.guest_path, "guest")?;

        let mut mount = format!(
            "type=bind,source={},target={}",
            m.host_path.display(),
            m.guest_path.display()
        );
        if m.readonly {
            mount.push_str(",readonly");
        }
        cmd.arg("--mount").arg(mount);
    }

    cmd.arg(&spec.image);
    for a in &spec.argv {
        cmd.arg(a);
    }

    run_command_capped(
        cmd,
        spec.limits.wall_ms,
        spec.limits.max_stdout_bytes,
        spec.limits.max_stderr_bytes,
    )
}

fn docker_like_soft_stop(bin: &str, container_id: &str, grace_ms: u64) -> Result<()> {
    let secs = (grace_ms.saturating_add(999) / 1000).max(1);
    let mut cmd = Command::new(bin);
    cmd.arg("stop")
        .arg("--time")
        .arg(secs.to_string())
        .arg(container_id);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("{bin} stop {container_id}"))?;
    Ok(())
}

pub fn docker_soft_stop(container_id: &str, grace_ms: u64) -> Result<()> {
    docker_like_soft_stop("docker", container_id, grace_ms)
}

pub fn podman_soft_stop(container_id: &str, grace_ms: u64) -> Result<()> {
    docker_like_soft_stop("podman", container_id, grace_ms)
}

pub fn apple_container_soft_stop(container_id: &str) -> Result<()> {
    let mut cmd = Command::new("container");
    cmd.arg("kill")
        .arg("--signal")
        .arg("SIGTERM")
        .arg(container_id);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("container kill SIGTERM {container_id}"))?;
    Ok(())
}

fn docker_like_hard_kill(bin: &str, container_id: &str) -> Result<()> {
    let mut cmd = Command::new(bin);
    cmd.arg("kill")
        .arg("--signal")
        .arg("SIGKILL")
        .arg(container_id);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("{bin} kill {container_id}"))?;
    Ok(())
}

pub fn docker_hard_kill(container_id: &str) -> Result<()> {
    docker_like_hard_kill("docker", container_id)
}

pub fn podman_hard_kill(container_id: &str) -> Result<()> {
    docker_like_hard_kill("podman", container_id)
}

pub fn apple_container_hard_kill(container_id: &str) -> Result<()> {
    let mut cmd = Command::new("container");
    cmd.arg("kill")
        .arg("--signal")
        .arg("KILL")
        .arg(container_id);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("container kill KILL {container_id}"))?;
    Ok(())
}

fn docker_like_cleanup(bin: &str, container_id: &str) -> Result<()> {
    let mut cmd = Command::new(bin);
    cmd.arg("rm").arg("-f").arg(container_id);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("{bin} rm -f {container_id}"))?;
    Ok(())
}

pub fn docker_cleanup(container_id: &str) -> Result<()> {
    docker_like_cleanup("docker", container_id)
}

pub fn podman_cleanup(container_id: &str) -> Result<()> {
    docker_like_cleanup("podman", container_id)
}

pub fn apple_container_cleanup(container_id: &str) -> Result<()> {
    let mut cmd = Command::new("container");
    cmd.arg("delete").arg("--force").arg(container_id);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("container delete --force {container_id}"))?;
    Ok(())
}

fn ctr_base_args(cfg: &FirecrackerCtrConfig) -> Vec<OsString> {
    vec![
        OsString::from("--address"),
        OsString::from(cfg.address.clone()),
        OsString::from("--namespace"),
        OsString::from(cfg.namespace.clone()),
    ]
}

fn duration_to_ctr_timeout_arg(timeout: Duration) -> OsString {
    let secs = timeout.as_secs().max(1);
    OsString::from(format!("{secs}s"))
}

pub fn run_firecracker_ctr(
    spec: &RunSpec,
    cfg: &FirecrackerCtrConfig,
    container_id: &str,
    labels: &BTreeMap<String, String>,
) -> Result<RunOutput> {
    let mut cmd = Command::new(&cfg.bin);
    cmd.args(ctr_base_args(cfg));
    cmd.arg("run");
    cmd.arg("--rm");
    cmd.arg("--runtime").arg(&cfg.runtime);
    cmd.arg("--snapshotter").arg(&cfg.snapshotter);

    for (k, v) in labels {
        cmd.arg("--label").arg(format!("{k}={v}"));
        cmd.arg("--annotation").arg(format!("{k}={v}"));
    }

    if let Some(mem_bytes) = spec.limits.mem_bytes {
        cmd.arg("--memory-limit").arg(mem_bytes.to_string());
    }
    if let Some(vcpus) = spec.limits.vcpus {
        cmd.arg("--cpus").arg(vcpus.to_string());
    }

    match spec.limits.network {
        NetworkMode::None => {}
        NetworkMode::Default => {
            cmd.arg("--cni");
        }
    }

    if let Some(workdir) = spec.workdir.as_ref() {
        cmd.arg("--cwd").arg(workdir);
    }

    for (k, v) in &spec.env {
        cmd.arg("--env").arg(format!("{k}={v}"));
    }

    for m in &spec.mounts {
        validate_mount_kv_string_safe(&m.host_path, "host")?;
        validate_mount_kv_string_safe(&m.guest_path, "guest")?;

        let options = if m.readonly { "rbind:ro" } else { "rbind" };
        cmd.arg("--mount").arg(format!(
            "type=bind,src={},dst={},options={options}",
            m.host_path.display(),
            m.guest_path.display()
        ));
    }

    cmd.arg(&spec.image);
    cmd.arg(container_id);
    for a in &spec.argv {
        cmd.arg(a);
    }

    run_command_capped(
        cmd,
        spec.limits.wall_ms,
        spec.limits.max_stdout_bytes,
        spec.limits.max_stderr_bytes,
    )
}

pub fn firecracker_ctr_soft_stop(
    cfg: &FirecrackerCtrConfig,
    container_id: &str,
    grace_ms: u64,
) -> Result<()> {
    let _ = grace_ms;
    let mut cmd = Command::new(&cfg.bin);
    cmd.args(ctr_base_args(cfg))
        .arg("--timeout")
        .arg(duration_to_ctr_timeout_arg(Duration::from_secs(2)))
        .args([
            OsString::from("tasks"),
            OsString::from("kill"),
            OsString::from("--all"),
            OsString::from("--signal"),
            OsString::from("SIGTERM"),
            OsString::from(container_id),
        ]);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("firecracker-ctr tasks kill SIGTERM {container_id}"))?;
    Ok(())
}

pub fn firecracker_ctr_hard_kill(cfg: &FirecrackerCtrConfig, container_id: &str) -> Result<()> {
    let mut cmd = Command::new(&cfg.bin);
    cmd.args(ctr_base_args(cfg))
        .arg("--timeout")
        .arg(duration_to_ctr_timeout_arg(Duration::from_secs(2)))
        .args([
            OsString::from("tasks"),
            OsString::from("kill"),
            OsString::from("--all"),
            OsString::from("--signal"),
            OsString::from("SIGKILL"),
            OsString::from(container_id),
        ]);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("firecracker-ctr tasks kill SIGKILL {container_id}"))?;
    Ok(())
}

pub fn firecracker_ctr_cleanup(cfg: &FirecrackerCtrConfig, container_id: &str) -> Result<()> {
    let mut cmd = Command::new(&cfg.bin);
    cmd.args(ctr_base_args(cfg))
        .arg("--timeout")
        .arg(duration_to_ctr_timeout_arg(Duration::from_secs(2)))
        .args([
            OsString::from("tasks"),
            OsString::from("delete"),
            OsString::from("--force"),
            OsString::from(container_id),
        ]);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("firecracker-ctr tasks delete --force {container_id}"))?;

    let mut cmd = Command::new(&cfg.bin);
    cmd.args(ctr_base_args(cfg))
        .arg("--timeout")
        .arg(duration_to_ctr_timeout_arg(Duration::from_secs(2)))
        .args([
            OsString::from("containers"),
            OsString::from("delete"),
            OsString::from(container_id),
        ]);
    let _ = run_command_capped(cmd, 2_000, 64 * 1024, 64 * 1024)
        .with_context(|| format!("firecracker-ctr containers delete {container_id}"))?;
    Ok(())
}

fn wait_child_with_wall_timeout_ms(
    child: &mut std::process::Child,
    wall_ms: u64,
) -> Result<(std::process::ExitStatus, bool)> {
    let wall_limit = Duration::from_millis(wall_ms.max(1));
    let start = Instant::now();
    let deadline = start.checked_add(wall_limit);

    loop {
        if let Some(status) = child.try_wait().context("try_wait child")? {
            return Ok((status, false));
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            let _ = child.kill();
            let status = child.wait().context("wait child after kill")?;
            return Ok((status, true));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn wait_child_output_capped(
    mut child: std::process::Child,
    wall_ms: u64,
    stdout_cap: usize,
    stderr_cap: usize,
) -> Result<RunOutput> {
    let stdout = child.stdout.take().context("take stdout")?;
    let stderr = child.stderr.take().context("take stderr")?;

    let stdout_thread = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        x07_host_runner::read_to_end_capped(stdout, stdout_cap)
    });
    let stderr_thread = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        x07_host_runner::read_to_end_capped(stderr, stderr_cap)
    });

    let (status, timed_out) = wait_child_with_wall_timeout_ms(&mut child, wall_ms)?;
    let (stdout_bytes, stdout_truncated) = stdout_thread
        .join()
        .unwrap_or_else(|_| Ok((Vec::new(), false)))?;
    let (stderr_bytes, stderr_truncated) = stderr_thread
        .join()
        .unwrap_or_else(|_| Ok((Vec::new(), false)))?;

    #[cfg(unix)]
    let exit_signal = {
        use std::os::unix::process::ExitStatusExt as _;
        status.signal()
    };
    #[cfg(not(unix))]
    let exit_signal: Option<i32> = None;

    let exit_status = match status.code() {
        Some(code) => code,
        None => exit_signal.map(|s| 128 + s).unwrap_or(1),
    };

    Ok(RunOutput {
        exit_status,
        timed_out,
        stdout: stdout_bytes,
        stderr: stderr_bytes,
        stdout_truncated,
        stderr_truncated,
    })
}

fn run_command_capped(
    mut cmd: Command,
    wall_ms: u64,
    stdout_cap: usize,
    stderr_cap: usize,
) -> Result<RunOutput> {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = cmd.spawn().context("spawn command")?;
    wait_child_output_capped(child, wall_ms, stdout_cap, stderr_cap)
}

pub fn to_os_args(args: &[String]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

pub fn append_root_mounts(
    mounts: &mut Vec<MountSpec>,
    read_roots: &[String],
    write_roots: &[String],
    base_host: &Path,
    base_guest: &Path,
) -> Result<()> {
    use std::collections::BTreeMap as Map;

    let mut by_guest: Map<PathBuf, MountSpec> = Map::new();

    let mut add = |root: &str, readonly: bool| -> Result<()> {
        let root_path = PathBuf::from(root);
        let (host_path, guest_path) = if root_path.is_absolute() {
            (root_path.clone(), root_path)
        } else {
            let host = base_host.join(&root_path);
            let guest = normalize_abs_path(&base_guest.join(&root_path))?;
            (host, guest)
        };

        if !readonly && !host_path.exists() {
            std::fs::create_dir_all(&host_path)
                .with_context(|| format!("create write_root dir: {}", host_path.display()))?;
        }
        if readonly && !host_path.exists() {
            anyhow::bail!("root does not exist: {}", host_path.display());
        }

        let entry = by_guest.entry(guest_path.clone()).or_insert(MountSpec {
            host_path,
            guest_path,
            readonly,
        });
        if !readonly {
            entry.readonly = false;
        }
        Ok(())
    };

    for r in read_roots {
        add(r, true)?;
    }
    for r in write_roots {
        add(r, false)?;
    }

    mounts.extend(by_guest.into_values());
    Ok(())
}

pub fn normalize_abs_path(p: &Path) -> Result<PathBuf> {
    if !p.is_absolute() {
        anyhow::bail!("expected absolute path, got {}", p.display());
    }

    let mut out = PathBuf::new();
    out.push(Path::new("/"));
    for comp in p.components() {
        use std::path::Component;
        match comp {
            Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
                if out.as_os_str().is_empty() {
                    out.push(Path::new("/"));
                }
            }
            Component::Normal(c) => out.push(c),
            Component::Prefix(_) => {
                anyhow::bail!("unexpected Windows prefix in path {}", p.display());
            }
        }
    }
    Ok(out)
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.is_dir() {
        anyhow::bail!(
            "copy_dir_recursive: source is not a directory: {}",
            src.display()
        );
    }
    std::fs::create_dir_all(dst)
        .with_context(|| format!("copy_dir_recursive: create dst dir: {}", dst.display()))?;

    for entry in std::fs::read_dir(src)
        .with_context(|| format!("copy_dir_recursive: read dir: {}", src.display()))?
    {
        let entry = entry
            .with_context(|| format!("copy_dir_recursive: read entry in {}", src.display()))?;
        let from_path = entry.path();
        let file_name = entry.file_name();
        let to_path = dst.join(file_name);

        let ty = entry
            .file_type()
            .with_context(|| format!("copy_dir_recursive: file_type {}", from_path.display()))?;

        if ty.is_dir() {
            copy_dir_recursive(&from_path, &to_path)?;
            continue;
        }

        if ty.is_file() {
            if let Some(parent) = to_path.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("copy_dir_recursive: create parent {}", parent.display())
                })?;
            }
            std::fs::copy(&from_path, &to_path).with_context(|| {
                format!(
                    "copy_dir_recursive: copy file {} -> {}",
                    from_path.display(),
                    to_path.display()
                )
            })?;
            continue;
        }

        if ty.is_symlink() {
            let target = std::fs::read_link(&from_path).with_context(|| {
                format!(
                    "copy_dir_recursive: read symlink target: {}",
                    from_path.display()
                )
            })?;

            #[cfg(unix)]
            {
                use std::os::unix::fs as unix_fs;
                unix_fs::symlink(&target, &to_path).with_context(|| {
                    format!(
                        "copy_dir_recursive: create symlink {} -> {} (target {})",
                        to_path.display(),
                        from_path.display(),
                        target.display()
                    )
                })?;
            }

            #[cfg(not(unix))]
            {
                let _ = (target, to_path);
                anyhow::bail!("copy_dir_recursive: symlinks are not supported on this platform");
            }
            continue;
        }

        anyhow::bail!(
            "copy_dir_recursive: unsupported file type: {}",
            from_path.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grace_ms_is_bounded() {
        assert_eq!(default_grace_ms(1), 1);
        assert_eq!(default_grace_ms(500), 50);
        assert_eq!(default_grace_ms(10_000), 1_000);
        assert!(default_grace_ms(100_000) <= 2_000);
    }

    #[test]
    fn container_id_validation() {
        validate_container_id("x07-abc.DEF_123").unwrap();
        assert!(validate_container_id("").is_err());
        assert!(validate_container_id("x07-!").is_err());
        assert!(validate_container_id(&"a".repeat(129)).is_err());
    }

    #[test]
    fn mount_kv_string_validation_rejects_comma() {
        assert!(validate_mount_kv_string_safe(Path::new("/tmp/has,comma"), "host").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn mount_kv_string_validation_rejects_nul() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt as _;

        let os = OsString::from_vec(vec![b'a', 0, b'b']);
        let p = PathBuf::from(os);
        assert!(validate_mount_kv_string_safe(&p, "host").is_err());
    }
}
