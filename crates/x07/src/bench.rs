use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, ValueEnum};
use jsonschema::Draft;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;
use x07_contracts::{
    X07_ARCH_PATCHSET_SCHEMA_VERSION, X07_BENCH_INSTANCE_SCHEMA_VERSION,
    X07_BENCH_REPORT_SCHEMA_VERSION, X07_BENCH_SUITE_SCHEMA_VERSION,
};
use x07_worlds::WorldId;
use x07c::{diagnostics, json_patch};

use crate::repair::{self, RepairArgs, RepairMode, RepairSummary};
use crate::util;

const X07_BENCH_SUITE_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-bench.suite.schema.json");
const X07_BENCH_INSTANCE_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-bench.instance.schema.json");
const X07_BENCH_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-bench.report.schema.json");
const X07_ARCH_PATCHSET_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-arch.patchset.schema.json");
const X07DIAG_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07diag.schema.json");
const BENCH_DOCKER_SENTINEL_ENV: &str = "X07BENCH_IN_DOCKER";

#[derive(Debug, Args)]
pub struct BenchArgs {
    #[command(subcommand)]
    pub cmd: Option<BenchCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum BenchCommand {
    /// List instances in a benchmark suite.
    List(BenchListArgs),

    /// Validate benchmark instances (baseline must fail; oracle must pass; determinism must hold).
    Validate(BenchValidateArgs),

    /// Evaluate predictions (patches) against a benchmark suite.
    Eval(BenchEvalArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum BenchFormat {
    Json,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[clap(rename_all = "kebab_case")]
#[serde(rename_all = "kebab-case")]
pub enum BenchRunner {
    Local,
    Docker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[clap(rename_all = "kebab_case")]
#[serde(rename_all = "kebab-case")]
enum BenchPatchKind {
    X07ArchPatchsetJson,
    UnifiedDiff,
}

#[derive(Debug, Args)]
pub struct BenchListArgs {
    /// Path to suite.json.
    #[arg(long, value_name = "PATH", default_value = "suite.json")]
    pub suite: PathBuf,

    /// Filter instances by id substring.
    #[arg(long, value_name = "SUBSTR")]
    pub filter: Option<String>,

    /// Treat --filter as an exact id match.
    #[arg(long)]
    pub exact: bool,

    #[arg(long, value_enum, default_value_t = BenchFormat::Json)]
    pub format: BenchFormat,
}

#[derive(Debug, Args)]
pub struct BenchValidateArgs {
    /// Path to suite.json.
    #[arg(long, value_name = "PATH", default_value = "suite.json")]
    pub suite: PathBuf,

    /// Filter instances by id substring.
    #[arg(long, value_name = "SUBSTR")]
    pub filter: Option<String>,

    /// Treat --filter as an exact id match.
    #[arg(long)]
    pub exact: bool,

    #[arg(long, value_enum, default_value_t = BenchFormat::Json)]
    pub format: BenchFormat,

    /// Number of x07 test jobs per instance.
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub jobs: usize,

    /// Keep per-instance work directories on success.
    #[arg(long)]
    pub keep_artifacts: bool,

    /// Directory where per-instance artifacts are written.
    #[arg(long, value_name = "DIR", default_value = "target/x07bench")]
    pub artifact_dir: PathBuf,

    /// Number of times to re-run an instance after it reaches green (determinism check).
    #[arg(long, value_name = "N", default_value_t = 2)]
    pub determinism_runs: u32,

    /// Evaluation runner.
    #[arg(long, value_enum, default_value_t = BenchRunner::Local)]
    pub runner: BenchRunner,

    /// Docker image to use when --runner docker.
    #[arg(long, value_name = "IMAGE")]
    pub docker_image: Option<String>,

    #[command(flatten)]
    pub repair: RepairArgs,
}

#[derive(Debug, Args)]
pub struct BenchEvalArgs {
    /// Path to suite.json.
    #[arg(long, value_name = "PATH", default_value = "suite.json")]
    pub suite: PathBuf,

    /// JSONL predictions file (one line per instance).
    #[arg(
        long,
        value_name = "PATH",
        required_unless_present = "oracle",
        conflicts_with = "oracle"
    )]
    pub predictions: Option<PathBuf>,

    /// Evaluate oracle patches from each instance instead of predictions.
    #[arg(long, conflicts_with = "predictions")]
    pub oracle: bool,

    /// Repair loop mode applied after patch application.
    #[command(flatten)]
    pub repair: RepairArgs,

    /// Evaluation runner.
    #[arg(long, value_enum, default_value_t = BenchRunner::Local)]
    pub runner: BenchRunner,

    /// Docker image to use when --runner docker.
    #[arg(long, value_name = "IMAGE")]
    pub docker_image: Option<String>,

    #[arg(long, value_enum, default_value_t = BenchFormat::Json)]
    pub format: BenchFormat,

    /// Filter instances by id substring.
    #[arg(long, value_name = "SUBSTR")]
    pub filter: Option<String>,

    /// Treat --filter as an exact id match.
    #[arg(long)]
    pub exact: bool,

    /// Max number of instances to evaluate (after filtering).
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,

    /// Number of x07 test jobs per instance.
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub jobs: usize,

    /// Keep per-instance work directories on success.
    #[arg(long)]
    pub keep_artifacts: bool,

    /// Directory where per-instance artifacts are written.
    #[arg(long, value_name = "DIR", default_value = "target/x07bench")]
    pub artifact_dir: PathBuf,
}

pub fn cmd_bench(
    machine: &crate::reporting::MachineArgs,
    args: BenchArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing bench subcommand (try --help)");
    };

    match cmd {
        BenchCommand::List(args) => cmd_bench_list(args),
        BenchCommand::Validate(args) => cmd_bench_validate(machine, args),
        BenchCommand::Eval(args) => cmd_bench_eval(machine, args),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BenchSuiteFile {
    schema_version: String,
    suite_id: String,
    #[serde(default)]
    description: String,
    instances: Vec<BenchSuiteInstanceRef>,
    #[serde(default)]
    defaults: BenchSuiteDefaults,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BenchSuiteInstanceRef {
    id: String,
    path: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BenchSuiteDefaults {
    #[serde(default)]
    world: Option<String>,
    #[serde(default)]
    repair_mode: Option<String>,
    #[serde(default)]
    jobs: Option<usize>,
    #[serde(default)]
    keep_artifacts: Option<bool>,
    #[serde(default)]
    artifact_dir: Option<String>,
    #[serde(default)]
    determinism_runs: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchInstanceFile {
    schema_version: String,
    instance_id: String,
    #[serde(default)]
    tags: Vec<String>,
    world: String,
    problem_statement_path: String,
    repo_path: String,
    eval: BenchEvalConfig,
    #[serde(default)]
    oracle: Option<BenchOracle>,
    #[serde(default)]
    notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchEvalConfig {
    kind: String,
    manifest: String,
    #[serde(default)]
    module_root: Vec<String>,
    #[serde(default = "default_stdlib_lock")]
    stdlib_lock: String,
    #[serde(default)]
    filter: Option<String>,
    #[serde(default)]
    exact: bool,
    #[serde(default = "default_repeat")]
    repeat: u32,
    #[serde(default = "default_jobs")]
    jobs: usize,
    #[serde(default)]
    keep_artifacts: bool,
    #[serde(default = "default_x07test_artifact_dir")]
    artifact_dir: String,
    #[serde(default)]
    no_fail_fast: bool,
    #[serde(default)]
    no_run: bool,
    #[serde(default)]
    verbose: bool,
    #[serde(default)]
    fail_to_pass: Vec<String>,
    #[serde(default)]
    pass_to_pass: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchOracle {
    #[serde(default)]
    patch_kind: Option<BenchPatchKind>,
    #[serde(default)]
    patch_path: Option<String>,
    #[serde(default)]
    patchset_path: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchPredictionLine {
    #[serde(default)]
    schema_version: Option<String>,
    instance_id: String,
    patch_kind: String,
    #[serde(default)]
    patch_path: Option<String>,
    #[serde(default)]
    patch_inline: Option<Value>,
    #[serde(default)]
    model_name_or_path: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    meta: Option<Value>,
}

#[derive(Debug, Clone)]
struct BenchPrediction {
    patch_kind: BenchPatchKind,
    source: PatchSource,
    model_name_or_path: Option<String>,
}

#[derive(Debug, Clone)]
enum PatchSource {
    Path(PathBuf),
    Inline(Value),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchPatchSet {
    schema_version: String,
    patches: Vec<ArchPatchTarget>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchPatchTarget {
    path: String,
    patch: Vec<diagnostics::PatchOp>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct BenchListReport {
    ok: bool,
    suite_path: String,
    suite_id: Option<String>,
    instances: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diags: Vec<diagnostics::Diagnostic>,
}

#[derive(Debug, Serialize)]
struct BenchValidateReport {
    ok: bool,
    suite_path: String,
    suite_id: Option<String>,
    summary: BenchValidateSummary,
    instances: Vec<BenchValidateInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diags: Vec<diagnostics::Diagnostic>,
}

#[derive(Debug, Serialize)]
struct BenchValidateSummary {
    instances_total: usize,
    valid: usize,
    invalid: usize,
    duration_ms: u64,
}

#[derive(Debug, Serialize)]
struct BenchValidateInstance {
    instance_id: String,
    status: BenchStatus,
    baseline_ok: bool,
    oracle_ok: bool,
    determinism_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifacts_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum BenchStatus {
    Resolved,
    Unresolved,
    Error,
    Skipped,
}

#[derive(Debug, Serialize)]
struct BenchReport {
    schema_version: String,
    tool: BenchTool,
    invocation: BenchInvocation,
    suite: BenchSuiteInfo,
    env: BenchEnv,
    summary: BenchSummary,
    instances: Vec<BenchInstanceResult>,
}

#[derive(Debug, Serialize)]
struct BenchTool {
    name: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<String>,
}

#[derive(Debug, Serialize)]
struct BenchInvocation {
    argv: Vec<String>,
    cwd: String,
    started_at_unix_ms: u64,
    suite_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    predictions_path: Option<String>,
    oracle: bool,
    runner: BenchRunner,
    repair_mode: RepairMode,
    jobs: usize,
    keep_artifacts: bool,
    artifact_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
    exact: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct BenchSuiteInfo {
    suite_id: String,
    suite_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suite_jcs_sha256_hex: Option<String>,
}

#[derive(Debug, Serialize)]
struct BenchEnv {
    runner: BenchRunner,
    platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    docker_image: Option<String>,
}

#[derive(Debug, Default, Serialize)]
struct BenchSummary {
    instances_total: usize,
    resolved: usize,
    unresolved: usize,
    errors: usize,
    skipped: usize,
    duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    avg_repair_iterations: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avg_repair_applied_ops: Option<f64>,
}

#[derive(Debug, Serialize)]
struct BenchInstanceResult {
    instance_id: String,
    status: BenchStatus,
    baseline: BenchTestSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    after_patch: Option<BenchTestSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repair: Option<RepairSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifacts_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    patch_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    patch_sha256_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_name_or_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BenchTestSummary {
    ok: bool,
    exit_code: Option<i32>,
    report_path: Option<String>,
    passed: u64,
    failed: u64,
    skipped: u64,
    errors: u64,
    duration_ms: u64,
    compile_failures: u64,
    run_failures: u64,
}

impl BenchTestSummary {
    fn empty() -> Self {
        Self {
            ok: false,
            exit_code: None,
            report_path: None,
            passed: 0,
            failed: 0,
            skipped: 0,
            errors: 0,
            duration_ms: 0,
            compile_failures: 0,
            run_failures: 0,
        }
    }
}

struct X07TestRun {
    exit_code: i32,
    summary: BenchTestSummary,
    test_statuses: BTreeMap<String, String>,
}

struct MaterializedPatch {
    kind: BenchPatchKind,
    path: PathBuf,
    sha256_hex: String,
}

struct EvalContext<'a> {
    suite_dir: &'a Path,
    suite: &'a BenchSuiteFile,
    artifact_dir: &'a Path,
    keep_artifacts: bool,
    jobs: usize,
    repair: &'a RepairArgs,
    determinism_runs: u32,
    predictions: Option<&'a BTreeMap<String, BenchPrediction>>,
    oracle_mode: bool,
}

fn running_inside_bench_docker() -> bool {
    std::env::var(BENCH_DOCKER_SENTINEL_ENV)
        .ok()
        .is_some_and(|v| v == "1")
}

fn resolve_bench_docker_runner_paths() -> Result<(PathBuf, PathBuf)> {
    let script = util::resolve_existing_path_upwards(Path::new("ci/x07bench/run.sh"));
    if !script.is_file() {
        bail!(
            "missing Docker benchmark runner script at {}",
            script.display()
        );
    }

    let repo_root = script
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("failed to resolve repo root from {}", script.display()))?;

    Ok((script, repo_root))
}

fn canonicalize_with_missing_tail(path: &Path) -> Result<PathBuf> {
    let mut missing = Vec::<OsString>::new();
    let mut cur = path;
    while !cur.exists() {
        let Some(name) = cur.file_name() else {
            break;
        };
        missing.push(name.to_os_string());
        cur = cur
            .parent()
            .ok_or_else(|| anyhow!("cannot resolve path {}", path.display()))?;
    }

    let mut base = if cur.exists() {
        cur.canonicalize()
            .with_context(|| format!("canonicalize {}", cur.display()))?
    } else {
        cur.to_path_buf()
    };

    for part in missing.into_iter().rev() {
        base.push(part);
    }

    Ok(base)
}

fn docker_mount_rel(path: &Path, repo_root: &Path, must_exist: bool) -> Result<String> {
    let cwd = std::env::current_dir().context("resolve current directory")?;
    let host_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let host_path = canonicalize_with_missing_tail(&host_path)?;
    if must_exist && !host_path.exists() {
        bail!("path does not exist: {}", host_path.display());
    }

    let repo_root = repo_root
        .canonicalize()
        .with_context(|| format!("canonicalize repo root {}", repo_root.display()))?;
    if !host_path.starts_with(&repo_root) {
        bail!(
            "path {} is outside repo root {}",
            host_path.display(),
            repo_root.display()
        );
    }

    let rel = host_path
        .strip_prefix(&repo_root)
        .with_context(|| format!("strip repo root from {}", host_path.display()))?;
    if rel.as_os_str().is_empty() {
        Ok(".".to_string())
    } else {
        Ok(rel.to_string_lossy().to_string())
    }
}

fn bench_format_cli_value(v: BenchFormat) -> &'static str {
    match v {
        BenchFormat::Json => "json",
        BenchFormat::Text => "text",
    }
}

fn repair_mode_cli_value(v: RepairMode) -> &'static str {
    match v {
        RepairMode::Off => "off",
        RepairMode::Memory => "memory",
        RepairMode::Write => "write",
    }
}

fn exit_code_from_i32(code: i32) -> std::process::ExitCode {
    std::process::ExitCode::from((code & 0xff) as u8)
}

fn run_docker_bench_command(
    script: &Path,
    docker_image: Option<&str>,
    argv: &[String],
) -> Result<std::process::ExitCode> {
    let mut cmd = Command::new(script);
    cmd.args(argv);
    if let Some(image) = docker_image {
        cmd.env("X07BENCH_DOCKER_IMAGE", image);
    }
    let status = cmd
        .status()
        .with_context(|| format!("run Docker benchmark command via {}", script.display()))?;
    Ok(exit_code_from_i32(status.code().unwrap_or(1)))
}

fn run_validate_via_docker(
    machine: &crate::reporting::MachineArgs,
    args: &BenchValidateArgs,
) -> Result<std::process::ExitCode> {
    let (script, repo_root) = resolve_bench_docker_runner_paths()?;
    let suite = docker_mount_rel(&args.suite, &repo_root, true)?;
    let artifact_dir = docker_mount_rel(&args.artifact_dir, &repo_root, false)?;

    let mut argv = vec![
        "bench".to_string(),
        "validate".to_string(),
        "--suite".to_string(),
        suite,
        "--format".to_string(),
        bench_format_cli_value(args.format).to_string(),
        "--jobs".to_string(),
        args.jobs.max(1).to_string(),
        "--artifact-dir".to_string(),
        artifact_dir,
        "--determinism-runs".to_string(),
        args.determinism_runs.max(1).to_string(),
        "--runner".to_string(),
        "docker".to_string(),
        "--repair".to_string(),
        repair_mode_cli_value(args.repair.repair).to_string(),
        "--repair-max-iters".to_string(),
        args.repair.repair_max_iters.max(1).to_string(),
    ];

    if let Some(filter) = args.filter.as_ref() {
        argv.push("--filter".to_string());
        argv.push(filter.clone());
    }
    if args.exact {
        argv.push("--exact".to_string());
    }
    if args.keep_artifacts {
        argv.push("--keep-artifacts".to_string());
    }
    if let Some(out) = machine.out.as_ref() {
        argv.push("--out".to_string());
        argv.push(docker_mount_rel(out, &repo_root, false)?);
    }
    if let Some(image) = args.docker_image.as_ref() {
        argv.push("--docker-image".to_string());
        argv.push(image.clone());
    }

    run_docker_bench_command(&script, args.docker_image.as_deref(), &argv)
}

fn run_eval_via_docker(
    machine: &crate::reporting::MachineArgs,
    args: &BenchEvalArgs,
) -> Result<std::process::ExitCode> {
    let (script, repo_root) = resolve_bench_docker_runner_paths()?;
    let suite = docker_mount_rel(&args.suite, &repo_root, true)?;
    let artifact_dir = docker_mount_rel(&args.artifact_dir, &repo_root, false)?;

    let mut argv = vec![
        "bench".to_string(),
        "eval".to_string(),
        "--suite".to_string(),
        suite,
        "--format".to_string(),
        bench_format_cli_value(args.format).to_string(),
        "--jobs".to_string(),
        args.jobs.max(1).to_string(),
        "--artifact-dir".to_string(),
        artifact_dir,
        "--runner".to_string(),
        "docker".to_string(),
        "--repair".to_string(),
        repair_mode_cli_value(args.repair.repair).to_string(),
        "--repair-max-iters".to_string(),
        args.repair.repair_max_iters.max(1).to_string(),
    ];

    if args.oracle {
        argv.push("--oracle".to_string());
    } else {
        let predictions = args
            .predictions
            .as_ref()
            .context("predictions are required unless --oracle")?;
        argv.push("--predictions".to_string());
        argv.push(docker_mount_rel(predictions, &repo_root, true)?);
    }
    if let Some(filter) = args.filter.as_ref() {
        argv.push("--filter".to_string());
        argv.push(filter.clone());
    }
    if args.exact {
        argv.push("--exact".to_string());
    }
    if let Some(limit) = args.limit {
        argv.push("--limit".to_string());
        argv.push(limit.to_string());
    }
    if args.keep_artifacts {
        argv.push("--keep-artifacts".to_string());
    }
    if let Some(out) = machine.out.as_ref() {
        argv.push("--out".to_string());
        argv.push(docker_mount_rel(out, &repo_root, false)?);
    }
    if let Some(image) = args.docker_image.as_ref() {
        argv.push("--docker-image".to_string());
        argv.push(image.clone());
    }

    run_docker_bench_command(&script, args.docker_image.as_deref(), &argv)
}

fn cmd_bench_list(args: BenchListArgs) -> Result<std::process::ExitCode> {
    let loaded = match load_suite(&args.suite) {
        Ok(v) => v,
        Err(err) => {
            let report = BenchListReport {
                ok: false,
                suite_path: args.suite.display().to_string(),
                suite_id: None,
                instances: Vec::new(),
                diags: vec![diag_parse_error(
                    "E_BENCH_SUITE_LOAD",
                    &err.to_string(),
                    None,
                )],
            };
            emit_list_report(args.format, &report)?;
            return Ok(std::process::ExitCode::from(20));
        }
    };

    let ids = select_instances(
        &loaded.suite,
        args.filter.as_deref(),
        args.exact,
        None,
        true,
    )
    .into_iter()
    .map(|i| i.id)
    .collect::<Vec<_>>();

    let report = BenchListReport {
        ok: true,
        suite_path: loaded.suite_path.display().to_string(),
        suite_id: Some(loaded.suite.suite_id),
        instances: ids,
        diags: Vec::new(),
    };

    emit_list_report(args.format, &report)?;
    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_bench_validate(
    machine: &crate::reporting::MachineArgs,
    args: BenchValidateArgs,
) -> Result<std::process::ExitCode> {
    if args.runner == BenchRunner::Docker && !running_inside_bench_docker() {
        return run_validate_via_docker(machine, &args);
    }

    let started = Instant::now();

    let loaded = match load_suite(&args.suite) {
        Ok(v) => v,
        Err(err) => {
            let report = BenchValidateReport {
                ok: false,
                suite_path: args.suite.display().to_string(),
                suite_id: None,
                summary: BenchValidateSummary {
                    instances_total: 0,
                    valid: 0,
                    invalid: 0,
                    duration_ms: 0,
                },
                instances: Vec::new(),
                diags: vec![diag_parse_error(
                    "E_BENCH_SUITE_LOAD",
                    &err.to_string(),
                    None,
                )],
            };
            emit_json_or_text(args.format, &report, machine.out.as_deref())?;
            return Ok(std::process::ExitCode::from(20));
        }
    };

    let selected = select_instances(
        &loaded.suite,
        args.filter.as_deref(),
        args.exact,
        None,
        true,
    );

    let mut instances = Vec::with_capacity(selected.len());

    let ctx = EvalContext {
        suite_dir: &loaded.suite_dir,
        suite: &loaded.suite,
        artifact_dir: &args.artifact_dir,
        keep_artifacts: args.keep_artifacts,
        jobs: args.jobs.max(1),
        repair: &args.repair,
        determinism_runs: args.determinism_runs.max(1),
        predictions: None,
        oracle_mode: true,
    };

    for inst in selected {
        let eval = eval_one_instance(&ctx, &inst)?;
        let status = eval.status;
        let baseline_ok = eval.baseline.exit_code.unwrap_or(1) != 0;
        let oracle_ok = eval
            .after_patch
            .as_ref()
            .is_some_and(|t| t.exit_code == Some(0));
        let determinism_ok = !eval.notes.iter().any(|n| n.contains("nondeterministic"));
        instances.push(BenchValidateInstance {
            instance_id: eval.instance_id,
            status,
            baseline_ok,
            oracle_ok,
            determinism_ok,
            artifacts_dir: eval.artifacts_dir,
            error: eval.error,
            notes: eval.notes,
        });
    }

    instances.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));

    let valid = instances
        .iter()
        .filter(|i| {
            i.status == BenchStatus::Resolved && i.baseline_ok && i.oracle_ok && i.determinism_ok
        })
        .count();
    let invalid = instances.len().saturating_sub(valid);
    let ok = invalid == 0;

    let report = BenchValidateReport {
        ok,
        suite_path: loaded.suite_path.display().to_string(),
        suite_id: Some(loaded.suite.suite_id),
        summary: BenchValidateSummary {
            instances_total: instances.len(),
            valid,
            invalid,
            duration_ms: started.elapsed().as_millis() as u64,
        },
        instances,
        diags: Vec::new(),
    };

    emit_json_or_text(args.format, &report, machine.out.as_deref())?;
    Ok(if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(20)
    })
}

fn cmd_bench_eval(
    machine: &crate::reporting::MachineArgs,
    args: BenchEvalArgs,
) -> Result<std::process::ExitCode> {
    if args.runner == BenchRunner::Docker && !running_inside_bench_docker() {
        return run_eval_via_docker(machine, &args);
    }

    let started = Instant::now();

    let loaded = match load_suite(&args.suite) {
        Ok(v) => v,
        Err(err) => {
            let report = BenchReport {
                schema_version: X07_BENCH_REPORT_SCHEMA_VERSION.to_string(),
                tool: bench_tool(),
                invocation: bench_invocation(
                    &args,
                    &args.suite,
                    None,
                    args.runner,
                    args.docker_image.clone(),
                ),
                suite: BenchSuiteInfo {
                    suite_id: String::new(),
                    suite_path: args.suite.display().to_string(),
                    suite_jcs_sha256_hex: None,
                },
                env: bench_env(args.runner, args.docker_image.clone()),
                summary: BenchSummary {
                    instances_total: 0,
                    errors: 1,
                    duration_ms: started.elapsed().as_millis() as u64,
                    ..BenchSummary::default()
                },
                instances: vec![BenchInstanceResult {
                    instance_id: "<suite>".to_string(),
                    status: BenchStatus::Error,
                    baseline: BenchTestSummary::empty(),
                    after_patch: None,
                    repair: None,
                    artifacts_dir: None,
                    patch_kind: None,
                    patch_sha256_hex: None,
                    model_name_or_path: None,
                    error: Some(err.to_string()),
                    notes: Vec::new(),
                }],
            };
            emit_report(args.format, &report, machine.out.as_deref())?;
            return Ok(std::process::ExitCode::from(20));
        }
    };

    std::fs::create_dir_all(&args.artifact_dir)
        .with_context(|| format!("create artifact_dir: {}", args.artifact_dir.display()))?;

    let predictions = if args.oracle {
        None
    } else {
        let pred_path = args
            .predictions
            .as_ref()
            .context("predictions are required unless --oracle")?;
        Some(load_predictions_jsonl(pred_path)?)
    };

    let mut selected = select_instances(
        &loaded.suite,
        args.filter.as_deref(),
        args.exact,
        args.limit,
        true,
    );

    if let Some(limit) = args.limit {
        selected.truncate(limit);
    }

    let determinism_runs = loaded.suite.defaults.determinism_runs.unwrap_or(1).max(1);

    let ctx = EvalContext {
        suite_dir: &loaded.suite_dir,
        suite: &loaded.suite,
        artifact_dir: &args.artifact_dir,
        keep_artifacts: args.keep_artifacts
            || loaded.suite.defaults.keep_artifacts.unwrap_or(false),
        jobs: args.jobs.max(1),
        repair: &args.repair,
        determinism_runs,
        predictions: predictions.as_ref(),
        oracle_mode: args.oracle,
    };

    let mut results = Vec::with_capacity(selected.len());
    for inst in selected {
        results.push(eval_one_instance(&ctx, &inst)?);
    }

    results.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));

    let mut summary = BenchSummary {
        instances_total: results.len(),
        duration_ms: started.elapsed().as_millis() as u64,
        ..BenchSummary::default()
    };

    let mut repair_iters_total: u64 = 0;
    let mut repair_ops_total: u64 = 0;
    let mut repair_count: u64 = 0;

    for r in &results {
        match r.status {
            BenchStatus::Resolved => summary.resolved += 1,
            BenchStatus::Unresolved => summary.unresolved += 1,
            BenchStatus::Error => summary.errors += 1,
            BenchStatus::Skipped => summary.skipped += 1,
        }

        if let Some(rep) = &r.repair {
            repair_iters_total = repair_iters_total.saturating_add(rep.iterations as u64);
            repair_ops_total = repair_ops_total.saturating_add(rep.applied_ops_count as u64);
            repair_count = repair_count.saturating_add(1);
        }
    }

    if repair_count > 0 {
        summary.avg_repair_iterations = Some(repair_iters_total as f64 / repair_count as f64);
        summary.avg_repair_applied_ops = Some(repair_ops_total as f64 / repair_count as f64);
    }

    let suite_sha = util::sha256_hex(&util::canonical_jcs_bytes(&serde_json::to_value(
        &loaded.suite,
    )?)?);

    let report = BenchReport {
        schema_version: X07_BENCH_REPORT_SCHEMA_VERSION.to_string(),
        tool: bench_tool(),
        invocation: bench_invocation(
            &args,
            &loaded.suite_path,
            args.predictions.as_ref(),
            args.runner,
            args.docker_image.clone(),
        ),
        suite: BenchSuiteInfo {
            suite_id: loaded.suite.suite_id,
            suite_path: loaded.suite_path.display().to_string(),
            suite_jcs_sha256_hex: Some(suite_sha),
        },
        env: bench_env(args.runner, args.docker_image.clone()),
        summary,
        instances: results,
    };

    let report_value = serde_json::to_value(&report)?;
    let report_diags = validate_schema(
        "E_BENCH_REPORT_SCHEMA_INVALID",
        X07_BENCH_REPORT_SCHEMA_BYTES,
        &report_value,
    )?;
    if !report_diags.is_empty() {
        anyhow::bail!("bench report did not validate against schema");
    }

    emit_report(args.format, &report, machine.out.as_deref())?;

    Ok(if report.summary.errors == 0 {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(12)
    })
}

struct LoadedSuite {
    suite_path: PathBuf,
    suite_dir: PathBuf,
    suite: BenchSuiteFile,
}

fn load_suite(path: &Path) -> Result<LoadedSuite> {
    let suite_path = util::resolve_existing_path_upwards(path);
    let bytes = std::fs::read(&suite_path)
        .with_context(|| format!("read suite: {}", suite_path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse JSON: {}", suite_path.display()))?;

    let schema_diags = validate_schema(
        "E_BENCH_SUITE_SCHEMA_INVALID",
        X07_BENCH_SUITE_SCHEMA_BYTES,
        &doc,
    )?;
    if !schema_diags.is_empty() {
        return Err(anyhow!(schema_diags[0].message.clone()));
    }

    let suite: BenchSuiteFile = serde_json::from_value(doc)
        .with_context(|| format!("decode suite: {}", suite_path.display()))?;

    if suite.schema_version.trim() != X07_BENCH_SUITE_SCHEMA_VERSION {
        bail!(
            "suite schema_version mismatch: expected {} got {:?}",
            X07_BENCH_SUITE_SCHEMA_VERSION,
            suite.schema_version
        );
    }

    let mut seen = BTreeSet::new();
    for inst in &suite.instances {
        if !seen.insert(inst.id.clone()) {
            bail!("duplicate suite instance id: {}", inst.id);
        }
    }

    let suite_dir = suite_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    Ok(LoadedSuite {
        suite_path,
        suite_dir,
        suite,
    })
}

fn select_instances(
    suite: &BenchSuiteFile,
    filter: Option<&str>,
    exact: bool,
    limit: Option<usize>,
    enabled_only: bool,
) -> Vec<BenchSuiteInstanceRef> {
    let mut out: Vec<BenchSuiteInstanceRef> = suite
        .instances
        .iter()
        .filter(|i| !enabled_only || i.enabled)
        .cloned()
        .collect();

    if let Some(f) = filter {
        if exact {
            out.retain(|i| i.id == f);
        } else {
            out.retain(|i| i.id.contains(f));
        }
    }

    out.sort_by(|a, b| a.id.cmp(&b.id));

    if let Some(n) = limit {
        out.truncate(n);
    }

    out
}

fn eval_one_instance(
    ctx: &EvalContext<'_>,
    inst_ref: &BenchSuiteInstanceRef,
) -> Result<BenchInstanceResult> {
    let mut notes = Vec::new();
    if let Some(n) = &inst_ref.note {
        notes.push(n.clone());
    }

    let instance_path = resolve_instance_path(ctx.suite_dir, &inst_ref.path);
    let instance_dir = instance_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let (instance_doc, instance) = match load_instance(&instance_path) {
        Ok(v) => v,
        Err(err) => {
            return Ok(BenchInstanceResult {
                instance_id: inst_ref.id.clone(),
                status: BenchStatus::Error,
                baseline: BenchTestSummary::empty(),
                after_patch: None,
                repair: None,
                artifacts_dir: None,
                patch_kind: None,
                patch_sha256_hex: None,
                model_name_or_path: None,
                error: Some(err.to_string()),
                notes,
            });
        }
    };

    if instance.instance_id != inst_ref.id {
        notes.push(format!(
            "instance_id mismatch: suite has {} but instance.json has {}",
            inst_ref.id, instance.instance_id
        ));
    }
    if !instance.tags.is_empty() {
        notes.push(format!("tags={}", instance.tags.join(",")));
    }
    notes.extend(instance.notes.clone());

    let artifact_root = if ctx.artifact_dir.is_absolute() {
        ctx.artifact_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(ctx.artifact_dir)
    };
    let suite_bucket = format!(
        "suites/{}/instances",
        safe_artifact_dir_name(&ctx.suite.suite_id)
    );
    let artifact_instance_dir = artifact_root
        .join(suite_bucket)
        .join(safe_artifact_dir_name(&instance.instance_id));
    std::fs::create_dir_all(&artifact_instance_dir).with_context(|| {
        format!(
            "create instance artifact dir: {}",
            artifact_instance_dir.display()
        )
    })?;

    let instance_copy_path = artifact_instance_dir.join("instance.json");
    util::write_atomic(
        &instance_copy_path,
        canonical_pretty_json_bytes(&instance_doc)?.as_slice(),
    )
    .with_context(|| format!("write {}", instance_copy_path.display()))?;

    let problem_path = instance_dir.join(&instance.problem_statement_path);
    if !problem_path.is_file() {
        notes.push(format!(
            "problem statement path missing: {}",
            problem_path.display()
        ));
    }

    let repo_src = instance_dir.join(&instance.repo_path);
    if !repo_src.is_dir() {
        return Ok(BenchInstanceResult {
            instance_id: instance.instance_id,
            status: BenchStatus::Error,
            baseline: BenchTestSummary::empty(),
            after_patch: None,
            repair: None,
            artifacts_dir: Some(artifact_instance_dir.display().to_string()),
            patch_kind: None,
            patch_sha256_hex: None,
            model_name_or_path: None,
            error: Some(format!("repo_path does not exist: {}", repo_src.display())),
            notes,
        });
    }

    let repo_work = artifact_instance_dir.join("repo");
    if repo_work.exists() {
        std::fs::remove_dir_all(&repo_work)
            .with_context(|| format!("clear {}", repo_work.display()))?;
    }
    copy_dir_recursive(&repo_src, &repo_work)
        .with_context(|| format!("copy repo snapshot from {}", repo_src.display()))?;

    let x07test_subdir = if is_safe_rel_path(&instance.eval.artifact_dir) {
        instance.eval.artifact_dir.clone()
    } else {
        "x07test".to_string()
    };
    let x07test_root = artifact_instance_dir.join(x07test_subdir);
    let baseline_report = artifact_instance_dir.join("baseline.x07test.report.json");
    let baseline_stderr = artifact_instance_dir.join("logs/baseline.stderr.txt");

    let baseline_run = match run_x07_test_subprocess(
        &repo_work,
        &instance.eval,
        &x07test_root.join("baseline"),
        &baseline_report,
        &baseline_stderr,
        ctx.jobs.max(instance.eval.jobs),
    ) {
        Ok(v) => v,
        Err(err) => {
            return Ok(BenchInstanceResult {
                instance_id: instance.instance_id,
                status: BenchStatus::Error,
                baseline: BenchTestSummary::empty(),
                after_patch: None,
                repair: None,
                artifacts_dir: Some(artifact_instance_dir.display().to_string()),
                patch_kind: None,
                patch_sha256_hex: None,
                model_name_or_path: None,
                error: Some(err.to_string()),
                notes,
            });
        }
    };

    let baseline_summary = baseline_run.summary.clone();
    let mut result = BenchInstanceResult {
        instance_id: instance.instance_id.clone(),
        status: BenchStatus::Error,
        baseline: baseline_summary,
        after_patch: None,
        repair: None,
        artifacts_dir: Some(artifact_instance_dir.display().to_string()),
        patch_kind: None,
        patch_sha256_hex: None,
        model_name_or_path: None,
        error: None,
        notes,
    };

    if baseline_run.exit_code == 0 {
        result.status = BenchStatus::Error;
        result.error = Some("baseline run is already green; instance is invalid".to_string());
        if !ctx.keep_artifacts {
            cleanup_instance_artifacts(&artifact_instance_dir)?;
        }
        return Ok(result);
    }

    let selected_patch = if ctx.oracle_mode {
        match oracle_as_prediction(&instance, instance_dir) {
            Ok(Some(v)) => Some(v),
            Ok(None) => {
                result.status = BenchStatus::Error;
                result.error =
                    Some("missing oracle patch for validation/eval --oracle".to_string());
                if !ctx.keep_artifacts {
                    cleanup_instance_artifacts(&artifact_instance_dir)?;
                }
                return Ok(result);
            }
            Err(err) => {
                result.status = BenchStatus::Error;
                result.error = Some(err.to_string());
                if !ctx.keep_artifacts {
                    cleanup_instance_artifacts(&artifact_instance_dir)?;
                }
                return Ok(result);
            }
        }
    } else {
        let pred = ctx
            .predictions
            .and_then(|m| m.get(&instance.instance_id).cloned());
        match pred {
            Some(v) => Some(v),
            None => {
                result.status = BenchStatus::Skipped;
                result.error = None;
                result
                    .notes
                    .push("no prediction for this instance".to_string());
                if !ctx.keep_artifacts {
                    cleanup_instance_artifacts(&artifact_instance_dir)?;
                }
                return Ok(result);
            }
        }
    };

    let pred = selected_patch.expect("prediction or oracle expected");

    let materialized = match materialize_submission_patch(&artifact_instance_dir, &pred) {
        Ok(v) => v,
        Err(err) => {
            result.status = BenchStatus::Error;
            result.patch_kind = Some(pred.patch_kind.to_string());
            result.model_name_or_path = pred.model_name_or_path.clone();
            result.error = Some(err.to_string());
            if !ctx.keep_artifacts {
                cleanup_instance_artifacts(&artifact_instance_dir)?;
            }
            return Ok(result);
        }
    };

    result.patch_kind = Some(materialized.kind.to_string());
    result.patch_sha256_hex = Some(materialized.sha256_hex.clone());
    result.model_name_or_path = pred.model_name_or_path.clone();

    let touched = match apply_materialized_patch(&repo_work, &materialized) {
        Ok(v) => v,
        Err(err) => {
            result.status = BenchStatus::Error;
            result.error = Some(err.to_string());
            if !ctx.keep_artifacts {
                cleanup_instance_artifacts(&artifact_instance_dir)?;
            }
            return Ok(result);
        }
    };

    let world = parse_world_id(&instance.world)
        .ok_or_else(|| anyhow!("unsupported instance world: {}", instance.world))?;

    match maybe_repair_touched_x07ast(world, &touched, ctx.repair) {
        Ok(rep) => result.repair = rep,
        Err(err) => {
            result.status = BenchStatus::Error;
            result.error = Some(err.to_string());
            if !ctx.keep_artifacts {
                cleanup_instance_artifacts(&artifact_instance_dir)?;
            }
            return Ok(result);
        }
    }

    let after_report = artifact_instance_dir.join("after_patch.x07test.report.json");
    let after_stderr = artifact_instance_dir.join("logs/after_patch.stderr.txt");
    let after_run = match run_x07_test_subprocess(
        &repo_work,
        &instance.eval,
        &x07test_root.join("after_patch"),
        &after_report,
        &after_stderr,
        ctx.jobs.max(instance.eval.jobs),
    ) {
        Ok(v) => v,
        Err(err) => {
            result.status = BenchStatus::Error;
            result.error = Some(err.to_string());
            if !ctx.keep_artifacts {
                cleanup_instance_artifacts(&artifact_instance_dir)?;
            }
            return Ok(result);
        }
    };

    result.after_patch = Some(after_run.summary.clone());

    if ctx.determinism_runs > 1 {
        for run_idx in 1..ctx.determinism_runs {
            let det_report =
                artifact_instance_dir.join(format!("after_patch.det{run_idx}.x07test.report.json"));
            let det_stderr =
                artifact_instance_dir.join(format!("logs/after_patch.det{run_idx}.stderr.txt"));
            let det_run = run_x07_test_subprocess(
                &repo_work,
                &instance.eval,
                &x07test_root.join(format!("after_patch_det{run_idx}")),
                &det_report,
                &det_stderr,
                ctx.jobs.max(instance.eval.jobs),
            )?;

            if det_run.exit_code != after_run.exit_code
                || det_run.summary.passed != after_run.summary.passed
                || det_run.summary.failed != after_run.summary.failed
                || det_run.summary.errors != after_run.summary.errors
                || det_run.summary.compile_failures != after_run.summary.compile_failures
                || det_run.summary.run_failures != after_run.summary.run_failures
                || det_run.test_statuses != after_run.test_statuses
            {
                result.status = BenchStatus::Error;
                result.error =
                    Some("nondeterministic instance: repeated post-patch run diverged".to_string());
                result
                    .notes
                    .push("nondeterministic after-patch run detected".to_string());
                if !ctx.keep_artifacts {
                    cleanup_instance_artifacts(&artifact_instance_dir)?;
                }
                return Ok(result);
            }
        }
    }

    let checks = check_expectations(
        &instance.eval,
        &baseline_run.test_statuses,
        &after_run.test_statuses,
    );
    result.notes.extend(checks.notes);

    if checks.invalid_instance {
        result.status = BenchStatus::Error;
        result.error =
            Some("instance expectation labels are inconsistent with baseline".to_string());
    } else if after_run.exit_code == 0 && checks.satisfied {
        result.status = BenchStatus::Resolved;
    } else {
        result.status = BenchStatus::Unresolved;
    }

    if !ctx.keep_artifacts {
        cleanup_instance_artifacts(&artifact_instance_dir)?;
    }

    Ok(result)
}

fn load_instance(path: &Path) -> Result<(Value, BenchInstanceFile)> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read instance: {}", path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse JSON: {}", path.display()))?;

    let schema_diags = validate_schema(
        "E_BENCH_INSTANCE_SCHEMA_INVALID",
        X07_BENCH_INSTANCE_SCHEMA_BYTES,
        &doc,
    )?;
    if !schema_diags.is_empty() {
        return Err(anyhow!(schema_diags[0].message.clone()));
    }

    let instance: BenchInstanceFile = serde_json::from_value(doc.clone())
        .with_context(|| format!("decode instance: {}", path.display()))?;

    if instance.schema_version.trim() != X07_BENCH_INSTANCE_SCHEMA_VERSION {
        bail!(
            "instance schema_version mismatch: expected {} got {:?}",
            X07_BENCH_INSTANCE_SCHEMA_VERSION,
            instance.schema_version
        );
    }

    if instance.eval.kind.trim() != "x07test" {
        bail!("unsupported eval kind: {}", instance.eval.kind);
    }

    Ok((doc, instance))
}

fn resolve_instance_path(suite_dir: &Path, rel: &str) -> PathBuf {
    let p = PathBuf::from(rel);
    if p.is_absolute() {
        return p;
    }
    let joined = suite_dir.join(&p);
    if joined.extension().and_then(|e| e.to_str()) == Some("json") {
        return joined;
    }
    joined.join("instance.json")
}

fn load_predictions_jsonl(path: &Path) -> Result<BTreeMap<String, BenchPrediction>> {
    let pred_path = util::resolve_existing_path_upwards(path);
    let file = std::fs::File::open(&pred_path)
        .with_context(|| format!("open predictions: {}", pred_path.display()))?;
    let reader = BufReader::new(file);

    let pred_dir = pred_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut out = BTreeMap::new();

    for (idx, line) in reader.lines().enumerate() {
        let line_no = idx + 1;
        let line = line.with_context(|| {
            format!(
                "E_BENCH_PRED_IO: failed reading {} line {}",
                pred_path.display(),
                line_no
            )
        })?;

        if line.trim().is_empty() {
            continue;
        }

        let row: BenchPredictionLine = serde_json::from_str(&line).with_context(|| {
            format!(
                "E_BENCH_PRED_JSON_PARSE: invalid JSON at {}:{}",
                pred_path.display(),
                line_no
            )
        })?;

        if let Some(sv) = &row.schema_version {
            if sv != "x07.bench.prediction@0.1.0" {
                bail!(
                    "E_BENCH_PRED_SCHEMA_VERSION: unsupported schema_version {:?} at line {}",
                    sv,
                    line_no
                );
            }
        }

        if row.instance_id.trim().is_empty() {
            bail!("E_BENCH_PRED_INSTANCE_ID_EMPTY at line {}", line_no);
        }

        let patch_kind = parse_patch_kind(&row.patch_kind)
            .ok_or_else(|| anyhow!("E_BENCH_PRED_PATCH_KIND_UNSUPPORTED at line {}", line_no))?;

        match (row.patch_path.as_ref(), row.patch_inline.as_ref()) {
            (Some(_), Some(_)) => {
                bail!(
                    "E_BENCH_PRED_PATCH_SOURCE_INVALID: both patch_path and patch_inline set at line {}",
                    line_no
                );
            }
            (None, None) => {
                bail!(
                    "E_BENCH_PRED_PATCH_SOURCE_INVALID: missing patch_path/patch_inline at line {}",
                    line_no
                );
            }
            _ => {}
        }

        if out.contains_key(&row.instance_id) {
            bail!(
                "E_BENCH_PRED_DUPLICATE_INSTANCE_ID: duplicate {} at line {}",
                row.instance_id,
                line_no
            );
        }

        let source = if let Some(rel) = row.patch_path.as_ref() {
            let abs = resolve_safe_patch_path(pred_dir, rel)
                .with_context(|| format!("E_BENCH_PRED_PATCH_PATH_INVALID at line {}", line_no))?;
            PatchSource::Path(abs)
        } else {
            let inline = row
                .patch_inline
                .clone()
                .ok_or_else(|| anyhow!("missing patch_inline at line {}", line_no))?;
            validate_patch_inline_kind(patch_kind, &inline).with_context(|| {
                format!("E_BENCH_PRED_PATCH_INLINE_INVALID at line {}", line_no)
            })?;
            PatchSource::Inline(inline)
        };

        let model = row.model_name_or_path.or(row.model);
        let note = row.note.unwrap_or_default();
        let meta_note = row
            .meta
            .as_ref()
            .map(|m| format!(" meta={}", m))
            .unwrap_or_default();
        let model_name_or_path = if note.is_empty() && meta_note.is_empty() {
            model
        } else if let Some(base) = model {
            Some(format!("{base}{meta_note}"))
        } else if note.is_empty() {
            Some(meta_note.trim_start().to_string())
        } else {
            Some(format!("{note}{meta_note}"))
        };

        out.insert(
            row.instance_id.clone(),
            BenchPrediction {
                patch_kind,
                source,
                model_name_or_path,
            },
        );
    }

    Ok(out)
}

fn validate_patch_inline_kind(kind: BenchPatchKind, value: &Value) -> Result<()> {
    match kind {
        BenchPatchKind::X07ArchPatchsetJson => {
            if !value.is_object() {
                bail!("expected object patch_inline for x07-arch-patchset-json");
            }
        }
        BenchPatchKind::UnifiedDiff => {
            if !value.is_string() {
                bail!("expected string patch_inline for unified-diff");
            }
        }
    }
    Ok(())
}

fn parse_patch_kind(s: &str) -> Option<BenchPatchKind> {
    match s.trim() {
        "x07-arch-patchset-json" | "x07_arch_patchset_json" => {
            Some(BenchPatchKind::X07ArchPatchsetJson)
        }
        "unified-diff" | "unified_diff" => Some(BenchPatchKind::UnifiedDiff),
        _ => None,
    }
}

fn resolve_safe_patch_path(base_dir: &Path, rel: &str) -> Result<PathBuf> {
    if !is_safe_rel_path(rel) {
        bail!("unsafe relative path: {rel}");
    }
    let path = base_dir.join(rel);
    if !path.is_file() {
        bail!("patch file not found: {}", path.display());
    }
    Ok(path)
}

fn oracle_as_prediction(
    instance: &BenchInstanceFile,
    instance_dir: &Path,
) -> Result<Option<BenchPrediction>> {
    let Some(oracle) = instance.oracle.as_ref() else {
        return Ok(None);
    };

    let (kind, rel_path) =
        if let (Some(k), Some(p)) = (oracle.patch_kind, oracle.patch_path.as_ref()) {
            (k, p.as_str())
        } else if let Some(p) = oracle.patchset_path.as_ref() {
            (BenchPatchKind::X07ArchPatchsetJson, p.as_str())
        } else {
            bail!("oracle must provide patch_kind + patch_path");
        };

    let abs = resolve_safe_patch_path(instance_dir, rel_path)?;

    Ok(Some(BenchPrediction {
        patch_kind: kind,
        source: PatchSource::Path(abs),
        model_name_or_path: oracle
            .note
            .as_ref()
            .map(|n| format!("oracle ({n})"))
            .or_else(|| Some("oracle".to_string())),
    }))
}

fn materialize_submission_patch(
    instance_dir: &Path,
    pred: &BenchPrediction,
) -> Result<MaterializedPatch> {
    let patch_dir = instance_dir.join("patch");
    std::fs::create_dir_all(&patch_dir)
        .with_context(|| format!("create patch dir: {}", patch_dir.display()))?;

    match pred.patch_kind {
        BenchPatchKind::X07ArchPatchsetJson => {
            let doc = match &pred.source {
                PatchSource::Path(p) => {
                    let bytes = std::fs::read(p)
                        .with_context(|| format!("read patchset: {}", p.display()))?;
                    serde_json::from_slice::<Value>(&bytes)
                        .with_context(|| format!("parse patchset JSON: {}", p.display()))?
                }
                PatchSource::Inline(v) => v.clone(),
            };

            let diags = validate_schema(
                "E_BENCH_PATCHSET_SCHEMA_INVALID",
                X07_ARCH_PATCHSET_SCHEMA_BYTES,
                &doc,
            )?;
            if !diags.is_empty() {
                bail!("patchset JSON does not match x07.arch.patchset schema");
            }

            let patchset: ArchPatchSet = serde_json::from_value(doc.clone())?;
            if patchset.schema_version.trim() != X07_ARCH_PATCHSET_SCHEMA_VERSION {
                bail!(
                    "patchset schema_version mismatch: expected {} got {:?}",
                    X07_ARCH_PATCHSET_SCHEMA_VERSION,
                    patchset.schema_version
                );
            }

            let sha = util::sha256_hex(&util::canonical_jcs_bytes(&doc)?);
            let out_path = patch_dir.join("submission.patchset.json");
            util::write_atomic(&out_path, canonical_pretty_json_bytes(&doc)?.as_slice())
                .with_context(|| format!("write patchset: {}", out_path.display()))?;

            Ok(MaterializedPatch {
                kind: BenchPatchKind::X07ArchPatchsetJson,
                path: out_path,
                sha256_hex: sha,
            })
        }
        BenchPatchKind::UnifiedDiff => {
            let bytes = match &pred.source {
                PatchSource::Path(p) => {
                    std::fs::read(p).with_context(|| format!("read diff patch: {}", p.display()))?
                }
                PatchSource::Inline(v) => {
                    let s = v
                        .as_str()
                        .ok_or_else(|| anyhow!("unified-diff patch_inline must be a string"))?;
                    s.as_bytes().to_vec()
                }
            };

            let sha = util::sha256_hex(&bytes);
            let out_path = patch_dir.join("submission.diff");
            util::write_atomic(&out_path, &bytes)
                .with_context(|| format!("write diff patch: {}", out_path.display()))?;

            Ok(MaterializedPatch {
                kind: BenchPatchKind::UnifiedDiff,
                path: out_path,
                sha256_hex: sha,
            })
        }
    }
}

fn apply_materialized_patch(repo_root: &Path, patch: &MaterializedPatch) -> Result<Vec<PathBuf>> {
    match patch.kind {
        BenchPatchKind::X07ArchPatchsetJson => apply_patchset_allow_create(repo_root, &patch.path),
        BenchPatchKind::UnifiedDiff => apply_unified_diff(repo_root, &patch.path),
    }
}

fn apply_patchset_allow_create(repo_root: &Path, patchset_path: &Path) -> Result<Vec<PathBuf>> {
    let bytes = std::fs::read(patchset_path)
        .with_context(|| format!("read patchset: {}", patchset_path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse patchset JSON: {}", patchset_path.display()))?;

    let diags = validate_schema(
        "E_BENCH_PATCHSET_SCHEMA_INVALID",
        X07_ARCH_PATCHSET_SCHEMA_BYTES,
        &doc,
    )?;
    if !diags.is_empty() {
        bail!("patchset JSON does not match x07.arch.patchset schema");
    }

    let patchset: ArchPatchSet = serde_json::from_value(doc)
        .with_context(|| format!("decode patchset: {}", patchset_path.display()))?;

    if patchset.schema_version.trim() != X07_ARCH_PATCHSET_SCHEMA_VERSION {
        bail!(
            "patchset schema_version mismatch: expected {} got {:?}",
            X07_ARCH_PATCHSET_SCHEMA_VERSION,
            patchset.schema_version
        );
    }

    let mut touched = BTreeSet::new();

    for target in patchset.patches {
        if let Some(note) = target.note.as_deref() {
            let _ = note;
        }
        if !is_safe_rel_path(&target.path) {
            bail!("unsafe patch target path: {}", target.path);
        }

        let abs = repo_root.join(&target.path);
        let mut doc = if abs.is_file() {
            let existing =
                std::fs::read(&abs).with_context(|| format!("read: {}", abs.display()))?;
            serde_json::from_slice::<Value>(&existing)
                .with_context(|| format!("parse JSON: {}", abs.display()))?
        } else {
            Value::Null
        };

        json_patch::apply_patch(&mut doc, &target.patch)
            .with_context(|| format!("apply patch: {}", abs.display()))?;

        let out_bytes = if target.path.ends_with(".x07.json") {
            let mut file = x07c::x07ast::parse_x07ast_json(&serde_json::to_vec(&doc)?)
                .map_err(|e| anyhow!("x07AST parse after patch: {e}"))?;
            x07c::x07ast::canonicalize_x07ast_file(&mut file);
            let mut v = x07c::x07ast::x07ast_file_to_value(&file);
            x07c::x07ast::canon_value_jcs(&mut v);
            let mut out = serde_json::to_vec_pretty(&v)?;
            if out.last() != Some(&b'\n') {
                out.push(b'\n');
            }
            out
        } else {
            canonical_pretty_json_bytes(&doc)?
        };

        util::write_atomic(&abs, &out_bytes)
            .with_context(|| format!("write patched file: {}", abs.display()))?;

        touched.insert(abs);
    }

    Ok(touched.into_iter().collect())
}

fn apply_unified_diff(repo_root: &Path, diff_path: &Path) -> Result<Vec<PathBuf>> {
    let mut diff_text = String::new();
    std::fs::File::open(diff_path)
        .with_context(|| format!("read diff patch: {}", diff_path.display()))?
        .read_to_string(&mut diff_text)
        .with_context(|| format!("decode diff patch as UTF-8: {}", diff_path.display()))?;

    let touched_rel = parse_unified_diff_touched_paths(&diff_text);
    for rel in &touched_rel {
        if !is_safe_rel_path(rel) {
            bail!("unsafe path in unified diff: {rel}");
        }
    }

    let git_status = Command::new("git")
        .current_dir(repo_root)
        .arg("apply")
        .arg("--whitespace=nowarn")
        .arg("--reject")
        .arg(diff_path)
        .status();

    let applied = match git_status {
        Ok(status) if status.success() => true,
        Ok(_) | Err(_) => {
            let patch_status = Command::new("patch")
                .current_dir(repo_root)
                .arg("-p1")
                .arg("-i")
                .arg(diff_path)
                .status();
            matches!(patch_status, Ok(status) if status.success())
        }
    };

    if !applied {
        bail!(
            "failed to apply unified diff with both git apply and patch: {}",
            diff_path.display()
        );
    }

    let touched = touched_rel.into_iter().map(|p| repo_root.join(p)).collect();
    Ok(touched)
}

fn parse_unified_diff_touched_paths(diff_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in diff_text.lines() {
        if !line.starts_with("+++ ") {
            continue;
        }
        let mut raw = line.trim_start_matches("+++ ").trim();
        if raw == "/dev/null" {
            continue;
        }
        if let Some((path, _meta)) = raw.split_once('\t') {
            raw = path;
        }
        if let Some(path) = raw.strip_prefix("b/") {
            raw = path;
        }
        if let Some(path) = raw.strip_prefix("a/") {
            raw = path;
        }
        if raw.is_empty() {
            continue;
        }
        out.push(raw.to_string());
    }

    out.sort();
    out.dedup();
    out
}

fn maybe_repair_touched_x07ast(
    world: WorldId,
    touched: &[PathBuf],
    args: &RepairArgs,
) -> Result<Option<RepairSummary>> {
    if args.repair == RepairMode::Off {
        return Ok(None);
    }

    let mut summaries = Vec::new();

    for path in touched {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let s = path.to_string_lossy();
        if !s.ends_with(".x07.json") {
            continue;
        }

        let repaired = repair::maybe_repair_x07ast_file(path, world, args)?;
        if let Some(r) = repaired {
            summaries.push(r.summary);
        }
    }

    if summaries.is_empty() {
        return Ok(Some(RepairSummary {
            mode: args.repair,
            iterations: 0,
            applied_ops_count: 0,
            last_lint_ok: true,
        }));
    }

    let mut iterations = 0;
    let mut applied_ops_count = 0;
    let mut last_lint_ok = true;

    for s in summaries {
        iterations = iterations.max(s.iterations);
        applied_ops_count += s.applied_ops_count;
        last_lint_ok = last_lint_ok && s.last_lint_ok;
    }

    Ok(Some(RepairSummary {
        mode: args.repair,
        iterations,
        applied_ops_count,
        last_lint_ok,
    }))
}

fn run_x07_test_subprocess(
    repo_root: &Path,
    eval: &BenchEvalConfig,
    artifact_dir: &Path,
    report_out: &Path,
    stderr_out: &Path,
    jobs_override: usize,
) -> Result<X07TestRun> {
    std::fs::create_dir_all(artifact_dir)
        .with_context(|| format!("create x07test artifact dir: {}", artifact_dir.display()))?;
    if let Some(parent) = report_out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create report dir: {}", parent.display()))?;
    }
    if let Some(parent) = stderr_out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create stderr dir: {}", parent.display()))?;
    }

    let x07_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("x07"));

    let manifest = resolve_repo_path(repo_root, &eval.manifest)?;
    let stdlib_lock = resolve_stdlib_lock(repo_root, &eval.stdlib_lock);

    let stderr_file = std::fs::File::create(stderr_out)
        .with_context(|| format!("create stderr file: {}", stderr_out.display()))?;

    let mut cmd = Command::new(&x07_exe);
    cmd.current_dir(repo_root);
    cmd.arg("test");
    cmd.arg("--manifest").arg(&manifest);
    for root in &eval.module_root {
        let abs = resolve_repo_path(repo_root, root)?;
        cmd.arg("--module-root").arg(abs);
    }
    cmd.arg("--stdlib-lock").arg(stdlib_lock);
    if let Some(filter) = &eval.filter {
        cmd.arg("--filter").arg(filter);
    }
    if eval.exact {
        cmd.arg("--exact");
    }
    cmd.arg("--repeat").arg(eval.repeat.to_string());
    cmd.arg("--jobs").arg(jobs_override.max(1).to_string());
    if eval.no_fail_fast {
        cmd.arg("--no-fail-fast");
    }
    if eval.no_run {
        cmd.arg("--no-run");
    }
    if eval.verbose {
        cmd.arg("--verbose");
    }
    if eval.keep_artifacts {
        cmd.arg("--keep-artifacts");
    }
    cmd.arg("--artifact-dir").arg(artifact_dir);
    cmd.arg("--report-out").arg(report_out);
    cmd.arg("--json").arg("false");
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::from(stderr_file));

    let status = cmd
        .status()
        .with_context(|| format!("exec {} test", x07_exe.display()))?;
    let exit_code = status.code().unwrap_or(1);

    if !report_out.is_file() {
        let snippet = read_stderr_snippet(stderr_out, 4096);
        bail!(
            "E_BENCH_X07TEST_NO_REPORT: x07 test did not produce report {} (exit={}) stderr={} ",
            report_out.display(),
            exit_code,
            snippet
        );
    }

    let report_bytes = std::fs::read(report_out)
        .with_context(|| format!("read report: {}", report_out.display()))?;
    let report_json: Value = serde_json::from_slice(&report_bytes)
        .with_context(|| format!("parse report JSON: {}", report_out.display()))?;

    let summary = report_json
        .get("summary")
        .ok_or_else(|| anyhow!("x07 test report missing summary"))?;

    let test_statuses = parse_test_status_map(&report_json)?;

    let bench_summary = BenchTestSummary {
        ok: exit_code == 0,
        exit_code: Some(exit_code),
        report_path: Some(report_out.display().to_string()),
        passed: summary_u64(summary, "passed"),
        failed: summary_u64(summary, "failed"),
        skipped: summary_u64(summary, "skipped"),
        errors: summary_u64(summary, "errors"),
        duration_ms: summary_u64(summary, "duration_ms"),
        compile_failures: summary_u64(summary, "compile_failures"),
        run_failures: summary_u64(summary, "run_failures"),
    };

    Ok(X07TestRun {
        exit_code,
        summary: bench_summary,
        test_statuses,
    })
}

fn summary_u64(summary: &Value, key: &str) -> u64 {
    summary.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn parse_test_status_map(report_json: &Value) -> Result<BTreeMap<String, String>> {
    let tests = report_json
        .get("tests")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("x07 test report missing tests[]"))?;

    let mut out = BTreeMap::new();
    for t in tests {
        let id = t
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("x07 test report test missing id"))?;
        let status = t
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("x07 test report test missing status"))?;
        out.insert(id.to_string(), status.to_string());
    }
    Ok(out)
}

struct ExpectationCheck {
    satisfied: bool,
    invalid_instance: bool,
    notes: Vec<String>,
}

fn check_expectations(
    eval: &BenchEvalConfig,
    baseline: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> ExpectationCheck {
    let mut satisfied = true;
    let mut invalid_instance = false;
    let mut notes = Vec::new();

    for id in &eval.fail_to_pass {
        let b = baseline.get(id).cloned();
        let a = after.get(id).cloned();
        match (b, a) {
            (Some(b_status), Some(a_status)) => {
                if b_status == "pass" {
                    invalid_instance = true;
                    notes.push(format!(
                        "fail_to_pass baseline is pass for {} (expected failing baseline)",
                        id
                    ));
                }
                if a_status != "pass" {
                    satisfied = false;
                    notes.push(format!(
                        "fail_to_pass not satisfied for {} (after_patch status={})",
                        id, a_status
                    ));
                }
            }
            _ => {
                invalid_instance = true;
                notes.push(format!("fail_to_pass test id missing in reports: {}", id));
            }
        }
    }

    for id in &eval.pass_to_pass {
        let b = baseline.get(id).cloned();
        let a = after.get(id).cloned();
        match (b, a) {
            (Some(b_status), Some(a_status)) => {
                if b_status != "pass" {
                    invalid_instance = true;
                    notes.push(format!(
                        "pass_to_pass baseline is not pass for {} (status={})",
                        id, b_status
                    ));
                }
                if a_status != "pass" {
                    satisfied = false;
                    notes.push(format!(
                        "pass_to_pass not satisfied for {} (after_patch status={})",
                        id, a_status
                    ));
                }
            }
            _ => {
                invalid_instance = true;
                notes.push(format!("pass_to_pass test id missing in reports: {}", id));
            }
        }
    }

    ExpectationCheck {
        satisfied,
        invalid_instance,
        notes,
    }
}

fn resolve_repo_path(repo_root: &Path, rel: &str) -> Result<PathBuf> {
    if !is_safe_rel_path(rel) {
        bail!("unsafe relative path: {rel}");
    }
    Ok(repo_root.join(rel))
}

fn resolve_stdlib_lock(repo_root: &Path, rel: &str) -> PathBuf {
    if is_safe_rel_path(rel) {
        let candidate = repo_root.join(rel);
        if candidate.exists() {
            return candidate;
        }
    }

    util::resolve_existing_path_upwards(Path::new("stdlib.lock"))
}

fn parse_world_id(s: &str) -> Option<WorldId> {
    match s.trim() {
        "solve-pure" => Some(WorldId::SolvePure),
        "solve-fs" => Some(WorldId::SolveFs),
        "solve-rr" => Some(WorldId::SolveRr),
        "solve-kv" => Some(WorldId::SolveKv),
        "solve-full" => Some(WorldId::SolveFull),
        "run-os" => Some(WorldId::RunOs),
        "run-os-sandboxed" => Some(WorldId::RunOsSandboxed),
        _ => None,
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.is_dir() {
        bail!(
            "copy_dir_recursive source is not a directory: {}",
            src.display()
        );
    }

    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let out = dst.join(rel);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&out)
                .with_context(|| format!("create dir: {}", out.display()))?;
            continue;
        }

        if entry.file_type().is_symlink() {
            bail!("symlinks are not supported in benchmark repo snapshots");
        }

        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create parent dir: {}", parent.display()))?;
        }

        std::fs::copy(entry.path(), &out)
            .with_context(|| format!("copy {} -> {}", entry.path().display(), out.display()))?;
    }

    Ok(())
}

fn cleanup_instance_artifacts(instance_dir: &Path) -> Result<()> {
    for rel in ["repo", "x07test"] {
        let path = instance_dir.join(rel);
        if path.exists() {
            std::fs::remove_dir_all(&path).with_context(|| format!("remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn emit_list_report(format: BenchFormat, report: &BenchListReport) -> Result<()> {
    match format {
        BenchFormat::Json => {
            let bytes = canonical_pretty_json_bytes(&serde_json::to_value(report)?)?;
            std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
        }
        BenchFormat::Text => {
            for id in &report.instances {
                println!("{id}");
            }
        }
    }
    Ok(())
}

fn emit_report(format: BenchFormat, report: &BenchReport, out: Option<&Path>) -> Result<()> {
    match format {
        BenchFormat::Json => {
            let bytes = canonical_pretty_json_bytes(&serde_json::to_value(report)?)?;
            if let Some(path) = out {
                util::write_atomic(path, &bytes)
                    .with_context(|| format!("write report: {}", path.display()))?;
            }
            std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
        }
        BenchFormat::Text => {
            println!(
                "suite={} total={} resolved={} unresolved={} errors={} skipped={}",
                report.suite.suite_id,
                report.summary.instances_total,
                report.summary.resolved,
                report.summary.unresolved,
                report.summary.errors,
                report.summary.skipped
            );
            for inst in &report.instances {
                println!("{}\t{}", inst.status.as_str(), inst.instance_id);
            }
            if let Some(path) = out {
                let bytes = canonical_pretty_json_bytes(&serde_json::to_value(report)?)?;
                util::write_atomic(path, &bytes)
                    .with_context(|| format!("write report: {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn emit_json_or_text<T: Serialize>(
    format: BenchFormat,
    value: &T,
    out: Option<&Path>,
) -> Result<()> {
    match format {
        BenchFormat::Json => {
            let bytes = canonical_pretty_json_bytes(&serde_json::to_value(value)?)?;
            if let Some(path) = out {
                util::write_atomic(path, &bytes)
                    .with_context(|| format!("write: {}", path.display()))?;
            } else {
                std::io::Write::write_all(&mut std::io::stdout(), &bytes)
                    .context("write stdout")?;
            }
        }
        BenchFormat::Text => {
            println!("{}", serde_json::to_string(value)?);
            if let Some(path) = out {
                let bytes = canonical_pretty_json_bytes(&serde_json::to_value(value)?)?;
                util::write_atomic(path, &bytes)
                    .with_context(|| format!("write: {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn bench_tool() -> BenchTool {
    BenchTool {
        name: "x07".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build: None,
    }
}

fn bench_invocation(
    args: &BenchEvalArgs,
    suite_path: &Path,
    predictions_path: Option<&PathBuf>,
    runner: BenchRunner,
    docker_image: Option<String>,
) -> BenchInvocation {
    let _ = docker_image;
    BenchInvocation {
        argv: std::env::args().collect(),
        cwd: std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .display()
            .to_string(),
        started_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        suite_path: suite_path.display().to_string(),
        predictions_path: predictions_path.map(|p| p.display().to_string()),
        oracle: args.oracle,
        runner,
        repair_mode: args.repair.repair,
        jobs: args.jobs,
        keep_artifacts: args.keep_artifacts,
        artifact_dir: args.artifact_dir.display().to_string(),
        filter: args.filter.clone(),
        exact: args.exact,
        limit: args.limit,
    }
}

fn bench_env(runner: BenchRunner, docker_image: Option<String>) -> BenchEnv {
    BenchEnv {
        runner,
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        cc: std::env::var("CC").ok(),
        docker_image,
    }
}

fn canonical_pretty_json_bytes(v: &Value) -> Result<Vec<u8>> {
    let mut v = v.clone();
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn validate_schema(
    code: &str,
    schema_bytes: &[u8],
    doc: &Value,
) -> Result<Vec<diagnostics::Diagnostic>> {
    let schema_json: Value = serde_json::from_slice(schema_bytes).context("parse JSON schema")?;
    let x07diag_schema_json: Value =
        serde_json::from_slice(X07DIAG_SCHEMA_BYTES).context("parse x07diag schema")?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_resource(
            "x07diag.schema.json",
            jsonschema::Resource::from_contents(x07diag_schema_json.clone()),
        )
        .with_resource(
            "https://x07.io/spec/x07diag.schema.json",
            jsonschema::Resource::from_contents(x07diag_schema_json),
        )
        .build(&schema_json)
        .context("build schema validator")?;

    let mut out = Vec::new();
    for err in validator.iter_errors(doc) {
        let mut data = BTreeMap::new();
        data.insert(
            "instance_path".to_string(),
            Value::String(err.instance_path().to_string()),
        );
        data.insert(
            "schema_path".to_string(),
            Value::String(err.schema_path().to_string()),
        );
        out.push(diag_parse_error(code, &err.to_string(), None).with_data(data));
    }
    Ok(out)
}

trait DiagnosticExt {
    fn with_data(self, data: BTreeMap<String, Value>) -> Self;
}

impl DiagnosticExt for diagnostics::Diagnostic {
    fn with_data(mut self, data: BTreeMap<String, Value>) -> Self {
        self.data = data;
        self
    }
}

fn diag_parse_error(code: &str, message: &str, file: Option<&str>) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Error,
        stage: diagnostics::Stage::Parse,
        message: message.to_string(),
        loc: file.map(|f| diagnostics::Location::Text {
            span: diagnostics::Span {
                start: diagnostics::Position {
                    line: 1,
                    col: 1,
                    offset: None,
                },
                end: diagnostics::Position {
                    line: 1,
                    col: 1,
                    offset: None,
                },
                file: Some(f.to_string()),
            },
            snippet: None,
        }),
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    }
}

fn safe_artifact_dir_name(id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    let hex = util::hex_lower(&hasher.finalize());
    format!("id_{hex}")
}

fn is_safe_rel_path(raw: &str) -> bool {
    if raw.is_empty() || raw.contains('\\') {
        return false;
    }

    let p = Path::new(raw);
    if p.is_absolute() {
        return false;
    }

    for c in p.components() {
        match c {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
            Component::Normal(_) | Component::CurDir => {}
        }
    }

    true
}

fn read_stderr_snippet(path: &Path, max_bytes: usize) -> String {
    match std::fs::read(path) {
        Ok(bytes) => {
            let clipped = &bytes[..bytes.len().min(max_bytes)];
            String::from_utf8_lossy(clipped).replace('\n', "\\n")
        }
        Err(_) => "<stderr unavailable>".to_string(),
    }
}

impl BenchStatus {
    fn as_str(self) -> &'static str {
        match self {
            BenchStatus::Resolved => "resolved",
            BenchStatus::Unresolved => "unresolved",
            BenchStatus::Error => "error",
            BenchStatus::Skipped => "skipped",
        }
    }
}

impl std::fmt::Display for BenchPatchKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchPatchKind::X07ArchPatchsetJson => write!(f, "x07-arch-patchset-json"),
            BenchPatchKind::UnifiedDiff => write!(f, "unified-diff"),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_stdlib_lock() -> String {
    "stdlib.lock".to_string()
}

fn default_repeat() -> u32 {
    1
}

fn default_jobs() -> usize {
    1
}

fn default_x07test_artifact_dir() -> String {
    "target/x07test".to_string()
}
