#![recursion_limit = "256"]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use anyhow::{Context, Result};
use base64::Engine;
use clap::{Args, Parser};
use sha2::{Digest, Sha256};
use x07_contracts::{
    PROJECT_LOCKFILE_SCHEMA_VERSION, X07AST_SCHEMA_VERSION, X07TEST_SCHEMA_VERSION,
};
use x07_host_runner::{run_artifact_file, RunnerConfig, RunnerResult};
use x07_worlds::WorldId;
use x07c::project;

mod arch;
mod ast;
mod bench;
mod bundle;
mod cli;
mod diag;
mod doc;
mod doctor;
mod guide;
mod init;
mod patch;
mod pkg;
mod policy;
mod policy_overrides;
mod repair;
mod report_common;
mod review;
mod rr;
mod run;
mod schema;
mod sm;
mod tool_api;
mod toolchain;
mod trust;
mod util;

#[derive(Parser, Debug)]
#[command(name = "x07")]
#[command(about = "X07 toolchain utilities.", long_about = None)]
#[command(version)]
#[command(subcommand_required = false)]
struct Cli {
    #[arg(long, global = true)]
    cli_specrows: bool,

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
    /// Format x07AST JSON files.
    Fmt(toolchain::FmtArgs),
    /// Lint x07AST JSON files.
    Lint(toolchain::LintArgs),
    /// Apply deterministic quickfixes to an x07AST JSON file.
    Fix(toolchain::FixArgs),
    /// Build a project to C.
    Build(toolchain::BuildArgs),
    /// Work with CLI specrows schemas and tooling.
    Cli(cli::CliArgs),
    /// Manage packages and lockfiles.
    Pkg(pkg::PkgArgs),
    /// Produce human review artifacts (semantic diffs).
    Review(review::ReviewArgs),
    /// Emit CI trust artifacts (budgets/caps, capabilities, nondeterminism, SBOM placeholders).
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

#[derive(Debug, Clone, Copy)]
enum TestReturns {
    ResultI32,
    BytesStatusV1,
}

#[derive(Debug, Clone)]
struct TestDecl {
    id: String,
    world: WorldId,
    entry: String,
    expect: Expect,
    returns: TestReturns,
    fixture_root: Option<PathBuf>,
    policy_json: Option<PathBuf>,
    timeout_ms: Option<u64>,
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

    #[arg(long, value_name = "SUBSTR")]
    filter: Option<String>,

    #[arg(long)]
    exact: bool,

    #[arg(long)]
    list: bool,

    #[arg(
        long,
        action = clap::ArgAction::Set,
        value_name = "BOOL",
        value_parser = clap::value_parser!(bool),
        default_value = "true"
    )]
    json: bool,

    #[arg(long, value_name = "PATH")]
    report_out: Option<PathBuf>,

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

    match try_main() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:#}");
            std::process::ExitCode::from(2)
        }
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
                Some(ast::AstCommand::ApplyPatch(_)) => vec!["ast", "apply-patch"],
                Some(ast::AstCommand::Validate(_)) => vec!["ast", "validate"],
                Some(ast::AstCommand::Canon(_)) => vec!["ast", "canon"],
                Some(ast::AstCommand::Schema(_)) => vec!["ast", "schema"],
                Some(ast::AstCommand::Grammar(_)) => vec!["ast", "grammar"],
            },
            Some(Command::Fmt(_)) => vec!["fmt"],
            Some(Command::Lint(_)) => vec!["lint"],
            Some(Command::Fix(_)) => vec!["fix"],
            Some(Command::Build(_)) => vec!["build"],
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
                Some(pkg::PkgCommand::Pack(_)) => vec!["pkg", "pack"],
                Some(pkg::PkgCommand::Lock(_)) => vec!["pkg", "lock"],
                Some(pkg::PkgCommand::Provides(_)) => vec!["pkg", "provides"],
                Some(pkg::PkgCommand::Login(_)) => vec!["pkg", "login"],
                Some(pkg::PkgCommand::Publish(_)) => vec!["pkg", "publish"],
            },
            Some(Command::Review(args)) => match &args.cmd {
                None => vec!["review"],
                Some(review::ReviewCommand::Diff(_)) => vec!["review", "diff"],
            },
            Some(Command::Trust(args)) => match &args.cmd {
                None => vec!["trust"],
                Some(trust::TrustCommand::Report(_)) => vec!["trust", "report"],
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
        Command::Init(args) => init::cmd_init(args),
        Command::Test(args) => cmd_test(args),
        Command::Bench(args) => bench::cmd_bench(args),
        Command::Arch(args) => arch::cmd_arch(args),
        Command::Run(args) => run::cmd_run(*args),
        Command::Bundle(args) => bundle::cmd_bundle(*args),
        Command::Guide(args) => guide::cmd_guide(args),
        Command::Doctor(args) => doctor::cmd_doctor(args),
        Command::Diag(args) => diag::cmd_diag(args),
        Command::Policy(args) => policy::cmd_policy(args),
        Command::Ast(args) => ast::cmd_ast(args),
        Command::Fmt(args) => toolchain::cmd_fmt(args),
        Command::Lint(args) => toolchain::cmd_lint(args),
        Command::Fix(args) => toolchain::cmd_fix(args),
        Command::Build(args) => toolchain::cmd_build(args),
        Command::Cli(args) => cli::cmd_cli(args),
        Command::Pkg(args) => pkg::cmd_pkg(args),
        Command::Review(args) => review::cmd_review(args),
        Command::Trust(args) => trust::cmd_trust(args),
        Command::Patch(args) => patch::cmd_patch(args),
        Command::Doc(args) => doc::cmd_doc(args),
        Command::Schema(args) => schema::cmd_schema(args),
        Command::Sm(args) => sm::cmd_sm(args),
        Command::Rr(args) => rr::cmd_rr(args),
    }
}

