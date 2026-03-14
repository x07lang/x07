use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use base64::Engine;
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07_contracts::{
    X07_VERIFY_CEX_SCHEMA_VERSION, X07_VERIFY_COVERAGE_SCHEMA_VERSION,
    X07_VERIFY_PRIMITIVES_SCHEMA_VERSION, X07_VERIFY_REPORT_SCHEMA_VERSION,
};
use x07_worlds::WorldId;

use crate::report_common;
use crate::repro::ToolInfo;
use crate::util;

const X07_VERIFY_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.report.schema.json");
const X07_VERIFY_COVERAGE_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.coverage.schema.json");
const X07_VERIFY_CEX_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.verify.cex@0.1.0.schema.json");
const X07_VERIFY_PRIMITIVES_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.primitives.schema.json");
const X07_VERIFY_PRIMITIVES_CATALOG_BYTES: &[u8] =
    include_bytes!("../../../catalog/verify_primitives.json");
const X07DIAG_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07diag.schema.json");

const VERIFY_INPUT_BUF_NAME: &str = "x07_verify_input";
const VERIFY_HARNESS_FN: &str = "x07_verify_harness";
const Z3_TIMEOUT_SECONDS: u64 = 10;
const PROCESS_SUMMARY_MAX_CHARS: usize = 1024;

#[derive(Debug, Clone, Args)]
pub struct VerifyArgs {
    /// Bounded model checking via CBMC (compile-to-C + assertions).
    #[arg(long, conflicts_with_all = ["smt", "prove", "coverage"])]
    pub bmc: bool,

    /// Emit an SMT-LIB2 formula (via CBMC) and optionally solve with Z3.
    #[arg(long, conflicts_with_all = ["bmc", "prove", "coverage"])]
    pub smt: bool,

    /// Attempt an unbounded proof for a certifiable pure target via the SMT flow.
    #[arg(long, conflicts_with_all = ["bmc", "smt", "coverage"])]
    pub prove: bool,

    /// Emit a lightweight coverage summary for the requested entry target.
    #[arg(long, conflicts_with_all = ["bmc", "smt", "prove"])]
    pub coverage: bool,

    /// Fully qualified function name to verify (must include a '.' module separator).
    #[arg(long, value_name = "SYM")]
    pub entry: String,

    /// Project manifest path (`x07.json`) or directory containing it (used to resolve module roots).
    #[arg(long, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Module root directory for resolving module ids. May be passed multiple times.
    ///
    /// If not provided, `x07 verify` tries to infer roots from a project manifest; otherwise it
    /// defaults to the current directory.
    #[arg(long, value_name = "DIR")]
    pub module_root: Vec<PathBuf>,

    /// Loop unwinding bound for CBMC.
    #[arg(long, value_name = "N", default_value_t = 8)]
    pub unwind: u32,

    /// Maximum length bound used for `bytes` and `bytes_view` parameters (encoded into input).
    #[arg(long, value_name = "N", default_value_t = 16)]
    pub max_bytes_len: u32,

