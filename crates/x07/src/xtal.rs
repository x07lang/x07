use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use base64::Engine as _;
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;
use x07_worlds::WorldId;
use x07c::ast::Expr;
use x07c::diagnostics;
use x07c::x07ast;

use crate::gen::{GenArgs, GenCommand, GenVerifyArgs};
use crate::patch::{PatchSet, PatchTarget};
use crate::report_common;
use crate::tasks_index::{
    ArchTasksIndex, ArchTasksIndexTask, ARCH_TASKS_INDEX_SCHEMA_BYTES,
    ARCH_TASKS_INDEX_SCHEMA_VERSION,
};
use crate::util;

const SPEC_SCHEMA_VERSION: &str = "x07.x07spec@0.1.0";
const SPEC_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07.x07spec@0.1.0.schema.json");

const EXAMPLES_SCHEMA_VERSION: &str = "x07.x07spec_examples@0.1.0";
const EXAMPLES_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.x07spec_examples@0.1.0.schema.json");

const XTAL_MANIFEST_SCHEMA_VERSION: &str = "x07.xtal.manifest@0.1.0";
const XTAL_MANIFEST_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.xtal.manifest@0.1.0.schema.json");

const CERTIFY_SUMMARY_SCHEMA_VERSION: &str = "x07.xtal.certify_summary@0.1.0";
const CERTIFY_SUMMARY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.xtal.certify_summary@0.1.0.schema.json");

const CERT_BUNDLE_SCHEMA_VERSION: &str = "x07.xtal.cert_bundle@0.1.0";
const CERT_BUNDLE_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.xtal.cert_bundle@0.1.0.schema.json");

const INGEST_SUMMARY_SCHEMA_VERSION: &str = "x07.xtal.ingest_summary@0.1.0";
const INGEST_SUMMARY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.xtal.ingest_summary@0.1.0.schema.json");

const IMPROVE_SUMMARY_SCHEMA_VERSION: &str = "x07.xtal.improve_summary@0.1.0";
const IMPROVE_SUMMARY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.xtal.improve_summary@0.1.0.schema.json");

const CONTRACT_REPRO_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.contract.repro@0.1.0.schema.json");

const RECOVERY_EVENT_SCHEMA_VERSION: &str = "x07.xtal.recovery_event@0.1.0";
const RECOVERY_EVENT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.xtal.recovery_event@0.1.0.schema.json");

const TESTS_MANIFEST_SCHEMA_VERSION: &str = "x07.tests_manifest@0.2.0";
const DEFAULT_SPEC_DIR: &str = "spec";
const DEFAULT_GEN_DIR: &str = "gen/xtal";
const DEFAULT_MANIFEST_PATH: &str = "gen/xtal/tests.json";
const DEFAULT_GEN_INDEX_PATH: &str = "arch/gen/index.x07gen.json";
const DEFAULT_IMPL_DIR: &str = "src";
const DEFAULT_VERIFY_DIR: &str = "target/xtal";
const DEFAULT_VERIFY_ARTIFACT_DIR: &str = "target/xtal/verify";
const DEFAULT_VERIFY_NESTED_ARTIFACT_DIR: &str = "target/xtal/verify/_artifacts";
const DEFAULT_VERIFY_NESTED_TEST_ARTIFACT_DIR: &str = "target/xtal/verify/_artifacts/test";
const DEFAULT_VERIFY_TEST_REPORT_PATH: &str = "target/xtal/tests.report.json";
const DEFAULT_VERIFY_DIAG_REPORT_PATH: &str = "target/xtal/xtal.verify.diag.json";
const DEFAULT_VERIFY_SUMMARY_PATH: &str = "target/xtal/verify/summary.json";
const DEFAULT_CERT_DIR: &str = "target/xtal/cert";
const DEFAULT_CERT_DIAG_REPORT_PATH: &str = "target/xtal/xtal.certify.diag.json";
const DEFAULT_INGEST_DIR: &str = "target/xtal/ingest";
const DEFAULT_INGEST_DIAG_REPORT_PATH: &str = "target/xtal/xtal.ingest.diag.json";
const DEFAULT_IMPROVE_DIR: &str = "target/xtal/improve";
const DEFAULT_IMPROVE_DIAG_REPORT_PATH: &str = "target/xtal/xtal.improve.diag.json";
const DEFAULT_REPAIR_DIR: &str = "target/xtal/repair";

const XTAL_BALANCED_DEFAULT_UNWIND: u32 = 1;
const XTAL_BALANCED_DEFAULT_MAX_BYTES_LEN: u32 = 8;
const XTAL_BALANCED_Z3_TIMEOUT_SECONDS: u64 = 1;

const DEFAULT_REPAIR_ATTEMPTS_DIR: &str = "target/xtal/repair/attempts";
const DEFAULT_REPAIR_PATCHSET_PATH: &str = "target/xtal/repair/patchset.json";
const DEFAULT_REPAIR_DIFF_PATH: &str = "target/xtal/repair/diff.txt";
const DEFAULT_REPAIR_SUMMARY_PATH: &str = "target/xtal/repair/summary.json";
const DEFAULT_REPAIR_DIAG_REPORT_PATH: &str = "target/xtal/xtal.repair.diag.json";
const DEFAULT_REPAIR_BASELINE_DIR: &str = "target/xtal/repair/baseline";
const DEFAULT_TASKS_DIR: &str = "target/xtal/tasks";
const DEFAULT_TASKS_DIAG_REPORT_PATH: &str = "target/xtal/xtal.tasks.diag.json";

static TMP_N: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Args)]
pub struct XtalArgs {
    #[command(subcommand)]
    pub cmd: Option<XtalCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum XtalCommand {
    /// Run the inner loop (prechecks + verify + optional repair-on-fail).
    Dev(XtalDevArgs),
    /// Verify spec + generated tests + test execution.
    Verify(XtalVerifyArgs),
    /// Emit a release certification bundle via `x07 trust certify`.
    Certify(XtalCertifyArgs),
    /// Attempt a bounded repair for a failing `x07 xtal verify`.
    Repair(XtalRepairArgs),
    /// Ingest an incident input (and optionally run an improvement loop).
    Ingest(XtalIngestArgs),
    /// Turn an incident input into a bounded improvement run.
    Improve(XtalImproveArgs),
    /// Run recovery tasks defined under `arch/tasks`.
    Tasks(XtalTasksArgs),
    /// Work with spec modules.
    Spec(XtalSpecArgs),
    /// Work with generated tests from spec examples.
    Tests(XtalTestsArgs),
    /// Implementation conformance helpers.
    Impl(XtalImplArgs),
}

#[derive(Debug, Args)]
pub struct XtalDevArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Spec directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,

    /// Generator index path relative to the project root (defaults to `arch/gen/index.x07gen.json` if present).
    #[arg(long, value_name = "PATH")]
    pub gen_index: Option<PathBuf>,

    /// Stop after spec/gen/impl prechecks (no verification).
    #[arg(long)]
    pub prechecks_only: bool,

    /// If verification fails, apply a bounded repair and re-run verification.
    #[arg(long)]
    pub repair_on_fail: bool,
}

#[derive(Debug, Args)]
pub struct XtalIngestImproveArgs {
    /// Optional review baseline path (passed through to `x07 xtal improve --baseline`).
    #[arg(long, value_name = "PATH")]
    pub baseline: Option<PathBuf>,

    /// Apply the selected patchset to the working tree after validation.
    #[arg(long)]
    pub write: bool,

    /// Permit applying patchsets that change `spec/**`.
    #[arg(long)]
    pub allow_spec_change: bool,

    /// Attempt to reduce the repro input while preserving the failure.
    #[arg(long)]
    pub reduce_repro: bool,

    /// Also run `x07 xtal certify` after applying a patchset.
    #[arg(long)]
    pub certify: bool,

    /// Run recovery tasks from `arch/tasks/index.x07tasks.json` after a verified repair is applied.
    #[arg(long)]
    pub run_tasks: bool,

    /// Output directory relative to the project root.
    #[arg(
        long = "improve-out-dir",
        value_name = "DIR",
        default_value = DEFAULT_IMPROVE_DIR
    )]
    pub improve_out_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct XtalIngestArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Path to a violation (`x07.xtal.violation@0.1.0`) or contract repro (`x07.contract.repro@0.1.0`).
    #[arg(long, value_name = "PATH")]
    pub input: PathBuf,

    /// Output directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_INGEST_DIR)]
    pub out_dir: PathBuf,

    /// Stop after normalization into the ingest workspace (no improvement run).
    #[arg(long)]
    pub normalize_only: bool,

    #[command(flatten)]
    pub improve: XtalIngestImproveArgs,
}

#[derive(Debug, Args)]
pub struct XtalImproveArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Path to a violation bundle, a contract repro, a recovery events log, or a directory containing them.
    #[arg(long, value_name = "PATH")]
    pub input: PathBuf,

    /// Optional review baseline path (passed through to `x07 xtal certify --baseline`).
    #[arg(long, value_name = "PATH")]
    pub baseline: Option<PathBuf>,

    /// Apply the selected patchset to the working tree after validation.
    #[arg(long)]
    pub write: bool,

    /// Permit applying patchsets that change `spec/**`.
    #[arg(long)]
    pub allow_spec_change: bool,

    /// Attempt to reduce the repro input while preserving the failure.
    #[arg(long)]
    pub reduce_repro: bool,

    /// Also run `x07 xtal certify` after applying a patchset.
    #[arg(long)]
    pub certify: bool,

    /// Run recovery tasks from `arch/tasks/index.x07tasks.json` after a verified repair is applied.
    #[arg(long)]
    pub run_tasks: bool,

    /// Output directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_IMPROVE_DIR)]
    pub out_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct XtalTasksArgs {
    #[command(subcommand)]
    pub cmd: Option<XtalTasksCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum XtalTasksCommand {
    /// Execute recovery tasks from the task policy graph.
    Run(XtalTasksRunArgs),
}

#[derive(Debug, Args)]
pub struct XtalTasksRunArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Path to a violation (`x07.xtal.violation@0.1.0`), contract repro (`x07.contract.repro@0.1.0`),
    /// recovery events log, or a directory containing them.
    #[arg(long, value_name = "PATH")]
    pub input: PathBuf,

    /// Task policy graph path relative to the project root.
    #[arg(
        long,
        value_name = "PATH",
        default_value = "arch/tasks/index.x07tasks.json"
    )]
    pub index: PathBuf,

    /// Output directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_TASKS_DIR)]
    pub out_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum ProofPolicy {
    Balanced,
    Strict,
}

impl ProofPolicy {
    fn as_str(self) -> &'static str {
        match self {
            ProofPolicy::Balanced => "balanced",
            ProofPolicy::Strict => "strict",
        }
    }
}

#[derive(Debug, Args)]
pub struct XtalVerifyArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Spec directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,

    /// Generator index path relative to the project root (defaults to `arch/gen/index.x07gen.json` if present).
    #[arg(long, value_name = "PATH")]
    pub gen_index: Option<PathBuf>,

    /// Generated output directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_GEN_DIR)]
    pub gen_dir: PathBuf,

    /// Generated tests manifest path relative to the project root.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_MANIFEST_PATH)]
    pub manifest: PathBuf,

    /// Proof lane policy (`balanced` warns on inconclusive/unsupported; `strict` fails).
    #[arg(long, value_enum, default_value_t = ProofPolicy::Balanced)]
    pub proof_policy: ProofPolicy,

    /// Permit OS-capable worlds (default: require solve-* worlds).
    #[arg(long)]
    pub allow_os_world: bool,

    /// Override the Z3 solver timeout budget passed to `x07 verify --prove` (seconds).
    #[arg(long, value_name = "SECONDS")]
    pub z3_timeout_seconds: Option<u64>,

    /// Override the Z3 solver memory limit passed to `x07 verify --prove` (MB).
    #[arg(long, value_name = "MEGABYTES")]
    pub z3_memory_mb: Option<u64>,

    /// Override loop unwind bound passed to `x07 verify`.
    #[arg(long, value_name = "N")]
    pub unwind: Option<u32>,

    /// Override max bytes length bound passed to `x07 verify`.
    #[arg(long, value_name = "N")]
    pub max_bytes_len: Option<u32>,

    /// Override the verification input encoding length (advanced; passed to `x07 verify`).
    #[arg(long, value_name = "N")]
    pub input_len_bytes: Option<u32>,
}

#[derive(Debug, Args)]
pub struct XtalRepairArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Apply the final patchset to the working tree after validation.
    #[arg(long)]
    pub write: bool,

    /// Maximum repair rounds.
    #[arg(long, value_name = "N", default_value_t = 3)]
    pub max_rounds: u32,

    /// Maximum semantic candidates per round.
    #[arg(long, value_name = "N", default_value_t = 64)]
    pub max_candidates: u32,

    /// Maximum depth for semantic expression enumeration.
    #[arg(long, value_name = "N", default_value_t = 4)]
    pub semantic_max_depth: u32,

    /// Semantic operator preset for expression enumeration.
    #[arg(long, value_enum, default_value_t = SemanticOpsPreset::Safe)]
    pub semantic_ops: SemanticOpsPreset,

    /// Restrict repair to one entrypoint.
    #[arg(long, value_name = "SYM")]
    pub entry: Option<String>,

    /// Only edit functions that match known generated stub bodies (default: true).
    #[arg(long, default_value_t = true, conflicts_with = "allow_edit_non_stubs")]
    pub stubs_only: bool,

    /// Permit editing non-stub implementations.
    #[arg(long)]
    pub allow_edit_non_stubs: bool,

    /// Skip quickfix fallback attempts.
    #[arg(long)]
    pub semantic_only: bool,

    /// Skip semantic repair attempts and only try quickfix repair.
    #[arg(long)]
    pub quickfix_only: bool,

    /// When no implementation patch is found, emit a spec patch suggestion for review.
    #[arg(long)]
    pub suggest_spec_patch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum SemanticOpsPreset {
    Safe,
    Full,
}

#[derive(Debug, Args)]
pub struct XtalCertifyArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Spec directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,

    /// Output directory for certification artifacts (relative to project root).
    #[arg(long, value_name = "DIR", default_value = DEFAULT_CERT_DIR)]
    pub out_dir: PathBuf,

    /// Certify one entry symbol from the XTAL manifest.
    #[arg(long, value_name = "SYM", conflicts_with = "all")]
    pub entry: Option<String>,

    /// Certify every entrypoint configured in the XTAL manifest.
    #[arg(long)]
    pub all: bool,

    /// Optional review baseline path for review gating.
    #[arg(long, value_name = "PATH")]
    pub baseline: Option<PathBuf>,

    /// Skip prechecks (`x07 xtal dev`).
    #[arg(long)]
    pub no_prechecks: bool,

    /// Preserve full test signal after the first failure.
    #[arg(long)]
    pub no_fail_fast: bool,

    /// Override loop unwind bound passed to `x07 trust certify`.
    #[arg(long, value_name = "N")]
    pub unwind: Option<u32>,

    /// Override `bytes`/`bytes_view` length bounds passed to `x07 trust certify`.
    #[arg(long, value_name = "N")]
    pub max_bytes_len: Option<u32>,

    /// Override the encoded proof input length passed to `x07 trust certify`.
    #[arg(long, value_name = "N")]
    pub input_len_bytes: Option<u32>,

    /// Override the Z3 solver timeout budget passed to `x07 trust certify` (seconds).
    #[arg(long, value_name = "SECONDS")]
    pub z3_timeout_seconds: Option<u64>,

    /// Override the Z3 solver memory limit passed to `x07 trust certify` (MB).
    #[arg(long, value_name = "MEGABYTES")]
    pub z3_memory_mb: Option<u64>,
}

#[derive(Debug, Args)]
pub struct XtalSpecArgs {
    #[command(subcommand)]
    pub cmd: Option<XtalSpecCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum XtalSpecCommand {
    /// Canonicalize spec JSON (`--check` / `--write`).
    Fmt(XtalSpecFmtArgs),
    /// Validate spec JSON shape against the schema.
    Lint(XtalSpecLintArgs),
    /// Validate spec semantics (including contracts and examples).
    Check(XtalSpecCheckArgs),
    /// Extract a best-effort spec module from an implementation module.
    Extract(XtalSpecExtractArgs),
    /// Create a spec skeleton file.
    Scaffold(XtalSpecScaffoldArgs),
}

#[derive(Debug, Args)]
pub struct XtalSpecFmtArgs {
    #[arg(long, value_name = "PATH")]
    pub input: Vec<PathBuf>,

    #[arg(long)]
    pub check: bool,

    #[arg(long)]
    pub write: bool,

    /// Inject deterministic ids for missing `operation.id`, `sort.id`, and clause ids.
    #[arg(long)]
    pub inject_ids: bool,
}

#[derive(Debug, Args)]
pub struct XtalSpecLintArgs {
    #[arg(long, value_name = "PATH")]
    pub input: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct XtalSpecCheckArgs {
    #[arg(long, value_name = "PATH")]
    pub input: Vec<PathBuf>,

    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct XtalSpecExtractArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Spec directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,

    /// Implementation directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_IMPL_DIR)]
    pub impl_dir: PathBuf,

    /// Implementation module id to extract from.
    #[arg(long, value_name = "MODULE_ID", conflicts_with = "impl_path")]
    pub module_id: Option<String>,

    /// Implementation module path to extract from.
    #[arg(long, value_name = "PATH", conflicts_with = "module_id")]
    pub impl_path: Option<PathBuf>,

    /// Emit an `x07.patchset@0.1.0` instead of writing spec files.
    #[arg(long, value_name = "PATH", conflicts_with = "write")]
    pub patchset_out: Option<PathBuf>,

    #[arg(long, conflicts_with = "patchset_out")]
    pub write: bool,
}

#[derive(Debug, Args)]
pub struct XtalSpecScaffoldArgs {
    #[arg(long, value_name = "MODULE_ID")]
    pub module_id: String,

    /// Operation local name (appended to module_id).
    #[arg(long, value_name = "NAME", default_value = "op_v1")]
    pub op: String,

    /// Operation parameter in `name:ty` form (repeatable).
    #[arg(long, value_name = "NAME:TY")]
    pub param: Vec<String>,

    /// Operation result type (currently supported: bytes, bytes_view, i32).
    #[arg(long, value_name = "TY", default_value = "i32")]
    pub result: String,

    /// Also create an examples JSONL stub and wire `examples_ref`.
    #[arg(long)]
    pub examples: bool,

    /// Output path for the spec module (defaults to `spec/<module_id>.x07spec.json`).
    #[arg(long, value_name = "PATH")]
    pub out_path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct XtalTestsArgs {
    #[command(subcommand)]
    pub cmd: Option<XtalTestsCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum XtalTestsCommand {
    /// Generate unit tests from spec examples (`--check` / `--write`).
    GenFromSpec(XtalTestsGenArgs),
}

#[derive(Debug, Args)]
pub struct XtalTestsGenArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// One or more spec module paths. If omitted, scans `spec_dir` for `*.x07spec.json`.
    #[arg(long, value_name = "PATH")]
    pub spec: Vec<PathBuf>,

    /// Spec directory relative to the project root (used only when `--spec` is omitted).
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,

    /// Output directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_GEN_DIR)]
    pub out_dir: PathBuf,

    #[arg(long)]
    pub check: bool,

    #[arg(long)]
    pub write: bool,
}

#[derive(Debug, Args)]
pub struct XtalImplArgs {
    #[command(subcommand)]
    pub cmd: Option<XtalImplCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum XtalImplCommand {
    /// Validate that implementations exist and match the spec.
    Check(XtalImplCheckArgs),
    /// Create or update implementation stubs from the spec.
    Sync(XtalImplSyncArgs),
}

#[derive(Debug, Args)]
pub struct XtalImplCheckArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Spec directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,

    /// Implementation directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_IMPL_DIR)]
    pub impl_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct XtalImplSyncArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Spec directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,

    /// Implementation directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_IMPL_DIR)]
    pub impl_dir: PathBuf,

    /// Emit an `x07.patchset@0.1.0` instead of writing impl files.
    #[arg(long, value_name = "PATH", conflicts_with = "write")]
    pub patchset_out: Option<PathBuf>,

    #[arg(long, conflicts_with = "patchset_out")]
    pub write: bool,
}

pub fn cmd_xtal(
    machine: &crate::reporting::MachineArgs,
    args: XtalArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing xtal subcommand (try --help)");
    };
    match cmd {
        XtalCommand::Dev(args) => cmd_xtal_dev(machine, args),
        XtalCommand::Verify(args) => cmd_xtal_verify(machine, args),
        XtalCommand::Certify(args) => cmd_xtal_certify(machine, args),
        XtalCommand::Repair(args) => cmd_xtal_repair(machine, args),
        XtalCommand::Ingest(args) => cmd_xtal_ingest(machine, args),
        XtalCommand::Improve(args) => cmd_xtal_improve(machine, args),
        XtalCommand::Tasks(args) => cmd_xtal_tasks(machine, args),
        XtalCommand::Spec(args) => cmd_xtal_spec(machine, args),
        XtalCommand::Tests(args) => cmd_xtal_tests(machine, args),
        XtalCommand::Impl(args) => cmd_xtal_impl(machine, args),
    }
}

fn cmd_xtal_certify(
    machine: &crate::reporting::MachineArgs,
    args: XtalCertifyArgs,
) -> Result<std::process::ExitCode> {
    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let out_dir_abs = project_root.join(&args.out_dir);
    std::fs::create_dir_all(&out_dir_abs)
        .with_context(|| format!("mkdir: {}", out_dir_abs.display()))?;

    let mut diagnostics = Vec::new();

    let xtal_manifest_path = project_root.join("arch").join("xtal").join("xtal.json");
    let xtal_manifest = if xtal_manifest_path.is_file() {
        load_xtal_manifest(
            &xtal_manifest_path,
            &mut diagnostics,
            "EXTAL_CERTIFY_MANIFEST_PARSE_FAILED",
        )?
    } else {
        diagnostics.push(diag_error(
            "EXTAL_CERTIFY_MANIFEST_MISSING",
            diagnostics::Stage::Parse,
            "arch/xtal/xtal.json is required for certification",
            None,
        ));
        None
    };
    let (entries, trust) = if let Some(manifest) = xtal_manifest.as_ref() {
        let entries =
            resolve_certify_entries(manifest, &args, &mut diagnostics).unwrap_or_default();
        let trust = manifest.trust.as_ref();
        if trust.is_none() {
            diagnostics.push(diag_error(
                "EXTAL_CERTIFY_MANIFEST_PARSE_FAILED",
                diagnostics::Stage::Parse,
                "failed to parse arch/xtal/xtal.json: missing trust section",
                None,
            ));
        }
        (entries, trust)
    } else {
        (Vec::new(), None)
    };

    if !args.no_prechecks {
        let dev_args = XtalDevArgs {
            project: Some(project_root.join("x07.json")),
            spec_dir: args.spec_dir.clone(),
            gen_index: None,
            prechecks_only: true,
            repair_on_fail: false,
        };
        match capture_report_json("xtal_certify_dev", |m| cmd_xtal_dev(m, dev_args)) {
            Ok((code, v)) => {
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                if code != std::process::ExitCode::SUCCESS {
                    diagnostics.push(diag_error(
                        "EXTAL_CERTIFY_PRECHECKS_FAILED",
                        diagnostics::Stage::Run,
                        "xtal prechecks failed; see diagnostics",
                        None,
                    ));
                }
            }
            Err(err) => {
                diagnostics.push(diag_error(
                    "EXTAL_CERTIFY_PRECHECKS_FAILED",
                    diagnostics::Stage::Run,
                    format!("xtal prechecks failed: {err:#}"),
                    None,
                ));
            }
        }
    }

    let trust_profile_path = trust.map(|trust| project_root.join(&trust.cert_profile));
    let trust_profile_exists = trust_profile_path.as_ref().is_some_and(|p| p.is_file());
    if trust.is_some() && !trust_profile_exists {
        diagnostics.push(diag_error(
            "EXTAL_CERTIFY_MANIFEST_PARSE_FAILED",
            diagnostics::Stage::Parse,
            "failed to parse arch/xtal/xtal.json: trust.cert_profile does not exist",
            None,
        ));
    }

    let can_run_trust = diagnostics
        .iter()
        .all(|d| d.severity != diagnostics::Severity::Error)
        && trust.is_some()
        && trust_profile_exists
        && !entries.is_empty();

    let mut results: Vec<Value> = Vec::new();
    if can_run_trust {
        let trust = trust.expect("trust exists when can_run_trust");
        for entry in &entries {
            let entry_dir_name = safe_entry_dir_component(entry);
            let entry_out_dir = out_dir_abs.join(&entry_dir_name);
            std::fs::create_dir_all(&entry_out_dir)
                .with_context(|| format!("mkdir: {}", entry_out_dir.display()))?;

            let entry_out_rel = entry_out_dir
                .strip_prefix(&project_root)
                .unwrap_or(entry_out_dir.as_path())
                .to_string_lossy()
                .replace('\\', "/");

            let mut trust_args: Vec<String> = vec![
                "trust".to_string(),
                "certify".to_string(),
                "--project".to_string(),
                "x07.json".to_string(),
                "--profile".to_string(),
                trust.cert_profile.clone(),
                "--entry".to_string(),
                entry.to_string(),
                "--out-dir".to_string(),
                entry_out_rel.clone(),
                "--tests-manifest".to_string(),
                DEFAULT_MANIFEST_PATH.to_string(),
            ];
            if let Some(unwind) = args.unwind {
                trust_args.push("--unwind".to_string());
                trust_args.push(unwind.to_string());
            }
            if let Some(max_bytes_len) = args.max_bytes_len {
                trust_args.push("--max-bytes-len".to_string());
                trust_args.push(max_bytes_len.to_string());
            }
            if let Some(input_len_bytes) = args.input_len_bytes {
                trust_args.push("--input-len-bytes".to_string());
                trust_args.push(input_len_bytes.to_string());
            }
            if let Some(timeout) = args.z3_timeout_seconds {
                trust_args.push("--z3-timeout-seconds".to_string());
                trust_args.push(timeout.to_string());
            }
            if let Some(mem_mb) = args.z3_memory_mb {
                trust_args.push("--z3-memory-mb".to_string());
                trust_args.push(mem_mb.to_string());
            }
            if let Some(baseline) = args.baseline.as_deref() {
                trust_args.push("--baseline".to_string());
                trust_args.push(baseline.display().to_string());
            }
            if args.no_fail_fast {
                trust_args.push("--no-fail-fast".to_string());
            }
            for gate in &trust.review_gates {
                if gate.trim().is_empty() {
                    continue;
                }
                trust_args.push("--review-fail-on".to_string());
                trust_args.push(gate.trim().to_string());
            }

            let trust_run = run_self_command(&project_root, &trust_args)?;
            let trust_ok = trust_run.exit_code == 0;
            if !trust_ok {
                diagnostics.push(diag_error(
                    "EXTAL_CERTIFY_TRUST_CERTIFY_FAILED",
                    diagnostics::Stage::Run,
                    format!(
                        "trust certification failed for entry {entry:?}; see {}",
                        entry_out_dir.display()
                    ),
                    None,
                ));
            }

            let certificate_path = entry_out_dir.join("certificate.json");
            let trust_report_path = entry_out_dir.join("trust.report.json");
            let review_diff_json_path = entry_out_dir.join("review.diff.json");
            let review_diff_txt_path = entry_out_dir.join("review.diff.txt");

            let cert_sha256 = crate::reporting::file_digest(&certificate_path)
                .ok()
                .map(|d| d.sha256);
            let trust_report_sha256 = crate::reporting::file_digest(&trust_report_path)
                .ok()
                .map(|d| d.sha256);

            let cert_rel = certificate_path
                .strip_prefix(&project_root)
                .unwrap_or(certificate_path.as_path())
                .to_string_lossy()
                .replace('\\', "/");
            let trust_rel = trust_report_path
                .strip_prefix(&project_root)
                .unwrap_or(trust_report_path.as_path())
                .to_string_lossy()
                .replace('\\', "/");
            let review_json_rel = review_diff_json_path
                .strip_prefix(&project_root)
                .unwrap_or(review_diff_json_path.as_path())
                .to_string_lossy()
                .replace('\\', "/");
            let review_txt_rel = review_diff_txt_path
                .strip_prefix(&project_root)
                .unwrap_or(review_diff_txt_path.as_path())
                .to_string_lossy()
                .replace('\\', "/");

            results.push(json!({
                "entry": entry,
                "out_dir": entry_out_rel,
                "ok": trust_ok,
                "certificate_path": cert_rel,
                "certificate_sha256": cert_sha256,
                "trust_report_path": trust_rel,
                "trust_report_sha256": trust_report_sha256,
                "review_diff_json_path": if args.baseline.is_some() { Value::String(review_json_rel) } else { Value::Null },
                "review_diff_txt_path": if args.baseline.is_some() { Value::String(review_txt_rel) } else { Value::Null },
            }));
        }
    }

    let per_entry_ok = results
        .iter()
        .all(|r| r.get("ok").and_then(Value::as_bool).unwrap_or(false));
    let overall_ok = per_entry_ok
        && diagnostics
            .iter()
            .all(|d| d.severity != diagnostics::Severity::Error);

    let review_gates: Vec<String> = trust
        .map(|trust| trust.review_gates.clone())
        .unwrap_or_default();

    let summary = build_certify_summary_value(
        &project_root,
        &args,
        Some(&xtal_manifest_path),
        trust_profile_path.as_deref(),
        args.baseline.as_deref(),
        &entries,
        &review_gates,
        &results,
        overall_ok,
    )?;

    let summary_path = out_dir_abs.join("summary.json");
    let summary_rel = summary_path
        .strip_prefix(&project_root)
        .unwrap_or(summary_path.as_path())
        .to_string_lossy()
        .replace('\\', "/");

    let mut report = diagnostics::Report::ok().with_diagnostics(diagnostics);
    report.ok = overall_ok;
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report.meta.insert(
        "certify_out_dir".to_string(),
        Value::String(args.out_dir.to_string_lossy().replace('\\', "/")),
    );
    report.meta.insert(
        "entries".to_string(),
        Value::Array(entries.iter().cloned().map(Value::String).collect()),
    );
    report.meta.insert(
        "review_gates".to_string(),
        Value::Array(review_gates.iter().cloned().map(Value::String).collect()),
    );
    report.meta.insert(
        "certify_summary_path".to_string(),
        Value::String(summary_rel),
    );
    report.meta.insert(
        "certify_bundle_path".to_string(),
        Value::String(format!(
            "{}/bundle.json",
            args.out_dir
                .to_string_lossy()
                .replace('\\', "/")
                .trim_end_matches('/')
        )),
    );
    report.meta.insert(
        "certify_diag_path".to_string(),
        Value::String(DEFAULT_CERT_DIAG_REPORT_PATH.to_string()),
    );

    write_certify_diag_report(&project_root, &report)?;
    write_certify_summary(&project_root, &args.out_dir, &summary)?;
    write_cert_bundle_manifest(
        &project_root,
        &args.out_dir,
        &args.spec_dir,
        &entries,
        report.ok,
        {
            let mut out = Vec::new();
            let x07_manifest = project_root.join("x07.json");
            if x07_manifest.is_file() {
                out.push(x07_manifest);
            }
            if xtal_manifest_path.is_file() {
                out.push(xtal_manifest_path.clone());
            }
            if let Some(trust_profile_path) = trust_profile_path.as_ref().filter(|p| p.is_file()) {
                out.push(trust_profile_path.to_path_buf());
            }
            if let Some(baseline) = args
                .baseline
                .as_deref()
                .map(|p| util::resolve_existing_path_upwards_from(&project_root, p))
                .filter(|p| p.is_file())
            {
                out.push(baseline);
            }
            let diag_abs = project_root.join(DEFAULT_CERT_DIAG_REPORT_PATH);
            if diag_abs.is_file() {
                out.push(diag_abs);
            }
            out
        }
        .as_slice(),
    )?;

    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(20)
    })
}

#[derive(Debug)]
struct IngestLoadError {
    code: &'static str,
    stage: diagnostics::Stage,
    message: String,
}

impl IngestLoadError {
    fn new(code: &'static str, stage: diagnostics::Stage, message: impl Into<String>) -> Self {
        Self {
            code,
            stage,
            message: message.into(),
        }
    }
}

#[derive(Debug)]
struct LoadedIngestInputs {
    input_kind: String,
    input_file_abs: PathBuf,
    input_files: Vec<Value>,
    integrity: Value,
    violation_doc: Value,
    repro_bytes: Vec<u8>,
    incident_id: String,
    events_file_abs: Option<PathBuf>,
    events_bytes: Option<Vec<u8>>,
}

fn is_sha256_hex(s: &str) -> bool {
    if s.len() != 64 {
        return false;
    }
    s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn resolve_ingest_input_file(input_abs: &Path) -> std::result::Result<PathBuf, IngestLoadError> {
    if input_abs.is_dir() {
        let violation_json = input_abs.join("violation.json");
        let repro_json = input_abs.join("repro.json");
        let events_jsonl = input_abs.join("events.jsonl");
        if violation_json.is_file() {
            return Ok(violation_json);
        }
        if repro_json.is_file() {
            return Ok(repro_json);
        }
        if events_jsonl.is_file() {
            return Ok(events_jsonl);
        }
        return Err(IngestLoadError::new(
            "EXTAL_INGEST_FAILED",
            diagnostics::Stage::Parse,
            format!(
                "input dir does not contain violation.json, repro.json, or events.jsonl: {}",
                input_abs.display()
            ),
        ));
    }
    Ok(input_abs.to_path_buf())
}

fn validate_recovery_events_jsonl(
    events_file: &Path,
    bytes: &[u8],
) -> std::result::Result<Option<String>, IngestLoadError> {
    let parent_id = events_file
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .filter(|s| is_sha256_hex(s))
        .map(|s| s.to_string());
    if parent_id.is_some() {
        // Still validate the file for schema correctness.
    }

    let text = std::str::from_utf8(bytes).map_err(|err| {
        IngestLoadError::new(
            "EXTAL_EVENTS_INVALID",
            diagnostics::Stage::Parse,
            format!(
                "recovery events file is not valid UTF-8: {}: {err}",
                events_file.display()
            ),
        )
    })?;

    let mut derived_id: Option<String> = parent_id;

    for (i, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let doc: Value = serde_json::from_str(line).map_err(|err| {
            IngestLoadError::new(
                "EXTAL_EVENTS_INVALID",
                diagnostics::Stage::Parse,
                format!(
                    "invalid recovery event JSON at line {} in {}: {err}",
                    i + 1,
                    events_file.display()
                ),
            )
        })?;

        let schema_version = doc
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or("");
        if schema_version != RECOVERY_EVENT_SCHEMA_VERSION {
            return Err(IngestLoadError::new(
                "EXTAL_EVENTS_UNSUPPORTED_VERSION",
                diagnostics::Stage::Parse,
                format!(
                    "unsupported recovery event schema_version at line {} in {} (expected {:?}): {:?}",
                    i + 1,
                    events_file.display(),
                    RECOVERY_EVENT_SCHEMA_VERSION,
                    schema_version
                ),
            ));
        }

        let schema_diags = report_common::validate_schema(
            RECOVERY_EVENT_SCHEMA_BYTES,
            "spec/x07.xtal.recovery_event@0.1.0.schema.json",
            &doc,
        )
        .map_err(|err| {
            IngestLoadError::new(
                "EXTAL_EVENTS_INVALID",
                diagnostics::Stage::Parse,
                format!("internal error: cannot validate recovery event schema: {err:#}"),
            )
        })?;
        if !schema_diags.is_empty() {
            return Err(IngestLoadError::new(
                "EXTAL_EVENTS_INVALID",
                diagnostics::Stage::Parse,
                format!(
                    "invalid recovery event at line {} in {}: {}",
                    i + 1,
                    events_file.display(),
                    schema_diags[0].message
                ),
            ));
        }

        if derived_id.is_none() {
            if let Some(id) = doc.get("related_violation_id").and_then(Value::as_str) {
                if is_sha256_hex(id) {
                    derived_id = Some(id.to_string());
                }
            }
        }
    }

    Ok(derived_id)
}

fn load_ingest_inputs(
    project_root: &Path,
    input_abs: &Path,
) -> std::result::Result<LoadedIngestInputs, IngestLoadError> {
    let input_file_abs = resolve_ingest_input_file(input_abs)?;

    if input_file_abs
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext == "jsonl")
        || input_file_abs
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n == "events.jsonl")
    {
        let events_bytes = std::fs::read(&input_file_abs).map_err(|err| {
            IngestLoadError::new(
                "EXTAL_EVENTS_IO",
                diagnostics::Stage::Parse,
                format!(
                    "failed to read recovery events: {}: {err}",
                    input_file_abs.display()
                ),
            )
        })?;

        let incident_id = validate_recovery_events_jsonl(&input_file_abs, &events_bytes)?
            .ok_or_else(|| {
                IngestLoadError::new(
                    "EXTAL_EVENTS_INVALID",
                    diagnostics::Stage::Parse,
                    format!(
                        "cannot determine related_violation_id for recovery events input (expected parent dir name or related_violation_id): {}",
                        input_file_abs.display()
                    ),
                )
            })?;

        let violation_root = crate::xtal_violation::resolve_violation_root_dir(project_root)
            .ok_or_else(|| {
                IngestLoadError::new(
                    "EXTAL_INGEST_FAILED",
                    diagnostics::Stage::Parse,
                    "cannot resolve violations directory for recovery events ingestion (set X07_XTAL_VIOLATIONS_DIR or add arch/xtal/xtal.json)",
                )
            })?;
        let violation_dir = violation_root.join(&incident_id);
        let violation_json = violation_dir.join("violation.json");
        if !violation_json.is_file() {
            return Err(IngestLoadError::new(
                "EXTAL_INGEST_FAILED",
                diagnostics::Stage::Parse,
                format!(
                    "cannot locate violation.json for incident {} (expected {}): {}",
                    incident_id,
                    violation_json.display(),
                    input_file_abs.display()
                ),
            ));
        }

        let mut loaded = load_ingest_inputs(project_root, &violation_json)?;
        loaded.input_kind = "recovery_events".to_string();
        loaded.input_file_abs = input_file_abs.clone();
        loaded.events_file_abs = Some(input_file_abs.clone());
        loaded.events_bytes = Some(events_bytes);

        loaded.input_files.push(
            file_digest_rel_value(project_root, &input_file_abs).map_err(|err| {
                IngestLoadError::new(
                    "EXTAL_EVENTS_IO",
                    diagnostics::Stage::Parse,
                    format!("failed to digest recovery events: {err:#}"),
                )
            })?,
        );
        loaded.input_files.sort_by(|a, b| {
            let ap = a.get("path").and_then(Value::as_str).unwrap_or("");
            let bp = b.get("path").and_then(Value::as_str).unwrap_or("");
            ap.cmp(bp)
        });

        if let Some(checks) = loaded
            .integrity
            .get_mut("checks")
            .and_then(Value::as_array_mut)
        {
            checks.push(json!({ "name": "recovery_events_schema_valid", "ok": true }));
        }

        return Ok(loaded);
    }

    let bytes = std::fs::read(&input_file_abs).map_err(|err| {
        IngestLoadError::new(
            "EXTAL_INGEST_INPUT_SCHEMA_INVALID",
            diagnostics::Stage::Parse,
            format!("failed to read input: {}: {err}", input_file_abs.display()),
        )
    })?;
    let doc: Value = serde_json::from_slice(&bytes).map_err(|err| {
        IngestLoadError::new(
            "EXTAL_INGEST_INPUT_SCHEMA_INVALID",
            diagnostics::Stage::Parse,
            format!("invalid JSON: {}: {err}", input_file_abs.display()),
        )
    })?;
    let schema_version = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");

    if schema_version == crate::xtal_violation::VIOLATION_SCHEMA_VERSION {
        let schema_diags = report_common::validate_schema(
            crate::xtal_violation::VIOLATION_SCHEMA_BYTES,
            "spec/x07.xtal.violation@0.1.0.schema.json",
            &doc,
        )
        .map_err(|err| {
            IngestLoadError::new(
                "EXTAL_INGEST_INPUT_SCHEMA_INVALID",
                diagnostics::Stage::Parse,
                format!("internal error: cannot validate violation schema: {err:#}"),
            )
        })?;
        if !schema_diags.is_empty() {
            return Err(IngestLoadError::new(
                "EXTAL_INGEST_INPUT_SCHEMA_INVALID",
                diagnostics::Stage::Parse,
                format!(
                    "input violation is not schema-valid: {}",
                    schema_diags[0].message
                ),
            ));
        }

        let violation_id = doc.get("id").and_then(Value::as_str).unwrap_or("");
        let repro_rel = doc
            .pointer("/repro/path")
            .and_then(Value::as_str)
            .unwrap_or("repro.json");
        if !crate::util::is_safe_rel_path(repro_rel) {
            return Err(IngestLoadError::new(
                "EXTAL_INGEST_REPRO_PATH_UNSAFE",
                diagnostics::Stage::Parse,
                format!(
                    "unsafe repro path in violation bundle: {:?} (from {})",
                    repro_rel,
                    input_file_abs.display()
                ),
            ));
        }

        let repro_abs = input_file_abs
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(repro_rel);
        if !repro_abs.is_file() {
            return Err(IngestLoadError::new(
                "EXTAL_INGEST_REPRO_NOT_FOUND",
                diagnostics::Stage::Parse,
                format!(
                    "repro file referenced by violation bundle was not found: {}",
                    repro_abs.display()
                ),
            ));
        }

        let repro_bytes = std::fs::read(&repro_abs).map_err(|err| {
            IngestLoadError::new(
                "EXTAL_INGEST_REPRO_NOT_FOUND",
                diagnostics::Stage::Parse,
                format!("failed to read repro: {}: {err}", repro_abs.display()),
            )
        })?;

        let repro_doc: Value = serde_json::from_slice(&repro_bytes).map_err(|err| {
            IngestLoadError::new(
                "EXTAL_INGEST_REPRO_SCHEMA_INVALID",
                diagnostics::Stage::Parse,
                format!("invalid JSON in repro: {}: {err}", repro_abs.display()),
            )
        })?;
        let repro_schema_diags = report_common::validate_schema(
            CONTRACT_REPRO_SCHEMA_BYTES,
            "spec/x07.contract.repro@0.1.0.schema.json",
            &repro_doc,
        )
        .map_err(|err| {
            IngestLoadError::new(
                "EXTAL_INGEST_REPRO_SCHEMA_INVALID",
                diagnostics::Stage::Parse,
                format!("internal error: cannot validate repro schema: {err:#}"),
            )
        })?;
        if !repro_schema_diags.is_empty() {
            return Err(IngestLoadError::new(
                "EXTAL_INGEST_REPRO_SCHEMA_INVALID",
                diagnostics::Stage::Parse,
                format!(
                    "input repro is not schema-valid: {}",
                    repro_schema_diags[0].message
                ),
            ));
        }

        let expected_id = util::sha256_hex(&repro_bytes);
        let actual_repro_sha256 = util::sha256_hex(&repro_bytes);
        let violation_repro_sha256 = doc
            .pointer("/repro/sha256")
            .and_then(Value::as_str)
            .unwrap_or("");
        let violation_repro_bytes_len = doc
            .pointer("/repro/bytes_len")
            .and_then(Value::as_u64)
            .unwrap_or(0);

        let checks: Vec<Value> = vec![
            json!({ "name": "violation_schema_valid", "ok": true }),
            json!({ "name": "repro_schema_valid", "ok": true }),
            json!({
                "name": "violation_id_matches_repro",
                "ok": violation_id == expected_id,
                "details": { "expected": expected_id, "actual": violation_id }
            }),
            json!({
                "name": "violation_repro_sha256_matches",
                "ok": violation_repro_sha256 == actual_repro_sha256,
                "details": { "expected": actual_repro_sha256, "actual": violation_repro_sha256 }
            }),
            json!({
                "name": "violation_repro_bytes_len_matches",
                "ok": violation_repro_bytes_len == repro_bytes.len() as u64,
                "details": { "expected": repro_bytes.len(), "actual": violation_repro_bytes_len }
            }),
        ];

        let ok = checks
            .iter()
            .all(|c| c.get("ok").and_then(Value::as_bool) == Some(true));
        if !ok {
            let mut mismatches: Vec<String> = Vec::new();
            if violation_id != expected_id {
                mismatches.push(format!(
                    "violation.id mismatch (expected {expected_id} got {violation_id})"
                ));
            }
            if violation_repro_sha256 != actual_repro_sha256 {
                mismatches.push(format!(
                    "violation.repro.sha256 mismatch (expected {actual_repro_sha256} got {violation_repro_sha256})"
                ));
            }
            if violation_repro_bytes_len != repro_bytes.len() as u64 {
                mismatches.push(format!(
                    "violation.repro.bytes_len mismatch (expected {} got {violation_repro_bytes_len})",
                    repro_bytes.len()
                ));
            }
            if mismatches.is_empty() {
                mismatches.push("unknown integrity mismatch".to_string());
            }
            return Err(IngestLoadError::new(
                "EXTAL_INGEST_INTEGRITY_MISMATCH",
                diagnostics::Stage::Run,
                mismatches.join("; "),
            ));
        }

        let (id, mut violation_doc) =
            crate::xtal_violation::build_contract_violation_doc(project_root, None, &repro_bytes)
                .map_err(|err| {
                IngestLoadError::new(
                    "EXTAL_INGEST_FAILED",
                    diagnostics::Stage::Run,
                    format!("failed to normalize violation doc: {err:#}"),
                )
            })?;

        if let Some(original_repro_path) = doc.get("original_repro_path").and_then(Value::as_str) {
            if let Some(obj) = violation_doc.as_object_mut() {
                obj.insert(
                    "original_repro_path".to_string(),
                    Value::String(original_repro_path.to_string()),
                );
            }
        }

        let mut input_files: Vec<Value> = vec![
            file_digest_rel_value(project_root, &input_file_abs).map_err(|err| {
                IngestLoadError::new(
                    "EXTAL_INGEST_FAILED",
                    diagnostics::Stage::Parse,
                    format!("digest input violation: {err:#}"),
                )
            })?,
            file_digest_rel_value(project_root, &repro_abs).map_err(|err| {
                IngestLoadError::new(
                    "EXTAL_INGEST_FAILED",
                    diagnostics::Stage::Parse,
                    format!("digest input repro: {err:#}"),
                )
            })?,
        ];
        input_files.sort_by(|a, b| {
            let ap = a.get("path").and_then(Value::as_str).unwrap_or("");
            let bp = b.get("path").and_then(Value::as_str).unwrap_or("");
            ap.cmp(bp)
        });

        let integrity = json!({
            "ok": true,
            "expected_repro_sha256": expected_id,
            "actual_repro_sha256": actual_repro_sha256,
            "checks": checks,
        });

        Ok(LoadedIngestInputs {
            input_kind: "violation".to_string(),
            input_file_abs,
            input_files,
            integrity,
            violation_doc,
            repro_bytes,
            incident_id: id,
            events_file_abs: None,
            events_bytes: None,
        })
    } else if schema_version == x07_contracts::X07_CONTRACT_REPRO_SCHEMA_VERSION {
        let schema_diags = report_common::validate_schema(
            CONTRACT_REPRO_SCHEMA_BYTES,
            "spec/x07.contract.repro@0.1.0.schema.json",
            &doc,
        )
        .map_err(|err| {
            IngestLoadError::new(
                "EXTAL_INGEST_REPRO_SCHEMA_INVALID",
                diagnostics::Stage::Parse,
                format!("internal error: cannot validate repro schema: {err:#}"),
            )
        })?;
        if !schema_diags.is_empty() {
            return Err(IngestLoadError::new(
                "EXTAL_INGEST_REPRO_SCHEMA_INVALID",
                diagnostics::Stage::Parse,
                format!(
                    "input repro is not schema-valid: {}",
                    schema_diags[0].message
                ),
            ));
        }

        let repro_bytes = bytes;
        let sha = util::sha256_hex(&repro_bytes);

        let (id, violation_doc) = crate::xtal_violation::build_contract_violation_doc(
            project_root,
            Some(&input_file_abs),
            &repro_bytes,
        )
        .map_err(|err| {
            IngestLoadError::new(
                "EXTAL_INGEST_FAILED",
                diagnostics::Stage::Run,
                format!("failed to build violation doc from repro: {err:#}"),
            )
        })?;

        let input_files = vec![
            file_digest_rel_value(project_root, &input_file_abs).map_err(|err| {
                IngestLoadError::new(
                    "EXTAL_INGEST_FAILED",
                    diagnostics::Stage::Parse,
                    format!("digest input repro: {err:#}"),
                )
            })?,
        ];

        let integrity = json!({
            "ok": true,
            "expected_repro_sha256": sha,
            "actual_repro_sha256": sha,
            "checks": [
                { "name": "repro_schema_valid", "ok": true },
                { "name": "repro_sha256_computed", "ok": true, "details": { "sha256": sha } }
            ]
        });

        Ok(LoadedIngestInputs {
            input_kind: "contract_repro".to_string(),
            input_file_abs,
            input_files,
            integrity,
            violation_doc,
            repro_bytes,
            incident_id: id,
            events_file_abs: None,
            events_bytes: None,
        })
    } else {
        Err(IngestLoadError::new(
            "EXTAL_INGEST_INPUT_SCHEMA_VERSION_UNSUPPORTED",
            diagnostics::Stage::Parse,
            format!(
                "unsupported input schema_version {:?} (expected {:?} or {:?})",
                schema_version,
                crate::xtal_violation::VIOLATION_SCHEMA_VERSION,
                x07_contracts::X07_CONTRACT_REPRO_SCHEMA_VERSION
            ),
        ))
    }
}

fn cmd_xtal_ingest(
    machine: &crate::reporting::MachineArgs,
    args: XtalIngestArgs,
) -> Result<std::process::ExitCode> {
    let project_root =
        resolve_project_root(args.project.as_deref(), None).context("resolve project root")?;

    let cwd = std::env::current_dir().context("cwd")?;
    let input_abs = if args.input.is_absolute() {
        args.input.clone()
    } else {
        cwd.join(&args.input)
    };

    let out_root_abs = project_root.join(&args.out_dir);

    let mut diagnostics: Vec<diagnostics::Diagnostic> = Vec::new();

    let mut report = diagnostics::Report::ok();
    report.meta.insert(
        "ingest_input_path".to_string(),
        Value::String(
            input_abs
                .strip_prefix(&project_root)
                .unwrap_or(&input_abs)
                .to_string_lossy()
                .replace('\\', "/"),
        ),
    );

    match load_ingest_inputs(&project_root, &input_abs) {
        Ok(loaded) => {
            let input_kind = loaded.input_kind.clone();
            let id = loaded.incident_id.clone();
            let mut improve_ok = true;

            std::fs::create_dir_all(&out_root_abs)
                .with_context(|| format!("mkdir: {}", out_root_abs.display()))?;

            let incident_dir_abs = out_root_abs.join(&id);
            crate::xtal_violation::write_violation_bundle(
                &incident_dir_abs,
                &loaded.violation_doc,
                &loaded.repro_bytes,
            )
            .context("write ingest violation bundle")?;

            if let Some(events_bytes) = loaded.events_bytes.as_deref() {
                let out_path = incident_dir_abs.join("events.jsonl");
                util::write_atomic(&out_path, events_bytes)
                    .with_context(|| format!("write: {}", out_path.display()))?;
            }

            let summary = build_ingest_summary_value(
                &project_root,
                &args.out_dir,
                &input_abs,
                &loaded,
                &incident_dir_abs,
            )?;
            write_ingest_summary(&project_root, &args.out_dir, &summary)?;

            let out_dir_rel = args.out_dir.to_string_lossy().replace('\\', "/");
            let out_dir_rel_trim = out_dir_rel.trim_end_matches('/');
            report.meta.insert(
                "ingest_summary_path".to_string(),
                Value::String(format!("{}/summary.json", out_dir_rel_trim)),
            );
            report
                .meta
                .insert("ingest_incident_id".to_string(), Value::String(id.clone()));
            report.meta.insert(
                "ingest_incident_dir".to_string(),
                Value::String(format!("{}/{}", out_dir_rel_trim, id)),
            );

            report
                .meta
                .insert("ingest_input_kind".to_string(), Value::String(input_kind));

            report.meta.insert(
                "ingest_violation_path".to_string(),
                Value::String(format!("{}/{}/violation.json", out_dir_rel_trim, id)),
            );
            report.meta.insert(
                "ingest_repro_path".to_string(),
                Value::String(format!("{}/{}/repro.json", out_dir_rel_trim, id)),
            );

            let events_abs = incident_dir_abs.join("events.jsonl");
            if events_abs.is_file() {
                report.meta.insert(
                    "ingest_events_path".to_string(),
                    Value::String(format!("{}/{}/events.jsonl", out_dir_rel_trim, id)),
                );
            }

            if !args.normalize_only {
                let improve_args = XtalImproveArgs {
                    project: Some(project_root.join("x07.json")),
                    input: incident_dir_abs.clone(),
                    baseline: args.improve.baseline.clone(),
                    write: args.improve.write,
                    allow_spec_change: args.improve.allow_spec_change,
                    reduce_repro: args.improve.reduce_repro,
                    certify: args.improve.certify,
                    run_tasks: args.improve.run_tasks,
                    out_dir: args.improve.improve_out_dir.clone(),
                };

                match capture_report_json("xtal_ingest_improve", |m| {
                    cmd_xtal_improve(m, improve_args)
                }) {
                    Ok((code, v)) => {
                        diagnostics.extend(
                            crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default(),
                        );
                        if code != std::process::ExitCode::SUCCESS {
                            improve_ok = false;
                        }

                        let improve_out_dir_rel = args
                            .improve
                            .improve_out_dir
                            .to_string_lossy()
                            .replace('\\', "/");
                        let improve_out_dir_rel_trim = improve_out_dir_rel.trim_end_matches('/');
                        report.meta.insert(
                            "improve_summary_path".to_string(),
                            Value::String(format!("{improve_out_dir_rel_trim}/summary.json")),
                        );
                    }
                    Err(err) => {
                        improve_ok = false;
                        diagnostics.push(diag_error(
                            "X07-INTERNAL-0001",
                            diagnostics::Stage::Run,
                            format!("xtal improve capture failed: {err:#}"),
                            None,
                        ));
                    }
                }
            }

            if !improve_ok {
                report.ok = false;
            }
        }
        Err(err) => {
            diagnostics.push(diag_error(err.code, err.stage, err.message, None));
            report.ok = false;
        }
    }

    report = report.with_diagnostics(diagnostics);

    write_ingest_diag_report(&project_root, &report)?;
    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_xtal_tasks(
    machine: &crate::reporting::MachineArgs,
    args: XtalTasksArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing xtal tasks subcommand (try --help)");
    };
    match cmd {
        XtalTasksCommand::Run(args) => cmd_xtal_tasks_run(machine, args),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskOutcome {
    Ok,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskFnKind {
    Defn,
    DefAsync,
}

#[derive(Debug, Clone)]
struct TaskFnSig {
    kind: TaskFnKind,
    params: Vec<String>,
    result: String,
}

fn cmd_xtal_tasks_run(
    machine: &crate::reporting::MachineArgs,
    args: XtalTasksRunArgs,
) -> Result<std::process::ExitCode> {
    let cwd = std::env::current_dir().context("cwd")?;
    let input_abs = if args.input.is_absolute() {
        args.input.clone()
    } else {
        cwd.join(&args.input)
    };

    let project_root = resolve_project_root(args.project.as_deref(), Some(&input_abs))
        .context("resolve project root")?;

    let mut diags: Vec<diagnostics::Diagnostic> = Vec::new();
    let mut report = diagnostics::Report::ok();

    report.meta.insert(
        "tasks_input_path".to_string(),
        Value::String(
            input_abs
                .strip_prefix(&project_root)
                .unwrap_or(&input_abs)
                .to_string_lossy()
                .replace('\\', "/"),
        ),
    );

    let loaded = match load_ingest_inputs(&project_root, &input_abs) {
        Ok(loaded) => loaded,
        Err(err) => {
            diags.push(diag_error(err.code, err.stage, err.message, None));
            report.ok = false;
            report = report.with_diagnostics(diags);
            write_tasks_diag_report(&project_root, &report)?;
            write_report(machine, &report)?;
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let incident_id = loaded.incident_id.clone();
    report.meta.insert(
        "tasks_incident_id".to_string(),
        Value::String(incident_id.clone()),
    );

    let out_root_abs = project_root.join(&args.out_dir);
    std::fs::create_dir_all(&out_root_abs)
        .with_context(|| format!("mkdir: {}", out_root_abs.display()))?;
    let incident_out_dir_abs = out_root_abs.join(&incident_id);
    std::fs::create_dir_all(&incident_out_dir_abs)
        .with_context(|| format!("mkdir: {}", incident_out_dir_abs.display()))?;

    report.meta.insert(
        "tasks_out_dir".to_string(),
        Value::String(
            incident_out_dir_abs
                .strip_prefix(&project_root)
                .unwrap_or(&incident_out_dir_abs)
                .to_string_lossy()
                .replace('\\', "/"),
        ),
    );

    let repro_doc: Value =
        serde_json::from_slice(&loaded.repro_bytes).context("parse repro JSON")?;
    let world = repro_doc
        .get("world")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let source = repro_doc
        .get("source")
        .cloned()
        .unwrap_or_else(|| json!({ "mode": "unknown" }));

    let world_id = x07c::world_config::parse_world_id(&world).context("parse repro world")?;

    let input_b64 = repro_doc
        .get("input_bytes_b64")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    #[derive(Debug, Clone)]
    struct RunnerCfg {
        solve_fuel: Option<u64>,
        max_memory_bytes: Option<u64>,
        max_output_bytes: Option<u64>,
        cpu_time_limit_seconds: Option<u64>,
        debug_borrow_checks: bool,
        fixture_fs_dir: Option<String>,
        fixture_fs_root: Option<String>,
        fixture_fs_latency_index: Option<String>,
        fixture_rr_dir: Option<String>,
        fixture_kv_dir: Option<String>,
        fixture_kv_seed: Option<String>,
    }

    let runner_cfg = {
        let runner = repro_doc.get("runner").cloned().unwrap_or(Value::Null);
        RunnerCfg {
            solve_fuel: runner.get("solve_fuel").and_then(Value::as_u64),
            max_memory_bytes: runner.get("max_memory_bytes").and_then(Value::as_u64),
            max_output_bytes: runner.get("max_output_bytes").and_then(Value::as_u64),
            cpu_time_limit_seconds: runner.get("cpu_time_limit_seconds").and_then(Value::as_u64),
            debug_borrow_checks: runner
                .get("debug_borrow_checks")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            fixture_fs_dir: runner
                .get("fixture_fs_dir")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
            fixture_fs_root: runner
                .get("fixture_fs_root")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
            fixture_fs_latency_index: runner
                .get("fixture_fs_latency_index")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
            fixture_rr_dir: runner
                .get("fixture_rr_dir")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
            fixture_kv_dir: runner
                .get("fixture_kv_dir")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
            fixture_kv_seed: runner
                .get("fixture_kv_seed")
                .and_then(Value::as_str)
                .map(|s| s.to_string()),
        }
    };

    let project_path = project_root.join("x07.json");
    if !project_path.is_file() {
        diags.push(diag_error(
            "EXTAL_TASKS_PROJECT_MISSING",
            diagnostics::Stage::Parse,
            "x07.json is required for running recovery tasks",
            None,
        ));
        report.ok = false;
        report = report.with_diagnostics(diags);
        write_tasks_diag_report(&project_root, &report)?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let ctx = crate::project_ctx::load_project_ctx(&project_path, true)
        .context("load project context")?;
    let module_roots: Vec<PathBuf> = ctx
        .module_roots
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                project_root.join(p)
            }
        })
        .collect();

    let index_abs = if args.index.is_absolute() {
        args.index.clone()
    } else {
        project_root.join(&args.index)
    };
    if !index_abs.is_file() {
        diags.push(diag_error(
            "EXTAL_TASKS_INDEX_MISSING",
            diagnostics::Stage::Parse,
            format!("tasks index not found: {}", index_abs.display()),
            None,
        ));
        report.ok = false;
        report = report.with_diagnostics(diags);
        write_tasks_diag_report(&project_root, &report)?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let index_bytes =
        std::fs::read(&index_abs).with_context(|| format!("read: {}", index_abs.display()))?;
    let index_doc: Value =
        serde_json::from_slice(&index_bytes).context("parse tasks index JSON")?;

    if index_doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("")
        != ARCH_TASKS_INDEX_SCHEMA_VERSION
    {
        diags.push(diag_error(
            "EXTAL_TASKS_INDEX_INVALID",
            diagnostics::Stage::Parse,
            "schema_version mismatch for tasks index",
            None,
        ));
        report.ok = false;
    } else {
        let schema_diags = report_common::validate_schema(
            ARCH_TASKS_INDEX_SCHEMA_BYTES,
            "spec/x07-arch.tasks.index.schema.json",
            &index_doc,
        )?;
        if !schema_diags.is_empty() {
            diags.push(diag_error(
                "EXTAL_TASKS_INDEX_INVALID",
                diagnostics::Stage::Parse,
                format!(
                    "tasks index is not schema-valid: {}",
                    schema_diags[0].message
                ),
                None,
            ));
            report.ok = false;
        }
    }

    let index_obj: ArchTasksIndex = match serde_json::from_value(index_doc.clone()) {
        Ok(v) => v,
        Err(err) => {
            diags.push(diag_error(
                "EXTAL_TASKS_INDEX_INVALID",
                diagnostics::Stage::Parse,
                format!("parse tasks index: {err}"),
                None,
            ));
            report.ok = false;
            ArchTasksIndex {
                schema_version: "".to_string(),
                tasks: Vec::new(),
            }
        }
    };

    if !report.ok {
        report = report.with_diagnostics(diags);
        write_tasks_diag_report(&project_root, &report)?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    fn topo_order_tasks(tasks: &[ArchTasksIndexTask]) -> Result<Vec<&ArchTasksIndexTask>> {
        let mut by_id: BTreeMap<&str, &ArchTasksIndexTask> = BTreeMap::new();
        for t in tasks {
            if by_id.contains_key(t.id.as_str()) {
                anyhow::bail!("duplicate task id in index: {:?}", t.id);
            }
            by_id.insert(t.id.as_str(), t);
        }

        let mut out = Vec::with_capacity(tasks.len());
        let mut perm: BTreeSet<&str> = BTreeSet::new();
        let mut temp: BTreeSet<&str> = BTreeSet::new();

        fn visit<'a>(
            id: &'a str,
            by_id: &BTreeMap<&'a str, &'a ArchTasksIndexTask>,
            perm: &mut BTreeSet<&'a str>,
            temp: &mut BTreeSet<&'a str>,
            out: &mut Vec<&'a ArchTasksIndexTask>,
        ) -> Result<()> {
            if perm.contains(id) {
                return Ok(());
            }
            if temp.contains(id) {
                anyhow::bail!("cycle detected in tasks index at {id:?}");
            }
            let Some(task) = by_id.get(id).copied() else {
                anyhow::bail!("task dependency references missing id: {id:?}");
            };
            temp.insert(id);
            for dep in &task.deps {
                visit(dep, by_id, perm, temp, out)?;
            }
            temp.remove(id);
            perm.insert(id);
            out.push(task);
            Ok(())
        }

        for id in by_id.keys().copied().collect::<Vec<_>>() {
            visit(id, &by_id, &mut perm, &mut temp, &mut out)?;
        }
        Ok(out)
    }

    fn load_task_fn_sig(
        module_roots: &[PathBuf],
        world: WorldId,
        fn_symbol: &str,
    ) -> Result<TaskFnSig> {
        let (module_id, _) = fn_symbol
            .rsplit_once('.')
            .context("task fn must contain '.'")?;
        let source = x07c::module_source::load_module_source(module_id, world, module_roots)
            .map_err(|err| anyhow::anyhow!(err.message.to_string()))?;
        let doc: Value = serde_json::from_str(&source.src)
            .with_context(|| format!("parse module JSON for {module_id:?}"))?;
        let decls = doc
            .get("decls")
            .and_then(Value::as_array)
            .context("module missing decls[]")?;
        for d in decls {
            let kind = d.get("kind").and_then(Value::as_str).unwrap_or("");
            let sig_kind = match kind {
                "defn" => TaskFnKind::Defn,
                "defasync" => TaskFnKind::DefAsync,
                _ => continue,
            };
            let name = d.get("name").and_then(Value::as_str).unwrap_or("");
            if name != fn_symbol {
                continue;
            }
            let params = d
                .get("params")
                .and_then(Value::as_array)
                .context("defn missing params[]")?;
            let mut out_params = Vec::with_capacity(params.len());
            for p in params {
                let ty = p
                    .get("ty")
                    .and_then(Value::as_str)
                    .context("param missing ty")?;
                out_params.push(ty.to_string());
            }
            let result = d
                .get("result")
                .and_then(Value::as_str)
                .context("defn missing result")?
                .to_string();
            return Ok(TaskFnSig {
                kind: sig_kind,
                params: out_params,
                result,
            });
        }
        anyhow::bail!("task fn not found: {fn_symbol:?}");
    }

    fn build_task_wrapper(task: &ArchTasksIndexTask, sig: &TaskFnSig) -> Result<Value> {
        if sig.result != "bytes" {
            anyhow::bail!(
                "unsupported task result type (expected bytes): {:?} -> {:?}",
                task.fn_symbol,
                sig.result
            );
        }

        let mut args: Vec<Value> = Vec::new();
        match sig.params.len() {
            0 => {}
            1 => {
                let ty = sig.params[0].as_str();
                match ty {
                    "bytes_view" => args.push(Value::String("input".to_string())),
                    "bytes" => args.push(Value::Array(vec![
                        Value::String("view.to_bytes".to_string()),
                        Value::String("input".to_string()),
                    ])),
                    _ => {
                        anyhow::bail!(
                            "unsupported task param type (expected bytes_view/bytes): {:?} param[0]={:?}",
                            task.fn_symbol,
                            ty
                        );
                    }
                }
            }
            n => {
                anyhow::bail!(
                    "unsupported task signature (expected 0 or 1 params): {:?} params={n}",
                    task.fn_symbol
                );
            }
        }

        let (module_id, _) = task
            .fn_symbol
            .rsplit_once('.')
            .context("task fn must contain '.'")?;

        let mut call_items: Vec<Value> = Vec::with_capacity(1 + args.len());
        call_items.push(Value::String(task.fn_symbol.clone()));
        call_items.extend(args);
        let call = Value::Array(call_items);
        let solve = match sig.kind {
            TaskFnKind::Defn => call,
            TaskFnKind::DefAsync => Value::Array(vec![Value::String("await".to_string()), call]),
        };

        Ok(json!({
            "schema_version": "x07.x07ast@0.8.0",
            "module_id": "main",
            "kind": "entry",
            "imports": [module_id],
            "decls": [],
            "solve": solve,
        }))
    }

    let ordered = match topo_order_tasks(&index_obj.tasks) {
        Ok(v) => v,
        Err(err) => {
            diags.push(diag_error(
                "EXTAL_TASKS_INDEX_INVALID",
                diagnostics::Stage::Parse,
                format!("tasks index is not runnable: {err:#}"),
                None,
            ));
            report.ok = false;
            report = report.with_diagnostics(diags);
            write_tasks_diag_report(&project_root, &report)?;
            write_report(machine, &report)?;
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let mut outcomes: BTreeMap<String, TaskOutcome> = BTreeMap::new();
    let mut overall_ok = true;
    let mut fail_fast_triggered = false;

    for task in ordered {
        if fail_fast_triggered {
            break;
        }

        let safe_task = safe_entry_dir_component(&task.id);
        let task_out_dir_abs = incident_out_dir_abs.join(&safe_task);
        std::fs::create_dir_all(&task_out_dir_abs)
            .with_context(|| format!("mkdir: {}", task_out_dir_abs.display()))?;

        let deps_ok = task
            .deps
            .iter()
            .all(|dep| outcomes.get(dep).copied() == Some(TaskOutcome::Ok));
        if !deps_ok {
            let _ = crate::xtal_events::maybe_append_recovery_event(
                &project_root,
                &incident_id,
                "task_skipped_v1",
                &world,
                &source,
                Some(task.id.as_str()),
                json!({
                    "reason": "deps_not_ok",
                    "deps": task.deps,
                    "fn": task.fn_symbol,
                }),
            );

            if task.policy.criticality == "critical_v1" {
                overall_ok = false;
            }
            outcomes.insert(task.id.clone(), TaskOutcome::Skipped);
            continue;
        }

        let retries = if task.policy.on_failure == "retry_bounded_v1" {
            task.policy.retry_max.unwrap_or(0)
        } else {
            0
        };
        let max_attempts = retries.saturating_add(1);

        let mut attempt = 1u32;
        let mut success = false;
        let mut last_err: Option<ToolRunOutcome> = None;

        let _ = crate::xtal_events::maybe_append_recovery_event(
            &project_root,
            &incident_id,
            "task_started_v1",
            &world,
            &source,
            Some(task.id.as_str()),
            json!({
                "attempt": attempt,
                "attempt_max": max_attempts,
                "criticality": task.policy.criticality.as_str(),
                "on_failure": task.policy.on_failure.as_str(),
                "fn": task.fn_symbol.as_str(),
            }),
        );

        while attempt <= max_attempts {
            let sig = match load_task_fn_sig(&module_roots, world_id, &task.fn_symbol) {
                Ok(v) => v,
                Err(err) => {
                    let _ = crate::xtal_events::maybe_append_recovery_event(
                        &project_root,
                        &incident_id,
                        "fallback_used_v1",
                        &world,
                        &source,
                        Some(task.id.as_str()),
                        json!({
                            "reason": "cannot_load_task_fn",
                            "fn": task.fn_symbol,
                            "error": format!("{err:#}"),
                        }),
                    );
                    last_err = Some(ToolRunOutcome {
                        exit_code: 1,
                        stderr: format!("{err:#}").into_bytes(),
                    });
                    break;
                }
            };

            let wrapper_doc = match build_task_wrapper(task, &sig) {
                Ok(v) => v,
                Err(err) => {
                    let _ = crate::xtal_events::maybe_append_recovery_event(
                        &project_root,
                        &incident_id,
                        "fallback_used_v1",
                        &world,
                        &source,
                        Some(task.id.as_str()),
                        json!({
                            "reason": "unsupported_task_signature",
                            "fn": task.fn_symbol,
                            "error": format!("{err:#}"),
                        }),
                    );
                    last_err = Some(ToolRunOutcome {
                        exit_code: 1,
                        stderr: format!("{err:#}").into_bytes(),
                    });
                    break;
                }
            };

            let wrapper_path_abs = task_out_dir_abs.join("program.x07.json");
            let wrapper_bytes = serde_json::to_vec(&wrapper_doc).context("serialize wrapper")?;
            util::write_atomic(&wrapper_path_abs, &wrapper_bytes)
                .with_context(|| format!("write: {}", wrapper_path_abs.display()))?;

            let wrapper_rel = wrapper_path_abs
                .strip_prefix(&project_root)
                .unwrap_or(&wrapper_path_abs)
                .to_string_lossy()
                .replace('\\', "/");

            let mut argv: Vec<String> = vec![
                "run".to_string(),
                "--program".to_string(),
                wrapper_rel,
                "--world".to_string(),
                world.clone(),
            ];

            for root in &module_roots {
                argv.push("--module-root".to_string());
                argv.push(root.display().to_string());
            }

            argv.push("--input-b64".to_string());
            argv.push(input_b64.clone());

            if let Some(v) = runner_cfg.solve_fuel {
                argv.push("--solve-fuel".to_string());
                argv.push(v.to_string());
            }
            if let Some(v) = runner_cfg.max_memory_bytes {
                argv.push("--max-memory-bytes".to_string());
                argv.push(v.to_string());
            }
            if let Some(v) = runner_cfg.max_output_bytes {
                argv.push("--max-output-bytes".to_string());
                argv.push(v.to_string());
            }
            if let Some(v) = runner_cfg.cpu_time_limit_seconds {
                argv.push("--cpu-time-limit-seconds".to_string());
                argv.push(v.to_string());
            }
            if runner_cfg.debug_borrow_checks {
                argv.push("--debug-borrow-checks".to_string());
            }

            if let Some(v) = runner_cfg.fixture_fs_dir.as_deref() {
                argv.push("--fixture-fs-dir".to_string());
                argv.push(v.to_string());
            }
            if let Some(v) = runner_cfg.fixture_fs_root.as_deref() {
                argv.push("--fixture-fs-root".to_string());
                argv.push(v.to_string());
            }
            if let Some(v) = runner_cfg.fixture_fs_latency_index.as_deref() {
                argv.push("--fixture-fs-latency-index".to_string());
                argv.push(v.to_string());
            }
            if let Some(v) = runner_cfg.fixture_rr_dir.as_deref() {
                argv.push("--fixture-rr-dir".to_string());
                argv.push(v.to_string());
            }
            if let Some(v) = runner_cfg.fixture_kv_dir.as_deref() {
                argv.push("--fixture-kv-dir".to_string());
                argv.push(v.to_string());
            }
            if let Some(v) = runner_cfg.fixture_kv_seed.as_deref() {
                argv.push("--fixture-kv-seed".to_string());
                argv.push(v.to_string());
            }

            let out = run_self_command(&project_root, &argv)?;
            let stderr_path = task_out_dir_abs.join(format!("attempt-{:04}.stderr.txt", attempt));
            let _ = util::write_atomic(&stderr_path, &out.stderr);

            if out.exit_code == 0 {
                success = true;
                break;
            }

            last_err = Some(out);
            if task.policy.on_failure == "retry_bounded_v1" && attempt < max_attempts {
                let next_attempt = attempt.saturating_add(1);
                let _ = crate::xtal_events::maybe_append_recovery_event(
                    &project_root,
                    &incident_id,
                    "task_retried_v1",
                    &world,
                    &source,
                    Some(task.id.as_str()),
                    json!({
                        "attempt": next_attempt,
                        "retry_max": retries,
                        "fn": task.fn_symbol,
                    }),
                );
                attempt = next_attempt;
                continue;
            }

            break;
        }

        if success {
            let _ = crate::xtal_events::maybe_append_recovery_event(
                &project_root,
                &incident_id,
                "task_finished_v1",
                &world,
                &source,
                Some(task.id.as_str()),
                json!({
                    "outcome": "ok",
                    "attempts": attempt,
                    "fn": task.fn_symbol.as_str(),
                }),
            );
            outcomes.insert(task.id.clone(), TaskOutcome::Ok);
            continue;
        }

        let err = last_err.unwrap_or(ToolRunOutcome {
            exit_code: 1,
            stderr: Vec::new(),
        });

        let _ = crate::xtal_events::maybe_append_recovery_event(
            &project_root,
            &incident_id,
            "task_failed_v1",
            &world,
            &source,
            Some(task.id.as_str()),
            json!({
                "exit_code": err.exit_code,
                "stderr": stderr_summary(&err.stderr),
                "fn": task.fn_symbol,
            }),
        );

        match task.policy.on_failure.as_str() {
            "skip_v1" => {
                let _ = crate::xtal_events::maybe_append_recovery_event(
                    &project_root,
                    &incident_id,
                    "task_skipped_v1",
                    &world,
                    &source,
                    Some(task.id.as_str()),
                    json!({
                        "reason": "task_failed",
                        "exit_code": err.exit_code,
                        "fn": task.fn_symbol,
                    }),
                );
                outcomes.insert(task.id.clone(), TaskOutcome::Skipped);
            }
            "retry_bounded_v1" => {
                if task.policy.criticality == "optional_v1" {
                    let _ = crate::xtal_events::maybe_append_recovery_event(
                        &project_root,
                        &incident_id,
                        "task_skipped_v1",
                        &world,
                        &source,
                        Some(task.id.as_str()),
                        json!({
                            "reason": "retry_exhausted",
                            "retry_max": retries,
                            "exit_code": err.exit_code,
                            "fn": task.fn_symbol,
                        }),
                    );
                    outcomes.insert(task.id.clone(), TaskOutcome::Skipped);
                } else {
                    outcomes.insert(task.id.clone(), TaskOutcome::Failed);
                    overall_ok = false;
                }
            }
            "fail_fast_v1" => {
                outcomes.insert(task.id.clone(), TaskOutcome::Failed);
                overall_ok = false;
                fail_fast_triggered = true;
            }
            _ => {
                outcomes.insert(task.id.clone(), TaskOutcome::Failed);
                overall_ok = false;
            }
        }

        let finished_outcome = match outcomes.get(&task.id) {
            Some(TaskOutcome::Ok) => "ok",
            Some(TaskOutcome::Skipped) => "skipped",
            Some(TaskOutcome::Failed) => "failed",
            None => "failed",
        };
        let _ = crate::xtal_events::maybe_append_recovery_event(
            &project_root,
            &incident_id,
            "task_finished_v1",
            &world,
            &source,
            Some(task.id.as_str()),
            json!({
                "outcome": finished_outcome,
                "attempts": attempt,
                "exit_code": err.exit_code,
                "fn": task.fn_symbol.as_str(),
            }),
        );

        if task.policy.criticality == "critical_v1"
            && outcomes.get(&task.id) != Some(&TaskOutcome::Ok)
        {
            overall_ok = false;
        }

        if outcomes.get(&task.id) == Some(&TaskOutcome::Failed) {
            diags.push(diag_error(
                "EXTAL_TASKS_TASK_FAILED",
                diagnostics::Stage::Run,
                format!(
                    "task {:?} failed (exit_code={}): {}",
                    task.id,
                    err.exit_code,
                    stderr_summary(&err.stderr)
                ),
                None,
            ));
        } else {
            diags.push(diag_warning(
                "EXTAL_TASKS_TASK_SKIPPED",
                diagnostics::Stage::Run,
                format!(
                    "task {:?} was skipped after failure (exit_code={}): {}",
                    task.id,
                    err.exit_code,
                    stderr_summary(&err.stderr)
                ),
                None,
            ));
        }
    }

    report.ok = overall_ok;
    report = report.with_diagnostics(diags);

    write_tasks_diag_report(&project_root, &report)?;
    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn write_improve_diag_report(project_root: &Path, report: &diagnostics::Report) -> Result<()> {
    let report_path = project_root.join(DEFAULT_IMPROVE_DIAG_REPORT_PATH);
    std::fs::create_dir_all(report_path.parent().unwrap_or(project_root)).with_context(|| {
        format!(
            "mkdir: {}",
            report_path.parent().unwrap_or(project_root).display()
        )
    })?;

    let mut report_bytes = serde_json::to_vec(report).context("serialize improve diag report")?;
    if report_bytes.last() != Some(&b'\n') {
        report_bytes.push(b'\n');
    }
    util::write_atomic(&report_path, &report_bytes)
        .with_context(|| format!("write: {}", report_path.display()))?;
    Ok(())
}

fn write_improve_summary(project_root: &Path, out_dir: &Path, summary: &Value) -> Result<()> {
    let out_dir_abs = project_root.join(out_dir);
    std::fs::create_dir_all(&out_dir_abs)
        .with_context(|| format!("mkdir: {}", out_dir_abs.display()))?;

    let summary_path = out_dir_abs.join("summary.json");
    let bytes = report_common::canonical_pretty_json_bytes(summary)
        .context("serialize xtal improve summary")?;
    util::write_atomic(&summary_path, &bytes)
        .with_context(|| format!("write: {}", summary_path.display()))?;
    Ok(())
}

fn contract_ref_from_repro_doc(repro_doc: &Value) -> Option<Value> {
    let contract = repro_doc.pointer("/contract").and_then(Value::as_object)?;
    let witness_count = contract
        .get("witness")
        .and_then(Value::as_array)
        .map(|w| w.len() as u64)
        .unwrap_or(0);
    Some(json!({
        "fn": contract.get("fn").cloned().unwrap_or(Value::Null),
        "contract_kind": contract.get("contract_kind").cloned().unwrap_or(Value::Null),
        "clause_id": contract.get("clause_id").cloned().unwrap_or(Value::Null),
        "clause_ptr": contract.get("clause_ptr").cloned().unwrap_or(Value::Null),
        "witness_count": witness_count,
    }))
}

fn resolve_manifest_path_for_repro(
    project_root: &Path,
    repro_doc: &Value,
) -> std::result::Result<Option<PathBuf>, String> {
    let raw = repro_doc
        .pointer("/source/tests_manifest_path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.starts_with("\\\\") {
        return Err(format!("unsupported tests_manifest_path: {raw:?}"));
    }
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return Ok(Some(p));
    }
    if !util::is_safe_rel_path(raw) {
        return Err(format!("unsafe tests_manifest_path: {raw:?}"));
    }
    Ok(Some(project_root.join(p)))
}

fn test_entry_from_manifest(manifest_path: &Path, test_id: &str) -> Result<Option<Value>> {
    let bytes = std::fs::read(manifest_path)
        .with_context(|| format!("read tests manifest: {}", manifest_path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse tests manifest JSON: {}", manifest_path.display()))?;
    let Some(tests) = doc.get("tests").and_then(Value::as_array) else {
        return Ok(None);
    };
    for t in tests {
        let id = t.get("id").and_then(Value::as_str).unwrap_or("");
        if id == test_id {
            return Ok(Some(t.clone()));
        }
    }
    Ok(None)
}

fn write_shadow_tests_manifest(
    project_root: &Path,
    out_path: &Path,
    repro_doc: &Value,
    input_bytes_b64: &str,
) -> Result<()> {
    let world = repro_doc
        .get("world")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let test_id = repro_doc
        .pointer("/source/test_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let test_entry = repro_doc
        .pointer("/source/test_entry")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let mut entry_obj: Value = if test_id.is_empty() {
        json!({ "id": "xtal/improve/incident", "entry": test_entry, "world": world })
    } else {
        let from_manifest = match resolve_manifest_path_for_repro(project_root, repro_doc) {
            Ok(Some(manifest_path)) if manifest_path.is_file() => {
                test_entry_from_manifest(&manifest_path, &test_id)?
            }
            Ok(_) => None,
            Err(_) => None,
        };
        from_manifest
            .unwrap_or_else(|| json!({ "id": test_id, "entry": test_entry, "world": world }))
    };

    if let Some(obj) = entry_obj.as_object_mut() {
        obj.insert(
            "input_b64".to_string(),
            Value::String(input_bytes_b64.to_string()),
        );
        obj.remove("input_path");
        obj.remove("pbt");

        if obj.get("fixture_root").is_none()
            && WorldId::parse(&world).is_some_and(|w| w == WorldId::SolveRr)
        {
            let raw = repro_doc
                .pointer("/runner/fixture_rr_dir")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if raw.is_empty() {
                anyhow::bail!("solve-rr incident is missing runner.fixture_rr_dir in repro.json");
            }
            if raw.starts_with("\\\\") {
                anyhow::bail!("unsupported runner.fixture_rr_dir in repro.json: {raw:?}");
            }
            let rr_src_abs = {
                let p = PathBuf::from(raw);
                if p.is_absolute() {
                    p
                } else if util::is_safe_rel_path(raw) {
                    project_root.join(p)
                } else {
                    anyhow::bail!("unsupported runner.fixture_rr_dir in repro.json: {raw:?}");
                }
            };
            let rr_src_abs_canon = rr_src_abs.canonicalize().unwrap_or(rr_src_abs.clone());
            let project_root_canon = project_root
                .canonicalize()
                .unwrap_or_else(|_| project_root.to_path_buf());
            if !rr_src_abs_canon.starts_with(&project_root_canon) {
                anyhow::bail!("unsupported runner.fixture_rr_dir outside project root: {raw:?}");
            }
            if !rr_src_abs.is_dir() {
                anyhow::bail!(
                    "solve-rr rr fixture dir does not exist: {}",
                    rr_src_abs.display()
                );
            }

            let rr_dst_abs = out_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("rr");
            let _ = std::fs::remove_dir_all(&rr_dst_abs);
            x07_vm::copy_dir_recursive(&rr_src_abs, &rr_dst_abs).with_context(|| {
                format!(
                    "copy rr fixture dir {} -> {}",
                    rr_src_abs.display(),
                    rr_dst_abs.display()
                )
            })?;

            obj.insert("fixture_root".to_string(), Value::String("rr".to_string()));
        }
    }

    let doc = json!({
        "schema_version": TESTS_MANIFEST_SCHEMA_VERSION,
        "tests": [entry_obj],
    });

    let mut bytes = serde_json::to_vec_pretty(&doc).context("serialize shadow tests manifest")?;
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }

    util::write_atomic(out_path, &bytes)
        .with_context(|| format!("write: {}", out_path.display()))?;
    Ok(())
}

fn reduction_predicate_x07test(
    project_root: &Path,
    scratch_dir: &Path,
    repro_doc: &Value,
    input_bytes: &[u8],
    expected_clause_id: &str,
    attempt_no: u64,
) -> Result<bool> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let input_b64 = b64.encode(input_bytes);

    let attempt_dir = scratch_dir.join(format!("attempt-{attempt_no:04}"));
    let manifest_path = attempt_dir.join("tests.json");
    let artifact_dir = attempt_dir.join("_artifacts");
    let report_path = attempt_dir.join("tests.report.json");
    let violations_dir = attempt_dir.join("violations");
    let events_dir = attempt_dir.join("events");

    std::fs::create_dir_all(&attempt_dir)
        .with_context(|| format!("mkdir: {}", attempt_dir.display()))?;

    write_shadow_tests_manifest(project_root, &manifest_path, repro_doc, &input_b64)?;

    let exe = std::env::current_exe().context("resolve x07 executable")?;
    let argv: Vec<String> = vec![
        "test".to_string(),
        "--all".to_string(),
        "--manifest".to_string(),
        manifest_path.display().to_string(),
        "--artifact-dir".to_string(),
        artifact_dir.display().to_string(),
        "--report-out".to_string(),
        report_path.display().to_string(),
        "--quiet-json".to_string(),
        "--json=canon".to_string(),
    ];
    let _output = Command::new(exe)
        .current_dir(project_root)
        .env("X07_TOOL_API_CHILD", "1")
        .env(
            crate::xtal_violation::ENV_X07_XTAL_VIOLATIONS_DIR,
            &violations_dir,
        )
        .env(crate::xtal_events::ENV_X07_XTAL_EVENTS_DIR, &events_dir)
        .args(&argv)
        .output()
        .with_context(|| format!("run x07 test in {}", project_root.display()))?;

    if !violations_dir.is_dir() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(&violations_dir)
        .with_context(|| format!("read dir: {}", violations_dir.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let violation_json = entry.path().join("violation.json");
        if !violation_json.is_file() {
            continue;
        }
        let bytes = std::fs::read(&violation_json)
            .with_context(|| format!("read: {}", violation_json.display()))?;
        let doc: Value = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse JSON: {}", violation_json.display()))?;
        let clause_id = doc.get("clause_id").and_then(Value::as_str).unwrap_or("");
        if clause_id == expected_clause_id {
            return Ok(true);
        }
    }

    Ok(false)
}

fn ddmin_reduce_bytes<F>(original: &[u8], max_runs: u64, mut predicate: F) -> Result<(Vec<u8>, u64)>
where
    F: FnMut(&[u8], u64) -> Result<bool>,
{
    if original.len() <= 1 {
        return Ok((original.to_vec(), 0));
    }

    let mut runs = 0u64;
    let mut current: Vec<u8> = original.to_vec();
    let mut n = 2usize;

    while current.len() >= 2 && runs < max_runs {
        let len = current.len();
        let chunk = len.div_ceil(n);
        let mut reduced = None;
        for i in 0..n {
            if runs >= max_runs {
                break;
            }
            let start = i * chunk;
            if start >= len {
                break;
            }
            let end = ((i + 1) * chunk).min(len);
            if start == 0 && end == len {
                continue;
            }
            let mut candidate = Vec::with_capacity(len - (end - start));
            candidate.extend_from_slice(&current[..start]);
            candidate.extend_from_slice(&current[end..]);

            runs += 1;
            if predicate(&candidate, runs)? {
                reduced = Some(candidate);
                break;
            }
        }

        if let Some(next) = reduced {
            current = next;
            n = n.saturating_sub(1).max(2);
        } else {
            if n >= len {
                break;
            }
            n = (n * 2).min(len);
        }
    }

    Ok((current, runs))
}

fn cmd_xtal_improve(
    machine: &crate::reporting::MachineArgs,
    args: XtalImproveArgs,
) -> Result<std::process::ExitCode> {
    let project_root =
        resolve_project_root(args.project.as_deref(), None).context("resolve project root")?;

    let cwd = std::env::current_dir().context("cwd")?;
    let input_abs = if args.input.is_absolute() {
        args.input.clone()
    } else {
        cwd.join(&args.input)
    };

    let out_root_abs = project_root.join(&args.out_dir);
    std::fs::create_dir_all(&out_root_abs)
        .with_context(|| format!("mkdir: {}", out_root_abs.display()))?;

    let mut diags: Vec<diagnostics::Diagnostic> = Vec::new();
    let mut report = diagnostics::Report::ok();

    report.meta.insert(
        "improve_input_path".to_string(),
        Value::String(
            input_abs
                .strip_prefix(&project_root)
                .unwrap_or(&input_abs)
                .to_string_lossy()
                .replace('\\', "/"),
        ),
    );

    let mut incidents: Vec<LoadedIngestInputs> = Vec::new();
    let mut input_kind = "unknown".to_string();

    if input_abs.is_dir() {
        let is_single = input_abs.join("violation.json").is_file()
            || input_abs.join("repro.json").is_file()
            || input_abs.join("events.jsonl").is_file();
        if is_single {
            match load_ingest_inputs(&project_root, &input_abs) {
                Ok(loaded) => {
                    input_kind = loaded.input_kind.clone();
                    incidents.push(loaded);
                }
                Err(err) => {
                    diags.push(diag_error(err.code, err.stage, err.message, None));
                    report.ok = false;
                }
            }
        } else {
            input_kind = "dir".to_string();
            let mut children: Vec<PathBuf> = Vec::new();
            for entry in std::fs::read_dir(&input_abs)
                .with_context(|| format!("read dir: {}", input_abs.display()))?
            {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    children.push(entry.path());
                }
            }
            children.sort();
            for child in children {
                let is_bundle = child.join("violation.json").is_file()
                    || child.join("repro.json").is_file()
                    || child.join("events.jsonl").is_file();
                if !is_bundle {
                    continue;
                }
                match load_ingest_inputs(&project_root, &child) {
                    Ok(loaded) => incidents.push(loaded),
                    Err(err) => {
                        diags.push(diag_warning(err.code, err.stage, err.message, None));
                    }
                }
            }
        }
    } else {
        match load_ingest_inputs(&project_root, &input_abs) {
            Ok(loaded) => {
                input_kind = loaded.input_kind.clone();
                incidents.push(loaded);
            }
            Err(err) => {
                diags.push(diag_error(err.code, err.stage, err.message, None));
                report.ok = false;
            }
        }
    }

    if incidents.is_empty() {
        diags.push(diag_error(
            "EXTAL_IMPROVE_NO_INCIDENTS",
            diagnostics::Stage::Parse,
            "no ingestable incidents were found under the provided --input",
            None,
        ));
        report.ok = false;
        report = report.with_diagnostics(diags);
        write_improve_diag_report(&project_root, &report)?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    incidents.sort_by(|a, b| a.incident_id.cmp(&b.incident_id));
    let target_idx = 0usize;

    let target_id = incidents[target_idx].incident_id.clone();
    let target_repro_bytes = incidents[target_idx].repro_bytes.clone();
    let target_repro_doc: Value =
        serde_json::from_slice(&target_repro_bytes).context("parse repro JSON")?;

    let Some(target_contract_ref) = contract_ref_from_repro_doc(&target_repro_doc) else {
        diags.push(diag_error(
            "EXTAL_IMPROVE_FAILED",
            diagnostics::Stage::Parse,
            "incident repro is missing contract metadata",
            None,
        ));
        report.ok = false;
        report = report.with_diagnostics(diags);
        write_improve_diag_report(&project_root, &report)?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    };

    let target_source_ref = target_repro_doc
        .get("source")
        .cloned()
        .unwrap_or_else(|| json!({ "mode": "unknown" }));

    let expected_clause_id = target_contract_ref
        .get("clause_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let b64 = base64::engine::general_purpose::STANDARD;
    let original_input_b64 = target_repro_doc
        .get("input_bytes_b64")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let original_input_bytes = b64
        .decode(original_input_b64.as_bytes())
        .context("decode repro input_bytes_b64")?;

    let incident_out_dir_abs = out_root_abs.join(&target_id);
    std::fs::create_dir_all(&incident_out_dir_abs)
        .with_context(|| format!("mkdir: {}", incident_out_dir_abs.display()))?;

    let mut effective_input_bytes = original_input_bytes.clone();
    let mut reduction_status = "skipped".to_string();
    let mut repro_min_path_rel: Option<String> = None;
    let mut reduction_report_path_rel: Option<String> = None;

    if args.reduce_repro {
        let world = target_repro_doc
            .get("world")
            .and_then(Value::as_str)
            .unwrap_or("");
        let mode = target_repro_doc
            .pointer("/source/mode")
            .and_then(Value::as_str)
            .unwrap_or("");

        let scratch_dir = incident_out_dir_abs.join("reduction");
        std::fs::create_dir_all(&scratch_dir)
            .with_context(|| format!("mkdir: {}", scratch_dir.display()))?;

        if mode == "x07test" && world == "solve-pure" && !expected_clause_id.is_empty() {
            let (min_bytes, runs) = ddmin_reduce_bytes(&original_input_bytes, 64, |cand, n| {
                reduction_predicate_x07test(
                    &project_root,
                    &scratch_dir,
                    &target_repro_doc,
                    cand,
                    &expected_clause_id,
                    n,
                )
            })?;

            effective_input_bytes = min_bytes;
            reduction_status = "ok".to_string();

            let mut repro_min_doc = target_repro_doc.clone();
            if let Some(obj) = repro_min_doc.as_object_mut() {
                obj.insert(
                    "input_bytes_b64".to_string(),
                    Value::String(b64.encode(&effective_input_bytes)),
                );
            }
            let schema_diags = report_common::validate_schema(
                CONTRACT_REPRO_SCHEMA_BYTES,
                "spec/x07.contract.repro@0.1.0.schema.json",
                &repro_min_doc,
            )?;
            if !schema_diags.is_empty() {
                anyhow::bail!(
                    "internal error: reduced repro is not schema-valid: {}",
                    schema_diags[0].message
                );
            }

            let repro_min_path_abs = incident_out_dir_abs.join("repro.min.json");
            let repro_min_bytes = report_common::canonical_pretty_json_bytes(&repro_min_doc)?;
            util::write_atomic(&repro_min_path_abs, &repro_min_bytes)
                .with_context(|| format!("write: {}", repro_min_path_abs.display()))?;
            repro_min_path_rel = Some(
                repro_min_path_abs
                    .strip_prefix(&project_root)
                    .unwrap_or(&repro_min_path_abs)
                    .to_string_lossy()
                    .replace('\\', "/"),
            );

            let report_doc = json!({
                "schema_version": "x07.xtal.repro_reduction@0.1.0",
                "ok": true,
                "predicate_runs": runs,
                "original_bytes_len": original_input_bytes.len(),
                "min_bytes_len": effective_input_bytes.len(),
            });
            let report_bytes = report_common::canonical_pretty_json_bytes(&report_doc)?;
            let reduction_report_abs = incident_out_dir_abs.join("reduction.report.json");
            util::write_atomic(&reduction_report_abs, &report_bytes)
                .with_context(|| format!("write: {}", reduction_report_abs.display()))?;
            reduction_report_path_rel = Some(
                reduction_report_abs
                    .strip_prefix(&project_root)
                    .unwrap_or(&reduction_report_abs)
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        } else {
            reduction_status = "unsupported".to_string();
        }
    }

    let effective_input_b64 = b64.encode(&effective_input_bytes);
    let shadow_manifest_abs = incident_out_dir_abs.join("tests.shadow.json");
    write_shadow_tests_manifest(
        &project_root,
        &shadow_manifest_abs,
        &target_repro_doc,
        &effective_input_b64,
    )?;
    let shadow_manifest_rel = shadow_manifest_abs
        .strip_prefix(&project_root)
        .unwrap_or(&shadow_manifest_abs)
        .to_string_lossy()
        .replace('\\', "/");

    let verify_manifest_rel = PathBuf::from(shadow_manifest_rel.clone());

    let build_verify_args = || XtalVerifyArgs {
        project: Some(project_root.join("x07.json")),
        spec_dir: PathBuf::from(DEFAULT_SPEC_DIR),
        gen_index: None,
        gen_dir: PathBuf::from(DEFAULT_GEN_DIR),
        manifest: verify_manifest_rel.clone(),
        proof_policy: ProofPolicy::Balanced,
        allow_os_world: false,
        z3_timeout_seconds: None,
        z3_memory_mb: None,
        unwind: None,
        max_bytes_len: None,
        input_len_bytes: None,
    };

    let (verify_code, _) = capture_report_json("xtal_improve_verify", |m| {
        cmd_xtal_verify(m, build_verify_args())
    })?;
    let mut verify_status = if verify_code == std::process::ExitCode::SUCCESS {
        "ok".to_string()
    } else {
        "failed".to_string()
    };

    let (repair_code, _) = capture_report_json("xtal_improve_repair", |m| {
        cmd_xtal_repair(
            m,
            XtalRepairArgs {
                project: Some(project_root.join("x07.json")),
                write: false,
                max_rounds: 3,
                max_candidates: 64,
                semantic_max_depth: 4,
                semantic_ops: SemanticOpsPreset::Safe,
                entry: None,
                stubs_only: true,
                allow_edit_non_stubs: false,
                semantic_only: false,
                quickfix_only: false,
                suggest_spec_patch: true,
            },
        )
    })?;

    let patchset_path_abs = project_root.join(DEFAULT_REPAIR_PATCHSET_PATH);
    let mut patchset: Option<PatchSet> = None;
    if patchset_path_abs.is_file() {
        let bytes = std::fs::read(&patchset_path_abs)
            .with_context(|| format!("read: {}", patchset_path_abs.display()))?;
        if let Ok(v) = serde_json::from_slice::<PatchSet>(&bytes) {
            patchset = Some(v);
        }
    }

    let repair_status = if patchset.is_some() || repair_code == std::process::ExitCode::SUCCESS {
        "ok".to_string()
    } else {
        "failed".to_string()
    };

    let spec_change_detected = patchset.as_ref().is_some_and(|ps| {
        ps.patches
            .iter()
            .any(|p| normalize_rel_path(&p.path).is_some_and(|path| path.starts_with("spec")))
    });

    let mut applied = false;
    let mut certify_status = "skipped".to_string();

    if args.write {
        if let Some(patchset) = patchset.as_ref() {
            let xtal_manifest_path = project_root.join("arch").join("xtal").join("xtal.json");
            let xtal_manifest = if xtal_manifest_path.is_file() {
                load_xtal_manifest(
                    &xtal_manifest_path,
                    &mut diags,
                    "EXTAL_IMPROVE_MANIFEST_PARSE_FAILED",
                )?
            } else {
                None
            };

            if let Some(xtal_manifest) = xtal_manifest.as_ref() {
                if spec_change_detected && !args.allow_spec_change {
                    diags.push(diag_error(
                        "EXTAL_IMPROVE_SPEC_CHANGE_REQUIRES_FLAG",
                        diagnostics::Stage::Run,
                        "patchset changes spec/** (pass --allow-spec-change to permit applying it)",
                        None,
                    ));
                    report.ok = false;
                } else {
                    let disallowed = disallowed_patch_paths_from_manifest(patchset, xtal_manifest);
                    if !disallowed.is_empty() {
                        diags.push(diag_error(
                            "EXTAL_IMPROVE_DISALLOWED_PATCH_PATHS",
                            diagnostics::Stage::Run,
                            format!(
                                "patchset touches disallowed paths: {}",
                                disallowed.join(", ")
                            ),
                            None,
                        ));
                        report.ok = false;
                    } else {
                        let apply_out = run_self_command(
                            &project_root,
                            &[
                                "patch".to_string(),
                                "apply".to_string(),
                                "--in".to_string(),
                                DEFAULT_REPAIR_PATCHSET_PATH.to_string(),
                                "--repo-root".to_string(),
                                ".".to_string(),
                                "--write".to_string(),
                                "--quiet-json".to_string(),
                            ],
                        )?;
                        if apply_out.exit_code != 0 {
                            diags.push(diag_error(
                                "EXTAL_IMPROVE_PATCH_APPLY_FAILED",
                                diagnostics::Stage::Run,
                                format!(
                                    "failed to apply patchset (exit_code={}): {}",
                                    apply_out.exit_code,
                                    stderr_summary(&apply_out.stderr)
                                ),
                                None,
                            ));
                            report.ok = false;
                        } else {
                            applied = true;

                            let (verify2_code, _) =
                                capture_report_json("xtal_improve_verify_post_apply", |m| {
                                    cmd_xtal_verify(m, build_verify_args())
                                })?;
                            verify_status = if verify2_code == std::process::ExitCode::SUCCESS {
                                "ok".to_string()
                            } else {
                                "failed".to_string()
                            };

                            if verify2_code == std::process::ExitCode::SUCCESS {
                                if args.run_tasks {
                                    report.meta.insert(
                                        "tasks_diag_path".to_string(),
                                        Value::String(DEFAULT_TASKS_DIAG_REPORT_PATH.to_string()),
                                    );

                                    let tasks_args = XtalTasksRunArgs {
                                        project: Some(project_root.join("x07.json")),
                                        input: incidents[target_idx].input_file_abs.clone(),
                                        index: PathBuf::from("arch/tasks/index.x07tasks.json"),
                                        out_dir: PathBuf::from(DEFAULT_TASKS_DIR),
                                    };

                                    match capture_report_json("xtal_improve_tasks", |m| {
                                        cmd_xtal_tasks_run(m, tasks_args)
                                    }) {
                                        Ok((code, v)) => {
                                            diags.extend(
                                                crate::tool_api::extract_diagnostics(Some(&v))
                                                    .unwrap_or_default(),
                                            );
                                            if code != std::process::ExitCode::SUCCESS {
                                                report.ok = false;
                                                diags.push(diag_error(
                                                    "EXTAL_IMPROVE_TASKS_FAILED",
                                                    diagnostics::Stage::Run,
                                                    "recovery tasks failed; see diagnostics",
                                                    None,
                                                ));
                                            }
                                        }
                                        Err(err) => {
                                            report.ok = false;
                                            diags.push(diag_error(
                                                "EXTAL_IMPROVE_TASKS_FAILED",
                                                diagnostics::Stage::Run,
                                                format!("recovery tasks failed: {err:#}"),
                                                None,
                                            ));
                                        }
                                    }
                                }

                                if args.certify {
                                    let (certify_code, _) =
                                        capture_report_json("xtal_improve_certify", |m| {
                                            cmd_xtal_certify(
                                                m,
                                                XtalCertifyArgs {
                                                    project: Some(project_root.join("x07.json")),
                                                    spec_dir: PathBuf::from(DEFAULT_SPEC_DIR),
                                                    out_dir: PathBuf::from(DEFAULT_CERT_DIR),
                                                    entry: None,
                                                    all: true,
                                                    baseline: args.baseline.clone(),
                                                    no_prechecks: false,
                                                    no_fail_fast: false,
                                                    unwind: None,
                                                    max_bytes_len: None,
                                                    input_len_bytes: None,
                                                    z3_timeout_seconds: None,
                                                    z3_memory_mb: None,
                                                },
                                            )
                                        })?;

                                    certify_status =
                                        if certify_code == std::process::ExitCode::SUCCESS {
                                            "ok".to_string()
                                        } else {
                                            "failed".to_string()
                                        };
                                }
                            }
                        }
                    }
                }
            } else {
                diags.push(diag_error(
                    "EXTAL_IMPROVE_WRITE_REQUIRES_MANIFEST",
                    diagnostics::Stage::Parse,
                    "--write requires arch/xtal/xtal.json so edit boundaries are explicit",
                    None,
                ));
                report.ok = false;
            }
        } else {
            diags.push(diag_warning(
                "WXTAL_IMPROVE_NO_PATCH",
                diagnostics::Stage::Run,
                "no patchset was produced by `x07 xtal repair`; nothing to apply",
                None,
            ));
        }
    }

    let mut incident_refs: Vec<Value> = Vec::new();
    for inc in &incidents {
        let repro_doc: Value = serde_json::from_slice(&inc.repro_bytes)
            .with_context(|| format!("parse repro JSON for incident {}", inc.incident_id))?;
        let contract_ref = match contract_ref_from_repro_doc(&repro_doc) {
            Some(v) => v,
            None => continue,
        };
        let clause_id = inc
            .violation_doc
            .get("clause_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let world = inc
            .violation_doc
            .get("world")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        incident_refs.push(json!({
            "id": inc.incident_id,
            "clause_id": clause_id,
            "world": world,
            "contract": contract_ref,
        }));
    }

    let mut files: Vec<Value> = Vec::new();
    files.push(file_digest_rel_value(&project_root, &shadow_manifest_abs)?);
    if let Some(p) = repro_min_path_rel.as_deref() {
        files.push(file_digest_rel_value(&project_root, &project_root.join(p))?);
    }
    if let Some(p) = reduction_report_path_rel.as_deref() {
        files.push(file_digest_rel_value(&project_root, &project_root.join(p))?);
    }
    if patchset_path_abs.is_file() {
        files.push(file_digest_rel_value(&project_root, &patchset_path_abs)?);
    }
    if project_root.join(DEFAULT_VERIFY_SUMMARY_PATH).is_file() {
        files.push(file_digest_rel_value(
            &project_root,
            &project_root.join(DEFAULT_VERIFY_SUMMARY_PATH),
        )?);
    }
    if project_root.join(DEFAULT_REPAIR_SUMMARY_PATH).is_file() {
        files.push(file_digest_rel_value(
            &project_root,
            &project_root.join(DEFAULT_REPAIR_SUMMARY_PATH),
        )?);
    }
    if certify_status != "skipped" && project_root.join("target/xtal/cert/summary.json").is_file() {
        files.push(file_digest_rel_value(
            &project_root,
            &project_root.join("target/xtal/cert/summary.json"),
        )?);
    }
    files.sort_by(|a, b| {
        let ap = a.get("path").and_then(Value::as_str).unwrap_or("");
        let bp = b.get("path").and_then(Value::as_str).unwrap_or("");
        ap.cmp(bp)
    });
    files.dedup_by(|a, b| {
        a.get("path").and_then(Value::as_str) == b.get("path").and_then(Value::as_str)
    });

    report = report.with_diagnostics(diags);
    if args.write {
        if verify_status != "ok" || certify_status == "failed" {
            report.ok = false;
        }
        if patchset.is_some() && !applied {
            report.ok = false;
        }
    } else if verify_status != "ok" || repair_status != "ok" {
        report.ok = false;
    }

    let mut summary = json!({
        "schema_version": IMPROVE_SUMMARY_SCHEMA_VERSION,
        "generated_at": "2000-01-01T00:00:00Z",
        "ok": report.ok,
        "input": {
            "path": input_abs.strip_prefix(&project_root).unwrap_or(&input_abs).to_string_lossy().replace('\\', "/"),
            "kind": input_kind,
        },
        "incidents": incident_refs,
        "triage": {
            "source": target_source_ref,
            "contract": target_contract_ref,
        },
        "reduction": {
            "status": reduction_status,
        },
        "verify": {
            "status": verify_status,
            "diag_path": DEFAULT_VERIFY_DIAG_REPORT_PATH,
            "summary_path": DEFAULT_VERIFY_SUMMARY_PATH,
        },
        "repair": {
            "status": repair_status,
            "diag_path": DEFAULT_REPAIR_DIAG_REPORT_PATH,
            "summary_path": DEFAULT_REPAIR_SUMMARY_PATH,
            "patchset_path": DEFAULT_REPAIR_PATCHSET_PATH,
        },
        "certify": {
            "status": certify_status,
        },
        "governance": {
            "write_requested": args.write,
            "spec_change_detected": spec_change_detected,
            "spec_change_allowed": args.allow_spec_change,
            "applied": applied,
        },
        "files": files,
    });

    if let Some(obj) = summary.get_mut("reduction").and_then(Value::as_object_mut) {
        if let Some(path) = repro_min_path_rel.clone() {
            obj.insert("repro_min_path".to_string(), Value::String(path));
        }
        if let Some(path) = reduction_report_path_rel.clone() {
            obj.insert("report_path".to_string(), Value::String(path));
        }
    }

    if certify_status != "skipped" {
        if let Some(obj) = summary.get_mut("certify").and_then(Value::as_object_mut) {
            obj.insert(
                "diag_path".to_string(),
                Value::String(DEFAULT_CERT_DIAG_REPORT_PATH.to_string()),
            );
            obj.insert(
                "summary_path".to_string(),
                Value::String("target/xtal/cert/summary.json".to_string()),
            );
            obj.insert(
                "bundle_path".to_string(),
                Value::String("target/xtal/cert/bundle.json".to_string()),
            );
        }
    }

    let schema_diags = report_common::validate_schema(
        IMPROVE_SUMMARY_SCHEMA_BYTES,
        "spec/x07.xtal.improve_summary@0.1.0.schema.json",
        &summary,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: xtal improve summary JSON is not schema-valid: {}",
            schema_diags[0].message
        );
    }

    write_improve_summary(&project_root, &args.out_dir, &summary)?;

    report.meta.insert(
        "improve_out_dir".to_string(),
        Value::String(args.out_dir.display().to_string()),
    );
    report.meta.insert(
        "improve_summary_path".to_string(),
        Value::String(
            project_root
                .join(&args.out_dir)
                .join("summary.json")
                .strip_prefix(&project_root)
                .unwrap_or(&project_root.join(&args.out_dir).join("summary.json"))
                .to_string_lossy()
                .replace('\\', "/"),
        ),
    );
    report
        .meta
        .insert("improve_incident_id".to_string(), Value::String(target_id));
    report.meta.insert(
        "shadow_manifest".to_string(),
        Value::String(shadow_manifest_rel),
    );

    write_improve_diag_report(&project_root, &report)?;
    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn safe_entry_dir_component(entry: &str) -> String {
    let mut out = String::with_capacity(entry.len());
    for ch in entry.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "entry".to_string()
    } else {
        out
    }
}

fn resolve_certify_entries(
    manifest: &XtalManifest,
    args: &XtalCertifyArgs,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Option<Vec<String>> {
    let mut entries: Vec<String> = manifest
        .entrypoints
        .iter()
        .map(|e| e.name.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    entries.sort();
    entries.dedup();

    if entries.is_empty() {
        diags.push(diag_error(
            "EXTAL_CERTIFY_NO_ENTRYPOINTS",
            diagnostics::Stage::Parse,
            "no entrypoints configured in arch/xtal/xtal.json",
            None,
        ));
        return None;
    }

    if let Some(entry) = args.entry.as_deref() {
        let entry = entry.trim();
        if entry.is_empty() || x07c::validate::validate_symbol(entry).is_err() {
            diags.push(diag_error(
                "EXTAL_CERTIFY_ENTRY_REQUIRED",
                diagnostics::Stage::Parse,
                "invalid --entry (expected a fully-qualified symbol)",
                None,
            ));
            return None;
        }
        return Some(vec![entry.to_string()]);
    }

    if args.all {
        return Some(entries);
    }

    if entries.len() == 1 {
        return Some(entries);
    }

    diags.push(diag_error(
        "EXTAL_CERTIFY_ENTRY_REQUIRED",
        diagnostics::Stage::Parse,
        "multiple entrypoints configured; pass --entry or --all",
        None,
    ));
    None
}

fn load_xtal_manifest(
    path: &Path,
    diags: &mut Vec<diagnostics::Diagnostic>,
    parse_code: &str,
) -> Result<Option<XtalManifest>> {
    let doc = match report_common::read_json_file(path) {
        Ok(v) => v,
        Err(err) => {
            diags.push(diag_error(
                parse_code,
                diagnostics::Stage::Parse,
                format!("failed to parse {}: {err:#}", path.display()),
                None,
            ));
            return Ok(None);
        }
    };

    let schema_diags = report_common::validate_schema(
        XTAL_MANIFEST_SCHEMA_BYTES,
        "spec/x07.xtal.manifest@0.1.0.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        diags.push(diag_error(
            parse_code,
            diagnostics::Stage::Parse,
            format!(
                "failed to parse {}: {}",
                path.display(),
                schema_diags[0].message
            ),
            None,
        ));
        return Ok(None);
    }

    let parsed: XtalManifest = match serde_json::from_value(doc) {
        Ok(v) => v,
        Err(err) => {
            diags.push(diag_error(
                parse_code,
                diagnostics::Stage::Parse,
                format!("failed to parse {}: {err}", path.display()),
                None,
            ));
            return Ok(None);
        }
    };
    if parsed.schema_version.trim() != XTAL_MANIFEST_SCHEMA_VERSION {
        diags.push(diag_error(
            parse_code,
            diagnostics::Stage::Parse,
            format!(
                "failed to parse {}: expected schema_version={XTAL_MANIFEST_SCHEMA_VERSION:?}",
                path.display()
            ),
            None,
        ));
        return Ok(None);
    }

    Ok(Some(parsed))
}

#[allow(clippy::too_many_arguments)]
fn build_certify_summary_value(
    project_root: &Path,
    args: &XtalCertifyArgs,
    xtal_manifest_path: Option<&Path>,
    trust_profile_path: Option<&Path>,
    baseline_path: Option<&Path>,
    entries: &[String],
    review_gates: &[String],
    results: &[Value],
    ok: bool,
) -> Result<Value> {
    let manifest_path = project_root.join("x07.json");
    let manifest_digest = crate::reporting::file_digest(&manifest_path)
        .with_context(|| format!("digest: {}", manifest_path.display()))?
        .sha256;

    let xtal_manifest_rel = xtal_manifest_path.and_then(|p| {
        p.strip_prefix(project_root)
            .ok()
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .or_else(|| Some(p.display().to_string()))
    });
    let xtal_manifest_sha256 =
        xtal_manifest_path.and_then(|p| crate::reporting::file_digest(p).ok().map(|d| d.sha256));

    let trust_profile_rel = trust_profile_path.and_then(|p| {
        p.strip_prefix(project_root)
            .ok()
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .or_else(|| Some(p.display().to_string()))
    });
    let trust_profile_sha256 =
        trust_profile_path.and_then(|p| crate::reporting::file_digest(p).ok().map(|d| d.sha256));

    let baseline_abs =
        baseline_path.map(|p| util::resolve_existing_path_upwards_from(project_root, p));
    let baseline_rel = baseline_abs.as_ref().map(|p| {
        p.strip_prefix(project_root)
            .unwrap_or(p.as_path())
            .to_string_lossy()
            .replace('\\', "/")
    });
    let baseline_sha256 = baseline_abs
        .as_ref()
        .and_then(|p| snapshot_digest_hex(p).ok().flatten());

    let out_dir_rel = args.out_dir.to_string_lossy().replace('\\', "/");

    let summary = json!({
        "schema_version": CERTIFY_SUMMARY_SCHEMA_VERSION,
        "project": {
            "root": ".",
            "manifest_path": "x07.json",
            "manifest_sha256": manifest_digest,
            "xtal_manifest_path": xtal_manifest_rel,
            "xtal_manifest_sha256": xtal_manifest_sha256,
            "trust_profile_path": trust_profile_rel,
            "trust_profile_sha256": trust_profile_sha256,
            "baseline_path": baseline_rel,
            "baseline_sha256": baseline_sha256,
        },
        "settings": {
            "out_dir": out_dir_rel,
            "entries": entries,
            "all_entries": args.all,
            "run_prechecks": !args.no_prechecks,
            "review_gates": review_gates,
        },
        "results": results,
        "ok": ok,
        "generated_at": "2000-01-01T00:00:00Z"
    });

    let schema_diags = report_common::validate_schema(
        CERTIFY_SUMMARY_SCHEMA_BYTES,
        "spec/x07.xtal.certify_summary@0.1.0.schema.json",
        &summary,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: xtal certify summary JSON is not schema-valid: {}",
            schema_diags[0].message
        );
    }

    Ok(summary)
}

fn snapshot_digest_hex(path: &Path) -> Result<Option<String>> {
    if path.is_file() {
        let bytes = std::fs::read(path).with_context(|| format!("read: {}", path.display()))?;
        return Ok(Some(util::sha256_hex(&bytes)));
    }
    if !path.is_dir() {
        return Ok(None);
    }

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(path).follow_links(false).into_iter().flatten() {
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();

    let mut hasher = Sha256::new();
    for file in files {
        let rel = file.strip_prefix(path).unwrap_or(file.as_path());
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        if let Ok(bytes) = std::fs::read(&file) {
            hasher.update(bytes);
            hasher.update(b"\0");
        }
    }
    Ok(Some(util::hex_lower(&hasher.finalize())))
}

fn write_certify_diag_report(project_root: &Path, report: &diagnostics::Report) -> Result<()> {
    let diag_path = project_root.join(DEFAULT_CERT_DIAG_REPORT_PATH);
    if let Some(parent) = diag_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir: {}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec(report).context("serialize xtal certify report")?;
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    util::write_atomic(&diag_path, &bytes)
        .with_context(|| format!("write: {}", diag_path.display()))?;
    Ok(())
}

fn write_certify_summary(project_root: &Path, out_dir: &Path, summary: &Value) -> Result<()> {
    let out_dir_abs = project_root.join(out_dir);
    std::fs::create_dir_all(&out_dir_abs)
        .with_context(|| format!("mkdir: {}", out_dir_abs.display()))?;
    let summary_path = out_dir_abs.join("summary.json");
    let bytes = report_common::canonical_pretty_json_bytes(summary)
        .context("serialize xtal certify summary")?;
    util::write_atomic(&summary_path, &bytes)
        .with_context(|| format!("write: {}", summary_path.display()))?;
    Ok(())
}

fn build_cert_bundle_manifest_value(
    project_root: &Path,
    out_dir: &Path,
    spec_dir: &Path,
    entries: &[String],
    ok: bool,
    bundle_path_abs: &Path,
    external_files_abs: &[PathBuf],
) -> Result<Value> {
    let out_dir_rel = out_dir.to_string_lossy().replace('\\', "/");
    let out_dir_rel_trim = out_dir_rel.trim_end_matches('/');

    let spec_dir_rel = spec_dir.to_string_lossy().replace('\\', "/");

    let mut entry_rows: Vec<Value> = Vec::new();
    for entry in entries {
        let entry_dir = safe_entry_dir_component(entry);
        entry_rows.push(json!({
            "entry": entry,
            "dir": format!("{}/{}", out_dir_rel_trim, entry_dir),
        }));
    }

    let out_dir_abs = project_root.join(out_dir);
    let mut files = collect_file_digests_under(project_root, &out_dir_abs, bundle_path_abs)?;

    // Deterministic ordering for consumers.
    files.sort_by(|a, b| {
        let ap = a.get("path").and_then(Value::as_str).unwrap_or("");
        let bp = b.get("path").and_then(Value::as_str).unwrap_or("");
        ap.cmp(bp)
    });

    let mut spec_digests: Vec<Value> = Vec::new();
    let mut examples_digests: Vec<Value> = Vec::new();
    let spec_root_abs = project_root.join(spec_dir);
    if spec_root_abs.is_dir() {
        let mut tmp_diags = Vec::new();
        let spec_files = collect_spec_files(&spec_root_abs, &Vec::new(), &mut tmp_diags);
        for path in &spec_files {
            spec_digests.push(file_digest_rel_value(project_root, path)?);
        }

        let mut examples_paths: BTreeSet<PathBuf> = BTreeSet::new();
        for entry in WalkDir::new(&spec_root_abs)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.starts_with('_') {
                continue;
            }
            if name.ends_with(".x07spec.examples.jsonl") {
                examples_paths.insert(path.to_path_buf());
            }
        }

        // Best-effort: also bind any examples referenced from spec modules, even if they live
        // outside spec_root_abs.
        for spec_path in &spec_files {
            let bytes = match std::fs::read(spec_path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let doc: Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(ops) = doc.get("operations").and_then(Value::as_array) else {
                continue;
            };
            for op in ops {
                let Some(ex_ref) = op.get("examples_ref").and_then(Value::as_str) else {
                    continue;
                };
                let ex_ref = ex_ref.trim();
                if ex_ref.is_empty() {
                    continue;
                }
                let ex_path = PathBuf::from(ex_ref);
                let abs = if ex_path.is_absolute() {
                    ex_path
                } else {
                    project_root.join(ex_ref)
                };
                examples_paths.insert(abs);
            }
        }

        for path in examples_paths.iter().filter(|p| p.is_file()) {
            examples_digests.push(file_digest_rel_value(project_root, path)?);
        }
    }

    // Deterministic ordering for consumers.
    spec_digests.sort_by(|a, b| {
        let ap = a.get("path").and_then(Value::as_str).unwrap_or("");
        let bp = b.get("path").and_then(Value::as_str).unwrap_or("");
        ap.cmp(bp)
    });
    examples_digests.sort_by(|a, b| {
        let ap = a.get("path").and_then(Value::as_str).unwrap_or("");
        let bp = b.get("path").and_then(Value::as_str).unwrap_or("");
        ap.cmp(bp)
    });

    let mut external_files: Vec<Value> = Vec::new();
    for path in external_files_abs.iter().filter(|p| p.is_file()) {
        external_files.push(file_digest_rel_value(project_root, path)?);
    }
    external_files.sort_by(|a, b| {
        let ap = a.get("path").and_then(Value::as_str).unwrap_or("");
        let bp = b.get("path").and_then(Value::as_str).unwrap_or("");
        ap.cmp(bp)
    });

    let doc = json!({
        "schema_version": CERT_BUNDLE_SCHEMA_VERSION,
        "out_dir": out_dir_rel,
        "spec_dir": spec_dir_rel,
        "generated_at": "2000-01-01T00:00:00Z",
        "ok": ok,
        "entries": entry_rows,
        "files": files,
        "external_files": external_files,
        "spec_digests": spec_digests,
        "examples_digests": examples_digests,
    });

    let schema_diags = report_common::validate_schema(
        CERT_BUNDLE_SCHEMA_BYTES,
        "spec/x07.xtal.cert_bundle@0.1.0.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: xtal certify bundle manifest JSON is not schema-valid: {}",
            schema_diags[0].message
        );
    }

    Ok(doc)
}

fn collect_file_digests_under(
    project_root: &Path,
    root: &Path,
    exclude: &Path,
) -> Result<Vec<Value>> {
    let mut out: Vec<Value> = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        if p == exclude {
            continue;
        }
        out.push(file_digest_rel_value(project_root, p)?);
    }
    Ok(out)
}

fn file_digest_rel_value(project_root: &Path, path: &Path) -> Result<Value> {
    let digest = crate::reporting::file_digest(path)
        .with_context(|| format!("digest: {}", path.display()))?;
    let rel = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(json!({
        "path": rel,
        "sha256": digest.sha256,
        "bytes_len": digest.bytes_len,
    }))
}

fn write_cert_bundle_manifest(
    project_root: &Path,
    out_dir: &Path,
    spec_dir: &Path,
    entries: &[String],
    ok: bool,
    external_files_abs: &[PathBuf],
) -> Result<()> {
    let out_dir_abs = project_root.join(out_dir);
    std::fs::create_dir_all(&out_dir_abs)
        .with_context(|| format!("mkdir: {}", out_dir_abs.display()))?;

    let bundle_path = out_dir_abs.join("bundle.json");
    let doc = build_cert_bundle_manifest_value(
        project_root,
        out_dir,
        spec_dir,
        entries,
        ok,
        &bundle_path,
        external_files_abs,
    )?;

    let bytes = report_common::canonical_pretty_json_bytes(&doc)
        .context("serialize xtal certify bundle manifest")?;
    util::write_atomic(&bundle_path, &bytes)
        .with_context(|| format!("write: {}", bundle_path.display()))?;
    Ok(())
}

fn build_ingest_summary_value(
    project_root: &Path,
    out_dir: &Path,
    input_abs: &Path,
    loaded: &LoadedIngestInputs,
    incident_dir_abs: &Path,
) -> Result<Value> {
    let input_kind = loaded.input_kind.as_str();
    let input_files = loaded.input_files.as_slice();
    let integrity = &loaded.integrity;
    let incident_id = loaded.incident_id.as_str();
    let violation_doc = &loaded.violation_doc;
    let repro_bytes = loaded.repro_bytes.as_slice();

    let out_dir_rel = out_dir.to_string_lossy().replace('\\', "/");
    let out_dir_rel_trim = out_dir_rel.trim_end_matches('/');

    let dir_rel = format!("{}/{}", out_dir_rel_trim, incident_id);
    let violation_path = format!("{}/violation.json", dir_rel);
    let repro_path = format!("{}/repro.json", dir_rel);

    let input_path = input_abs
        .strip_prefix(project_root)
        .unwrap_or(input_abs)
        .to_string_lossy()
        .replace('\\', "/");

    let clause_id = violation_doc
        .get("clause_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let world = violation_doc
        .get("world")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let mut files: Vec<Value> = Vec::new();
    let violation_abs = incident_dir_abs.join("violation.json");
    if violation_abs.is_file() {
        files.push(file_digest_rel_value(project_root, &violation_abs)?);
    }
    let repro_abs = incident_dir_abs.join("repro.json");
    if repro_abs.is_file() {
        files.push(file_digest_rel_value(project_root, &repro_abs)?);
    }
    let events_abs = incident_dir_abs.join("events.jsonl");
    if events_abs.is_file() {
        files.push(file_digest_rel_value(project_root, &events_abs)?);
    }
    files.sort_by(|a, b| {
        let ap = a.get("path").and_then(Value::as_str).unwrap_or("");
        let bp = b.get("path").and_then(Value::as_str).unwrap_or("");
        ap.cmp(bp)
    });

    let mut input_violation_digest: Option<Value> = None;
    let mut input_repro_digest: Option<Value> = None;
    let mut input_events_digest: Option<Value> = None;
    for d in input_files {
        let p = d.get("path").and_then(Value::as_str).unwrap_or("");
        if p.ends_with("/violation.json") || p == "violation.json" {
            input_violation_digest = Some(d.clone());
        } else if p.ends_with("/repro.json") || p == "repro.json" {
            input_repro_digest = Some(d.clone());
        } else if p.ends_with("/events.jsonl") || p == "events.jsonl" {
            input_events_digest = Some(d.clone());
        }
    }

    let mut ingested_violation_digest: Option<Value> = None;
    let mut ingested_repro_digest: Option<Value> = None;
    let mut ingested_events_digest: Option<Value> = None;
    for d in &files {
        let p = d.get("path").and_then(Value::as_str).unwrap_or("");
        if p.ends_with("/violation.json") || p == "violation.json" {
            ingested_violation_digest = Some(d.clone());
        } else if p.ends_with("/repro.json") || p == "repro.json" {
            ingested_repro_digest = Some(d.clone());
        } else if p.ends_with("/events.jsonl") || p == "events.jsonl" {
            ingested_events_digest = Some(d.clone());
        }
    }

    let repro_doc: Value = serde_json::from_slice(repro_bytes).context("parse repro bytes")?;
    let tool_ref = repro_doc.get("tool").cloned();
    let source_ref = repro_doc.get("source").cloned();
    let contract_ref = repro_doc
        .pointer("/contract")
        .and_then(Value::as_object)
        .map(|c| {
            let witness_count = c
                .get("witness")
                .and_then(Value::as_array)
                .map(|w| w.len() as u64)
                .unwrap_or(0);
            json!({
                "fn": c.get("fn").cloned().unwrap_or(Value::Null),
                "contract_kind": c.get("contract_kind").cloned().unwrap_or(Value::Null),
                "clause_ptr": c.get("clause_ptr").cloned().unwrap_or(Value::Null),
                "witness_count": witness_count,
            })
        });

    let mut doc = json!({
        "schema_version": INGEST_SUMMARY_SCHEMA_VERSION,
        "generated_at": "2000-01-01T00:00:00Z",
        "ok": true,
        "input": {
            "path": input_path,
            "kind": input_kind,
        },
        "ingested": {
            "id": incident_id,
            "dir": dir_rel,
            "violation_path": violation_path,
            "repro_path": repro_path,
            "clause_id": clause_id,
            "world": world,
        },
        "files": files,
        "integrity": integrity,
    });

    if let Some(obj) = doc.get_mut("input").and_then(Value::as_object_mut) {
        if let Some(v) = input_violation_digest {
            obj.insert("violation".to_string(), v);
        }
        if let Some(r) = input_repro_digest {
            obj.insert("repro".to_string(), r);
        }
        if let Some(e) = input_events_digest {
            obj.insert("events".to_string(), e);
        }
    }

    if let Some(obj) = doc.get_mut("ingested").and_then(Value::as_object_mut) {
        if let Some(v) = ingested_violation_digest {
            obj.insert("violation".to_string(), v);
        }
        if let Some(r) = ingested_repro_digest {
            obj.insert("repro".to_string(), r);
        }
        if let Some(e) = ingested_events_digest {
            obj.insert("events".to_string(), e);
        }
        if let Some(t) = tool_ref {
            obj.insert("tool".to_string(), t);
        }
        if let Some(s) = source_ref {
            obj.insert("source".to_string(), s);
        }
        if let Some(c) = contract_ref {
            obj.insert("contract".to_string(), c);
        }
    }

    let schema_diags = report_common::validate_schema(
        INGEST_SUMMARY_SCHEMA_BYTES,
        "spec/x07.xtal.ingest_summary@0.1.0.schema.json",
        &doc,
    )?;
    if !schema_diags.is_empty() {
        anyhow::bail!(
            "internal error: xtal ingest summary JSON is not schema-valid: {}",
            schema_diags[0].message
        );
    }

    Ok(doc)
}

fn write_ingest_summary(project_root: &Path, out_dir: &Path, summary: &Value) -> Result<()> {
    let out_dir_abs = project_root.join(out_dir);
    std::fs::create_dir_all(&out_dir_abs)
        .with_context(|| format!("mkdir: {}", out_dir_abs.display()))?;

    let summary_path = out_dir_abs.join("summary.json");

    let bytes = report_common::canonical_pretty_json_bytes(summary)
        .context("serialize xtal ingest summary")?;
    util::write_atomic(&summary_path, &bytes)
        .with_context(|| format!("write: {}", summary_path.display()))?;
    Ok(())
}

fn write_ingest_diag_report(project_root: &Path, report: &diagnostics::Report) -> Result<()> {
    let report_path = project_root.join(DEFAULT_INGEST_DIAG_REPORT_PATH);
    std::fs::create_dir_all(report_path.parent().unwrap_or(project_root)).with_context(|| {
        format!(
            "mkdir: {}",
            report_path.parent().unwrap_or(project_root).display()
        )
    })?;

    let mut report_bytes = serde_json::to_vec(report).context("serialize ingest diag report")?;
    if report_bytes.last() != Some(&b'\n') {
        report_bytes.push(b'\n');
    }
    util::write_atomic(&report_path, &report_bytes)
        .with_context(|| format!("write: {}", report_path.display()))?;
    Ok(())
}

fn write_tasks_diag_report(project_root: &Path, report: &diagnostics::Report) -> Result<()> {
    let report_path = project_root.join(DEFAULT_TASKS_DIAG_REPORT_PATH);
    std::fs::create_dir_all(report_path.parent().unwrap_or(project_root)).with_context(|| {
        format!(
            "mkdir: {}",
            report_path.parent().unwrap_or(project_root).display()
        )
    })?;

    let mut report_bytes = serde_json::to_vec(report).context("serialize tasks diag report")?;
    if report_bytes.last() != Some(&b'\n') {
        report_bytes.push(b'\n');
    }
    util::write_atomic(&report_path, &report_bytes)
        .with_context(|| format!("write: {}", report_path.display()))?;
    Ok(())
}

fn cmd_xtal_impl(
    machine: &crate::reporting::MachineArgs,
    args: XtalImplArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing xtal impl subcommand (try --help)");
    };
    match cmd {
        XtalImplCommand::Check(args) => cmd_xtal_impl_check(machine, args),
        XtalImplCommand::Sync(args) => cmd_xtal_impl_sync(machine, args),
    }
}

fn cmd_xtal_dev(
    machine: &crate::reporting::MachineArgs,
    args: XtalDevArgs,
) -> Result<std::process::ExitCode> {
    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let spec_root = project_root.join(&args.spec_dir);
    let gen_index = resolve_gen_index_path(&project_root, args.gen_index.as_deref());

    let mut diagnostics = Vec::new();
    let spec_files = collect_spec_files(&spec_root, &Vec::new(), &mut diagnostics);
    let mut merged_spec_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut merged_examples_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut merged_impl_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut spec_fmt_ok = true;
    let mut spec_fmt_report: Option<Value> = None;
    let mut spec_lint_ok = true;
    let mut spec_lint_report: Option<Value> = None;
    let mut spec_check_ok = true;
    let mut spec_check_report: Option<Value> = None;
    if spec_files.is_empty() {
        diagnostics.push(diag_error(
            "EXTAL_DEV_NO_SPECS",
            diagnostics::Stage::Parse,
            format!("no spec files found under {}", spec_root.display()),
            None,
        ));
    }

    if !spec_files.is_empty() {
        let fmt_args = XtalSpecFmtArgs {
            input: spec_files.clone(),
            check: true,
            write: false,
            inject_ids: false,
        };
        match capture_report_json("xtal_spec_fmt", |m| cmd_xtal_spec_fmt(m, fmt_args)) {
            Ok((code, v)) => {
                if code != std::process::ExitCode::SUCCESS {
                    spec_fmt_ok = false;
                }
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                spec_fmt_report = Some(v);
            }
            Err(err) => {
                spec_fmt_ok = false;
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("spec fmt capture failed: {err:#}"),
                    None,
                ));
            }
        }

        let lint_args = XtalSpecLintArgs {
            input: spec_files.clone(),
        };
        match capture_report_json("xtal_spec_lint", |m| cmd_xtal_spec_lint(m, lint_args)) {
            Ok((code, v)) => {
                if code != std::process::ExitCode::SUCCESS {
                    spec_lint_ok = false;
                }
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                spec_lint_report = Some(v);
            }
            Err(err) => {
                spec_lint_ok = false;
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("spec lint capture failed: {err:#}"),
                    None,
                ));
            }
        }

        let check_args = XtalSpecCheckArgs {
            input: spec_files.clone(),
            project: Some(project_root.join("x07.json")),
        };
        match capture_report_json("xtal_spec_check", |m| cmd_xtal_spec_check(m, check_args)) {
            Ok((code, v)) => {
                if code != std::process::ExitCode::SUCCESS {
                    spec_check_ok = false;
                }
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                spec_check_report = Some(v);
            }
            Err(err) => {
                spec_check_ok = false;
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("spec check capture failed: {err:#}"),
                    None,
                ));
            }
        }
    }

    let mut gen_ok = true;
    let mut gen_report: Option<Value> = None;
    if let Some(gen_index) = gen_index {
        if gen_index.is_file() {
            let gen_args = GenArgs {
                cmd: Some(GenCommand::Verify(GenVerifyArgs { index: gen_index })),
            };
            match capture_report_json("xtal_gen_verify", |m| crate::gen::cmd_gen(m, gen_args)) {
                Ok((code, v)) => {
                    if code != std::process::ExitCode::SUCCESS {
                        gen_ok = false;
                    }
                    diagnostics
                        .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                    gen_report = Some(v);
                }
                Err(err) => {
                    gen_ok = false;
                    diagnostics.push(diag_error(
                        "X07-INTERNAL-0001",
                        diagnostics::Stage::Run,
                        format!("gen verify capture failed: {err:#}"),
                        None,
                    ));
                }
            }
        } else {
            gen_ok = false;
            diagnostics.push(diag_error(
                "EXTAL_GEN_INDEX_MISSING",
                diagnostics::Stage::Parse,
                format!("generator index does not exist: {}", gen_index.display()),
                None,
            ));
        }
    } else if !spec_files.is_empty() {
        let gen_args = XtalTestsGenArgs {
            project: Some(project_root.join("x07.json")),
            spec: Vec::new(),
            spec_dir: args.spec_dir.clone(),
            out_dir: PathBuf::from(DEFAULT_GEN_DIR),
            check: true,
            write: false,
        };
        match capture_report_json("xtal_gen_from_spec", |m| {
            cmd_xtal_tests_gen_from_spec(m, gen_args)
        }) {
            Ok((code, v)) => {
                if code != std::process::ExitCode::SUCCESS {
                    gen_ok = false;
                }
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                gen_report = Some(v);
            }
            Err(err) => {
                gen_ok = false;
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("tests gen-from-spec capture failed: {err:#}"),
                    None,
                ));
            }
        }
    }

    let mut impl_ok = true;
    let mut impl_report: Option<Value> = None;
    if !spec_files.is_empty() {
        let impl_args = XtalImplCheckArgs {
            project: Some(project_root.join("x07.json")),
            spec_dir: args.spec_dir.clone(),
            impl_dir: PathBuf::from(DEFAULT_IMPL_DIR),
        };
        match capture_report_json("xtal_impl_check", |m| cmd_xtal_impl_check(m, impl_args)) {
            Ok((code, v)) => {
                if code != std::process::ExitCode::SUCCESS {
                    impl_ok = false;
                }
                merge_meta_digests(&v, "impl_digests", &mut merged_impl_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                impl_report = Some(v);
            }
            Err(err) => {
                impl_ok = false;
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("impl check capture failed: {err:#}"),
                    None,
                ));
            }
        }
    }

    let prechecks_ok = spec_fmt_ok
        && spec_lint_ok
        && spec_check_ok
        && gen_ok
        && impl_ok
        && !diagnostics
            .iter()
            .any(|d| matches!(d.severity, diagnostics::Severity::Error));

    let mut verify_status = "skipped".to_string();
    let mut verify_report: Option<Value> = None;
    let mut repair_status = "skipped".to_string();
    let mut repair_report: Option<Value> = None;

    if prechecks_ok && !args.prechecks_only {
        let verify_args = XtalVerifyArgs {
            project: Some(project_root.join("x07.json")),
            spec_dir: args.spec_dir.clone(),
            gen_index: args.gen_index.clone(),
            gen_dir: PathBuf::from(DEFAULT_GEN_DIR),
            manifest: PathBuf::from(DEFAULT_MANIFEST_PATH),
            proof_policy: ProofPolicy::Balanced,
            allow_os_world: false,
            z3_timeout_seconds: None,
            z3_memory_mb: None,
            unwind: None,
            max_bytes_len: None,
            input_len_bytes: None,
        };
        let mut verify_ok = false;
        match capture_report_json("xtal_dev_verify", |m| cmd_xtal_verify(m, verify_args)) {
            Ok((code, v)) => {
                verify_ok = code == std::process::ExitCode::SUCCESS;
                verify_status = if verify_ok {
                    "ok".to_string()
                } else {
                    "failed".to_string()
                };
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                verify_report = Some(v);
            }
            Err(err) => {
                verify_status = "failed".to_string();
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("xtal verify capture failed: {err:#}"),
                    None,
                ));
            }
        }

        if !verify_ok && args.repair_on_fail {
            let repair_args = XtalRepairArgs {
                project: Some(project_root.join("x07.json")),
                write: true,
                max_rounds: 3,
                max_candidates: 64,
                semantic_max_depth: 4,
                semantic_ops: SemanticOpsPreset::Safe,
                entry: None,
                stubs_only: true,
                allow_edit_non_stubs: false,
                semantic_only: false,
                quickfix_only: false,
                suggest_spec_patch: false,
            };
            match capture_report_json("xtal_dev_repair", |m| cmd_xtal_repair(m, repair_args)) {
                Ok((code, v)) => {
                    let ok = code == std::process::ExitCode::SUCCESS;
                    repair_status = if ok {
                        "ok".to_string()
                    } else {
                        "failed".to_string()
                    };
                    diagnostics
                        .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                    repair_report = Some(v);
                    if ok {
                        verify_status = "ok".to_string();
                    }
                }
                Err(err) => {
                    repair_status = "failed".to_string();
                    diagnostics.push(diag_error(
                        "X07-INTERNAL-0001",
                        diagnostics::Stage::Run,
                        format!("xtal repair capture failed: {err:#}"),
                        None,
                    ));
                }
            }
        }
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diagnostics);
    if !spec_fmt_ok {
        report.ok = false;
    }
    if !spec_lint_ok {
        report.ok = false;
    }
    if !spec_check_ok {
        report.ok = false;
    }
    if !gen_ok {
        report.ok = false;
    }
    if !impl_ok {
        report.ok = false;
    }
    if !args.prechecks_only && verify_status != "ok" {
        report.ok = false;
    }
    if report
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, diagnostics::Severity::Error))
    {
        report.ok = false;
    }
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report.meta.insert(
        "spec_dir".to_string(),
        Value::String(args.spec_dir.display().to_string()),
    );
    report.meta.insert(
        "spec_digests".to_string(),
        Value::Array(merged_spec_digests.into_values().collect()),
    );
    report.meta.insert(
        "examples_digests".to_string(),
        Value::Array(merged_examples_digests.into_values().collect()),
    );
    report.meta.insert(
        "impl_digests".to_string(),
        Value::Array(merged_impl_digests.into_values().collect()),
    );
    if let Some(v) = spec_fmt_report {
        report.meta.insert("spec_fmt_report".to_string(), v);
    }
    if let Some(v) = spec_lint_report {
        report.meta.insert("spec_lint_report".to_string(), v);
    }
    if let Some(v) = spec_check_report {
        report.meta.insert("spec_check_report".to_string(), v);
    }
    if let Some(v) = gen_report {
        report.meta.insert("generator_report".to_string(), v);
    }
    if let Some(v) = impl_report {
        report.meta.insert("impl_check_report".to_string(), v);
    }
    report.meta.insert(
        "prechecks_only".to_string(),
        Value::Bool(args.prechecks_only),
    );
    report.meta.insert(
        "repair_on_fail".to_string(),
        Value::Bool(args.repair_on_fail),
    );
    report
        .meta
        .insert("verify_status".to_string(), Value::String(verify_status));
    report
        .meta
        .insert("repair_status".to_string(), Value::String(repair_status));
    if let Some(v) = verify_report {
        report.meta.insert("verify_report".to_string(), v);
    }
    if let Some(v) = repair_report {
        report.meta.insert("repair_report".to_string(), v);
    }
    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_xtal_spec(
    machine: &crate::reporting::MachineArgs,
    args: XtalSpecArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing xtal spec subcommand (try --help)");
    };
    match cmd {
        XtalSpecCommand::Fmt(args) => cmd_xtal_spec_fmt(machine, args),
        XtalSpecCommand::Lint(args) => cmd_xtal_spec_lint(machine, args),
        XtalSpecCommand::Check(args) => cmd_xtal_spec_check(machine, args),
        XtalSpecCommand::Extract(args) => cmd_xtal_spec_extract(machine, args),
        XtalSpecCommand::Scaffold(args) => cmd_xtal_spec_scaffold(args),
    }
}

fn cmd_xtal_tests(
    machine: &crate::reporting::MachineArgs,
    args: XtalTestsArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing xtal tests subcommand (try --help)");
    };
    match cmd {
        XtalTestsCommand::GenFromSpec(args) => cmd_xtal_tests_gen_from_spec(machine, args),
    }
}

fn cmd_xtal_verify(
    machine: &crate::reporting::MachineArgs,
    args: XtalVerifyArgs,
) -> Result<std::process::ExitCode> {
    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let spec_root = project_root.join(&args.spec_dir);
    let gen_index = resolve_gen_index_path(&project_root, args.gen_index.as_deref());
    let manifest_path = project_root.join(&args.manifest);

    let mut diagnostics = Vec::new();

    let project_manifest_path = project_root.join("x07.json");
    let mut project_world_str = "unknown".to_string();
    let mut project_world_id: Option<WorldId> = None;
    match x07c::project::load_project_manifest(&project_manifest_path) {
        Ok(manifest) => {
            project_world_str = manifest.world.trim().to_string();
            project_world_id = WorldId::parse(&project_world_str);
        }
        Err(err) => {
            diagnostics.push(diag_error(
                "X07-INTERNAL-0001",
                diagnostics::Stage::Parse,
                format!(
                    "cannot load project manifest for world validation ({}): {err:#}",
                    project_manifest_path.display()
                ),
                None,
            ));
        }
    }
    let eval_world_ok = project_world_id.is_some_and(WorldId::is_eval_world);
    if !eval_world_ok && !args.allow_os_world {
        diagnostics.push(diag_error(
            "EXTAL_VERIFY_WORLD_UNSAFE",
            diagnostics::Stage::Lint,
            format!(
                "XTAL verify requires a deterministic solve-* world by default; found world={project_world_str:?} (pass --allow-os-world to override)."
            ),
            None,
        ));
    }

    let spec_files = collect_spec_files(&spec_root, &Vec::new(), &mut diagnostics);
    let mut merged_spec_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut merged_examples_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut merged_impl_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut spec_fmt_ok = true;
    let mut spec_fmt_report: Option<Value> = None;
    let mut spec_lint_ok = true;
    let mut spec_lint_report: Option<Value> = None;

    // fmt --check (canonical JSON only; do not inject ids implicitly).
    if !spec_files.is_empty() {
        let fmt_args = XtalSpecFmtArgs {
            input: spec_files.clone(),
            check: true,
            write: false,
            inject_ids: false,
        };
        match capture_report_json("xtal_spec_fmt", |m| cmd_xtal_spec_fmt(m, fmt_args)) {
            Ok((code, v)) => {
                if code != std::process::ExitCode::SUCCESS {
                    spec_fmt_ok = false;
                }
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                spec_fmt_report = Some(v);
            }
            Err(err) => {
                spec_fmt_ok = false;
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("spec fmt capture failed: {err:#}"),
                    None,
                ));
            }
        }
    }

    // spec lint (captured report for meta; wrapper emits a single report).
    if !spec_files.is_empty() {
        let lint_args = XtalSpecLintArgs {
            input: spec_files.clone(),
        };
        match capture_report_json("xtal_spec_lint", |m| cmd_xtal_spec_lint(m, lint_args)) {
            Ok((code, v)) => {
                if code != std::process::ExitCode::SUCCESS {
                    spec_lint_ok = false;
                }
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                spec_lint_report = Some(v);
            }
            Err(err) => {
                spec_lint_ok = false;
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("spec lint capture failed: {err:#}"),
                    None,
                ));
            }
        }
    }

    // spec check (captured report for meta; wrapper emits a single report).
    let check_args = XtalSpecCheckArgs {
        input: spec_files.clone(),
        project: Some(project_root.join("x07.json")),
    };
    let (check_code, spec_check_report) = match capture_report_json("xtal_verify_spec_check", |m| {
        cmd_xtal_spec_check(m, check_args)
    }) {
        Ok((code, v)) => {
            merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
            merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
            diagnostics.extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
            (code, Some(v))
        }
        Err(err) => {
            diagnostics.push(diag_error(
                "X07-INTERNAL-0001",
                diagnostics::Stage::Run,
                format!("spec check capture failed: {err:#}"),
                None,
            ));
            (std::process::ExitCode::from(1), None)
        }
    };

    // tests gen-from-spec --check
    let (gen_code, gen_report) = if let Some(gen_index) = gen_index.as_ref() {
        if gen_index.is_file() {
            let gen_args = GenArgs {
                cmd: Some(GenCommand::Verify(GenVerifyArgs {
                    index: gen_index.clone(),
                })),
            };
            match capture_report_json("xtal_verify_gen_verify", |m| {
                crate::gen::cmd_gen(m, gen_args)
            }) {
                Ok((code, v)) => {
                    diagnostics
                        .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                    (code, Some(v))
                }
                Err(err) => {
                    diagnostics.push(diag_error(
                        "X07-INTERNAL-0001",
                        diagnostics::Stage::Run,
                        format!("gen verify capture failed: {err:#}"),
                        None,
                    ));
                    (std::process::ExitCode::from(1), None)
                }
            }
        } else {
            diagnostics.push(diag_error(
                "EXTAL_GEN_INDEX_MISSING",
                diagnostics::Stage::Parse,
                format!("generator index does not exist: {}", gen_index.display()),
                None,
            ));
            (std::process::ExitCode::from(1), None)
        }
    } else {
        let gen_args = XtalTestsGenArgs {
            project: Some(project_root.join("x07.json")),
            spec: Vec::new(),
            spec_dir: args.spec_dir.clone(),
            out_dir: args.gen_dir.clone(),
            check: true,
            write: false,
        };
        match capture_report_json("xtal_verify_gen_from_spec", |m| {
            cmd_xtal_tests_gen_from_spec(m, gen_args)
        }) {
            Ok((code, v)) => {
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                (code, Some(v))
            }
            Err(err) => {
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("tests gen-from-spec capture failed: {err:#}"),
                    None,
                ));
                (std::process::ExitCode::from(1), None)
            }
        }
    };

    let impl_args = XtalImplCheckArgs {
        project: Some(project_root.join("x07.json")),
        spec_dir: args.spec_dir.clone(),
        impl_dir: PathBuf::from(DEFAULT_IMPL_DIR),
    };
    let (impl_code, impl_report) = match capture_report_json("xtal_verify_impl_check", |m| {
        cmd_xtal_impl_check(m, impl_args)
    }) {
        Ok((code, v)) => {
            merge_meta_digests(&v, "impl_digests", &mut merged_impl_digests);
            diagnostics.extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
            (code, Some(v))
        }
        Err(err) => {
            diagnostics.push(diag_error(
                "X07-INTERNAL-0001",
                diagnostics::Stage::Run,
                format!("impl check capture failed: {err:#}"),
                None,
            ));
            (std::process::ExitCode::from(1), None)
        }
    };

    let prechecks_ok = check_code == std::process::ExitCode::SUCCESS
        && spec_fmt_ok
        && spec_lint_ok
        && gen_code == std::process::ExitCode::SUCCESS
        && impl_code == std::process::ExitCode::SUCCESS;

    if manifest_path.is_file() {
        match std::fs::read(&manifest_path) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(doc) => {
                    if let Some(entries) = doc.get("tests").and_then(Value::as_array) {
                        for (idx, entry) in entries.iter().enumerate() {
                            let world = entry.get("world").and_then(Value::as_str).unwrap_or("");
                            let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
                            let world_ok =
                                WorldId::parse(world).is_some_and(WorldId::is_eval_world);
                            if !world_ok && !args.allow_os_world {
                                diagnostics.push(diag_error(
                                    "EXTAL_VERIFY_WORLD_UNSAFE",
                                    diagnostics::Stage::Lint,
                                    format!(
                                        "XTAL verify requires deterministic solve-* tests by default; found test world={world:?} (id={id:?}, tests[{idx}] in {}).",
                                        args.manifest.display()
                                    ),
                                    None,
                                ));
                            }
                        }
                    }
                }
                Err(err) => {
                    diagnostics.push(diag_error(
                        "X07-INTERNAL-0001",
                        diagnostics::Stage::Parse,
                        format!(
                            "cannot parse generated tests manifest for world validation: {}: {err}",
                            manifest_path.display()
                        ),
                        None,
                    ));
                }
            },
            Err(err) => {
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Parse,
                    format!(
                        "cannot read generated tests manifest for world validation: {}: {err}",
                        manifest_path.display()
                    ),
                    None,
                ));
            }
        }
    }

    let stderr_1l = |stderr: &[u8]| -> String {
        let text = String::from_utf8_lossy(stderr);
        let line = text.lines().next().unwrap_or("").trim();
        if line.is_empty() {
            "no stderr output".to_string()
        } else {
            line.to_string()
        }
    };

    let mut verify_entries: Vec<Value> = Vec::new();
    let mut verify_report_refs: Vec<Value> = Vec::new();
    let mut coverage_fail: u64 = 0;
    let mut prove_proven: u64 = 0;
    let mut prove_counterexample: u64 = 0;
    let mut prove_inconclusive: u64 = 0;
    let mut prove_unsupported: u64 = 0;
    let mut prove_timeout: u64 = 0;
    let mut prove_error: u64 = 0;
    let mut prove_tool_missing: u64 = 0;
    let mut prove_reason_rows: Vec<(String, String, String)> = Vec::new();

    let policy_str = args.proof_policy.as_str();

    let mut effective_unwind = args.unwind;
    let mut effective_max_bytes_len = args.max_bytes_len;
    let effective_input_len_bytes = args.input_len_bytes;
    if args.proof_policy == ProofPolicy::Balanced {
        if effective_unwind.is_none() {
            effective_unwind = Some(XTAL_BALANCED_DEFAULT_UNWIND);
        }
        if effective_max_bytes_len.is_none() {
            effective_max_bytes_len = Some(XTAL_BALANCED_DEFAULT_MAX_BYTES_LEN);
        }
    }

    if prechecks_ok && (eval_world_ok || args.allow_os_world) {
        std::fs::create_dir_all(project_root.join(DEFAULT_VERIFY_ARTIFACT_DIR)).with_context(
            || {
                format!(
                    "mkdir: {}",
                    project_root.join(DEFAULT_VERIFY_ARTIFACT_DIR).display()
                )
            },
        )?;

        let mut specs: Vec<(PathBuf, SpecFile)> = Vec::new();
        for spec_path in &spec_files {
            let bytes = match std::fs::read(spec_path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let spec: SpecFile = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(_) => continue,
            };
            specs.push((spec_path.clone(), spec));
        }

        let mut bound_args: Vec<String> = Vec::new();
        if let Some(v) = effective_unwind {
            bound_args.push("--unwind".to_string());
            bound_args.push(v.to_string());
        }
        if let Some(v) = effective_max_bytes_len {
            bound_args.push("--max-bytes-len".to_string());
            bound_args.push(v.to_string());
        }
        if let Some(v) = effective_input_len_bytes {
            bound_args.push("--input-len-bytes".to_string());
            bound_args.push(v.to_string());
        }

        for (spec_path, spec) in &specs {
            for op in &spec.operations {
                let entry = op.name.trim();
                if entry.is_empty() {
                    continue;
                }
                let Ok((module_id, local)) = parse_symbol_to_module_and_local(entry) else {
                    continue;
                };
                let module_path = module_id.replace('.', "/");
                let op_id = op.id.as_deref().unwrap_or(entry).trim().to_string();
                let spec_rel = spec_path
                    .strip_prefix(&project_root)
                    .unwrap_or(spec_path)
                    .to_string_lossy()
                    .replace('\\', "/");

                let coverage_report_rel = format!(
                    "{DEFAULT_VERIFY_ARTIFACT_DIR}/coverage/{module_path}/{local}.report.json"
                );
                let prove_report_rel = format!(
                    "{DEFAULT_VERIFY_ARTIFACT_DIR}/prove/{module_path}/{local}.report.json"
                );
                let proof_object_rel = format!(
                    "{DEFAULT_VERIFY_ARTIFACT_DIR}/prove/{module_path}/{local}/proof.object.json"
                );

                if let Some(parent) = project_root.join(&coverage_report_rel).parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("mkdir: {}", parent.display()))?;
                }
                if let Some(parent) = project_root.join(&prove_report_rel).parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("mkdir: {}", parent.display()))?;
                }

                let mut coverage_args = vec![
                    "verify".to_string(),
                    "--coverage".to_string(),
                    "--entry".to_string(),
                    entry.to_string(),
                    "--project".to_string(),
                    "x07.json".to_string(),
                    "--artifact-dir".to_string(),
                    DEFAULT_VERIFY_NESTED_ARTIFACT_DIR.to_string(),
                ];
                coverage_args.extend(bound_args.clone());
                coverage_args.extend([
                    "--report-out".to_string(),
                    coverage_report_rel.clone(),
                    "--quiet-json".to_string(),
                ]);
                let coverage_run = run_self_command(&project_root, &coverage_args)?;
                let coverage_path = project_root.join(&coverage_report_rel);
                let mut coverage_ok = false;
                let mut coverage_sha256 = util::sha256_hex(b"");
                let mut coverage_schema_version = "unknown".to_string();
                let mut coverage_missing = false;
                if !coverage_path.is_file() {
                    coverage_missing = true;
                    diagnostics.push(diag_error(
                        "EXTAL_VERIFY_REPORT_MISSING",
                        diagnostics::Stage::Run,
                        format!(
                            "Expected verify report was not produced for \"{entry}\" (world={project_world_str:?}): {coverage_report_rel}."
                        ),
                        None,
                    ));
                } else {
                    match std::fs::read(&coverage_path) {
                        Ok(bytes) => {
                            coverage_sha256 = util::sha256_hex(&bytes);
                            match serde_json::from_slice::<Value>(&bytes) {
                                Ok(v) => {
                                    coverage_schema_version = v
                                        .get("schema_version")
                                        .and_then(Value::as_str)
                                        .unwrap_or("unknown")
                                        .to_string();
                                    coverage_ok = coverage_run.exit_code == 0
                                        && v.get("ok").and_then(Value::as_bool).unwrap_or(false);
                                    if !coverage_ok {
                                        diagnostics.push(diag_error(
                                            "EXTAL_VERIFY_COVERAGE_FAILED",
                                            diagnostics::Stage::Run,
                                            format!(
                                                "Coverage verification failed for \"{entry}\" (world={project_world_str:?}). See report: {coverage_report_rel}."
                                            ),
                                            None,
                                        ));
                                    }
                                }
                                Err(err) => {
                                    diagnostics.push(diag_error(
                                        "X07-INTERNAL-0001",
                                        diagnostics::Stage::Run,
                                        format!(
                                            "cannot parse coverage report JSON {}: {err}",
                                            coverage_path.display()
                                        ),
                                        None,
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            diagnostics.push(diag_error(
                                "X07-INTERNAL-0001",
                                diagnostics::Stage::Run,
                                format!(
                                    "cannot read coverage report {}: {err}",
                                    coverage_path.display()
                                ),
                                None,
                            ));
                        }
                    }
                }
                let coverage_outcome = if coverage_ok { "pass" } else { "fail" };
                let coverage_ref = json!({
                    "kind": "x07_verify_coverage_report",
                    "path": coverage_report_rel.clone(),
                    "sha256": coverage_sha256,
                    "schema_version": coverage_schema_version,
                });
                verify_report_refs.push(coverage_ref.clone());
                if !coverage_ok {
                    coverage_fail += 1;
                    if coverage_missing {
                        // Missing reports count as failures, but are already surfaced as `EXTAL_VERIFY_REPORT_MISSING`.
                    }
                }

                let mut prove_args = vec![
                    "verify".to_string(),
                    "--prove".to_string(),
                    "--entry".to_string(),
                    entry.to_string(),
                    "--project".to_string(),
                    "x07.json".to_string(),
                    "--artifact-dir".to_string(),
                    DEFAULT_VERIFY_NESTED_ARTIFACT_DIR.to_string(),
                ];
                prove_args.extend(bound_args.clone());
                if let Some(timeout) = args.z3_timeout_seconds {
                    prove_args.push("--z3-timeout-seconds".to_string());
                    prove_args.push(timeout.to_string());
                } else if args.proof_policy == ProofPolicy::Balanced {
                    prove_args.push("--z3-timeout-seconds".to_string());
                    prove_args.push(XTAL_BALANCED_Z3_TIMEOUT_SECONDS.to_string());
                }
                if let Some(mem_mb) = args.z3_memory_mb {
                    prove_args.push("--z3-memory-mb".to_string());
                    prove_args.push(mem_mb.to_string());
                }
                prove_args.extend([
                    "--emit-proof".to_string(),
                    proof_object_rel.clone(),
                    "--report-out".to_string(),
                    prove_report_rel.clone(),
                    "--quiet-json".to_string(),
                ]);
                let prove_run = run_self_command(&project_root, &prove_args)?;
                let prove_path = project_root.join(&prove_report_rel);
                let mut prove_sha256 = util::sha256_hex(b"");
                let mut prove_schema_version = "unknown".to_string();
                let mut prove_raw = "error";
                if !prove_path.is_file() {
                    diagnostics.push(diag_error(
                        "EXTAL_VERIFY_REPORT_MISSING",
                        diagnostics::Stage::Run,
                        format!(
                            "Expected verify report was not produced for \"{entry}\" (world={project_world_str:?}): {prove_report_rel}."
                        ),
                        None,
                    ));
                } else {
                    match std::fs::read(&prove_path) {
                        Ok(bytes) => {
                            prove_sha256 = util::sha256_hex(&bytes);
                            match serde_json::from_slice::<Value>(&bytes) {
                                Ok(v) => {
                                    prove_schema_version = v
                                        .get("schema_version")
                                        .and_then(Value::as_str)
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let prove_kind = v
                                        .get("result")
                                        .and_then(|v| v.get("kind"))
                                        .and_then(Value::as_str)
                                        .unwrap_or("");
                                    let result_details = v
                                        .get("result")
                                        .and_then(|r| r.get("details"))
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_string();
                                    let (diag0_code, diag0_message) = v
                                        .get("diagnostics")
                                        .and_then(Value::as_array)
                                        .and_then(|arr| arr.first())
                                        .map(|d| {
                                            let code = d
                                                .get("code")
                                                .and_then(Value::as_str)
                                                .unwrap_or("")
                                                .to_string();
                                            let message = d
                                                .get("message")
                                                .and_then(Value::as_str)
                                                .unwrap_or("")
                                                .to_string();
                                            (code, message)
                                        })
                                        .unwrap_or_else(|| (String::new(), String::new()));
                                    let diag0_code_str = diag0_code.as_str();
                                    prove_raw = match prove_kind {
                                        "proven" => "proven",
                                        "counterexample_found" => "counterexample",
                                        "tool_missing" => "tool_missing",
                                        "unsupported" => "unsupported",
                                        "error" => "error",
                                        "inconclusive" => match diag0_code_str {
                                            "X07V_SMT_TIMEOUT" => "timeout",
                                            "X07V_EZ3_MISSING" | "X07V_ECBMC_MISSING" => {
                                                "tool_missing"
                                            }
                                            _ => "inconclusive",
                                        },
                                        _ => "error",
                                    };
                                    if matches!(
                                        prove_raw,
                                        "unsupported" | "tool_missing" | "timeout" | "inconclusive"
                                    ) {
                                        let code = if diag0_code.is_empty() {
                                            prove_kind.to_string()
                                        } else {
                                            diag0_code
                                        };
                                        let msg = if diag0_message.is_empty() {
                                            result_details
                                        } else {
                                            diag0_message
                                        };
                                        if !code.is_empty() || !msg.is_empty() {
                                            prove_reason_rows.push((entry.to_string(), code, msg));
                                        }
                                    }
                                }
                                Err(err) => {
                                    diagnostics.push(diag_error(
                                        "X07-INTERNAL-0001",
                                        diagnostics::Stage::Run,
                                        format!(
                                            "cannot parse prove report JSON {}: {err}",
                                            prove_path.display()
                                        ),
                                        None,
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            diagnostics.push(diag_error(
                                "X07-INTERNAL-0001",
                                diagnostics::Stage::Run,
                                format!("cannot read prove report {}: {err}", prove_path.display()),
                                None,
                            ));
                        }
                    }
                }

                let prove_ref = json!({
                    "kind": "x07_verify_prove_report",
                    "path": prove_report_rel.clone(),
                    "sha256": prove_sha256,
                    "schema_version": prove_schema_version,
                });
                verify_report_refs.push(prove_ref.clone());

                let proof_object_path = project_root.join(&proof_object_rel);
                let proof_object_ref = if proof_object_path.is_file() {
                    let bytes = match std::fs::read(&proof_object_path) {
                        Ok(v) => v,
                        Err(err) => {
                            diagnostics.push(diag_error(
                                "X07-INTERNAL-0001",
                                diagnostics::Stage::Run,
                                format!(
                                    "cannot read proof object {}: {err}",
                                    proof_object_path.display()
                                ),
                                None,
                            ));
                            Vec::new()
                        }
                    };
                    if bytes.is_empty() {
                        None
                    } else {
                        let sha256 = util::sha256_hex(&bytes);
                        let schema_version = match serde_json::from_slice::<Value>(&bytes) {
                            Ok(doc) => doc
                                .get("schema_version")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                                .to_string(),
                            Err(err) => {
                                diagnostics.push(diag_error(
                                    "X07-INTERNAL-0001",
                                    diagnostics::Stage::Run,
                                    format!(
                                        "cannot parse proof object JSON {}: {err}",
                                        proof_object_path.display()
                                    ),
                                    None,
                                ));
                                "unknown".to_string()
                            }
                        };
                        let v = json!({
                            "kind": "x07_verify_proof_object",
                            "path": proof_object_rel,
                            "sha256": sha256,
                            "schema_version": schema_version,
                        });
                        verify_report_refs.push(v.clone());
                        Some(v)
                    }
                } else {
                    None
                };

                let policy_outcome = match args.proof_policy {
                    ProofPolicy::Balanced => match prove_raw {
                        "proven" => "pass",
                        "unsupported" | "inconclusive" | "timeout" | "tool_missing" => "warn",
                        _ => "fail",
                    },
                    ProofPolicy::Strict => {
                        if prove_raw == "proven" {
                            "pass"
                        } else {
                            "fail"
                        }
                    }
                };

                match prove_raw {
                    "proven" => prove_proven += 1,
                    "counterexample" => prove_counterexample += 1,
                    "inconclusive" => prove_inconclusive += 1,
                    "unsupported" => prove_unsupported += 1,
                    "timeout" => prove_timeout += 1,
                    "tool_missing" => prove_tool_missing += 1,
                    _ => prove_error += 1,
                }

                let diag_stage = diagnostics::Stage::Run;
                match prove_raw {
                    "counterexample" => diagnostics.push(diag_error(
                        "EXTAL_VERIFY_PROVE_COUNTEREXAMPLE",
                        diag_stage,
                        format!(
                            "Proof attempt found a counterexample for \"{entry}\" (world={project_world_str:?}, policy=\"{policy_str}\"). See report: {prove_report_rel}."
                        ),
                        None,
                    )),
                    "tool_missing" => {
                        if policy_outcome == "fail" {
                            diagnostics.push(diag_error(
                                "EXTAL_VERIFY_PROVE_TOOL_MISSING",
                                diag_stage,
                                format!(
                                    "Proof tool is missing or unavailable while proving \"{entry}\" (world={project_world_str:?}). See report: {prove_report_rel}."
                                ),
                                None,
                            ));
                        } else {
                            diagnostics.push(diag_warning(
                                "WXTAL_VERIFY_PROVE_TOOL_MISSING",
                                diag_stage,
                                format!(
                                    "Proof tool is missing or unavailable while proving \"{entry}\" (world={project_world_str:?}, policy=\"{policy_str}\"). See report: {prove_report_rel}."
                                ),
                                None,
                            ));
                        }
                    }
                    "error" => diagnostics.push(diag_error(
                        "EXTAL_VERIFY_PROVE_ERROR",
                        diag_stage,
                        format!(
                            "Proof attempt failed with an internal error for \"{entry}\" (world={project_world_str:?}). Exit code {}. See report: {prove_report_rel}. stderr: {}",
                            prove_run.exit_code,
                            stderr_1l(&prove_run.stderr),
                        ),
                        None,
                    )),
                    "unsupported" => {
                        let d = if policy_outcome == "fail" {
                            diag_error(
                                "WXTAL_VERIFY_PROVE_UNSUPPORTED",
                                diag_stage,
                                format!(
                                    "Proof attempt is unsupported for \"{entry}\" (world={project_world_str:?}, policy=\"{policy_str}\"). See report: {prove_report_rel}."
                                ),
                                None,
                            )
                        } else {
                            diag_warning(
                                "WXTAL_VERIFY_PROVE_UNSUPPORTED",
                                diag_stage,
                                format!(
                                    "Proof attempt is unsupported for \"{entry}\" (world={project_world_str:?}, policy=\"{policy_str}\"). See report: {prove_report_rel}."
                                ),
                                None,
                            )
                        };
                        diagnostics.push(d);
                    }
                    "timeout" => {
                        let d = if policy_outcome == "fail" {
                            diag_error(
                                "WXTAL_VERIFY_PROVE_TIMEOUT",
                                diag_stage,
                                format!(
                                    "Proof attempt hit the configured budget for \"{entry}\" (world={project_world_str:?}, policy=\"{policy_str}\"). See report: {prove_report_rel}."
                                ),
                                None,
                            )
                        } else {
                            diag_warning(
                                "WXTAL_VERIFY_PROVE_TIMEOUT",
                                diag_stage,
                                format!(
                                    "Proof attempt hit the configured budget for \"{entry}\" (world={project_world_str:?}, policy=\"{policy_str}\"). See report: {prove_report_rel}."
                                ),
                                None,
                            )
                        };
                        diagnostics.push(d);
                    }
                    "inconclusive" => {
                        let d = if policy_outcome == "fail" {
                            diag_error(
                                "WXTAL_VERIFY_PROVE_INCONCLUSIVE",
                                diag_stage,
                                format!(
                                    "Proof attempt was inconclusive for \"{entry}\" (world={project_world_str:?}, policy=\"{policy_str}\"). See report: {prove_report_rel}."
                                ),
                                None,
                            )
                        } else {
                            diag_warning(
                                "WXTAL_VERIFY_PROVE_INCONCLUSIVE",
                                diag_stage,
                                format!(
                                    "Proof attempt was inconclusive for \"{entry}\" (world={project_world_str:?}, policy=\"{policy_str}\"). See report: {prove_report_rel}."
                                ),
                                None,
                            )
                        };
                        diagnostics.push(d);
                    }
                    _ => {}
                }

                let mut prove_obj = json!({
                    "raw": prove_raw,
                    "policy_outcome": policy_outcome,
                    "report": prove_ref,
                });
                if let Some(proof_ref) = proof_object_ref {
                    if let Some(obj) = prove_obj.as_object_mut() {
                        obj.insert("proof_object".to_string(), proof_ref);
                    }
                }

                verify_entries.push(json!({
                    "entry": entry,
                    "op_id": op_id,
                    "spec_path": spec_rel,
                    "coverage": {
                        "outcome": coverage_outcome,
                        "report": coverage_ref,
                    },
                    "prove": prove_obj,
                }));
            }
        }
    }

    if !prove_reason_rows.is_empty() {
        let mut msg = String::from("Proof support summary (first diagnostic per entry):");
        msg.push_str("\nentry | code | message");
        for (entry, code, message) in &prove_reason_rows {
            let code = if code.is_empty() {
                "unknown"
            } else {
                code.as_str()
            };
            let message = message.replace('\n', " ").trim().to_string();
            msg.push_str(&format!("\n{entry} | {code} | {message}"));
        }
        diagnostics.push(diag_warning(
            "WXTAL_VERIFY_PROVE_SUPPORT",
            diagnostics::Stage::Run,
            msg,
            None,
        ));
    }

    std::fs::create_dir_all(project_root.join(DEFAULT_VERIFY_DIR))
        .with_context(|| format!("mkdir: {}", project_root.join(DEFAULT_VERIFY_DIR).display()))?;
    let test_run = run_self_command(
        &project_root,
        &[
            "test".to_string(),
            "--all".to_string(),
            "--no-fail-fast".to_string(),
            "--manifest".to_string(),
            args.manifest.display().to_string(),
            "--allow-empty".to_string(),
            "--artifact-dir".to_string(),
            DEFAULT_VERIFY_NESTED_TEST_ARTIFACT_DIR.to_string(),
            "--report-out".to_string(),
            DEFAULT_VERIFY_TEST_REPORT_PATH.to_string(),
            "--quiet-json".to_string(),
        ],
    )?;
    let mut tests_ok = test_run.exit_code == 0;
    if !tests_ok {
        diagnostics.push(diag_error(
            "EXTAL_VERIFY_TESTS_FAILED",
            diagnostics::Stage::Run,
            format!(
                "x07 test failed (exit_code={}): {}",
                test_run.exit_code,
                stderr_summary(&test_run.stderr)
            ),
            None,
        ));
    }

    let mut tests_report_schema_version: Option<String> = None;
    let mut tests_report_sha256: Option<String> = None;
    let mut tests_passed: Option<u64> = None;
    let mut tests_failed: Option<u64> = None;
    let mut tests_skipped: Option<u64> = None;
    let tests_report_path = project_root.join(DEFAULT_VERIFY_TEST_REPORT_PATH);
    if tests_report_path.is_file() {
        let bytes = std::fs::read(&tests_report_path)
            .with_context(|| format!("read: {}", tests_report_path.display()))?;
        tests_report_sha256 = Some(util::sha256_hex(&bytes));
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(v) => {
                tests_report_schema_version = v
                    .get("schema_version")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                tests_passed = v
                    .get("summary")
                    .and_then(|s| s.get("passed"))
                    .and_then(Value::as_u64);
                tests_failed = v
                    .get("summary")
                    .and_then(|s| s.get("failed"))
                    .and_then(Value::as_u64);
                tests_skipped = v
                    .get("summary")
                    .and_then(|s| s.get("skipped"))
                    .and_then(Value::as_u64);
            }
            Err(err) => {
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!(
                        "cannot parse tests report JSON {}: {err}",
                        tests_report_path.display()
                    ),
                    None,
                ));
            }
        }
    } else {
        tests_ok = false;
        diagnostics.push(diag_error(
            "EXTAL_VERIFY_REPORT_MISSING",
            diagnostics::Stage::Run,
            format!(
                "Expected verify report was not produced for \"x07 test\": {DEFAULT_VERIFY_TEST_REPORT_PATH}."
            ),
            None,
        ));
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diagnostics);
    if !spec_fmt_ok {
        report.ok = false;
    }
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report.meta.insert(
        "spec_dir".to_string(),
        Value::String(args.spec_dir.display().to_string()),
    );
    report.meta.insert(
        "gen_dir".to_string(),
        Value::String(args.gen_dir.display().to_string()),
    );
    report.meta.insert(
        "tests_manifest".to_string(),
        Value::String(manifest_path.display().to_string()),
    );
    report.meta.insert(
        "spec_digests".to_string(),
        Value::Array(merged_spec_digests.values().cloned().collect()),
    );
    report.meta.insert(
        "examples_digests".to_string(),
        Value::Array(merged_examples_digests.values().cloned().collect()),
    );
    report.meta.insert(
        "impl_digests".to_string(),
        Value::Array(merged_impl_digests.values().cloned().collect()),
    );
    report
        .meta
        .insert("tests_ok".to_string(), Value::Bool(tests_ok));
    if let Some(v) = spec_check_report {
        report.meta.insert("spec_check_report".to_string(), v);
    }
    if let Some(v) = gen_report {
        report.meta.insert("generator_report".to_string(), v);
    }
    if let Some(v) = impl_report {
        report.meta.insert("impl_check_report".to_string(), v);
    }
    if let Some(v) = spec_lint_report {
        report.meta.insert("spec_lint_report".to_string(), v);
    }
    if let Some(v) = spec_fmt_report {
        report.meta.insert("spec_fmt_report".to_string(), v);
    }

    report.meta.insert(
        "proof_policy".to_string(),
        Value::String(policy_str.to_string()),
    );
    if effective_unwind.is_some()
        || effective_max_bytes_len.is_some()
        || effective_input_len_bytes.is_some()
    {
        let mut bounds = serde_json::Map::new();
        if let Some(v) = effective_unwind {
            bounds.insert(
                "unwind".to_string(),
                Value::Number(serde_json::Number::from(v as u64)),
            );
        }
        if let Some(v) = effective_max_bytes_len {
            bounds.insert(
                "max_bytes_len".to_string(),
                Value::Number(serde_json::Number::from(v as u64)),
            );
        }
        if let Some(v) = effective_input_len_bytes {
            bounds.insert(
                "input_len_bytes".to_string(),
                Value::Number(serde_json::Number::from(v as u64)),
            );
        }
        report
            .meta
            .insert("verify_bounds".to_string(), Value::Object(bounds));
    }
    report.meta.insert(
        "verify_summary_path".to_string(),
        Value::String(DEFAULT_VERIFY_SUMMARY_PATH.to_string()),
    );
    report.meta.insert(
        "verify_diag_path".to_string(),
        Value::String(DEFAULT_VERIFY_DIAG_REPORT_PATH.to_string()),
    );

    let diag_path = project_root.join(DEFAULT_VERIFY_DIAG_REPORT_PATH);
    if let Some(parent) = diag_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir: {}", parent.display()))?;
    }
    let mut diag_bytes = serde_json::to_vec(&report).context("serialize xtal verify report")?;
    if diag_bytes.last() != Some(&b'\n') {
        diag_bytes.push(b'\n');
    }
    util::write_atomic(&diag_path, &diag_bytes)
        .with_context(|| format!("write: {}", diag_path.display()))?;
    let diag_sha256 = util::sha256_hex(&diag_bytes);
    let diag_schema_version = report.schema_version.clone();

    let tree_digest_hex = |root: &Path| -> Result<String> {
        let mut files: Vec<PathBuf> = Vec::new();
        if root.is_dir() {
            for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
                if entry.file_type().is_file() {
                    files.push(entry.into_path());
                }
            }
        }
        files.sort();
        let mut hasher = Sha256::new();
        for path in files {
            let rel = path.strip_prefix(root).unwrap_or(path.as_path());
            let rel = rel.to_string_lossy().replace('\\', "/");
            let digest = crate::reporting::file_digest(&path)
                .with_context(|| format!("digest: {}", path.display()))?
                .sha256;
            hasher.update(rel.as_bytes());
            hasher.update([0]);
            hasher.update(digest.as_bytes());
            hasher.update([b'\n']);
        }
        Ok(util::hex_lower(&hasher.finalize()))
    };

    let to_summary_file_digest = |v: &Value| -> Option<Value> {
        let path = v.get("path").and_then(Value::as_str)?;
        let sha256 = v.get("sha256").and_then(Value::as_str)?;
        let p = PathBuf::from(path);
        let rel = p
            .strip_prefix(&project_root)
            .unwrap_or(p.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        Some(json!({"path": rel, "sha256": sha256}))
    };

    let mut spec_modules: Vec<Value> = merged_spec_digests
        .values()
        .filter_map(to_summary_file_digest)
        .collect();
    spec_modules.sort_by(|a, b| {
        a.get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("path").and_then(Value::as_str).unwrap_or(""))
    });

    let mut example_streams: Vec<Value> = merged_examples_digests
        .values()
        .filter_map(to_summary_file_digest)
        .collect();
    example_streams.sort_by(|a, b| {
        a.get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("path").and_then(Value::as_str).unwrap_or(""))
    });

    let mut impl_modules: Vec<Value> = merged_impl_digests
        .values()
        .filter_map(to_summary_file_digest)
        .collect();
    impl_modules.sort_by(|a, b| {
        a.get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("path").and_then(Value::as_str).unwrap_or(""))
    });

    let mut generated_artifacts: Vec<Value> = Vec::new();
    if manifest_path.is_file() {
        let digest = crate::reporting::file_digest(&manifest_path)
            .with_context(|| format!("digest: {}", manifest_path.display()))?;
        generated_artifacts.push(json!({
            "path": args.manifest.to_string_lossy().replace('\\', "/"),
            "sha256": digest.sha256,
        }));
    }
    if let Some(gen_index) = gen_index.as_deref().filter(|p| p.is_file()) {
        let digest = crate::reporting::file_digest(gen_index)
            .with_context(|| format!("digest: {}", gen_index.display()))?;
        let rel = gen_index
            .strip_prefix(&project_root)
            .unwrap_or(gen_index)
            .to_string_lossy()
            .replace('\\', "/");
        generated_artifacts.push(json!({
            "path": rel,
            "sha256": digest.sha256,
        }));
    }
    generated_artifacts.sort_by(|a, b| {
        a.get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("path").and_then(Value::as_str).unwrap_or(""))
    });

    let entries_total = verify_entries.len() as u64;
    let coverage_outcome = if entries_total == 0 {
        "skip"
    } else if coverage_fail == 0 {
        "pass"
    } else {
        "fail"
    };
    let prove_outcome = if entries_total == 0 {
        "skip"
    } else {
        let warn_base = prove_inconclusive + prove_unsupported + prove_timeout + prove_tool_missing;
        let fail = prove_counterexample
            + prove_error
            + if args.proof_policy == ProofPolicy::Strict {
                warn_base
            } else {
                0
            };
        let warn = if args.proof_policy == ProofPolicy::Strict {
            0
        } else {
            warn_base
        };
        if fail > 0 {
            "fail"
        } else if warn > 0 {
            "warn"
        } else {
            "pass"
        }
    };

    let mut verify_reports = verify_report_refs;
    verify_reports.sort_by(|a, b| {
        a.get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("path").and_then(Value::as_str).unwrap_or(""))
    });
    verify_reports.dedup_by(|a, b| {
        a.get("kind").and_then(Value::as_str).unwrap_or("")
            == b.get("kind").and_then(Value::as_str).unwrap_or("")
            && a.get("path").and_then(Value::as_str).unwrap_or("")
                == b.get("path").and_then(Value::as_str).unwrap_or("")
    });

    let mut error_count = 0u64;
    let mut warning_count = 0u64;
    let mut code_counts: BTreeMap<String, u64> = BTreeMap::new();
    for d in &report.diagnostics {
        match d.severity {
            diagnostics::Severity::Error => error_count += 1,
            diagnostics::Severity::Warning => warning_count += 1,
            _ => {}
        }
        *code_counts.entry(d.code.clone()).or_insert(0) += 1;
    }
    let diagnostics_outcome = if error_count > 0 {
        "fail"
    } else if warning_count > 0 {
        "warn"
    } else {
        "pass"
    };
    let overall_outcome = if report.ok {
        if warning_count > 0 {
            "warn"
        } else {
            "pass"
        }
    } else {
        "fail"
    };

    let tests_report_ref = json!({
        "kind": "x07_tests_report",
        "path": DEFAULT_VERIFY_TEST_REPORT_PATH,
        "sha256": tests_report_sha256.clone().unwrap_or_else(|| util::sha256_hex(b"")),
        "schema_version": tests_report_schema_version.clone().unwrap_or_else(|| "unknown".to_string()),
    });
    let diag_report_ref = json!({
        "kind": "xtal_diag_report",
        "path": DEFAULT_VERIFY_DIAG_REPORT_PATH,
        "sha256": diag_sha256,
        "schema_version": diag_schema_version,
    });

    let mut settings = json!({
        "world": project_world_str.clone(),
        "proof_policy": policy_str,
    });
    if effective_unwind.is_some()
        || effective_max_bytes_len.is_some()
        || effective_input_len_bytes.is_some()
    {
        let mut bounds = serde_json::Map::new();
        if let Some(v) = effective_unwind {
            bounds.insert(
                "unwind".to_string(),
                Value::Number(serde_json::Number::from(v as u64)),
            );
        }
        if let Some(v) = effective_max_bytes_len {
            bounds.insert(
                "max_bytes_len".to_string(),
                Value::Number(serde_json::Number::from(v as u64)),
            );
        }
        if let Some(v) = effective_input_len_bytes {
            bounds.insert(
                "input_len_bytes".to_string(),
                Value::Number(serde_json::Number::from(v as u64)),
            );
        }
        if let Some(obj) = settings.as_object_mut() {
            obj.insert("verify_bounds".to_string(), Value::Object(bounds));
        }
    }

    let manifest_digest = crate::reporting::file_digest(&project_root.join("x07.json"))
        .with_context(|| format!("digest: {}", project_root.join("x07.json").display()))?
        .sha256;

    let mut project_obj = json!({
        "root": ".",
        "manifest_path": "x07.json",
        "manifest_sha256": manifest_digest,
    });
    let xtal_manifest_path = project_root.join("arch").join("xtal").join("xtal.json");
    if xtal_manifest_path.is_file() {
        if let Ok(digest) = crate::reporting::file_digest(&xtal_manifest_path) {
            if let Some(obj) = project_obj.as_object_mut() {
                obj.insert(
                    "xtal_manifest_path".to_string(),
                    Value::String(
                        xtal_manifest_path
                            .strip_prefix(&project_root)
                            .unwrap_or(xtal_manifest_path.as_path())
                            .to_string_lossy()
                            .replace('\\', "/"),
                    ),
                );
                obj.insert(
                    "xtal_manifest_sha256".to_string(),
                    Value::String(digest.sha256),
                );
            }
        }
    }

    let tool_argv: Vec<String> = std::env::args().collect();
    let mut summary = json!({
        "schema_version": "x07.xtal.verify_summary@0.1.0",
        "tool": {
            "name": "x07",
            "version": env!("CARGO_PKG_VERSION"),
            "argv": tool_argv,
            "env": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH }
        },
        "project": project_obj,
        "settings": settings,
        "inputs": {
            "digests": {
                "spec_tree_sha256": tree_digest_hex(&spec_root)?,
                "impl_tree_sha256": tree_digest_hex(&project_root.join(DEFAULT_IMPL_DIR))?,
                "arch_tree_sha256": tree_digest_hex(&project_root.join("arch"))?,
                "gen_tree_sha256": tree_digest_hex(&project_root.join(&args.gen_dir))?
            },
            "spec_modules": spec_modules,
            "example_streams": example_streams,
            "impl_modules": impl_modules,
            "generated_artifacts": generated_artifacts
        },
        "results": {
            "outcome": overall_outcome,
            "prechecks": {
                "spec": if spec_fmt_ok && spec_lint_ok && check_code == std::process::ExitCode::SUCCESS { "pass" } else { "fail" },
                "generation": if gen_code == std::process::ExitCode::SUCCESS { "pass" } else { "fail" },
                "impl": if impl_code == std::process::ExitCode::SUCCESS { "pass" } else { "fail" }
            },
            "verification": {
                "coverage_outcome": coverage_outcome,
                "prove_outcome": prove_outcome,
                "counts": {
                    "entries_total": entries_total,
                    "coverage_fail": coverage_fail,
                    "prove_proven": prove_proven,
                    "prove_counterexample": prove_counterexample,
                    "prove_inconclusive": prove_inconclusive,
                    "prove_unsupported": prove_unsupported,
                    "prove_timeout": prove_timeout,
                    "prove_error": prove_error,
                    "prove_tool_missing": prove_tool_missing
                }
            },
            "tests": {
                "outcome": if tests_ok { "pass" } else { "fail" },
                "report": tests_report_ref,
                "passed": tests_passed.unwrap_or(0),
                "failed": tests_failed.unwrap_or(0),
                "skipped": tests_skipped.unwrap_or(0)
            },
            "diagnostics": {
                "outcome": diagnostics_outcome,
                "report": diag_report_ref,
                "error_count": error_count,
                "warning_count": warning_count
            }
        },
        "artifacts": {
            "verify_dir": DEFAULT_VERIFY_ARTIFACT_DIR,
            "reports": verify_reports
        },
        "entries": verify_entries
    });

    let mut top_codes: Vec<(String, u64)> = code_counts.into_iter().collect();
    top_codes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_codes.truncate(10);
    if let Some(obj) = summary
        .get_mut("results")
        .and_then(|r| r.get_mut("diagnostics"))
        .and_then(Value::as_object_mut)
    {
        obj.insert(
            "top_codes".to_string(),
            Value::Array(
                top_codes
                    .into_iter()
                    .map(|(code, count)| json!({ "code": code, "count": count }))
                    .collect(),
            ),
        );
    }

    let summary_path = project_root.join(DEFAULT_VERIFY_SUMMARY_PATH);
    if let Some(parent) = summary_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir: {}", parent.display()))?;
    }
    let summary_bytes = report_common::canonical_pretty_json_bytes(&summary)
        .context("serialize xtal verify summary")?;
    util::write_atomic(&summary_path, &summary_bytes)
        .with_context(|| format!("write: {}", summary_path.display()))?;

    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_xtal_repair(
    machine: &crate::reporting::MachineArgs,
    args: XtalRepairArgs,
) -> Result<std::process::ExitCode> {
    if args.semantic_only && args.quickfix_only {
        anyhow::bail!("--semantic-only and --quickfix-only are mutually exclusive");
    }
    if args.max_rounds == 0 {
        anyhow::bail!("--max-rounds must be >= 1");
    }
    if args.max_candidates == 0 {
        anyhow::bail!("--max-candidates must be >= 1");
    }
    if args.semantic_max_depth == 0 {
        anyhow::bail!("--semantic-max-depth must be >= 1");
    }

    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let verify_summary_path = project_root.join(DEFAULT_VERIFY_SUMMARY_PATH);
    let verify_diag_path = project_root.join(DEFAULT_VERIFY_DIAG_REPORT_PATH);
    let repair_root = project_root.join(DEFAULT_REPAIR_DIR);

    std::fs::create_dir_all(&repair_root)
        .with_context(|| format!("mkdir: {}", repair_root.display()))?;

    let mut diags: Vec<diagnostics::Diagnostic> = Vec::new();

    let baseline_summary_bytes = match std::fs::read(&verify_summary_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            diags.push(diag_error(
                "EXTAL_REPAIR_BASELINE_MISSING",
                diagnostics::Stage::Parse,
                format!(
                    "baseline verify summary is missing (run `x07 xtal verify`): {}: {err}",
                    verify_summary_path.display()
                ),
                None,
            ));
            let mut report = diagnostics::Report::ok();
            report = report.with_diagnostics(diags);
            report.ok = false;
            write_repair_artifacts(&project_root, None, &report, None, None, &[])?;
            write_report(machine, &report)?;
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let baseline_summary: Value = match serde_json::from_slice(&baseline_summary_bytes) {
        Ok(v) => v,
        Err(err) => {
            diags.push(diag_error(
                "EXTAL_REPAIR_BASELINE_MISSING",
                diagnostics::Stage::Parse,
                format!(
                    "cannot parse baseline verify summary JSON (rerun `x07 xtal verify`): {}: {err}",
                    verify_summary_path.display()
                ),
                None,
            ));
            let mut report = diagnostics::Report::ok();
            report = report.with_diagnostics(diags);
            report.ok = false;
            write_repair_artifacts(&project_root, None, &report, None, None, &[])?;
            write_report(machine, &report)?;
            return Ok(std::process::ExitCode::from(1));
        }
    };

    let baseline_outcome = baseline_summary
        .get("results")
        .and_then(|r| r.get("outcome"))
        .and_then(Value::as_str)
        .unwrap_or("fail");
    let baseline_verify_ok = baseline_outcome != "fail";

    if baseline_verify_ok {
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(diags);
        write_repair_artifacts(
            &project_root,
            Some(&baseline_summary),
            &report,
            None,
            None,
            &[],
        )?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let xtal_manifest_path = project_root.join("arch").join("xtal").join("xtal.json");
    let xtal_manifest = if xtal_manifest_path.is_file() {
        load_xtal_manifest(
            &xtal_manifest_path,
            &mut diags,
            "EXTAL_REPAIR_MANIFEST_PARSE_FAILED",
        )?
    } else {
        None
    };
    if args.write && xtal_manifest.is_none() {
        diags.push(diag_error(
            "EXTAL_REPAIR_WRITE_REQUIRES_MANIFEST",
            diagnostics::Stage::Parse,
            "--write requires arch/xtal/xtal.json so edit boundaries are explicit",
            None,
        ));
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(diags);
        report.ok = false;
        write_repair_artifacts(
            &project_root,
            Some(&baseline_summary),
            &report,
            None,
            None,
            &[],
        )?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let tests_manifest_path = read_baseline_tests_manifest_path(&verify_diag_path)
        .unwrap_or_else(|| project_root.join(DEFAULT_MANIFEST_PATH));
    let tests_manifest_rel = tests_manifest_path
        .strip_prefix(&project_root)
        .unwrap_or(&tests_manifest_path)
        .to_string_lossy()
        .replace('\\', "/");

    let baseline_entries = baseline_verify_entries(&baseline_summary);
    let tests_report_rel = baseline_tests_report_path_rel(&baseline_summary);
    let failing_test_entries =
        failing_xtal_entries_from_tests_report(&project_root, &tests_report_rel, &baseline_entries);

    let entry_filter = args.entry.as_deref();
    let target = if let Some(filter) = entry_filter {
        baseline_entries.iter().find(|e| e.entry == filter).cloned()
    } else {
        let semantic_target = baseline_entries
            .iter()
            .find(|e| e.prove_raw == "counterexample")
            .cloned();
        let tests_target = failing_test_entries
            .iter()
            .next()
            .and_then(|name| baseline_entries.iter().find(|e| &e.entry == name))
            .cloned();
        let coverage_target = baseline_entries
            .iter()
            .find(|e| e.coverage_outcome == "fail")
            .cloned();
        semantic_target.or(tests_target).or(coverage_target)
    };

    let Some(target) = target else {
        let msg = if entry_filter.is_some() {
            "baseline verify summary did not include the requested --entry"
        } else {
            "baseline verify failed, but no eligible entries were found for repair"
        };
        diags.push(diag_error(
            "EXTAL_REPAIR_NO_ACTIONABLE_FAILURE",
            diagnostics::Stage::Run,
            msg.to_string(),
            None,
        ));
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(diags);
        report.ok = false;
        write_repair_artifacts(
            &project_root,
            Some(&baseline_summary),
            &report,
            None,
            None,
            &[],
        )?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    };

    let target_entry = target.entry;
    let target_op_id = target.op_id;
    let target_prove_report_rel = target.prove_report_path_rel;
    let target_spec_path_rel = target.spec_path_rel;
    let can_semantic = target.prove_raw == "counterexample";

    let effective_stubs_only = args.stubs_only && !args.allow_edit_non_stubs;
    let (module_id, _local) = parse_symbol_to_module_and_local(&target_entry)?;
    let impl_path = module_id_to_impl_path(&project_root.join(DEFAULT_IMPL_DIR), &module_id);
    if !impl_path.is_file() {
        diags.push(diag_error(
            "EXTAL_REPAIR_NO_ACTIONABLE_FAILURE",
            diagnostics::Stage::Parse,
            format!(
                "implementation module is missing for {target_entry:?}: {}",
                impl_path.display()
            ),
            None,
        ));
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(diags);
        report.ok = false;
        write_repair_artifacts(
            &project_root,
            Some(&baseline_summary),
            &report,
            None,
            None,
            &[],
        )?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let (spec_op, _spec_path) = load_target_spec_op(
        &project_root,
        &target_spec_path_rel,
        &target_entry,
        &mut diags,
    )?;

    let file_bytes = std::fs::read(&impl_path)
        .with_context(|| format!("read impl module: {}", impl_path.display()))?;
    let mut file =
        x07ast::parse_x07ast_json(&file_bytes).context("parse implementation module x07ast")?;
    x07ast::canonicalize_x07ast_file(&mut file);
    let decl_ptr = find_decl_body_ptr(&file, &target_entry).ok_or_else(|| {
        anyhow::anyhow!(
            "target entry {target_entry:?} was not found in {}",
            impl_path.display()
        )
    })?;
    let Some(defn_idx) = file.functions.iter().position(|f| f.name == target_entry) else {
        anyhow::bail!(
            "target entry {:?} is not a defn in {}",
            target_entry,
            impl_path.display()
        );
    };

    let mut stub_diags = Vec::new();
    let stub_body = default_body_for_ty(&spec_op.result, &mut stub_diags);
    let is_stub_body = file.functions[defn_idx].body == stub_body;
    if effective_stubs_only && !is_stub_body {
        diags.push(diag_error(
            "EXTAL_REPAIR_TARGET_NOT_ELIGIBLE",
            diagnostics::Stage::Run,
            format!(
                "stubs-only repair refused to edit non-stub body for {target_entry:?} (pass --allow-edit-non-stubs to override)"
            ),
            None,
        ));
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(diags);
        report.ok = false;
        write_repair_artifacts(
            &project_root,
            Some(&baseline_summary),
            &report,
            None,
            None,
            &[],
        )?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let module_roots = collect_project_module_roots(&project_root)?;
    let op_test_filter = format!("xtal/{module_id}/{target_op_id}/");

    let mut attempts: Vec<Value> = Vec::new();
    let mut final_patchset: Option<PatchSet> = None;
    let mut final_diff: Option<String> = None;
    let mut spec_patch_suggestion: Option<String> = None;

    let mut attempt_no: u32 = 0;
    if can_semantic && !args.quickfix_only {
        attempt_no += 1;
        let (patchset, diff_text, mut ok) = run_semantic_repair_attempt(
            &project_root,
            attempt_no,
            args.max_candidates as usize,
            args.semantic_max_depth as usize,
            args.semantic_ops,
            args.semantic_only,
            &target_entry,
            &target_op_id,
            &decl_ptr,
            &impl_path,
            &file,
            defn_idx,
            &spec_op,
            &module_roots,
            &tests_manifest_rel,
            &op_test_filter,
            &baseline_summary,
            &target_prove_report_rel,
            &mut diags,
        )?;
        if ok {
            if let Some(manifest) = xtal_manifest.as_ref() {
                let disallowed = disallowed_patch_paths_from_manifest(&patchset, manifest);
                if !disallowed.is_empty() {
                    diags.push(diag_error(
                        "EXTAL_REPAIR_PATCH_OUTSIDE_ALLOWED_PATHS",
                        diagnostics::Stage::Run,
                        format!(
                            "repair patch touches files outside agent_write_paths: {}",
                            disallowed.join(", ")
                        ),
                        None,
                    ));
                    ok = false;
                }
            }
        }
        attempts.push(json!({
            "attempt": attempt_no,
            "strategy": "semantic_cegis",
            "target_entry": target_entry,
            "status": if ok { "succeeded" } else { "failed" },
            "patchset_path": format!("{DEFAULT_REPAIR_ATTEMPTS_DIR}/attempt-{attempt_no:04}/patchset.json"),
            "diff_path": format!("{DEFAULT_REPAIR_ATTEMPTS_DIR}/attempt-{attempt_no:04}/diff.txt"),
            "evaluation": { "verify_ok": ok },
        }));
        if ok {
            final_patchset = Some(patchset);
            final_diff = Some(diff_text);
        }
    } else if !can_semantic && args.semantic_only {
        diags.push(diag_error(
            "EXTAL_REPAIR_NO_ACTIONABLE_FAILURE",
            diagnostics::Stage::Run,
            "semantic repair requires a prove counterexample baseline; pass --quickfix-only or rerun `x07 xtal verify` with proofs enabled".to_string(),
            None,
        ));
        let mut report = diagnostics::Report::ok();
        report = report.with_diagnostics(diags);
        report.ok = false;
        write_repair_artifacts(
            &project_root,
            Some(&baseline_summary),
            &report,
            None,
            None,
            &[],
        )?;
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    if final_patchset.is_none() && !args.semantic_only && attempt_no < args.max_rounds {
        attempt_no += 1;
        let (patchset, diff_text, mut ok) = run_quickfix_repair_attempt(
            &project_root,
            attempt_no,
            &target_entry,
            &impl_path,
            &module_roots,
            &tests_manifest_rel,
            &op_test_filter,
            &baseline_summary,
            &target_prove_report_rel,
            &mut diags,
        )?;
        if ok {
            if let Some(manifest) = xtal_manifest.as_ref() {
                let disallowed = disallowed_patch_paths_from_manifest(&patchset, manifest);
                if !disallowed.is_empty() {
                    diags.push(diag_error(
                        "EXTAL_REPAIR_PATCH_OUTSIDE_ALLOWED_PATHS",
                        diagnostics::Stage::Run,
                        format!(
                            "repair patch touches files outside agent_write_paths: {}",
                            disallowed.join(", ")
                        ),
                        None,
                    ));
                    ok = false;
                }
            }
        }
        attempts.push(json!({
            "attempt": attempt_no,
            "strategy": "diag_quickfix",
            "target_entry": target_entry,
            "status": if ok { "succeeded" } else { "failed" },
            "patchset_path": format!("{DEFAULT_REPAIR_ATTEMPTS_DIR}/attempt-{attempt_no:04}/patchset.json"),
            "diff_path": format!("{DEFAULT_REPAIR_ATTEMPTS_DIR}/attempt-{attempt_no:04}/diff.txt"),
            "evaluation": { "verify_ok": ok },
        }));
        if ok {
            final_patchset = Some(patchset);
            final_diff = Some(diff_text);
        }
    }

    if final_patchset.is_none() && args.suggest_spec_patch && can_semantic {
        attempt_no += 1;
        let (_patchset, _diff_text, ok) = run_spec_witness_suggestion_attempt(
            &project_root,
            attempt_no,
            &target_entry,
            &target_spec_path_rel,
            &target_prove_report_rel,
            &mut diags,
        )?;
        attempts.push(json!({
            "attempt": attempt_no,
            "strategy": "spec_witness_suggestion",
            "target_entry": target_entry,
            "status": if ok { "suggested" } else { "failed" },
            "patchset_path": format!("{DEFAULT_REPAIR_ATTEMPTS_DIR}/attempt-{attempt_no:04}/patchset.json"),
            "diff_path": format!("{DEFAULT_REPAIR_ATTEMPTS_DIR}/attempt-{attempt_no:04}/diff.txt"),
        }));
        if ok {
            diags.push(diag_warning(
                "WXTAL_REPAIR_SPEC_PATCH_SUGGESTED",
                diagnostics::Stage::Run,
                "no spec-preserving patch found; emitted a spec patch suggestion for review",
                None,
            ));
            spec_patch_suggestion = Some(format!(
                "{DEFAULT_REPAIR_ATTEMPTS_DIR}/attempt-{attempt_no:04}/patchset.json"
            ));
        }
    }

    let (result_status, exit_code) = if let Some(patchset) = final_patchset.as_ref() {
        let diff_text = final_diff.as_deref().unwrap_or("");
        let patchset_path = project_root.join(DEFAULT_REPAIR_PATCHSET_PATH);
        if let Err(err) = write_patchset(&patchset_path, patchset) {
            diags.push(diag_error(
                "EXTAL_REPAIR_PATCHSET_WRITE_FAILED",
                diagnostics::Stage::Run,
                format!(
                    "cannot write patchset: {}: {err:#}",
                    patchset_path.display()
                ),
                None,
            ));
        }
        let diff_path = project_root.join(DEFAULT_REPAIR_DIFF_PATH);
        if let Err(err) = util::write_atomic(&diff_path, diff_text.as_bytes()) {
            diags.push(diag_error(
                "EXTAL_REPAIR_PATCHSET_WRITE_FAILED",
                diagnostics::Stage::Run,
                format!("cannot write diff: {}: {err}", diff_path.display()),
                None,
            ));
        }

        if args.write {
            let apply_out = run_self_command(
                &project_root,
                &[
                    "patch".to_string(),
                    "apply".to_string(),
                    "--in".to_string(),
                    DEFAULT_REPAIR_PATCHSET_PATH.to_string(),
                    "--repo-root".to_string(),
                    ".".to_string(),
                    "--write".to_string(),
                ],
            )?;
            if apply_out.exit_code != 0 {
                diags.push(diag_error(
                    "EXTAL_REPAIR_APPLY_FAILED",
                    diagnostics::Stage::Run,
                    format!(
                        "patch application failed (exit_code={}): {}",
                        apply_out.exit_code,
                        stderr_summary(&apply_out.stderr)
                    ),
                    None,
                ));
                ("failed", std::process::ExitCode::from(1))
            } else {
                let verify_out = run_self_command(
                    &project_root,
                    &[
                        "xtal".to_string(),
                        "verify".to_string(),
                        "--project".to_string(),
                        "x07.json".to_string(),
                        "--quiet-json".to_string(),
                    ],
                )?;
                if verify_out.exit_code == 0 {
                    ("patch_applied", std::process::ExitCode::SUCCESS)
                } else {
                    diags.push(diag_error(
                        "EXTAL_REPAIR_VERIFY_FAILED",
                        diagnostics::Stage::Run,
                        format!(
                            "post-apply verify failed (exit_code={}): {}",
                            verify_out.exit_code,
                            stderr_summary(&verify_out.stderr)
                        ),
                        None,
                    ));
                    ("failed", std::process::ExitCode::from(1))
                }
            }
        } else {
            ("patch_suggested", std::process::ExitCode::from(1))
        }
    } else if spec_patch_suggestion.is_some() {
        ("spec_patch_suggested", std::process::ExitCode::from(1))
    } else {
        diags.push(diag_error(
            "EXTAL_REPAIR_NO_PATCH_FOUND",
            diagnostics::Stage::Run,
            format!("no patch found for {target_entry:?} after {attempt_no} attempt(s)"),
            None,
        ));
        ("failed", std::process::ExitCode::from(1))
    };

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diags);
    if result_status != "patch_applied" && result_status != "no_action_needed" {
        report.ok = false;
    }
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report.meta.insert(
        "baseline_verify_summary".to_string(),
        Value::String(
            verify_summary_path
                .strip_prefix(&project_root)
                .unwrap_or(&verify_summary_path)
                .to_string_lossy()
                .replace('\\', "/"),
        ),
    );
    report.meta.insert(
        "tests_manifest".to_string(),
        Value::String(tests_manifest_rel.clone()),
    );
    report.meta.insert(
        "repair_summary_path".to_string(),
        Value::String(DEFAULT_REPAIR_SUMMARY_PATH.to_string()),
    );
    if let Some(path) = spec_patch_suggestion.as_deref() {
        report.meta.insert(
            "spec_patch_suggestion".to_string(),
            Value::String(path.to_string()),
        );
    }

    let result_patchset_path = project_root.join(DEFAULT_REPAIR_PATCHSET_PATH);
    let patchset_written = result_patchset_path.is_file();
    let result_diff_path = project_root.join(DEFAULT_REPAIR_DIFF_PATH);
    let diff_written = result_diff_path.is_file();

    write_repair_artifacts(
        &project_root,
        Some(&baseline_summary),
        &report,
        Some(&attempts),
        Some(result_status),
        &[
            ("patchset_written", Value::Bool(patchset_written)),
            ("diff_written", Value::Bool(diff_written)),
        ],
    )?;

    write_report(machine, &report)?;
    Ok(exit_code)
}

fn disallowed_patch_paths_from_manifest(
    patchset: &PatchSet,
    manifest: &XtalManifest,
) -> Vec<String> {
    let Some(autonomy) = manifest.autonomy.as_ref() else {
        return patchset.patches.iter().map(|p| p.path.clone()).collect();
    };

    let allow_specs = autonomy.agent_write_specs;
    let allow_arch = autonomy.agent_write_arch;
    let allowed: Vec<PathBuf> = autonomy
        .agent_write_paths
        .iter()
        .filter_map(|p| normalize_rel_path(p))
        .collect();

    let mut out = Vec::new();
    for patch in &patchset.patches {
        let Some(path) = normalize_rel_path(&patch.path) else {
            out.push(patch.path.clone());
            continue;
        };

        if !allow_specs && path.starts_with("spec") {
            out.push(patch.path.clone());
            continue;
        }
        if !allow_arch && path.starts_with("arch") {
            out.push(patch.path.clone());
            continue;
        }
        if allowed.is_empty() || !allowed.iter().any(|prefix| path.starts_with(prefix)) {
            out.push(patch.path.clone());
        }
    }
    out
}

fn normalize_rel_path(raw: &str) -> Option<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let raw = raw.replace('\\', "/");
    let p = Path::new(&raw);
    if p.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => return None,
            std::path::Component::Normal(c) => out.push(c),
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn read_baseline_tests_manifest_path(diag_path: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(diag_path).ok()?;
    let doc: Value = serde_json::from_slice(&bytes).ok()?;
    let raw = doc
        .get("meta")
        .and_then(Value::as_object)
        .and_then(|m| m.get("tests_manifest"))
        .and_then(Value::as_str)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let p = PathBuf::from(raw);
    p.is_file().then_some(p)
}

#[derive(Debug, Clone)]
struct BaselineVerifyEntry {
    entry: String,
    op_id: String,
    spec_path_rel: String,
    coverage_outcome: String,
    prove_raw: String,
    prove_report_path_rel: String,
}

fn baseline_verify_entries(baseline_summary: &Value) -> Vec<BaselineVerifyEntry> {
    let Some(entries) = baseline_summary.get("entries").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry_name = entry.get("entry").and_then(Value::as_str).unwrap_or("");
        let op_id = entry.get("op_id").and_then(Value::as_str).unwrap_or("");
        let spec_path_rel = entry.get("spec_path").and_then(Value::as_str).unwrap_or("");
        if entry_name.trim().is_empty()
            || op_id.trim().is_empty()
            || spec_path_rel.trim().is_empty()
        {
            continue;
        }
        let coverage_outcome = entry
            .get("coverage")
            .and_then(|c| c.get("outcome"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let prove_raw = entry
            .get("prove")
            .and_then(|p| p.get("raw"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let prove_report_path_rel = entry
            .get("prove")
            .and_then(|p| p.get("report"))
            .and_then(|r| r.get("path"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        out.push(BaselineVerifyEntry {
            entry: entry_name.to_string(),
            op_id: op_id.to_string(),
            spec_path_rel: spec_path_rel.to_string(),
            coverage_outcome,
            prove_raw,
            prove_report_path_rel,
        });
    }
    out
}

fn baseline_tests_report_path_rel(baseline_summary: &Value) -> String {
    baseline_summary
        .get("results")
        .and_then(|r| r.get("tests"))
        .and_then(|t| t.get("report"))
        .and_then(|r| r.get("path"))
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_VERIFY_TEST_REPORT_PATH)
        .to_string()
}

fn failing_xtal_entries_from_tests_report(
    project_root: &Path,
    tests_report_path_rel: &str,
    baseline_entries: &[BaselineVerifyEntry],
) -> BTreeSet<String> {
    let mut op_to_entry: BTreeMap<String, String> = BTreeMap::new();
    for row in baseline_entries {
        let Ok((module_id, _local)) = parse_symbol_to_module_and_local(&row.entry) else {
            continue;
        };
        op_to_entry.insert(format!("{module_id}/{}", row.op_id), row.entry.clone());
    }

    let path = project_root.join(tests_report_path_rel);
    let Ok(bytes) = std::fs::read(&path) else {
        return BTreeSet::new();
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return BTreeSet::new();
    };
    let Some(tests) = doc.get("tests").and_then(Value::as_array) else {
        return BTreeSet::new();
    };

    let mut out: BTreeSet<String> = BTreeSet::new();
    for test in tests {
        let status = test.get("status").and_then(Value::as_str).unwrap_or("");
        let is_failure = matches!(status, "fail" | "error" | "xfail_pass");
        if !is_failure {
            continue;
        }
        let id = test.get("id").and_then(Value::as_str).unwrap_or("");
        let mut parts = id.split('/');
        let Some(prefix) = parts.next() else { continue };
        if prefix != "xtal" {
            continue;
        }
        let Some(module_id) = parts.next() else {
            continue;
        };
        let Some(op_id) = parts.next() else { continue };
        let key = format!("{module_id}/{op_id}");
        let Some(entry) = op_to_entry.get(&key) else {
            continue;
        };
        out.insert(entry.clone());
    }
    out
}

fn load_target_spec_op(
    project_root: &Path,
    spec_path_rel: &str,
    entry: &str,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<(SpecOperation, PathBuf)> {
    let spec_path = project_root.join(spec_path_rel);
    let bytes = match std::fs::read(&spec_path) {
        Ok(b) => b,
        Err(err) => {
            diags.push(diag_error(
                "EXTAL_REPAIR_BASELINE_MISSING",
                diagnostics::Stage::Parse,
                format!("cannot read spec file {}: {err}", spec_path.display()),
                None,
            ));
            anyhow::bail!("cannot read spec file {}", spec_path.display());
        }
    };
    let spec: SpecFile = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(err) => {
            diags.push(diag_error(
                "EXTAL_REPAIR_BASELINE_MISSING",
                diagnostics::Stage::Parse,
                format!("cannot parse spec JSON {}: {err}", spec_path.display()),
                None,
            ));
            anyhow::bail!("cannot parse spec JSON {}", spec_path.display());
        }
    };
    let Some(op) = spec.operations.into_iter().find(|op| op.name == entry) else {
        diags.push(diag_error(
            "EXTAL_REPAIR_NO_ACTIONABLE_FAILURE",
            diagnostics::Stage::Parse,
            format!(
                "spec operation {entry:?} was not found in {}",
                spec_path.display()
            ),
            None,
        ));
        anyhow::bail!("spec operation missing");
    };
    Ok((op, spec_path))
}

fn find_decl_body_ptr(file: &x07ast::X07AstFile, entry: &str) -> Option<String> {
    let doc = x07ast::x07ast_file_to_value(file);
    let decls = doc.get("decls").and_then(Value::as_array)?;
    for (idx, decl) in decls.iter().enumerate() {
        let kind = decl.get("kind").and_then(Value::as_str).unwrap_or("");
        if kind != "defn" {
            continue;
        }
        if decl.get("name").and_then(Value::as_str) != Some(entry) {
            continue;
        }
        return Some(format!("/decls/{idx}/body"));
    }
    None
}

fn collect_project_module_roots(project_root: &Path) -> Result<Vec<PathBuf>> {
    let project_path = project_root.join("x07.json");
    let manifest =
        x07c::project::load_project_manifest(&project_path).context("load project manifest")?;
    let lock_path = x07c::project::default_lockfile_path(&project_path, &manifest);
    let lock_bytes = std::fs::read(&lock_path)
        .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
    let lock: x07c::project::Lockfile = serde_json::from_slice(&lock_bytes)
        .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;
    x07c::project::verify_lockfile(&project_path, &manifest, &lock).context("verify lockfile")?;

    let mut roots =
        x07c::project::collect_module_roots(&project_path, &manifest, &lock).context("roots")?;
    if !roots.contains(&project_root.to_path_buf()) {
        roots.push(project_root.to_path_buf());
    }
    if let Some(toolchain_root) = util::detect_toolchain_root_best_effort(project_root) {
        for root in util::toolchain_stdlib_module_roots(&toolchain_root) {
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    Ok(roots)
}

fn write_patchset(path: &Path, patchset: &PatchSet) -> Result<()> {
    let patchset_value = serde_json::to_value(patchset).context("patchset to value")?;
    let bytes = report_common::canonical_pretty_json_bytes(&patchset_value)?;
    util::write_atomic(path, &bytes).with_context(|| format!("write patchset: {}", path.display()))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        std::fs::remove_dir_all(dst).with_context(|| format!("rm -r: {}", dst.display()))?;
    }
    std::fs::create_dir_all(dst).with_context(|| format!("mkdir: {}", dst.display()))?;

    for entry in WalkDir::new(src) {
        let entry = entry.with_context(|| format!("walk: {}", src.display()))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(src)
            .with_context(|| format!("strip prefix: {}", src.display()))?;
        let out = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&out).with_context(|| format!("mkdir: {}", out.display()))?;
            continue;
        }
        if entry.file_type().is_file() {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("mkdir: {}", parent.display()))?;
            }
            std::fs::copy(path, &out)
                .with_context(|| format!("copy: {} -> {}", path.display(), out.display()))?;
        }
    }
    Ok(())
}

fn prepare_attempt_module_roots(
    attempt_dir: &Path,
    module_roots: &[PathBuf],
    impl_path: &Path,
    module_rel: &str,
) -> Result<(PathBuf, PathBuf, Vec<PathBuf>)> {
    let shadow_root = attempt_dir.join("module_root");
    let shadow_file = shadow_root.join(module_rel);

    let root_to_shadow = module_roots
        .iter()
        .find(|root| root.join(module_rel) == impl_path)
        .or_else(|| {
            module_roots
                .iter()
                .find(|root| root.join(module_rel).is_file())
        })
        .cloned();

    let attempt_roots = if let Some(root_to_shadow) = root_to_shadow {
        copy_dir_recursive(&root_to_shadow, &shadow_root).with_context(|| {
            format!(
                "copy module root for attempt ({} -> {})",
                root_to_shadow.display(),
                shadow_root.display()
            )
        })?;

        let mut out: Vec<PathBuf> = Vec::new();
        for root in module_roots {
            if root.join(module_rel).is_file() {
                if *root == root_to_shadow {
                    out.push(shadow_root.clone());
                }
                continue;
            }
            out.push(root.clone());
        }
        out
    } else {
        if let Some(parent) = shadow_file.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("mkdir: {}", parent.display()))?;
        }
        std::iter::once(shadow_root.clone())
            .chain(module_roots.iter().cloned())
            .collect()
    };

    Ok((shadow_root, shadow_file, attempt_roots))
}

fn run_spec_witness_suggestion_attempt(
    project_root: &Path,
    attempt_no: u32,
    target_entry: &str,
    spec_path_rel: &str,
    baseline_prove_report_rel: &str,
    _diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<(PatchSet, String, bool)> {
    let attempt_dir = project_root
        .join(DEFAULT_REPAIR_ATTEMPTS_DIR)
        .join(format!("attempt-{attempt_no:04}"));
    std::fs::create_dir_all(&attempt_dir)
        .with_context(|| format!("mkdir: {}", attempt_dir.display()))?;

    let patchset_path = attempt_dir.join("patchset.json");
    let diff_path = attempt_dir.join("diff.txt");

    let mut out_patchset = PatchSet {
        schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
        patches: Vec::new(),
    };
    let mut out_diff = String::new();
    let mut ok = false;

    let Some(contract) = read_prove_contract_payload(project_root, baseline_prove_report_rel)
    else {
        write_patchset(&patchset_path, &out_patchset)?;
        util::write_atomic(&diff_path, out_diff.as_bytes())
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((out_patchset, out_diff, ok));
    };

    if contract.witness.is_empty() {
        write_patchset(&patchset_path, &out_patchset)?;
        util::write_atomic(&diff_path, out_diff.as_bytes())
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((out_patchset, out_diff, ok));
    }

    let kind_field = match contract.contract_kind.as_str() {
        "requires" => "requires",
        "ensures" => "ensures",
        "invariant_entry" | "invariant_exit" => "invariant",
        _ => {
            write_patchset(&patchset_path, &out_patchset)?;
            util::write_atomic(&diff_path, out_diff.as_bytes())
                .with_context(|| format!("write diff: {}", diff_path.display()))?;
            return Ok((out_patchset, out_diff, ok));
        }
    };

    let spec_path_abs = project_root.join(spec_path_rel);
    let spec_bytes = std::fs::read(&spec_path_abs)
        .with_context(|| format!("read: {}", spec_path_abs.display()))?;
    let before: Value =
        serde_json::from_slice(&spec_bytes).context("parse spec JSON for witness suggestion")?;

    let Some(ops) = before.get("operations").and_then(Value::as_array) else {
        write_patchset(&patchset_path, &out_patchset)?;
        util::write_atomic(&diff_path, out_diff.as_bytes())
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((out_patchset, out_diff, ok));
    };

    let op_idx = ops.iter().position(|op| {
        op.get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name == target_entry)
    });
    let Some(op_idx) = op_idx else {
        write_patchset(&patchset_path, &out_patchset)?;
        util::write_atomic(&diff_path, out_diff.as_bytes())
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((out_patchset, out_diff, ok));
    };

    let clauses_ptr = format!("/operations/{op_idx}/{kind_field}");
    let Some(clauses) = before.pointer(&clauses_ptr).and_then(Value::as_array) else {
        write_patchset(&patchset_path, &out_patchset)?;
        util::write_atomic(&diff_path, out_diff.as_bytes())
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((out_patchset, out_diff, ok));
    };

    let clause_idx = clauses.iter().position(|c| {
        c.get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == contract.clause_id)
    });
    let clause_idx = clause_idx.or(contract.clause_index);
    let Some(clause_idx) = clause_idx else {
        write_patchset(&patchset_path, &out_patchset)?;
        util::write_atomic(&diff_path, out_diff.as_bytes())
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((out_patchset, out_diff, ok));
    };
    if clause_idx >= clauses.len() {
        write_patchset(&patchset_path, &out_patchset)?;
        util::write_atomic(&diff_path, out_diff.as_bytes())
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((out_patchset, out_diff, ok));
    }

    let witness_ptr = format!("/operations/{op_idx}/{kind_field}/{clause_idx}/witness");
    let witness_before: Vec<Value> = before
        .pointer(&witness_ptr)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut witness_out = witness_before.clone();
    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    for w in &witness_out {
        let key = util::canonical_jcs_bytes(w).unwrap_or_default();
        seen.insert(key);
    }
    for w in contract.witness {
        let key = util::canonical_jcs_bytes(&w).unwrap_or_default();
        if seen.insert(key) {
            witness_out.push(w);
        }
    }

    if witness_out == witness_before {
        write_patchset(&patchset_path, &out_patchset)?;
        util::write_atomic(&diff_path, out_diff.as_bytes())
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((out_patchset, out_diff, ok));
    }

    let mut after = before.clone();
    let clause_ptr = format!("/operations/{op_idx}/{kind_field}/{clause_idx}");
    if let Some(obj) = after
        .pointer_mut(&clause_ptr)
        .and_then(Value::as_object_mut)
    {
        obj.insert("witness".to_string(), Value::Array(witness_out.clone()));
    }

    let op = if before.pointer(&witness_ptr).is_some() {
        diagnostics::PatchOp::Replace {
            path: witness_ptr.clone(),
            value: Value::Array(witness_out),
        }
    } else {
        diagnostics::PatchOp::Add {
            path: witness_ptr.clone(),
            value: Value::Array(witness_out),
        }
    };

    let path_rel = spec_path_rel.replace('\\', "/");
    out_patchset.patches.push(PatchTarget {
        path: path_rel.clone(),
        patch: vec![op],
        note: Some(format!(
            "spec witness suggestion: contract_kind={}, clause_id={}",
            contract.contract_kind, contract.clause_id
        )),
    });

    out_diff = unified_json_diff(&path_rel, &before, &after)?;
    ok = true;

    write_patchset(&patchset_path, &out_patchset)?;
    util::write_atomic(&diff_path, out_diff.as_bytes())
        .with_context(|| format!("write diff: {}", diff_path.display()))?;

    Ok((out_patchset, out_diff, ok))
}

#[allow(clippy::too_many_arguments)]
fn run_semantic_repair_attempt(
    project_root: &Path,
    attempt_no: u32,
    max_candidates: usize,
    max_depth: usize,
    ops_preset: SemanticOpsPreset,
    semantic_only: bool,
    target_entry: &str,
    target_op_id: &str,
    decl_ptr: &str,
    impl_path: &Path,
    file: &x07ast::X07AstFile,
    defn_idx: usize,
    spec_op: &SpecOperation,
    module_roots: &[PathBuf],
    tests_manifest_rel: &str,
    op_test_filter: &str,
    baseline_summary: &Value,
    baseline_prove_report_rel: &str,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<(PatchSet, String, bool)> {
    let attempt_dir = project_root
        .join(DEFAULT_REPAIR_ATTEMPTS_DIR)
        .join(format!("attempt-{attempt_no:04}"));
    std::fs::create_dir_all(&attempt_dir)
        .with_context(|| format!("mkdir: {}", attempt_dir.display()))?;

    let patchset_path = attempt_dir.join("patchset.json");
    let diff_path = attempt_dir.join("diff.txt");

    let (module_id, _local) = parse_symbol_to_module_and_local(target_entry)?;
    let module_rel = format!("{}.x07.json", module_id.replace('.', "/"));
    let (_shadow_root, shadow_file, attempt_module_roots) =
        prepare_attempt_module_roots(&attempt_dir, module_roots, impl_path, &module_rel)?;

    let impl_rel = impl_path
        .strip_prefix(project_root)
        .unwrap_or(impl_path)
        .to_string_lossy()
        .replace('\\', "/");

    let target_defn = &file.functions[defn_idx];
    let examples =
        load_examples_for_entry(project_root, spec_op, target_op_id, target_defn, diags)?;
    if examples.is_empty() {
        diags.push(semantic_repair_diag(
            semantic_only,
            "EXTAL_REPAIR_SEMANTIC_NO_EXAMPLES",
            format!(
                "semantic repair requires examples for entry '{target_entry}', but none were found in spec"
            ),
        ));
        let empty_patchset = PatchSet {
            schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
            patches: Vec::new(),
        };
        write_patchset(&patchset_path, &empty_patchset)?;
        util::write_atomic(&diff_path, &[])
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((empty_patchset, String::new(), false));
    }
    if spec_op.result.trim() != "i32" {
        diags.push(semantic_repair_diag(
            semantic_only,
            "EXTAL_REPAIR_SEMANTIC_UNSUPPORTED_RETURN_TYPE",
            format!(
                "semantic repair does not support return type '{}' for entry '{target_entry}'",
                spec_op.result.trim()
            ),
        ));
        let empty_patchset = PatchSet {
            schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
            patches: Vec::new(),
        };
        write_patchset(&patchset_path, &empty_patchset)?;
        util::write_atomic(&diff_path, &[])
            .with_context(|| format!("write diff: {}", diff_path.display()))?;
        return Ok((empty_patchset, String::new(), false));
    }

    let max_nodes = semantic_max_nodes(max_depth);
    let semantic_params = semantic_params_for_entry(spec_op, target_defn);
    let mut candidates = gen_semantic_candidates(spec_op, file, defn_idx, max_candidates);
    let enum_limit = max_candidates.saturating_mul(4);
    for (idx, expr) in
        enumerate_expr_candidates("i32", &semantic_params, max_depth, max_nodes, ops_preset)
            .take(enum_limit)
            .enumerate()
    {
        candidates.push((format!("enum_{idx:04}"), expr));
    }
    let candidates = normalize_semantic_candidates(candidates, max_candidates);

    let verify_bounds = baseline_summary
        .get("settings")
        .and_then(|s| s.get("verify_bounds"))
        .and_then(Value::as_object);

    let mut best_patchset = PatchSet {
        schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
        patches: Vec::new(),
    };
    let mut best_diff = String::new();
    let mut ok = false;

    for (label, body) in candidates {
        if !semantic_expr_matches_examples(&body, &examples) {
            continue;
        }
        let mut patched = file.clone();
        patched.functions[defn_idx].body = body.clone();
        let (patched_value, patched_bytes) = canon_x07ast_file_value_and_text(&mut patched)?;
        util::write_atomic(&shadow_file, &patched_bytes)
            .with_context(|| format!("write shadow impl: {}", shadow_file.display()))?;

        let attempt_artifacts = attempt_dir.join("_artifacts");
        let attempt_test_artifacts = attempt_dir.join("_artifacts").join("test");
        std::fs::create_dir_all(&attempt_artifacts)
            .with_context(|| format!("mkdir: {}", attempt_artifacts.display()))?;
        std::fs::create_dir_all(&attempt_test_artifacts)
            .with_context(|| format!("mkdir: {}", attempt_test_artifacts.display()))?;

        let tests_report_rel = attempt_dir
            .join("tests.report.json")
            .strip_prefix(project_root)
            .unwrap_or(attempt_dir.join("tests.report.json").as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let mut test_args = vec![
            "test".to_string(),
            "--all".to_string(),
            "--manifest".to_string(),
            tests_manifest_rel.to_string(),
            "--filter".to_string(),
            op_test_filter.to_string(),
            "--allow-empty".to_string(),
        ];
        for root in attempt_module_roots.iter().map(PathBuf::as_path) {
            test_args.push("--module-root".to_string());
            test_args.push(root.display().to_string());
        }
        test_args.extend([
            "--artifact-dir".to_string(),
            attempt_test_artifacts
                .strip_prefix(project_root)
                .unwrap_or(attempt_test_artifacts.as_path())
                .to_string_lossy()
                .replace('\\', "/"),
            "--report-out".to_string(),
            tests_report_rel.clone(),
            "--quiet-json".to_string(),
        ]);
        let test_run = run_self_command(project_root, &test_args)?;
        let tests_ok = test_run.exit_code == 0;

        if !tests_ok {
            continue;
        }

        let prove_report_rel = attempt_dir
            .join("prove.report.json")
            .strip_prefix(project_root)
            .unwrap_or(attempt_dir.join("prove.report.json").as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let mut prove_args = vec![
            "verify".to_string(),
            "--prove".to_string(),
            "--entry".to_string(),
            target_entry.to_string(),
        ];
        for root in attempt_module_roots.iter().map(PathBuf::as_path) {
            prove_args.push("--module-root".to_string());
            prove_args.push(root.display().to_string());
        }
        if let Some(bounds) = verify_bounds {
            if let Some(n) = bounds.get("unwind").and_then(Value::as_u64) {
                prove_args.push("--unwind".to_string());
                prove_args.push(n.to_string());
            }
            if let Some(n) = bounds.get("max_bytes_len").and_then(Value::as_u64) {
                prove_args.push("--max-bytes-len".to_string());
                prove_args.push(n.to_string());
            }
            if let Some(n) = bounds.get("input_len_bytes").and_then(Value::as_u64) {
                prove_args.push("--input-len-bytes".to_string());
                prove_args.push(n.to_string());
            }
        }
        prove_args.extend([
            "--artifact-dir".to_string(),
            attempt_artifacts
                .strip_prefix(project_root)
                .unwrap_or(attempt_artifacts.as_path())
                .to_string_lossy()
                .replace('\\', "/"),
            "--report-out".to_string(),
            prove_report_rel.clone(),
            "--quiet-json".to_string(),
        ]);
        let prove_run = run_self_command(project_root, &prove_args)?;
        let prove_ok = prove_run.exit_code == 0
            && project_root.join(&prove_report_rel).is_file()
            && std::fs::read(project_root.join(&prove_report_rel))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|v| v.get("ok").and_then(Value::as_bool))
                == Some(true);

        if prove_ok && tests_ok {
            let mut note = format!("xtal repair semantic candidate: {label}");
            if let Some((contract_kind, clause_id)) =
                prove_contract_location(project_root, baseline_prove_report_rel)
            {
                note.push_str(&format!(
                    " (contract_kind={contract_kind}, clause_id={clause_id})"
                ));
            }
            best_patchset = PatchSet {
                schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
                patches: vec![PatchTarget {
                    path: impl_rel.clone(),
                    patch: vec![diagnostics::PatchOp::Replace {
                        path: decl_ptr.to_string(),
                        value: x07ast::expr_to_value(&body),
                    }],
                    note: Some(note),
                }],
            };
            best_diff = unified_json_diff(
                &impl_rel,
                &x07ast::x07ast_file_to_value(file),
                &patched_value,
            )?;
            ok = true;
            break;
        }
    }

    if !ok {
        diags.push(semantic_repair_diag(
            semantic_only,
            "EXTAL_REPAIR_SEMANTIC_SEARCH_EXHAUSTED",
            format!(
                "semantic repair exhausted its budget (max_candidates={max_candidates}, max_depth={max_depth}) without finding a valid patch for '{target_entry}'"
            ),
        ));
    }

    write_patchset(&patchset_path, &best_patchset)?;
    util::write_atomic(&diff_path, best_diff.as_bytes())
        .with_context(|| format!("write diff: {}", diff_path.display()))?;

    Ok((best_patchset, best_diff, ok))
}

fn semantic_repair_diag(
    semantic_only: bool,
    code: &str,
    message: impl Into<String>,
) -> diagnostics::Diagnostic {
    if semantic_only {
        diag_error(code, diagnostics::Stage::Run, message, None)
    } else {
        diag_warning(code, diagnostics::Stage::Run, message, None)
    }
}

fn semantic_max_nodes(max_depth: usize) -> usize {
    max_depth.saturating_mul(8).saturating_add(2).max(8)
}

#[derive(Debug, Clone)]
struct SemanticParam {
    name: String,
    ty: String,
}

fn semantic_params_for_entry(
    spec_op: &SpecOperation,
    defn: &x07ast::AstFunctionDef,
) -> Vec<SemanticParam> {
    let mut out = Vec::new();
    for (idx, sp) in spec_op.params.iter().enumerate() {
        let Some(ip) = defn.params.get(idx) else {
            break;
        };
        out.push(SemanticParam {
            name: ip.name.clone(),
            ty: sp.ty.clone(),
        });
    }
    out
}

#[derive(Debug, Clone)]
enum SemanticValue {
    I32(i32),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
struct SemanticExample {
    args: BTreeMap<String, SemanticValue>,
    expect: i32,
}

fn load_examples_for_entry(
    project_root: &Path,
    spec_op: &SpecOperation,
    target_op_id: &str,
    defn: &x07ast::AstFunctionDef,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<Vec<SemanticExample>> {
    let Some(ex_ref) = spec_op.examples_ref.as_deref() else {
        return Ok(Vec::new());
    };
    let ex_ref = ex_ref.trim();
    if ex_ref.is_empty() {
        return Ok(Vec::new());
    }

    let examples_path = project_root.join(ex_ref);
    if !examples_path.is_file() {
        return Ok(Vec::new());
    }

    let lines = read_examples_file(&examples_path, diags)?;
    if lines.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for line in lines {
        if line.op != target_op_id {
            continue;
        }
        let mut args: BTreeMap<String, SemanticValue> = BTreeMap::new();
        let mut ok = true;
        for (idx, sp) in spec_op.params.iter().enumerate() {
            let Some(ip) = defn.params.get(idx) else {
                ok = false;
                break;
            };
            let Some(v) = line.args.get(&sp.name) else {
                ok = false;
                break;
            };
            match sp.ty.trim() {
                "i32" => match decode_i32_value(v) {
                    Ok(n) => {
                        args.insert(ip.name.clone(), SemanticValue::I32(n));
                    }
                    Err(_) => {
                        ok = false;
                        break;
                    }
                },
                "bytes" | "bytes_view" => match decode_bytes_b64_value(v) {
                    Ok(bytes) => {
                        args.insert(ip.name.clone(), SemanticValue::Bytes(bytes));
                    }
                    Err(_) => {
                        ok = false;
                        break;
                    }
                },
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        let expect = match decode_i32_value(&line.expect) {
            Ok(n) => n,
            Err(_) => continue,
        };
        out.push(SemanticExample { args, expect });
    }

    Ok(out)
}

fn semantic_expr_matches_examples(expr: &Expr, examples: &[SemanticExample]) -> bool {
    for ex in examples {
        let Some(v) = eval_expr_on_example(expr, ex) else {
            return false;
        };
        let Some(n) = v.as_i64().and_then(|n| i32::try_from(n).ok()) else {
            return false;
        };
        if n != ex.expect {
            return false;
        }
    }
    true
}

fn eval_expr_on_example(expr: &Expr, example: &SemanticExample) -> Option<Value> {
    let out = eval_semantic_expr(expr, &example.args)?;
    match out {
        SemanticValue::I32(n) => Some(Value::Number(n.into())),
        SemanticValue::Bytes(_) => None,
    }
}

fn eval_expr_to_i32(expr: &Expr, env: &BTreeMap<String, SemanticValue>) -> Option<i32> {
    match eval_semantic_expr(expr, env)? {
        SemanticValue::I32(n) => Some(n),
        SemanticValue::Bytes(_) => None,
    }
}

fn eval_semantic_expr(expr: &Expr, env: &BTreeMap<String, SemanticValue>) -> Option<SemanticValue> {
    match expr {
        Expr::Int { value, .. } => Some(SemanticValue::I32(*value)),
        Expr::Ident { name, .. } => env.get(name).cloned(),
        Expr::List { items, .. } => {
            if items.is_empty() {
                return None;
            }
            let op = items[0].as_ident()?;
            match op {
                "bytes.len" | "view.len" => {
                    if items.len() != 2 {
                        return None;
                    }
                    let SemanticValue::Bytes(bytes) = eval_semantic_expr(&items[1], env)? else {
                        return None;
                    };
                    Some(SemanticValue::I32(
                        i32::try_from(bytes.len()).unwrap_or(i32::MAX),
                    ))
                }
                "+" => {
                    if items.len() != 3 {
                        return None;
                    }
                    let a = eval_expr_to_i32(&items[1], env)?;
                    let b = eval_expr_to_i32(&items[2], env)?;
                    Some(SemanticValue::I32(a.wrapping_add(b)))
                }
                "-" => {
                    if items.len() != 3 {
                        return None;
                    }
                    let a = eval_expr_to_i32(&items[1], env)?;
                    let b = eval_expr_to_i32(&items[2], env)?;
                    Some(SemanticValue::I32(a.wrapping_sub(b)))
                }
                "*" => {
                    if items.len() != 3 {
                        return None;
                    }
                    let a = eval_expr_to_i32(&items[1], env)?;
                    let b = eval_expr_to_i32(&items[2], env)?;
                    Some(SemanticValue::I32(a.wrapping_mul(b)))
                }
                "/" => {
                    if items.len() != 3 {
                        return None;
                    }
                    let a = eval_expr_to_i32(&items[1], env)?;
                    let b = eval_expr_to_i32(&items[2], env)?;
                    if b == 0 {
                        return None;
                    }
                    let out = if a == i32::MIN && b == -1 {
                        i32::MIN
                    } else {
                        a / b
                    };
                    Some(SemanticValue::I32(out))
                }
                "%" => {
                    if items.len() != 3 {
                        return None;
                    }
                    let a = eval_expr_to_i32(&items[1], env)?;
                    let b = eval_expr_to_i32(&items[2], env)?;
                    if b == 0 {
                        return None;
                    }
                    let out = if a == i32::MIN && b == -1 { 0 } else { a % b };
                    Some(SemanticValue::I32(out))
                }
                "=" => {
                    if items.len() != 3 {
                        return None;
                    }
                    let a = eval_expr_to_i32(&items[1], env)?;
                    let b = eval_expr_to_i32(&items[2], env)?;
                    Some(SemanticValue::I32(i32::from(a == b)))
                }
                "<" => {
                    if items.len() != 3 {
                        return None;
                    }
                    let a = eval_expr_to_i32(&items[1], env)?;
                    let b = eval_expr_to_i32(&items[2], env)?;
                    Some(SemanticValue::I32(i32::from(a < b)))
                }
                "if" => {
                    if items.len() != 4 {
                        return None;
                    }
                    let cond = eval_expr_to_i32(&items[1], env)?;
                    if cond != 0 {
                        eval_semantic_expr(&items[2], env)
                    } else {
                        eval_semantic_expr(&items[3], env)
                    }
                }
                _ => None,
            }
        }
    }
}

fn normalize_semantic_candidates(
    candidates: Vec<(String, Expr)>,
    max_candidates: usize,
) -> Vec<(String, Expr)> {
    #[derive(Clone)]
    struct Item {
        label: String,
        expr: Expr,
        key: Vec<u8>,
        node_count: usize,
    }

    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut out: Vec<Item> = Vec::new();
    for (label, expr) in candidates {
        let key = canonical_expr_key(&expr);
        if !seen.insert(key.clone()) {
            continue;
        }
        out.push(Item {
            label,
            node_count: expr.node_count(),
            expr,
            key,
        });
    }
    out.sort_by(|a, b| {
        a.node_count
            .cmp(&b.node_count)
            .then_with(|| a.key.cmp(&b.key))
    });
    out.truncate(max_candidates);
    out.into_iter().map(|i| (i.label, i.expr)).collect()
}

#[derive(Clone)]
struct SizedExpr {
    expr: Expr,
    key: Vec<u8>,
    depth: usize,
}

struct ExprCandidateIter {
    max_depth: usize,
    max_nodes: usize,
    binary_ops: Vec<(&'static str, bool)>,
    by_size: Vec<Vec<SizedExpr>>,
    seen: BTreeSet<Vec<u8>>,
    current_size: usize,
    current_index: usize,
    prepared_up_to: usize,
}

impl ExprCandidateIter {
    fn empty() -> Self {
        Self {
            max_depth: 0,
            max_nodes: 0,
            binary_ops: Vec::new(),
            by_size: vec![Vec::new()],
            seen: BTreeSet::new(),
            current_size: 1,
            current_index: 0,
            prepared_up_to: 0,
        }
    }

    fn new_i32(
        params: &[SemanticParam],
        max_depth: usize,
        max_nodes: usize,
        ops_preset: SemanticOpsPreset,
    ) -> Self {
        if max_depth == 0 || max_nodes == 0 {
            return Self::empty();
        }

        let mut binary_ops: Vec<(&'static str, bool)> = vec![
            ("+", true),
            ("-", false),
            ("*", true),
            ("=", true),
            ("<", false),
        ];
        if ops_preset == SemanticOpsPreset::Full {
            binary_ops.extend([("/", false), ("%", false)]);
        }

        let mut by_size: Vec<Vec<SizedExpr>> = vec![Vec::new(); max_nodes + 1];
        let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();

        for n in [-1, 0, 1, 2] {
            let expr = int_expr(n);
            let key = canonical_expr_key(&expr);
            if seen.insert(key.clone()) {
                by_size[1].push(SizedExpr {
                    expr,
                    key,
                    depth: 1,
                });
            }
        }

        for p in params {
            match p.ty.trim() {
                "i32" => {
                    let expr = ident(p.name.clone());
                    let key = canonical_expr_key(&expr);
                    if seen.insert(key.clone()) {
                        by_size[1].push(SizedExpr {
                            expr,
                            key,
                            depth: 1,
                        });
                    }
                }
                "bytes" => {
                    let expr = list_expr([ident("bytes.len"), ident(p.name.clone())]);
                    let key = canonical_expr_key(&expr);
                    if seen.insert(key.clone()) && 3 < by_size.len() {
                        by_size[3].push(SizedExpr {
                            expr,
                            key,
                            depth: 2,
                        });
                    }
                }
                "bytes_view" => {
                    let expr = list_expr([ident("view.len"), ident(p.name.clone())]);
                    let key = canonical_expr_key(&expr);
                    if seen.insert(key.clone()) && 3 < by_size.len() {
                        by_size[3].push(SizedExpr {
                            expr,
                            key,
                            depth: 2,
                        });
                    }
                }
                _ => {}
            }
        }

        Self {
            max_depth,
            max_nodes,
            binary_ops,
            by_size,
            seen,
            current_size: 1,
            current_index: 0,
            prepared_up_to: 0,
        }
    }

    fn prepare_size(&mut self, size: usize) {
        if size == 0 || size >= self.by_size.len() {
            return;
        }

        if size >= 4 {
            let want_sum = size - 2;
            for (op, commutative) in &self.binary_ops {
                for a_size in 1..=(want_sum.saturating_sub(1)) {
                    let b_size = want_sum - a_size;
                    if b_size == 0 || a_size >= self.by_size.len() || b_size >= self.by_size.len() {
                        continue;
                    }
                    let mut next_size: Vec<SizedExpr> = Vec::new();
                    for a in &self.by_size[a_size] {
                        for b in &self.by_size[b_size] {
                            if *commutative && a.key > b.key {
                                continue;
                            }
                            let depth = 1 + a.depth.max(b.depth);
                            if depth > self.max_depth {
                                continue;
                            }
                            let expr = list_expr([ident(*op), a.expr.clone(), b.expr.clone()]);
                            let key = canonical_expr_key(&expr);
                            if !self.seen.insert(key.clone()) {
                                continue;
                            }
                            next_size.push(SizedExpr { expr, key, depth });
                        }
                    }
                    self.by_size[size].extend(next_size);
                }
            }
        }

        if size >= 5 {
            let want_sum = size - 2;
            for cond_size in 1..=(want_sum.saturating_sub(2)) {
                for then_size in 1..=(want_sum - cond_size).saturating_sub(1) {
                    let else_size = want_sum - cond_size - then_size;
                    if else_size == 0 {
                        continue;
                    }
                    if cond_size >= self.by_size.len()
                        || then_size >= self.by_size.len()
                        || else_size >= self.by_size.len()
                    {
                        continue;
                    }
                    let mut next_size: Vec<SizedExpr> = Vec::new();
                    for cond in &self.by_size[cond_size] {
                        for then_expr in &self.by_size[then_size] {
                            for else_expr in &self.by_size[else_size] {
                                let depth =
                                    1 + cond.depth.max(then_expr.depth).max(else_expr.depth);
                                if depth > self.max_depth {
                                    continue;
                                }
                                let expr = list_expr([
                                    ident("if"),
                                    cond.expr.clone(),
                                    then_expr.expr.clone(),
                                    else_expr.expr.clone(),
                                ]);
                                let key = canonical_expr_key(&expr);
                                if !self.seen.insert(key.clone()) {
                                    continue;
                                }
                                next_size.push(SizedExpr { expr, key, depth });
                            }
                        }
                    }
                    self.by_size[size].extend(next_size);
                }
            }
        }

        self.by_size[size].sort_by(|a, b| a.key.cmp(&b.key));
    }
}

impl Iterator for ExprCandidateIter {
    type Item = Expr;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_size <= self.max_nodes {
            if self.prepared_up_to < self.current_size {
                self.prepare_size(self.current_size);
                self.prepared_up_to = self.current_size;
                self.current_index = 0;
            }

            let bucket = self.by_size.get(self.current_size)?;
            if let Some(item) = bucket.get(self.current_index) {
                self.current_index += 1;
                return Some(item.expr.clone());
            }

            self.current_size += 1;
            self.current_index = 0;
        }
        None
    }
}

fn enumerate_expr_candidates(
    return_ty: &str,
    params: &[SemanticParam],
    max_depth: usize,
    max_nodes: usize,
    ops_preset: SemanticOpsPreset,
) -> ExprCandidateIter {
    if return_ty.trim() != "i32" {
        return ExprCandidateIter::empty();
    }
    ExprCandidateIter::new_i32(params, max_depth, max_nodes, ops_preset)
}

fn canonical_expr_key(expr: &Expr) -> Vec<u8> {
    let key_val = x07ast::expr_to_value(expr);
    util::canonical_jcs_bytes(&key_val)
        .or_else(|_| serde_json::to_vec(&key_val).map_err(anyhow::Error::from))
        .unwrap_or_default()
}

fn gen_semantic_candidates(
    spec_op: &SpecOperation,
    file: &x07ast::X07AstFile,
    defn_idx: usize,
    max_candidates: usize,
) -> Vec<(String, Expr)> {
    let target = &file.functions[defn_idx];

    let mut spec_to_impl: BTreeMap<String, String> = BTreeMap::new();
    for (i, sp) in spec_op.params.iter().enumerate() {
        if let Some(ip) = target.params.get(i) {
            if sp.name != ip.name {
                spec_to_impl.insert(sp.name.clone(), ip.name.clone());
            }
        }
    }

    let spec_result_ty = spec_op.result.trim().to_string();
    let spec_param_tys: Vec<String> = spec_op
        .params
        .iter()
        .map(|p| p.ty.trim().to_string())
        .collect();

    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut out: Vec<(String, Expr)> = Vec::new();

    let push =
        |label: String, expr: Expr, out: &mut Vec<(String, Expr)>, seen: &mut BTreeSet<Vec<u8>>| {
            if out.len() >= max_candidates {
                return;
            }
            let key_val = x07ast::expr_to_value(&expr);
            let key = util::canonical_jcs_bytes(&key_val)
                .or_else(|_| serde_json::to_vec(&key_val).map_err(anyhow::Error::from))
                .unwrap_or_default();
            if seen.insert(key) {
                out.push((label, expr));
            }
        };

    for clause in &spec_op.ensures {
        if let Ok(expr) = x07c::ast::expr_from_json(&clause.expr) {
            if let Some(rhs) = find_result_equality_rhs(&expr) {
                let rhs = rewrite_idents(&rhs, &spec_to_impl);
                let label = clause
                    .id
                    .as_deref()
                    .map(|id| format!("ensures({id})"))
                    .unwrap_or_else(|| "ensures".to_string());
                push(label, rhs, &mut out, &mut seen);
            }
        }
    }

    for (i, ty) in spec_param_tys.iter().enumerate() {
        if ty == &spec_result_ty {
            if let Some(ip) = target.params.get(i) {
                push(
                    format!("identity({})", ip.name),
                    ident(&ip.name),
                    &mut out,
                    &mut seen,
                );
            }
        }
    }

    for (idx, f) in file.functions.iter().enumerate() {
        if idx == defn_idx {
            continue;
        }
        if !type_ref_matches_str(&f.result, &spec_result_ty) {
            continue;
        }
        if f.params.len() != spec_param_tys.len() {
            continue;
        }
        let mut sig_ok = true;
        for (p, want) in f.params.iter().zip(spec_param_tys.iter()) {
            if !type_ref_matches_str(&p.ty, want) {
                sig_ok = false;
                break;
            }
        }
        if !sig_ok {
            continue;
        }

        let mut items: Vec<Expr> = Vec::with_capacity(1 + target.params.len());
        items.push(ident(&f.name));
        for p in &target.params {
            items.push(ident(&p.name));
        }
        push(
            format!("delegate({})", f.name),
            list_expr_vec(items),
            &mut out,
            &mut seen,
        );
    }

    if spec_result_ty == "bytes" {
        for (i, ty) in spec_param_tys.iter().enumerate() {
            if ty == "bytes" {
                if let Some(ip) = target.params.get(i) {
                    let e = list_expr_vec(vec![
                        ident("view.to_bytes"),
                        list_expr_vec(vec![ident("bytes.view"), ident(&ip.name)]),
                    ]);
                    push(format!("clone({})", ip.name), e, &mut out, &mut seen);
                }
            } else if ty == "bytes_view" {
                if let Some(ip) = target.params.get(i) {
                    let e = list_expr_vec(vec![ident("view.to_bytes"), ident(&ip.name)]);
                    push(format!("to_bytes({})", ip.name), e, &mut out, &mut seen);
                }
            }
        }
        push(
            "bytes.empty".to_string(),
            list_expr([ident("bytes.empty")]),
            &mut out,
            &mut seen,
        );
    } else if spec_result_ty == "bytes_view" {
        for (i, ty) in spec_param_tys.iter().enumerate() {
            if ty == "bytes" {
                if let Some(ip) = target.params.get(i) {
                    let e = list_expr([ident("bytes.view"), ident(&ip.name)]);
                    push(format!("view({})", ip.name), e, &mut out, &mut seen);
                }
            }
        }
    } else if spec_result_ty == "i32" {
        for (i, ty) in spec_param_tys.iter().enumerate() {
            if ty == "i32" {
                if let Some(ip) = target.params.get(i) {
                    push(
                        format!("identity({})", ip.name),
                        ident(&ip.name),
                        &mut out,
                        &mut seen,
                    );
                }
            } else if ty == "bytes" {
                if let Some(ip) = target.params.get(i) {
                    let e = list_expr([ident("bytes.len"), ident(&ip.name)]);
                    push(format!("len({})", ip.name), e, &mut out, &mut seen);
                }
            } else if ty == "bytes_view" {
                if let Some(ip) = target.params.get(i) {
                    let e = list_expr([ident("view.len"), ident(&ip.name)]);
                    push(format!("len({})", ip.name), e, &mut out, &mut seen);
                }
            }
        }
        push("0".to_string(), int_expr(0), &mut out, &mut seen);
        push("1".to_string(), int_expr(1), &mut out, &mut seen);
        push("-1".to_string(), int_expr(-1), &mut out, &mut seen);
    }

    if out.is_empty() {
        out.push(("no_candidates".to_string(), target.body.clone()));
    }

    out.truncate(max_candidates);
    out
}

fn type_ref_matches_str(ty: &x07ast::TypeRef, want: &str) -> bool {
    match ty {
        x07ast::TypeRef::Named(n) => n.trim() == want.trim(),
        _ => false,
    }
}

fn rewrite_idents(expr: &Expr, mapping: &BTreeMap<String, String>) -> Expr {
    match expr {
        Expr::Ident { name, .. } => mapping
            .get(name)
            .cloned()
            .map(ident)
            .unwrap_or_else(|| ident(name.clone())),
        Expr::Int { value, .. } => int_expr(*value),
        Expr::List { items, .. } => {
            list_expr_vec(items.iter().map(|e| rewrite_idents(e, mapping)).collect())
        }
    }
}

fn find_result_equality_rhs(expr: &Expr) -> Option<Expr> {
    if let Some(rhs) = direct_result_equality_rhs(expr) {
        return Some(rhs);
    }
    match expr {
        Expr::List { items, .. } => items.iter().find_map(find_result_equality_rhs),
        _ => None,
    }
}

fn direct_result_equality_rhs(expr: &Expr) -> Option<Expr> {
    let Expr::List { items, .. } = expr else {
        return None;
    };
    if items.len() != 3 {
        return None;
    }
    let Expr::Ident { name: op, .. } = &items[0] else {
        return None;
    };
    if op != "=" {
        return None;
    }
    match (&items[1], &items[2]) {
        (Expr::Ident { name: a, .. }, rhs) if a == "__result" => Some(rhs.clone()),
        (lhs, Expr::Ident { name: b, .. }) if b == "__result" => Some(lhs.clone()),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_quickfix_repair_attempt(
    project_root: &Path,
    attempt_no: u32,
    target_entry: &str,
    impl_path: &Path,
    module_roots: &[PathBuf],
    tests_manifest_rel: &str,
    op_test_filter: &str,
    baseline_summary: &Value,
    baseline_prove_report_rel: &str,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<(PatchSet, String, bool)> {
    let attempt_dir = project_root
        .join(DEFAULT_REPAIR_ATTEMPTS_DIR)
        .join(format!("attempt-{attempt_no:04}"));
    std::fs::create_dir_all(&attempt_dir)
        .with_context(|| format!("mkdir: {}", attempt_dir.display()))?;

    let patchset_path = attempt_dir.join("patchset.json");
    let diff_path = attempt_dir.join("diff.txt");

    let (module_id, _local) = parse_symbol_to_module_and_local(target_entry)?;
    let module_rel = format!("{}.x07.json", module_id.replace('.', "/"));
    let (_shadow_root, shadow_file, attempt_module_roots) =
        prepare_attempt_module_roots(&attempt_dir, module_roots, impl_path, &module_rel)?;

    let original_bytes = std::fs::read(impl_path)
        .with_context(|| format!("read impl module: {}", impl_path.display()))?;
    util::write_atomic(&shadow_file, &original_bytes)
        .with_context(|| format!("write shadow impl: {}", shadow_file.display()))?;

    let shadow_rel = shadow_file
        .strip_prefix(project_root)
        .unwrap_or(&shadow_file)
        .to_string_lossy()
        .replace('\\', "/");
    let fix_run = run_self_command(
        project_root,
        &[
            "fix".to_string(),
            "--input".to_string(),
            shadow_rel.clone(),
            "--write".to_string(),
        ],
    )?;

    let fixed_bytes = std::fs::read(&shadow_file)
        .with_context(|| format!("read fixed shadow impl: {}", shadow_file.display()))?;
    let changed = fixed_bytes != original_bytes;
    if changed {
        diags.push(diag_warning(
            "WXTAL_REPAIR_QUICKFIX_APPLIED",
            diagnostics::Stage::Run,
            format!(
                "quickfix repair modified {} (fix_exit_code={})",
                shadow_rel, fix_run.exit_code
            ),
            None,
        ));
    }

    let mut best_patchset = PatchSet {
        schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
        patches: Vec::new(),
    };
    let mut best_diff = String::new();
    let mut ok = false;

    if changed && fix_run.exit_code == 0 {
        let mut original_file =
            x07ast::parse_x07ast_json(&original_bytes).context("parse original x07ast")?;
        x07ast::canonicalize_x07ast_file(&mut original_file);
        let (original_value, _original_text) =
            canon_x07ast_file_value_and_text(&mut original_file)?;

        let mut fixed_file =
            x07ast::parse_x07ast_json(&fixed_bytes).context("parse fixed x07ast")?;
        x07ast::canonicalize_x07ast_file(&mut fixed_file);
        let (fixed_value, _fixed_text) = canon_x07ast_file_value_and_text(&mut fixed_file)?;

        let impl_rel = impl_path
            .strip_prefix(project_root)
            .unwrap_or(impl_path)
            .to_string_lossy()
            .replace('\\', "/");
        best_patchset = PatchSet {
            schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
            patches: vec![PatchTarget {
                path: impl_rel.clone(),
                patch: vec![diagnostics::PatchOp::Replace {
                    path: "".to_string(),
                    value: fixed_value.clone(),
                }],
                note: Some("xtal repair quickfix attempt".to_string()),
            }],
        };
        best_diff = unified_json_diff(&impl_rel, &original_value, &fixed_value)?;

        let attempt_artifacts = attempt_dir.join("_artifacts");
        let attempt_test_artifacts = attempt_dir.join("_artifacts").join("test");
        std::fs::create_dir_all(&attempt_artifacts)
            .with_context(|| format!("mkdir: {}", attempt_artifacts.display()))?;
        std::fs::create_dir_all(&attempt_test_artifacts)
            .with_context(|| format!("mkdir: {}", attempt_test_artifacts.display()))?;

        let verify_bounds = baseline_summary
            .get("settings")
            .and_then(|s| s.get("verify_bounds"))
            .and_then(Value::as_object);

        let prove_report_rel = attempt_dir
            .join("prove.report.json")
            .strip_prefix(project_root)
            .unwrap_or(attempt_dir.join("prove.report.json").as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let mut prove_args = vec![
            "verify".to_string(),
            "--prove".to_string(),
            "--entry".to_string(),
            target_entry.to_string(),
        ];
        for root in attempt_module_roots.iter().map(PathBuf::as_path) {
            prove_args.push("--module-root".to_string());
            prove_args.push(root.display().to_string());
        }
        if let Some(bounds) = verify_bounds {
            if let Some(n) = bounds.get("unwind").and_then(Value::as_u64) {
                prove_args.push("--unwind".to_string());
                prove_args.push(n.to_string());
            }
            if let Some(n) = bounds.get("max_bytes_len").and_then(Value::as_u64) {
                prove_args.push("--max-bytes-len".to_string());
                prove_args.push(n.to_string());
            }
            if let Some(n) = bounds.get("input_len_bytes").and_then(Value::as_u64) {
                prove_args.push("--input-len-bytes".to_string());
                prove_args.push(n.to_string());
            }
        }
        prove_args.extend([
            "--artifact-dir".to_string(),
            attempt_artifacts
                .strip_prefix(project_root)
                .unwrap_or(attempt_artifacts.as_path())
                .to_string_lossy()
                .replace('\\', "/"),
            "--report-out".to_string(),
            prove_report_rel.clone(),
            "--quiet-json".to_string(),
        ]);
        let prove_run = run_self_command(project_root, &prove_args)?;
        let prove_ok = prove_run.exit_code == 0;

        let tests_report_rel = attempt_dir
            .join("tests.report.json")
            .strip_prefix(project_root)
            .unwrap_or(attempt_dir.join("tests.report.json").as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let mut test_args = vec![
            "test".to_string(),
            "--all".to_string(),
            "--manifest".to_string(),
            tests_manifest_rel.to_string(),
            "--filter".to_string(),
            op_test_filter.to_string(),
            "--allow-empty".to_string(),
        ];
        for root in attempt_module_roots.iter().map(PathBuf::as_path) {
            test_args.push("--module-root".to_string());
            test_args.push(root.display().to_string());
        }
        test_args.extend([
            "--artifact-dir".to_string(),
            attempt_test_artifacts
                .strip_prefix(project_root)
                .unwrap_or(attempt_test_artifacts.as_path())
                .to_string_lossy()
                .replace('\\', "/"),
            "--report-out".to_string(),
            tests_report_rel.clone(),
            "--quiet-json".to_string(),
        ]);
        let test_run = run_self_command(project_root, &test_args)?;
        let tests_ok = test_run.exit_code == 0;

        if prove_ok && tests_ok {
            ok = true;
        }
    }

    if changed && !ok {
        if let Some((contract_kind, clause_id)) =
            prove_contract_location(project_root, baseline_prove_report_rel)
        {
            let _ = (contract_kind, clause_id);
        }
    }

    write_patchset(&patchset_path, &best_patchset)?;
    util::write_atomic(&diff_path, best_diff.as_bytes())
        .with_context(|| format!("write diff: {}", diff_path.display()))?;

    Ok((best_patchset, best_diff, ok))
}

fn prove_contract_location(
    project_root: &Path,
    prove_report_rel: &str,
) -> Option<(String, String)> {
    let payload = read_prove_contract_payload(project_root, prove_report_rel)?;
    Some((payload.contract_kind, payload.clause_id))
}

#[derive(Debug, Clone)]
struct ProveContractPayload {
    contract_kind: String,
    clause_id: String,
    clause_index: Option<usize>,
    witness: Vec<Value>,
}

fn read_prove_contract_payload(
    project_root: &Path,
    prove_report_rel: &str,
) -> Option<ProveContractPayload> {
    let path = project_root.join(prove_report_rel);
    let bytes = std::fs::read(&path).ok()?;
    let doc: Value = serde_json::from_slice(&bytes).ok()?;
    let contract = doc
        .get("result")
        .and_then(|r| r.get("contract"))
        .and_then(Value::as_object)?;
    let kind = contract.get("contract_kind").and_then(Value::as_str)?;
    let clause_id = contract.get("clause_id").and_then(Value::as_str)?;
    let clause_index = contract
        .get("clause_index")
        .and_then(Value::as_u64)
        .map(|n| n as usize);
    let witness = contract
        .get("witness")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Some(ProveContractPayload {
        contract_kind: kind.to_string(),
        clause_id: clause_id.to_string(),
        clause_index,
        witness,
    })
}

fn unified_json_diff(path: &str, before: &Value, after: &Value) -> Result<String> {
    fn pretty(v: &Value) -> Result<String> {
        let bytes = report_common::canonical_pretty_json_bytes(v)?;
        String::from_utf8(bytes).context("diff JSON must be UTF-8")
    }

    let before = pretty(before)?;
    let after = pretty(after)?;
    if before == after {
        return Ok(String::new());
    }

    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();

    let mut start = 0usize;
    while start < before_lines.len()
        && start < after_lines.len()
        && before_lines[start] == after_lines[start]
    {
        start += 1;
    }

    let mut end_before = before_lines.len();
    let mut end_after = after_lines.len();
    while end_before > start
        && end_after > start
        && before_lines[end_before - 1] == after_lines[end_after - 1]
    {
        end_before -= 1;
        end_after -= 1;
    }

    let context = 3usize;
    let ctx_start = start.saturating_sub(context);
    let ctx_end_before = (end_before + context).min(before_lines.len());
    let ctx_end_after = (end_after + context).min(after_lines.len());

    let old_start = ctx_start + 1;
    let old_count = ctx_end_before.saturating_sub(ctx_start);
    let new_start = ctx_start + 1;
    let new_count = ctx_end_after.saturating_sub(ctx_start);

    let mut out = String::new();
    out.push_str(&format!("--- a/{path}\n"));
    out.push_str(&format!("+++ b/{path}\n"));
    out.push_str(&format!(
        "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
    ));

    for line in &before_lines[ctx_start..start] {
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }
    for line in &before_lines[start..end_before] {
        out.push('-');
        out.push_str(line);
        out.push('\n');
    }
    for line in &after_lines[start..end_after] {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    for line in &before_lines[end_before..ctx_end_before] {
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

fn write_repair_artifacts(
    project_root: &Path,
    baseline_summary: Option<&Value>,
    report: &diagnostics::Report,
    attempts: Option<&[Value]>,
    result_status: Option<&str>,
    extra_meta: &[(&str, Value)],
) -> Result<()> {
    let diag_path = project_root.join(DEFAULT_REPAIR_DIAG_REPORT_PATH);
    if let Some(parent) = diag_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir: {}", parent.display()))?;
    }
    let mut diag_bytes = serde_json::to_vec(report).context("serialize xtal repair report")?;
    if diag_bytes.last() != Some(&b'\n') {
        diag_bytes.push(b'\n');
    }
    util::write_atomic(&diag_path, &diag_bytes)
        .with_context(|| format!("write: {}", diag_path.display()))?;

    let baseline_dir = project_root.join(DEFAULT_REPAIR_BASELINE_DIR);
    if let Some(summary) = baseline_summary {
        std::fs::create_dir_all(&baseline_dir)
            .with_context(|| format!("mkdir: {}", baseline_dir.display()))?;
        let baseline_summary_path = baseline_dir.join("verify.summary.json");
        let bytes = report_common::canonical_pretty_json_bytes(summary)?;
        util::write_atomic(&baseline_summary_path, &bytes)
            .with_context(|| format!("write: {}", baseline_summary_path.display()))?;
    }

    let mut failing_entries: Vec<Value> = Vec::new();
    let mut baseline_verify_ok = false;
    let mut baseline_summary_path = DEFAULT_VERIFY_SUMMARY_PATH.to_string();
    if baseline_dir.join("verify.summary.json").is_file() {
        baseline_summary_path = format!("{DEFAULT_REPAIR_BASELINE_DIR}/verify.summary.json");
    }
    if let Some(summary) = baseline_summary {
        let baseline_entries = baseline_verify_entries(summary);
        let outcome = summary
            .get("results")
            .and_then(|r| r.get("outcome"))
            .and_then(Value::as_str)
            .unwrap_or("fail");
        baseline_verify_ok = outcome != "fail";
        let tests_outcome = summary
            .get("results")
            .and_then(|r| r.get("tests"))
            .and_then(|t| t.get("outcome"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let tests_report = baseline_tests_report_path_rel(summary);
        let failing_test_entries = if tests_outcome == "fail" {
            failing_xtal_entries_from_tests_report(project_root, &tests_report, &baseline_entries)
        } else {
            BTreeSet::new()
        };

        for row in &baseline_entries {
            let reason = if row.prove_raw == "counterexample" {
                Some("prove_counterexample")
            } else if row.coverage_outcome == "fail" {
                Some("coverage_failed")
            } else if failing_test_entries.contains(&row.entry) {
                Some("tests_failed")
            } else {
                None
            };
            let Some(reason) = reason else { continue };
            let mut obj = json!({
                "entry": row.entry,
                "reason": reason,
                "tests_report_path": tests_report,
            });
            if let Some(map) = obj.as_object_mut() {
                if !row.prove_report_path_rel.trim().is_empty() {
                    map.insert(
                        "prove_report_path".to_string(),
                        Value::String(row.prove_report_path_rel.clone()),
                    );
                }
            }
            failing_entries.push(obj);
        }
    }

    let mut result_obj = json!({
        "status": result_status.unwrap_or(if baseline_verify_ok { "no_action_needed" } else { "failed" }),
        "patchset_path": DEFAULT_REPAIR_PATCHSET_PATH,
        "diff_path": DEFAULT_REPAIR_DIFF_PATH,
    });
    if let Some(map) = result_obj.as_object_mut() {
        for (k, v) in extra_meta {
            map.insert(k.to_string(), v.clone());
        }
    }

    let tool_argv: Vec<String> = std::env::args().collect();
    let summary = json!({
        "schema_version": "x07.xtal.repair_summary@0.1.0",
        "generated_at": "2000-01-01T00:00:00Z",
        "tool": {
            "name": "x07",
            "version": env!("CARGO_PKG_VERSION"),
            "command": "xtal repair",
            "argv": tool_argv,
            "env": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH }
        },
        "project": { "root": ".", "manifest": "x07.json" },
        "baseline": {
            "verify_summary_path": baseline_summary_path,
            "verify_ok": baseline_verify_ok,
            "failing_entries": failing_entries,
        },
        "attempts": attempts.unwrap_or(&[]),
        "result": result_obj,
    });

    let summary_path = project_root.join(DEFAULT_REPAIR_SUMMARY_PATH);
    if let Some(parent) = summary_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir: {}", parent.display()))?;
    }
    let bytes = report_common::canonical_pretty_json_bytes(&summary)
        .context("serialize xtal repair summary")?;
    util::write_atomic(&summary_path, &bytes)
        .with_context(|| format!("write: {}", summary_path.display()))?;
    Ok(())
}

fn resolve_gen_index_path(project_root: &Path, arg: Option<&Path>) -> Option<PathBuf> {
    let Some(arg) = arg else {
        let default = project_root.join(DEFAULT_GEN_INDEX_PATH);
        return default.is_file().then_some(default);
    };
    Some(if arg.is_absolute() {
        arg.to_path_buf()
    } else {
        project_root.join(arg)
    })
}

fn cmd_xtal_impl_check(
    machine: &crate::reporting::MachineArgs,
    args: XtalImplCheckArgs,
) -> Result<std::process::ExitCode> {
    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let spec_root = project_root.join(&args.spec_dir);
    let impl_root = project_root.join(&args.impl_dir);

    let mut diags = Vec::new();
    let spec_files = collect_spec_files(&spec_root, &Vec::new(), &mut diags);
    if spec_files.is_empty() {
        diags.push(diag_error(
            "EXTAL_IMPL_NO_SPECS",
            diagnostics::Stage::Parse,
            format!("no spec files found under {}", spec_root.display()),
            None,
        ));
    }

    let mut modules: Vec<(PathBuf, SpecFile)> = Vec::new();
    for spec_path in &spec_files {
        let (doc_opt, lint_diags) = lint_one_spec_file(spec_path)?;
        diags.extend(lint_diags);
        let Some(doc) = doc_opt else {
            continue;
        };
        let spec: SpecFile = match serde_json::from_value(doc) {
            Ok(v) => v,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_SPEC_SCHEMA_INVALID",
                    diagnostics::Stage::Parse,
                    spec_path,
                    None,
                    format!("spec JSON shape is invalid: {err}"),
                ));
                continue;
            }
        };
        modules.push((spec_path.clone(), spec));
    }

    let mut impl_paths: BTreeSet<PathBuf> = BTreeSet::new();
    for (spec_path, spec) in &modules {
        check_impl_module(&impl_root, spec_path, spec, &mut diags, &mut impl_paths)?;
    }

    // Ensure referenced property functions exist and match the spec-derived signatures.
    let mut prop_module_cache: BTreeMap<String, x07ast::X07AstFile> = BTreeMap::new();
    for (spec_path, spec) in &modules {
        check_impl_properties(
            &impl_root,
            spec_path,
            spec,
            &mut diags,
            &mut impl_paths,
            &mut prop_module_cache,
        )?;
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diags);
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report.meta.insert(
        "spec_dir".to_string(),
        Value::String(args.spec_dir.display().to_string()),
    );
    report.meta.insert(
        "impl_dir".to_string(),
        Value::String(args.impl_dir.display().to_string()),
    );
    let spec_digests: Vec<Value> = spec_files
        .iter()
        .filter_map(|p| file_digest_value(p))
        .collect();
    report
        .meta
        .insert("spec_digests".to_string(), Value::Array(spec_digests));
    let impl_digests: Vec<Value> = impl_paths
        .iter()
        .filter_map(|p| file_digest_value(p))
        .collect();
    report
        .meta
        .insert("impl_digests".to_string(), Value::Array(impl_digests));
    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_xtal_impl_sync(
    machine: &crate::reporting::MachineArgs,
    args: XtalImplSyncArgs,
) -> Result<std::process::ExitCode> {
    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let spec_root = project_root.join(&args.spec_dir);
    let impl_root = project_root.join(&args.impl_dir);
    let patchset_out = args.patchset_out.as_deref().map(|p| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            project_root.join(p)
        }
    });

    let mut diags = Vec::new();
    let spec_files = collect_spec_files(&spec_root, &Vec::new(), &mut diags);
    if spec_files.is_empty() {
        diags.push(diag_error(
            "EXTAL_IMPL_NO_SPECS",
            diagnostics::Stage::Parse,
            format!("no spec files found under {}", spec_root.display()),
            None,
        ));
    }

    let mut modules: Vec<(PathBuf, SpecFile)> = Vec::new();
    for spec_path in &spec_files {
        let (doc_opt, lint_diags) = lint_one_spec_file(spec_path)?;
        diags.extend(lint_diags);
        let Some(doc) = doc_opt else {
            continue;
        };
        let spec: SpecFile = match serde_json::from_value(doc) {
            Ok(v) => v,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_SPEC_SCHEMA_INVALID",
                    diagnostics::Stage::Parse,
                    spec_path,
                    None,
                    format!("spec JSON shape is invalid: {err}"),
                ));
                continue;
            }
        };
        modules.push((spec_path.clone(), spec));
    }

    let mut ids_ok = true;
    for (spec_path, spec) in &modules {
        for (op_idx, op) in spec.operations.iter().enumerate() {
            let core =
                collect_contract_core_clauses(spec_path, &spec.module_id, op_idx, op, &mut diags)?;
            for c in &core.requires {
                if c.clause.id.as_deref().unwrap_or("").trim().is_empty() {
                    ids_ok = false;
                    diags.push(spec_error(
                        "EXTAL_SPEC_IDS_REQUIRED_FOR_SYNC",
                        diagnostics::Stage::Lint,
                        spec_path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("{}/id", c.spec_ptr),
                        }),
                        "contract-core clause missing id (run `x07 xtal spec fmt --inject-ids --write`)",
                    ));
                }
            }
            for c in &core.ensures {
                if c.clause.id.as_deref().unwrap_or("").trim().is_empty() {
                    ids_ok = false;
                    diags.push(spec_error(
                        "EXTAL_SPEC_IDS_REQUIRED_FOR_SYNC",
                        diagnostics::Stage::Lint,
                        spec_path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("{}/id", c.spec_ptr),
                        }),
                        "contract-core clause missing id (run `x07 xtal spec fmt --inject-ids --write`)",
                    ));
                }
            }
            for c in &core.invariant {
                if c.clause.id.as_deref().unwrap_or("").trim().is_empty() {
                    ids_ok = false;
                    diags.push(spec_error(
                        "EXTAL_SPEC_IDS_REQUIRED_FOR_SYNC",
                        diagnostics::Stage::Lint,
                        spec_path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("{}/id", c.spec_ptr),
                        }),
                        "contract-core clause missing id (run `x07 xtal spec fmt --inject-ids --write`)",
                    ));
                }
            }
        }
    }

    let mut planned_edits: Vec<PlannedImplEdit> = Vec::new();
    if ids_ok {
        planned_edits = plan_impl_sync_edits(&impl_root, &modules, &mut diags)?;
        planned_edits.sort_by(|a, b| a.path.cmp(&b.path));
    }

    if let Some(patchset_path) = patchset_out.as_ref() {
        let mut patch_targets = Vec::new();
        for edit in &planned_edits {
            let rel = edit
                .path
                .strip_prefix(&project_root)
                .unwrap_or(&edit.path)
                .display()
                .to_string();
            patch_targets.push(PatchTarget {
                path: rel,
                patch: vec![diagnostics::PatchOp::Replace {
                    path: "".to_string(),
                    value: edit.new_value.clone(),
                }],
                note: None,
            });
        }
        let patchset = PatchSet {
            schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
            patches: patch_targets,
        };
        let patchset_value = serde_json::to_value(patchset)?;
        let bytes = report_common::canonical_pretty_json_bytes(&patchset_value)?;
        util::write_atomic(patchset_path, &bytes)
            .with_context(|| format!("write patchset: {}", patchset_path.display()))?;
    } else if args.write {
        for edit in &planned_edits {
            util::write_atomic(&edit.path, &edit.new_text)
                .with_context(|| format!("write: {}", edit.path.display()))?;
        }
    } else if !planned_edits.is_empty() {
        diags.push(diag_error(
            "EXTAL_IMPL_SYNC_REQUIRED",
            diagnostics::Stage::Lint,
            format!(
                "implementation sync would change {} file(s) (rerun with --write or --patchset-out)",
                planned_edits.len()
            ),
            None,
        ));
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diags);
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report.meta.insert(
        "spec_dir".to_string(),
        Value::String(args.spec_dir.display().to_string()),
    );
    report.meta.insert(
        "impl_dir".to_string(),
        Value::String(args.impl_dir.display().to_string()),
    );
    let spec_digests: Vec<Value> = spec_files
        .iter()
        .filter_map(|p| file_digest_value(p))
        .collect();
    report
        .meta
        .insert("spec_digests".to_string(), Value::Array(spec_digests));
    if let Some(patchset_path) = patchset_out.as_ref() {
        let display = patchset_path
            .strip_prefix(&project_root)
            .unwrap_or(patchset_path)
            .display()
            .to_string();
        report
            .meta
            .insert("patchset_path".to_string(), Value::String(display));
        report.meta.insert(
            "patch_count".to_string(),
            Value::Number(serde_json::Number::from(planned_edits.len())),
        );
        let mut touched = Vec::new();
        for edit in &planned_edits {
            let rel = edit
                .path
                .strip_prefix(&project_root)
                .unwrap_or(&edit.path)
                .display()
                .to_string();
            touched.push(Value::String(rel));
        }
        touched.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        touched.dedup();
        report
            .meta
            .insert("touched_paths".to_string(), Value::Array(touched));
    }
    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

#[derive(Debug, Clone)]
struct PlannedImplEdit {
    path: PathBuf,
    new_value: Value,
    new_text: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PropExpectedParam {
    arg_name: String,
    ty: String,
    brand: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PropRequirement {
    symbol: String,
    op_id: String,
    params: Vec<PropExpectedParam>,
}

fn plan_impl_sync_edits(
    impl_root: &Path,
    modules: &[(PathBuf, SpecFile)],
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<Vec<PlannedImplEdit>> {
    let mut spec_by_module_id: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, (_path, spec)) in modules.iter().enumerate() {
        if spec_by_module_id.contains_key(&spec.module_id) {
            continue;
        }
        spec_by_module_id.insert(spec.module_id.clone(), idx);
    }

    let mut prop_reqs_by_module_id: BTreeMap<String, BTreeMap<String, PropRequirement>> =
        BTreeMap::new();
    for (_spec_path, spec) in modules {
        for op in &spec.operations {
            let op_id = op.id.as_deref().unwrap_or(op.name.as_str()).to_string();
            for prop in &op.ensures_props {
                let prop_symbol = prop.prop.trim();
                if prop_symbol.is_empty() {
                    continue;
                }
                if x07c::validate::validate_symbol(prop_symbol).is_err() {
                    continue;
                }
                let Ok((prop_module_id, _local)) = parse_symbol_to_module_and_local(prop_symbol)
                else {
                    continue;
                };
                let expected_args: Vec<&str> = prop
                    .args
                    .iter()
                    .map(|a| a.trim())
                    .filter(|a| !a.is_empty())
                    .collect();
                if expected_args.is_empty() {
                    continue;
                }
                let mut params = Vec::new();
                let mut ok = true;
                for arg_name in &expected_args {
                    let Some(op_param) = op.params.iter().find(|p| p.name == *arg_name) else {
                        ok = false;
                        break;
                    };
                    params.push(PropExpectedParam {
                        arg_name: (*arg_name).to_string(),
                        ty: op_param.ty.clone(),
                        brand: op_param.brand.clone(),
                    });
                }
                if !ok {
                    continue;
                }

                let module_entry = prop_reqs_by_module_id.entry(prop_module_id).or_default();
                match module_entry.get(prop_symbol) {
                    None => {
                        module_entry.insert(
                            prop_symbol.to_string(),
                            PropRequirement {
                                symbol: prop_symbol.to_string(),
                                op_id: op_id.clone(),
                                params,
                            },
                        );
                    }
                    Some(existing) => {
                        if existing.params != params {
                            diags.push(diag_error(
                                "EXTAL_IMPL_PROP_SIGNATURE_MISMATCH",
                                diagnostics::Stage::Lint,
                                format!(
                                    "Property \"{prop_symbol}\" referenced by op \"{op_id}\" has a signature mismatch. Expected: conflicting_spec."
                                ),
                                None,
                            ));
                        }
                    }
                }
            }
        }
    }

    let mut module_ids: BTreeSet<String> = BTreeSet::new();
    module_ids.extend(spec_by_module_id.keys().cloned());
    module_ids.extend(prop_reqs_by_module_id.keys().cloned());

    let mut edits = Vec::new();
    for module_id in module_ids {
        let impl_path = module_id_to_impl_path(impl_root, &module_id);
        let original_bytes = if impl_path.is_file() {
            match std::fs::read(&impl_path) {
                Ok(bytes) => bytes,
                Err(err) => {
                    diags.push(impl_error(
                        "EXTAL_IMPL_IO_READ_FAILED",
                        diagnostics::Stage::Parse,
                        &impl_path,
                        None,
                        format!("cannot read file: {err}"),
                    ));
                    continue;
                }
            }
        } else {
            Vec::new()
        };

        let mut file = if impl_path.is_file() {
            match x07ast::parse_x07ast_json(&original_bytes) {
                Ok(v) => v,
                Err(err) => {
                    diags.push(impl_error(
                        "EXTAL_IMPL_X07AST_PARSE",
                        diagnostics::Stage::Parse,
                        &impl_path,
                        None,
                        err.to_string(),
                    ));
                    continue;
                }
            }
        } else {
            x07ast::X07AstFile {
                schema_version: x07_contracts::X07AST_SCHEMA_VERSION.to_string(),
                kind: x07ast::X07AstKind::Module,
                module_id: module_id.clone(),
                imports: BTreeSet::new(),
                exports: BTreeSet::new(),
                functions: Vec::new(),
                async_functions: Vec::new(),
                extern_functions: Vec::new(),
                solve: None,
                meta: BTreeMap::new(),
            }
        };

        if file.kind != x07ast::X07AstKind::Module {
            diags.push(impl_error(
                "EXTAL_IMPL_KIND_UNSUPPORTED",
                diagnostics::Stage::Parse,
                &impl_path,
                Some(diagnostics::Location::X07Ast {
                    ptr: "/kind".to_string(),
                }),
                format!("expected kind=\"module\" (got {:?})", file.kind),
            ));
            continue;
        }
        if file.module_id != module_id {
            diags.push(impl_error(
                "EXTAL_IMPL_MODULE_ID_MISMATCH",
                diagnostics::Stage::Parse,
                &impl_path,
                Some(diagnostics::Location::X07Ast {
                    ptr: "/module_id".to_string(),
                }),
                format!(
                    "module_id mismatch: expected {:?} got {:?}",
                    module_id, file.module_id
                ),
            ));
            continue;
        }

        if let Some(idx) = spec_by_module_id.get(&module_id).copied() {
            let (spec_path, spec) = &modules[idx];
            sync_one_impl_module(spec_path, spec, &mut file, diags)?;
        }
        if let Some(reqs) = prop_reqs_by_module_id.get(&module_id) {
            sync_prop_defns(&impl_path, reqs.values(), &mut file, diags);
        }

        let (new_value, new_text) = canon_x07ast_file_value_and_text(&mut file)?;
        if new_text != original_bytes {
            edits.push(PlannedImplEdit {
                path: impl_path,
                new_value,
                new_text,
            });
        }
    }

    Ok(edits)
}

fn canon_x07ast_file_value_and_text(file: &mut x07ast::X07AstFile) -> Result<(Value, Vec<u8>)> {
    x07ast::canonicalize_x07ast_file(file);
    let mut v = x07ast::x07ast_file_to_value(file);
    x07ast::canon_value_jcs(&mut v);
    let mut bytes = serde_json::to_string(&v)?.into_bytes();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok((v, bytes))
}

#[derive(Debug, Clone)]
struct CoreContractClauses {
    requires: Vec<CoreClauseItem>,
    ensures: Vec<CoreClauseItem>,
    invariant: Vec<CoreClauseItem>,
}

#[derive(Debug, Clone)]
struct CoreClauseItem {
    spec_ptr: String,
    clause: x07ast::ContractClauseAst,
}

#[derive(Debug, Clone, Copy)]
enum ContractClauseKind {
    Requires,
    Ensures,
    Invariant,
}

fn collect_contract_core_clauses(
    spec_path: &Path,
    module_id: &str,
    op_idx: usize,
    op: &SpecOperation,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<CoreContractClauses> {
    let mut core = CoreContractClauses {
        requires: Vec::new(),
        ensures: Vec::new(),
        invariant: Vec::new(),
    };

    for (c_idx, clause) in op.requires.iter().enumerate() {
        let spec_ptr = format!("/operations/{op_idx}/requires/{c_idx}");
        let Some(clause) = parse_spec_clause_to_contract_ast(spec_path, &spec_ptr, clause, diags)
        else {
            continue;
        };
        if contract_clause_is_contract_core(module_id, op, ContractClauseKind::Requires, &clause)? {
            core.requires.push(CoreClauseItem { spec_ptr, clause });
        }
    }
    for (c_idx, clause) in op.ensures.iter().enumerate() {
        let spec_ptr = format!("/operations/{op_idx}/ensures/{c_idx}");
        let Some(clause) = parse_spec_clause_to_contract_ast(spec_path, &spec_ptr, clause, diags)
        else {
            continue;
        };
        if contract_clause_is_contract_core(module_id, op, ContractClauseKind::Ensures, &clause)? {
            core.ensures.push(CoreClauseItem { spec_ptr, clause });
        }
    }
    for (c_idx, clause) in op.invariant.iter().enumerate() {
        let spec_ptr = format!("/operations/{op_idx}/invariant/{c_idx}");
        let Some(clause) = parse_spec_clause_to_contract_ast(spec_path, &spec_ptr, clause, diags)
        else {
            continue;
        };
        if contract_clause_is_contract_core(module_id, op, ContractClauseKind::Invariant, &clause)?
        {
            core.invariant.push(CoreClauseItem { spec_ptr, clause });
        }
    }

    Ok(core)
}

fn parse_spec_clause_to_contract_ast(
    spec_path: &Path,
    base_ptr: &str,
    clause: &SpecClause,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Option<x07ast::ContractClauseAst> {
    let expr = match x07c::ast::expr_from_json(&clause.expr) {
        Ok(e) => e,
        Err(err) => {
            diags.push(spec_error(
                "EXTAL_SPEC_CONTRACT_EXPR_PARSE",
                diagnostics::Stage::Parse,
                spec_path,
                Some(diagnostics::Location::X07Ast {
                    ptr: format!("{base_ptr}/expr"),
                }),
                err,
            ));
            return None;
        }
    };

    let mut witness = Vec::new();
    for (w_idx, w) in clause.witness.iter().enumerate() {
        match x07c::ast::expr_from_json(w) {
            Ok(e) => witness.push(e),
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_SPEC_CONTRACT_WITNESS_INVALID",
                    diagnostics::Stage::Parse,
                    spec_path,
                    Some(diagnostics::Location::X07Ast {
                        ptr: format!("{base_ptr}/witness/{w_idx}"),
                    }),
                    err,
                ));
                return None;
            }
        }
    }

    Some(x07ast::ContractClauseAst {
        id: clause.id.clone(),
        expr,
        witness,
    })
}

fn contract_clause_is_contract_core(
    module_id: &str,
    op: &SpecOperation,
    kind: ContractClauseKind,
    clause: &x07ast::ContractClauseAst,
) -> Result<bool> {
    if !is_supported_ty(&op.result) {
        return Ok(false);
    }
    let params = op
        .params
        .iter()
        .map(|p| x07ast::AstFunctionParam {
            name: p.name.clone(),
            ty: x07ast::TypeRef::Named(p.ty.clone()),
            brand: p.brand.clone(),
        })
        .collect();

    let mut f = x07ast::AstFunctionDef {
        name: op.name.clone(),
        type_params: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        invariant: Vec::new(),
        loop_contracts: Vec::new(),
        params,
        result: x07ast::TypeRef::Named(op.result.clone()),
        result_brand: op.result_brand.clone(),
        body: int_expr(0),
    };
    match kind {
        ContractClauseKind::Requires => f.requires.push(clause.clone()),
        ContractClauseKind::Ensures => f.ensures.push(clause.clone()),
        ContractClauseKind::Invariant => f.invariant.push(clause.clone()),
    }

    let file = x07ast::X07AstFile {
        schema_version: x07_contracts::X07AST_SCHEMA_VERSION.to_string(),
        kind: x07ast::X07AstKind::Module,
        module_id: module_id.to_string(),
        imports: BTreeSet::new(),
        exports: BTreeSet::new(),
        functions: vec![f],
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: None,
        meta: BTreeMap::new(),
    };

    let report = x07c::typecheck::typecheck_file_local(
        &file,
        &x07c::typecheck::TypecheckOptions {
            mode: x07c::typecheck::TypecheckMode::ContractsOnly,
            compat: x07c::compat::Compat::default(),
        },
    );
    Ok(report
        .diagnostics
        .iter()
        .all(|d| d.severity != diagnostics::Severity::Error))
}

fn check_impl_module(
    impl_root: &Path,
    spec_path: &Path,
    spec: &SpecFile,
    diags: &mut Vec<diagnostics::Diagnostic>,
    impl_paths: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let impl_path = module_id_to_impl_path(impl_root, &spec.module_id);
    if !impl_path.is_file() {
        diags.push(impl_error(
            "EXTAL_IMPL_MODULE_MISSING",
            diagnostics::Stage::Lint,
            &impl_path,
            None,
            format!(
                "missing impl module for spec module_id {:?} (expected {})",
                spec.module_id,
                impl_path.display()
            ),
        ));
        return Ok(());
    }
    impl_paths.insert(impl_path.clone());

    let bytes = match std::fs::read(&impl_path) {
        Ok(b) => b,
        Err(err) => {
            diags.push(impl_error(
                "EXTAL_IMPL_IO_READ_FAILED",
                diagnostics::Stage::Parse,
                &impl_path,
                None,
                format!("cannot read file: {err}"),
            ));
            return Ok(());
        }
    };
    let file = match x07ast::parse_x07ast_json(&bytes) {
        Ok(f) => f,
        Err(err) => {
            diags.push(impl_error(
                "EXTAL_IMPL_X07AST_PARSE",
                diagnostics::Stage::Parse,
                &impl_path,
                None,
                err.to_string(),
            ));
            return Ok(());
        }
    };

    if file.kind != x07ast::X07AstKind::Module {
        diags.push(impl_error(
            "EXTAL_IMPL_KIND_UNSUPPORTED",
            diagnostics::Stage::Lint,
            &impl_path,
            Some(diagnostics::Location::X07Ast {
                ptr: "/kind".to_string(),
            }),
            format!("expected kind=\"module\" (got {:?})", file.kind),
        ));
    }
    if file.module_id != spec.module_id {
        diags.push(impl_error(
            "EXTAL_IMPL_MODULE_ID_MISMATCH",
            diagnostics::Stage::Lint,
            &impl_path,
            Some(diagnostics::Location::X07Ast {
                ptr: "/module_id".to_string(),
            }),
            format!(
                "module_id mismatch: expected {:?} got {:?}",
                spec.module_id, file.module_id
            ),
        ));
    }

    for (op_idx, op) in spec.operations.iter().enumerate() {
        if op.name.trim().is_empty() {
            continue;
        }
        if !file.exports.contains(&op.name) {
            diags.push(impl_error(
                "EXTAL_IMPL_EXPORT_MISSING",
                diagnostics::Stage::Lint,
                &impl_path,
                Some(diagnostics::Location::X07Ast {
                    ptr: "/exports".to_string(),
                }),
                format!("operation is not exported: {:?}", op.name),
            ));
        }

        let Some(defn) = file.functions.iter().find(|f| f.name == op.name) else {
            diags.push(impl_error(
                "EXTAL_IMPL_SIGNATURE_MISMATCH",
                diagnostics::Stage::Lint,
                &impl_path,
                None,
                format!("missing defn for operation {:?}", op.name),
            ));
            continue;
        };

        let (sig_ok, mut sig_warns, sig_errors) = compare_defn_signature_to_spec(op, defn);
        diags.append(&mut sig_warns);
        if !sig_errors.is_empty() {
            diags.push(impl_error(
                "EXTAL_IMPL_SIGNATURE_MISMATCH",
                diagnostics::Stage::Lint,
                &impl_path,
                None,
                format!(
                    "signature mismatch for {:?}: {}",
                    op.name,
                    sig_errors.join("; ")
                ),
            ));
        }
        if !sig_ok {
            continue;
        }

        let core = collect_contract_core_clauses(spec_path, &spec.module_id, op_idx, op, diags)?;
        check_contract_alignment(&impl_path, op, defn, &core, diags);
    }

    Ok(())
}

fn check_impl_properties(
    impl_root: &Path,
    _spec_path: &Path,
    spec: &SpecFile,
    diags: &mut Vec<diagnostics::Diagnostic>,
    impl_paths: &mut BTreeSet<PathBuf>,
    cache: &mut BTreeMap<String, x07ast::X07AstFile>,
) -> Result<()> {
    for op in &spec.operations {
        let op_id = op.id.as_deref().unwrap_or(op.name.as_str());
        for prop in &op.ensures_props {
            let prop_symbol = prop.prop.trim();
            if prop_symbol.is_empty() {
                continue;
            }
            let Ok((prop_module_id, _local)) = parse_symbol_to_module_and_local(prop_symbol) else {
                continue;
            };

            let impl_path = module_id_to_impl_path(impl_root, &prop_module_id);
            if !impl_path.is_file() {
                diags.push(impl_error(
                    "EXTAL_IMPL_PROP_MODULE_MISSING",
                    diagnostics::Stage::Lint,
                    &impl_path,
                    None,
                    format!(
                        "Property \"{prop_symbol}\" referenced by op \"{op_id}\" was not found: module \"{prop_module_id}\" does not exist at \"{}\".",
                        impl_path.display(),
                    ),
                ));
                continue;
            }
            impl_paths.insert(impl_path.clone());

            if !cache.contains_key(&prop_module_id) {
                let bytes = match std::fs::read(&impl_path) {
                    Ok(b) => b,
                    Err(err) => {
                        diags.push(impl_error(
                            "EXTAL_IMPL_IO_READ_FAILED",
                            diagnostics::Stage::Parse,
                            &impl_path,
                            None,
                            format!("cannot read file: {err}"),
                        ));
                        continue;
                    }
                };
                let file = match x07ast::parse_x07ast_json(&bytes) {
                    Ok(f) => f,
                    Err(err) => {
                        diags.push(impl_error(
                            "EXTAL_IMPL_X07AST_PARSE",
                            diagnostics::Stage::Parse,
                            &impl_path,
                            None,
                            err.to_string(),
                        ));
                        continue;
                    }
                };
                cache.insert(prop_module_id.clone(), file);
            };
            let Some(file) = cache.get(&prop_module_id) else {
                continue;
            };

            if !file.exports.contains(prop_symbol) {
                diags.push(impl_error(
                    "EXTAL_IMPL_PROP_EXPORT_MISSING",
                    diagnostics::Stage::Lint,
                    &impl_path,
                    Some(diagnostics::Location::X07Ast {
                        ptr: "/exports".to_string(),
                    }),
                    format!(
                        "Property \"{prop_symbol}\" referenced by op \"{op_id}\" exists but is not exported by \"{}\".",
                        impl_path.display(),
                    ),
                ));
                continue;
            }

            let Some(defn) = file.functions.iter().find(|f| f.name == prop_symbol) else {
                diags.push(impl_error(
                    "EXTAL_IMPL_PROP_DEFN_MISSING",
                    diagnostics::Stage::Lint,
                    &impl_path,
                    None,
                    format!(
                        "Property \"{prop_symbol}\" referenced by op \"{op_id}\" is exported by \"{}\" but no matching defn was found.",
                        impl_path.display(),
                    ),
                ));
                continue;
            };

            let expected_args: Vec<&str> = prop
                .args
                .iter()
                .map(|a| a.trim())
                .filter(|a| !a.is_empty())
                .collect();

            if defn.params.len() != expected_args.len() {
                diags.push(impl_error(
                    "EXTAL_IMPL_PROP_SIGNATURE_MISMATCH",
                    diagnostics::Stage::Lint,
                    &impl_path,
                    None,
                    format!(
                        "Property \"{prop_symbol}\" referenced by op \"{op_id}\" has a signature mismatch in \"{}\". Expected: param_count.",
                        impl_path.display(),
                    ),
                ));
                continue;
            }

            let mut mismatch_reason: Option<String> = None;
            for (idx, arg_name) in expected_args.iter().enumerate() {
                let got = &defn.params[idx];
                let Some(want) = op.params.iter().find(|p| p.name == *arg_name) else {
                    mismatch_reason = Some(format!("arg_mapping:{arg_name}"));
                    break;
                };
                if !type_ref_matches_spec(&got.ty, &want.ty) {
                    mismatch_reason = Some(format!("param_type:{arg_name}"));
                    break;
                }
                if got.brand != want.brand {
                    mismatch_reason = Some(format!("param_brand:{arg_name}"));
                    break;
                }
            }

            if let Some(reason) = mismatch_reason {
                diags.push(impl_error(
                    "EXTAL_IMPL_PROP_SIGNATURE_MISMATCH",
                    diagnostics::Stage::Lint,
                    &impl_path,
                    None,
                    format!(
                        "Property \"{prop_symbol}\" referenced by op \"{op_id}\" has a signature mismatch in \"{}\". Expected: {reason}.",
                        impl_path.display(),
                    ),
                ));
                continue;
            }

            if defn.result.as_named().unwrap_or("") != "bytes" {
                let got = defn
                    .result
                    .as_named()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{:?}", defn.result));
                diags.push(impl_error(
                    "EXTAL_IMPL_PROP_RESULT_TYPE_MISMATCH",
                    diagnostics::Stage::Lint,
                    &impl_path,
                    None,
                    format!(
                        "Property \"{prop_symbol}\" referenced by op \"{op_id}\" must return bytes_status_v1 but returns {got} in \"{}\".",
                        impl_path.display(),
                    ),
                ));
                continue;
            }
        }
    }
    Ok(())
}

fn sync_one_impl_module(
    spec_path: &Path,
    spec: &SpecFile,
    file: &mut x07ast::X07AstFile,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<()> {
    let mut required_exports: BTreeSet<String> = BTreeSet::new();
    for op in &spec.operations {
        if !op.name.trim().is_empty() {
            required_exports.insert(op.name.clone());
        }
    }

    for name in &required_exports {
        file.exports.insert(name.clone());
    }

    for (op_idx, op) in spec.operations.iter().enumerate() {
        if op.name.trim().is_empty() {
            continue;
        }
        let core = collect_contract_core_clauses(spec_path, &spec.module_id, op_idx, op, diags)?;
        let Some(defn_idx) = file.functions.iter().position(|f| f.name == op.name) else {
            file.functions.push(stub_defn_from_spec(op, &core, diags));
            continue;
        };

        if !defn_signature_exact_matches_spec(op, &file.functions[defn_idx]) {
            continue;
        }

        let desired_requires: Vec<x07ast::ContractClauseAst> =
            core.requires.iter().map(|c| c.clause.clone()).collect();
        let desired_ensures: Vec<x07ast::ContractClauseAst> =
            core.ensures.iter().map(|c| c.clause.clone()).collect();
        let desired_invariant: Vec<x07ast::ContractClauseAst> =
            core.invariant.iter().map(|c| c.clause.clone()).collect();

        file.functions[defn_idx].requires = desired_requires;
        upsert_contract_clauses(&mut file.functions[defn_idx].ensures, &desired_ensures);
        upsert_contract_clauses(&mut file.functions[defn_idx].invariant, &desired_invariant);
    }
    Ok(())
}

fn sync_prop_defns<'a>(
    impl_path: &Path,
    reqs: impl IntoIterator<Item = &'a PropRequirement>,
    file: &mut x07ast::X07AstFile,
    diags: &mut Vec<diagnostics::Diagnostic>,
) {
    fn make_unique_local_name(used: &mut BTreeSet<String>, desired: &str) -> String {
        let base = if x07c::validate::validate_local_name(desired).is_ok() {
            desired.to_string()
        } else {
            "arg".to_string()
        };
        if used.insert(base.clone()) {
            return base;
        }
        let mut n = 1u32;
        loop {
            let cand = format!("{base}_v{n}");
            if used.insert(cand.clone()) {
                return cand;
            }
            n = n.saturating_add(1);
        }
    }

    let stub_body = list_expr([
        ident("std.test.status_fail"),
        list_expr([ident("std.test.code_fail_generic")]),
    ]);

    for req in reqs {
        if req.symbol.trim().is_empty() {
            continue;
        }
        file.exports.insert(req.symbol.clone());

        let expected_len = req.params.len();
        if expected_len == 0 {
            continue;
        }

        if let Some(defn) = file.functions.iter_mut().find(|f| f.name == req.symbol) {
            if defn.params.len() > expected_len {
                defn.params.truncate(expected_len);
            }
            if defn.params.len() < expected_len {
                let mut used: BTreeSet<String> =
                    defn.params.iter().map(|p| p.name.clone()).collect();
                for want in req.params.iter().skip(defn.params.len()) {
                    let name = make_unique_local_name(&mut used, &want.arg_name);
                    defn.params.push(x07ast::AstFunctionParam {
                        name,
                        ty: x07ast::TypeRef::Named(want.ty.clone()),
                        brand: want.brand.clone(),
                    });
                }
            }
            for (idx, want) in req.params.iter().enumerate() {
                if let Some(param) = defn.params.get_mut(idx) {
                    param.ty = x07ast::TypeRef::Named(want.ty.clone());
                    param.brand = want.brand.clone();
                }
            }
            defn.result = x07ast::TypeRef::Named("bytes".to_string());
            defn.result_brand = None;
            continue;
        }

        if file.async_functions.iter().any(|f| f.name == req.symbol)
            || file.extern_functions.iter().any(|f| f.name == req.symbol)
        {
            diags.push(impl_error(
                "EXTAL_IMPL_PROP_DEFN_MISSING",
                diagnostics::Stage::Lint,
                impl_path,
                None,
                format!(
                    "Property \"{}\" referenced by op \"{}\" exists in \"{}\" but is not a defn.",
                    req.symbol,
                    req.op_id,
                    impl_path.display()
                ),
            ));
            continue;
        }

        file.imports.insert("std.test".to_string());
        let mut used = BTreeSet::new();
        let params = req
            .params
            .iter()
            .map(|p| x07ast::AstFunctionParam {
                name: make_unique_local_name(&mut used, &p.arg_name),
                ty: x07ast::TypeRef::Named(p.ty.clone()),
                brand: p.brand.clone(),
            })
            .collect();
        file.functions.push(x07ast::AstFunctionDef {
            name: req.symbol.clone(),
            type_params: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            invariant: Vec::new(),
            loop_contracts: Vec::new(),
            params,
            result: x07ast::TypeRef::Named("bytes".to_string()),
            result_brand: None,
            body: stub_body.clone(),
        });
    }
}

fn stub_defn_from_spec(
    op: &SpecOperation,
    core: &CoreContractClauses,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> x07ast::AstFunctionDef {
    let params = op
        .params
        .iter()
        .map(|p| x07ast::AstFunctionParam {
            name: p.name.clone(),
            ty: x07ast::TypeRef::Named(p.ty.clone()),
            brand: p.brand.clone(),
        })
        .collect();

    let requires: Vec<x07ast::ContractClauseAst> =
        core.requires.iter().map(|c| c.clause.clone()).collect();
    let ensures: Vec<x07ast::ContractClauseAst> =
        core.ensures.iter().map(|c| c.clause.clone()).collect();
    let invariant: Vec<x07ast::ContractClauseAst> =
        core.invariant.iter().map(|c| c.clause.clone()).collect();

    let body = default_body_for_ty(&op.result, diags);

    x07ast::AstFunctionDef {
        name: op.name.clone(),
        type_params: Vec::new(),
        requires,
        ensures,
        invariant,
        loop_contracts: Vec::new(),
        params,
        result: x07ast::TypeRef::Named(op.result.clone()),
        result_brand: op.result_brand.clone(),
        body,
    }
}

fn default_body_for_ty(ty: &str, diags: &mut Vec<diagnostics::Diagnostic>) -> Expr {
    match ty.trim() {
        "i32" => int_expr(0),
        "bytes" => list_expr([ident("bytes.empty")]),
        "bytes_view" => list_expr([ident("bytes.view"), list_expr([ident("bytes.empty")])]),
        other => {
            diags.push(diag_error(
                "EXTAL_IMPL_UNSUPPORTED_TY",
                diagnostics::Stage::Lower,
                format!("unsupported type for stub body: {other:?}"),
                None,
            ));
            int_expr(0)
        }
    }
}

fn module_id_to_impl_path(impl_root: &Path, module_id: &str) -> PathBuf {
    let rel = module_id.replace('.', "/");
    impl_root.join(format!("{rel}.x07.json"))
}

fn parse_symbol_to_module_and_local(symbol: &str) -> Result<(String, String)> {
    let symbol = symbol.trim();
    if symbol.is_empty() {
        anyhow::bail!("symbol must be non-empty");
    }
    if let Err(msg) = x07c::validate::validate_symbol(symbol) {
        anyhow::bail!("{msg}");
    }
    let Some((module_id, local)) = symbol.rsplit_once('.') else {
        anyhow::bail!("symbol must contain '.'");
    };
    Ok((module_id.to_string(), local.to_string()))
}

fn compare_defn_signature_to_spec(
    op: &SpecOperation,
    defn: &x07ast::AstFunctionDef,
) -> (bool, Vec<diagnostics::Diagnostic>, Vec<String>) {
    let mut ok = true;
    let mut warns = Vec::new();
    let mut errors = Vec::new();

    if defn.params.len() != op.params.len() {
        ok = false;
        errors.push(format!(
            "param count: expected {} got {}",
            op.params.len(),
            defn.params.len()
        ));
    }

    for (idx, (spec_p, impl_p)) in op.params.iter().zip(defn.params.iter()).enumerate() {
        if spec_p.name != impl_p.name {
            warns.push(diag_warning(
                "WXTAL_IMPL_PARAM_NAME_MISMATCH",
                diagnostics::Stage::Lint,
                format!(
                    "param name mismatch at index {idx}: expected {:?} got {:?}",
                    spec_p.name, impl_p.name
                ),
                None,
            ));
        }
        if !type_ref_matches_spec(&impl_p.ty, &spec_p.ty) {
            ok = false;
            errors.push(format!(
                "param {idx} type: expected {:?} got {:?}",
                spec_p.ty, impl_p.ty
            ));
        }
        if spec_p.brand != impl_p.brand {
            ok = false;
            errors.push(format!(
                "param {idx} brand: expected {:?} got {:?}",
                spec_p.brand, impl_p.brand
            ));
        }
    }

    if !type_ref_matches_spec(&defn.result, &op.result) {
        ok = false;
        errors.push(format!(
            "result type: expected {:?} got {:?}",
            op.result, defn.result
        ));
    }
    if op.result_brand != defn.result_brand {
        ok = false;
        errors.push(format!(
            "result brand: expected {:?} got {:?}",
            op.result_brand, defn.result_brand
        ));
    }

    (ok, warns, errors)
}

fn defn_signature_exact_matches_spec(op: &SpecOperation, defn: &x07ast::AstFunctionDef) -> bool {
    if defn.params.len() != op.params.len() {
        return false;
    }
    for (spec_p, impl_p) in op.params.iter().zip(defn.params.iter()) {
        if spec_p.name != impl_p.name {
            return false;
        }
        if spec_p.brand != impl_p.brand {
            return false;
        }
        if !type_ref_matches_spec(&impl_p.ty, &spec_p.ty) {
            return false;
        }
    }
    if !type_ref_matches_spec(&defn.result, &op.result) {
        return false;
    }
    if op.result_brand != defn.result_brand {
        return false;
    }
    true
}

fn type_ref_matches_spec(impl_ty: &x07ast::TypeRef, spec_ty: &str) -> bool {
    impl_ty.as_named().unwrap_or("") == spec_ty.trim()
}

fn check_contract_alignment(
    impl_path: &Path,
    op: &SpecOperation,
    defn: &x07ast::AstFunctionDef,
    core: &CoreContractClauses,
    diags: &mut Vec<diagnostics::Diagnostic>,
) {
    let requires_spec: Vec<&x07ast::ContractClauseAst> =
        core.requires.iter().map(|c| &c.clause).collect();
    let ensures_spec: Vec<&x07ast::ContractClauseAst> =
        core.ensures.iter().map(|c| &c.clause).collect();
    let invariant_spec: Vec<&x07ast::ContractClauseAst> =
        core.invariant.iter().map(|c| &c.clause).collect();

    check_contract_clause_list(
        impl_path,
        op,
        "requires",
        &requires_spec,
        &defn.requires,
        true,
        diags,
    );
    check_contract_clause_list(
        impl_path,
        op,
        "ensures",
        &ensures_spec,
        &defn.ensures,
        false,
        diags,
    );
    check_contract_clause_list(
        impl_path,
        op,
        "invariant",
        &invariant_spec,
        &defn.invariant,
        false,
        diags,
    );
}

fn check_contract_clause_list(
    impl_path: &Path,
    op: &SpecOperation,
    kind: &str,
    spec_core: &[&x07ast::ContractClauseAst],
    impl_clauses: &[x07ast::ContractClauseAst],
    extra_is_error: bool,
    diags: &mut Vec<diagnostics::Diagnostic>,
) {
    let mut matched_impl: BTreeSet<usize> = BTreeSet::new();
    for spec_clause in spec_core {
        match find_matching_impl_clause(spec_clause, impl_clauses) {
            None => diags.push(impl_error(
                "EXTAL_IMPL_CONTRACT_MISSING",
                diagnostics::Stage::Lint,
                impl_path,
                None,
                format!(
                    "missing {kind} clause for {:?} (id={:?})",
                    op.name, spec_clause.id
                ),
            )),
            Some((idx, matched_by_id)) => {
                matched_impl.insert(idx);
                if matched_by_id && impl_clauses[idx].expr != spec_clause.expr {
                    diags.push(impl_error(
                        "EXTAL_IMPL_CONTRACT_MISMATCH",
                        diagnostics::Stage::Lint,
                        impl_path,
                        None,
                        format!(
                            "{kind} clause mismatch for {:?} (id={:?})",
                            op.name, spec_clause.id
                        ),
                    ));
                }
            }
        }
    }

    for (idx, clause) in impl_clauses.iter().enumerate() {
        if matched_impl.contains(&idx) {
            continue;
        }
        if extra_is_error {
            diags.push(impl_error(
                "EXTAL_IMPL_CONTRACT_EXTRA_REQUIRES",
                diagnostics::Stage::Lint,
                impl_path,
                None,
                format!(
                    "extra requires clause for {:?} (id={:?})",
                    op.name, clause.id
                ),
            ));
        } else {
            diags.push(impl_warning(
                "EXTAL_IMPL_CONTRACT_EXTRA",
                diagnostics::Stage::Lint,
                impl_path,
                None,
                format!("extra {kind} clause for {:?} (id={:?})", op.name, clause.id),
            ));
        }
    }
}

fn find_matching_impl_clause(
    spec_clause: &x07ast::ContractClauseAst,
    impl_clauses: &[x07ast::ContractClauseAst],
) -> Option<(usize, bool)> {
    let spec_id = spec_clause.id.as_deref().unwrap_or("").trim();
    if !spec_id.is_empty() {
        for (idx, clause) in impl_clauses.iter().enumerate() {
            if clause.id.as_deref().unwrap_or("").trim() == spec_id {
                return Some((idx, true));
            }
        }
    }
    for (idx, clause) in impl_clauses.iter().enumerate() {
        if clause.expr == spec_clause.expr {
            return Some((idx, false));
        }
    }
    None
}

fn upsert_contract_clauses(
    existing: &mut Vec<x07ast::ContractClauseAst>,
    desired: &[x07ast::ContractClauseAst],
) {
    for want in desired {
        let want_id = want.id.as_deref().unwrap_or("").trim();
        if !want_id.is_empty() {
            if let Some(slot) = existing
                .iter_mut()
                .find(|c| c.id.as_deref().unwrap_or("").trim() == want_id)
            {
                slot.expr = want.expr.clone();
                slot.witness = want.witness.clone();
                continue;
            }
        }

        if let Some(slot) = existing.iter_mut().find(|c| c.expr == want.expr) {
            if slot.id.is_none() {
                slot.id = want.id.clone();
            }
            continue;
        }

        existing.push(want.clone());
    }
}

fn impl_error(
    code: &str,
    stage: diagnostics::Stage,
    file: &Path,
    loc: Option<diagnostics::Location>,
    message: impl Into<String>,
) -> diagnostics::Diagnostic {
    let mut d = diag_error(code, stage, message, loc);
    d.data.insert(
        "file".to_string(),
        Value::String(file.display().to_string()),
    );
    d
}

fn impl_warning(
    code: &str,
    stage: diagnostics::Stage,
    file: &Path,
    loc: Option<diagnostics::Location>,
    message: impl Into<String>,
) -> diagnostics::Diagnostic {
    let mut d = diag_warning(code, stage, message, loc);
    d.data.insert(
        "file".to_string(),
        Value::String(file.display().to_string()),
    );
    d
}

fn cmd_xtal_spec_fmt(
    machine: &crate::reporting::MachineArgs,
    args: XtalSpecFmtArgs,
) -> Result<std::process::ExitCode> {
    if args.check == args.write {
        anyhow::bail!("set exactly one of --check or --write");
    }
    if args.input.is_empty() {
        anyhow::bail!("missing --input (pass one or more spec files)");
    }

    let inputs = collect_spec_inputs(&args.input)?;
    let mut diags = Vec::new();
    let mut not_formatted = Vec::new();

    for input in &inputs {
        let bytes = match std::fs::read(input) {
            Ok(b) => b,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_SPEC_IO_READ_FAILED",
                    diagnostics::Stage::Parse,
                    input,
                    None,
                    format!("cannot read file: {err}"),
                ));
                continue;
            }
        };
        let mut doc: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_SPEC_JSON_PARSE",
                    diagnostics::Stage::Parse,
                    input,
                    None,
                    format!("invalid JSON: {err}"),
                ));
                continue;
            }
        };
        let mut file_has_error = false;

        let schema_version = doc
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or("");
        if schema_version != SPEC_SCHEMA_VERSION {
            file_has_error = true;
            diags.push(spec_error(
                "EXTAL_SPEC_SCHEMA_VERSION_UNSUPPORTED",
                diagnostics::Stage::Parse,
                input,
                Some(diagnostics::Location::X07Ast {
                    ptr: "/schema_version".to_string(),
                }),
                format!(
                    "unsupported schema_version: expected {SPEC_SCHEMA_VERSION:?} got {schema_version:?}"
                ),
            ));
        }

        let schema_diags = report_common::validate_schema(
            SPEC_SCHEMA_BYTES,
            "spec/x07.x07spec@0.1.0.schema.json",
            &doc,
        )?;
        if !schema_diags.is_empty() {
            file_has_error = true;
            for d in schema_diags {
                diags.push(remap_schema_diag("EXTAL_SPEC_SCHEMA_INVALID", input, d));
            }
        }

        if args.inject_ids {
            match serde_json::from_value::<SpecFile>(doc.clone()) {
                Ok(mut spec) => {
                    inject_missing_ids(&mut spec);
                    doc = serde_json::to_value(spec)?;
                }
                Err(err) => {
                    file_has_error = true;
                    diags.push(spec_error(
                        "EXTAL_SPEC_SCHEMA_INVALID",
                        diagnostics::Stage::Parse,
                        input,
                        None,
                        format!("spec JSON shape is invalid: {err}"),
                    ));
                }
            }
        }
        if file_has_error {
            continue;
        }

        let formatted = report_common::canonical_pretty_json_bytes(&doc)?;
        if args.check && bytes != formatted {
            diags.push(spec_warning(
                "WXTAL_SPEC_NONCANONICAL_JSON",
                diagnostics::Stage::Rewrite,
                input,
                Some(diagnostics::Location::X07Ast {
                    ptr: "".to_string(),
                }),
                "spec JSON is not in canonical form (run `x07 xtal spec fmt --write`)",
            ));
            not_formatted.push(input.clone());
            continue;
        }
        if args.write && bytes != formatted {
            util::write_atomic(input, &formatted)
                .with_context(|| format!("write: {}", input.display()))?;
        }
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diags);
    report.meta.insert(
        "inputs".to_string(),
        Value::Array(
            inputs
                .iter()
                .map(|p| Value::String(p.display().to_string()))
                .collect(),
        ),
    );
    let spec_digests: Vec<Value> = inputs.iter().filter_map(|p| file_digest_value(p)).collect();
    report
        .meta
        .insert("spec_digests".to_string(), Value::Array(spec_digests));
    if args.check && !not_formatted.is_empty() {
        report.ok = false;
    }
    write_report(machine, &report)?;
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_xtal_spec_lint(
    machine: &crate::reporting::MachineArgs,
    args: XtalSpecLintArgs,
) -> Result<std::process::ExitCode> {
    if args.input.is_empty() {
        anyhow::bail!("missing --input (pass one or more spec files)");
    }
    let inputs = collect_spec_inputs(&args.input)?;
    let mut diags = Vec::new();

    for input in &inputs {
        let (doc_opt, file_diags) = lint_one_spec_file(input)?;
        diags.extend(file_diags);
        let _ = doc_opt;
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diags);
    report.meta.insert(
        "inputs".to_string(),
        Value::Array(
            inputs
                .iter()
                .map(|p| Value::String(p.display().to_string()))
                .collect(),
        ),
    );
    let spec_digests: Vec<Value> = inputs.iter().filter_map(|p| file_digest_value(p)).collect();
    report
        .meta
        .insert("spec_digests".to_string(), Value::Array(spec_digests));
    write_report(machine, &report)?;
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_xtal_spec_check(
    machine: &crate::reporting::MachineArgs,
    args: XtalSpecCheckArgs,
) -> Result<std::process::ExitCode> {
    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let inputs = if args.input.is_empty() {
        let spec_root = project_root.join(DEFAULT_SPEC_DIR);
        collect_spec_files(&spec_root, &Vec::new(), &mut Vec::new())
    } else {
        collect_spec_inputs(&args.input)?
    };
    let mut diags = Vec::new();

    let mut specs: Vec<(PathBuf, Value, SpecFile)> = Vec::new();
    for input in &inputs {
        let (doc_opt, lint_diags) = lint_one_spec_file(input)?;
        diags.extend(lint_diags);
        let Some(doc) = doc_opt else {
            continue;
        };
        let spec: SpecFile = match serde_json::from_value(doc.clone()) {
            Ok(v) => v,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_SPEC_SCHEMA_INVALID",
                    diagnostics::Stage::Parse,
                    input,
                    None,
                    format!("spec JSON shape is invalid: {err}"),
                ));
                continue;
            }
        };
        specs.push((input.clone(), doc, spec));
    }

    for (path, _doc, spec) in &specs {
        if spec.assumptions.is_empty() {
            continue;
        }
        let mut ids: Vec<&str> = spec
            .assumptions
            .iter()
            .map(|a| a.id.trim())
            .filter(|id| !id.is_empty())
            .collect();
        ids.sort();
        ids.dedup();
        let shown: Vec<&str> = ids.iter().copied().take(20).collect();
        let more = ids.len().saturating_sub(shown.len());
        let mut msg = format!("spec declares assumptions: {:?}", shown);
        if more > 0 {
            msg.push_str(&format!(" (+{more} more)"));
        }
        diags.push(spec_warning(
            "EXTAL_SPEC_HAS_ASSUMPTIONS",
            diagnostics::Stage::Lint,
            path,
            Some(diagnostics::Location::X07Ast {
                ptr: "/assumptions".to_string(),
            }),
            msg,
        ));
    }

    // If lint has errors, keep going best-effort but avoid cascading on empty specs.
    let mut seen_op_ids: BTreeMap<String, Vec<(PathBuf, usize)>> = BTreeMap::new();
    for (path, _doc, spec) in &specs {
        if let Err(msg) = x07c::validate::validate_module_id(&spec.module_id) {
            diags.push(spec_error(
                "EXTAL_SPEC_MODULE_ID_INVALID",
                diagnostics::Stage::Lint,
                path,
                Some(diagnostics::Location::X07Ast {
                    ptr: "/module_id".to_string(),
                }),
                msg,
            ));
        }
        if spec.operations.is_empty() {
            diags.push(spec_error(
                "EXTAL_SPEC_OPS_EMPTY",
                diagnostics::Stage::Lint,
                path,
                Some(diagnostics::Location::X07Ast {
                    ptr: "/operations".to_string(),
                }),
                "spec has zero operations".to_string(),
            ));
        }

        for (op_idx, op) in spec.operations.iter().enumerate() {
            if op.id.as_deref().unwrap_or("").trim().is_empty() {
                diags.push(spec_error(
                    "EXTAL_SPEC_OP_ID_MISSING",
                    diagnostics::Stage::Lint,
                    path,
                    Some(diagnostics::Location::X07Ast {
                        ptr: format!("/operations/{op_idx}/id"),
                    }),
                    "operation is missing id".to_string(),
                ));
            } else if let Some(id) = op.id.as_deref() {
                seen_op_ids
                    .entry(id.to_string())
                    .or_default()
                    .push((path.clone(), op_idx));
            }

            if op.name.trim().is_empty() {
                diags.push(spec_error(
                    "EXTAL_SPEC_OP_NAME_MISSING",
                    diagnostics::Stage::Lint,
                    path,
                    Some(diagnostics::Location::X07Ast {
                        ptr: format!("/operations/{op_idx}/name"),
                    }),
                    "operation is missing name".to_string(),
                ));
            } else if let Err(msg) = x07c::validate::validate_symbol(&op.name) {
                diags.push(spec_error(
                    "EXTAL_SPEC_OP_NAME_INVALID",
                    diagnostics::Stage::Lint,
                    path,
                    Some(diagnostics::Location::X07Ast {
                        ptr: format!("/operations/{op_idx}/name"),
                    }),
                    msg,
                ));
            } else if !op.name.starts_with(&format!("{}.", spec.module_id)) {
                diags.push(spec_error(
                    "EXTAL_SPEC_OP_NAME_INVALID",
                    diagnostics::Stage::Lint,
                    path,
                    Some(diagnostics::Location::X07Ast {
                        ptr: format!("/operations/{op_idx}/name"),
                    }),
                    format!(
                        "operation name must start with module_id prefix {:?}",
                        format!("{}.", spec.module_id)
                    ),
                ));
            }

            let mut param_names = BTreeSet::new();
            for (p_idx, p) in op.params.iter().enumerate() {
                if p.name.trim().is_empty() {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PARAM_NAME_INVALID",
                        diagnostics::Stage::Lint,
                        path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/params/{p_idx}/name"),
                        }),
                        "param name must be non-empty".to_string(),
                    ));
                    continue;
                }
                if p.name == "__result" {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PARAM_NAME_INVALID",
                        diagnostics::Stage::Lint,
                        path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/params/{p_idx}/name"),
                        }),
                        "reserved name is not allowed here: \"__result\"".to_string(),
                    ));
                }
                if !param_names.insert(p.name.clone()) {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PARAM_NAME_DUPLICATE",
                        diagnostics::Stage::Lint,
                        path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/params/{p_idx}/name"),
                        }),
                        format!("duplicate param name: {:?}", p.name),
                    ));
                }
                if !is_supported_ty(&p.ty) {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PARAM_TY_UNSUPPORTED",
                        diagnostics::Stage::Lint,
                        path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/params/{p_idx}/ty"),
                        }),
                        format!("unsupported param type in this stage: {:?}", p.ty),
                    ));
                }
            }
            if !is_supported_ty(&op.result) {
                diags.push(spec_error(
                    "EXTAL_SPEC_RESULT_TY_UNSUPPORTED",
                    diagnostics::Stage::Lint,
                    path,
                    Some(diagnostics::Location::X07Ast {
                        ptr: format!("/operations/{op_idx}/result"),
                    }),
                    format!("unsupported result type in this stage: {:?}", op.result),
                ));
            }

            if let Some(examples_ref) = op.examples_ref.as_deref() {
                let examples_path = project_root.join(examples_ref);
                if !examples_path.is_file() {
                    diags.push(spec_error(
                        "EXTAL_SPEC_EXAMPLES_REF_MISSING",
                        diagnostics::Stage::Lint,
                        path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/examples_ref"),
                        }),
                        format!("examples_ref does not exist: {}", examples_path.display()),
                    ));
                }
            }
        }
    }

    for (op_id, entries) in &seen_op_ids {
        if entries.len() <= 1 {
            continue;
        }
        for (path, op_idx) in entries {
            diags.push(spec_error(
                "EXTAL_SPEC_OP_ID_DUPLICATE",
                diagnostics::Stage::Lint,
                path,
                Some(diagnostics::Location::X07Ast {
                    ptr: format!("/operations/{op_idx}/id"),
                }),
                format!("duplicate operation id: {:?}", op_id),
            ));
        }
    }

    // Contract checks (best-effort, contract-pure subset).
    for (path, _doc, spec) in &specs {
        diags.extend(typecheck_spec_contracts(path, spec)?);
    }

    // Examples checks.
    for (path, _doc, spec) in &specs {
        diags.extend(check_spec_examples(&project_root, path, spec)?);
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diags);
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report.meta.insert(
        "inputs".to_string(),
        Value::Array(
            inputs
                .iter()
                .map(|p| Value::String(p.display().to_string()))
                .collect(),
        ),
    );
    let spec_digests: Vec<Value> = inputs.iter().filter_map(|p| file_digest_value(p)).collect();
    let mut examples_paths: BTreeSet<PathBuf> = BTreeSet::new();
    for (_path, _doc, spec) in &specs {
        for op in &spec.operations {
            let Some(ex_ref) = op.examples_ref.as_deref() else {
                continue;
            };
            let ex_ref = ex_ref.trim();
            if ex_ref.is_empty() {
                continue;
            }
            examples_paths.insert(project_root.join(ex_ref));
        }
    }
    let examples_digests: Vec<Value> = examples_paths
        .iter()
        .filter_map(|p| {
            if p.is_file() {
                file_digest_value(p)
            } else {
                None
            }
        })
        .collect();
    report
        .meta
        .insert("spec_digests".to_string(), Value::Array(spec_digests));
    report.meta.insert(
        "examples_digests".to_string(),
        Value::Array(examples_digests),
    );
    write_report(machine, &report)?;
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_xtal_spec_extract(
    machine: &crate::reporting::MachineArgs,
    args: XtalSpecExtractArgs,
) -> Result<std::process::ExitCode> {
    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let impl_root = project_root.join(&args.impl_dir);
    let spec_root = project_root.join(&args.spec_dir);
    let patchset_out = args.patchset_out.as_deref().map(|p| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            project_root.join(p)
        }
    });

    let impl_path = match (args.module_id.as_deref(), args.impl_path.as_deref()) {
        (Some(module_id), None) => module_id_to_impl_path(&impl_root, module_id),
        (None, Some(path)) => {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                project_root.join(path)
            }
        }
        _ => anyhow::bail!("set exactly one of --module-id or --impl-path"),
    };

    let mut diags = Vec::new();
    if !impl_path.is_file() {
        diags.push(diag_error(
            "EXTAL_SPEC_EXTRACT_IMPL_MODULE_MISSING",
            diagnostics::Stage::Parse,
            format!(
                "implementation module does not exist: {}",
                impl_path.display()
            ),
            None,
        ));
        let mut report = diagnostics::Report::ok().with_diagnostics(diags);
        report.meta.insert(
            "project_root".to_string(),
            Value::String(project_root.display().to_string()),
        );
        report.meta.insert(
            "impl_path".to_string(),
            Value::String(impl_path.display().to_string()),
        );
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let bytes = match std::fs::read(&impl_path) {
        Ok(v) => v,
        Err(err) => {
            diags.push(diag_error(
                "EXTAL_IMPL_IO_READ_FAILED",
                diagnostics::Stage::Parse,
                format!("cannot read impl module: {err}"),
                None,
            ));
            let mut report = diagnostics::Report::ok().with_diagnostics(diags);
            report.meta.insert(
                "project_root".to_string(),
                Value::String(project_root.display().to_string()),
            );
            report.meta.insert(
                "impl_path".to_string(),
                Value::String(impl_path.display().to_string()),
            );
            write_report(machine, &report)?;
            return Ok(std::process::ExitCode::from(1));
        }
    };
    let file = match x07ast::parse_x07ast_json(&bytes) {
        Ok(v) => v,
        Err(err) => {
            diags.push(diag_error(
                "EXTAL_IMPL_X07AST_PARSE",
                diagnostics::Stage::Parse,
                format!("cannot parse impl module: {err}"),
                None,
            ));
            let mut report = diagnostics::Report::ok().with_diagnostics(diags);
            report.meta.insert(
                "project_root".to_string(),
                Value::String(project_root.display().to_string()),
            );
            report.meta.insert(
                "impl_path".to_string(),
                Value::String(impl_path.display().to_string()),
            );
            write_report(machine, &report)?;
            return Ok(std::process::ExitCode::from(1));
        }
    };

    if file.kind != x07ast::X07AstKind::Module {
        diags.push(diag_error(
            "EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE",
            diagnostics::Stage::Parse,
            format!("expected kind=\"module\" (got {:?})", file.kind),
            None,
        ));
        let mut report = diagnostics::Report::ok().with_diagnostics(diags);
        report.meta.insert(
            "project_root".to_string(),
            Value::String(project_root.display().to_string()),
        );
        report.meta.insert(
            "impl_path".to_string(),
            Value::String(impl_path.display().to_string()),
        );
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }

    let module_id = file.module_id.trim().to_string();
    if module_id.is_empty() {
        diags.push(diag_error(
            "EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE",
            diagnostics::Stage::Parse,
            "module_id must be non-empty".to_string(),
            None,
        ));
        let mut report = diagnostics::Report::ok().with_diagnostics(diags);
        report.meta.insert(
            "project_root".to_string(),
            Value::String(project_root.display().to_string()),
        );
        report.meta.insert(
            "impl_path".to_string(),
            Value::String(impl_path.display().to_string()),
        );
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }
    if let Err(msg) = x07c::validate::validate_module_id(&module_id) {
        diags.push(diag_error(
            "EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE",
            diagnostics::Stage::Parse,
            msg,
            None,
        ));
        let mut report = diagnostics::Report::ok().with_diagnostics(diags);
        report.meta.insert(
            "project_root".to_string(),
            Value::String(project_root.display().to_string()),
        );
        report.meta.insert(
            "impl_path".to_string(),
            Value::String(impl_path.display().to_string()),
        );
        write_report(machine, &report)?;
        return Ok(std::process::ExitCode::from(1));
    }
    if let Some(expected) = args.module_id.as_deref() {
        if expected.trim() != module_id {
            diags.push(diag_error(
                "EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE",
                diagnostics::Stage::Parse,
                format!(
                    "module_id mismatch: expected {:?} got {:?}",
                    expected.trim(),
                    module_id
                ),
                None,
            ));
            let mut report = diagnostics::Report::ok().with_diagnostics(diags);
            report.meta.insert(
                "project_root".to_string(),
                Value::String(project_root.display().to_string()),
            );
            report.meta.insert(
                "impl_path".to_string(),
                Value::String(impl_path.display().to_string()),
            );
            write_report(machine, &report)?;
            return Ok(std::process::ExitCode::from(1));
        }
    }

    let spec_path = spec_root.join(format!("{module_id}.x07spec.json"));

    let existing_spec = if spec_path.is_file() {
        report_common::read_json_file(&spec_path)
            .ok()
            .and_then(|v| serde_json::from_value::<SpecFile>(v).ok())
    } else {
        None
    };

    let mut existing_ops: BTreeMap<String, SpecOperation> = BTreeMap::new();
    if let Some(existing) = existing_spec.as_ref() {
        for op in &existing.operations {
            existing_ops.insert(op.name.clone(), op.clone());
        }
    }

    if file.exports.is_empty() {
        diags.push(diag_error(
            "EXTAL_SPEC_EXTRACT_NO_EXPORTS",
            diagnostics::Stage::Lint,
            format!("no exports found in {}", impl_path.display()),
            None,
        ));
    }

    let mut extracted_ops = Vec::new();
    for export in file.exports.iter() {
        let Some(defn) = file.functions.iter().find(|f| f.name == *export) else {
            diags.push(diag_warning(
                "EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE",
                diagnostics::Stage::Lint,
                format!("skipping export {:?}: not a defn", export),
                None,
            ));
            continue;
        };
        if !defn.type_params.is_empty() {
            diags.push(diag_warning(
                "EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE",
                diagnostics::Stage::Lint,
                format!("skipping export {:?}: generics are unsupported", export),
                None,
            ));
            continue;
        }
        if defn
            .params
            .iter()
            .any(|p| !p.ty.as_named().is_some_and(is_supported_ty))
        {
            diags.push(diag_warning(
                "EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE",
                diagnostics::Stage::Lint,
                format!("skipping export {:?}: unsupported param type", export),
                None,
            ));
            continue;
        }
        if !defn.result.as_named().is_some_and(is_supported_ty) {
            diags.push(diag_warning(
                "EXTAL_SPEC_EXTRACT_UNSUPPORTED_SIGNATURE",
                diagnostics::Stage::Lint,
                format!("skipping export {:?}: unsupported result type", export),
                None,
            ));
            continue;
        }

        let fallback_id = match parse_symbol_to_module_and_local(&defn.name) {
            Ok((_m, local)) => format!("op.{local}.v1"),
            Err(_) => format!("op.{export}.v1"),
        };
        let prior = existing_ops.get(&defn.name);
        let op_id = prior.and_then(|op| op.id.clone()).unwrap_or(fallback_id);

        let mut params = Vec::new();
        for p in &defn.params {
            let Some(ty) = p.ty.as_named() else {
                continue;
            };
            params.push(SpecParam {
                name: p.name.clone(),
                ty: ty.to_string(),
                brand: p.brand.clone(),
            });
        }

        let mut requires = Vec::new();
        for c in &defn.requires {
            requires.push(SpecClause {
                id: c.id.clone(),
                expr: x07ast::expr_to_value(&c.expr),
                witness: c.witness.iter().map(x07ast::expr_to_value).collect(),
            });
        }
        let mut ensures = Vec::new();
        for c in &defn.ensures {
            ensures.push(SpecClause {
                id: c.id.clone(),
                expr: x07ast::expr_to_value(&c.expr),
                witness: c.witness.iter().map(x07ast::expr_to_value).collect(),
            });
        }
        let mut invariant = Vec::new();
        for c in &defn.invariant {
            invariant.push(SpecClause {
                id: c.id.clone(),
                expr: x07ast::expr_to_value(&c.expr),
                witness: c.witness.iter().map(x07ast::expr_to_value).collect(),
            });
        }

        extracted_ops.push(SpecOperation {
            id: Some(op_id),
            name: defn.name.clone(),
            doc: prior.and_then(|op| op.doc.clone()),
            params,
            result: defn.result.as_named().unwrap_or("i32").to_string(),
            result_brand: defn.result_brand.clone(),
            requires,
            ensures,
            invariant,
            ensures_props: prior.map(|op| op.ensures_props.clone()).unwrap_or_default(),
            examples_ref: prior.and_then(|op| op.examples_ref.clone()),
        });
    }

    if extracted_ops.is_empty() {
        diags.push(diag_error(
            "EXTAL_SPEC_EXTRACT_NO_EXPORTS",
            diagnostics::Stage::Lint,
            format!(
                "no eligible exported defns found in {}",
                impl_path.display()
            ),
            None,
        ));
    }

    let mut out_spec = existing_spec.unwrap_or(SpecFile {
        schema_version: SPEC_SCHEMA_VERSION.to_string(),
        module_id: module_id.clone(),
        title: None,
        doc: None,
        ids: None,
        sorts: Vec::new(),
        operations: Vec::new(),
        assumptions: Vec::new(),
        nonfunctional: None,
    });
    out_spec.schema_version = SPEC_SCHEMA_VERSION.to_string();
    out_spec.module_id = module_id.clone();
    out_spec.operations = extracted_ops;

    let out_value = serde_json::to_value(out_spec)?;
    let out_bytes = report_common::canonical_pretty_json_bytes(&out_value)?;

    let existing_bytes = std::fs::read(&spec_path).unwrap_or_default();
    let differs = if existing_bytes.is_empty() {
        true
    } else if let Ok(existing_value) = serde_json::from_slice::<Value>(&existing_bytes) {
        report_common::canonical_pretty_json_bytes(&existing_value).unwrap_or(existing_bytes)
            != out_bytes
    } else {
        true
    };

    if let Some(patchset_path) = patchset_out.as_ref() {
        let patches = if differs {
            let rel = spec_path
                .strip_prefix(&project_root)
                .unwrap_or(&spec_path)
                .display()
                .to_string();
            vec![PatchTarget {
                path: rel,
                patch: vec![diagnostics::PatchOp::Replace {
                    path: "".to_string(),
                    value: out_value.clone(),
                }],
                note: None,
            }]
        } else {
            Vec::new()
        };
        let patchset = PatchSet {
            schema_version: x07_contracts::X07_PATCHSET_SCHEMA_VERSION.to_string(),
            patches,
        };
        let patchset_value = serde_json::to_value(patchset)?;
        let bytes = report_common::canonical_pretty_json_bytes(&patchset_value)?;
        util::write_atomic(patchset_path, &bytes)
            .with_context(|| format!("write patchset: {}", patchset_path.display()))?;
    } else if args.write {
        util::write_atomic(&spec_path, &out_bytes)
            .with_context(|| format!("write: {}", spec_path.display()))?;
    } else if differs {
        diags.push(diag_error(
            "EXTAL_SPEC_EXTRACT_DIFFERS",
            diagnostics::Stage::Lint,
            format!(
                "extracted spec differs from {} (rerun with --write or --patchset-out)",
                spec_path.display()
            ),
            None,
        ));
    }

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diags);
    report.meta.insert(
        "project_root".to_string(),
        Value::String(project_root.display().to_string()),
    );
    report.meta.insert(
        "impl_path".to_string(),
        Value::String(impl_path.display().to_string()),
    );
    report.meta.insert(
        "spec_path".to_string(),
        Value::String(spec_path.display().to_string()),
    );
    if let Some(patchset_path) = patchset_out.as_ref() {
        let display = patchset_path
            .strip_prefix(&project_root)
            .unwrap_or(patchset_path)
            .display()
            .to_string();
        report
            .meta
            .insert("patchset_path".to_string(), Value::String(display));
    }
    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
}

fn cmd_xtal_spec_scaffold(args: XtalSpecScaffoldArgs) -> Result<std::process::ExitCode> {
    x07c::validate::validate_module_id(&args.module_id)
        .map_err(|e| anyhow::anyhow!("module_id invalid: {e}"))?;
    x07c::validate::validate_local_name(&args.op)
        .map_err(|e| anyhow::anyhow!("op local name invalid: {e}"))?;

    let project_root = resolve_project_root(None, None)?;
    let spec_path = args.out_path.unwrap_or_else(|| {
        PathBuf::from(DEFAULT_SPEC_DIR).join(format!("{}.x07spec.json", args.module_id))
    });
    let spec_path = project_root.join(spec_path);

    let mut params = Vec::new();
    for raw in &args.param {
        let Some((name, ty)) = raw.split_once(':') else {
            anyhow::bail!("--param expects NAME:TY (got {raw:?})");
        };
        params.push(SpecParam {
            name: name.trim().to_string(),
            ty: ty.trim().to_string(),
            brand: None,
        });
    }

    let op_local = args.op.trim();
    let op_id = format!("op.{op_local}.v1");
    let op_name = format!("{}.{}", args.module_id, op_local);
    let examples_ref = args.examples.then(|| {
        format!(
            "{}/{}.x07spec.examples.jsonl",
            DEFAULT_SPEC_DIR, args.module_id
        )
    });

    let spec = SpecFile {
        schema_version: SPEC_SCHEMA_VERSION.to_string(),
        module_id: args.module_id.clone(),
        title: None,
        doc: None,
        ids: None,
        sorts: Vec::new(),
        operations: vec![SpecOperation {
            id: Some(op_id),
            name: op_name,
            doc: Some("".to_string()),
            params,
            result: args.result.trim().to_string(),
            result_brand: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            invariant: Vec::new(),
            ensures_props: Vec::new(),
            examples_ref,
        }],
        assumptions: Vec::new(),
        nonfunctional: None,
    };

    let spec_value = serde_json::to_value(spec)?;
    let bytes = report_common::canonical_pretty_json_bytes(&spec_value)?;
    util::write_atomic(&spec_path, &bytes)
        .with_context(|| format!("write: {}", spec_path.display()))?;

    if args.examples {
        let examples_path = project_root.join(format!(
            "{}/{}.x07spec.examples.jsonl",
            DEFAULT_SPEC_DIR, args.module_id
        ));
        if !examples_path.exists() {
            let line = json!({
                "schema_version": EXAMPLES_SCHEMA_VERSION,
                "op": format!("op.{op_local}.v1"),
                "args": {},
                "expect": 0,
                "tags": ["smoke"],
                "doc": "TODO",
            });
            let text = serde_json::to_string(&line)? + "\n";
            util::write_atomic(&examples_path, text.as_bytes())
                .with_context(|| format!("write: {}", examples_path.display()))?;
        }
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn cmd_xtal_tests_gen_from_spec(
    machine: &crate::reporting::MachineArgs,
    args: XtalTestsGenArgs,
) -> Result<std::process::ExitCode> {
    if args.check == args.write {
        anyhow::bail!("set exactly one of --check or --write");
    }

    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let out_root = project_root.join(&args.out_dir);
    let spec_root = project_root.join(&args.spec_dir);

    let mut diags = Vec::new();
    let spec_files = collect_spec_files(&spec_root, &args.spec, &mut diags);
    if spec_files.is_empty() {
        diags.push(diag_error(
            "EXTAL_GEN_NO_SPECS",
            diagnostics::Stage::Parse,
            format!("no spec files found under {}", spec_root.display()),
            None,
        ));
    }

    let mut modules = Vec::new();
    for spec_path in &spec_files {
        let (doc_opt, lint_diags) = lint_one_spec_file(spec_path)?;
        diags.extend(lint_diags);
        let Some(doc) = doc_opt else {
            continue;
        };
        let spec: SpecFile = match serde_json::from_value(doc) {
            Ok(v) => v,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_SPEC_SCHEMA_INVALID",
                    diagnostics::Stage::Parse,
                    spec_path,
                    None,
                    format!("spec JSON shape is invalid: {err}"),
                ));
                continue;
            }
        };

        modules.push((spec_path.clone(), spec));
    }

    let generated = generate_tests_from_specs(&project_root, &modules, &mut diags)?;

    let mut report = diagnostics::Report::ok();
    report = report.with_diagnostics(diags);
    report.meta.insert(
        "out_dir".to_string(),
        Value::String(out_root.display().to_string()),
    );
    report.meta.insert(
        "specs".to_string(),
        Value::Array(
            spec_files
                .iter()
                .map(|p| Value::String(p.display().to_string()))
                .collect(),
        ),
    );
    let spec_digests: Vec<Value> = spec_files
        .iter()
        .filter_map(|p| file_digest_value(p))
        .collect();
    let mut examples_paths: BTreeSet<PathBuf> = BTreeSet::new();
    for (_spec_path, spec) in &modules {
        for op in &spec.operations {
            let Some(ex_ref) = op.examples_ref.as_deref() else {
                continue;
            };
            let ex_ref = ex_ref.trim();
            if ex_ref.is_empty() {
                continue;
            }
            examples_paths.insert(project_root.join(ex_ref));
        }
    }
    let examples_digests: Vec<Value> = examples_paths
        .iter()
        .filter_map(|p| {
            if p.is_file() {
                file_digest_value(p)
            } else {
                None
            }
        })
        .collect();
    report
        .meta
        .insert("spec_digests".to_string(), Value::Array(spec_digests));
    report.meta.insert(
        "examples_digests".to_string(),
        Value::Array(examples_digests),
    );

    if report.ok && args.check {
        let drift = check_generated_tree(&out_root, &generated)?;
        if !drift.is_empty() {
            report.ok = false;
            for rel in drift.iter().take(100) {
                report.diagnostics.push(diag_error(
                    "EXTAL_GEN_DRIFT",
                    diagnostics::Stage::Run,
                    format!("drifted: {rel}"),
                    None,
                ));
            }
        }
    }

    if report.ok && args.write {
        for (rel, bytes) in &generated {
            let path = out_root.join(rel);
            util::write_atomic(&path, bytes)
                .with_context(|| format!("write: {}", path.display()))?;
        }
    }

    write_report(machine, &report)?;
    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(2)
    })
}

fn check_generated_tree(
    out_root: &Path,
    generated: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<Vec<String>> {
    let mut drift = Vec::new();

    // Compare expected outputs.
    for (rel, want) in generated {
        let path = out_root.join(rel);
        match std::fs::read(&path) {
            Ok(got) if got == *want => {}
            _ => drift.push(rel.to_string_lossy().replace('\\', "/")),
        }
    }

    // Ensure there are no extra files under out_root.
    if out_root.is_dir() {
        for entry in WalkDir::new(out_root)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = match entry.path().strip_prefix(out_root) {
                Ok(p) => p.to_path_buf(),
                Err(_) => continue,
            };
            if !generated.contains_key(&rel) {
                drift.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    drift.sort();
    drift.dedup();
    Ok(drift)
}

fn generate_tests_from_specs(
    project_root: &Path,
    modules: &[(PathBuf, SpecFile)],
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut outputs: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    let mut test_entries: Vec<Value> = Vec::new();

    // Load examples by file path once.
    let mut examples_cache: BTreeMap<String, Vec<ExampleLine>> = BTreeMap::new();

    for (spec_path, spec) in modules {
        let module_id = spec.module_id.as_str();
        let gen_module_id = format!("gen.xtal.{module_id}.tests");
        let mut exports = BTreeSet::new();
        let mut functions = Vec::new();
        let mut imports: BTreeSet<String> = BTreeSet::new();
        imports.insert("std.test".to_string());
        imports.insert(module_id.to_string());

        let mut global_idx = 0usize;
        let mut prop_idx = 0usize;

        for op in &spec.operations {
            let Some(op_id) = op.id.as_deref() else {
                continue;
            };
            let Some(ex_ref) = op.examples_ref.as_deref() else {
                continue;
            };
            if ex_ref.trim().is_empty() {
                continue;
            }

            let lines = if let Some(cached) = examples_cache.get(ex_ref) {
                cached.clone()
            } else {
                let path = project_root.join(ex_ref);
                let parsed = read_examples_file(&path, diags)?;
                examples_cache.insert(ex_ref.to_string(), parsed.clone());
                parsed
            };

            let mut mine = Vec::new();
            for line in lines {
                if line.op == op_id {
                    mine.push(line);
                }
            }

            if mine.is_empty() {
                diags.push(diag_error(
                    "EXTAL_GEN_NO_EXAMPLES",
                    diagnostics::Stage::Lower,
                    format!(
                        "no examples found for op {:?} (examples_ref={:?})",
                        op_id, ex_ref
                    ),
                    None,
                ));
                continue;
            }

            for (op_ex_idx0, ex) in mine.iter().enumerate() {
                let op_ex_idx = op_ex_idx0 + 1;
                global_idx += 1;
                let fn_name = format!("{gen_module_id}.ex_{global_idx:04}");
                exports.insert(fn_name.clone());
                let test_id = format!("xtal/{module_id}/{op_id}/ex{op_ex_idx:04}");

                let defn = gen_test_defn(&fn_name, op, ex, op_ex_idx, diags)?;
                functions.push(defn);

                test_entries.push(json!({
                    "id": test_id,
                    "world": "solve-pure",
                    "entry": fn_name,
                    "expect": "pass",
                    "returns": "result_i32"
                }));
            }
        }

        for (op_idx, op) in spec.operations.iter().enumerate() {
            let Some(op_id) = op.id.as_deref() else {
                continue;
            };

            for (op_prop_idx, prop) in op.ensures_props.iter().enumerate() {
                let prop_symbol = prop.prop.trim();
                if prop_symbol.is_empty() {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PROP_NAME_INVALID",
                        diagnostics::Stage::Lint,
                        spec_path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/ensures_props/{op_prop_idx}/prop"),
                        }),
                        "property name must be non-empty".to_string(),
                    ));
                    continue;
                }
                if let Err(msg) = x07c::validate::validate_symbol(prop_symbol) {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PROP_NAME_INVALID",
                        diagnostics::Stage::Lint,
                        spec_path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/ensures_props/{op_prop_idx}/prop"),
                        }),
                        msg,
                    ));
                    continue;
                }
                let Some((prop_module_id, _local)) = prop_symbol.rsplit_once('.') else {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PROP_NAME_INVALID",
                        diagnostics::Stage::Lint,
                        spec_path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/ensures_props/{op_prop_idx}/prop"),
                        }),
                        "property name must be a qualified symbol (module.name)".to_string(),
                    ));
                    continue;
                };
                if let Err(msg) = x07c::validate::validate_module_id(prop_module_id) {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PROP_NAME_INVALID",
                        diagnostics::Stage::Lint,
                        spec_path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/ensures_props/{op_prop_idx}/prop"),
                        }),
                        msg,
                    ));
                    continue;
                }

                if prop.args.is_empty() {
                    diags.push(spec_error(
                        "EXTAL_SPEC_PROP_ARGS_EMPTY",
                        diagnostics::Stage::Lint,
                        spec_path,
                        Some(diagnostics::Location::X07Ast {
                            ptr: format!("/operations/{op_idx}/ensures_props/{op_prop_idx}/args"),
                        }),
                        "property args must be non-empty".to_string(),
                    ));
                    continue;
                }

                let mut wrapper_params: Vec<x07ast::AstFunctionParam> = Vec::new();
                let mut pbt_params: Vec<Value> = Vec::new();
                let mut call_args: Vec<Expr> = Vec::new();
                let mut seen_param_names: BTreeSet<String> = BTreeSet::new();

                let mut prop_ok = true;
                for (arg_idx, arg_name_raw) in prop.args.iter().enumerate() {
                    let arg_name = arg_name_raw.trim();
                    if arg_name.is_empty() {
                        diags.push(spec_error(
                            "EXTAL_SPEC_PROP_ARG_UNKNOWN",
                            diagnostics::Stage::Lint,
                            spec_path,
                            Some(diagnostics::Location::X07Ast {
                                ptr: format!(
                                    "/operations/{op_idx}/ensures_props/{op_prop_idx}/args/{arg_idx}"
                                ),
                            }),
                            "arg name must be non-empty".to_string(),
                        ));
                        prop_ok = false;
                        continue;
                    }
                    if !seen_param_names.insert(arg_name.to_string()) {
                        diags.push(spec_error(
                            "EXTAL_SPEC_PROP_ARG_DUPLICATE",
                            diagnostics::Stage::Lint,
                            spec_path,
                            Some(diagnostics::Location::X07Ast {
                                ptr: format!(
                                    "/operations/{op_idx}/ensures_props/{op_prop_idx}/args/{arg_idx}"
                                ),
                            }),
                            format!("duplicate arg name: {:?}", arg_name),
                        ));
                        prop_ok = false;
                        continue;
                    }

                    let Some(op_param) = op.params.iter().find(|p| p.name == arg_name) else {
                        diags.push(spec_error(
                            "EXTAL_SPEC_PROP_ARG_UNKNOWN",
                            diagnostics::Stage::Lint,
                            spec_path,
                            Some(diagnostics::Location::X07Ast {
                                ptr: format!(
                                    "/operations/{op_idx}/ensures_props/{op_prop_idx}/args/{arg_idx}"
                                ),
                            }),
                            format!("unknown op param name: {:?}", arg_name),
                        ));
                        prop_ok = false;
                        continue;
                    };

                    let (wrapper_ty, gen) = match op_param.ty.trim() {
                        "i32" => (
                            "i32".to_string(),
                            json!({"kind": "i32", "min": -1000, "max": 1000}),
                        ),
                        "bytes" => ("bytes".to_string(), json!({"kind": "bytes", "max_len": 64})),
                        "bytes_view" => {
                            ("bytes".to_string(), json!({"kind": "bytes", "max_len": 64}))
                        }
                        other => {
                            diags.push(spec_error(
                                "EXTAL_SPEC_PROP_TY_UNSUPPORTED",
                                diagnostics::Stage::Lint,
                                spec_path,
                                Some(diagnostics::Location::X07Ast {
                                    ptr: format!(
                                        "/operations/{op_idx}/ensures_props/{op_prop_idx}/args/{arg_idx}"
                                    ),
                                }),
                                format!("unsupported arg type for PBT: {other:?}"),
                            ));
                            prop_ok = false;
                            continue;
                        }
                    };

                    let mut wrapper_name = arg_name.to_string();
                    if wrapper_name == "input" {
                        wrapper_name = format!("{wrapper_name}_v1");
                    }
                    if let Err(msg) = x07c::validate::validate_local_name(&wrapper_name) {
                        diags.push(spec_error(
                            "EXTAL_SPEC_PROP_ARG_NAME_INVALID",
                            diagnostics::Stage::Lint,
                            spec_path,
                            Some(diagnostics::Location::X07Ast {
                                ptr: format!(
                                    "/operations/{op_idx}/ensures_props/{op_prop_idx}/args/{arg_idx}"
                                ),
                            }),
                            msg,
                        ));
                        prop_ok = false;
                        continue;
                    }

                    wrapper_params.push(x07ast::AstFunctionParam {
                        name: wrapper_name.clone(),
                        ty: x07ast::TypeRef::Named(wrapper_ty),
                        brand: op_param.brand.clone(),
                    });
                    pbt_params.push(json!({
                        "name": wrapper_name,
                        "gen": gen
                    }));

                    call_args.push(if op_param.ty.trim() == "bytes_view" {
                        list_expr([
                            ident("bytes.view"),
                            ident(wrapper_params.last().unwrap().name.clone()),
                        ])
                    } else {
                        ident(wrapper_params.last().unwrap().name.clone())
                    });
                }

                if !prop_ok {
                    continue;
                }

                imports.insert(prop_module_id.to_string());
                prop_idx += 1;
                let wrapper_name = format!("{gen_module_id}.prop_{prop_idx:04}");
                exports.insert(wrapper_name.clone());

                let mut call_items: Vec<Expr> = Vec::with_capacity(1 + call_args.len());
                call_items.push(ident(prop_symbol.to_string()));
                call_items.extend(call_args);
                let body = list_expr_vec(call_items);

                functions.push(x07ast::AstFunctionDef {
                    name: wrapper_name.clone(),
                    type_params: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    invariant: Vec::new(),
                    loop_contracts: Vec::new(),
                    params: wrapper_params,
                    result: x07ast::TypeRef::Named("bytes".to_string()),
                    result_brand: None,
                    body,
                });

                let test_id = format!("xtal/{module_id}/{op_id}/prop{prop_idx:04}");
                test_entries.push(json!({
                    "id": test_id,
                    "world": "solve-pure",
                    "entry": wrapper_name,
                    "expect": "pass",
                    "returns": "bytes_status_v1",
                    "pbt": {
                        "cases": 100,
                        "max_shrinks": 4096,
                        "params": pbt_params
                    }
                }));
            }
        }

        let file = x07ast::X07AstFile {
            schema_version: x07_contracts::X07AST_SCHEMA_VERSION.to_string(),
            kind: x07ast::X07AstKind::Module,
            module_id: gen_module_id.clone(),
            imports,
            exports,
            functions,
            async_functions: Vec::new(),
            extern_functions: Vec::new(),
            solve: None,
            meta: BTreeMap::new(),
        };

        let mut v = x07ast::x07ast_file_to_value(&file);
        x07ast::canon_value_jcs(&mut v);
        let mut bytes = serde_json::to_vec(&v)?;
        if bytes.last() != Some(&b'\n') {
            bytes.push(b'\n');
        }

        let rel_path = module_id_to_tests_path(module_id);
        outputs.insert(rel_path, bytes);
    }

    // Manifest.
    let mut manifest = json!({
        "schema_version": TESTS_MANIFEST_SCHEMA_VERSION,
        "tests": test_entries,
    });
    x07ast::canon_value_jcs(&mut manifest);
    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    if manifest_bytes.last() != Some(&b'\n') {
        manifest_bytes.push(b'\n');
    }
    outputs.insert(PathBuf::from("tests.json"), manifest_bytes);

    Ok(outputs)
}

fn module_id_to_tests_path(module_id: &str) -> PathBuf {
    let rel = module_id.replace('.', "/");
    PathBuf::from(rel).join("tests.x07.json")
}

fn sanitize_ident_segment(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    let first = out.chars().next().unwrap_or('_');
    if !(first.is_ascii_alphabetic() || first == '_') {
        out.insert(0, '_');
    }
    out
}

fn gen_test_defn(
    fn_name: &str,
    op: &SpecOperation,
    ex: &ExampleLine,
    ordinal: usize,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<x07ast::AstFunctionDef> {
    let mut body: Vec<Expr> = Vec::new();

    // Bind params in declared order.
    let mut arg_syms: Vec<Expr> = Vec::new();
    for p in &op.params {
        let Some(arg_val) = ex.args.get(&p.name) else {
            diags.push(diag_error(
                "EXTAL_EXAMPLES_ARGS_MISSING",
                diagnostics::Stage::Lint,
                format!("example ex{ordinal:04}: missing arg {:?}", p.name),
                Some(diagnostics::Location::Text {
                    span: diagnostics::Span {
                        start: diagnostics::Position {
                            line: ex.line as u32,
                            col: 1,
                            offset: None,
                        },
                        end: diagnostics::Position {
                            line: ex.line as u32,
                            col: 1,
                            offset: None,
                        },
                        file: ex.file.clone(),
                    },
                    snippet: None,
                }),
            ));
            continue;
        };

        let lit = literal_expr_for_ty(
            &p.ty,
            arg_val,
            &format!("ex{ordinal:04} arg {}", p.name),
            &p.name,
            "EXTAL_EXAMPLES_ARG_KIND_UNSUPPORTED",
            diags,
        )?;
        if p.ty == "bytes_view" {
            let bytes_name = format!("{}_bytes", p.name);
            body.push(list_expr([ident("let"), ident(bytes_name.clone()), lit]));
            body.push(list_expr([
                ident("let"),
                ident(p.name.clone()),
                list_expr([ident("bytes.view"), ident(bytes_name)]),
            ]));
            arg_syms.push(ident(p.name.clone()));
        } else {
            body.push(list_expr([ident("let"), ident(p.name.clone()), lit]));
            arg_syms.push(ident(p.name.clone()));
        }
    }

    let extra: Vec<&String> = ex
        .args
        .keys()
        .filter(|k| !op.params.iter().any(|p| &p.name == *k))
        .collect();
    if !extra.is_empty() {
        diags.push(diag_error(
            "EXTAL_EXAMPLES_ARGS_EXTRA",
            diagnostics::Stage::Lint,
            format!("example ex{ordinal:04}: unexpected args: {:?}", extra),
            Some(diagnostics::Location::Text {
                span: diagnostics::Span {
                    start: diagnostics::Position {
                        line: ex.line as u32,
                        col: 1,
                        offset: None,
                    },
                    end: diagnostics::Position {
                        line: ex.line as u32,
                        col: 1,
                        offset: None,
                    },
                    file: ex.file.clone(),
                },
                snippet: None,
            }),
        ));
    }

    body.push(list_expr([
        ident("let"),
        ident("got"),
        list_expr_vec(
            std::iter::once(ident(op.name.clone()))
                .chain(arg_syms)
                .collect::<Vec<_>>(),
        ),
    ]));

    // Expect literal.
    let expect_lit = literal_expr_for_ty(
        &op.result,
        &ex.expect,
        &format!("ex{ordinal:04} expect"),
        "expect",
        "EXTAL_EXAMPLES_EXPECT_KIND_UNSUPPORTED",
        diags,
    )?;
    if op.result == "bytes_view" {
        body.push(list_expr([ident("let"), ident("expect_bytes"), expect_lit]));
        body.push(list_expr([
            ident("let"),
            ident("expect"),
            list_expr([ident("bytes.view"), ident("expect_bytes")]),
        ]));
    } else {
        body.push(list_expr([ident("let"), ident("expect"), expect_lit]));
    }

    let (assert_fn, assert_code_fn) = assertion_fns_for_result_ty(&op.result).ok_or_else(|| {
        anyhow::anyhow!("unsupported result type for assertions: {:?}", op.result)
    })?;
    body.push(list_expr([
        ident("try"),
        list_expr([
            ident(assert_fn),
            ident("got"),
            ident("expect"),
            list_expr([ident(assert_code_fn)]),
        ]),
    ]));
    body.push(list_expr([ident("std.test.pass")]));

    let begin = list_expr_vec(
        std::iter::once(ident("begin"))
            .chain(body)
            .collect::<Vec<_>>(),
    );

    Ok(x07ast::AstFunctionDef {
        name: fn_name.to_string(),
        type_params: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        invariant: Vec::new(),
        loop_contracts: Vec::new(),
        params: Vec::new(),
        result: x07ast::TypeRef::Named("result_i32".to_string()),
        result_brand: None,
        body: begin,
    })
}

fn assertion_fns_for_result_ty(result_ty: &str) -> Option<(&'static str, &'static str)> {
    match result_ty.trim() {
        "bytes" => Some(("std.test.assert_bytes_eq", "std.test.code_assert_bytes_eq")),
        "bytes_view" => Some(("std.test.assert_view_eq", "std.test.code_assert_view_eq")),
        "i32" => Some(("std.test.assert_i32_eq", "std.test.code_assert_i32_eq")),
        _ => None,
    }
}

fn literal_expr_for_ty(
    ty: &str,
    v: &Value,
    context: &str,
    hint: &str,
    unsupported_code: &'static str,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<Expr> {
    match ty.trim() {
        "bytes" | "bytes_view" => match decode_bytes_b64_value(v) {
            Ok(bytes) => Ok(bytes_constructor_expr(&bytes, &format!("{hint}_v"))),
            Err(err) => {
                let code = if err.contains("base64") {
                    "EXTAL_EXAMPLES_B64_INVALID"
                } else {
                    unsupported_code
                };
                diags.push(diag_error(
                    code,
                    diagnostics::Stage::Lint,
                    format!("{context}: {err}"),
                    None,
                ));
                Ok(int_expr(0))
            }
        },
        "i32" => match decode_i32_value(v) {
            Ok(n) => Ok(int_expr(n)),
            Err(err) => {
                diags.push(diag_error(
                    unsupported_code,
                    diagnostics::Stage::Lint,
                    format!("{context}: {err}"),
                    None,
                ));
                Ok(int_expr(0))
            }
        },
        other => {
            diags.push(diag_error(
                "EXTAL_GEN_UNSUPPORTED_TY",
                diagnostics::Stage::Lower,
                format!("{context}: unsupported type {other:?}"),
                None,
            ));
            Ok(int_expr(0))
        }
    }
}

fn bytes_constructor_expr(bytes: &[u8], vname: &str) -> Expr {
    let mut stmts: Vec<Expr> = Vec::new();
    stmts.push(list_expr([
        ident("let"),
        ident(vname),
        list_expr([
            ident("vec_u8.with_capacity"),
            int_expr(i32::try_from(bytes.len()).unwrap_or(i32::MAX)),
        ]),
    ]));
    for b in bytes {
        stmts.push(list_expr([
            ident("set"),
            ident(vname),
            list_expr([ident("vec_u8.push"), ident(vname), int_expr(*b as i32)]),
        ]));
    }
    stmts.push(list_expr([ident("vec_u8.into_bytes"), ident(vname)]));
    list_expr_vec(
        std::iter::once(ident("begin"))
            .chain(stmts)
            .collect::<Vec<_>>(),
    )
}

fn decode_b64(b64: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .context("decode base64")
}

fn decode_bytes_b64_value(v: &Value) -> Result<Vec<u8>, String> {
    let Value::Object(obj) = v else {
        return Err("expected bytes_b64 object encoding".to_string());
    };
    let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("");
    if kind != "bytes_b64" {
        return Err(format!("expected kind=\"bytes_b64\" (got {kind:?})"));
    }
    let b64 = obj
        .get("b64")
        .and_then(Value::as_str)
        .ok_or_else(|| "bytes_b64 encoding requires string field \"b64\"".to_string())?;
    decode_b64(b64).map_err(|e| format!("invalid base64: {e:#}"))
}

fn decode_i32_value(v: &Value) -> Result<i32, String> {
    if let Some(n) = v.as_i64() {
        return i32::try_from(n).map_err(|_| format!("number out of i32 range: {n}"));
    }
    let Value::Object(obj) = v else {
        return Err("expected i32 encoding".to_string());
    };
    let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("");
    if kind != "i32" {
        return Err(format!("expected kind=\"i32\" (got {kind:?})"));
    }
    let n = obj
        .get("i32")
        .or_else(|| obj.get("value"))
        .and_then(Value::as_i64)
        .ok_or_else(|| "i32 encoding requires integer field \"i32\" or \"value\"".to_string())?;
    i32::try_from(n).map_err(|_| format!("number out of i32 range: {n}"))
}

fn ident(name: impl Into<String>) -> Expr {
    Expr::Ident {
        name: name.into(),
        ptr: String::new(),
    }
}

fn int_expr(value: i32) -> Expr {
    Expr::Int {
        value,
        ptr: String::new(),
    }
}

fn list_expr<const N: usize>(items: [Expr; N]) -> Expr {
    Expr::List {
        items: items.into_iter().collect(),
        ptr: String::new(),
    }
}

fn list_expr_vec(items: Vec<Expr>) -> Expr {
    Expr::List {
        items,
        ptr: String::new(),
    }
}

fn is_supported_ty(ty: &str) -> bool {
    matches!(ty.trim(), "bytes" | "bytes_view" | "i32")
}

fn collect_spec_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for input in inputs {
        if input.is_file() {
            if seen.insert(input.clone()) {
                out.push(input.clone());
            }
            continue;
        }
        if input.is_dir() {
            let mut files = Vec::new();
            for entry in WalkDir::new(input)
                .follow_links(false)
                .into_iter()
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.into_path();
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".x07spec.json"))
                {
                    files.push(path);
                }
            }
            files.sort();
            for p in files {
                if seen.insert(p.clone()) {
                    out.push(p);
                }
            }
            continue;
        }
        anyhow::bail!(
            "--input does not exist or is not a file/dir: {}",
            input.display()
        );
    }
    Ok(out)
}

fn collect_spec_files(
    spec_root: &Path,
    explicit: &[PathBuf],
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Vec<PathBuf> {
    if !explicit.is_empty() {
        match collect_spec_inputs(explicit) {
            Ok(v) => return v,
            Err(err) => {
                diags.push(diag_error(
                    "EXTAL_SPEC_IO_READ_FAILED",
                    diagnostics::Stage::Parse,
                    format!("{err:#}"),
                    None,
                ));
                return Vec::new();
            }
        }
    }
    if !spec_root.is_dir() {
        return Vec::new();
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(spec_root)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".x07spec.json"))
            && !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('_'))
        {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn lint_one_spec_file(path: &Path) -> Result<(Option<Value>, Vec<diagnostics::Diagnostic>)> {
    let mut diags = Vec::new();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(err) => {
            diags.push(spec_error(
                "EXTAL_SPEC_IO_READ_FAILED",
                diagnostics::Stage::Parse,
                path,
                None,
                format!("cannot read file: {err}"),
            ));
            return Ok((None, diags));
        }
    };
    let doc: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(err) => {
            diags.push(spec_error(
                "EXTAL_SPEC_JSON_PARSE",
                diagnostics::Stage::Parse,
                path,
                None,
                format!("invalid JSON: {err}"),
            ));
            return Ok((None, diags));
        }
    };

    let schema_version = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema_version != SPEC_SCHEMA_VERSION {
        diags.push(spec_error(
            "EXTAL_SPEC_SCHEMA_VERSION_UNSUPPORTED",
            diagnostics::Stage::Parse,
            path,
            Some(diagnostics::Location::X07Ast {
                ptr: "/schema_version".to_string(),
            }),
            format!(
                "unsupported schema_version: expected {SPEC_SCHEMA_VERSION:?} got {schema_version:?}"
            ),
        ));
    }

    let schema_diags = report_common::validate_schema(
        SPEC_SCHEMA_BYTES,
        "spec/x07.x07spec@0.1.0.schema.json",
        &doc,
    )?;
    for d in schema_diags {
        diags.push(remap_schema_diag("EXTAL_SPEC_SCHEMA_INVALID", path, d));
    }

    Ok((Some(doc), diags))
}

fn check_spec_examples(
    project_root: &Path,
    spec_path: &Path,
    spec: &SpecFile,
) -> Result<Vec<diagnostics::Diagnostic>> {
    let mut diags = Vec::new();
    let mut ops_by_id = BTreeMap::new();
    for (op_idx, op) in spec.operations.iter().enumerate() {
        if let Some(id) = op.id.as_deref() {
            ops_by_id.insert(id.to_string(), (op_idx, op));
        }
    }

    let mut seen_paths = BTreeSet::new();
    for (op_idx, op) in spec.operations.iter().enumerate() {
        let Some(ex_ref) = op.examples_ref.as_deref() else {
            continue;
        };
        if !seen_paths.insert(ex_ref.to_string()) {
            continue;
        }
        let examples_path = project_root.join(ex_ref);
        let lines = read_examples_file(&examples_path, &mut diags)?;

        for line in &lines {
            if line.schema_version != EXAMPLES_SCHEMA_VERSION {
                diags.push(spec_error(
                    "EXTAL_EXAMPLES_SCHEMA_VERSION_UNSUPPORTED",
                    diagnostics::Stage::Parse,
                    &examples_path,
                    Some(diagnostics::Location::Text {
                        span: diagnostics::Span {
                            start: diagnostics::Position {
                                line: line.line as u32,
                                col: 1,
                                offset: None,
                            },
                            end: diagnostics::Position {
                                line: line.line as u32,
                                col: 1,
                                offset: None,
                            },
                            file: Some(examples_path.display().to_string()),
                        },
                        snippet: None,
                    }),
                    format!(
                        "unsupported schema_version: expected {EXAMPLES_SCHEMA_VERSION:?} got {:?}",
                        line.schema_version
                    ),
                ));
            }

            let Some((_, op_def)) = ops_by_id.get(&line.op) else {
                diags.push(spec_error(
                    "EXTAL_EXAMPLES_OP_UNKNOWN",
                    diagnostics::Stage::Lint,
                    &examples_path,
                    Some(diagnostics::Location::Text {
                        span: diagnostics::Span {
                            start: diagnostics::Position {
                                line: line.line as u32,
                                col: 1,
                                offset: None,
                            },
                            end: diagnostics::Position {
                                line: line.line as u32,
                                col: 1,
                                offset: None,
                            },
                            file: Some(examples_path.display().to_string()),
                        },
                        snippet: None,
                    }),
                    format!("unknown op id: {:?}", line.op),
                ));
                continue;
            };

            // required args present / no extras
            for p in &op_def.params {
                if !line.args.contains_key(&p.name) {
                    diags.push(spec_error(
                        "EXTAL_EXAMPLES_ARGS_MISSING",
                        diagnostics::Stage::Lint,
                        &examples_path,
                        Some(diagnostics::Location::Text {
                            span: diagnostics::Span {
                                start: diagnostics::Position {
                                    line: line.line as u32,
                                    col: 1,
                                    offset: None,
                                },
                                end: diagnostics::Position {
                                    line: line.line as u32,
                                    col: 1,
                                    offset: None,
                                },
                                file: Some(examples_path.display().to_string()),
                            },
                            snippet: None,
                        }),
                        format!("missing arg {:?} for op {:?}", p.name, line.op),
                    ));
                }
            }
            for k in line.args.keys() {
                if !op_def.params.iter().any(|p| &p.name == k) {
                    diags.push(spec_error(
                        "EXTAL_EXAMPLES_ARGS_EXTRA",
                        diagnostics::Stage::Lint,
                        &examples_path,
                        Some(diagnostics::Location::Text {
                            span: diagnostics::Span {
                                start: diagnostics::Position {
                                    line: line.line as u32,
                                    col: 1,
                                    offset: None,
                                },
                                end: diagnostics::Position {
                                    line: line.line as u32,
                                    col: 1,
                                    offset: None,
                                },
                                file: Some(examples_path.display().to_string()),
                            },
                            snippet: None,
                        }),
                        format!("extra arg {:?} for op {:?}", k, line.op),
                    ));
                }
            }

            // encoding matches param/result tys
            for p in &op_def.params {
                let Some(v) = line.args.get(&p.name) else {
                    continue;
                };
                if let Err(issue) = validate_example_value_for_ty(&p.ty, v) {
                    let (code, err) = match issue {
                        ExampleValueIssue::InvalidBase64(msg) => {
                            ("EXTAL_EXAMPLES_B64_INVALID", msg)
                        }
                        ExampleValueIssue::Unsupported(msg) => {
                            ("EXTAL_EXAMPLES_ARG_KIND_UNSUPPORTED", msg)
                        }
                    };
                    diags.push(spec_error(
                        code,
                        diagnostics::Stage::Lint,
                        &examples_path,
                        Some(diagnostics::Location::Text {
                            span: diagnostics::Span {
                                start: diagnostics::Position {
                                    line: line.line as u32,
                                    col: 1,
                                    offset: None,
                                },
                                end: diagnostics::Position {
                                    line: line.line as u32,
                                    col: 1,
                                    offset: None,
                                },
                                file: Some(examples_path.display().to_string()),
                            },
                            snippet: None,
                        }),
                        format!("arg {:?}: {err}", p.name),
                    ));
                }
            }
            if let Err(issue) = validate_example_value_for_ty(&op_def.result, &line.expect) {
                let (code, err) = match issue {
                    ExampleValueIssue::InvalidBase64(msg) => ("EXTAL_EXAMPLES_B64_INVALID", msg),
                    ExampleValueIssue::Unsupported(msg) => {
                        ("EXTAL_EXAMPLES_EXPECT_KIND_UNSUPPORTED", msg)
                    }
                };
                diags.push(spec_error(
                    code,
                    diagnostics::Stage::Lint,
                    &examples_path,
                    Some(diagnostics::Location::Text {
                        span: diagnostics::Span {
                            start: diagnostics::Position {
                                line: line.line as u32,
                                col: 1,
                                offset: None,
                            },
                            end: diagnostics::Position {
                                line: line.line as u32,
                                col: 1,
                                offset: None,
                            },
                            file: Some(examples_path.display().to_string()),
                        },
                        snippet: None,
                    }),
                    err,
                ));
            }
        }

        // Spec points to examples; keep pointers for missing file already checked.
        let _ = op_idx;
        let _ = spec_path;
    }

    Ok(diags)
}

enum ExampleValueIssue {
    Unsupported(String),
    InvalidBase64(String),
}

fn validate_example_value_for_ty(ty: &str, v: &Value) -> Result<(), ExampleValueIssue> {
    match ty.trim() {
        "bytes" | "bytes_view" => match decode_bytes_b64_value(v) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.contains("invalid base64") {
                    Err(ExampleValueIssue::InvalidBase64(err))
                } else {
                    Err(ExampleValueIssue::Unsupported(err))
                }
            }
        },
        "i32" => decode_i32_value(v)
            .map(|_| ())
            .map_err(ExampleValueIssue::Unsupported),
        _ => Err(ExampleValueIssue::Unsupported(format!(
            "unsupported ty {ty:?}"
        ))),
    }
}

fn read_examples_file(
    path: &Path,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<Vec<ExampleLine>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(err) => {
            diags.push(spec_error(
                "EXTAL_EXAMPLES_IO_READ_FAILED",
                diagnostics::Stage::Parse,
                path,
                None,
                format!("cannot read file: {err}"),
            ));
            return Ok(Vec::new());
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut out = Vec::new();

    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        let doc: Value = match serde_json::from_str(s) {
            Ok(v) => v,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_EXAMPLES_JSON_PARSE",
                    diagnostics::Stage::Parse,
                    path,
                    Some(diagnostics::Location::Text {
                        span: diagnostics::Span {
                            start: diagnostics::Position {
                                line: line_no as u32,
                                col: 1,
                                offset: None,
                            },
                            end: diagnostics::Position {
                                line: line_no as u32,
                                col: 1,
                                offset: None,
                            },
                            file: Some(path.display().to_string()),
                        },
                        snippet: None,
                    }),
                    format!("invalid JSON: {err}"),
                ));
                continue;
            }
        };

        let schema_diags = report_common::validate_schema(
            EXAMPLES_SCHEMA_BYTES,
            "spec/x07.x07spec_examples@0.1.0.schema.json",
            &doc,
        )?;
        for d in schema_diags {
            diags.push(remap_schema_diag("EXTAL_EXAMPLES_SCHEMA_INVALID", path, d));
        }

        let parsed: ExampleDoc = match serde_json::from_value(doc) {
            Ok(v) => v,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_EXAMPLES_SCHEMA_INVALID",
                    diagnostics::Stage::Parse,
                    path,
                    Some(diagnostics::Location::Text {
                        span: diagnostics::Span {
                            start: diagnostics::Position {
                                line: line_no as u32,
                                col: 1,
                                offset: None,
                            },
                            end: diagnostics::Position {
                                line: line_no as u32,
                                col: 1,
                                offset: None,
                            },
                            file: Some(path.display().to_string()),
                        },
                        snippet: None,
                    }),
                    format!("examples line shape invalid: {err}"),
                ));
                continue;
            }
        };
        let _ = parsed.tags.len();
        let _ = parsed.doc.as_deref();
        out.push(ExampleLine {
            file: Some(path.display().to_string()),
            line: line_no,
            schema_version: parsed.schema_version,
            op: parsed.op,
            args: parsed.args,
            expect: parsed.expect,
        });
    }

    Ok(out)
}

fn typecheck_spec_contracts(
    spec_path: &Path,
    spec: &SpecFile,
) -> Result<Vec<diagnostics::Diagnostic>> {
    let mut out = Vec::new();
    let mut functions = Vec::new();

    for (op_idx, op) in spec.operations.iter().enumerate() {
        if op.requires.is_empty() && op.ensures.is_empty() && op.invariant.is_empty() {
            continue;
        }
        if !is_supported_ty(&op.result) {
            continue;
        }

        let mut requires = Vec::new();
        for (c_idx, clause) in op.requires.iter().enumerate() {
            requires.push(clause_to_contract(
                spec_path,
                &format!("/operations/{op_idx}/requires/{c_idx}"),
                clause,
                &mut out,
            )?);
        }
        let mut ensures = Vec::new();
        for (c_idx, clause) in op.ensures.iter().enumerate() {
            ensures.push(clause_to_contract(
                spec_path,
                &format!("/operations/{op_idx}/ensures/{c_idx}"),
                clause,
                &mut out,
            )?);
        }
        let mut invariant = Vec::new();
        for (c_idx, clause) in op.invariant.iter().enumerate() {
            invariant.push(clause_to_contract(
                spec_path,
                &format!("/operations/{op_idx}/invariant/{c_idx}"),
                clause,
                &mut out,
            )?);
        }

        let params = op
            .params
            .iter()
            .map(|p| x07ast::AstFunctionParam {
                name: p.name.clone(),
                ty: x07ast::TypeRef::Named(p.ty.clone()),
                brand: p.brand.clone(),
            })
            .collect();

        functions.push(x07ast::AstFunctionDef {
            name: op.name.clone(),
            type_params: Vec::new(),
            requires,
            ensures,
            invariant,
            loop_contracts: Vec::new(),
            params,
            result: x07ast::TypeRef::Named(op.result.clone()),
            result_brand: op.result_brand.clone(),
            body: int_expr(0),
        });
    }

    for (s_idx, sort) in spec.sorts.iter().enumerate() {
        if sort.invariant.is_empty() {
            continue;
        }
        let mut inv = Vec::new();
        for (c_idx, clause) in sort.invariant.iter().enumerate() {
            inv.push(clause_to_contract(
                spec_path,
                &format!("/sorts/{s_idx}/invariant/{c_idx}"),
                clause,
                &mut out,
            )?);
        }

        functions.push(x07ast::AstFunctionDef {
            name: format!(
                "{}.{}_invariant_v1",
                spec.module_id,
                sanitize_ident_segment(&sort.name)
            ),
            type_params: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            invariant: inv,
            loop_contracts: Vec::new(),
            params: Vec::new(),
            result: x07ast::TypeRef::Named("i32".to_string()),
            result_brand: None,
            body: int_expr(0),
        });
    }

    let file = x07ast::X07AstFile {
        schema_version: x07_contracts::X07AST_SCHEMA_VERSION.to_string(),
        kind: x07ast::X07AstKind::Module,
        module_id: spec.module_id.clone(),
        imports: BTreeSet::new(),
        exports: BTreeSet::new(),
        functions,
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: None,
        meta: BTreeMap::new(),
    };

    let report = x07c::typecheck::typecheck_file_local(
        &file,
        &x07c::typecheck::TypecheckOptions {
            mode: x07c::typecheck::TypecheckMode::ContractsOnly,
            compat: x07c::compat::Compat::default(),
        },
    );

    for d in report.diagnostics {
        out.push(remap_contract_diag(spec_path, d));
    }

    Ok(out)
}

fn clause_to_contract(
    spec_path: &Path,
    base_ptr: &str,
    clause: &SpecClause,
    diags: &mut Vec<diagnostics::Diagnostic>,
) -> Result<x07ast::ContractClauseAst> {
    let mut expr = match x07c::ast::expr_from_json(&clause.expr) {
        Ok(e) => e,
        Err(err) => {
            diags.push(spec_error(
                "EXTAL_SPEC_CONTRACT_EXPR_PARSE",
                diagnostics::Stage::Parse,
                spec_path,
                Some(diagnostics::Location::X07Ast {
                    ptr: format!("{base_ptr}/expr"),
                }),
                err,
            ));
            int_expr(0)
        }
    };
    reptr_expr(&mut expr, &format!("{base_ptr}/expr"));

    let mut witness = Vec::new();
    for (w_idx, w) in clause.witness.iter().enumerate() {
        let mut wexpr = match x07c::ast::expr_from_json(w) {
            Ok(e) => e,
            Err(err) => {
                diags.push(spec_error(
                    "EXTAL_SPEC_CONTRACT_WITNESS_INVALID",
                    diagnostics::Stage::Parse,
                    spec_path,
                    Some(diagnostics::Location::X07Ast {
                        ptr: format!("{base_ptr}/witness/{w_idx}"),
                    }),
                    err,
                ));
                int_expr(0)
            }
        };
        reptr_expr(&mut wexpr, &format!("{base_ptr}/witness/{w_idx}"));
        witness.push(wexpr);
    }

    Ok(x07ast::ContractClauseAst {
        id: clause.id.clone(),
        expr,
        witness,
    })
}

fn reptr_expr(expr: &mut Expr, ptr: &str) {
    match expr {
        Expr::Int { ptr: p, .. } | Expr::Ident { ptr: p, .. } => {
            *p = ptr.to_string();
        }
        Expr::List { items, ptr: p } => {
            *p = ptr.to_string();
            for (idx, item) in items.iter_mut().enumerate() {
                reptr_expr(item, &format!("{ptr}/{idx}"));
            }
        }
    }
}

fn remap_contract_diag(spec_path: &Path, d: diagnostics::Diagnostic) -> diagnostics::Diagnostic {
    let mut out = d.clone();
    let new_code = match d.code.as_str() {
        "X07-CONTRACT-0001" => "EXTAL_SPEC_CONTRACT_EXPR_NOT_I32",
        "X07-CONTRACT-0002" => {
            let callee = extract_disallowed_callee(&d.message).unwrap_or_default();
            if looks_like_module_call(&callee) {
                "EXTAL_SPEC_CONTRACT_MODULE_CALL_FORBIDDEN"
            } else {
                "EXTAL_SPEC_CONTRACT_BUILTIN_DISALLOWED"
            }
        }
        "X07-CONTRACT-0003" => "EXTAL_SPEC_CONTRACT_USES_RESULT_OUTSIDE_ENSURES",
        "X07-CONTRACT-0005" => "EXTAL_SPEC_CONTRACT_WITNESS_INVALID",
        _ => "EXTAL_SPEC_CONTRACT_EXPR_NOT_I32",
    };
    out.code = new_code.to_string();
    out.data.insert(
        "file".to_string(),
        Value::String(spec_path.display().to_string()),
    );
    out.data
        .insert("x07_code".to_string(), Value::String(d.code));
    out
}

fn extract_disallowed_callee(message: &str) -> Option<String> {
    // Messages look like: contract expression is not pure: disallowed call "std.world.fs.read_file"
    let (_, tail) = message.split_once("disallowed ")?;
    let (_, quoted) = tail.split_once(' ')?;
    let q = quoted.trim();
    if !q.starts_with('"') || !q.ends_with('"') {
        return None;
    }
    Some(q.trim_matches('"').to_string())
}

fn looks_like_module_call(head: &str) -> bool {
    let h = head.trim();
    if h.is_empty() {
        return false;
    }
    if h.starts_with("bytes.") || h.starts_with("view.") {
        return false;
    }
    h.contains('.')
}

fn inject_missing_ids(spec: &mut SpecFile) {
    for (s_idx, sort) in spec.sorts.iter_mut().enumerate() {
        if sort.id.as_deref().unwrap_or("").trim().is_empty() {
            sort.id = Some(format!("sort.{}.v1", sanitize_ident_segment(&sort.name)));
        }
        inject_clause_ids(&mut sort.invariant, &format!("sort.{s_idx}.invariant"));
    }

    for (op_idx, op) in spec.operations.iter_mut().enumerate() {
        if op.id.as_deref().unwrap_or("").trim().is_empty() {
            let short = op
                .name
                .rsplit_once('.')
                .map(|(_, s)| s)
                .unwrap_or(op.name.as_str());
            op.id = Some(format!("op.{}.v1", sanitize_ident_segment(short)));
        }

        inject_clause_ids(&mut op.requires, &format!("op.{op_idx}.requires"));
        inject_clause_ids(&mut op.ensures, &format!("op.{op_idx}.ensures"));
        inject_clause_ids(&mut op.invariant, &format!("op.{op_idx}.invariant"));
    }
}

fn inject_clause_ids(clauses: &mut [SpecClause], prefix: &str) {
    let mut seen = BTreeSet::new();
    for c in clauses.iter() {
        if let Some(id) = c.id.as_deref() {
            let _ = seen.insert(id.to_string());
        }
    }
    for (idx, c) in clauses.iter_mut().enumerate() {
        if c.id.as_deref().unwrap_or("").trim().is_empty() {
            let mut base = format!("{prefix}.{}", idx + 1);
            if seen.contains(&base) {
                base = format!("{base}_{}", idx + 1);
            }
            seen.insert(base.clone());
            c.id = Some(base);
        }
    }
}

fn remap_schema_diag(
    code: &str,
    file: &Path,
    mut d: diagnostics::Diagnostic,
) -> diagnostics::Diagnostic {
    d.code = code.to_string();
    d.data.insert(
        "file".to_string(),
        Value::String(file.display().to_string()),
    );
    d
}

fn spec_error(
    code: &str,
    stage: diagnostics::Stage,
    file: &Path,
    loc: Option<diagnostics::Location>,
    message: impl Into<String>,
) -> diagnostics::Diagnostic {
    let mut d = diag_error(code, stage, message, loc);
    d.data.insert(
        "file".to_string(),
        Value::String(file.display().to_string()),
    );
    d
}

fn spec_warning(
    code: &str,
    stage: diagnostics::Stage,
    file: &Path,
    loc: Option<diagnostics::Location>,
    message: impl Into<String>,
) -> diagnostics::Diagnostic {
    let mut d = diag_warning(code, stage, message, loc);
    d.data.insert(
        "file".to_string(),
        Value::String(file.display().to_string()),
    );
    d
}

fn diag_error(
    code: &str,
    stage: diagnostics::Stage,
    message: impl Into<String>,
    loc: Option<diagnostics::Location>,
) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Error,
        stage,
        message: message.into(),
        loc,
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    }
}

fn diag_warning(
    code: &str,
    stage: diagnostics::Stage,
    message: impl Into<String>,
    loc: Option<diagnostics::Location>,
) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic {
        code: code.to_string(),
        severity: diagnostics::Severity::Warning,
        stage,
        message: message.into(),
        loc,
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    }
}

fn resolve_project_root(project_path: Option<&Path>, start: Option<&Path>) -> Result<PathBuf> {
    if let Some(project_path) = project_path {
        let project_path = if project_path.is_absolute() {
            project_path.to_path_buf()
        } else {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            cwd.join(project_path)
        };
        if project_path.is_file() {
            return Ok(project_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf());
        }
        anyhow::bail!(
            "--project must point to x07.json (got {})",
            project_path.display()
        );
    }

    let start = start
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let mut dir: Option<&Path> = Some(start.as_path());
    while let Some(d) = dir {
        if d.join("x07.json").is_file() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    anyhow::bail!("could not find x07.json (run from a project directory or pass --project)")
}

fn capture_report_json<F>(prefix: &str, f: F) -> Result<(std::process::ExitCode, Value)>
where
    F: FnOnce(&crate::reporting::MachineArgs) -> Result<std::process::ExitCode>,
{
    let pid = std::process::id();
    let n = TMP_N.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("x07_{prefix}_{pid}_{n}.x07diag.json"));
    let _ = std::fs::remove_file(&path);

    let tmp_machine = crate::reporting::MachineArgs {
        out: None,
        json: None,
        jsonl: false,
        json_schema: false,
        json_schema_id: false,
        report_out: Some(path.clone()),
        quiet_json: true,
    };

    let code = f(&tmp_machine)?;
    let bytes =
        std::fs::read(&path).with_context(|| format!("read temp report: {}", path.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse temp report JSON: {}", path.display()))?;
    let _ = std::fs::remove_file(&path);
    Ok((code, value))
}

struct ToolRunOutcome {
    exit_code: i32,
    stderr: Vec<u8>,
}

fn run_self_command(cwd: &Path, args: &[String]) -> Result<ToolRunOutcome> {
    let exe = std::env::current_exe().context("resolve current x07 executable")?;
    let out = Command::new(exe)
        .current_dir(cwd)
        .env("X07_TOOL_API_CHILD", "1")
        .args(args)
        .output()
        .with_context(|| format!("run x07 command in {}", cwd.display()))?;
    Ok(ToolRunOutcome {
        exit_code: out.status.code().unwrap_or(-1),
        stderr: out.stderr,
    })
}

fn stderr_summary(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr).trim().to_string();
    if text.is_empty() {
        "no stderr output".to_string()
    } else {
        text
    }
}

fn write_report(
    machine: &crate::reporting::MachineArgs,
    report: &diagnostics::Report,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(report)?;
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }

    if let Some(path) = machine.report_out.as_deref() {
        if path.as_os_str() == std::ffi::OsStr::new("-") {
            anyhow::bail!("--report-out '-' is not supported (stdout is reserved for the report)");
        }
        crate::reporting::write_bytes(path, &bytes)?;
    }
    if machine.quiet_json {
        return Ok(());
    }
    std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
    Ok(())
}

fn file_digest_value(path: &Path) -> Option<Value> {
    crate::reporting::file_digest(path)
        .ok()
        .and_then(|d| serde_json::to_value(d).ok())
}

fn merge_meta_digests(report: &Value, meta_key: &str, out_by_path: &mut BTreeMap<String, Value>) {
    let Some(arr) = report
        .get("meta")
        .and_then(Value::as_object)
        .and_then(|m| m.get(meta_key))
        .and_then(Value::as_array)
    else {
        return;
    };

    for v in arr {
        let Some(path) = v.get("path").and_then(Value::as_str) else {
            continue;
        };
        out_by_path
            .entry(path.to_string())
            .or_insert_with(|| v.clone());
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct XtalManifest {
    schema_version: String,
    #[allow(dead_code)]
    #[serde(default)]
    xtal_version: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    spec_roots: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    impl_roots: Vec<String>,
    #[serde(default)]
    entrypoints: Vec<XtalEntrypoint>,
    #[allow(dead_code)]
    #[serde(default)]
    profiles: Option<XtalProfiles>,
    #[allow(dead_code)]
    #[serde(default)]
    sandbox: Option<XtalSandbox>,
    #[serde(default)]
    trust: Option<XtalTrust>,
    #[serde(default)]
    autonomy: Option<XtalAutonomy>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct XtalEntrypoint {
    name: String,
    #[allow(dead_code)]
    kind: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct XtalProfiles {
    dev_world: String,
    ci_world: String,
    prod_world: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct XtalSandbox {
    policy: String,
    backend: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct XtalTrust {
    #[serde(default)]
    review_gates: Vec<String>,
    cert_profile: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct XtalAutonomy {
    #[serde(default)]
    agent_write_paths: Vec<String>,
    #[serde(default)]
    agent_write_specs: bool,
    #[serde(default)]
    agent_write_arch: bool,
    #[allow(dead_code)]
    #[serde(default)]
    max_repair_iters: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecFile {
    schema_version: String,
    module_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ids: Option<BTreeMap<String, String>>,
    #[serde(default)]
    sorts: Vec<SpecSort>,
    operations: Vec<SpecOperation>,
    #[serde(default)]
    assumptions: Vec<SpecAssumption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nonfunctional: Option<SpecNonfunctional>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecSort {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
    #[serde(default)]
    invariant: Vec<SpecClause>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecOperation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
    params: Vec<SpecParam>,
    result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result_brand: Option<String>,
    #[serde(default)]
    requires: Vec<SpecClause>,
    #[serde(default)]
    ensures: Vec<SpecClause>,
    #[serde(default)]
    invariant: Vec<SpecClause>,
    #[serde(default)]
    ensures_props: Vec<SpecProp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    examples_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecParam {
    name: String,
    ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    brand: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecClause {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    expr: Value,
    #[serde(default)]
    witness: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecProp {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    prop: String,
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecAssumption {
    id: String,
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    severity: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecNonfunctional {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    determinism: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capability_profile: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExampleDoc {
    schema_version: String,
    op: String,
    args: BTreeMap<String, Value>,
    expect: Value,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    doc: Option<String>,
}

#[derive(Debug, Clone)]
struct ExampleLine {
    file: Option<String>,
    line: usize,
    schema_version: String,
    op: String,
    args: BTreeMap<String, Value>,
    expect: Value,
}
