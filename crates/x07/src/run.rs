use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use base64::Engine;
use clap::{Args, ValueEnum};
use jsonschema::Draft;
use serde::{Deserialize, Serialize};
use serde_json::{value::RawValue, Value};
use sha2::{Digest, Sha256};
use x07_contracts::X07_RUN_REPORT_SCHEMA_VERSION;
use x07_host_runner::CcProfile;
use x07_worlds::WorldId;
use x07c::project;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const DEFAULT_SOLVE_FUEL: u64 = 50_000_000;
const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;
const RUN_OS_POLICY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../schemas/run-os-policy.schema.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum ReportMode {
    Runner,
    Wrapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum RunnerMode {
    Auto,
    Host,
    Os,
}

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    /// Run a project (manifest + lockfile).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Compile+run a single `*.x07.json` file.
    #[arg(long, value_name = "PATH")]
    pub program: Option<PathBuf>,

    /// Run a precompiled executable produced by the X07 toolchain runners.
    #[arg(long, value_name = "PATH")]
    pub artifact: Option<PathBuf>,

    #[arg(long, value_enum)]
    pub world: Option<WorldId>,

    /// Run profile name (resolved from `x07.json.profiles`).
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Force runner selection.
    #[arg(long, value_enum, conflicts_with_all = ["host", "os"])]
    pub runner: Option<RunnerMode>,

    /// Force deterministic host runner selection (`solve-*` worlds).
    #[arg(long)]
    pub host: bool,

    /// Force OS runner selection (`run-os*` worlds).
    #[arg(long)]
    pub os: bool,

    #[arg(long, conflicts_with_all = ["stdin", "input_b64"], value_name = "PATH")]
    pub input: Option<PathBuf>,

    #[arg(long, conflicts_with_all = ["input", "input_b64"])]
    pub stdin: bool,

    #[arg(long, conflicts_with_all = ["input", "stdin"], value_name = "BASE64")]
    pub input_b64: Option<String>,

    /// Trailing arguments after `--` are encoded as `argv_v1` and provided as runner input.
    #[arg(
        trailing_var_arg = true,
        value_name = "ARG",
        conflicts_with_all = ["input", "stdin", "input_b64"]
    )]
    pub argv: Vec<String>,

    #[arg(long, value_enum)]
    pub cc_profile: Option<CcProfile>,

    /// Override the generated C source budget (in bytes).
    #[arg(long, value_name = "BYTES")]
    pub max_c_bytes: Option<usize>,

    #[arg(long, value_name = "PATH")]
    pub compiled_out: Option<PathBuf>,

    #[arg(long)]
    pub compile_only: bool,

    #[arg(long, value_name = "PATH")]
    pub module_root: Vec<PathBuf>,

    /// A base directory for fixtures (shorthand for world-specific fixture dirs).
    #[arg(long, value_name = "DIR")]
    pub fixtures: Option<PathBuf>,

    #[arg(long, value_name = "PATH")]
    pub fixture_fs_dir: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub fixture_fs_root: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub fixture_fs_latency_index: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub fixture_rr_dir: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub fixture_rr_index: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub fixture_kv_dir: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub fixture_kv_seed: Option<PathBuf>,

    /// Policy JSON (required for `run-os-sandboxed`; not a hardened sandbox).
    #[arg(long, value_name = "PATH")]
    pub policy: Option<PathBuf>,

    /// Append network destinations to the sandbox policy allowlist (repeatable).
    #[arg(long, value_name = "HOST:PORTS")]
    pub allow_host: Vec<String>,

    /// Read allow-host entries from a file (repeatable).
    #[arg(long, value_name = "PATH")]
    pub allow_host_file: Vec<PathBuf>,

    /// Remove network destinations from the sandbox policy allowlist (repeatable; deny wins).
    #[arg(long, value_name = "HOST:PORTS")]
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

    #[arg(long)]
    pub solve_fuel: Option<u64>,

    #[arg(long)]
    pub max_memory_bytes: Option<usize>,

    #[arg(long)]
    pub max_output_bytes: Option<usize>,

    #[arg(long)]
    pub cpu_time_limit_seconds: Option<u64>,

    #[arg(long)]
    pub debug_borrow_checks: bool,

    /// For OS worlds: collect and apply C FFI flags from dependency packages.
    #[arg(long, conflicts_with = "no_auto_ffi")]
    pub auto_ffi: bool,

    /// For OS worlds: disable automatic C FFI collection.
    #[arg(long, conflicts_with = "auto_ffi")]
    pub no_auto_ffi: bool,

    #[arg(long, value_enum, default_value_t = ReportMode::Runner)]
    pub report: ReportMode,

    #[arg(long, value_name = "PATH")]
    pub report_out: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct WrappedTarget {
    kind: &'static str,
    path: String,
    project_root: Option<String>,
    lockfile: Option<String>,
    resolved_module_roots: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WrappedReport {
    schema_version: &'static str,
    runner: &'static str,
    world: &'static str,
    target: WrappedTarget,
    report: Box<RawValue>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectRunProfilesFile {
    #[serde(default)]
    default_profile: Option<String>,
    #[serde(default)]
    profiles: Option<BTreeMap<String, ProjectRunProfile>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectRunProfile {
    world: String,
    #[serde(default)]
    policy: Option<String>,
    #[serde(default)]
    runner: Option<String>,
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    auto_ffi: Option<bool>,
    #[serde(default)]
    solve_fuel: Option<u64>,
    #[serde(default)]
    cpu_time_limit_seconds: Option<u64>,
    #[serde(default)]
    max_memory_bytes: Option<u64>,
    #[serde(default)]
    max_output_bytes: Option<u64>,
    #[serde(default)]
    cc_profile: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedProfile {
    world: WorldId,
    policy: Option<PathBuf>,
    runner: Option<RunnerMode>,
    input: Option<PathBuf>,
    auto_ffi: Option<bool>,
    solve_fuel: Option<u64>,
    cpu_time_limit_seconds: Option<u64>,
    max_memory_bytes: Option<usize>,
    max_output_bytes: Option<usize>,
    cc_profile: Option<CcProfile>,
}

#[derive(Debug)]
enum PolicyResolution {
    None,
    Base(PathBuf),
    Derived { derived: PathBuf },
    SchemaInvalid(Vec<String>),
}

pub fn cmd_run(args: RunArgs) -> Result<std::process::ExitCode> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let (target_kind, target_path, project_manifest) = resolve_target(&cwd, &args)?;

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
        Some(path) => Some(load_project_profiles(path)?),
        None => None,
    };

    let selected_profile = resolve_selected_profile(
        project_manifest.as_deref(),
        profiles_file.as_ref(),
        args.profile.as_deref(),
    )?;

    let world = resolve_world(
        &args,
        project_manifest.as_deref(),
        selected_profile.as_ref(),
    )?;
    let runner = resolve_runner(&args, selected_profile.as_ref(), world)?;

    let cc_profile = resolve_cc_profile(&args, selected_profile.as_ref());
    let solve_fuel = args
        .solve_fuel
        .or(selected_profile.as_ref().and_then(|p| p.solve_fuel))
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

    let profile_input = selected_profile.as_ref().and_then(|p| p.input.clone());
    let profile_input = profile_input.map(|p| resolve_project_relative(policy_root, &p));

    let (input_flag, temp_input) = prepare_input_flag(&cwd, &args, profile_input)?;
    let _temp_input_guard = temp_input;

    let policy_resolution =
        resolve_policy_for_run(&args, selected_profile.as_ref(), world, policy_root)?;
    let policy = match &policy_resolution {
        PolicyResolution::None => None,
        PolicyResolution::Base(p) => Some(p.clone()),
        PolicyResolution::Derived { derived, .. } => Some(derived.clone()),
        PolicyResolution::SchemaInvalid(errors) => {
            print_policy_schema_x07diag_stderr(errors.clone());
            return Ok(std::process::ExitCode::from(3));
        }
    };

    if let PolicyResolution::Derived { derived, .. } = &policy_resolution {
        let msg = format!("x07 run: using derived policy {}\n", derived.display());
        let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
    }

    if world == WorldId::RunOsSandboxed && target_kind == TargetKind::Artifact {
        anyhow::bail!("run-os-sandboxed does not support --artifact; use --program or --project so policy can be enforced at compile time");
    }

    if args.compile_only && runner != RunnerKind::Host {
        anyhow::bail!("--compile-only is only valid for deterministic solve worlds");
    }
    if args.compile_only && target_kind == TargetKind::Artifact {
        anyhow::bail!("--compile-only is only valid for --program or --project");
    }
    if !args.module_root.is_empty() && target_kind != TargetKind::Program {
        anyhow::bail!("--module-root is only valid with --program");
    }

    let mut argv: Vec<String> = vec![
        "--cc-profile".to_string(),
        match cc_profile {
            CcProfile::Default => "default".to_string(),
            CcProfile::Size => "size".to_string(),
        },
        "--world".to_string(),
        world.as_str().to_string(),
    ];

    if let Some(max_c_bytes) = args.max_c_bytes {
        argv.push("--max-c-bytes".to_string());
        argv.push(max_c_bytes.to_string());
    }

    if let Some(path) = &args.compiled_out {
        argv.push("--compiled-out".to_string());
        argv.push(path.display().to_string());
    }

    if args.compile_only {
        argv.push("--compile-only".to_string());
    }

    if let Some(flag) = input_flag {
        argv.extend(flag);
    }

    match runner {
        RunnerKind::Host => {
            argv.push("--solve-fuel".to_string());
            argv.push(solve_fuel.to_string());
            argv.push("--max-memory-bytes".to_string());
            argv.push(max_memory_bytes.to_string());
            if let Some(max_output_bytes) = max_output_bytes {
                argv.push("--max-output-bytes".to_string());
                argv.push(max_output_bytes.to_string());
            }
            if let Some(cpu) = cpu_time_limit_seconds {
                argv.push("--cpu-time-limit-seconds".to_string());
                argv.push(cpu.to_string());
            }
            if args.debug_borrow_checks {
                argv.push("--debug-borrow-checks".to_string());
            }

            let fixtures = resolve_fixtures(world, &args, project_root.as_deref())?;
            if let Some(dir) = fixtures.fs_dir {
                argv.push("--fixture-fs-dir".to_string());
                argv.push(dir.display().to_string());
            }
            if let Some(root) = fixtures.fs_root {
                argv.push("--fixture-fs-root".to_string());
                argv.push(root.display().to_string());
            }
            if let Some(idx) = fixtures.fs_latency_index {
                argv.push("--fixture-fs-latency-index".to_string());
                argv.push(idx.display().to_string());
            }
            if let Some(dir) = fixtures.rr_dir {
                argv.push("--fixture-rr-dir".to_string());
                argv.push(dir.display().to_string());
            }
            if let Some(idx) = fixtures.rr_index {
                argv.push("--fixture-rr-index".to_string());
                argv.push(idx.display().to_string());
            }
            if let Some(dir) = fixtures.kv_dir {
                argv.push("--fixture-kv-dir".to_string());
                argv.push(dir.display().to_string());
            }
            if let Some(seed) = fixtures.kv_seed {
                argv.push("--fixture-kv-seed".to_string());
                argv.push(seed.display().to_string());
            }
        }
        RunnerKind::Os => {
            argv.push("--solve-fuel".to_string());
            argv.push(solve_fuel.to_string());
            argv.push("--max-memory-bytes".to_string());
            argv.push(max_memory_bytes.to_string());
            if let Some(max_output_bytes) = max_output_bytes {
                argv.push("--max-output-bytes".to_string());
                argv.push(max_output_bytes.to_string());
            }
            if let Some(cpu) = cpu_time_limit_seconds {
                argv.push("--cpu-time-limit-seconds".to_string());
                argv.push(cpu.to_string());
            }
            if args.debug_borrow_checks {
                argv.push("--debug-borrow-checks".to_string());
            }

            if let Some(path) = policy.as_ref() {
                argv.push("--policy".to_string());
                argv.push(path.display().to_string());
            }

            if resolve_auto_ffi(&args, selected_profile.as_ref()) {
                argv.push("--auto-ffi".to_string());
            }
        }
    }

    match target_kind {
        TargetKind::Project => {
            argv.push("--project".to_string());
            argv.push(target_path.display().to_string());
        }
        TargetKind::Program => {
            let program = &target_path;
            argv.push("--program".to_string());
            argv.push(program.display().to_string());

            let module_roots = if !args.module_root.is_empty() {
                args.module_root.clone()
            } else {
                infer_program_module_roots(program, project_manifest.as_deref())?
            };
            for root in module_roots {
                argv.push("--module-root".to_string());
                argv.push(root.display().to_string());
            }
        }
        TargetKind::Artifact => {
            argv.push("--artifact".to_string());
            argv.push(target_path.display().to_string());
        }
    }

    let bin = match runner {
        RunnerKind::Host => resolve_sibling_or_path("x07-host-runner"),
        RunnerKind::Os => resolve_sibling_or_path("x07-os-runner"),
    };

    let output = Command::new(&bin)
        .args(&argv)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("exec {}", bin.display()))?;

    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).context("write stderr")?;

    let exit_code = output.status.code().unwrap_or(2);
    let runner_stdout = output.stdout;

    let emitted = match args.report {
        ReportMode::Runner => runner_stdout,
        ReportMode::Wrapped => {
            let runner_stdout_str =
                std::str::from_utf8(&runner_stdout).context("runner report is not utf-8")?;
            let report = RawValue::from_string(runner_stdout_str.to_string())
                .context("parse runner report JSON")?;

            let lockfile = project_manifest
                .as_deref()
                .and_then(|p| project_lockfile_path(p).ok());

            let resolved_module_roots = resolve_module_roots_for_wrapper(
                runner,
                target_kind,
                &target_path,
                project_manifest.as_deref(),
                &args,
                &bin,
            )?;

            let wrapped = WrappedReport {
                schema_version: X07_RUN_REPORT_SCHEMA_VERSION,
                runner: match runner {
                    RunnerKind::Host => "host",
                    RunnerKind::Os => "os",
                },
                world: world.as_str(),
                target: WrappedTarget {
                    kind: target_kind.as_str(),
                    path: target_path.display().to_string(),
                    project_root: project_root.as_ref().map(|p| p.display().to_string()),
                    lockfile: lockfile.as_ref().map(|p| p.display().to_string()),
                    resolved_module_roots: resolved_module_roots
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect(),
                },
                report,
            };

            let mut bytes = serde_json::to_vec_pretty(&wrapped)?;
            bytes.push(b'\n');
            bytes
        }
    };

    if let Some(path) = &args.report_out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create report-out dir: {}", parent.display()))?;
        }
        std::fs::write(path, &emitted).with_context(|| format!("write: {}", path.display()))?;
    }

    std::io::Write::write_all(&mut std::io::stdout(), &emitted).context("write stdout")?;

    Ok(std::process::ExitCode::from(exit_code as u8))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerKind {
    Host,
    Os,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Project,
    Program,
    Artifact,
}