    /// Base directory for verification artifacts.
    ///
    /// Defaults to `<project_root>/.x07/artifacts` or `<cwd>/.x07/artifacts`.
    #[arg(long, value_name = "DIR")]
    pub artifact_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Bmc,
    Smt,
    Prove,
    Coverage,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Bmc => "bmc",
            Mode::Smt => "smt",
            Mode::Prove => "prove",
            Mode::Coverage => "coverage",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Bounds {
    unwind: u32,
    max_bytes_len: u32,
    input_len_bytes: u32,
}

impl Bounds {
    fn for_args(args: &VerifyArgs) -> Self {
        Bounds {
            unwind: args.unwind,
            max_bytes_len: args.max_bytes_len,
            input_len_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct VerifyResult {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
struct Artifacts {
    #[serde(skip_serializing_if = "Option::is_none")]
    driver_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    c_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cbmc_json_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cex_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    smt2_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    z3_out_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct VerifyCoverage {
    schema_version: &'static str,
    entry: String,
    worlds: Vec<String>,
    summary: VerifyCoverageSummary,
    functions: Vec<VerifyCoverageFunction>,
}

#[derive(Debug, Clone, Serialize)]
struct VerifyCoverageSummary {
    reachable_defn: u64,
    proven_defn: u64,
    trusted_primitives: u64,
    uncovered_defn: u64,
    unsupported_defn: u64,
}

#[derive(Debug, Clone, Serialize)]
struct VerifyCoverageFunction {
    symbol: String,
    kind: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyPrimitiveCatalog {
    schema_version: String,
    primitives: Vec<VerifyPrimitiveEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyPrimitiveEntry {
    symbol: String,
    kind: String,
    #[allow(dead_code)]
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct TrustedPrimitiveStub {
    symbol: String,
    params: Vec<String>,
    result: String,
}

#[derive(Debug, Clone)]
struct CoverageModule {
    alias_map: BTreeMap<String, String>,
    decls: BTreeMap<String, CoverageDecl>,
}

#[derive(Debug, Clone)]
struct CoverageDecl {
    kind: String,
    params: Vec<String>,
    has_contracts: bool,
    body: Option<Value>,
    contract_exprs: Vec<Value>,
    source_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct VerifyReport {
    schema_version: &'static str,
    mode: &'static str,
    ok: bool,
    entry: String,
    bounds: Bounds,
    result: VerifyResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    coverage: Option<VerifyCoverage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifacts: Option<Artifacts>,
    diagnostics_count: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<x07c::diagnostics::Diagnostic>,
    exit_code: u8,
}

#[derive(Debug, Clone, Serialize)]
struct VerifyCex {
    schema_version: String,
    tool: ToolInfo,
    entry: String,
    bounds: Bounds,
    input_bytes_b64: String,
    contract: Value,
    cbmc: CbmcInfo,
}

#[derive(Debug, Clone, Serialize)]
struct CbmcInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    argv: Vec<String>,
    exit_code: i32,
    stdout_json_path: String,
    stdout_json_sha256: String,
}

pub fn cmd_verify(
    machine: &crate::reporting::MachineArgs,
    args: VerifyArgs,
) -> Result<std::process::ExitCode> {
    let mode = selected_mode(&args).unwrap_or(Mode::Bmc);
    let bounds0 = Bounds::for_args(&args);
    let entry = args.entry.clone();

    if mode_count(&args) != 1 {
        let d = diag_verify(
            "X07V_EARGS",
            "set exactly one of --bmc, --smt, --prove, or --coverage",
        );
        return write_report_and_exit(machine, VerifyReport::error(mode, &entry, bounds0, d, 1));
    }

    match cmd_verify_inner(machine, args, mode) {
        Ok(code) => Ok(code),
        Err(err) => {
            let d = diag_verify("X07V_INTERNAL", format!("{err:#}"));
            write_report_and_exit(machine, VerifyReport::error(mode, &entry, bounds0, d, 1))
        }
    }
}

fn cmd_verify_inner(
    machine: &crate::reporting::MachineArgs,
    args: VerifyArgs,
    mode: Mode,
) -> Result<std::process::ExitCode> {
    let cwd = std::env::current_dir().context("get cwd")?;

    let project_path = match resolve_project_manifest(&cwd, args.project.as_deref()) {
        Ok(v) => v,
        Err(err) => {
            let d = diag_verify("X07V_EPROJECT", format!("{err:#}"));
            return write_report_and_exit(
                machine,
                VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
            );
        }
    };
    let project_root = project_path
        .as_deref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf);

    let module_roots = match resolve_module_roots(&cwd, project_path.as_deref(), &args.module_root)
    {
        Ok(v) => v,
        Err(err) => {
            let d = diag_verify("X07V_EMODULE_ROOTS", format!("{err:#}"));
            return write_report_and_exit(
                machine,
                VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
            );
        }
    };

    let target = match load_target_info(&module_roots, &args.entry) {
        Ok(v) => v,
        Err(err) => {
            let d = diag_verify("X07V_ETARGET", format!("{err:#}"));
            return write_report_and_exit(
                machine,
                VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
            );
        }
    };

    if mode == Mode::Coverage {
        return cmd_verify_coverage(machine, &args, project_path.as_deref(), &target);
    }

    if mode == Mode::Prove {
        if let Some((code, msg)) =
            prove_unsupported_reason(&target, &args.entry, args.max_bytes_len)
        {
            return write_report_and_exit(
                machine,
                VerifyReport::unsupported(mode, &args.entry, Bounds::for_args(&args), code, msg, 2),
            );
        }
    } else if let Some(d) = verify_precheck_diag(&target, &args.entry, args.max_bytes_len) {
        return write_report_and_exit(
            machine,
            VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
        );
    }

    let input_len_bytes = compute_input_len_bytes(&target, args.max_bytes_len).map_err(|err| {
        anyhow::anyhow!(
            "internal verify precheck mismatch for {:?}: {err}",
            args.entry
        )
    })?;
    let bounds = Bounds {
        unwind: args.unwind,
        max_bytes_len: args.max_bytes_len,
        input_len_bytes,
    };

    let artifact_base =
        resolve_artifact_base_dir(&cwd, project_root.as_deref(), args.artifact_dir.as_deref());

    let mut artifacts = Artifacts::default();

    let work_dir = match mode {
        Mode::Bmc => artifact_base
            .join("verify")
            .join("bmc")
            .join(util::safe_artifact_dir_name(&args.entry)),
        Mode::Smt => artifact_base
            .join("verify")
            .join("smt")
            .join(util::safe_artifact_dir_name(&args.entry)),
        Mode::Prove => artifact_base
            .join("verify")
            .join("prove")
            .join(util::safe_artifact_dir_name(&args.entry)),
        Mode::Coverage => unreachable!("coverage returns before artifact generation"),
    };
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("create artifact dir: {}", work_dir.display()))?;

    let driver_src = build_verify_driver_x07ast_json(&args.entry, &target, args.max_bytes_len)?;
    let driver_path = work_dir.join("driver.x07.json");
    util::write_atomic(&driver_path, &driver_src)
        .with_context(|| format!("write verify driver: {}", driver_path.display()))?;
    artifacts.driver_path = Some(driver_path.display().to_string());

    let trusted_primitive_stubs = if mode == Mode::Prove {
        trusted_primitive_stubs_for_prove(&args, project_path.as_deref(), &target, &module_roots)?
    } else {
        Vec::new()
    };

    let c_src = match compile_driver_to_c(&driver_src, &module_roots) {
        Ok(v) => v,
        Err(err) if mode == Mode::Prove => {
            return write_report_and_exit(
                machine,
                VerifyReport::unsupported(
                    mode,
                    &args.entry,
                    bounds,
                    "X07V_PROVE_UNSUPPORTED",
                    format!("target is outside the certifiable pure subset: {err}"),
                    2,
                )
                .with_artifacts(artifacts),
            );
        }
        Err(err) => return Err(err),
    };
    let c_src = if mode == Mode::Prove {
        apply_trusted_primitive_stubs(&c_src, &trusted_primitive_stubs)?
    } else {
        c_src
    };
    let c_with_harness = format!("{c_src}\n\n{}\n", build_c_harness(bounds.input_len_bytes));
    let c_path = work_dir.join("verify.c");
    util::write_atomic(&c_path, c_with_harness.as_bytes())
        .with_context(|| format!("write verify C: {}", c_path.display()))?;
    artifacts.c_path = Some(c_path.display().to_string());

    match mode {
        Mode::Bmc => cmd_verify_bmc(machine, &args, bounds, &work_dir, &c_path, artifacts),
        Mode::Smt | Mode::Prove => {
            cmd_verify_smt(machine, &args, bounds, &work_dir, &c_path, artifacts, mode)
        }
        Mode::Coverage => unreachable!("coverage returns before solver dispatch"),
    }
}

fn cmd_verify_bmc(
    machine: &crate::reporting::MachineArgs,
    args: &VerifyArgs,
    bounds: Bounds,
    work_dir: &Path,
    c_path: &Path,
    mut artifacts: Artifacts,
) -> Result<std::process::ExitCode> {
    if !command_exists("cbmc") {
        let msg = "cbmc is required for `x07 verify --bmc` (install: `brew install cbmc` or see https://diffblue.github.io/cbmc/)";
        let d = diag_verify("X07V_ECBMC_MISSING", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::tool_missing(Mode::Bmc, &args.entry, bounds, d, artifacts, 1),
        );
    }

    let mut cbmc_args = vec![
        c_path.display().to_string(),
        "--function".to_string(),
        VERIFY_HARNESS_FN.to_string(),
        "--unwind".to_string(),
        args.unwind.to_string(),
        "--unwinding-assertions".to_string(),
        "--trace".to_string(),
        "--json-ui".to_string(),
    ];
    maybe_disable_cbmc_standard_checks(&mut cbmc_args);

    let out = Command::new("cbmc")
        .args(&cbmc_args)
        .output()
        .context("run cbmc")?;

    if !out.stderr.is_empty() {
        // cbmc can print UI status to stdout (in json-ui mode), but unexpected stderr is a signal.
        let msg = summarize_process_text(&out.stderr, PROCESS_SUMMARY_MAX_CHARS);
        let d = diag_verify("X07V_ECBMC_STDERR", format!("cbmc wrote to stderr: {msg}"));
        return write_report_and_exit(
            machine,
            VerifyReport::error(Mode::Bmc, &args.entry, bounds, d, 1).with_artifacts(artifacts),
        );
    }

    let cbmc_json: Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(err) => {
            let msg = format!("failed to parse cbmc --json-ui output: {err}");
            let d = diag_verify("X07V_ECBMC_JSON_PARSE", msg);
            return write_report_and_exit(
                machine,
                VerifyReport::error(Mode::Bmc, &args.entry, bounds, d, 1).with_artifacts(artifacts),
            );
        }
    };

    let cbmc_json_path = work_dir.join("cbmc.json");
    let cbmc_json_bytes =
        report_common::canonical_pretty_json_bytes(&cbmc_json).context("canon cbmc.json")?;
    util::write_atomic(&cbmc_json_path, cbmc_json_bytes.as_slice())
        .with_context(|| format!("write cbmc output: {}", cbmc_json_path.display()))?;
    artifacts.cbmc_json_path = Some(cbmc_json_path.display().to_string());

    let cbmc_errors = cbmc_messages_of_type(&cbmc_json, "ERROR");
    if !cbmc_errors.is_empty() {
        let msg = cbmc_errors.join("; ");
        let d = diag_verify("X07V_ECBMC_ERROR", format!("cbmc reported an error: {msg}"));
        return write_report_and_exit(
            machine,
            VerifyReport::error(Mode::Bmc, &args.entry, bounds, d, 1).with_artifacts(artifacts),
        );
    }

    let cbmc_version = cbmc_program_version(&cbmc_json);
    let failures = cbmc_failures(&cbmc_json);
    if failures.is_empty() {
        return write_report_and_exit(
            machine,
            VerifyReport::verified(Mode::Bmc, &args.entry, bounds, artifacts),
        );
    }

    if failures.iter().all(is_unwind_failure) {
        let msg =
            "cbmc reported an unwinding assertion failure (increase --unwind for a complete bound)";
        let d = diag_verify("X07V_UNWIND_INCOMPLETE", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::inconclusive(Mode::Bmc, &args.entry, bounds, d, artifacts, 2),
        );
    }

    let Some(contract_failure) = failures.iter().find_map(parse_contract_failure) else {
        let msg = "cbmc reported a failing property that is not an X07 contract assertion";
        let d = diag_verify("X07V_ECBMC_FAILURE", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::error(Mode::Bmc, &args.entry, bounds, d, 1).with_artifacts(artifacts),
        );
    };

    let trace = contract_failure
        .trace
        .as_ref()
        .and_then(Value::as_array)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let input_bytes = extract_input_bytes_from_trace(
        trace,
        VERIFY_INPUT_BUF_NAME,
        bounds.input_len_bytes as usize,
    );

    let cex = VerifyCex {
        schema_version: X07_VERIFY_CEX_SCHEMA_VERSION.to_string(),
        tool: crate::repro::tool_info(),
        entry: args.entry.clone(),
        bounds: bounds.clone(),
        input_bytes_b64: base64::engine::general_purpose::STANDARD.encode(&input_bytes),
        contract: contract_failure.payload.clone(),
        cbmc: CbmcInfo {
            version: cbmc_version,
            argv: std::iter::once("cbmc".to_string())
                .chain(cbmc_args)
                .collect(),
            exit_code: out.status.code().unwrap_or(-1),
            stdout_json_path: "cbmc.json".to_string(),
            stdout_json_sha256: util::sha256_hex(cbmc_json_bytes.as_slice()),
        },
    };

    let cex_path = work_dir.join("cex.json");
    let cex_bytes = verify_cex_to_pretty_canon_bytes(&cex)?;
    util::write_atomic(&cex_path, &cex_bytes)
        .with_context(|| format!("write verify cex: {}", cex_path.display()))?;
    artifacts.cex_path = Some(cex_path.display().to_string());

    let report = VerifyReport {
        schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
        mode: Mode::Bmc.as_str(),
        ok: false,
        entry: args.entry.clone(),
        bounds,
        result: VerifyResult {
            kind: "counterexample_found".to_string(),
            contract: Some(contract_failure.payload),
            details: None,
        },
        coverage: None,
        artifacts: Some(artifacts),
        diagnostics_count: 0,
        diagnostics: Vec::new(),
        exit_code: 10,
    };
    write_report_and_exit(machine, report)
}

fn cmd_verify_smt(
    machine: &crate::reporting::MachineArgs,
    args: &VerifyArgs,
    bounds: Bounds,
    work_dir: &Path,
    c_path: &Path,
    mut artifacts: Artifacts,
    mode: Mode,
) -> Result<std::process::ExitCode> {
    if !command_exists("cbmc") {
        let msg = format!(
            "cbmc is required for `x07 verify --{}` (install: `brew install cbmc` or see https://diffblue.github.io/cbmc/)",
            mode.as_str()
        );
        let d = diag_verify("X07V_ECBMC_MISSING", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::tool_missing(mode, &args.entry, bounds, d, artifacts, 1),
        );
    }

    let smt2_path = work_dir.join("verify.smt2");

    let mut cbmc_args = vec![
        c_path.display().to_string(),
        "--function".to_string(),
        VERIFY_HARNESS_FN.to_string(),
        "--unwind".to_string(),
        args.unwind.to_string(),
        "--unwinding-assertions".to_string(),
        "--smt2".to_string(),
        "--outfile".to_string(),
        smt2_path.display().to_string(),
    ];
    maybe_disable_cbmc_standard_checks(&mut cbmc_args);

    let out = Command::new("cbmc")
        .args(&cbmc_args)
        .output()
        .context("run cbmc (smt2 emit)")?;

    if !out.status.success() {
        let msg = summarize_process_failure(&out.stdout, &out.stderr, PROCESS_SUMMARY_MAX_CHARS);
        let diag_msg = format!("cbmc failed to emit SMT2: {msg}");
        let d = diag_verify("X07V_ECBMC_SMT2", diag_msg);
        return write_report_and_exit(
            machine,
            VerifyReport::error(mode, &args.entry, bounds, d, 1).with_artifacts(artifacts),
        );
    }

    if !out.stderr.is_empty() {
        let msg = summarize_process_text(&out.stderr, PROCESS_SUMMARY_MAX_CHARS);
        let d = diag_verify("X07V_ECBMC_STDERR", format!("cbmc wrote to stderr: {msg}"));
        return write_report_and_exit(
            machine,
            VerifyReport::error(mode, &args.entry, bounds, d, 1).with_artifacts(artifacts),
        );
    }

    normalize_smt2_logic_for_z3(&smt2_path)?;
    artifacts.smt2_path = Some(smt2_path.display().to_string());

    if !command_exists("z3") {
        let msg = "z3 is not installed (SMT2 was emitted; install: `brew install z3` or https://github.com/Z3Prover/z3)";
        let d = diag_verify("X07V_EZ3_MISSING", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::inconclusive(mode, &args.entry, bounds, d, artifacts, 2),
        );
    }

    let z3_out = Command::new("z3")
        .arg(format!("-T:{Z3_TIMEOUT_SECONDS}"))
        .arg("-smt2")
        .arg(&smt2_path)
        .output()
        .context("run z3")?;

    if !z3_out.status.success() {
        let msg = summarize_process_text(&z3_out.stderr, PROCESS_SUMMARY_MAX_CHARS);
        let d = diag_verify("X07V_EZ3_RUN", format!("z3 failed: {msg}"));
        return write_report_and_exit(
            machine,
            VerifyReport::error(mode, &args.entry, bounds, d, 1).with_artifacts(artifacts),
        );
    }

    let z3_stdout = String::from_utf8_lossy(&z3_out.stdout).to_string();
    let z3_out_path = work_dir.join("z3.out.txt");
    util::write_atomic(&z3_out_path, z3_stdout.as_bytes())
        .with_context(|| format!("write z3 output: {}", z3_out_path.display()))?;
    artifacts.z3_out_path = Some(z3_out_path.display().to_string());

    let status = z3_stdout.lines().next().unwrap_or("").trim();
    match status {
        "unsat" => write_report_and_exit(
            machine,
            if mode == Mode::Prove {
                VerifyReport::proven(&args.entry, bounds, artifacts)
            } else {
                VerifyReport::verified(mode, &args.entry, bounds, artifacts)
            },
        ),
        "sat" => write_report_and_exit(
            machine,
            VerifyReport::counterexample_found(
                mode,
                &args.entry,
                bounds,
                diag_verify("X07V_SMT_SAT", "solver reported SAT (counterexample found)"),
                artifacts,
                10,
            ),
        ),
        other => write_report_and_exit(
            machine,
            VerifyReport::inconclusive(
                mode,
                &args.entry,
                bounds,
                diag_verify("X07V_SMT_UNKNOWN", format!("solver returned {other:?}")),
                artifacts,
                2,
            ),
        ),
    }
}

fn normalize_smt2_logic_for_z3(path: &Path) -> Result<()> {
    let raw = std::fs::read(path).with_context(|| format!("read smt2: {}", path.display()))?;
    let text = String::from_utf8(raw).context("SMT2 output is not valid UTF-8")?;
    let has_quantifiers = text.contains("(forall") || text.contains("(exists");

    let mut changed = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("(get-") {
            changed = true;
            continue;
        }
        if has_quantifiers && !changed && trimmed.starts_with("(set-logic QF_") {
            let prefix_len = line.len() - trimmed.len();
            let indent = &line[..prefix_len];
            let rest = trimmed.trim_start_matches("(set-logic QF_");
            lines.push(format!("{indent}(set-logic {rest}"));
            changed = true;
            continue;
        }
        lines.push(line.to_string());
    }

    if !changed {
        return Ok(());
    }

    let mut normalized = lines.join("\n");
    if text.ends_with('\n') {
        normalized.push('\n');
    }
    util::write_atomic(path, normalized.as_bytes())
        .with_context(|| format!("rewrite smt2 logic: {}", path.display()))
}

fn resolve_project_manifest(cwd: &Path, explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(p) = explicit {
        let p = util::resolve_existing_path_upwards(p);
        if p.is_dir() {
            let cand = p.join("x07.json");
            if cand.is_file() {
                return Ok(Some(cand));
            }
            anyhow::bail!("--project dir does not contain x07.json: {}", p.display());
        }
        if p.is_file() {
            return Ok(Some(p));
        }
        anyhow::bail!("--project path not found: {}", p.display());
    }

    let found = util::resolve_existing_path_upwards_from(cwd, Path::new("x07.json"));
    if found.is_file() {
        return Ok(Some(found));
    }
    Ok(None)
}

fn resolve_module_roots(
    cwd: &Path,
    project_path: Option<&Path>,
    explicit: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    if !explicit.is_empty() {
        return Ok(explicit.to_vec());
    }

    if let Some(project_path) = project_path {
        let manifest =
            x07c::project::load_project_manifest(project_path).context("load project manifest")?;
        let lock_path = x07c::project::default_lockfile_path(project_path, &manifest);
        let lock_bytes = std::fs::read(&lock_path)
            .with_context(|| format!("read lockfile: {}", lock_path.display()))?;
        let lock: x07c::project::Lockfile = serde_json::from_slice(&lock_bytes)
            .with_context(|| format!("parse lockfile JSON: {}", lock_path.display()))?;
        x07c::project::verify_lockfile(project_path, &manifest, &lock)
            .context("verify lockfile")?;
        let mut roots = x07c::project::collect_module_roots(project_path, &manifest, &lock)
            .context("collect module roots")?;

        if let Some(project_root) = project_path.parent() {
            if !roots.contains(&project_root.to_path_buf()) {
                roots.push(project_root.to_path_buf());
            }
        }
        if !roots.contains(&cwd.to_path_buf()) {
            roots.push(cwd.to_path_buf());
        }
        return Ok(roots);
    }

    Ok(vec![cwd.to_path_buf()])
}

fn resolve_artifact_base_dir(
    cwd: &Path,
    project_root: Option<&Path>,
    explicit: Option<&Path>,
) -> PathBuf {
    if let Some(p) = explicit {
        if p.is_absolute() {
            return p.to_path_buf();
        }
        return cwd.join(p);
    }
    let base = project_root.unwrap_or(cwd);
    base.join(".x07").join("artifacts")
}

#[derive(Debug, Clone)]
struct TargetSig {
    params: Vec<String>,
    result: String,
    is_async: bool,
    has_contracts: bool,
    body: Value,
    source_path: PathBuf,
}

fn load_target_info(module_roots: &[PathBuf], entry: &str) -> Result<TargetSig> {
    let (module_id, _) = entry.rsplit_once('.').context("--entry must contain '.'")?;
    let source =
        x07c::module_source::load_module_source(module_id, WorldId::SolvePure, module_roots)
            .map_err(|err| anyhow::anyhow!(err.message.to_string()))?;
    let doc: Value = serde_json::from_str(&source.src)
        .with_context(|| format!("parse module JSON for {module_id:?}"))?;

    let decls = doc
        .get("decls")
        .and_then(Value::as_array)
        .context("module missing decls[]")?;
    for d in decls {
        let kind = d.get("kind").and_then(Value::as_str).unwrap_or("");
        if kind != "defn" && kind != "defasync" {
            continue;
        }
        let name = d.get("name").and_then(Value::as_str).unwrap_or("");
        if name != entry {
            continue;
        }
        let params = d
            .get("params")
            .and_then(Value::as_array)
            .context("defn missing params[]")?;
        let mut out = Vec::with_capacity(params.len());
        for p in params {
            let ty = p
                .get("ty")
                .and_then(Value::as_str)
                .context("param missing ty")?;
            out.push(ty.to_string());
        }
        let has_contracts = has_any_contracts(d);
        let body = d.get("body").cloned().context("defn missing body")?;
        return Ok(TargetSig {
            params: out,
            result: d
                .get("result")
                .and_then(Value::as_str)
                .context("defn missing result")?
                .to_string(),
            is_async: kind == "defasync",
            has_contracts,
            body,
            source_path: source
                .path
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("{module_id}.x07.json"))),
        });
    }

    anyhow::bail!("could not find function {entry:?} in resolved module {module_id:?}")
}

fn load_coverage_module<'a>(
    module_roots: &[PathBuf],
    world: WorldId,
    module_id: &str,
    cache: &'a mut BTreeMap<String, CoverageModule>,
) -> Result<&'a CoverageModule> {
    if !cache.contains_key(module_id) {
        let source = x07c::module_source::load_module_source(module_id, world, module_roots)
            .map_err(|err| anyhow::anyhow!(err.message.to_string()))?;
        let doc: Value = serde_json::from_str(&source.src)
            .with_context(|| format!("parse module JSON for {module_id:?}"))?;

        let imports = doc
            .get("imports")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut alias_map = BTreeMap::new();
        let local_alias = module_id.rsplit('.').next().unwrap_or(module_id);
        alias_map.insert(local_alias.to_string(), module_id.to_string());
        for import in imports {
            let Some(import) = import.as_str() else {
                continue;
            };
            let alias = import.rsplit('.').next().unwrap_or(import);
            alias_map
                .entry(alias.to_string())
                .or_insert_with(|| import.to_string());
        }

        let decls = doc
            .get("decls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut out_decls = BTreeMap::new();
        for decl in decls {
            let Some(kind) = decl.get("kind").and_then(Value::as_str) else {
                continue;
            };
            if kind != "defn" && kind != "defasync" && kind != "extern" {
                continue;
            }
            let Some(name) = decl.get("name").and_then(Value::as_str) else {
                continue;
            };
            let params = decl
                .get("params")
                .and_then(Value::as_array)
                .map(|params| {
                    params
                        .iter()
                        .filter_map(|p| p.get("ty").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            out_decls.insert(
                name.to_string(),
                CoverageDecl {
                    kind: kind.to_string(),
                    params,
                    has_contracts: has_any_contracts(&decl),
                    body: decl.get("body").cloned(),
                    contract_exprs: collect_contract_exprs(&decl),
                    source_path: source
                        .path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from(format!("{module_id}.x07.json"))),
                },
            );
        }

        cache.insert(
            module_id.to_string(),
            CoverageModule {
                alias_map,
                decls: out_decls,
            },
        );
    }

    Ok(cache.get(module_id).expect("coverage module inserted"))
}

fn load_verify_primitive_catalog() -> Result<BTreeMap<String, String>> {
    let doc: Value = serde_json::from_slice(X07_VERIFY_PRIMITIVES_CATALOG_BYTES)
        .context("parse catalog/verify_primitives.json")?;
    let diags = report_common::validate_schema(
        X07_VERIFY_PRIMITIVES_SCHEMA_BYTES,
        "spec/x07-verify.primitives.schema.json",
        &doc,
    )?;
    if !diags.is_empty() {
        anyhow::bail!(
            "verify primitives catalog is not schema-valid: {}",
            diags[0].message
        );
    }
    let catalog: VerifyPrimitiveCatalog =
        serde_json::from_value(doc).context("decode verify primitives catalog")?;
    if catalog.schema_version.trim() != X07_VERIFY_PRIMITIVES_SCHEMA_VERSION {
        anyhow::bail!(
            "verify primitives schema_version mismatch: expected {:?} got {:?}",
            X07_VERIFY_PRIMITIVES_SCHEMA_VERSION,
            catalog.schema_version
        );
    }

    let mut out = BTreeMap::new();
    for primitive in catalog.primitives {
        out.insert(primitive.symbol, primitive.kind);
    }
    Ok(out)
}

fn enqueue_decl_refs(
    module_id: &str,
    module: &CoverageModule,
    decl: &CoverageDecl,
    queue: &mut VecDeque<String>,
) {
    if let Some(body) = decl.body.as_ref() {
        collect_decl_refs(module_id, module, body, queue);
    }
    for expr in &decl.contract_exprs {
        collect_decl_refs(module_id, module, expr, queue);
    }
}

fn collect_decl_refs(
    module_id: &str,
    module: &CoverageModule,
    value: &Value,
    queue: &mut VecDeque<String>,
) {
    match value {
        Value::Array(items) => {
            if let Some(head) = items.first().and_then(Value::as_str) {
                if head == "tapp" {
                    if let Some(callee) = items.get(1).and_then(Value::as_str) {
                        if let Some(resolved) =
                            resolve_ref_symbol(module_id, &module.alias_map, callee)
                        {
                            queue.push_back(resolved);
                        }
                    }
                } else {
                    if let Some(resolved) = resolve_ref_symbol(module_id, &module.alias_map, head) {
                        queue.push_back(resolved);
                    }
                    if head.ends_with(".fn_v1") {
                        if let Some(callee) = items.get(1).and_then(Value::as_str) {
                            if let Some(resolved) =
                                resolve_ref_symbol(module_id, &module.alias_map, callee)
                            {
                                queue.push_back(resolved);
                            }
                        }
                    }
                }
            }
            for item in items {
                collect_decl_refs(module_id, module, item, queue);
            }
        }
        Value::Object(obj) => {
            for child in obj.values() {
                collect_decl_refs(module_id, module, child, queue);
            }
        }
        _ => {}
    }
}

fn resolve_ref_symbol(
    module_id: &str,
    alias_map: &BTreeMap<String, String>,
    raw: &str,
) -> Option<String> {
    let (prefix, suffix) = raw.rsplit_once('.')?;
    if prefix == module_id || prefix.contains('.') {
        return Some(raw.to_string());
    }
    alias_map
        .get(prefix)
        .map(|target| format!("{target}.{suffix}"))
        .or_else(|| Some(raw.to_string()))
}

fn collect_contract_exprs(defn: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for key in ["requires", "ensures", "invariant"] {
        let Some(clauses) = defn.get(key).and_then(Value::as_array) else {
            continue;
        };
        for clause in clauses {
            if let Some(expr) = clause.get("expr") {
                out.push(expr.clone());
            }
            if let Some(witness) = clause.get("witness").and_then(Value::as_array) {
                out.extend(witness.iter().cloned());
            }
        }
    }
    out
}

fn compute_input_len_bytes(sig: &TargetSig, max_bytes_len: u32) -> Result<u32> {
    let mut total: u64 = 0;
    for ty in &sig.params {
        match ty.as_str() {
            "i32" | "u32" => total = total.saturating_add(4),
            "bytes" | "bytes_view" => total = total.saturating_add(4 + max_bytes_len as u64),
            other => anyhow::bail!(
                "unsupported verify param type {other:?} (supported: i32,u32,bytes,bytes_view)"
            ),
        }
    }
    if total > u32::MAX as u64 {
        anyhow::bail!("verify input encoding too large: {total} bytes");
    }
    Ok(total as u32)
}

fn prove_unsupported_reason(
    target: &TargetSig,
    entry: &str,
    max_bytes_len: u32,
) -> Option<(&'static str, String)> {
    if target.is_async {
        return Some((
            "X07V_UNSUPPORTED_ASYNC",
            "x07 verify --prove does not support defasync targets (use a defn wrapper)".to_string(),
        ));
    }
    if !target.has_contracts {
        return Some((
            "X07V_NO_CONTRACTS",
            "target function has no requires/ensures/invariant clauses".to_string(),
        ));
    }
    if contains_direct_recursion(&target.body, entry) {
        return Some((
            "X07V_UNSUPPORTED_RECURSION",
            "x07 verify v0.1 does not support recursive targets".to_string(),
        ));
    }
    if let Some(msg) = find_for_with_non_literal_bounds(&target.body) {
        return Some(("X07V_UNSUPPORTED_FOR_BOUNDS", msg));
    }
    if let Err(err) = compute_input_len_bytes(target, max_bytes_len) {
        return Some(("X07V_UNSUPPORTED_PARAM", err.to_string()));
    }
    None
}

fn verify_precheck_diag(
    target: &TargetSig,
    entry: &str,
    max_bytes_len: u32,
) -> Option<x07c::diagnostics::Diagnostic> {
    if target.is_async {
        return Some(diag_verify(
            "X07V_UNSUPPORTED_ASYNC",
            "x07 verify does not support defasync targets (use a defn wrapper)",
        ));
    }
    let (code, msg) = prove_unsupported_reason(target, entry, max_bytes_len)?;
    Some(diag_verify(code, msg))
}

fn cmd_verify_coverage(
    machine: &crate::reporting::MachineArgs,
    args: &VerifyArgs,
    project_path: Option<&Path>,
    target: &TargetSig,
) -> Result<std::process::ExitCode> {
    let coverage = match coverage_report_for_entry(args, project_path, target) {
        Ok(coverage) => coverage,
        Err(err) => coverage_report_fallback(
            &args.entry,
            project_path,
            target,
            args.max_bytes_len,
            Some(format!("could not materialize reachable closure: {err:#}")),
        ),
    };
    write_report_and_exit(
        machine,
        VerifyReport::coverage_report(&args.entry, Bounds::for_args(args), coverage),
    )
}

fn coverage_worlds(project_path: Option<&Path>) -> Vec<String> {
    let Some(project_path) = project_path else {
        return vec![WorldId::SolvePure.as_str().to_string()];
    };
    match x07c::project::load_project_manifest(project_path) {
        Ok(manifest) if !manifest.world.trim().is_empty() => vec![manifest.world],
        _ => vec![WorldId::SolvePure.as_str().to_string()],
    }
}

fn coverage_world(project_path: Option<&Path>) -> WorldId {
    coverage_worlds(project_path)
        .first()
        .and_then(|world| WorldId::parse(world))
        .unwrap_or(WorldId::SolvePure)
}

fn coverage_function_for_target(
    entry: &str,
    target: &TargetSig,
    max_bytes_len: u32,
) -> VerifyCoverageFunction {
    let (kind, status, details) = if target.is_async {
        (
            "defasync".to_string(),
            "runtime_only".to_string(),
            Some(
                "defasync targets are runtime-only and are not certifiable by x07 verify"
                    .to_string(),
            ),
        )
    } else if !target.has_contracts {
        (
            "defn".to_string(),
            "uncovered".to_string(),
            Some("target function has no requires/ensures/invariant clauses".to_string()),
        )
    } else if let Some((_, msg)) = prove_unsupported_reason(target, entry, max_bytes_len) {
        ("defn".to_string(), "unsupported".to_string(), Some(msg))
    } else {
        ("defn".to_string(), "proven".to_string(), None)
    };

    VerifyCoverageFunction {
        symbol: entry.to_string(),
        kind,
        status,
        source_path: Some(target.source_path.display().to_string()),
        details,
    }
}

fn coverage_report_for_entry(
    args: &VerifyArgs,
    project_path: Option<&Path>,
    _target: &TargetSig,
) -> Result<VerifyCoverage> {
    let cwd = std::env::current_dir().context("get cwd")?;
    let module_roots = resolve_module_roots(&cwd, project_path, &args.module_root)?;
    let world = coverage_world(project_path);
    let primitive_catalog = load_verify_primitive_catalog()?;
    let mut module_cache: BTreeMap<String, CoverageModule> = BTreeMap::new();
    let mut queue = VecDeque::from([args.entry.clone()]);
    let mut visited = BTreeSet::new();
    let mut functions = BTreeMap::new();

    while let Some(symbol) = queue.pop_front() {
        if !visited.insert(symbol.clone()) {
            continue;
        }

        if let Some(kind) = primitive_catalog.get(&symbol) {
            functions.insert(
                symbol.clone(),
                VerifyCoverageFunction {
                    symbol,
                    kind: kind.clone(),
                    status: "trusted_primitive".to_string(),
                    source_path: None,
                    details: None,
                },
            );
            continue;
        }

        let Some((module_id, _)) = symbol.rsplit_once('.') else {
            functions.insert(
                symbol.clone(),
                VerifyCoverageFunction {
                    symbol,
                    kind: "imported".to_string(),
                    status: "unsupported".to_string(),
                    source_path: None,
                    details: Some("symbol is not fully qualified".to_string()),
                },
            );
            continue;
        };

        let module = match load_coverage_module(&module_roots, world, module_id, &mut module_cache)
        {
            Ok(module) => module,
            Err(_err) if is_builtin_like_symbol(&symbol) => {
                functions.insert(
                    symbol.clone(),
                    VerifyCoverageFunction {
                        symbol,
                        kind: "builtin".to_string(),
                        status: "trusted_primitive".to_string(),
                        source_path: None,
                        details: None,
                    },
                );
                continue;
            }
            Err(err) => {
                functions.insert(
                    symbol.clone(),
                    VerifyCoverageFunction {
                        symbol,
                        kind: "imported".to_string(),
                        status: "unsupported".to_string(),
                        source_path: None,
                        details: Some(err.to_string()),
                    },
                );
                continue;
            }
        };
        let Some(decl) = module.decls.get(&symbol) else {
            if is_builtin_like_symbol(&symbol) {
                functions.insert(
                    symbol.clone(),
                    VerifyCoverageFunction {
                        symbol,
                        kind: "builtin".to_string(),
                        status: "trusted_primitive".to_string(),
                        source_path: None,
                        details: None,
                    },
                );
                continue;
            }
            functions.insert(
                symbol.clone(),
                VerifyCoverageFunction {
                    symbol,
                    kind: "imported".to_string(),
                    status: "unsupported".to_string(),
                    source_path: None,
                    details: Some(
                        "referenced symbol could not be resolved in the loaded module graph"
                            .to_string(),
                    ),
                },
            );
            continue;
        };

        functions.insert(
            symbol.clone(),
            coverage_function_for_decl(&symbol, decl, args.max_bytes_len),
        );
        enqueue_decl_refs(module_id, module, decl, &mut queue);
    }

    let functions = functions.into_values().collect::<Vec<_>>();
    Ok(VerifyCoverage {
        schema_version: X07_VERIFY_COVERAGE_SCHEMA_VERSION,
        entry: args.entry.clone(),
        worlds: coverage_worlds(project_path),
        summary: summarize_coverage_functions(&functions),
        functions,
    })
}

fn coverage_report_fallback(
    entry: &str,
    project_path: Option<&Path>,
    target: &TargetSig,
    max_bytes_len: u32,
    extra_details: Option<String>,
) -> VerifyCoverage {
    let mut function = coverage_function_for_target(entry, target, max_bytes_len);
    match extra_details {
        Some(details) if function.status == "proven" => {
            function.status = "unsupported".to_string();
            function.details = Some(details);
        }
        Some(details) if function.details.is_none() => {
            function.details = Some(details);
        }
        _ => {}
    }
    let functions = vec![function];
    VerifyCoverage {
        schema_version: X07_VERIFY_COVERAGE_SCHEMA_VERSION,
        entry: entry.to_string(),
        worlds: coverage_worlds(project_path),
        summary: summarize_coverage_functions(&functions),
        functions,
    }
}

fn coverage_function_for_decl(
    symbol: &str,
    decl: &CoverageDecl,
    max_bytes_len: u32,
) -> VerifyCoverageFunction {
    let source_path = Some(decl.source_path.display().to_string());
    match decl.kind.as_str() {
        "defasync" => VerifyCoverageFunction {
            symbol: symbol.to_string(),
            kind: "defasync".to_string(),
            status: "runtime_only".to_string(),
            source_path,
            details: Some(
                "defasync targets are runtime-only and are not certifiable by x07 verify"
                    .to_string(),
            ),
        },
        "extern" => VerifyCoverageFunction {
            symbol: symbol.to_string(),
            kind: "extern".to_string(),
            status: "unsupported".to_string(),
            source_path,
            details: Some(
                "extern declarations are outside the certifiable pure subset".to_string(),
            ),
        },
        _ => {
            let body = decl.body.clone().unwrap_or(Value::Null);
            let target = TargetSig {
                params: decl.params.clone(),
                result: "i32".to_string(),
                is_async: false,
                has_contracts: decl.has_contracts,
                body,
                source_path: decl.source_path.clone(),
            };
            coverage_function_for_target(symbol, &target, max_bytes_len)
        }
    }
}

fn summarize_coverage_functions(functions: &[VerifyCoverageFunction]) -> VerifyCoverageSummary {
    VerifyCoverageSummary {
        reachable_defn: functions.iter().filter(|f| f.kind == "defn").count() as u64,
        proven_defn: functions
            .iter()
            .filter(|f| f.kind == "defn" && f.status == "proven")
            .count() as u64,
        trusted_primitives: functions
            .iter()
            .filter(|f| f.status == "trusted_primitive")
            .count() as u64,
        uncovered_defn: functions
            .iter()
            .filter(|f| f.kind == "defn" && f.status == "uncovered")
            .count() as u64,
        unsupported_defn: functions
            .iter()
            .filter(|f| f.kind == "defn" && f.status == "unsupported")
            .count() as u64,
    }
}

fn is_builtin_like_symbol(symbol: &str) -> bool {
    symbol
        .rsplit_once('.')
        .is_some_and(|(prefix, _)| !prefix.contains('.'))
}

fn build_verify_driver_x07ast_json(
    entry: &str,
    sig: &TargetSig,
    max_bytes_len: u32,
) -> Result<Vec<u8>> {
    let (module_id, _) = entry.rsplit_once('.').context("--entry must contain '.'")?;

    let max_plus_1: u64 = max_bytes_len as u64 + 1;
    if max_plus_1 > i64::MAX as u64 {
        anyhow::bail!("max_bytes_len too large");
    }

    let mut stmts: Vec<Value> = Vec::new();
    let mut call_args: Vec<Value> = Vec::new();

    stmts.push(serde_json::json!(["let", "off", 0]));

    for (idx, ty) in sig.params.iter().enumerate() {
        let off = Value::String("off".to_string());
        match ty.as_str() {
            "i32" | "u32" => {
                let name = format!("p{idx}");
                stmts.push(serde_json::json!([
                    "let",
                    name,
                    ["std.codec.read_u32_le", "input", off]
                ]));
                stmts.push(serde_json::json!(["set", "off", ["+", "off", 4]]));
                call_args.push(Value::String(format!("p{idx}")));
            }
            "bytes" | "bytes_view" => {
                let n_raw = format!("p{idx}_len_raw");
                let n = format!("p{idx}_len");
                let data_off = serde_json::json!(["+", "off", 4]);
                let raw_len = serde_json::json!(["std.codec.read_u32_le", "input", off]);
                stmts.push(serde_json::json!(["let", n_raw, raw_len]));
                stmts.push(serde_json::json!([
                    "let",
                    n,
                    [
                        "if",
                        ["<u", n_raw, max_plus_1 as i64],
                        n_raw,
                        max_bytes_len as i64
                    ]
                ]));

                let slice = serde_json::json!(["view.slice", "input", data_off, n.clone()]);
                if ty == "bytes" {
                    let bname = format!("p{idx}_bytes");
                    stmts.push(serde_json::json!(["let", bname, ["view.to_bytes", slice]]));
                    call_args.push(Value::String(bname));
                } else {
                    let vname = format!("p{idx}_view");
                    stmts.push(serde_json::json!(["let", vname, slice]));
                    call_args.push(Value::String(vname));
                }

                let step = 4u64 + max_bytes_len as u64;
                stmts.push(serde_json::json!(["set", "off", ["+", "off", step as i64]]));
            }
            other => anyhow::bail!("unsupported verify param type {other:?}"),
        }
    }

    let mut call_items = Vec::with_capacity(1 + call_args.len());
    call_items.push(Value::String(entry.to_string()));
    call_items.extend(call_args);

    stmts.push(serde_json::json!(["let", "_", Value::Array(call_items)]));
    stmts.push(serde_json::json!(["bytes.empty"]));

    let mut solve_items = Vec::with_capacity(1 + stmts.len());
    solve_items.push(Value::String("begin".to_string()));
    solve_items.extend(stmts);
    let solve = Value::Array(solve_items);

    let mut imports = vec![module_id.to_string(), "std.codec".to_string()];
    imports.sort();
    imports.dedup();

    let file = serde_json::json!({
        "schema_version": x07_contracts::X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": imports,
        "decls": [],
        "solve": solve,
    });

    let mut out = serde_json::to_vec_pretty(&file).context("encode verify driver JSON")?;
    out.push(b'\n');
    Ok(out)
}

fn compile_driver_to_c(driver_src: &[u8], module_roots: &[PathBuf]) -> Result<String> {
    let mut opts =
        x07c::world_config::compile_options_for_world(WorldId::SolvePure, module_roots.to_vec());
    opts.emit_main = false;
    opts.freestanding = true;
    opts.contract_mode = x07c::compile::ContractMode::VerifyBmc;
    let out = x07c::compile::compile_program_to_c_with_meta(driver_src, &opts)
        .map_err(|err| anyhow::anyhow!("{:?}: {}", err.kind, err.message))?;
    Ok(out.c_src)
}

fn trusted_primitive_stubs_for_prove(
    args: &VerifyArgs,
    project_path: Option<&Path>,
    target: &TargetSig,
    module_roots: &[PathBuf],
) -> Result<Vec<TrustedPrimitiveStub>> {
    let coverage = coverage_report_for_entry(args, project_path, target)?;
    let mut out = Vec::new();
    for function in coverage.functions {
        if function.kind != "imported" || function.status != "trusted_primitive" {
            continue;
        }
        let sig = load_target_info(module_roots, &function.symbol)?;
        out.push(TrustedPrimitiveStub {
            symbol: function.symbol,
            params: sig.params,
            result: sig.result,
        });
    }
    Ok(out)
}

fn c_user_fn_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    out.push_str("user_");
    for ch in name.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => out.push(ch),
            '.' => out.push('_'),
            _ => out.push('_'),
        }
    }
    out
}

fn trusted_primitive_stub_body(stub: &TrustedPrimitiveStub) -> Result<String> {
    let mut lines = Vec::new();
    lines.push("  (void)ctx;".to_string());
    lines.push("  (void)input;".to_string());
    for (idx, _) in stub.params.iter().enumerate() {
        lines.push(format!("  (void)p{idx};"));
    }
    match stub.result.as_str() {
        "i32" | "u32" => {
            lines.push("  return UINT32_C(0);".to_string());
        }
        "bytes" => {
            lines.push("  return (bytes_t){ .ptr = NULL, .len = UINT32_C(0) };".to_string());
        }
        "bytes_view" => {
            lines.push("  return (bytes_view_t){ .ptr = NULL, .len = UINT32_C(0) };".to_string());
        }
        "option_i32" => {
            lines.push("  option_i32_t out;".to_string());
            lines.push("  memset(&out, 0, sizeof(out));".to_string());
            lines.push("  return out;".to_string());
        }
        "option_bytes" => {
            lines.push("  option_bytes_t out;".to_string());
            lines.push("  memset(&out, 0, sizeof(out));".to_string());
            lines.push("  return out;".to_string());
        }
        "option_bytes_view" => {
            lines.push("  option_bytes_view_t out;".to_string());
            lines.push("  memset(&out, 0, sizeof(out));".to_string());
            lines.push("  return out;".to_string());
        }
        "result_i32" => {
            lines.push("  result_i32_t out;".to_string());
            lines.push("  memset(&out, 0, sizeof(out));".to_string());
            lines.push("  return out;".to_string());
        }
        "result_bytes" => {
            lines.push("  result_bytes_t out;".to_string());
            lines.push("  memset(&out, 0, sizeof(out));".to_string());
            lines.push("  return out;".to_string());
        }
        "result_bytes_view" => {
            lines.push("  result_bytes_view_t out;".to_string());
            lines.push("  memset(&out, 0, sizeof(out));".to_string());
            lines.push("  return out;".to_string());
        }
        other => {
            anyhow::bail!(
                "trusted primitive prove stub does not support result type {other:?} for {:?}",
                stub.symbol
            );
        }
    }
    Ok(lines.join("\n"))
}

fn find_matching_delimiter(text: &str, open_idx: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for (idx, byte) in bytes.iter().enumerate().skip(open_idx) {
        if *byte == open {
            depth += 1;
        } else if *byte == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(idx);
            }
        }
    }
    None
}

