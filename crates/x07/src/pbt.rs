use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;
use jsonschema::Resource;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07_contracts::{X07AST_SCHEMA_VERSION, X07_PBT_REPRO_SCHEMA_VERSION};
use x07_host_runner::{run_artifact_file, RunnerConfig, RunnerResult};
use x07_worlds::WorldId;
use x07c::x07ast;

use crate::repro::ToolInfo;

const X07_PBT_REPRO_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.pbt.repro@0.1.0.schema.json");
const X07_PBT_PARAMS_SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../spec/x07.pbt.params@0.1.0.schema.json");

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PbtDeclRaw {
    #[serde(default)]
    pub cases: Option<u32>,
    #[serde(default)]
    pub max_shrinks: Option<u32>,
    #[serde(default)]
    pub params: Vec<PbtParamRaw>,
    #[serde(default)]
    pub case_budget: Option<PbtCaseBudgetRaw>,
    #[serde(default)]
    pub budget_scope: Option<PbtBudgetScopeRaw>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PbtParamRaw {
    pub name: String,
    pub gen: PbtGenRaw,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PbtGenRaw {
    pub kind: String,
    #[serde(default)]
    pub min: Option<i32>,
    #[serde(default)]
    pub max: Option<i32>,
    #[serde(default)]
    pub max_len: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PbtCaseBudgetRaw {
    #[serde(default)]
    pub fuel: Option<u64>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub max_mem_bytes: Option<u64>,
    #[serde(default)]
    pub max_output_bytes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PbtBudgetScopeRaw {
    #[serde(default)]
    pub alloc_bytes: Option<u64>,
    #[serde(default)]
    pub alloc_calls: Option<u64>,
    #[serde(default)]
    pub realloc_calls: Option<u64>,
    #[serde(default)]
    pub memcpy_bytes: Option<u64>,
    #[serde(default)]
    pub sched_ticks: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PbtTy {
    I32,
    Bytes,
}

#[derive(Debug, Clone)]
pub(crate) enum PbtGen {
    I32 { min: i32, max: i32 },
    Bytes { max_len: u32 },
}

impl PbtGen {
    pub(crate) fn ty(&self) -> PbtTy {
        match self {
            PbtGen::I32 { .. } => PbtTy::I32,
            PbtGen::Bytes { .. } => PbtTy::Bytes,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PbtParam {
    pub name: String,
    pub gen: PbtGen,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct PbtCaseBudget {
    pub fuel: u64,
    pub timeout_ms: u64,
    pub max_mem_bytes: u64,
    pub max_output_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PbtBudgetScope {
    pub alloc_bytes: i32,
    pub alloc_calls: i32,
    pub realloc_calls: i32,
    pub memcpy_bytes: i32,
    pub sched_ticks: i32,
}

impl PbtBudgetScope {
    fn is_active(self) -> bool {
        self.alloc_bytes > 0
            || self.alloc_calls > 0
            || self.realloc_calls > 0
            || self.memcpy_bytes > 0
            || self.sched_ticks > 0
    }
}

pub(crate) fn checked_u64_to_i32(field: &'static str, v: u64) -> Result<i32> {
    let max = i32::MAX as u64;
    if v > max {
        anyhow::bail!("{field} must be <= {max}, got {v}");
    }
    Ok(v as i32)
}

#[derive(Debug, Clone)]
pub(crate) struct PbtDecl {
    pub cases: u32,
    pub max_shrinks: u32,
    pub params: Vec<PbtParam>,
    pub case_budget: PbtCaseBudget,
    pub budget_scope: Option<PbtBudgetScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParamValue {
    I32(i32),
    Bytes(Vec<u8>),
}

impl ParamValue {
    fn ty(&self) -> PbtTy {
        match self {
            ParamValue::I32(_) => PbtTy::I32,
            ParamValue::Bytes(_) => PbtTy::Bytes,
        }
    }
}

pub(crate) fn build_case_call_begin_expr(
    entry: &str,
    tys: &[PbtTy],
    budget_scope: Option<PbtBudgetScope>,
) -> Result<(Vec<String>, Value)> {
    let (module_id, _name) = entry.rsplit_once('.').context("entry must contain '.'")?;

    let mut imports: Vec<String> = vec!["std.codec".to_string(), "std.test".to_string()];
    if module_id != "std.codec" && module_id != "std.test" {
        imports.push(module_id.to_string());
    }
    imports.sort();
    imports.dedup();

    let expected_n = tys.len() as i32;
    let fail_code = serde_json::json!(["std.test.code_fail_generic"]);
    let fail_status = serde_json::json!(["std.test.status_fail", fail_code]);

    let mut stmts: Vec<Value> = Vec::new();
    stmts.push(serde_json::json!([
        "let",
        "n",
        ["codec.read_u32_le", "input", 0]
    ]));
    stmts.push(serde_json::json!([
        "if",
        ["=", "n", expected_n],
        0,
        ["return", fail_status]
    ]));

    // payload_base = 4 + 4 * (n + 1)
    stmts.push(serde_json::json!([
        "let",
        "payload_base",
        ["+", 4, ["*", 4, ["+", "n", 1]]]
    ]));

    for (i, ty) in tys.iter().enumerate() {
        let i_i32 = i as i32;
        let off_i_name = format!("off{i}");
        let off_j_name = format!("off{}", i + 1);
        let slice_name = format!("s{i}");
        let arg_name = format!("a{i}");

        let off_i_pos = 4 + 4 * i_i32;
        let off_j_pos = 4 + 4 * (i_i32 + 1);

        stmts.push(serde_json::json!([
            "let",
            off_i_name,
            ["codec.read_u32_le", "input", off_i_pos]
        ]));
        stmts.push(serde_json::json!([
            "let",
            off_j_name,
            ["codec.read_u32_le", "input", off_j_pos]
        ]));

        stmts.push(serde_json::json!([
            "let",
            slice_name,
            [
                "view.slice",
                "input",
                ["+", "payload_base", off_i_name],
                ["-", off_j_name, off_i_name]
            ]
        ]));

        let value_expr = match *ty {
            PbtTy::I32 => serde_json::json!(["codec.read_u32_le", slice_name, 0]),
            PbtTy::Bytes => serde_json::json!(["view.to_bytes", slice_name]),
        };
        stmts.push(serde_json::json!(["let", arg_name, value_expr]));
    }

    let mut call_items: Vec<Value> = Vec::with_capacity(1 + tys.len());
    call_items.push(Value::String(entry.to_string()));
    for i in 0..tys.len() {
        call_items.push(Value::String(format!("a{i}")));
    }
    let call_expr = Value::Array(call_items);

    let call_expr = if let Some(scope) = budget_scope.filter(|s| s.is_active()) {
        let mut cfg_items: Vec<Value> = Vec::new();
        cfg_items.push(Value::String("budget.cfg_v1".to_string()));
        cfg_items.push(serde_json::json!(["mode", "trap_v1"]));
        cfg_items.push(serde_json::json!(["label", ["bytes.lit", "pbt_case"]]));
        if scope.alloc_bytes > 0 {
            cfg_items.push(serde_json::json!(["alloc_bytes", scope.alloc_bytes]));
        }
        if scope.alloc_calls > 0 {
            cfg_items.push(serde_json::json!(["alloc_calls", scope.alloc_calls]));
        }
        if scope.realloc_calls > 0 {
            cfg_items.push(serde_json::json!(["realloc_calls", scope.realloc_calls]));
        }
        if scope.memcpy_bytes > 0 {
            cfg_items.push(serde_json::json!(["memcpy_bytes", scope.memcpy_bytes]));
        }
        if scope.sched_ticks > 0 {
            cfg_items.push(serde_json::json!(["sched_ticks", scope.sched_ticks]));
        }
        serde_json::json!(["budget.scope_v1", Value::Array(cfg_items), call_expr])
    } else {
        call_expr
    };

    stmts.push(call_expr);

    let solve = Value::Array(
        std::iter::once(Value::String("begin".to_string()))
            .chain(stmts)
            .collect::<Vec<_>>(),
    );

    Ok((imports, solve))
}

pub(crate) fn build_case_driver_x07ast_json(
    entry: &str,
    tys: &[PbtTy],
    budget_scope: Option<PbtBudgetScope>,
) -> Result<Vec<u8>> {
    let (imports, solve) = build_case_call_begin_expr(entry, tys, budget_scope)?;

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

pub(crate) fn derive_seed_u64(test_id: &str, suite_seed_u64: u64) -> u64 {
    // FNV-1a 64-bit.
    const OFFSET_BASIS: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;

    let mut h = OFFSET_BASIS;
    for &b in format!("x07:pbt:{test_id}").as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h ^ suite_seed_u64
}

pub(crate) fn lcg_next_u32(state: u32) -> u32 {
    state.wrapping_mul(1103515245).wrapping_add(12345)
}

fn seed_to_lcg_state(seed: u64) -> u32 {
    (seed as u32) ^ ((seed >> 32) as u32)
}

fn next_u32(state: &mut u32) -> u32 {
    *state = lcg_next_u32(*state);
    *state
}

fn next_bounded_u32(state: &mut u32, bound: u32) -> u32 {
    if bound <= 1 {
        return 0;
    }

    let zone = u32::MAX - (u32::MAX % bound);
    loop {
        let v = next_u32(state);
        if v < zone {
            return v % bound;
        }
    }
}

fn gen_i32(state: &mut u32, min: i32, max: i32, size: u32) -> i32 {
    let size_i32 = size.min(i32::MAX as u32) as i32;
    let (min, max) = {
        let bmin = min.max(-size_i32);
        let bmax = max.min(size_i32);
        if bmin <= bmax {
            (bmin, bmax)
        } else {
            (min, max)
        }
    };

    if min == i32::MIN && max == i32::MAX {
        return next_u32(state) as i32;
    }
    if min == max {
        return min;
    }

    let span_u64 = (max as i64 - min as i64 + 1) as u64;
    let span_u32: u32 = span_u64.try_into().unwrap_or(u32::MAX);
    let r = next_bounded_u32(state, span_u32) as i64;
    (min as i64 + r) as i32
}

fn gen_bytes(state: &mut u32, max_len: u32, size: u32) -> Vec<u8> {
    let len = max_len.min(size) as usize;
    let mut out = vec![0u8; len];
    let mut i = 0;
    while i < len {
        let v = next_u32(state).to_le_bytes();
        let n = (len - i).min(4);
        out[i..i + n].copy_from_slice(&v[..n]);
        i += n;
    }
    out
}

pub(crate) fn encode_case_bytes(values: &[ParamValue]) -> Vec<u8> {
    let n = values.len() as u32;
    let mut payload: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(values.len() + 1);
    offsets.push(0);
    for v in values {
        match v {
            ParamValue::I32(x) => payload.extend_from_slice(&x.to_le_bytes()),
            ParamValue::Bytes(b) => payload.extend_from_slice(b),
        }
        offsets.push(payload.len() as u32);
    }

    let mut out = Vec::with_capacity(4 + offsets.len() * 4 + payload.len());
    out.extend_from_slice(&n.to_le_bytes());
    for off in offsets {
        out.extend_from_slice(&off.to_le_bytes());
    }
    out.extend_from_slice(&payload);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureKind {
    Assert,
    Trap,
    Timeout,
    Fuel,
    Nondeterminism,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PbtRepro {
    pub schema_version: String,
    pub tool: ToolInfo,
    pub test: TestInfo,
    pub suite_seed_u64: u64,
    pub effective_seed_u64: u64,
    pub cases: CasesInfo,
    pub shrinking: ShrinkingInfo,
    pub failure: FailureInfo,
    pub counterexample: CounterexampleInfo,
    pub budget: PbtCaseBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TestInfo {
    pub id: String,
    pub entry: String,
    pub world: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CasesInfo {
    pub attempted: u32,
    pub configured: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ShrinkingInfo {
    pub attempted: u32,
    pub limit: u32,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FailureInfo {
    pub kind: FailureKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assert_code_u32: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trap_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trap_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CounterexampleInfo {
    pub params: Vec<CounterexampleParam>,
    pub case_bytes_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CounterexampleParam {
    pub name: String,
    pub ty: PbtTy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub len: Option<u32>,
}

#[derive(Debug)]
pub(crate) enum ReproJsonError {
    JsonParse(serde_json::Error),
    SchemaVersion { expected: &'static str, got: String },
    SchemaValidatorBuild(String),
    SchemaInvalid(String),
    Decode(serde_json::Error),
}

pub(crate) fn parse_repro_json_detailed(
    bytes: &[u8],
) -> std::result::Result<PbtRepro, ReproJsonError> {
    let doc: Value = serde_json::from_slice(bytes).map_err(ReproJsonError::JsonParse)?;
    let schema_version = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema_version != X07_PBT_REPRO_SCHEMA_VERSION {
        return Err(ReproJsonError::SchemaVersion {
            expected: X07_PBT_REPRO_SCHEMA_VERSION,
            got: schema_version.to_string(),
        });
    }

    // Best-effort schema validation to catch drift/typos early.
    let schema_json: Value =
        serde_json::from_slice(X07_PBT_REPRO_SCHEMA_BYTES).map_err(ReproJsonError::JsonParse)?;
    let params_schema_json: Value =
        serde_json::from_slice(X07_PBT_PARAMS_SCHEMA_BYTES).map_err(ReproJsonError::JsonParse)?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_resource(
            "https://x07.io/spec/x07.pbt.params@0.1.0.schema.json",
            Resource::from_contents(params_schema_json),
        )
        .build(&schema_json)
        .map_err(|e| ReproJsonError::SchemaValidatorBuild(e.to_string()))?;
    if let Some(err) = validator.iter_errors(&doc).next() {
        return Err(ReproJsonError::SchemaInvalid(err.to_string()));
    }

    serde_json::from_value(doc).map_err(ReproJsonError::Decode)
}

pub(crate) fn parse_repro_json(bytes: &[u8]) -> Result<PbtRepro> {
    match parse_repro_json_detailed(bytes) {
        Ok(repro) => Ok(repro),
        Err(ReproJsonError::JsonParse(err)) => Err(err).context("parse repro JSON"),
        Err(ReproJsonError::SchemaVersion { expected, got }) => {
            anyhow::bail!("unsupported repro schema_version: expected {expected} got {got:?}");
        }
        Err(ReproJsonError::SchemaValidatorBuild(err)) => {
            Err(anyhow::anyhow!(err)).context("build PBT repro schema validator")
        }
        Err(ReproJsonError::SchemaInvalid(err)) => anyhow::bail!("invalid repro JSON: {err}"),
        Err(ReproJsonError::Decode(err)) => Err(err).context("decode repro JSON"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PbtObservation {
    pub attempted: u32,
    pub shrink_attempted: u32,
    pub failure_kind: Option<FailureKind>,
    pub assert_code: Option<u32>,
    pub trap_message: Option<String>,
    pub case_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PbtSuiteRun {
    pub final_run: RunnerResult,
    pub status_tag: Option<u8>,
    pub status_code_u32: Option<u32>,
    pub repro: Option<PbtRepro>,
}

fn classify_runner_trap(trap: Option<&str>) -> (FailureKind, Option<String>, Option<String>) {
    let Some(trap) = trap else {
        return (FailureKind::Trap, None, None);
    };
    if trap == "wall timeout" {
        return (FailureKind::Timeout, None, Some(trap.to_string()));
    }
    if trap == "fuel exhausted" || trap == "X07T_BUDGET_EXCEEDED_FUEL" {
        return (
            FailureKind::Fuel,
            Some(trap.to_string())
                .filter(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')),
            Some(trap.to_string()),
        );
    }
    let trap_id = if trap
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && trap.len() <= 128
    {
        Some(trap.to_string())
    } else {
        None
    };
    (FailureKind::Trap, trap_id, Some(trap.to_string()))
}

fn eval_case(
    exe: &Path,
    base_cfg: &RunnerConfig,
    budget: &PbtCaseBudget,
    input: &[u8],
) -> Result<RunnerResult> {
    let mut cfg = base_cfg.clone();
    cfg.solve_fuel = budget.fuel;
    cfg.max_memory_bytes = budget.max_mem_bytes as usize;
    cfg.max_output_bytes = budget.max_output_bytes as usize;
    cfg.cpu_time_limit_seconds = crate::ms_to_ceiling_seconds(budget.timeout_ms)?;
    run_artifact_file(&cfg, exe, input)
}

fn shrink_candidates_i32(v: i32) -> Vec<i32> {
    if v == 0 {
        return Vec::new();
    }
    let mut out: Vec<i32> = Vec::new();
    let mut seen: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();

    let push = |x: i32, out: &mut Vec<i32>, seen: &mut std::collections::BTreeSet<i32>| {
        if x == v {
            return;
        }
        if seen.insert(x) {
            out.push(x);
        }
    };

    // Order: 0, then halving toward 0, then step-by-1 toward 0.
    push(0, &mut out, &mut seen);

    let mut cur = v / 2;
    while cur != 0 {
        push(cur, &mut out, &mut seen);
        cur /= 2;
    }

    if v > 0 {
        for x in (1..v).rev() {
            push(x, &mut out, &mut seen);
        }
    } else {
        for x in (v + 1)..0 {
            push(x, &mut out, &mut seen);
        }
    }

    out
}

fn shrink_candidates_bytes(b: &[u8]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut seen: std::collections::BTreeSet<Vec<u8>> = std::collections::BTreeSet::new();
    let len = b.len();
    if len == 0 {
        return out;
    }

    // Shorten suffix.
    let mut l = len / 2;
    while l < len {
        let v = b[..l].to_vec();
        if seen.insert(v.clone()) {
            out.push(v);
        }
        if l == 0 {
            break;
        }
        l /= 2;
    }

    // Shrink bytes left-to-right.
    for i in 0..len {
        let orig = b[i];
        let mut candidates: Vec<u8> = Vec::new();
        candidates.push(0);
        candidates.push(orig / 2);
        if orig > 0 {
            candidates.push(orig - 1);
        }

        for cand in candidates {
            if cand == orig {
                continue;
            }
            let mut tmp = b.to_vec();
            tmp[i] = cand;
            if seen.insert(tmp.clone()) {
                out.push(tmp);
            }
        }
    }

    out.retain(|x| x.as_slice() != b);
    out
}

fn shrink_candidates_param(v: &ParamValue) -> Vec<ParamValue> {
    match v {
        ParamValue::I32(x) => shrink_candidates_i32(*x)
            .into_iter()
            .map(ParamValue::I32)
            .collect(),
        ParamValue::Bytes(b) => shrink_candidates_bytes(b)
            .into_iter()
            .map(ParamValue::Bytes)
            .collect(),
    }
}

fn values_match_params(values: &[ParamValue], params: &[PbtParam]) -> bool {
    values.len() == params.len() && values.iter().zip(params).all(|(v, p)| v.ty() == p.gen.ty())
}

pub(crate) struct RunPbtSuiteArgs<'a> {
    pub exe: &'a Path,
    pub base_cfg: &'a RunnerConfig,
    pub test_id: &'a str,
    pub entry: &'a str,
    pub world: WorldId,
    pub params: &'a [PbtParam],
    pub budget: &'a PbtCaseBudget,
    pub suite_seed_u64: u64,
    pub cases: u32,
    pub max_shrinks: u32,
}

#[derive(Debug, Clone)]
struct CaseEvalOutcome {
    fails: bool,
    run: RunnerResult,
    status_tag: Option<u8>,
    status_code_u32: Option<u32>,
    failure_kind: Option<FailureKind>,
    trap_message: Option<String>,
    trap_id: Option<String>,
}

pub(crate) fn run_pbt_suite(args: RunPbtSuiteArgs<'_>) -> Result<(PbtSuiteRun, PbtObservation)> {
    let RunPbtSuiteArgs {
        exe,
        base_cfg,
        test_id,
        entry,
        world,
        params,
        budget,
        suite_seed_u64,
        cases,
        max_shrinks,
    } = args;

    let effective_seed_u64 = derive_seed_u64(test_id, suite_seed_u64);
    let mut state = seed_to_lcg_state(effective_seed_u64);

    let mut attempted_cases: u32 = 0;
    let mut last_run: Option<RunnerResult> = None;
    let mut failure_values: Option<Vec<ParamValue>> = None;
    let mut failure_kind: Option<FailureKind> = None;
    let mut failure_assert_code: Option<u32> = None;
    let mut failure_trap_id: Option<String> = None;
    let mut failure_trap: Option<String> = None;

    for case_idx in 0..cases {
        let size = case_idx.saturating_add(1);
        let mut values: Vec<ParamValue> = Vec::with_capacity(params.len());
        for p in params {
            match p.gen {
                PbtGen::I32 { min, max } => {
                    values.push(ParamValue::I32(gen_i32(&mut state, min, max, size)))
                }
                PbtGen::Bytes { max_len } => {
                    values.push(ParamValue::Bytes(gen_bytes(&mut state, max_len, size)))
                }
            }
        }

        let case_bytes = encode_case_bytes(&values);
        let run = eval_case(exe, base_cfg, budget, &case_bytes)?;
        attempted_cases = attempted_cases.saturating_add(1);
        last_run = Some(run.clone());

        if !run.ok {
            let (k, trap_id, trap_msg) = classify_runner_trap(run.trap.as_deref());
            failure_kind = Some(k);
            failure_trap_id = trap_id;
            failure_trap = trap_msg;
            failure_values = Some(values);
            break;
        }

        let (tag, code_u32) = crate::parse_evtest_status_v1(&run.solve_output)
            .context("parse std.test.status_v1 output")?;

        match tag {
            1 => {}
            0 => {
                failure_kind = Some(FailureKind::Assert);
                failure_assert_code = Some(code_u32);
                failure_values = Some(values);
                break;
            }
            other => {
                anyhow::bail!(
                    "unsupported std.test.status_v1 tag from property: {}",
                    other
                );
            }
        }
    }

    let Some(failure_values) = failure_values else {
        let run = last_run.context("internal error: missing last run")?;
        let (tag, code_u32) = crate::parse_evtest_status_v1(&run.solve_output)
            .context("parse std.test.status_v1 output")?;
        let suite = PbtSuiteRun {
            final_run: run,
            status_tag: Some(tag),
            status_code_u32: Some(code_u32),
            repro: None,
        };
        let obs = PbtObservation {
            attempted: attempted_cases,
            shrink_attempted: 0,
            failure_kind: None,
            assert_code: None,
            trap_message: None,
            case_bytes: None,
        };
        return Ok((suite, obs));
    };

    let mut current = failure_values;
    let mut shrink_attempted: u32 = 0;

    let still_fails = |candidate: &[ParamValue]| -> Result<CaseEvalOutcome> {
        if !values_match_params(candidate, params) {
            anyhow::bail!("internal error: candidate values do not match param types");
        }
        let case_bytes = encode_case_bytes(candidate);
        let run = eval_case(exe, base_cfg, budget, &case_bytes)?;
        if !run.ok {
            let (k, trap_id, trap_msg) = classify_runner_trap(run.trap.as_deref());
            return Ok(CaseEvalOutcome {
                fails: true,
                run,
                status_tag: None,
                status_code_u32: None,
                failure_kind: Some(k),
                trap_message: trap_msg,
                trap_id,
            });
        }
        let (tag, code_u32) = crate::parse_evtest_status_v1(&run.solve_output)?;
        match tag {
            1 => Ok(CaseEvalOutcome {
                fails: false,
                run,
                status_tag: Some(tag),
                status_code_u32: Some(code_u32),
                failure_kind: None,
                trap_message: None,
                trap_id: None,
            }),
            0 => Ok(CaseEvalOutcome {
                fails: true,
                run,
                status_tag: Some(tag),
                status_code_u32: Some(code_u32),
                failure_kind: Some(FailureKind::Assert),
                trap_message: None,
                trap_id: None,
            }),
            other => anyhow::bail!(
                "unsupported std.test.status_v1 tag from property: {}",
                other
            ),
        }
    };

    let mut changed = true;
    while changed && shrink_attempted < max_shrinks {
        changed = false;
        for i in 0..current.len() {
            loop {
                let mut improved = false;
                for cand in shrink_candidates_param(&current[i]) {
                    if shrink_attempted >= max_shrinks {
                        break;
                    }
                    shrink_attempted = shrink_attempted.saturating_add(1);

                    let mut tmp = current.clone();
                    tmp[i] = cand;
                    let outcome = still_fails(&tmp)?;
                    if outcome.fails {
                        current = tmp;
                        improved = true;
                        changed = true;
                        break;
                    }
                }
                if !improved {
                    break;
                }
            }
        }
    }

    let case_bytes = encode_case_bytes(&current);
    let final_outcome = still_fails(&current)?;
    if !final_outcome.fails {
        anyhow::bail!("internal error: shrink produced non-failing counterexample");
    }

    let final_run = final_outcome.run;
    let tag = final_outcome.status_tag;
    let code_u32 = final_outcome.status_code_u32;
    let k = final_outcome.failure_kind;
    let trap_msg = final_outcome.trap_message;
    let trap_id2 = final_outcome.trap_id;

    let kind = k.or(failure_kind).unwrap_or(FailureKind::Trap);
    let mut assert_code = failure_assert_code;
    let mut trap_id = failure_trap_id;
    let mut trap_message = failure_trap;
    if let Some(code) = code_u32 {
        if matches!(kind, FailureKind::Assert) {
            assert_code = Some(code);
        }
    }
    if trap_msg.is_some() {
        trap_message = trap_msg;
    }
    if trap_id2.is_some() {
        trap_id = trap_id2;
    }

    let counterexample_params: Vec<CounterexampleParam> = params
        .iter()
        .zip(&current)
        .map(|(p, v)| match v {
            ParamValue::I32(x) => CounterexampleParam {
                name: p.name.clone(),
                ty: PbtTy::I32,
                value: Some(*x),
                b64: None,
                len: None,
            },
            ParamValue::Bytes(b) => CounterexampleParam {
                name: p.name.clone(),
                ty: PbtTy::Bytes,
                value: None,
                b64: Some(base64::engine::general_purpose::STANDARD.encode(b)),
                len: Some(b.len() as u32),
            },
        })
        .collect();

    let repro = PbtRepro {
        schema_version: X07_PBT_REPRO_SCHEMA_VERSION.to_string(),
        tool: crate::repro::tool_info(),
        test: TestInfo {
            id: test_id.to_string(),
            entry: entry.to_string(),
            world: world.as_str().to_string(),
        },
        suite_seed_u64,
        effective_seed_u64,
        cases: CasesInfo {
            attempted: attempted_cases,
            configured: cases,
        },
        shrinking: ShrinkingInfo {
            attempted: shrink_attempted,
            limit: max_shrinks,
            result: if shrink_attempted < max_shrinks {
                "minimal_found".to_string()
            } else {
                "limit_hit".to_string()
            },
        },
        failure: FailureInfo {
            kind,
            assert_code_u32: assert_code,
            trap_id,
            trap_message,
        },
        counterexample: CounterexampleInfo {
            params: counterexample_params,
            case_bytes_b64: base64::engine::general_purpose::STANDARD.encode(&case_bytes),
        },
        budget: *budget,
    };

    let (status_tag, status_code_u32) = match kind {
        FailureKind::Assert => (tag, code_u32),
        _ => (None, None),
    };

    let suite = PbtSuiteRun {
        final_run: final_run.clone(),
        status_tag,
        status_code_u32,
        repro: Some(repro.clone()),
    };

    let obs = PbtObservation {
        attempted: attempted_cases,
        shrink_attempted,
        failure_kind: Some(kind),
        assert_code,
        trap_message: repro.failure.trap_message.clone(),
        case_bytes: Some(case_bytes),
    };
    Ok((suite, obs))
}

pub(crate) fn repro_to_pretty_canon_bytes(repro: &PbtRepro) -> Result<Vec<u8>> {
    let mut v = serde_json::to_value(repro)?;
    x07ast::canon_value_jcs(&mut v);
    let mut bytes = serde_json::to_vec_pretty(&v)?;
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok(bytes)
}

pub(crate) fn repro_to_details_value(repro: &PbtRepro) -> Result<Value> {
    let mut v = serde_json::to_value(repro)?;
    x07ast::canon_value_jcs(&mut v);
    Ok(v)
}

pub(crate) fn counterexample_case_bytes(repro: &PbtRepro) -> Result<Vec<u8>> {
    let b64 = base64::engine::general_purpose::STANDARD;
    b64.decode(repro.counterexample.case_bytes_b64.as_bytes())
        .context("decode counterexample.case_bytes_b64")
}

pub(crate) fn counterexample_tys(repro: &PbtRepro) -> Vec<PbtTy> {
    repro.counterexample.params.iter().map(|p| p.ty).collect()
}

#[allow(dead_code)]
pub(crate) fn counterexample_param_names(repro: &PbtRepro) -> Vec<String> {
    repro
        .counterexample
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect()
}

pub(crate) fn validate_repro_test_matches_manifest(
    repro: &PbtRepro,
    test_id: &str,
    entry: &str,
) -> Result<()> {
    if repro.test.id != test_id {
        anyhow::bail!(
            "repro test.id mismatch: expected {:?} got {:?}",
            test_id,
            repro.test.id
        );
    }
    if repro.test.entry != entry {
        anyhow::bail!(
            "repro test.entry mismatch: expected {:?} got {:?}",
            entry,
            repro.test.entry
        );
    }
    Ok(())
}

pub(crate) fn test_id_from_repro(bytes: &[u8]) -> Result<String> {
    let doc: Value = serde_json::from_slice(bytes).context("parse repro JSON")?;
    let schema_version = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema_version != X07_PBT_REPRO_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported repro schema_version: expected {} got {:?}",
            X07_PBT_REPRO_SCHEMA_VERSION,
            schema_version
        );
    }
    let id = doc
        .get("test")
        .and_then(Value::as_object)
        .and_then(|t| t.get("id"))
        .and_then(Value::as_str)
        .context("repro missing test.id")?;
    Ok(id.to_string())
}