impl TargetKind {
    fn as_str(self) -> &'static str {
        match self {
            TargetKind::Project => "project",
            TargetKind::Program => "program",
            TargetKind::Artifact => "artifact",
        }
    }
}

fn resolve_target(cwd: &Path, args: &RunArgs) -> Result<(TargetKind, PathBuf, Option<PathBuf>)> {
    let mut count = 0;
    if args.project.is_some() {
        count += 1;
    }
    if args.program.is_some() {
        count += 1;
    }
    if args.artifact.is_some() {
        count += 1;
    }
    if count > 1 {
        anyhow::bail!("set exactly one of --project, --program, or --artifact");
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
        let project_manifest = discover_project_manifest(base)?;
        return Ok((TargetKind::Program, path.to_path_buf(), project_manifest));
    }
    if let Some(path) = &args.artifact {
        return Ok((TargetKind::Artifact, path.to_path_buf(), None));
    }

    let found = discover_project_manifest(cwd)?
        .context("no project found (pass --project, --program, or --artifact)")?;
    Ok((TargetKind::Project, found.clone(), Some(found)))
}

fn load_project_profiles(project_manifest: &Path) -> Result<ProjectRunProfilesFile> {
    let bytes = std::fs::read(project_manifest)
        .with_context(|| format!("read project: {}", project_manifest.display()))?;
    let mut file: ProjectRunProfilesFile = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse project JSON: {}", project_manifest.display()))?;
    if let Some(dp) = file.default_profile.as_mut() {
        if dp.trim() != dp.as_str() {
            *dp = dp.trim().to_string();
        }
        if dp.is_empty() {
            file.default_profile = None;
        }
    }
    Ok(file)
}

