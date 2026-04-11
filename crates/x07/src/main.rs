#![recursion_limit = "256"]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use anyhow::{Context, Result};
use base64::Engine;
use clap::{Args, Parser};
use serde_json::Value;
use x07_contracts::{
    PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED, X07AST_SCHEMA_VERSION, X07TEST_SCHEMA_VERSION,
};
use x07_host_runner::{run_artifact_file, RunnerConfig, RunnerResult};
use x07_worlds::WorldId;
use x07c::project;

mod agent;
mod arch;
mod assets_cmd;
mod ast;
mod ast_slice_engine;
mod bench;
mod bundle;
mod cli;
mod contract_repro;
mod delegate;
mod diag;
mod doc;
mod doctor;
mod fix_suggest;
mod guide;
mod init;
mod patch;
mod pbt;
mod pbt_fix;
mod pkg;
mod policy;
mod policy_overrides;
mod prove;
mod repair;
mod report_common;
mod reporting;
mod repro;
mod review;
mod rr;
mod run;
mod schema;
mod service;
mod service_genpack;
mod service_validate;
mod sm;
mod tool_api;
mod tool_report_schemas;
mod toolchain;
mod trust;
mod util;
mod verify;
mod x07ast_util;

#[derive(Parser, Debug)]
#[command(name = "x07")]
#[command(about = "X07 toolchain utilities.", long_about = None)]
#[command(version)]
#[command(subcommand_required = false)]
struct Cli {
    #[arg(long, global = true)]
    cli_specrows: bool,

    #[command(flatten)]
    machine: reporting::MachineArgs,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Create a new X07 project skeleton (and agent kit).
    Init(init::InitArgs),
    /// Run deterministic test suites.
    Test(TestArgs),
    /// Run x07bench suites (agent correctness benchmark harness).
    Bench(bench::BenchArgs),
    /// Check architecture manifests (architecture as data).
    Arch(arch::ArchArgs),
    /// Embed and manage asset bundles (codegen helpers).
    Assets(assets_cmd::AssetsArgs),
    /// Run X07 programs via the appropriate runner.
    Run(Box<run::RunArgs>),
    /// Produce a distributable native executable (no toolchain required at runtime).
    Bundle(Box<bundle::BundleArgs>),
    /// Print the built-in language + stdlib guide.
    Guide(guide::GuideArgs),
    /// Check platform prerequisites for OS worlds.
    Doctor(doctor::DoctorArgs),
    /// Inspect and enforce diagnostics catalog/coverage.
    Diag(diag::DiagArgs),
    /// Generate and manage run-os-sandboxed policy files.
    Policy(policy::PolicyArgs),
    /// Initialize, validate, and patch x07AST JSON files.
    Ast(ast::AstArgs),
    /// Agent-focused artifacts and utilities.
    Agent(agent::AgentArgs),
    /// Format x07AST JSON files.
    Fmt(toolchain::FmtArgs),
    /// Lint x07AST JSON files.
    Lint(toolchain::LintArgs),
    /// Apply deterministic quickfixes to an x07AST JSON file.
    Fix(toolchain::FixArgs),
    /// Apply mechanical migrations to update code for a newer compat mode.
    Migrate(toolchain::MigrateArgs),
    /// Build a project to C.
    Build(toolchain::BuildArgs),
    /// Check a project (lint + typecheck + backend-check; no emit).
    Check(toolchain::CheckArgs),
    /// Service authoring, archetype discovery, and validation.
    Service(service::ServiceArgs),
    /// Work with CLI specrows schemas and tooling.
    Cli(cli::CliArgs),
    /// Manage packages and lockfiles.
    Pkg(pkg::PkgArgs),
    /// Proof-object tooling.
    Prove(prove::ProveArgs),
    /// Produce human review artifacts (semantic diffs).
    Review(review::ReviewArgs),
    /// Emit CI trust artifacts (budgets/caps, capabilities, nondeterminism, SBOM artifacts).
    Trust(trust::TrustArgs),
    /// Apply deterministic multi-file JSON patchsets.
    Patch(patch::PatchArgs),
    /// Inspect module exports and signatures.
    Doc(doc::DocArgs),
    /// Derive schema modules from x07schema JSON.
    Schema(schema::SchemaArgs),
    /// Generate and validate state machines.
    Sm(sm::SmArgs),
    /// Record RR fixtures.
    #[command(hide = true)]
    Rr(rr::RrArgs),
    /// Verify contracts within bounds (BMC / SMT).
    Verify(verify::VerifyArgs),
    /// MCP server kit tooling (delegates to `x07-mcp`).
    Mcp(McpArgs),
    /// WASM tooling (delegates to `x07-wasm`).
    Wasm(WasmArgs),
}

#[derive(Debug, Clone, Args)]
struct McpArgs {
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARG"
    )]
    args: Vec<OsString>,
}

#[derive(Debug, Clone, Args)]
struct WasmArgs {
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARG"
    )]
    args: Vec<OsString>,
}

#[derive(Debug, Clone, Copy)]
enum Expect {
    Pass,
    Fail,
    Skip,
}

impl Expect {
    fn as_str(self) -> &'static str {
        match self {
            Expect::Pass => "pass",
            Expect::Fail => "fail",
            Expect::Skip => "skip",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestReturns {
    ResultI32,
    BytesStatusV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestEntryKind {
    Defn,
    Defasync,
}

impl TestEntryKind {
    fn as_str(self) -> &'static str {
        match self {
            TestEntryKind::Defn => "defn",
            TestEntryKind::Defasync => "defasync",
        }
    }
}

#[derive(Debug, Clone)]
struct TestDecl {
    id: String,
    world: WorldId,
    entry: String,
    entry_kind: TestEntryKind,
    entry_result_ty: String,
    expect: Expect,
    returns: TestReturns,
    pbt: Option<pbt::PbtDecl>,
    input: Option<Vec<u8>>,
    fixture_root: Option<PathBuf>,
    policy_json: Option<PathBuf>,
    require_runtime_attestation: bool,
    required_capsules: Vec<String>,
    sandbox_smoke: bool,
    timeout_ms: Option<u64>,
    solve_fuel: Option<u64>,
}

#[derive(Debug, Clone, Args)]
struct TestArgs {
    #[arg(long, value_name = "PATH", default_value = "tests/tests.json")]
    manifest: PathBuf,

    /// Module root directory for resolving module ids.
    /// May be passed multiple times.
    ///
    /// Defaults:
    /// - if a project `x07.json` exists (searched upwards from the manifest dir), use the project's
    ///   resolved module roots (including dependencies from the lockfile)
    /// - otherwise, use the manifest directory
    #[arg(long, value_name = "DIR")]
    module_root: Vec<PathBuf>,

    #[arg(long, value_name = "PATH", default_value = "stdlib.lock")]
    stdlib_lock: PathBuf,

    #[arg(long, value_enum, hide = true)]
    world: Option<WorldId>,

    /// Override the language/toolchain compatibility mode.
    #[arg(long, value_name = "COMPAT")]
    compat: Option<String>,

    #[arg(long, value_name = "SUBSTR")]
    filter: Option<String>,

    #[arg(long)]
    exact: bool,

    /// Allow filters that select zero tests (default: treat as an error).
    #[arg(long)]
    allow_empty: bool,

    /// Run property-based tests only (tests where `pbt` is set in the manifest).
    #[arg(long)]
    pbt: bool,

    /// Run both unit tests and property-based tests.
    #[arg(long)]
    all: bool,

    /// Override the PBT suite seed (default: 0).
    #[arg(long, value_name = "U64")]
    pbt_seed: Option<u64>,

    /// Override per-test generated case count.
    #[arg(long, value_name = "N")]
    pbt_cases: Option<u32>,

    /// Override per-test max shrink attempts.
    #[arg(long, value_name = "N")]
    pbt_max_shrinks: Option<u32>,

    /// Override per-case fuel cap.
    #[arg(long, value_name = "FUEL")]
    pbt_case_fuel: Option<u64>,

    /// Override per-case timeout cap (milliseconds; rounded up to seconds).
    #[arg(long, value_name = "MS")]
    pbt_case_timeout_ms: Option<u64>,

    /// Override per-case memory cap (bytes).
    #[arg(long, value_name = "BYTES")]
    pbt_case_mem_bytes: Option<u64>,

    /// Override per-case output cap (bytes).
    #[arg(long, value_name = "BYTES")]
    pbt_case_output_bytes: Option<u64>,

    /// Replay exactly one counterexample artifact (runs the single failing case only).
    #[arg(long, value_name = "PATH")]
    pbt_repro: Option<PathBuf>,

    #[arg(long)]
    list: bool,

    #[arg(long)]
    keep_artifacts: bool,

    #[arg(long, value_name = "DIR", default_value = "target/x07test")]
    artifact_dir: PathBuf,

    #[arg(long, value_name = "N", default_value_t = 1)]
    repeat: u32,

    #[arg(long, value_name = "N", default_value_t = 1)]
    jobs: usize,

    #[arg(long)]
    no_fail_fast: bool,

    #[arg(long)]
    no_run: bool,

    #[arg(long)]
    verbose: bool,
}

fn main() -> std::process::ExitCode {
    let raw_args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    match tool_api::maybe_handle(&raw_args) {
        Ok(Some(code)) => return code,
        Ok(None) => {}
        Err(err) => {
            eprintln!("{err:#}");
            return std::process::ExitCode::from(2);
        }
    }

    let run_main = || match try_main() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:#}");
            std::process::ExitCode::from(2)
        }
    };

    match std::thread::Builder::new()
        .name("x07-main".to_string())
        .stack_size(32 * 1024 * 1024)
        .spawn(run_main)
    {
        Ok(handle) => match handle.join() {
            Ok(code) => code,
            Err(_) => {
                eprintln!("x07 main thread panicked");
                std::process::ExitCode::from(101)
            }
        },
        Err(_) => run_main(),
    }
}