fn find_c_function_definition(text: &str, c_name: &str) -> Result<Option<(usize, usize)>> {
    let needle = format!("{c_name}(");
    let mut search_from = 0usize;
    while let Some(rel) = text[search_from..].find(&needle) {
        let name_idx = search_from + rel;
        let open_paren_idx = name_idx + c_name.len();
        let close_paren_idx = match find_matching_delimiter(text, open_paren_idx, b'(', b')') {
            Some(idx) => idx,
            None => {
                anyhow::bail!("could not match parameter list for generated C function {c_name}");
            }
        };
        let mut cursor = close_paren_idx + 1;
        while cursor < text.len() && text.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= text.len() {
            break;
        }
        match text.as_bytes()[cursor] {
            b';' => {
                search_from = cursor + 1;
                continue;
            }
            b'{' => {
                let close_brace_idx = find_matching_delimiter(text, cursor, b'{', b'}')
                    .with_context(|| {
                        format!("could not match body for generated C function {c_name}")
                    })?;
                return Ok(Some((cursor, close_brace_idx)));
            }
            _ => {
                search_from = cursor + 1;
            }
        }
    }
    Ok(None)
}

fn apply_trusted_primitive_stubs(c_src: &str, stubs: &[TrustedPrimitiveStub]) -> Result<String> {
    if stubs.is_empty() {
        return Ok(c_src.to_string());
    }

    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for stub in stubs {
        let c_name = c_user_fn_name(&stub.symbol);
        let Some((open_brace_idx, close_brace_idx)) = find_c_function_definition(c_src, &c_name)?
        else {
            anyhow::bail!(
                "could not locate generated C body for trusted primitive {:?}",
                stub.symbol
            );
        };
        let body = trusted_primitive_stub_body(stub)?;
        replacements.push((open_brace_idx, close_brace_idx, format!("{{\n{body}\n}}")));
    }

    replacements.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = c_src.to_string();
    for (start, end, replacement) in replacements {
        out.replace_range(start..=end, &replacement);
    }
    Ok(out)
}

