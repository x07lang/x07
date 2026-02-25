use super::c_emit_types::{FnSig, InferCtx};
use super::c_emit_worlds::{
    load_budget_profile_cfg_from_arch_v1, load_rr_cfg_v1_from_arch_v1, parse_budget_scope_cfg_v1,
    parse_bytes_lit_ascii, parse_i32_lit, BudgetScopeModeV1,
};
use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct TaskScopeCfgV1 {
    max_children: u32,
    max_ticks: u64,
    max_blocked_waits: u64,
    max_join_polls: u64,
    max_slot_result_bytes: u32,
}

pub(super) fn parse_task_scope_cfg_v1(expr: &Expr) -> Result<TaskScopeCfgV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope.cfg_v1 must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("task.scope.cfg_v1") {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope cfg must be task.scope.cfg_v1".to_string(),
        ));
    }

    let mut max_children: Option<u32> = None;
    let mut max_ticks: Option<u64> = None;
    let mut max_blocked_waits: Option<u64> = None;
    let mut max_join_polls: Option<u64> = None;
    let mut max_slot_result_bytes: Option<u32> = None;

    for field in items.iter().skip(1) {
        let Expr::List { items: kv, .. } = field else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.cfg_v1 field must be a pair".to_string(),
            ));
        };
        if kv.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.cfg_v1 field must be a pair".to_string(),
            ));
        }
        let Some(key) = kv[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.cfg_v1 key must be an identifier".to_string(),
            ));
        };
        let Expr::Int { value, .. } = kv[1] else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.cfg_v1 value must be an integer".to_string(),
            ));
        };

        if value < 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("task.scope.cfg_v1 {key} must be >= 0"),
            ));
        }

        match key {
            "max_children" => {
                if max_children.replace(value as u32).is_some() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.cfg_v1 has duplicate max_children".to_string(),
                    ));
                }
            }
            "max_ticks" => {
                if max_ticks.replace(value as u64).is_some() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.cfg_v1 has duplicate max_ticks".to_string(),
                    ));
                }
            }
            "max_blocked_waits" => {
                if max_blocked_waits.replace(value as u64).is_some() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.cfg_v1 has duplicate max_blocked_waits".to_string(),
                    ));
                }
            }
            "max_join_polls" => {
                if max_join_polls.replace(value as u64).is_some() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.cfg_v1 has duplicate max_join_polls".to_string(),
                    ));
                }
            }
            "max_slot_result_bytes" => {
                if max_slot_result_bytes.replace(value as u32).is_some() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.cfg_v1 has duplicate max_slot_result_bytes".to_string(),
                    ));
                }
            }
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("task.scope.cfg_v1 unknown field: {key}"),
                ));
            }
        }
    }

    Ok(TaskScopeCfgV1 {
        max_children: max_children.unwrap_or(1024),
        max_ticks: max_ticks.unwrap_or(0),
        max_blocked_waits: max_blocked_waits.unwrap_or(0),
        max_join_polls: max_join_polls.unwrap_or(0),
        max_slot_result_bytes: max_slot_result_bytes.unwrap_or(0),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskSelectPolicyV1 {
    PriorityV1,
    RrV1,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TaskSelectCfgV1 {
    pub(super) max_cases: u32,
    pub(super) poll_sleep_ticks: u32,
    pub(super) max_polls: u32,
    pub(super) policy: TaskSelectPolicyV1,
    pub(super) timeout_ticks: u32,
}

pub(super) fn parse_task_select_cfg_v1(expr: &Expr) -> Result<TaskSelectCfgV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope.select.cfg_v1 must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("task.scope.select.cfg_v1") {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope.select cfg must be task.scope.select.cfg_v1".to_string(),
        ));
    }

    let mut max_cases: Option<u32> = None;
    let mut poll_sleep_ticks: Option<u32> = None;
    let mut max_polls: Option<u32> = None;
    let mut policy: Option<TaskSelectPolicyV1> = None;
    let mut timeout_ticks: Option<u32> = None;

    for field in items.iter().skip(1) {
        let Expr::List { items: kv, .. } = field else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.select.cfg_v1 field must be a pair".to_string(),
            ));
        };
        if kv.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.select.cfg_v1 field must be a pair".to_string(),
            ));
        }
        let Some(key) = kv[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.select.cfg_v1 key must be an identifier".to_string(),
            ));
        };

        match key {
            "policy" => {
                let Some(p) = kv[1].as_ident() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.select.cfg_v1 policy must be an identifier".to_string(),
                    ));
                };
                let p = match p {
                    "priority_v1" => TaskSelectPolicyV1::PriorityV1,
                    "rr_v1" => TaskSelectPolicyV1::RrV1,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "task.scope.select.cfg_v1 policy must be \"priority_v1\" or \"rr_v1\""
                                .to_string(),
                        ));
                    }
                };
                if policy.replace(p).is_some() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.select.cfg_v1 has duplicate policy".to_string(),
                    ));
                }
            }
            "max_cases" | "poll_sleep_ticks" | "max_polls" | "timeout_ticks" => {
                let Expr::Int { value, .. } = kv[1] else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.select.cfg_v1 value must be an integer".to_string(),
                    ));
                };
                if value < 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("task.scope.select.cfg_v1 {key} must be >= 0"),
                    ));
                }
                let x = value as u32;
                match key {
                    "max_cases" => {
                        if max_cases.replace(x).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.select.cfg_v1 has duplicate max_cases".to_string(),
                            ));
                        }
                    }
                    "poll_sleep_ticks" => {
                        if poll_sleep_ticks.replace(x).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.select.cfg_v1 has duplicate poll_sleep_ticks"
                                    .to_string(),
                            ));
                        }
                    }
                    "max_polls" => {
                        if max_polls.replace(x).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.select.cfg_v1 has duplicate max_polls".to_string(),
                            ));
                        }
                    }
                    "timeout_ticks" => {
                        if timeout_ticks.replace(x).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.select.cfg_v1 has duplicate timeout_ticks".to_string(),
                            ));
                        }
                    }
                    _ => unreachable!(),
                }
            }
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("task.scope.select.cfg_v1 unknown field: {key}"),
                ));
            }
        }
    }

    let max_cases = max_cases.ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope.select.cfg_v1 missing max_cases".to_string(),
        )
    })?;
    if max_cases == 0 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope.select.cfg_v1 max_cases must be >= 1".to_string(),
        ));
    }

    Ok(TaskSelectCfgV1 {
        max_cases,
        poll_sleep_ticks: poll_sleep_ticks.unwrap_or(1),
        max_polls: max_polls.unwrap_or(0),
        policy: policy.unwrap_or(TaskSelectPolicyV1::PriorityV1),
        timeout_ticks: timeout_ticks.unwrap_or(0),
    })
}

#[derive(Debug, Clone)]
pub(super) enum TaskSelectCaseV1 {
    SlotBytes { slot: Expr },
    ChanRecvBytes { chan: Expr },
}

pub(super) fn parse_task_select_cases_v1(
    expr: &Expr,
) -> Result<Vec<TaskSelectCaseV1>, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope.select.cases_v1 must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("task.scope.select.cases_v1") {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope.select cases must be task.scope.select.cases_v1".to_string(),
        ));
    }
    if items.len() < 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "task.scope.select.cases_v1 must have at least one case".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(items.len() - 1);
    for case in items.iter().skip(1) {
        let Expr::List { items: inner, .. } = case else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.select.cases_v1 case must be a list".to_string(),
            ));
        };
        let Some(head) = inner.first().and_then(Expr::as_ident) else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.select.cases_v1 case must start with an identifier".to_string(),
            ));
        };
        match head {
            "task.scope.select.case_slot_bytes_v1" => {
                if inner.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects 1 argument"),
                    ));
                }
                out.push(TaskSelectCaseV1::SlotBytes {
                    slot: inner[1].clone(),
                });
            }
            "task.scope.select.case_chan_recv_bytes_v1" => {
                if inner.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects 1 argument"),
                    ));
                }
                out.push(TaskSelectCaseV1::ChanRecvBytes {
                    chan: inner[1].clone(),
                });
            }
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("task.scope.select.cases_v1 unknown case: {head}"),
                ));
            }
        }
    }
    Ok(out)
}

impl<'a> Emitter<'a> {
    pub(super) fn emit_async_function_prototypes(&mut self) {
        for f in &self.program.async_functions {
            self.line(&format!(
                "static uint32_t {}(ctx_t* ctx, void* fut, rt_task_out_t* out);",
                c_async_poll_name(&f.name)
            ));
            self.line(&format!(
                "static uint32_t {}(ctx_t* ctx, bytes_view_t input{});",
                c_async_new_name(&f.name),
                c_param_list_value(&f.params)
            ));
        }
        if !self.program.async_functions.is_empty() {
            self.push_char('\n');
        }
    }

    pub(super) fn emit_async_functions(&mut self) -> Result<(), CompilerError> {
        for f in &self.program.async_functions {
            self.emit_async_function(f)?;
            self.push_char('\n');
        }
        Ok(())
    }

