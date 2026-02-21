use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;
use sha2::Digest as _;

use crate::ast::Expr;
use crate::compile::{CompileErrorKind, CompilerError};
use crate::program::{
    AsyncFunctionDef, ExternAbi, ExternFunctionDecl, FunctionDef, FunctionParam, Program,
};
use crate::types::Ty;
use crate::x07ast::{
    canon_value_jcs, type_ref_from_expr, type_ref_to_value, AstAsyncFunctionDef,
    AstExternFunctionDecl, AstFunctionDef, AstFunctionParam, TypeParam, TypeRef,
};

pub const MONO_NAME_MARKER: &str = "__x07_mono_v1__";

#[derive(Debug, Clone, Serialize)]
pub struct MonoLimitsV1 {
    pub max_specializations: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonoStatsV1 {
    pub generic_functions_defined: usize,
    pub specializations_emitted: usize,
    pub tapp_sites_total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonoSiteV1 {
    pub caller: String,
    pub caller_module: String,
    pub ptr: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonoItemV1 {
    pub generic: String,
    pub type_args: Vec<Value>,
    pub specialized: String,
    pub def_module: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sites: Vec<MonoSiteV1>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonoMapV1 {
    pub schema_version: String,
    pub tool: String,
    pub tool_version: String,
    pub input_schema_version: String,
    pub entry_module: String,
    pub limits: MonoLimitsV1,
    pub stats: MonoStatsV1,
    pub items: Vec<MonoItemV1>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub meta: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct GenericProgram {
    pub functions: Vec<AstFunctionDef>,
    pub async_functions: Vec<AstAsyncFunctionDef>,
    pub extern_functions: Vec<AstExternFunctionDecl>,
    pub solve: Expr,
}

const DEFAULT_MAX_SPECIALIZATIONS: usize = 4096;
const DEFAULT_MAX_TYPE_DEPTH: usize = 64;
const MAX_SITES_PER_ITEM: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FnKind {
    Defn,
    Defasync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    Any,
    BytesLike,
    NumLike,
    Value,
    Hashable,
    Orderable,
}

#[derive(Debug, Clone)]
struct TypeParamSig {
    name: String,
    bound: Option<Bound>,
}

#[derive(Debug, Clone)]
struct FnSig {
    kind: FnKind,
    type_params: Vec<TypeParamSig>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct InstantiationKey {
    generic: String,
    type_args_key: String,
}

#[derive(Debug, Clone)]
struct InstanceRecord {
    kind: FnKind,
    def_module: String,
    type_args: Vec<TypeRef>,
    specialized: String,
    sites: Vec<MonoSiteV1>,
    generated: bool,
}

#[derive(Debug, Clone)]
struct RewriteCtx {
    caller: String,
    caller_module: String,
}

pub fn monomorphize(
    program: GenericProgram,
    module_exports: &BTreeMap<String, BTreeSet<String>>,
    input_schema_version: &str,
) -> Result<(Program, MonoMapV1), CompilerError> {
    let max_specializations = DEFAULT_MAX_SPECIALIZATIONS;
    let max_type_depth = DEFAULT_MAX_TYPE_DEPTH;

    let mut generic_fn_defs: BTreeMap<String, AstFunctionDef> = BTreeMap::new();
    let mut generic_async_defs: BTreeMap<String, AstAsyncFunctionDef> = BTreeMap::new();
    let mut non_generic_fn_defs: Vec<AstFunctionDef> = Vec::new();
    let mut non_generic_async_defs: Vec<AstAsyncFunctionDef> = Vec::new();
    let mut sigs: BTreeMap<String, FnSig> = BTreeMap::new();
    let mut generic_symbols: BTreeSet<String> = BTreeSet::new();
    let mut declared_symbols: BTreeSet<String> = BTreeSet::new();

    for f in program.functions {
        if f.name.contains(MONO_NAME_MARKER) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("reserved function name: {:?}", f.name),
            ));
        }
        if !declared_symbols.insert(f.name.clone()) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate function name: {:?}", f.name),
            ));
        }
        let sig = build_sig(FnKind::Defn, &f.type_params)?;
        if sigs.insert(f.name.clone(), sig).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate function name: {:?}", f.name),
            ));
        }
        if f.type_params.is_empty() {
            non_generic_fn_defs.push(f);
        } else {
            generic_symbols.insert(f.name.clone());
            generic_fn_defs.insert(f.name.clone(), f);
        }
    }
    for f in program.async_functions {
        if f.name.contains(MONO_NAME_MARKER) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("reserved function name: {:?}", f.name),
            ));
        }
        if !declared_symbols.insert(f.name.clone()) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate function name: {:?}", f.name),
            ));
        }
        let sig = build_sig(FnKind::Defasync, &f.type_params)?;
        if sigs.insert(f.name.clone(), sig).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate async function name: {:?}", f.name),
            ));
        }
        if f.type_params.is_empty() {
            non_generic_async_defs.push(f);
        } else {
            generic_symbols.insert(f.name.clone());
            generic_async_defs.insert(f.name.clone(), f);
        }
    }

    let mut extern_symbols: BTreeSet<String> = BTreeSet::new();
    let extern_functions = program
        .extern_functions
        .into_iter()
        .map(|f| {
            if !extern_symbols.insert(f.name.clone()) || declared_symbols.contains(&f.name) {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("duplicate extern function name: {:?}", f.name),
                ));
            }
            declared_symbols.insert(f.name.clone());
            lower_extern(f)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let generic_functions_defined = generic_fn_defs.len() + generic_async_defs.len();

    let mut instances: BTreeMap<InstantiationKey, InstanceRecord> = BTreeMap::new();
    let mut pending: BTreeSet<InstantiationKey> = BTreeSet::new();
    let mut tapp_sites_total: usize = 0;

    let solve = rewrite_expr(
        program.solve,
        &RewriteCtx {
            caller: "main".to_string(),
            caller_module: "main".to_string(),
        },
        &sigs,
        &generic_symbols,
        module_exports,
        &extern_symbols,
        None,
        &mut instances,
        &mut pending,
        &mut tapp_sites_total,
        max_specializations,
        max_type_depth,
    )?;
    assert_no_generic_syntax(&solve)?;

    let mut out_functions: Vec<FunctionDef> = Vec::new();
    let mut out_async_functions: Vec<AsyncFunctionDef> = Vec::new();
    let mut emitted_symbols: BTreeSet<String> = BTreeSet::new();

    for mut f in non_generic_fn_defs {
        let caller_module = symbol_module_id(&f.name)?;
        let ctx = RewriteCtx {
            caller: f.name.clone(),
            caller_module: caller_module.to_string(),
        };
        f.body = rewrite_expr(
            f.body,
            &ctx,
            &sigs,
            &generic_symbols,
            module_exports,
            &extern_symbols,
            None,
            &mut instances,
            &mut pending,
            &mut tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        rewrite_contract_clauses(
            &mut f.requires,
            &ctx,
            &sigs,
            &generic_symbols,
            module_exports,
            &extern_symbols,
            None,
            &mut instances,
            &mut pending,
            &mut tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        rewrite_contract_clauses(
            &mut f.ensures,
            &ctx,
            &sigs,
            &generic_symbols,
            module_exports,
            &extern_symbols,
            None,
            &mut instances,
            &mut pending,
            &mut tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        rewrite_contract_clauses(
            &mut f.invariant,
            &ctx,
            &sigs,
            &generic_symbols,
            module_exports,
            &extern_symbols,
            None,
            &mut instances,
            &mut pending,
            &mut tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        assert_no_generic_syntax(&f.body)?;
        let lowered = lower_defn(f)?;
        if !emitted_symbols.insert(lowered.name.clone()) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate function name: {:?}", lowered.name),
            ));
        }
        out_functions.push(lowered);
    }

    for mut f in non_generic_async_defs {
        let caller_module = symbol_module_id(&f.name)?;
        let ctx = RewriteCtx {
            caller: f.name.clone(),
            caller_module: caller_module.to_string(),
        };
        f.body = rewrite_expr(
            f.body,
            &ctx,
            &sigs,
            &generic_symbols,
            module_exports,
            &extern_symbols,
            None,
            &mut instances,
            &mut pending,
            &mut tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        rewrite_contract_clauses(
            &mut f.requires,
            &ctx,
            &sigs,
            &generic_symbols,
            module_exports,
            &extern_symbols,
            None,
            &mut instances,
            &mut pending,
            &mut tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        rewrite_contract_clauses(
            &mut f.ensures,
            &ctx,
            &sigs,
            &generic_symbols,
            module_exports,
            &extern_symbols,
            None,
            &mut instances,
            &mut pending,
            &mut tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        rewrite_contract_clauses(
            &mut f.invariant,
            &ctx,
            &sigs,
            &generic_symbols,
            module_exports,
            &extern_symbols,
            None,
            &mut instances,
            &mut pending,
            &mut tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        assert_no_generic_syntax(&f.body)?;
        let lowered = lower_defasync(f)?;
        if !emitted_symbols.insert(lowered.name.clone()) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate async function name: {:?}", lowered.name),
            ));
        }
        out_async_functions.push(lowered);
    }

    while let Some(key) = pending.pop_first() {
        let (kind, def_module, type_args, specialized_name) = {
            let record = instances.get_mut(&key).ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing mono instance record".to_string(),
                )
            })?;
            if record.generated {
                continue;
            }
            record.generated = true;
            (
                record.kind,
                record.def_module.clone(),
                record.type_args.clone(),
                record.specialized.clone(),
            )
        };

        if declared_symbols.contains(&specialized_name) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!(
                    "generated specialization collides with existing symbol: {specialized_name:?}"
                ),
            ));
        }
        declared_symbols.insert(specialized_name.clone());

        match kind {
            FnKind::Defn => {
                let g = generic_fn_defs.get(&key.generic).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown generic function: {:?}", key.generic),
                    )
                })?;
                let mut g = g.clone();
                let subst = build_subst_map(&g.type_params, &type_args)?;
                g.type_params.clear();
                g.name = specialized_name.clone();
                g.params = g
                    .params
                    .into_iter()
                    .map(|mut p| {
                        p.ty = subst_type_ref(&p.ty, &subst)?;
                        Ok(p)
                    })
                    .collect::<Result<Vec<_>, CompilerError>>()?;
                g.result = subst_type_ref(&g.result, &subst)?;
                let ctx = RewriteCtx {
                    caller: specialized_name.clone(),
                    caller_module: def_module.clone(),
                };
                g.body = rewrite_expr(
                    g.body,
                    &ctx,
                    &sigs,
                    &generic_symbols,
                    module_exports,
                    &extern_symbols,
                    Some(&subst),
                    &mut instances,
                    &mut pending,
                    &mut tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                )?;
                rewrite_contract_clauses(
                    &mut g.requires,
                    &ctx,
                    &sigs,
                    &generic_symbols,
                    module_exports,
                    &extern_symbols,
                    Some(&subst),
                    &mut instances,
                    &mut pending,
                    &mut tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                )?;
                rewrite_contract_clauses(
                    &mut g.ensures,
                    &ctx,
                    &sigs,
                    &generic_symbols,
                    module_exports,
                    &extern_symbols,
                    Some(&subst),
                    &mut instances,
                    &mut pending,
                    &mut tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                )?;
                rewrite_contract_clauses(
                    &mut g.invariant,
                    &ctx,
                    &sigs,
                    &generic_symbols,
                    module_exports,
                    &extern_symbols,
                    Some(&subst),
                    &mut instances,
                    &mut pending,
                    &mut tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                )?;
                assert_no_generic_syntax(&g.body)?;
                let lowered = lower_defn(g)?;
                if !emitted_symbols.insert(lowered.name.clone()) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("duplicate function name: {:?}", lowered.name),
                    ));
                }
                out_functions.push(lowered);
            }
            FnKind::Defasync => {
                let g = generic_async_defs.get(&key.generic).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown generic async function: {:?}", key.generic),
                    )
                })?;
                let mut g = g.clone();
                let subst = build_subst_map(&g.type_params, &type_args)?;
                g.type_params.clear();
                g.name = specialized_name.clone();
                g.params = g
                    .params
                    .into_iter()
                    .map(|mut p| {
                        p.ty = subst_type_ref(&p.ty, &subst)?;
                        Ok(p)
                    })
                    .collect::<Result<Vec<_>, CompilerError>>()?;
                g.result = subst_type_ref(&g.result, &subst)?;
                let ctx = RewriteCtx {
                    caller: specialized_name.clone(),
                    caller_module: def_module.clone(),
                };
                g.body = rewrite_expr(
                    g.body,
                    &ctx,
                    &sigs,
                    &generic_symbols,
                    module_exports,
                    &extern_symbols,
                    Some(&subst),
                    &mut instances,
                    &mut pending,
                    &mut tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                )?;
                rewrite_contract_clauses(
                    &mut g.requires,
                    &ctx,
                    &sigs,
                    &generic_symbols,
                    module_exports,
                    &extern_symbols,
                    Some(&subst),
                    &mut instances,
                    &mut pending,
                    &mut tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                )?;
                rewrite_contract_clauses(
                    &mut g.ensures,
                    &ctx,
                    &sigs,
                    &generic_symbols,
                    module_exports,
                    &extern_symbols,
                    Some(&subst),
                    &mut instances,
                    &mut pending,
                    &mut tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                )?;
                rewrite_contract_clauses(
                    &mut g.invariant,
                    &ctx,
                    &sigs,
                    &generic_symbols,
                    module_exports,
                    &extern_symbols,
                    Some(&subst),
                    &mut instances,
                    &mut pending,
                    &mut tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                )?;
                assert_no_generic_syntax(&g.body)?;
                let lowered = lower_defasync(g)?;
                if !emitted_symbols.insert(lowered.name.clone()) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("duplicate async function name: {:?}", lowered.name),
                    ));
                }
                out_async_functions.push(lowered);
            }
        }
    }

    out_functions.sort_by(|a, b| a.name.cmp(&b.name));
    out_async_functions.sort_by(|a, b| a.name.cmp(&b.name));

    let items: Vec<MonoItemV1> = instances
        .iter()
        .map(|(key, r)| MonoItemV1 {
            generic: key.generic.clone(),
            type_args: r.type_args.iter().map(type_ref_to_value).collect(),
            specialized: r.specialized.clone(),
            def_module: r.def_module.clone(),
            sites: r.sites.clone(),
        })
        .collect();
    let specializations_emitted = instances.len();

    let mono = Program {
        functions: out_functions,
        async_functions: out_async_functions,
        extern_functions,
        solve,
    };

    let mono_map = MonoMapV1 {
        schema_version: x07_contracts::X07_MONO_MAP_SCHEMA_VERSION.to_string(),
        tool: "x07c".to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        input_schema_version: input_schema_version.to_string(),
        entry_module: "main".to_string(),
        limits: MonoLimitsV1 {
            max_specializations,
            max_depth: max_type_depth,
        },
        stats: MonoStatsV1 {
            generic_functions_defined,
            specializations_emitted,
            tapp_sites_total,
        },
        items,
        meta: BTreeMap::new(),
    };

    Ok((mono, mono_map))
}

