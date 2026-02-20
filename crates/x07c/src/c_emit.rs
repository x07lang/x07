use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::ast::Expr;
use crate::compile::{CompileErrorKind, CompileOptions, CompilerError, ContractMode};
use crate::contracts_elab::{clause_id_or_hash, ContractClauseKind};
use crate::language;
use crate::native;
use crate::native::NativeBackendReq;
use crate::program::{AsyncFunctionDef, ExternFunctionDecl, FunctionDef, FunctionParam, Program};
use crate::types::Ty;
use x07_contracts::{
    X07_ARCH_RR_INDEX_SCHEMA_VERSION, X07_ARCH_RR_POLICY_SCHEMA_VERSION,
    X07_BUDGET_PROFILE_SCHEMA_VERSION,
};

#[path = "c_emit_async.rs"]
mod c_emit_async;
#[path = "c_emit_builtins.rs"]
mod c_emit_builtins;
#[path = "c_emit_contracts.rs"]
mod c_emit_contracts;
#[path = "c_emit_core.rs"]
mod c_emit_core;
#[path = "c_emit_streams.rs"]
mod c_emit_streams;
#[path = "c_emit_types.rs"]
mod c_emit_types;
#[path = "c_emit_worlds.rs"]
mod c_emit_worlds;

use self::c_emit_contracts::{
    contract_payload_json_v1, emit_contract_trap_payload_v1, ContractWitnessC,
    CONTRACT_ALLOC_BYTES, CONTRACT_FUEL, CONTRACT_RUNTIME_HELPERS_C, CONTRACT_WITNESS_MAX_BYTES,
};
use self::c_emit_streams::program_uses_stream_xf_plugin_json_jcs;
use self::c_emit_worlds::{load_rr_cfg_v1_from_arch_v1, parse_bytes_lit_ascii, parse_i32_lit};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TyBrand {
    None,
    Any,
    Brand(String),
}

impl TyBrand {
    fn is_none(&self) -> bool {
        matches!(self, TyBrand::None)
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            TyBrand::Brand(b) => Some(b.as_str()),
            TyBrand::None | TyBrand::Any => None,
        }
    }
}

#[derive(Debug, Clone)]
struct TyInfo {
    ty: Ty,
    brand: TyBrand,
    view_full: bool,
}

impl TyInfo {
    fn unbranded(ty: Ty) -> Self {
        Self {
            ty,
            brand: TyBrand::None,
            view_full: false,
        }
    }

    fn branded(ty: Ty, brand: String) -> Self {
        Self {
            ty,
            brand: TyBrand::Brand(brand),
            view_full: false,
        }
    }

    fn is_ptr_ty(&self) -> bool {
        self.ty.is_ptr_ty()
    }
}

impl PartialEq for TyInfo {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty && self.brand == other.brand
    }
}

impl Eq for TyInfo {}

impl From<Ty> for TyInfo {
    fn from(ty: Ty) -> Self {
        TyInfo::unbranded(ty)
    }
}

impl PartialEq<Ty> for TyInfo {
    fn eq(&self, other: &Ty) -> bool {
        self.ty == *other
    }
}

impl PartialEq<TyInfo> for Ty {
    fn eq(&self, other: &TyInfo) -> bool {
        *self == other.ty
    }
}

fn ty_brand_from_opt(v: &Option<String>) -> TyBrand {
    v.as_ref()
        .map(|b| TyBrand::Brand(b.clone()))
        .unwrap_or(TyBrand::None)
}

fn tybrand_diag(brand: &TyBrand) -> String {
    if let Some(b) = brand.as_str() {
        return b.to_string();
    }
    match brand {
        TyBrand::Any => "any".to_string(),
        TyBrand::None => "unbranded".to_string(),
        TyBrand::Brand(_) => unreachable!(),
    }
}