    pub(super) fn emit_async_function(
        &mut self,
        f: &AsyncFunctionDef,
    ) -> Result<(), CompilerError> {
        self.reset_fn_state();
        self.current_fn_name = Some(f.name.clone());
        self.fn_ret_ty = f.ret_ty;
        self.allow_async_ops = true;
        self.emit_source_line_for_symbol(&f.name);

        if f.ret_ty != Ty::Bytes && f.ret_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("defasync {:?} must return bytes or result_bytes", f.name),
            ));
        }

        let fut_type = c_async_fut_type_name(&f.name);
        let poll_name = c_async_poll_name(&f.name);
        let new_name = c_async_new_name(&f.name);
        let drop_name = c_async_drop_name(&f.name);

        let mut fields: Vec<(String, Ty)> = vec![
            ("state".to_string(), Ty::I32),
            ("input".to_string(), Ty::BytesView),
            ("ret".to_string(), f.ret_ty),
        ];
        for (i, p) in f.params.iter().enumerate() {
            fields.push((format!("p{i}"), p.ty));
        }

        let functions = {
            let mut functions: BTreeMap<String, FnSig> = BTreeMap::new();
            for fun in &self.program.functions {
                functions.insert(
                    fun.name.clone(),
                    FnSig {
                        ret: TyInfo {
                            ty: fun.ret_ty,
                            brand: ty_brand_from_opt(&fun.ret_brand),
                            view_full: false,
                        },
                        params: fun
                            .params
                            .iter()
                            .map(|p| TyInfo {
                                ty: p.ty,
                                brand: ty_brand_from_opt(&p.brand),
                                view_full: false,
                            })
                            .collect(),
                    },
                );
            }
            for fun in &self.program.async_functions {
                let (call_ret_ty, call_ret_brand) = match fun.ret_ty {
                    Ty::Bytes => (Ty::TaskHandleBytesV1, ty_brand_from_opt(&fun.ret_brand)),
                    Ty::ResultBytes => (
                        Ty::TaskHandleResultBytesV1,
                        ty_brand_from_opt(&fun.ret_brand),
                    ),
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Internal,
                            format!(
                                "internal error: invalid defasync return type: {:?}",
                                fun.ret_ty
                            ),
                        ));
                    }
                };
                functions.insert(
                    fun.name.clone(),
                    FnSig {
                        ret: TyInfo {
                            ty: call_ret_ty,
                            brand: call_ret_brand,
                            view_full: false,
                        },
                        params: fun
                            .params
                            .iter()
                            .map(|p| TyInfo {
                                ty: p.ty,
                                brand: ty_brand_from_opt(&p.brand),
                                view_full: false,
                            })
                            .collect(),
                    },
                );
            }
            functions
        };

        struct Machine {
            options: CompileOptions,
            functions: BTreeMap<String, FnSig>,
            extern_functions: BTreeMap<String, ExternFunctionDecl>,
            fn_c_names: BTreeMap<String, String>,
            async_fn_new_names: BTreeMap<String, String>,
            native_requires: BTreeMap<String, NativeReqAcc>,
            fields: Vec<(String, Ty)>,
            tmp_counter: u32,
            local_count: usize,
            unsafe_depth: usize,
            scopes: Vec<BTreeMap<String, AsyncVarRef>>,
            task_scopes: Vec<AsyncVarRef>,
            cleanup_scopes: Vec<CleanupScope>,
            states: Vec<Vec<String>>,
            ret_state: usize,
            fn_name: String,
            fn_ret_ty: Ty,
        }

        impl Machine {
            fn storage_ty_for(ty: Ty) -> Ty {
                match ty {
                    Ty::Never => Ty::I32,
                    _ => ty,
                }
            }

            fn new_state(&mut self) -> usize {
                let id = self.states.len();
                self.states.push(Vec::new());
                id
            }

            fn line(&mut self, state: usize, s: impl Into<String>) {
                self.states[state].push(s.into());
            }

            fn trap_ptr_set(&mut self, state: usize, ptr: &str) {
                let ptr = ptr.trim();
                if ptr.is_empty() {
                    self.line(state, "ctx->trap_ptr = NULL;");
                    return;
                }
                let escaped = c_escape_c_string(ptr);
                self.line(state, format!("ctx->trap_ptr = \"{escaped}\";"));
            }

            fn trap_ptr_clear(&mut self, state: usize) {
                self.line(state, "ctx->trap_ptr = NULL;");
            }

            fn push_scope(&mut self) {
                self.scopes.push(BTreeMap::new());
            }

            fn pop_scope(&mut self) {
                self.scopes.pop();
            }

            fn bind(&mut self, name: String, var: AsyncVarRef) {
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name, var);
                }
            }

            fn lookup(&self, name: &str) -> Option<AsyncVarRef> {
                for scope in self.scopes.iter().rev() {
                    if let Some(v) = scope.get(name) {
                        return Some(v.clone());
                    }
                }
                None
            }

            fn lookup_mut(&mut self, name: &str) -> Option<&mut AsyncVarRef> {
                for scope in self.scopes.iter_mut().rev() {
                    if let Some(v) = scope.get_mut(name) {
                        return Some(v);
                    }
                }
                None
            }

            fn require_native_backend(
                &mut self,
                backend_id: &str,
                abi_major: u32,
                feature: &str,
            ) -> Result<(), CompilerError> {
                let mismatch = {
                    let entry = self
                        .native_requires
                        .entry(backend_id.to_string())
                        .or_insert_with(|| NativeReqAcc {
                            abi_major,
                            features: BTreeSet::new(),
                        });

                    if entry.abi_major != abi_major {
                        Some(entry.abi_major)
                    } else {
                        entry.features.insert(feature.to_string());
                        None
                    }
                };

                if let Some(expected) = mismatch {
                    return Err(CompilerError::new(
                        CompileErrorKind::Internal,
                        format!(
                            "native backend ABI mismatch for {backend_id}: got {abi_major} expected {expected}"
                        ),
                    ));
                }
                Ok(())
            }

            fn parse_bytes_lit_text_arg(
                &self,
                head: &str,
                arg: &Expr,
            ) -> Result<String, CompilerError> {
                let Expr::List { items, .. } = arg else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects bytes.lit"),
                    ));
                };
                if items.first().and_then(Expr::as_ident) != Some("bytes.lit") || items.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects bytes.lit"),
                    ));
                }
                let Some(text) = items[1].as_ident() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects bytes.lit"),
                    ));
                };
                Ok(text.to_string())
            }

            fn parse_i32_lit_arg(&self, head: &str, arg: &Expr) -> Result<i32, CompilerError> {
                let Expr::Int { value, .. } = arg else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects integer literal"),
                    ));
                };
                Ok(*value)
            }

            fn lookup_borrowed_bytes_ident_arg(
                &self,
                head: &str,
                arg: &Expr,
            ) -> Result<AsyncVarRef, CompilerError> {
                let Some(name) = arg.as_ident() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects bytes identifier"),
                    ));
                };
                let Some(var) = self.lookup(name) else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                if var.moved {
                    let moved_ptr = var
                        .moved_ptr
                        .as_deref()
                        .filter(|p| !p.is_empty())
                        .unwrap_or("<unknown>");
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("use after move: {name:?} moved_ptr={moved_ptr}"),
                    ));
                }
                if var.ty != Ty::Bytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects bytes identifier"),
                    ));
                }
                Ok(var)
            }

            fn alloc_local(&mut self, prefix: &str, ty: Ty) -> Result<AsyncVarRef, CompilerError> {
                let max_locals = language::limits::max_locals();
                if self.local_count >= max_locals {
                    return Err(CompilerError::new(
                        CompileErrorKind::Budget,
                        format!(
                            "max locals exceeded: {} (fn={}) (hint: split this function body (extract helper defn/defasync) or raise X07_MAX_LOCALS)",
                            max_locals, self.fn_name
                        ),
                    ));
                }
                self.local_count += 1;
                self.tmp_counter += 1;
                let name = format!("{prefix}{}", self.tmp_counter);
                self.fields.push((name.clone(), ty));
                Ok(AsyncVarRef {
                    ty,
                    brand: TyBrand::None,
                    c_name: format!("f->{name}"),
                    moved: false,
                    moved_ptr: None,
                })
            }

            fn infer_expr(&self, expr: &Expr) -> Result<TyInfo, CompilerError> {
                let mut infer = InferCtx {
                    options: self.options.clone(),
                    fn_ret_ty: TyInfo::unbranded(self.fn_ret_ty),
                    allow_async_ops: true,
                    unsafe_depth: self.unsafe_depth,
                    task_scope_depth: self.task_scopes.len(),
                    scopes: self
                        .scopes
                        .iter()
                        .map(|s| {
                            s.iter()
                                .map(|(k, v)| {
                                    (
                                        k.clone(),
                                        TyInfo {
                                            ty: v.ty,
                                            brand: v.brand.clone(),
                                            view_full: false,
                                        },
                                    )
                                })
                                .collect::<BTreeMap<_, _>>()
                        })
                        .collect(),
                    functions: self.functions.clone(),
                    extern_functions: self.extern_functions.clone(),
                };
                infer.infer(expr)
            }

            fn emit_expr_entry(
                &mut self,
                state: usize,
                expr: &Expr,
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                match expr {
                    Expr::Int { .. } | Expr::Ident { .. } | Expr::List { .. } => {}
                }
                match expr {
                    Expr::Int { value: i, .. } => {
                        self.line(state, "rt_fuel(ctx, 1);");
                        if dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "int literal used where bytes expected".to_string(),
                            ));
                        }
                        let v = *i as u32;
                        self.line(state, format!("{} = UINT32_C({v});", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        Ok(())
                    }
                    Expr::Ident { name, ptr: use_ptr } => {
                        self.line(state, "rt_fuel(ctx, 1);");
                        if name == "input" {
                            if dest.ty != Ty::BytesView {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "input is bytes_view".to_string(),
                                ));
                            }
                            self.line(state, format!("{} = f->input;", dest.c_name));
                            self.line(state, format!("goto st_{cont};"));
                            return Ok(());
                        }
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if is_owned_ty(v.ty) && v.moved {
                            let moved_ptr = v
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                ),
                            ));
                        }
                        let wrote = match (dest.ty, v.ty) {
                            (Ty::BytesView, Ty::BytesView) => {
                                self.line(state, format!("{} = {};", dest.c_name, v.c_name));
                                true
                            }
                            (Ty::BytesView, Ty::Bytes) => {
                                self.line(
                                    state,
                                    format!("{} = rt_bytes_view(ctx, {});", dest.c_name, v.c_name),
                                );
                                true
                            }
                            (Ty::BytesView, Ty::VecU8) => {
                                self.line(
                                    state,
                                    format!(
                                        "{} = rt_vec_u8_as_view(ctx, {});",
                                        dest.c_name, v.c_name
                                    ),
                                );
                                true
                            }
                            _ => false,
                        };
                        if !wrote {
                            if v.ty != dest.ty && !ty_compat_task_handle_as_i32(v.ty, dest.ty) {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("type mismatch for identifier {name:?}"),
                                ));
                            }
                            self.line(state, format!("{} = {};", dest.c_name, v.c_name));
                            if is_owned_ty(dest.ty) {
                                if let Some(v) = self.lookup_mut(name) {
                                    v.moved = true;
                                    v.moved_ptr = Some(use_ptr.clone());
                                }
                                self.line(state, format!("{} = {};", v.c_name, c_empty(dest.ty)));
                            }
                        }
                        self.line(state, format!("goto st_{cont};"));
                        Ok(())
                    }
                    Expr::List { items, ptr } => {
                        self.emit_list_entry(state, ptr, items, dest, cont)
                    }
                }
            }

            fn emit_list_entry(
                &mut self,
                state: usize,
                expr_ptr: &str,
                items: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "unsafe" => return self.emit_unsafe(state, args, dest, cont),
                    "begin" => return self.emit_begin(state, args, dest, cont),
                    "let" => return self.emit_let(state, args, dest, cont),
                    "set" => return self.emit_set(state, args, dest, cont),
                    "set0" => return self.emit_set0(state, args, dest, cont),
                    "if" => return self.emit_if(state, args, dest, cont),
                    "for" => return self.emit_for(state, args, dest, cont),
                    "budget.scope_v1" => return self.emit_budget_scope_v1(state, args, dest, cont),
                    "budget.scope_from_arch_v1" => {
                        return self.emit_budget_scope_from_arch_v1(state, args, dest, cont)
                    }
                    "std.rr.with_policy_v1" => {
                        return self.emit_std_rr_with_policy_v1(state, args, dest, cont)
                    }
                    "rr.next_v1" | "std.rr.next_v1" => {
                        return self.emit_rr_next_v1(state, args, dest, cont)
                    }
                    "task.scope_v1" => return self.emit_task_scope_v1(state, args, dest, cont),
                    "task.scope.wait_all_v1" => {
                        return self.emit_task_scope_wait_all_v1(state, args, dest, cont)
                    }
                    "task.scope.select_v1" | "task.scope.select_try_v1" => {
                        return self.emit_task_scope_select_v1(state, head, args, dest, cont)
                    }
                    "return" => return self.emit_return(state, args),
                    _ => {}
                }

                if head == "&&" || head == "||" {
                    if args.len() != 2 || dest.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} expects (i32,i32) and returns i32"),
                        ));
                    }
                    if self.infer_expr(&args[0])? != Ty::I32
                        || self.infer_expr(&args[1])? != Ty::I32
                    {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} expects i32 args"),
                        ));
                    }

                    self.line(state, "rt_fuel(ctx, 1);");
                    let lhs_tmp = self.alloc_local("t_bool_sc_lhs_", Ty::I32)?;
                    let lhs_state = self.new_state();
                    let decide_state = self.new_state();
                    self.line(state, format!("goto st_{lhs_state};"));
                    self.emit_expr_entry(lhs_state, &args[0], lhs_tmp.clone(), decide_state)?;

                    let rhs_state = self.new_state();
                    let rhs_done = self.new_state();

                    match head {
                        "&&" => {
                            self.line(
                                decide_state,
                                format!("if ({} == UINT32_C(0)) {{", lhs_tmp.c_name),
                            );
                            self.line(decide_state, format!("  {} = UINT32_C(0);", dest.c_name));
                            self.line(decide_state, format!("  goto st_{cont};"));
                            self.line(decide_state, "} else {");
                            self.line(decide_state, format!("  goto st_{rhs_state};"));
                            self.line(decide_state, "}");
                        }
                        "||" => {
                            self.line(
                                decide_state,
                                format!("if ({} != UINT32_C(0)) {{", lhs_tmp.c_name),
                            );
                            self.line(decide_state, format!("  {} = UINT32_C(1);", dest.c_name));
                            self.line(decide_state, format!("  goto st_{cont};"));
                            self.line(decide_state, "} else {");
                            self.line(decide_state, format!("  goto st_{rhs_state};"));
                            self.line(decide_state, "}");
                        }
                        _ => unreachable!(),
                    }

                    self.emit_expr_entry(rhs_state, &args[1], dest.clone(), rhs_done)?;
                    self.line(
                        rhs_done,
                        format!("{} = ({} != UINT32_C(0));", dest.c_name, dest.c_name),
                    );
                    self.line(rhs_done, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "vec_u8.len" {
                    if args.len() != 1 || dest.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "vec_u8.len expects vec_u8 and returns i32".to_string(),
                        ));
                    }
                    if let Expr::Ident { name, ptr: use_ptr } = &args[0] {
                        self.line(state, "rt_fuel(ctx, 1);");
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if v.moved {
                            let moved_ptr = v
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                ),
                            ));
                        }
                        if v.ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("type mismatch for identifier {name:?}"),
                            ));
                        }
                        self.line(state, format!("{} = {}.len;", dest.c_name, v.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                }

                if head == "bytes.len" {
                    if args.len() != 1 || dest.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.len expects bytes_view and returns i32".to_string(),
                        ));
                    }
                    if let Expr::Ident { name, ptr: use_ptr } = &args[0] {
                        self.line(state, "rt_fuel(ctx, 1);");
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if is_owned_ty(v.ty) && v.moved {
                            let moved_ptr = v
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                ),
                            ));
                        }
                        if v.ty != Ty::Bytes && v.ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("type mismatch for identifier {name:?}"),
                            ));
                        }
                        self.line(state, format!("{} = {}.len;", dest.c_name, v.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                }

                if head == "bytes.get_u8" {
                    if args.len() != 2 || dest.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.get_u8 expects (bytes_view, i32) and returns i32".to_string(),
                        ));
                    }
                    if let Expr::Ident { name, ptr: use_ptr } = &args[0] {
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if is_owned_ty(v.ty) && v.moved {
                            let moved_ptr = v
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                ),
                            ));
                        }
                        if v.ty != Ty::Bytes && v.ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.get_u8 expects bytes_view".to_string(),
                            ));
                        }
                        if self.infer_expr(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.get_u8 expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        self.line(state, "rt_fuel(ctx, 1);");
                        let idx_tmp = self.alloc_local("t_get_u8_idx_", Ty::I32)?;
                        let idx_state = self.new_state();
                        let apply_state = self.new_state();
                        self.line(state, format!("goto st_{idx_state};"));
                        self.emit_expr_entry(idx_state, &args[1], idx_tmp.clone(), apply_state)?;
                        let view = if v.ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", v.c_name)
                        } else {
                            v.c_name.clone()
                        };
                        self.trap_ptr_set(apply_state, expr_ptr);
                        self.line(
                            apply_state,
                            format!(
                                "{} = rt_view_get_u8(ctx, {}, {});",
                                dest.c_name, view, idx_tmp.c_name
                            ),
                        );
                        self.trap_ptr_clear(apply_state);
                        self.line(apply_state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                }

                if head == "result_bytes.is_ok" {
                    if args.len() != 1 || dest.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "result_bytes.is_ok expects (result_bytes) and returns i32".to_string(),
                        ));
                    }
                    if let Expr::Ident { name, ptr: use_ptr } = &args[0] {
                        self.line(state, "rt_fuel(ctx, 1);");
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if v.moved {
                            let moved_ptr = v
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                ),
                            ));
                        }
                        if v.ty != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.is_ok expects result_bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ({}.tag == UINT32_C(1));", dest.c_name, v.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                }

                if head == "result_bytes.err_code" {
                    if args.len() != 1 || dest.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "result_bytes.err_code expects (result_bytes) and returns i32"
                                .to_string(),
                        ));
                    }
                    if let Expr::Ident { name, ptr: use_ptr } = &args[0] {
                        self.line(state, "rt_fuel(ctx, 1);");
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if v.moved {
                            let moved_ptr = v
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                ),
                            ));
                        }
                        if v.ty != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.err_code expects result_bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
                                dest.c_name, v.c_name, v.c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                }

                if head == "result_result_bytes.is_ok" {
                    if args.len() != 1 || dest.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "result_result_bytes.is_ok expects (result_result_bytes) and returns i32"
                                .to_string(),
                        ));
                    }
                    if let Expr::Ident { name, ptr: use_ptr } = &args[0] {
                        self.line(state, "rt_fuel(ctx, 1);");
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if v.moved {
                            let moved_ptr = v
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                ),
                            ));
                        }
                        if v.ty != Ty::ResultResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.is_ok expects result_result_bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ({}.tag == UINT32_C(1));", dest.c_name, v.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                }

                if head == "result_result_bytes.err_code" {
                    if args.len() != 1 || dest.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "result_result_bytes.err_code expects (result_result_bytes) and returns i32"
                                .to_string(),
                        ));
                    }
                    if let Expr::Ident { name, ptr: use_ptr } = &args[0] {
                        self.line(state, "rt_fuel(ctx, 1);");
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if v.moved {
                            let moved_ptr = v
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                ),
                            ));
                        }
                        if v.ty != Ty::ResultResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.err_code expects result_result_bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
                                dest.c_name, v.c_name, v.c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                }

                if head == "bytes.view" {
                    if args.len() != 1 || dest.ty != Ty::BytesView {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.view expects bytes and returns bytes_view".to_string(),
                        ));
                    }
                    let Expr::Ident { name, ptr: use_ptr } = &args[0] else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.view requires an identifier owner (bind the value to a local with let first)"
                                .to_string(),
                        ));
                    };
                    self.line(state, "rt_fuel(ctx, 1);");
                    let Some(v) = self.lookup(name) else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {name:?}"),
                        ));
                    };
                    if v.moved {
                        let moved_ptr = v
                            .moved_ptr
                            .as_deref()
                            .filter(|p| !p.is_empty())
                            .unwrap_or("<unknown>");
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"),
                        ));
                    }
                    if v.ty != Ty::Bytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.view expects bytes owner".to_string(),
                        ));
                    }
                    self.line(
                        state,
                        format!("{} = rt_bytes_view(ctx, {});", dest.c_name, v.c_name),
                    );
                    self.line(state, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "bytes.subview" {
                    if args.len() != 3 || dest.ty != Ty::BytesView {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.subview expects (bytes, i32, i32) and returns bytes_view"
                                .to_string(),
                        ));
                    }
                    let Expr::Ident {
                        name: owner_name,
                        ptr: use_ptr,
                    } = &args[0]
                    else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.subview requires an identifier owner (bind the value to a local with let first)"
                                .to_string(),
                        ));
                    };
                    let Some(owner) = self.lookup(owner_name) else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {owner_name:?}"),
                        ));
                    };
                    if owner.moved {
                        let moved_ptr = owner
                            .moved_ptr
                            .as_deref()
                            .filter(|p| !p.is_empty())
                            .unwrap_or("<unknown>");
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!(
                                "use after move: {owner_name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                            ),
                        ));
                    }
                    if owner.ty != Ty::Bytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.subview expects bytes owner".to_string(),
                        ));
                    }
                    if self.infer_expr(&args[1])? != Ty::I32
                        || self.infer_expr(&args[2])? != Ty::I32
                    {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.subview expects (bytes, i32, i32)".to_string(),
                        ));
                    }

                    self.line(state, "rt_fuel(ctx, 1);");
                    let start_tmp = self.alloc_local("t_subview_start_", Ty::I32)?;
                    let len_tmp = self.alloc_local("t_subview_len_", Ty::I32)?;
                    let start_state = self.new_state();
                    let len_state = self.new_state();
                    let apply_state = self.new_state();
                    self.line(state, format!("goto st_{start_state};"));
                    self.emit_expr_entry(start_state, &args[1], start_tmp.clone(), len_state)?;
                    self.emit_expr_entry(len_state, &args[2], len_tmp.clone(), apply_state)?;
                    self.trap_ptr_set(apply_state, expr_ptr);
                    self.line(
                        apply_state,
                        format!(
                            "{} = rt_bytes_subview(ctx, {}, {}, {});",
                            dest.c_name, owner.c_name, start_tmp.c_name, len_tmp.c_name
                        ),
                    );
                    self.trap_ptr_clear(apply_state);
                    self.line(apply_state, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "vec_u8.as_view" {
                    if args.len() != 1 || dest.ty != Ty::BytesView {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "vec_u8.as_view expects vec_u8 and returns bytes_view".to_string(),
                        ));
                    }
                    let Expr::Ident { name, ptr: use_ptr } = &args[0] else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "vec_u8.as_view requires an identifier owner (bind the value to a local with let first)"
                                .to_string(),
                        ));
                    };
                    self.line(state, "rt_fuel(ctx, 1);");
                    let Some(v) = self.lookup(name) else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {name:?}"),
                        ));
                    };
                    if v.moved {
                        let moved_ptr = v
                            .moved_ptr
                            .as_deref()
                            .filter(|p| !p.is_empty())
                            .unwrap_or("<unknown>");
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"),
                        ));
                    }
                    if v.ty != Ty::VecU8 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "vec_u8.as_view expects vec_u8 owner".to_string(),
                        ));
                    }
                    self.line(
                        state,
                        format!("{} = rt_vec_u8_as_view(ctx, {});", dest.c_name, v.c_name),
                    );
                    self.line(state, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "bytes.lit" {
                    if args.len() != 1 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Parse,
                            "bytes.lit expects 1 arg".to_string(),
                        ));
                    }
                    if dest.ty != Ty::Bytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.lit returns bytes".to_string(),
                        ));
                    }
                    let ident = args[0].as_ident().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Parse,
                            "bytes.lit expects a text string".to_string(),
                        )
                    })?;
                    let lit_bytes = ident.as_bytes();
                    self.line(state, "rt_fuel(ctx, 1);");
                    self.tmp_counter += 1;
                    let lit_name = format!("lit_{}", self.tmp_counter);
                    let escaped = c_escape_string(lit_bytes);
                    self.line(
                        state,
                        format!("static const char {lit_name}[] = \"{escaped}\";"),
                    );
                    self.line(
                        state,
                        format!(
                            "{} = rt_bytes_from_literal(ctx, (const uint8_t*){lit_name}, UINT32_C({}));",
                            dest.c_name,
                            lit_bytes.len()
                        ),
                    );
                    self.line(state, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "bytes.view_lit" {
                    if args.len() != 1 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Parse,
                            "bytes.view_lit expects 1 arg".to_string(),
                        ));
                    }
                    if dest.ty != Ty::BytesView {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.view_lit returns bytes_view".to_string(),
                        ));
                    }
                    let ident = args[0].as_ident().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Parse,
                            "bytes.view_lit expects a text string".to_string(),
                        )
                    })?;
                    let lit_bytes = ident.as_bytes();
                    self.line(state, "rt_fuel(ctx, 1);");
                    self.tmp_counter += 1;
                    let lit_name = format!("lit_{}", self.tmp_counter);
                    let escaped = c_escape_string(lit_bytes);
                    self.line(
                        state,
                        format!("static const char {lit_name}[] = \"{escaped}\";"),
                    );
                    self.line(
                        state,
                        format!(
                            "{} = rt_view_from_literal(ctx, (const uint8_t*){lit_name}, UINT32_C({}));",
                            dest.c_name,
                            lit_bytes.len()
                        ),
                    );
                    self.line(state, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "__internal.stream_xf.plugin_init_v1" {
                    if args.len() != 12 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Parse,
                            "__internal.stream_xf.plugin_init_v1 expects 12 args".to_string(),
                        ));
                    }
                    if dest.ty != Ty::ResultBytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_init_v1 returns result_bytes".to_string(),
                        ));
                    }

                    let backend_id = self.parse_bytes_lit_text_arg(
                        "__internal.stream_xf.plugin_init_v1 backend_id",
                        &args[0],
                    )?;
                    crate::validate::validate_symbol(&backend_id)
                        .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

                    let abi_major_i32 = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_init_v1 abi_major",
                        &args[1],
                    )?;
                    let abi_major = u32::try_from(abi_major_i32).unwrap_or(0);
                    if abi_major == 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_init_v1 abi_major must be >= 1"
                                .to_string(),
                        ));
                    }

                    let export_symbol = self.parse_bytes_lit_text_arg(
                        "__internal.stream_xf.plugin_init_v1 export_symbol",
                        &args[2],
                    )?;
                    crate::validate::validate_local_name(&export_symbol)
                        .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

                    self.require_native_backend(&backend_id, abi_major, &export_symbol)?;

                    let state_b = self.lookup_borrowed_bytes_ident_arg(
                        "__internal.stream_xf.plugin_init_v1 state",
                        &args[3],
                    )?;
                    let scratch_b = self.lookup_borrowed_bytes_ident_arg(
                        "__internal.stream_xf.plugin_init_v1 scratch",
                        &args[4],
                    )?;
                    let cfg_b = self.lookup_borrowed_bytes_ident_arg(
                        "__internal.stream_xf.plugin_init_v1 cfg",
                        &args[5],
                    )?;

                    let cfg_max_bytes = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_init_v1 cfg_max_bytes",
                        &args[6],
                    )?;
                    if cfg_max_bytes < 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_init_v1 cfg_max_bytes must be >= 0"
                                .to_string(),
                        ));
                    }

                    let canon_mode = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_init_v1 canon_mode",
                        &args[7],
                    )?;
                    if canon_mode != 0 && canon_mode != 1 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_init_v1 canon_mode must be 0 or 1"
                                .to_string(),
                        ));
                    }

                    let strict_cfg_canon = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_init_v1 strict_cfg_canon",
                        &args[8],
                    )?;
                    if strict_cfg_canon != 0 && strict_cfg_canon != 1 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_init_v1 strict_cfg_canon must be 0 or 1"
                                .to_string(),
                        ));
                    }

                    let max_out_bytes_per_step = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_init_v1 max_out_bytes_per_step",
                        &args[9],
                    )?;
                    let max_out_items_per_step = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_init_v1 max_out_items_per_step",
                        &args[10],
                    )?;
                    let max_out_buf_bytes = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_init_v1 max_out_buf_bytes",
                        &args[11],
                    )?;
                    if max_out_bytes_per_step < 0
                        || max_out_items_per_step < 0
                        || max_out_buf_bytes < 0
                    {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_init_v1 limits must be >= 0".to_string(),
                        ));
                    }

                    self.line(state, "rt_fuel(ctx, 1);");
                    self.line(
                        state,
                        format!("extern x07_stream_xf_plugin_v1 {export_symbol};"),
                    );
                    self.line(state, format!(
                        "{} = rt_stream_xf_plugin_init_v1(ctx, &{export_symbol}, UINT32_C({abi_major}), {}, {}, {}, (uint32_t){cfg_max_bytes}, (uint32_t){canon_mode}, (uint32_t){strict_cfg_canon}, (uint32_t){max_out_bytes_per_step}, (uint32_t){max_out_items_per_step}, (uint32_t){max_out_buf_bytes});",
                        dest.c_name,
                        state_b.c_name,
                        scratch_b.c_name,
                        cfg_b.c_name,
                    ));
                    self.line(state, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "__internal.stream_xf.plugin_step_v1" {
                    if args.len() != 9 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Parse,
                            "__internal.stream_xf.plugin_step_v1 expects 9 args".to_string(),
                        ));
                    }
                    if dest.ty != Ty::ResultBytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_step_v1 returns result_bytes".to_string(),
                        ));
                    }

                    let backend_id = self.parse_bytes_lit_text_arg(
                        "__internal.stream_xf.plugin_step_v1 backend_id",
                        &args[0],
                    )?;
                    crate::validate::validate_symbol(&backend_id)
                        .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

                    let abi_major_i32 = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_step_v1 abi_major",
                        &args[1],
                    )?;
                    let abi_major = u32::try_from(abi_major_i32).unwrap_or(0);
                    if abi_major == 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_step_v1 abi_major must be >= 1"
                                .to_string(),
                        ));
                    }

                    let export_symbol = self.parse_bytes_lit_text_arg(
                        "__internal.stream_xf.plugin_step_v1 export_symbol",
                        &args[2],
                    )?;
                    crate::validate::validate_local_name(&export_symbol)
                        .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

                    self.require_native_backend(&backend_id, abi_major, &export_symbol)?;

                    let state_b = self.lookup_borrowed_bytes_ident_arg(
                        "__internal.stream_xf.plugin_step_v1 state",
                        &args[3],
                    )?;
                    let scratch_b = self.lookup_borrowed_bytes_ident_arg(
                        "__internal.stream_xf.plugin_step_v1 scratch",
                        &args[4],
                    )?;

                    let max_out_bytes_per_step = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_step_v1 max_out_bytes_per_step",
                        &args[5],
                    )?;
                    let max_out_items_per_step = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_step_v1 max_out_items_per_step",
                        &args[6],
                    )?;
                    let max_out_buf_bytes = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_step_v1 max_out_buf_bytes",
                        &args[7],
                    )?;
                    if max_out_bytes_per_step < 0
                        || max_out_items_per_step < 0
                        || max_out_buf_bytes < 0
                    {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_step_v1 limits must be >= 0".to_string(),
                        ));
                    }

                    let input_tmp = self.alloc_local("t_xf_in_", Ty::BytesView)?;
                    let expr_state = self.new_state();
                    let after = self.new_state();
                    self.line(state, format!("goto st_{expr_state};"));
                    self.emit_expr_entry(expr_state, &args[8], input_tmp.clone(), after)?;

                    self.line(after, "rt_fuel(ctx, 1);");
                    self.line(
                        after,
                        format!("extern x07_stream_xf_plugin_v1 {export_symbol};"),
                    );
                    self.line(after, format!(
                        "{} = rt_stream_xf_plugin_step_v1(ctx, &{export_symbol}, UINT32_C({abi_major}), {}, {}, (uint32_t){max_out_bytes_per_step}, (uint32_t){max_out_items_per_step}, (uint32_t){max_out_buf_bytes}, {});",
                        dest.c_name,
                        state_b.c_name,
                        scratch_b.c_name,
                        input_tmp.c_name,
                    ));
                    self.line(after, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "__internal.stream_xf.plugin_flush_v1" {
                    if args.len() != 8 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Parse,
                            "__internal.stream_xf.plugin_flush_v1 expects 8 args".to_string(),
                        ));
                    }
                    if dest.ty != Ty::ResultBytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_flush_v1 returns result_bytes".to_string(),
                        ));
                    }

                    let backend_id = self.parse_bytes_lit_text_arg(
                        "__internal.stream_xf.plugin_flush_v1 backend_id",
                        &args[0],
                    )?;
                    crate::validate::validate_symbol(&backend_id)
                        .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

                    let abi_major_i32 = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_flush_v1 abi_major",
                        &args[1],
                    )?;
                    let abi_major = u32::try_from(abi_major_i32).unwrap_or(0);
                    if abi_major == 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_flush_v1 abi_major must be >= 1"
                                .to_string(),
                        ));
                    }

                    let export_symbol = self.parse_bytes_lit_text_arg(
                        "__internal.stream_xf.plugin_flush_v1 export_symbol",
                        &args[2],
                    )?;
                    crate::validate::validate_local_name(&export_symbol)
                        .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

                    self.require_native_backend(&backend_id, abi_major, &export_symbol)?;

                    let state_b = self.lookup_borrowed_bytes_ident_arg(
                        "__internal.stream_xf.plugin_flush_v1 state",
                        &args[3],
                    )?;
                    let scratch_b = self.lookup_borrowed_bytes_ident_arg(
                        "__internal.stream_xf.plugin_flush_v1 scratch",
                        &args[4],
                    )?;

                    let max_out_bytes_per_step = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_flush_v1 max_out_bytes_per_step",
                        &args[5],
                    )?;
                    let max_out_items_per_step = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_flush_v1 max_out_items_per_step",
                        &args[6],
                    )?;
                    let max_out_buf_bytes = self.parse_i32_lit_arg(
                        "__internal.stream_xf.plugin_flush_v1 max_out_buf_bytes",
                        &args[7],
                    )?;
                    if max_out_bytes_per_step < 0
                        || max_out_items_per_step < 0
                        || max_out_buf_bytes < 0
                    {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.stream_xf.plugin_flush_v1 limits must be >= 0".to_string(),
                        ));
                    }

                    self.line(state, "rt_fuel(ctx, 1);");
                    self.line(
                        state,
                        format!("extern x07_stream_xf_plugin_v1 {export_symbol};"),
                    );
                    self.line(state, format!(
                        "{} = rt_stream_xf_plugin_flush_v1(ctx, &{export_symbol}, UINT32_C({abi_major}), {}, {}, (uint32_t){max_out_bytes_per_step}, (uint32_t){max_out_items_per_step}, (uint32_t){max_out_buf_bytes});",
                        dest.c_name,
                        state_b.c_name,
                        scratch_b.c_name,
                    ));
                    self.line(state, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "__internal.brand.assume_view_v1" {
                    if args.len() != 2 || dest.ty != Ty::BytesView {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "__internal.brand.assume_view_v1 expects (brand_id, bytes_view) and returns bytes_view".to_string(),
                        ));
                    }
                    let brand_id = args[0].as_ident().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Parse,
                            "__internal.brand.assume_view_v1 expects a brand_id string".to_string(),
                        )
                    })?;
                    crate::validate::validate_symbol(brand_id)
                        .map_err(|message| CompilerError::new(CompileErrorKind::Parse, message))?;

                    self.line(state, "rt_fuel(ctx, 1);");
                    let expr_state = self.new_state();
                    self.line(state, format!("goto st_{expr_state};"));
                    self.emit_expr_entry(expr_state, &args[1], dest, cont)?;
                    return Ok(());
                }

                if head == "ptr.cast" {
                    return self.emit_ptr_cast_form(state, args, dest, cont);
                }
                if head == "addr_of" {
                    return self.emit_addr_of_form(state, args, dest, cont, false);
                }
                if head == "addr_of_mut" {
                    return self.emit_addr_of_form(state, args, dest, cont, true);
                }

                let call_ty = self.infer_expr(&Expr::List {
                    items: items.to_vec(),
                    ptr: String::new(),
                })?;
                let dest_ty = TyInfo {
                    ty: dest.ty,
                    brand: dest.brand.clone(),
                    view_full: false,
                };
                if call_ty != Ty::Never && !tyinfo_compat_assign(&call_ty, &dest_ty) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("call must evaluate to {:?}, got {:?}", dest.ty, call_ty),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");

                let want_params: Option<Vec<Ty>> = if self.extern_functions.contains_key(head) {
                    let f = self.extern_functions.get(head).cloned().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Internal,
                            format!("internal error: missing extern decl for {head:?}"),
                        )
                    })?;
                    Some(f.params.iter().map(|p| p.ty).collect())
                } else if self.fn_c_names.contains_key(head)
                    || self.async_fn_new_names.contains_key(head)
                {
                    let sig = self.functions.get(head).cloned().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Internal,
                            format!("internal error: missing function signature for {head:?}"),
                        )
                    })?;
                    Some(sig.params.into_iter().map(|p| p.ty).collect())
                } else {
                    None
                };
                if let Some(want) = &want_params {
                    if want.len() != args.len() {
                        return Err(CompilerError::new(
                            CompileErrorKind::Parse,
                            format!("call {head:?} expects {} args", want.len()),
                        ));
                    }
                }

                let mut arg_vars: Vec<AsyncVarRef> = Vec::with_capacity(args.len());
                let mut arg_pre_states: Vec<usize> = Vec::with_capacity(args.len());
                let mut arg_eval_states: Vec<usize> = Vec::with_capacity(args.len());
                for (i, arg_expr) in args.iter().enumerate() {
                    let inferred = self.infer_expr(arg_expr)?.ty;
                    let mut ty = match &want_params {
                        Some(want) => want[i],
                        None => inferred,
                    };

                    // Builtin calls default to evaluating args at their inferred type. For certain
                    // builtins that accept `bytes_view` (and allow `bytes` as a call-arg), prefer
                    // borrowing from identifier `bytes` to avoid incorrect moves in async codegen.
                    if want_params.is_none()
                        && matches!(
                            (head, i, inferred, arg_expr),
                            ("vec_u8.extend_bytes", 1, Ty::Bytes, Expr::Ident { .. })
                                | (
                                    "vec_u8.extend_bytes_range",
                                    1,
                                    Ty::Bytes,
                                    Expr::Ident { .. }
                                )
                        )
                    {
                        ty = Ty::BytesView;
                    }
                    let storage_ty = match ty {
                        Ty::Never => Ty::I32,
                        other => other,
                    };
                    let tmp = self.alloc_local("t_arg_", storage_ty)?;
                    arg_vars.push(tmp);
                    arg_pre_states.push(self.new_state());
                    arg_eval_states.push(self.new_state());
                }

                let apply_state = self.new_state();
                if let Some(first) = arg_pre_states.first().copied() {
                    self.line(state, format!("goto st_{first};"));
                } else {
                    self.line(state, format!("goto st_{apply_state};"));
                }

                for i in 0..arg_pre_states.len() {
                    let eval_state = arg_eval_states[i];
                    let pre_state = arg_pre_states[i];
                    let next = if i + 1 < arg_pre_states.len() {
                        arg_pre_states[i + 1]
                    } else {
                        apply_state
                    };
                    let expr = &args[i];
                    let tmp = arg_vars[i].clone();

                    // Arg temps are fields on the async future and may be reused across loop
                    // iterations. Drop any previous owned value before overwriting to avoid leaks.
                    if is_owned_ty(tmp.ty) {
                        match tmp.ty {
                            Ty::Bytes => self
                                .line(pre_state, format!("rt_bytes_drop(ctx, &{});", tmp.c_name)),
                            Ty::VecU8 => self
                                .line(pre_state, format!("rt_vec_u8_drop(ctx, &{});", tmp.c_name)),
                            Ty::OptionBytes => {
                                self.line(pre_state, format!("if ({}.tag) {{", tmp.c_name));
                                self.line(
                                    pre_state,
                                    format!("  rt_bytes_drop(ctx, &{}.payload);", tmp.c_name),
                                );
                                self.line(pre_state, "}");
                                self.line(pre_state, format!("{}.tag = UINT32_C(0);", tmp.c_name));
                            }
                            Ty::ResultBytes => {
                                self.line(pre_state, format!("if ({}.tag) {{", tmp.c_name));
                                self.line(
                                    pre_state,
                                    format!("  rt_bytes_drop(ctx, &{}.payload.ok);", tmp.c_name),
                                );
                                self.line(pre_state, "}");
                                self.line(pre_state, format!("{}.tag = UINT32_C(0);", tmp.c_name));
                                self.line(
                                    pre_state,
                                    format!("{}.payload.err = UINT32_C(0);", tmp.c_name),
                                );
                            }
                            Ty::ResultResultBytes => {
                                self.line(pre_state, format!("if ({}.tag) {{", tmp.c_name));
                                self.line(
                                    pre_state,
                                    format!("  if ({}.payload.ok.tag) {{", tmp.c_name),
                                );
                                self.line(
                                    pre_state,
                                    format!(
                                        "    rt_bytes_drop(ctx, &{}.payload.ok.payload.ok);",
                                        tmp.c_name
                                    ),
                                );
                                self.line(pre_state, "  }");
                                self.line(
                                    pre_state,
                                    format!("  {}.payload.ok.tag = UINT32_C(0);", tmp.c_name),
                                );
                                self.line(
                                    pre_state,
                                    format!(
                                        "  {}.payload.ok.payload.err = UINT32_C(0);",
                                        tmp.c_name
                                    ),
                                );
                                self.line(pre_state, "}");
                                self.line(pre_state, format!("{}.tag = UINT32_C(0);", tmp.c_name));
                                self.line(
                                    pre_state,
                                    format!("{}.payload.err = UINT32_C(0);", tmp.c_name),
                                );
                            }
                            _ => {}
                        }
                    }
                    self.line(pre_state, format!("goto st_{eval_state};"));
                    self.emit_expr_entry(eval_state, expr, tmp, next)?;
                }

                self.emit_apply_call(apply_state, expr_ptr, head, &arg_vars, dest, cont)
            }

            fn emit_ptr_cast_form(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if !self.options.allow_unsafe() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!(
                            "ptr.cast requires unsafe capability; {}",
                            self.options.hint_enable_unsafe()
                        ),
                    ));
                }
                if self.unsafe_depth == 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "unsafe-required: ptr.cast".to_string(),
                    ));
                }
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "ptr.cast form: (ptr.cast <ptr_ty> <ptr>)".to_string(),
                    ));
                }
                self.line(state, "rt_fuel(ctx, 1);");

                let ty_name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "ptr.cast target type must be an identifier".to_string(),
                    )
                })?;
                let target = Ty::parse_named(ty_name).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("ptr.cast unknown type: {ty_name:?}"),
                    )
                })?;
                if !target.is_ptr_ty() || dest.ty != target {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "ptr.cast target type must match the expression type".to_string(),
                    ));
                }

                let ptr_expr = &args[1];
                let src_ty = self.infer_expr(ptr_expr)?;
                if !src_ty.is_ptr_ty() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "ptr.cast expects a raw pointer".to_string(),
                    ));
                }

                let tmp = self.alloc_local("t_cast_", src_ty.ty)?;
                let expr_state = self.new_state();
                let after = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));
                self.emit_expr_entry(expr_state, ptr_expr, tmp.clone(), after)?;
                self.line(
                    after,
                    format!("{} = ({}){};", dest.c_name, c_ret_ty(target), tmp.c_name),
                );
                self.line(after, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_addr_of_form(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
                is_mut: bool,
            ) -> Result<(), CompilerError> {
                let head = if is_mut { "addr_of_mut" } else { "addr_of" };
                if !self.options.allow_unsafe() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!(
                            "{head} requires unsafe capability; {}",
                            self.options.hint_enable_unsafe()
                        ),
                    ));
                }
                if args.len() != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "addr_of expects 1 arg".to_string(),
                    ));
                }
                let want = if is_mut {
                    Ty::PtrMutVoid
                } else {
                    Ty::PtrConstVoid
                };
                if dest.ty != want {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "addr_of returns a void raw pointer".to_string(),
                    ));
                }
                let name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "addr_of expects an identifier".to_string(),
                    )
                })?;

                let lvalue = if name == "input" {
                    "f->input".to_string()
                } else {
                    let Some(var) = self.lookup(name) else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {name:?}"),
                        ));
                    };
                    var.c_name
                };

                self.line(state, "rt_fuel(ctx, 1);");
                let cty = if is_mut { "void*" } else { "const void*" };
                self.line(state, format!("{} = ({cty})&({lvalue});", dest.c_name));
                self.line(state, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_apply_call(
                &mut self,
                state: usize,
                call_ptr: &str,
                head: &str,
                args: &[AsyncVarRef],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                match head {
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<u" | ">>u" | "=" | "!="
                    | "<" | "<=" | ">" | ">=" | "<u" | ">=u" | ">u" | "<=u" => {
                        if args.len() != 2 || dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects 2 i32 args"),
                            ));
                        }
                        let a = &args[0];
                        let b = &args[1];
                        if a.ty != Ty::I32 || b.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 args"),
                            ));
                        }
                        match head {
                            "+" => self.line(
                                state,
                                format!("{} = {} + {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "-" => self.line(
                                state,
                                format!("{} = {} - {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "*" => self.line(
                                state,
                                format!("{} = {} * {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "/" => self.line(
                                state,
                                format!(
                                    "{} = ({} == UINT32_C(0)) ? UINT32_C(0) : ({} / {});",
                                    dest.c_name, b.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "%" => self.line(
                                state,
                                format!(
                                    "{} = ({} == UINT32_C(0)) ? {} : ({} % {});",
                                    dest.c_name, b.c_name, a.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "&" => self.line(
                                state,
                                format!("{} = {} & {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "|" => self.line(
                                state,
                                format!("{} = {} | {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "^" => self.line(
                                state,
                                format!("{} = {} ^ {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "<<u" => self.line(
                                state,
                                format!(
                                    "{} = {} << ({} & UINT32_C(31));",
                                    dest.c_name, a.c_name, b.c_name
                                ),
                            ),
                            ">>u" => self.line(
                                state,
                                format!(
                                    "{} = {} >> ({} & UINT32_C(31));",
                                    dest.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "=" => self.line(
                                state,
                                format!("{} = ({} == {});", dest.c_name, a.c_name, b.c_name),
                            ),
                            "!=" => self.line(
                                state,
                                format!("{} = ({} != {});", dest.c_name, a.c_name, b.c_name),
                            ),
                            "<" => self.line(
                                state,
                                format!(
                                    "{} = (({} ^ UINT32_C(0x80000000)) < ({} ^ UINT32_C(0x80000000)));",
                                    dest.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "<=" => self.line(
                                state,
                                format!(
                                    "{} = (({} ^ UINT32_C(0x80000000)) >= ({} ^ UINT32_C(0x80000000)));",
                                    dest.c_name, b.c_name, a.c_name
                                ),
                            ),
                            ">" => self.line(
                                state,
                                format!(
                                    "{} = (({} ^ UINT32_C(0x80000000)) < ({} ^ UINT32_C(0x80000000)));",
                                    dest.c_name, b.c_name, a.c_name
                                ),
                            ),
                            ">=" => self.line(
                                state,
                                format!(
                                    "{} = (({} ^ UINT32_C(0x80000000)) >= ({} ^ UINT32_C(0x80000000)));",
                                    dest.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "<u" => self.line(
                                state,
                                format!("{} = ({} < {});", dest.c_name, a.c_name, b.c_name),
                            ),
                            ">=u" => self.line(
                                state,
                                format!("{} = ({} >= {});", dest.c_name, a.c_name, b.c_name),
                            ),
                            ">u" => self.line(
                                state,
                                format!("{} = ({} < {});", dest.c_name, b.c_name, a.c_name),
                            ),
                            "<=u" => self.line(
                                state,
                                format!("{} = ({} >= {});", dest.c_name, b.c_name, a.c_name),
                            ),
                            _ => unreachable!(),
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.len" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.len expects bytes_view".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.len;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.get_u8" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.get_u8 expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        let v = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        self.trap_ptr_set(state, call_ptr);
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_get_u8(ctx, {}, {});",
                                dest.c_name, v, args[1].c_name
                            ),
                        );
                        self.trap_ptr_clear(state);
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.set_u8" => {
                        if args.len() != 3
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
                            ));
                        }
                        self.trap_ptr_set(state, call_ptr);
                        self.line(
                            state,
                            format!(
                                "{} = rt_bytes_set_u8(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.trap_ptr_clear(state);
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.add_v1" | "math.f64.sub_v1" | "math.f64.mul_v1"
                    | "math.f64.div_v1" | "math.f64.pow_v1" | "math.f64.atan2_v1"
                    | "math.f64.min_v1" | "math.f64.max_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_MATH,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (bytes_view, bytes_view)"),
                            ));
                        }
                        let a = if args[0].ty == Ty::Bytes {
                            args[0].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[0].c_name, args[0].c_name
                            )
                        };
                        let b = if args[1].ty == Ty::Bytes {
                            args[1].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[1].c_name, args[1].c_name
                            )
                        };
                        let c_fn = match head {
                            "math.f64.add_v1" => "ev_math_f64_add_v1",
                            "math.f64.sub_v1" => "ev_math_f64_sub_v1",
                            "math.f64.mul_v1" => "ev_math_f64_mul_v1",
                            "math.f64.div_v1" => "ev_math_f64_div_v1",
                            "math.f64.pow_v1" => "ev_math_f64_pow_v1",
                            "math.f64.atan2_v1" => "ev_math_f64_atan2_v1",
                            "math.f64.min_v1" => "ev_math_f64_min_v1",
                            "math.f64.max_v1" => "ev_math_f64_max_v1",
                            _ => unreachable!(),
                        };
                        self.line(state, format!("{} = {c_fn}({a}, {b});", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.sqrt_v1"
                    | "math.f64.neg_v1"
                    | "math.f64.abs_v1"
                    | "math.f64.sin_v1"
                    | "math.f64.cos_v1"
                    | "math.f64.tan_v1"
                    | "math.f64.exp_v1"
                    | "math.f64.log_v1"
                    | "math.f64.floor_v1"
                    | "math.f64.ceil_v1"
                    | "math.f64.fmt_shortest_v1"
                    | "math.f64.to_bits_u64le_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_MATH,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes_view"),
                            ));
                        }
                        let x = if args[0].ty == Ty::Bytes {
                            args[0].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[0].c_name, args[0].c_name
                            )
                        };
                        let c_fn = match head {
                            "math.f64.sqrt_v1" => "ev_math_f64_sqrt_v1",
                            "math.f64.neg_v1" => "ev_math_f64_neg_v1",
                            "math.f64.abs_v1" => "ev_math_f64_abs_v1",
                            "math.f64.sin_v1" => "ev_math_f64_sin_v1",
                            "math.f64.cos_v1" => "ev_math_f64_cos_v1",
                            "math.f64.tan_v1" => "ev_math_f64_tan_v1",
                            "math.f64.exp_v1" => "ev_math_f64_exp_v1",
                            "math.f64.log_v1" => "ev_math_f64_ln_v1",
                            "math.f64.floor_v1" => "ev_math_f64_floor_v1",
                            "math.f64.ceil_v1" => "ev_math_f64_ceil_v1",
                            "math.f64.fmt_shortest_v1" => "ev_math_f64_fmt_shortest_v1",
                            "math.f64.to_bits_u64le_v1" => "ev_math_f64_to_bits_u64le_v1",
                            _ => unreachable!(),
                        };
                        self.line(state, format!("{} = {c_fn}({x});", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.from_i32_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_MATH,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.from_i32_v1 expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ev_math_f64_from_i32_v1({});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.to_i32_trunc_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_MATH,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if args.len() != 1
                            || dest.ty != Ty::ResultI32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.to_i32_trunc_v1 expects bytes_view".to_string(),
                            ));
                        }
                        let x = if args[0].ty == Ty::Bytes {
                            args[0].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[0].c_name, args[0].c_name
                            )
                        };
                        self.line(
                            state,
                            format!("{} = ev_math_f64_to_i32_trunc_v1({x});", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.parse_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_MATH,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if args.len() != 1
                            || dest.ty != Ty::ResultBytes
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.parse_v1 expects bytes_view".to_string(),
                            ));
                        }
                        let s = if args[0].ty == Ty::Bytes {
                            args[0].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[0].c_name, args[0].c_name
                            )
                        };
                        self.line(
                            state,
                            format!("{} = ev_math_f64_parse_v1({s});", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.alloc" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.alloc expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_bytes_alloc(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "__internal.bytes.alloc_aligned_v1" => {
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.bytes.alloc_aligned_v1 expects (i32 len, i32 align) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_bytes_alloc_aligned(ctx, (uint32_t){}, (uint32_t){});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "__internal.bytes.clone_v1" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.bytes.clone_v1 expects bytes and returns bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_bytes_clone(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "__internal.bytes.drop_v1" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.bytes.drop_v1 expects bytes and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(state, format!("rt_bytes_drop(ctx, &{});", args[0].c_name));
                        self.line(
                            state,
                            format!("{} = {};", args[0].c_name, c_empty(Ty::Bytes)),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.empty" => {
                        if !args.is_empty() || dest.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.empty expects 0 args".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = rt_bytes_empty(ctx);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes1" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes1 expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_bytes_alloc(ctx, UINT32_C(1));", dest.c_name),
                        );
                        self.trap_ptr_set(state, call_ptr);
                        self.line(
                            state,
                            format!(
                                "{} = rt_bytes_set_u8(ctx, {}, UINT32_C(0), {});",
                                dest.c_name, dest.c_name, args[0].c_name
                            ),
                        );
                        self.trap_ptr_clear(state);
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.lit" => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Unsupported,
                            "bytes.lit is not supported inside async codegen yet".to_string(),
                        ));
                    }
                    "bytes.slice" => {
                        if args.len() != 3
                            || dest.ty != Ty::Bytes
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.slice expects (bytes_view, i32, i32)".to_string(),
                            ));
                        }
                        let v = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        self.trap_ptr_set(state, call_ptr);
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_to_bytes(ctx, rt_view_slice(ctx, {}, {}, {}));",
                                dest.c_name, v, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.trap_ptr_clear(state);
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.eq" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.eq expects (bytes_view, bytes_view)".to_string(),
                            ));
                        }
                        let a = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        let b = if args[1].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[1].c_name)
                        } else {
                            args[1].c_name.clone()
                        };
                        self.line(
                            state,
                            format!("{} = rt_view_eq(ctx, {}, {});", dest.c_name, a, b),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.cmp_range" => {
                        if args.len() != 6
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                            || (args[3].ty != Ty::Bytes && args[3].ty != Ty::BytesView)
                            || args[4].ty != Ty::I32
                            || args[5].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.cmp_range expects (bytes_view,i32,i32,bytes_view,i32,i32)"
                                    .to_string(),
                            ));
                        }
                        let a = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        let b = if args[3].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[3].c_name)
                        } else {
                            args[3].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_cmp_range(ctx, {}, {}, {}, {}, {}, {});",
                                dest.c_name,
                                a,
                                args[1].c_name,
                                args[2].c_name,
                                b,
                                args[4].c_name,
                                args[5].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.as_ptr" => {
                        if args.len() != 1 || dest.ty != Ty::PtrConstU8 || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.as_ptr expects bytes".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.ptr;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.as_mut_ptr" => {
                        if args.len() != 1 || dest.ty != Ty::PtrMutU8 || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.as_mut_ptr expects bytes".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.ptr;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.view" => {
                        if args.len() != 1 || dest.ty != Ty::BytesView || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.view expects bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_bytes_view(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.subview" => {
                        if args.len() != 3
                            || dest.ty != Ty::BytesView
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.subview expects (bytes,i32,i32)".to_string(),
                            ));
                        }
                        self.trap_ptr_set(state, call_ptr);
                        self.line(
                            state,
                            format!(
                                "{} = rt_bytes_subview(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.trap_ptr_clear(state);
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.len expects bytes_view".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.len;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.get_u8" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.get_u8 expects (bytes_view,i32)".to_string(),
                            ));
                        }
                        self.trap_ptr_set(state, call_ptr);
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_get_u8(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.trap_ptr_clear(state);
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.slice" => {
                        if args.len() != 3
                            || dest.ty != Ty::BytesView
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.slice expects (bytes_view,i32,i32)".to_string(),
                            ));
                        }
                        self.trap_ptr_set(state, call_ptr);
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_slice(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.trap_ptr_clear(state);
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.to_bytes" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.to_bytes expects bytes_view".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_to_bytes(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "__internal.brand.view_to_bytes_preserve_brand_v1" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.brand.view_to_bytes_preserve_brand_v1 expects bytes_view"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_to_bytes(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "__internal.result_bytes.unwrap_ok_v1" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::ResultBytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.result_bytes.unwrap_ok_v1 expects result_bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("if ({}.tag == UINT32_C(1)) {{", args[0].c_name),
                        );
                        self.line(
                            state,
                            format!("{} = {}.payload.ok;", dest.c_name, args[0].c_name),
                        );
                        self.line(
                            state,
                            format!("{}.payload.ok = rt_bytes_empty(ctx);", args[0].c_name),
                        );
                        self.line(state, format!("{}.tag = UINT32_C(0);", args[0].c_name));
                        self.line(state, "} else {");
                        self.line(state, format!("{} = rt_bytes_empty(ctx);", dest.c_name));
                        self.line(state, "}");
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.as_ptr" => {
                        if args.len() != 1
                            || dest.ty != Ty::PtrConstU8
                            || args[0].ty != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.as_ptr expects bytes_view".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.ptr;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.eq" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.eq expects (bytes_view,bytes_view)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_eq(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.cmp_range" => {
                        if args.len() != 6
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                            || args[3].ty != Ty::BytesView
                            || args[4].ty != Ty::I32
                            || args[5].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.cmp_range expects (bytes_view,i32,i32,bytes_view,i32,i32)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_cmp_range(ctx, {}, {}, {}, {}, {}, {});",
                                dest.c_name,
                                args[0].c_name,
                                args[1].c_name,
                                args[2].c_name,
                                args[3].c_name,
                                args[4].c_name,
                                args[5].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "await" | "task.join.bytes" => {
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || (args[0].ty != Ty::TaskHandleBytesV1 && args[0].ty != Ty::I32)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes task handle and returns bytes"),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_task_join_bytes_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.join.result_bytes" => {
                        if args.len() != 1
                            || dest.ty != Ty::ResultBytes
                            || (args[0].ty != Ty::TaskHandleResultBytesV1 && args[0].ty != Ty::I32)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.join.result_bytes expects result_bytes task handle and returns result_bytes"
                                    .to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_task_join_result_bytes_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.try_join.bytes" => {
                        if args.len() != 1
                            || dest.ty != Ty::ResultBytes
                            || (args[0].ty != Ty::TaskHandleBytesV1 && args[0].ty != Ty::I32)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.try_join.bytes expects bytes task handle and returns result_bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_task_try_join_bytes(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.try_join.result_bytes" => {
                        if args.len() != 1
                            || dest.ty != Ty::ResultResultBytes
                            || (args[0].ty != Ty::TaskHandleResultBytesV1 && args[0].ty != Ty::I32)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.try_join.result_bytes expects result_bytes task handle and returns result_result_bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_task_try_join_result_bytes(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.spawn" => {
                        if args.len() != 1
                            || !ty_compat_task_handle_as_i32(args[0].ty, dest.ty)
                            || (args[0].ty != Ty::TaskHandleBytesV1
                                && args[0].ty != Ty::TaskHandleResultBytesV1
                                && args[0].ty != Ty::I32)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.spawn expects task handle and returns task handle"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_task_spawn(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.yield" => {
                        if !args.is_empty() || dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.yield expects 0 args".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, "rt_task_yield(ctx);");
                        self.line(state, format!("f->state = UINT32_C({cont});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.sleep" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.sleep expects i32".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("rt_task_sleep(ctx, {});", args[0].c_name));
                        self.line(state, format!("f->state = UINT32_C({cont});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.cancel" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::TaskHandleBytesV1
                                && args[0].ty != Ty::TaskHandleResultBytesV1
                                && args[0].ty != Ty::I32)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.cancel expects task handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_task_cancel(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.scope.start_soon_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::TaskHandleBytesV1
                                && args[0].ty != Ty::TaskHandleResultBytesV1)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.start_soon_v1 expects task handle and returns i32"
                                    .to_string(),
                            ));
                        }
                        let scope = self
                            .task_scopes
                            .last()
                            .cloned()
                            .ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "X07E_SCOPE_001: task.scope.start_soon_v1 used outside task.scope_v1"
                                        .to_string(),
                                )
                            })?;
                        let kind = match args[0].ty {
                            Ty::TaskHandleBytesV1 => "RT_TASK_OUT_KIND_BYTES",
                            Ty::TaskHandleResultBytesV1 => "RT_TASK_OUT_KIND_RESULT_BYTES",
                            _ => unreachable!(),
                        };
                        self.line(state, format!("rt_task_spawn(ctx, {});", args[0].c_name));
                        self.line(
                            state,
                            format!(
                                "{} = rt_scope_start_soon(ctx, &{}, {}, {});",
                                dest.c_name, scope.c_name, args[0].c_name, kind
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.scope.cancel_all_v1" => {
                        if !args.is_empty() || dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.cancel_all_v1 expects 0 args and returns i32"
                                    .to_string(),
                            ));
                        }
                        let scope = self
                            .task_scopes
                            .last()
                            .cloned()
                            .ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "X07E_SCOPE_001: task.scope.cancel_all_v1 used outside task.scope_v1"
                                        .to_string(),
                                )
                            })?;
                        self.line(
                            state,
                            format!(
                                "{} = rt_scope_cancel_all(ctx, &{});",
                                dest.c_name, scope.c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.scope.async_let_bytes_v1" | "task.scope.async_let_result_bytes_v1" => {
                        if args.len() != 1 || dest.ty != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects 1 arg and returns task_slot_v1"),
                            ));
                        }
                        let scope = self.task_scopes.last().cloned().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("X07E_SCOPE_SLOT_001: {head} used outside task.scope_v1"),
                            )
                        })?;
                        let (want, kind) = match head {
                            "task.scope.async_let_bytes_v1" => {
                                (Ty::TaskHandleBytesV1, "RT_TASK_OUT_KIND_BYTES")
                            }
                            "task.scope.async_let_result_bytes_v1" => {
                                (Ty::TaskHandleResultBytesV1, "RT_TASK_OUT_KIND_RESULT_BYTES")
                            }
                            _ => unreachable!(),
                        };
                        if args[0].ty != want {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects {want:?}"),
                            ));
                        }
                        self.line(state, format!("rt_task_spawn(ctx, {});", args[0].c_name));
                        self.line(
                            state,
                            format!(
                                "{} = rt_scope_async_let(ctx, &{}, {}, {});",
                                dest.c_name, scope.c_name, args[0].c_name, kind
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.scope.await_slot_bytes_v1" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.await_slot_bytes_v1 expects task_slot_v1 and returns bytes"
                                    .to_string(),
                            ));
                        }
                        let scope = self
                            .task_scopes
                            .last()
                            .cloned()
                            .ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "X07E_SCOPE_SLOT_002: task.scope.await_slot_bytes_v1 used outside task.scope_v1"
                                        .to_string(),
                                )
                            })?;
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_scope_await_slot_bytes_poll(ctx, &{}, {}, &{})) goto st_{cont};",
                                scope.c_name, args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.scope.await_slot_result_bytes_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::TaskSlotV1
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.await_slot_result_bytes_v1 expects task_slot_v1 and returns result_bytes"
                                    .to_string(),
                            ));
                        }
                        let scope = self
                            .task_scopes
                            .last()
                            .cloned()
                            .ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "X07E_SCOPE_SLOT_002: task.scope.await_slot_result_bytes_v1 used outside task.scope_v1"
                                        .to_string(),
                                )
                            })?;
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_scope_await_slot_result_bytes_poll(ctx, &{}, {}, &{})) goto st_{cont};",
                                scope.c_name, args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.scope.try_await_slot.bytes_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::TaskSlotV1
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.try_await_slot.bytes_v1 expects task_slot_v1 and returns result_bytes"
                                    .to_string(),
                            ));
                        }
                        let scope = self
                            .task_scopes
                            .last()
                            .cloned()
                            .ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "X07E_SCOPE_SLOT_002: task.scope.try_await_slot.bytes_v1 used outside task.scope_v1"
                                        .to_string(),
                                )
                            })?;
                        self.line(
                            state,
                            format!(
                                "{} = rt_scope_try_await_slot_bytes(ctx, &{}, {});",
                                dest.c_name, scope.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.scope.try_await_slot.result_bytes_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::ResultResultBytes
                            || args[0].ty != Ty::TaskSlotV1
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.try_await_slot.result_bytes_v1 expects task_slot_v1 and returns result_result_bytes"
                                    .to_string(),
                            ));
                        }
                        let scope = self
                            .task_scopes
                            .last()
                            .cloned()
                            .ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "X07E_SCOPE_SLOT_002: task.scope.try_await_slot.result_bytes_v1 used outside task.scope_v1"
                                        .to_string(),
                                )
                            })?;
                        self.line(
                            state,
                            format!(
                                "{} = rt_scope_try_await_slot_result_bytes(ctx, &{}, {});",
                                dest.c_name, scope.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.scope.slot_is_finished_v1" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.slot_is_finished_v1 expects task_slot_v1 and returns i32"
                                    .to_string(),
                            ));
                        }
                        let scope = self
                            .task_scopes
                            .last()
                            .cloned()
                            .ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "X07E_SCOPE_SLOT_002: task.scope.slot_is_finished_v1 used outside task.scope_v1"
                                        .to_string(),
                                )
                            })?;
                        self.line(
                            state,
                            format!(
                                "{} = rt_scope_slot_is_finished(ctx, &{}, {});",
                                dest.c_name, scope.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.scope.slot_to_i32_v1" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.slot_to_i32_v1 expects task_slot_v1 and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {};", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.scope.slot_from_i32_v1" => {
                        if args.len() != 1 || dest.ty != Ty::TaskSlotV1 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.slot_from_i32_v1 expects i32 and returns task_slot_v1"
                                    .to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {};", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.select_evt.tag_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::TaskSelectEvtV1
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.select_evt.tag_v1 expects task_select_evt_v1 and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_select_evt_tag(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.select_evt.case_index_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::TaskSelectEvtV1
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.select_evt.case_index_v1 expects task_select_evt_v1 and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_select_evt_case_index(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.select_evt.src_id_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::TaskSelectEvtV1
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.select_evt.src_id_v1 expects task_select_evt_v1 and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_select_evt_src_id(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.select_evt.take_bytes_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::TaskSelectEvtV1
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.select_evt.take_bytes_v1 expects task_select_evt_v1 and returns bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_select_evt_take_bytes(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.select_evt.drop_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::TaskSelectEvtV1
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.select_evt.drop_v1 expects task_select_evt_v1 and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("rt_select_evt_drop(ctx, {});", args[0].c_name),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", args[0].c_name));
                        self.line(state, format!("{} = UINT32_C(1);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "chan.bytes.new" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.new expects i32 cap".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_chan_bytes_new(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "chan.bytes.send" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.send expects (i32, bytes)".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_chan_bytes_send_poll(ctx, {}, {})) {{ {} = UINT32_C(1); {} = {}; goto st_{cont}; }}",
                                args[0].c_name, args[1].c_name, dest.c_name, args[1].c_name, c_empty(Ty::Bytes)
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "chan.bytes.recv" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.recv expects i32".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_chan_bytes_recv_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "chan.bytes.close" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.close expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_chan_bytes_close(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fs.read" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.read is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.read expects bytes_view path".to_string(),
                            ));
                        }
                        let path = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_fs_latency_ticks(ctx, {});", path),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_fs_read(ctx, {});", dest.c_name, path),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(
                            done,
                            format!("{} = rt_fs_read(ctx, {});", dest.c_name, path),
                        );
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fs.list_dir" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.list_dir is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.list_dir expects bytes_view path".to_string(),
                            ));
                        }
                        let path = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!("{} = rt_fs_list_dir(ctx, {});", dest.c_name, path),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fs.read_async" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.read_async is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.read_async expects bytes_view path".to_string(),
                            ));
                        }
                        let path = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_fs_latency_ticks(ctx, {});", path),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_fs_read(ctx, {});", dest.c_name, path),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(
                            done,
                            format!("{} = rt_fs_read(ctx, {});", dest.c_name, path),
                        );
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fs.open_read" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.open_read is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Iface
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.open_read expects bytes_view path".to_string(),
                            ));
                        }
                        let path = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = rt_fs_open_read(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
                                dest.c_name, path
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.read_file" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.read_file is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.read_file expects bytes path".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_fs_read_file(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.write_file" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.write_file is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.write_file expects (bytes path, bytes data)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_fs_write_file(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.read_all_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.read_all_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.read_all_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_read_all_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.write_all_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.write_all_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 3
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                            || args[2].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.write_all_v1 expects (bytes path, bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_write_all_v1({}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.append_all_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.append_all_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 3
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                            || args[2].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.append_all_v1 expects (bytes path, bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_append_all_v1({}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.stream_open_write_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.stream_open_write_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stream_open_write_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_stream_open_write_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.stream_write_all_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.stream_write_all_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stream_write_all_v1 expects (i32 writer_handle, bytes_view data)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_stream_write_all_v1((int32_t){}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.stream_close_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.stream_close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::ResultI32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stream_close_v1 expects i32 writer_handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_stream_close_v1((int32_t){});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.stream_drop_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.stream_drop_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stream_drop_v1 expects i32 writer_handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_stream_drop_v1((int32_t){});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.mkdirs_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.mkdirs_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.mkdirs_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_mkdirs_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.remove_file_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.remove_file_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.remove_file_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_remove_file_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.remove_dir_all_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.remove_dir_all_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.remove_dir_all_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_remove_dir_all_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.rename_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.rename_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 3
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                            || args[2].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.rename_v1 expects (bytes src, bytes dst, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_rename_v1({}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.list_dir_sorted_text_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.list_dir_sorted_text_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.list_dir_sorted_text_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_list_dir_sorted_text_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.walk_glob_sorted_text_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.walk_glob_sorted_text_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 3
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                            || args[2].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.walk_glob_sorted_text_v1 expects (bytes root, bytes glob, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_walk_glob_sorted_text_v1({}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.stat_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_FS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.stat_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stat_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_stat_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.stdio.read_line_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_STDIO,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.stdio.read_line_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::ResultBytes || args[0].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.stdio.read_line_v1 expects (bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_stdio_read_line_v1({});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.stdio.write_stdout_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_STDIO,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.stdio.write_stdout_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.stdio.write_stdout_v1 expects (bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_stdio_write_stdout_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.stdio.write_stderr_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_STDIO,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.stdio.write_stderr_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.stdio.write_stderr_v1 expects (bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_stdio_write_stderr_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.stdio.flush_stdout_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_STDIO,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.stdio.flush_stdout_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if !args.is_empty() || dest.ty != Ty::ResultI32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.stdio.flush_stdout_v1 expects 0 args".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = x07_ext_stdio_flush_stdout_v1();", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.stdio.flush_stderr_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_STDIO,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.stdio.flush_stderr_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if !args.is_empty() || dest.ty != Ty::ResultI32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.stdio.flush_stderr_v1 expects 0 args".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = x07_ext_stdio_flush_stderr_v1();", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.rand.bytes_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_RAND,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.rand.bytes_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.rand.bytes_v1 expects (i32 n, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_rand_bytes_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.rand.u64_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_RAND,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.rand.u64_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::ResultBytes || args[0].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.rand.u64_v1 expects (bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = x07_ext_rand_u64_v1({});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.sqlite.open_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_SQLITE,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.sqlite.open_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_sqlite_open_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.sqlite.query_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_SQLITE,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.sqlite.query_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_sqlite_query_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.sqlite.exec_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_SQLITE,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.sqlite.exec_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_sqlite_exec_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.sqlite.close_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_SQLITE,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.sqlite.close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_sqlite_close_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.pg.open_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_PG,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.pg.open_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_pg_open_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.pg.query_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_PG,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.pg.query_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_pg_query_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.pg.exec_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_PG,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.pg.exec_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_pg_exec_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.pg.close_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_PG,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.pg.close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_pg_close_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.mysql.open_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_MYSQL,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.mysql.open_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_mysql_open_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.mysql.query_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_MYSQL,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.mysql.query_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_mysql_query_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.mysql.exec_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_MYSQL,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.mysql.exec_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_mysql_exec_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.mysql.close_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_MYSQL,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.mysql.close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_mysql_close_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.redis.open_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_REDIS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.redis.open_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_redis_open_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.redis.cmd_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_REDIS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.redis.cmd_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.cmd_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_redis_cmd_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.redis.close_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_EXT_DB_REDIS,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.redis.close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_redis_close_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.env.get" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.env.get is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.env.get expects bytes key".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_os_env_get(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.now_unix_ms" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.time.now_unix_ms is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if !args.is_empty() || dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.now_unix_ms expects 0 args and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_os_time_now_unix_ms(ctx);", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.now_instant_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.time.now_instant_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if !args.is_empty() || dest.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.now_instant_v1 expects 0 args and returns bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_os_time_now_instant_v1(ctx);", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.sleep_ms_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.time.sleep_ms_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.sleep_ms_v1 expects i32 ms and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = (int32_t)rt_os_time_sleep_ms_v1(ctx, (int32_t){});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.local_tzid_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.time.local_tzid_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if !args.is_empty() || dest.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.local_tzid_v1 expects 0 args and returns bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_os_time_local_tzid_v1(ctx);", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.tzdb_is_valid_tzid_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_TIME,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_is_valid_tzid_v1 expects bytes_view tzid and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ev_time_tzdb_is_valid_tzid_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }});",
                                dest.c_name, args[0].c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.tzdb_offset_duration_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_TIME,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if args.len() != 3
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_offset_duration_v1 expects (bytes_view tzid, i32 unix_s_lo, i32 unix_s_hi) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ev_time_tzdb_offset_duration_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){}, (int32_t){});",
                                dest.c_name, args[0].c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.tzdb_snapshot_id_v1" => {
                        self.require_native_backend(
                            native::BACKEND_ID_TIME,
                            native::ABI_MAJOR_V1,
                            head,
                        )?;
                        if !args.is_empty() || dest.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_snapshot_id_v1 expects 0 args and returns bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ev_time_tzdb_snapshot_id_v1();", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "process.set_exit_code_v1" => {
                        if args.len() != 1 || args[0].ty != Ty::I32 || dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "process.set_exit_code_v1 expects (i32 code) and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("ctx->exit_code = (int32_t){};", args[0].c_name),
                        );
                        self.line(
                            state,
                            format!("{} = (int32_t){};", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.exit" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.exit is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.exit expects i32 code".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("rt_os_process_exit(ctx, (int32_t){});", args[0].c_name),
                        );
                        self.line(state, "__builtin_unreachable();");
                        return Ok(());
                    }
                    "os.process.spawn_capture_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.spawn_capture_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.spawn_capture_v1 expects (bytes req, bytes caps) and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_spawn_capture_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.spawn_piped_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.spawn_piped_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.spawn_piped_v1 expects (bytes req, bytes caps) and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_spawn_piped_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.try_join_capture_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.try_join_capture_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::OptionBytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.try_join_capture_v1 expects i32 and returns option_bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_try_join_capture_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.join_capture_v1" | "std.os.process.join_capture_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.join_capture_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.join_capture_v1 expects i32 and returns bytes"
                                    .to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_os_process_join_capture_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "os.process.stdout_read_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.stdout_read_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdout_read_v1 expects (i32 handle, i32 max) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_stdout_read_v1(ctx, {}, (int32_t){});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.stderr_read_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.stderr_read_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stderr_read_v1 expects (i32 handle, i32 max) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_stderr_read_v1(ctx, {}, (int32_t){});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.stdin_write_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.stdin_write_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdin_write_v1 expects (i32 handle, bytes chunk) and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_stdin_write_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.stdin_close_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.stdin_close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdin_close_v1 expects i32 and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_stdin_close_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.try_wait_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.try_wait_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.try_wait_v1 expects i32 and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_try_wait_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.join_exit_v1" | "std.os.process.join_exit_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.join_exit_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.join_exit_v1 expects i32 and returns i32".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_os_process_join_exit_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "os.process.take_exit_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.take_exit_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.take_exit_v1 expects i32 and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_take_exit_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.kill_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.kill_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.kill_v1 expects (i32 proc_handle, i32 sig) and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_kill_v1(ctx, {}, (int32_t){});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.drop_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.drop_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.drop_v1 expects i32 proc handle and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_drop_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.run_capture_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.run_capture_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.run_capture_v1 expects (bytes req, bytes caps) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_run_capture_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.net.http_request" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.net.http_request is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.net.http_request expects bytes req".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_net_http_request(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "rr.send_request" => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Unsupported,
                            "rr.send_request has been removed; use std.rr.with_policy_v1 + std.rr.next_v1 / std.rr.append_v1".to_string(),
                        ));
                    }
                    "rr.fetch" => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Unsupported,
                            "rr.fetch has been removed; use std.rr.with_policy_v1 + std.rr.next_v1 + std.rr.entry_resp_v1".to_string(),
                        ));
                    }
                    "rr.send" => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Unsupported,
                            "rr.send has been removed; use std.stream.src.rr_send_v1 inside std.stream.pipe_v1".to_string(),
                        ));
                    }
                    "kv.get" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get expects bytes_view key".to_string(),
                            ));
                        }
                        let key = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_kv_latency_ticks(ctx, {});", key),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_kv_get(ctx, {});", dest.c_name, key),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(done, format!("{} = rt_kv_get(ctx, {});", dest.c_name, key));
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "kv.get_async" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get_async is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get_async expects bytes_view key".to_string(),
                            ));
                        }
                        let key = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_kv_latency_ticks(ctx, {});", key),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_kv_get(ctx, {});", dest.c_name, key),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(done, format!("{} = rt_kv_get(ctx, {});", dest.c_name, key));
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "kv.get_stream" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get_stream is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Iface
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get_stream expects bytes_view key".to_string(),
                            ));
                        }
                        let key = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = rt_kv_get_stream(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
                                dest.c_name, key
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "kv.set" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.set is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.set expects (bytes,bytes)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_kv_set(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(
                            state,
                            format!("{} = {};", args[0].c_name, c_empty(Ty::Bytes)),
                        );
                        self.line(
                            state,
                            format!("{} = {};", args[1].c_name, c_empty(Ty::Bytes)),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "io.open_read_bytes" => {
                        if args.len() != 1
                            || dest.ty != Ty::Iface
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "io.open_read_bytes expects bytes".to_string(),
                            ));
                        }
                        let b = match args[0].ty {
                            Ty::Bytes => args[0].c_name.clone(),
                            Ty::BytesView => format!("rt_view_to_bytes(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_into_bytes(ctx, &{})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = rt_io_reader_new_bytes(ctx, {}, UINT32_C(0)), .vtable = RT_IFACE_VTABLE_IO_READER }};",
                                dest.c_name, b
                            ),
                        );
                        if args[0].ty == Ty::Bytes {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "io.read" => {
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Iface
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "io.read expects (iface, i32)".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if ({}.vtable != RT_IFACE_VTABLE_IO_READER) {{ {} = rt_iface_io_read_block(ctx, {}, {}); goto st_{cont}; }}",
                                args[0].c_name,
                                dest.c_name,
                                args[0].c_name,
                                args[1].c_name,
                            ),
                        );
                        self.line(
                            state,
                            format!(
                                "if (rt_io_read_poll(ctx, {}.data, {}, &{})) goto st_{cont};",
                                args[0].c_name, args[1].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "iface.make_v1" => {
                        if args.len() != 2
                            || dest.ty != Ty::Iface
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "iface.make_v1 expects (i32, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = {}, .vtable = {} }};",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bufread.new" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Iface
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.new expects (iface, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_bufread_new(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bufread.fill" => {
                        if args.len() != 1 || dest.ty != Ty::BytesView || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.fill expects i32".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_bufread_fill_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "bufread.consume" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.consume expects (i32, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_bufread_consume(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "scratch_u8_fixed_v1.new" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.new expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_scratch_u8_fixed_new(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "scratch_u8_fixed_v1.clear" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.clear expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_scratch_u8_fixed_clear(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "scratch_u8_fixed_v1.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.len expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_scratch_u8_fixed_len(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "scratch_u8_fixed_v1.cap" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.cap expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_scratch_u8_fixed_cap(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "scratch_u8_fixed_v1.as_view" => {
                        if args.len() != 1 || dest.ty != Ty::BytesView || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.as_view expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_scratch_u8_fixed_as_view(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "scratch_u8_fixed_v1.try_write" => {
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::I32
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.try_write expects (i32, bytes_view)"
                                    .to_string(),
                            ));
                        }
                        let b = if args[1].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[1].c_name)
                        } else {
                            args[1].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_scratch_u8_fixed_try_write(ctx, {}, {});",
                                dest.c_name, args[0].c_name, b
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "scratch_u8_fixed_v1.drop" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.drop expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_scratch_u8_fixed_drop(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "codec.read_u32_le" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "codec.read_u32_le expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        let b = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_codec_read_u32_le(ctx, {}, {});",
                                dest.c_name, b, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "codec.write_u32_le" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "codec.write_u32_le expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_codec_write_u32_le(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fmt.u32_to_dec" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fmt.u32_to_dec expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_fmt_u32_to_dec(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fmt.s32_to_dec" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fmt.s32_to_dec expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_fmt_s32_to_dec(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "parse.u32_dec" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "parse.u32_dec expects bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_parse_u32_dec(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "parse.u32_dec_at" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "parse.u32_dec_at expects (bytes,i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_parse_u32_dec_at(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "prng.lcg_next_u32" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "prng.lcg_next_u32 expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_prng_lcg_next_u32(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.with_capacity" => {
                        if args.len() != 1 || dest.ty != Ty::VecU8 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.with_capacity expects i32 cap".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_vec_u8_new(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.len expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_vec_u8_len(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.cap" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.cap expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_vec_u8_cap(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.clear" => {
                        if args.len() != 1 || dest.ty != Ty::VecU8 || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.clear expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_clear(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.get" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.get expects (vec_u8, idx)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_get(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.set" => {
                        if args.len() != 3
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.set expects (vec_u8, idx, val)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_set(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.push" => {
                        if args.len() != 2
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.push expects (vec_u8, val)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_push(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.reserve_exact" => {
                        if args.len() != 2
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.reserve_exact expects (vec_u8, additional)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_reserve_exact(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.extend_zeroes" => {
                        if args.len() != 2
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_extend_zeroes(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.extend_bytes" => {
                        if args.len() != 2
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
                            ));
                        }
                        let b = if args[1].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[1].c_name)
                        } else {
                            args[1].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_extend_bytes(ctx, {}, {});",
                                dest.c_name, args[0].c_name, b
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.extend_bytes_range" => {
                        if args.len() != 4
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                            || args[2].ty != Ty::I32
                            || args[3].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, off, len)"
                                    .to_string(),
                            ));
                        }
                        let b = if args[1].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[1].c_name)
                        } else {
                            args[1].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_extend_bytes_range(ctx, {}, {}, {}, {});",
                                dest.c_name, args[0].c_name, b, args[2].c_name, args[3].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.into_bytes" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.into_bytes expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_into_bytes(ctx, &{});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.as_view" => {
                        if args.len() != 1 || dest.ty != Ty::BytesView || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.as_view expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_as_view(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.as_ptr" => {
                        if args.len() != 1 || dest.ty != Ty::PtrConstU8 || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.as_ptr expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ({}).data;", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.as_mut_ptr" => {
                        if args.len() != 1 || dest.ty != Ty::PtrMutU8 || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.as_mut_ptr expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ({}).data;", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.null" => {
                        if !args.is_empty() || dest.ty != Ty::PtrMutVoid {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.null expects 0 args".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = NULL;", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.as_const" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.as_const expects 1 arg".to_string(),
                            ));
                        }
                        let ok = matches!(
                            (args[0].ty, dest.ty),
                            (Ty::PtrMutU8, Ty::PtrConstU8)
                                | (Ty::PtrConstU8, Ty::PtrConstU8)
                                | (Ty::PtrMutVoid, Ty::PtrConstVoid)
                                | (Ty::PtrConstVoid, Ty::PtrConstVoid)
                                | (Ty::PtrMutI32, Ty::PtrConstI32)
                                | (Ty::PtrConstI32, Ty::PtrConstI32)
                        );
                        if !ok {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.as_const expects a raw pointer and returns a const raw pointer"
                                    .to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {};", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.add" | "ptr.sub" => {
                        if args.len() != 2 || args[0].ty != dest.ty || args[1].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (ptr, i32)"),
                            ));
                        }
                        let op = if head == "ptr.add" { "+" } else { "-" };
                        self.line(
                            state,
                            format!(
                                "{} = {} {op} (size_t){};",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.offset" => {
                        if args.len() != 2 || args[0].ty != dest.ty || args[1].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.offset expects (ptr, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = {} + (int32_t){};",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.read_u8" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || !matches!(args[0].ty, Ty::PtrConstU8 | Ty::PtrMutU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.read_u8 expects (ptr_const_u8|ptr_mut_u8)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = (uint32_t)(*{});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.write_u8" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::PtrMutU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.write_u8 expects (ptr_mut_u8, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("*{} = (uint8_t){};", args[0].c_name, args[1].c_name),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.read_i32" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || !matches!(args[0].ty, Ty::PtrConstI32 | Ty::PtrMutI32)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.read_i32 expects (ptr_const_i32|ptr_mut_i32)".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = *{};", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.write_i32" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::PtrMutI32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.write_i32 expects (ptr_mut_i32, i32)".to_string(),
                            ));
                        }
                        self.line(state, format!("*{} = {};", args[0].c_name, args[1].c_name));
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "memcpy" | "memmove" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::PtrMutVoid
                            || !matches!(args[1].ty, Ty::PtrConstVoid | Ty::PtrMutVoid)
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (ptr_mut_void, ptr_const_void, i32)"),
                            ));
                        }
                        self.line(state, format!("rt_mem_on_memcpy(ctx, {});", args[2].c_name));
                        self.line(
                            state,
                            format!(
                                "(void){head}((void*){}, (const void*){}, (size_t){});",
                                args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "memset" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::PtrMutVoid
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "memset expects (ptr_mut_void, i32, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "(void)memset((void*){}, (int){}, (size_t){});",
                                args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_value.with_capacity_v1" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.with_capacity_v1 expects (i32 ty_id, i32 cap)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_value_with_capacity_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_value.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.len expects i32 handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_value_len(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_value.reserve_exact" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.reserve_exact expects (i32 handle, i32 additional)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_value_reserve_exact(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_value.pop" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.pop expects i32 handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_value_pop(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_value.clear" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.clear expects i32 handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_value_clear(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    h if h.starts_with("vec_value.push_") => {
                        if args.len() != 2 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, x)"),
                            ));
                        }
                        let Some(suffix) = parse_value_suffix_single(head, "vec_value.push_")
                        else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_x_ty = value_suffix_ty(suffix)
                            .expect("suffix validated by parse_value_suffix");
                        if args[1].ty != want_x_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects x ({want_x_ty:?})"),
                            ));
                        }
                        let rt_fn = format!("rt_{}", head.replace('.', "_"));
                        self.line(
                            state,
                            format!(
                                "{} = {rt_fn}(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if want_x_ty == Ty::Bytes && dest.c_name != args[1].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[1].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    h if h.starts_with("vec_value.get_") => {
                        if args.len() != 3 || args[0].ty != Ty::I32 || args[1].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, i32 idx, default)"),
                            ));
                        }
                        let Some(suffix) = parse_value_suffix_single(head, "vec_value.get_") else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_out_ty = value_suffix_ty(suffix)
                            .expect("suffix validated by parse_value_suffix");
                        if dest.ty != want_out_ty || args[2].ty != want_out_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects default ({want_out_ty:?})"),
                            ));
                        }
                        let rt_fn = format!("rt_{}", head.replace('.', "_"));
                        self.line(
                            state,
                            format!(
                                "{} = {rt_fn}(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        if want_out_ty == Ty::Bytes && dest.c_name != args[2].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[2].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    h if h.starts_with("vec_value.set_") => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, i32 idx, x)"),
                            ));
                        }
                        let Some(suffix) = parse_value_suffix_single(head, "vec_value.set_") else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_x_ty = value_suffix_ty(suffix)
                            .expect("suffix validated by parse_value_suffix");
                        if args[2].ty != want_x_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects x ({want_x_ty:?})"),
                            ));
                        }
                        let rt_fn = format!("rt_{}", head.replace('.', "_"));
                        self.line(
                            state,
                            format!(
                                "{} = {rt_fn}(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        if want_x_ty == Ty::Bytes && dest.c_name != args[2].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[2].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }

                    "map_value.new_v1" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_value.new_v1 expects (i32 k_id, i32 v_id, i32 cap_pow2)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_value_new_v1(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_value.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_value.len expects i32 handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_value_len(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_value.clear" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_value.clear expects i32 handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_value_clear(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    h if h.starts_with("map_value.contains_") => {
                        if args.len() != 2 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, key)"),
                            ));
                        }
                        let Some(suffix) = parse_value_suffix_single(head, "map_value.contains_")
                        else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_k_ty = value_suffix_ty(suffix)
                            .expect("suffix validated by parse_value_suffix");
                        if args[1].ty != want_k_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects key ({want_k_ty:?})"),
                            ));
                        }
                        let rt_fn = format!("rt_{}", head.replace('.', "_"));
                        self.line(
                            state,
                            format!(
                                "{} = {rt_fn}(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if want_k_ty == Ty::Bytes && dest.c_name != args[1].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[1].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    h if h.starts_with("map_value.remove_") => {
                        if args.len() != 2 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, key)"),
                            ));
                        }
                        let Some(suffix) = parse_value_suffix_single(head, "map_value.remove_")
                        else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_k_ty = value_suffix_ty(suffix)
                            .expect("suffix validated by parse_value_suffix");
                        if args[1].ty != want_k_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects key ({want_k_ty:?})"),
                            ));
                        }
                        let rt_fn = format!("rt_{}", head.replace('.', "_"));
                        self.line(
                            state,
                            format!(
                                "{} = {rt_fn}(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if want_k_ty == Ty::Bytes && dest.c_name != args[1].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[1].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    h if h.starts_with("map_value.get_") => {
                        if args.len() != 3 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, key, default)"),
                            ));
                        }
                        let Some((k_suffix, v_suffix)) =
                            parse_value_suffix_pair(head, "map_value.get_")
                        else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_k_ty = value_suffix_ty(k_suffix)
                            .expect("suffix validated by parse_value_suffix");
                        let want_v_ty = value_suffix_ty(v_suffix)
                            .expect("suffix validated by parse_value_suffix");
                        if args[1].ty != want_k_ty
                            || args[2].ty != want_v_ty
                            || dest.ty != want_v_ty
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, {want_k_ty:?} key, {want_v_ty:?} default)"),
                            ));
                        }
                        let rt_fn = format!("rt_{}", head.replace('.', "_"));
                        self.line(
                            state,
                            format!(
                                "{} = {rt_fn}(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        if want_k_ty == Ty::Bytes && dest.c_name != args[1].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[1].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        if want_v_ty == Ty::Bytes && dest.c_name != args[2].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[2].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    h if h.starts_with("map_value.set_") => {
                        if args.len() != 3 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, key, val)"),
                            ));
                        }
                        let Some((k_suffix, v_suffix)) =
                            parse_value_suffix_pair(head, "map_value.set_")
                        else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_k_ty = value_suffix_ty(k_suffix)
                            .expect("suffix validated by parse_value_suffix");
                        let want_v_ty = value_suffix_ty(v_suffix)
                            .expect("suffix validated by parse_value_suffix");
                        if args[1].ty != want_k_ty || args[2].ty != want_v_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (i32 handle, {want_k_ty:?} key, {want_v_ty:?} val)"),
                            ));
                        }
                        let rt_fn = format!("rt_{}", head.replace('.', "_"));
                        self.line(
                            state,
                            format!(
                                "{} = {rt_fn}(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        if want_k_ty == Ty::Bytes && dest.c_name != args[1].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[1].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        if want_v_ty == Ty::Bytes && dest.c_name != args[2].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[2].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.new" | "set_u32.new" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.new expects i32 cap".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_map_u32_new(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.len expects i32 handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_map_u32_len(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.get" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.get expects (handle,key,default)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_get(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.set" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                            || !(args[2].ty == Ty::I32 || is_task_handle_ty(args[2].ty))
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.set expects (handle,key,val)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_set(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.contains" | "set_u32.contains" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.contains expects (handle,key)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_contains(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.remove" | "set_u32.remove" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.remove expects (handle,key)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_remove(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "set_u32.add" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "set_u32.add expects (handle,key)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_set(ctx, {}, {}, UINT32_C(1));",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "set_u32.dump_u32le" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "set_u32.dump_u32le expects (handle)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_set_u32_dump_u32le(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.dump_kv_u32le_u32le" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.dump_kv_u32le_u32le expects (handle)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_dump_kv_u32le_u32le(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "result_bytes.ok" => {
                        if args.len() != 1 || dest.ty != Ty::ResultBytes || args[0].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.ok expects (bytes)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = (result_bytes_t){{ .tag = UINT32_C(1), .payload.ok = {} }};",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("{} = rt_bytes_empty(ctx);", args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "result_bytes.err" => {
                        if args.len() != 1 || dest.ty != Ty::ResultBytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.err expects (i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {} }};",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "result_bytes.is_ok" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.is_ok expects (result_bytes)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ({}.tag == UINT32_C(1));", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "result_bytes.err_code" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.err_code expects (result_bytes)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
                                dest.c_name, args[0].c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "result_bytes.unwrap_or" => {
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::ResultBytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.unwrap_or expects (result_bytes, bytes default)"
                                    .to_string(),
                            ));
                        }
                        let res = &args[0];
                        let default = &args[1];
                        self.line(state, format!("if ({}.tag == UINT32_C(1)) {{", res.c_name));
                        self.line(
                            state,
                            format!("  {} = {}.payload.ok;", dest.c_name, res.c_name),
                        );
                        self.line(
                            state,
                            format!("  {}.payload.ok = rt_bytes_empty(ctx);", res.c_name),
                        );
                        self.line(state, format!("  {}.tag = UINT32_C(0);", res.c_name));
                        self.line(state, "} else {");
                        if dest.c_name != default.c_name {
                            self.line(state, format!("  {} = {};", dest.c_name, default.c_name));
                            self.line(
                                state,
                                format!("  {} = rt_bytes_empty(ctx);", default.c_name),
                            );
                        }
                        self.line(state, "}");
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "result_result_bytes.is_ok" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::ResultResultBytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.is_ok expects (result_result_bytes)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ({}.tag == UINT32_C(1));", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "result_result_bytes.err_code" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::ResultResultBytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.err_code expects (result_result_bytes)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
                                dest.c_name, args[0].c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "result_result_bytes.unwrap_or" => {
                        if args.len() != 2
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::ResultResultBytes
                            || args[1].ty != Ty::ResultBytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.unwrap_or expects (result_result_bytes, result_bytes default)"
                                    .to_string(),
                            ));
                        }
                        let res = &args[0];
                        let default = &args[1];
                        self.line(state, format!("if ({}.tag == UINT32_C(1)) {{", res.c_name));
                        self.line(
                            state,
                            format!("  {} = {}.payload.ok;", dest.c_name, res.c_name),
                        );
                        self.line(
                            state,
                            format!(
                                "  {}.payload.ok = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = UINT32_C(0) }};",
                                res.c_name
                            ),
                        );
                        self.line(state, format!("  {}.tag = UINT32_C(0);", res.c_name));
                        self.line(
                            state,
                            format!("  {}.payload.err = UINT32_C(0);", res.c_name),
                        );
                        self.line(state, "} else {");
                        if dest.c_name != default.c_name {
                            self.line(state, format!("  {} = {};", dest.c_name, default.c_name));
                            self.line(
                                state,
                                format!("  {}.payload.ok = rt_bytes_empty(ctx);", default.c_name),
                            );
                            self.line(state, format!("  {}.tag = UINT32_C(0);", default.c_name));
                            self.line(
                                state,
                                format!("  {}.payload.err = UINT32_C(0);", default.c_name),
                            );
                        }
                        self.line(state, "}");
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    _ if self.extern_functions.contains_key(head) => {
                        let f = self.extern_functions.get(head).cloned().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Internal,
                                format!("internal error: missing extern decl for {head:?}"),
                            )
                        })?;

                        if args.len() != f.params.len() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("call {head:?} expects {} args", f.params.len()),
                            ));
                        }
                        if dest.ty != f.ret_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("call {head:?} returns {:?}", f.ret_ty),
                            ));
                        }

                        let mut rendered_args = Vec::with_capacity(args.len());
                        for (i, (arg, p)) in args.iter().zip(f.params.iter()).enumerate() {
                            let got = arg.ty;
                            let want = p.ty;
                            let ok = got == want
                                || matches!(
                                    (got, want),
                                    (Ty::PtrMutU8, Ty::PtrConstU8)
                                        | (Ty::PtrMutVoid, Ty::PtrConstVoid)
                                        | (Ty::PtrMutI32, Ty::PtrConstI32)
                                );
                            if !ok {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("call {head:?} arg {i} expects {want:?}"),
                                ));
                            }
                            rendered_args.push(arg.c_name.clone());
                        }
                        let c_args = rendered_args.join(", ");
                        if f.ret_is_void {
                            self.line(state, format!("{}({c_args});", f.link_name));
                            self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        } else {
                            self.line(
                                state,
                                format!("{} = {}({c_args});", dest.c_name, f.link_name),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    _ if self.fn_c_names.contains_key(head) => {
                        let cfn = self.fn_c_names.get(head).cloned().unwrap_or_default();
                        let rendered = args.iter().map(|a| a.c_name.clone()).collect::<Vec<_>>();
                        let c_args = if rendered.is_empty() {
                            String::new()
                        } else {
                            format!(", {}", rendered.join(", "))
                        };
                        self.line(
                            state,
                            format!("{} = {}(ctx, f->input{});", dest.c_name, cfn, c_args),
                        );
                        for a in args {
                            if is_owned_ty(a.ty) {
                                self.line(state, format!("{} = {};", a.c_name, c_empty(a.ty)));
                            }
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    _ if self.async_fn_new_names.contains_key(head) => {
                        let cfn = self
                            .async_fn_new_names
                            .get(head)
                            .cloned()
                            .unwrap_or_default();
                        let rendered = args.iter().map(|a| a.c_name.clone()).collect::<Vec<_>>();
                        let c_args = if rendered.is_empty() {
                            String::new()
                        } else {
                            format!(", {}", rendered.join(", "))
                        };
                        self.line(
                            state,
                            format!("{} = {}(ctx, f->input{});", dest.c_name, cfn, c_args),
                        );
                        for a in args {
                            if is_owned_ty(a.ty) {
                                self.line(state, format!("{} = {};", a.c_name, c_empty(a.ty)));
                            }
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    _ => {}
                }

                Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!("unsupported head in async: {head:?}"),
                ))
            }

            fn emit_unsafe(
                &mut self,
                state: usize,
                exprs: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if !self.options.allow_unsafe() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        "unsafe is not allowed in this world".to_string(),
                    ));
                }
                if exprs.is_empty() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "(unsafe ...) requires at least 1 expression".to_string(),
                    ));
                }
                let prev = self.unsafe_depth;
                self.unsafe_depth = self.unsafe_depth.saturating_add(1);
                let res = self.emit_begin(state, exprs, dest, cont);
                self.unsafe_depth = prev;
                res
            }

            fn emit_begin(
                &mut self,
                state: usize,
                exprs: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if exprs.is_empty() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "(begin ...) requires at least 1 expression".to_string(),
                    ));
                }
                self.line(state, "rt_fuel(ctx, 1);");
                self.push_scope();

                let mut states = Vec::with_capacity(exprs.len());
                for _ in exprs {
                    states.push(self.new_state());
                }

                self.line(state, format!("goto st_{};", states[0]));

                for (i, e) in exprs.iter().enumerate() {
                    let s = states[i];
                    let next = if i + 1 < exprs.len() {
                        states[i + 1]
                    } else {
                        cont
                    };
                    if i + 1 == exprs.len() {
                        self.emit_expr_entry(s, e, dest.clone(), next)?;
                    } else {
                        let ty = self.infer_expr(e)?;
                        let storage_ty = Self::storage_ty_for(ty.ty);
                        let tmp = self.alloc_local("t_begin_", storage_ty)?;
                        self.emit_expr_entry(s, e, tmp, next)?;
                    }
                }

                self.pop_scope();
                Ok(())
            }

            fn emit_let(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "let form: (let <name> <expr>)".to_string(),
                    ));
                }
                let name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "let name must be an identifier".to_string(),
                    )
                })?;
                let expr_ty = self.infer_expr(&args[1])?;
                let dest_ty = TyInfo {
                    ty: dest.ty,
                    brand: dest.brand.clone(),
                    view_full: false,
                };
                if expr_ty != Ty::Never && !tyinfo_compat_assign(&expr_ty, &dest_ty) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("let expression must match context type {:?}", dest.ty),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                let storage_ty = Self::storage_ty_for(expr_ty.ty);
                let binding = self.alloc_local("v_", storage_ty)?;
                if is_owned_ty(storage_ty) {
                    match storage_ty {
                        Ty::Bytes => {
                            self.line(state, format!("rt_bytes_drop(ctx, &{});", binding.c_name))
                        }
                        Ty::VecU8 => {
                            self.line(state, format!("rt_vec_u8_drop(ctx, &{});", binding.c_name))
                        }
                        Ty::OptionBytes => {
                            self.line(state, format!("if ({}.tag) {{", binding.c_name));
                            self.line(
                                state,
                                format!("  rt_bytes_drop(ctx, &{}.payload);", binding.c_name),
                            );
                            self.line(state, "}");
                            self.line(state, format!("{}.tag = UINT32_C(0);", binding.c_name));
                        }
                        Ty::ResultBytes => {
                            self.line(state, format!("if ({}.tag) {{", binding.c_name));
                            self.line(
                                state,
                                format!("  rt_bytes_drop(ctx, &{}.payload.ok);", binding.c_name),
                            );
                            self.line(state, "}");
                            self.line(state, format!("{}.tag = UINT32_C(0);", binding.c_name));
                            self.line(
                                state,
                                format!("{}.payload.err = UINT32_C(0);", binding.c_name),
                            );
                        }
                        Ty::ResultResultBytes => {
                            self.line(state, format!("if ({}.tag) {{", binding.c_name));
                            self.line(
                                state,
                                format!("  if ({}.payload.ok.tag) {{", binding.c_name),
                            );
                            self.line(
                                state,
                                format!(
                                    "    rt_bytes_drop(ctx, &{}.payload.ok.payload.ok);",
                                    binding.c_name
                                ),
                            );
                            self.line(state, "  }");
                            self.line(
                                state,
                                format!("  {}.payload.ok.tag = UINT32_C(0);", binding.c_name),
                            );
                            self.line(
                                state,
                                format!(
                                    "  {}.payload.ok.payload.err = UINT32_C(0);",
                                    binding.c_name
                                ),
                            );
                            self.line(state, "}");
                            self.line(state, format!("{}.tag = UINT32_C(0);", binding.c_name));
                            self.line(
                                state,
                                format!("{}.payload.err = UINT32_C(0);", binding.c_name),
                            );
                        }
                        _ => {}
                    }
                }

                let expr_state = self.new_state();
                let after = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));
                self.emit_expr_entry(expr_state, &args[1], binding.clone(), after)?;

                self.bind(
                    name.to_string(),
                    AsyncVarRef {
                        ty: expr_ty.ty,
                        brand: expr_ty.brand.clone(),
                        c_name: binding.c_name.clone(),
                        moved: false,
                        moved_ptr: None,
                    },
                );

                if binding.c_name != dest.c_name && expr_ty != Ty::Never {
                    if is_owned_ty(expr_ty.ty) {
                        self.line(after, format!("{} = {};", dest.c_name, c_empty(expr_ty.ty)));
                    } else {
                        self.line(after, format!("{} = {};", dest.c_name, binding.c_name));
                    }
                }
                self.line(after, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_set(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "set form: (set <name> <expr>)".to_string(),
                    ));
                }
                let name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "set name must be an identifier".to_string(),
                    )
                })?;
                let Some(var) = self.lookup(name) else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("set of unknown variable: {name:?}"),
                    ));
                };
                if var.ty != dest.ty && var.ty != Ty::Never {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("set expression must match context type {:?}", dest.ty),
                    ));
                }
                let expr_ty = self.infer_expr(&args[1])?;
                let want = TyInfo {
                    ty: var.ty,
                    brand: var.brand.clone(),
                    view_full: false,
                };
                if expr_ty != Ty::Never && !tyinfo_compat_assign(&expr_ty, &want) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("type mismatch in set for variable {name:?}"),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                let tmp = self.alloc_local("t_set_", var.ty)?;
                let expr_state = self.new_state();
                let after = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));
                self.emit_expr_entry(expr_state, &args[1], tmp.clone(), after)?;
                if is_owned_ty(var.ty) {
                    match var.ty {
                        Ty::Bytes => {
                            self.line(after, format!("rt_bytes_drop(ctx, &{});", var.c_name))
                        }
                        Ty::VecU8 => {
                            self.line(after, format!("rt_vec_u8_drop(ctx, &{});", var.c_name))
                        }
                        Ty::OptionBytes => {
                            self.line(after, format!("if ({}.tag) {{", var.c_name));
                            self.line(
                                after,
                                format!("  rt_bytes_drop(ctx, &{}.payload);", var.c_name),
                            );
                            self.line(after, "}");
                            self.line(after, format!("{}.tag = UINT32_C(0);", var.c_name));
                        }
                        Ty::ResultBytes => {
                            self.line(after, format!("if ({}.tag) {{", var.c_name));
                            self.line(
                                after,
                                format!("  rt_bytes_drop(ctx, &{}.payload.ok);", var.c_name),
                            );
                            self.line(after, "}");
                            self.line(after, format!("{}.tag = UINT32_C(0);", var.c_name));
                            self.line(after, format!("{}.payload.err = UINT32_C(0);", var.c_name));
                        }
                        _ => {}
                    }
                }
                self.line(after, format!("{} = {};", var.c_name, tmp.c_name));
                if is_owned_ty(var.ty) {
                    self.line(after, format!("{} = {};", tmp.c_name, c_empty(var.ty)));
                }
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = false;
                    v.moved_ptr = None;
                }
                if var.c_name != dest.c_name && var.ty != Ty::Never {
                    if is_owned_ty(var.ty) {
                        self.line(after, format!("{} = {};", dest.c_name, c_empty(var.ty)));
                    } else {
                        self.line(after, format!("{} = {};", dest.c_name, var.c_name));
                    }
                }
                self.line(after, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_set0(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "set0 form: (set0 <name> <expr>)".to_string(),
                    ));
                }
                if dest.ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("set0 expression must match context type {:?}", dest.ty),
                    ));
                }
                let name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "set0 name must be an identifier".to_string(),
                    )
                })?;
                let Some(var) = self.lookup(name) else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("set0 of unknown variable: {name:?}"),
                    ));
                };
                let expr_ty = self.infer_expr(&args[1])?;
                let want = TyInfo {
                    ty: var.ty,
                    brand: var.brand.clone(),
                    view_full: false,
                };
                if expr_ty != Ty::Never && !tyinfo_compat_assign(&expr_ty, &want) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("type mismatch in set0 for variable {name:?}"),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                let tmp = self.alloc_local("t_set0_", var.ty)?;
                let expr_state = self.new_state();
                let after = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));
                self.emit_expr_entry(expr_state, &args[1], tmp.clone(), after)?;
                if is_owned_ty(var.ty) {
                    match var.ty {
                        Ty::Bytes => {
                            self.line(after, format!("rt_bytes_drop(ctx, &{});", var.c_name))
                        }
                        Ty::VecU8 => {
                            self.line(after, format!("rt_vec_u8_drop(ctx, &{});", var.c_name))
                        }
                        Ty::OptionBytes => {
                            self.line(after, format!("if ({}.tag) {{", var.c_name));
                            self.line(
                                after,
                                format!("  rt_bytes_drop(ctx, &{}.payload);", var.c_name),
                            );
                            self.line(after, "}");
                            self.line(after, format!("{}.tag = UINT32_C(0);", var.c_name));
                        }
                        Ty::ResultBytes => {
                            self.line(after, format!("if ({}.tag) {{", var.c_name));
                            self.line(
                                after,
                                format!("  rt_bytes_drop(ctx, &{}.payload.ok);", var.c_name),
                            );
                            self.line(after, "}");
                            self.line(after, format!("{}.tag = UINT32_C(0);", var.c_name));
                            self.line(after, format!("{}.payload.err = UINT32_C(0);", var.c_name));
                        }
                        _ => {}
                    }
                }
                self.line(after, format!("{} = {};", var.c_name, tmp.c_name));
                if is_owned_ty(var.ty) {
                    self.line(after, format!("{} = {};", tmp.c_name, c_empty(var.ty)));
                }
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = false;
                    v.moved_ptr = None;
                }
                self.line(after, format!("{} = UINT32_C(0);", dest.c_name));
                self.line(after, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_if(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 3 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                    ));
                }
                self.line(state, "rt_fuel(ctx, 1);");
                let cond_tmp = self.alloc_local("t_if_", Ty::I32)?;
                let cond_state = self.new_state();
                let branch_state = self.new_state();
                let then_state = self.new_state();
                let else_state = self.new_state();

                self.line(state, format!("goto st_{cond_state};"));
                self.emit_expr_entry(cond_state, &args[0], cond_tmp.clone(), branch_state)?;

                self.line(
                    branch_state,
                    format!(
                        "if ({} != UINT32_C(0)) goto st_{then_state}; else goto st_{else_state};",
                        cond_tmp.c_name
                    ),
                );

                let then_ty = self.infer_expr(&args[1])?;
                let else_ty = self.infer_expr(&args[2])?;
                let scopes_before = self.scopes.clone();

                self.scopes = scopes_before.clone();
                self.push_scope();
                self.emit_expr_entry(then_state, &args[1], dest.clone(), cont)?;
                self.pop_scope();
                let scopes_then = self.scopes.clone();

                self.scopes = scopes_before.clone();
                self.push_scope();
                self.emit_expr_entry(else_state, &args[2], dest, cont)?;
                self.pop_scope();
                let scopes_else = self.scopes.clone();

                self.scopes = if then_ty.ty == Ty::Never && else_ty.ty == Ty::Never {
                    scopes_before
                } else if then_ty.ty == Ty::Never {
                    scopes_else
                } else if else_ty.ty == Ty::Never {
                    scopes_then
                } else {
                    self.merge_if_states(&scopes_before, &scopes_then, &scopes_else)?
                };

                Ok(())
            }

            fn merge_if_states(
                &self,
                before: &[BTreeMap<String, AsyncVarRef>],
                then_state: &[BTreeMap<String, AsyncVarRef>],
                else_state: &[BTreeMap<String, AsyncVarRef>],
            ) -> Result<Vec<BTreeMap<String, AsyncVarRef>>, CompilerError> {
                if before.len() != then_state.len() || before.len() != else_state.len() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: if scope depth mismatch".to_string(),
                    ));
                }

                let mut merged_scopes = Vec::with_capacity(before.len());
                for (i, before_scope) in before.iter().enumerate() {
                    let then_scope = &then_state[i];
                    let else_scope = &else_state[i];
                    let mut merged = BTreeMap::new();
                    for (name, pre) in before_scope {
                        let Some(t) = then_scope.get(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Internal,
                                "internal error: missing var in then branch".to_string(),
                            ));
                        };
                        let Some(e) = else_scope.get(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Internal,
                                "internal error: missing var in else branch".to_string(),
                            ));
                        };
                        if pre.ty != t.ty
                            || pre.ty != e.ty
                            || pre.c_name != t.c_name
                            || pre.c_name != e.c_name
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Internal,
                                "internal error: if branch var mismatch".to_string(),
                            ));
                        }

                        let mut v = pre.clone();
                        v.moved = t.moved || e.moved;
                        v.moved_ptr = if v.moved {
                            t.moved_ptr.clone().or_else(|| e.moved_ptr.clone())
                        } else {
                            None
                        };
                        merged.insert(name.clone(), v);
                    }
                    merged_scopes.push(merged);
                }
                Ok(merged_scopes)
            }

            fn emit_for(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 4 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "for form: (for <var> <start:i32> <end:i32> <body:any>)".to_string(),
                    ));
                }
                if dest.ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "for returns i32".to_string(),
                    ));
                }
                let var_name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "for variable must be an identifier".to_string(),
                    )
                })?;

                let (var, var_ty) = match self.lookup(var_name) {
                    Some(v) => (v.c_name, v.ty),
                    None => {
                        let v = self.alloc_local("v_for_", Ty::I32)?;
                        self.bind(var_name.to_string(), v.clone());
                        (v.c_name, Ty::I32)
                    }
                };
                if var_ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("for variable must be i32: {var_name:?}"),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                let end_tmp = self.alloc_local("t_for_end_", Ty::I32)?;
                let start_state = self.new_state();
                let end_state = self.new_state();
                let loop_check = self.new_state();
                let body_state = self.new_state();
                let inc_state = self.new_state();
                let done_state = self.new_state();

                self.line(state, format!("goto st_{start_state};"));
                self.emit_expr_entry(
                    start_state,
                    &args[1],
                    AsyncVarRef {
                        ty: Ty::I32,
                        brand: TyBrand::None,
                        c_name: var.clone(),
                        moved: false,
                        moved_ptr: None,
                    },
                    end_state,
                )?;
                self.emit_expr_entry(end_state, &args[2], end_tmp.clone(), loop_check)?;

                self.line(
                    loop_check,
                    format!("if ({var} >= {}) goto st_{done_state};", end_tmp.c_name),
                );
                self.line(loop_check, format!("goto st_{body_state};"));

                self.push_scope();
                let body_ty = self.infer_expr(&args[3])?;
                let body_storage = Self::storage_ty_for(body_ty.ty);
                let body_tmp = self.alloc_local("t_for_body_", body_storage)?;
                self.emit_expr_entry(body_state, &args[3], body_tmp, inc_state)?;
                self.pop_scope();

                self.line(inc_state, format!("{var} = {var} + UINT32_C(1);"));
                self.line(inc_state, format!("goto st_{loop_check};"));

                self.line(done_state, format!("{} = UINT32_C(0);", dest.c_name));
                self.line(done_state, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_overwrite_result_with_err(
                &mut self,
                state: usize,
                ty: Ty,
                c_name: &str,
                err_code: &str,
            ) {
                match ty {
                    Ty::ResultI32 => {
                        self.line(
                            state,
                            format!(
                                "{c_name} = (result_i32_t){{ .tag = UINT32_C(0), .payload.err = {err_code} }};"
                            ),
                        );
                    }
                    Ty::ResultBytes => {
                        self.line(state, format!("if ({c_name}.tag) {{"));
                        self.line(
                            state,
                            format!("  rt_bytes_drop(ctx, &{c_name}.payload.ok);"),
                        );
                        self.line(state, "}");
                        self.line(
                            state,
                            format!(
                                "{c_name} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {err_code} }};"
                            ),
                        );
                    }
                    Ty::ResultBytesView => {
                        self.line(
                            state,
                            format!(
                                "{c_name} = (result_bytes_view_t){{ .tag = UINT32_C(0), .payload.err = {err_code} }};"
                            ),
                        );
                    }
                    Ty::ResultResultBytes => {
                        self.line(state, format!("if ({c_name}.tag) {{"));
                        self.line(state, format!("  if ({c_name}.payload.ok.tag) {{"));
                        self.line(
                            state,
                            format!("    rt_bytes_drop(ctx, &{c_name}.payload.ok.payload.ok);"),
                        );
                        self.line(state, "  }");
                        self.line(state, "}");
                        self.line(
                            state,
                            format!(
                                "{c_name} = (result_result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {err_code} }};"
                            ),
                        );
                    }
                    _ => {}
                }
            }

            fn emit_budget_scope_v1(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "budget.scope_v1 expects 2 args".to_string(),
                    ));
                }
                let cfg = parse_budget_scope_cfg_v1(&args[0])?;
                if cfg.mode == BudgetScopeModeV1::ResultErrV1
                    && !matches!(
                        dest.ty,
                        Ty::ResultI32
                            | Ty::ResultBytes
                            | Ty::ResultBytesView
                            | Ty::ResultResultBytes
                    )
                {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.scope_v1 mode=result_err_v1 returns result_*".to_string(),
                    ));
                }

                let mode = match cfg.mode {
                    BudgetScopeModeV1::TrapV1 => "RT_BUDGET_MODE_TRAP",
                    BudgetScopeModeV1::ResultErrV1 => "RT_BUDGET_MODE_RESULT_ERR",
                    BudgetScopeModeV1::StatsOnlyV1 => "RT_BUDGET_MODE_STATS_ONLY",
                    BudgetScopeModeV1::YieldV1 => "RT_BUDGET_MODE_YIELD",
                };

                self.line(state, "rt_fuel(ctx, 1);");
                let scope = self.alloc_local("b_scope_", Ty::BudgetScopeV1)?;
                let label_bytes = cfg.label.as_bytes();
                self.tmp_counter += 1;
                let label_name = format!("budget_label_{}", self.tmp_counter);
                let label_escaped = c_escape_string(label_bytes);
                self.line(
                    state,
                    format!("static const char {label_name}[] = \"{label_escaped}\";"),
                );
                self.line(
                    state,
                    format!(
                        "rt_budget_scope_init(ctx, &{}, {mode}, (const uint8_t*){label_name}, UINT32_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}));",
                        scope.c_name,
                        label_bytes.len(),
                        cfg.alloc_bytes,
                        cfg.alloc_calls,
                        cfg.realloc_calls,
                        cfg.memcpy_bytes,
                        cfg.sched_ticks,
                        cfg.fuel
                    ),
                );

                let body_state = self.new_state();
                let exit_state = self.new_state();
                self.line(state, format!("goto st_{body_state};"));

                self.cleanup_scopes.push(CleanupScope::Budget {
                    c_name: scope.c_name.clone(),
                });
                self.emit_expr_entry(body_state, &args[1], dest.clone(), exit_state)?;
                let popped_cleanup = self.cleanup_scopes.pop();
                debug_assert!(matches!(
                    popped_cleanup,
                    Some(CleanupScope::Budget { c_name }) if c_name == scope.c_name
                ));

                let resume = exit_state;
                self.line(exit_state, "rt_fuel(ctx, 1);");
                self.line(
                    exit_state,
                    format!("if (rt_budget_scope_exit_poll(ctx, &{})) {{", scope.c_name),
                );
                if cfg.mode == BudgetScopeModeV1::ResultErrV1 {
                    self.line(exit_state, format!("  if ({}.violated) {{", scope.c_name));
                    self.emit_overwrite_result_with_err(
                        exit_state,
                        dest.ty,
                        &dest.c_name,
                        &format!("{}.err_code", scope.c_name),
                    );
                    self.line(exit_state, "  }");
                }
                self.line(exit_state, format!("  goto st_{cont};"));
                self.line(exit_state, "}");
                self.line(exit_state, format!("f->state = UINT32_C({resume});"));
                self.line(exit_state, "return UINT32_C(0);");
                Ok(())
            }

            fn emit_budget_scope_from_arch_v1(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "budget.scope_from_arch_v1 expects 2 args".to_string(),
                    ));
                }
                let Expr::List { items: lit, .. } = &args[0] else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.scope_from_arch_v1 expects bytes.lit profile_id".to_string(),
                    ));
                };
                if lit.first().and_then(Expr::as_ident) != Some("bytes.lit") || lit.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.scope_from_arch_v1 expects bytes.lit profile_id".to_string(),
                    ));
                }
                let Some(profile_id) = lit[1].as_ident() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.scope_from_arch_v1 expects bytes.lit profile_id".to_string(),
                    ));
                };
                let cfg = load_budget_profile_cfg_from_arch_v1(&self.options, profile_id)?;
                if cfg.mode == BudgetScopeModeV1::ResultErrV1
                    && !matches!(
                        dest.ty,
                        Ty::ResultI32
                            | Ty::ResultBytes
                            | Ty::ResultBytesView
                            | Ty::ResultResultBytes
                    )
                {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.scope_from_arch_v1 returns result_* for this profile".to_string(),
                    ));
                }

                let mode = match cfg.mode {
                    BudgetScopeModeV1::TrapV1 => "RT_BUDGET_MODE_TRAP",
                    BudgetScopeModeV1::ResultErrV1 => "RT_BUDGET_MODE_RESULT_ERR",
                    BudgetScopeModeV1::StatsOnlyV1 => "RT_BUDGET_MODE_STATS_ONLY",
                    BudgetScopeModeV1::YieldV1 => "RT_BUDGET_MODE_YIELD",
                };

                self.line(state, "rt_fuel(ctx, 1);");
                let scope = self.alloc_local("b_scope_", Ty::BudgetScopeV1)?;
                let label_bytes = cfg.label.as_bytes();
                self.tmp_counter += 1;
                let label_name = format!("budget_label_{}", self.tmp_counter);
                let label_escaped = c_escape_string(label_bytes);
                self.line(
                    state,
                    format!("static const char {label_name}[] = \"{label_escaped}\";"),
                );
                self.line(
                    state,
                    format!(
                        "rt_budget_scope_init(ctx, &{}, {mode}, (const uint8_t*){label_name}, UINT32_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}));",
                        scope.c_name,
                        label_bytes.len(),
                        cfg.alloc_bytes,
                        cfg.alloc_calls,
                        cfg.realloc_calls,
                        cfg.memcpy_bytes,
                        cfg.sched_ticks,
                        cfg.fuel
                    ),
                );

                let body_state = self.new_state();
                let exit_state = self.new_state();
                self.line(state, format!("goto st_{body_state};"));

                self.cleanup_scopes.push(CleanupScope::Budget {
                    c_name: scope.c_name.clone(),
                });
                self.emit_expr_entry(body_state, &args[1], dest.clone(), exit_state)?;
                let popped_cleanup = self.cleanup_scopes.pop();
                debug_assert!(matches!(
                    popped_cleanup,
                    Some(CleanupScope::Budget { c_name }) if c_name == scope.c_name
                ));

                let resume = exit_state;
                self.line(exit_state, "rt_fuel(ctx, 1);");
                self.line(
                    exit_state,
                    format!("if (rt_budget_scope_exit_poll(ctx, &{})) {{", scope.c_name),
                );
                if cfg.mode == BudgetScopeModeV1::ResultErrV1 {
                    self.line(exit_state, format!("  if ({}.violated) {{", scope.c_name));
                    self.emit_overwrite_result_with_err(
                        exit_state,
                        dest.ty,
                        &dest.c_name,
                        &format!("{}.err_code", scope.c_name),
                    );
                    self.line(exit_state, "  }");
                }
                self.line(exit_state, format!("  goto st_{cont};"));
                self.line(exit_state, "}");
                self.line(exit_state, format!("f->state = UINT32_C({resume});"));
                self.line(exit_state, "return UINT32_C(0);");
                Ok(())
            }

            fn emit_std_rr_with_policy_v1(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if !self.options.enable_rr {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        "std.rr.with_policy_v1 is disabled in this world".to_string(),
                    ));
                }
                if args.len() != 4 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "std.rr.with_policy_v1 expects 4 args".to_string(),
                    ));
                }
                if self.fn_ret_ty != Ty::ResultBytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "std.rr.with_policy_v1 requires function return type result_bytes (open failure propagation)".to_string(),
                    ));
                }

                let body_ty = self.infer_expr(&args[3])?;
                if body_ty.ty != dest.ty && body_ty.ty != Ty::Never {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "std.rr.with_policy_v1 body must evaluate to {:?} (or return)",
                            dest.ty
                        ),
                    ));
                }

                let policy_id = parse_bytes_lit_ascii(&args[0], "std.rr.with_policy_v1 policy_id")?;
                let cassette_path =
                    parse_bytes_lit_ascii(&args[1], "std.rr.with_policy_v1 cassette_path")?;
                let mode_i32 = parse_i32_lit(&args[2], "std.rr.with_policy_v1 mode")?;

                let cfg = load_rr_cfg_v1_from_arch_v1(
                    &self.options,
                    &policy_id,
                    &cassette_path,
                    mode_i32,
                )?;

                self.line(state, "rt_fuel(ctx, 1);");

                self.tmp_counter += 1;
                let cfg_name = format!("rr_cfg_{}", self.tmp_counter);
                let escaped = c_escape_string(cfg.as_slice());
                self.line(
                    state,
                    format!("static const char {cfg_name}[] = \"{escaped}\";"),
                );

                let cfg_bytes = self.alloc_local("t_rr_cfg_bytes_", Ty::Bytes)?;
                let cfg_view = self.alloc_local("t_rr_cfg_view_", Ty::BytesView)?;
                let open_res = self.alloc_local("t_rr_open_", Ty::ResultI32)?;

                self.line(
                    state,
                    format!(
                        "{} = rt_bytes_from_literal(ctx, (const uint8_t*){cfg_name}, UINT32_C({}));",
                        cfg_bytes.c_name,
                        cfg.len()
                    ),
                );
                self.line(
                    state,
                    format!(
                        "{} = rt_bytes_view(ctx, {});",
                        cfg_view.c_name, cfg_bytes.c_name
                    ),
                );
                self.line(
                    state,
                    format!(
                        "{} = rt_rr_open_v1(ctx, {});",
                        open_res.c_name, cfg_view.c_name
                    ),
                );
                self.line(state, format!("rt_bytes_drop(ctx, &{});", cfg_bytes.c_name));
                self.line(
                    state,
                    format!("{} = rt_bytes_empty(ctx);", cfg_bytes.c_name),
                );

                let fail_state = self.new_state();
                let ok_state = self.new_state();
                self.line(
                    state,
                    format!(
                        "if ({}.tag == UINT32_C(0)) goto st_{fail_state};",
                        open_res.c_name
                    ),
                );
                self.line(state, format!("goto st_{ok_state};"));

                let scopes_snapshot = self.scopes.clone();
                let task_scopes_snapshot = self.task_scopes.clone();
                let cleanup_scopes_snapshot = self.cleanup_scopes.clone();

                self.line(
                    fail_state,
                    format!(
                        "f->ret = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {}.payload.err }};",
                        open_res.c_name
                    ),
                );

                let mut next = self.ret_state;
                for scope in cleanup_scopes_snapshot.iter() {
                    let st = self.new_state();
                    let resume = st;
                    match scope {
                        CleanupScope::Task { c_name } => {
                            self.line(
                                st,
                                format!("if (rt_scope_exit_poll(ctx, &{c_name})) goto st_{next};"),
                            );
                        }
                        CleanupScope::Budget { c_name } => {
                            self.line(
                                st,
                                format!("if (rt_budget_scope_exit_poll(ctx, &{c_name})) {{"),
                            );
                            self.line(st, format!("  goto st_{next};"));
                            self.line(st, "}");
                        }
                        CleanupScope::Rr {
                            handle_c_name,
                            prev_c_name,
                        } => {
                            self.line(st, format!("ctx->rr_current = {prev_c_name};"));
                            self.line(st, format!("rt_rr_close_v1(ctx, {handle_c_name});"));
                            self.line(st, format!("goto st_{next};"));
                        }
                    }
                    self.line(st, format!("f->state = UINT32_C({resume});"));
                    self.line(st, "return UINT32_C(0);");
                    next = st;
                }
                self.line(fail_state, format!("goto st_{next};"));

                self.scopes = scopes_snapshot;
                self.task_scopes = task_scopes_snapshot;
                self.cleanup_scopes = cleanup_scopes_snapshot;

                let body_state = self.new_state();
                let exit_state = self.new_state();

                let handle_name = self.alloc_local("t_rr_h_", Ty::I32)?;
                let prev_name = self.alloc_local("t_rr_prev_", Ty::I32)?;

                self.line(
                    ok_state,
                    format!(
                        "{} = (int32_t){}.payload.ok;",
                        handle_name.c_name, open_res.c_name
                    ),
                );
                self.line(ok_state, format!("{} = ctx->rr_current;", prev_name.c_name));
                self.line(
                    ok_state,
                    format!("ctx->rr_current = {};", handle_name.c_name),
                );
                self.line(ok_state, format!("goto st_{body_state};"));

                self.cleanup_scopes.push(CleanupScope::Rr {
                    handle_c_name: handle_name.c_name.clone(),
                    prev_c_name: prev_name.c_name.clone(),
                });
                self.emit_expr_entry(body_state, &args[3], dest, exit_state)?;
                let popped_cleanup = self.cleanup_scopes.pop();
                debug_assert!(matches!(popped_cleanup, Some(CleanupScope::Rr { .. })));

                self.line(exit_state, "rt_fuel(ctx, 1);");
                self.line(
                    exit_state,
                    format!("ctx->rr_current = {};", prev_name.c_name),
                );
                self.line(
                    exit_state,
                    format!("rt_rr_close_v1(ctx, {});", handle_name.c_name),
                );
                self.line(exit_state, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_expr_as_bytes_view_entry(
                &mut self,
                state: usize,
                head: &str,
                expr: &Expr,
                dest_view: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if dest_view.ty != Ty::BytesView {
                    return Err(CompilerError::new(
                        CompileErrorKind::Internal,
                        "emit_expr_as_bytes_view_entry expects bytes_view dest".to_string(),
                    ));
                }
                let ty = self.infer_expr(expr)?;
                match ty.ty {
                    Ty::BytesView => self.emit_expr_entry(state, expr, dest_view, cont),
                    Ty::Bytes => match expr {
                        Expr::Ident { name, ptr: use_ptr } if name != "input" => {
                            self.line(state, "rt_fuel(ctx, 1);");
                            let Some(v) = self.lookup(name) else {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("unknown identifier: {name:?}"),
                                ));
                            };
                            if v.moved {
                                let moved_ptr = v
                                    .moved_ptr
                                    .as_deref()
                                    .filter(|p| !p.is_empty())
                                    .unwrap_or("<unknown>");
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!(
                                        "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                    ),
                                ));
                            }
                            if v.ty != Ty::Bytes {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("{head} expects bytes_view"),
                                ));
                            }
                            self.line(
                                state,
                                format!("{} = rt_bytes_view(ctx, {});", dest_view.c_name, v.c_name),
                            );
                            self.line(state, format!("goto st_{cont};"));
                            Ok(())
                        }
                        _ => {
                            let owner = self.alloc_local("t_view_owner_", Ty::Bytes)?;
                            let view_state = self.new_state();
                            self.emit_expr_entry(state, expr, owner.clone(), view_state)?;
                            self.line(view_state, "rt_fuel(ctx, 1);");
                            self.line(
                                view_state,
                                format!(
                                    "{} = rt_bytes_view(ctx, {});",
                                    dest_view.c_name, owner.c_name
                                ),
                            );
                            self.line(view_state, format!("goto st_{cont};"));
                            Ok(())
                        }
                    },
                    Ty::VecU8 => match expr {
                        Expr::Ident { name, ptr: use_ptr } => {
                            self.line(state, "rt_fuel(ctx, 1);");
                            let Some(v) = self.lookup(name) else {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("unknown identifier: {name:?}"),
                                ));
                            };
                            if v.moved {
                                let moved_ptr = v
                                    .moved_ptr
                                    .as_deref()
                                    .filter(|p| !p.is_empty())
                                    .unwrap_or("<unknown>");
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!(
                                        "use after move: {name:?} ptr={use_ptr} moved_ptr={moved_ptr}"
                                    ),
                                ));
                            }
                            if v.ty != Ty::VecU8 {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("{head} expects bytes_view"),
                                ));
                            }
                            self.line(
                                state,
                                format!(
                                    "{} = rt_vec_u8_as_view(ctx, {});",
                                    dest_view.c_name, v.c_name
                                ),
                            );
                            self.line(state, format!("goto st_{cont};"));
                            Ok(())
                        }
                        _ => {
                            let owner = self.alloc_local("t_view_owner_", Ty::VecU8)?;
                            let view_state = self.new_state();
                            self.emit_expr_entry(state, expr, owner.clone(), view_state)?;
                            self.line(view_state, "rt_fuel(ctx, 1);");
                            self.line(
                                view_state,
                                format!(
                                    "{} = rt_vec_u8_as_view(ctx, {});",
                                    dest_view.c_name, owner.c_name
                                ),
                            );
                            self.line(view_state, format!("goto st_{cont};"));
                            Ok(())
                        }
                    },
                    _ => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} expects bytes_view"),
                    )),
                }
            }

            fn emit_rr_next_v1(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if !self.options.enable_rr {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        "rr.next_v1 is disabled in this world".to_string(),
                    ));
                }
                if args.len() != 4 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "rr.next_v1 expects 4 args".to_string(),
                    ));
                }
                if dest.ty != Ty::ResultBytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "rr.next_v1 returns result_bytes".to_string(),
                    ));
                }

                let h_ty = self.infer_expr(&args[0])?;
                if h_ty.ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "rr.next_v1 expects (i32 rr_handle_v1, bytes_view kind, bytes_view op, bytes_view key)"
                            .to_string(),
                    ));
                }

                let h_tmp = self.alloc_local("t_rr_h_", Ty::I32)?;
                let kind_view = self.alloc_local("t_rr_kind_", Ty::BytesView)?;
                let op_view = self.alloc_local("t_rr_op_", Ty::BytesView)?;
                let key_view = self.alloc_local("t_rr_key_", Ty::BytesView)?;
                let latency = self.alloc_local("t_rr_latency_", Ty::I32)?;

                self.line(state, "rt_fuel(ctx, 1);");
                let h_state = self.new_state();
                self.line(state, format!("goto st_{h_state};"));

                let kind_state = self.new_state();
                self.emit_expr_entry(h_state, &args[0], h_tmp.clone(), kind_state)?;

                let op_state = self.new_state();
                self.emit_expr_as_bytes_view_entry(
                    kind_state,
                    "rr.next_v1",
                    &args[1],
                    kind_view.clone(),
                    op_state,
                )?;

                let key_state = self.new_state();
                self.emit_expr_as_bytes_view_entry(
                    op_state,
                    "rr.next_v1",
                    &args[2],
                    op_view.clone(),
                    key_state,
                )?;

                let apply_state = self.new_state();
                self.emit_expr_as_bytes_view_entry(
                    key_state,
                    "rr.next_v1",
                    &args[3],
                    key_view.clone(),
                    apply_state,
                )?;

                let resume_state = self.new_state();

                self.line(apply_state, "rt_fuel(ctx, 1);");
                self.line(apply_state, format!("{} = UINT32_C(0);", latency.c_name));
                self.line(
                    apply_state,
                    format!(
                        "{} = rt_rr_next_v1(ctx, {}, {}, {}, {}, &{}, UINT32_C(0));",
                        dest.c_name,
                        h_tmp.c_name,
                        kind_view.c_name,
                        op_view.c_name,
                        key_view.c_name,
                        latency.c_name
                    ),
                );
                self.line(
                    apply_state,
                    format!("if ({}.tag == UINT32_C(0)) goto st_{cont};", dest.c_name),
                );
                self.line(
                    apply_state,
                    format!("if ({} != UINT32_C(0)) {{", latency.c_name),
                );
                self.line(
                    apply_state,
                    format!("  rt_task_sleep(ctx, {});", latency.c_name),
                );
                self.line(
                    apply_state,
                    format!("  f->state = UINT32_C({resume_state});"),
                );
                self.line(apply_state, "  return UINT32_C(0);");
                self.line(apply_state, "}");
                self.line(apply_state, format!("goto st_{cont};"));

                self.line(resume_state, "rt_fuel(ctx, 1);");
                self.line(resume_state, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_task_scope_v1(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "task.scope_v1 expects 2 args".to_string(),
                    ));
                }
                let cfg = parse_task_scope_cfg_v1(&args[0])?;

                self.line(state, "rt_fuel(ctx, 1);");
                let scope = self.alloc_local("t_scope_", Ty::TaskScopeV1)?;
                self.line(
                    state,
                    format!(
                        "rt_scope_init(ctx, &{}, UINT32_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT32_C({}));",
                        scope.c_name,
                        cfg.max_children,
                        cfg.max_ticks,
                        cfg.max_blocked_waits,
                        cfg.max_join_polls,
                        cfg.max_slot_result_bytes
                    ),
                );

                let body_state = self.new_state();
                let exit_state = self.new_state();
                self.line(state, format!("goto st_{body_state};"));

                self.task_scopes.push(scope.clone());
                self.cleanup_scopes.push(CleanupScope::Task {
                    c_name: scope.c_name.clone(),
                });
                self.emit_expr_entry(body_state, &args[1], dest, exit_state)?;
                let popped = self.task_scopes.pop();
                debug_assert_eq!(popped.as_ref().map(|v| &v.c_name), Some(&scope.c_name));
                let popped_cleanup = self.cleanup_scopes.pop();
                debug_assert!(matches!(
                    popped_cleanup,
                    Some(CleanupScope::Task { c_name }) if c_name == scope.c_name
                ));

                let resume = exit_state;
                self.line(exit_state, "rt_fuel(ctx, 1);");
                self.line(
                    exit_state,
                    format!(
                        "if (rt_scope_exit_poll(ctx, &{})) goto st_{cont};",
                        scope.c_name
                    ),
                );
                self.line(exit_state, format!("f->state = UINT32_C({resume});"));
                self.line(exit_state, "return UINT32_C(0);");
                Ok(())
            }

            fn emit_task_scope_wait_all_v1(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                let scope = self.task_scopes.last().cloned().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        "X07E_SCOPE_001: task.scope.wait_all_v1 used outside task.scope_v1"
                            .to_string(),
                    )
                })?;
                if !args.is_empty() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "task.scope.wait_all_v1 expects 0 args".to_string(),
                    ));
                }
                if dest.ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.wait_all_v1 returns i32".to_string(),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                self.line(
                    state,
                    format!(
                        "{} = rt_scope_wait_all_count(&{});",
                        dest.c_name, scope.c_name
                    ),
                );
                let join_state = self.new_state();
                self.line(state, format!("goto st_{join_state};"));

                let resume = join_state;
                self.line(join_state, "rt_fuel(ctx, 1);");
                self.line(
                    join_state,
                    format!(
                        "if (rt_scope_join_drop_remaining_poll(ctx, &{})) {{ rt_scope_reset_active(&{}); goto st_{cont}; }}",
                        scope.c_name, scope.c_name
                    ),
                );
                self.line(join_state, format!("f->state = UINT32_C({resume});"));
                self.line(join_state, "return UINT32_C(0);");
                Ok(())
            }

            fn emit_task_scope_select_v1(
                &mut self,
                state: usize,
                head: &str,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                let scope = self.task_scopes.last().cloned().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        "X07E_SELECT_OUTSIDE_SCOPE: task.scope.select used outside task.scope_v1"
                            .to_string(),
                    )
                })?;
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("{head} expects 2 args"),
                    ));
                }
                let cfg = parse_task_select_cfg_v1(&args[0])?;
                let cases = parse_task_select_cases_v1(&args[1])?;
                if cases.len() > cfg.max_cases as usize {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "X07E_SELECT_TOO_MANY_CASES: too many cases".to_string(),
                    ));
                }

                let is_try = head == "task.scope.select_try_v1";
                if is_try && dest.ty != Ty::OptionTaskSelectEvtV1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.select_try_v1 returns option_i32".to_string(),
                    ));
                }
                if !is_try && dest.ty != Ty::TaskSelectEvtV1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.select_v1 returns task_select_evt_v1".to_string(),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                let slot_tmp = self.alloc_local("t_select_slot_", Ty::TaskSlotV1)?;
                let chan_tmp = self.alloc_local("t_select_chan_", Ty::I32)?;
                let rr_start_tmp = if cfg.policy == TaskSelectPolicyV1::RrV1 {
                    Some(self.alloc_local("t_select_rr_", Ty::I32)?)
                } else {
                    None
                };
                let poll_tmp = if is_try {
                    None
                } else {
                    Some(self.alloc_local("t_select_polls_", Ty::I32)?)
                };
                let slept_tmp = if is_try || cfg.timeout_ticks == 0 {
                    None
                } else {
                    Some(self.alloc_local("t_select_slept_", Ty::I32)?)
                };

                if let Some(polls) = &poll_tmp {
                    self.line(state, format!("{} = UINT32_C(0);", polls.c_name));
                }
                if let Some(slept) = &slept_tmp {
                    self.line(state, format!("{} = UINT32_C(0);", slept.c_name));
                }

                let scan_state = self.new_state();
                let no_ready_state = self.new_state();
                self.line(state, format!("goto st_{scan_state};"));

                self.line(scan_state, "rt_fuel(ctx, 1);");
                if let Some(rr_start) = &rr_start_tmp {
                    let n_cases = cases.len();
                    self.line(
                        scan_state,
                        format!("{} = {}.select_rr_cursor;", rr_start.c_name, scope.c_name),
                    );
                    if n_cases != 0 {
                        self.line(
                            scan_state,
                            format!(
                                "if ({} >= UINT32_C({})) {} = UINT32_C(0);",
                                rr_start.c_name, n_cases, rr_start.c_name
                            ),
                        );
                    }
                }

                let mut first_pass_states: Vec<(usize, usize, Option<usize>)> = Vec::new();
                let mut second_pass_states: Vec<(usize, usize, Option<usize>)> = Vec::new();

                fn pass_skip_cond(
                    idx: usize,
                    rr_start: Option<&AsyncVarRef>,
                    first_pass: bool,
                ) -> Option<String> {
                    let rr_start = rr_start?;
                    if first_pass {
                        Some(format!(
                            "if (UINT32_C({idx}) < {}) goto {{NEXT}};",
                            rr_start.c_name
                        ))
                    } else {
                        Some(format!(
                            "if (UINT32_C({idx}) >= {}) goto {{NEXT}};",
                            rr_start.c_name
                        ))
                    }
                }

                for (idx, case) in cases.iter().enumerate() {
                    let eval_state = self.new_state();
                    let check_state = self.new_state();
                    let skip = pass_skip_cond(idx, rr_start_tmp.as_ref(), true);
                    first_pass_states.push((eval_state, check_state, skip.map(|_| 0)));

                    let eval_state2 = self.new_state();
                    let check_state2 = self.new_state();
                    let skip2 = pass_skip_cond(idx, rr_start_tmp.as_ref(), false);
                    second_pass_states.push((eval_state2, check_state2, skip2.map(|_| 0)));

                    // Silence unused warnings for `case`.
                    let _ = case;
                }

                // Wire scan start to first-pass first eval.
                if !first_pass_states.is_empty() {
                    self.line(scan_state, format!("goto st_{};", first_pass_states[0].0));
                } else {
                    self.line(scan_state, format!("goto st_{no_ready_state};"));
                }

                // Helper to emit a single case evaluation + check.
                let emit_case = |this: &mut Machine,
                                 eval_state: usize,
                                 check_state: usize,
                                 next_state: usize,
                                 rr_skip: Option<&str>,
                                 case_idx: usize,
                                 case: &TaskSelectCaseV1,
                                 scope: &AsyncVarRef,
                                 slot_tmp: &AsyncVarRef,
                                 chan_tmp: &AsyncVarRef,
                                 dest: &AsyncVarRef,
                                 cont: usize,
                                 cases_len: usize,
                                 policy: TaskSelectPolicyV1|
                 -> Result<(), CompilerError> {
                    if let Some(skip) = rr_skip {
                        this.line(
                            eval_state,
                            skip.replace("{NEXT}", &format!("st_{next_state}")),
                        );
                    }
                    let res_var = format!("t_select_r_{check_state}");
                    match case {
                        TaskSelectCaseV1::SlotBytes { slot } => {
                            this.emit_expr_entry(eval_state, slot, slot_tmp.clone(), check_state)?;

                            // Sentinel-based disabling:
                            // - slot cases: UINT32_MAX disables the case
                            this.line(
                                check_state,
                                format!(
                                    "if ({} == UINT32_MAX) goto st_{next_state};",
                                    slot_tmp.c_name
                                ),
                            );
                            this.line(
                                check_state,
                                format!(
                                    "result_bytes_t {res_var} = rt_scope_try_await_slot_bytes(ctx, &{}, {});",
                                    scope.c_name, slot_tmp.c_name
                                ),
                            );
                            this.line(check_state, format!("if ({res_var}.tag) {{"));
                            this.line(
                                check_state,
                                if dest.ty == Ty::OptionTaskSelectEvtV1 {
                                    format!(
                                        "  {}.tag = UINT32_C(1); {}.payload = rt_select_evt_new(ctx, {}.key, UINT32_C(1), UINT32_C({}), {}, {res_var}.payload.ok);",
                                        dest.c_name,
                                        dest.c_name,
                                        scope.c_name,
                                        case_idx,
                                        slot_tmp.c_name
                                    )
                                } else {
                                    format!(
                                        "  {} = rt_select_evt_new(ctx, {}.key, UINT32_C(1), UINT32_C({}), {}, {res_var}.payload.ok);",
                                        dest.c_name,
                                        scope.c_name,
                                        case_idx,
                                        slot_tmp.c_name
                                    )
                                },
                            );
                            if policy == TaskSelectPolicyV1::RrV1 && cases_len != 0 {
                                let next = (case_idx + 1) % cases_len;
                                this.line(
                                    check_state,
                                    format!(
                                        "  {}.select_rr_cursor = UINT32_C({next});",
                                        scope.c_name
                                    ),
                                );
                            }
                            this.line(check_state, format!("  goto st_{cont};"));
                            this.line(check_state, "}");
                            this.line(
                                check_state,
                                format!("if ({res_var}.payload.err == UINT32_C(2)) {{"),
                            );
                            this.line(
                                check_state,
                                if dest.ty == Ty::OptionTaskSelectEvtV1 {
                                    format!(
                                        "  {}.tag = UINT32_C(1); {}.payload = rt_select_evt_new(ctx, {}.key, UINT32_C(2), UINT32_C({}), {}, rt_bytes_empty(ctx));",
                                        dest.c_name,
                                        dest.c_name,
                                        scope.c_name,
                                        case_idx,
                                        slot_tmp.c_name
                                    )
                                } else {
                                    format!(
                                        "  {} = rt_select_evt_new(ctx, {}.key, UINT32_C(2), UINT32_C({}), {}, rt_bytes_empty(ctx));",
                                        dest.c_name,
                                        scope.c_name,
                                        case_idx,
                                        slot_tmp.c_name
                                    )
                                },
                            );
                            if policy == TaskSelectPolicyV1::RrV1 && cases_len != 0 {
                                let next = (case_idx + 1) % cases_len;
                                this.line(
                                    check_state,
                                    format!(
                                        "  {}.select_rr_cursor = UINT32_C({next});",
                                        scope.c_name
                                    ),
                                );
                            }
                            this.line(check_state, format!("  goto st_{cont};"));
                            this.line(check_state, "}");
                            this.line(check_state, format!("goto st_{next_state};"));
                        }
                        TaskSelectCaseV1::ChanRecvBytes { chan } => {
                            this.emit_expr_entry(eval_state, chan, chan_tmp.clone(), check_state)?;

                            // Sentinel-based disabling:
                            // - chan cases: 0 disables the case (chan ids are 1-based)
                            this.line(
                                check_state,
                                format!(
                                    "if ({} == UINT32_C(0)) goto st_{next_state};",
                                    chan_tmp.c_name
                                ),
                            );
                            this.line(
                                check_state,
                                format!(
                                    "result_bytes_t {res_var} = rt_chan_bytes_try_recv(ctx, {});",
                                    chan_tmp.c_name
                                ),
                            );
                            this.line(check_state, format!("if ({res_var}.tag) {{"));
                            this.line(
                                check_state,
                                if dest.ty == Ty::OptionTaskSelectEvtV1 {
                                    format!(
                                        "  {}.tag = UINT32_C(1); {}.payload = rt_select_evt_new(ctx, {}.key, UINT32_C(3), UINT32_C({}), {}, {res_var}.payload.ok);",
                                        dest.c_name,
                                        dest.c_name,
                                        scope.c_name,
                                        case_idx,
                                        chan_tmp.c_name
                                    )
                                } else {
                                    format!(
                                        "  {} = rt_select_evt_new(ctx, {}.key, UINT32_C(3), UINT32_C({}), {}, {res_var}.payload.ok);",
                                        dest.c_name,
                                        scope.c_name,
                                        case_idx,
                                        chan_tmp.c_name
                                    )
                                },
                            );
                            if policy == TaskSelectPolicyV1::RrV1 && cases_len != 0 {
                                let next = (case_idx + 1) % cases_len;
                                this.line(
                                    check_state,
                                    format!(
                                        "  {}.select_rr_cursor = UINT32_C({next});",
                                        scope.c_name
                                    ),
                                );
                            }
                            this.line(check_state, format!("  goto st_{cont};"));
                            this.line(check_state, "}");
                            this.line(
                                check_state,
                                format!("if ({res_var}.payload.err == UINT32_C(2)) {{"),
                            );
                            this.line(
                                check_state,
                                if dest.ty == Ty::OptionTaskSelectEvtV1 {
                                    format!(
                                        "  {}.tag = UINT32_C(1); {}.payload = rt_select_evt_new(ctx, {}.key, UINT32_C(4), UINT32_C({}), {}, rt_bytes_empty(ctx));",
                                        dest.c_name,
                                        dest.c_name,
                                        scope.c_name,
                                        case_idx,
                                        chan_tmp.c_name
                                    )
                                } else {
                                    format!(
                                        "  {} = rt_select_evt_new(ctx, {}.key, UINT32_C(4), UINT32_C({}), {}, rt_bytes_empty(ctx));",
                                        dest.c_name,
                                        scope.c_name,
                                        case_idx,
                                        chan_tmp.c_name
                                    )
                                },
                            );
                            if policy == TaskSelectPolicyV1::RrV1 && cases_len != 0 {
                                let next = (case_idx + 1) % cases_len;
                                this.line(
                                    check_state,
                                    format!(
                                        "  {}.select_rr_cursor = UINT32_C({next});",
                                        scope.c_name
                                    ),
                                );
                            }
                            this.line(check_state, format!("  goto st_{cont};"));
                            this.line(check_state, "}");
                            this.line(check_state, format!("goto st_{next_state};"));
                        }
                    }
                    Ok(())
                };

                // Emit first pass cases.
                for (idx, (eval_state, check_state, _)) in first_pass_states.iter().enumerate() {
                    let next = if idx + 1 < first_pass_states.len() {
                        first_pass_states[idx + 1].0
                    } else if cfg.policy == TaskSelectPolicyV1::RrV1 {
                        // Second pass starts.
                        second_pass_states
                            .first()
                            .map(|s| s.0)
                            .unwrap_or(no_ready_state)
                    } else {
                        no_ready_state
                    };
                    let rr_skip = pass_skip_cond(idx, rr_start_tmp.as_ref(), true);
                    emit_case(
                        self,
                        *eval_state,
                        *check_state,
                        next,
                        rr_skip.as_deref(),
                        idx,
                        &cases[idx],
                        &scope,
                        &slot_tmp,
                        &chan_tmp,
                        &dest,
                        cont,
                        cases.len(),
                        cfg.policy,
                    )?;
                }

                // Emit second pass cases for rr.
                if cfg.policy == TaskSelectPolicyV1::RrV1 {
                    for (idx, (eval_state, check_state, _)) in second_pass_states.iter().enumerate()
                    {
                        let next = if idx + 1 < second_pass_states.len() {
                            second_pass_states[idx + 1].0
                        } else {
                            no_ready_state
                        };
                        let rr_skip = pass_skip_cond(idx, rr_start_tmp.as_ref(), false);
                        emit_case(
                            self,
                            *eval_state,
                            *check_state,
                            next,
                            rr_skip.as_deref(),
                            idx,
                            &cases[idx],
                            &scope,
                            &slot_tmp,
                            &chan_tmp,
                            &dest,
                            cont,
                            cases.len(),
                            cfg.policy,
                        )?;
                    }
                }

                // No ready handling.
                if is_try {
                    self.line(
                        no_ready_state,
                        format!("{}.tag = UINT32_C(0);", dest.c_name),
                    );
                    self.line(
                        no_ready_state,
                        format!("{}.payload = UINT32_C(0);", dest.c_name),
                    );
                    self.line(no_ready_state, format!("goto st_{cont};"));
                    return Ok(());
                }

                let polls = poll_tmp.expect("blocking select has polls");
                self.line(
                    no_ready_state,
                    format!("{} = {} + UINT32_C(1);", polls.c_name, polls.c_name),
                );
                if cfg.max_polls != 0 {
                    self.line(
                        no_ready_state,
                        format!(
                            "if ({} > UINT32_C({})) rt_trap(\"X07T_SELECT_MAX_POLLS\");",
                            polls.c_name, cfg.max_polls
                        ),
                    );
                }
                if cfg.timeout_ticks != 0 {
                    let slept = slept_tmp.as_ref().expect("timeout has slept");
                    self.line(
                        no_ready_state,
                        format!(
                            "if ({} >= UINT32_C({})) {{ {} = rt_select_evt_new(ctx, {}.key, UINT32_C(5), UINT32_C(0), UINT32_C(0), rt_bytes_empty(ctx)); goto st_{cont}; }}",
                            slept.c_name,
                            cfg.timeout_ticks,
                            dest.c_name,
                            scope.c_name
                        ),
                    );
                }
                if cfg.poll_sleep_ticks != 0 {
                    self.line(
                        no_ready_state,
                        format!("rt_task_sleep(ctx, UINT32_C({}));", cfg.poll_sleep_ticks),
                    );
                    if let Some(slept) = &slept_tmp {
                        self.line(
                            no_ready_state,
                            format!(
                                "{} = {} + UINT32_C({});",
                                slept.c_name, slept.c_name, cfg.poll_sleep_ticks
                            ),
                        );
                    }
                } else {
                    self.line(no_ready_state, "rt_task_yield(ctx);");
                }
                self.line(
                    no_ready_state,
                    format!("f->state = UINT32_C({scan_state});"),
                );
                self.line(no_ready_state, "return UINT32_C(0);");

                Ok(())
            }

            fn emit_return(&mut self, state: usize, args: &[Expr]) -> Result<(), CompilerError> {
                if args.len() != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "return form: (return <expr>)".to_string(),
                    ));
                }
                let scopes_snapshot = self.scopes.clone();
                let task_scopes_snapshot = self.task_scopes.clone();
                let cleanup_scopes_snapshot = self.cleanup_scopes.clone();

                self.line(state, "rt_fuel(ctx, 1);");
                let expr_state = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));

                let cleanup_start = self.new_state();
                self.emit_expr_entry(
                    expr_state,
                    &args[0],
                    AsyncVarRef {
                        ty: self.fn_ret_ty,
                        brand: TyBrand::None,
                        c_name: "f->ret".to_string(),
                        moved: false,
                        moved_ptr: None,
                    },
                    cleanup_start,
                )?;

                // Unwind any active cleanup scopes (inner-to-outer) before returning.
                let mut next = self.ret_state;
                for scope in cleanup_scopes_snapshot.iter() {
                    let st = self.new_state();
                    let resume = st;
                    match scope {
                        CleanupScope::Task { c_name } => {
                            self.line(
                                st,
                                format!("if (rt_scope_exit_poll(ctx, &{c_name})) goto st_{next};"),
                            );
                        }
                        CleanupScope::Budget { c_name } => {
                            self.line(
                                st,
                                format!("if (rt_budget_scope_exit_poll(ctx, &{c_name})) {{"),
                            );
                            if matches!(
                                self.fn_ret_ty,
                                Ty::ResultI32
                                    | Ty::ResultBytes
                                    | Ty::ResultBytesView
                                    | Ty::ResultResultBytes
                            ) {
                                self.line(
                                    st,
                                    format!(
                                        "  if ({c_name}.mode == RT_BUDGET_MODE_RESULT_ERR && {c_name}.violated) {{"
                                    ),
                                );
                                self.emit_overwrite_result_with_err(
                                    st,
                                    self.fn_ret_ty,
                                    "f->ret",
                                    &format!("{c_name}.err_code"),
                                );
                                self.line(st, "  }");
                            }
                            self.line(st, format!("  goto st_{next};"));
                            self.line(st, "}");
                        }
                        CleanupScope::Rr {
                            handle_c_name,
                            prev_c_name,
                        } => {
                            self.line(st, format!("ctx->rr_current = {prev_c_name};"));
                            self.line(st, format!("rt_rr_close_v1(ctx, {handle_c_name});"));
                            self.line(st, format!("goto st_{next};"));
                        }
                    }
                    self.line(st, format!("f->state = UINT32_C({resume});"));
                    self.line(st, "return UINT32_C(0);");
                    next = st;
                }
                self.line(cleanup_start, format!("goto st_{next};"));

                // `return` terminates control flow. Moves/sets performed while evaluating the return
                // expression must not affect the remaining compilation state.
                self.scopes = scopes_snapshot;
                self.task_scopes = task_scopes_snapshot;
                self.cleanup_scopes = cleanup_scopes_snapshot;
                Ok(())
            }

            fn emit_contract_entry_checks(
                &mut self,
                state: usize,
                cont: usize,
                requires: &[crate::x07ast::ContractClauseAst],
                invariant: &[crate::x07ast::ContractClauseAst],
            ) -> Result<(), CompilerError> {
                let mut clauses: Vec<(
                    &'static str,
                    ContractClauseKind,
                    usize,
                    &crate::x07ast::ContractClauseAst,
                )> = Vec::new();
                for (idx, c) in requires.iter().enumerate() {
                    clauses.push(("requires", ContractClauseKind::Requires, idx, c));
                }
                for (idx, c) in invariant.iter().enumerate() {
                    clauses.push(("invariant_entry", ContractClauseKind::Invariant, idx, c));
                }

                let mut next = cont;
                for (kind, id_kind, idx, clause) in clauses.into_iter().rev() {
                    let st = self.new_state();
                    self.emit_contract_clause_check(st, next, kind, id_kind, idx, clause)?;
                    next = st;
                }
                self.line(state, format!("goto st_{next};"));
                Ok(())
            }

            fn emit_contract_exit_checks(
                &mut self,
                state: usize,
                cont: usize,
                ensures: &[crate::x07ast::ContractClauseAst],
                invariant: &[crate::x07ast::ContractClauseAst],
            ) -> Result<(), CompilerError> {
                let mut clauses: Vec<(
                    &'static str,
                    ContractClauseKind,
                    usize,
                    &crate::x07ast::ContractClauseAst,
                )> = Vec::new();
                for (idx, c) in ensures.iter().enumerate() {
                    clauses.push(("ensures", ContractClauseKind::Ensures, idx, c));
                }
                for (idx, c) in invariant.iter().enumerate() {
                    clauses.push(("invariant_exit", ContractClauseKind::Invariant, idx, c));
                }

                let mut next = cont;
                for (kind, id_kind, idx, clause) in clauses.into_iter().rev() {
                    let st = self.new_state();
                    self.emit_contract_clause_check(st, next, kind, id_kind, idx, clause)?;
                    next = st;
                }
                self.line(state, format!("goto st_{next};"));
                Ok(())
            }

            fn emit_contract_clause_check(
                &mut self,
                state: usize,
                cont: usize,
                contract_kind: &str,
                id_kind: ContractClauseKind,
                clause_index: usize,
                clause: &crate::x07ast::ContractClauseAst,
            ) -> Result<(), CompilerError> {
                let clause_id = clause_id_or_hash(
                    &self.fn_name,
                    id_kind,
                    clause_index,
                    &clause.expr,
                    clause.id.as_deref(),
                );
                let clause_ptr = clause.expr.ptr().to_string();

                if self.options.contract_mode == ContractMode::VerifyBmc {
                    let msg = contract_payload_json_v1(
                        contract_kind,
                        &self.fn_name,
                        &clause_id,
                        clause_index,
                        &clause_ptr,
                    )?;
                    let msg_escaped = c_escape_c_string(&msg);

                    let cond = self.alloc_local("t_contract_cond_", Ty::I32)?;
                    let check_state = self.new_state();
                    self.emit_expr_entry(state, &clause.expr, cond.clone(), check_state)?;

                    if contract_kind == "requires" {
                        self.line(
                            check_state,
                            format!(
                                "__CPROVER_assume({} != UINT32_C(0)); goto st_{cont};",
                                cond.c_name
                            ),
                        );
                    } else {
                        self.line(
                            check_state,
                            format!(
                                "__CPROVER_assert({} != UINT32_C(0), \"{}\"); goto st_{cont};",
                                cond.c_name, msg_escaped
                            ),
                        );
                    }
                    return Ok(());
                }

                let budget = self.alloc_local("t_contract_budget_", Ty::BudgetScopeV1)?;
                self.line(state, format!(
                    "rt_budget_scope_init(ctx, &{}, RT_BUDGET_MODE_TRAP, (const uint8_t*)\"contract\", UINT32_C(8), UINT64_C({}), UINT64_C(0), UINT64_C(0), UINT64_C(0), UINT64_C(0), UINT64_C({}));",
                    budget.c_name, CONTRACT_ALLOC_BYTES, CONTRACT_FUEL,
                ));

                let cond = self.alloc_local("t_contract_cond_", Ty::I32)?;
                let check_state = self.new_state();
                self.emit_expr_entry(state, &clause.expr, cond.clone(), check_state)?;

                let fail_state = self.new_state();
                self.line(
                    check_state,
                    format!("if ({} == UINT32_C(0)) goto st_{fail_state};", cond.c_name),
                );
                self.line(
                    check_state,
                    format!("rt_budget_scope_exit_block(ctx, &{});", budget.c_name),
                );
                self.line(check_state, format!("goto st_{cont};"));

                let print_state = self.new_state();
                let mut witness_vars: Vec<AsyncVarRef> = Vec::new();

                if clause.witness.is_empty() {
                    self.line(fail_state, format!("goto st_{print_state};"));
                } else {
                    witness_vars = Vec::with_capacity(clause.witness.len());
                    for w in &clause.witness {
                        let tyinfo = self.infer_expr(w)?;
                        let mut v = self.alloc_local("t_contract_witness_", tyinfo.ty)?;
                        v.brand = tyinfo.brand;
                        witness_vars.push(v);
                    }

                    let mut cur_state = fail_state;
                    for (idx, (w_expr, w_var)) in
                        clause.witness.iter().zip(witness_vars.iter()).enumerate()
                    {
                        let next_state = if idx + 1 < clause.witness.len() {
                            self.new_state()
                        } else {
                            print_state
                        };
                        self.emit_expr_entry(cur_state, w_expr, w_var.clone(), next_state)?;
                        cur_state = next_state;
                    }
                }

                let witnesses: Vec<ContractWitnessC<'_>> = witness_vars
                    .iter()
                    .map(|w| ContractWitnessC {
                        ty: w.ty,
                        c_name: w.c_name.as_str(),
                    })
                    .collect();
                let fn_name = self.fn_name.clone();
                emit_contract_trap_payload_v1(
                    |s| self.line(print_state, s),
                    contract_kind,
                    &fn_name,
                    &clause_id,
                    clause_index,
                    &clause_ptr,
                    &witnesses,
                    CONTRACT_WITNESS_MAX_BYTES,
                )?;

                Ok(())
            }
        }

        let mut machine = Machine {
            options: self.options.clone(),
            functions,
            extern_functions: self.extern_functions.clone(),
            fn_c_names: self.fn_c_names.clone(),
            async_fn_new_names: self.async_fn_new_names.clone(),
            native_requires: BTreeMap::new(),
            fields,
            tmp_counter: 0,
            local_count: 0,
            unsafe_depth: 0,
            scopes: vec![BTreeMap::new()],
            task_scopes: Vec::new(),
            cleanup_scopes: Vec::new(),
            states: Vec::new(),
            ret_state: 0,
            fn_name: f.name.clone(),
            fn_ret_ty: f.ret_ty,
        };

        machine.bind(
            "input".to_string(),
            AsyncVarRef {
                ty: Ty::BytesView,
                brand: TyBrand::None,
                c_name: "f->input".to_string(),
                moved: false,
                moved_ptr: None,
            },
        );

        for (i, p) in f.params.iter().enumerate() {
            machine.bind(
                p.name.clone(),
                AsyncVarRef {
                    ty: p.ty,
                    brand: TyBrand::None,
                    c_name: format!("f->p{i}"),
                    moved: false,
                    moved_ptr: None,
                },
            );
        }

        let has_contracts =
            !(f.requires.is_empty() && f.ensures.is_empty() && f.invariant.is_empty());

        let out_state = if has_contracts {
            let entry_state = machine.new_state();
            let body_state = machine.new_state();
            let exit_state = machine.new_state();
            let out_state = machine.new_state();
            machine.ret_state = exit_state;

            machine.emit_contract_entry_checks(
                entry_state,
                body_state,
                &f.requires,
                &f.invariant,
            )?;

            machine.emit_expr_entry(
                body_state,
                &f.body,
                AsyncVarRef {
                    ty: f.ret_ty,
                    brand: TyBrand::None,
                    c_name: "f->ret".to_string(),
                    moved: false,
                    moved_ptr: None,
                },
                exit_state,
            )?;

            machine.push_scope();
            machine.bind(
                "__result".to_string(),
                AsyncVarRef {
                    ty: f.ret_ty,
                    brand: ty_brand_from_opt(&f.ret_brand),
                    c_name: "f->ret".to_string(),
                    moved: false,
                    moved_ptr: None,
                },
            );
            machine.emit_contract_exit_checks(exit_state, out_state, &f.ensures, &f.invariant)?;
            machine.pop_scope();
            out_state
        } else {
            let start = machine.new_state();
            let out_state = machine.new_state();
            machine.ret_state = out_state;

            machine.emit_expr_entry(
                start,
                &f.body,
                AsyncVarRef {
                    ty: f.ret_ty,
                    brand: TyBrand::None,
                    c_name: "f->ret".to_string(),
                    moved: false,
                    moved_ptr: None,
                },
                out_state,
            )?;
            out_state
        };

        for (backend_id, acc) in machine.native_requires.iter() {
            for feature in acc.features.iter() {
                self.require_native_backend(backend_id, acc.abi_major, feature)?;
            }
        }

        match f.ret_ty {
            Ty::Bytes => {
                machine.line(out_state, "out->kind = RT_TASK_OUT_KIND_BYTES;");
                machine.line(out_state, "out->payload.bytes = f->ret;");
                machine.line(out_state, "f->ret = rt_bytes_empty(ctx);");
            }
            Ty::ResultBytes => {
                machine.line(out_state, "out->kind = RT_TASK_OUT_KIND_RESULT_BYTES;");
                machine.line(out_state, "out->payload.result_bytes = f->ret;");
                machine.line(out_state, "f->ret.tag = UINT32_C(0);");
                machine.line(out_state, "f->ret.payload.err = UINT32_C(0);");
            }
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Internal,
                    format!(
                        "internal error: unsupported defasync return type: {:?}",
                        f.ret_ty
                    ),
                ));
            }
        }
        machine.line(out_state, "return UINT32_C(1);");

        self.line("typedef struct {");
        self.indent += 1;
        for (name, ty) in &machine.fields {
            self.line(&format!("{} {};", c_ret_ty(*ty), name));
        }
        self.indent -= 1;
        self.line(&format!("}} {fut_type};"));
        self.push_char('\n');

        self.line(&format!(
            "static uint32_t {poll_name}(ctx_t* ctx, void* fut, rt_task_out_t* out) {{"
        ));
        self.indent += 1;
        self.line(&format!("{fut_type}* f = ({fut_type}*)fut;"));
        self.line("switch (f->state) {");
        self.indent += 1;
        for i in 0..machine.states.len() {
            self.line(&format!("case UINT32_C({i}): goto st_{i};"));
        }
        self.line("default: rt_trap(\"bad async state\");");
        self.indent -= 1;
        self.line("}");
        for (i, lines) in machine.states.iter().enumerate() {
            self.line(&format!("st_{i}: ;"));
            for l in lines {
                self.line(l);
            }
        }
        self.indent -= 1;
        self.line("}");
        self.push_char('\n');

        self.line(&format!(
            "static void {drop_name}(ctx_t* ctx, void* fut) {{"
        ));
        self.indent += 1;
        self.line("if (!fut) return;");
        self.line(&format!("{fut_type}* f = ({fut_type}*)fut;"));
        for (name, ty) in &machine.fields {
            if !is_owned_ty(*ty) {
                continue;
            }
            let field = format!("f->{name}");
            self.emit_drop_var(*ty, &field);
        }
        self.line(&format!(
            "rt_free(ctx, f, (uint32_t)sizeof({fut_type}), (uint32_t)_Alignof({fut_type}));"
        ));
        self.indent -= 1;
        self.line("}");
        self.push_char('\n');

        self.line(&format!(
            "static uint32_t {new_name}(ctx_t* ctx, bytes_view_t input{}) {{",
            c_param_list_value(&f.params)
        ));
        self.indent += 1;
        self.line(&format!(
            "{fut_type}* f = ({fut_type}*)rt_alloc(ctx, (uint32_t)sizeof({fut_type}), (uint32_t)_Alignof({fut_type}));"
        ));
        self.line(&format!("memset(f, 0, sizeof({fut_type}));"));
        self.line("f->state = UINT32_C(0);");
        self.line("f->input = input;");
        for (i, _p) in f.params.iter().enumerate() {
            self.line(&format!("f->p{i} = p{i};"));
        }
        self.line(&format!(
            "return rt_task_create(ctx, {poll_name}, {drop_name}, f);"
        ));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn emit_async_call_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let f = self
            .program
            .async_functions
            .iter()
            .find(|f| f.name == head)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    format!("internal error: missing async function def for {head:?}"),
                )
            })?;

        if args.len() != f.params.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("call {:?} expects {} args", head, f.params.len()),
            ));
        }
        let expected_dest_ty = match f.ret_ty {
            Ty::Bytes => Ty::TaskHandleBytesV1,
            Ty::ResultBytes => Ty::TaskHandleResultBytesV1,
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Internal,
                    format!(
                        "internal error: invalid defasync return type: {:?}",
                        f.ret_ty
                    ),
                ));
            }
        };
        if dest_ty != expected_dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("async call {:?} returns {:?}", head, expected_dest_ty),
            ));
        }

        let mut rendered_args = Vec::with_capacity(args.len());
        let mut arg_vals = Vec::with_capacity(args.len());
        for (i, (arg_expr, param)) in args.iter().zip(f.params.iter()).enumerate() {
            let v = self.emit_expr(arg_expr)?;
            if v.ty != param.ty && !ty_compat_task_handle_as_i32(v.ty, param.ty) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("call {:?} arg {} expects {:?}", head, i, param.ty),
                ));
            }
            rendered_args.push(v.c_name.clone());
            arg_vals.push(v);
        }
        let c_args = if rendered_args.is_empty() {
            String::new()
        } else {
            format!(", {}", rendered_args.join(", "))
        };
        self.line(&format!(
            "{dest} = {}(ctx, input{c_args});",
            self.async_fn_new_c_name(head)
        ));
        for v in arg_vals {
            if is_owned_ty(v.ty) {
                self.line(&format!("{} = {};", v.c_name, c_empty(v.ty)));
            }
        }
        Ok(())
    }

    pub(super) fn emit_task_await_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "await expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "await returns bytes".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::TaskHandleBytesV1 && tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "await expects bytes task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_join_bytes_block(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_spawn_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.spawn expects 1 arg".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::TaskHandleBytesV1
            && tid.ty != Ty::TaskHandleResultBytesV1
            && tid.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.spawn expects task handle".to_string(),
            ));
        }
        if dest_ty != tid.ty && !ty_compat_task_handle_as_i32(tid.ty, dest_ty) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.spawn returns task handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_task_spawn(ctx, {});", tid.c_name));
        Ok(())
    }

    pub(super) fn emit_task_is_finished_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.is_finished expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.is_finished returns i32".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::TaskHandleBytesV1
            && tid.ty != Ty::TaskHandleResultBytesV1
            && tid.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.is_finished expects task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_is_finished(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_try_join_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.try_join.bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.try_join.bytes returns result_bytes".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::TaskHandleBytesV1 && tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.try_join.bytes expects bytes task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_try_join_bytes(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_try_join_result_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.try_join.result_bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.try_join.result_bytes returns result_result_bytes".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::TaskHandleResultBytesV1 && tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.try_join.result_bytes expects result_bytes task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_try_join_result_bytes(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_join_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.join.bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.join.bytes returns bytes".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::TaskHandleBytesV1 && tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.join.bytes expects bytes task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_join_bytes_block(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_join_result_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.join.result_bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.join.result_bytes returns result_bytes".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::TaskHandleResultBytesV1 && tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.join.result_bytes expects result_bytes task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_join_result_bytes_block(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_yield_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.yield expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.yield returns i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_task_yield_block(ctx);"));
        Ok(())
    }

    pub(super) fn emit_task_sleep_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.sleep expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.sleep returns i32".to_string(),
            ));
        }
        let ticks = self.emit_expr(&args[0])?;
        if ticks.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.sleep expects i32 ticks".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_sleep_block(ctx, {});",
            ticks.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_cancel_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.cancel expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.cancel returns i32".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::TaskHandleBytesV1
            && tid.ty != Ty::TaskHandleResultBytesV1
            && tid.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.cancel expects task handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_task_cancel(ctx, {});", tid.c_name));
        Ok(())
    }

    pub(super) fn emit_task_scope_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "task.scope_v1 is only allowed in solve or defasync".to_string(),
            ));
        }
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope_v1 expects 2 args".to_string(),
            ));
        }
        let cfg = parse_task_scope_cfg_v1(&args[0])?;

        let body_ty = self
            .infer_expr_in_new_scope_with_task_scope_depth(&args[1], self.task_scopes.len() + 1)?;
        if body_ty != dest_ty && body_ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("task.scope_v1 body must evaluate to {dest_ty:?} (or return)"),
            ));
        }

        let scope_name = self.alloc_local("t_scope_")?;
        self.decl_local(Ty::TaskScopeV1, &scope_name);
        self.line(&format!(
            "rt_scope_init(ctx, &{scope_name}, UINT32_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT32_C({}));",
            cfg.max_children,
            cfg.max_ticks,
            cfg.max_blocked_waits,
            cfg.max_join_polls,
            cfg.max_slot_result_bytes
        ));

        self.task_scopes.push(scope_name.clone());
        self.cleanup_scopes.push(CleanupScope::Task {
            c_name: scope_name.clone(),
        });
        self.emit_expr_to(&args[1], dest_ty, dest)?;
        let popped = self.task_scopes.pop();
        debug_assert_eq!(popped.as_deref(), Some(scope_name.as_str()));
        let popped_cleanup = self.cleanup_scopes.pop();
        debug_assert!(matches!(
            popped_cleanup,
            Some(CleanupScope::Task { c_name }) if c_name == scope_name
        ));

        self.line(&format!("rt_scope_exit_block(ctx, &{scope_name});"));
        Ok(())
    }

    pub(super) fn emit_task_scope_start_soon_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "task.scope.start_soon_v1 is only allowed in solve or defasync".to_string(),
            ));
        }
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_001: task.scope.start_soon_v1 used outside task.scope_v1".to_string(),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.start_soon_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.start_soon_v1 returns i32".to_string(),
            ));
        }

        let Expr::List { items: call, .. } = &args[0] else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_002: task.scope.start_soon_v1 expects an immediate defasync call expression"
                    .to_string(),
            ));
        };
        let head = call.first().and_then(Expr::as_ident).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_002: task.scope.start_soon_v1 expects an immediate defasync call expression"
                    .to_string(),
            )
        })?;
        let call_args = &call[1..];
        let async_f = self.program.async_functions.iter().find(|f| f.name == head).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_002: task.scope.start_soon_v1 expects an immediate defasync call expression"
                    .to_string(),
            )
        })?;

        let handle_ty = match async_f.ret_ty {
            Ty::Bytes => Ty::TaskHandleBytesV1,
            Ty::ResultBytes => Ty::TaskHandleResultBytesV1,
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Internal,
                    format!(
                        "internal error: unsupported defasync return type: {:?}",
                        async_f.ret_ty
                    ),
                ));
            }
        };
        let kind = match handle_ty {
            Ty::TaskHandleBytesV1 => "RT_TASK_OUT_KIND_BYTES",
            Ty::TaskHandleResultBytesV1 => "RT_TASK_OUT_KIND_RESULT_BYTES",
            _ => unreachable!(),
        };

        let task_id = self.alloc_local("t_task_")?;
        self.decl_local(handle_ty, &task_id);
        self.emit_async_call_to(head, call_args, handle_ty, &task_id)?;
        self.line(&format!("(void)rt_task_spawn(ctx, {task_id});"));
        self.line(&format!(
            "{dest} = rt_scope_start_soon(ctx, &{scope_name}, {task_id}, {kind});"
        ));
        Ok(())
    }

    pub(super) fn emit_task_scope_cancel_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "task.scope.cancel_all_v1 is only allowed in solve or defasync".to_string(),
            ));
        }
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_001: task.scope.cancel_all_v1 used outside task.scope_v1".to_string(),
            )
        })?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.cancel_all_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.cancel_all_v1 returns i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scope_cancel_all(ctx, &{scope_name});"
        ));
        Ok(())
    }

    pub(super) fn emit_task_scope_wait_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "task.scope.wait_all_v1 is only allowed in solve or defasync".to_string(),
            ));
        }
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_001: task.scope.wait_all_v1 used outside task.scope_v1".to_string(),
            )
        })?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.wait_all_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.wait_all_v1 returns i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_scope_wait_all_count(&{scope_name});"));
        self.line(&format!(
            "rt_scope_join_drop_remaining_block(ctx, &{scope_name});"
        ));
        self.line(&format!("rt_scope_reset_active(&{scope_name});"));
        Ok(())
    }

    pub(super) fn emit_task_scope_async_let_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
        expect_handle_ty: Ty,
        kind: &str,
    ) -> Result<(), CompilerError> {
        if !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{head} is only allowed in solve or defasync"),
            ));
        }
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                format!("X07E_SCOPE_SLOT_001: {head} used outside task.scope_v1"),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 1 arg"),
            ));
        }
        if dest_ty != Ty::TaskSlotV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns task_slot_v1"),
            ));
        }

        let Expr::List { items: call, .. } = &args[0] else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "X07E_SCOPE_SLOT_003: {head} expects an immediate defasync call expression"
                ),
            ));
        };
        let callee = call.first().and_then(Expr::as_ident).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "X07E_SCOPE_SLOT_003: {head} expects an immediate defasync call expression"
                ),
            )
        })?;
        let call_args = &call[1..];
        let async_f = self
            .program
            .async_functions
            .iter()
            .find(|f| f.name == callee)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "X07E_SCOPE_SLOT_003: {head} expects an immediate defasync call expression"
                    ),
                )
            })?;
        let got_handle_ty = match async_f.ret_ty {
            Ty::Bytes => Ty::TaskHandleBytesV1,
            Ty::ResultBytes => Ty::TaskHandleResultBytesV1,
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Internal,
                    format!(
                        "internal error: unsupported defasync return type: {:?}",
                        async_f.ret_ty
                    ),
                ));
            }
        };
        if got_handle_ty != expect_handle_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} defasync return type mismatch"),
            ));
        }

        let task_id = self.alloc_local("t_task_")?;
        self.decl_local(expect_handle_ty, &task_id);
        self.emit_async_call_to(callee, call_args, expect_handle_ty, &task_id)?;
        self.line(&format!("(void)rt_task_spawn(ctx, {task_id});"));
        self.line(&format!(
            "{dest} = rt_scope_async_let(ctx, &{scope_name}, {task_id}, {kind});"
        ));
        Ok(())
    }

    pub(super) fn emit_task_scope_await_slot_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "task.scope.await_slot_bytes_v1 is only allowed in solve or defasync".to_string(),
            ));
        }
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_SLOT_002: task.scope.await_slot_bytes_v1 used outside task.scope_v1"
                    .to_string(),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.await_slot_bytes_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.await_slot_bytes_v1 returns bytes".to_string(),
            ));
        }
        let slot_id = self.emit_expr(&args[0])?;
        if slot_id.ty != Ty::TaskSlotV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.await_slot_bytes_v1 expects task_slot_v1".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scope_await_slot_bytes_block(ctx, &{scope_name}, {});",
            slot_id.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_scope_await_slot_result_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "task.scope.await_slot_result_bytes_v1 is only allowed in solve or defasync"
                    .to_string(),
            ));
        }
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_SLOT_002: task.scope.await_slot_result_bytes_v1 used outside task.scope_v1".to_string(),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.await_slot_result_bytes_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.await_slot_result_bytes_v1 returns result_bytes".to_string(),
            ));
        }
        let slot_id = self.emit_expr(&args[0])?;
        if slot_id.ty != Ty::TaskSlotV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.await_slot_result_bytes_v1 expects task_slot_v1".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scope_await_slot_result_bytes_block(ctx, &{scope_name}, {});",
            slot_id.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_scope_try_await_slot_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_SLOT_002: task.scope.try_await_slot.bytes_v1 used outside task.scope_v1"
                    .to_string(),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.try_await_slot.bytes_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.try_await_slot.bytes_v1 returns result_bytes".to_string(),
            ));
        }
        let slot_id = self.emit_expr(&args[0])?;
        if slot_id.ty != Ty::TaskSlotV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.try_await_slot.bytes_v1 expects task_slot_v1".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scope_try_await_slot_bytes(ctx, &{scope_name}, {});",
            slot_id.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_scope_try_await_slot_result_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_SLOT_002: task.scope.try_await_slot.result_bytes_v1 used outside task.scope_v1"
                    .to_string(),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.try_await_slot.result_bytes_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.try_await_slot.result_bytes_v1 returns result_result_bytes".to_string(),
            ));
        }
        let slot_id = self.emit_expr(&args[0])?;
        if slot_id.ty != Ty::TaskSlotV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.try_await_slot.result_bytes_v1 expects task_slot_v1".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scope_try_await_slot_result_bytes(ctx, &{scope_name}, {});",
            slot_id.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_scope_slot_is_finished_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_SLOT_002: task.scope.slot_is_finished_v1 used outside task.scope_v1"
                    .to_string(),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.slot_is_finished_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.slot_is_finished_v1 returns i32".to_string(),
            ));
        }
        let slot_id = self.emit_expr(&args[0])?;
        if slot_id.ty != Ty::TaskSlotV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.slot_is_finished_v1 expects task_slot_v1".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scope_slot_is_finished(ctx, &{scope_name}, {});",
            slot_id.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_scope_slot_to_i32_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let _scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.slot_to_i32_v1 used outside task.scope_v1".to_string(),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.slot_to_i32_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.slot_to_i32_v1 returns i32".to_string(),
            ));
        }
        let slot = self.emit_expr(&args[0])?;
        if slot.ty != Ty::TaskSlotV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.slot_to_i32_v1 expects task_slot_v1".to_string(),
            ));
        }
        self.line(&format!("{dest} = {};", slot.c_name));
        Ok(())
    }

    pub(super) fn emit_task_scope_slot_from_i32_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let _scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.slot_from_i32_v1 used outside task.scope_v1".to_string(),
            )
        })?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.scope.slot_from_i32_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::TaskSlotV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.slot_from_i32_v1 returns task_slot_v1".to_string(),
            ));
        }
        let slot = self.emit_expr(&args[0])?;
        if slot.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.scope.slot_from_i32_v1 expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = {};", slot.c_name));
        Ok(())
    }

    pub(super) fn emit_task_scope_select_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("{head} is only allowed in solve or defasync"),
            ));
        }
        let scope_name = self.task_scopes.last().cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SELECT_OUTSIDE_SCOPE: task.scope.select used outside task.scope_v1"
                    .to_string(),
            )
        })?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        let cfg = parse_task_select_cfg_v1(&args[0])?;
        let cases = parse_task_select_cases_v1(&args[1])?;
        if cases.len() > cfg.max_cases as usize {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SELECT_TOO_MANY_CASES: too many cases".to_string(),
            ));
        }

        let is_try = head == "task.scope.select_try_v1";
        match (is_try, dest_ty) {
            (false, Ty::TaskSelectEvtV1) => {}
            (true, Ty::OptionTaskSelectEvtV1) => {}
            (false, _) => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "task.scope.select_v1 returns task_select_evt_v1".to_string(),
                ));
            }
            (true, _) => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "task.scope.select_try_v1 returns option_i32".to_string(),
                ));
            }
        }

        let res_var = self.alloc_local("t_select_res_")?;
        self.decl_local(Ty::ResultBytes, &res_var);

        let polls_var = if is_try {
            None
        } else {
            let v = self.alloc_local("t_select_polls_")?;
            self.decl_local(Ty::I32, &v);
            Some(v)
        };
        let slept_var = if is_try || cfg.timeout_ticks == 0 {
            None
        } else {
            let v = self.alloc_local("t_select_slept_")?;
            self.decl_local(Ty::I32, &v);
            Some(v)
        };

        let rr_start_var = if cfg.policy == TaskSelectPolicyV1::RrV1 {
            let v = self.alloc_local("t_select_rr_")?;
            self.decl_local(Ty::I32, &v);
            Some(v)
        } else {
            None
        };

        let done_label = self.alloc_local("lbl_select_done_")?;
        let loop_label = self.alloc_local("lbl_select_loop_")?;

        // Initialize per-call state.
        if let Some(polls) = &polls_var {
            self.line(&format!("{polls} = UINT32_C(0);"));
        }
        if let Some(slept) = &slept_var {
            self.line(&format!("{slept} = UINT32_C(0);"));
        }

        self.line(&format!("{loop_label}: ;"));
        if let Some(rr) = &rr_start_var {
            self.line(&format!("{rr} = {scope_name}.select_rr_cursor;"));
            if !cases.is_empty() {
                self.line(&format!(
                    "if ({rr} >= UINT32_C({})) {rr} = UINT32_C(0);",
                    cases.len()
                ));
            }
        }

        let emit_case_check = |this: &mut Self,
                               idx: usize,
                               case: &TaskSelectCaseV1|
         -> Result<(), CompilerError> {
            match case {
                TaskSelectCaseV1::SlotBytes { slot } => {
                    let slot_id = this.emit_expr(slot)?;
                    if slot_id.ty != Ty::TaskSlotV1 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "task.scope.select slot case expects task_slot_v1".to_string(),
                        ));
                    }
                    this.line(&format!("if ({} != UINT32_MAX) {{", slot_id.c_name));
                    this.indent += 1;
                    this.line(&format!(
                        "{res_var} = rt_scope_try_await_slot_bytes(ctx, &{scope_name}, {});",
                        slot_id.c_name
                    ));
                    this.line(&format!("if ({res_var}.tag) {{"));
                    this.indent += 1;
                    if !is_try {
                        this.line(&format!(
                            "{dest} = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(1), UINT32_C({idx}), {}, {res_var}.payload.ok);",
                            slot_id.c_name
                        ));
                    } else {
                        this.line(&format!("{dest}.tag = UINT32_C(1);"));
                        this.line(&format!(
                            "{dest}.payload = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(1), UINT32_C({idx}), {}, {res_var}.payload.ok);",
                            slot_id.c_name
                        ));
                    }
                    // Prevent double-free of payload moved into the select event.
                    this.line(&format!("{res_var}.tag = UINT32_C(0);"));
                    this.line(&format!("{res_var}.payload.err = UINT32_C(0);"));
                    if cfg.policy == TaskSelectPolicyV1::RrV1 && !cases.is_empty() {
                        let next = (idx + 1) % cases.len();
                        this.line(&format!(
                            "{scope_name}.select_rr_cursor = UINT32_C({next});"
                        ));
                    }
                    this.line(&format!("goto {done_label};"));
                    this.indent -= 1;
                    this.line("}");
                    this.line(&format!("if ({res_var}.payload.err == UINT32_C(2)) {{"));
                    this.indent += 1;
                    if !is_try {
                        this.line(&format!(
                            "{dest} = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(2), UINT32_C({idx}), {}, rt_bytes_empty(ctx));",
                            slot_id.c_name
                        ));
                    } else {
                        this.line(&format!("{dest}.tag = UINT32_C(1);"));
                        this.line(&format!(
                            "{dest}.payload = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(2), UINT32_C({idx}), {}, rt_bytes_empty(ctx));",
                            slot_id.c_name
                        ));
                    }
                    if cfg.policy == TaskSelectPolicyV1::RrV1 && !cases.is_empty() {
                        let next = (idx + 1) % cases.len();
                        this.line(&format!(
                            "{scope_name}.select_rr_cursor = UINT32_C({next});"
                        ));
                    }
                    this.line(&format!("goto {done_label};"));
                    this.indent -= 1;
                    this.line("}");
                    this.indent -= 1;
                    this.line("}");
                }
                TaskSelectCaseV1::ChanRecvBytes { chan } => {
                    let chan_id = this.emit_expr(chan)?;
                    if chan_id.ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "task.scope.select chan.recv case expects i32 chan handle".to_string(),
                        ));
                    }
                    this.line(&format!("if ({} != UINT32_C(0)) {{", chan_id.c_name));
                    this.indent += 1;
                    this.line(&format!(
                        "{res_var} = rt_chan_bytes_try_recv(ctx, {});",
                        chan_id.c_name
                    ));
                    this.line(&format!("if ({res_var}.tag) {{"));
                    this.indent += 1;
                    if !is_try {
                        this.line(&format!(
                            "{dest} = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(3), UINT32_C({idx}), {}, {res_var}.payload.ok);",
                            chan_id.c_name
                        ));
                    } else {
                        this.line(&format!("{dest}.tag = UINT32_C(1);"));
                        this.line(&format!(
                            "{dest}.payload = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(3), UINT32_C({idx}), {}, {res_var}.payload.ok);",
                            chan_id.c_name
                        ));
                    }
                    this.line(&format!("{res_var}.tag = UINT32_C(0);"));
                    this.line(&format!("{res_var}.payload.err = UINT32_C(0);"));
                    if cfg.policy == TaskSelectPolicyV1::RrV1 && !cases.is_empty() {
                        let next = (idx + 1) % cases.len();
                        this.line(&format!(
                            "{scope_name}.select_rr_cursor = UINT32_C({next});"
                        ));
                    }
                    this.line(&format!("goto {done_label};"));
                    this.indent -= 1;
                    this.line("}");
                    this.line(&format!("if ({res_var}.payload.err == UINT32_C(2)) {{"));
                    this.indent += 1;
                    if !is_try {
                        this.line(&format!(
                            "{dest} = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(4), UINT32_C({idx}), {}, rt_bytes_empty(ctx));",
                            chan_id.c_name
                        ));
                    } else {
                        this.line(&format!("{dest}.tag = UINT32_C(1);"));
                        this.line(&format!(
                            "{dest}.payload = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(4), UINT32_C({idx}), {}, rt_bytes_empty(ctx));",
                            chan_id.c_name
                        ));
                    }
                    if cfg.policy == TaskSelectPolicyV1::RrV1 && !cases.is_empty() {
                        let next = (idx + 1) % cases.len();
                        this.line(&format!(
                            "{scope_name}.select_rr_cursor = UINT32_C({next});"
                        ));
                    }
                    this.line(&format!("goto {done_label};"));
                    this.indent -= 1;
                    this.line("}");
                    this.indent -= 1;
                    this.line("}");
                }
            }
            Ok(())
        };

        let rr_start = rr_start_var.as_deref();

        // First pass (rr: indices >= rr_start; priority: all).
        for (idx, case) in cases.iter().enumerate() {
            if let Some(rr) = rr_start {
                self.line(&format!("if (UINT32_C({idx}) >= {rr}) {{"));
                self.indent += 1;
                emit_case_check(self, idx, case)?;
                self.indent -= 1;
                self.line("}");
            } else {
                emit_case_check(self, idx, case)?;
            }
        }
        // Second pass for rr (indices < rr_start).
        if let Some(rr) = rr_start {
            for (idx, case) in cases.iter().enumerate() {
                self.line(&format!("if (UINT32_C({idx}) < {rr}) {{"));
                self.indent += 1;
                emit_case_check(self, idx, case)?;
                self.indent -= 1;
                self.line("}");
            }
        }

        if is_try {
            self.line(&format!("{dest}.tag = UINT32_C(0);"));
            self.line(&format!("{dest}.payload = UINT32_C(0);"));
            self.line(&format!("goto {done_label};"));
            self.line(&format!("{done_label}: ;"));
            return Ok(());
        }

        // Nothing ready: apply blocking policy and retry.
        if let Some(polls) = &polls_var {
            self.line(&format!("{polls} = {polls} + UINT32_C(1);"));
            if cfg.max_polls != 0 {
                self.line(&format!(
                    "if ({polls} > UINT32_C({})) rt_trap(\"X07T_SELECT_MAX_POLLS\");",
                    cfg.max_polls
                ));
            }
        }
        if cfg.timeout_ticks != 0 {
            let slept = slept_var.as_deref().unwrap();
            self.line(&format!(
                "if ({slept} >= UINT32_C({})) {{ {dest} = rt_select_evt_new(ctx, {scope_name}.key, UINT32_C(5), UINT32_C(0), UINT32_C(0), rt_bytes_empty(ctx)); goto {done_label}; }}",
                cfg.timeout_ticks
            ));
        }
        if cfg.poll_sleep_ticks != 0 {
            self.line(&format!(
                "(void)rt_task_sleep_block(ctx, UINT32_C({}));",
                cfg.poll_sleep_ticks
            ));
            if let Some(slept) = slept_var.as_deref() {
                self.line(&format!(
                    "{slept} = {slept} + UINT32_C({});",
                    cfg.poll_sleep_ticks
                ));
            }
        } else {
            self.line("(void)rt_task_yield_block(ctx);");
        }
        self.line(&format!("goto {loop_label};"));

        self.line(&format!("{done_label}: ;"));
        Ok(())
    }

    pub(super) fn emit_task_select_evt_tag_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.select_evt.tag_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.tag_v1 returns i32".to_string(),
            ));
        }
        let evt = self.emit_expr(&args[0])?;
        if evt.ty != Ty::TaskSelectEvtV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.tag_v1 expects task_select_evt_v1".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_select_evt_tag(ctx, {});", evt.c_name));
        Ok(())
    }

    pub(super) fn emit_task_select_evt_case_index_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.select_evt.case_index_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.case_index_v1 returns i32".to_string(),
            ));
        }
        let evt = self.emit_expr(&args[0])?;
        if evt.ty != Ty::TaskSelectEvtV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.case_index_v1 expects task_select_evt_v1".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_select_evt_case_index(ctx, {});",
            evt.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_select_evt_src_id_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.select_evt.src_id_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.src_id_v1 returns i32".to_string(),
            ));
        }
        let evt = self.emit_expr(&args[0])?;
        if evt.ty != Ty::TaskSelectEvtV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.src_id_v1 expects task_select_evt_v1".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_select_evt_src_id(ctx, {});",
            evt.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_select_evt_take_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.select_evt.take_bytes_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.take_bytes_v1 returns bytes".to_string(),
            ));
        }
        let evt = self.emit_expr(&args[0])?;
        if evt.ty != Ty::TaskSelectEvtV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.take_bytes_v1 expects task_select_evt_v1".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_select_evt_take_bytes(ctx, {});",
            evt.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_task_select_evt_drop_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.select_evt.drop_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.drop_v1 returns i32".to_string(),
            ));
        }
        let evt = self.emit_expr(&args[0])?;
        if evt.ty != Ty::TaskSelectEvtV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.select_evt.drop_v1 expects task_select_evt_v1".to_string(),
            ));
        }
        self.line(&format!("rt_select_evt_drop(ctx, {});", evt.c_name));
        self.line(&format!("{dest} = UINT32_C(1);"));
        Ok(())
    }

    pub(super) fn emit_chan_bytes_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.new expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.new returns i32".to_string(),
            ));
        }
        let cap = self.emit_expr(&args[0])?;
        if cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.new expects i32 cap".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_chan_bytes_new(ctx, {});", cap.c_name));
        Ok(())
    }

    pub(super) fn emit_chan_bytes_try_send_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.try_send expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.try_send returns i32".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        let msg = self.emit_expr_as_bytes_view(&args[1])?;
        if chan.ty != Ty::I32 || msg.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.try_send expects (i32, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_try_send_view(ctx, {}, {});",
            chan.c_name, msg.c_name
        ));
        self.release_temp_view_borrow(&msg)?;
        Ok(())
    }

    pub(super) fn emit_chan_bytes_send_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.send expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.send returns i32".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        let msg = self.emit_expr(&args[1])?;
        if chan.ty != Ty::I32 || msg.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.send expects (i32, bytes)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_send_block(ctx, {}, {});",
            chan.c_name, msg.c_name
        ));
        self.line(&format!("{} = {};", msg.c_name, c_empty(Ty::Bytes)));
        Ok(())
    }

    pub(super) fn emit_chan_bytes_try_recv_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.try_recv expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.try_recv returns result_bytes".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        if chan.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.try_recv expects i32 chan handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_try_recv(ctx, {});",
            chan.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_chan_bytes_recv_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.recv expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.recv returns bytes".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        if chan.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.recv expects i32 chan handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_recv_block(ctx, {});",
            chan.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_chan_bytes_close_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.close expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.close returns i32".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        if chan.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.close expects i32 chan handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_close(ctx, {});",
            chan.c_name
        ));
        Ok(())
    }
}
