use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use base64::Engine;
use clap::Args;
use serde::Serialize;
use serde_json::Value;
use x07_contracts::{X07_VERIFY_CEX_SCHEMA_VERSION, X07_VERIFY_REPORT_SCHEMA_VERSION};
use x07_worlds::WorldId;

use crate::report_common;
use crate::repro::ToolInfo;
use crate::util;

const X07_VERIFY_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.report.schema.json");
const X07_VERIFY_CEX_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.verify.cex@0.1.0.schema.json");

const VERIFY_INPUT_BUF_NAME: &str = "x07_verify_input";
const VERIFY_HARNESS_FN: &str = "x07_verify_harness";

#[derive(Debug, Clone, Args)]
pub struct VerifyArgs {
    /// Bounded model checking via CBMC (compile-to-C + assertions).
    #[arg(long, conflicts_with = "smt")]
    pub bmc: bool,

    /// Emit an SMT-LIB2 formula (via CBMC) and optionally solve with Z3.
    #[arg(long, conflicts_with = "bmc")]
    pub smt: bool,

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
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Bmc => "bmc",
            Mode::Smt => "smt",
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
struct VerifyReport {
    schema_version: &'static str,
    mode: &'static str,
    ok: bool,
    entry: String,
    bounds: Bounds,
    result: VerifyResult,
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
    let mode = if args.smt { Mode::Smt } else { Mode::Bmc };
    let bounds0 = Bounds {
        unwind: args.unwind,
        max_bytes_len: args.max_bytes_len,
        input_len_bytes: 0,
    };
    let entry = args.entry.clone();

    if !args.bmc && !args.smt {
        let d = diag_verify("X07V_EARGS", "set exactly one of --bmc or --smt");
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
    if target.is_async {
        let msg = "x07 verify does not support defasync targets (use a defn wrapper)";
        let d = diag_verify("X07V_UNSUPPORTED_ASYNC", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::error(
                mode,
                &args.entry,
                Bounds {
                    unwind: args.unwind,
                    max_bytes_len: args.max_bytes_len,
                    input_len_bytes: 0,
                },
                d,
                1,
            ),
        );
    }

    if !target.has_contracts {
        let msg = "target function has no requires/ensures/invariant clauses";
        let d = diag_verify("X07V_NO_CONTRACTS", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
        );
    }

    if contains_direct_recursion(&target.body, &args.entry) {
        let msg = "x07 verify v0.1 does not support recursive targets";
        let d = diag_verify("X07V_UNSUPPORTED_RECURSION", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
        );
    }

    if let Some(msg) = find_for_with_non_literal_bounds(&target.body) {
        return write_report_and_exit(
            machine,
            VerifyReport::error(
                mode,
                &args.entry,
                Bounds::for_args(&args),
                diag_verify("X07V_UNSUPPORTED_FOR_BOUNDS", msg),
                1,
            ),
        );
    }

    let input_len_bytes = match compute_input_len_bytes(&target, args.max_bytes_len) {
        Ok(v) => v,
        Err(err) => {
            let d = diag_verify("X07V_UNSUPPORTED_PARAM", err.to_string());
            return write_report_and_exit(
                machine,
                VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
            );
        }
    };
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
    };
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("create artifact dir: {}", work_dir.display()))?;

    let driver_src = build_verify_driver_x07ast_json(&args.entry, &target, args.max_bytes_len)?;
    let driver_path = work_dir.join("driver.x07.json");
    util::write_atomic(&driver_path, &driver_src)
        .with_context(|| format!("write verify driver: {}", driver_path.display()))?;
    artifacts.driver_path = Some(driver_path.display().to_string());

    let c_src = compile_driver_to_c(&driver_src, &module_roots)?;
    let c_with_harness = format!("{c_src}\n\n{}\n", build_c_harness(bounds.input_len_bytes));
    let c_path = work_dir.join("verify.c");
    util::write_atomic(&c_path, c_with_harness.as_bytes())
        .with_context(|| format!("write verify C: {}", c_path.display()))?;
    artifacts.c_path = Some(c_path.display().to_string());