fn try_main() -> Result<std::process::ExitCode> {
    let cli = Cli::parse();

    if cli.cli_specrows {
        use clap::CommandFactory as _;
        let root = Cli::command();
        let path: Vec<&str> = match &cli.command {
            None => Vec::new(),
            Some(Command::Test(_)) => vec!["test"],
            Some(Command::Bench(args)) => match &args.cmd {
                None => vec!["bench"],
                Some(bench::BenchCommand::List(_)) => vec!["bench", "list"],
                Some(bench::BenchCommand::Validate(_)) => vec!["bench", "validate"],
                Some(bench::BenchCommand::Eval(_)) => vec!["bench", "eval"],
            },
            Some(Command::Init(_)) => vec!["init"],
            Some(Command::Arch(args)) => match &args.cmd {
                None => vec!["arch"],
                Some(arch::ArchCommand::Check(_)) => vec!["arch", "check"],
            },
            Some(Command::Assets(args)) => match &args.cmd {
                None => vec!["assets"],
                Some(assets_cmd::AssetsCommand::EmbedDir(_)) => vec!["assets", "embed-dir"],
            },
            Some(Command::Run(_)) => vec!["run"],
            Some(Command::Bundle(_)) => vec!["bundle"],
            Some(Command::Guide(_)) => vec!["guide"],
            Some(Command::Doctor(_)) => vec!["doctor"],
            Some(Command::Diag(args)) => match &args.cmd {
                None => vec!["diag"],
                Some(diag::DiagCommand::Catalog(_)) => vec!["diag", "catalog"],
                Some(diag::DiagCommand::InitCatalog(_)) => vec!["diag", "init-catalog"],
                Some(diag::DiagCommand::Explain(_)) => vec!["diag", "explain"],
                Some(diag::DiagCommand::Check(_)) => vec!["diag", "check"],
                Some(diag::DiagCommand::Coverage(_)) => vec!["diag", "coverage"],
                Some(diag::DiagCommand::Sarif(_)) => vec!["diag", "sarif"],
            },
            Some(Command::Policy(args)) => match &args.cmd {
                None => vec!["policy"],
                Some(policy::PolicyCommand::Init(_)) => vec!["policy", "init"],
            },
            Some(Command::Ast(args)) => match &args.cmd {
                None => vec!["ast"],
                Some(ast::AstCommand::Init(_)) => vec!["ast", "init"],
                Some(ast::AstCommand::Get(_)) => vec!["ast", "get"],
                Some(ast::AstCommand::Slice(_)) => vec!["ast", "slice"],
                Some(ast::AstCommand::ApplyPatch(_)) => vec!["ast", "apply-patch"],
                Some(ast::AstCommand::Edit(args)) => match &args.cmd {
                    ast::AstEditCommand::InsertStmts(_) => vec!["ast", "edit", "insert-stmts"],
                    ast::AstEditCommand::ApplyQuickfix(_) => vec!["ast", "edit", "apply-quickfix"],
                },
                Some(ast::AstCommand::Validate(_)) => vec!["ast", "validate"],
                Some(ast::AstCommand::Canon(_)) => vec!["ast", "canon"],
                Some(ast::AstCommand::Schema(_)) => vec!["ast", "schema"],
                Some(ast::AstCommand::Grammar(_)) => vec!["ast", "grammar"],
            },
            Some(Command::Agent(args)) => match &args.cmd {
                None => vec!["agent"],
                Some(agent::AgentCommand::Context(_)) => vec!["agent", "context"],
            },
            Some(Command::Fmt(_)) => vec!["fmt"],
            Some(Command::Lint(_)) => vec!["lint"],
            Some(Command::Fix(_)) => vec!["fix"],
            Some(Command::Migrate(_)) => vec!["migrate"],
            Some(Command::Build(_)) => vec!["build"],
            Some(Command::Check(_)) => vec!["check"],
            Some(Command::Service(args)) => match &args.cmd {
                None => vec!["service"],
                Some(service::ServiceCommand::Archetypes(_)) => vec!["service", "archetypes"],
                Some(service::ServiceCommand::Genpack(args)) => match &args.cmd {
                    None => vec!["service", "genpack"],
                    Some(service_genpack::ServiceGenpackCommand::Schema(_)) => {
                        vec!["service", "genpack", "schema"]
                    }
                    Some(service_genpack::ServiceGenpackCommand::Grammar(_)) => {
                        vec!["service", "genpack", "grammar"]
                    }
                },
                Some(service::ServiceCommand::Validate(_)) => vec!["service", "validate"],
            },
            Some(Command::Cli(args)) => match &args.cmd {
                None => vec!["cli"],
                Some(cli::CliCommand::Spec(args)) => match &args.cmd {
                    None => vec!["cli", "spec"],
                    Some(cli::SpecCommand::Fmt(_)) => vec!["cli", "spec", "fmt"],
                    Some(cli::SpecCommand::Check(_)) => vec!["cli", "spec", "check"],
                    Some(cli::SpecCommand::Compile(_)) => vec!["cli", "spec", "compile"],
                },
            },
            Some(Command::Pkg(args)) => match &args.cmd {
                None => vec!["pkg"],
                Some(pkg::PkgCommand::Add(_)) => vec!["pkg", "add"],
                Some(pkg::PkgCommand::Remove(_)) => vec!["pkg", "remove"],
                Some(pkg::PkgCommand::Versions(_)) => vec!["pkg", "versions"],
                Some(pkg::PkgCommand::Info(_)) => vec!["pkg", "info"],
                Some(pkg::PkgCommand::List(_)) => vec!["pkg", "list"],
                Some(pkg::PkgCommand::Pack(_)) => vec!["pkg", "pack"],
                Some(pkg::PkgCommand::Lock(_)) => vec!["pkg", "lock"],
                Some(pkg::PkgCommand::Repair(_)) => vec!["pkg", "repair"],
                Some(pkg::PkgCommand::AttestClosure(_)) => vec!["pkg", "attest-closure"],
                Some(pkg::PkgCommand::Provides(_)) => vec!["pkg", "provides"],
                Some(pkg::PkgCommand::Login(_)) => vec!["pkg", "login"],
                Some(pkg::PkgCommand::Publish(_)) => vec!["pkg", "publish"],
            },
            Some(Command::Prove(args)) => match &args.cmd {
                prove::ProveCommand::Check(_) => vec!["prove", "check"],
            },
            Some(Command::Review(args)) => match &args.cmd {
                None => vec!["review"],
                Some(review::ReviewCommand::Diff(_)) => vec!["review", "diff"],
            },
            Some(Command::Trust(args)) => match &args.cmd {
                None => vec!["trust"],
                Some(trust::TrustCommand::Report(_)) => vec!["trust", "report"],
                Some(trust::TrustCommand::Profile(args)) => match &args.cmd {
                    None => vec!["trust", "profile"],
                    Some(trust::TrustProfileCommand::Check(_)) => {
                        vec!["trust", "profile", "check"]
                    }
                },
                Some(trust::TrustCommand::Capsule(args)) => match &args.cmd {
                    None => vec!["trust", "capsule"],
                    Some(trust::TrustCapsuleCommand::Check(_)) => {
                        vec!["trust", "capsule", "check"]
                    }
                    Some(trust::TrustCapsuleCommand::Attest(_)) => {
                        vec!["trust", "capsule", "attest"]
                    }
                },
                Some(trust::TrustCommand::Certify(_)) => vec!["trust", "certify"],
            },
            Some(Command::Patch(args)) => match &args.cmd {
                None => vec!["patch"],
                Some(patch::PatchCommand::Apply(_)) => vec!["patch", "apply"],
            },
            Some(Command::Doc(_)) => vec!["doc"],
            Some(Command::Schema(args)) => match &args.cmd {
                None => vec!["schema"],
                Some(schema::SchemaCommand::Derive(_)) => vec!["schema", "derive"],
            },
            Some(Command::Sm(args)) => match &args.cmd {
                None => vec!["sm"],
                Some(sm::SmCommand::Check(_)) => vec!["sm", "check"],
                Some(sm::SmCommand::Gen(_)) => vec!["sm", "gen"],
            },
            Some(Command::Rr(args)) => match &args.cmd {
                None => vec!["rr"],
                Some(rr::RrCommand::Record(_)) => vec!["rr", "record"],
            },
            Some(Command::Verify(_)) => vec!["verify"],
            Some(Command::Mcp(_)) => vec!["mcp"],
            Some(Command::Wasm(_)) => vec!["wasm"],
        };

        let node = x07c::cli_specrows::find_command(&root, &path).unwrap_or(&root);
        let doc = x07c::cli_specrows::command_to_specrows(node);
        println!("{}", serde_json::to_string(&doc)?);
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let Some(command) = cli.command else {
        anyhow::bail!("missing subcommand (try --help)");
    };

    match command {
        Command::Init(args) => init::cmd_init(&cli.machine, args),
        Command::Test(args) => cmd_test(&cli.machine, args),
        Command::Bench(args) => bench::cmd_bench(&cli.machine, args),
        Command::Arch(args) => arch::cmd_arch(&cli.machine, args),
        Command::Assets(args) => assets_cmd::cmd_assets(&cli.machine, args),
        Command::Run(args) => run::cmd_run(&cli.machine, *args),
        Command::Bundle(args) => bundle::cmd_bundle(&cli.machine, *args),
        Command::Guide(args) => guide::cmd_guide(&cli.machine, args),
        Command::Doctor(args) => doctor::cmd_doctor(&cli.machine, args),
        Command::Diag(args) => diag::cmd_diag(&cli.machine, args),
        Command::Policy(args) => policy::cmd_policy(&cli.machine, args),
        Command::Ast(args) => ast::cmd_ast(&cli.machine, args),
        Command::Agent(args) => agent::cmd_agent(&cli.machine, args),
        Command::Fmt(args) => toolchain::cmd_fmt(&cli.machine, args),
        Command::Lint(args) => toolchain::cmd_lint(&cli.machine, args),
        Command::Fix(args) => toolchain::cmd_fix(&cli.machine, args),
        Command::Migrate(args) => toolchain::cmd_migrate(&cli.machine, args),
        Command::Build(args) => toolchain::cmd_build(&cli.machine, args),
        Command::Check(args) => toolchain::cmd_check(&cli.machine, args),
        Command::Service(args) => service::cmd_service(&cli.machine, args),
        Command::Cli(args) => cli::cmd_cli(&cli.machine, args),
        Command::Pkg(args) => pkg::cmd_pkg(&cli.machine, args),
        Command::Prove(args) => prove::cmd_prove(&cli.machine, args),
        Command::Review(args) => review::cmd_review(&cli.machine, args),
        Command::Trust(args) => trust::cmd_trust(&cli.machine, args),
        Command::Patch(args) => patch::cmd_patch(&cli.machine, args),
        Command::Doc(args) => doc::cmd_doc(&cli.machine, args),
        Command::Schema(args) => schema::cmd_schema(&cli.machine, args),
        Command::Sm(args) => sm::cmd_sm(&cli.machine, args),
        Command::Rr(args) => rr::cmd_rr(&cli.machine, args),
        Command::Verify(args) => verify::cmd_verify(&cli.machine, args),
        Command::Mcp(args) => cmd_mcp(args),
        Command::Wasm(args) => cmd_wasm(args),
    }
}

fn cmd_mcp(args: McpArgs) -> Result<std::process::ExitCode> {
    match delegate::run_inherit("x07-mcp", &args.args)? {
        delegate::DelegateOutput::Exited(status) => Ok(delegate::exit_code_from_status(&status)),
        delegate::DelegateOutput::NotFound => {
            eprintln!("x07-mcp not found on PATH");
            eprintln!("hint: install x07-mcp and ensure it is discoverable on PATH");
            Ok(std::process::ExitCode::from(2))
        }
    }
}

fn cmd_wasm(args: WasmArgs) -> Result<std::process::ExitCode> {
    match delegate::run_inherit("x07-wasm", &args.args)? {
        delegate::DelegateOutput::Exited(status) => Ok(delegate::exit_code_from_status(&status)),
        delegate::DelegateOutput::NotFound => {
            eprintln!("x07-wasm not found on PATH");
            eprintln!("hint: install x07-wasm and ensure it is discoverable on PATH");
            Ok(std::process::ExitCode::from(2))
        }
    }
}

fn cmd_test(machine: &reporting::MachineArgs, args: TestArgs) -> Result<std::process::ExitCode> {
    let started = Instant::now();

    let mut args = args;
    if args.pbt && args.all {
        anyhow::bail!("--pbt and --all are mutually exclusive");
    }
    if args.pbt_repro.is_some() && !args.pbt {
        anyhow::bail!("--pbt-repro requires --pbt");
    }
    if args.pbt_repro.is_some() && args.all {
        anyhow::bail!("--pbt-repro cannot be combined with --all");
    }
    args.manifest = util::resolve_existing_path_upwards(&args.manifest);

    let validated = match validate_manifest_json(&args.manifest) {
        Ok(m) => m,
        Err(diags) => {
            for d in &diags {
                eprintln!("{}: {} ({})", d.code, d.message, d.path);
            }
            let report = X07TestReport::error_from_manifest(&args, started.elapsed(), diags);
            return write_report_and_exit(machine, args, report, 12);
        }
    };

    let module_root_used = args
        .module_root
        .first()
        .cloned()
        .unwrap_or_else(|| validated.manifest_dir.clone());
    let module_roots = compute_test_module_roots(&args, &validated)?;

    let mut tests = validated.tests;
    let entry_diags = hydrate_test_entry_info(&module_roots, &mut tests);
    if !entry_diags.is_empty() {
        for d in &entry_diags {
            eprintln!("{}: {} ({})", d.code, d.message, d.path);
        }
        let report = X07TestReport::error_from_manifest(&args, started.elapsed(), entry_diags);
        return write_report_and_exit(machine, args, report, 12);
    }
    let total_tests = tests.len();
    if let Some(world) = args.world {
        tests.retain(|t| t.world == world);
    }
    if let Some(filter) = &args.filter {
        if args.exact {
            tests.retain(|t| t.id == *filter);
        } else {
            tests.retain(|t| t.id.contains(filter));
        }
    }

    if args.pbt {
        tests.retain(|t| t.pbt.is_some());
    } else if !args.all {
        tests.retain(|t| t.pbt.is_none());
    }

    if let Some(repro_path) = args.pbt_repro.as_deref() {
        let bytes = std::fs::read(repro_path)
            .with_context(|| format!("read PBT repro: {}", repro_path.display()))?;
        let id = pbt::test_id_from_repro(&bytes)?;
        tests.retain(|t| t.id == id);
    }
    tests.sort_by(|a, b| a.id.cmp(&b.id));

    if args.pbt_repro.is_some() && tests.is_empty() {
        anyhow::bail!(
            "--pbt-repro: referenced test id was not found in the manifest (after filters)"
        );
    }

    if tests.is_empty() && !args.allow_empty {
        let mut selectors: Vec<String> = Vec::new();
        if let Some(world) = args.world {
            selectors.push(format!("world={}", world.as_str()));
        }
        if let Some(filter) = args.filter.as_deref() {
            let kind = if args.exact { "exact" } else { "substr" };
            selectors.push(format!("filter({kind})={filter:?}"));
        }
        if args.pbt {
            selectors.push("pbt_only=true".to_string());
        } else if !args.all {
            selectors.push("pbt_only=false".to_string());
        }
        if selectors.is_empty() {
            selectors.push("no filters".to_string());
        }
        anyhow::bail!(
            "0 tests selected (manifest had {total_tests}; {})\n\
             hint: pass --allow-empty to treat this as success",
            selectors.join(", ")
        );
    }

    if args.list {
        for t in &tests {
            println!(
                "{}\t{}\t{}\t{}",
                t.id,
                t.world.as_str(),
                t.expect.as_str(),
                t.entry
            );
        }
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let stdlib_lock_raw = args.stdlib_lock.clone();
    let mut stdlib_lock_path =
        util::resolve_existing_path_upwards_from(&validated.manifest_dir, &stdlib_lock_raw);
    if !stdlib_lock_path.exists() && !stdlib_lock_raw.is_absolute() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let cand = util::resolve_existing_path_upwards_from(exe_dir, &stdlib_lock_raw);
                if cand.exists() {
                    stdlib_lock_path = cand;
                }
            }
        }
    }
    if !stdlib_lock_path.exists() {
        anyhow::bail!(
            "could not find stdlib lock file: {} (pass --stdlib-lock <path>)",
            stdlib_lock_raw.display()
        );
    }
    args.stdlib_lock = stdlib_lock_path;

    if args.verbose {
        eprintln!(
            "x07 test: {} tests (repeat={}, jobs={})",
            tests.len(),
            args.repeat,
            args.jobs
        );
    }

    let project_compat = {
        let project_path = util::resolve_existing_path_upwards_from(
            &validated.manifest_dir,
            Path::new("x07.json"),
        );
        if project_path.is_file() {
            Some(
                project::load_project_manifest(&project_path)
                    .context("load project manifest")?
                    .compat,
            )
            .flatten()
        } else {
            None
        }
    };
    let compat = crate::util::resolve_compat(args.compat.as_deref(), project_compat.as_deref())
        .context("resolve compat")?;

    let results = run_tests(&args, &module_roots, compat, &tests)?;

    let report = finalize_report(&args, &module_root_used, started.elapsed(), results);

    let exit_code = compute_exit_code(&args, &report);
    write_report_and_exit(machine, args, report, exit_code)
}

fn compute_test_module_roots(
    args: &TestArgs,
    validated: &ValidatedManifest,
) -> Result<Vec<PathBuf>> {
    if !args.module_root.is_empty() {
        return Ok(args.module_root.clone());
    }

    let manifest_dir = &validated.manifest_dir;
    let project_path =
        util::resolve_existing_path_upwards_from(manifest_dir, Path::new("x07.json"));
    if !project_path.is_file() {
        return Ok(vec![manifest_dir.clone()]);
    }

    let hydrated = crate::pkg::ensure_project_deps_hydrated_quiet(project_path.clone())
        .context("hydrate project deps")?;
    if hydrated {
        eprintln!(
            "x07 test: hydrated project dependencies via `x07 pkg lock --project {}`",
            project_path.display()
        );
    }

    let project_manifest =
        project::load_project_manifest(&project_path).context("load project manifest")?;
    let lock_path = project::default_lockfile_path(&project_path, &project_manifest);

    let lock: project::Lockfile = if lock_path.is_file() {
        let bytes = std::fs::read(&lock_path)
            .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?
    } else if project_manifest.dependencies.is_empty() {
        project::compute_lockfile(&project_path, &project_manifest)?
    } else {
        anyhow::bail!(
            "missing lockfile for project with dependencies: {}",
            lock_path.display()
        );
    };

    if !PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED
        .iter()
        .any(|v| *v == lock.schema_version.trim())
    {
        anyhow::bail!(
            "lockfile schema_version mismatch: expected one of {:?} got {:?}",
            PROJECT_LOCKFILE_SCHEMA_VERSIONS_SUPPORTED,
            lock.schema_version
        );
    }

    let mut roots = project::collect_module_roots(&project_path, &project_manifest, &lock)
        .context("collect module roots")?;

    let project_root = project_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let project_root_norm = normalize_module_root_path(&project_root);
    let project_root_already_in_roots = roots
        .iter()
        .any(|p| normalize_module_root_path(p) == project_root_norm);
    if !project_root_already_in_roots {
        roots.push(project_root);
    }

    let manifest_norm = normalize_module_root_path(manifest_dir);
    let already_in_roots = roots
        .iter()
        .any(|p| normalize_module_root_path(p) == manifest_norm);
    if !already_in_roots {
        roots.push(manifest_dir.clone());
    }

    Ok(roots)
}