#[derive(Debug, Clone)]
struct VarRef {
    ty: Ty,
    brand: TyBrand,
    c_name: String,
    moved: bool,
    moved_ptr: Option<String>,
    borrow_count: u32,
    // For `bytes_view` values that borrow from an owned buffer, this is the C local name of the
    // owner (`bytes_t` or `vec_u8_t`) whose backing allocation must outlive the view.
    borrow_of: Option<String>,
    borrow_ptr: Option<String>,
    borrow_is_full: bool,
    // Temporaries participate in scope cleanup (drops / borrow releases).
    is_temp: bool,
}

#[derive(Debug, Clone)]
struct AsyncVarRef {
    ty: Ty,
    brand: TyBrand,
    c_name: String,
    moved: bool,
    moved_ptr: Option<String>,
}

#[derive(Debug, Clone)]
enum CleanupScope {
    Task {
        c_name: String,
    },
    Budget {
        c_name: String,
    },
    Rr {
        handle_c_name: String,
        prev_c_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ViewBorrowFrom {
    Runtime,
    Param(usize),
    LocalOwned(String),
}

#[derive(Debug, Default)]
struct ViewBorrowEnv {
    scopes: Vec<BTreeMap<String, ViewBorrowFrom>>,
}

impl ViewBorrowEnv {
    fn new() -> Self {
        Self {
            scopes: vec![BTreeMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind(&mut self, name: String, src: ViewBorrowFrom) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, src);
        }
    }

    fn lookup(&self, name: &str) -> Option<ViewBorrowFrom> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }
}

#[derive(Debug, Default)]
struct ViewBorrowCollector {
    src: Option<ViewBorrowFrom>,
}

impl ViewBorrowCollector {
    fn merge(&mut self, fn_name: &str, new_src: ViewBorrowFrom) -> Result<(), CompilerError> {
        self.src = match self.src.take() {
            None => Some(new_src),
            Some(old_src) => Some(merge_view_borrow_from(fn_name, old_src, new_src)?),
        };
        Ok(())
    }
}

fn merge_view_borrow_from(
    fn_name: &str,
    a: ViewBorrowFrom,
    b: ViewBorrowFrom,
) -> Result<ViewBorrowFrom, CompilerError> {
    if a == b {
        return Ok(a);
    }
    match (a, b) {
        (ViewBorrowFrom::Runtime, other) | (other, ViewBorrowFrom::Runtime) => Ok(other),
        (ViewBorrowFrom::Param(a), ViewBorrowFrom::Param(b)) => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "function {fn_name:?} returns bytes_view that can borrow from multiple params ({a} vs {b})"
            ),
        )),
        (ViewBorrowFrom::LocalOwned(a), ViewBorrowFrom::LocalOwned(b)) => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "function {fn_name:?} returns bytes_view that can borrow from multiple locals ({a:?} vs {b:?})"
            ),
        )),
        (ViewBorrowFrom::Param(a), ViewBorrowFrom::LocalOwned(b))
        | (ViewBorrowFrom::LocalOwned(b), ViewBorrowFrom::Param(a)) => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "function {fn_name:?} returns bytes_view that can borrow from param {a} or local {b:?}"
            ),
        )),
    }
}

struct ViewBorrowFnCache<'a> {
    cache: &'a mut BTreeMap<String, Option<usize>>,
    visiting: &'a mut BTreeSet<String>,
}

impl<'a> ViewBorrowFnCache<'a> {
    fn new(
        cache: &'a mut BTreeMap<String, Option<usize>>,
        visiting: &'a mut BTreeSet<String>,
    ) -> Self {
        Self { cache, visiting }
    }
}

fn c_escape_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7E => out.push(b as char),
            _ => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\{:03o}", b);
            }
        }
    }
    out
}

fn c_escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

fn value_suffix_ty(suffix: &str) -> Option<Ty> {
    match suffix {
        "i32" => Some(Ty::I32),
        "bytes" => Some(Ty::Bytes),
        "bytes_view" => Some(Ty::BytesView),
        _ => None,
    }
}