pub fn mangle_specialized_name(generic: &str, type_args: &[TypeRef]) -> String {
    let pretty = pretty_type_args(type_args);
    let hash8 = sha256_hex8(canonical_type_args_bytes(type_args));
    format!("{generic}{MONO_NAME_MARKER}{pretty}__h{hash8}")
}

fn lower_mono_ty(tr: &TypeRef) -> Result<Ty, CompilerError> {
    tr.as_mono_ty().ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Parse,
            format!("unsupported non-monomorphic type expression: {tr:?}"),
        )
    })
}

fn lower_param(p: AstFunctionParam) -> Result<FunctionParam, CompilerError> {
    Ok(FunctionParam {
        name: p.name,
        ty: lower_mono_ty(&p.ty)?,
        brand: p.brand,
    })
}

fn lower_defn(f: AstFunctionDef) -> Result<FunctionDef, CompilerError> {
    if !f.type_params.is_empty() {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "generics are not supported yet: function {:?} has type_params={:?}",
                f.name, f.type_params
            ),
        ));
    }
    Ok(FunctionDef {
        name: f.name,
        requires: f.requires,
        ensures: f.ensures,
        invariant: f.invariant,
        params: f
            .params
            .into_iter()
            .map(lower_param)
            .collect::<Result<Vec<_>, _>>()?,
        ret_ty: lower_mono_ty(&f.result)?,
        ret_brand: f.result_brand,
        body: f.body,
    })
}