fn normalize_module_root_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        if component == std::path::Component::CurDir {
            continue;
        }
        out.push(Path::new(component.as_os_str()));
    }
    out
}

#[derive(Debug, Clone)]
struct ResolvedTestEntry {
    kind: TestEntryKind,
    result: String,
}

fn resolve_test_entry(module_roots: &[PathBuf], entry: &str) -> Result<ResolvedTestEntry> {
    let (module_id, _) = entry
        .rsplit_once('.')
        .context("test entry must contain '.'")?;
    let rel = format!("{}.x07.json", module_id.replace('.', "/"));
    for root in module_roots {
        let path = root.join(&rel);
        if !path.is_file() {
            continue;
        }
        let doc = report_common::read_json_file(&path)
            .with_context(|| format!("read test entry module: {}", path.display()))?;
        let decls = doc
            .get("decls")
            .and_then(Value::as_array)
            .context("test entry module is missing decls[]")?;
        for decl in decls {
            if decl.get("name").and_then(Value::as_str) != Some(entry) {
                continue;
            }
            let kind = match decl.get("kind").and_then(Value::as_str).unwrap_or("") {
                "defn" => TestEntryKind::Defn,
                "defasync" => TestEntryKind::Defasync,
                other => anyhow::bail!("unsupported test entry declaration kind: {other}"),
            };
            let result = decl
                .get("result")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            return Ok(ResolvedTestEntry { kind, result });
        }
    }
    anyhow::bail!("entry {entry:?} was not found under the resolved module roots");
}

fn hydrate_test_entry_info(module_roots: &[PathBuf], tests: &mut [TestDecl]) -> Vec<ManifestDiag> {
    let mut diags = Vec::new();
    for test in tests {
        match resolve_test_entry(module_roots, &test.entry) {
            Ok(resolved) => {
                test.entry_kind = resolved.kind;
                test.entry_result_ty = resolved.result.clone();
                if resolved.kind == TestEntryKind::Defasync {
                    let returns_supported = match test.returns {
                        TestReturns::ResultI32 => resolved.result == "result_i32",
                        TestReturns::BytesStatusV1 => {
                            resolved.result == "bytes" || resolved.result == "result_bytes"
                        }
                    };
                    if !returns_supported {
                        diags.push(ManifestDiag {
                            code: "X07TEST_ASYNC_ENTRY_UNSUPPORTED",
                            message: format!(
                                "async entry {:?} returns {:?}, which is incompatible with x07 test returns={:?}",
                                test.entry,
                                resolved.result,
                                match test.returns {
                                    TestReturns::ResultI32 => "result_i32",
                                    TestReturns::BytesStatusV1 => "bytes_status_v1",
                                }
                            ),
                            path: test.entry.clone(),
                        });
                    }
                }
            }
            Err(err) => diags.push(ManifestDiag {
                code: "ETEST_ENTRY_INVALID",
                message: err.to_string(),
                path: test.entry.clone(),
            }),
        }
    }
    diags
}

#[derive(Debug, serde::Deserialize)]
struct TestCapsuleIndex {
    #[serde(default)]
    capsules: Vec<TestCapsuleRef>,
}

#[derive(Debug, serde::Deserialize)]
struct TestCapsuleRef {
    id: String,
    contract_path: String,
}

#[derive(Debug, serde::Deserialize)]
struct TestCapsuleContract {
    effect_log: TestCapsuleEffectLog,
}

#[derive(Debug, serde::Deserialize)]
struct TestCapsuleEffectLog {
    schema_path: String,
}

fn resolve_runtime_attestation_path(manifest_path: &Path, raw_path: &str) -> PathBuf {
    let candidate = PathBuf::from(raw_path);
    if candidate.is_absolute() {
        candidate
    } else {
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(candidate)
    }
}

fn collect_required_capsule_effect_log_digests(
    manifest_path: &Path,
    required_capsules: &[String],
) -> Result<Vec<String>> {
    if required_capsules.is_empty() {
        return Ok(Vec::new());
    }
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let project_path =
        util::resolve_existing_path_upwards_from(manifest_dir, Path::new("x07.json"));
    if !project_path.is_file() {
        anyhow::bail!(
            "could not resolve project root for capsule evidence from {}",
            manifest_path.display()
        );
    }
    let project_root = project_path.parent().unwrap_or_else(|| Path::new("."));
    let index_path = project_root
        .join("arch")
        .join("capsules")
        .join("index.x07capsule.json");
    let index: TestCapsuleIndex = serde_json::from_slice(
        &std::fs::read(&index_path)
            .with_context(|| format!("read capsule index: {}", index_path.display()))?,
    )
    .with_context(|| format!("parse capsule index JSON: {}", index_path.display()))?;
    let index_root = index_path.parent().unwrap_or_else(|| Path::new("."));
    let by_id = index
        .capsules
        .into_iter()
        .map(|capsule| (capsule.id.clone(), capsule))
        .collect::<BTreeMap<_, _>>();

    let mut digests = Vec::new();
    for capsule_id in required_capsules {
        let Some(capsule) = by_id.get(capsule_id) else {
            anyhow::bail!(
                "required capsule {:?} is missing from {}",
                capsule_id,
                index_path.display()
            );
        };
        let contract_path = index_root.join(&capsule.contract_path);
        let contract: TestCapsuleContract = serde_json::from_slice(
            &std::fs::read(&contract_path)
                .with_context(|| format!("read capsule contract: {}", contract_path.display()))?,
        )
        .with_context(|| format!("parse capsule contract JSON: {}", contract_path.display()))?;
        let effect_log_path = index_root.join(&contract.effect_log.schema_path);
        let bytes = std::fs::read(&effect_log_path).with_context(|| {
            format!(
                "read capsule effect-log schema: {}",
                effect_log_path.display()
            )
        })?;
        digests.push(format!("sha256:{}", util::sha256_hex(&bytes)));
    }
    digests.sort();
    digests.dedup();
    Ok(digests)
}

fn attach_test_sandbox_evidence(
    manifest_path: &Path,
    test: &TestDecl,
    result: &mut TestCaseResult,
) -> bool {
    let Some(run) = result.run.as_ref() else {
        return false;
    };
    let mut failed = false;

    let runtime_attestation = run.runtime_attestation.clone();
    let mut effect_log_digests = Vec::new();

    if test.require_runtime_attestation && runtime_attestation.is_none() {
        result.diags.push(Diag::new(
            "X07TEST_RUNTIME_ATTEST_REQUIRED",
            if test.sandbox_smoke {
                "sandbox_smoke test requires runtime attestation evidence, but no runtime attestation was emitted"
            } else {
                "test requires runtime attestation evidence, but no runtime attestation was emitted"
            },
        ));
        result.failure_kind = Some("runtime_attestation_missing".to_string());
        result.status = "error".to_string();
        failed = true;
    }
    if let Some(attestation_ref) = runtime_attestation.as_ref() {
        let resolved = resolve_runtime_attestation_path(manifest_path, &attestation_ref.path);
        match std::fs::read(&resolved) {
            Ok(bytes) => match serde_json::from_slice::<RuntimeAttestationDoc>(&bytes) {
                Ok(doc) => effect_log_digests = doc.effect_log_digests,
                Err(err) => {
                    if test.require_runtime_attestation {
                        result.diags.push(Diag::new(
                            "X07TEST_RUNTIME_ATTEST_REQUIRED",
                            format!(
                                "required runtime attestation is not valid JSON: {} ({err})",
                                resolved.display()
                            ),
                        ));
                        result.failure_kind = Some("runtime_attestation_missing".to_string());
                        result.status = "error".to_string();
                        failed = true;
                    }
                }
            },
            Err(err) => {
                if test.require_runtime_attestation {
                    result.diags.push(Diag::new(
                        "X07TEST_RUNTIME_ATTEST_REQUIRED",
                        format!(
                            "required runtime attestation is missing or unreadable: {} ({err})",
                            resolved.display()
                        ),
                    ));
                    result.failure_kind = Some("runtime_attestation_missing".to_string());
                    result.status = "error".to_string();
                    failed = true;
                }
            }
        }
    }
    if effect_log_digests.is_empty() && !test.required_capsules.is_empty() {
        match collect_required_capsule_effect_log_digests(manifest_path, &test.required_capsules) {
            Ok(digests) => effect_log_digests = digests,
            Err(err) => {
                result.diags.push(Diag::new(
                    "X07TEST_CAPSULE_EVIDENCE_MISSING",
                    err.to_string(),
                ));
                result.failure_kind = Some("capsule_evidence_missing".to_string());
                result.status = "error".to_string();
                failed = true;
            }
        }
    }
    if !test.required_capsules.is_empty() && effect_log_digests.len() < test.required_capsules.len()
    {
        result.diags.push(Diag::new(
            "X07TEST_CAPSULE_EVIDENCE_MISSING",
            format!(
                "test requires capsule evidence for {:?}, but only {} effect-log digest(s) were available",
                test.required_capsules,
                effect_log_digests.len()
            ),
        ));
        result.failure_kind = Some("capsule_evidence_missing".to_string());
        result.status = "error".to_string();
        failed = true;
    }
    if let Some(run) = result.run.as_mut() {
        run.capsule_ids = test.required_capsules.clone();
        run.effect_log_digests = effect_log_digests;
    }
    failed
}

