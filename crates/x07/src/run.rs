use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use base64::Engine;
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::Value;
use x07_contracts::{PROJECT_LOCKFILE_SCHEMA_VERSION, X07_RUN_REPORT_SCHEMA_VERSION};
use x07_host_runner::CcProfile;
use x07_worlds::WorldId;
use x07c::project;

use crate::repair::{RepairArgs, RepairMode, RepairSummary};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const DEFAULT_SOLVE_FUEL: u64 = 50_000_000;
const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;
const AUTO_DEPS_ENV: &str = "X07_INTERNAL_AUTO_DEPS";

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
    /// Run a project (`x07.json` path, or a directory containing `x07.json`).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Compile+run a single `*.x07.json` file.
    #[arg(long, value_name = "PATH")]
    pub program: Option<PathBuf>,

    /// Run a precompiled executable produced by the X07 toolchain runners.
    #[arg(long, value_name = "PATH")]
    pub artifact: Option<PathBuf>,

    /// Override the resolved world (advanced; prefer `--profile`).
    #[arg(long, value_enum, hide = true)]
    pub world: Option<WorldId>,

    /// Run profile name (resolved from `x07.json.profiles`).
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Force runner selection.
    #[arg(long, value_enum, conflicts_with_all = ["host", "os"], hide = true)]
    pub runner: Option<RunnerMode>,

    /// Force deterministic host runner selection.
    #[arg(long, hide = true)]
    pub host: bool,

    /// Force OS runner selection.
    #[arg(long, hide = true)]
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

    #[arg(long, hide = true)]
    pub compile_only: bool,

    #[arg(long, value_name = "PATH")]
    pub module_root: Vec<PathBuf>,

    /// A base directory for fixtures (shorthand for world-specific fixture dirs).
    #[arg(long, value_name = "DIR", hide = true)]
    pub fixtures: Option<PathBuf>,

    #[arg(long, value_name = "PATH", hide = true)]
    pub fixture_fs_dir: Option<PathBuf>,
    #[arg(long, value_name = "PATH", hide = true)]
    pub fixture_fs_root: Option<PathBuf>,
    #[arg(long, value_name = "PATH", hide = true)]
    pub fixture_fs_latency_index: Option<PathBuf>,
    #[arg(long, value_name = "PATH", hide = true)]
    pub fixture_rr_dir: Option<PathBuf>,
    #[arg(long, value_name = "PATH", hide = true)]
    pub fixture_kv_dir: Option<PathBuf>,
    #[arg(long, value_name = "PATH", hide = true)]
    pub fixture_kv_seed: Option<PathBuf>,

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

    #[command(flatten)]
    pub repair: RepairArgs,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    repair: Option<RepairSummary>,
    report: Box<RawValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectRunProfilesFile {
    #[serde(default)]
    default_profile: Option<String>,
    #[serde(default)]
    profiles: Option<BTreeMap<String, ProjectRunProfile>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectRunProfile {
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
pub(crate) struct ResolvedProfile {
    pub(crate) world: WorldId,
    pub(crate) policy: Option<PathBuf>,
    pub(crate) runner: Option<RunnerMode>,
    pub(crate) input: Option<PathBuf>,
    pub(crate) auto_ffi: Option<bool>,
    pub(crate) solve_fuel: Option<u64>,
    pub(crate) cpu_time_limit_seconds: Option<u64>,
    pub(crate) max_memory_bytes: Option<usize>,
    pub(crate) max_output_bytes: Option<usize>,
    pub(crate) cc_profile: Option<CcProfile>,
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
    let runner_bin = match runner {
        RunnerKind::Host => crate::util::resolve_sibling_or_path("x07-host-runner"),
        RunnerKind::Os => crate::util::resolve_sibling_or_path("x07-os-runner"),
    };

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

    let policy_overrides = crate::policy_overrides::PolicyOverrides {
        allow_host: args.allow_host.clone(),
        allow_host_file: args.allow_host_file.clone(),
        deny_host: args.deny_host.clone(),
        deny_host_file: args.deny_host_file.clone(),
        allow_read_root: args.allow_read_root.clone(),
        allow_write_root: args.allow_write_root.clone(),
    };
    let policy_resolution = crate::policy_overrides::resolve_policy_for_world(
        world,
        policy_root,
        args.policy.clone(),
        selected_profile.as_ref().and_then(|p| p.policy.clone()),
        &policy_overrides,
    )?;
    let policy = match &policy_resolution {
        crate::policy_overrides::PolicyResolution::None => None,
        crate::policy_overrides::PolicyResolution::Base(p) => Some(p.clone()),
        crate::policy_overrides::PolicyResolution::Derived { derived, .. } => Some(derived.clone()),
        crate::policy_overrides::PolicyResolution::SchemaInvalid(errors) => {
            crate::policy_overrides::print_policy_schema_x07diag_stderr(errors.clone());
            return Ok(std::process::ExitCode::from(3));
        }
    };

    if let crate::policy_overrides::PolicyResolution::Derived { derived, .. } = &policy_resolution {
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

    let mut staged_program: Option<PathBuf> = None;
    let repair = match target_kind {
        TargetKind::Artifact => None,
        TargetKind::Program => {
            let repair = crate::repair::maybe_repair_x07ast_file(&target_path, world, &args.repair)
                .with_context(|| format!("repair program: {}", target_path.display()))?;
            if args.repair.repair == RepairMode::Memory {
                if let Some(r) = repair.as_ref() {
                    let stage_id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
                    let stage_dir = target_path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(&cwd)
                        .join(".x07")
                        .join("repair")
                        .join("_staged")
                        .join(stage_id.to_string());
                    std::fs::create_dir_all(&stage_dir).with_context(|| {
                        format!("create repair staging dir: {}", stage_dir.display())
                    })?;
                    let staged_path = stage_dir.join(
                        target_path
                            .file_name()
                            .unwrap_or_else(|| std::ffi::OsStr::new("program.x07.json")),
                    );
                    std::fs::write(&staged_path, r.formatted.as_bytes()).with_context(|| {
                        format!("write staged repaired program: {}", staged_path.display())
                    })?;
                    staged_program = Some(staged_path);
                }
            }
            repair
        }
        TargetKind::Project => {
            let Some(project_path) = project_manifest.as_deref() else {
                anyhow::bail!("internal error: missing project manifest for project target");
            };
            let manifest = project::load_project_manifest(project_path)
                .with_context(|| format!("load project: {}", project_path.display()))?;
            let base = project_path.parent().unwrap_or_else(|| Path::new("."));
            let entry_path = base.join(&manifest.entry);

            let repair = crate::repair::maybe_repair_x07ast_file(&entry_path, world, &args.repair)
                .with_context(|| format!("repair entry: {}", entry_path.display()))?;
            if args.repair.repair == RepairMode::Memory {
                if let Some(r) = repair.as_ref() {
                    let stage_id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
                    let stage_dir = base
                        .join(".x07")
                        .join("repair")
                        .join("_staged")
                        .join(stage_id.to_string());
                    std::fs::create_dir_all(&stage_dir).with_context(|| {
                        format!("create repair staging dir: {}", stage_dir.display())
                    })?;
                    let staged_path = stage_dir.join(
                        entry_path
                            .file_name()
                            .unwrap_or_else(|| std::ffi::OsStr::new("main.x07.json")),
                    );
                    std::fs::write(&staged_path, r.formatted.as_bytes()).with_context(|| {
                        format!("write staged repaired entry: {}", staged_path.display())
                    })?;
                    staged_program = Some(staged_path);
                }
            }
            repair
        }
    };

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
            if args.repair.repair == RepairMode::Memory {
                let staged = staged_program
                    .as_ref()
                    .context("internal error: repair=memory but staged program missing")?;
                argv.push("--program".to_string());
                argv.push(staged.display().to_string());

                let roots = resolve_module_roots_for_wrapper(
                    runner,
                    target_kind,
                    &target_path,
                    project_manifest.as_deref(),
                    &args,
                    &runner_bin,
                )?;
                for root in roots {
                    argv.push("--module-root".to_string());
                    argv.push(root.display().to_string());
                }
            } else {
                argv.push("--project".to_string());
                argv.push(target_path.display().to_string());
            }
        }
        TargetKind::Program => {
            let program = staged_program.as_deref().unwrap_or(&target_path);
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

    let run_runner = |set_guard: bool| -> Result<std::process::Output> {
        let mut cmd = Command::new(&runner_bin);
        cmd.args(&argv).stdin(Stdio::null());
        if set_guard {
            cmd.env(AUTO_DEPS_ENV, "1");
        }
        cmd.output()
            .with_context(|| format!("exec {}", runner_bin.display()))
    };

    let can_auto_deps = target_kind == TargetKind::Project
        && args.repair.repair != RepairMode::Off
        && std::env::var_os(AUTO_DEPS_ENV).is_none();

    let mut output = run_runner(false)?;
    let mut exit_code = output.status.code().unwrap_or(2);

    if can_auto_deps && exit_code != 0 {
        if let Some(project_path) = project_manifest.as_ref() {
            let mut seen_missing: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for _ in 0..3 {
                let Some(compile_error) = parse_compile_error_from_runner_stdout(&output.stdout)
                else {
                    break;
                };
                let Some(module_id) = missing_module_id_from_compile_error(&compile_error) else {
                    break;
                };
                if !seen_missing.insert(module_id.clone()) {
                    break;
                }
                let Some((name, version)) =
                    x07_host_runner::best_external_package_for_module(&module_id)
                else {
                    break;
                };

                let spec = format!("{name}@{version}");
                let msg = format!(
                    "x07 run: auto-adding dependency {spec} (missing module {module_id})\n"
                );
                let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());

                if let Err(err) = crate::pkg::pkg_add_sync_quiet(project_path.clone(), spec, None) {
                    let msg = format!("x07 run: auto-deps failed: {err}\n");
                    let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
                    break;
                }

                output = run_runner(true)?;
                exit_code = output.status.code().unwrap_or(2);
                if exit_code == 0 {
                    break;
                }
            }
        }
    }

    if exit_code != 0 {
        if let Some(compile_error) = parse_compile_error_from_runner_stdout(&output.stdout) {
            if let Ok(module_roots) = resolve_module_roots_for_wrapper(
                runner,
                target_kind,
                &target_path,
                project_manifest.as_deref(),
                &args,
                &runner_bin,
            ) {
                print_ptr_hints_for_compile_error(&compile_error, &module_roots);
            }
        }
    }

    std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).context("write stderr")?;

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
                &runner_bin,
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
                repair: repair.as_ref().map(|r| r.summary.clone()),
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
        let path = match std::fs::metadata(path) {
            Ok(meta) if meta.is_dir() => path.join("x07.json"),
            _ => path.to_path_buf(),
        };
        return Ok((TargetKind::Project, path.clone(), Some(path)));
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

pub(crate) fn load_project_profiles(project_manifest: &Path) -> Result<ProjectRunProfilesFile> {
    let bytes = std::fs::read(project_manifest).with_context(|| {
        format!(
            "[X07PROJECT_READ] read project: {}",
            project_manifest.display()
        )
    })?;
    let mut file: ProjectRunProfilesFile = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "[X07PROJECT_PARSE] parse project JSON: {}",
            project_manifest.display()
        )
    })?;
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