fn resolve_selected_profile(
    project_manifest: Option<&Path>,
    profiles_file: Option<&ProjectRunProfilesFile>,
    cli_profile: Option<&str>,
) -> Result<Option<ResolvedProfile>> {
    let cli_profile = cli_profile.map(|s| s.trim()).filter(|s| !s.is_empty());

    let Some(_project_manifest) = project_manifest else {
        if cli_profile.is_some() {
            anyhow::bail!(
                "--profile requires a project manifest (--project or a discovered x07.json)"
            );
        }
        return Ok(None);
    };

    let Some(profiles_file) = profiles_file else {
        if cli_profile.is_some() {
            anyhow::bail!("--profile requires x07.json.profiles");
        }
        return Ok(None);
    };

    let Some(profiles) = profiles_file.profiles.as_ref() else {
        if cli_profile.is_some() {
            anyhow::bail!("--profile requires x07.json.profiles");
        }
        return Ok(None);
    };

    let selected = if let Some(name) = cli_profile {
        name.to_string()
    } else if let Some(default_profile) = profiles_file.default_profile.as_deref() {
        default_profile.to_string()
    } else if profiles.contains_key("dev") {
        "dev".to_string()
    } else if profiles.contains_key("os") {
        "os".to_string()
    } else {
        anyhow::bail!(
            "profiles present but default_profile missing; set default_profile or pass --profile"
        );
    };

    let profile = profiles.get(&selected).with_context(|| {
        format!(
            "unknown profile {selected:?} (available: {})",
            profiles.keys().cloned().collect::<Vec<String>>().join(", ")
        )
    })?;

    let world = x07c::world_config::parse_world_id(profile.world.trim())
        .with_context(|| format!("invalid profiles[{selected:?}].world {:?}", profile.world))?;

    let policy = profile
        .policy
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    if world == WorldId::RunOsSandboxed && policy.is_none() {
        anyhow::bail!("profiles[{selected:?}].policy is required when profiles[{selected:?}].world is run-os-sandboxed");
    }
    if world != WorldId::RunOsSandboxed && policy.is_some() {
        anyhow::bail!("profiles[{selected:?}].policy is only valid when profiles[{selected:?}].world is run-os-sandboxed");
    }

    let runner = profile
        .runner
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(parse_profile_runner)
        .transpose()
        .with_context(|| format!("invalid profiles[{selected:?}].runner"))?;

    let input = profile
        .input
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    let cc_profile = profile
        .cc_profile
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(parse_profile_cc_profile)
        .transpose()
        .with_context(|| format!("invalid profiles[{selected:?}].cc_profile"))?;

    let max_memory_bytes = profile
        .max_memory_bytes
        .map(u64_to_usize)
        .transpose()
        .with_context(|| format!("invalid profiles[{selected:?}].max_memory_bytes"))?;
    let max_output_bytes = profile
        .max_output_bytes
        .map(u64_to_usize)
        .transpose()
        .with_context(|| format!("invalid profiles[{selected:?}].max_output_bytes"))?;

    Ok(Some(ResolvedProfile {
        world,
        policy,
        runner,
        input,
        auto_ffi: profile.auto_ffi,
        solve_fuel: profile.solve_fuel,
        cpu_time_limit_seconds: profile.cpu_time_limit_seconds,
        max_memory_bytes,
        max_output_bytes,
        cc_profile,
    }))
}

