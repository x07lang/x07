use std::path::{Path, PathBuf};

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
use x07_runner_common::{auto_ffi, os_env, os_paths, os_policy};
use x07_worlds::WorldId;
use x07c::project;

use crate::policy_overrides::{PolicyOverrides, PolicyResolution};
use crate::repair::RepairArgs;

const DEFAULT_SOLVE_FUEL: u64 = 50_000_000;
const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;

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

    /// Output path. If a directory, the binary name defaults to `app`.
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,

    /// Policy JSON (required for `run-os-sandboxed`; not a hardened sandbox).
    #[arg(long, value_name = "PATH")]
    pub policy: Option<PathBuf>,

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

    #[arg(long, value_name = "PATH")]
    pub report_out: Option<PathBuf>,

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

pub fn cmd_bundle(args: BundleArgs) -> Result<std::process::ExitCode> {
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

    let out_path = resolve_out_path(&args.out, target_kind, &project_root);
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

    let mut compile_options = x07c::world_config::compile_options_for_world(world, module_roots);
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
        },
        report: host_report,
    };

    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');

    if let Some(path) = &args.report_out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create report-out dir: {}", parent.display()))?;
        }
        std::fs::write(path, &bytes).with_context(|| format!("write: {}", path.display()))?;
    }

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

fn normalize_exe_extension(mut out: PathBuf) -> PathBuf {
    if cfg!(windows) {
        let has_exe = out
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
        if !has_exe {
            out.set_extension("exe");
        }
    }
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