pub(crate) fn resolve_selected_profile(
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
    Ok(WorldId::RunOs)
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
                } else if let Ok(manifest) = project::load_project_manifest(project_path) {
                    // Best-effort: if the lockfile is missing or invalid, still include
                    // the project's own module_roots for agent/debugging affordances.
                    let empty_lock = project::Lockfile {
                        schema_version: PROJECT_LOCKFILE_SCHEMA_VERSION.to_string(),
                        dependencies: Vec::new(),
                    };
                    if let Ok(project_roots) =
                        project::collect_module_roots(project_path, &manifest, &empty_lock)
                    {
                        roots = project_roots;
                    }
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
    x07_runner_common::os_paths::default_os_module_roots_best_effort_from_exe(Some(runner_bin))
}

fn parse_compile_error_from_runner_stdout(stdout: &[u8]) -> Option<String> {
    let doc: Value = serde_json::from_slice(stdout).ok()?;
    doc.get("compile")?
        .get("compile_error")?
        .as_str()
        .map(|s| s.to_string())
}

fn missing_module_id_from_compile_error(message: &str) -> Option<String> {
    let idx = message.find("unknown module: ")?;
    let rest = &message[idx + "unknown module: ".len()..];
    let rest = rest.trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let quoted = take_rust_debug_quoted_string(rest)?;
    serde_json::from_str::<String>(quoted).ok()
}

