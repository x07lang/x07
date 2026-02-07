use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use jsonschema::Draft;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07_contracts::X07_SM_SPEC_SCHEMA_VERSION;
use x07_worlds::WorldId;
use x07c::ast::Expr;
use x07c::program::{FunctionDef, FunctionParam};
use x07c::types::Ty;
use x07c::x07ast::{X07AstFile, X07AstKind};

use crate::util;

const X07_SM_SPEC_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07-sm.spec.schema.json");
const X07_TESTS_MANIFEST_SCHEMA_VERSION: &str = "x07.tests_manifest@0.1.0";

// Stable error codes for generated v1 machines.
const SM_ERR_INVALID_SNAPSHOT_V1: i32 = 1;
const SM_ERR_INVALID_EVENT_V1: i32 = 2;
const SM_ERR_NO_TRANSITION_V1: i32 = 3;
const SM_ERR_INVALID_ACTION_RESULT_V1: i32 = 4;
const SM_ERR_BUDGET_EXCEEDED_V1: i32 = 5;

#[derive(Debug, Args)]
pub struct SmArgs {
    #[command(subcommand)]
    pub cmd: Option<SmCommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum SmCommand {
    /// Validate an SM spec file.
    Check(SmCheckArgs),
    /// Generate X07 modules from an SM spec file.
    Gen(SmGenArgs),
}

#[derive(Debug, Args)]
pub struct SmCheckArgs {
    #[arg(long, value_name = "PATH")]
    pub input: PathBuf,
}

#[derive(Debug, Args)]
pub struct SmGenArgs {
    #[arg(long, value_name = "PATH")]
    pub input: PathBuf,

    #[arg(long)]
    pub write: bool,

    #[arg(long)]
    pub check: bool,