fn parse_value_suffix_single<'a>(head: &'a str, prefix: &str) -> Option<&'a str> {
    let suffix = head
        .strip_prefix(prefix)?
        .strip_suffix("_v1")
        .filter(|s| value_suffix_ty(s).is_some())?;
    Some(suffix)
}

fn parse_value_suffix_pair<'a>(head: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = head.strip_prefix(prefix)?.strip_suffix("_v1")?;
    for k in ["i32", "bytes", "bytes_view"] {
        let Some(v) = rest.strip_prefix(k).and_then(|r| r.strip_prefix('_')) else {
            continue;
        };
        if value_suffix_ty(v).is_some() {
            return Some((k, v));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::c_escape_string;

    #[test]
    fn escapes_do_not_greedily_consume_following_hex_digits() {
        let bytes = [0xEF, 0xBC, 0xAF, b'1', b'2', b'3'];
        let escaped = c_escape_string(&bytes);
        assert!(!escaped.contains("\\x"));
        assert!(escaped.starts_with("\\357\\274\\257123"));
    }

    #[test]
    fn escapes_basic_controls() {
        let bytes = [b'\n', b'\r', b'\t', b'\\', b'"'];
        let escaped = c_escape_string(&bytes);
        assert_eq!(escaped, "\\n\\r\\t\\\\\\\"");
    }
}

fn is_owned_ty(ty: Ty) -> bool {
    matches!(
        ty,
        Ty::Bytes
            | Ty::VecU8
            | Ty::OptionBytes
            | Ty::ResultBytes
            | Ty::ResultResultBytes
            | Ty::TaskScopeV1
            | Ty::TaskSelectEvtV1
            | Ty::OptionTaskSelectEvtV1
    )
}

fn is_view_like_ty(ty: Ty) -> bool {
    matches!(
        ty,
        Ty::BytesView | Ty::OptionBytesView | Ty::ResultBytesView
    )
}

fn is_task_handle_ty(ty: Ty) -> bool {
    matches!(ty, Ty::TaskHandleBytesV1 | Ty::TaskHandleResultBytesV1)
}

fn ty_compat_task_handle_as_i32(got: Ty, want: Ty) -> bool {
    got == want || (want == Ty::I32 && is_task_handle_ty(got))
}

fn ty_compat_call_arg(got: Ty, want: Ty) -> bool {
    ty_compat_task_handle_as_i32(got, want)
        || matches!(
            (got, want),
            (Ty::Bytes, Ty::BytesView) | (Ty::VecU8, Ty::BytesView)
        )
}

fn tybrand_compat(base: Ty, got: &TyBrand, want: &TyBrand) -> bool {
    match want {
        TyBrand::None => true,
        TyBrand::Any => matches!(got, TyBrand::Any),
        TyBrand::Brand(want_brand) => match got {
            TyBrand::Brand(got_brand) => got_brand == want_brand,
            TyBrand::Any => matches!(
                base,
                Ty::OptionBytes | Ty::OptionBytesView | Ty::ResultBytes | Ty::ResultBytesView
            ),
            TyBrand::None => false,
        },
    }
}

fn tybrand_join(base: Ty, a: &TyBrand, b: &TyBrand) -> TyBrand {
    let allow_any = matches!(
        base,
        Ty::OptionBytes | Ty::OptionBytesView | Ty::ResultBytes | Ty::ResultBytesView
    );
    if allow_any {
        if matches!(a, TyBrand::Any) && matches!(b, TyBrand::Any) {
            return TyBrand::Any;
        }
        if matches!(a, TyBrand::Any) {
            return b.clone();
        }
        if matches!(b, TyBrand::Any) {
            return a.clone();
        }
    }

    match (a, b) {
        (TyBrand::Brand(a), TyBrand::Brand(b)) if a == b => TyBrand::Brand(a.clone()),
        _ => TyBrand::None,
    }
}

fn tyinfo_compat_assign(got: &TyInfo, want: &TyInfo) -> bool {
    ty_compat_task_handle_as_i32(got.ty, want.ty)
        || (got.ty == want.ty && tybrand_compat(want.ty, &got.brand, &want.brand))
}

fn tyinfo_compat_call_arg(got: &TyInfo, want: &TyInfo) -> bool {
    if ty_compat_task_handle_as_i32(got.ty, want.ty) {
        return true;
    }
    if got.ty == want.ty {
        return tybrand_compat(want.ty, &got.brand, &want.brand);
    }
    match (got.ty, want.ty) {
        (Ty::Bytes, Ty::BytesView) => tybrand_compat(Ty::BytesView, &got.brand, &want.brand),
        (Ty::VecU8, Ty::BytesView) => tybrand_compat(Ty::BytesView, &TyBrand::None, &want.brand),
        _ => false,
    }
}

fn call_arg_mismatch_message(head: &str, idx: usize, got: &TyInfo, want: &TyInfo) -> String {
    let base_ok = ty_compat_call_arg(got.ty, want.ty) || got.ty == want.ty;
    if base_ok && !want.brand.is_none() {
        return format!(
            "E_BRAND_MISMATCH: call {head:?} arg {idx} expected {:?}@{}, got {:?}@{}",
            want.ty,
            tybrand_diag(&want.brand),
            got.ty,
            tybrand_diag(&got.brand),
        );
    }
    format!("call {head:?} arg {idx} expects {:?}", want.ty)
}

fn ty_compat_call_arg_extern(got: Ty, want: Ty) -> bool {
    ty_compat_call_arg(got, want)
        || matches!(
            (got, want),
            (Ty::PtrMutU8, Ty::PtrConstU8)
                | (Ty::PtrMutVoid, Ty::PtrConstVoid)
                | (Ty::PtrMutI32, Ty::PtrConstI32)
        )
}

fn expr_uses_head(expr: &Expr, head: &str) -> bool {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => false,
        Expr::List { items, .. } => {
            if let Some(Expr::Ident { name: h, .. }) = items.first() {
                if h == head {
                    return true;
                }
            }
            items.iter().any(|e| expr_uses_head(e, head))
        }
    }
}