fn lower_defasync(f: AstAsyncFunctionDef) -> Result<AsyncFunctionDef, CompilerError> {
    if !f.type_params.is_empty() {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "generics are not supported yet: async function {:?} has type_params={:?}",
                f.name, f.type_params
            ),
        ));
    }
    Ok(AsyncFunctionDef {
        name: f.name,
        requires: f.requires,
        ensures: f.ensures,
        invariant: f.invariant,
        params: f
            .params
            .into_iter()
            .map(lower_param)
            .collect::<Result<Vec<_>, _>>()?,
        ret_ty: lower_mono_ty(&f.result)?,
        ret_brand: f.result_brand,
        body: f.body,
    })
}

fn lower_extern(f: AstExternFunctionDecl) -> Result<ExternFunctionDecl, CompilerError> {
    let (ret_ty, ret_is_void) = match &f.result {
        None => (Ty::I32, true),
        Some(tr) => (lower_mono_ty(tr)?, false),
    };

    Ok(ExternFunctionDecl {
        name: f.name,
        link_name: f.link_name,
        abi: ExternAbi::C,
        params: f
            .params
            .into_iter()
            .map(lower_param)
            .collect::<Result<Vec<_>, _>>()?,
        ret_ty,
        ret_is_void,
    })
}

fn canonical_type_args_bytes(type_args: &[TypeRef]) -> Vec<u8> {
    let mut v = Value::Array(type_args.iter().map(type_ref_to_value).collect());
    canon_value_jcs(&mut v);
    serde_json::to_vec(&v).expect("serialize canonical type args")
}

fn sha256_hex8(bytes: Vec<u8>) -> String {
    let mut h = sha2::Sha256::new();
    h.update(&bytes);
    let digest = h.finalize();
    let mut out = String::new();
    for b in digest.iter().take(4) {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn pretty_type_args(type_args: &[TypeRef]) -> String {
    if type_args.is_empty() {
        return "no_targs".to_string();
    }
    type_args
        .iter()
        .map(pretty_type_ref)
        .collect::<Vec<_>>()
        .join("__")
}

fn pretty_type_ref(tr: &TypeRef) -> String {
    match tr {
        TypeRef::Named(s) => s.clone(),
        TypeRef::Var(name) => format!("t_{name}"),
        TypeRef::App { head, args } => {
            let mut out = head.clone();
            for a in args {
                out.push('_');
                out.push_str(&pretty_type_ref(a));
            }
            out
        }
    }
}

fn build_sig(kind: FnKind, type_params: &[TypeParam]) -> Result<FnSig, CompilerError> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<TypeParamSig> = Vec::with_capacity(type_params.len());
    for tp in type_params {
        if !seen.insert(tp.name.clone()) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate type param name: {:?}", tp.name),
            ));
        }
        let bound = match tp.bound.as_deref() {
            None => None,
            Some(s) => Some(parse_bound(s)?),
        };
        out.push(TypeParamSig {
            name: tp.name.clone(),
            bound,
        });
    }
    Ok(FnSig {
        kind,
        type_params: out,
    })
}

fn parse_bound(s: &str) -> Result<Bound, CompilerError> {
    match s.trim() {
        "any" => Ok(Bound::Any),
        "bytes_like" => Ok(Bound::BytesLike),
        "num_like" => Ok(Bound::NumLike),
        "value" => Ok(Bound::Value),
        "hashable" => Ok(Bound::Hashable),
        "orderable" => Ok(Bound::Orderable),
        other => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("X07-TY-0103: unknown bound name: {other:?}"),
        )),
    }
}

fn build_subst_map(
    type_params: &[TypeParam],
    type_args: &[TypeRef],
) -> Result<BTreeMap<String, TypeRef>, CompilerError> {
    if type_params.len() != type_args.len() {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "X07-TY-0105: tapp arity mismatch: expected {} type args got {}",
                type_params.len(),
                type_args.len()
            ),
        ));
    }

    let mut out: BTreeMap<String, TypeRef> = BTreeMap::new();
    for (tp, ta) in type_params.iter().zip(type_args.iter()) {
        if !type_ref_is_concrete(ta) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "X07-TY-0107: non-concrete type arg for {:?}: {:?}",
                    tp.name, ta
                ),
            ));
        }

        if let Some(bound) = tp.bound.as_deref() {
            let bound = parse_bound(bound)?;
            if !type_satisfies_bound(ta, bound) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "X07-TY-0104: bound unsatisfied for {:?}: expected {:?} got {:?}",
                        tp.name, bound, ta
                    ),
                ));
            }
        }

        if out.insert(tp.name.clone(), ta.clone()).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("duplicate type param name: {:?}", tp.name),
            ));
        }
    }

    Ok(out)
}

fn type_satisfies_bound(ta: &TypeRef, bound: Bound) -> bool {
    match bound {
        Bound::Any => true,
        Bound::BytesLike => matches!(
            ta,
            TypeRef::Named(s) if s == "bytes" || s == "bytes_view"
        ),
        Bound::NumLike => matches!(ta, TypeRef::Named(s) if s == "i32" || s == "u32"),
        Bound::Value => matches!(
            ta,
            TypeRef::Named(s) if s == "i32" || s == "u32" || s == "bytes" || s == "bytes_view"
        ),
        Bound::Hashable => matches!(
            ta,
            TypeRef::Named(s) if s == "i32" || s == "u32" || s == "bytes" || s == "bytes_view"
        ),
        Bound::Orderable => matches!(
            ta,
            TypeRef::Named(s) if s == "i32" || s == "u32" || s == "bytes" || s == "bytes_view"
        ),
    }
}

fn type_ref_is_concrete(tr: &TypeRef) -> bool {
    match tr {
        TypeRef::Named(_) => true,
        TypeRef::Var(_) => false,
        TypeRef::App { args, .. } => args.iter().all(type_ref_is_concrete),
    }
}

fn type_ref_max_depth(tr: &TypeRef) -> usize {
    match tr {
        TypeRef::Named(_) | TypeRef::Var(_) => 1,
        TypeRef::App { args, .. } => 1 + args.iter().map(type_ref_max_depth).max().unwrap_or(0),
    }
}

fn subst_type_ref(
    tr: &TypeRef,
    subst: &BTreeMap<String, TypeRef>,
) -> Result<TypeRef, CompilerError> {
    match tr {
        TypeRef::Named(s) => Ok(TypeRef::Named(s.clone())),
        TypeRef::Var(name) => subst.get(name).cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                format!("X07-TY-0107: unknown type var: {name:?}"),
            )
        }),
        TypeRef::App { head, args } => Ok(TypeRef::App {
            head: head.clone(),
            args: args
                .iter()
                .map(|a| subst_type_ref(a, subst))
                .collect::<Result<Vec<_>, CompilerError>>()?,
        }),
    }
}