fn resolve_world(
    args: &RunArgs,
    project_manifest: Option<&Path>,
    profile: Option<&ResolvedProfile>,
) -> Result<WorldId> {
    if let Some(world) = args.world {
        return Ok(world);
    }
    if args.os {
        return Ok(WorldId::RunOs);
    }
    if args.host {
        return Ok(WorldId::SolvePure);
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
    Ok(WorldId::SolvePure)
}

fn resolve_runner(
    args: &RunArgs,
    profile: Option<&ResolvedProfile>,
    world: WorldId,
) -> Result<RunnerKind> {
    if args.host && args.os {
        anyhow::bail!("--host and --os are mutually exclusive");
    }

    let mode = if let Some(mode) = args.runner {
        mode
    } else if args.host {
        RunnerMode::Host
    } else if args.os {
        RunnerMode::Os
    } else if let Some(mode) = profile.and_then(|p| p.runner) {
        mode
    } else {
        RunnerMode::Auto
    };

    let runner = match mode {
        RunnerMode::Auto => {
            if world.is_eval_world() {
                RunnerKind::Host
            } else {
                RunnerKind::Os
            }
        }
        RunnerMode::Host => RunnerKind::Host,
        RunnerMode::Os => RunnerKind::Os,
    };

    match runner {
        RunnerKind::Host if !world.is_eval_world() => {
            anyhow::bail!(
                "host runner is incompatible with --world {}",
                world.as_str()
            );
        }
        RunnerKind::Os if world.is_eval_world() => {
            anyhow::bail!("os runner is incompatible with --world {}", world.as_str());
        }
        _ => {}
    }

    Ok(runner)
}

fn resolve_cc_profile(args: &RunArgs, profile: Option<&ResolvedProfile>) -> CcProfile {
    args.cc_profile
        .or(profile.and_then(|p| p.cc_profile))
        .unwrap_or(CcProfile::Default)
}

fn resolve_auto_ffi(args: &RunArgs, profile: Option<&ResolvedProfile>) -> bool {
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

fn resolve_project_relative(project_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn parse_profile_runner(raw: &str) -> Result<RunnerMode> {
    match raw.trim() {
        "auto" => Ok(RunnerMode::Auto),
        "host" => Ok(RunnerMode::Host),
        "os" => Ok(RunnerMode::Os),
        other => anyhow::bail!("expected one of \"auto\", \"host\", or \"os\", got {other:?}"),
    }
}

fn parse_profile_cc_profile(raw: &str) -> Result<CcProfile> {
    match raw.trim() {
        "default" => Ok(CcProfile::Default),
        "size" => Ok(CcProfile::Size),
        other => anyhow::bail!("expected one of \"default\" or \"size\", got {other:?}"),
    }
}

fn u64_to_usize(v: u64) -> Result<usize> {
    usize::try_from(v).map_err(|_| anyhow::anyhow!("value is too large for this platform: {v}"))
}

fn resolve_policy_for_run(
    args: &RunArgs,
    profile: Option<&ResolvedProfile>,
    world: WorldId,
    policy_root: &Path,
) -> Result<PolicyResolution> {
    let has_policy_overrides = !args.allow_host.is_empty()
        || !args.allow_host_file.is_empty()
        || !args.deny_host.is_empty()
        || !args.deny_host_file.is_empty()
        || !args.allow_read_root.is_empty()
        || !args.allow_write_root.is_empty();

    if has_policy_overrides && world != WorldId::RunOsSandboxed {
        anyhow::bail!("--allow-host/--deny-host/--allow-read-root/--allow-write-root requires run-os-sandboxed (policy-enforced OS world)");
    }

    if args.policy.is_some() && world != WorldId::RunOsSandboxed {
        anyhow::bail!("--policy is only valid for --world run-os-sandboxed");
    }

    if world != WorldId::RunOsSandboxed {
        return Ok(PolicyResolution::None);
    }

    let base_policy = args
        .policy
        .clone()
        .or_else(|| profile.and_then(|p| p.policy.clone()))
        .context("run-os-sandboxed requires a policy file (--policy or profile policy)")?;
    let base_policy = resolve_project_relative(policy_root, &base_policy);

    if !base_policy.is_file() {
        anyhow::bail!("missing policy file: {}", base_policy.display());
    }

    if !has_policy_overrides {
        return Ok(PolicyResolution::Base(base_policy));
    }

    derive_policy_with_overrides(policy_root, &base_policy, args)
}

#[derive(Debug, Clone)]
struct DenySpec {
    all_ports: bool,
    ports: BTreeSet<u16>,
}

fn derive_policy_with_overrides(
    policy_root: &Path,
    base_policy_path: &Path,
    args: &RunArgs,
) -> Result<PolicyResolution> {
    let base_bytes = match std::fs::read(base_policy_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            anyhow::bail!("read policy: {}: {err}", base_policy_path.display());
        }
    };

    let base_policy: Value = match serde_json::from_slice(&base_bytes) {
        Ok(v) => v,
        Err(err) => {
            return Ok(PolicyResolution::SchemaInvalid(vec![format!(
                "parse policy JSON: {err}"
            )]));
        }
    };

    let base_schema_errors = validate_run_os_policy_schema(&base_policy);
    if !base_schema_errors.is_empty() {
        return Ok(PolicyResolution::SchemaInvalid(base_schema_errors));
    }

    let base_policy_id = base_policy
        .get("policy_id")
        .and_then(Value::as_str)
        .context("policy.policy_id must be a string")?
        .to_string();

    let net_enabled = base_policy
        .pointer("/net/enabled")
        .and_then(Value::as_bool)
        .context("policy.net.enabled must be a bool")?;
    if !net_enabled
        && (!args.allow_host.is_empty()
            || !args.allow_host_file.is_empty()
            || !args.deny_host.is_empty()
            || !args.deny_host_file.is_empty())
    {
        anyhow::bail!("base policy disables networking (net.enabled=false)");
    }

    let allow_tcp = base_policy
        .pointer("/net/allow_tcp")
        .and_then(Value::as_bool)
        .context("policy.net.allow_tcp must be a bool")?;
    if !allow_tcp
        && (!args.allow_host.is_empty()
            || !args.allow_host_file.is_empty()
            || !args.deny_host.is_empty()
            || !args.deny_host_file.is_empty())
    {
        anyhow::bail!("base policy forbids TCP (net.allow_tcp=false)");
    }

    let fs_enabled = base_policy
        .pointer("/fs/enabled")
        .and_then(Value::as_bool)
        .context("policy.fs.enabled must be a bool")?;
    if !fs_enabled && (!args.allow_read_root.is_empty() || !args.allow_write_root.is_empty()) {
        anyhow::bail!("base policy disables filesystem access (fs.enabled=false)");
    }

    let mut allow_map: HashMap<String, BTreeSet<u16>> = HashMap::new();
    for path in &args.allow_host_file {
        for spec in read_host_specs_from_file(policy_root, path)? {
            let (host, ports) = parse_allow_host_spec(&spec)?;
            allow_map.entry(host).or_default().extend(ports);
        }
    }
    for spec in &args.allow_host {
        let (host, ports) = parse_allow_host_spec(spec)?;
        allow_map.entry(host).or_default().extend(ports);
    }

    let mut deny_map: HashMap<String, DenySpec> = HashMap::new();
    for path in &args.deny_host_file {
        for spec in read_host_specs_from_file(policy_root, path)? {
            let (host, deny) = parse_deny_host_spec(&spec)?;
            merge_deny_spec(&mut deny_map, host, deny);
        }
    }
    for spec in &args.deny_host {
        let (host, deny) = parse_deny_host_spec(spec)?;
        merge_deny_spec(&mut deny_map, host, deny);
    }

    let mut allow_read_roots = normalize_roots(&args.allow_read_root)?;
    let mut allow_write_roots = normalize_roots(&args.allow_write_root)?;

    // Canonicalize overrides for hashing.
    let mut allow_hosts_digest: Vec<Value> = allow_map
        .iter()
        .map(|(host, ports)| {
            let mut ports: Vec<u16> = ports.iter().copied().collect();
            ports.sort_unstable();
            Value::Object(
                [
                    ("host".to_string(), Value::String(host.clone())),
                    (
                        "ports".to_string(),
                        Value::Array(ports.into_iter().map(Value::from).collect()),
                    ),
                ]
                .into_iter()
                .collect(),
            )
        })
        .collect();
    allow_hosts_digest.sort_by(|a, b| {
        let ah = a.get("host").and_then(Value::as_str).unwrap_or("");
        let bh = b.get("host").and_then(Value::as_str).unwrap_or("");
        ah.cmp(bh)
    });

    let mut deny_hosts_digest: Vec<Value> = deny_map
        .iter()
        .map(|(host, deny)| {
            let mut ports: Vec<u16> = deny.ports.iter().copied().collect();
            ports.sort_unstable();
            Value::Object(
                [
                    ("host".to_string(), Value::String(host.clone())),
                    ("all_ports".to_string(), Value::Bool(deny.all_ports)),
                    (
                        "ports".to_string(),
                        Value::Array(ports.into_iter().map(Value::from).collect()),
                    ),
                ]
                .into_iter()
                .collect(),
            )
        })
        .collect();
    deny_hosts_digest.sort_by(|a, b| {
        let ah = a.get("host").and_then(Value::as_str).unwrap_or("");
        let bh = b.get("host").and_then(Value::as_str).unwrap_or("");
        ah.cmp(bh)
    });

    allow_read_roots.sort();
    allow_read_roots.dedup();
    allow_write_roots.sort();
    allow_write_roots.dedup();

    let mut overrides = Value::Object(
        [
            ("allow_hosts".to_string(), Value::Array(allow_hosts_digest)),
            ("deny_hosts".to_string(), Value::Array(deny_hosts_digest)),
            (
                "allow_read_roots".to_string(),
                Value::Array(
                    allow_read_roots
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
            (
                "allow_write_roots".to_string(),
                Value::Array(
                    allow_write_roots
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );
    x07c::x07ast::canon_value_jcs(&mut overrides);
    let overrides_bytes = serde_json::to_vec(&overrides)?;

    let mut hasher = Sha256::new();
    hasher.update(&base_bytes);
    hasher.update(&overrides_bytes);
    let digest = hasher.finalize();
    let digest8 = hex8(&digest);

    let derived_dir = policy_root.join(".x07/policies/_generated");
    std::fs::create_dir_all(&derived_dir)
        .with_context(|| format!("create dir: {}", derived_dir.display()))?;

    let derived_path = derived_dir.join(format!("{base_policy_id}.g{digest8}.policy.json"));

    let mut derived_policy = base_policy;
    apply_policy_net_overrides(&mut derived_policy, &allow_map, &deny_map)?;
    apply_policy_fs_overrides(
        &mut derived_policy,
        &args.allow_read_root,
        &args.allow_write_root,
    )?;
    apply_policy_id_and_notes(
        &mut derived_policy,
        &base_policy_id,
        &digest8,
        base_policy_path,
    );

    let derived_schema_errors = validate_run_os_policy_schema(&derived_policy);
    if !derived_schema_errors.is_empty() {
        return Ok(PolicyResolution::SchemaInvalid(derived_schema_errors));
    }

    let mut derived_value = derived_policy;
    x07c::x07ast::canon_value_jcs(&mut derived_value);
    let mut derived_bytes = serde_json::to_vec_pretty(&derived_value)?;
    if derived_bytes.last() != Some(&b'\n') {
        derived_bytes.push(b'\n');
    }

    if derived_path.is_file() {
        if let Ok(existing) = std::fs::read(&derived_path) {
            if existing == derived_bytes {
                return Ok(PolicyResolution::Derived {
                    derived: derived_path,
                });
            }
        }
    }

    write_atomic_next_to(&derived_path, &derived_bytes)?;

    Ok(PolicyResolution::Derived {
        derived: derived_path,
    })
}

fn validate_run_os_policy_schema(doc: &Value) -> Vec<String> {
    let schema_json: Value = match serde_json::from_slice(RUN_OS_POLICY_SCHEMA_BYTES) {
        Ok(v) => v,
        Err(err) => return vec![format!("parse run-os-policy schema: {err}")],
    };
    let validator = match jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema_json)
    {
        Ok(v) => v,
        Err(err) => return vec![format!("build run-os-policy schema validator: {err}")],
    };

    validator
        .iter_errors(doc)
        .map(|err| format!("{} ({})", err, err.instance_path()))
        .collect()
}

fn print_policy_schema_x07diag_stderr(errors: Vec<String>) {
    use x07c::diagnostics::{Diagnostic, Report, Severity, Stage};

    let diagnostics = errors
        .into_iter()
        .map(|message| Diagnostic {
            code: "X07-POLICY-SCHEMA-0001".to_string(),
            severity: Severity::Error,
            stage: Stage::Parse,
            message,
            loc: None,
            notes: Vec::new(),
            related: Vec::new(),
            data: Default::default(),
            quickfix: None,
        })
        .collect();

    let report = Report {
        schema_version: x07_contracts::X07DIAG_SCHEMA_VERSION.to_string(),
        ok: false,
        diagnostics,
        meta: Default::default(),
    };

    if let Ok(mut bytes) = serde_json::to_vec(&report) {
        bytes.push(b'\n');
        let _ = std::io::Write::write_all(&mut std::io::stderr(), &bytes);
    }
}

fn write_atomic_next_to(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output dir: {}", parent.display()))?;
    }

    let tmp = temp_path_next_to(path);
    std::fs::write(&tmp, contents).with_context(|| format!("write temp: {}", tmp.display()))?;

    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp, path).with_context(|| format!("rename: {}", path.display()))?;
            Ok(())
        }
    }
}

fn temp_path_next_to(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{file_name}.{pid}.{n}.tmp"))
}

fn hex8(digest: &[u8]) -> String {
    let mut out = String::with_capacity(8);
    for b in digest.iter().take(4) {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn read_host_specs_from_file(root: &Path, path: &Path) -> Result<Vec<String>> {
    let path = resolve_project_relative(root, path);
    let bytes = std::fs::read(&path).with_context(|| format!("read: {}", path.display()))?;
    let s = std::str::from_utf8(&bytes).context("host spec file must be utf-8")?;
    let mut out = Vec::new();
    for line in s.lines() {
        let mut line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((before, _)) = line.split_once('#') {
            line = before.trim();
            if line.is_empty() {
                continue;
            }
        }
        out.push(line.to_string());
    }
    Ok(out)
}

fn parse_allow_host_spec(raw: &str) -> Result<(String, BTreeSet<u16>)> {
    let (host_raw, ports_raw) = split_host_ports(raw)?;
    if ports_raw.trim() == "*" {
        anyhow::bail!("Expected HOST:PORTS (ports are 1..65535 or *).");
    }
    let host = normalize_host_token(host_raw)?;
    let ports = parse_ports_list(ports_raw)?;
    Ok((host, ports))
}

fn parse_deny_host_spec(raw: &str) -> Result<(String, DenySpec)> {
    let (host_raw, ports_raw) = split_host_ports(raw)?;
    let host = normalize_host_token(host_raw)?;
    let ports_raw = ports_raw.trim();
    if ports_raw == "*" {
        return Ok((
            host,
            DenySpec {
                all_ports: true,
                ports: BTreeSet::new(),
            },
        ));
    }
    let ports = parse_ports_list(ports_raw)?;
    Ok((
        host,
        DenySpec {
            all_ports: false,
            ports,
        },
    ))
}

fn split_host_ports(raw: &str) -> Result<(&str, &str)> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("Expected HOST:PORTS (ports are 1..65535 or *).");
    }
    if raw.starts_with('[') {
        let close = raw
            .find(']')
            .context("Expected HOST:PORTS (ports are 1..65535 or *).")?;
        let after = &raw[close + 1..];
        let ports = after
            .strip_prefix(':')
            .context("Expected HOST:PORTS (ports are 1..65535 or *).")?;
        let host = &raw[1..close];
        if host.is_empty() || ports.trim().is_empty() {
            anyhow::bail!("Expected HOST:PORTS (ports are 1..65535 or *).");
        }
        return Ok((host, ports));
    }

    let idx = raw
        .rfind(':')
        .context("Expected HOST:PORTS (ports are 1..65535 or *).")?;
    let (host, ports) = raw.split_at(idx);
    let ports = &ports[1..];
    if host.trim().is_empty() || ports.trim().is_empty() {
        anyhow::bail!("Expected HOST:PORTS (ports are 1..65535 or *).");
    }
    Ok((host, ports))
}

fn normalize_host_token(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        anyhow::bail!("host must be non-empty");
    }
    if raw.len() > 255 {
        anyhow::bail!("host must be <= 255 chars");
    }
    if raw
        .as_bytes()
        .iter()
        .any(|&b| b == b';' || b.is_ascii_whitespace())
    {
        anyhow::bail!("host must not contain whitespace or semicolons");
    }
    let mut out = raw.to_string();
    out.make_ascii_lowercase();
    Ok(out)
}

fn parse_ports_list(raw: &str) -> Result<BTreeSet<u16>> {
    let mut out: BTreeSet<u16> = BTreeSet::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            anyhow::bail!("Expected HOST:PORTS (ports are 1..65535 or *).");
        }
        let port: u16 = part
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected HOST:PORTS (ports are 1..65535 or *)."))?;
        if port == 0 {
            anyhow::bail!("ports are 1..65535");
        }
        out.insert(port);
    }
    if out.len() > 64 {
        anyhow::bail!("ports list would exceed 64 entries");
    }
    Ok(out)
}