fn program_uses_head(program: &Program, head: &str) -> bool {
    if expr_uses_head(&program.solve, head) {
        return true;
    }
    for f in &program.functions {
        if expr_uses_head(&f.body, head) {
            return true;
        }
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            if expr_uses_head(&c.expr, head) {
                return true;
            }
            for w in &c.witness {
                if expr_uses_head(w, head) {
                    return true;
                }
            }
        }
    }
    for f in &program.async_functions {
        if expr_uses_head(&f.body, head) {
            return true;
        }
        for c in f
            .requires
            .iter()
            .chain(f.ensures.iter())
            .chain(f.invariant.iter())
        {
            if expr_uses_head(&c.expr, head) {
                return true;
            }
            for w in &c.witness {
                if expr_uses_head(w, head) {
                    return true;
                }
            }
        }
    }
    false
}

fn program_uses_contracts(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|f| !f.requires.is_empty() || !f.ensures.is_empty() || !f.invariant.is_empty())
        || program
            .async_functions
            .iter()
            .any(|f| !f.requires.is_empty() || !f.ensures.is_empty() || !f.invariant.is_empty())
}

fn trim_preamble_section(
    src: &str,
    start: &str,
    end: &str,
    label: &str,
) -> Result<String, CompilerError> {
    let (head, rest) = src.split_once(start).ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: runtime preamble missing {label} start marker"),
        )
    })?;
    let (_, tail) = rest.split_once(end).ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: runtime preamble missing {label} end marker"),
        )
    })?;
    let mut out = String::with_capacity(head.len() + end.len() + tail.len());
    out.push_str(head);
    out.push_str(end);
    out.push_str(tail);
    Ok(out)
}