fn symbol_module_id(sym: &str) -> Result<&str, CompilerError> {
    let Some((module_id, _rest)) = sym.rsplit_once('.') else {
        return Err(CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: function name missing module prefix: {sym:?}"),
        ));
    };
    Ok(module_id)
}

fn assert_no_generic_syntax(expr: &Expr) -> Result<(), CompilerError> {
    fn walk(e: &Expr) -> Option<String> {
        match e {
            Expr::Int { .. } | Expr::Ident { .. } => None,
            Expr::List { items, .. } => {
                if let Some(head) = items.first().and_then(Expr::as_ident) {
                    if head == "tapp" || head.starts_with("ty.") {
                        return Some(head.to_string());
                    }
                }
                for it in items {
                    if let Some(hit) = walk(it) {
                        return Some(hit);
                    }
                }
                None
            }
        }
    }

    if let Some(hit) = walk(expr) {
        return Err(CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: generics syntax not eliminated: {hit:?}"),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn rewrite_contract_clauses(
    clauses: &mut [crate::x07ast::ContractClauseAst],
    ctx: &RewriteCtx,
    sigs: &BTreeMap<String, FnSig>,
    generic_symbols: &BTreeSet<String>,
    module_exports: &BTreeMap<String, BTreeSet<String>>,
    extern_symbols: &BTreeSet<String>,
    subst: Option<&BTreeMap<String, TypeRef>>,
    instances: &mut BTreeMap<InstantiationKey, InstanceRecord>,
    pending: &mut BTreeSet<InstantiationKey>,
    tapp_sites_total: &mut usize,
    max_specializations: usize,
    max_type_depth: usize,
) -> Result<(), CompilerError> {
    for c in clauses {
        c.expr = rewrite_expr(
            c.expr.clone(),
            ctx,
            sigs,
            generic_symbols,
            module_exports,
            extern_symbols,
            subst,
            instances,
            pending,
            tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?;
        assert_no_generic_syntax(&c.expr)?;
        for w in &mut c.witness {
            *w = rewrite_expr(
                w.clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            assert_no_generic_syntax(w)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn rewrite_expr(
    expr: Expr,
    ctx: &RewriteCtx,
    sigs: &BTreeMap<String, FnSig>,
    generic_symbols: &BTreeSet<String>,
    module_exports: &BTreeMap<String, BTreeSet<String>>,
    extern_symbols: &BTreeSet<String>,
    subst: Option<&BTreeMap<String, TypeRef>>,
    instances: &mut BTreeMap<InstantiationKey, InstanceRecord>,
    pending: &mut BTreeSet<InstantiationKey>,
    tapp_sites_total: &mut usize,
    max_specializations: usize,
    max_type_depth: usize,
) -> Result<Expr, CompilerError> {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => Ok(expr),
        Expr::List { items, ptr } => {
            let Some(head) = items
                .first()
                .and_then(Expr::as_ident)
                .map(|s| s.to_string())
            else {
                let items = items
                    .into_iter()
                    .map(|e| {
                        rewrite_expr(
                            e,
                            ctx,
                            sigs,
                            generic_symbols,
                            module_exports,
                            extern_symbols,
                            subst,
                            instances,
                            pending,
                            tapp_sites_total,
                            max_specializations,
                            max_type_depth,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(Expr::List { items, ptr });
            };

            if head == "tapp" {
                return lower_tapp(
                    items,
                    ptr,
                    ctx,
                    sigs,
                    generic_symbols,
                    module_exports,
                    extern_symbols,
                    subst,
                    instances,
                    pending,
                    tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                );
            }

            if head.starts_with("ty.") {
                return lower_ty_intrinsic(
                    head.as_str(),
                    items,
                    ptr,
                    ctx,
                    sigs,
                    generic_symbols,
                    module_exports,
                    extern_symbols,
                    subst,
                    instances,
                    pending,
                    tapp_sites_total,
                    max_specializations,
                    max_type_depth,
                );
            }

            if generic_symbols.contains(head.as_str()) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0108: generic function must be called via tapp: {head:?}"),
                ));
            }

            let items = items
                .into_iter()
                .map(|e| {
                    rewrite_expr(
                        e,
                        ctx,
                        sigs,
                        generic_symbols,
                        module_exports,
                        extern_symbols,
                        subst,
                        instances,
                        pending,
                        tapp_sites_total,
                        max_specializations,
                        max_type_depth,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Expr::List { items, ptr })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_tapp(
    items: Vec<Expr>,
    ptr: String,
    ctx: &RewriteCtx,
    sigs: &BTreeMap<String, FnSig>,
    _generic_symbols: &BTreeSet<String>,
    module_exports: &BTreeMap<String, BTreeSet<String>>,
    extern_symbols: &BTreeSet<String>,
    subst: Option<&BTreeMap<String, TypeRef>>,
    instances: &mut BTreeMap<InstantiationKey, InstanceRecord>,
    pending: &mut BTreeSet<InstantiationKey>,
    tapp_sites_total: &mut usize,
    max_specializations: usize,
    max_type_depth: usize,
) -> Result<Expr, CompilerError> {
    *tapp_sites_total = tapp_sites_total.saturating_add(1);

    if items.len() < 3 {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "X07-TY-0105: tapp requires at least 2 args (got {})",
                items.len() - 1
            ),
        ));
    }

    let head_ptr = items
        .first()
        .map(|e| e.ptr().to_string())
        .unwrap_or_default();

    let callee = items[1]
        .as_ident()
        .ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "X07-TY-0105: tapp callee must be an identifier".to_string(),
            )
        })?
        .to_string();
    if extern_symbols.contains(&callee) {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("X07-TY-0105: tapp is not allowed on extern symbol: {callee:?}"),
        ));
    }

    // Builtin generic container intrinsics: lower directly to monomorphic builtins.
    //
    // These intrinsics intentionally bypass module export checks and do not generate
    // monomorphized instances.
    if let Some(builtin_arity) = match callee.as_str() {
        "vec_value.with_capacity" | "vec_value.push" | "vec_value.get" | "vec_value.set" => {
            Some(1usize)
        }
        "map_value.new" | "map_value.get" | "map_value.set" => Some(2usize),
        "map_value.contains" | "map_value.remove" => Some(1usize),
        _ => None,
    } {
        let (type_arg_exprs, value_arg_exprs) = match items.get(2) {
            Some(Expr::List { items: tys, .. })
                if tys.first().and_then(Expr::as_ident) == Some("tys") =>
            {
                let type_arg_exprs = tys.iter().skip(1).cloned().collect::<Vec<_>>();
                if type_arg_exprs.len() != builtin_arity {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0105: tapp arity mismatch for {callee:?}: expected {builtin_arity} type args got {}",
                            type_arg_exprs.len()
                        ),
                    ));
                }
                (type_arg_exprs, items[3..].to_vec())
            }
            _ => {
                if items.len() < 2 + builtin_arity {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0105: tapp arity mismatch for {callee:?}: expected {builtin_arity} type args"
                        ),
                    ));
                }
                let type_arg_exprs = items[2..2 + builtin_arity].to_vec();
                let value_arg_exprs = items[2 + builtin_arity..].to_vec();
                (type_arg_exprs, value_arg_exprs)
            }
        };

        let mut type_args: Vec<TypeRef> = Vec::with_capacity(builtin_arity);
        for e in type_arg_exprs {
            let mut tr = type_ref_from_expr(&e)
                .map_err(|m| CompilerError::new(CompileErrorKind::Parse, m))?;
            if let Some(subst) = subst {
                tr = subst_type_ref(&tr, subst)?;
            }
            if !type_ref_is_concrete(&tr) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0107: non-concrete type arg in tapp to {callee:?}: {tr:?}"),
                ));
            }
            if type_ref_max_depth(&tr) > max_type_depth {
                return Err(CompilerError::new(
                    CompileErrorKind::Budget,
                    format!(
                        "X07-TY-0109: type expression too deep in tapp to {callee:?}: max_depth={max_type_depth}"
                    ),
                ));
            }
            type_args.push(tr);
        }

        fn require_named<'a>(callee: &str, tr: &'a TypeRef) -> Result<&'a str, CompilerError> {
            match tr {
                TypeRef::Named(s) => Ok(s),
                _ => Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "X07-TY-0101: unsupported type expression in tapp to {callee:?}: {tr:?}"
                    ),
                )),
            }
        }
        let value_ty_id = |type_name: &str| -> Result<i32, CompilerError> {
            match type_name {
                "i32" => Ok(1),
                "u32" => Ok(2),
                "bytes" => Ok(3),
                "bytes_view" => Ok(4),
                _ => Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0101: unsupported type for {callee:?}: {type_name:?}"),
                )),
            }
        };
        let value_ty_suffix = |type_name: &str| -> Result<&'static str, CompilerError> {
            match type_name {
                "i32" | "u32" => Ok("i32"),
                "bytes" => Ok("bytes"),
                "bytes_view" => Ok("bytes_view"),
                _ => Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0101: unsupported type for {callee:?}: {type_name:?}"),
                )),
            }
        };

        let mut value_args: Vec<Expr> = Vec::with_capacity(value_arg_exprs.len());
        for a in value_arg_exprs {
            value_args.push(rewrite_expr(
                a,
                ctx,
                sigs,
                _generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?);
        }

        match callee.as_str() {
            "vec_value.with_capacity" => {
                if value_args.len() != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "vec_value.with_capacity expects 1 argument".to_string(),
                    ));
                }
                let t = require_named(&callee, &type_args[0])?;
                if !type_satisfies_bound(&type_args[0], Bound::Value) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0104: bound unsatisfied for \"T\": expected {:?} got {:?}",
                            Bound::Value,
                            type_args[0],
                        ),
                    ));
                }
                let ty_id = value_ty_id(t)?;
                return Ok(Expr::List {
                    items: vec![
                        Expr::Ident {
                            name: "vec_value.with_capacity_v1".to_string(),
                            ptr: head_ptr.clone(),
                        },
                        Expr::Int {
                            value: ty_id,
                            ptr: ptr.clone(),
                        },
                        value_args[0].clone(),
                    ],
                    ptr,
                });
            }
            "vec_value.push" | "vec_value.get" | "vec_value.set" => {
                if !type_satisfies_bound(&type_args[0], Bound::Value) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0104: bound unsatisfied for \"T\": expected {:?} got {:?}",
                            Bound::Value,
                            type_args[0],
                        ),
                    ));
                }
                let t = require_named(&callee, &type_args[0])?;
                let t_suffix = value_ty_suffix(t)?;

                let (head, want_args) = match callee.as_str() {
                    "vec_value.push" => (format!("vec_value.push_{t_suffix}_v1"), 2usize),
                    "vec_value.get" => (format!("vec_value.get_{t_suffix}_v1"), 3usize),
                    "vec_value.set" => (format!("vec_value.set_{t_suffix}_v1"), 3usize),
                    _ => unreachable!("callee is known"),
                };
                if value_args.len() != want_args {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("{callee:?} expects {want_args} arguments"),
                    ));
                }
                let mut out_items: Vec<Expr> = Vec::with_capacity(1 + value_args.len());
                out_items.push(Expr::Ident {
                    name: head,
                    ptr: head_ptr.clone(),
                });
                out_items.extend(value_args);
                return Ok(Expr::List {
                    items: out_items,
                    ptr,
                });
            }
            "map_value.new" => {
                if value_args.len() != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "map_value.new expects 1 argument".to_string(),
                    ));
                }
                if !type_satisfies_bound(&type_args[0], Bound::Hashable) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0104: bound unsatisfied for \"K\": expected {:?} got {:?}",
                            Bound::Hashable,
                            type_args[0],
                        ),
                    ));
                }
                if !type_satisfies_bound(&type_args[1], Bound::Value) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0104: bound unsatisfied for \"V\": expected {:?} got {:?}",
                            Bound::Value,
                            type_args[1],
                        ),
                    ));
                }
                let k = require_named(&callee, &type_args[0])?;
                let v = require_named(&callee, &type_args[1])?;
                let k_id = value_ty_id(k)?;
                let v_id = value_ty_id(v)?;
                return Ok(Expr::List {
                    items: vec![
                        Expr::Ident {
                            name: "map_value.new_v1".to_string(),
                            ptr: head_ptr.clone(),
                        },
                        Expr::Int {
                            value: k_id,
                            ptr: ptr.clone(),
                        },
                        Expr::Int {
                            value: v_id,
                            ptr: ptr.clone(),
                        },
                        value_args[0].clone(),
                    ],
                    ptr,
                });
            }
            "map_value.contains" | "map_value.remove" => {
                if !type_satisfies_bound(&type_args[0], Bound::Hashable) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0104: bound unsatisfied for \"K\": expected {:?} got {:?}",
                            Bound::Hashable,
                            type_args[0],
                        ),
                    ));
                }
                let k = require_named(&callee, &type_args[0])?;
                let k_suffix = value_ty_suffix(k)?;
                let (head, want_args) = match callee.as_str() {
                    "map_value.contains" => (format!("map_value.contains_{k_suffix}_v1"), 2usize),
                    "map_value.remove" => (format!("map_value.remove_{k_suffix}_v1"), 2usize),
                    _ => unreachable!("callee is known"),
                };
                if value_args.len() != want_args {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("{callee:?} expects {want_args} arguments"),
                    ));
                }
                let mut out_items: Vec<Expr> = Vec::with_capacity(1 + value_args.len());
                out_items.push(Expr::Ident {
                    name: head,
                    ptr: head_ptr.clone(),
                });
                out_items.extend(value_args);
                return Ok(Expr::List {
                    items: out_items,
                    ptr,
                });
            }
            "map_value.get" | "map_value.set" => {
                if !type_satisfies_bound(&type_args[0], Bound::Hashable) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0104: bound unsatisfied for \"K\": expected {:?} got {:?}",
                            Bound::Hashable,
                            type_args[0],
                        ),
                    ));
                }
                if !type_satisfies_bound(&type_args[1], Bound::Value) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "X07-TY-0104: bound unsatisfied for \"V\": expected {:?} got {:?}",
                            Bound::Value,
                            type_args[1],
                        ),
                    ));
                }
                let k = require_named(&callee, &type_args[0])?;
                let v = require_named(&callee, &type_args[1])?;
                let k_suffix = value_ty_suffix(k)?;
                let v_suffix = value_ty_suffix(v)?;
                let (head, want_args) = match callee.as_str() {
                    "map_value.get" => (format!("map_value.get_{k_suffix}_{v_suffix}_v1"), 3usize),
                    "map_value.set" => (format!("map_value.set_{k_suffix}_{v_suffix}_v1"), 3usize),
                    _ => unreachable!("callee is known"),
                };
                if value_args.len() != want_args {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("{callee:?} expects {want_args} arguments"),
                    ));
                }
                let mut out_items: Vec<Expr> = Vec::with_capacity(1 + value_args.len());
                out_items.push(Expr::Ident {
                    name: head,
                    ptr: head_ptr.clone(),
                });
                out_items.extend(value_args);
                return Ok(Expr::List {
                    items: out_items,
                    ptr,
                });
            }
            _ => {}
        }
    }

    let sig = sigs.get(&callee).ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Typing,
            format!("X07-TY-0100: unknown function: {callee:?}"),
        )
    })?;
    if sig.type_params.is_empty() {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("X07-TY-0105: tapp used on non-generic function: {callee:?}"),
        ));
    }

    let arity = sig.type_params.len();

    let (type_arg_exprs, value_arg_exprs) = match items.get(2) {
        Some(Expr::List { items: tys, .. })
            if tys.first().and_then(Expr::as_ident) == Some("tys") =>
        {
            let type_arg_exprs = tys.iter().skip(1).cloned().collect::<Vec<_>>();
            if type_arg_exprs.len() != arity {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "X07-TY-0105: tapp arity mismatch for {callee:?}: expected {arity} type args got {}",
                        type_arg_exprs.len()
                    ),
                ));
            }
            (type_arg_exprs, items[3..].to_vec())
        }
        _ => {
            if items.len() < 2 + arity {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "X07-TY-0105: tapp arity mismatch for {callee:?}: expected {arity} type args"
                    ),
                ));
            }
            let type_arg_exprs = items[2..2 + arity].to_vec();
            let value_arg_exprs = items[2 + arity..].to_vec();
            (type_arg_exprs, value_arg_exprs)
        }
    };

    let mut type_args: Vec<TypeRef> = Vec::with_capacity(arity);
    for e in type_arg_exprs {
        let mut tr =
            type_ref_from_expr(&e).map_err(|m| CompilerError::new(CompileErrorKind::Parse, m))?;
        if let Some(subst) = subst {
            tr = subst_type_ref(&tr, subst)?;
        }
        if !type_ref_is_concrete(&tr) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("X07-TY-0107: non-concrete type arg in tapp to {callee:?}: {tr:?}"),
            ));
        }
        if type_ref_max_depth(&tr) > max_type_depth {
            return Err(CompilerError::new(
                CompileErrorKind::Budget,
                format!(
                    "X07-TY-0109: type expression too deep in tapp to {callee:?}: max_depth={max_type_depth}"
                ),
            ));
        }
        type_args.push(tr);
    }

    for (tp, ta) in sig.type_params.iter().zip(type_args.iter()) {
        if let Some(bound) = tp.bound {
            if !type_satisfies_bound(ta, bound) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "X07-TY-0104: bound unsatisfied for {:?}: expected {:?} got {:?}",
                        tp.name, bound, ta
                    ),
                ));
            }
        }
    }

    let callee_module = symbol_module_id(&callee)?;
    if callee_module != ctx.caller_module {
        let exports = module_exports.get(callee_module).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                format!("unknown module: {callee_module:?}"),
            )
        })?;
        if !exports.contains(&callee) {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!(
                    "function {callee:?} is not exported by module {callee_module:?} (required by tapp)"
                ),
            ));
        }
    }

    let specialized = ensure_instance(
        &callee,
        sig.kind,
        callee_module,
        &type_args,
        ctx,
        &ptr,
        instances,
        pending,
        max_specializations,
    )?;

    let mut new_items: Vec<Expr> = Vec::with_capacity(1 + value_arg_exprs.len());
    new_items.push(Expr::Ident {
        name: specialized,
        ptr: head_ptr,
    });
    for a in value_arg_exprs {
        new_items.push(rewrite_expr(
            a,
            ctx,
            sigs,
            _generic_symbols,
            module_exports,
            extern_symbols,
            subst,
            instances,
            pending,
            tapp_sites_total,
            max_specializations,
            max_type_depth,
        )?);
    }
    Ok(Expr::List {
        items: new_items,
        ptr,
    })
}