fn build_c_harness(input_len: u32) -> String {
    let mut out = String::new();
    out.push_str("static unsigned char x07_nondet_u8(void) {\n");
    out.push_str("  unsigned char value;\n");
    out.push_str("  return value;\n");
    out.push_str("}\n");
    out.push_str(&format!("static void {VERIFY_HARNESS_FN}(void) {{\n"));
    out.push_str("  uint8_t arena_mem[65536];\n");
    out.push_str("  ctx_t ctx;\n");
    out.push_str("  memset(&ctx, 0, sizeof(ctx));\n");
    out.push_str("  ctx.fuel_init = (uint64_t)(X07_FUEL_INIT);\n");
    out.push_str("  ctx.fuel = ctx.fuel_init;\n");
    out.push_str("  ctx.heap.mem = arena_mem;\n");
    out.push_str("  ctx.heap.cap = (uint32_t)sizeof(arena_mem);\n");
    out.push_str("  rt_heap_init(&ctx);\n");
    out.push_str("  rt_allocator_init(&ctx);\n");
    out.push_str("  rt_ext_ctx = &ctx;\n");
    out.push_str("  rt_kv_init(&ctx);\n");
    let buf_cap = std::cmp::max(1u32, input_len);
    out.push_str(&format!("  uint8_t {VERIFY_INPUT_BUF_NAME}[{buf_cap}];\n"));
    for i in 0..input_len {
        out.push_str(&format!(
            "  {VERIFY_INPUT_BUF_NAME}[{i}] = x07_nondet_u8();\n"
        ));
    }
    out.push_str(&format!(
        "  bytes_view_t input = (bytes_view_t){{ .ptr = {VERIFY_INPUT_BUF_NAME}, .len = UINT32_C({input_len}) }};\n"
    ));
    out.push_str("  bytes_t out = solve(&ctx, input);\n");
    out.push_str("  rt_bytes_drop(&ctx, &out);\n");
    out.push_str("  rt_ext_ctx = NULL;\n");
    out.push_str("}\n");
    out
}