    match mode {
        Mode::Bmc => cmd_verify_bmc(machine, &args, bounds, &work_dir, &c_path, artifacts),
        Mode::Smt => cmd_verify_smt(machine, &args, bounds, &work_dir, &c_path, artifacts),
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

    let cbmc_args = vec![
        c_path.display().to_string(),
        "--function".to_string(),
        VERIFY_HARNESS_FN.to_string(),
        "--unwind".to_string(),
        args.unwind.to_string(),
        "--unwinding-assertions".to_string(),
        "--no-standard-checks".to_string(),
        "--trace".to_string(),
        "--json-ui".to_string(),
    ];

    let out = Command::new("cbmc")
        .args(&cbmc_args)
        .output()
        .context("run cbmc")?;

    if !out.stderr.is_empty() {
        // cbmc can print UI status to stdout (in json-ui mode), but unexpected stderr is a signal.
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
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
) -> Result<std::process::ExitCode> {
    if !command_exists("cbmc") {
        let msg = "cbmc is required for `x07 verify --smt` (install: `brew install cbmc` or see https://diffblue.github.io/cbmc/)";
        let d = diag_verify("X07V_ECBMC_MISSING", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::tool_missing(Mode::Smt, &args.entry, bounds, d, artifacts, 1),
        );
    }

    let smt2_path = work_dir.join("verify.smt2");

    let cbmc_args = vec![
        c_path.display().to_string(),
        "--function".to_string(),
        VERIFY_HARNESS_FN.to_string(),
        "--unwind".to_string(),
        args.unwind.to_string(),
        "--unwinding-assertions".to_string(),
        "--no-standard-checks".to_string(),
        "--smt2".to_string(),
        "--outfile".to_string(),
        smt2_path.display().to_string(),
    ];

    let out = Command::new("cbmc")
        .args(&cbmc_args)
        .output()
        .context("run cbmc (smt2 emit)")?;

    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let diag_msg = format!("cbmc failed to emit SMT2: {msg}");
        let d = diag_verify("X07V_ECBMC_SMT2", diag_msg);
        return write_report_and_exit(
            machine,
            VerifyReport::error(Mode::Smt, &args.entry, bounds, d, 1).with_artifacts(artifacts),
        );
    }

    if !out.stderr.is_empty() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let d = diag_verify("X07V_ECBMC_STDERR", format!("cbmc wrote to stderr: {msg}"));
        return write_report_and_exit(
            machine,
            VerifyReport::error(Mode::Smt, &args.entry, bounds, d, 1).with_artifacts(artifacts),
        );
    }

    artifacts.smt2_path = Some(smt2_path.display().to_string());

    if !command_exists("z3") {
        let msg = "z3 is not installed (SMT2 was emitted; install: `brew install z3` or https://github.com/Z3Prover/z3)";
        let d = diag_verify("X07V_EZ3_MISSING", msg);
        return write_report_and_exit(
            machine,
            VerifyReport::inconclusive(Mode::Smt, &args.entry, bounds, d, artifacts, 2),
        );
    }

    let z3_out = Command::new("z3")
        .arg("-smt2")
        .arg(&smt2_path)
        .output()
        .context("run z3")?;

    if !z3_out.status.success() {
        let msg = String::from_utf8_lossy(&z3_out.stderr).trim().to_string();
        let d = diag_verify("X07V_EZ3_RUN", format!("z3 failed: {msg}"));
        return write_report_and_exit(
            machine,
            VerifyReport::error(Mode::Smt, &args.entry, bounds, d, 1).with_artifacts(artifacts),
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
            VerifyReport::verified(Mode::Smt, &args.entry, bounds, artifacts),
        ),
        "sat" => write_report_and_exit(
            machine,
            VerifyReport::counterexample_found(
                Mode::Smt,
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
                Mode::Smt,
                &args.entry,
                bounds,
                diag_verify("X07V_SMT_UNKNOWN", format!("solver returned {other:?}")),
                artifacts,
                2,
            ),
        ),
    }
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
    is_async: bool,
    has_contracts: bool,
    body: Value,
}

fn load_target_info(module_roots: &[PathBuf], entry: &str) -> Result<TargetSig> {
    let (module_id, _) = entry.rsplit_once('.').context("--entry must contain '.'")?;
    let rel = format!("{}.x07.json", module_id.replace('.', "/"));
    let mut found: Option<PathBuf> = None;
    for root in module_roots {
        let cand = root.join(&rel);
        if cand.is_file() {
            found = Some(cand);
            break;
        }
    }
    let path = found
        .with_context(|| format!("could not resolve module {module_id:?} (looked for {rel:?})"))?;
    let bytes = std::fs::read(&path).with_context(|| format!("read module: {}", path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse module JSON: {}", path.display()))?;

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
            is_async: kind == "defasync",
            has_contracts,
            body,
        });
    }

    anyhow::bail!(
        "could not find function {entry:?} in module {}",
        path.display()
    )
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

fn build_c_harness(input_len: u32) -> String {
    let mut out = String::new();
    out.push_str("extern unsigned char x07_nondet_u8(void);\n");
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
            artifacts: Some(artifacts),
            diagnostics_count: 1,
            diagnostics: vec![d],
            exit_code,
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
    let diags = report_common::validate_schema(
        X07_VERIFY_REPORT_SCHEMA_BYTES,
        "spec/x07-verify.report.schema.json",
        &v,
    )?;
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

    #[test]
    fn verify_driver_imports_std_codec_and_uses_std_codec_read() {
        let sig = TargetSig {
            params: vec!["i32".to_string()],
            is_async: false,
            has_contracts: true,
            body: json!(0),
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
}
