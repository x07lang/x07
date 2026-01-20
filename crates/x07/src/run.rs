use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use base64::Engine;
use clap::{Args, ValueEnum};
use serde::Serialize;
use serde_json::value::RawValue;
use x07_contracts::X07_RUN_REPORT_SCHEMA_VERSION;
use x07_host_runner::CcProfile;
use x07_worlds::WorldId;
use x07c::project;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum ReportMode {
    Runner,
    Wrapped,
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

    #[arg(long, value_enum, default_value_t = CcProfile::Default)]
    pub cc_profile: CcProfile,

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

    /// Policy JSON (required for `run-os-sandboxed`).
    #[arg(long, value_name = "PATH")]
    pub policy: Option<PathBuf>,

    #[arg(long, default_value_t = 50_000_000)]
    pub solve_fuel: u64,

    #[arg(long, default_value_t = 64 * 1024 * 1024)]
    pub max_memory_bytes: usize,

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

pub fn cmd_run(args: RunArgs) -> Result<std::process::ExitCode> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let (target_kind, target_path, project_manifest) = resolve_target(&cwd, &args)?;

    let project_root = project_manifest
        .as_deref()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());

    let world = resolve_world(&args, project_manifest.as_deref())?;
    let runner = resolve_runner(&args, world)?;

    if args.policy.is_some() && world == WorldId::RunOs {
        anyhow::bail!("--policy is only valid for --world run-os-sandboxed");
    }
    if world == WorldId::RunOsSandboxed && args.policy.is_none() {
        anyhow::bail!("run-os-sandboxed requires --policy");
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

    let (input_flag, temp_input) = prepare_input_flag(&cwd, &args)?;
    let _temp_input_guard = temp_input;

    let mut argv: Vec<String> = vec![
        "--cc-profile".to_string(),
        match args.cc_profile {
            CcProfile::Default => "default".to_string(),
            CcProfile::Size => "size".to_string(),
        },
        "--world".to_string(),
        world.as_str().to_string(),
    ];

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
            argv.push(args.solve_fuel.to_string());
            argv.push("--max-memory-bytes".to_string());
            argv.push(args.max_memory_bytes.to_string());
            if let Some(max_output_bytes) = args.max_output_bytes {
                argv.push("--max-output-bytes".to_string());
                argv.push(max_output_bytes.to_string());
            }
            if let Some(cpu) = args.cpu_time_limit_seconds {
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
            argv.push(args.solve_fuel.to_string());
            argv.push("--max-memory-bytes".to_string());
            argv.push(args.max_memory_bytes.to_string());
            if let Some(max_output_bytes) = args.max_output_bytes {
                argv.push("--max-output-bytes".to_string());
                argv.push(max_output_bytes.to_string());
            }
            if let Some(cpu) = args.cpu_time_limit_seconds {
                argv.push("--cpu-time-limit-seconds".to_string());
                argv.push(cpu.to_string());
            }
            if args.debug_borrow_checks {
                argv.push("--debug-borrow-checks".to_string());
            }

            if let Some(path) = &args.policy {
                argv.push("--policy".to_string());
                argv.push(path.display().to_string());
            }

            if !args.no_auto_ffi {
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

fn resolve_world(args: &RunArgs, project_manifest: Option<&Path>) -> Result<WorldId> {
    if let Some(world) = args.world {
        return Ok(world);
    }
    if args.os {
        return Ok(WorldId::RunOs);
    }
    if args.host {
        return Ok(WorldId::SolvePure);
    }
    if let Some(project_path) = project_manifest {
        let manifest = project::load_project_manifest(project_path)?;
        let world = x07c::world_config::parse_world_id(&manifest.world)
            .with_context(|| format!("invalid project world {:?}", manifest.world))?;
        return Ok(world);
    }
    Ok(WorldId::SolvePure)
}

fn resolve_runner(args: &RunArgs, world: WorldId) -> Result<RunnerKind> {
    if args.host && args.os {
        anyhow::bail!("--host and --os are mutually exclusive");
    }
    if args.host {
        if !world.is_eval_world() {
            anyhow::bail!("--host is incompatible with --world {}", world.as_str());
        }
        return Ok(RunnerKind::Host);
    }
    if args.os {
        if world.is_eval_world() {
            anyhow::bail!("--os is incompatible with --world {}", world.as_str());
        }
        return Ok(RunnerKind::Os);
    }
    Ok(if world.is_eval_world() {
        RunnerKind::Host
    } else {
        RunnerKind::Os
    })
}

fn discover_project_manifest(start: &Path) -> Result<Option<PathBuf>> {
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
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let mut cand = dir.join(name);
            if cfg!(windows) {
                cand.set_extension("exe");
            }
            if cand.is_file() {
                return cand;
            }
        }
    }
    PathBuf::from(name)
}

fn prepare_input_flag(
    cwd: &Path,
    args: &RunArgs,
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
    Ok((None, None))
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