fn cbmc_program_version(doc: &Value) -> Option<String> {
    let arr = doc.as_array()?;
    for item in arr {
        if let Some(p) = item.get("program").and_then(Value::as_str) {
            return Some(p.to_string());
        }
    }
    None
}

fn cbmc_messages_of_type(doc: &Value, message_type: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(arr) = doc.as_array() else {
        return out;
    };
    for item in arr {
        if item.get("messageType").and_then(Value::as_str) != Some(message_type) {
            continue;
        }
        let msg = item
            .get("messageText")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if msg.is_empty() {
            continue;
        }
        if let Some(loc) = item.get("sourceLocation") {
            let file = loc.get("file").and_then(Value::as_str).unwrap_or("").trim();
            let line = loc.get("line").and_then(Value::as_str).unwrap_or("").trim();
            if !file.is_empty() && !line.is_empty() {
                out.push(format!("{msg} ({file}:{line})"));
                continue;
            }
        }
        out.push(msg.to_string());
    }
    out
}

fn cbmc_failures(doc: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    let Some(arr) = doc.as_array() else {
        return out;
    };
    for item in arr {
        let Some(results) = item.get("result").and_then(Value::as_array) else {
            continue;
        };
        for r in results {
            if r.get("status").and_then(Value::as_str) == Some("FAILURE") {
                out.push(r.clone());
            }
        }
    }
    out
}