#[allow(clippy::too_many_arguments)]
fn ensure_instance(
    generic: &str,
    kind: FnKind,
    def_module: &str,
    type_args: &[TypeRef],
    ctx: &RewriteCtx,
    ptr: &str,
    instances: &mut BTreeMap<InstantiationKey, InstanceRecord>,
    pending: &mut BTreeSet<InstantiationKey>,
    max_specializations: usize,
) -> Result<String, CompilerError> {
    let type_args_key = canonical_type_args_key(type_args);
    let key = InstantiationKey {
        generic: generic.to_string(),
        type_args_key,
    };

    let site = MonoSiteV1 {
        caller: ctx.caller.clone(),
        caller_module: ctx.caller_module.clone(),
        ptr: ptr.to_string(),
    };

    if let Some(r) = instances.get_mut(&key) {
        if r.sites.len() < MAX_SITES_PER_ITEM {
            r.sites.push(site);
        }
        return Ok(r.specialized.clone());
    }

    if instances.len() >= max_specializations {
        return Err(CompilerError::new(
            CompileErrorKind::Budget,
            format!(
                "X07-TY-0106: monomorphization explosion: max_specializations={max_specializations}"
            ),
        ));
    }

    let specialized = mangle_specialized_name(generic, type_args);
    instances.insert(
        key.clone(),
        InstanceRecord {
            kind,
            def_module: def_module.to_string(),
            type_args: type_args.to_vec(),
            specialized: specialized.clone(),
            sites: vec![site],
            generated: false,
        },
    );
    pending.insert(key);
    Ok(specialized)
}