fn run_tests(
    args: &TestArgs,
    module_roots: &[PathBuf],
    compat: x07c::compat::Compat,
    tests: &[TestDecl],
) -> Result<Vec<TestCaseResult>> {
    if args.jobs != 1 && !args.no_fail_fast {
        anyhow::bail!("--jobs >1 requires --no-fail-fast");
    }

    let mut out: Vec<TestCaseResult> = Vec::with_capacity(tests.len());
    let mut fail_fast_triggered = false;

    if args.jobs == 1 {
        for test in tests {
            if fail_fast_triggered {
                out.push(TestCaseResult::skipped_due_to_fail_fast(test));
                continue;
            }

            if args.verbose {
                eprintln!("test: {}", test.id);
            }

            let result = run_one_test(args, module_roots, compat, test)?;
            if !args.no_fail_fast {
                let fail_fast = if args.no_run {
                    result.compile.as_ref().is_some_and(|c| !c.ok)
                } else {
                    !matches_expectation(&result)
                };
                if fail_fast {
                    fail_fast_triggered = true;
                }
            }
            out.push(result);
        }

        out.sort_by(|a, b| a.id.cmp(&b.id));
        return Ok(out);
    }

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<TestCaseResult>> = Mutex::new(Vec::with_capacity(tests.len()));
    let first_err: Mutex<Option<anyhow::Error>> = Mutex::new(None);

    std::thread::scope(|scope| {
        let jobs = args.jobs.min(tests.len().max(1));
        for _ in 0..jobs {
            scope.spawn(|| loop {
                if let Ok(guard) = first_err.lock() {
                    if guard.is_some() {
                        return;
                    }
                }
                let idx = next.fetch_add(1, Ordering::Relaxed);
                if idx >= tests.len() {
                    return;
                }
                let test = &tests[idx];
                if args.verbose {
                    eprintln!("test: {}", test.id);
                }
                match run_one_test(args, module_roots, compat, test) {
                    Ok(r) => {
                        if let Ok(mut guard) = results.lock() {
                            guard.push(r);
                        }
                    }
                    Err(err) => {
                        if let Ok(mut guard) = first_err.lock() {
                            if guard.is_none() {
                                *guard = Some(err);
                            }
                        }
                        return;
                    }
                }
            });
        }
    });

    if let Some(err) = first_err.into_inner().unwrap_or_else(|e| e.into_inner()) {
        return Err(err);
    }
    out = results.into_inner().unwrap_or_else(|e| e.into_inner());

    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn matches_expectation(r: &TestCaseResult) -> bool {
    matches_expectation_strs(&r.expect, &r.status)
}

fn matches_expectation_strs(expect: &str, status: &str) -> bool {
    matches!(
        (expect, status),
        ("pass", "pass") | ("fail", "xfail_fail") | ("skip", "skip")
    )
}

fn infer_arch_root_from_manifest(manifest: &Path) -> Option<PathBuf> {
    let start_dir = manifest
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let start_dir = std::fs::canonicalize(&start_dir).unwrap_or(start_dir);

    let mut dir: Option<&Path> = Some(start_dir.as_path());
    while let Some(d) = dir {
        if d.join("arch").is_dir() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

fn parse_run_os_policy_read_roots(policy_path: &Path) -> Result<Vec<PathBuf>> {
    let bytes = std::fs::read(policy_path)
        .with_context(|| format!("read run-os policy: {}", policy_path.display()))?;
    let doc: serde_json::Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse run-os policy JSON: {}", policy_path.display()))?;

    let fs = doc
        .get("fs")
        .and_then(|v| v.as_object())
        .context("run-os policy: expected fs object")?;
    let roots = fs
        .get("read_roots")
        .and_then(|v| v.as_array())
        .context("run-os policy: expected fs.read_roots array")?;

    let mut out = Vec::with_capacity(roots.len());
    for (idx, v) in roots.iter().enumerate() {
        let s = v
            .as_str()
            .with_context(|| format!("run-os policy: fs.read_roots[{idx}] must be a string"))?;
        out.push(PathBuf::from(s));
    }
    Ok(out)
}

fn policy_roots_fit_cwd(read_roots: &[PathBuf], cwd: &Path) -> bool {
    read_roots.iter().all(|root| {
        if root.is_absolute() {
            root.exists()
        } else {
            cwd.join(root).exists()
        }
    })
}

fn run_one_test(
    args: &TestArgs,
    module_roots: &[PathBuf],
    compat: x07c::compat::Compat,
    test: &TestDecl,
) -> Result<TestCaseResult> {
    let start = Instant::now();

    if matches!(test.expect, Expect::Skip) {
        return Ok(TestCaseResult {
            id: test.id.clone(),
            world: test.world.as_str().to_string(),
            expect: test.expect.as_str().to_string(),
            status: "skip".to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
            entry_kind: test.entry_kind.as_str().to_string(),
            failure_kind: None,
            contract_repro_path: None,
            entry: Some(test.entry.clone()),
            fixture_root: test.fixture_root.as_ref().map(display_path),
            compile: None,
            run: None,
            diags: Vec::new(),
        });
    }

    if test.pbt.is_some() {
        return run_one_pbt_test(args, module_roots, compat, test, start);
    }

    let driver_src = build_test_driver_x07ast_json(test)?;

    if !test.world.is_eval_world() {
        return run_one_test_os(args, module_roots, compat, test, &driver_src, start);
    }

    let (driver_out_dir, driver_path, exe_out_path) = if args.keep_artifacts {
        let out_dir = args
            .artifact_dir
            .join("tests")
            .join(util::safe_artifact_dir_name(&test.id));
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("create artifact dir: {}", out_dir.display()))?;
        let driver_path = out_dir.join("driver.x07.json");
        std::fs::write(&driver_path, &driver_src)
            .with_context(|| format!("write driver: {}", driver_path.display()))?;
        let exe_path = out_dir.join("solver");
        (Some(out_dir), Some(driver_path), Some(exe_path))
    } else {
        (None, None, None)
    };

    let mut compile_options =
        x07c::world_config::compile_options_for_world(test.world, module_roots.to_vec());
    compile_options.compat = compat;
    compile_options.arch_root = infer_arch_root_from_manifest(&args.manifest)
        .or_else(|| args.manifest.parent().map(|p| p.to_path_buf()))
        .or_else(|| std::env::current_dir().ok());

    let runner_config = runner_config_for_test(test)?;

    let compiled_out = exe_out_path.as_deref();
    let compile_res = x07_host_runner::compile_program_with_options(
        &driver_src,
        &runner_config,
        compiled_out,
        &compile_options,
        &[],
    )?;

    let mut result = TestCaseResult {
        id: test.id.clone(),
        world: test.world.as_str().to_string(),
        expect: test.expect.as_str().to_string(),
        status: "error".to_string(),
        duration_ms: 0,
        entry_kind: test.entry_kind.as_str().to_string(),
        failure_kind: None,
        contract_repro_path: None,
        entry: Some(test.entry.clone()),
        fixture_root: test.fixture_root.as_ref().map(display_path),
        compile: Some(CompileSection {
            ok: compile_res.ok,
            exit_code: Some(compile_res.exit_status),
            compiler_diags: Vec::new(),
            artifact_path: compiled_out.map(display_path),
            c_bytes: Some(compile_res.c_source_size as u64),
        }),
        run: None,
        diags: Vec::new(),
    };

    if !compile_res.ok {
        if let Some(msg) = compile_res.compile_error.as_ref() {
            result.diags.push(Diag::new("ETEST_COMPILE", msg));
        } else {
            result
                .diags
                .push(Diag::new("ETEST_COMPILE", "compile failed"));
        }
        result.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(result);
    }

    if args.no_run {
        result.status = "skip".to_string();
        result.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(result);
    }

    let exe = compile_res
        .compiled_exe
        .as_deref()
        .context("internal error: compile ok but missing compiled_exe")?;

    let input: &[u8] = test.input.as_deref().unwrap_or(&[]);

    let mut first_obs: Option<ObservedRun> = None;
    let mut last_run: Option<RunnerResult> = None;

    for rep in 0..args.repeat {
        let run_res = run_artifact_file(&runner_config, exe, input)?;
        let obs = ObservedRun::from_runner_result(&run_res);
        if let Some(first) = &first_obs {
            if first != &obs {
                result.diags.push(Diag::new(
                    "EDETERMINISM",
                    format!("nondeterminism detected at repeat {}", rep + 1),
                ));
                result.run = Some(RunSection::from_runner_result(&run_res));
                result.status = "error".to_string();
                result.duration_ms = start.elapsed().as_millis() as u64;
                return Ok(result);
            }
        } else {
            first_obs = Some(obs);
        }
        last_run = Some(run_res);
    }

    let run_res = last_run.context("internal error: missing run result")?;

    if !run_res.ok || run_res.exit_status != 0 {
        if let Some(trap) = run_res.trap.as_deref() {
            match contract_repro::try_parse_contract_trap(trap) {
                Ok(Some(info)) => {
                    let source = contract_repro::SourceInfo {
                        mode: "x07test".to_string(),
                        tests_manifest_path: Some(display_path(&args.manifest)),
                        test_id: Some(test.id.clone()),
                        test_entry: Some(test.entry.clone()),
                        target_kind: None,
                        target_path: None,
                    };

                    match contract_repro::write_repro(
                        &args.artifact_dir,
                        test.world.as_str(),
                        &runner_config,
                        input,
                        info.payload,
                        source,
                        &info.clause_id,
                    ) {
                        Ok(path) => {
                            result.failure_kind = Some("contract_violation".to_string());
                            result.contract_repro_path = Some(display_path(&path));
                        }
                        Err(err) => {
                            result
                                .diags
                                .push(Diag::new("X07T_ECONTRACT_REPRO_WRITE", err.to_string()));
                        }
                    }

                    result.run = Some(RunSection::from_runner_result(&run_res));
                    result.status = "error".to_string();
                    result.duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(result);
                }
                Ok(None) => {}
                Err(err) => {
                    result
                        .diags
                        .push(Diag::new("X07T_ECONTRACT_TRAP_PARSE", err.to_string()));
                    result.run = Some(RunSection::from_runner_result(&run_res));
                    result.status = "error".to_string();
                    result.duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(result);
                }
            }
        }

        if let Some(trap) = run_res.trap.as_deref() {
            result
                .diags
                .push(Diag::new("X07T_RUN_TRAP", "runner trapped").with_details(
                    serde_json::json!({
                        "trap": trap,
                    }),
                ));
        } else {
            result.diags.push(Diag::new(
                "ETEST_RUN",
                format!(
                    "runner failed: ok={} exit_status={}",
                    run_res.ok, run_res.exit_status
                ),
            ));
        }

        result.run = Some(RunSection::from_runner_result(&run_res));
        result.status = "error".to_string();
        result.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(result);
    }

    let status_bytes = run_res.solve_output.clone();
    let status_v1 = match parse_evtest_status_v1(&status_bytes) {
        Ok(x) => x,
        Err(msg) => {
            result.diags.push(Diag::new("EBAD_STATUS", msg.to_string()));
            result.run = Some(RunSection::from_runner_result(&run_res));
            result.status = "error".to_string();
            result.duration_ms = start.elapsed().as_millis() as u64;
            return Ok(result);
        }
    };
    let tag = status_v1.tag;
    let code_u32 = status_v1.code_u32;
    if let Some(details) = status_v1.assert_bytes_eq_details {
        result.diags.push(
            Diag::new("X07T_ASSERT_BYTES_EQ", "assert_bytes_eq failed").with_details(details),
        );
    }

    result.run = Some(RunSection {
        failure_code_u32: Some(code_u32 as u64),
        ..RunSection::from_runner_result(&run_res)
    });

    result.status = compute_status(test.expect, tag);
    result.duration_ms = start.elapsed().as_millis() as u64;

    if let Some(out_dir) = driver_out_dir {
        if args.verbose {
            eprintln!("artifacts: {}", out_dir.display());
        }
    }
    if let Some(driver_path) = driver_path {
        if args.verbose {
            eprintln!("driver: {}", driver_path.display());
        }
    }

    Ok(result)
}

fn run_one_pbt_test(
    args: &TestArgs,
    module_roots: &[PathBuf],
    compat: x07c::compat::Compat,
    test: &TestDecl,
    start: Instant,
) -> Result<TestCaseResult> {
    let pbt_decl = test
        .pbt
        .as_ref()
        .context("internal error: missing pbt decl")?;

    let mut budget = pbt_decl.case_budget;
    if args.pbt_case_fuel.is_some() {
        budget.fuel = args.pbt_case_fuel.context("pbt_case_fuel")?;
    }
    if args.pbt_case_timeout_ms.is_some() {
        budget.timeout_ms = args.pbt_case_timeout_ms.context("pbt_case_timeout_ms")?;
    }
    if args.pbt_case_mem_bytes.is_some() {
        budget.max_mem_bytes = args.pbt_case_mem_bytes.context("pbt_case_mem_bytes")?;
    }
    if args.pbt_case_output_bytes.is_some() {
        budget.max_output_bytes = args
            .pbt_case_output_bytes
            .context("pbt_case_output_bytes")?;
    }

    let suite_seed_u64 = args.pbt_seed.unwrap_or(0);
    let cases = args.pbt_cases.unwrap_or(pbt_decl.cases);
    let max_shrinks = args.pbt_max_shrinks.unwrap_or(pbt_decl.max_shrinks);

    let repro_mode = args.pbt_repro.is_some();
    let repro = if let Some(path) = args.pbt_repro.as_deref() {
        let bytes =
            std::fs::read(path).with_context(|| format!("read PBT repro: {}", path.display()))?;
        let repro = pbt::parse_repro_json(&bytes)?;
        pbt::validate_repro_test_matches_manifest(&repro, &test.id, &test.entry)?;
        Some(repro)
    } else {
        None
    };

    let tys: Vec<pbt::PbtTy> = if let Some(repro) = repro.as_ref() {
        pbt::counterexample_tys(repro)
    } else {
        pbt_decl.params.iter().map(|p| p.gen.ty()).collect()
    };

    let driver_src = pbt::build_case_driver_x07ast_json(&test.entry, &tys, pbt_decl.budget_scope)?;

    let out_dir = args
        .artifact_dir
        .join("pbt")
        .join(util::safe_artifact_dir_name(&test.id));
    if args.keep_artifacts {
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("create artifact dir: {}", out_dir.display()))?;
        let driver_path = out_dir.join("driver.x07.json");
        std::fs::write(&driver_path, &driver_src)
            .with_context(|| format!("write driver: {}", driver_path.display()))?;
    }

    let exe_out_path = if args.keep_artifacts {
        Some(out_dir.join("solver"))
    } else {
        None
    };

    let mut compile_options =
        x07c::world_config::compile_options_for_world(test.world, module_roots.to_vec());
    compile_options.compat = compat;
    compile_options.arch_root = infer_arch_root_from_manifest(&args.manifest)
        .or_else(|| args.manifest.parent().map(|p| p.to_path_buf()))
        .or_else(|| std::env::current_dir().ok());

    let runner_config = runner_config_for_test(test)?;
    let compiled_out = exe_out_path.as_deref();
    let compile_res = x07_host_runner::compile_program_with_options(
        &driver_src,
        &runner_config,
        compiled_out,
        &compile_options,
        &[],
    )?;

    let mut result = TestCaseResult {
        id: test.id.clone(),
        world: test.world.as_str().to_string(),
        expect: test.expect.as_str().to_string(),
        status: "error".to_string(),
        duration_ms: 0,
        entry_kind: test.entry_kind.as_str().to_string(),
        failure_kind: None,
        contract_repro_path: None,
        entry: Some(test.entry.clone()),
        fixture_root: test.fixture_root.as_ref().map(display_path),
        compile: Some(CompileSection {
            ok: compile_res.ok,
            exit_code: Some(compile_res.exit_status),
            compiler_diags: Vec::new(),
            artifact_path: compiled_out.map(display_path),
            c_bytes: Some(compile_res.c_source_size as u64),
        }),
        run: None,
        diags: Vec::new(),
    };

    if !compile_res.ok {
        if let Some(msg) = compile_res.compile_error.as_ref() {
            result.diags.push(Diag::new("ETEST_COMPILE", msg));
        } else {
            result
                .diags
                .push(Diag::new("ETEST_COMPILE", "compile failed"));
        }
        result.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(result);
    }

    if args.no_run {
        result.status = "skip".to_string();
        result.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(result);
    }

    let exe = compile_res
        .compiled_exe
        .as_deref()
        .context("internal error: compile ok but missing compiled_exe")?;

    if repro_mode {
        let repro = repro.context("internal error: missing repro doc")?;
        let case_bytes = pbt::counterexample_case_bytes(&repro)?;

        let mut case_cfg = runner_config.clone();
        case_cfg.solve_fuel = repro.budget.fuel;
        case_cfg.max_memory_bytes = repro.budget.max_mem_bytes as usize;
        case_cfg.max_output_bytes = repro.budget.max_output_bytes as usize;
        case_cfg.cpu_time_limit_seconds = ms_to_ceiling_seconds(repro.budget.timeout_ms)?;

        let mut first_obs: Option<ObservedRun> = None;
        let mut last_run: Option<RunnerResult> = None;

        for rep in 0..args.repeat {
            let run_res = run_artifact_file(&case_cfg, exe, &case_bytes)?;
            let obs = ObservedRun::from_runner_result(&run_res);
            if let Some(first) = &first_obs {
                if first != &obs {
                    result.diags.push(Diag::new(
                        "EDETERMINISM",
                        format!("nondeterminism detected at repeat {}", rep + 1),
                    ));
                    result.run = Some(RunSection::from_runner_result(&run_res));
                    result.status = "error".to_string();
                    result.duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(result);
                }
            } else {
                first_obs = Some(obs);
            }
            last_run = Some(run_res);
        }

        let run_res = last_run.context("internal error: missing run result")?;
        result.run = Some(RunSection::from_runner_result(&run_res));

        if !run_res.ok {
            if let Some(trap) = run_res.trap.as_deref() {
                result
                    .diags
                    .push(Diag::new("X07T_RUN_TRAP", "runner trapped").with_details(
                        serde_json::json!({
                            "trap": trap,
                        }),
                    ));
            }
            result.diags.push(
                Diag::new("X07T_EPBT_FAIL", "repro case failed (runner trap)")
                    .with_details(pbt::repro_to_details_value(&repro)?),
            );
            result.status = "error".to_string();
            result.duration_ms = start.elapsed().as_millis() as u64;
            return Ok(result);
        }

        let status_v1 = parse_evtest_status_v1(&run_res.solve_output)?;
        let tag = status_v1.tag;
        let code_u32 = status_v1.code_u32;
        if let Some(run) = result.run.as_mut() {
            run.failure_code_u32 = Some(code_u32 as u64);
        }
        result.status = compute_status(test.expect, tag);
        result.duration_ms = start.elapsed().as_millis() as u64;

        if let Some(details) = status_v1.assert_bytes_eq_details {
            result.diags.push(
                Diag::new("X07T_ASSERT_BYTES_EQ", "assert_bytes_eq failed").with_details(details),
            );
        }

        if tag == 1 {
            result.diags.push(Diag::new(
                "X07T_EPBT_REPRO_NONREPRO",
                "repro no longer reproduces (property passed)",
            ));
        } else {
            result.diags.push(
                Diag::new("X07T_EPBT_FAIL", "repro case failed")
                    .with_details(pbt::repro_to_details_value(&repro)?),
            );
        }

        return Ok(result);
    }

    let mut first_obs: Option<(pbt::PbtObservation, Option<u8>, Option<u32>)> = None;
    let mut last_suite: Option<pbt::PbtSuiteRun> = None;

    for rep in 0..args.repeat {
        let (suite, obs) = pbt::run_pbt_suite(pbt::RunPbtSuiteArgs {
            exe,
            base_cfg: &runner_config,
            test_id: &test.id,
            entry: &test.entry,
            world: test.world,
            params: &pbt_decl.params,
            budget: &budget,
            suite_seed_u64,
            cases,
            max_shrinks,
        })?;
        let cur = (obs, suite.status_tag, suite.status_code_u32);
        if let Some(first) = &first_obs {
            if first != &cur {
                result.diags.push(Diag::new(
                    "EDETERMINISM",
                    format!("nondeterminism detected at repeat {}", rep + 1),
                ));
                result.run = Some(RunSection::from_runner_result(&suite.final_run));
                result.status = "error".to_string();
                result.duration_ms = start.elapsed().as_millis() as u64;
                return Ok(result);
            }
        } else {
            first_obs = Some(cur);
        }
        last_suite = Some(suite);
    }

    let suite = last_suite.context("internal error: missing suite result")?;

    result.run = Some(RunSection::from_runner_result(&suite.final_run));
    if let (Some(tag), Some(code_u32)) = (suite.status_tag, suite.status_code_u32) {
        if let Some(run) = result.run.as_mut() {
            run.failure_code_u32 = Some(code_u32 as u64);
        }
        result.status = compute_status(test.expect, tag);
    } else {
        result.status = "error".to_string();
    }

    if let Some(repro) = suite.repro.as_ref() {
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("create artifact dir: {}", out_dir.display()))?;
        let repro_path = out_dir.join("repro.json");
        util::write_atomic(&repro_path, &pbt::repro_to_pretty_canon_bytes(repro)?)
            .with_context(|| format!("write repro: {}", repro_path.display()))?;
        result.diags.push(
            Diag::new("X07T_EPBT_FAIL", "property failed")
                .with_details(pbt::repro_to_details_value(repro)?),
        );
    }

    result.duration_ms = start.elapsed().as_millis() as u64;

    if args.verbose && args.keep_artifacts {
        eprintln!("artifacts: {}", out_dir.display());
    }

    Ok(result)
}