fn take_rust_debug_quoted_string(s: &str) -> Option<&str> {
    let mut escaped = false;
    let mut end = None;
    for (i, ch) in s.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            end = Some(i);
            break;
        }
    }
    let end = end?;
    Some(&s[..=end])
}

fn print_ptr_hints_for_compile_error(compile_error: &str, module_roots: &[PathBuf]) {
    let Some(fn_name) = fn_name_from_compile_error(compile_error) else {
        return;
    };
    let module_id = module_id_from_fn_name(fn_name);
    let Some(module_file) = module_file_from_roots(module_roots, module_id) else {
        return;
    };
    let ptrs = pointers_from_compile_error(compile_error);
    if ptrs.is_empty() {
        return;
    }

    let mut stderr = std::io::stderr();
    for ptr in ptrs.iter().take(4) {
        let msg = format!(
            "hint: x07 ast get --in {} --ptr {}\n",
            module_file.display(),
            ptr
        );
        let _ = std::io::Write::write_all(&mut stderr, msg.as_bytes());
    }
}

fn fn_name_from_compile_error(message: &str) -> Option<&str> {
    let idx = message.find("fn=")?;
    let rest = &message[idx + "fn=".len()..];
    let end = rest
        .char_indices()
        .find(|(_, ch)| ch.is_whitespace() || *ch == ')' || *ch == ',')
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(&rest[..end])
}