    /// Repository root for canonicalizing embedded paths.
    /// When set, source_contract_path is stored relative to this directory.
    #[arg(long, value_name = "DIR")]
    pub repo_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmSpecFile {
    schema_version: String,
    machine_id: String,
    version: i32,
    world: String,
    #[serde(default)]
    brand: Option<String>,
    #[serde(default)]
    states: Vec<SmState>,
    #[serde(default)]
    events: Vec<SmEvent>,
    #[serde(default)]
    transitions: Vec<SmTransition>,
    #[serde(default)]
    context: Option<SmContext>,
    #[serde(default)]
    budgets: Option<SmBudgets>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmState {
    id: i32,
    name: String,
    terminal: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmEvent {
    id: i32,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmTransition {
    id: i32,
    from: i32,
    on: i32,
    to: i32,
    action: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmContext {
    codec: String,
    max_ctx_bytes: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmBudgets {
    max_events_per_step: i32,
    max_actions_per_step: i32,
    max_cmds_per_step: i32,
    max_cmd_bytes: i32,
}

#[derive(Debug, Serialize)]
struct SmCheckReport {
    schema_version: String,
    ok: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<String>,
}

pub fn cmd_sm(
    machine: &crate::reporting::MachineArgs,
    args: SmArgs,
) -> Result<std::process::ExitCode> {
    let Some(cmd) = args.cmd else {
        anyhow::bail!("missing sm subcommand (try --help)");
    };

    match cmd {
        SmCommand::Check(args) => cmd_sm_check(args),
        SmCommand::Gen(args) => cmd_sm_gen(machine, args),
    }
}

fn cmd_sm_check(args: SmCheckArgs) -> Result<std::process::ExitCode> {
    let (spec, errors) = load_and_validate_spec(&args.input)?;
    let ok = errors.is_empty();

    let report = SmCheckReport {
        schema_version: "x07.sm.check@0.1.0".to_string(),
        ok,
        errors,
    };
    println!("{}", serde_json::to_string(&report)?);

    if ok {
        // Ensure we don't warn about unused parsed spec.
        let _ = spec;
        Ok(std::process::ExitCode::SUCCESS)
    } else {
        Ok(std::process::ExitCode::from(2))
    }
}

fn cmd_sm_gen(
    machine: &crate::reporting::MachineArgs,
    args: SmGenArgs,
) -> Result<std::process::ExitCode> {
    if args.write && args.check {
        anyhow::bail!("sm gen: choose exactly one of --write or --check");
    }
    if !args.write && !args.check {
        anyhow::bail!("sm gen: missing mode (pass --write or --check)");
    }

    let (spec, errors) = load_and_validate_spec(&args.input)?;
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("{e}");
        }
        return Ok(std::process::ExitCode::from(2));
    }

    let input_rel = if let Some(ref repo_root) = args.repo_root {
        let abs_input = std::fs::canonicalize(&args.input)
            .with_context(|| format!("canonicalize input: {}", args.input.display()))?;
        let abs_root = std::fs::canonicalize(repo_root)
            .with_context(|| format!("canonicalize repo-root: {}", repo_root.display()))?;
        match abs_input.strip_prefix(&abs_root) {
            Ok(rel) => normalize_rel_path_for_meta(rel),
            Err(_) => anyhow::bail!(
                "input {} is not under repo-root {}",
                args.input.display(),
                repo_root.display()
            ),
        }
    } else {
        normalize_rel_path_for_meta(&args.input)
    };
    let spec_doc: Value = serde_json::from_slice(
        &std::fs::read(&args.input).with_context(|| format!("read: {}", args.input.display()))?,
    )
    .with_context(|| format!("parse JSON: {}", args.input.display()))?;
    let spec_hash = util::sha256_hex(&util::canonical_jcs_bytes(&spec_doc)?);

    let out_dir = machine
        .out
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("sm gen: missing --out <DIR>"))?
        .clone();
    let out = render_sm_artifacts(&spec, &out_dir, &input_rel, &spec_hash)?;

    if args.check {
        let mut failed = false;
        for (path, want) in out {
            let got = std::fs::read(&path)
                .with_context(|| format!("read generated file for --check: {}", path.display()))
                .unwrap_or_default();
            if got != want {
                eprintln!(
                    "sm gen: output mismatch (re-run with --write): {}",
                    path.display()
                );
                failed = true;
            }
        }
        return Ok(if failed {
            std::process::ExitCode::from(2)
        } else {
            std::process::ExitCode::SUCCESS
        });
    }

    for (path, bytes) in out {
        util::write_atomic(&path, &bytes).with_context(|| format!("write: {}", path.display()))?;
    }
    // Ensure out_dir exists (for users expecting `gen/sm/`).
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("create out_dir: {}", out_dir.display()))?;

    Ok(std::process::ExitCode::SUCCESS)
}

fn normalize_rel_path_for_meta(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    s.trim_start_matches("./").to_string()
}

fn load_and_validate_spec(path: &Path) -> Result<(SmSpecFile, Vec<String>)> {
    let bytes = std::fs::read(path).with_context(|| format!("read: {}", path.display()))?;
    let doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse JSON: {}", path.display()))?;

    let mut errors: Vec<String> = Vec::new();

    // Schema validation (deterministic, machine-readable).
    let schema: Value =
        serde_json::from_slice(X07_SM_SPEC_SCHEMA_BYTES).context("parse SM spec schema")?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .context("build SM spec schema validator")?;
    for e in validator.iter_errors(&doc) {
        errors.push(format!("schema: {} at {}", e, e.instance_path()));
    }

    let spec: SmSpecFile = match serde_json::from_value(doc.clone()) {
        Ok(s) => s,
        Err(err) => {
            errors.push(format!("parse: {err}"));
            // Return a minimal placeholder so callers can continue consistently.
            return Ok((
                SmSpecFile {
                    schema_version: String::new(),
                    machine_id: String::new(),
                    version: 0,
                    world: String::new(),
                    brand: None,
                    states: Vec::new(),
                    events: Vec::new(),
                    transitions: Vec::new(),
                    context: None,
                    budgets: None,
                },
                errors,
            ));
        }
    };

    validate_sm_spec(&spec, &mut errors);
    errors.sort();

    Ok((spec, errors))
}

fn validate_sm_spec(spec: &SmSpecFile, errors: &mut Vec<String>) {
    if spec.schema_version.trim() != X07_SM_SPEC_SCHEMA_VERSION {
        errors.push(format!(
            "schema_version mismatch: got {:?} expected {:?}",
            spec.schema_version, X07_SM_SPEC_SCHEMA_VERSION
        ));
    }

    if let Err(msg) = x07c::validate::validate_module_id(&spec.machine_id) {
        errors.push(format!("machine_id invalid: {msg}"));
    }
    if spec.version < 1 {
        errors.push("version must be >= 1".to_string());
    }
    if WorldId::parse(&spec.world).is_none() {
        errors.push(format!("world is not a known world id: {:?}", spec.world));
    }
    if let Some(brand) = spec.brand.as_deref() {
        if let Err(msg) = x07c::validate::validate_symbol(brand) {
            errors.push(format!("brand invalid: {msg}"));
        }
    }

    let mut state_ids: BTreeSet<i32> = BTreeSet::new();
    for s in &spec.states {
        if s.id < 0 {
            errors.push(format!("state.id must be >= 0 (got {})", s.id));
        }
        if !state_ids.insert(s.id) {
            errors.push(format!("duplicate state id: {}", s.id));
        }
        if s.name.trim().is_empty() {
            errors.push(format!("state {} name must be non-empty", s.id));
        }
    }
    if !state_ids.contains(&0) {
        errors.push("missing initial state id 0".to_string());
    }

    let mut event_ids: BTreeSet<i32> = BTreeSet::new();
    for e in &spec.events {
        if e.id < 0 {
            errors.push(format!("event.id must be >= 0 (got {})", e.id));
        }
        if !event_ids.insert(e.id) {
            errors.push(format!("duplicate event id: {}", e.id));
        }
        if e.name.trim().is_empty() {
            errors.push(format!("event {} name must be non-empty", e.id));
        }
    }

    let mut transition_ids: BTreeSet<i32> = BTreeSet::new();
    let mut keys: BTreeSet<(i32, i32)> = BTreeSet::new();
    for t in &spec.transitions {
        if t.id < 0 {
            errors.push(format!("transition.id must be >= 0 (got {})", t.id));
        }
        if !transition_ids.insert(t.id) {
            errors.push(format!("duplicate transition id: {}", t.id));
        }
        if !state_ids.contains(&t.from) {
            errors.push(format!("transition {} unknown from state {}", t.id, t.from));
        }
        if !state_ids.contains(&t.to) {
            errors.push(format!("transition {} unknown to state {}", t.id, t.to));
        }
        if !event_ids.contains(&t.on) {
            errors.push(format!("transition {} unknown event {}", t.id, t.on));
        }
        if !keys.insert((t.from, t.on)) {
            errors.push(format!(
                "duplicate transition key (from={}, on={})",
                t.from, t.on
            ));
        }
        if let Err(msg) = x07c::validate::validate_symbol(&t.action) {
            errors.push(format!("transition {} action invalid: {msg}", t.id));
        }
    }

    if let Some(ctx) = &spec.context {
        if ctx.max_ctx_bytes < 0 {
            errors.push("context.max_ctx_bytes must be >= 0".to_string());
        }
        if ctx.codec.trim().is_empty() {
            errors.push("context.codec must be non-empty".to_string());
        }
    }
    if let Some(b) = &spec.budgets {
        if b.max_events_per_step != 1 {
            errors.push("budgets.max_events_per_step must be 1 in v1".to_string());
        }
        if b.max_actions_per_step < 0 || b.max_cmds_per_step < 0 || b.max_cmd_bytes < 0 {
            errors.push("budgets fields must be >= 0".to_string());
        }
    }
}

fn canonical_json_bytes(v: &Value) -> Result<Vec<u8>> {
    let mut v = v.clone();
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_vec(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn render_sm_artifacts(
    spec: &SmSpecFile,
    out_dir: &Path,
    spec_rel_path: &str,
    spec_jcs_sha256_hex: &str,
) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let module_id = machine_module_id(spec);
    let tests_module_id = format!("{module_id}.tests");

    let machine_file = sm_module_file_path(out_dir, &module_id)?;
    let tests_file = sm_module_file_path(out_dir, &tests_module_id)?;
    let manifest_file = out_dir.join("tests.manifest.json");

    let machine = gen_machine_module(spec, &module_id, spec_rel_path, spec_jcs_sha256_hex)?;
    let tests = gen_tests_module(
        spec,
        &tests_module_id,
        &module_id,
        spec_rel_path,
        spec_jcs_sha256_hex,
    )?;
    let manifest = gen_tests_manifest(spec, &tests_module_id)?;

    Ok(vec![
        (
            machine_file,
            canonical_json_bytes(&x07c::x07ast::x07ast_file_to_value(&machine))?,
        ),
        (
            tests_file,
            canonical_json_bytes(&x07c::x07ast::x07ast_file_to_value(&tests))?,
        ),
        (manifest_file, canonical_json_bytes(&manifest)?),
    ])
}

fn sm_module_file_path(out_dir: &Path, module_id: &str) -> Result<PathBuf> {
    let parts: Vec<&str> = module_id.split('.').collect();
    if parts.len() < 3 || parts[0] != "gen" || parts[1] != "sm" {
        anyhow::bail!("sm gen: unexpected generated module id: {module_id:?}");
    }
    let rel_parts = &parts[2..];
    let Some((file_stem, subdirs)) = rel_parts.split_last() else {
        anyhow::bail!("sm gen: invalid module id: {module_id:?}");
    };

    let mut path = out_dir.to_path_buf();
    for seg in subdirs {
        path.push(seg);
    }
    path.push(format!("{file_stem}.x07.json"));
    Ok(path)
}

fn machine_module_id(spec: &SmSpecFile) -> String {
    format!("gen.sm.{}_v{}", spec.machine_id, spec.version)
}

fn gen_machine_module(
    spec: &SmSpecFile,
    module_id: &str,
    spec_rel_path: &str,
    spec_jcs_sha256_hex: &str,
) -> Result<X07AstFile> {
    let mut imports: BTreeSet<String> = BTreeSet::new();
    for t in &spec.transitions {
        let (m, _) = split_symbol(&t.action)?;
        imports.insert(m);
    }

    let init_name = format!("{module_id}.init_v1");
    let step_name = format!("{module_id}.step_v1");
    let helper_name = format!("{module_id}._pack_step_result_v1");

    let mut exports: BTreeSet<String> = BTreeSet::new();
    exports.insert(init_name.clone());
    exports.insert(step_name.clone());

    let max_ctx_bytes = spec.context.as_ref().map(|c| c.max_ctx_bytes).unwrap_or(0);
    let max_cmds = spec
        .budgets
        .as_ref()
        .map(|b| b.max_cmds_per_step)
        .unwrap_or(0);
    let max_cmd_bytes = spec.budgets.as_ref().map(|b| b.max_cmd_bytes).unwrap_or(0);

    let init = FunctionDef {
        name: init_name,
        params: Vec::new(),
        ret_ty: Ty::Bytes,
        ret_brand: None,
        body: gen_init_body(max_ctx_bytes),
    };
    let helper = FunctionDef {
        name: helper_name.clone(),
        params: vec![
            FunctionParam {
                name: "to_state".to_string(),
                ty: Ty::I32,
                brand: None,
            },
            FunctionParam {
                name: "action_out".to_string(),
                ty: Ty::BytesView,
                brand: None,
            },
        ],
        ret_ty: Ty::ResultBytes,
        ret_brand: None,
        body: gen_pack_step_result_body(max_ctx_bytes, max_cmds, max_cmd_bytes),
    };
    let step = FunctionDef {
        name: step_name,
        params: vec![
            FunctionParam {
                name: "snapshot".to_string(),
                ty: Ty::BytesView,
                brand: None,
            },
            FunctionParam {
                name: "event".to_string(),
                ty: Ty::BytesView,
                brand: None,
            },
        ],
        ret_ty: Ty::ResultBytes,
        ret_brand: None,
        body: gen_step_body(spec, &helper_name),
    };

    let mut meta: BTreeMap<String, Value> = BTreeMap::new();
    meta.insert(
        "source_contract_path".to_string(),
        Value::String(spec_rel_path.to_string()),
    );
    meta.insert(
        "source_contract_jcs_sha256_hex".to_string(),
        Value::String(spec_jcs_sha256_hex.to_string()),
    );

    let mut file = X07AstFile {
        kind: X07AstKind::Module,
        module_id: module_id.to_string(),
        imports,
        exports,
        functions: vec![init, helper, step],
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: None,
        meta,
    };
    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    Ok(file)
}

fn gen_tests_module(
    spec: &SmSpecFile,
    tests_module_id: &str,
    machine_module_id: &str,
    spec_rel_path: &str,
    spec_jcs_sha256_hex: &str,
) -> Result<X07AstFile> {
    let mut imports: BTreeSet<String> = BTreeSet::new();
    imports.insert(machine_module_id.to_string());
    imports.insert("std.test".to_string());

    let mut exports: BTreeSet<String> = BTreeSet::new();
    let mut functions: Vec<FunctionDef> = Vec::new();

    // One test per transition.
    for t in &spec.transitions {
        let fn_name = format!("{tests_module_id}.transition_{}_v1", t.id);
        exports.insert(fn_name.clone());
        functions.push(FunctionDef {
            name: fn_name,
            params: Vec::new(),
            ret_ty: Ty::ResultI32,
            ret_brand: None,
            body: gen_transition_test_body(spec, machine_module_id, t),
        });
    }

    // Sanity test: init state id is 0.
    let init_test_name = format!("{tests_module_id}.init_state_is_0_v1");
    exports.insert(init_test_name.clone());
    functions.push(FunctionDef {
        name: init_test_name,
        params: Vec::new(),
        ret_ty: Ty::ResultI32,
        ret_brand: None,
        body: gen_init_test_body(spec, machine_module_id),
    });

    let mut meta: BTreeMap<String, Value> = BTreeMap::new();
    meta.insert(
        "source_contract_path".to_string(),
        Value::String(spec_rel_path.to_string()),
    );
    meta.insert(
        "source_contract_jcs_sha256_hex".to_string(),
        Value::String(spec_jcs_sha256_hex.to_string()),
    );

    let mut file = X07AstFile {
        kind: X07AstKind::Module,
        module_id: tests_module_id.to_string(),
        imports,
        exports,
        functions,
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: None,
        meta,
    };
    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    Ok(file)
}

fn gen_tests_manifest(spec: &SmSpecFile, tests_module_id: &str) -> Result<Value> {
    let world = WorldId::parse(&spec.world).unwrap_or(WorldId::SolvePure);
    let mut tests = Vec::new();

    tests.push(serde_json::json!({
        "id": format!("sm/{}/init_state_is_0_v1", spec.machine_id),
        "world": world.as_str(),
        "entry": format!("{tests_module_id}.init_state_is_0_v1"),
        "expect": "pass",
    }));

    for t in &spec.transitions {
        tests.push(serde_json::json!({
            "id": format!("sm/{}/transition/{}", spec.machine_id, t.id),
            "world": world.as_str(),
            "entry": format!("{tests_module_id}.transition_{}_v1", t.id),
            "expect": "pass",
        }));
    }

    Ok(serde_json::json!({
        "schema_version": X07_TESTS_MANIFEST_SCHEMA_VERSION,
        "tests": tests,
    }))
}

fn split_symbol(sym: &str) -> Result<(String, String)> {
    let sym = sym.trim();
    let (m, f) = sym
        .rsplit_once('.')
        .ok_or_else(|| anyhow::anyhow!("invalid symbol (missing '.'): {sym:?}"))?;
    Ok((m.to_string(), f.to_string()))
}

// ----------------------------
// Expr helpers
// ----------------------------

fn e_int(v: i32) -> Expr {
    Expr::Int {
        value: v,
        ptr: String::new(),
    }
}

fn e_ident(s: &str) -> Expr {
    Expr::Ident {
        name: s.to_string(),
        ptr: String::new(),
    }
}

fn e_list(items: Vec<Expr>) -> Expr {
    Expr::List {
        items,
        ptr: String::new(),
    }
}

fn e_call(name: &str, args: Vec<Expr>) -> Expr {
    let mut items = Vec::with_capacity(1 + args.len());
    items.push(e_ident(name));
    items.extend(args);
    e_list(items)
}

fn e_begin(stmts: Vec<Expr>) -> Expr {
    let mut items = Vec::with_capacity(1 + stmts.len());
    items.push(e_ident("begin"));
    items.extend(stmts);
    e_list(items)
}

fn e_let(name: &str, v: Expr) -> Expr {
    e_list(vec![e_ident("let"), e_ident(name), v])
}

fn e_set(name: &str, v: Expr) -> Expr {
    e_list(vec![e_ident("set"), e_ident(name), v])
}

fn e_if(cond: Expr, then_e: Expr, else_e: Expr) -> Expr {
    e_list(vec![e_ident("if"), cond, then_e, else_e])
}

fn e_return(v: Expr) -> Expr {
    e_list(vec![e_ident("return"), v])
}

// ----------------------------
// Generated machine bodies (v1)
// ----------------------------

fn gen_init_body(_max_ctx_bytes: i32) -> Expr {
    // snapshot_v1 = u32le state_id (0) + u32le ctx_len (0) + ctx bytes (empty)
    let mut stmts = Vec::new();
    let out_cap = 8;
    stmts.push(e_let(
        "out",
        e_call("vec_u8.with_capacity", vec![e_int(out_cap)]),
    ));
    stmts.push(e_let("b0", e_call("codec.write_u32_le", vec![e_int(0)])));
    stmts.push(e_set(
        "out",
        e_call(
            "vec_u8.extend_bytes",
            vec![e_ident("out"), e_call("bytes.view", vec![e_ident("b0")])],
        ),
    ));
    stmts.push(e_let("b1", e_call("codec.write_u32_le", vec![e_int(0)])));
    stmts.push(e_set(
        "out",
        e_call(
            "vec_u8.extend_bytes",
            vec![e_ident("out"), e_call("bytes.view", vec![e_ident("b1")])],
        ),
    ));
    stmts.push(e_call("vec_u8.into_bytes", vec![e_ident("out")]));
    e_begin(stmts)
}

fn gen_pack_step_result_body(max_ctx_bytes: i32, max_cmds: i32, max_cmd_bytes: i32) -> Expr {
    // action_result_v1 encoding:
    //   u32_le ctx_len
    //   ctx bytes
    //   u32_le cmds_count
    //   repeat cmds_count: [u32_le cmd_len][cmd bytes]
    //
    // step_result_v1 encoding:
    //   u32_le next_state
    //   u32_le ctx_len
    //   ctx bytes
    //   cmds_count+frames (verbatim)
    let mut stmts = Vec::new();

    stmts.push(e_let("n", e_call("view.len", vec![e_ident("action_out")])));
    stmts.push(e_if(
        e_call("<u", vec![e_ident("n"), e_int(8)]),
        e_return(e_call(
            "result_bytes.err",
            vec![e_int(SM_ERR_INVALID_ACTION_RESULT_V1)],
        )),
        e_int(0),
    ));

    stmts.push(e_let(
        "ctx_len",
        e_call("codec.read_u32_le", vec![e_ident("action_out"), e_int(0)]),
    ));
    if max_ctx_bytes > 0 {
        stmts.push(e_if(
            e_call(">u", vec![e_ident("ctx_len"), e_int(max_ctx_bytes)]),
            e_return(e_call(
                "result_bytes.err",
                vec![e_int(SM_ERR_BUDGET_EXCEEDED_V1)],
            )),
            e_int(0),
        ));
    }

    stmts.push(e_let("ctx_off", e_int(4)));
    stmts.push(e_let(
        "cmds_off",
        e_call("+", vec![e_ident("ctx_off"), e_ident("ctx_len")]),
    ));
    stmts.push(e_if(
        e_call(
            "<u",
            vec![
                e_ident("n"),
                e_call("+", vec![e_ident("cmds_off"), e_int(4)]),
            ],
        ),
        e_return(e_call(
            "result_bytes.err",
            vec![e_int(SM_ERR_INVALID_ACTION_RESULT_V1)],
        )),
        e_int(0),
    ));

    stmts.push(e_let(
        "ctx",
        e_call(
            "view.slice",
            vec![
                e_ident("action_out"),
                e_ident("ctx_off"),
                e_ident("ctx_len"),
            ],
        ),
    ));
    stmts.push(e_let(
        "cmds",
        e_call(
            "view.slice",
            vec![
                e_ident("action_out"),
                e_ident("cmds_off"),
                e_call("-", vec![e_ident("n"), e_ident("cmds_off")]),
            ],
        ),
    ));

    stmts.push(e_let("cmds_n", e_call("view.len", vec![e_ident("cmds")])));
    stmts.push(e_if(
        e_call("<u", vec![e_ident("cmds_n"), e_int(4)]),
        e_return(e_call(
            "result_bytes.err",
            vec![e_int(SM_ERR_INVALID_ACTION_RESULT_V1)],
        )),
        e_int(0),
    ));

    stmts.push(e_let(
        "cmds_count",
        e_call("codec.read_u32_le", vec![e_ident("cmds"), e_int(0)]),
    ));
    if max_cmds > 0 {
        stmts.push(e_if(
            e_call(">u", vec![e_ident("cmds_count"), e_int(max_cmds)]),
            e_return(e_call(
                "result_bytes.err",
                vec![e_int(SM_ERR_BUDGET_EXCEEDED_V1)],
            )),
            e_int(0),
        ));
    }

    // Validate framing: each cmd is [u32_le len][bytes].
    stmts.push(e_let("pos", e_int(4)));
    stmts.push(e_list(vec![
        e_ident("for"),
        e_ident("i"),
        e_int(0),
        e_ident("cmds_count"),
        e_begin(vec![
            e_if(
                e_call(
                    ">=u",
                    vec![
                        e_call("+", vec![e_ident("pos"), e_int(4)]),
                        e_call("+", vec![e_ident("cmds_n"), e_int(1)]),
                    ],
                ),
                e_return(e_call(
                    "result_bytes.err",
                    vec![e_int(SM_ERR_INVALID_ACTION_RESULT_V1)],
                )),
                e_int(0),
            ),
            e_let(
                "cmd_len",
                e_call("codec.read_u32_le", vec![e_ident("cmds"), e_ident("pos")]),
            ),
            if max_cmd_bytes > 0 {
                e_if(
                    e_call(">u", vec![e_ident("cmd_len"), e_int(max_cmd_bytes)]),
                    e_return(e_call(
                        "result_bytes.err",
                        vec![e_int(SM_ERR_BUDGET_EXCEEDED_V1)],
                    )),
                    e_int(0),
                )
            } else {
                e_int(0)
            },
            e_set("pos", e_call("+", vec![e_ident("pos"), e_int(4)])),
            e_if(
                e_call(
                    ">=u",
                    vec![
                        e_call("+", vec![e_ident("pos"), e_ident("cmd_len")]),
                        e_call("+", vec![e_ident("cmds_n"), e_int(1)]),
                    ],
                ),
                e_return(e_call(
                    "result_bytes.err",
                    vec![e_int(SM_ERR_INVALID_ACTION_RESULT_V1)],
                )),
                e_int(0),
            ),
            e_set("pos", e_call("+", vec![e_ident("pos"), e_ident("cmd_len")])),
            e_int(0),
        ]),
    ]));
    stmts.push(e_if(
        e_call("!=", vec![e_ident("pos"), e_ident("cmds_n")]),
        e_return(e_call(
            "result_bytes.err",
            vec![e_int(SM_ERR_INVALID_ACTION_RESULT_V1)],
        )),
        e_int(0),
    ));

    // Build step_result bytes.
    stmts.push(e_let(
        "cap",
        e_call(
            "+",
            vec![
                e_int(8),
                e_call("+", vec![e_ident("ctx_len"), e_ident("cmds_n")]),
            ],
        ),
    ));
    stmts.push(e_let(
        "out",
        e_call("vec_u8.with_capacity", vec![e_ident("cap")]),
    ));
    stmts.push(e_let(
        "b_state",
        e_call("codec.write_u32_le", vec![e_ident("to_state")]),
    ));
    stmts.push(e_set(
        "out",
        e_call(
            "vec_u8.extend_bytes",
            vec![
                e_ident("out"),
                e_call("bytes.view", vec![e_ident("b_state")]),
            ],
        ),
    ));
    stmts.push(e_let(
        "b_ctx_len",
        e_call("codec.write_u32_le", vec![e_ident("ctx_len")]),
    ));
    stmts.push(e_set(
        "out",
        e_call(
            "vec_u8.extend_bytes",
            vec![
                e_ident("out"),
                e_call("bytes.view", vec![e_ident("b_ctx_len")]),
            ],
        ),
    ));
    stmts.push(e_set(
        "out",
        e_call("vec_u8.extend_bytes", vec![e_ident("out"), e_ident("ctx")]),
    ));
    stmts.push(e_set(
        "out",
        e_call("vec_u8.extend_bytes", vec![e_ident("out"), e_ident("cmds")]),
    ));
    stmts.push(e_return(e_call(
        "result_bytes.ok",
        vec![e_call("vec_u8.into_bytes", vec![e_ident("out")])],
    )));

    e_begin(stmts)
}

fn gen_step_body(spec: &SmSpecFile, helper_fn: &str) -> Expr {
    let max_ctx_bytes = spec.context.as_ref().map(|c| c.max_ctx_bytes).unwrap_or(0);

    let mut transitions_by_state: BTreeMap<i32, Vec<&SmTransition>> = BTreeMap::new();
    for t in &spec.transitions {
        transitions_by_state.entry(t.from).or_default().push(t);
    }
    for v in transitions_by_state.values_mut() {
        v.sort_by(|a, b| (a.on, a.id).cmp(&(b.on, b.id)));
    }

    let mut stmts: Vec<Expr> = vec![
        // Parse snapshot.
        e_let("snap_n", e_call("view.len", vec![e_ident("snapshot")])),
        e_if(
            e_call("<u", vec![e_ident("snap_n"), e_int(8)]),
            e_return(e_call(
                "result_bytes.err",
                vec![e_int(SM_ERR_INVALID_SNAPSHOT_V1)],
            )),
            e_int(0),
        ),
        e_let(
            "state_id",
            e_call("codec.read_u32_le", vec![e_ident("snapshot"), e_int(0)]),
        ),
        e_let(
            "ctx_len",
            e_call("codec.read_u32_le", vec![e_ident("snapshot"), e_int(4)]),
        ),
    ];
    if max_ctx_bytes > 0 {
        stmts.push(e_if(
            e_call(">u", vec![e_ident("ctx_len"), e_int(max_ctx_bytes)]),
            e_return(e_call(
                "result_bytes.err",
                vec![e_int(SM_ERR_BUDGET_EXCEEDED_V1)],
            )),
            e_int(0),
        ));
    }
    stmts.push(e_if(
        e_call(
            "<u",
            vec![
                e_ident("snap_n"),
                e_call("+", vec![e_int(8), e_ident("ctx_len")]),
            ],
        ),
        e_return(e_call(
            "result_bytes.err",
            vec![e_int(SM_ERR_INVALID_SNAPSHOT_V1)],
        )),
        e_int(0),
    ));
    stmts.push(e_let(
        "ctx",
        e_call(
            "view.slice",
            vec![e_ident("snapshot"), e_int(8), e_ident("ctx_len")],
        ),
    ));

    // Parse event.
    stmts.push(e_let("event_n", e_call("view.len", vec![e_ident("event")])));
    stmts.push(e_if(
        e_call("<u", vec![e_ident("event_n"), e_int(8)]),
        e_return(e_call(
            "result_bytes.err",
            vec![e_int(SM_ERR_INVALID_EVENT_V1)],
        )),
        e_int(0),
    ));
    stmts.push(e_let(
        "event_id",
        e_call("codec.read_u32_le", vec![e_ident("event"), e_int(0)]),
    ));
    stmts.push(e_let(
        "payload_len",
        e_call("codec.read_u32_le", vec![e_ident("event"), e_int(4)]),
    ));
    stmts.push(e_if(
        e_call(
            "<u",
            vec![
                e_ident("event_n"),
                e_call("+", vec![e_int(8), e_ident("payload_len")]),
            ],
        ),
        e_return(e_call(
            "result_bytes.err",
            vec![e_int(SM_ERR_INVALID_EVENT_V1)],
        )),
        e_int(0),
    ));

    // Transition dispatch: if no match, return ERR.
    let mut state_ifs: Option<Expr> = None;
    for (from_state, transitions) in transitions_by_state.iter().rev() {
        let mut event_ifs: Option<Expr> = None;
        for t in transitions.iter().rev() {
            let action_call = e_call(t.action.as_str(), vec![e_ident("ctx"), e_ident("event")]);
            let branch = e_begin(vec![
                e_let("a_res", action_call),
                e_let("a_out", e_list(vec![e_ident("try"), e_ident("a_res")])),
                e_call(
                    helper_fn,
                    vec![e_int(t.to), e_call("bytes.view", vec![e_ident("a_out")])],
                ),
            ]);

            let cond = e_call("=", vec![e_ident("event_id"), e_int(t.on)]);
            event_ifs = Some(match event_ifs {
                None => e_if(
                    cond,
                    branch,
                    e_call("result_bytes.err", vec![e_int(SM_ERR_NO_TRANSITION_V1)]),
                ),
                Some(prev) => e_if(cond, branch, prev),
            });
        }

        let cond = e_call("=", vec![e_ident("state_id"), e_int(*from_state)]);
        let then_e = event_ifs
            .unwrap_or_else(|| e_call("result_bytes.err", vec![e_int(SM_ERR_NO_TRANSITION_V1)]));
        state_ifs = Some(match state_ifs {
            None => e_if(
                cond,
                then_e,
                e_call("result_bytes.err", vec![e_int(SM_ERR_NO_TRANSITION_V1)]),
            ),
            Some(prev) => e_if(cond, then_e, prev),
        });
    }

    stmts.push(
        state_ifs
            .unwrap_or_else(|| e_call("result_bytes.err", vec![e_int(SM_ERR_NO_TRANSITION_V1)])),
    );
    e_begin(stmts)
}

// ----------------------------
// Generated test bodies (v1)
// ----------------------------

fn gen_init_test_body(_spec: &SmSpecFile, machine_module_id: &str) -> Expr {
    let init_fn = format!("{machine_module_id}.init_v1");

    e_begin(vec![
        e_let("snap", e_call(&init_fn, vec![])),
        e_if(
            e_call(
                "<u",
                vec![e_call("bytes.len", vec![e_ident("snap")]), e_int(8)],
            ),
            e_return(e_call(
                "std.test.fail",
                vec![e_call("std.test.code_assert_true", vec![])],
            )),
            e_int(0),
        ),
        e_let("snap_v", e_call("bytes.view", vec![e_ident("snap")])),
        e_let(
            "state_id",
            e_call("codec.read_u32_le", vec![e_ident("snap_v"), e_int(0)]),
        ),
        e_list(vec![
            e_ident("try"),
            e_call(
                "std.test.assert_i32_eq",
                vec![
                    e_ident("state_id"),
                    e_int(0),
                    e_call("std.test.code_assert_i32_eq", vec![]),
                ],
            ),
        ]),
        e_call("std.test.pass", vec![]),
    ])
}

fn gen_transition_test_body(spec: &SmSpecFile, machine_module_id: &str, t: &SmTransition) -> Expr {
    let step_fn = format!("{machine_module_id}.step_v1");

    // Build snapshot: u32le state + u32le ctx_len(0).
    let build_snapshot = e_begin(vec![
        e_let("v", e_call("vec_u8.with_capacity", vec![e_int(8)])),
        e_let("b0", e_call("codec.write_u32_le", vec![e_int(t.from)])),
        e_set(
            "v",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("v"), e_call("bytes.view", vec![e_ident("b0")])],
            ),
        ),
        e_let("b1", e_call("codec.write_u32_le", vec![e_int(0)])),
        e_set(
            "v",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("v"), e_call("bytes.view", vec![e_ident("b1")])],
            ),
        ),
        e_call("vec_u8.into_bytes", vec![e_ident("v")]),
    ]);