pub fn emit_c(expr: &Expr, options: &CompileOptions) -> Result<String, CompilerError> {
    let program = Program {
        functions: Vec::new(),
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: expr.clone(),
    };
    emit_c_program(&program, options)
}

pub fn emit_c_program(
    program: &Program,
    options: &CompileOptions,
) -> Result<String, CompilerError> {
    emit_c_program_with_native_requires(program, options).map(|(src, _)| src)
}

pub fn emit_c_program_with_native_requires(
    program: &Program,
    options: &CompileOptions,
) -> Result<(String, Vec<NativeBackendReq>), CompilerError> {
    let mut emitter = Emitter::new(program, options.clone());
    emitter.emit_program().map_err(|mut e| {
        if let Some(name) = &emitter.current_fn_name {
            if !e.message.contains("(fn=") {
                if let Some(ptr) = emitter.current_ptr.as_deref().filter(|p| !p.is_empty()) {
                    e.message = format!("{} (fn={name} ptr={ptr})", e.message);
                } else {
                    e.message = format!("{} (fn={name})", e.message);
                }
            }
        }
        if !e.message.contains("ptr=") {
            if let Some(ptr) = emitter.current_ptr.as_deref().filter(|p| !p.is_empty()) {
                e.message = format!("{} (ptr={ptr})", e.message);
            }
        }
        e
    })?;
    let native_requires = emitter.native_requires();
    Ok((emitter.out, native_requires))
}

pub fn check_c_program(program: &Program, options: &CompileOptions) -> Result<(), CompilerError> {
    let mut emitter = Emitter::new(program, options.clone());
    emitter.suppress_output = true;
    emitter.check_program().map_err(|mut e| {
        if let Some(name) = &emitter.current_fn_name {
            if !e.message.contains("(fn=") {
                if let Some(ptr) = emitter.current_ptr.as_deref().filter(|p| !p.is_empty()) {
                    e.message = format!("{} (fn={name} ptr={ptr})", e.message);
                } else {
                    e.message = format!("{} (fn={name})", e.message);
                }
            }
        }
        if !e.message.contains("ptr=") {
            if let Some(ptr) = emitter.current_ptr.as_deref().filter(|p| !p.is_empty()) {
                e.message = format!("{} (ptr={ptr})", e.message);
            }
        }
        e
    })
}

pub fn emit_c_header(options: &CompileOptions) -> Result<String, CompilerError> {
    if options.emit_main {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            "C header emission requires emit_main=false".to_string(),
        ));
    }
    Ok(c_emit_core::RUNTIME_C_HEADER.to_string())
}

struct Emitter<'a> {
    program: &'a Program,
    options: CompileOptions,
    out: String,
    suppress_output: bool,
    indent: usize,
    tmp_counter: u32,
    local_count: usize,
    scopes: Vec<BTreeMap<String, VarRef>>,
    task_scopes: Vec<String>,
    cleanup_scopes: Vec<CleanupScope>,
    fn_c_names: BTreeMap<String, String>,
    async_fn_new_names: BTreeMap<String, String>,
    extern_functions: BTreeMap<String, ExternFunctionDecl>,
    fn_view_return_arg: BTreeMap<String, Option<usize>>,
    fn_option_bytes_view_return_arg: BTreeMap<String, Option<usize>>,
    fn_result_bytes_view_return_arg: BTreeMap<String, Option<usize>>,
    fn_ret_ty: Ty,
    fn_contracts: FnContractsV1,
    allow_async_ops: bool,
    unsafe_depth: usize,
    current_fn_name: Option<String>,
    current_ptr: Option<String>,
    native_requires: BTreeMap<String, NativeReqAcc>,
}