fn module_id_from_fn_name(fn_name: &str) -> &str {
    fn_name.rsplit_once('.').map(|(m, _)| m).unwrap_or(fn_name)
}

fn module_file_from_roots(module_roots: &[PathBuf], module_id: &str) -> Option<PathBuf> {
    let rel = format!("{}.x07.json", module_id.replace('.', "/"));
    for root in module_roots {
        let cand = root.join(&rel);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn pointers_from_compile_error(message: &str) -> Vec<String> {
    let mut raw: Vec<&str> = Vec::new();
    collect_pointers(message, "ptr=", &mut raw);
    collect_pointers(message, "moved_ptr=", &mut raw);
    collect_pointers(message, "borrowed_ptr=", &mut raw);

    let mut out: Vec<String> = Vec::new();
    for p in raw {
        if !out.iter().any(|v| v == p) {
            out.push(p.to_string());
        }
    }
    out
}

fn collect_pointers<'a>(message: &'a str, key: &str, out: &mut Vec<&'a str>) {
    let mut rest = message;
    while let Some(idx) = rest.find(key) {
        rest = &rest[idx + key.len()..];
        let end = rest
            .char_indices()
            .find(|(_, ch)| ch.is_whitespace() || *ch == ')' || *ch == ',')
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        if end == 0 {
            break;
        }
        let ptr = &rest[..end];
        if ptr.starts_with('/') {
            out.push(ptr);
        }
        rest = &rest[end..];
    }
}