#[derive(Debug, serde::Deserialize)]
struct OsRunnerReportRaw {
    compile: OsRunnerCompileRaw,
    #[serde(default)]
    solve: Option<OsRunnerSolveRaw>,
    #[serde(default)]
    sandbox_backend: Option<String>,
    #[serde(default)]
    runtime_attestation: Option<RuntimeAttestationRef>,
}

#[derive(Debug, serde::Deserialize)]
struct OsRunnerCompileRaw {
    ok: bool,
    exit_status: i32,
    c_source_size: u64,
    #[serde(default)]
    compile_error: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct OsRunnerSolveRaw {
    ok: bool,
    exit_status: i32,
    solve_output_b64: String,
    stdout_b64: String,
    stderr_b64: String,
    #[serde(default)]
    fuel_used: Option<u64>,
    #[serde(default)]
    mem_stats: Option<x07_host_runner::MemStats>,
    #[serde(default)]
    sched_stats: Option<x07_host_runner::SchedStats>,
    #[serde(default)]
    trap: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct RuntimeAttestationRef {
    path: String,
    sandbox_backend: String,
    weaker_isolation: bool,
    #[serde(default)]
    network_mode: String,
    #[serde(default)]
    network_enforcement: String,
}

#[derive(Debug, serde::Deserialize)]
struct RuntimeAttestationDoc {
    #[serde(default)]
    effect_log_digests: Vec<String>,
}

static X07TEST_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const X07TEST_SOLVE_FUEL: u64 = 400_000_000;

fn create_temp_test_dir(base: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(base)
        .with_context(|| format!("create temp dir base: {}", base.display()))?;

    let pid = std::process::id();
    for _ in 0..10_000 {
        let n = X07TEST_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!("x07test_{pid}_{n}"));
        if std::fs::create_dir(&path).is_ok() {
            return Ok(path);
        }
    }

    anyhow::bail!("failed to create temp dir under {}", base.display());
}

fn rm_rf(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

fn run_one_test_os(
    args: &TestArgs,
    module_roots: &[PathBuf],
    compat: x07c::compat::Compat,
    test: &TestDecl,
    driver_src: &[u8],
    start: Instant,
) -> Result<TestCaseResult> {
    if args.no_run {
        return Ok(TestCaseResult {
            id: test.id.clone(),
            world: test.world.as_str().to_string(),
            expect: test.expect.as_str().to_string(),
            status: "error".to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
            entry_kind: test.entry_kind.as_str().to_string(),
            failure_kind: None,
            contract_repro_path: None,
            entry: Some(test.entry.clone()),
            fixture_root: None,
            compile: None,
            run: None,
            diags: vec![Diag::new(
                "ETEST_NO_RUN_UNSUPPORTED",
                "--no-run is only supported for deterministic solve worlds",
            )],
        });
    }

    let (out_dir, cleanup_dir) = if args.keep_artifacts {
        let out_dir = args
            .artifact_dir
            .join("tests")
            .join(util::safe_artifact_dir_name(&test.id));
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("create artifact dir: {}", out_dir.display()))?;
        (out_dir, false)
    } else {
        let base = args.artifact_dir.join("tests").join("_tmp");
        (create_temp_test_dir(&base)?, true)
    };

    let out_dir = out_dir
        .canonicalize()
        .with_context(|| format!("canonicalize out_dir: {}", out_dir.display()))?;

    let driver_path = out_dir.join("driver.x07.json");
    std::fs::write(&driver_path, driver_src)
        .with_context(|| format!("write driver: {}", driver_path.display()))?;

    let exe_out_path = out_dir.join("solver");
    let runtime_attestation_path = if test.world == WorldId::RunOsSandboxed {
        let path = args
            .artifact_dir
            .join("runtime-attest")
            .join(format!("{}.json", util::safe_artifact_dir_name(&test.id)));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create runtime attestation dir: {}", parent.display()))?;
        }
        Some(path)
    } else {
        None
    };

    let mut cmd = std::process::Command::new(crate::util::resolve_sibling_or_path("x07-os-runner"));
    let manifest_dir = args.manifest.parent().map(Path::to_path_buf);
    let manifest_dir = manifest_dir
        .as_deref()
        .and_then(|d| std::fs::canonicalize(d).ok())
        .or(manifest_dir);
    let arch_root = infer_arch_root_from_manifest(&args.manifest);
    let policy_path = if test.world == WorldId::RunOsSandboxed {
        let Some(policy) = &test.policy_json else {
            anyhow::bail!("internal error: run-os-sandboxed test missing policy_json");
        };
        let raw = if policy.is_absolute() {
            policy.clone()
        } else if let Some(dir) = &manifest_dir {
            dir.join(policy)
        } else {
            policy.clone()
        };
        Some(std::fs::canonicalize(&raw).unwrap_or(raw))
    } else {
        None
    };

    let cmd_cwd = if test.world == WorldId::RunOsSandboxed {
        let current_dir = std::env::current_dir().ok();
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(d) = &arch_root {
            candidates.push(d.clone());
        }
        if let Some(d) = &manifest_dir {
            if !candidates.iter().any(|c| c == d) {
                candidates.push(d.clone());
            }
        }
        if let Some(d) = &current_dir {
            if !candidates.iter().any(|c| c == d) {
                candidates.push(d.clone());
            }
        }
        let read_roots = policy_path
            .as_deref()
            .and_then(|p| parse_run_os_policy_read_roots(p).ok())
            .unwrap_or_default();
        candidates
            .iter()
            .find(|cwd| policy_roots_fit_cwd(&read_roots, cwd))
            .cloned()
            .or_else(|| manifest_dir.clone())
            .or_else(|| arch_root.clone())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        manifest_dir
            .clone()
            .or_else(|| arch_root.clone())
            .unwrap_or_else(|| PathBuf::from("."))
    };

    cmd.current_dir(cmd_cwd);
    cmd.arg("--world").arg(test.world.as_str());
    cmd.arg("--compat").arg(compat.to_string_lossy());
    cmd.arg("--program").arg(&driver_path);
    cmd.arg("--compiled-out").arg(&exe_out_path);
    cmd.arg("--auto-ffi");
    if test.world == WorldId::RunOsSandboxed {
        let policy_path = policy_path
            .as_deref()
            .context("internal error: missing resolved policy path")?;
        cmd.arg("--policy").arg(policy_path);
        if let Some(path) = runtime_attestation_path.as_ref() {
            cmd.arg("--attest-runtime").arg(path);
        }
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for root in module_roots {
        let abs = if root.is_absolute() {
            root.clone()
        } else {
            cwd.join(root)
        };
        cmd.arg("--module-root").arg(abs);
    }

    let cpu_time_limit_seconds = match test.timeout_ms {
        Some(ms) => ms_to_ceiling_seconds(ms)?,
        None => 5,
    };
    cmd.arg("--cpu-time-limit-seconds")
        .arg(cpu_time_limit_seconds.to_string());
    cmd.arg("--solve-fuel")
        .arg(test.solve_fuel.unwrap_or(X07TEST_SOLVE_FUEL).to_string());

    let output = cmd.output().with_context(|| {
        format!(
            "exec {}",
            crate::util::resolve_sibling_or_path("x07-os-runner").display()
        )
    })?;

    let mut result = TestCaseResult {
        id: test.id.clone(),
        world: test.world.as_str().to_string(),
        expect: test.expect.as_str().to_string(),
        status: "error".to_string(),
        duration_ms: 0,
        entry_kind: test.entry_kind.as_str().to_string(),
        failure_kind: None,
        contract_repro_path: None,
        entry: Some(test.entry.clone()),
        fixture_root: None,
        compile: None,
        run: None,
        diags: Vec::new(),
    };

    let report: OsRunnerReportRaw = match serde_json::from_slice(&output.stdout) {
        Ok(r) => r,
        Err(err) => {
            result.diags.push(Diag::new(
                "ETEST_OS_RUNNER_JSON",
                format!("x07-os-runner did not emit valid JSON: {err}"),
            ));
            if !output.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                result
                    .diags
                    .push(Diag::new("ETEST_OS_RUNNER_STDERR", stderr.to_string()));
            }
            result.duration_ms = start.elapsed().as_millis() as u64;
            if cleanup_dir {
                rm_rf(&out_dir);
            }
            return Ok(result);
        }
    };
    let OsRunnerReportRaw {
        compile,
        solve,
        sandbox_backend,
        runtime_attestation,
    } = report;

    result.compile = Some(CompileSection {
        ok: compile.ok,
        exit_code: Some(compile.exit_status),
        compiler_diags: Vec::new(),
        artifact_path: Some(display_path(&exe_out_path)),
        c_bytes: Some(compile.c_source_size),
    });

    if !compile.ok {
        if let Some(msg) = compile.compile_error {
            result.diags.push(Diag::new("ETEST_COMPILE", msg));
        } else {
            result
                .diags
                .push(Diag::new("ETEST_COMPILE", "compile failed"));
        }
        result.duration_ms = start.elapsed().as_millis() as u64;
        if cleanup_dir {
            rm_rf(&out_dir);
        }
        return Ok(result);
    }

    let Some(solve) = solve else {
        result.diags.push(Diag::new(
            "ETEST_RUN",
            "missing solve section in x07-os-runner report",
        ));
        result.duration_ms = start.elapsed().as_millis() as u64;
        if cleanup_dir {
            rm_rf(&out_dir);
        }
        return Ok(result);
    };

    result.run = Some(RunSection {
        ok: solve.ok,
        exit_code: solve.exit_status,
        fuel_used: solve.fuel_used,
        mem_stats: solve.mem_stats,
        sched_stats: solve.sched_stats,
        solve_output_b64: Some(solve.solve_output_b64.clone()),
        stdout_b64: Some(solve.stdout_b64),
        stderr_b64: Some(solve.stderr_b64),
        failure_code_u32: None,
        sandbox_backend,
        runtime_attestation,
        effect_log_digests: Vec::new(),
        capsule_ids: Vec::new(),
    });

    if attach_test_sandbox_evidence(&args.manifest, test, &mut result) {
        result.duration_ms = start.elapsed().as_millis() as u64;
        if cleanup_dir {
            rm_rf(&out_dir);
        }
        return Ok(result);
    }

    if !solve.ok || solve.exit_status != 0 {
        if let Some(trap) = solve.trap.as_deref() {
            match contract_repro::try_parse_contract_trap(trap) {
                Ok(Some(info)) => {
                    let source = contract_repro::SourceInfo {
                        mode: "x07test".to_string(),
                        tests_manifest_path: Some(display_path(&args.manifest)),
                        test_id: Some(test.id.clone()),
                        test_entry: Some(test.entry.clone()),
                        target_kind: None,
                        target_path: None,
                    };
                    let runner_cfg = RunnerConfig {
                        world: test.world,
                        fixture_fs_dir: None,
                        fixture_fs_root: None,
                        fixture_fs_latency_index: None,
                        fixture_rr_dir: None,
                        fixture_kv_dir: None,
                        fixture_kv_seed: None,
                        solve_fuel: test.solve_fuel.unwrap_or(X07TEST_SOLVE_FUEL),
                        max_memory_bytes: 64 * 1024 * 1024,
                        max_output_bytes: 1024 * 1024,
                        cpu_time_limit_seconds,
                        debug_borrow_checks: false,
                    };

                    match contract_repro::write_repro(
                        &args.artifact_dir,
                        test.world.as_str(),
                        &runner_cfg,
                        &[],
                        info.payload,
                        source,
                        &info.clause_id,
                    ) {
                        Ok(path) => {
                            result.failure_kind = Some("contract_violation".to_string());
                            result.contract_repro_path = Some(display_path(&path));
                        }
                        Err(err) => {
                            result
                                .diags
                                .push(Diag::new("X07T_ECONTRACT_REPRO_WRITE", err.to_string()));
                        }
                    }

                    result.status = "error".to_string();
                    result.duration_ms = start.elapsed().as_millis() as u64;
                    if cleanup_dir {
                        rm_rf(&out_dir);
                    }
                    return Ok(result);
                }
                Ok(None) => {}
                Err(err) => {
                    result
                        .diags
                        .push(Diag::new("X07T_ECONTRACT_TRAP_PARSE", err.to_string()));
                    result.status = "error".to_string();
                    result.duration_ms = start.elapsed().as_millis() as u64;
                    if cleanup_dir {
                        rm_rf(&out_dir);
                    }
                    return Ok(result);
                }
            }
        }

        if let Some(trap) = solve.trap.as_deref() {
            result
                .diags
                .push(Diag::new("X07T_RUN_TRAP", "runner trapped").with_details(
                    serde_json::json!({
                        "trap": trap,
                    }),
                ));
        }
        result.diags.push(Diag::new(
            "ETEST_RUN",
            format!(
                "runner failed: ok={} exit_status={}",
                solve.ok, solve.exit_status
            ),
        ));
        result.duration_ms = start.elapsed().as_millis() as u64;
        if cleanup_dir {
            rm_rf(&out_dir);
        }
        return Ok(result);
    }