#[derive(Debug, Clone)]
struct NativeReqAcc {
    abi_major: u32,
    features: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct FnContractsV1 {
    requires: Vec<crate::x07ast::ContractClauseAst>,
    ensures: Vec<crate::x07ast::ContractClauseAst>,
    invariant: Vec<crate::x07ast::ContractClauseAst>,
}

impl FnContractsV1 {
    fn has_any(&self) -> bool {
        !self.requires.is_empty() || !self.ensures.is_empty() || !self.invariant.is_empty()
    }
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

fn c_async_new_name(name: &str) -> String {
    let base = c_user_fn_name(name);
    let suffix = base.strip_prefix("user_").unwrap_or(base.as_str());
    format!("async_new_{suffix}")
}

fn c_async_poll_name(name: &str) -> String {
    let base = c_user_fn_name(name);
    let suffix = base.strip_prefix("user_").unwrap_or(base.as_str());
    format!("async_poll_{suffix}")
}

fn c_async_drop_name(name: &str) -> String {
    let base = c_user_fn_name(name);
    let suffix = base.strip_prefix("user_").unwrap_or(base.as_str());
    format!("async_drop_{suffix}")
}

fn c_async_fut_type_name(name: &str) -> String {
    let base = c_user_fn_name(name);
    let suffix = base.strip_prefix("user_").unwrap_or(base.as_str());
    format!("async_fut_{suffix}_t")
}

fn c_ret_ty(ty: Ty) -> &'static str {
    match ty {
        Ty::I32
        | Ty::TaskHandleBytesV1
        | Ty::TaskHandleResultBytesV1
        | Ty::TaskSlotV1
        | Ty::TaskSelectEvtV1
        | Ty::Never => "uint32_t",
        Ty::TaskScopeV1 => "rt_scope_t",
        Ty::BudgetScopeV1 => "rt_budget_scope_t",
        Ty::Bytes => "bytes_t",
        Ty::BytesView => "bytes_view_t",
        Ty::VecU8 => "vec_u8_t",
        Ty::OptionI32 | Ty::OptionTaskSelectEvtV1 => "option_i32_t",
        Ty::OptionBytes => "option_bytes_t",
        Ty::OptionBytesView => "option_bytes_view_t",
        Ty::ResultI32 => "result_i32_t",
        Ty::ResultBytes => "result_bytes_t",
        Ty::ResultBytesView => "result_bytes_view_t",
        Ty::ResultResultBytes => "result_result_bytes_t",
        Ty::Iface => "iface_t",
        Ty::PtrConstU8 => "const uint8_t*",
        Ty::PtrMutU8 => "uint8_t*",
        Ty::PtrConstVoid => "const void*",
        Ty::PtrMutVoid => "void*",
        Ty::PtrConstI32 => "const uint32_t*",
        Ty::PtrMutI32 => "uint32_t*",
    }
}

fn c_zero(ty: Ty) -> &'static str {
    match ty {
        Ty::I32
        | Ty::TaskHandleBytesV1
        | Ty::TaskHandleResultBytesV1
        | Ty::TaskSlotV1
        | Ty::TaskSelectEvtV1
        | Ty::Never => "UINT32_C(0)",
        Ty::TaskScopeV1 => "(rt_scope_t){0}",
        Ty::BudgetScopeV1 => "(rt_budget_scope_t){0}",
        Ty::Bytes => "rt_bytes_empty(ctx)",
        Ty::BytesView => "rt_view_empty(ctx)",
        Ty::VecU8 => "(vec_u8_t){0}",
        Ty::OptionI32 | Ty::OptionTaskSelectEvtV1 => "(option_i32_t){0}",
        Ty::OptionBytes => "(option_bytes_t){0}",
        Ty::OptionBytesView => "(option_bytes_view_t){0}",
        Ty::ResultI32 => "(result_i32_t){0}",
        Ty::ResultBytes => "(result_bytes_t){0}",
        Ty::ResultBytesView => "(result_bytes_view_t){0}",
        Ty::ResultResultBytes => "(result_result_bytes_t){0}",
        Ty::Iface => "(iface_t){0}",
        Ty::PtrConstU8
        | Ty::PtrMutU8
        | Ty::PtrConstVoid
        | Ty::PtrMutVoid
        | Ty::PtrConstI32
        | Ty::PtrMutI32 => "NULL",
    }
}