fn merge_deny_spec(map: &mut HashMap<String, DenySpec>, host: String, deny: DenySpec) {
    map.entry(host)
        .and_modify(|existing| {
            if deny.all_ports {
                existing.all_ports = true;
                existing.ports.clear();
            } else if !existing.all_ports {
                existing.ports.extend(deny.ports.iter().copied());
            }
        })
        .or_insert(deny);
}

fn normalize_roots(roots: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for raw in roots {
        let s = raw.trim();
        if s.is_empty() {
            anyhow::bail!("root path must be non-empty");
        }
        if s.len() > 4096 {
            anyhow::bail!("root path must be <= 4096 chars");
        }
        out.push(s.to_string());
    }
    Ok(out)
}

fn apply_policy_net_overrides(
    policy: &mut Value,
    allow_map: &HashMap<String, BTreeSet<u16>>,
    deny_map: &HashMap<String, DenySpec>,
) -> Result<()> {
    let base_allow_dns = policy
        .pointer("/net/allow_dns")
        .and_then(Value::as_bool)
        .context("policy.net.allow_dns must be a bool")?;

    let base_allow_hosts = policy
        .pointer("/net/allow_hosts")
        .and_then(Value::as_array)
        .context("policy.net.allow_hosts must be an array")?;

    let mut ordered_hosts: Vec<String> = Vec::new();
    let mut allowed: HashMap<String, BTreeSet<u16>> = HashMap::new();

    for entry in base_allow_hosts {
        let host_raw = entry
            .get("host")
            .and_then(Value::as_str)
            .context("policy.net.allow_hosts[].host must be a string")?;
        let host = normalize_host_token(host_raw)?;
        if !allowed.contains_key(&host) {
            ordered_hosts.push(host.clone());
            allowed.insert(host.clone(), BTreeSet::new());
        }

        let ports_val = entry
            .get("ports")
            .and_then(Value::as_array)
            .context("policy.net.allow_hosts[].ports must be an array")?;
        let ports_set = allowed.get_mut(&host).expect("inserted");
        for port in ports_val {
            let port = port
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .context("policy.net.allow_hosts[].ports must be u16")?;
            if port == 0 {
                anyhow::bail!("policy contains port 0");
            }
            ports_set.insert(port);
        }
        if ports_set.len() > 64 {
            anyhow::bail!("Host {host} would exceed 64 allowed ports");
        }
    }

    for (host, ports) in allow_map {
        if !allowed.contains_key(host) {
            ordered_hosts.push(host.clone());
            allowed.insert(host.clone(), BTreeSet::new());
            if ordered_hosts.len() > 256 {
                anyhow::bail!("Policy net.allow_hosts would exceed 256 entries");
            }
        }
        let set = allowed.get_mut(host).expect("present");
        set.extend(ports.iter().copied());
        if set.len() > 64 {
            anyhow::bail!("Host {host} would exceed 64 allowed ports");
        }
    }

    for (host, deny) in deny_map {
        if deny.all_ports {
            allowed.remove(host);
            continue;
        }
        let Some(set) = allowed.get_mut(host) else {
            continue;
        };
        for port in &deny.ports {
            set.remove(port);
        }
        if set.is_empty() {
            allowed.remove(host);
        }
    }

    let allow_dns_final = base_allow_dns
        || allowed
            .keys()
            .any(|h| h.as_bytes().iter().any(|b| b.is_ascii_alphabetic()));

    let mut out_allow_hosts: Vec<Value> = Vec::new();
    for host in ordered_hosts {
        let Some(ports) = allowed.get(&host) else {
            continue;
        };
        let ports: Vec<Value> = ports.iter().copied().map(Value::from).collect();
        out_allow_hosts.push(Value::Object(
            [
                ("host".to_string(), Value::String(host)),
                ("ports".to_string(), Value::Array(ports)),
            ]
            .into_iter()
            .collect(),
        ));
    }

    *policy
        .pointer_mut("/net/allow_dns")
        .context("missing policy.net.allow_dns")? = Value::Bool(allow_dns_final);
    *policy
        .pointer_mut("/net/allow_hosts")
        .context("missing policy.net.allow_hosts")? = Value::Array(out_allow_hosts);

    Ok(())
}