    let b64 = base64::engine::general_purpose::STANDARD;
    let status_bytes = match b64.decode(solve.solve_output_b64.as_bytes()) {
        Ok(b) => b,
        Err(err) => {
            result.diags.push(Diag::new(
                "EBAD_STATUS",
                format!("invalid base64 solve_output: {err}"),
            ));
            result.duration_ms = start.elapsed().as_millis() as u64;
            if cleanup_dir {
                rm_rf(&out_dir);
            }
            return Ok(result);
        }
    };

    let status_v1 = match parse_evtest_status_v1(&status_bytes) {
        Ok(x) => x,
        Err(msg) => {
            result.diags.push(Diag::new("EBAD_STATUS", msg.to_string()));
            result.duration_ms = start.elapsed().as_millis() as u64;
            if cleanup_dir {
                rm_rf(&out_dir);
            }
            return Ok(result);
        }
    };
    let tag = status_v1.tag;
    let code_u32 = status_v1.code_u32;

    if let Some(run) = result.run.as_mut() {
        run.failure_code_u32 = Some(code_u32 as u64);
    }

    result.status = compute_status(test.expect, tag);
    result.duration_ms = start.elapsed().as_millis() as u64;

    if let Some(details) = status_v1.assert_bytes_eq_details {
        result.diags.push(
            Diag::new("X07T_ASSERT_BYTES_EQ", "assert_bytes_eq failed").with_details(details),
        );
    }

    if args.verbose && args.keep_artifacts {
        eprintln!("artifacts: {}", out_dir.display());
        eprintln!("driver: {}", driver_path.display());
    }

    if cleanup_dir {
        rm_rf(&out_dir);
    }

    Ok(result)
}

fn runner_config_for_test(test: &TestDecl) -> Result<RunnerConfig> {
    let cpu_time_limit_seconds = match test.timeout_ms {
        Some(ms) => ms_to_ceiling_seconds(ms)?,
        None => 5,
    };

    let mut cfg = RunnerConfig {
        world: test.world,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: test.solve_fuel.unwrap_or(X07TEST_SOLVE_FUEL),
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds,
        debug_borrow_checks: false,
    };

    match test.world {
        WorldId::SolvePure => {}
        WorldId::SolveFs => {
            let fixture = test
                .fixture_root
                .as_deref()
                .context("solve-fs requires fixture_root")?;
            cfg.fixture_fs_dir = Some(fixture.to_path_buf());
            if fixture.join("root").is_dir() {
                cfg.fixture_fs_root = Some(PathBuf::from("root"));
            }
            if fixture.join("latency.json").is_file() {
                cfg.fixture_fs_latency_index = Some(PathBuf::from("latency.json"));
            }
        }
        WorldId::SolveRr => {
            let fixture = test
                .fixture_root
                .as_deref()
                .context("solve-rr requires fixture_root")?;
            cfg.fixture_rr_dir = Some(fixture.to_path_buf());
        }
        WorldId::SolveKv => {
            let fixture = test
                .fixture_root
                .as_deref()
                .context("solve-kv requires fixture_root")?;
            cfg.fixture_kv_dir = Some(fixture.to_path_buf());
            if fixture.join("seed.json").is_file() {
                cfg.fixture_kv_seed = Some(PathBuf::from("seed.json"));
            }
        }
        WorldId::SolveFull => {
            let fixture = test
                .fixture_root
                .as_deref()
                .context("solve-full requires fixture_root")?;
            let fs_fixture = fixture.join("fs");
            let rr_fixture = fixture.join("rr");
            let kv_fixture = fixture.join("kv");

            cfg.fixture_fs_dir = Some(fs_fixture.clone());
            if fs_fixture.join("root").is_dir() {
                cfg.fixture_fs_root = Some(PathBuf::from("root"));
            }
            if fs_fixture.join("latency.json").is_file() {
                cfg.fixture_fs_latency_index = Some(PathBuf::from("latency.json"));
            }

            cfg.fixture_rr_dir = Some(rr_fixture.clone());

            cfg.fixture_kv_dir = Some(kv_fixture.clone());
            if kv_fixture.join("seed.json").is_file() {
                cfg.fixture_kv_seed = Some(PathBuf::from("seed.json"));
            }
        }
        WorldId::RunOs | WorldId::RunOsSandboxed => {
            anyhow::bail!("internal error: x07 test does not support OS worlds");
        }
    }

    Ok(cfg)
}

fn ms_to_ceiling_seconds(ms: u64) -> Result<u64> {
    if ms == 0 {
        anyhow::bail!("timeout_ms must be >= 1");
    }
    Ok(ms.div_ceil(1000))
}

fn compute_status(expect: Expect, tag: u8) -> String {
    match (expect, tag) {
        (Expect::Skip, _) => "skip",
        (Expect::Pass, 1) => "pass",
        (Expect::Pass, 0) => "fail",
        (Expect::Pass, 2) => "skip",
        (Expect::Fail, 0) => "xfail_fail",
        (Expect::Fail, 1) => "xfail_pass",
        (Expect::Fail, 2) => "skip",
        (Expect::Pass, _) | (Expect::Fail, _) => "error",
    }
    .to_string()
}

#[derive(Debug, Clone)]
struct EvtestStatusV1 {
    tag: u8,
    code_u32: u32,
    assert_bytes_eq_details: Option<serde_json::Value>,
}

fn parse_assert_bytes_eq_payload_v1(payload: &[u8]) -> Result<serde_json::Value> {
    if payload.len() < 13 {
        anyhow::bail!(
            "X7TEST_ASSERT_BYTES_EQ payload too short: got {}",
            payload.len()
        );
    }
    if &payload[..4] != b"X7T1" {
        anyhow::bail!("X7TEST_ASSERT_BYTES_EQ payload magic mismatch");
    }
    let prefix_max_bytes = payload[4] as usize;
    if prefix_max_bytes > 64 {
        anyhow::bail!(
            "X7TEST_ASSERT_BYTES_EQ prefix_max_bytes must be <= 64, got {}",
            prefix_max_bytes
        );
    }
    let got_len = u32::from_le_bytes([payload[5], payload[6], payload[7], payload[8]]);
    let expected_len = u32::from_le_bytes([payload[9], payload[10], payload[11], payload[12]]);
    let got_prefix_len = std::cmp::min(got_len as usize, prefix_max_bytes);
    let expected_prefix_len = std::cmp::min(expected_len as usize, prefix_max_bytes);
    let total = 13 + got_prefix_len + expected_prefix_len;
    if payload.len() != total {
        anyhow::bail!(
            "X7TEST_ASSERT_BYTES_EQ payload length mismatch: expected {} got {}",
            total,
            payload.len()
        );
    }
    let got_prefix = &payload[13..13 + got_prefix_len];
    let expected_prefix = &payload[13 + got_prefix_len..];
    Ok(serde_json::json!({
        "prefix_max_bytes": prefix_max_bytes,
        "got": {
            "len": got_len,
            "prefix_hex": crate::util::hex_lower(got_prefix),
            "prefix_utf8_lossy": String::from_utf8_lossy(got_prefix),
        },
        "expected": {
            "len": expected_len,
            "prefix_hex": crate::util::hex_lower(expected_prefix),
            "prefix_utf8_lossy": String::from_utf8_lossy(expected_prefix),
        }
    }))
}

fn parse_evtest_status_v1(status: &[u8]) -> Result<EvtestStatusV1> {
    if status.len() < 5 {
        anyhow::bail!("X7TEST_STATUS_V1 must be >= 5 bytes, got {}", status.len());
    }
    let tag = status[0];
    if !matches!(tag, 0..=2) {
        anyhow::bail!("X7TEST_STATUS_V1 tag must be 0, 1, or 2, got {}", tag);
    }
    let code_u32 = u32::from_le_bytes([status[1], status[2], status[3], status[4]]);
    let trailing = &status[5..];
    if !trailing.is_empty() {
        if tag != 0 || code_u32 != 1003 {
            anyhow::bail!("X7TEST_STATUS_V1 must be 5 bytes, got {}", status.len());
        }
        let details = parse_assert_bytes_eq_payload_v1(trailing)?;
        return Ok(EvtestStatusV1 {
            tag,
            code_u32,
            assert_bytes_eq_details: Some(details),
        });
    }
    Ok(EvtestStatusV1 {
        tag,
        code_u32,
        assert_bytes_eq_details: None,
    })
}

fn build_test_driver_x07ast_json(test: &TestDecl) -> Result<Vec<u8>> {
    let (module_id, _name) = test
        .entry
        .rsplit_once('.')
        .context("entry must contain '.'")?;

    let mut imports: Vec<&str> = vec!["std.test"];
    if module_id != "std.test" {
        imports.push(module_id);
    }
    imports.sort_unstable();
    imports.dedup();

    let call_entry = serde_json::json!([test.entry]);
    let solve = match (test.entry_kind, test.returns) {
        (TestEntryKind::Defn, TestReturns::ResultI32) => {
            serde_json::json!(["std.test.status_from_result_i32", call_entry])
        }
        (TestEntryKind::Defn, TestReturns::BytesStatusV1) => call_entry,
        (TestEntryKind::Defasync, TestReturns::ResultI32) => serde_json::json!([
            "begin",
            ["let", "task", call_entry],
            ["task.spawn", "task"],
            ["std.test.status_from_result_i32", ["await", "task"]]
        ]),
        (TestEntryKind::Defasync, TestReturns::BytesStatusV1) => serde_json::json!([
            "begin",
            ["let", "task", call_entry],
            ["task.spawn", "task"],
            if test.entry_result_ty == "result_bytes" {
                serde_json::json!(["let", "out", ["task.join.result_bytes", "task"]])
            } else {
                serde_json::json!(["let", "out", ["await", "task"]])
            },
            if test.entry_result_ty == "result_bytes" {
                serde_json::json!([
                    "if",
                    ["result_bytes.is_ok", "out"],
                    ["std.test.status_ok"],
                    ["std.test.status_fail", ["result_bytes.err_code", "out"]]
                ])
            } else {
                serde_json::json!("out")
            }
        ]),
    };

    let file = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": imports,
        "decls": [],
        "solve": solve,
    });

    let mut out = serde_json::to_vec(&file)?;
    out.push(b'\n');
    Ok(out)
}

#[derive(Debug, serde::Deserialize)]
struct ManifestRaw {
    schema_version: String,
    #[serde(default)]
    tests: Vec<TestRaw>,
}

#[derive(Debug, serde::Deserialize)]
struct TestRaw {
    id: String,
    world: String,
    entry: String,
    #[serde(default)]
    solve_fuel: Option<u64>,
    #[serde(default)]
    input_b64: Option<String>,
    #[serde(default)]
    input_path: Option<String>,
    #[serde(default)]
    pbt: Option<pbt::PbtDeclRaw>,
    #[serde(default)]
    expect: Option<String>,
    #[serde(default)]
    returns: Option<String>,
    #[serde(default)]
    fixture_root: Option<String>,
    #[serde(default)]
    policy_json: Option<String>,
    #[serde(default)]
    require_runtime_attestation: bool,
    #[serde(default)]
    required_capsules: Vec<String>,
    #[serde(default)]
    sandbox_smoke: bool,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct ValidatedManifest {
    manifest_dir: PathBuf,
    tests: Vec<TestDecl>,
}

#[derive(Debug, Clone)]
struct ManifestDiag {
    code: &'static str,
    message: String,
    path: String,
}

fn validate_manifest_json(manifest_path: &Path) -> Result<ValidatedManifest, Vec<ManifestDiag>> {
    let mut diags: Vec<ManifestDiag> = Vec::new();

    let bytes = match std::fs::read(manifest_path) {
        Ok(b) => b,
        Err(err) => {
            diags.push(ManifestDiag {
                code: "ETEST_MANIFEST_IO",
                message: format!("failed to read manifest: {err}"),
                path: "".to_string(),
            });
            return Err(diags);
        }
    };

    let raw: ManifestRaw = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(err) => {
            diags.push(ManifestDiag {
                code: "ETEST_MANIFEST_JSON",
                message: format!("invalid JSON: {err}"),
                path: "".to_string(),
            });
            return Err(diags);
        }
    };

    let allows_input = match raw.schema_version.as_str() {
        "x07.tests_manifest@0.1.0" => false,
        "x07.tests_manifest@0.2.0" => true,
        _ => false,
    };
    if raw.schema_version != "x07.tests_manifest@0.1.0"
        && raw.schema_version != "x07.tests_manifest@0.2.0"
    {
        diags.push(ManifestDiag {
            code: "ETEST_SCHEMA_VERSION",
            message: format!(
                "schema_version must be x07.tests_manifest@0.1.0 or x07.tests_manifest@0.2.0, got {}",
                raw.schema_version
            ),
            path: "/schema_version".to_string(),
        });
    }

    if raw.tests.is_empty() {
        diags.push(ManifestDiag {
            code: "ETEST_TESTS_EMPTY",
            message: "tests array is empty".to_string(),
            path: "/tests".to_string(),
        });
    }

    let manifest_dir = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut out: Vec<TestDecl> = Vec::new();