fn c_empty(ty: Ty) -> &'static str {
    c_zero(ty)
}

fn c_param_list_value(params: &[FunctionParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, p) in params.iter().enumerate() {
        out.push_str(", ");
        out.push_str(match p.ty {
            Ty::I32
            | Ty::TaskHandleBytesV1
            | Ty::TaskHandleResultBytesV1
            | Ty::TaskSlotV1
            | Ty::TaskSelectEvtV1
            | Ty::Never => "uint32_t",
            Ty::TaskScopeV1 => "rt_scope_t",
            Ty::BudgetScopeV1 => "rt_budget_scope_t",
            Ty::Bytes => "bytes_t",
            Ty::BytesView => "bytes_view_t",
            Ty::VecU8 => "vec_u8_t",
            Ty::OptionI32 | Ty::OptionTaskSelectEvtV1 => "option_i32_t",
            Ty::OptionBytes => "option_bytes_t",
            Ty::OptionBytesView => "option_bytes_view_t",
            Ty::ResultI32 => "result_i32_t",
            Ty::ResultBytes => "result_bytes_t",
            Ty::ResultBytesView => "result_bytes_view_t",
            Ty::ResultResultBytes => "result_result_bytes_t",
            Ty::Iface => "iface_t",
            Ty::PtrConstU8 => "const uint8_t*",
            Ty::PtrMutU8 => "uint8_t*",
            Ty::PtrConstVoid => "const void*",
            Ty::PtrMutVoid => "void*",
            Ty::PtrConstI32 => "const uint32_t*",
            Ty::PtrMutI32 => "uint32_t*",
        });
        out.push(' ');
        out.push_str(&format!("p{i}"));
    }
    out
}

fn c_param_list_user(params: &[FunctionParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, p) in params.iter().enumerate() {
        out.push_str(", ");
        out.push_str(match p.ty {
            Ty::I32
            | Ty::TaskHandleBytesV1
            | Ty::TaskHandleResultBytesV1
            | Ty::TaskSlotV1
            | Ty::TaskSelectEvtV1
            | Ty::Never => "uint32_t",
            Ty::TaskScopeV1 => "rt_scope_t",
            Ty::BudgetScopeV1 => "rt_budget_scope_t",
            Ty::Bytes => "bytes_t",
            Ty::BytesView => "bytes_view_t",
            Ty::VecU8 => "vec_u8_t",
            Ty::OptionI32 | Ty::OptionTaskSelectEvtV1 => "option_i32_t",
            Ty::OptionBytes => "option_bytes_t",
            Ty::OptionBytesView => "option_bytes_view_t",
            Ty::ResultI32 => "result_i32_t",
            Ty::ResultBytes => "result_bytes_t",
            Ty::ResultBytesView => "result_bytes_view_t",
            Ty::ResultResultBytes => "result_result_bytes_t",
            Ty::Iface => "iface_t",
            Ty::PtrConstU8 => "const uint8_t*",
            Ty::PtrMutU8 => "uint8_t*",
            Ty::PtrConstVoid => "const void*",
            Ty::PtrMutVoid => "void*",
            Ty::PtrConstI32 => "const uint32_t*",
            Ty::PtrMutI32 => "uint32_t*",
        });
        out.push(' ');
        out.push_str(&format!("p{i}"));
    }
    out
}

fn c_extern_param_list(params: &[FunctionParam]) -> String {
    if params.is_empty() {
        return "void".to_string();
    }
    let mut out = String::new();
    for (idx, p) in params.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(c_ret_ty(p.ty));
        out.push(' ');
        out.push_str(&p.name);
    }
    out
}
