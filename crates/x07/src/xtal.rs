use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use base64::Engine as _;
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use walkdir::WalkDir;
use x07c::ast::Expr;
use x07c::diagnostics;
use x07c::x07ast;

use crate::report_common;
use crate::util;

const SPEC_SCHEMA_VERSION: &str = "x07.x07spec@0.1.0";
const SPEC_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07.x07spec@0.1.0.schema.json");

const EXAMPLES_SCHEMA_VERSION: &str = "x07.x07spec_examples@0.1.0";
const EXAMPLES_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.x07spec_examples@0.1.0.schema.json");

const TESTS_MANIFEST_SCHEMA_VERSION: &str = "x07.tests_manifest@0.2.0";
const DEFAULT_SPEC_DIR: &str = "spec";
const DEFAULT_GEN_DIR: &str = "gen/xtal";
const DEFAULT_MANIFEST_PATH: &str = "gen/xtal/tests.json";
const DEFAULT_VERIFY_DIR: &str = "target/xtal";
const DEFAULT_VERIFY_TEST_REPORT_PATH: &str = "target/xtal/tests.report.json";

static TMP_N: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Args)]
pub struct XtalArgs {
    #[command(subcommand)]
    pub cmd: Option<XtalCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum XtalCommand {
    /// Run Phase-A spec pipeline (fmt/lint/check).
    Dev(XtalDevArgs),
    /// Verify spec + generated tests + test execution.
    Verify(XtalVerifyArgs),
    /// Work with spec modules.
    Spec(XtalSpecArgs),
    /// Work with generated tests from spec examples.
    Tests(XtalTestsArgs),
}

#[derive(Debug, Args)]
pub struct XtalDevArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// XTAL phase to run (currently only `A` is supported).
    #[arg(long, value_name = "PHASE", default_value = "A")]
    pub phase: String,

    /// Spec directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct XtalVerifyArgs {
    /// Project manifest path (defaults to searching upwards for x07.json).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Spec directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_SPEC_DIR)]
    pub spec_dir: PathBuf,

    /// Generated output directory relative to the project root.
    #[arg(long, value_name = "DIR", default_value = DEFAULT_GEN_DIR)]
    pub gen_dir: PathBuf,

    /// Generated tests manifest path relative to the project root.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_MANIFEST_PATH)]
    pub manifest: PathBuf,
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
pub struct XtalSpecScaffoldArgs {
    #[arg(long, value_name = "MODULE_ID")]
    pub module_id: String,

    /// Operation local name (appended to module_id).
    #[arg(long, value_name = "NAME", default_value = "op_v1")]
    pub op: String,

    /// Operation parameter in `name:ty` form (repeatable).
    #[arg(long, value_name = "NAME:TY")]
    pub param: Vec<String>,

    /// Operation result type (Phase-A minimum: bytes, bytes_view, i32).
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
        XtalCommand::Spec(args) => cmd_xtal_spec(machine, args),
        XtalCommand::Tests(args) => cmd_xtal_tests(machine, args),
    }
}

