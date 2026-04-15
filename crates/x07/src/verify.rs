use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result};
use base64::Engine;
use clap::Args;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07_contracts::{
    X07AST_SCHEMA_VERSION, X07_VERIFY_CEX_SCHEMA_VERSION, X07_VERIFY_COVERAGE_SCHEMA_VERSION,
    X07_VERIFY_PRIMITIVES_SCHEMA_VERSION, X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_VERSION,
    X07_VERIFY_PROOF_OBJECT_SCHEMA_VERSION, X07_VERIFY_PROOF_SUMMARY_SCHEMA_VERSION,
    X07_VERIFY_REPORT_SCHEMA_VERSION, X07_VERIFY_SUMMARY_SCHEMA_VERSION,
};
use x07_worlds::WorldId;
use x07c::ast::Expr;
use x07c::x07ast::{
    self, AsyncProtocolAst, ContractClauseAst, LoopContractAst, TypeRef, X07AstFile,
};

use crate::report_common;
use crate::repro::ToolInfo;
use crate::util;

const X07_VERIFY_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.report.schema.json");
const X07_VERIFY_COVERAGE_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.coverage.schema.json");
const X07_VERIFY_CEX_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.verify.cex@0.2.0.schema.json");
const X07_VERIFY_PRIMITIVES_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.primitives.schema.json");
const X07_VERIFY_SUMMARY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.summary.schema.json");
const X07_VERIFY_PROOF_SUMMARY_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.proof-summary.schema.json");
const X07_VERIFY_PROOF_OBJECT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.proof-object.schema.json");
const X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07-verify.proof-check.report.schema.json");
const X07_VERIFY_PRIMITIVES_CATALOG_BYTES: &[u8] =
    include_bytes!("../../../catalog/verify_primitives.json");
const X07_VERIFY_SCHEDULER_MODEL_BYTES: &[u8] =
    include_bytes!("../../../catalog/verify_scheduler_model.json");
const X07DIAG_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07diag.schema.json");

const VERIFY_INPUT_BUF_NAME: &str = "x07_verify_input";
const VERIFY_HARNESS_FN: &str = "x07_verify_harness";
const Z3_TIMEOUT_SECONDS: u64 = 10;
const Z3_ASYNC_PROVE_TIMEOUT_SECONDS: u64 = 90;
const PROCESS_SUMMARY_MAX_CHARS: usize = 1024;
const CBMC_OBJECT_BITS_RETRY_VALUES: [u32; 2] = [12, 16];
const VERIFY_VEC_SUPPORT_MODULE_ID: &str = "x07.verify.vec_support_v1";

fn expr_ident(name: impl Into<String>) -> Expr {
    Expr::Ident {
        name: name.into(),
        ptr: String::new(),
    }
}

fn expr_int(value: i32) -> Expr {
    Expr::Int {
        value,
        ptr: String::new(),
    }
}

fn expr_list(items: Vec<Expr>) -> Expr {
    Expr::List {
        items,
        ptr: String::new(),
    }
}

fn expr_call(head: impl Into<String>, args: Vec<Expr>) -> Expr {
    let mut items = Vec::with_capacity(args.len() + 1);
    items.push(expr_ident(head));
    items.extend(args);
    expr_list(items)
}

fn rewrite_verify_overlay_type_ref(ty: &mut TypeRef) -> bool {
    match ty {
        TypeRef::Named(name) => {
            if name == "vec_u8" {
                *name = "bytes".to_string();
                true
            } else {
                false
            }
        }
        TypeRef::Var(_) => false,
        TypeRef::App { args, .. } => args.iter_mut().fold(false, |changed, arg| {
            rewrite_verify_overlay_type_ref(arg) || changed
        }),
    }
}

fn rewrite_verify_overlay_contracts(clauses: &mut [ContractClauseAst]) -> bool {
    let mut changed = false;
    for clause in clauses {
        changed |= rewrite_verify_overlay_expr(&mut clause.expr);
        for witness in &mut clause.witness {
            changed |= rewrite_verify_overlay_expr(witness);
        }
    }
    changed
}

fn rewrite_verify_overlay_loop_contracts(loop_contracts: &mut [LoopContractAst]) -> bool {
    let mut changed = false;
    for loop_contract in loop_contracts {
        changed |= rewrite_verify_overlay_contracts(&mut loop_contract.invariant);
        changed |= rewrite_verify_overlay_contracts(&mut loop_contract.decreases);
    }
    changed
}

fn rewrite_verify_overlay_async_protocol(protocol: Option<&mut AsyncProtocolAst>) -> bool {
    let Some(protocol) = protocol else {
        return false;
    };
    rewrite_verify_overlay_contracts(&mut protocol.await_invariant)
        | rewrite_verify_overlay_contracts(&mut protocol.scope_invariant)
        | rewrite_verify_overlay_contracts(&mut protocol.cancellation_ensures)
}

fn rewrite_verify_overlay_expr(expr: &mut Expr) -> bool {
    let Expr::List { items, .. } = expr else {
        return false;
    };
    let mut changed = false;
    for item in items.iter_mut() {
        changed |= rewrite_verify_overlay_expr(item);
    }
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        return changed;
    };
    let args = items[1..].to_vec();
    let replacement = match head {
        "vec_u8.with_capacity" | "std.vec.with_capacity" if args.len() == 1 => {
            Some(expr_call("bytes.alloc", vec![expr_int(0)]))
        }
        "vec_u8.into_bytes" | "std.vec.as_bytes" if args.len() == 1 => Some(args[0].clone()),
        "vec_u8.clear" if args.len() == 1 => Some(expr_call("bytes.alloc", vec![expr_int(0)])),
        "vec_u8.reserve_exact" | "std.vec.reserve_exact" if args.len() == 2 => {
            Some(args[0].clone())
        }
        "vec_u8.len" | "std.vec.len" if args.len() == 1 => Some(expr_call(
            format!("{VERIFY_VEC_SUPPORT_MODULE_ID}.len_v1"),
            vec![args[0].clone()],
        )),
        "vec_u8.get" | "std.vec.get" if args.len() == 2 => Some(expr_call(
            format!("{VERIFY_VEC_SUPPORT_MODULE_ID}.get_v1"),
            vec![args[0].clone(), args[1].clone()],
        )),
        "vec_u8.as_view" if args.len() == 1 => Some(expr_call("bytes.view", vec![args[0].clone()])),
        "vec_u8.push" | "std.vec.push" if args.len() == 2 => Some(expr_call(
            format!("{VERIFY_VEC_SUPPORT_MODULE_ID}.push_v1"),
            vec![args[0].clone(), args[1].clone()],
        )),
        "vec_u8.set" if args.len() == 3 => Some(expr_call(
            format!("{VERIFY_VEC_SUPPORT_MODULE_ID}.set_v1"),
            vec![args[0].clone(), args[1].clone(), args[2].clone()],
        )),
        "vec_u8.extend_bytes" | "std.vec.extend_bytes" if args.len() == 2 => Some(expr_call(
            format!("{VERIFY_VEC_SUPPORT_MODULE_ID}.extend_bytes_v1"),
            vec![args[0].clone(), args[1].clone()],
        )),
        "vec_u8.extend_bytes_range" if args.len() == 4 => Some(expr_call(
            format!("{VERIFY_VEC_SUPPORT_MODULE_ID}.extend_bytes_range_v1"),
            vec![
                args[0].clone(),
                args[1].clone(),
                args[2].clone(),
                args[3].clone(),
            ],
        )),
        _ => None,
    };
    if let Some(replacement) = replacement {
        *expr = replacement;
        true
    } else {
        changed
    }
}

fn rewrite_verify_overlay_file(file: &mut X07AstFile) {
    let mut needs_vec_support = false;
    for function in &mut file.functions {
        for param in &mut function.params {
            needs_vec_support |= rewrite_verify_overlay_type_ref(&mut param.ty);
        }
        needs_vec_support |= rewrite_verify_overlay_type_ref(&mut function.result);
        needs_vec_support |= rewrite_verify_overlay_contracts(&mut function.requires);
        needs_vec_support |= rewrite_verify_overlay_contracts(&mut function.ensures);
        needs_vec_support |= rewrite_verify_overlay_contracts(&mut function.invariant);
        needs_vec_support |= rewrite_verify_overlay_loop_contracts(&mut function.loop_contracts);
        needs_vec_support |= rewrite_verify_overlay_expr(&mut function.body);
    }
    for function in &mut file.async_functions {
        for param in &mut function.params {
            needs_vec_support |= rewrite_verify_overlay_type_ref(&mut param.ty);
        }
        needs_vec_support |= rewrite_verify_overlay_type_ref(&mut function.result);
        needs_vec_support |= rewrite_verify_overlay_contracts(&mut function.requires);
        needs_vec_support |= rewrite_verify_overlay_contracts(&mut function.ensures);
        needs_vec_support |= rewrite_verify_overlay_contracts(&mut function.invariant);
        needs_vec_support |= rewrite_verify_overlay_async_protocol(function.protocol.as_mut());
        needs_vec_support |= rewrite_verify_overlay_loop_contracts(&mut function.loop_contracts);
        needs_vec_support |= rewrite_verify_overlay_expr(&mut function.body);
    }
    for function in &mut file.extern_functions {
        for param in &mut function.params {
            needs_vec_support |= rewrite_verify_overlay_type_ref(&mut param.ty);
        }
        if let Some(result) = function.result.as_mut() {
            needs_vec_support |= rewrite_verify_overlay_type_ref(result);
        }
    }
    if let Some(solve) = file.solve.as_mut() {
        needs_vec_support |= rewrite_verify_overlay_expr(solve);
    }
    if needs_vec_support {
        file.imports
            .insert(VERIFY_VEC_SUPPORT_MODULE_ID.to_string());
    }
    x07ast::canonicalize_x07ast_file(file);
}

fn verify_overlay_module_path(root: &Path, module_id: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for seg in module_id.split('.') {
        path.push(seg);
    }
    path.set_extension("x07.json");
    path
}

fn build_verify_vec_support_module() -> Value {
    serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": VERIFY_VEC_SUPPORT_MODULE_ID,
        "imports": [],
        "decls": [
            {
                "kind": "export",
                "names": [
                    "x07.verify.vec_support_v1.extend_bytes_range_v1",
                    "x07.verify.vec_support_v1.extend_bytes_v1",
                    "x07.verify.vec_support_v1.get_v1",
                    "x07.verify.vec_support_v1.len_v1",
                    "x07.verify.vec_support_v1.push_v1",
                    "x07.verify.vec_support_v1.set_v1"
                ]
            },
            {
                "kind": "defn",
                "name": "x07.verify.vec_support_v1.len_v1",
                "params": [{"name": "v", "ty": "bytes"}],
                "result": "i32",
                "body": ["view.len", ["bytes.view", "v"]]
            },
            {
                "kind": "defn",
                "name": "x07.verify.vec_support_v1.get_v1",
                "params": [{"name": "v", "ty": "bytes"}, {"name": "idx", "ty": "i32"}],
                "result": "i32",
                "body": ["view.get_u8", ["bytes.view", "v"], "idx"]
            },
            {
                "kind": "defn",
                "name": "x07.verify.vec_support_v1.extend_bytes_v1",
                "params": [{"name": "v", "ty": "bytes"}, {"name": "chunk", "ty": "bytes_view"}],
                "result": "bytes",
                "body": ["bytes.concat", "v", ["view.to_bytes", "chunk"]]
            },
            {
                "kind": "defn",
                "name": "x07.verify.vec_support_v1.extend_bytes_range_v1",
                "params": [
                    {"name": "v", "ty": "bytes"},
                    {"name": "chunk", "ty": "bytes_view"},
                    {"name": "start", "ty": "i32"},
                    {"name": "len", "ty": "i32"}
                ],
                "result": "bytes",
                "body": [
                    "bytes.concat",
                    "v",
                    ["view.to_bytes", ["view.slice", "chunk", "start", "len"]]
                ]
            },
            {
                "kind": "defn",
                "name": "x07.verify.vec_support_v1.push_v1",
                "params": [{"name": "v", "ty": "bytes"}, {"name": "x", "ty": "i32"}],
                "result": "bytes",
                "body": [
                    "begin",
                    ["let", "word", ["codec.write_u32_le", "x"]],
                    ["let", "word_v", ["bytes.view", "word"]],
                    ["let", "one", ["view.to_bytes", ["view.slice", "word_v", 0, 1]]],
                    ["bytes.concat", "v", "one"]
                ]
            },
            {
                "kind": "defn",
                "name": "x07.verify.vec_support_v1.set_v1",
                "params": [
                    {"name": "v", "ty": "bytes"},
                    {"name": "idx", "ty": "i32"},
                    {"name": "x", "ty": "i32"}
                ],
                "result": "bytes",
                "body": [
                    "begin",
                    ["let", "base_v", ["bytes.view", "v"]],
                    ["let", "n", ["view.len", "base_v"]],
                    [
                        "if",
                        ["<u", "idx", "n"],
                        [
                            "begin",
                            ["let", "prefix", ["view.to_bytes", ["view.slice", "base_v", 0, "idx"]]],
                            ["let", "word", ["codec.write_u32_le", "x"]],
                            ["let", "word_v", ["bytes.view", "word"]],
                            ["let", "one", ["view.to_bytes", ["view.slice", "word_v", 0, 1]]],
                            ["let", "next_idx", ["+", "idx", 1]],
                            ["let", "suffix", ["view.to_bytes", ["view.slice", "base_v", "next_idx", ["-", "n", "next_idx"]]]],
                            ["bytes.concat", ["bytes.concat", "prefix", "one"], "suffix"]
                        ],
                        ["begin", ["view.get_u8", "base_v", "idx"], ["bytes.alloc", 0]]
                    ]
                ]
            }
        ]
    })
}