fn apply_policy_fs_overrides(
    policy: &mut Value,
    allow_read_roots: &[String],
    allow_write_roots: &[String],
) -> Result<()> {
    if allow_read_roots.is_empty() && allow_write_roots.is_empty() {
        return Ok(());
    }

    let fs_enabled = policy
        .pointer("/fs/enabled")
        .and_then(Value::as_bool)
        .context("policy.fs.enabled must be a bool")?;
    if !fs_enabled {
        anyhow::bail!("base policy disables filesystem access (fs.enabled=false)");
    }

    let mut read_roots: Vec<String> = policy
        .pointer("/fs/read_roots")
        .and_then(Value::as_array)
        .context("policy.fs.read_roots must be an array")?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    let mut write_roots: Vec<String> = policy
        .pointer("/fs/write_roots")
        .and_then(Value::as_array)
        .context("policy.fs.write_roots must be an array")?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let mut seen_read: HashSet<String> = read_roots.iter().cloned().collect();
    for root in allow_read_roots
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if root.len() > 4096 {
            anyhow::bail!("root path must be <= 4096 chars");
        }
        if seen_read.insert(root.to_string()) {
            read_roots.push(root.to_string());
        }
    }
    if read_roots.len() > 128 {
        anyhow::bail!("fs.read_roots would exceed 128 entries");
    }

    let mut seen_write: HashSet<String> = write_roots.iter().cloned().collect();
    for root in allow_write_roots
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if root.len() > 4096 {
            anyhow::bail!("root path must be <= 4096 chars");
        }
        if seen_write.insert(root.to_string()) {
            write_roots.push(root.to_string());
        }
    }
    if write_roots.len() > 128 {
        anyhow::bail!("fs.write_roots would exceed 128 entries");
    }

    *policy
        .pointer_mut("/fs/read_roots")
        .context("missing policy.fs.read_roots")? =
        Value::Array(read_roots.into_iter().map(Value::String).collect());
    *policy
        .pointer_mut("/fs/write_roots")
        .context("missing policy.fs.write_roots")? =
        Value::Array(write_roots.into_iter().map(Value::String).collect());

    Ok(())
}