fn cmd_xtal_dev(
    machine: &crate::reporting::MachineArgs,
    args: XtalDevArgs,
) -> Result<std::process::ExitCode> {
    if args.phase.trim() != "A" && args.phase.trim() != "a" {
        anyhow::bail!(
            "unsupported --phase {:?} (only \"A\" is supported)",
            args.phase
        );
    }

    let project_root = resolve_project_root(args.project.as_deref(), None)?;
    let spec_root = project_root.join(&args.spec_dir);

    let mut diagnostics = Vec::new();
    let spec_files = collect_spec_files(&spec_root, &Vec::new(), &mut diagnostics);
    let mut merged_spec_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut merged_examples_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut spec_fmt_ok = true;
    let mut spec_fmt_report: Option<Value> = None;
    let mut spec_lint_report: Option<Value> = None;
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
                let _ = code;
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                spec_lint_report = Some(v);
            }
            Err(err) => {
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
                let _ = code;
                merge_meta_digests(&v, "spec_digests", &mut merged_spec_digests);
                merge_meta_digests(&v, "examples_digests", &mut merged_examples_digests);
                diagnostics
                    .extend(crate::tool_api::extract_diagnostics(Some(&v)).unwrap_or_default());
                spec_check_report = Some(v);
            }
            Err(err) => {
                diagnostics.push(diag_error(
                    "X07-INTERNAL-0001",
                    diagnostics::Stage::Run,
                    format!("spec check capture failed: {err:#}"),
                    None,
                ));
            }
        }
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
        "spec_digests".to_string(),
        Value::Array(merged_spec_digests.into_values().collect()),
    );
    report.meta.insert(
        "examples_digests".to_string(),
        Value::Array(merged_examples_digests.into_values().collect()),
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
    let manifest_path = project_root.join(&args.manifest);

    let mut diagnostics = Vec::new();
    let spec_files = collect_spec_files(&spec_root, &Vec::new(), &mut diagnostics);
    let mut merged_spec_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut merged_examples_digests: BTreeMap<String, Value> = BTreeMap::new();
    let mut spec_fmt_ok = true;
    let mut spec_fmt_report: Option<Value> = None;

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
    let gen_args = XtalTestsGenArgs {
        project: Some(project_root.join("x07.json")),
        spec: Vec::new(),
        spec_dir: args.spec_dir.clone(),
        out_dir: args.gen_dir.clone(),
        check: true,
        write: false,
    };
    let (gen_code, gen_report) = match capture_report_json("xtal_verify_gen_from_spec", |m| {
        cmd_xtal_tests_gen_from_spec(m, gen_args)
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
                format!("tests gen-from-spec capture failed: {err:#}"),
                None,
            ));
            (std::process::ExitCode::from(1), None)
        }
    };

    // Run tests if prechecks succeeded.
    let mut tests_ok = false;
    if check_code == std::process::ExitCode::SUCCESS && gen_code == std::process::ExitCode::SUCCESS
    {
        std::fs::create_dir_all(project_root.join(DEFAULT_VERIFY_DIR)).with_context(|| {
            format!("mkdir: {}", project_root.join(DEFAULT_VERIFY_DIR).display())
        })?;
        let report_out = project_root.join(DEFAULT_VERIFY_TEST_REPORT_PATH);
        let test_run = run_self_command(
            &project_root,
            &[
                "test".to_string(),
                "--manifest".to_string(),
                manifest_path.display().to_string(),
                "--report-out".to_string(),
                report_out.display().to_string(),
                "--quiet-json".to_string(),
            ],
        )?;
        if test_run.exit_code == 0 {
            tests_ok = true;
        } else {
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
        Value::Array(merged_spec_digests.into_values().collect()),
    );
    report.meta.insert(
        "examples_digests".to_string(),
        Value::Array(merged_examples_digests.into_values().collect()),
    );
    report
        .meta
        .insert("tests_ok".to_string(), Value::Bool(tests_ok));
    if let Some(v) = spec_check_report {
        report.meta.insert("spec_check_report".to_string(), v);
    }
    if let Some(v) = gen_report {
        report.meta.insert("gen_from_spec_report".to_string(), v);
    }
    if let Some(v) = spec_fmt_report {
        report.meta.insert("spec_fmt_report".to_string(), v);
    }
    write_report(machine, &report)?;

    Ok(if report.ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    })
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

    for (_spec_path, spec) in modules {
        let module_id = spec.module_id.as_str();
        let gen_module_id = format!("gen.xtal.{module_id}.tests");
        let mut exports = BTreeSet::new();
        let mut functions = Vec::new();

        let mut global_idx = 0usize;

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

        let file = x07ast::X07AstFile {
            schema_version: x07_contracts::X07AST_SCHEMA_VERSION.to_string(),
            kind: x07ast::X07AstKind::Module,
            module_id: gen_module_id.clone(),
            imports: {
                let mut imps = BTreeSet::new();
                imps.insert("std.test".to_string());
                imps.insert(module_id.to_string());
                imps
            },
            exports,
            functions,
            async_functions: Vec::new(),
            extern_functions: Vec::new(),
            solve: None,
            meta: BTreeMap::new(),
        };

        let mut v = x07ast::x07ast_file_to_value(&file);
        x07ast::canon_value_jcs(&mut v);
        let mut bytes = serde_json::to_vec_pretty(&v)?;
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecFile {
    schema_version: String,
    module_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    doc: Option<String>,
    #[serde(default)]
    ids: Option<BTreeMap<String, String>>,
    #[serde(default)]
    sorts: Vec<SpecSort>,
    operations: Vec<SpecOperation>,
    #[serde(default)]
    assumptions: Vec<SpecAssumption>,
    #[serde(default)]
    nonfunctional: Option<SpecNonfunctional>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecSort {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    doc: Option<String>,
    #[serde(default)]
    invariant: Vec<SpecClause>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecOperation {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    doc: Option<String>,
    params: Vec<SpecParam>,
    result: String,
    #[serde(default)]
    result_brand: Option<String>,
    #[serde(default)]
    requires: Vec<SpecClause>,
    #[serde(default)]
    ensures: Vec<SpecClause>,
    #[serde(default)]
    invariant: Vec<SpecClause>,
    #[serde(default)]
    ensures_props: Vec<SpecProp>,
    #[serde(default)]
    examples_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecParam {
    name: String,
    ty: String,
    #[serde(default)]
    brand: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecClause {
    #[serde(default)]
    id: Option<String>,
    expr: Value,
    #[serde(default)]
    witness: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecProp {
    #[serde(default)]
    id: Option<String>,
    prop: String,
    args: Vec<String>,
    #[serde(default)]
    doc: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecAssumption {
    id: String,
    text: String,
    #[serde(default)]
    severity: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecNonfunctional {
    #[serde(default)]
    determinism: Option<String>,
    #[serde(default)]
    budget_profile: Option<String>,
    #[serde(default)]
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