fn has_any_contracts(defn: &Value) -> bool {
    for k in ["requires", "ensures", "invariant"] {
        if defn
            .get(k)
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty())
        {
            return true;
        }
    }
    false
}

fn contains_direct_recursion(expr: &Value, entry: &str) -> bool {
    match expr {
        Value::Array(items) => {
            if matches!(items.first(), Some(Value::String(head)) if head == entry) {
                return true;
            }
            for item in items {
                if contains_direct_recursion(item, entry) {
                    return true;
                }
            }
            false
        }
        Value::Object(map) => map.values().any(|v| contains_direct_recursion(v, entry)),
        _ => false,
    }
}

fn find_for_with_non_literal_bounds(expr: &Value) -> Option<String> {
    match expr {
        Value::Array(items) => {
            if matches!(items.first(), Some(Value::String(head)) if head == "for") {
                if items.len() != 5 {
                    return Some("unsupported `for` form in target body".to_string());
                }
                let start = &items[2];
                let end = &items[3];
                if start.as_i64().is_none() || end.as_i64().is_none() {
                    return Some(
                        "x07 verify v0.1 requires `for` bounds to be integer literals".to_string(),
                    );
                }
            }
            for item in items {
                if let Some(msg) = find_for_with_non_literal_bounds(item) {
                    return Some(msg);
                }
            }
            None
        }
        Value::Object(map) => {
            for v in map.values() {
                if let Some(msg) = find_for_with_non_literal_bounds(v) {
                    return Some(msg);
                }
            }
            None
        }
        _ => None,
    }
}

fn is_unwind_failure(result: &Value) -> bool {
    let prop = result.get("property").and_then(Value::as_str).unwrap_or("");
    let desc = result
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("");
    prop.contains(".unwind.") || desc.starts_with("unwinding assertion")
}