    // Build event: u32le event + u32le payload_len(0).
    let build_event = e_begin(vec![
        e_let("v", e_call("vec_u8.with_capacity", vec![e_int(8)])),
        e_let("b0", e_call("codec.write_u32_le", vec![e_int(t.on)])),
        e_set(
            "v",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("v"), e_call("bytes.view", vec![e_ident("b0")])],
            ),
        ),
        e_let("b1", e_call("codec.write_u32_le", vec![e_int(0)])),
        e_set(
            "v",
            e_call(
                "vec_u8.extend_bytes",
                vec![e_ident("v"), e_call("bytes.view", vec![e_ident("b1")])],
            ),
        ),
        e_call("vec_u8.into_bytes", vec![e_ident("v")]),
    ]);

    let mut stmts = vec![
        e_let("snap", build_snapshot),
        e_let("evt", build_event),
        e_let(
            "res",
            e_call(
                &step_fn,
                vec![
                    e_call("bytes.view", vec![e_ident("snap")]),
                    e_call("bytes.view", vec![e_ident("evt")]),
                ],
            ),
        ),
        e_let(
            "out",
            e_call(
                "result_bytes.unwrap_or",
                vec![e_ident("res"), e_call("bytes.alloc", vec![e_int(0)])],
            ),
        ),
        e_list(vec![
            e_ident("try"),
            e_call(
                "std.test.assert_true",
                vec![
                    e_call(
                        ">=u",
                        vec![e_call("bytes.len", vec![e_ident("out")]), e_int(4)],
                    ),
                    e_call("std.test.code_assert_true", vec![]),
                ],
            ),
        ]),
        e_let("out_v", e_call("bytes.view", vec![e_ident("out")])),
        e_let(
            "next_state",
            e_call("codec.read_u32_le", vec![e_ident("out_v"), e_int(0)]),
        ),
        // If the machine returns SM_ERR_NO_TRANSITION, unwrap_or yields empty and the test fails above.
        e_list(vec![
            e_ident("try"),
            e_call(
                "std.test.assert_i32_eq",
                vec![
                    e_ident("next_state"),
                    e_int(t.to),
                    e_call("std.test.code_assert_i32_eq", vec![]),
                ],
            ),
        ]),
    ];

    // Ensure the machine's definition of init state is stable.
    if !spec.states.iter().any(|s| s.id == 0 && !s.terminal) {
        // No-op: spec doesn't require non-terminal initial state.
        stmts.push(e_int(0));
    }

    stmts.push(e_call("std.test.pass", vec![]));
    e_begin(stmts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_requires_state_0_and_unique_transition_key() {
        let spec = SmSpecFile {
            schema_version: X07_SM_SPEC_SCHEMA_VERSION.to_string(),
            machine_id: "app.sm.test".to_string(),
            version: 1,
            world: "solve-pure".to_string(),
            brand: None,
            states: vec![SmState {
                id: 1,
                name: "only".to_string(),
                terminal: false,
            }],
            events: vec![SmEvent {
                id: 0,
                name: "evt".to_string(),
            }],
            transitions: vec![
                SmTransition {
                    id: 0,
                    from: 1,
                    on: 0,
                    to: 1,
                    action: "app.sm.test.actions.a".to_string(),
                },
                SmTransition {
                    id: 1,
                    from: 1,
                    on: 0,
                    to: 1,
                    action: "app.sm.test.actions.b".to_string(),
                },
            ],
            context: None,
            budgets: Some(SmBudgets {
                max_events_per_step: 1,
                max_actions_per_step: 1,
                max_cmds_per_step: 0,
                max_cmd_bytes: 0,
            }),
        };
        let mut errors = Vec::new();
        validate_sm_spec(&spec, &mut errors);
        assert!(errors
            .iter()
            .any(|e| e.contains("missing initial state id 0")));
        assert!(errors
            .iter()
            .any(|e| e.contains("duplicate transition key")));
    }

    #[test]
    fn gen_outputs_paths_match_module_id_layout() {
        let spec = SmSpecFile {
            schema_version: X07_SM_SPEC_SCHEMA_VERSION.to_string(),
            machine_id: "app.minimal_fsm".to_string(),
            version: 1,
            world: "solve-pure".to_string(),
            brand: None,
            states: vec![
                SmState {
                    id: 0,
                    name: "init".to_string(),
                    terminal: false,
                },
                SmState {
                    id: 1,
                    name: "ready".to_string(),
                    terminal: true,
                },
            ],
            events: vec![SmEvent {
                id: 0,
                name: "start".to_string(),
            }],
            transitions: vec![SmTransition {
                id: 0,
                from: 0,
                on: 0,
                to: 1,
                action: "actions.noop_v1".to_string(),
            }],
            context: None,
            budgets: None,
        };

        let out_dir = std::path::PathBuf::from("gen/sm");
        let out = render_sm_artifacts(&spec, &out_dir, "spec", "deadbeef").expect("render");
        let paths: Vec<_> = out.into_iter().map(|(p, _)| p).collect();

        assert!(
            paths
                .iter()
                .any(|p| p == &out_dir.join("app/minimal_fsm_v1.x07.json")),
            "missing machine module file path; got {paths:?}"
        );
        assert!(
            paths
                .iter()
                .any(|p| p == &out_dir.join("app/minimal_fsm_v1/tests.x07.json")),
            "missing tests module file path; got {paths:?}"
        );
    }

    #[test]
    fn normalize_rel_path_strips_dotslash() {
        assert_eq!(
            normalize_rel_path_for_meta(Path::new("./spec/foo.json")),
            "spec/foo.json"
        );
        assert_eq!(
            normalize_rel_path_for_meta(Path::new("spec/foo.json")),
            "spec/foo.json"
        );
    }
}