fn cmd_test(args: TestArgs) -> Result<std::process::ExitCode> {
    let started = Instant::now();

    let mut args = args;
    args.manifest = util::resolve_existing_path_upwards(&args.manifest);

    let validated = match validate_manifest_json(&args.manifest) {
        Ok(m) => m,
        Err(diags) => {
            for d in &diags {
                eprintln!("{}: {} ({})", d.code, d.message, d.path);
            }
            let report = X07TestReport::error_from_manifest(&args, started.elapsed(), diags);
            return write_report_and_exit(args, report, 12);
        }
    };

    let module_root_used = args
        .module_root
        .first()
        .cloned()
        .unwrap_or_else(|| validated.manifest_dir.clone());
    let module_roots = compute_test_module_roots(&args, &validated)?;

    let mut tests = validated.tests;
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
    tests.sort_by(|a, b| a.id.cmp(&b.id));

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

    let results = run_tests(&args, &module_roots, &tests)?;

    let report = finalize_report(&args, &module_root_used, started.elapsed(), results);

    let exit_code = compute_exit_code(&args, &report);
    write_report_and_exit(args, report, exit_code)
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

    let project_manifest =
        project::load_project_manifest(&project_path).context("load project manifest")?;
    let lock_path = project::default_lockfile_path(&project_path, &project_manifest);

    let lock: project::Lockfile = if lock_path.is_file() {
        let bytes = std::fs::read(&lock_path)
            .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?
    } else if project_manifest.dependencies.is_empty() {
        project::Lockfile {
            schema_version: PROJECT_LOCKFILE_SCHEMA_VERSION.to_string(),
            dependencies: Vec::new(),
        }
    } else {
        anyhow::bail!(
            "missing lockfile for project with dependencies: {}",
            lock_path.display()
        );
    };

    if lock.schema_version.trim() != PROJECT_LOCKFILE_SCHEMA_VERSION {
        anyhow::bail!(
            "lockfile schema_version mismatch: expected {} got {:?}",
            PROJECT_LOCKFILE_SCHEMA_VERSION,
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

fn run_tests(
    args: &TestArgs,
    module_roots: &[PathBuf],
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

            let result = run_one_test(args, module_roots, test)?;
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
                match run_one_test(args, module_roots, test) {
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
            entry: Some(test.entry.clone()),
            fixture_root: test.fixture_root.as_ref().map(display_path),
            compile: None,
            run: None,
            diags: Vec::new(),
        });
    }

    let driver_src = build_test_driver_x07ast_json(test)?;

    if !test.world.is_eval_world() {
        return run_one_test_os(args, module_roots, test, &driver_src, start);
    }

    let (driver_out_dir, driver_path, exe_out_path) = if args.keep_artifacts {
        let out_dir = args
            .artifact_dir
            .join("tests")
            .join(safe_artifact_dir_name(&test.id));
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

    let mut first_obs: Option<ObservedRun> = None;
    let mut last_run: Option<RunnerResult> = None;

    for rep in 0..args.repeat {
        let run_res = run_artifact_file(&runner_config, exe, &[])?;
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

    let status_bytes = run_res.solve_output.clone();
    let (tag, code_u32) = match parse_evtest_status_v1(&status_bytes) {
        Ok(x) => x,
        Err(msg) => {
            result.diags.push(Diag::new("EBAD_STATUS", msg.to_string()));
            result.run = Some(RunSection::from_runner_result(&run_res));
            result.status = "error".to_string();
            result.duration_ms = start.elapsed().as_millis() as u64;
            return Ok(result);
        }
    };

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

#[derive(Debug, serde::Deserialize)]
struct OsRunnerReportRaw {
    compile: OsRunnerCompileRaw,
    #[serde(default)]
    solve: Option<OsRunnerSolveRaw>,
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
}

static X07TEST_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
            .join(safe_artifact_dir_name(&test.id));
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
        if let Some(d) = &manifest_dir {
            candidates.push(d.clone());
        }
        if let Some(d) = &arch_root {
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
    cmd.arg("--program").arg(&driver_path);
    cmd.arg("--compiled-out").arg(&exe_out_path);
    cmd.arg("--auto-ffi");
    if test.world == WorldId::RunOsSandboxed {
        let policy_path = policy_path
            .as_deref()
            .context("internal error: missing resolved policy path")?;
        cmd.arg("--policy").arg(policy_path);
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

    result.compile = Some(CompileSection {
        ok: report.compile.ok,
        exit_code: Some(report.compile.exit_status),
        compiler_diags: Vec::new(),
        artifact_path: Some(display_path(&exe_out_path)),
        c_bytes: Some(report.compile.c_source_size),
    });

    if !report.compile.ok {
        if let Some(msg) = report.compile.compile_error {
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

    let Some(solve) = report.solve else {
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
    });

    if !solve.ok || solve.exit_status != 0 {
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

    let (tag, code_u32) = match parse_evtest_status_v1(&status_bytes) {
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

    if let Some(run) = result.run.as_mut() {
        run.failure_code_u32 = Some(code_u32 as u64);
    }

    result.status = compute_status(test.expect, tag);
    result.duration_ms = start.elapsed().as_millis() as u64;

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
        solve_fuel: 50_000_000,
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

fn parse_evtest_status_v1(status: &[u8]) -> Result<(u8, u32)> {
    if status.len() != 5 {
        anyhow::bail!("X7TEST_STATUS_V1 must be 5 bytes, got {}", status.len());
    }
    let tag = status[0];
    if !matches!(tag, 0..=2) {
        anyhow::bail!("X7TEST_STATUS_V1 tag must be 0, 1, or 2, got {}", tag);
    }
    let code = u32::from_le_bytes([status[1], status[2], status[3], status[4]]);
    Ok((tag, code))
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
    let solve = match test.returns {
        TestReturns::ResultI32 => {
            serde_json::json!(["std.test.status_from_result_i32", call_entry])
        }
        TestReturns::BytesStatusV1 => call_entry,
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
    expect: Option<String>,
    #[serde(default)]
    returns: Option<String>,
    #[serde(default)]
    fixture_root: Option<String>,
    #[serde(default)]
    policy_json: Option<String>,
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

    if raw.schema_version != "x07.tests_manifest@0.1.0" {
        diags.push(ManifestDiag {
            code: "ETEST_SCHEMA_VERSION",
            message: format!(
                "schema_version must be x07.tests_manifest@0.1.0, got {}",
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

        let returns = match parse_returns(t.returns.as_deref()) {
            Some(r) => r,
            None => {
                diags.push(ManifestDiag {
                    code: "ETEST_RETURNS_INVALID",
                    message: format!("invalid returns: {:?}", t.returns),
                    path: format!("{base}/returns"),
                });
                continue;
            }
        };

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

        out.push(TestDecl {
            id: t.id.clone(),
            world,
            entry: t.entry.clone(),
            expect,
            returns,
            fixture_root,
            policy_json,
            timeout_ms: t.timeout_ms,
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
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct Diag {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

impl Diag {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: None,
        }
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
    args: TestArgs,
    report: X07TestReport,
    exit_code: u8,
) -> Result<std::process::ExitCode> {
    let json = serde_json::to_string(&report)? + "\n";

    if let Some(out_path) = &args.report_out {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create report dir: {}", parent.display()))?;
        }
        std::fs::write(out_path, json.as_bytes())
            .with_context(|| format!("write report: {}", out_path.display()))?;
        eprintln!(
            "x07test: passed={} failed={} skipped={} errors={} (exit={})",
            report.summary.passed,
            report.summary.failed,
            report.summary.skipped,
            report.summary.errors,
            exit_code
        );
    }

    if args.json {
        print!("{json}");
    } else {
        for t in &report.tests {
            println!("{}\t{}", t.status, t.id);
        }
        println!(
            "summary: passed={} failed={} skipped={} errors={} xfail_passed={} xfail_failed={} (exit={})",
            report.summary.passed,
            report.summary.failed,
            report.summary.skipped,
            report.summary.errors,
            report.summary.xfail_passed,
            report.summary.xfail_failed,
            exit_code
        );
    }

    Ok(std::process::ExitCode::from(exit_code))
}

fn safe_artifact_dir_name(id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    let hex = util::hex_lower(&hasher.finalize());
    format!("id_{hex}")
}

fn display_path<P: AsRef<Path>>(p: P) -> String {
    p.as_ref().display().to_string()
}