fn canonical_type_args_key(type_args: &[TypeRef]) -> String {
    let bytes = canonical_type_args_bytes(type_args);
    String::from_utf8(bytes).expect("canonical type args are valid UTF-8")
}

#[allow(clippy::too_many_arguments)]
fn lower_ty_intrinsic(
    head: &str,
    items: Vec<Expr>,
    ptr: String,
    ctx: &RewriteCtx,
    sigs: &BTreeMap<String, FnSig>,
    generic_symbols: &BTreeSet<String>,
    module_exports: &BTreeMap<String, BTreeSet<String>>,
    extern_symbols: &BTreeSet<String>,
    subst: Option<&BTreeMap<String, TypeRef>>,
    instances: &mut BTreeMap<InstantiationKey, InstanceRecord>,
    pending: &mut BTreeSet<InstantiationKey>,
    tapp_sites_total: &mut usize,
    max_specializations: usize,
    max_type_depth: usize,
) -> Result<Expr, CompilerError> {
    let head_ptr = items
        .first()
        .map(|e| e.ptr().to_string())
        .unwrap_or_default();

    if items.len() < 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("{head:?} requires a type argument"),
        ));
    }

    let mut tr = type_ref_from_expr(&items[1])
        .map_err(|m| CompilerError::new(CompileErrorKind::Parse, m))?;
    if let Some(subst) = subst {
        tr = subst_type_ref(&tr, subst)?;
    }
    if !type_ref_is_concrete(&tr) {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("X07-TY-0107: non-concrete type in intrinsic {head:?}: {tr:?}"),
        ));
    }

    let type_name = match &tr {
        TypeRef::Named(s) => s.as_str(),
        _ => {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "X07-TY-0101: intrinsic {head:?} supports only named primitive types (got {tr:?})"
                ),
            ));
        }
    };

    match head {
        "ty.size_bytes" | "ty.size" => {
            if items.len() != 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 1 argument"),
                ));
            }
            let ptr_bytes = std::mem::size_of::<usize>() as i32;
            let n = match type_name {
                "i32" | "u32" => 4,
                "bytes" | "bytes_view" => ptr_bytes * 2,
                _ => {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                    ));
                }
            };
            Ok(Expr::Int { value: n, ptr })
        }
        "ty.read_le_at" => {
            if items.len() != 4 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 3 arguments"),
                ));
            }
            if !matches!(type_name, "i32" | "u32") {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                ));
            }
            let b = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            let off = rewrite_expr(
                items[3].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            Ok(Expr::List {
                items: vec![
                    Expr::Ident {
                        name: "std.u32.read_le_at".to_string(),
                        ptr: head_ptr,
                    },
                    b,
                    off,
                ],
                ptr,
            })
        }
        "ty.write_le_at" => {
            if items.len() != 5 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 4 arguments"),
                ));
            }
            if !matches!(type_name, "i32" | "u32") {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                ));
            }
            let b = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            let off = rewrite_expr(
                items[3].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            let x = rewrite_expr(
                items[4].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            Ok(Expr::List {
                items: vec![
                    Expr::Ident {
                        name: "std.u32.write_le_at".to_string(),
                        ptr: head_ptr,
                    },
                    b,
                    off,
                    x,
                ],
                ptr,
            })
        }
        "ty.push_le" => {
            if items.len() != 4 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 3 arguments"),
                ));
            }
            if !matches!(type_name, "i32" | "u32") {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                ));
            }
            let v = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            let x = rewrite_expr(
                items[3].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            Ok(Expr::List {
                items: vec![
                    Expr::Ident {
                        name: "std.u32.push_le".to_string(),
                        ptr: head_ptr,
                    },
                    v,
                    x,
                ],
                ptr,
            })
        }
        "ty.lt" => {
            if items.len() != 4 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 3 arguments"),
                ));
            }
            let a = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            let b = rewrite_expr(
                items[3].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;

            if matches!(type_name, "bytes" | "bytes_view") {
                let hash8 = sha256_hex8(ptr.as_bytes().to_vec());
                let a_name = format!("_x07_lt_a_{hash8}");
                let b_name = format!("_x07_lt_b_{hash8}");
                let an_name = format!("_x07_lt_an_{hash8}");
                let bn_name = format!("_x07_lt_bn_{hash8}");
                return Ok(Expr::List {
                    items: vec![
                        Expr::Ident {
                            name: "begin".to_string(),
                            ptr: head_ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "let".to_string(),
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: a_name.clone(),
                                    ptr: ptr.clone(),
                                },
                                a,
                            ],
                            ptr: ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "let".to_string(),
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: b_name.clone(),
                                    ptr: ptr.clone(),
                                },
                                b,
                            ],
                            ptr: ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "let".to_string(),
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: an_name.clone(),
                                    ptr: ptr.clone(),
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "bytes.len".to_string(),
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: a_name.clone(),
                                            ptr: ptr.clone(),
                                        },
                                    ],
                                    ptr: ptr.clone(),
                                },
                            ],
                            ptr: ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "let".to_string(),
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: bn_name.clone(),
                                    ptr: ptr.clone(),
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "bytes.len".to_string(),
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: b_name.clone(),
                                            ptr: ptr.clone(),
                                        },
                                    ],
                                    ptr: ptr.clone(),
                                },
                            ],
                            ptr: ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "<".to_string(),
                                    ptr: head_ptr,
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "bytes.cmp_range".to_string(),
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: a_name,
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Int {
                                            value: 0,
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: an_name,
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: b_name,
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Int {
                                            value: 0,
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: bn_name,
                                            ptr: ptr.clone(),
                                        },
                                    ],
                                    ptr: ptr.clone(),
                                },
                                Expr::Int {
                                    value: 0,
                                    ptr: ptr.clone(),
                                },
                            ],
                            ptr: ptr.clone(),
                        },
                    ],
                    ptr,
                });
            }

            let op = match type_name {
                "u32" => "<u",
                "i32" => "<",
                _ => {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                    ));
                }
            };
            Ok(Expr::List {
                items: vec![
                    Expr::Ident {
                        name: op.to_string(),
                        ptr: head_ptr,
                    },
                    a,
                    b,
                ],
                ptr,
            })
        }
        "ty.eq" => {
            if items.len() != 4 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 3 arguments"),
                ));
            }
            let a = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            let b = rewrite_expr(
                items[3].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            let op = match type_name {
                "i32" | "u32" => "=",
                "bytes" | "bytes_view" => "bytes.eq",
                _ => {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                    ));
                }
            };
            Ok(Expr::List {
                items: vec![
                    Expr::Ident {
                        name: op.to_string(),
                        ptr: head_ptr,
                    },
                    a,
                    b,
                ],
                ptr,
            })
        }
        "ty.hash32" => {
            if items.len() != 3 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 2 arguments"),
                ));
            }
            let x = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            match type_name {
                "i32" | "u32" => Ok(Expr::List {
                    items: vec![
                        Expr::Ident {
                            name: "std.hash.mix32".to_string(),
                            ptr: head_ptr,
                        },
                        x,
                    ],
                    ptr,
                }),
                "bytes" | "bytes_view" => Ok(Expr::List {
                    items: vec![
                        Expr::Ident {
                            name: "std.hash.mix32".to_string(),
                            ptr: head_ptr,
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "std.hash.fnv1a32_view".to_string(),
                                    ptr: ptr.clone(),
                                },
                                x,
                            ],
                            ptr: ptr.clone(),
                        },
                    ],
                    ptr,
                }),
                _ => Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                )),
            }
        }
        "ty.clone" => {
            if items.len() != 3 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 2 arguments"),
                ));
            }
            let x = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;

            match type_name {
                "i32" | "u32" | "bytes_view" => Ok(x),
                "bytes" => match x {
                    Expr::Ident { .. } => Ok(Expr::List {
                        items: vec![
                            Expr::Ident {
                                name: "__internal.bytes.clone_v1".to_string(),
                                ptr: head_ptr,
                            },
                            x,
                        ],
                        ptr,
                    }),
                    _ => {
                        let hash8 = sha256_hex8(ptr.as_bytes().to_vec());
                        let tmp_name = format!("_x07_clone_src_{hash8}");
                        Ok(Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "begin".to_string(),
                                    ptr: head_ptr.clone(),
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "let".to_string(),
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: tmp_name.clone(),
                                            ptr: ptr.clone(),
                                        },
                                        x,
                                    ],
                                    ptr: ptr.clone(),
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "__internal.bytes.clone_v1".to_string(),
                                            ptr: head_ptr,
                                        },
                                        Expr::Ident {
                                            name: tmp_name,
                                            ptr: ptr.clone(),
                                        },
                                    ],
                                    ptr: ptr.clone(),
                                },
                            ],
                            ptr,
                        })
                    }
                },
                _ => Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                )),
            }
        }
        "ty.drop" => {
            if items.len() != 3 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 2 arguments"),
                ));
            }
            let x = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;

            match type_name {
                "i32" | "u32" | "bytes_view" => Ok(Expr::List {
                    items: vec![
                        Expr::Ident {
                            name: "begin".to_string(),
                            ptr: head_ptr,
                        },
                        x,
                        Expr::Int {
                            value: 0,
                            ptr: ptr.clone(),
                        },
                    ],
                    ptr,
                }),
                "bytes" => match x {
                    Expr::Ident { .. } => Ok(Expr::List {
                        items: vec![
                            Expr::Ident {
                                name: "__internal.bytes.drop_v1".to_string(),
                                ptr: head_ptr,
                            },
                            x,
                        ],
                        ptr,
                    }),
                    _ => {
                        let hash8 = sha256_hex8(ptr.as_bytes().to_vec());
                        let tmp_name = format!("_x07_drop_src_{hash8}");
                        Ok(Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "begin".to_string(),
                                    ptr: head_ptr.clone(),
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "let".to_string(),
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: tmp_name.clone(),
                                            ptr: ptr.clone(),
                                        },
                                        x,
                                    ],
                                    ptr: ptr.clone(),
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "__internal.bytes.drop_v1".to_string(),
                                            ptr: head_ptr,
                                        },
                                        Expr::Ident {
                                            name: tmp_name,
                                            ptr: ptr.clone(),
                                        },
                                    ],
                                    ptr: ptr.clone(),
                                },
                            ],
                            ptr,
                        })
                    }
                },
                _ => Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                )),
            }
        }
        "ty.cmp" => {
            if items.len() != 4 {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("{head:?} expects 3 arguments"),
                ));
            }
            let a = rewrite_expr(
                items[2].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            let b = rewrite_expr(
                items[3].clone(),
                ctx,
                sigs,
                generic_symbols,
                module_exports,
                extern_symbols,
                subst,
                instances,
                pending,
                tapp_sites_total,
                max_specializations,
                max_type_depth,
            )?;
            if matches!(type_name, "bytes" | "bytes_view") {
                let hash8 = sha256_hex8(ptr.as_bytes().to_vec());
                let a_name = format!("_x07_cmp_a_{hash8}");
                let b_name = format!("_x07_cmp_b_{hash8}");
                let an_name = format!("_x07_cmp_an_{hash8}");
                let bn_name = format!("_x07_cmp_bn_{hash8}");
                return Ok(Expr::List {
                    items: vec![
                        Expr::Ident {
                            name: "begin".to_string(),
                            ptr: head_ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "let".to_string(),
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: a_name.clone(),
                                    ptr: ptr.clone(),
                                },
                                a,
                            ],
                            ptr: ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "let".to_string(),
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: b_name.clone(),
                                    ptr: ptr.clone(),
                                },
                                b,
                            ],
                            ptr: ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "let".to_string(),
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: an_name.clone(),
                                    ptr: ptr.clone(),
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "bytes.len".to_string(),
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: a_name.clone(),
                                            ptr: ptr.clone(),
                                        },
                                    ],
                                    ptr: ptr.clone(),
                                },
                            ],
                            ptr: ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "let".to_string(),
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: bn_name.clone(),
                                    ptr: ptr.clone(),
                                },
                                Expr::List {
                                    items: vec![
                                        Expr::Ident {
                                            name: "bytes.len".to_string(),
                                            ptr: ptr.clone(),
                                        },
                                        Expr::Ident {
                                            name: b_name.clone(),
                                            ptr: ptr.clone(),
                                        },
                                    ],
                                    ptr: ptr.clone(),
                                },
                            ],
                            ptr: ptr.clone(),
                        },
                        Expr::List {
                            items: vec![
                                Expr::Ident {
                                    name: "bytes.cmp_range".to_string(),
                                    ptr: head_ptr,
                                },
                                Expr::Ident {
                                    name: a_name,
                                    ptr: ptr.clone(),
                                },
                                Expr::Int {
                                    value: 0,
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: an_name,
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: b_name,
                                    ptr: ptr.clone(),
                                },
                                Expr::Int {
                                    value: 0,
                                    ptr: ptr.clone(),
                                },
                                Expr::Ident {
                                    name: bn_name,
                                    ptr: ptr.clone(),
                                },
                            ],
                            ptr: ptr.clone(),
                        },
                    ],
                    ptr,
                });
            }
            let lt_op = match type_name {
                "u32" => "<u",
                "i32" => "<",
                _ => {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("X07-TY-0101: unsupported type for {head:?}: {type_name:?}"),
                    ));
                }
            };
            let lt_expr = Expr::List {
                items: vec![
                    Expr::Ident {
                        name: lt_op.to_string(),
                        ptr: head_ptr.clone(),
                    },
                    a.clone(),
                    b.clone(),
                ],
                ptr: ptr.clone(),
            };
            let eq_expr = Expr::List {
                items: vec![
                    Expr::Ident {
                        name: "=".to_string(),
                        ptr: head_ptr.clone(),
                    },
                    a,
                    b,
                ],
                ptr: ptr.clone(),
            };
            Ok(Expr::List {
                items: vec![
                    Expr::Ident {
                        name: "if".to_string(),
                        ptr: head_ptr,
                    },
                    lt_expr,
                    Expr::Int {
                        value: -1,
                        ptr: ptr.clone(),
                    },
                    Expr::List {
                        items: vec![
                            Expr::Ident {
                                name: "if".to_string(),
                                ptr: ptr.clone(),
                            },
                            eq_expr,
                            Expr::Int {
                                value: 0,
                                ptr: ptr.clone(),
                            },
                            Expr::Int {
                                value: 1,
                                ptr: ptr.clone(),
                            },
                        ],
                        ptr: ptr.clone(),
                    },
                ],
                ptr,
            })
        }
        other => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("X07-TY-0102: unknown ty intrinsic: {other:?}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::ast::Expr;
    use crate::x07ast::{AstFunctionDef, AstFunctionParam, TypeParam, TypeRef};

    use super::{
        assert_no_generic_syntax, ensure_instance, mangle_specialized_name, monomorphize, FnKind,
        GenericProgram, RewriteCtx,
    };

    fn ident(name: &str) -> Expr {
        Expr::Ident {
            name: name.to_string(),
            ptr: String::new(),
        }
    }

    fn int(value: i32) -> Expr {
        Expr::Int {
            value,
            ptr: String::new(),
        }
    }

    fn list(items: Vec<Expr>) -> Expr {
        Expr::List {
            items,
            ptr: String::new(),
        }
    }

    #[test]
    fn ensure_instance_enforces_max_specializations() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let ctx = RewriteCtx {
            caller: "main".to_string(),
            caller_module: "main".to_string(),
        };
        let mut instances = BTreeMap::new();
        let mut pending = BTreeSet::new();

        let _ = ensure_instance(
            "main.id",
            FnKind::Defn,
            "main",
            &[TypeRef::Named("i32".to_string())],
            &ctx,
            "/solve",
            &mut instances,
            &mut pending,
            1,
        )
        .expect("first instance ok");

        let err = ensure_instance(
            "main.id",
            FnKind::Defn,
            "main",
            &[TypeRef::Named("bytes".to_string())],
            &ctx,
            "/solve",
            &mut instances,
            &mut pending,
            1,
        )
        .expect_err("must enforce specialization cap");
        assert_eq!(err.kind, crate::compile::CompileErrorKind::Budget);
        assert!(
            err.message.contains("X07-TY-0106"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn monomorphize_deduplicates_recursive_generic_instance() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let loop_fn = AstFunctionDef {
            name: "main.loop".to_string(),
            type_params: vec![TypeParam {
                name: "A".to_string(),
                bound: None,
            }],
            requires: Vec::new(),
            ensures: Vec::new(),
            invariant: Vec::new(),
            params: vec![AstFunctionParam {
                name: "x".to_string(),
                ty: TypeRef::Var("A".to_string()),
                brand: None,
            }],
            result: TypeRef::Var("A".to_string()),
            result_brand: None,
            body: list(vec![
                ident("tapp"),
                ident("main.loop"),
                list(vec![ident("t"), ident("A")]),
                ident("x"),
            ]),
        };

        let program = GenericProgram {
            functions: vec![loop_fn],
            async_functions: Vec::new(),
            extern_functions: Vec::new(),
            solve: list(vec![
                ident("tapp"),
                ident("main.loop"),
                ident("i32"),
                int(0),
            ]),
        };

        let module_exports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let (mono, mono_map) =
            monomorphize(program, &module_exports, "x07.x07ast@0.5.0").expect("monomorphize");

        let type_args = vec![TypeRef::Named("i32".to_string())];
        let expected = mangle_specialized_name("main.loop", type_args.as_slice());

        assert_eq!(
            mono_map.items.len(),
            1,
            "expected exactly one specialization, got: {:?}",
            mono_map.items
        );
        let item = mono_map.items.first().expect("len == 1");
        assert_eq!(item.generic, "main.loop");
        assert_eq!(item.specialized, expected);

        let f = mono
            .functions
            .iter()
            .find(|f| f.name == expected)
            .expect("specialized function emitted");
        assert_no_generic_syntax(&f.body).expect("generic syntax must be eliminated");

        let Expr::List { items, .. } = &f.body else {
            panic!("expected recursive call list in body");
        };
        assert_eq!(
            items.first().and_then(Expr::as_ident),
            Some(expected.as_str())
        );
    }
}