fn apply_policy_id_and_notes(
    policy: &mut Value,
    base_policy_id: &str,
    digest8: &str,
    base_path: &Path,
) {
    let suffix = format!(".g{digest8}");
    let max_base_len = 64usize.saturating_sub(suffix.len());
    let truncated = if base_policy_id.len() > max_base_len {
        &base_policy_id[..max_base_len]
    } else {
        base_policy_id
    };
    let derived_id = format!("{truncated}{suffix}");
    if let Some(v) = policy.pointer_mut("/policy_id") {
        *v = Value::String(derived_id);
    }

    let line = format!(
        "Derived by x07 run from `{}` (g{digest8})",
        base_path.display()
    );
    let Some(notes_val) = policy.pointer_mut("/notes") else {
        policy
            .as_object_mut()
            .map(|obj| obj.insert("notes".to_string(), Value::String(line)));
        return;
    };
    let existing = notes_val.as_str().unwrap_or("");
    if existing.is_empty() {
        *notes_val = Value::String(line);
        return;
    }
    let candidate = format!("{existing}\n{line}");
    if candidate.len() <= 4096 {
        *notes_val = Value::String(candidate);
    }
}

pub(crate) fn discover_project_manifest(start: &Path) -> Result<Option<PathBuf>> {
    let mut dir: Option<&Path> = Some(start);
    while let Some(d) = dir {
        let x07_json = d.join("x07.json");
        if x07_json.is_file() {
            return Ok(Some(x07_json));
        }

        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(d) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().ends_with(".x07project.json"))
                {
                    candidates.push(path);
                }
            }
        }
        if candidates.len() == 1 {
            return Ok(Some(candidates.remove(0)));
        }

        dir = d.parent();
    }
    Ok(None)
}

fn resolve_sibling_or_path(name: &str) -> PathBuf {
    let Ok(exe) = std::env::current_exe() else {
        return PathBuf::from(name);
    };
    let Some(dir) = exe.parent() else {
        return PathBuf::from(name);
    };

    let mut candidates = Vec::new();

    let mut cand = dir.join(name);
    if cfg!(windows) {
        cand.set_extension("exe");
    }
    candidates.push(cand);

    if dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "deps")
    {
        if let Some(parent) = dir.parent() {
            let mut cand = parent.join(name);
            if cfg!(windows) {
                cand.set_extension("exe");
            }
            candidates.push(cand);
        }
    }

    for cand in candidates {
        if cand.is_file() {
            return cand;
        }
    }

    PathBuf::from(name)
}

fn prepare_input_flag(
    cwd: &Path,
    args: &RunArgs,
    default_input: Option<PathBuf>,
) -> Result<(Option<Vec<String>>, Option<TempPathGuard>)> {
    if let Some(path) = &args.input {
        return Ok((
            Some(vec!["--input".to_string(), path.display().to_string()]),
            None,
        ));
    }
    if args.stdin {
        let bytes = read_all_stdin().context("read stdin")?;
        let path = write_temp_file(cwd, "x07_run_input", &bytes)?;
        return Ok((
            Some(vec!["--input".to_string(), path.display().to_string()]),
            Some(TempPathGuard { path }),
        ));
    }
    if let Some(b64) = &args.input_b64 {
        let engine = base64::engine::general_purpose::STANDARD;
        let bytes = engine.decode(b64.trim()).context("decode --input-b64")?;
        let path = write_temp_file(cwd, "x07_run_input", &bytes)?;
        return Ok((
            Some(vec!["--input".to_string(), path.display().to_string()]),
            Some(TempPathGuard { path }),
        ));
    }
    if !args.argv.is_empty() {
        let bytes = pack_argv_v1(&args.argv)?;
        let path = write_temp_file(cwd, "x07_run_argv_v1", &bytes)?;
        return Ok((
            Some(vec!["--input".to_string(), path.display().to_string()]),
            Some(TempPathGuard { path }),
        ));
    }
    if let Some(path) = default_input {
        return Ok((
            Some(vec!["--input".to_string(), path.display().to_string()]),
            None,
        ));
    }
    Ok((None, None))
}

fn pack_argv_v1(tokens: &[String]) -> Result<Vec<u8>> {
    let argc: u32 = tokens.len().try_into().context("argv_v1 argc overflow")?;

    let mut out = Vec::new();
    out.extend_from_slice(&argc.to_le_bytes());

    for tok in tokens {
        let b = tok.as_bytes();
        let len: u32 = b
            .len()
            .try_into()
            .context("argv_v1 token length overflow")?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(b);
    }

    Ok(out)
}

fn read_all_stdin() -> Result<Vec<u8>> {
    use std::io::Read as _;
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf)?;
    Ok(buf)
}

fn write_temp_file(base: &Path, prefix: &str, bytes: &[u8]) -> Result<PathBuf> {
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let filename = format!("{prefix}_{pid}_{n}.bin");

    let dir = if base.join("target").is_dir() {
        base.join("target")
    } else {
        std::env::temp_dir()
    };
    let path = dir.join(filename);
    std::fs::write(&path, bytes)
        .with_context(|| format!("write temp input: {}", path.display()))?;
    Ok(path)
}

struct TempPathGuard {
    path: PathBuf,
}