    for (i, t) in raw.tests.iter().enumerate() {
        let base = format!("/tests/{i}");

        if t.id.is_empty() {
            diags.push(ManifestDiag {
                code: "ETEST_ID_EMPTY",
                message: "id must be non-empty".to_string(),
                path: format!("{base}/id"),
            });
            continue;
        }
        if !is_ascii_printable(&t.id) {
            diags.push(ManifestDiag {
                code: "ETEST_ID_NON_ASCII",
                message: "id must be ASCII printable".to_string(),
                path: format!("{base}/id"),
            });
            continue;
        }
        if let Some(prev) = seen.get(&t.id) {
            diags.push(ManifestDiag {
                code: "ETEST_ID_DUPLICATE",
                message: format!("duplicate id: {} (previous at index {})", t.id, prev),
                path: format!("{base}/id"),
            });
            continue;
        }
        seen.insert(t.id.clone(), i);

        let world = match parse_world(&t.world) {
            Some(w) => w,
            None => {
                diags.push(ManifestDiag {
                    code: "ETEST_WORLD_INVALID",
                    message: format!(
                        "invalid world: {} (allowed: solve-pure, solve-fs, solve-rr, solve-kv, solve-full, run-os, run-os-sandboxed)",
                        t.world
                    ),
                    path: format!("{base}/world"),
                });
                continue;
            }
        };

        let input = if t.input_b64.is_some() || t.input_path.is_some() {
            if !allows_input {
                diags.push(ManifestDiag {
                    code: "ETEST_INPUT_NOT_ALLOWED_V010",
                    message: "input_b64/input_path is only allowed in x07.tests_manifest@0.2.0"
                        .to_string(),
                    path: format!("{base}/input_b64"),
                });
                None
            } else if !world.is_eval_world() {
                diags.push(ManifestDiag {
                    code: "ETEST_INPUT_UNSUPPORTED_WORLD",
                    message: format!(
                        "input_b64/input_path is not supported for world {}",
                        world.as_str()
                    ),
                    path: format!("{base}/world"),
                });
                None
            } else if t.input_b64.is_some() && t.input_path.is_some() {
                diags.push(ManifestDiag {
                    code: "ETEST_INPUT_CONFLICT",
                    message: "at most one of input_b64 or input_path may be set".to_string(),
                    path: format!("{base}/input_b64"),
                });
                None
            } else if let Some(s) = t.input_b64.as_deref() {
                let b64 = base64::engine::general_purpose::STANDARD;
                match b64.decode(s.as_bytes()) {
                    Ok(bytes) => Some(bytes),
                    Err(err) => {
                        diags.push(ManifestDiag {
                            code: "ETEST_INPUT_B64_INVALID",
                            message: format!("invalid base64 input_b64: {err}"),
                            path: format!("{base}/input_b64"),
                        });
                        None
                    }
                }
            } else if let Some(p) = t.input_path.as_deref() {
                if p.contains('\\') {
                    diags.push(ManifestDiag {
                        code: "ETEST_INPUT_UNSAFE_PATH",
                        message: format!("input_path must not contain '\\\\': {p}"),
                        path: format!("{base}/input_path"),
                    });
                    None
                } else {
                    let rel = Path::new(p);
                    if let Err(err) = x07_host_runner::ensure_safe_rel_path(rel) {
                        diags.push(ManifestDiag {
                            code: "ETEST_INPUT_UNSAFE_PATH",
                            message: format!("unsafe input_path: {err}"),
                            path: format!("{base}/input_path"),
                        });
                        None
                    } else {
                        let abs = manifest_dir.join(rel);
                        match std::fs::read(&abs) {
                            Ok(bytes) => Some(bytes),
                            Err(err) => {
                                diags.push(ManifestDiag {
                                    code: "ETEST_INPUT_PATH_READ_FAILED",
                                    message: format!("failed to read input_path {p}: {err}"),
                                    path: format!("{base}/input_path"),
                                });
                                None
                            }
                        }
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        if !t.entry.contains('.') {
            diags.push(ManifestDiag {
                code: "ETEST_ENTRY_INVALID",
                message: format!("entry must contain '.', got: {}", t.entry),
                path: format!("{base}/entry"),
            });
            continue;
        }
        if let Err(msg) = x07c::validate::validate_symbol(&t.entry) {
            diags.push(ManifestDiag {
                code: "ETEST_ENTRY_INVALID",
                message: format!("invalid entry: {msg}"),
                path: format!("{base}/entry"),
            });
            continue;
        }

        let expect = match parse_expect(t.expect.as_deref()) {
            Some(e) => e,
            None => {
                diags.push(ManifestDiag {
                    code: "ETEST_EXPECT_INVALID",
                    message: format!("invalid expect: {:?}", t.expect),
                    path: format!("{base}/expect"),
                });
                continue;
            }
        };

        let is_pbt = t.pbt.is_some();
        let returns = if is_pbt && t.returns.is_none() {
            TestReturns::BytesStatusV1
        } else {
            match parse_returns(t.returns.as_deref()) {
                Some(r) => r,
                None => {
                    diags.push(ManifestDiag {
                        code: "ETEST_RETURNS_INVALID",
                        message: format!("invalid returns: {:?}", t.returns),
                        path: format!("{base}/returns"),
                    });
                    continue;
                }
            }
        };

        if is_pbt && returns != TestReturns::BytesStatusV1 {
            diags.push(ManifestDiag {
                code: "X07T_EPBT_MANIFEST_INVALID",
                message: "PBT tests must use returns=\"bytes_status_v1\"".to_string(),
                path: format!("{base}/returns"),
            });
            continue;
        }

        if is_pbt && input.is_some() {
            diags.push(ManifestDiag {
                code: "X07T_EPBT_MANIFEST_INVALID",
                message: "PBT tests must not set input_b64/input_path".to_string(),
                path: format!("{base}/input_b64"),
            });
            continue;
        }

        let pbt_decl = if let Some(raw) = t.pbt.as_ref() {
            if !world.is_eval_world() {
                diags.push(ManifestDiag {
                    code: "X07T_EPBT_UNSUPPORTED_WORLD",
                    message: format!(
                        "PBT tests are only supported for deterministic solve worlds, got {}",
                        world.as_str()
                    ),
                    path: format!("{base}/world"),
                });
                None
            } else if raw.params.is_empty() {
                diags.push(ManifestDiag {
                    code: "X07T_EPBT_PARAM_EMPTY",
                    message: "pbt.params must be non-empty".to_string(),
                    path: format!("{base}/pbt/params"),
                });
                None
            } else {
                let cases = raw.cases.unwrap_or(100);
                if cases == 0 {
                    diags.push(ManifestDiag {
                        code: "X07T_EPBT_MANIFEST_INVALID",
                        message: "pbt.cases must be >= 1".to_string(),
                        path: format!("{base}/pbt/cases"),
                    });
                    None
                } else {
                    let max_shrinks = raw.max_shrinks.unwrap_or(4096);

                    let mut params: Vec<pbt::PbtParam> = Vec::new();
                    let mut seen_param_names: BTreeMap<String, usize> = BTreeMap::new();
                    let mut ok = true;

                    for (pi, p) in raw.params.iter().enumerate() {
                        let pbase = format!("{base}/pbt/params/{pi}");

                        if let Err(msg) = x07c::validate::validate_local_name(&p.name) {
                            diags.push(ManifestDiag {
                                code: "X07T_EPBT_MANIFEST_INVALID",
                                message: format!("invalid param name: {msg}"),
                                path: format!("{pbase}/name"),
                            });
                            ok = false;
                            continue;
                        }
                        if p.name == "input" {
                            diags.push(ManifestDiag {
                                code: "X07T_EPBT_MANIFEST_INVALID",
                                message: "param name \"input\" is reserved".to_string(),
                                path: format!("{pbase}/name"),
                            });
                            ok = false;
                            continue;
                        }
                        if let Some(prev) = seen_param_names.get(&p.name) {
                            diags.push(ManifestDiag {
                                code: "X07T_EPBT_MANIFEST_INVALID",
                                message: format!(
                                    "duplicate param name: {} (previous at index {})",
                                    p.name, prev
                                ),
                                path: format!("{pbase}/name"),
                            });
                            ok = false;
                            continue;
                        }
                        seen_param_names.insert(p.name.clone(), pi);

                        let gen = match p.gen.kind.as_str() {
                            "i32" => {
                                let min = p.gen.min.unwrap_or(i32::MIN);
                                let max = p.gen.max.unwrap_or(i32::MAX);
                                if min > max {
                                    diags.push(ManifestDiag {
                                        code: "X07T_EPBT_MANIFEST_INVALID",
                                        message: format!(
                                            "i32 gen requires min <= max, got min={min} max={max}"
                                        ),
                                        path: format!("{pbase}/gen"),
                                    });
                                    ok = false;
                                    continue;
                                }
                                pbt::PbtGen::I32 { min, max }
                            }
                            "bytes" => {
                                let Some(max_len) = p.gen.max_len else {
                                    diags.push(ManifestDiag {
                                        code: "X07T_EPBT_MANIFEST_INVALID",
                                        message: "bytes gen requires max_len".to_string(),
                                        path: format!("{pbase}/gen/max_len"),
                                    });
                                    ok = false;
                                    continue;
                                };
                                pbt::PbtGen::Bytes { max_len }
                            }
                            other => {
                                diags.push(ManifestDiag {
                                    code: "X07T_EPBT_UNKNOWN_GEN_KIND",
                                    message: format!("unknown gen kind: {other:?}"),
                                    path: format!("{pbase}/gen/kind"),
                                });
                                ok = false;
                                continue;
                            }
                        };

                        params.push(pbt::PbtParam {
                            name: p.name.clone(),
                            gen,
                        });
                    }

                    let case_budget_raw = raw.case_budget.as_ref();
                    let case_budget = pbt::PbtCaseBudget {
                        fuel: case_budget_raw.and_then(|b| b.fuel).unwrap_or(200_000),
                        timeout_ms: case_budget_raw.and_then(|b| b.timeout_ms).unwrap_or(250),
                        max_mem_bytes: case_budget_raw
                            .and_then(|b| b.max_mem_bytes)
                            .unwrap_or(64 * 1024 * 1024),
                        max_output_bytes: case_budget_raw
                            .and_then(|b| b.max_output_bytes)
                            .unwrap_or(1024 * 1024),
                    };

                    let budget_scope = if let Some(raw_scope) = raw.budget_scope.as_ref() {
                        let mut scope_ok = true;
                        let mut field_or_diag = |field: &'static str, value: Option<u64>| -> i32 {
                            let Some(v) = value else {
                                return 0;
                            };
                            match pbt::checked_u64_to_i32(field, v) {
                                Ok(x) => x,
                                Err(err) => {
                                    diags.push(ManifestDiag {
                                        code: "X07T_EPBT_MANIFEST_INVALID",
                                        message: err.to_string(),
                                        path: format!("{base}/pbt/budget_scope/{field}"),
                                    });
                                    scope_ok = false;
                                    0
                                }
                            }
                        };

                        let scope = pbt::PbtBudgetScope {
                            alloc_bytes: field_or_diag("alloc_bytes", raw_scope.alloc_bytes),
                            alloc_calls: field_or_diag("alloc_calls", raw_scope.alloc_calls),
                            realloc_calls: field_or_diag("realloc_calls", raw_scope.realloc_calls),
                            memcpy_bytes: field_or_diag("memcpy_bytes", raw_scope.memcpy_bytes),
                            sched_ticks: field_or_diag("sched_ticks", raw_scope.sched_ticks),
                        };
                        if scope_ok
                            && (scope.alloc_bytes > 0
                                || scope.alloc_calls > 0
                                || scope.realloc_calls > 0
                                || scope.memcpy_bytes > 0
                                || scope.sched_ticks > 0)
                        {
                            Some(scope)
                        } else {
                            if !scope_ok {
                                ok = false;
                            }
                            None
                        }
                    } else {
                        None
                    };

                    if case_budget.fuel == 0 {
                        diags.push(ManifestDiag {
                            code: "X07T_EPBT_MANIFEST_INVALID",
                            message: "pbt.case_budget.fuel must be >= 1".to_string(),
                            path: format!("{base}/pbt/case_budget/fuel"),
                        });
                        ok = false;
                    }
                    if case_budget.timeout_ms == 0 {
                        diags.push(ManifestDiag {
                            code: "X07T_EPBT_MANIFEST_INVALID",
                            message: "pbt.case_budget.timeout_ms must be >= 1".to_string(),
                            path: format!("{base}/pbt/case_budget/timeout_ms"),
                        });
                        ok = false;
                    }
                    if case_budget.max_mem_bytes == 0 {
                        diags.push(ManifestDiag {
                            code: "X07T_EPBT_MANIFEST_INVALID",
                            message: "pbt.case_budget.max_mem_bytes must be >= 1".to_string(),
                            path: format!("{base}/pbt/case_budget/max_mem_bytes"),
                        });
                        ok = false;
                    }
                    if case_budget.max_output_bytes == 0 {
                        diags.push(ManifestDiag {
                            code: "X07T_EPBT_MANIFEST_INVALID",
                            message: "pbt.case_budget.max_output_bytes must be >= 1".to_string(),
                            path: format!("{base}/pbt/case_budget/max_output_bytes"),
                        });
                        ok = false;
                    }

                    if ok {
                        Some(pbt::PbtDecl {
                            cases,
                            max_shrinks,
                            params,
                            case_budget,
                            budget_scope,
                        })
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        };
        if t.pbt.is_some() && pbt_decl.is_none() {
            continue;
        }

        if let Some(ms) = t.timeout_ms {
            if ms == 0 {
                diags.push(ManifestDiag {
                    code: "ETEST_TIMEOUT_INVALID",
                    message: "timeout_ms must be >= 1".to_string(),
                    path: format!("{base}/timeout_ms"),
                });
                continue;
            }
        }

        let fixture_root = match world {
            WorldId::SolvePure => {
                if t.fixture_root.is_some() {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_FORBIDDEN",
                        message: "fixture_root must not be set for solve-pure".to_string(),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                None
            }
            WorldId::SolveFs | WorldId::SolveRr | WorldId::SolveKv => {
                let Some(fr) = t.fixture_root.as_deref() else {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_REQUIRED",
                        message: format!("fixture_root is required for {}", world.as_str()),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                };
                if fr.contains('\\') {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_UNSAFE_PATH",
                        message: format!("fixture_root must not contain '\\\\': {fr}"),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                let rel = Path::new(fr);
                if let Err(err) = x07_host_runner::ensure_safe_rel_path(rel) {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_UNSAFE_PATH",
                        message: format!("unsafe fixture_root path: {err}"),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                let abs = manifest_dir.join(rel);
                if !abs.is_dir() {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_MISSING",
                        message: format!("fixture_root must be an existing directory: {fr}"),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                Some(abs)
            }
            WorldId::SolveFull => {
                let Some(fr) = t.fixture_root.as_deref() else {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_REQUIRED",
                        message: "fixture_root is required for solve-full".to_string(),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                };
                if fr.contains('\\') {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_UNSAFE_PATH",
                        message: format!("fixture_root must not contain '\\\\': {fr}"),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                let rel = Path::new(fr);
                if let Err(err) = x07_host_runner::ensure_safe_rel_path(rel) {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_UNSAFE_PATH",
                        message: format!("unsafe fixture_root path: {err}"),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                let abs = manifest_dir.join(rel);
                if !abs.is_dir() {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_MISSING",
                        message: format!("fixture_root must be an existing directory: {fr}"),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                let missing_sub = ["fs", "rr", "kv"]
                    .into_iter()
                    .find(|sub| !abs.join(sub).is_dir());
                if let Some(sub) = missing_sub {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_MISSING",
                        message: format!("solve-full fixture_root must contain {sub}/ directory"),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                Some(abs)
            }
            WorldId::RunOs | WorldId::RunOsSandboxed => {
                if t.fixture_root.is_some() {
                    diags.push(ManifestDiag {
                        code: "ETEST_FIXTURE_FORBIDDEN",
                        message: "fixture_root must not be set for OS worlds".to_string(),
                        path: format!("{base}/fixture_root"),
                    });
                    continue;
                }
                None
            }
        };

        let policy_json = match world {
            WorldId::RunOsSandboxed => {
                let Some(pol) = t.policy_json.as_deref() else {
                    diags.push(ManifestDiag {
                        code: "ETEST_POLICY_REQUIRED",
                        message: "policy_json is required for run-os-sandboxed".to_string(),
                        path: format!("{base}/policy_json"),
                    });
                    continue;
                };
                if pol.contains('\\') {
                    diags.push(ManifestDiag {
                        code: "ETEST_POLICY_UNSAFE_PATH",
                        message: format!("policy_json must not contain '\\\\': {pol}"),
                        path: format!("{base}/policy_json"),
                    });
                    continue;
                }
                let rel = Path::new(pol);
                if let Err(err) = x07_host_runner::ensure_safe_rel_path(rel) {
                    diags.push(ManifestDiag {
                        code: "ETEST_POLICY_UNSAFE_PATH",
                        message: format!("unsafe policy_json path: {err}"),
                        path: format!("{base}/policy_json"),
                    });
                    continue;
                }
                let abs = manifest_dir.join(rel);
                if !abs.is_file() {
                    diags.push(ManifestDiag {
                        code: "ETEST_POLICY_MISSING",
                        message: format!("policy_json must be an existing file: {pol}"),
                        path: format!("{base}/policy_json"),
                    });
                    continue;
                }
                Some(abs)
            }
            WorldId::RunOs => {
                if t.policy_json.is_some() {
                    diags.push(ManifestDiag {
                        code: "ETEST_POLICY_FORBIDDEN",
                        message: "policy_json is only valid for run-os-sandboxed".to_string(),
                        path: format!("{base}/policy_json"),
                    });
                    continue;
                }
                None
            }
            _ => {
                if t.policy_json.is_some() {
                    diags.push(ManifestDiag {
                        code: "ETEST_POLICY_FORBIDDEN",
                        message: "policy_json is only valid for run-os-sandboxed".to_string(),
                        path: format!("{base}/policy_json"),
                    });
                    continue;
                }
                None
            }
        };

        if (t.require_runtime_attestation || t.sandbox_smoke) && world != WorldId::RunOsSandboxed {
            diags.push(ManifestDiag {
                code: "X07TEST_RUNTIME_ATTEST_REQUIRED",
                message: "sandbox smoke and runtime attestation evidence are only supported for run-os-sandboxed tests".to_string(),
                path: format!("{base}/world"),
            });
            continue;
        }

        let mut required_capsules = t.required_capsules.clone();
        required_capsules.sort();
        required_capsules.dedup();
        for (ci, capsule_id) in required_capsules.iter().enumerate() {
            if capsule_id.trim().is_empty() {
                diags.push(ManifestDiag {
                    code: "X07TEST_CAPSULE_EVIDENCE_MISSING",
                    message: "required_capsules entries must be non-empty".to_string(),
                    path: format!("{base}/required_capsules/{ci}"),
                });
                continue;
            }
            if let Err(msg) = x07c::validate::validate_symbol(capsule_id) {
                diags.push(ManifestDiag {
                    code: "X07TEST_CAPSULE_EVIDENCE_MISSING",
                    message: format!("invalid required_capsules entry: {msg}"),
                    path: format!("{base}/required_capsules/{ci}"),
                });
                continue;
            }
        }

        out.push(TestDecl {
            id: t.id.clone(),
            world,
            entry: t.entry.clone(),
            entry_kind: TestEntryKind::Defn,
            entry_result_ty: "result_i32".to_string(),
            expect,
            returns,
            pbt: pbt_decl,
            input,
            fixture_root,
            policy_json,
            require_runtime_attestation: t.require_runtime_attestation || t.sandbox_smoke,
            required_capsules,
            sandbox_smoke: t.sandbox_smoke,
            timeout_ms: t.timeout_ms,
            solve_fuel: match t.solve_fuel {
                Some(0) => {
                    diags.push(ManifestDiag {
                        code: "ETEST_SOLVE_FUEL_INVALID",
                        message: "solve_fuel must be > 0".to_string(),
                        path: format!("{base}/solve_fuel"),
                    });
                    continue;
                }
                other => other,
            },
        });
    }

    if !diags.is_empty() {
        diags.sort_by(|a, b| {
            (a.path.as_str(), a.code, a.message.as_str()).cmp(&(
                b.path.as_str(),
                b.code,
                b.message.as_str(),
            ))
        });
        return Err(diags);
    }

    out.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(ValidatedManifest {
        manifest_dir,
        tests: out,
    })
}

fn is_ascii_printable(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, 0x20..=0x7e))
}

fn parse_world(s: &str) -> Option<WorldId> {
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

fn parse_expect(s: Option<&str>) -> Option<Expect> {
    match s.unwrap_or("pass") {
        "pass" => Some(Expect::Pass),
        "fail" => Some(Expect::Fail),
        "skip" => Some(Expect::Skip),
        _ => None,
    }
}

fn parse_returns(s: Option<&str>) -> Option<TestReturns> {
    match s.unwrap_or("result_i32") {
        "result_i32" => Some(TestReturns::ResultI32),
        "bytes_status_v1" => Some(TestReturns::BytesStatusV1),
        _ => None,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct X07TestReport {
    schema_version: String,
    tool: ToolInfo,
    invocation: InvocationInfo,
    summary: Summary,
    tests: Vec<TestCaseResult>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ToolInfo {
    name: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct InvocationInfo {
    argv: Vec<String>,
    cwd: String,
    started_at_unix_ms: u64,
    repeat: u32,
    jobs: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    module_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdlib_lock: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
struct Summary {
    passed: u64,
    failed: u64,
    skipped: u64,
    errors: u64,
    xfail_passed: u64,
    xfail_failed: u64,
    duration_ms: u64,
    compile_failures: u64,
    run_failures: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TestCaseResult {
    id: String,
    world: String,
    expect: String,
    status: String,
    duration_ms: u64,
    entry_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract_repro_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixture_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compile: Option<CompileSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run: Option<RunSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diags: Vec<Diag>,
}

impl TestCaseResult {
    fn skipped_due_to_fail_fast(test: &TestDecl) -> Self {
        Self {
            id: test.id.clone(),
            world: test.world.as_str().to_string(),
            expect: test.expect.as_str().to_string(),
            status: "skip".to_string(),
            duration_ms: 0,
            entry_kind: test.entry_kind.as_str().to_string(),
            failure_kind: None,
            contract_repro_path: None,
            entry: Some(test.entry.clone()),
            fixture_root: test.fixture_root.as_ref().map(display_path),
            compile: None,
            run: None,
            diags: vec![Diag::new(
                "EFAIL_FAST",
                "skipped due to earlier failure (fail-fast)",
            )],
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct CompileSection {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    compiler_diags: Vec<Diag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    c_bytes: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct RunSection {
    ok: bool,
    exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    fuel_used: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mem_stats: Option<x07_host_runner::MemStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sched_stats: Option<x07_host_runner::SchedStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    solve_output_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_code_u32: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_attestation: Option<RuntimeAttestationRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    effect_log_digests: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    capsule_ids: Vec<String>,
}

impl RunSection {
    fn from_runner_result(r: &RunnerResult) -> Self {
        let b64 = base64::engine::general_purpose::STANDARD;
        Self {
            ok: r.ok,
            exit_code: r.exit_status,
            fuel_used: r.fuel_used,
            mem_stats: r.mem_stats,
            sched_stats: r.sched_stats.clone(),
            solve_output_b64: Some(b64.encode(&r.solve_output)),
            stdout_b64: Some(b64.encode(&r.stdout)),
            stderr_b64: Some(b64.encode(&r.stderr)),
            failure_code_u32: None,
            sandbox_backend: None,
            runtime_attestation: None,
            effect_log_digests: Vec::new(),
            capsule_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct Diag {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl Diag {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: None,
            details: None,
        }
    }

    fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedRun {
    ok: bool,
    exit_status: i32,
    solve_output: Vec<u8>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    fuel_used: Option<u64>,
    heap_used: Option<u64>,
    mem_stats: Option<x07_host_runner::MemStats>,
    sched_stats: Option<x07_host_runner::SchedStats>,
    trap: Option<String>,
}

impl ObservedRun {
    fn from_runner_result(r: &RunnerResult) -> Self {
        Self {
            ok: r.ok,
            exit_status: r.exit_status,
            solve_output: r.solve_output.clone(),
            stdout: r.stdout.clone(),
            stderr: r.stderr.clone(),
            fuel_used: r.fuel_used,
            heap_used: r.heap_used,
            mem_stats: r.mem_stats,
            sched_stats: r.sched_stats.clone(),
            trap: r.trap.clone(),
        }
    }
}

fn finalize_report(
    args: &TestArgs,
    module_root_used: &Path,
    elapsed: std::time::Duration,
    tests: Vec<TestCaseResult>,
) -> X07TestReport {
    let mut summary = Summary::default();

    for t in &tests {
        match t.status.as_str() {
            "pass" => summary.passed += 1,
            "fail" => summary.failed += 1,
            "skip" => summary.skipped += 1,
            "error" => summary.errors += 1,
            "xfail_pass" => summary.xfail_passed += 1,
            "xfail_fail" => summary.xfail_failed += 1,
            _ => summary.errors += 1,
        }
        if t.compile.as_ref().is_some_and(|c| !c.ok) {
            summary.compile_failures += 1;
        }
        if t.run.as_ref().is_some_and(|r| !r.ok) {
            summary.run_failures += 1;
        }
    }

    summary.duration_ms = elapsed.as_millis() as u64;

    let invocation = InvocationInfo {
        argv: std::env::args().collect(),
        cwd: std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .display()
            .to_string(),
        started_at_unix_ms: 0,
        repeat: args.repeat,
        jobs: args.jobs,
        manifest_path: Some(args.manifest.display().to_string()),
        module_root: Some(display_path(module_root_used)),
        stdlib_lock: Some(args.stdlib_lock.display().to_string()),
    };

    X07TestReport {
        schema_version: X07TEST_SCHEMA_VERSION.to_string(),
        tool: ToolInfo {
            name: "x07".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            build: None,
        },
        invocation,
        summary,
        tests,
    }
}

impl X07TestReport {
    fn error_from_manifest(
        args: &TestArgs,
        elapsed: std::time::Duration,
        diags: Vec<ManifestDiag>,
    ) -> Self {
        let tests = Vec::new();
        let errors = if diags.is_empty() {
            1
        } else {
            diags.len() as u64
        };
        let summary = Summary {
            errors,
            duration_ms: elapsed.as_millis() as u64,
            ..Summary::default()
        };

        let invocation = InvocationInfo {
            argv: std::env::args().collect(),
            cwd: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .display()
                .to_string(),
            started_at_unix_ms: 0,
            repeat: args.repeat,
            jobs: args.jobs,
            manifest_path: Some(args.manifest.display().to_string()),
            module_root: args.module_root.first().map(display_path),
            stdlib_lock: Some(args.stdlib_lock.display().to_string()),
        };

        X07TestReport {
            schema_version: X07TEST_SCHEMA_VERSION.to_string(),
            tool: ToolInfo {
                name: "x07".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                build: None,
            },
            invocation,
            summary,
            tests,
        }
    }
}

fn compute_exit_code(args: &TestArgs, report: &X07TestReport) -> u8 {
    if report.summary.compile_failures > 0 {
        return 11;
    }
    if args.no_run {
        return 0;
    }
    if report.summary.run_failures > 0 {
        return 12;
    }
    if report
        .tests
        .iter()
        .any(|t| !matches_expectation_strs(&t.expect, &t.status))
    {
        return 10;
    }
    0
}

fn write_report_and_exit(
    machine: &reporting::MachineArgs,
    args: TestArgs,
    report: X07TestReport,
    exit_code: u8,
) -> Result<std::process::ExitCode> {
    let _ = args;

    let mut report_bytes = serde_json::to_vec(&report)?;
    if report_bytes.last() != Some(&b'\n') {
        report_bytes.push(b'\n');
    }

    if let Some(path) = machine.report_out.as_deref() {
        if path.as_os_str() == std::ffi::OsStr::new("-") {
            anyhow::bail!("--report-out '-' is not supported (stdout is reserved for the report)");
        }
        reporting::write_bytes(path, &report_bytes)?;
    }

    if machine.quiet_json {
        return Ok(std::process::ExitCode::from(exit_code));
    }

    if matches!(machine.json, Some(reporting::JsonArg::Off)) {
        println!(
            "summary: passed={} failed={} skipped={} errors={} duration_ms={} compile_failures={} run_failures={}",
            report.summary.passed,
            report.summary.failed,
            report.summary.skipped,
            report.summary.errors,
            report.summary.duration_ms,
            report.summary.compile_failures,
            report.summary.run_failures,
        );
    } else {
        std::io::Write::write_all(&mut std::io::stdout(), &report_bytes).context("write stdout")?;
    }

    Ok(std::process::ExitCode::from(exit_code))
}

fn display_path<P: AsRef<Path>>(p: P) -> String {
    p.as_ref().display().to_string()
}