fn build_verify_compile_module_roots(
    module_roots: &[PathBuf],
    entry: &str,
    work_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let overlay_root = work_dir.join("module_overlay");
    std::fs::create_dir_all(&overlay_root)
        .with_context(|| format!("create verify overlay dir: {}", overlay_root.display()))?;

    let support_path = verify_overlay_module_path(&overlay_root, VERIFY_VEC_SUPPORT_MODULE_ID);
    if let Some(parent) = support_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create verify overlay module dir: {}", parent.display()))?;
    }
    let mut support_bytes = serde_json::to_vec_pretty(&build_verify_vec_support_module())
        .context("encode vec support module JSON")?;
    support_bytes.push(b'\n');
    util::write_atomic(&support_path, &support_bytes)
        .with_context(|| format!("write verify support module: {}", support_path.display()))?;

    let (entry_module, _) = entry.rsplit_once('.').context("--entry must contain '.'")?;
    let mut queue = VecDeque::from([entry_module.to_string()]);
    let mut visited = BTreeSet::new();
    while let Some(module_id) = queue.pop_front() {
        if module_id == VERIFY_VEC_SUPPORT_MODULE_ID || !visited.insert(module_id.clone()) {
            continue;
        }
        let source =
            x07c::module_source::load_module_source(&module_id, WorldId::SolvePure, module_roots)
                .map_err(|err| anyhow::anyhow!("{:?}: {}", err.kind, err.message))?;
        let mut file = x07c::x07ast::parse_x07ast_json(source.src.as_bytes())
            .map_err(|err| anyhow::anyhow!("parse overlay module {module_id:?}: {err}"))?;
        rewrite_verify_overlay_file(&mut file);
        let imports = file.imports.iter().cloned().collect::<Vec<_>>();
        let mut bytes = serde_json::to_vec_pretty(&x07c::x07ast::x07ast_file_to_value(&file))
            .with_context(|| format!("encode overlay module JSON for {module_id:?}"))?;
        bytes.push(b'\n');
        let out_path = verify_overlay_module_path(&overlay_root, &module_id);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("create verify overlay module dir: {}", parent.display())
            })?;
        }
        util::write_atomic(&out_path, &bytes)
            .with_context(|| format!("write verify overlay module: {}", out_path.display()))?;
        for import in imports {
            if import != VERIFY_VEC_SUPPORT_MODULE_ID {
                queue.push_back(import);
            }
        }
    }

    Ok(vec![overlay_root])
}

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

    /// Override the encoded verification input length (bytes).
    ///
    /// If set, it must be >= the required encoding length for the target signature.
    #[arg(long, value_name = "N")]
    pub input_len_bytes: Option<u32>,

    /// Base directory for verification artifacts.
    ///
    /// Defaults to `<project_root>/.x07/artifacts` or `<cwd>/.x07/artifacts`.
    #[arg(long, value_name = "DIR")]
    pub artifact_dir: Option<PathBuf>,

    /// Import one or more proof-summary artifacts produced by `x07 verify`.
    ///
    /// Deprecated alias: `--summary`.
    #[arg(long = "proof-summary", alias = "summary", value_name = "PATH")]
    pub summary: Vec<PathBuf>,

    /// Developer-only: permit imported_stub assumptions in prove mode.
    #[arg(long)]
    pub allow_imported_stubs: bool,

    /// Emit a proof object for independent checking.
    #[arg(long, value_name = "PATH")]
    pub emit_proof: Option<PathBuf>,
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
            input_len_bytes: args.input_len_bytes.unwrap_or(0),
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    verify_coverage_summary_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verify_proof_summary_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_object_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_check_report_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyCoverage {
    schema_version: &'static str,
    entry: String,
    worlds: Vec<String>,
    summary: VerifyCoverageSummary,
    functions: Vec<VerifyCoverageFunction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyCoverageSummary {
    reachable_defn: u64,
    supported_defn: u64,
    recursive_defn: u64,
    supported_recursive_defn: u64,
    imported_proof_summary_defn: u64,
    termination_proven_defn: u64,
    unsupported_recursive_defn: u64,
    reachable_async: u64,
    supported_async: u64,
    trusted_primitives: u64,
    trusted_scheduler_models: u64,
    capsule_boundaries: u64,
    uncovered_defn: u64,
    unsupported_defn: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    async_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyCoverageFunction {
    symbol: String,
    kind: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<VerifyFunctionSignature>,
    #[serde(skip_serializing_if = "Option::is_none")]
    support_summary: Option<VerifyFunctionSupportSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decl_sha256_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyFunctionSupportSummary {
    recursion_kind: String,
    has_decreases: bool,
    decreases_count: u64,
    prove_supported: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyFunctionSignature {
    params: Vec<VerifySignatureParam>,
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_brand: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifySignatureParam {
    ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    brand: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyCoverageSummaryArtifact {
    schema_version: String,
    summary_kind: String,
    entry: String,
    worlds: Vec<String>,
    summary: VerifyCoverageSummary,
    functions: Vec<VerifyCoverageFunction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    imported_summaries: Vec<VerifyImportedSummaryRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyImportedSummaryRef {
    path: String,
    sha256_hex: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    symbols: Vec<String>,
}

#[derive(Debug, Clone)]
struct ImportedSummaryFunction {
    function: VerifyProofSummaryArtifact,
    source: VerifyImportedSummaryRef,
}

#[derive(Debug, Clone, Default)]
struct ImportedSummaryIndex {
    by_symbol: BTreeMap<String, ImportedSummaryFunction>,
    inventory: Vec<VerifyImportedSummaryRef>,
}

#[derive(Debug, Clone)]
struct CoverageAnalysis {
    coverage: VerifyCoverage,
    diagnostics: Vec<x07c::diagnostics::Diagnostic>,
    imported_summaries: Vec<VerifyImportedSummaryRef>,
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
    assumption_class: String,
    certification_policy: String,
    #[allow(dead_code)]
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct TrustedPrimitiveStub {
    symbol: String,
    params: Vec<VerifySignatureParam>,
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
    param_names: Vec<String>,
    params: Vec<VerifySignatureParam>,
    result: String,
    result_brand: Option<String>,
    decl_sha256_hex: String,
    has_contracts: bool,
    decreases_count: usize,
    decreases: Vec<Value>,
    body: Option<Value>,
    contract_exprs: Vec<Value>,
    source_path: PathBuf,
}

#[derive(Debug, Clone)]
struct VerifyBrandModule {
    imports: Vec<String>,
    validators: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifySchedulerModel {
    schema_version: String,
    id: String,
    guarantees: Vec<String>,
}

#[derive(Debug, Clone)]
struct CoverageTrustZoneIndex {
    project_root: PathBuf,
    nodes: Vec<CoverageTrustZoneNode>,
}

#[derive(Debug, Clone)]
struct CoverageTrustZoneNode {
    id: String,
    module_prefixes: Vec<String>,
    path_globs: GlobSet,
    trust_zone: String,
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
    proof_summary: Option<VerifyProofSummary>,
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

#[derive(Debug, Clone, Serialize)]
struct VerifyProofSummary {
    engine: String,
    recursion_kind: String,
    has_decreases: bool,
    decreases_count: u64,
    bounded_by_unwind: bool,
    recursion_bound_kind: String,
    dependency_symbols: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyProofSummaryArtifact {
    schema_version: String,
    summary_kind: String,
    symbol: String,
    kind: String,
    decl_sha256_hex: String,
    result_kind: String,
    engine: String,
    recursion_kind: String,
    recursion_bound_kind: String,
    dependency_symbols: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_object_digest: Option<String>,
    #[serde(default)]
    assumptions: Vec<VerifyProofAssumption>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct VerifyProofAssumption {
    kind: String,
    subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
    certifiable: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct VerifyProofObject {
    schema_version: String,
    project_manifest_digest: String,
    entry_symbol: String,
    symbol: String,
    kind: String,
    decl_sha256_hex: String,
    verify_engine: String,
    primitive_manifest_digest: String,
    #[serde(default)]
    imported_proof_summary_digests: Vec<String>,
    proof_summary_digest: String,
    obligation_digest: String,
    expected_solver_result: String,
    recursion_kind: String,
    recursion_bound_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler_model_digest: Option<String>,
    unwind: u32,
    max_bytes_len: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct VerifyProofCheckReport {
    schema_version: String,
    pub(crate) ok: bool,
    pub(crate) proof_object_digest: String,
    pub(crate) checker: String,
    pub(crate) result: String,
    pub(crate) symbol: String,
    pub(crate) entry_symbol: String,
    pub(crate) verify_engine: String,
    pub(crate) expected_obligation_digest: String,
    pub(crate) replayed_obligation_digest: String,
    pub(crate) expected_solver_result: String,
    pub(crate) replayed_solver_result: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) validated_imported_proof_summary_digests: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) validated_scheduler_model_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<x07c::diagnostics::Diagnostic>,
}

#[derive(Debug, Clone)]
struct ProveDriverBuild {
    driver_src: Vec<u8>,
    c_with_harness: String,
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
    let recursion = match recursion_summary_for_symbol(
        &module_roots,
        coverage_world(project_path.as_deref()),
        &args.entry,
    ) {
        Ok(v) => v,
        Err(err) => {
            let d = diag_verify("X07V_ETARGET", format!("{err:#}"));
            return write_report_and_exit(
                machine,
                VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
            );
        }
    };

    let imported_summary_index = match load_imported_summary_index(&cwd, &args.summary) {
        Ok(v) => v,
        Err(diagnostics) => {
            let d = diagnostics.first().cloned().unwrap_or_else(|| {
                diag_verify(
                    "X07V_SUMMARY_MISMATCH",
                    "imported proof-summary validation failed",
                )
            });
            return write_report_and_exit(
                machine,
                VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1)
                    .with_diagnostics(diagnostics),
            );
        }
    };

    let artifact_base =
        resolve_artifact_base_dir(&cwd, project_root.as_deref(), args.artifact_dir.as_deref());

    if mode == Mode::Coverage {
        return cmd_verify_coverage(
            machine,
            &args,
            project_path.as_deref(),
            &module_roots,
            &target,
            &imported_summary_index,
            &artifact_base,
        );
    }

    if mode == Mode::Prove {
        if args.emit_proof.is_some() && project_path.is_none() {
            let d = diag_verify(
                "X07V_EPROJECT",
                "emit-proof requires a project manifest (`--project` or a reachable x07.json)",
            );
            return write_report_and_exit(
                machine,
                VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
            );
        }
        if let Some((code, msg)) = prove_unsupported_reason(
            &module_roots,
            &target,
            &args.entry,
            args.max_bytes_len,
            &recursion,
        ) {
            return write_report_and_exit(
                machine,
                VerifyReport::unsupported(mode, &args.entry, Bounds::for_args(&args), code, msg, 2),
            );
        }
        if target.is_async {
            if let Err(err) = load_verify_scheduler_model() {
                return write_report_and_exit(
                    machine,
                    VerifyReport::unsupported(
                        mode,
                        &args.entry,
                        Bounds::for_args(&args),
                        "X07V_SCHEDULER_MODEL_UNTRUSTED",
                        format!("trusted scheduler model is unavailable: {err:#}"),
                        2,
                    ),
                );
            }
        }
    } else if let Some(d) = verify_precheck_diag(
        &module_roots,
        &target,
        &args.entry,
        args.max_bytes_len,
        &recursion,
    ) {
        return write_report_and_exit(
            machine,
            VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
        );
    }

    let required_input_len_bytes =
        compute_input_len_bytes(&target, args.max_bytes_len).map_err(|err| {
            anyhow::anyhow!(
                "internal verify precheck mismatch for {:?}: {err}",
                args.entry
            )
        })?;
    let input_len_bytes = match args.input_len_bytes {
        Some(v) if v < required_input_len_bytes => {
            let d = diag_verify(
                "X07V_EARGS",
                format!(
                    "--input-len-bytes must be >= {required_input_len_bytes} for {:?} (got {v})",
                    args.entry
                ),
            );
            return write_report_and_exit(
                machine,
                VerifyReport::error(mode, &args.entry, Bounds::for_args(&args), d, 1),
            );
        }
        Some(v) => v,
        None => required_input_len_bytes,
    };
    let bounds = Bounds {
        unwind: args.unwind,
        max_bytes_len: args.max_bytes_len,
        input_len_bytes,
    };

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

    let driver_path = work_dir.join("driver.x07.json");
    let driver_src = if mode == Mode::Prove {
        None
    } else {
        let driver_src = build_verify_driver_x07ast_json(
            &module_roots,
            &args.entry,
            &target,
            args.max_bytes_len,
            false,
        )?;
        util::write_atomic(&driver_path, &driver_src)
            .with_context(|| format!("write verify driver: {}", driver_path.display()))?;
        artifacts.driver_path = Some(driver_path.display().to_string());
        Some(driver_src)
    };

    let prove_coverage = if mode == Mode::Prove {
        Some(coverage_report_for_entry(
            &args,
            project_path.as_deref(),
            &target,
            &imported_summary_index,
            true,
        )?)
    } else {
        None
    };
    let proof_summary = prove_coverage
        .as_ref()
        .map(|analysis| report_proof_summary(&analysis.coverage, &target, &recursion));
    let proof_summary_artifact = if let Some(analysis) = &prove_coverage {
        let primitive_catalog = load_verify_primitive_catalog()?;
        let artifact = build_verify_proof_summary_artifact(
            &analysis.coverage,
            &analysis.imported_summaries,
            &primitive_catalog,
            &target,
            &recursion,
        );
        if !args.allow_imported_stubs
            && artifact
                .assumptions
                .iter()
                .any(|assumption| assumption.kind == "imported_stub")
        {
            return write_report_and_exit(
                machine,
                VerifyReport::unsupported(
                    mode,
                    &args.entry,
                    bounds.clone(),
                    "X07V_IMPORTED_STUB_FORBIDDEN",
                    "prove mode depends on imported_stub assumptions; rerun with --allow-imported-stubs for a developer-only flow".to_string(),
                    2,
                ),
            );
        }
        Some(artifact)
    } else {
        None
    };

    if mode == Mode::Prove
        && prove_coverage
            .as_ref()
            .is_some_and(|analysis| !analysis.diagnostics.is_empty())
    {
        let diag = prove_coverage
            .as_ref()
            .and_then(|analysis| analysis.diagnostics.first())
            .cloned()
            .unwrap_or_else(|| {
                diag_verify(
                    "X07V_PROOF_SUMMARY_REQUIRED",
                    "imported proof-summary reuse failed for a reachable symbol",
                )
            });
        return write_report_and_exit(
            machine,
            VerifyReport::error(mode, &args.entry, bounds, diag, 2).with_artifacts(artifacts),
        );
    }

    if let Some(analysis) = &prove_coverage {
        let verify_summary_path = work_dir.join("verify.summary.json");
        write_verify_summary_artifact(
            &verify_summary_path,
            &analysis.coverage,
            &analysis.imported_summaries,
        )?;
        artifacts.verify_coverage_summary_path = Some(verify_summary_path.display().to_string());
    }

    let c_with_harness = if mode == Mode::Prove {
        match build_prove_driver_build(
            &args.entry,
            &target,
            &module_roots,
            prove_coverage.as_ref().map(|analysis| &analysis.coverage),
            args.max_bytes_len,
            bounds.input_len_bytes,
            &work_dir,
        ) {
            Ok(build) => {
                util::write_atomic(&driver_path, &build.driver_src)
                    .with_context(|| format!("write verify driver: {}", driver_path.display()))?;
                artifacts.driver_path = Some(driver_path.display().to_string());
                build.c_with_harness
            }
            Err(err) if target.is_async => {
                return write_report_and_exit(
                    machine,
                    VerifyReport::unsupported(
                        mode,
                        &args.entry,
                        bounds,
                        "X07V_UNSUPPORTED_DEFASYNC_FORM",
                        format!("defasync target uses an unsupported proof form: {err}"),
                        2,
                    )
                    .with_artifacts(artifacts),
                );
            }
            Err(err) => {
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
        }
    } else {
        let compile_module_roots = module_roots.clone();
        let c_src = compile_driver_to_c(
            driver_src
                .as_deref()
                .expect("driver source for non-prove modes"),
            &compile_module_roots,
        )?;
        let harness_src = build_c_harness(bounds.input_len_bytes);
        format!("{c_src}\n\n{harness_src}\n")
    };
    let c_path = work_dir.join("verify.c");
    util::write_atomic(&c_path, c_with_harness.as_bytes())
        .with_context(|| format!("write verify C: {}", c_path.display()))?;
    artifacts.c_path = Some(c_path.display().to_string());

    match mode {
        Mode::Bmc => cmd_verify_bmc(machine, &args, bounds, &work_dir, &c_path, artifacts),
        Mode::Smt | Mode::Prove => cmd_verify_smt(
            machine,
            &args,
            bounds,
            artifacts,
            VerifySmtPlan {
                target: &target,
                work_dir: &work_dir,
                c_path: &c_path,
                mode,
                project_path: project_path.clone(),
                proof_summary,
                proof_summary_artifact,
                proof_imported_summaries: prove_coverage
                    .as_ref()
                    .map(|analysis| analysis.imported_summaries.clone())
                    .unwrap_or_default(),
                emit_proof_path: args.emit_proof.clone(),
            },
        ),
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

    let (out, used_cbmc_args) = run_cbmc_with_object_bits_retry(&cbmc_args, "run cbmc")?;

    if !out.stderr.is_empty() && !cbmc_stderr_is_benign(&out.stderr) {
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
                .chain(used_cbmc_args)
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
        proof_summary: None,
        coverage: None,
        artifacts: Some(artifacts),
        diagnostics_count: 0,
        diagnostics: Vec::new(),
        exit_code: 10,
    };
    write_report_and_exit(machine, report)
}

struct VerifySmtPlan<'a> {
    target: &'a TargetSig,
    work_dir: &'a Path,
    c_path: &'a Path,
    mode: Mode,
    project_path: Option<PathBuf>,
    proof_summary: Option<VerifyProofSummary>,
    proof_summary_artifact: Option<VerifyProofSummaryArtifact>,
    proof_imported_summaries: Vec<VerifyImportedSummaryRef>,
    emit_proof_path: Option<PathBuf>,
}

fn cmd_verify_smt(
    machine: &crate::reporting::MachineArgs,
    args: &VerifyArgs,
    bounds: Bounds,
    mut artifacts: Artifacts,
    plan: VerifySmtPlan<'_>,
) -> Result<std::process::ExitCode> {
    let attach_summary = |report: VerifyReport| {
        if plan.mode == Mode::Prove {
            if let Some(summary) = plan.proof_summary.clone() {
                return report.with_proof_summary(summary);
            }
        }
        report
    };
    if !command_exists("cbmc") {
        let msg = format!(
            "cbmc is required for `x07 verify --{}` (install: `brew install cbmc` or see https://diffblue.github.io/cbmc/)",
            plan.mode.as_str()
        );
        let d = diag_verify("X07V_ECBMC_MISSING", msg);
        return write_report_and_exit(
            machine,
            attach_summary(VerifyReport::tool_missing(
                plan.mode,
                &args.entry,
                bounds,
                d,
                artifacts,
                1,
            )),
        );
    }

    let smt2_path = plan.work_dir.join("verify.smt2");

    let mut cbmc_args = vec![
        plan.c_path.display().to_string(),
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

    let (out, _) = run_cbmc_with_object_bits_retry(&cbmc_args, "run cbmc (smt2 emit)")?;

    if !out.status.success() {
        let msg = summarize_process_failure(&out.stdout, &out.stderr, PROCESS_SUMMARY_MAX_CHARS);
        let diag_msg = format!("cbmc failed to emit SMT2: {msg}");
        let d = diag_verify("X07V_ECBMC_SMT2", diag_msg);
        return write_report_and_exit(
            machine,
            attach_summary(
                VerifyReport::error(plan.mode, &args.entry, bounds, d, 1).with_artifacts(artifacts),
            ),
        );
    }

    if !out.stderr.is_empty() && !cbmc_stderr_is_benign(&out.stderr) {
        let msg = summarize_process_text(&out.stderr, PROCESS_SUMMARY_MAX_CHARS);
        let d = diag_verify("X07V_ECBMC_STDERR", format!("cbmc wrote to stderr: {msg}"));
        return write_report_and_exit(
            machine,
            attach_summary(
                VerifyReport::error(plan.mode, &args.entry, bounds, d, 1).with_artifacts(artifacts),
            ),
        );
    }

    normalize_smt2_logic_for_z3(&smt2_path)?;
    ensure_smt2_reason_unknown_query(&smt2_path)?;
    artifacts.smt2_path = Some(smt2_path.display().to_string());

    if !command_exists("z3") {
        let msg = "z3 is not installed (SMT2 was emitted; install: `brew install z3` or https://github.com/Z3Prover/z3)";
        let d = diag_verify("X07V_EZ3_MISSING", msg);
        return write_report_and_exit(
            machine,
            attach_summary(VerifyReport::inconclusive(
                plan.mode,
                &args.entry,
                bounds,
                d,
                artifacts,
                2,
            )),
        );
    }

    let z3_out = Command::new("z3")
        .arg(format!("-T:{}", z3_timeout_seconds(plan.mode, plan.target)))
        .arg("-smt2")
        .arg(&smt2_path)
        .output()
        .context("run z3")?;

    if !z3_out.status.success() {
        let msg = summarize_process_text(&z3_out.stderr, PROCESS_SUMMARY_MAX_CHARS);
        let d = diag_verify("X07V_EZ3_RUN", format!("z3 failed: {msg}"));
        return write_report_and_exit(
            machine,
            attach_summary(
                VerifyReport::error(plan.mode, &args.entry, bounds, d, 1).with_artifacts(artifacts),
            ),
        );
    }

    let z3_stdout = String::from_utf8_lossy(&z3_out.stdout).to_string();
    let z3_out_path = plan.work_dir.join("z3.out.txt");
    util::write_atomic(&z3_out_path, z3_stdout.as_bytes())
        .with_context(|| format!("write z3 output: {}", z3_out_path.display()))?;
    artifacts.z3_out_path = Some(z3_out_path.display().to_string());

    let finalize_success_artifacts = |mut artifacts: Artifacts| -> Result<Artifacts> {
        if plan.mode != Mode::Prove {
            return Ok(artifacts);
        }
        if let Some(proof_summary_artifact) = plan.proof_summary_artifact.as_ref() {
            let proof_summary_path = if let Some(proof_path) = plan.emit_proof_path.as_deref() {
                let project_path = plan
                    .project_path
                    .as_deref()
                    .context("proof emission requires a project manifest")?;
                let (bundle_artifacts, _updated_summary) = write_prove_bundle_artifacts(
                    proof_path,
                    proof_summary_artifact,
                    &plan.proof_imported_summaries,
                    project_path,
                    &bounds,
                    &smt2_path,
                    &z3_out_path,
                )?;
                let proof_summary_path = bundle_artifacts.verify_proof_summary_path.clone();
                if let Some(path) = proof_summary_path.clone() {
                    artifacts.verify_proof_summary_path = Some(path);
                }
                if let Some(path) = bundle_artifacts.proof_object_path.clone() {
                    artifacts.proof_object_path = Some(path);
                }
                if let Some(path) = bundle_artifacts.proof_check_report_path.clone() {
                    artifacts.proof_check_report_path = Some(path);
                }
                proof_summary_path
            } else {
                let proof_summary_path = plan.work_dir.join("verify.proof-summary.json");
                write_verify_proof_summary_artifact(&proof_summary_path, proof_summary_artifact)?;
                Some(proof_summary_path.display().to_string())
            };
            artifacts.verify_proof_summary_path = proof_summary_path;
        }
        Ok(artifacts)
    };

    let status = z3_stdout.lines().next().unwrap_or("").trim();
    let reason_unknown = parse_z3_reason_unknown(&z3_stdout);
    if status.is_empty() && !smt2_has_solver_query(&smt2_path)? {
        let artifacts = finalize_success_artifacts(artifacts)?;
        return write_report_and_exit(
            machine,
            attach_summary(if plan.mode == Mode::Prove {
                VerifyReport::proven(&args.entry, bounds, artifacts)
            } else {
                VerifyReport::verified(plan.mode, &args.entry, bounds, artifacts)
            }),
        );
    }
    match status {
        "unsat" => {
            let artifacts = finalize_success_artifacts(artifacts)?;
            write_report_and_exit(
                machine,
                attach_summary(if plan.mode == Mode::Prove {
                    VerifyReport::proven(&args.entry, bounds, artifacts)
                } else {
                    VerifyReport::verified(plan.mode, &args.entry, bounds, artifacts)
                }),
            )
        }
        "sat" => {
            if plan.mode == Mode::Prove && plan.target.is_async {
                let mut cbmc_args = vec![
                    plan.c_path.display().to_string(),
                    "--function".to_string(),
                    VERIFY_HARNESS_FN.to_string(),
                    "--unwind".to_string(),
                    args.unwind.to_string(),
                    "--unwinding-assertions".to_string(),
                    "--trace".to_string(),
                    "--json-ui".to_string(),
                ];
                maybe_disable_cbmc_standard_checks(&mut cbmc_args);
                let (out, used_cbmc_args) = run_cbmc_with_object_bits_retry(
                    &cbmc_args,
                    "run cbmc (async counterexample capture)",
                )?;
                if !out.stderr.is_empty() && !cbmc_stderr_is_benign(&out.stderr) {
                    let msg = summarize_process_text(&out.stderr, PROCESS_SUMMARY_MAX_CHARS);
                    let d =
                        diag_verify("X07V_ECBMC_STDERR", format!("cbmc wrote to stderr: {msg}"));
                    return write_report_and_exit(
                        machine,
                        attach_summary(
                            VerifyReport::error(plan.mode, &args.entry, bounds, d, 1)
                                .with_artifacts(artifacts),
                        ),
                    );
                }
                let cbmc_json: Value = match serde_json::from_slice(&out.stdout) {
                    Ok(v) => v,
                    Err(err) => {
                        let d = diag_verify(
                            "X07V_ECBMC_JSON_PARSE",
                            format!("failed to parse cbmc --json-ui output: {err}"),
                        );
                        return write_report_and_exit(
                            machine,
                            attach_summary(
                                VerifyReport::error(plan.mode, &args.entry, bounds, d, 1)
                                    .with_artifacts(artifacts),
                            ),
                        );
                    }
                };
                let cbmc_json_path = plan.work_dir.join("cbmc.json");
                let cbmc_json_bytes = report_common::canonical_pretty_json_bytes(&cbmc_json)
                    .context("canon cbmc.json")?;
                util::write_atomic(&cbmc_json_path, cbmc_json_bytes.as_slice())
                    .with_context(|| format!("write cbmc output: {}", cbmc_json_path.display()))?;
                artifacts.cbmc_json_path = Some(cbmc_json_path.display().to_string());

                let failures = cbmc_failures(&cbmc_json);
                if let Some(contract_failure) = failures.iter().find_map(parse_contract_failure) {
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
                        input_bytes_b64: base64::engine::general_purpose::STANDARD
                            .encode(&input_bytes),
                        contract: contract_failure.payload.clone(),
                        cbmc: CbmcInfo {
                            version: cbmc_program_version(&cbmc_json),
                            argv: std::iter::once("cbmc".to_string())
                                .chain(used_cbmc_args)
                                .collect(),
                            exit_code: out.status.code().unwrap_or(-1),
                            stdout_json_path: "cbmc.json".to_string(),
                            stdout_json_sha256: util::sha256_hex(cbmc_json_bytes.as_slice()),
                        },
                    };
                    let cex_path = plan.work_dir.join("cex.json");
                    let cex_bytes = verify_cex_to_pretty_canon_bytes(&cex)?;
                    util::write_atomic(&cex_path, &cex_bytes)
                        .with_context(|| format!("write verify cex: {}", cex_path.display()))?;
                    artifacts.cex_path = Some(cex_path.display().to_string());

                    let diag = async_counterexample_diag(&contract_failure.payload);
                    return write_report_and_exit(
                        machine,
                        attach_summary(VerifyReport {
                            schema_version: X07_VERIFY_REPORT_SCHEMA_VERSION,
                            mode: plan.mode.as_str(),
                            ok: false,
                            entry: args.entry.clone(),
                            bounds,
                            result: VerifyResult {
                                kind: "counterexample_found".to_string(),
                                contract: Some(contract_failure.payload),
                                details: None,
                            },
                            proof_summary: None,
                            coverage: None,
                            artifacts: Some(artifacts),
                            diagnostics_count: 1,
                            diagnostics: vec![diag],
                            exit_code: 10,
                        }),
                    );
                }
            }
            write_report_and_exit(
                machine,
                attach_summary(VerifyReport::counterexample_found(
                    plan.mode,
                    &args.entry,
                    bounds,
                    diag_verify("X07V_SMT_SAT", "solver reported SAT (counterexample found)"),
                    artifacts,
                    10,
                )),
            )
        }
        other => {
            let (code, message) =
                if other == "unknown" && reason_unknown.as_deref() == Some("timeout") {
                    (
                        "X07V_SMT_TIMEOUT",
                        "solver returned \"unknown\" (reason=timeout)".to_string(),
                    )
                } else {
                    ("X07V_SMT_UNKNOWN", format!("solver returned {other:?}"))
                };
            write_report_and_exit(
                machine,
                attach_summary(VerifyReport::inconclusive(
                    plan.mode,
                    &args.entry,
                    bounds,
                    diag_verify(code, message),
                    artifacts,
                    2,
                )),
            )
        }
    }
}

fn async_counterexample_diag(payload: &Value) -> x07c::diagnostics::Diagnostic {
    match payload
        .get("contract_kind")
        .and_then(Value::as_str)
        .unwrap_or("")
    {
        "scope_invariant" => diag_verify(
            "X07V_SCOPE_INVARIANT_FAILED",
            "async scope invariant failed under the scheduler model",
        ),
        "cancellation_ensures" => diag_verify(
            "X07V_CANCELLATION_ENSURE_FAILED",
            "async cancellation ensure failed under the scheduler model",
        ),
        _ => diag_verify(
            "X07V_ASYNC_COUNTEREXAMPLE",
            "async counterexample found under the scheduler model",
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

fn ensure_smt2_reason_unknown_query(path: &Path) -> Result<()> {
    if !smt2_has_solver_query(path)? {
        return Ok(());
    }
    let raw = std::fs::read(path).with_context(|| format!("read smt2: {}", path.display()))?;
    let mut text = String::from_utf8(raw).context("SMT2 output is not valid UTF-8")?;
    if text.contains(":reason-unknown") {
        return Ok(());
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("(get-info :reason-unknown)\n");
    util::write_atomic(path, text.as_bytes())
        .with_context(|| format!("write smt2: {}", path.display()))?;
    Ok(())
}

fn parse_z3_reason_unknown(stdout: &str) -> Option<String> {
    for line in stdout.lines().skip(1) {
        let line = line.trim();
        if !line.starts_with("(:reason-unknown") {
            continue;
        }
        let start = line.find('"')?;
        let end = line.rfind('"')?;
        if end <= start {
            return None;
        }
        let reason = &line[start + 1..end];
        if reason.is_empty() {
            return None;
        }
        return Some(reason.to_string());
    }
    None
}

fn smt2_has_solver_query(path: &Path) -> Result<bool> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("read smt2: {}", path.display()))?;
    Ok(text
        .lines()
        .any(|line| line.trim_start().starts_with("(check-sat")))
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
        return Ok(normalize_module_roots(explicit.to_vec()));
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
        if let Some(toolchain_root) = util::detect_toolchain_root_best_effort(cwd) {
            for root in util::toolchain_stdlib_module_roots(&toolchain_root) {
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
        return Ok(normalize_module_roots(roots));
    }

    let mut roots = vec![cwd.to_path_buf()];
    if let Some(toolchain_root) = util::detect_toolchain_root_best_effort(cwd) {
        for root in util::toolchain_stdlib_module_roots(&toolchain_root) {
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    Ok(normalize_module_roots(roots))
}

fn normalize_module_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(roots.len());
    let mut seen = BTreeSet::new();
    for root in roots {
        let normalized = std::fs::canonicalize(&root).unwrap_or(root);
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
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

fn report_source_path(source_path: &Path, project_root: Option<&Path>) -> String {
    if let Some(project_root) = project_root {
        if source_path.is_absolute() {
            if let Ok(rel) = source_path.strip_prefix(project_root) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        } else {
            return source_path.to_string_lossy().replace('\\', "/");
        }
    }
    source_path.to_string_lossy().replace('\\', "/")
}

#[derive(Debug, Clone)]
struct TargetSig {
    param_names: Vec<String>,
    params: Vec<VerifySignatureParam>,
    result: String,
    result_brand: Option<String>,
    decl_sha256_hex: String,
    is_async: bool,
    has_contracts: bool,
    decreases_count: usize,
    decreases: Vec<Value>,
    body: Value,
    source_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecursionKind {
    None,
    SelfRecursive,
    Mutual,
}

#[derive(Debug, Clone)]
struct RecursionSummary {
    kind: RecursionKind,
    cycle_symbol: Option<String>,
}

impl RecursionSummary {
    fn kind_str(&self) -> &'static str {
        match self.kind {
            RecursionKind::None => "none",
            RecursionKind::SelfRecursive => "self_recursive",
            RecursionKind::Mutual => "mutual",
        }
    }
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
        let mut param_names = Vec::with_capacity(params.len());
        let mut out = Vec::with_capacity(params.len());
        for p in params {
            param_names.push(
                p.get("name")
                    .and_then(Value::as_str)
                    .context("param missing name")?
                    .to_string(),
            );
            let ty = p
                .get("ty")
                .and_then(Value::as_str)
                .context("param missing ty")?;
            out.push(VerifySignatureParam {
                ty: ty.to_string(),
                brand: p.get("brand").and_then(Value::as_str).map(str::to_string),
            });
        }
        let has_contracts = has_any_contracts(d);
        let body = d.get("body").cloned().context("defn missing body")?;
        let decreases = d
            .get("decreases")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("expr").cloned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        return Ok(TargetSig {
            param_names,
            params: out,
            result: d
                .get("result")
                .and_then(Value::as_str)
                .context("defn missing result")?
                .to_string(),
            result_brand: d
                .get("result_brand")
                .and_then(Value::as_str)
                .map(str::to_string),
            decl_sha256_hex: decl_sha256_hex_for_value(d)?,
            is_async: kind == "defasync",
            has_contracts,
            decreases_count: decreases.len(),
            decreases,
            body,
            source_path: source
                .path
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("{module_id}.x07.json"))),
        });
    }

    anyhow::bail!("could not find function {entry:?} in resolved module {module_id:?}")
}

fn load_verify_brand_module<'a>(
    module_roots: &[PathBuf],
    module_id: &str,
    cache: &'a mut BTreeMap<String, VerifyBrandModule>,
) -> Result<&'a VerifyBrandModule> {
    if !cache.contains_key(module_id) {
        let source =
            x07c::module_source::load_module_source(module_id, WorldId::SolvePure, module_roots)
                .map_err(|err| anyhow::anyhow!(err.message.to_string()))?;
        let doc: Value = serde_json::from_str(&source.src)
            .with_context(|| format!("parse module JSON for {module_id:?}"))?;
        let imports = doc
            .get("imports")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let meta = doc
            .get("meta")
            .and_then(Value::as_object)
            .map(|obj| {
                obj.iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let validators = x07c::stream_pipe::brand_registry_from_meta_v1(&meta)
            .map_err(|err| anyhow::anyhow!(err.message.to_string()))?;
        cache.insert(
            module_id.to_string(),
            VerifyBrandModule {
                imports,
                validators,
            },
        );
    }
    Ok(cache
        .get(module_id)
        .expect("verify brand module inserted into cache"))
}

fn resolve_verify_brand_validator(
    module_roots: &[PathBuf],
    entry: &str,
    brand_id: &str,
) -> Result<String> {
    let (module_id, _) = entry.rsplit_once('.').context("--entry must contain '.'")?;
    let mut cache = BTreeMap::new();
    let mut queue = VecDeque::from([module_id.to_string()]);
    let mut visited = BTreeSet::new();
    let mut matches = BTreeSet::new();

    while let Some(current) = queue.pop_front() {
        if !visited.insert(current.clone()) {
            continue;
        }
        let module = load_verify_brand_module(module_roots, &current, &mut cache)?;
        if let Some(validator) = module.validators.get(brand_id) {
            matches.insert(validator.clone());
        }
        for import in &module.imports {
            queue.push_back(import.clone());
        }
    }

    match matches.len() {
        1 => Ok(matches.into_iter().next().expect("single validator match")),
        0 => anyhow::bail!(
            "brand {:?} is missing meta.brands_v1.validate in the reachable module graph for {:?}",
            brand_id,
            entry
        ),
        _ => anyhow::bail!(
            "brand {:?} resolves to multiple validators in the reachable module graph for {:?}: {:?}",
            brand_id,
            entry,
            matches.into_iter().collect::<Vec<_>>()
        ),
    }
}

fn decl_sha256_hex_for_value(value: &Value) -> Result<String> {
    let bytes = report_common::canonical_pretty_json_bytes(value)?;
    Ok(util::sha256_hex(&bytes))
}

fn load_imported_summary_index(
    cwd: &Path,
    paths: &[PathBuf],
) -> std::result::Result<ImportedSummaryIndex, Vec<x07c::diagnostics::Diagnostic>> {
    let mut index = ImportedSummaryIndex::default();
    for raw_path in paths {
        let path = if raw_path.is_absolute() {
            raw_path.clone()
        } else {
            cwd.join(raw_path)
        };
        let bytes = std::fs::read(&path).map_err(|err| {
            vec![diag_verify(
                "X07V_SUMMARY_MISMATCH",
                format!("read verify proof summary {}: {err:#}", path.display()),
            )]
        })?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|err| {
            vec![diag_verify(
                "X07V_SUMMARY_MISMATCH",
                format!(
                    "parse verify proof summary JSON {}: {err:#}",
                    path.display()
                ),
            )]
        })?;
        let diags = validate_verify_proof_summary_schema(&value).map_err(|err| {
            vec![diag_verify(
                "X07V_SUMMARY_MISMATCH",
                format!(
                    "validate verify proof-summary schema {}: {err:#}",
                    path.display()
                ),
            )]
        })?;
        if !diags.is_empty() {
            let coverage_diags = validate_verify_summary_schema(&value).map_err(|err| {
                vec![diag_verify(
                    "X07V_SUMMARY_MISMATCH",
                    format!(
                        "validate verify coverage-summary schema {}: {err:#}",
                        path.display()
                    ),
                )]
            })?;
            if coverage_diags.is_empty() {
                return Err(vec![
                    diag_verify_warning(
                        "X07V_COVERAGE_NOT_PROOF",
                        format!(
                            "coverage/support summary {} is posture-only and does not count as proof evidence",
                            path.display()
                        ),
                    ),
                    diag_verify(
                        "X07V_COVERAGE_SUMMARY_FORBIDDEN",
                        format!(
                            "coverage/support summary {} cannot be imported via --proof-summary; use a proof summary emitted by `x07 verify --prove`",
                            path.display()
                        ),
                    ),
                ]);
            }
            return Err(vec![diag_verify(
                "X07V_SUMMARY_MISMATCH",
                format!(
                    "verify proof summary schema invalid for {}: {}",
                    path.display(),
                    diags[0].message
                ),
            )]);
        }
        let summary: VerifyProofSummaryArtifact = serde_json::from_value(value).map_err(|err| {
            vec![diag_verify(
                "X07V_SUMMARY_MISMATCH",
                format!(
                    "decode verify proof summary JSON {}: {err:#}",
                    path.display()
                ),
            )]
        })?;
        if summary.schema_version != X07_VERIFY_PROOF_SUMMARY_SCHEMA_VERSION {
            return Err(vec![diag_verify(
                "X07V_SUMMARY_MISMATCH",
                format!(
                    "verify proof summary schema_version mismatch for {}: expected {:?} got {:?}",
                    path.display(),
                    X07_VERIFY_PROOF_SUMMARY_SCHEMA_VERSION,
                    summary.schema_version
                ),
            )]);
        }
        let symbols = vec![summary.symbol.clone()];
        let source = VerifyImportedSummaryRef {
            path: path.display().to_string(),
            sha256_hex: util::sha256_hex(&bytes),
            symbols,
        };
        let symbol = summary.symbol.clone();
        if symbol.trim().is_empty() {
            continue;
        }
        if index.by_symbol.contains_key(&symbol) {
            return Err(vec![diag_verify(
                "X07V_SUMMARY_MISMATCH",
                format!(
                    "duplicate imported verify proof summary for symbol {:?} via {}",
                    symbol,
                    path.display()
                ),
            )]);
        }
        index.by_symbol.insert(
            symbol,
            ImportedSummaryFunction {
                function: summary,
                source: source.clone(),
            },
        );
        index.inventory.push(source);
    }
    index.inventory.sort_by(|a, b| {
        (a.path.as_str(), a.sha256_hex.as_str()).cmp(&(b.path.as_str(), b.sha256_hex.as_str()))
    });
    Ok(index)
}

fn add_used_imported_summary(
    used_imported_summaries: &mut BTreeMap<String, VerifyImportedSummaryRef>,
    source: &VerifyImportedSummaryRef,
    symbol: &str,
) {
    let entry = used_imported_summaries
        .entry(source.path.clone())
        .or_insert_with(|| VerifyImportedSummaryRef {
            path: source.path.clone(),
            sha256_hex: source.sha256_hex.clone(),
            symbols: Vec::new(),
        });
    if !entry.symbols.iter().any(|existing| existing == symbol) {
        entry.symbols.push(symbol.to_string());
        entry.symbols.sort();
    }
}

fn coverage_function_from_imported_summary(
    symbol: &str,
    imported: &ImportedSummaryFunction,
    signature: Option<VerifyFunctionSignature>,
    decl_sha256_hex: Option<String>,
    source_path: Option<String>,
) -> VerifyCoverageFunction {
    let status = if imported.function.result_kind == "proven_async" {
        "supported_async".to_string()
    } else {
        "imported_proof_summary".to_string()
    };
    VerifyCoverageFunction {
        symbol: symbol.to_string(),
        kind: imported.function.kind.clone(),
        status,
        signature,
        support_summary: Some(VerifyFunctionSupportSummary {
            recursion_kind: imported.function.recursion_kind.clone(),
            has_decreases: false,
            decreases_count: 0,
            prove_supported: true,
        }),
        decl_sha256_hex,
        source_path,
        details: Some(format!(
            "proof summary imported from {}",
            imported.source.path
        )),
    }
}

fn summary_mismatch_function_for_decl(
    symbol: &str,
    decl: &CoverageDecl,
    imported: &ImportedSummaryFunction,
    project_root: Option<&Path>,
) -> VerifyCoverageFunction {
    VerifyCoverageFunction {
        symbol: symbol.to_string(),
        kind: decl.kind.clone(),
        status: "unsupported".to_string(),
        signature: Some(verify_function_signature(
            &decl.params,
            &decl.result,
            decl.result_brand.as_deref(),
        )),
        support_summary: Some(VerifyFunctionSupportSummary {
            recursion_kind: imported.function.recursion_kind.clone(),
            has_decreases: false,
            decreases_count: 0,
            prove_supported: true,
        }),
        decl_sha256_hex: Some(decl.decl_sha256_hex.clone()),
        source_path: Some(report_source_path(&decl.source_path, project_root)),
        details: Some(format!(
            "imported proof summary from {} does not match the current declaration",
            imported.source.path
        )),
    }
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
                .and_then(|params| {
                    params
                        .iter()
                        .map(|p| {
                            Some((
                                p.get("name").and_then(Value::as_str)?.to_string(),
                                VerifySignatureParam {
                                    ty: p.get("ty").and_then(Value::as_str)?.to_string(),
                                    brand: p
                                        .get("brand")
                                        .and_then(Value::as_str)
                                        .map(str::to_string),
                                },
                            ))
                        })
                        .collect::<Option<Vec<_>>>()
                })
                .unwrap_or_default();
            let param_names = params
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>();
            let params = params
                .into_iter()
                .map(|(_, param)| param)
                .collect::<Vec<_>>();
            let decreases = decl
                .get("decreases")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.get("expr").cloned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            out_decls.insert(
                name.to_string(),
                CoverageDecl {
                    kind: kind.to_string(),
                    param_names,
                    params,
                    result: decl
                        .get("result")
                        .and_then(Value::as_str)
                        .unwrap_or("i32")
                        .to_string(),
                    result_brand: decl
                        .get("result_brand")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    decl_sha256_hex: decl_sha256_hex_for_value(&decl)?,
                    has_contracts: has_any_contracts(&decl),
                    decreases_count: decreases.len(),
                    decreases,
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

fn load_verify_primitive_catalog() -> Result<BTreeMap<String, VerifyPrimitiveEntry>> {
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
        out.insert(primitive.symbol.clone(), primitive);
    }
    Ok(out)
}

fn load_verify_scheduler_model() -> Result<VerifySchedulerModel> {
    let doc: VerifySchedulerModel = serde_json::from_slice(X07_VERIFY_SCHEDULER_MODEL_BYTES)
        .context("parse catalog/verify_scheduler_model.json")?;
    if doc.schema_version.trim() != "x07.verify.scheduler_model@0.1.0" {
        anyhow::bail!(
            "verify scheduler model schema_version mismatch: expected {:?} got {:?}",
            "x07.verify.scheduler_model@0.1.0",
            doc.schema_version
        );
    }
    if doc.id.trim().is_empty() {
        anyhow::bail!("verify scheduler model id must not be empty");
    }
    if doc.guarantees.is_empty() {
        anyhow::bail!("verify scheduler model must declare at least one guarantee");
    }
    Ok(doc)
}

fn load_coverage_trust_zone_index(
    project_path: Option<&Path>,
) -> Result<Option<CoverageTrustZoneIndex>> {
    let Some(project_path) = project_path else {
        return Ok(None);
    };
    let Some(project_root) = project_path.parent() else {
        return Ok(None);
    };
    let manifest_path = project_root.join("arch").join("manifest.x07arch.json");
    if !manifest_path.is_file() {
        return Ok(None);
    }

    let doc = report_common::read_json_file(&manifest_path)?;
    let mut nodes = Vec::new();
    for node in doc
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let trust_zone = node
            .get("trust_zone")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if trust_zone != "certified_capsule" {
            continue;
        }
        let id = node
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown_capsule")
            .to_string();
        let module_prefixes = node
            .pointer("/match/module_prefixes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut builder = GlobSetBuilder::new();
        let mut has_globs = false;
        for glob in node
            .pointer("/match/path_globs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            builder.add(Glob::new(glob).with_context(|| {
                format!(
                    "invalid capsule path_glob {glob:?} in {}",
                    manifest_path.display()
                )
            })?);
            has_globs = true;
        }
        let path_globs = if has_globs {
            builder
                .build()
                .context("build capsule trust-zone globset")?
        } else {
            GlobSetBuilder::new()
                .build()
                .context("build empty capsule trust-zone globset")?
        };
        nodes.push(CoverageTrustZoneNode {
            id,
            module_prefixes,
            path_globs,
            trust_zone,
        });
    }

    Ok(Some(CoverageTrustZoneIndex {
        project_root: project_root.to_path_buf(),
        nodes,
    }))
}

fn capsule_boundary_node<'a>(
    index: Option<&'a CoverageTrustZoneIndex>,
    symbol: &str,
    source_path: &Path,
) -> Option<&'a CoverageTrustZoneNode> {
    let index = index?;
    let module_id = symbol.rsplit_once('.').map(|(m, _)| m).unwrap_or(symbol);
    let rel_path = if source_path.is_absolute() {
        source_path
            .strip_prefix(&index.project_root)
            .ok()
            .map(|p| p.to_path_buf())
    } else {
        Some(source_path.to_path_buf())
    }?;
    let rel_str = rel_path.to_string_lossy().replace('\\', "/");
    index.nodes.iter().find(|node| {
        node.trust_zone == "certified_capsule"
            && (node
                .module_prefixes
                .iter()
                .any(|prefix| module_id == prefix || module_id.starts_with(&format!("{prefix}.")))
                || node.path_globs.is_match(rel_str.as_str()))
    })
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

fn decl_refs(module_id: &str, module: &CoverageModule, decl: &CoverageDecl) -> BTreeSet<String> {
    let mut queue = VecDeque::new();
    enqueue_decl_refs(module_id, module, decl, &mut queue);
    queue.into_iter().collect()
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
    if let Some(protocol) = defn.get("protocol") {
        for key in ["await_invariant", "scope_invariant", "cancellation_ensures"] {
            let Some(clauses) = protocol.get(key).and_then(Value::as_array) else {
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
    }
    out
}

fn recursion_summary_for_symbol(
    module_roots: &[PathBuf],
    world: WorldId,
    entry: &str,
) -> Result<RecursionSummary> {
    let mut module_cache: BTreeMap<String, CoverageModule> = BTreeMap::new();
    let mut queue = VecDeque::from([entry.to_string()]);
    let mut visited = BTreeSet::new();
    let mut saw_self_recursion = false;
    let mut cycle_symbol = None;

    while let Some(symbol) = queue.pop_front() {
        if !visited.insert(symbol.clone()) {
            continue;
        }
        let Some((module_id, _)) = symbol.rsplit_once('.') else {
            continue;
        };
        let module = match load_coverage_module(module_roots, world, module_id, &mut module_cache) {
            Ok(module) => module,
            Err(_) => continue,
        };
        let Some(decl) = module.decls.get(&symbol) else {
            continue;
        };
        if decl.kind == "extern" {
            continue;
        }

        let refs = decl_refs(module_id, module, decl);
        if symbol == entry && refs.contains(entry) {
            saw_self_recursion = true;
        }
        if symbol != entry && refs.contains(entry) {
            cycle_symbol = Some(symbol);
        }
        for next in refs {
            queue.push_back(next);
        }
    }

    Ok(if let Some(symbol) = cycle_symbol {
        RecursionSummary {
            kind: RecursionKind::Mutual,
            cycle_symbol: Some(symbol),
        }
    } else if saw_self_recursion {
        RecursionSummary {
            kind: RecursionKind::SelfRecursive,
            cycle_symbol: None,
        }
    } else {
        RecursionSummary {
            kind: RecursionKind::None,
            cycle_symbol: None,
        }
    })
}

fn supported_verify_param_type(ty: &str) -> bool {
    matches!(
        ty,
        "i32"
            | "u32"
            | "bytes"
            | "bytes_view"
            | "vec_u8"
            | "option_i32"
            | "option_bytes"
            | "option_bytes_view"
            | "result_i32"
            | "result_bytes"
            | "result_bytes_view"
    )
}

fn supported_verify_result_type(ty: &str) -> bool {
    supported_verify_param_type(ty)
}

fn encoded_verify_param_bytes(param: &VerifySignatureParam, max_bytes_len: u32) -> Result<u64> {
    match param.ty.as_str() {
        "i32" | "u32" => Ok(4),
        "bytes" | "bytes_view" | "vec_u8" => Ok(4u64 + max_bytes_len as u64),
        "option_i32" | "result_i32" => Ok(5),
        "option_bytes" | "option_bytes_view" | "result_bytes" | "result_bytes_view" => {
            Ok(1u64 + 4u64 + max_bytes_len as u64)
        }
        other => anyhow::bail!("unsupported verify type {other:?}"),
    }
}

fn verify_brand_supported_carrier(ty: &str) -> bool {
    matches!(ty, "bytes_view" | "option_bytes_view" | "result_bytes_view")
}

fn verify_driver_raw_param(param: &VerifySignatureParam) -> Result<VerifySignatureParam> {
    let ty = match param.ty.as_str() {
        "i32" | "u32" | "bytes_view" | "option_i32" | "result_i32" => param.ty.as_str(),
        "bytes" | "vec_u8" => "bytes_view",
        "option_bytes" | "option_bytes_view" => "option_bytes_view",
        "result_bytes" | "result_bytes_view" => "result_bytes_view",
        other => anyhow::bail!("unsupported verify param type {other:?}"),
    };
    Ok(VerifySignatureParam {
        ty: ty.to_string(),
        brand: None,
    })
}

fn verify_compile_param(
    param: &VerifySignatureParam,
    normalize_for_overlay: bool,
) -> VerifySignatureParam {
    if normalize_for_overlay && param.ty == "vec_u8" {
        VerifySignatureParam {
            ty: "bytes".to_string(),
            brand: param.brand.clone(),
        }
    } else {
        param.clone()
    }
}

fn find_signature_rich_type_unsupported(
    module_roots: &[PathBuf],
    entry: &str,
    target: &TargetSig,
) -> Option<String> {
    for (idx, param) in target.params.iter().enumerate() {
        if target.is_async && (param.brand.is_some() || param.ty == "vec_u8") {
            let param_name = target
                .param_names
                .get(idx)
                .map(String::as_str)
                .unwrap_or("__arg");
            return Some(format!(
                "x07 verify defasync proof inputs keep the async line on unbranded byte carriers; param {:?} uses {:?}",
                param_name, param.ty
            ));
        }
        if param.brand.is_some() {
            if !verify_brand_supported_carrier(&param.ty) {
                return Some(format!(
                    "x07 verify currently supports proof-input brands on bytes_view and its option/result view carriers, got {:?}",
                    param.ty
                ));
            }
            if let Err(err) = resolve_verify_brand_validator(
                module_roots,
                entry,
                param.brand.as_deref().expect("brand checked"),
            ) {
                return Some(err.to_string());
            }
        }
        if !supported_verify_param_type(&param.ty) {
            return Some(format!(
                "x07 verify does not support param type {:?} in the certifiable richer-data subset",
                param.ty
            ));
        }
        if let Err(err) = verify_driver_raw_param(param) {
            return Some(err.to_string());
        }
    }
    if !supported_verify_result_type(&target.result) {
        return Some(format!(
            "x07 verify does not support result type {:?} in the certifiable richer-data subset",
            target.result
        ));
    }
    None
}

fn find_unsupported_heap_effect(body: &Value) -> Option<String> {
    match body {
        Value::Array(items) => {
            if let Some(head) = items.first().and_then(Value::as_str) {
                if matches!(
                    head,
                    "unsafe"
                        | "addr_of"
                        | "addr_of_mut"
                        | "bytes.set_u8"
                        | "bytes.as_ptr"
                        | "bytes.as_mut_ptr"
                        | "view.as_ptr"
                        | "vec_u8.as_ptr"
                        | "vec_u8.as_mut_ptr"
                        | "ptr.null"
                        | "ptr.as_const"
                        | "ptr.cast"
                        | "ptr.add"
                        | "ptr.sub"
                        | "ptr.offset"
                        | "ptr.read_u8"
                        | "ptr.write_u8"
                        | "ptr.read_i32"
                        | "ptr.write_i32"
                        | "memcpy"
                        | "memmove"
                        | "memset"
                ) {
                    return Some(format!(
                        "x07 verify does not support heap/pointer effect {:?} in the certifiable pure subset",
                        head
                    ));
                }
            }
            for item in items {
                if let Some(msg) = find_unsupported_heap_effect(item) {
                    return Some(msg);
                }
            }
            None
        }
        Value::Object(obj) => obj.values().find_map(find_unsupported_heap_effect),
        _ => None,
    }
}

fn compute_input_len_bytes(sig: &TargetSig, max_bytes_len: u32) -> Result<u32> {
    let mut total: u64 = 0;
    for param in &sig.params {
        total = total.saturating_add(encoded_verify_param_bytes(param, max_bytes_len)?);
    }
    if total > u32::MAX as u64 {
        anyhow::bail!("verify input encoding too large: {total} bytes");
    }
    Ok(total as u32)
}

fn prove_unsupported_reason(
    module_roots: &[PathBuf],
    target: &TargetSig,
    entry: &str,
    max_bytes_len: u32,
    recursion: &RecursionSummary,
) -> Option<(&'static str, String)> {
    if !target.has_contracts {
        return Some((
            "X07V_NO_CONTRACTS",
            "target function has no requires/ensures/invariant clauses".to_string(),
        ));
    }
    match recursion.kind {
        RecursionKind::None => {}
        RecursionKind::SelfRecursive => {
            if target.is_async {
                return Some((
                    "X07V_UNSUPPORTED_RECURSION",
                    "x07 verify does not support recursive defasync targets".to_string(),
                ));
            }
            if target.decreases_count == 0 {
                return Some((
                    "X07V_RECURSIVE_DECREASES_REQUIRED",
                    "self-recursive targets must declare decreases[] to use x07 verify".to_string(),
                ));
            }
            if let Some(msg) = find_recursive_termination_failure(target, entry) {
                return Some(("X07V_RECURSION_TERMINATION_FAILED", msg));
            }
        }
        RecursionKind::Mutual => {
            return Some((
                "X07V_UNSUPPORTED_MUTUAL_RECURSION",
                format!(
                    "x07 verify does not support mutual recursion involving {:?}",
                    recursion.cycle_symbol.as_deref().unwrap_or(entry)
                ),
            ));
        }
    }
    if let Some(msg) = find_for_with_non_literal_bounds(&target.body) {
        return Some(("X07V_UNSUPPORTED_FOR_BOUNDS", msg));
    }
    if let Some(msg) = find_signature_rich_type_unsupported(module_roots, entry, target) {
        return Some(("X07V_UNSUPPORTED_RICH_TYPE", msg));
    }
    if let Some(msg) = find_unsupported_heap_effect(&target.body) {
        return Some(("X07V_UNSUPPORTED_HEAP_EFFECT", msg));
    }
    if let Err(err) = compute_input_len_bytes(target, max_bytes_len) {
        return Some(("X07V_UNSUPPORTED_RICH_TYPE", err.to_string()));
    }
    if target.is_async && target.result != "bytes" && target.result != "result_bytes" {
        return Some((
            "X07V_UNSUPPORTED_DEFASYNC_FORM",
            format!(
                "defasync target {:?} must return bytes or result_bytes for proof support",
                entry
            ),
        ));
    }
    None
}

fn verify_precheck_diag(
    module_roots: &[PathBuf],
    target: &TargetSig,
    entry: &str,
    max_bytes_len: u32,
    recursion: &RecursionSummary,
) -> Option<x07c::diagnostics::Diagnostic> {
    let (code, msg) =
        prove_unsupported_reason(module_roots, target, entry, max_bytes_len, recursion)?;
    Some(diag_verify(code, msg))
}

fn function_support_summary(
    target: &TargetSig,
    recursion: &RecursionSummary,
    prove_supported: bool,
) -> VerifyFunctionSupportSummary {
    VerifyFunctionSupportSummary {
        recursion_kind: recursion.kind_str().to_string(),
        has_decreases: target.decreases_count != 0,
        decreases_count: target.decreases_count as u64,
        prove_supported,
    }
}

fn verify_function_signature(
    params: &[VerifySignatureParam],
    result: &str,
    result_brand: Option<&str>,
) -> VerifyFunctionSignature {
    VerifyFunctionSignature {
        params: params.to_vec(),
        result: result.to_string(),
        result_brand: result_brand.map(str::to_string),
    }
}

fn report_proof_summary(
    coverage: &VerifyCoverage,
    target: &TargetSig,
    recursion: &RecursionSummary,
) -> VerifyProofSummary {
    let mut dependency_symbols = coverage
        .functions
        .iter()
        .filter_map(|function| {
            if function.symbol == coverage.entry {
                None
            } else {
                Some(function.symbol.clone())
            }
        })
        .collect::<Vec<_>>();
    dependency_symbols.sort();
    dependency_symbols.dedup();
    VerifyProofSummary {
        engine: "cbmc_z3".to_string(),
        recursion_kind: recursion.kind_str().to_string(),
        has_decreases: target.decreases_count != 0,
        decreases_count: target.decreases_count as u64,
        bounded_by_unwind: recursion.kind != RecursionKind::None,
        recursion_bound_kind: if recursion.kind == RecursionKind::None {
            "none".to_string()
        } else {
            "bounded_by_unwind".to_string()
        },
        dependency_symbols,
    }
}

fn imported_summary_digest_for_symbol(
    imported_summaries: &[VerifyImportedSummaryRef],
    symbol: &str,
) -> Option<String> {
    imported_summaries
        .iter()
        .find(|entry| entry.symbols.iter().any(|candidate| candidate == symbol))
        .map(|entry| format!("sha256:{}", entry.sha256_hex))
}

fn build_proof_assumptions(
    coverage: &VerifyCoverage,
    imported_summaries: &[VerifyImportedSummaryRef],
    primitive_catalog: &BTreeMap<String, VerifyPrimitiveEntry>,
) -> Vec<VerifyProofAssumption> {
    let mut assumptions = BTreeSet::new();
    for function in &coverage.functions {
        if function.symbol == coverage.entry {
            continue;
        }
        let assumption = match function.status.as_str() {
            "trusted_primitive" => {
                primitive_catalog
                    .get(&function.symbol)
                    .map(|primitive| VerifyProofAssumption {
                        kind: if primitive.kind == "builtin" {
                            "trusted_builtin".to_string()
                        } else if primitive.assumption_class == "dev_stub" {
                            "imported_stub".to_string()
                        } else {
                            "trusted_builtin".to_string()
                        },
                        subject: function.symbol.clone(),
                        digest: None,
                        certifiable: primitive.certification_policy == "allowed",
                    })
            }
            "imported_proof_summary" => Some(VerifyProofAssumption {
                kind: "imported_proof_summary".to_string(),
                subject: function.symbol.clone(),
                digest: imported_summary_digest_for_symbol(imported_summaries, &function.symbol),
                certifiable: true,
            }),
            "trusted_scheduler_model" => Some(VerifyProofAssumption {
                kind: "trusted_scheduler_model".to_string(),
                subject: function.symbol.clone(),
                digest: None,
                certifiable: true,
            }),
            "capsule_boundary" => Some(VerifyProofAssumption {
                kind: "capsule_boundary".to_string(),
                subject: function.symbol.clone(),
                digest: None,
                certifiable: true,
            }),
            _ => None,
        };
        if let Some(assumption) = assumption {
            assumptions.insert(assumption);
        }
    }
    assumptions.into_iter().collect()
}

fn build_verify_proof_summary_artifact(
    coverage: &VerifyCoverage,
    imported_summaries: &[VerifyImportedSummaryRef],
    primitive_catalog: &BTreeMap<String, VerifyPrimitiveEntry>,
    target: &TargetSig,
    recursion: &RecursionSummary,
) -> VerifyProofSummaryArtifact {
    let report_summary = report_proof_summary(coverage, target, recursion);
    VerifyProofSummaryArtifact {
        schema_version: X07_VERIFY_PROOF_SUMMARY_SCHEMA_VERSION.to_string(),
        summary_kind: "proof".to_string(),
        symbol: coverage.entry.clone(),
        kind: if target.is_async {
            "defasync".to_string()
        } else {
            "defn".to_string()
        },
        decl_sha256_hex: target.decl_sha256_hex.clone(),
        result_kind: if target.is_async {
            "proven_async".to_string()
        } else {
            "proven".to_string()
        },
        engine: report_summary.engine,
        recursion_kind: report_summary.recursion_kind,
        recursion_bound_kind: report_summary.recursion_bound_kind,
        dependency_symbols: report_summary.dependency_symbols,
        proof_object_digest: None,
        assumptions: build_proof_assumptions(coverage, imported_summaries, primitive_catalog),
    }
}

fn cmd_verify_coverage(
    machine: &crate::reporting::MachineArgs,
    args: &VerifyArgs,
    project_path: Option<&Path>,
    module_roots: &[PathBuf],
    target: &TargetSig,
    imported_summary_index: &ImportedSummaryIndex,
    artifact_base: &Path,
) -> Result<std::process::ExitCode> {
    let analysis = match coverage_report_for_entry(
        args,
        project_path,
        target,
        imported_summary_index,
        false,
    ) {
        Ok(analysis) => analysis,
        Err(err) => coverage_report_fallback(
            module_roots,
            &args.entry,
            project_path,
            target,
            args.max_bytes_len,
            Some(format!("could not materialize reachable closure: {err:#}")),
        ),
    };
    let work_dir = artifact_base
        .join("verify")
        .join("coverage")
        .join(util::safe_artifact_dir_name(&args.entry));
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("create artifact dir: {}", work_dir.display()))?;
    let verify_coverage_summary_path = work_dir.join("verify.summary.json");
    write_verify_summary_artifact(
        &verify_coverage_summary_path,
        &analysis.coverage,
        &analysis.imported_summaries,
    )?;
    write_report_and_exit(
        machine,
        VerifyReport::coverage_report(&args.entry, Bounds::for_args(args), analysis.coverage)
            .with_artifacts(Artifacts {
                verify_coverage_summary_path: Some(
                    verify_coverage_summary_path.display().to_string(),
                ),
                ..Artifacts::default()
            })
            .with_diagnostics(analysis.diagnostics),
    )
}

fn missing_summary_code(proof_summaries_required: bool) -> &'static str {
    if proof_summaries_required {
        "X07V_PROOF_SUMMARY_REQUIRED"
    } else {
        "X07V_SUMMARY_MISSING"
    }
}

fn missing_summary_label(proof_summaries_required: bool) -> &'static str {
    if proof_summaries_required {
        "proof summary"
    } else {
        "summary"
    }
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
    module_roots: &[PathBuf],
    entry: &str,
    target: &TargetSig,
    max_bytes_len: u32,
    recursion: &RecursionSummary,
    project_root: Option<&Path>,
) -> VerifyCoverageFunction {
    let (kind, status, details) = if target.is_async {
        if !target.has_contracts {
            (
                "defasync".to_string(),
                "uncovered".to_string(),
                Some(
                    "target function has no requires/ensures/invariant/protocol clauses"
                        .to_string(),
                ),
            )
        } else if let Some((_, msg)) =
            prove_unsupported_reason(module_roots, target, entry, max_bytes_len, recursion)
        {
            ("defasync".to_string(), "unsupported".to_string(), Some(msg))
        } else {
            ("defasync".to_string(), "supported_async".to_string(), None)
        }
    } else if !target.has_contracts {
        (
            "defn".to_string(),
            "uncovered".to_string(),
            Some("target function has no requires/ensures/invariant clauses".to_string()),
        )
    } else if let Some((code, msg)) =
        prove_unsupported_reason(module_roots, target, entry, max_bytes_len, recursion)
    {
        if recursion.kind == RecursionKind::SelfRecursive
            && target.decreases_count != 0
            && matches!(
                code,
                "X07V_UNSUPPORTED_RICH_TYPE" | "X07V_UNSUPPORTED_HEAP_EFFECT"
            )
        {
            (
                "defn".to_string(),
                "termination_proven".to_string(),
                Some(format!(
                    "recursive termination posture remains covered by decreases[], but full proof is unsupported: {msg}"
                )),
            )
        } else {
            ("defn".to_string(), "unsupported".to_string(), Some(msg))
        }
    } else if recursion.kind == RecursionKind::SelfRecursive {
        (
            "defn".to_string(),
            "supported_recursive".to_string(),
            Some(format!(
                "self-recursive target with {} decreases clause(s)",
                target.decreases_count
            )),
        )
    } else {
        ("defn".to_string(), "supported".to_string(), None)
    };
    let prove_supported = matches!(
        status.as_str(),
        "supported" | "supported_recursive" | "supported_async"
    );

    VerifyCoverageFunction {
        symbol: entry.to_string(),
        kind,
        status,
        signature: Some(verify_function_signature(
            &target.params,
            &target.result,
            target.result_brand.as_deref(),
        )),
        support_summary: Some(function_support_summary(target, recursion, prove_supported)),
        decl_sha256_hex: Some(target.decl_sha256_hex.clone()),
        source_path: Some(report_source_path(&target.source_path, project_root)),
        details,
    }
}

fn coverage_report_for_entry(
    args: &VerifyArgs,
    project_path: Option<&Path>,
    _target: &TargetSig,
    imported_summary_index: &ImportedSummaryIndex,
    proof_summaries_required: bool,
) -> Result<CoverageAnalysis> {
    let cwd = std::env::current_dir().context("get cwd")?;
    let module_roots = resolve_module_roots(&cwd, project_path, &args.module_root)?;
    let world = coverage_world(project_path);
    let primitive_catalog = load_verify_primitive_catalog()?;
    let trust_zones = load_coverage_trust_zone_index(project_path)?;
    let project_root = project_path.and_then(|p| p.parent());
    let mut module_cache: BTreeMap<String, CoverageModule> = BTreeMap::new();
    let mut queue = VecDeque::from([args.entry.clone()]);
    let mut visited = BTreeSet::new();
    let mut functions = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let mut used_imported_summaries = BTreeMap::new();
    let mut saw_async = false;

    while let Some(symbol) = queue.pop_front() {
        if !visited.insert(symbol.clone()) {
            continue;
        }

        if let Some(primitive) = primitive_catalog.get(&symbol) {
            functions.insert(
                symbol.clone(),
                VerifyCoverageFunction {
                    symbol,
                    kind: if primitive.kind == "builtin" {
                        "builtin".to_string()
                    } else {
                        "imported".to_string()
                    },
                    status: "trusted_primitive".to_string(),
                    signature: None,
                    support_summary: None,
                    decl_sha256_hex: None,
                    source_path: None,
                    details: primitive.note.clone(),
                },
            );
            continue;
        }

        if symbol != args.entry {
            if let Some(imported) = imported_summary_index.by_symbol.get(&symbol) {
                let current_decl = symbol
                    .rsplit_once('.')
                    .and_then(|(module_id, _)| {
                        load_coverage_module(&module_roots, world, module_id, &mut module_cache)
                            .ok()
                    })
                    .and_then(|module| module.decls.get(&symbol));
                if let Some(decl) = current_decl {
                    let summary_decl_sha = Some(imported.function.decl_sha256_hex.as_str());
                    if summary_decl_sha != Some(decl.decl_sha256_hex.as_str()) {
                        diagnostics.push(diag_verify(
                            "X07V_SUMMARY_MISMATCH",
                            format!(
                                "imported proof summary for {:?} does not match the current declaration",
                                symbol
                            ),
                        ));
                        functions.insert(
                            symbol.clone(),
                            summary_mismatch_function_for_decl(
                                &symbol,
                                decl,
                                imported,
                                project_root,
                            ),
                        );
                        continue;
                    }
                    add_used_imported_summary(
                        &mut used_imported_summaries,
                        &imported.source,
                        &symbol,
                    );
                    functions.insert(
                        symbol.clone(),
                        coverage_function_from_imported_summary(
                            &symbol,
                            imported,
                            Some(verify_function_signature(
                                &decl.params,
                                &decl.result,
                                decl.result_brand.as_deref(),
                            )),
                            Some(decl.decl_sha256_hex.clone()),
                            Some(report_source_path(&decl.source_path, project_root)),
                        ),
                    );
                    continue;
                }
                add_used_imported_summary(&mut used_imported_summaries, &imported.source, &symbol);
                functions.insert(
                    symbol.clone(),
                    coverage_function_from_imported_summary(&symbol, imported, None, None, None),
                );
                continue;
            }
        }

        let Some((module_id, _)) = symbol.rsplit_once('.') else {
            diagnostics.push(diag_verify(
                missing_summary_code(proof_summaries_required),
                format!(
                    "reachable symbol {:?} could not be resolved and no imported {} was supplied",
                    symbol,
                    missing_summary_label(proof_summaries_required)
                ),
            ));
            functions.insert(
                symbol.clone(),
                VerifyCoverageFunction {
                    symbol,
                    kind: "imported".to_string(),
                    status: "unsupported".to_string(),
                    signature: None,
                    support_summary: None,
                    decl_sha256_hex: None,
                    source_path: None,
                    details: Some(format!(
                        "symbol is not fully qualified and no imported {} was supplied",
                        missing_summary_label(proof_summaries_required)
                    )),
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
                        signature: None,
                        support_summary: None,
                        decl_sha256_hex: None,
                        source_path: None,
                        details: None,
                    },
                );
                continue;
            }
            Err(err) => {
                diagnostics.push(diag_verify(
                    missing_summary_code(proof_summaries_required),
                    format!(
                        "reachable symbol {:?} could not be resolved and no imported {} was supplied: {err}",
                        symbol,
                        missing_summary_label(proof_summaries_required)
                    ),
                ));
                functions.insert(
                    symbol.clone(),
                    VerifyCoverageFunction {
                        symbol,
                        kind: "imported".to_string(),
                        status: "unsupported".to_string(),
                        signature: None,
                        support_summary: None,
                        decl_sha256_hex: None,
                        source_path: None,
                        details: Some(format!(
                            "{err}; no imported {} was supplied for this reachable symbol",
                            missing_summary_label(proof_summaries_required)
                        )),
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
                        signature: None,
                        support_summary: None,
                        decl_sha256_hex: None,
                        source_path: None,
                        details: None,
                    },
                );
                continue;
            }
            diagnostics.push(diag_verify(
                missing_summary_code(proof_summaries_required),
                format!(
                    "reachable symbol {:?} was not present in the loaded module graph and no imported {} was supplied",
                    symbol,
                    missing_summary_label(proof_summaries_required)
                ),
            ));
            functions.insert(
                symbol.clone(),
                VerifyCoverageFunction {
                    symbol,
                    kind: "imported".to_string(),
                    status: "unsupported".to_string(),
                    signature: None,
                    support_summary: None,
                    decl_sha256_hex: None,
                    source_path: None,
                    details: Some(format!(
                        "referenced symbol could not be resolved in the loaded module graph and no imported {} was supplied",
                        missing_summary_label(proof_summaries_required)
                    )),
                },
            );
            continue;
        };

        if decl.kind == "defasync" {
            saw_async = true;
        }
        if let Some(node) = capsule_boundary_node(trust_zones.as_ref(), &symbol, &decl.source_path)
        {
            functions.insert(
                symbol.clone(),
                VerifyCoverageFunction {
                    symbol,
                    kind: decl.kind.clone(),
                    status: "capsule_boundary".to_string(),
                    signature: Some(verify_function_signature(
                        &decl.params,
                        &decl.result,
                        decl.result_brand.as_deref(),
                    )),
                    support_summary: None,
                    decl_sha256_hex: Some(decl.decl_sha256_hex.clone()),
                    source_path: Some(report_source_path(&decl.source_path, project_root)),
                    details: Some(format!(
                        "reachable closure terminates at certified capsule node {:?}",
                        node.id
                    )),
                },
            );
            continue;
        }
        functions.insert(
            symbol.clone(),
            coverage_function_for_decl(
                &module_roots,
                world,
                &symbol,
                decl,
                args.max_bytes_len,
                project_root,
            ),
        );
        enqueue_decl_refs(module_id, module, decl, &mut queue);
    }

    let mut functions = functions.into_values().collect::<Vec<_>>();
    let async_model = if saw_async {
        let model = load_verify_scheduler_model()?;
        functions.push(VerifyCoverageFunction {
            symbol: format!("x07.verify.scheduler_model.{}", model.id),
            kind: "builtin".to_string(),
            status: "trusted_scheduler_model".to_string(),
            signature: None,
            support_summary: None,
            decl_sha256_hex: None,
            source_path: None,
            details: Some(model.guarantees.join("; ")),
        });
        Some(model.id)
    } else {
        None
    };
    let mut summary = summarize_coverage_functions(&functions);
    summary.async_model = async_model;
    let mut imported_summaries = used_imported_summaries.into_values().collect::<Vec<_>>();
    imported_summaries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(CoverageAnalysis {
        coverage: VerifyCoverage {
            schema_version: X07_VERIFY_COVERAGE_SCHEMA_VERSION,
            entry: args.entry.clone(),
            worlds: coverage_worlds(project_path),
            summary,
            functions,
        },
        diagnostics,
        imported_summaries,
    })
}

fn coverage_report_fallback(
    module_roots: &[PathBuf],
    entry: &str,
    project_path: Option<&Path>,
    target: &TargetSig,
    max_bytes_len: u32,
    extra_details: Option<String>,
) -> CoverageAnalysis {
    let project_root = project_path.and_then(|p| p.parent());
    let recursion = RecursionSummary {
        kind: if target.decreases_count > 0 && contains_direct_recursion(&target.body, entry) {
            RecursionKind::SelfRecursive
        } else {
            RecursionKind::None
        },
        cycle_symbol: None,
    };
    let mut function = coverage_function_for_target(
        module_roots,
        entry,
        target,
        max_bytes_len,
        &recursion,
        project_root,
    );
    match extra_details {
        Some(details)
            if matches!(
                function.status.as_str(),
                "supported" | "supported_recursive" | "supported_async"
            ) =>
        {
            function.status = "unsupported".to_string();
            function.details = Some(details);
        }
        Some(details) if function.details.is_none() => {
            function.details = Some(details);
        }
        _ => {}
    }
    let functions = vec![function];
    CoverageAnalysis {
        coverage: VerifyCoverage {
            schema_version: X07_VERIFY_COVERAGE_SCHEMA_VERSION,
            entry: entry.to_string(),
            worlds: coverage_worlds(project_path),
            summary: summarize_coverage_functions(&functions),
            functions,
        },
        diagnostics: Vec::new(),
        imported_summaries: Vec::new(),
    }
}

fn coverage_function_for_decl(
    module_roots: &[PathBuf],
    world: WorldId,
    symbol: &str,
    decl: &CoverageDecl,
    max_bytes_len: u32,
    project_root: Option<&Path>,
) -> VerifyCoverageFunction {
    let source_path = Some(report_source_path(&decl.source_path, project_root));
    match decl.kind.as_str() {
        "extern" => VerifyCoverageFunction {
            symbol: symbol.to_string(),
            kind: "extern".to_string(),
            status: "unsupported".to_string(),
            signature: Some(verify_function_signature(
                &decl.params,
                &decl.result,
                decl.result_brand.as_deref(),
            )),
            support_summary: None,
            decl_sha256_hex: Some(decl.decl_sha256_hex.clone()),
            source_path,
            details: Some(
                "extern declarations are outside the certifiable pure subset".to_string(),
            ),
        },
        _ => {
            let body = decl.body.clone().unwrap_or(Value::Null);
            let target = TargetSig {
                param_names: decl.param_names.clone(),
                params: decl.params.clone(),
                result: decl.result.clone(),
                result_brand: decl.result_brand.clone(),
                decl_sha256_hex: decl.decl_sha256_hex.clone(),
                is_async: decl.kind == "defasync",
                has_contracts: decl.has_contracts,
                decreases_count: decl.decreases_count,
                decreases: decl.decreases.clone(),
                body,
                source_path: decl.source_path.clone(),
            };
            let recursion = recursion_summary_for_symbol(module_roots, world, symbol).unwrap_or(
                RecursionSummary {
                    kind: RecursionKind::None,
                    cycle_symbol: None,
                },
            );
            coverage_function_for_target(
                module_roots,
                symbol,
                &target,
                max_bytes_len,
                &recursion,
                project_root,
            )
        }
    }
}

fn summarize_coverage_functions(functions: &[VerifyCoverageFunction]) -> VerifyCoverageSummary {
    VerifyCoverageSummary {
        reachable_defn: functions.iter().filter(|f| f.kind == "defn").count() as u64,
        supported_defn: functions
            .iter()
            .filter(|f| {
                f.kind == "defn"
                    && matches!(
                        f.status.as_str(),
                        "supported" | "supported_recursive" | "imported_proof_summary"
                    )
            })
            .count() as u64,
        recursive_defn: functions
            .iter()
            .filter(|f| {
                f.kind == "defn"
                    && f.support_summary
                        .as_ref()
                        .is_some_and(|summary| summary.recursion_kind != "none")
            })
            .count() as u64,
        supported_recursive_defn: functions
            .iter()
            .filter(|f| {
                f.kind == "defn"
                    && matches!(
                        f.status.as_str(),
                        "supported_recursive" | "imported_proof_summary"
                    )
                    && f.support_summary
                        .as_ref()
                        .is_some_and(|summary| summary.recursion_kind == "self_recursive")
            })
            .count() as u64,
        imported_proof_summary_defn: functions
            .iter()
            .filter(|f| f.kind == "defn" && f.status == "imported_proof_summary")
            .count() as u64,
        termination_proven_defn: functions
            .iter()
            .filter(|f| f.kind == "defn" && f.status == "termination_proven")
            .count() as u64,
        unsupported_recursive_defn: functions
            .iter()
            .filter(|f| {
                f.kind == "defn"
                    && f.status == "unsupported"
                    && f.support_summary
                        .as_ref()
                        .is_some_and(|summary| summary.recursion_kind != "none")
            })
            .count() as u64,
        reachable_async: functions.iter().filter(|f| f.kind == "defasync").count() as u64,
        supported_async: functions
            .iter()
            .filter(|f| f.kind == "defasync" && f.status == "supported_async")
            .count() as u64,
        trusted_primitives: functions
            .iter()
            .filter(|f| f.status == "trusted_primitive")
            .count() as u64,
        trusted_scheduler_models: functions
            .iter()
            .filter(|f| f.status == "trusted_scheduler_model")
            .count() as u64,
        capsule_boundaries: functions
            .iter()
            .filter(|f| f.status == "capsule_boundary")
            .count() as u64,
        uncovered_defn: functions
            .iter()
            .filter(|f| f.kind == "defn" && f.status == "uncovered")
            .count() as u64,
        unsupported_defn: functions
            .iter()
            .filter(|f| f.kind == "defn" && f.status == "unsupported")
            .count() as u64,
        async_model: None,
    }
}

fn is_builtin_like_symbol(symbol: &str) -> bool {
    symbol
        .rsplit_once('.')
        .is_some_and(|(prefix, _)| !prefix.contains('.'))
}

fn build_verify_driver_x07ast_json(
    module_roots: &[PathBuf],
    entry: &str,
    sig: &TargetSig,
    max_bytes_len: u32,
    normalize_for_overlay: bool,
) -> Result<Vec<u8>> {
    let (module_id, _) = entry.rsplit_once('.').context("--entry must contain '.'")?;

    let max_plus_1: u64 = max_bytes_len as u64 + 1;
    if max_plus_1 > i64::MAX as u64 {
        anyhow::bail!("max_bytes_len too large");
    }

    let mut stmts: Vec<Value> = Vec::new();
    let mut call_args: Vec<Value> = Vec::new();
    let mut helper_decls = Vec::new();
    let mut imports = BTreeSet::from([module_id.to_string(), "std.codec".to_string()]);
    let decode_params = if sig.is_async {
        sig.params.clone()
    } else {
        sig.params
            .iter()
            .map(verify_driver_raw_param)
            .collect::<Result<Vec<_>>>()?
    };

    stmts.push(serde_json::json!(["let", "off", 0]));

    for (idx, param) in decode_params.iter().enumerate() {
        let step = append_verify_param_setup(
            &mut stmts,
            &mut call_args,
            idx,
            param,
            max_bytes_len,
            max_plus_1,
        )?;
        stmts.push(serde_json::json!(["set", "off", ["+", "off", step as i64]]));
    }

    let mut call_items = Vec::with_capacity(1 + call_args.len());
    if sig.is_async {
        call_items.push(Value::String(entry.to_string()));
    } else {
        let helper_name = build_verify_sync_helper_decl(
            &mut helper_decls,
            &mut imports,
            module_roots,
            entry,
            sig,
            normalize_for_overlay,
        )?;
        call_items.push(Value::String(helper_name));
    }
    call_items.extend(call_args);

    if sig.is_async {
        stmts.push(serde_json::json!([
            "let",
            "t_normal",
            Value::Array(call_items.clone())
        ]));
        stmts.push(serde_json::json!([
            "let",
            "_spawn0",
            ["task.spawn", "t_normal"]
        ]));
        let normal_join = if sig.result == "result_bytes" {
            serde_json::json!(["task.join.result_bytes", "t_normal"])
        } else {
            serde_json::json!(["task.join.bytes", "t_normal"])
        };
        stmts.push(serde_json::json!(["let", "_normal", normal_join]));

        stmts.push(serde_json::json!([
            "let",
            "t_cancel",
            Value::Array(call_items)
        ]));
        stmts.push(serde_json::json!([
            "let",
            "_spawn1",
            ["task.spawn", "t_cancel"]
        ]));
        stmts.push(serde_json::json!([
            "let",
            "_cancel",
            ["task.cancel", "t_cancel"]
        ]));
        let cancel_join = if sig.result == "result_bytes" {
            serde_json::json!(["task.join.result_bytes", "t_cancel"])
        } else {
            serde_json::json!(["task.join.bytes", "t_cancel"])
        };
        stmts.push(serde_json::json!(["let", "_canceled", cancel_join]));
    } else {
        stmts.push(serde_json::json!(["let", "_", Value::Array(call_items)]));
    }
    stmts.push(serde_json::json!(["bytes.empty"]));

    let mut solve_items = Vec::with_capacity(1 + stmts.len());
    solve_items.push(Value::String("begin".to_string()));
    solve_items.extend(stmts);
    let solve = Value::Array(solve_items);

    let file = serde_json::json!({
        "schema_version": x07_contracts::X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": imports.into_iter().collect::<Vec<_>>(),
        "decls": helper_decls,
        "solve": solve,
    });

    let mut out = serde_json::to_vec_pretty(&file).context("encode verify driver JSON")?;
    out.push(b'\n');
    Ok(out)
}

fn build_verify_sync_helper_decl(
    helper_decls: &mut Vec<Value>,
    imports: &mut BTreeSet<String>,
    module_roots: &[PathBuf],
    entry: &str,
    sig: &TargetSig,
    normalize_for_overlay: bool,
) -> Result<String> {
    let helper_name = "main.__verify_call_v1".to_string();
    let compile_params = sig
        .params
        .iter()
        .map(|param| verify_compile_param(param, normalize_for_overlay))
        .collect::<Vec<_>>();
    let raw_params = sig
        .params
        .iter()
        .enumerate()
        .map(|(idx, param)| {
            let mut raw = serde_json::Map::new();
            raw.insert("name".to_string(), Value::String(format!("p{idx}_raw")));
            raw.insert(
                "ty".to_string(),
                Value::String(verify_driver_raw_param(param)?.ty),
            );
            Ok(Value::Object(raw))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut call_items = Vec::with_capacity(1 + sig.params.len());
    call_items.push(Value::String(entry.to_string()));
    for (idx, (param, compile_param)) in sig.params.iter().zip(compile_params.iter()).enumerate() {
        if let Some(brand_id) = param.brand.as_deref() {
            let validator = resolve_verify_brand_validator(module_roots, entry, brand_id)?;
            if let Some((validator_module, _)) = validator.rsplit_once('.') {
                imports.insert(validator_module.to_string());
            }
        }
        call_items.push(build_verify_sync_helper_arg_expr(
            &format!("p{idx}_raw"),
            param,
            compile_param,
            module_roots,
            entry,
        )?);
    }

    let body = serde_json::json!([
        "begin",
        ["let", "empty_bytes", ["bytes.empty"]],
        ["let", "empty_view", ["bytes.view", "empty_bytes"]],
        ["let", "_", Value::Array(call_items)],
        ["result_i32.ok", 0]
    ]);

    helper_decls.push(serde_json::json!({
        "kind": "defn",
        "name": helper_name,
        "params": raw_params,
        "result": "result_i32",
        "body": body,
    }));
    Ok("main.__verify_call_v1".to_string())
}

fn build_verify_sync_helper_arg_expr(
    raw_name: &str,
    param: &VerifySignatureParam,
    compile_param: &VerifySignatureParam,
    module_roots: &[PathBuf],
    entry: &str,
) -> Result<Value> {
    let raw = Value::String(raw_name.to_string());
    let empty_view = Value::String("empty_view".to_string());
    let brand_cast = |target_ty: &str, value: Value| -> Result<Value> {
        let brand_id = param
            .brand
            .as_deref()
            .context("verify helper missing brand id")?;
        let validator = resolve_verify_brand_validator(module_roots, entry, brand_id)?;
        let check_name = format!("{raw_name}_brand_check");
        let view_name = format!("{raw_name}_brand_view");
        let validate_value = value.clone();
        Ok(if target_ty == "bytes" {
            serde_json::json!([
                "begin",
                ["let", check_name, ["try", [validator, validate_value]]],
                [
                    "let",
                    view_name,
                    ["__internal.brand.assume_view_v1", brand_id, value]
                ],
                [
                    "__internal.brand.view_to_bytes_preserve_brand_v1",
                    view_name
                ]
            ])
        } else {
            serde_json::json!([
                "begin",
                ["let", check_name, ["try", [validator, validate_value]]],
                ["__internal.brand.assume_view_v1", brand_id, value]
            ])
        })
    };

    match compile_param.ty.as_str() {
        "i32" | "u32" | "option_i32" | "result_i32" => Ok(raw),
        "vec_u8" => Ok(serde_json::json!([
            "vec_u8.extend_bytes",
            ["vec_u8.with_capacity", ["view.len", raw]],
            raw
        ])),
        "bytes_view" => {
            if param.brand.is_some() {
                brand_cast("bytes_view", raw)
            } else {
                Ok(raw)
            }
        }
        "bytes" => {
            if param.brand.is_some() {
                brand_cast("bytes", raw)
            } else {
                Ok(serde_json::json!(["view.to_bytes", raw]))
            }
        }
        "option_bytes_view" => {
            let payload = serde_json::json!(["option_bytes_view.unwrap_or", raw, empty_view]);
            let some_payload = if param.brand.is_some() {
                brand_cast("bytes_view", payload)?
            } else {
                payload
            };
            Ok(serde_json::json!([
                "if",
                ["!=", ["option_bytes_view.is_some", raw], 0],
                ["option_bytes_view.some", some_payload],
                ["option_bytes_view.none"]
            ]))
        }
        "option_bytes" => {
            let payload = serde_json::json!(["option_bytes_view.unwrap_or", raw, empty_view]);
            let some_payload = if param.brand.is_some() {
                brand_cast("bytes", payload)?
            } else {
                serde_json::json!(["view.to_bytes", payload])
            };
            Ok(serde_json::json!([
                "if",
                ["!=", ["option_bytes_view.is_some", raw], 0],
                ["option_bytes.some", some_payload],
                ["option_bytes.none"]
            ]))
        }
        "result_bytes_view" => {
            let payload = serde_json::json!(["result_bytes_view.unwrap_or", raw, empty_view]);
            let ok_payload = if param.brand.is_some() {
                brand_cast("bytes_view", payload)?
            } else {
                payload
            };
            Ok(serde_json::json!([
                "if",
                ["!=", ["result_bytes_view.is_ok", raw], 0],
                ["result_bytes_view.ok", ok_payload],
                ["result_bytes_view.err", ["result_bytes_view.err_code", raw]]
            ]))
        }
        "result_bytes" => {
            let payload = serde_json::json!(["result_bytes_view.unwrap_or", raw, empty_view]);
            let ok_payload = if param.brand.is_some() {
                brand_cast("bytes", payload)?
            } else {
                serde_json::json!(["view.to_bytes", payload])
            };
            Ok(serde_json::json!([
                "if",
                ["!=", ["result_bytes_view.is_ok", raw], 0],
                ["result_bytes.ok", ok_payload],
                ["result_bytes.err", ["result_bytes_view.err_code", raw]]
            ]))
        }
        other => anyhow::bail!("unsupported verify sync helper param type {other:?}"),
    }
}

fn append_verify_param_setup(
    stmts: &mut Vec<Value>,
    call_args: &mut Vec<Value>,
    idx: usize,
    param: &VerifySignatureParam,
    max_bytes_len: u32,
    max_plus_1: u64,
) -> Result<u64> {
    let off = Value::String("off".to_string());
    match param.ty.as_str() {
        "i32" | "u32" => {
            let name = format!("p{idx}");
            stmts.push(serde_json::json!([
                "let",
                name,
                ["std.codec.read_u32_le", "input", off]
            ]));
            call_args.push(Value::String(format!("p{idx}")));
            Ok(4)
        }
        "bytes" | "bytes_view" | "vec_u8" => {
            append_verify_bytes_like_param(
                stmts,
                call_args,
                idx,
                if param.ty == "vec_u8" {
                    "bytes_view"
                } else {
                    &param.ty
                },
                off,
                max_bytes_len,
                max_plus_1,
            )?;
            Ok(4u64 + max_bytes_len as u64)
        }
        "option_i32" => {
            let tag = format!("p{idx}_tag");
            let value = format!("p{idx}_value");
            let arg = format!("p{idx}_arg");
            stmts.push(serde_json::json!([
                "let",
                tag,
                ["view.get_u8", "input", off]
            ]));
            stmts.push(serde_json::json!([
                "let",
                value,
                ["std.codec.read_u32_le", "input", ["+", "off", 1]]
            ]));
            stmts.push(serde_json::json!([
                "let",
                arg,
                [
                    "if",
                    ["!=", tag, 0],
                    ["option_i32.some", value],
                    ["option_i32.none"]
                ]
            ]));
            call_args.push(Value::String(arg));
            Ok(5)
        }
        "option_bytes" | "option_bytes_view" => {
            let tag = format!("p{idx}_tag");
            stmts.push(serde_json::json!([
                "let",
                tag,
                ["view.get_u8", "input", off]
            ]));
            let payload_name = append_verify_bytes_payload(
                stmts,
                idx,
                "payload",
                if param.ty == "option_bytes" {
                    "bytes"
                } else {
                    "bytes_view"
                },
                serde_json::json!(["+", "off", 1]),
                max_bytes_len,
                max_plus_1,
            )?;
            let arg = format!("p{idx}_arg");
            let ctor = if param.ty == "option_bytes" {
                "option_bytes.some"
            } else {
                "option_bytes_view.some"
            };
            let none_ctor = if param.ty == "option_bytes" {
                "option_bytes.none"
            } else {
                "option_bytes_view.none"
            };
            stmts.push(serde_json::json!([
                "let",
                arg,
                ["if", ["!=", tag, 0], [ctor, payload_name], [none_ctor]]
            ]));
            call_args.push(Value::String(arg));
            Ok(1u64 + 4u64 + max_bytes_len as u64)
        }
        "result_i32" => {
            let tag = format!("p{idx}_tag");
            let value = format!("p{idx}_value");
            let arg = format!("p{idx}_arg");
            stmts.push(serde_json::json!([
                "let",
                tag,
                ["view.get_u8", "input", off]
            ]));
            stmts.push(serde_json::json!([
                "let",
                value,
                ["std.codec.read_u32_le", "input", ["+", "off", 1]]
            ]));
            stmts.push(serde_json::json!([
                "let",
                arg,
                [
                    "if",
                    ["!=", tag, 0],
                    ["result_i32.ok", value],
                    ["result_i32.err", value]
                ]
            ]));
            call_args.push(Value::String(arg));
            Ok(5)
        }
        "result_bytes" | "result_bytes_view" => {
            let tag = format!("p{idx}_tag");
            let err_code = format!("p{idx}_err_code");
            let arg = format!("p{idx}_arg");
            stmts.push(serde_json::json!([
                "let",
                tag,
                ["view.get_u8", "input", off]
            ]));
            stmts.push(serde_json::json!([
                "let",
                err_code,
                ["std.codec.read_u32_le", "input", ["+", "off", 1]]
            ]));
            let payload_name = append_verify_bytes_payload(
                stmts,
                idx,
                "payload",
                if param.ty == "result_bytes" {
                    "bytes"
                } else {
                    "bytes_view"
                },
                serde_json::json!(["+", "off", 1]),
                max_bytes_len,
                max_plus_1,
            )?;
            let ok_ctor = if param.ty == "result_bytes" {
                "result_bytes.ok"
            } else {
                "result_bytes_view.ok"
            };
            let err_ctor = if param.ty == "result_bytes" {
                "result_bytes.err"
            } else {
                "result_bytes_view.err"
            };
            stmts.push(serde_json::json!([
                "let",
                arg,
                [
                    "if",
                    ["!=", tag, 0],
                    [ok_ctor, payload_name],
                    [err_ctor, err_code]
                ]
            ]));
            call_args.push(Value::String(arg));
            Ok(1u64 + 4u64 + max_bytes_len as u64)
        }
        other => anyhow::bail!("unsupported verify param type {other:?}"),
    }
}

fn append_verify_bytes_like_param(
    stmts: &mut Vec<Value>,
    call_args: &mut Vec<Value>,
    idx: usize,
    ty: &str,
    off: Value,
    max_bytes_len: u32,
    max_plus_1: u64,
) -> Result<()> {
    let value_name =
        append_verify_bytes_payload(stmts, idx, "arg", ty, off, max_bytes_len, max_plus_1)?;
    call_args.push(Value::String(value_name));
    Ok(())
}

fn append_verify_bytes_payload(
    stmts: &mut Vec<Value>,
    idx: usize,
    suffix: &str,
    ty: &str,
    off: Value,
    max_bytes_len: u32,
    max_plus_1: u64,
) -> Result<String> {
    let n_raw = format!("p{idx}_{suffix}_len_raw");
    let n = format!("p{idx}_{suffix}_len");
    let slice_name = format!("p{idx}_{suffix}_slice");
    let data_off = serde_json::json!(["+", off, 4]);
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
    stmts.push(serde_json::json!([
        "let",
        slice_name,
        ["view.slice", "input", data_off, n.clone()]
    ]));

    match ty {
        "bytes_view" => Ok(slice_name),
        "bytes" => append_verify_bytes_value(stmts, idx, suffix, &slice_name),
        other => anyhow::bail!("unsupported verify bytes payload type {other:?}"),
    }
}

fn append_verify_bytes_value(
    stmts: &mut Vec<Value>,
    idx: usize,
    suffix: &str,
    slice_name: &str,
) -> Result<String> {
    let bytes_name = format!("p{idx}_{suffix}_bytes");
    stmts.push(serde_json::json!([
        "let",
        bytes_name,
        ["view.to_bytes", slice_name]
    ]));
    Ok(bytes_name)
}

fn compile_driver_to_c(driver_src: &[u8], module_roots: &[PathBuf]) -> Result<String> {
    let mut opts =
        x07c::world_config::compile_options_for_world(WorldId::SolvePure, module_roots.to_vec());
    opts.emit_main = false;
    opts.freestanding = true;
    opts.contract_mode = x07c::compile::ContractMode::VerifyBmc;
    opts.allow_internal_only_heads_in_entry = true;
    opts.prefer_module_roots_first = true;
    let out = x07c::compile::compile_program_to_c_with_meta(driver_src, &opts)
        .map_err(|err| anyhow::anyhow!("{:?}: {}", err.kind, err.message))?;
    Ok(out.c_src)
}

fn direct_prove_harness_supported(target: &TargetSig) -> bool {
    !target.is_async
        && target.result_brand.is_none()
        && target.params.iter().all(|param| {
            param.brand.is_none()
                && matches!(
                    param.ty.as_str(),
                    "i32"
                        | "u32"
                        | "bytes"
                        | "bytes_view"
                        | "vec_u8"
                        | "option_i32"
                        | "option_bytes"
                        | "option_bytes_view"
                        | "result_i32"
                        | "result_bytes"
                        | "result_bytes_view"
                )
        })
        && matches!(
            target.result.as_str(),
            "i32"
                | "u32"
                | "bytes"
                | "bytes_view"
                | "vec_u8"
                | "option_i32"
                | "option_bytes"
                | "option_bytes_view"
                | "result_i32"
                | "result_bytes"
                | "result_bytes_view"
        )
}

fn c_verify_ty_name(ty: &str) -> Result<&'static str> {
    Ok(match ty {
        "i32" | "u32" => "uint32_t",
        "bytes" => "bytes_t",
        "bytes_view" => "bytes_view_t",
        "vec_u8" => "vec_u8_t",
        "option_i32" => "option_i32_t",
        "option_bytes" => "option_bytes_t",
        "option_bytes_view" => "option_bytes_view_t",
        "result_i32" => "result_i32_t",
        "result_bytes" => "result_bytes_t",
        "result_bytes_view" => "result_bytes_view_t",
        other => anyhow::bail!("unsupported direct prove C type {other:?}"),
    })
}

fn append_direct_harness_bytes_param(
    out: &mut String,
    name: &str,
    ty: &str,
    max_bytes_len: u32,
) -> Result<String> {
    let buf_name = format!("{name}_buf");
    let len_name = format!("{name}_len");
    let buf_cap = std::cmp::max(1u32, max_bytes_len);
    out.push_str(&format!("  uint8_t {buf_name}[{buf_cap}];\n"));
    for i in 0..buf_cap {
        out.push_str(&format!("  {buf_name}[{i}] = x07_nondet_u8();\n"));
    }
    out.push_str(&format!("  uint32_t {len_name} = x07_nondet_u32();\n"));
    out.push_str(&format!(
        "  __CPROVER_assume({len_name} <= UINT32_C({max_bytes_len}));\n"
    ));
    match ty {
        "bytes" => {
            out.push_str(&format!(
                "  bytes_t {name} = (bytes_t){{ .ptr = {buf_name}, .len = {len_name} }};\n"
            ));
        }
        "bytes_view" => {
            out.push_str(&format!(
                "  bytes_view_t {name} = (bytes_view_t){{ .ptr = {buf_name}, .len = {len_name} }};\n"
            ));
        }
        "vec_u8" => {
            out.push_str(&format!(
                "  vec_u8_t {name} = (vec_u8_t){{ .data = {buf_name}, .len = {len_name}, .cap = UINT32_C({max_bytes_len}) }};\n"
            ));
        }
        other => anyhow::bail!("unsupported direct prove bytes-like type {other:?}"),
    }
    Ok(name.to_string())
}

fn append_direct_harness_param(
    out: &mut String,
    idx: usize,
    param: &VerifySignatureParam,
    max_bytes_len: u32,
) -> Result<String> {
    let name = format!("p{idx}");
    match param.ty.as_str() {
        "i32" | "u32" => {
            out.push_str(&format!("  uint32_t {name} = x07_nondet_u32();\n"));
        }
        "bytes" | "bytes_view" | "vec_u8" => {
            return append_direct_harness_bytes_param(out, &name, &param.ty, max_bytes_len);
        }
        "option_i32" => {
            out.push_str(&format!("  uint32_t {name}_tag = x07_nondet_u32();\n"));
            out.push_str(&format!("  __CPROVER_assume({name}_tag <= UINT32_C(1));\n"));
            out.push_str(&format!("  uint32_t {name}_payload = x07_nondet_u32();\n"));
            out.push_str(&format!(
                "  option_i32_t {name} = (option_i32_t){{ .tag = {name}_tag, .payload = {name}_payload }};\n"
            ));
        }
        "option_bytes" | "option_bytes_view" => {
            let payload = append_direct_harness_bytes_param(
                out,
                &format!("{name}_payload"),
                if param.ty == "option_bytes" {
                    "bytes"
                } else {
                    "bytes_view"
                },
                max_bytes_len,
            )?;
            out.push_str(&format!("  uint32_t {name}_tag = x07_nondet_u32();\n"));
            out.push_str(&format!("  __CPROVER_assume({name}_tag <= UINT32_C(1));\n"));
            out.push_str(&format!("  {} {name};\n", c_verify_ty_name(&param.ty)?));
            out.push_str(&format!("  {name}.tag = {name}_tag;\n"));
            out.push_str(&format!("  {name}.payload = {payload};\n"));
        }
        "result_i32" => {
            out.push_str(&format!("  uint32_t {name}_tag = x07_nondet_u32();\n"));
            out.push_str(&format!("  __CPROVER_assume({name}_tag <= UINT32_C(1));\n"));
            out.push_str(&format!("  uint32_t {name}_payload = x07_nondet_u32();\n"));
            out.push_str(&format!(
                "  result_i32_t {name} = (result_i32_t){{ .tag = {name}_tag, .payload.ok = {name}_payload }};\n"
            ));
        }
        "result_bytes" | "result_bytes_view" => {
            let ok = append_direct_harness_bytes_param(
                out,
                &format!("{name}_ok"),
                if param.ty == "result_bytes" {
                    "bytes"
                } else {
                    "bytes_view"
                },
                max_bytes_len,
            )?;
            out.push_str(&format!("  uint32_t {name}_tag = x07_nondet_u32();\n"));
            out.push_str(&format!("  __CPROVER_assume({name}_tag <= UINT32_C(1));\n"));
            out.push_str(&format!("  uint32_t {name}_err = x07_nondet_u32();\n"));
            out.push_str(&format!("  {} {name};\n", c_verify_ty_name(&param.ty)?));
            out.push_str(&format!("  {name}.tag = {name}_tag;\n"));
            out.push_str(&format!("  if ({name}_tag != 0) {{ {name}.payload.ok = {ok}; }} else {{ {name}.payload.err = {name}_err; }}\n"));
        }
        other => anyhow::bail!("unsupported direct prove param type {other:?}"),
    }
    Ok(name)
}

fn build_direct_prove_c_harness(
    entry: &str,
    target: &TargetSig,
    max_bytes_len: u32,
) -> Result<String> {
    let mut out = String::new();
    out.push_str("static unsigned char x07_nondet_u8(void) {\n");
    out.push_str("  unsigned char value;\n");
    out.push_str("  return value;\n");
    out.push_str("}\n");
    out.push_str("static uint32_t x07_nondet_u32(void) {\n");
    out.push_str("  uint32_t value;\n");
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
    out.push_str("  bytes_view_t input = rt_view_empty(&ctx);\n");
    let mut arg_names = Vec::with_capacity(target.params.len());
    for (idx, param) in target.params.iter().enumerate() {
        arg_names.push(append_direct_harness_param(
            &mut out,
            idx,
            param,
            max_bytes_len,
        )?);
    }
    let result_ty = c_verify_ty_name(&target.result)?;
    out.push_str(&format!(
        "  {result_ty} out = {}(&ctx, input",
        c_user_fn_name(entry)
    ));
    for arg in &arg_names {
        out.push_str(&format!(", {arg}"));
    }
    out.push_str(");\n");
    out.push_str("  (void)out;\n");
    out.push_str("  rt_ext_ctx = NULL;\n");
    out.push_str("}\n");
    Ok(out)
}

fn trusted_primitive_stubs_for_prove(
    module_roots: &[PathBuf],
    coverage: &VerifyCoverage,
) -> Result<Vec<TrustedPrimitiveStub>> {
    let mut out = Vec::new();
    for function in &coverage.functions {
        if function.kind != "imported" || function.status != "trusted_primitive" {
            continue;
        }
        let sig = load_target_info(module_roots, &function.symbol)?;
        out.push(TrustedPrimitiveStub {
            symbol: function.symbol.clone(),
            params: sig.params,
            result: sig.result,
        });
    }
    Ok(out)
}

fn build_prove_driver_build(
    entry: &str,
    target: &TargetSig,
    module_roots: &[PathBuf],
    coverage: Option<&VerifyCoverage>,
    max_bytes_len: u32,
    input_len_bytes: u32,
    work_dir: &Path,
) -> Result<ProveDriverBuild> {
    let coverage = coverage.context("prove coverage is required for prove driver build")?;
    let use_direct_prove_harness = direct_prove_harness_supported(target);
    let driver_src = build_verify_driver_x07ast_json(
        module_roots,
        entry,
        target,
        max_bytes_len,
        !use_direct_prove_harness,
    )?;
    let compile_module_roots = if use_direct_prove_harness {
        module_roots.to_vec()
    } else {
        build_verify_compile_module_roots(module_roots, entry, work_dir)?
    };
    let c_src = compile_driver_to_c(&driver_src, &compile_module_roots)?;
    let c_src = apply_trusted_primitive_stubs(
        &c_src,
        &trusted_primitive_stubs_for_prove(module_roots, coverage)?,
    )?;
    let harness_src = if use_direct_prove_harness {
        build_direct_prove_c_harness(entry, target, max_bytes_len)?
    } else {
        build_c_harness(input_len_bytes)
    };
    Ok(ProveDriverBuild {
        driver_src,
        c_with_harness: format!("{c_src}\n\n{harness_src}\n"),
    })
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
        "vec_u8" => {
            lines.push("  vec_u8_t out;".to_string());
            lines.push("  memset(&out, 0, sizeof(out));".to_string());
            lines.push("  return out;".to_string());
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
        "result_result_bytes" => {
            lines.push("  result_result_bytes_t out;".to_string());
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

fn apply_c_function_body_replacements(c_src: &str, bodies: &[(String, String)]) -> Result<String> {
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for (c_name, body) in bodies {
        let Some((open_brace_idx, close_brace_idx)) = find_c_function_definition(c_src, c_name)?
        else {
            anyhow::bail!("could not locate generated C body for {:?}", c_name);
        };
        replacements.push((open_brace_idx, close_brace_idx, format!("{{\n{body}\n}}")));
    }

    replacements.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = c_src.to_string();
    for (start, end, replacement) in replacements {
        out.replace_range(start..=end, &replacement);
    }
    Ok(out)
}

fn apply_trusted_primitive_stubs(c_src: &str, stubs: &[TrustedPrimitiveStub]) -> Result<String> {
    if stubs.is_empty() {
        return Ok(c_src.to_string());
    }
    let replacements = stubs
        .iter()
        .map(|stub| {
            Ok((
                c_user_fn_name(&stub.symbol),
                trusted_primitive_stub_body(stub)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    apply_c_function_body_replacements(c_src, &replacements)
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
    if let Some(protocol) = defn.get("protocol") {
        for k in ["await_invariant", "scope_invariant", "cancellation_ensures"] {
            if protocol
                .get(k)
                .and_then(Value::as_array)
                .is_some_and(|v| !v.is_empty())
            {
                return true;
            }
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

fn recursive_arg_is_obviously_non_decreasing(arg: &Value, decreases_ident: &str) -> bool {
    match arg {
        Value::String(ident) => ident == decreases_ident,
        Value::Array(items) => match items.first().and_then(Value::as_str) {
            Some("+") if items.len() == 3 => {
                let lhs_is_rank =
                    matches!(items.get(1), Some(Value::String(ident)) if ident == decreases_ident);
                let rhs_is_rank =
                    matches!(items.get(2), Some(Value::String(ident)) if ident == decreases_ident);
                (lhs_is_rank && items.get(2).and_then(Value::as_i64).is_some_and(|n| n >= 0))
                    || (rhs_is_rank && items.get(1).and_then(Value::as_i64).is_some_and(|n| n >= 0))
            }
            Some("-") if items.len() == 3 => {
                matches!(items.get(1), Some(Value::String(ident)) if ident == decreases_ident)
                    && items.get(2).and_then(Value::as_i64).is_some_and(|n| n <= 0)
            }
            _ => false,
        },
        _ => false,
    }
}

fn find_recursive_termination_failure(target: &TargetSig, entry: &str) -> Option<String> {
    let decreases_ident = target.decreases.first()?.as_str()?;
    let decreases_param_idx = target
        .param_names
        .iter()
        .position(|name| name == decreases_ident)?;

    fn walk(
        expr: &Value,
        entry: &str,
        decreases_ident: &str,
        decreases_param_idx: usize,
    ) -> Option<String> {
        match expr {
            Value::Array(items) => {
                if matches!(items.first(), Some(Value::String(head)) if head == entry) {
                    let arg = items.get(decreases_param_idx + 1)?;
                    if recursive_arg_is_obviously_non_decreasing(arg, decreases_ident) {
                        return Some(format!(
                            "recursive self-call does not obviously decrease {:?}",
                            decreases_ident
                        ));
                    }
                }
                items
                    .iter()
                    .find_map(|item| walk(item, entry, decreases_ident, decreases_param_idx))
            }
            Value::Object(map) => map
                .values()
                .find_map(|value| walk(value, entry, decreases_ident, decreases_param_idx)),
            _ => None,
        }
    }

    walk(&target.body, entry, decreases_ident, decreases_param_idx)
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
        "spec/x07.verify.cex@0.2.0.schema.json",
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

fn verify_summary_to_pretty_canon_bytes(
    summary: &VerifyCoverageSummaryArtifact,
) -> Result<Vec<u8>> {
    let v = serde_json::to_value(summary).context("serialize verify summary JSON")?;
    let diags = validate_verify_summary_schema(&v)?;
    if !diags.is_empty() {
        anyhow::bail!(
            "internal error: verify summary JSON is not schema-valid: {}",
            diags[0].message
        );
    }
    report_common::canonical_pretty_json_bytes(&v).context("canon verify summary JSON")
}

fn write_verify_summary_artifact(
    path: &Path,
    coverage: &VerifyCoverage,
    imported_summaries: &[VerifyImportedSummaryRef],
) -> Result<()> {
    let summary = VerifyCoverageSummaryArtifact {
        schema_version: X07_VERIFY_SUMMARY_SCHEMA_VERSION.to_string(),
        summary_kind: "coverage_support".to_string(),
        entry: coverage.entry.clone(),
        worlds: coverage.worlds.clone(),
        summary: coverage.summary.clone(),
        functions: coverage.functions.clone(),
        imported_summaries: imported_summaries.to_vec(),
    };
    let bytes = verify_summary_to_pretty_canon_bytes(&summary)?;
    util::write_atomic(path, &bytes)
        .with_context(|| format!("write verify summary: {}", path.display()))
}

fn verify_proof_summary_to_pretty_canon_bytes(
    summary: &VerifyProofSummaryArtifact,
) -> Result<Vec<u8>> {
    let v = serde_json::to_value(summary).context("serialize verify proof summary JSON")?;
    let diags = validate_verify_proof_summary_schema(&v)?;
    if !diags.is_empty() {
        anyhow::bail!(
            "internal error: verify proof summary JSON is not schema-valid: {}",
            diags[0].message
        );
    }
    report_common::canonical_pretty_json_bytes(&v).context("canon verify proof summary JSON")
}

fn write_verify_proof_summary_artifact(
    path: &Path,
    summary: &VerifyProofSummaryArtifact,
) -> Result<Vec<u8>> {
    let bytes = verify_proof_summary_to_pretty_canon_bytes(summary)?;
    util::write_atomic(path, &bytes)
        .with_context(|| format!("write verify proof summary: {}", path.display()))?;
    Ok(bytes)
}

fn verify_proof_object_to_pretty_canon_bytes(object: &VerifyProofObject) -> Result<Vec<u8>> {
    let v = serde_json::to_value(object).context("serialize verify proof object JSON")?;
    let diags = report_common::validate_schema(
        X07_VERIFY_PROOF_OBJECT_SCHEMA_BYTES,
        "spec/x07-verify.proof-object.schema.json",
        &v,
    )?;
    if !diags.is_empty() {
        anyhow::bail!(
            "internal error: verify proof object JSON is not schema-valid: {}",
            diags[0].message
        );
    }
    report_common::canonical_pretty_json_bytes(&v).context("canon verify proof object JSON")
}

fn verify_proof_check_report_to_pretty_canon_bytes(
    report: &VerifyProofCheckReport,
) -> Result<Vec<u8>> {
    let v = serde_json::to_value(report).context("serialize verify proof-check report JSON")?;
    let diags = report_common::validate_schema(
        X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_BYTES,
        "spec/x07-verify.proof-check.report.schema.json",
        &v,
    )?;
    if !diags.is_empty() {
        anyhow::bail!(
            "internal error: verify proof-check report JSON is not schema-valid: {}",
            diags[0].message
        );
    }
    report_common::canonical_pretty_json_bytes(&v).context("canon verify proof-check report JSON")
}

fn proof_summary_bundle_path(dir: &Path) -> PathBuf {
    dir.join("verify.proof-summary.json")
}

fn proof_imported_summaries_bundle_dir(dir: &Path) -> PathBuf {
    dir.join("imported_proof_summaries")
}

fn proof_obligation_bundle_path(dir: &Path) -> PathBuf {
    dir.join("verify.smt2")
}

fn proof_solver_transcript_bundle_path(dir: &Path) -> PathBuf {
    dir.join("z3.out.txt")
}

fn proof_check_report_path(proof_path: &Path) -> PathBuf {
    let stem = proof_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("proof");
    proof_path.with_file_name(format!("{stem}.check.json"))
}

fn prefixed_sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", util::sha256_hex(bytes))
}

fn project_manifest_digest(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("read project manifest: {}", path.display()))?;
    Ok(prefixed_sha256(&bytes))
}

fn verify_primitive_manifest_digest() -> String {
    prefixed_sha256(X07_VERIFY_PRIMITIVES_CATALOG_BYTES)
}

fn verify_scheduler_model_digest() -> String {
    prefixed_sha256(X07_VERIFY_SCHEDULER_MODEL_BYTES)
}

fn load_bundle_imported_summary_paths(bundle_dir: &Path) -> Result<Vec<PathBuf>> {
    let dir = proof_imported_summaries_bundle_dir(bundle_dir);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = std::fs::read_dir(&dir)
        .with_context(|| format!("read imported proof-summary bundle dir: {}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn write_bundle_imported_summaries(
    bundle_dir: &Path,
    imported_summaries: &[VerifyImportedSummaryRef],
) -> Result<Vec<String>> {
    let imported_dir = proof_imported_summaries_bundle_dir(bundle_dir);
    if imported_summaries.is_empty() {
        if imported_dir.exists() {
            std::fs::remove_dir_all(&imported_dir).with_context(|| {
                format!(
                    "remove imported proof-summary bundle dir: {}",
                    imported_dir.display()
                )
            })?;
        }
        return Ok(Vec::new());
    }

    std::fs::create_dir_all(&imported_dir).with_context(|| {
        format!(
            "create imported proof-summary bundle dir: {}",
            imported_dir.display()
        )
    })?;
    let mut digests = Vec::with_capacity(imported_summaries.len());
    for imported in imported_summaries {
        let source_path = Path::new(&imported.path);
        let bytes = std::fs::read(source_path)
            .with_context(|| format!("read imported proof summary: {}", source_path.display()))?;
        let digest = prefixed_sha256(&bytes);
        if digest != format!("sha256:{}", imported.sha256_hex) {
            anyhow::bail!(
                "imported proof summary digest mismatch for {}: expected sha256:{} got {}",
                source_path.display(),
                imported.sha256_hex,
                digest
            );
        }
        let bundle_path = imported_dir.join(format!("{}.json", imported.sha256_hex));
        util::write_atomic(&bundle_path, &bytes).with_context(|| {
            format!(
                "write bundled imported proof summary: {}",
                bundle_path.display()
            )
        })?;
        digests.push(digest);
    }
    digests.sort();
    digests.dedup();
    Ok(digests)
}

fn resolve_replay_project_manifest(cwd: &Path, proof_path: &Path) -> Result<PathBuf> {
    for start in [Some(cwd), proof_path.parent()] {
        let mut current = start.map(Path::to_path_buf);
        while let Some(dir) = current {
            let candidate = dir.join("x07.json");
            if candidate.is_file() {
                return Ok(candidate);
            }
            current = dir.parent().map(Path::to_path_buf);
        }
    }
    anyhow::bail!(
        "could not resolve x07.json for proof replay from {} or {}",
        proof_path.display(),
        cwd.display()
    )
}

fn write_prove_bundle_artifacts(
    proof_path: &Path,
    proof_summary_artifact: &VerifyProofSummaryArtifact,
    imported_summaries: &[VerifyImportedSummaryRef],
    project_path: &Path,
    bounds: &Bounds,
    smt2_path: &Path,
    z3_out_path: &Path,
) -> Result<(Artifacts, VerifyProofSummaryArtifact)> {
    let bundle_dir = proof_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(bundle_dir)
        .with_context(|| format!("create proof bundle dir: {}", bundle_dir.display()))?;

    let proof_summary_path = proof_summary_bundle_path(bundle_dir);
    let proof_summary_bytes =
        write_verify_proof_summary_artifact(&proof_summary_path, proof_summary_artifact)?;
    let proof_summary_digest = prefixed_sha256(&proof_summary_bytes);

    let smt2_bytes =
        std::fs::read(smt2_path).with_context(|| format!("read smt2: {}", smt2_path.display()))?;
    let proof_obligation_path = proof_obligation_bundle_path(bundle_dir);
    util::write_atomic(&proof_obligation_path, &smt2_bytes).with_context(|| {
        format!(
            "write proof obligation: {}",
            proof_obligation_path.display()
        )
    })?;

    let solver_bytes = std::fs::read(z3_out_path)
        .with_context(|| format!("read solver transcript: {}", z3_out_path.display()))?;
    let solver_transcript_path = proof_solver_transcript_bundle_path(bundle_dir);
    util::write_atomic(&solver_transcript_path, &solver_bytes).with_context(|| {
        format!(
            "write solver transcript: {}",
            solver_transcript_path.display()
        )
    })?;
    let imported_proof_summary_digests =
        write_bundle_imported_summaries(bundle_dir, imported_summaries)?;

    let object = VerifyProofObject {
        schema_version: X07_VERIFY_PROOF_OBJECT_SCHEMA_VERSION.to_string(),
        project_manifest_digest: project_manifest_digest(project_path)?,
        entry_symbol: proof_summary_artifact.symbol.clone(),
        symbol: proof_summary_artifact.symbol.clone(),
        kind: proof_summary_artifact.kind.clone(),
        decl_sha256_hex: proof_summary_artifact.decl_sha256_hex.clone(),
        verify_engine: proof_summary_artifact.engine.clone(),
        primitive_manifest_digest: verify_primitive_manifest_digest(),
        imported_proof_summary_digests,
        proof_summary_digest: proof_summary_digest.clone(),
        obligation_digest: prefixed_sha256(&smt2_bytes),
        expected_solver_result: "unsat".to_string(),
        recursion_kind: proof_summary_artifact.recursion_kind.clone(),
        recursion_bound_kind: proof_summary_artifact.recursion_bound_kind.clone(),
        scheduler_model_digest: if proof_summary_artifact.kind == "defasync" {
            Some(verify_scheduler_model_digest())
        } else {
            None
        },
        unwind: bounds.unwind,
        max_bytes_len: bounds.max_bytes_len,
    };
    let object_bytes = verify_proof_object_to_pretty_canon_bytes(&object)?;
    util::write_atomic(proof_path, &object_bytes)
        .with_context(|| format!("write proof object: {}", proof_path.display()))?;

    let check_report = check_proof_object_path(proof_path)?;
    let check_report_path = proof_check_report_path(proof_path);
    let check_report_bytes = verify_proof_check_report_to_pretty_canon_bytes(&check_report)?;
    util::write_atomic(&check_report_path, &check_report_bytes)
        .with_context(|| format!("write proof-check report: {}", check_report_path.display()))?;

    Ok((
        Artifacts {
            verify_proof_summary_path: Some(proof_summary_path.display().to_string()),
            proof_object_path: Some(proof_path.display().to_string()),
            proof_check_report_path: Some(check_report_path.display().to_string()),
            ..Artifacts::default()
        },
        proof_summary_artifact.clone(),
    ))
}

struct ReplayTempDir {
    path: PathBuf,
}

impl ReplayTempDir {
    fn new(prefix: &str) -> Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "{prefix}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .context("system time before UNIX_EPOCH")?
                .as_nanos()
        ));
        std::fs::create_dir_all(&path)
            .with_context(|| format!("create replay temp dir: {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ReplayTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn accepted_proof_check_report(
    object: &VerifyProofObject,
    proof_object_digest: String,
    replayed_obligation_digest: String,
    replayed_solver_result: String,
    validated_imported_proof_summary_digests: Vec<String>,
    validated_scheduler_model_digest: Option<String>,
) -> VerifyProofCheckReport {
    VerifyProofCheckReport {
        schema_version: X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_VERSION.to_string(),
        ok: true,
        proof_object_digest,
        checker: "x07.proof_replay_checker".to_string(),
        result: "accepted".to_string(),
        symbol: object.symbol.clone(),
        entry_symbol: object.entry_symbol.clone(),
        verify_engine: object.verify_engine.clone(),
        expected_obligation_digest: object.obligation_digest.clone(),
        replayed_obligation_digest,
        expected_solver_result: object.expected_solver_result.clone(),
        replayed_solver_result,
        validated_imported_proof_summary_digests,
        validated_scheduler_model_digest,
        diagnostics: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn rejected_proof_check_report_for_object(
    object: &VerifyProofObject,
    proof_object_digest: String,
    code: &str,
    message: String,
    replayed_obligation_digest: Option<String>,
    replayed_solver_result: Option<String>,
    validated_imported_proof_summary_digests: Vec<String>,
    validated_scheduler_model_digest: Option<String>,
) -> VerifyProofCheckReport {
    VerifyProofCheckReport {
        schema_version: X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_VERSION.to_string(),
        ok: false,
        proof_object_digest,
        checker: "x07.proof_replay_checker".to_string(),
        result: "rejected".to_string(),
        symbol: object.symbol.clone(),
        entry_symbol: object.entry_symbol.clone(),
        verify_engine: object.verify_engine.clone(),
        expected_obligation_digest: object.obligation_digest.clone(),
        replayed_obligation_digest: replayed_obligation_digest
            .unwrap_or_else(|| object.obligation_digest.clone()),
        expected_solver_result: object.expected_solver_result.clone(),
        replayed_solver_result: replayed_solver_result
            .unwrap_or_else(|| object.expected_solver_result.clone()),
        validated_imported_proof_summary_digests,
        validated_scheduler_model_digest,
        diagnostics: vec![diag_verify(code, message)],
    }
}

fn load_proof_object_path(proof_path: &Path) -> Result<(String, VerifyProofObject)> {
    let proof_bytes = std::fs::read(proof_path)
        .with_context(|| format!("read proof object: {}", proof_path.display()))?;
    let proof_digest = prefixed_sha256(&proof_bytes);
    let value: Value = serde_json::from_slice(&proof_bytes)
        .with_context(|| format!("parse proof object JSON {}", proof_path.display()))?;
    let diags = report_common::validate_schema(
        X07_VERIFY_PROOF_OBJECT_SCHEMA_BYTES,
        "spec/x07-verify.proof-object.schema.json",
        &value,
    )?;
    if !diags.is_empty() {
        anyhow::bail!("proof object schema invalid: {}", diags[0].message);
    }
    let object: VerifyProofObject = serde_json::from_value(value)
        .with_context(|| format!("decode proof object JSON {}", proof_path.display()))?;
    Ok((proof_digest, object))
}

pub(crate) fn load_proof_check_report_path(
    proof_check_path: &Path,
) -> Result<VerifyProofCheckReport> {
    let doc = report_common::read_json_file(proof_check_path)?;
    let diags = report_common::validate_schema(
        X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_BYTES,
        "spec/x07-verify.proof-check.report.schema.json",
        &doc,
    )?;
    if !diags.is_empty() {
        anyhow::bail!("proof-check report schema invalid: {}", diags[0].message);
    }
    let required_str = |key: &str| -> Result<String> {
        doc.get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .with_context(|| format!("proof-check report missing string field {:?}", key))
    };
    Ok(VerifyProofCheckReport {
        schema_version: required_str("schema_version")?,
        ok: doc
            .get("ok")
            .and_then(Value::as_bool)
            .context("proof-check report missing bool field \"ok\"")?,
        proof_object_digest: required_str("proof_object_digest")?,
        checker: required_str("checker")?,
        result: required_str("result")?,
        symbol: required_str("symbol")?,
        entry_symbol: required_str("entry_symbol")?,
        verify_engine: required_str("verify_engine")?,
        expected_obligation_digest: required_str("expected_obligation_digest")?,
        replayed_obligation_digest: required_str("replayed_obligation_digest")?,
        expected_solver_result: required_str("expected_solver_result")?,
        replayed_solver_result: required_str("replayed_solver_result")?,
        validated_imported_proof_summary_digests: doc
            .get("validated_imported_proof_summary_digests")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        validated_scheduler_model_digest: doc
            .get("validated_scheduler_model_digest")
            .and_then(Value::as_str)
            .map(str::to_string),
        diagnostics: Vec::new(),
    })
}

pub(crate) fn check_proof_object_path(proof_path: &Path) -> Result<VerifyProofCheckReport> {
    let (proof_digest, object) = match load_proof_object_path(proof_path) {
        Ok(loaded) => loaded,
        Err(err) => {
            let proof_digest = std::fs::read(proof_path)
                .map(|bytes| prefixed_sha256(&bytes))
                .unwrap_or_else(|_| format!("sha256:{}", "0".repeat(64)));
            return Ok(rejected_proof_check_report(
                proof_digest,
                "X07PROOF_EOBJECT_INVALID",
                format!("{err:#}"),
            ));
        }
    };
    let bundle_dir = proof_path.parent().unwrap_or_else(|| Path::new("."));
    let proof_summary_path = proof_summary_bundle_path(bundle_dir);
    let summary_bytes = match std::fs::read(&proof_summary_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!(
                    "read proof summary {}: {err:#}",
                    proof_summary_path.display()
                ),
                None,
                None,
                Vec::new(),
                None,
            ));
        }
    };
    let bundled_summary_digest = prefixed_sha256(&summary_bytes);
    if bundled_summary_digest != object.proof_summary_digest {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!(
                "bundled proof summary digest mismatch for {}: expected {} got {}",
                proof_summary_path.display(),
                object.proof_summary_digest,
                bundled_summary_digest
            ),
            None,
            None,
            Vec::new(),
            None,
        ));
    }

    let cwd = std::env::current_dir().context("get cwd for proof replay")?;
    let project_path = match resolve_replay_project_manifest(&cwd, proof_path) {
        Ok(path) => path,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                Vec::new(),
                None,
            ));
        }
    };
    let current_project_manifest_digest = match project_manifest_digest(&project_path) {
        Ok(digest) => digest,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                Vec::new(),
                None,
            ));
        }
    };
    if current_project_manifest_digest != object.project_manifest_digest {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!(
                "project manifest digest mismatch: expected {} got {}",
                object.project_manifest_digest, current_project_manifest_digest
            ),
            None,
            None,
            Vec::new(),
            None,
        ));
    }
    let primitive_manifest_digest = verify_primitive_manifest_digest();
    if primitive_manifest_digest != object.primitive_manifest_digest {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!(
                "trusted primitive manifest digest mismatch: expected {} got {}",
                object.primitive_manifest_digest, primitive_manifest_digest
            ),
            None,
            None,
            Vec::new(),
            None,
        ));
    }

    let imported_summary_paths = match load_bundle_imported_summary_paths(bundle_dir) {
        Ok(paths) => paths,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_EIMPORTED_SUMMARY_MISMATCH",
                format!("{err:#}"),
                None,
                None,
                Vec::new(),
                None,
            ));
        }
    };
    let mut bundled_imported_digests = Vec::new();
    for path in &imported_summary_paths {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                return Ok(rejected_proof_check_report_for_object(
                    &object,
                    proof_digest,
                    "X07PROOF_EIMPORTED_SUMMARY_MISMATCH",
                    format!("read imported proof summary {}: {err:#}", path.display()),
                    None,
                    None,
                    bundled_imported_digests,
                    None,
                ));
            }
        };
        bundled_imported_digests.push(prefixed_sha256(&bytes));
    }
    bundled_imported_digests.sort();
    bundled_imported_digests.dedup();
    let mut expected_imported_digests = object.imported_proof_summary_digests.clone();
    expected_imported_digests.sort();
    expected_imported_digests.dedup();
    if bundled_imported_digests != expected_imported_digests {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_EIMPORTED_SUMMARY_MISMATCH",
            format!(
                "bundled imported proof-summary digests mismatch: expected {:?} got {:?}",
                expected_imported_digests, bundled_imported_digests
            ),
            None,
            None,
            bundled_imported_digests,
            None,
        ));
    }

    let project_root = project_path.parent().unwrap_or_else(|| Path::new("."));
    let module_roots = match resolve_module_roots(&cwd, Some(&project_path), &[]) {
        Ok(roots) => roots,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                None,
            ));
        }
    };
    let target = match load_target_info(&module_roots, &object.entry_symbol) {
        Ok(target) => target,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                None,
            ));
        }
    };
    let replay_kind = if target.is_async { "defasync" } else { "defn" };
    if object.symbol != object.entry_symbol
        || object.kind != replay_kind
        || object.decl_sha256_hex != target.decl_sha256_hex
    {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!(
                "replayed declaration mismatch for {:?}: kind={} decl_sha256_hex={}",
                object.entry_symbol, replay_kind, target.decl_sha256_hex
            ),
            None,
            None,
            bundled_imported_digests,
            None,
        ));
    }
    let recursion = match recursion_summary_for_symbol(
        &module_roots,
        coverage_world(Some(&project_path)),
        &object.entry_symbol,
    ) {
        Ok(recursion) => recursion,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                None,
            ));
        }
    };

    let imported_summary_index =
        match load_imported_summary_index(project_root, &imported_summary_paths) {
            Ok(index) => index,
            Err(diags) => {
                let message = diags
                    .first()
                    .map(|diag| diag.message.clone())
                    .unwrap_or_else(|| "imported proof-summary replay failed".to_string());
                return Ok(rejected_proof_check_report_for_object(
                    &object,
                    proof_digest,
                    "X07PROOF_EIMPORTED_SUMMARY_MISMATCH",
                    message,
                    None,
                    None,
                    bundled_imported_digests,
                    None,
                ));
            }
        };
    let replay_args = VerifyArgs {
        bmc: false,
        smt: false,
        prove: true,
        coverage: false,
        entry: object.entry_symbol.clone(),
        project: Some(project_path.clone()),
        module_root: Vec::new(),
        unwind: object.unwind,
        max_bytes_len: object.max_bytes_len,
        input_len_bytes: None,
        artifact_dir: None,
        summary: imported_summary_paths.clone(),
        allow_imported_stubs: true,
        emit_proof: None,
    };
    let analysis = match coverage_report_for_entry(
        &replay_args,
        Some(&project_path),
        &target,
        &imported_summary_index,
        true,
    ) {
        Ok(analysis) => analysis,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                None,
            ));
        }
    };
    if !analysis.diagnostics.is_empty() {
        let diag = &analysis.diagnostics[0];
        let code = if diag.code == "X07V_SUMMARY_MISMATCH"
            || diag.code == "X07V_COVERAGE_SUMMARY_FORBIDDEN"
            || diag.code == "X07V_COVERAGE_NOT_PROOF"
            || diag.code == "X07V_SUMMARY_MISSING"
            || diag.code == "X07V_PROOF_SUMMARY_REQUIRED"
        {
            "X07PROOF_EIMPORTED_SUMMARY_MISMATCH"
        } else {
            "X07PROOF_ESOURCE_REPLAY_FAILED"
        };
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            code,
            diag.message.clone(),
            None,
            None,
            bundled_imported_digests,
            None,
        ));
    }
    let primitive_catalog = match load_verify_primitive_catalog() {
        Ok(catalog) => catalog,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                None,
            ));
        }
    };
    let replayed_summary_artifact = build_verify_proof_summary_artifact(
        &analysis.coverage,
        &analysis.imported_summaries,
        &primitive_catalog,
        &target,
        &recursion,
    );
    if replayed_summary_artifact.engine != object.verify_engine
        || replayed_summary_artifact.recursion_kind != object.recursion_kind
        || replayed_summary_artifact.recursion_bound_kind != object.recursion_bound_kind
    {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!(
                "replayed proof summary metadata mismatch: engine={} recursion_kind={} recursion_bound_kind={}",
                replayed_summary_artifact.engine,
                replayed_summary_artifact.recursion_kind,
                replayed_summary_artifact.recursion_bound_kind
            ),
            None,
            None,
            bundled_imported_digests,
            None,
        ));
    }
    let replayed_summary_bytes =
        match verify_proof_summary_to_pretty_canon_bytes(&replayed_summary_artifact) {
            Ok(bytes) => bytes,
            Err(err) => {
                return Ok(rejected_proof_check_report_for_object(
                    &object,
                    proof_digest,
                    "X07PROOF_ESOURCE_REPLAY_FAILED",
                    format!("{err:#}"),
                    None,
                    None,
                    bundled_imported_digests,
                    None,
                ));
            }
        };
    let replayed_summary_digest = prefixed_sha256(&replayed_summary_bytes);
    if replayed_summary_digest != object.proof_summary_digest {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!(
                "replayed proof summary digest mismatch: expected {} got {}",
                object.proof_summary_digest, replayed_summary_digest
            ),
            None,
            None,
            bundled_imported_digests,
            None,
        ));
    }

    let validated_scheduler_model_digest = if object.kind == "defasync" {
        let digest = verify_scheduler_model_digest();
        if object.scheduler_model_digest.as_deref() != Some(digest.as_str()) {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESCHEDULER_MODEL_MISMATCH",
                format!(
                    "scheduler model digest mismatch: expected {:?} got {}",
                    object.scheduler_model_digest, digest
                ),
                None,
                None,
                bundled_imported_digests,
                Some(digest),
            ));
        }
        Some(digest)
    } else {
        None
    };

    let input_len_bytes = match compute_input_len_bytes(&target, object.max_bytes_len) {
        Ok(value) => value,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                validated_scheduler_model_digest,
            ));
        }
    };
    let replay_temp_dir = match ReplayTempDir::new("x07_proof_replay") {
        Ok(dir) => dir,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                validated_scheduler_model_digest,
            ));
        }
    };
    let build = match build_prove_driver_build(
        &object.entry_symbol,
        &target,
        &module_roots,
        Some(&analysis.coverage),
        object.max_bytes_len,
        input_len_bytes,
        replay_temp_dir.path(),
    ) {
        Ok(build) => build,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                validated_scheduler_model_digest,
            ));
        }
    };
    let driver_path = replay_temp_dir.path().join("driver.x07.json");
    if let Err(err) = util::write_atomic(&driver_path, &build.driver_src) {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!("write replay driver {}: {err:#}", driver_path.display()),
            None,
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    let c_path = replay_temp_dir.path().join("verify.c");
    if let Err(err) = util::write_atomic(&c_path, build.c_with_harness.as_bytes()) {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!("write replay C {}: {err:#}", c_path.display()),
            None,
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    if !command_exists("cbmc") {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            "cbmc is not installed for semantic proof replay".to_string(),
            None,
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    let replay_smt2_path = replay_temp_dir.path().join("verify.smt2");
    let mut cbmc_args = vec![
        c_path.display().to_string(),
        "--function".to_string(),
        VERIFY_HARNESS_FN.to_string(),
        "--unwind".to_string(),
        object.unwind.to_string(),
        "--unwinding-assertions".to_string(),
        "--smt2".to_string(),
        "--outfile".to_string(),
        replay_smt2_path.display().to_string(),
    ];
    maybe_disable_cbmc_standard_checks(&mut cbmc_args);
    let (cbmc_out, _) = match run_cbmc_with_object_bits_retry(&cbmc_args, "run cbmc (proof replay)")
    {
        Ok(out) => out,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("{err:#}"),
                None,
                None,
                bundled_imported_digests,
                validated_scheduler_model_digest,
            ));
        }
    };
    if !cbmc_out.status.success() {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!(
                "cbmc failed to emit replay SMT2: {}",
                summarize_process_failure(
                    &cbmc_out.stdout,
                    &cbmc_out.stderr,
                    PROCESS_SUMMARY_MAX_CHARS
                )
            ),
            None,
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    if !cbmc_out.stderr.is_empty() && !cbmc_stderr_is_benign(&cbmc_out.stderr) {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!(
                "cbmc wrote unexpected stderr during replay: {}",
                summarize_process_text(&cbmc_out.stderr, PROCESS_SUMMARY_MAX_CHARS)
            ),
            None,
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    if let Err(err) = normalize_smt2_logic_for_z3(&replay_smt2_path) {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!("{err:#}"),
            None,
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    if let Err(err) = ensure_smt2_reason_unknown_query(&replay_smt2_path) {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOURCE_REPLAY_FAILED",
            format!("{err:#}"),
            None,
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    let replay_smt2_bytes = match std::fs::read(&replay_smt2_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOURCE_REPLAY_FAILED",
                format!("read replay SMT2 {}: {err:#}", replay_smt2_path.display()),
                None,
                None,
                bundled_imported_digests,
                validated_scheduler_model_digest,
            ));
        }
    };
    let replayed_obligation_digest = prefixed_sha256(&replay_smt2_bytes);
    if replayed_obligation_digest != object.obligation_digest {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_EOBLIGATION_MISMATCH",
            format!(
                "replayed SMT obligation digest mismatch: expected {} got {}",
                object.obligation_digest, replayed_obligation_digest
            ),
            Some(replayed_obligation_digest),
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    if !command_exists("z3") {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOLVER_REPLAY_FAILED",
            "z3 is not installed for semantic proof replay".to_string(),
            Some(replayed_obligation_digest),
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    let z3_out = match Command::new("z3")
        .arg(format!("-T:{}", z3_timeout_seconds(Mode::Prove, &target)))
        .arg("-smt2")
        .arg(&replay_smt2_path)
        .output()
    {
        Ok(out) => out,
        Err(err) => {
            return Ok(rejected_proof_check_report_for_object(
                &object,
                proof_digest,
                "X07PROOF_ESOLVER_REPLAY_FAILED",
                format!("run z3: {err:#}"),
                Some(replayed_obligation_digest),
                None,
                bundled_imported_digests,
                validated_scheduler_model_digest,
            ));
        }
    };
    if !z3_out.status.success() {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOLVER_REPLAY_FAILED",
            format!(
                "z3 failed during replay: {}",
                summarize_process_text(&z3_out.stderr, PROCESS_SUMMARY_MAX_CHARS)
            ),
            Some(replayed_obligation_digest),
            None,
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    let replayed_solver_result = z3_out
        .stdout
        .split(|byte| *byte == b'\n')
        .next()
        .and_then(|line| std::str::from_utf8(line).ok())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("unsat")
        .to_string();
    if replayed_solver_result != object.expected_solver_result {
        return Ok(rejected_proof_check_report_for_object(
            &object,
            proof_digest,
            "X07PROOF_ESOLVER_REPLAY_FAILED",
            format!(
                "replayed solver result mismatch: expected {} got {}",
                object.expected_solver_result, replayed_solver_result
            ),
            Some(replayed_obligation_digest),
            Some(replayed_solver_result),
            bundled_imported_digests,
            validated_scheduler_model_digest,
        ));
    }
    Ok(accepted_proof_check_report(
        &object,
        proof_digest,
        replayed_obligation_digest,
        replayed_solver_result,
        bundled_imported_digests,
        validated_scheduler_model_digest,
    ))
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
            proof_summary: None,
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
            proof_summary: None,
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
            proof_summary: None,
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
            proof_summary: None,
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
            proof_summary: None,
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
            proof_summary: None,
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
            proof_summary: None,
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
            proof_summary: None,
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

    fn with_proof_summary(mut self, proof_summary: VerifyProofSummary) -> Self {
        self.proof_summary = Some(proof_summary);
        self
    }

    fn with_diagnostics(mut self, diagnostics: Vec<x07c::diagnostics::Diagnostic>) -> Self {
        self.diagnostics_count = diagnostics.len() as u64;
        self.diagnostics = diagnostics;
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

fn run_cbmc_with_object_bits_retry(
    cbmc_args: &[String],
    context: &'static str,
) -> Result<(Output, Vec<String>)> {
    run_cbmc_command_with_object_bits_retry("cbmc", cbmc_args, context)
}

fn run_cbmc_command_with_object_bits_retry(
    cbmc_command: &str,
    cbmc_args: &[String],
    context: &'static str,
) -> Result<(Output, Vec<String>)> {
    let mut used_args = cbmc_args.to_vec();
    let mut out = Command::new(cbmc_command)
        .args(&used_args)
        .output()
        .context(context)?;
    if !cbmc_too_many_addressed_objects(&out.stdout, &out.stderr)
        || used_args.iter().any(|arg| arg == "--object-bits")
    {
        return Ok((out, used_args));
    }

    for object_bits in CBMC_OBJECT_BITS_RETRY_VALUES {
        let mut retry_args = cbmc_args.to_vec();
        retry_args.push("--object-bits".to_string());
        retry_args.push(object_bits.to_string());
        out = Command::new(cbmc_command)
            .args(&retry_args)
            .output()
            .context(context)?;
        used_args = retry_args;
        if !cbmc_too_many_addressed_objects(&out.stdout, &out.stderr) {
            break;
        }
    }

    Ok((out, used_args))
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

fn cbmc_too_many_addressed_objects(stdout: &[u8], stderr: &[u8]) -> bool {
    let needle = "too many addressed objects";
    String::from_utf8_lossy(stdout).contains(needle)
        || String::from_utf8_lossy(stderr).contains(needle)
}

fn z3_timeout_seconds(mode: Mode, target: &TargetSig) -> u64 {
    if mode == Mode::Prove && target.is_async {
        Z3_ASYNC_PROVE_TIMEOUT_SECONDS
    } else {
        Z3_TIMEOUT_SECONDS
    }
}

fn cbmc_stderr_is_benign(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr);
    let mut saw_line = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        saw_line = true;
        if !line.starts_with("**** WARNING: no body for function __builtin_trap") {
            return false;
        }
    }
    saw_line
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

fn validate_verify_summary_schema(value: &Value) -> Result<Vec<x07c::diagnostics::Diagnostic>> {
    let diags = report_common::validate_schema(
        X07_VERIFY_SUMMARY_SCHEMA_BYTES,
        "spec/x07-verify.summary.schema.json",
        value,
    )?;
    Ok(diags)
}

fn validate_verify_proof_summary_schema(
    value: &Value,
) -> Result<Vec<x07c::diagnostics::Diagnostic>> {
    let diags = report_common::validate_schema(
        X07_VERIFY_PROOF_SUMMARY_SCHEMA_BYTES,
        "spec/x07-verify.proof-summary.schema.json",
        value,
    )?;
    Ok(diags)
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

fn diag_verify_with_severity(
    code: &str,
    severity: x07c::diagnostics::Severity,
    message: impl Into<String>,
) -> x07c::diagnostics::Diagnostic {
    x07c::diagnostics::Diagnostic {
        code: code.to_string(),
        severity,
        stage: x07c::diagnostics::Stage::Run,
        message: message.into(),
        loc: None,
        notes: Vec::new(),
        related: Vec::new(),
        data: std::collections::BTreeMap::new(),
        quickfix: None,
    }
}

fn diag_verify(code: &str, message: impl Into<String>) -> x07c::diagnostics::Diagnostic {
    diag_verify_with_severity(code, x07c::diagnostics::Severity::Error, message)
}

fn diag_verify_warning(code: &str, message: impl Into<String>) -> x07c::diagnostics::Diagnostic {
    diag_verify_with_severity(code, x07c::diagnostics::Severity::Warning, message)
}

fn rejected_proof_check_report(
    proof_object_digest: String,
    code: &str,
    message: String,
) -> VerifyProofCheckReport {
    VerifyProofCheckReport {
        schema_version: X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_VERSION.to_string(),
        ok: false,
        proof_object_digest,
        checker: "x07.proof_replay_checker".to_string(),
        result: "rejected".to_string(),
        symbol: "unknown".to_string(),
        entry_symbol: "unknown".to_string(),
        verify_engine: "unknown".to_string(),
        expected_obligation_digest: format!("sha256:{}", "0".repeat(64)),
        replayed_obligation_digest: format!("sha256:{}", "0".repeat(64)),
        expected_solver_result: "unknown".to_string(),
        replayed_solver_result: "unknown".to_string(),
        validated_imported_proof_summary_digests: Vec::new(),
        validated_scheduler_model_digest: None,
        diagnostics: vec![diag_verify(code, message)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write as _;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FAKE_COMMAND_SEQ: AtomicU64 = AtomicU64::new(0);

    fn write_fake_command(script_body: &str) -> PathBuf {
        let seq = FAKE_COMMAND_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "x07_verify_fake_command_{}_{}_{}",
            std::process::id(),
            seq,
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

    fn temp_test_dir(label: &str) -> PathBuf {
        let seq = FAKE_COMMAND_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "x07_verify_tests_{}_{}_{}",
            label,
            std::process::id(),
            seq
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_module(path: &Path, module_id: &str, imports: &[&str], decls: Vec<Value>) {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({
                "schema_version": X07AST_SCHEMA_VERSION,
                "kind": "module",
                "module_id": module_id,
                "imports": imports,
                "decls": decls
            }))
            .expect("serialize module"),
        )
        .expect("write module");
    }

    fn write_valid_proof_bundle(dir: &Path) -> PathBuf {
        let proof_path = dir.join("proof.json");
        let smt2_path = dir.join("source.smt2");
        let solver_path = dir.join("source.z3.out.txt");
        std::fs::write(
            dir.join("x07.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": "x07.project@0.4.0",
                "name": "proof_fixture",
                "version": "0.1.0",
                "language": { "edition": "2025" },
                "module_roots": ["src"],
                "world": "solve-pure"
            }))
            .expect("serialize project manifest"),
        )
        .expect("write project manifest");
        std::fs::write(&smt2_path, "(set-logic AUFBV)\n(check-sat)\n").expect("write smt2");
        std::fs::write(&solver_path, "unsat\n").expect("write solver transcript");
        let summary = VerifyProofSummaryArtifact {
            schema_version: X07_VERIFY_PROOF_SUMMARY_SCHEMA_VERSION.to_string(),
            summary_kind: "proof".to_string(),
            symbol: "example.main".to_string(),
            kind: "defn".to_string(),
            decl_sha256_hex: "11".repeat(32),
            result_kind: "proven".to_string(),
            engine: "z3".to_string(),
            recursion_kind: "none".to_string(),
            recursion_bound_kind: "none".to_string(),
            dependency_symbols: Vec::new(),
            proof_object_digest: None,
            assumptions: Vec::new(),
        };
        write_prove_bundle_artifacts(
            &proof_path,
            &summary,
            &[],
            &dir.join("x07.json"),
            &Bounds {
                unwind: 8,
                max_bytes_len: 16,
                input_len_bytes: 0,
            },
            &smt2_path,
            &solver_path,
        )
        .expect("write proof bundle");
        proof_path
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
    fn cbmc_too_many_addressed_objects_detects_known_exhaustion_message() {
        assert!(cbmc_too_many_addressed_objects(
            b"",
            b"too many addressed objects: maximum number of objects is set to 2^n=256"
        ));
        assert!(cbmc_too_many_addressed_objects(
            b"too many addressed objects",
            b""
        ));
        assert!(!cbmc_too_many_addressed_objects(b"", b"unrelated failure"));
    }

    #[test]
    fn z3_timeout_seconds_uses_longer_budget_for_async_prove() {
        let async_target = TargetSig {
            param_names: Vec::new(),
            params: Vec::new(),
            result: "bytes".to_string(),
            result_brand: None,
            decl_sha256_hex: "00".repeat(32),
            is_async: true,
            has_contracts: true,
            decreases_count: 0,
            decreases: Vec::new(),
            body: json!(0),
            source_path: PathBuf::from("async_fixture.x07.json"),
        };
        let sync_target = TargetSig {
            is_async: false,
            source_path: PathBuf::from("sync_fixture.x07.json"),
            ..async_target.clone()
        };
        assert_eq!(
            z3_timeout_seconds(Mode::Prove, &async_target),
            Z3_ASYNC_PROVE_TIMEOUT_SECONDS
        );
        assert_eq!(
            z3_timeout_seconds(Mode::Smt, &async_target),
            Z3_TIMEOUT_SECONDS
        );
        assert_eq!(
            z3_timeout_seconds(Mode::Prove, &sync_target),
            Z3_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn run_cbmc_with_object_bits_retry_retries_exhaustion() {
        let fake = write_fake_command(
            r#"
prev=''
for arg in "$@"; do
  if [ "$prev" = "--object-bits" ] && [ "$arg" = "12" ]; then
    printf '%s\n' 'ok after retry'
    exit 0
  fi
  prev="$arg"
done
printf '%s\n' 'too many addressed objects: maximum number of objects is set to 2^n=256' >&2
exit 1
"#,
        );
        let result = run_cbmc_command_with_object_bits_retry(
            fake.to_str().expect("utf-8 fake path"),
            &["--flag".to_string()],
            "run fake cbmc",
        )
        .expect("run fake cbmc");
        assert!(result.0.status.success());
        assert_eq!(
            result.1,
            vec![
                "--flag".to_string(),
                "--object-bits".to_string(),
                "12".to_string()
            ]
        );
        std::fs::remove_file(&fake).expect("remove fake command");
        std::fs::remove_dir(fake.parent().expect("fake parent")).expect("remove temp dir");
    }

    #[test]
    fn cbmc_stderr_is_benign_for_builtin_trap_warning() {
        assert!(cbmc_stderr_is_benign(
            b"**** WARNING: no body for function __builtin_trap\n"
        ));
        assert!(cbmc_stderr_is_benign(
            b"\n**** WARNING: no body for function __builtin_trap\n\n"
        ));
        assert!(!cbmc_stderr_is_benign(
            b"**** WARNING: no body for function __builtin_trap\nunexpected\n"
        ));
        assert!(!cbmc_stderr_is_benign(b""));
    }

    #[test]
    fn verify_driver_imports_std_codec_and_uses_std_codec_read() {
        let sig = TargetSig {
            param_names: vec!["x".to_string()],
            params: vec![VerifySignatureParam {
                ty: "i32".to_string(),
                brand: None,
            }],
            result: "bytes".to_string(),
            result_brand: None,
            decl_sha256_hex: "11".repeat(32),
            is_async: false,
            has_contracts: true,
            decreases_count: 0,
            decreases: Vec::new(),
            body: json!(0),
            source_path: PathBuf::from("verify_fixture.x07.json"),
        };
        let bytes = build_verify_driver_x07ast_json(&[], "verify_fixture.f", &sig, 16, false)
            .expect("build driver");
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
    fn resolve_module_roots_adds_toolchain_stdlib_roots_for_project() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/trusted_sandbox_program_v1/x07.json");
        let cwd = manifest.parent().expect("example dir").to_path_buf();
        let roots = resolve_module_roots(&cwd, Some(&manifest), &[]).expect("module roots");
        assert!(
            roots
                .iter()
                .any(|root| root.join("std/os/env.x07.json").is_file()),
            "expected stdlib os module root in {:?}",
            roots
        );
    }

    #[test]
    fn resolve_module_roots_dedupes_equivalent_paths() {
        let seq = FAKE_COMMAND_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "x07_verify_module_roots_{}_{}",
            std::process::id(),
            seq
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("verify_fixture.x07.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": X07AST_SCHEMA_VERSION,
                "kind": "module",
                "module_id": "verify_fixture",
                "imports": [],
                "decls": [
                    {"kind":"export", "names":["verify_fixture.f"]},
                    {"kind":"defn", "name":"verify_fixture.f", "params": [], "result":"i32", "body": 0}
                ]
            }))
            .expect("serialize module"),
        )
        .expect("write module");

        let expected_path = dir.join("verify_fixture.x07.json");
        let roots =
            resolve_module_roots(&dir, None, &[dir.clone(), dir.join(".")]).expect("module roots");
        assert_eq!(
            roots.len(),
            1,
            "expected duplicate roots to collapse: {roots:?}"
        );
        let source =
            x07c::module_source::load_module_source("verify_fixture", WorldId::SolvePure, &roots)
                .expect("load module from deduped roots");
        let expected_path = std::fs::canonicalize(&expected_path).expect("canonicalize module");
        assert_eq!(source.path.as_deref(), Some(expected_path.as_path()));

        std::fs::remove_file(&expected_path).expect("remove module");
        std::fs::remove_dir(&dir).expect("remove temp dir");
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
    fn find_recursive_termination_failure_flags_non_decreasing_self_call() {
        let target = TargetSig {
            param_names: vec!["n".to_string()],
            params: vec![VerifySignatureParam {
                ty: "i32".to_string(),
                brand: None,
            }],
            result: "i32".to_string(),
            result_brand: None,
            decl_sha256_hex: "22".repeat(32),
            is_async: false,
            has_contracts: true,
            decreases_count: 1,
            decreases: vec![json!("n")],
            body: json!(["if", ["=", "n", 0], 0, ["verify_fixture.f", "n"]]),
            source_path: PathBuf::from("verify_fixture.x07.json"),
        };
        assert_eq!(
            find_recursive_termination_failure(&target, "verify_fixture.f"),
            Some("recursive self-call does not obviously decrease \"n\"".to_string())
        );
    }

    #[test]
    fn find_recursive_termination_failure_accepts_subtracting_literal_step() {
        let target = TargetSig {
            param_names: vec!["n".to_string()],
            params: vec![VerifySignatureParam {
                ty: "i32".to_string(),
                brand: None,
            }],
            result: "i32".to_string(),
            result_brand: None,
            decl_sha256_hex: "33".repeat(32),
            is_async: false,
            has_contracts: true,
            decreases_count: 1,
            decreases: vec![json!("n")],
            body: json!(["if", ["=", "n", 0], 0, ["verify_fixture.f", ["-", "n", 1]]]),
            source_path: PathBuf::from("verify_fixture.x07.json"),
        };
        assert_eq!(
            find_recursive_termination_failure(&target, "verify_fixture.f"),
            None
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

    #[test]
    fn smt2_has_solver_query_detects_missing_check_sat() {
        let dir =
            std::env::temp_dir().join(format!("x07_verify_smt2_query_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("verify.smt2");
        std::fs::write(&path, "(set-logic QF_AUFBV)\n(assert true)\n").expect("write smt2");

        assert!(!smt2_has_solver_query(&path).expect("check missing query"));

        std::fs::write(&path, "(set-logic QF_AUFBV)\n(check-sat)\n").expect("write smt2");
        assert!(smt2_has_solver_query(&path).expect("check present query"));
        std::fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn load_imported_summary_index_rejects_coverage_summary_as_proof() {
        let dir = temp_test_dir("coverage_summary_forbidden");
        let summary_path = dir.join("verify.summary.json");
        let coverage = VerifyCoverage {
            schema_version: X07_VERIFY_COVERAGE_SCHEMA_VERSION,
            entry: "example.main".to_string(),
            worlds: vec!["solve-pure".to_string()],
            summary: VerifyCoverageSummary {
                reachable_defn: 1,
                supported_defn: 1,
                recursive_defn: 0,
                supported_recursive_defn: 0,
                imported_proof_summary_defn: 0,
                termination_proven_defn: 0,
                unsupported_recursive_defn: 0,
                reachable_async: 0,
                supported_async: 0,
                trusted_primitives: 0,
                trusted_scheduler_models: 0,
                capsule_boundaries: 0,
                uncovered_defn: 0,
                unsupported_defn: 0,
                async_model: None,
            },
            functions: Vec::new(),
        };
        write_verify_summary_artifact(&summary_path, &coverage, &[]).expect("write summary");

        let diagnostics =
            load_imported_summary_index(Path::new("."), &[summary_path]).expect_err("reject");
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, "X07V_COVERAGE_NOT_PROOF");
        assert_eq!(
            diagnostics[0].severity,
            x07c::diagnostics::Severity::Warning
        );
        assert_eq!(diagnostics[1].code, "X07V_COVERAGE_SUMMARY_FORBIDDEN");

        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn coverage_report_for_entry_requires_proof_summary_in_prove_mode() {
        let dir = temp_test_dir("proof_summary_required");
        write_module(
            &dir.join("app.x07.json"),
            "app",
            &[],
            vec![
                json!({"kind":"export","names":["app.main"]}),
                json!({
                    "kind":"defn",
                    "name":"app.main",
                    "params": [],
                    "result":"i32",
                    "body": ["vendor.dep.helper"]
                }),
            ],
        );
        let args = VerifyArgs {
            bmc: false,
            smt: false,
            prove: true,
            coverage: false,
            entry: "app.main".to_string(),
            project: None,
            module_root: vec![dir.clone()],
            unwind: 8,
            max_bytes_len: 16,
            input_len_bytes: None,
            artifact_dir: None,
            summary: Vec::new(),
            allow_imported_stubs: false,
            emit_proof: None,
        };
        let target = load_target_info(std::slice::from_ref(&dir), "app.main").expect("target");
        let analysis =
            coverage_report_for_entry(&args, None, &target, &ImportedSummaryIndex::default(), true)
                .expect("coverage analysis");

        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diag| diag.code == "X07V_PROOF_SUMMARY_REQUIRED"),
            "{:?}",
            analysis.diagnostics
        );

        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn check_proof_object_path_reports_invalid_object_diagnostic() {
        let dir = temp_test_dir("invalid_proof_object");
        let proof_path = dir.join("proof.json");
        std::fs::write(&proof_path, "{}\n").expect("write proof object");

        let report = check_proof_object_path(&proof_path).expect("check proof");
        assert!(!report.ok);
        assert_eq!(report.result, "rejected");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "X07PROOF_EOBJECT_INVALID");

        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn check_proof_object_path_reports_rejected_check_diagnostic() {
        let dir = temp_test_dir("proof_check_failed");
        let proof_path = write_valid_proof_bundle(&dir);
        std::fs::write(dir.join("z3.out.txt"), "unsat\n").expect("tamper solver transcript");

        let report = check_proof_object_path(&proof_path).expect("check proof");
        assert!(!report.ok);
        assert_eq!(report.result, "rejected");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "X07PROOF_ESOURCE_REPLAY_FAILED");

        std::fs::remove_dir_all(dir).expect("cleanup temp dir");
    }

    #[test]
    fn resolve_replay_project_manifest_prefers_cwd_before_proof_ancestors() {
        let cwd_dir = temp_test_dir("replay_manifest_cwd");
        let proof_root = temp_test_dir("replay_manifest_proof");
        std::fs::write(cwd_dir.join("x07.json"), "{}\n").expect("write cwd manifest");
        std::fs::write(proof_root.join("x07.json"), "{}\n").expect("write proof-root manifest");
        let proof_dir = proof_root.join("bundle");
        std::fs::create_dir_all(&proof_dir).expect("create proof dir");
        let proof_path = proof_dir.join("proof.json");
        std::fs::write(&proof_path, "{}\n").expect("write proof object");

        let resolved =
            resolve_replay_project_manifest(&cwd_dir, &proof_path).expect("resolve manifest");
        assert_eq!(resolved, cwd_dir.join("x07.json"));

        std::fs::remove_dir_all(cwd_dir).expect("cleanup cwd dir");
        std::fs::remove_dir_all(proof_root).expect("cleanup proof dir");
    }
}