impl Drop for TempPathGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Default)]
struct ResolvedFixtures {
    fs_dir: Option<PathBuf>,
    fs_root: Option<PathBuf>,
    fs_latency_index: Option<PathBuf>,
    rr_dir: Option<PathBuf>,
    rr_index: Option<PathBuf>,
    kv_dir: Option<PathBuf>,
    kv_seed: Option<PathBuf>,
}

fn resolve_fixtures(
    world: WorldId,
    args: &RunArgs,
    project_root: Option<&Path>,
) -> Result<ResolvedFixtures> {
    if world == WorldId::SolvePure {
        return Ok(ResolvedFixtures::default());
    }
    if !world.is_eval_world() {
        return Ok(ResolvedFixtures::default());
    }

    let mut out = ResolvedFixtures {
        fs_dir: None,
        fs_root: args.fixture_fs_root.clone(),
        fs_latency_index: args.fixture_fs_latency_index.clone(),
        rr_dir: None,
        rr_index: args.fixture_rr_index.clone(),
        kv_dir: None,
        kv_seed: args.fixture_kv_seed.clone(),
    };

    out.fs_dir = resolve_fixture_dir(
        args.fixture_fs_dir.as_deref(),
        args.fixtures.as_deref().map(|p| p.join("fs")),
        project_root,
        "fs",
    )?;
    out.rr_dir = resolve_fixture_dir(
        args.fixture_rr_dir.as_deref(),
        args.fixtures.as_deref().map(|p| p.join("rr")),
        project_root,
        "rr",
    )?;
    out.kv_dir = resolve_fixture_dir(
        args.fixture_kv_dir.as_deref(),
        args.fixtures.as_deref().map(|p| p.join("kv")),
        project_root,
        "kv",
    )?;

    match world {
        WorldId::SolveFs => {
            if out.fs_dir.is_none() {
                anyhow::bail!(
                    "solve-fs requires a fixture fs dir (set --fixture-fs-dir or --fixtures)"
                );
            }
        }
        WorldId::SolveRr => {
            if out.rr_dir.is_none() {
                anyhow::bail!(
                    "solve-rr requires a fixture rr dir (set --fixture-rr-dir or --fixtures)"
                );
            }
        }
        WorldId::SolveKv => {
            if out.kv_dir.is_none() {
                anyhow::bail!(
                    "solve-kv requires a fixture kv dir (set --fixture-kv-dir or --fixtures)"
                );
            }
        }
        WorldId::SolveFull => {
            if out.fs_dir.is_none() || out.rr_dir.is_none() || out.kv_dir.is_none() {
                anyhow::bail!("solve-full requires fs/rr/kv fixture dirs (set --fixtures or the per-world flags)");
            }
        }
        WorldId::SolvePure | WorldId::RunOs | WorldId::RunOsSandboxed => {}
    }

    Ok(out)
}

fn resolve_fixture_dir(
    explicit: Option<&Path>,
    from_fixtures: Option<PathBuf>,
    project_root: Option<&Path>,
    kind: &str,
) -> Result<Option<PathBuf>> {
    if let Some(p) = explicit {
        return Ok(Some(p.to_path_buf()));
    }
    if let Some(p) = from_fixtures {
        if p.is_dir() {
            return Ok(Some(p));
        }
    }
    if let Some(root) = project_root {
        let a = root.join(".x07").join("fixtures").join(kind);
        if a.is_dir() {
            return Ok(Some(a));
        }
        let b = root.join("fixtures").join(kind);
        if b.is_dir() {
            return Ok(Some(b));
        }
    }
    Ok(None)
}

fn project_lockfile_path(project_path: &Path) -> Result<PathBuf> {
    let manifest = project::load_project_manifest(project_path)?;
    Ok(project::default_lockfile_path(project_path, &manifest))
}

fn try_collect_project_module_roots(
    project_path: &Path,
) -> Result<Option<(PathBuf, Vec<PathBuf>)>> {
    let manifest = project::load_project_manifest(project_path)?;
    let lock_path = project::default_lockfile_path(project_path, &manifest);
    let lock_bytes = match std::fs::read(&lock_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err).with_context(|| format!("read lockfile: {}", lock_path.display()))
        }
    };
    let lock: project::Lockfile = serde_json::from_slice(&lock_bytes)
        .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;
    project::verify_lockfile(project_path, &manifest, &lock)?;
    let roots = project::collect_module_roots(project_path, &manifest, &lock)?;
    Ok(Some((lock_path, roots)))
}

fn infer_program_module_roots(
    program: &Path,
    project_manifest: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    if let Some(project_path) = project_manifest {
        if let Ok(Some((_lock, roots))) = try_collect_project_module_roots(project_path) {
            return Ok(roots);
        }
    }

    let base = program
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let found = discover_project_manifest(base)?;
    if let Some(project_path) = found.as_deref() {
        if let Ok(Some((_lock, roots))) = try_collect_project_module_roots(project_path) {
            return Ok(roots);
        }
    }

    Ok(vec![base.to_path_buf()])
}

fn resolve_module_roots_for_wrapper(
    runner: RunnerKind,
    target_kind: TargetKind,
    target_path: &Path,
    project_manifest: Option<&Path>,
    args: &RunArgs,
    runner_bin: &Path,
) -> Result<Vec<PathBuf>> {
    match target_kind {
        TargetKind::Artifact => Ok(Vec::new()),
        TargetKind::Program => {
            let mut roots = if !args.module_root.is_empty() {
                args.module_root.clone()
            } else {
                infer_program_module_roots(target_path, project_manifest)?
            };
            if runner == RunnerKind::Os {
                append_unique(&mut roots, default_os_module_roots_best_effort(runner_bin));
            }
            Ok(roots)
        }
        TargetKind::Project => {
            let mut roots = Vec::new();
            if let Some(project_path) = project_manifest {
                if let Ok(Some((_lock, project_roots))) =
                    try_collect_project_module_roots(project_path)
                {
                    roots = project_roots;
                }
            }
            if runner == RunnerKind::Os {
                append_unique(&mut roots, default_os_module_roots_best_effort(runner_bin));
            }
            Ok(roots)
        }
    }
}

fn append_unique(into: &mut Vec<PathBuf>, extra: Vec<PathBuf>) {
    for r in extra {
        if !into.contains(&r) {
            into.push(r);
        }
    }
}

fn default_os_module_roots_best_effort(runner_bin: &Path) -> Vec<PathBuf> {
    let rel = PathBuf::from("stdlib/os/0.2.0/modules");
    if rel.is_dir() {
        return vec![rel];
    }

    if let Some(runner_dir) = runner_bin.parent() {
        for base in [Some(runner_dir), runner_dir.parent()] {
            let Some(base) = base else { continue };
            let cand = base.join("stdlib/os/0.2.0/modules");
            if cand.is_dir() {
                return vec![cand];
            }
        }
    }

    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(workspace_root) = crate_dir.parent().and_then(|p| p.parent()) {
        let abs = workspace_root.join("stdlib/os/0.2.0/modules");
        if abs.is_dir() {
            return vec![abs];
        }
    }

    Vec::new()
}