#[derive(Debug, Clone)]
struct ContractFailure {
    payload: Value,
    trace: Option<Value>,
}

fn parse_contract_failure(result: &Value) -> Option<ContractFailure> {
    let desc = result.get("description").and_then(Value::as_str)?;
    let info = crate::contract_repro::try_parse_contract_trap(desc).ok()??;
    Some(ContractFailure {
        payload: info.payload,
        trace: result.get("trace").cloned(),
    })
}

fn extract_input_bytes_from_trace(trace: &[Value], buf_name: &str, len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    for step in trace {
        if step.get("stepType").and_then(Value::as_str) != Some("assignment") {
            continue;
        }
        if step.get("hidden").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let lhs = step.get("lhs").and_then(Value::as_str).unwrap_or("");
        let Some(idx) = parse_array_lhs_index(lhs, buf_name) else {
            continue;
        };
        if idx >= len {
            continue;
        }
        let Some(data) = step
            .get("value")
            .and_then(|v| v.get("data"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if let Some(n) = parse_c_integer_data(data) {
            out[idx] = (n & 0xFF) as u8;
        }
    }
    out
}

fn parse_array_lhs_index(lhs: &str, base: &str) -> Option<usize> {
    let rest = lhs.strip_prefix(base)?;
    let rest = rest.strip_prefix('[')?;
    let rest = rest.strip_suffix(']')?;
    let digits = rest.trim().trim_end_matches(['l', 'u', 'U', 'L']);
    digits.parse::<usize>().ok()
}

fn parse_c_integer_data(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        let hex = s[2..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect::<String>();
        return u64::from_str_radix(&hex, 16).ok();
    }
    let dec = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect::<String>();
    dec.parse::<i64>().ok().map(|v| v as u64)
}

fn verify_cex_to_pretty_canon_bytes(cex: &VerifyCex) -> Result<Vec<u8>> {
    let v = serde_json::to_value(cex).context("serialize verify cex JSON")?;
    let diags = report_common::validate_schema(
        X07_VERIFY_CEX_SCHEMA_BYTES,
        "spec/x07.verify.cex@0.1.0.schema.json",
        &v,
    )?;
    if !diags.is_empty() {
        anyhow::bail!(
            "internal error: verify cex JSON is not schema-valid: {}",
            diags[0].message
        );
    }
    report_common::canonical_pretty_json_bytes(&v).context("canon verify cex JSON")
}

impl VerifyReport {
    fn verified(mode: Mode, entry: &str, bounds: Bounds, artifacts: Artifacts) -> Self {
        Self {
            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
            mode: mode.as_str(),
            ok: true,
            entry: entry.to_string(),
            bounds,
            result: VerifyResult {
                kind: "verified_within_bounds".to_string(),
                contract: None,
                details: None,
            },
            coverage: None,
            artifacts: Some(artifacts),
            diagnostics_count: 0,
            diagnostics: Vec::new(),
            exit_code: 0,
        }
    }

    fn proven(entry: &str, bounds: Bounds, artifacts: Artifacts) -> Self {
        Self {
            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
            mode: Mode::Prove.as_str(),
            ok: true,
            entry: entry.to_string(),
            bounds,
            result: VerifyResult {
                kind: "proven".to_string(),
                contract: None,
                details: None,
            },
            coverage: None,
            artifacts: Some(artifacts),
            diagnostics_count: 0,
            diagnostics: Vec::new(),
            exit_code: 0,
        }
    }

    fn counterexample_found(
        mode: Mode,
        entry: &str,
        bounds: Bounds,
        d: x07c::diagnostics::Diagnostic,
        artifacts: Artifacts,
        exit_code: u8,
    ) -> Self {
        Self {
            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
            mode: mode.as_str(),
            ok: false,
            entry: entry.to_string(),
            bounds,
            result: VerifyResult {
                kind: "counterexample_found".to_string(),
                contract: None,
                details: None,
            },
            coverage: None,
            artifacts: Some(artifacts),
            diagnostics_count: 1,
            diagnostics: vec![d],
            exit_code,
        }
    }

    fn inconclusive(
        mode: Mode,
        entry: &str,
        bounds: Bounds,
        d: x07c::diagnostics::Diagnostic,
        artifacts: Artifacts,
        exit_code: u8,
    ) -> Self {
        Self {
            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
            mode: mode.as_str(),
            ok: false,
            entry: entry.to_string(),
            bounds,
            result: VerifyResult {
                kind: "inconclusive".to_string(),
                contract: None,
                details: None,
            },
            coverage: None,
            artifacts: Some(artifacts),
            diagnostics_count: 1,
            diagnostics: vec![d],
            exit_code,
        }
    }

    fn tool_missing(
        mode: Mode,
        entry: &str,
        bounds: Bounds,
        d: x07c::diagnostics::Diagnostic,
        artifacts: Artifacts,
        exit_code: u8,
    ) -> Self {
        Self {
            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
            mode: mode.as_str(),
            ok: false,
            entry: entry.to_string(),
            bounds,
            result: VerifyResult {
                kind: "tool_missing".to_string(),
                contract: None,
                details: None,
            },
            coverage: None,
            artifacts: Some(artifacts),
            diagnostics_count: 1,
            diagnostics: vec![d],
            exit_code,
        }
    }

    fn unsupported(
        mode: Mode,
        entry: &str,
        bounds: Bounds,
        code: &'static str,
        details: String,
        exit_code: u8,
    ) -> Self {
        let d = diag_verify(code, details.clone());
        Self {
            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
            mode: mode.as_str(),
            ok: false,
            entry: entry.to_string(),
            bounds,
            result: VerifyResult {
                kind: "unsupported".to_string(),
                contract: None,
                details: Some(details),
            },
            coverage: None,
            artifacts: None,
            diagnostics_count: 1,
            diagnostics: vec![d],
            exit_code,
        }
    }

    fn coverage_report(entry: &str, bounds: Bounds, coverage: VerifyCoverage) -> Self {
        Self {
            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
            mode: Mode::Coverage.as_str(),
            ok: true,
            entry: entry.to_string(),
            bounds,
            result: VerifyResult {
                kind: "coverage_report".to_string(),
                contract: None,
                details: None,
            },
            coverage: Some(coverage),
            artifacts: None,
            diagnostics_count: 0,
            diagnostics: Vec::new(),
            exit_code: 0,
        }
    }

    fn error(
        mode: Mode,
        entry: &str,
        bounds: Bounds,
        d: x07c::diagnostics::Diagnostic,
        exit_code: u8,
    ) -> Self {
        Self {
            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
            mode: mode.as_str(),
            ok: false,
            entry: entry.to_string(),
            bounds,
            result: VerifyResult {
                kind: "error".to_string(),
                contract: None,
                details: None,
            },
            coverage: None,
            artifacts: None,
            diagnostics_count: 1,
            diagnostics: vec![d],
            exit_code,
        }
    }

    fn with_artifacts(mut self, artifacts: Artifacts) -> Self {
        self.artifacts = Some(artifacts);
        self
    }
}

fn write_report_and_exit(
    machine: &crate::reporting::MachineArgs,
    report: VerifyReport,
) -> Result<std::process::ExitCode> {
    let v = serde_json::to_value(&report).context("serialize verify report JSON")?;
    let diags = validate_verify_report_schema(&v)?;
    if !diags.is_empty() {
        anyhow::bail!(
            "internal error: verify report JSON is not schema-valid: {}",
            diags[0].message
        );
    }
    let bytes = report_common::canonical_pretty_json_bytes(&v)?;
    if let Some(path) = machine.report_out.as_deref() {
        crate::reporting::write_bytes(path, &bytes)?;
    }
    if machine.quiet_json {
        return Ok(std::process::ExitCode::from(report.exit_code));
    }

    if matches!(machine.json, Some(crate::reporting::JsonArg::Off)) {
        println!(
            "verify: mode={} entry={} kind={} exit_code={}",
            report.mode, report.entry, report.result.kind, report.exit_code
        );
    } else {
        std::io::Write::write_all(&mut std::io::stdout(), &bytes).context("write stdout")?;
    }

    Ok(std::process::ExitCode::from(report.exit_code))
}

fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn maybe_disable_cbmc_standard_checks(cbmc_args: &mut Vec<String>) {
    if command_supports_option("cbmc", "--help", "--no-standard-checks") {
        cbmc_args.push("--no-standard-checks".to_string());
    }
}

fn command_supports_option(command: &str, help_flag: &str, option: &str) -> bool {
    let Ok(out) = Command::new(command).arg(help_flag).output() else {
        return false;
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    stdout.contains(option) || stderr.contains(option)
}

fn summarize_process_failure(stdout: &[u8], stderr: &[u8], max_chars: usize) -> String {
    let stderr_text = summarize_process_text(stderr, max_chars);
    let stdout_text = summarize_process_text(stdout, max_chars);
    match (
        stderr_text.as_str() != "no output",
        stdout_text.as_str() != "no output",
    ) {
        (true, true) => format!("stderr: {stderr_text}; stdout: {stdout_text}"),
        (true, false) => stderr_text,
        (false, true) => stdout_text,
        (false, false) => "no output".to_string(),
    }
}

fn summarize_process_text(bytes: &[u8], max_chars: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "no output".to_string();
    }

    let truncated: String = trimmed.chars().take(max_chars).collect();
    if trimmed.chars().count() > max_chars {
        format!("{truncated}... [truncated]")
    } else {
        truncated
    }
}

fn validate_verify_report_schema(value: &Value) -> Result<Vec<x07c::diagnostics::Diagnostic>> {
    let schema_json: Value = serde_json::from_slice(X07_VERIFY_REPORT_SCHEMA_BYTES)
        .context("parse spec/x07-verify.report.schema.json")?;
    let x07diag_schema_json: Value =
        serde_json::from_slice(X07DIAG_SCHEMA_BYTES).context("parse spec/x07diag.schema.json")?;
    let coverage_schema_json: Value = serde_json::from_slice(X07_VERIFY_COVERAGE_SCHEMA_BYTES)
        .context("parse spec/x07-verify.coverage.schema.json")?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_resource(
            "x07diag.schema.json",
            jsonschema::Resource::from_contents(x07diag_schema_json.clone()),
        )
        .with_resource(
            "https://x07.io/spec/x07diag.schema.json",
            jsonschema::Resource::from_contents(x07diag_schema_json),
        )
        .with_resource(
            "x07-verify.coverage.schema.json",
            jsonschema::Resource::from_contents(coverage_schema_json.clone()),
        )
        .with_resource(
            "https://x07.io/spec/x07-verify.coverage.schema.json",
            jsonschema::Resource::from_contents(coverage_schema_json),
        )
        .build(&schema_json)
        .context("build spec/x07-verify.report.schema.json validator")?;

    let mut out = Vec::new();
    for error in validator.iter_errors(value) {
        let mut data = std::collections::BTreeMap::new();
        data.insert(
            "schema_path".to_string(),
            Value::String(error.schema_path().to_string()),
        );
        out.push(x07c::diagnostics::Diagnostic {
            code: "X07-SCHEMA-0001".to_string(),
            severity: x07c::diagnostics::Severity::Error,
            stage: x07c::diagnostics::Stage::Parse,
            message: error.to_string(),
            loc: Some(x07c::diagnostics::Location::X07Ast {
                ptr: error.instance_path().to_string(),
            }),
            notes: Vec::new(),
            related: Vec::new(),
            data,
            quickfix: None,
        });
    }
    Ok(out)
}

fn mode_count(args: &VerifyArgs) -> usize {
    [args.bmc, args.smt, args.prove, args.coverage]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
}

fn selected_mode(args: &VerifyArgs) -> Option<Mode> {
    if args.bmc {
        Some(Mode::Bmc)
    } else if args.smt {
        Some(Mode::Smt)
    } else if args.prove {
        Some(Mode::Prove)
    } else if args.coverage {
        Some(Mode::Coverage)
    } else {
        None
    }
}

fn diag_verify(code: &str, message: impl Into<String>) -> x07c::diagnostics::Diagnostic {
    x07c::diagnostics::Diagnostic {
        code: code.to_string(),
        severity: x07c::diagnostics::Severity::Error,
        stage: x07c::diagnostics::Stage::Run,
        message: message.into(),
        loc: None,
        notes: Vec::new(),
        related: Vec::new(),
        data: std::collections::BTreeMap::new(),
        quickfix: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write as _;

    fn write_fake_command(script_body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "x07_verify_fake_command_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("fake-cbmc.sh");
        let mut file = std::fs::File::create(&path).expect("create fake command");
        writeln!(file, "#!/bin/sh").expect("write shebang");
        writeln!(file, "{script_body}").expect("write script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("chmod");
        }
        path
    }

    #[test]
    fn command_supports_option_detects_help_output() {
        let fake = write_fake_command(
            r#"
if [ "$1" = "--help" ]; then
  printf '%s\n' 'cbmc help --no-standard-checks'
  exit 0
fi
exit 0
"#,
        );
        assert!(command_supports_option(
            fake.to_str().expect("utf-8 fake path"),
            "--help",
            "--no-standard-checks"
        ));
        std::fs::remove_file(&fake).expect("remove fake command");
        std::fs::remove_dir(fake.parent().expect("fake parent")).expect("remove temp dir");
    }

    #[test]
    fn command_supports_option_returns_false_when_help_lacks_option() {
        let fake = write_fake_command(
            r#"
if [ "$1" = "--help" ]; then
  printf '%s\n' 'cbmc help'
  exit 0
fi
exit 0
"#,
        );
        assert!(!command_supports_option(
            fake.to_str().expect("utf-8 fake path"),
            "--help",
            "--no-standard-checks"
        ));
        std::fs::remove_file(&fake).expect("remove fake command");
        std::fs::remove_dir(fake.parent().expect("fake parent")).expect("remove temp dir");
    }

    #[test]
    fn summarize_process_failure_prefers_stderr_and_truncates_streams() {
        let stdout = format!("{}\n{}", "x".repeat(1500), "tail");
        let stderr = "Usage error!\nUnknown option: --no-standard-checks\n";
        let summary = summarize_process_failure(stdout.as_bytes(), stderr.as_bytes(), 32);
        assert!(
            summary.starts_with("stderr: Usage error!\nUnknown option"),
            "summary={summary}"
        );
        assert!(summary.contains("stdout: "), "summary={summary}");
        assert!(summary.contains("[truncated]"), "summary={summary}");
    }

    #[test]
    fn verify_driver_imports_std_codec_and_uses_std_codec_read() {
        let sig = TargetSig {
            params: vec!["i32".to_string()],
            result: "bytes".to_string(),
            is_async: false,
            has_contracts: true,
            body: json!(0),
            source_path: PathBuf::from("verify_fixture.x07.json"),
        };
        let bytes =
            build_verify_driver_x07ast_json("verify_fixture.f", &sig, 16).expect("build driver");
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains("std.codec.read_u32_le"),
            "expected driver to use std.codec.read_u32_le, got:\n{text}"
        );

        let v: Value = serde_json::from_slice(&bytes).expect("parse driver json");
        assert_eq!(v["module_id"], "main");
        let imports = v["imports"].as_array().expect("imports[]");
        assert!(
            imports.iter().any(|x| x.as_str() == Some("std.codec")),
            "missing std.codec import"
        );
        assert!(
            imports.iter().any(|x| x.as_str() == Some("verify_fixture")),
            "missing target module import"
        );
    }

    #[test]
    fn contains_direct_recursion_detects_call_head() {
        assert!(
            contains_direct_recursion(&json!(["verify_fixture.f"]), "verify_fixture.f"),
            "expected direct recursion"
        );
        assert!(
            contains_direct_recursion(
                &json!(["begin", ["verify_fixture.f", 1], 0]),
                "verify_fixture.f"
            ),
            "expected nested recursion"
        );
        assert!(
            !contains_direct_recursion(
                &json!(["begin", ["verify_fixture.g"], 0]),
                "verify_fixture.f"
            ),
            "unexpected recursion false positive"
        );
        assert!(
            !contains_direct_recursion(&json!("verify_fixture.f"), "verify_fixture.f"),
            "strings are not call heads"
        );
    }

    #[test]
    fn find_for_with_non_literal_bounds_requires_integer_literals() {
        assert!(find_for_with_non_literal_bounds(&json!(["for", "i", 0, 10, 0])).is_none());
        assert!(find_for_with_non_literal_bounds(&json!(["for", "i", "s", 10, 0])).is_some());
        assert!(find_for_with_non_literal_bounds(&json!(["for", "i", 0, "n", 0])).is_some());
        assert!(find_for_with_non_literal_bounds(&json!(["for", "i", 0, 10])).is_some());
    }

    #[test]
    fn extract_input_bytes_from_trace_handles_hex_and_suffixes() {
        let trace = vec![
            json!({"stepType":"assignment","lhs":"x07_verify_input[0]","value":{"data":"1"}}),
            json!({"stepType":"assignment","lhs":"x07_verify_input[1]","value":{"data":"0x2"}}),
            json!({"stepType":"assignment","hidden":true,"lhs":"x07_verify_input[1]","value":{"data":"0x9"}}),
            json!({"stepType":"assignment","lhs":"x07_verify_input[2]","value":{"data":"255u"}}),
            json!({"stepType":"assignment","lhs":"other[0]","value":{"data":"7"}}),
            json!({"stepType":"output","lhs":"x07_verify_input[0]","value":{"data":"8"}}),
        ];
        let bytes = extract_input_bytes_from_trace(&trace, "x07_verify_input", 3);
        assert_eq!(bytes, vec![1u8, 2u8, 255u8]);
    }

    #[test]
    fn normalize_smt2_logic_for_z3_drops_qf_prefix_when_quantifiers_present() {
        let dir =
            std::env::temp_dir().join(format!("x07_verify_smt2_quant_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("verify.smt2");
        std::fs::write(
            &path,
            "(set-logic QF_AUFBV)\n(assert (forall ((x Int)) true))\n(get-model)\n",
        )
        .expect("write smt2");

        normalize_smt2_logic_for_z3(&path).expect("normalize smt2");
        let text = std::fs::read_to_string(&path).expect("read smt2");
        assert!(text.starts_with("(set-logic AUFBV)\n"));
        assert!(!text.contains("(get-model)"));
        std::fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn normalize_smt2_logic_for_z3_leaves_quantifier_free_files_unchanged() {
        let dir = std::env::temp_dir().join(format!("x07_verify_smt2_qf_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("verify.smt2");
        let original = "(set-logic QF_AUFBV)\n(assert true)\n";
        std::fs::write(&path, original).expect("write smt2");

        normalize_smt2_logic_for_z3(&path).expect("normalize smt2");
        let text = std::fs::read_to_string(&path).expect("read smt2");
        assert_eq!(text, original);
        std::fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn normalize_smt2_logic_for_z3_strips_model_queries_without_touching_logic() {
        let dir = std::env::temp_dir().join(format!(
            "x07_verify_smt2_model_queries_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("verify.smt2");
        std::fs::write(
            &path,
            "(set-logic QF_AUFBV)\n(assert true)\n(check-sat)\n(get-value (x))\n(exit)\n",
        )
        .expect("write smt2");

        normalize_smt2_logic_for_z3(&path).expect("normalize smt2");
        let text = std::fs::read_to_string(&path).expect("read smt2");
        assert!(text.contains("(check-sat)\n"));
        assert!(!text.contains("(get-value (x))"));
        assert!(text.contains("(exit)\n"));
        std::fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
}
