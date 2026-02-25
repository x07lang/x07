use super::c_emit_async::{
    parse_task_scope_cfg_v1, parse_task_select_cases_v1, parse_task_select_cfg_v1, TaskSelectCaseV1,
};
use super::c_emit_worlds::{
    load_budget_profile_cfg_from_arch_v1, load_rr_cfg_v1_from_arch_v1, parse_budget_scope_cfg_v1,
    parse_bytes_lit_ascii, parse_i32_lit, BudgetScopeModeV1,
};
use super::*;

#[derive(Debug, Clone)]
pub(super) struct FnSig {
    pub(super) ret: TyInfo,
    pub(super) params: Vec<TyInfo>,
}

pub(super) struct InferCtx {
    pub(super) options: CompileOptions,
    pub(super) fn_ret_ty: TyInfo,
    pub(super) allow_async_ops: bool,
    pub(super) unsafe_depth: usize,
    pub(super) task_scope_depth: usize,
    pub(super) scopes: Vec<BTreeMap<String, TyInfo>>,
    pub(super) functions: BTreeMap<String, FnSig>,
    pub(super) extern_functions: BTreeMap<String, ExternFunctionDecl>,
}

impl InferCtx {
    fn require_standalone_only(&self, head: &str) -> Result<(), CompilerError> {
        if !self.options.world.is_standalone_only() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{head} is standalone-only; compile with --world run-os or --world run-os-sandboxed"
                ),
            ));
        }
        Ok(())
    }

    fn require_unsafe_world(&self, head: &str) -> Result<(), CompilerError> {
        if !self.options.allow_unsafe() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{head} requires unsafe capability; {}",
                    self.options.hint_enable_unsafe()
                ),
            ));
        }
        Ok(())
    }

    fn require_ffi_world(&self, head: &str) -> Result<(), CompilerError> {
        if !self.options.allow_ffi() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{head} requires ffi capability; {}",
                    self.options.hint_enable_ffi()
                ),
            ));
        }
        Ok(())
    }

    fn require_unsafe_block(&self, head: &str) -> Result<(), CompilerError> {
        if self.unsafe_depth == 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unsafe-required: {head}"),
            ));
        }
        Ok(())
    }

    fn require_brand_validator_v1(&self, validator_symbol: &str) -> Result<(), CompilerError> {
        let Some(sig) = self.functions.get(validator_symbol) else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {validator_symbol:?}"),
            ));
        };
        if sig.params.len() != 1
            || sig.params[0].ty != Ty::BytesView
            || !sig.params[0].brand.is_none()
            || sig.ret.ty != Ty::ResultI32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "validator must have signature (bytes_view)->result_i32: {validator_symbol:?}"
                ),
            ));
        }
        Ok(())
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind(&mut self, name: String, ty: TyInfo) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn lookup(&self, name: &str) -> Option<TyInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    fn infer_immediate_defasync_call_expr(&mut self, expr: &Expr) -> Result<TyInfo, CompilerError> {
        let Expr::List { items, .. } = expr else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_002: expected an immediate defasync call expression".to_string(),
            ));
        };
        let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_002: expected an immediate defasync call expression".to_string(),
            )
        })?;
        let args = &items[1..];

        let Some(sig) = self.functions.get(head).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {head:?}"),
            ));
        };
        if sig.ret.ty != Ty::TaskHandleBytesV1 && sig.ret.ty != Ty::TaskHandleResultBytesV1 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "X07E_SCOPE_002: expected an immediate defasync call expression".to_string(),
            ));
        }
        if args.len() != sig.params.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("call {head:?} expects {} args", sig.params.len()),
            ));
        }
        for (i, (arg, want)) in args.iter().zip(sig.params.iter()).enumerate() {
            let got = self.infer(arg)?;
            let ok = tyinfo_compat_call_arg(&got, want);
            if !ok {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    call_arg_mismatch_message(head, i, &got, want),
                ));
            }
        }
        Ok(sig.ret)
    }

    pub(super) fn infer(&mut self, expr: &Expr) -> Result<TyInfo, CompilerError> {
        match expr {
            Expr::Int { .. } => Ok(TyInfo::unbranded(Ty::I32)),
            Expr::Ident { name, .. } => {
                if name == "input" {
                    return Ok(TyInfo::unbranded(Ty::BytesView));
                }
                self.lookup(name).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    )
                })
            }
            Expr::List { items, .. } => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "begin" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "(begin ...) requires at least 1 expression".to_string(),
                            ));
                        }
                        self.push_scope();
                        for e in &args[..args.len() - 1] {
                            self.infer_stmt(e)?;
                        }
                        let ty = self.infer(&args[args.len() - 1])?;
                        self.pop_scope();
                        Ok(ty)
                    }
                    "unsafe" => {
                        self.require_unsafe_world("unsafe")?;
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "(unsafe ...) requires at least 1 expression".to_string(),
                            ));
                        }
                        self.unsafe_depth = self.unsafe_depth.saturating_add(1);
                        self.push_scope();
                        for e in &args[..args.len() - 1] {
                            self.infer_stmt(e)?;
                        }
                        let ty = self.infer(&args[args.len() - 1])?;
                        self.pop_scope();
                        self.unsafe_depth = self.unsafe_depth.saturating_sub(1);
                        Ok(ty)
                    }
                    "let" => {
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
                        if self.task_scope_depth != 0 {
                            if let Expr::List { items: call_items, .. } = &args[1] {
                                if let Some(call_head) =
                                    call_items.first().and_then(Expr::as_ident)
                                {
                                    if matches!(
                                        self.functions.get(call_head).map(|s| s.ret.ty),
                                        Some(Ty::TaskHandleBytesV1 | Ty::TaskHandleResultBytesV1)
                                    ) {
                                        return Err(CompilerError::new(
                                            CompileErrorKind::Typing,
                                            "X07E_SCOPE_003: illegal spawn pattern inside task.scope_v1 (use task.scope.start_soon_v1 or task.scope.async_let_*_v1)"
                                                .to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                        let ty = self.infer(&args[1])?;
                        self.bind(name.to_string(), ty.clone());
                        Ok(ty)
                    }
                    "set" => {
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
                        let prev = self.lookup(name).ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("set of unknown variable: {name:?}"),
                            )
                        })?;
                        let ty = self.infer(&args[1])?;
                        if !tyinfo_compat_assign(&ty, &prev) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("type mismatch in set for variable {name:?}"),
                            ));
                        }
                        Ok(prev)
                    }
                    "set0" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "set0 form: (set0 <name> <expr>)".to_string(),
                            ));
                        }
                        let name = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "set0 name must be an identifier".to_string(),
                            )
                        })?;
                        let prev = self.lookup(name).ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("set0 of unknown variable: {name:?}"),
                            )
                        })?;
                        let ty = self.infer(&args[1])?;
                        if !tyinfo_compat_assign(&ty, &prev) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("type mismatch in set0 for variable {name:?}"),
                            ));
                        }
                        Ok(TyInfo::unbranded(Ty::I32))
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "if condition must be i32".to_string(),
                            ));
                        }
                        self.push_scope();
                        let then_ty = self.infer(&args[1])?;
                        self.pop_scope();

                        self.push_scope();
                        let else_ty = self.infer(&args[2])?;
                        self.pop_scope();

                        if then_ty != Ty::Never && else_ty != Ty::Never && then_ty.ty != else_ty.ty
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "if branches must have same type (then={then_ty:?}, else={else_ty:?})"
                                ),
                            ));
                        }
                        Ok(if then_ty == Ty::Never {
                            else_ty
                        } else if else_ty == Ty::Never {
                            then_ty
                        } else {
                            TyInfo {
                                ty: then_ty.ty,
                                brand: tybrand_join(then_ty.ty, &then_ty.brand, &else_ty.brand),
                                view_full: then_ty.ty == Ty::BytesView
                                    && then_ty.view_full
                                    && else_ty.view_full,
                            }
                        })
                    }
                    "for" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "for form: (for <i> <start:i32> <end:i32> <body:any>)".to_string(),
                            ));
                        }
                        let var = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "for variable must be an identifier".to_string(),
                            )
                        })?;
                        match self.lookup(var) {
                            Some(v) if v.ty == Ty::I32 => {}
                            Some(_) => {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("for variable must be i32: {var:?}"),
                                ));
                            }
                            None => {
                                self.bind(var.to_string(), Ty::I32.into());
                            }
                        }
                        if self.infer(&args[1])? != Ty::I32 || self.infer(&args[2])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "for bounds must be i32".to_string(),
                            ));
                        }
                        self.push_scope();
                        self.infer_stmt(&args[3])?;
                        self.pop_scope();
                        Ok(Ty::I32.into())
                    }
                    "return" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "return form: (return <expr>)".to_string(),
                            ));
                        }
                        let got = self.infer(&args[0])?;
                        if !tyinfo_compat_assign(&got, &self.fn_ret_ty) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("return expression must evaluate to {:?}", self.fn_ret_ty),
                            ));
                        }
                        Ok(Ty::Never.into())
                    }
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<u" | ">>u" | "=" | "!="
                    | "<" | "<=" | ">" | ">=" | "<u" | ">=u" | ">u" | "<=u" | "&&" | "||" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 args"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "bytes.as_ptr" | "bytes.as_mut_ptr" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes"),
                            ));
                        }
                        Ok((if head == "bytes.as_ptr" {
                            Ty::PtrConstU8
                        } else {
                            Ty::PtrMutU8
                        })
                        .into())
                    }
                    "view.as_ptr" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.as_ptr expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.as_ptr expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::PtrConstU8.into())
                    }
                    "vec_u8.as_ptr" | "vec_u8.as_mut_ptr" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects vec_u8"),
                            ));
                        }
                        Ok((if head == "vec_u8.as_ptr" {
                            Ty::PtrConstU8
                        } else {
                            Ty::PtrMutU8
                        })
                        .into())
                    }
                    "ptr.null" => {
                        self.require_unsafe_world(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.null expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::PtrMutVoid.into())
                    }
                    "ptr.as_const" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.as_const expects 1 arg".to_string(),
                            ));
                        }
                        let ty = self.infer(&args[0])?;
                        Ok(match ty.ty {
                            Ty::PtrMutU8 => Ty::PtrConstU8,
                            Ty::PtrMutVoid => Ty::PtrConstVoid,
                            Ty::PtrMutI32 => Ty::PtrConstI32,
                            Ty::PtrConstU8 | Ty::PtrConstVoid | Ty::PtrConstI32 => ty.ty,
                            _ => {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "ptr.as_const expects a raw pointer".to_string(),
                                ));
                            }
                        }
                        .into())
                    }
                    "ptr.cast" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.cast form: (ptr.cast <ptr_ty> <ptr>)".to_string(),
                            ));
                        }
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
                        if !target.is_ptr_ty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("ptr.cast target must be a pointer type, got {target:?}"),
                            ));
                        }
                        let src = self.infer(&args[1])?;
                        if !src.is_ptr_ty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("ptr.cast expects a pointer, got {src:?}"),
                            ));
                        }
                        Ok(target.into())
                    }
                    "addr_of" | "addr_of_mut" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        let name = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects an identifier"),
                            )
                        })?;
                        if name != "input" && self.lookup(name).is_none() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        }
                        Ok((if head == "addr_of" {
                            Ty::PtrConstVoid
                        } else {
                            Ty::PtrMutVoid
                        })
                        .into())
                    }
                    "ptr.add" | "ptr.sub" | "ptr.offset" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        let ptr_ty = self.infer(&args[0])?;
                        if !matches!(
                            ptr_ty.ty,
                            Ty::PtrConstU8 | Ty::PtrMutU8 | Ty::PtrConstI32 | Ty::PtrMutI32
                        ) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects a non-void raw pointer"),
                            ));
                        }
                        if self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 offset"),
                            ));
                        }
                        Ok(ptr_ty)
                    }
                    "ptr.read_u8" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.read_u8 expects 1 arg".to_string(),
                            ));
                        }
                        let ptr_ty = self.infer(&args[0])?;
                        if !matches!(ptr_ty.ty, Ty::PtrConstU8 | Ty::PtrMutU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.read_u8 expects ptr_const_u8 or ptr_mut_u8".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "ptr.write_u8" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.write_u8 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::PtrMutU8 || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.write_u8 expects (ptr_mut_u8, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "ptr.read_i32" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.read_i32 expects 1 arg".to_string(),
                            ));
                        }
                        let ptr_ty = self.infer(&args[0])?;
                        if !matches!(ptr_ty.ty, Ty::PtrConstI32 | Ty::PtrMutI32) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.read_i32 expects ptr_const_i32 or ptr_mut_i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "ptr.write_i32" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.write_i32 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::PtrMutI32
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.write_i32 expects (ptr_mut_i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "memcpy" | "memmove" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 3 args"),
                            ));
                        }
                        let dest_ptr = self.infer(&args[0])?;
                        let src_ptr = self.infer(&args[1])?;
                        if dest_ptr != Ty::PtrMutVoid {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects dest ptr_mut_void"),
                            ));
                        }
                        if src_ptr != Ty::PtrConstVoid && src_ptr != Ty::PtrMutVoid {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects src ptr_const_void"),
                            ));
                        }
                        if self.infer(&args[2])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 len"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "memset" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "memset expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::PtrMutVoid
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "memset expects (ptr_mut_void, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "bytes.len" | "bytes.get_u8" | "bytes.eq" | "bytes.cmp_range" => {
                        let (want_args, want_ty) = match head {
                            "bytes.len" => (1, Ty::I32),
                            "bytes.get_u8" => (2, Ty::I32),
                            "bytes.eq" => (2, Ty::I32),
                            "bytes.cmp_range" => (6, Ty::I32),
                            _ => unreachable!(),
                        };
                        if args.len() != want_args {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects {want_args} args"),
                            ));
                        }
                        match head {
                            "bytes.len" => {
                                let b = self.infer(&args[0])?;
                                if b != Ty::Bytes && b != Ty::BytesView {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("{head} expects bytes_view"),
                                    ));
                                }
                                Ok(want_ty.into())
                            }
                            "bytes.get_u8" => {
                                let b = self.infer(&args[0])?;
                                if (b != Ty::Bytes && b != Ty::BytesView)
                                    || self.infer(&args[1])? != Ty::I32
                                {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "bytes.get_u8 expects (bytes_view, i32)".to_string(),
                                    ));
                                }
                                Ok(want_ty.into())
                            }
                            "bytes.eq" => {
                                let a = self.infer(&args[0])?;
                                let b = self.infer(&args[1])?;
                                if (a != Ty::Bytes && a != Ty::BytesView)
                                    || (b != Ty::Bytes && b != Ty::BytesView)
                                {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("{head} expects (bytes_view, bytes_view)"),
                                    ));
                                }
                                Ok(want_ty.into())
                            }
                            "bytes.cmp_range" => {
                                let a = self.infer(&args[0])?;
                                let b = self.infer(&args[3])?;
                                if (a != Ty::Bytes && a != Ty::BytesView)
                                    || self.infer(&args[1])? != Ty::I32
                                    || self.infer(&args[2])? != Ty::I32
                                    || (b != Ty::Bytes && b != Ty::BytesView)
                                    || self.infer(&args[4])? != Ty::I32
                                    || self.infer(&args[5])? != Ty::I32
                                {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "bytes.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)"
                                            .to_string(),
                                    ));
                                }
                                Ok(want_ty.into())
                            }
                            _ => unreachable!(),
                        }
                    }
                    "bytes.set_u8" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.set_u8 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "math.f64.add_v1" | "math.f64.sub_v1" | "math.f64.mul_v1"
                    | "math.f64.div_v1" | "math.f64.pow_v1" | "math.f64.atan2_v1"
                    | "math.f64.min_v1" | "math.f64.max_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        let a = self.infer(&args[0])?;
                        let b = self.infer(&args[1])?;
                        if (a != Ty::Bytes && a != Ty::BytesView)
                            || (b != Ty::Bytes && b != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (bytes_view, bytes_view)"),
                            ));
                        }
                        Ok(Ty::Bytes.into())
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
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        let x = self.infer(&args[0])?;
                        if x != Ty::Bytes && x != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes_view"),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "math.f64.parse_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "math.f64.parse_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let s = self.infer(&args[0])?;
                        if s != Ty::Bytes && s != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.parse_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "json.jcs.canon_doc_v1" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "json.jcs.canon_doc_v1 expects 4 args".to_string(),
                            ));
                        }
                        let input = self.infer(&args[0])?;
                        if input != Ty::Bytes && input != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "json.jcs.canon_doc_v1 expects bytes_view".to_string(),
                            ));
                        }
                        if self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                            || self.infer(&args[3])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "json.jcs.canon_doc_v1 expects (bytes_view, i32, i32, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "math.f64.from_i32_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "math.f64.from_i32_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.from_i32_v1 expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "math.f64.to_i32_trunc_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "math.f64.to_i32_trunc_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let x = self.infer(&args[0])?;
                        if x != Ty::Bytes && x != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.to_i32_trunc_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "bytes.alloc" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.alloc expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.alloc length must be i32".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "bytes.empty" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.empty expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "bytes1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes1 expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "bytes.lit" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.lit expects 1 arg".to_string(),
                            ));
                        }
                        if args[0].as_ident().is_none() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.lit expects a text string".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "bytes.view_lit" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.view_lit expects 1 arg".to_string(),
                            ));
                        }
                        if args[0].as_ident().is_none() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.view_lit expects a text string".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::BytesView,
                            brand: TyBrand::None,
                            view_full: true,
                        })
                    }
                    "bytes.copy" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.copy expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.copy expects (bytes, bytes)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "bytes.concat" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.concat expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.concat expects (bytes, bytes)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "bytes.slice" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.slice expects 3 args".to_string(),
                            ));
                        }
                        let b = self.infer(&args[0])?;
                        if (b != Ty::Bytes && b != Ty::BytesView)
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.slice expects (bytes_view, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "bytes.view" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.view expects 1 arg".to_string(),
                            ));
                        }
                        let b = self.infer(&args[0])?;
                        if b != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.view expects bytes".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::BytesView,
                            brand: b.brand,
                            view_full: true,
                        })
                    }
                    "std.brand.erase_bytes_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.erase_bytes_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.erase_bytes_v1 expects bytes".to_string(),
                            ));
                        }
                        Ok(TyInfo::unbranded(Ty::Bytes))
                    }
                    "std.brand.erase_view_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.erase_view_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let v = self.infer(&args[0])?;
                        if v != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.erase_view_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::BytesView,
                            brand: TyBrand::None,
                            view_full: v.view_full,
                        })
                    }
                    "std.brand.view_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.view_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if args[0].as_ident().is_none() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.view_v1 requires an identifier owner (bind the value to a local with let first)"
                                    .to_string(),
                            ));
                        }
                        let b = self.infer(&args[0])?;
                        if b != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.view_v1 expects bytes".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::BytesView,
                            brand: b.brand,
                            view_full: true,
                        })
                    }
                    "bytes.subview" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.subview expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.subview expects (bytes, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView.into())
                    }
                    "view.len" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.len expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.len expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "view.get_u8" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.get_u8 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.get_u8 expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "view.slice" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.slice expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.slice expects (bytes_view, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView.into())
                    }
                    "view.to_bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.to_bytes expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.to_bytes expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "__internal.brand.assume_view_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.brand.assume_view_v1 expects 2 args".to_string(),
                            ));
                        }
                        let brand_id = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.brand.assume_view_v1 expects a brand_id string"
                                    .to_string(),
                            )
                        })?;
                        crate::validate::validate_symbol(brand_id).map_err(|message| {
                            CompilerError::new(CompileErrorKind::Parse, message)
                        })?;
                        let v = self.infer(&args[1])?;
                        if v != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.brand.assume_view_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::BytesView,
                            brand: TyBrand::Brand(brand_id.to_string()),
                            view_full: v.view_full,
                        })
                    }
                    "__internal.brand.view_to_bytes_preserve_brand_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.brand.view_to_bytes_preserve_brand_v1 expects 1 arg"
                                    .to_string(),
                            ));
                        }
                        let v = self.infer(&args[0])?;
                        if v != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.brand.view_to_bytes_preserve_brand_v1 expects bytes_view"
                                    .to_string(),
                            ));
                        }
                        let out_brand = match v.brand {
                            TyBrand::Brand(b) => TyBrand::Brand(b),
                            TyBrand::Any | TyBrand::None => TyBrand::None,
                        };
                        Ok(TyInfo {
                            ty: Ty::Bytes,
                            brand: out_brand,
                            view_full: false,
                        })
                    }
                    "std.brand.assume_bytes_v1" => {
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.assume_bytes_v1 expects 2 args".to_string(),
                            ));
                        }
                        let brand_id = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.assume_bytes_v1 expects a brand_id string".to_string(),
                            )
                        })?;
                        crate::validate::validate_symbol(brand_id).map_err(|message| {
                            CompilerError::new(CompileErrorKind::Parse, message)
                        })?;
                        let b = self.infer(&args[1])?;
                        if b != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.assume_bytes_v1 expects bytes".to_string(),
                            ));
                        }
                        Ok(TyInfo::branded(Ty::Bytes, brand_id.to_string()))
                    }
                    "std.brand.cast_bytes_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_bytes_v1 expects 3 args".to_string(),
                            ));
                        }
                        let brand_id = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_bytes_v1 expects a brand_id string".to_string(),
                            )
                        })?;
                        crate::validate::validate_symbol(brand_id).map_err(|message| {
                            CompilerError::new(CompileErrorKind::Parse, message)
                        })?;
                        let validator_id = args[1].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_bytes_v1 expects a validator symbol".to_string(),
                            )
                        })?;
                        crate::validate::validate_symbol(validator_id).map_err(|message| {
                            CompilerError::new(CompileErrorKind::Parse, message)
                        })?;
                        self.require_brand_validator_v1(validator_id)?;
                        if self.infer(&args[2])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.cast_bytes_v1 expects bytes".to_string(),
                            ));
                        }
                        Ok(TyInfo::branded(Ty::ResultBytes, brand_id.to_string()))
                    }
                    "std.brand.cast_view_copy_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_copy_v1 expects 3 args".to_string(),
                            ));
                        }
                        let brand_id = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_copy_v1 expects a brand_id string".to_string(),
                            )
                        })?;
                        crate::validate::validate_symbol(brand_id).map_err(|message| {
                            CompilerError::new(CompileErrorKind::Parse, message)
                        })?;
                        let validator_id = args[1].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_copy_v1 expects a validator symbol".to_string(),
                            )
                        })?;
                        crate::validate::validate_symbol(validator_id).map_err(|message| {
                            CompilerError::new(CompileErrorKind::Parse, message)
                        })?;
                        self.require_brand_validator_v1(validator_id)?;
                        if self.infer(&args[2])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.cast_view_copy_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(TyInfo::branded(Ty::ResultBytes, brand_id.to_string()))
                    }
                    "std.brand.cast_view_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_v1 expects 3 args".to_string(),
                            ));
                        }
                        let brand_id = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_v1 expects a brand_id string".to_string(),
                            )
                        })?;
                        crate::validate::validate_symbol(brand_id).map_err(|message| {
                            CompilerError::new(CompileErrorKind::Parse, message)
                        })?;
                        let validator_id = args[1].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_v1 expects a validator symbol".to_string(),
                            )
                        })?;
                        crate::validate::validate_symbol(validator_id).map_err(|message| {
                            CompilerError::new(CompileErrorKind::Parse, message)
                        })?;
                        self.require_brand_validator_v1(validator_id)?;
                        if self.infer(&args[2])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.cast_view_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(TyInfo::branded(Ty::ResultBytesView, brand_id.to_string()))
                    }
                    "std.brand.to_bytes_preserve_if_full_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.to_bytes_preserve_if_full_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let v = self.infer(&args[0])?;
                        if v != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.brand.to_bytes_preserve_if_full_v1 expects bytes_view".to_string(),
                            ));
                        }
                        match (&v.brand, v.view_full) {
                            (TyBrand::Brand(b), true) => Ok(TyInfo::branded(Ty::Bytes, b.clone())),
                            _ => Ok(TyInfo::unbranded(Ty::Bytes)),
                        }
                    }
                    "view.eq" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.eq expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.eq expects (bytes_view, bytes_view)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "view.cmp_range" => {
                        if args.len() != 6 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.cmp_range expects 6 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                            || self.infer(&args[3])? != Ty::BytesView
                            || self.infer(&args[4])? != Ty::I32
                            || self.infer(&args[5])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "fs.read" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.read is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "fs.read expects 1 arg".to_string(),
                            ));
                        }
                        let path_ty = self.infer(&args[0])?;
                        if !matches!(path_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.read expects bytes_view path".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "fs.read_async" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.read_async is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "fs.read_async expects 1 arg".to_string(),
                            ));
                        }
                        let path_ty = self.infer(&args[0])?;
                        if !matches!(path_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.read_async expects bytes_view path".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "fs.open_read" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.open_read is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "fs.open_read expects 1 arg".to_string(),
                            ));
                        }
                        let path_ty = self.infer(&args[0])?;
                        if !matches!(path_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.open_read expects bytes_view path".to_string(),
                            ));
                        }
                        Ok(Ty::Iface.into())
                    }
                    "io.open_read_bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "io.open_read_bytes expects 1 arg".to_string(),
                            ));
                        }
                        let b_ty = self.infer(&args[0])?;
                        if !matches!(b_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "io.open_read_bytes expects bytes".to_string(),
                            ));
                        }
                        Ok(Ty::Iface.into())
                    }
                    "fs.list_dir" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.list_dir is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "fs.list_dir expects 1 arg".to_string(),
                            ));
                        }
                        let path_ty = self.infer(&args[0])?;
                        if !matches!(path_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.list_dir expects bytes_view path".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.fs.read_file" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.read_file expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.read_file expects bytes path".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.fs.write_file" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.write_file expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.write_file expects (bytes path, bytes data)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.fs.read_all_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.read_all_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.read_all_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "os.fs.write_all_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.write_all_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::Bytes
                            || self.infer(&args[2])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.write_all_v1 expects (bytes path, bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.append_all_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.append_all_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::Bytes
                            || self.infer(&args[2])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.append_all_v1 expects (bytes path, bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.stream_open_write_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.stream_open_write_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stream_open_write_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.stream_write_all_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.stream_write_all_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32
                            || self.infer(&args[1])? != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stream_write_all_v1 expects (i32 writer_handle, bytes_view data)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.stream_close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.stream_close_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stream_close_v1 expects i32 writer_handle".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.stream_drop_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.stream_drop_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stream_drop_v1 expects i32 writer_handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.fs.mkdirs_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.mkdirs_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.mkdirs_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.remove_file_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.remove_file_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.remove_file_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.remove_dir_all_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.remove_dir_all_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.remove_dir_all_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.rename_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.rename_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::Bytes
                            || self.infer(&args[2])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.rename_v1 expects (bytes src, bytes dst, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.fs.list_dir_sorted_text_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.list_dir_sorted_text_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.list_dir_sorted_text_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "os.fs.walk_glob_sorted_text_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.walk_glob_sorted_text_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::Bytes
                            || self.infer(&args[2])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.walk_glob_sorted_text_v1 expects (bytes root, bytes glob, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "os.fs.stat_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.stat_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stat_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "os.stdio.read_line_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.stdio.read_line_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.stdio.read_line_v1 expects (bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "os.stdio.write_stdout_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.stdio.write_stdout_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.stdio.write_stdout_v1 expects (bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.stdio.write_stderr_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.stdio.write_stderr_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.stdio.write_stderr_v1 expects (bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.stdio.flush_stdout_v1" => {
                        self.require_standalone_only(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.stdio.flush_stdout_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.stdio.flush_stderr_v1" => {
                        self.require_standalone_only(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.stdio.flush_stderr_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "os.rand.bytes_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.rand.bytes_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.rand.bytes_v1 expects (i32 n, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "os.rand.u64_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.rand.u64_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.rand.u64_v1 expects (bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "os.db.sqlite.open_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.sqlite.open_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.sqlite.query_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.sqlite.query_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.sqlite.exec_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.sqlite.exec_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.sqlite.close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.sqlite.close_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.pg.open_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.pg.open_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.pg.query_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.pg.query_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.pg.exec_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.pg.exec_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.pg.close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.pg.close_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.mysql.open_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.mysql.open_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.mysql.query_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.mysql.query_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.mysql.exec_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.mysql.exec_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.mysql.close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.mysql.close_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.redis.open_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.redis.open_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.redis.cmd_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.redis.cmd_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.cmd_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.db.redis.close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.redis.close_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.env.get" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.env.get expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.env.get expects bytes key".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.time.now_unix_ms" => {
                        self.require_standalone_only(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.now_unix_ms expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.time.now_instant_v1" => {
                        self.require_standalone_only(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.now_instant_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.time.sleep_ms_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.sleep_ms_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.sleep_ms_v1 expects i32 ms".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.time.local_tzid_v1" => {
                        self.require_standalone_only(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.local_tzid_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.time.tzdb_is_valid_tzid_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.tzdb_is_valid_tzid_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_is_valid_tzid_v1 expects bytes_view tzid".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.time.tzdb_offset_duration_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.tzdb_offset_duration_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_offset_duration_v1 expects (bytes_view tzid, i32 unix_s_lo, i32 unix_s_hi)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.time.tzdb_snapshot_id_v1" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.tzdb_snapshot_id_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "process.set_exit_code_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "process.set_exit_code_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "process.set_exit_code_v1 expects i32 code".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.exit" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.exit expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.exit expects i32 code".to_string(),
                            ));
                        }
                        Ok(Ty::Never.into())
                    }
                    "os.process.spawn_capture_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.spawn_capture_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.spawn_capture_v1 expects (bytes req, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.spawn_piped_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.spawn_piped_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.spawn_piped_v1 expects (bytes req, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.try_join_capture_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.try_join_capture_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.try_join_capture_v1 expects i32 proc handle"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::OptionBytes.into())
                    }
                    "os.process.join_capture_v1" | "std.os.process.join_capture_v1" => {
                        self.require_standalone_only(head)?;
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.join_capture_v1 is only allowed in solve or defasync"
                                    .to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.join_capture_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.join_capture_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.process.stdout_read_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.stdout_read_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdout_read_v1 expects (i32 handle, i32 max)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.process.stderr_read_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.stderr_read_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stderr_read_v1 expects (i32 handle, i32 max)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.process.stdin_write_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.stdin_write_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdin_write_v1 expects (i32 handle, bytes chunk)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.stdin_close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.stdin_close_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdin_close_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.try_wait_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.try_wait_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.try_wait_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.join_exit_v1" | "std.os.process.join_exit_v1" => {
                        self.require_standalone_only(head)?;
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.join_exit_v1 is only allowed in solve or defasync"
                                    .to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.join_exit_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.join_exit_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.take_exit_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.take_exit_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.take_exit_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.kill_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.kill_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.kill_v1 expects (i32 proc_handle, i32 sig)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.drop_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.drop_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.drop_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "os.process.run_capture_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.run_capture_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.run_capture_v1 expects (bytes req, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "os.net.http_request" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.net.http_request expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.net.http_request expects bytes req".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "rr.current_v1" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.current_v1 is disabled in this world".to_string(),
                            ));
                        }
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.current_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "rr.open_v1" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.open_v1 is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.open_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let cfg_ty = self.infer(&args[0])?;
                        if !matches!(cfg_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.open_v1 expects bytes_view cfg".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "rr.close_v1" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.close_v1 is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.close_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.close_v1 expects i32 rr_handle_v1".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "rr.stats_v1" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.stats_v1 is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.stats_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.stats_v1 expects i32 rr_handle_v1".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "rr.next_v1" => {
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
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.next_v1 expects i32 rr_handle_v1".to_string(),
                            ));
                        }
                        for (i, what) in ["kind", "op", "key"].into_iter().enumerate() {
                            let ty = self.infer(&args[i + 1])?;
                            if !matches!(ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("rr.next_v1 expects bytes_view {what}"),
                                ));
                            }
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "rr.append_v1" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.append_v1 is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.append_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.append_v1 expects i32 rr_handle_v1".to_string(),
                            ));
                        }
                        let ty = self.infer(&args[1])?;
                        if !matches!(ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.append_v1 expects bytes_view entry".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "rr.entry_resp_v1" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.entry_resp_v1 is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.entry_resp_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let ty = self.infer(&args[0])?;
                        if !matches!(ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.entry_resp_v1 expects bytes_view entry".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "rr.entry_err_v1" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.entry_err_v1 is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.entry_err_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let ty = self.infer(&args[0])?;
                        if !matches!(ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.entry_err_v1 expects bytes_view entry".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "kv.get" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "kv.get expects 1 arg".to_string(),
                            ));
                        }
                        let key_ty = self.infer(&args[0])?;
                        if !matches!(key_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get expects bytes_view key".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "kv.get_async" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get_async is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "kv.get_async expects 1 arg".to_string(),
                            ));
                        }
                        let key_ty = self.infer(&args[0])?;
                        if !matches!(key_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get_async expects bytes_view key".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "kv.get_stream" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get_stream is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "kv.get_stream expects 1 arg".to_string(),
                            ));
                        }
                        let key_ty = self.infer(&args[0])?;
                        if !matches!(key_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get_stream expects bytes_view key".to_string(),
                            ));
                        }
                        Ok(Ty::Iface.into())
                    }
                    "kv.set" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.set is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "kv.set expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.set expects (bytes, bytes)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "io.read" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "io.read expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Iface || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "io.read expects (iface, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "iface.make_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "iface.make_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "iface.make_v1 expects (i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Iface.into())
                    }
                    "bufread.new" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bufread.new expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Iface || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.new expects (iface, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "bufread.fill" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bufread.fill expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.fill expects i32 bufread handle".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView.into())
                    }
                    "bufread.consume" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bufread.consume expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.consume expects (i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "scratch_u8_fixed_v1.new" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "scratch_u8_fixed_v1.new expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.new expects i32 cap".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "scratch_u8_fixed_v1.clear"
                    | "scratch_u8_fixed_v1.len"
                    | "scratch_u8_fixed_v1.cap"
                    | "scratch_u8_fixed_v1.drop" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 handle"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "scratch_u8_fixed_v1.as_view" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "scratch_u8_fixed_v1.as_view expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.as_view expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView.into())
                    }
                    "scratch_u8_fixed_v1.try_write" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "scratch_u8_fixed_v1.try_write expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.try_write expects i32 handle".to_string(),
                            ));
                        }
                        let bty = self.infer(&args[1])?;
                        if bty != Ty::Bytes && bty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "scratch_u8_fixed_v1.try_write expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "budget.cfg_v1" => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.cfg_v1 is a descriptor; use it only as the first argument to budget.scope_v1".to_string(),
                    )),
                    "budget.scope_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "budget.scope_v1 expects 2 args".to_string(),
                            ));
                        }
                        let cfg = parse_budget_scope_cfg_v1(&args[0])?;
                        if cfg.mode == BudgetScopeModeV1::YieldV1 && !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "budget.scope_v1 mode=yield_v1 is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        let body_ty = self.infer(&args[1])?;
                        if cfg.mode == BudgetScopeModeV1::ResultErrV1 {
                            if !matches!(
                                body_ty.ty,
                                Ty::ResultI32
                                    | Ty::ResultBytes
                                    | Ty::ResultBytesView
                                    | Ty::ResultResultBytes
                                    | Ty::Never
                            ) {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "budget.scope_v1 mode=result_err_v1 requires a result_* body".to_string(),
                                ));
                            }
                            if body_ty.ty == Ty::Never
                                && !matches!(
                                    self.fn_ret_ty.ty,
                                    Ty::ResultI32
                                        | Ty::ResultBytes
                                        | Ty::ResultBytesView
                                        | Ty::ResultResultBytes
                                )
                            {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "budget.scope_v1 mode=result_err_v1 requires the function return type to be result_* when the body returns".to_string(),
                                ));
                            }
                        }
                        Ok(body_ty)
                    }
                    "budget.scope_from_arch_v1" => {
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
                        if lit.first().and_then(Expr::as_ident) != Some("bytes.lit") || lit.len() != 2
                        {
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
                        if cfg.mode == BudgetScopeModeV1::YieldV1 && !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "budget.scope_from_arch_v1 mode=yield_v1 is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        let body_ty = self.infer(&args[1])?;
                        if cfg.mode == BudgetScopeModeV1::ResultErrV1 {
                            if !matches!(
                                body_ty.ty,
                                Ty::ResultI32
                                    | Ty::ResultBytes
                                    | Ty::ResultBytesView
                                    | Ty::ResultResultBytes
                                    | Ty::Never
                            ) {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "budget.scope_from_arch_v1 requires a result_* body for this profile".to_string(),
                                ));
                            }
                            if body_ty.ty == Ty::Never
                                && !matches!(
                                    self.fn_ret_ty.ty,
                                    Ty::ResultI32
                                        | Ty::ResultBytes
                                        | Ty::ResultBytesView
                                        | Ty::ResultResultBytes
                                )
                            {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "budget.scope_from_arch_v1 requires the function return type to be result_* when the body returns for this profile".to_string(),
                                ));
                            }
                        }
                        Ok(body_ty)
                    }
                    "std.rr.with_v1" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "std.rr.with_v1 is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.rr.with_v1 expects 2 args".to_string(),
                            ));
                        }
                        if !matches!(self.fn_ret_ty.ty, Ty::ResultI32 | Ty::ResultBytes) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.rr.with_v1 requires function return type result_i32 or result_bytes (open failure propagation)".to_string(),
                            ));
                        }
                        let cfg_ty = self.infer(&args[0])?;
                        if !matches!(cfg_ty.ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.rr.with_v1 expects bytes_view cfg".to_string(),
                            ));
                        }
                        let body_ty = self.infer(&args[1])?;
                        Ok(body_ty)
                    }
                    "std.rr.with_policy_v1" => {
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
                        if !matches!(self.fn_ret_ty.ty, Ty::ResultI32 | Ty::ResultBytes) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "std.rr.with_policy_v1 requires function return type result_i32 or result_bytes (open failure propagation)".to_string(),
                            ));
                        }
                        let policy_id = parse_bytes_lit_ascii(&args[0], "std.rr.with_policy_v1 policy_id")?;
                        let cassette_path =
                            parse_bytes_lit_ascii(&args[1], "std.rr.with_policy_v1 cassette_path")?;
                        let mode_i32 = parse_i32_lit(&args[2], "std.rr.with_policy_v1 mode")?;
                        let _cfg = load_rr_cfg_v1_from_arch_v1(&self.options, &policy_id, &cassette_path, mode_i32)?;
                        let body_ty = self.infer(&args[3])?;
                        Ok(body_ty)
                    }
                    "task.scope.cfg_v1" => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.cfg_v1 is a descriptor; use it only as the first argument to task.scope_v1".to_string(),
                    )),
                    "task.scope_v1" => {
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
                        let _cfg = parse_task_scope_cfg_v1(&args[0])?;
                        self.task_scope_depth = self.task_scope_depth.saturating_add(1);
                        let body_ty = self.infer(&args[1])?;
                        self.task_scope_depth = self.task_scope_depth.saturating_sub(1);
                        if body_ty == Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_SLOT_004: task_slot_v1 must not escape task.scope_v1"
                                    .to_string(),
                            ));
                        }
                        if body_ty == Ty::TaskSelectEvtV1 || body_ty == Ty::OptionTaskSelectEvtV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SELECT_EVT_ESCAPES_SCOPE: task_select_evt_v1 must not escape task.scope_v1".to_string(),
                            ));
                        }
                        Ok(body_ty)
                    }
                    "task.scope.start_soon_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_001: task.scope.start_soon_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.start_soon_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let _call_ret = self.infer_immediate_defasync_call_expr(&args[0])?;
                        Ok(Ty::I32.into())
                    }
                    "task.scope.cancel_all_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_001: task.scope.cancel_all_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.cancel_all_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "task.scope.wait_all_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_001: task.scope.wait_all_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.wait_all_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "task.scope.async_let_bytes_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_SLOT_001: task.scope.async_let_bytes_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.async_let_bytes_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let call_ret = self.infer_immediate_defasync_call_expr(&args[0])?;
                        if call_ret != Ty::TaskHandleBytesV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.async_let_bytes_v1 expects a defasync that returns bytes".to_string(),
                            ));
                        }
                        Ok(Ty::TaskSlotV1.into())
                    }
                    "task.scope.async_let_result_bytes_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_SLOT_001: task.scope.async_let_result_bytes_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.async_let_result_bytes_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let call_ret = self.infer_immediate_defasync_call_expr(&args[0])?;
                        if call_ret != Ty::TaskHandleResultBytesV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.async_let_result_bytes_v1 expects a defasync that returns result_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::TaskSlotV1.into())
                    }
                    "task.scope.await_slot_bytes_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_SLOT_002: task.scope.await_slot_bytes_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.await_slot_bytes_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.await_slot_bytes_v1 expects task_slot_v1".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "task.scope.await_slot_result_bytes_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_SLOT_002: task.scope.await_slot_result_bytes_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.await_slot_result_bytes_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.await_slot_result_bytes_v1 expects task_slot_v1".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "task.scope.try_await_slot.bytes_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_SLOT_002: task.scope.try_await_slot.bytes_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.try_await_slot.bytes_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.try_await_slot.bytes_v1 expects task_slot_v1".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "task.scope.try_await_slot.result_bytes_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_SLOT_002: task.scope.try_await_slot.result_bytes_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.try_await_slot.result_bytes_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.try_await_slot.result_bytes_v1 expects task_slot_v1".to_string(),
                            ));
                        }
                        Ok(Ty::ResultResultBytes.into())
                    }
                    "task.scope.slot_is_finished_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SCOPE_SLOT_002: task.scope.slot_is_finished_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.slot_is_finished_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.slot_is_finished_v1 expects task_slot_v1".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "task.scope.slot_to_i32_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.slot_to_i32_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.slot_to_i32_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSlotV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.slot_to_i32_v1 expects task_slot_v1".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "task.scope.slot_from_i32_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.slot_from_i32_v1 used outside task.scope_v1".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.scope.slot_from_i32_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.scope.slot_from_i32_v1 expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::TaskSlotV1.into())
                    }
                    "task.scope.select.cfg_v1" => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "task.scope.select.cfg_v1 is a descriptor; use it only as the first argument to task.scope.select_*_v1".to_string(),
                    )),
                    "task.scope.select.cases_v1"
                    | "task.scope.select.case_slot_bytes_v1"
                    | "task.scope.select.case_chan_recv_bytes_v1" => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} is a descriptor; use it only as an argument to task.scope.select_*_v1"),
                    )),
                    "task.scope.select_v1" | "task.scope.select_try_v1" => {
                        if self.task_scope_depth == 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "X07E_SELECT_OUTSIDE_SCOPE: task.scope.select used outside task.scope_v1".to_string(),
                            ));
                        }
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
                                "X07E_SELECT_TOO_MANY_CASES: too many select cases".to_string(),
                            ));
                        }
                        for case in &cases {
                            match case {
                                TaskSelectCaseV1::SlotBytes { slot } => {
                                    if self.infer(slot)? != Ty::TaskSlotV1 {
                                        return Err(CompilerError::new(
                                            CompileErrorKind::Typing,
                                            "task.scope.select slot case expects task_slot_v1"
                                                .to_string(),
                                        ));
                                    }
                                }
                                TaskSelectCaseV1::ChanRecvBytes { chan } => {
                                    if self.infer(chan)? != Ty::I32 {
                                        return Err(CompilerError::new(
                                            CompileErrorKind::Typing,
                                            "task.scope.select chan.recv case expects i32 chan handle"
                                                .to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                        Ok((if head == "task.scope.select_v1" {
                            Ty::TaskSelectEvtV1
                        } else {
                            Ty::OptionTaskSelectEvtV1
                        })
                        .into())
                    }
                    "task.select_evt.tag_v1"
                    | "task.select_evt.case_index_v1"
                    | "task.select_evt.src_id_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSelectEvtV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects task_select_evt_v1"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "task.select_evt.take_bytes_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.select_evt.take_bytes_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSelectEvtV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.select_evt.take_bytes_v1 expects task_select_evt_v1".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "task.select_evt.drop_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.select_evt.drop_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::TaskSelectEvtV1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.select_evt.drop_v1 expects task_select_evt_v1".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "await" | "task.spawn" => {
                        if head == "await" && !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "await is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        let hty = self.infer(&args[0])?;
                        match head {
                            "await" => {
                                if hty != Ty::TaskHandleBytesV1 && hty != Ty::I32 {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "await expects bytes task handle".to_string(),
                                    ));
                                }
                                Ok(if hty.ty == Ty::TaskHandleBytesV1 {
                                    TyInfo {
                                        ty: Ty::Bytes,
                                        brand: hty.brand,
                                        view_full: false,
                                    }
                                } else {
                                    Ty::Bytes.into()
                                })
                            }
                            "task.spawn" => {
                                if hty != Ty::TaskHandleBytesV1 && hty != Ty::TaskHandleResultBytesV1
                                    && hty != Ty::I32
                                {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "task.spawn expects task handle".to_string(),
                                    ));
                                }
                                Ok(hty)
                            }
                            _ => unreachable!(),
                        }
                    }
                    "task.is_finished" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.is_finished expects 1 arg".to_string(),
                            ));
                        }
                        let hty = self.infer(&args[0])?;
                        if hty != Ty::TaskHandleBytesV1
                            && hty != Ty::TaskHandleResultBytesV1
                            && hty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.is_finished expects task handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "task.try_join.bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.try_join.bytes expects 1 arg".to_string(),
                            ));
                        }
                        let hty = self.infer(&args[0])?;
                        if hty != Ty::TaskHandleBytesV1 && hty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.try_join.bytes expects bytes task handle".to_string(),
                            ));
                        }
                        Ok(if hty.ty == Ty::TaskHandleBytesV1 {
                            TyInfo {
                                ty: Ty::ResultBytes,
                                brand: hty.brand,
                                view_full: false,
                            }
                        } else {
                            Ty::ResultBytes.into()
                        })
                    }
                    "task.try_join.result_bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.try_join.result_bytes expects 1 arg".to_string(),
                            ));
                        }
                        let hty = self.infer(&args[0])?;
                        if hty != Ty::TaskHandleResultBytesV1 && hty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.try_join.result_bytes expects result_bytes task handle"
                                    .to_string(),
                            ));
                        }
                        Ok(if hty.ty == Ty::TaskHandleResultBytesV1 {
                            TyInfo {
                                ty: Ty::ResultResultBytes,
                                brand: hty.brand,
                                view_full: false,
                            }
                        } else {
                            Ty::ResultResultBytes.into()
                        })
                    }
                    "task.join.bytes" | "task.join.result_bytes" | "task.cancel" => {
                        if (head == "task.join.bytes" || head == "task.join.result_bytes")
                            && !self.allow_async_ops
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                format!("{head} is only allowed in solve or defasync"),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        let hty = self.infer(&args[0])?;
                        match head {
                            "task.join.bytes" => {
                                if hty != Ty::TaskHandleBytesV1 && hty != Ty::I32 {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "task.join.bytes expects bytes task handle".to_string(),
                                    ));
                                }
                                Ok(if hty.ty == Ty::TaskHandleBytesV1 {
                                    TyInfo {
                                        ty: Ty::Bytes,
                                        brand: hty.brand,
                                        view_full: false,
                                    }
                                } else {
                                    Ty::Bytes.into()
                                })
                            }
                            "task.join.result_bytes" => {
                                if hty != Ty::TaskHandleResultBytesV1 && hty != Ty::I32 {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "task.join.result_bytes expects result_bytes task handle"
                                            .to_string(),
                                    ));
                                }
                                Ok(if hty.ty == Ty::TaskHandleResultBytesV1 {
                                    TyInfo {
                                        ty: Ty::ResultBytes,
                                        brand: hty.brand,
                                        view_full: false,
                                    }
                                } else {
                                    Ty::ResultBytes.into()
                                })
                            }
                            "task.cancel" => {
                                if hty != Ty::TaskHandleBytesV1
                                    && hty != Ty::TaskHandleResultBytesV1
                                    && hty != Ty::I32
                                {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "task.cancel expects task handle".to_string(),
                                    ));
                                }
                                Ok(Ty::I32.into())
                            }
                            _ => unreachable!(),
                        }
                    }
                    "task.yield" => {
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "task.yield is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.yield expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "task.sleep" => {
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "task.sleep is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.sleep expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.sleep expects i32 ticks".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "chan.bytes.new" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.new expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.new expects i32 cap".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "chan.bytes.send" => {
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "chan.bytes.send is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.send expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.send expects (i32, bytes)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "chan.bytes.try_send" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.try_send expects 2 args".to_string(),
                            ));
                        }
                        let payload = self.infer(&args[1])?;
                        if self.infer(&args[0])? != Ty::I32
                            || (payload != Ty::Bytes
                                && payload != Ty::BytesView
                                && payload != Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.try_send expects (i32, bytes_view)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "chan.bytes.recv" => {
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "chan.bytes.recv is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.recv expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.recv expects i32 chan handle".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "chan.bytes.try_recv" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.try_recv expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.try_recv expects i32 chan handle".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "chan.bytes.close" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.close expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.close expects i32 chan handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "codec.read_u32_le" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "codec.read_u32_le expects 2 args".to_string(),
                            ));
                        }
                        let b = self.infer(&args[0])?;
                        if (b != Ty::Bytes && b != Ty::BytesView)
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "codec.read_u32_le expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "codec.write_u32_le" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "codec.write_u32_le expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "codec.write_u32_le expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "fmt.u32_to_dec" | "fmt.s32_to_dec" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32"),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "parse.u32_dec" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "parse.u32_dec expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "parse.u32_dec expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "parse.u32_dec_at" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "parse.u32_dec_at expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "parse.u32_dec_at expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "prng.lcg_next_u32" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "prng.lcg_next_u32 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "prng.lcg_next_u32 expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "regex.compile_opts_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.compile_opts_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.compile_opts_v1 expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "regex.exec_from_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.exec_from_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.exec_from_v1 expects (bytes_view, bytes_view, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "regex.exec_caps_from_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.exec_caps_from_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.exec_caps_from_v1 expects (bytes_view, bytes_view, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "regex.find_all_x7sl_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.find_all_x7sl_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.find_all_x7sl_v1 expects (bytes_view, bytes_view, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(TyInfo::branded(
                            Ty::Bytes,
                            "std.text.slices.x7sl_v1".to_string(),
                        ))
                    }
                    "regex.split_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.split_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.split_v1 expects (bytes_view, bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(TyInfo::branded(
                            Ty::Bytes,
                            "std.text.slices.x7sl_v1".to_string(),
                        ))
                    }
                    "regex.replace_all_v1" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.replace_all_v1 expects 4 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::BytesView
                            || self.infer(&args[3])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.replace_all_v1 expects (bytes_view, bytes_view, bytes_view, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "jsonschema.compile_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "jsonschema.compile_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "jsonschema.compile_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "jsonschema.validate_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "jsonschema.validate_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "jsonschema.validate_v1 expects (bytes_view, bytes_view)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "vec_u8.with_capacity" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.with_capacity expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.with_capacity expects i32 cap".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8.into())
                    }
                    "vec_value.with_capacity_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_value.with_capacity_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.with_capacity_v1 expects (i32 ty_id, i32 cap)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_value.len" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_value.len expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.len expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_value.reserve_exact" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_value.reserve_exact expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.reserve_exact expects (i32 handle, i32 additional)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_value.pop" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_value.pop expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.pop expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_value.clear" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_value.clear expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_value.clear expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    h if h.starts_with("vec_value.push_") => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 handle"),
                            ));
                        }
                        let Some(suffix) = parse_value_suffix_single(head, "vec_value.push_") else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_x_ty =
                            value_suffix_ty(suffix).expect("suffix validated by parse_value_suffix");
                        if self.infer(&args[1])? != want_x_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects ({want_x_ty:?})"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    h if h.starts_with("vec_value.get_") => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 3 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
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
                        let want_out_ty =
                            value_suffix_ty(suffix).expect("suffix validated by parse_value_suffix");
                        let default = self.infer(&args[2])?;
                        if default != want_out_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects default ({want_out_ty:?})"),
                            ));
                        }
                        Ok(TyInfo {
                            ty: want_out_ty,
                            brand: default.brand,
                            view_full: false,
                        })
                    }
                    h if h.starts_with("vec_value.set_") => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 3 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
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
                        let want_x_ty =
                            value_suffix_ty(suffix).expect("suffix validated by parse_value_suffix");
                        if self.infer(&args[2])? != want_x_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects x ({want_x_ty:?})"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }

                    "map_value.new_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_value.new_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_value.new_v1 expects (i32 k_id, i32 v_id, i32 cap_pow2)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "map_value.len" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_value.len expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_value.len expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "map_value.clear" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_value.clear expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_value.clear expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    h if h.starts_with("map_value.contains_") => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 handle"),
                            ));
                        }
                        let Some(suffix) = parse_value_suffix_single(head, "map_value.contains_")
                        else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_k_ty =
                            value_suffix_ty(suffix).expect("suffix validated by parse_value_suffix");
                        if self.infer(&args[1])? != want_k_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects key ({want_k_ty:?})"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    h if h.starts_with("map_value.remove_") => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 handle"),
                            ));
                        }
                        let Some(suffix) = parse_value_suffix_single(head, "map_value.remove_")
                        else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unsupported head: {head:?}"),
                            ));
                        };
                        let want_k_ty =
                            value_suffix_ty(suffix).expect("suffix validated by parse_value_suffix");
                        if self.infer(&args[1])? != want_k_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects key ({want_k_ty:?})"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    h if h.starts_with("map_value.get_") => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 3 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 handle"),
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
                        let want_k_ty =
                            value_suffix_ty(k_suffix).expect("suffix validated by parse_value_suffix");
                        let want_v_ty =
                            value_suffix_ty(v_suffix).expect("suffix validated by parse_value_suffix");
                        let key = self.infer(&args[1])?;
                        if key != want_k_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects key ({want_k_ty:?})"),
                            ));
                        }
                        let default = self.infer(&args[2])?;
                        if default != want_v_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects default ({want_v_ty:?})"),
                            ));
                        }
                        Ok(TyInfo {
                            ty: want_v_ty,
                            brand: default.brand,
                            view_full: false,
                        })
                    }
                    h if h.starts_with("map_value.set_") => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 3 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 handle"),
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
                        let want_k_ty =
                            value_suffix_ty(k_suffix).expect("suffix validated by parse_value_suffix");
                        let want_v_ty =
                            value_suffix_ty(v_suffix).expect("suffix validated by parse_value_suffix");
                        if self.infer(&args[1])? != want_k_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects key ({want_k_ty:?})"),
                            ));
                        }
                        if self.infer(&args[2])? != want_v_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects val ({want_v_ty:?})"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "map_u32.new" | "set_u32.new" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_u8.len" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.len expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.len expects vec_u8".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_u8.cap" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.cap expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.cap expects vec_u8".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_u8.clear" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.clear expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.clear expects vec_u8".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8.into())
                    }
                    "map_u32.len" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_u32.len expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.len expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_u8.reserve_exact" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.reserve_exact expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8.into())
                    }
                    "vec_u8.extend_zeroes" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.extend_zeroes expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8.into())
                    }
                    "vec_u8.get" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.get expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.get expects (vec_u8, i32 index)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "vec_u8.set" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.set expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.set expects (vec_u8, i32 index, i32 value)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8.into())
                    }
                    "vec_u8.push" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.push expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.push expects (vec_u8, i32 value)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8.into())
                    }
                    "vec_u8.extend_bytes" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.extend_bytes expects 2 args".to_string(),
                            ));
                        }
                        let b = self.infer(&args[1])?;
                        if self.infer(&args[0])? != Ty::VecU8
                            || (b != Ty::Bytes && b != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8.into())
                    }
                    "vec_u8.extend_bytes_range" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.extend_bytes_range expects 4 args".to_string(),
                            ));
                        }
                        let b = self.infer(&args[1])?;
                        if self.infer(&args[0])? != Ty::VecU8
                            || (b != Ty::Bytes && b != Ty::BytesView)
                            || self.infer(&args[2])? != Ty::I32
                            || self.infer(&args[3])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8.into())
                    }
                    "vec_u8.into_bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.into_bytes expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.into_bytes expects vec_u8".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "vec_u8.as_view" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.as_view expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.as_view expects vec_u8".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView.into())
                    }
                    "option_i32.none" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_i32.none expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::OptionI32.into())
                    }
                    "option_i32.some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_i32.some expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_i32.some expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::OptionI32.into())
                    }
                    "option_i32.is_some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_i32.is_some expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::OptionI32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_i32.is_some expects option_i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "option_i32.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_i32.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::OptionI32
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_i32.unwrap_or expects (option_i32, i32 default)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "option_bytes.none" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes.none expects 0 args".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::OptionBytes,
                            brand: TyBrand::Any,
                            view_full: false,
                        })
                    }
                    "option_bytes.some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes.some expects 1 arg".to_string(),
                            ));
                        }
                        let payload = self.infer(&args[0])?;
                        if payload != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes.some expects bytes".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::OptionBytes,
                            brand: payload.brand,
                            view_full: false,
                        })
                    }
                    "option_bytes.is_some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes.is_some expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::OptionBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes.is_some expects option_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "option_bytes.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        let opt = self.infer(&args[0])?;
                        let default = self.infer(&args[1])?;
                        if opt != Ty::OptionBytes || default != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes.unwrap_or expects (option_bytes, bytes default)"
                                    .to_string(),
                            ));
                        }
                        let out_brand = match opt.brand {
                            TyBrand::Any => default.brand,
                            other => tybrand_join(Ty::Bytes, &other, &default.brand),
                        };
                        Ok(TyInfo {
                            ty: Ty::Bytes,
                            brand: out_brand,
                            view_full: false,
                        })
                    }
                    "option_bytes_view.none" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes_view.none expects 0 args".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::OptionBytesView,
                            brand: TyBrand::Any,
                            view_full: false,
                        })
                    }
                    "option_bytes_view.some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes_view.some expects 1 arg".to_string(),
                            ));
                        }
                        let payload = self.infer(&args[0])?;
                        if payload != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes_view.some expects bytes_view".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::OptionBytesView,
                            brand: payload.brand,
                            view_full: false,
                        })
                    }
                    "option_bytes_view.is_some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes_view.is_some expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::OptionBytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes_view.is_some expects option_bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "option_bytes_view.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes_view.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        let opt = self.infer(&args[0])?;
                        let default = self.infer(&args[1])?;
                        if opt != Ty::OptionBytesView || default != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes_view.unwrap_or expects (option_bytes_view, bytes_view default)"
                                    .to_string(),
                            ));
                        }
                        let out_brand = match opt.brand {
                            TyBrand::Any => default.brand,
                            other => tybrand_join(Ty::BytesView, &other, &default.brand),
                        };
                        Ok(TyInfo {
                            ty: Ty::BytesView,
                            brand: out_brand,
                            view_full: false,
                        })
                    }
                    "result_i32.ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.ok expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "result_i32.err" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.err expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.err expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32.into())
                    }
                    "result_i32.is_ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.is_ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultI32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.is_ok expects result_i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_i32.err_code" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.err_code expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultI32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.err_code expects result_i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_i32.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultI32
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.unwrap_or expects (result_i32, i32 default)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_bytes.ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.ok expects 1 arg".to_string(),
                            ));
                        }
                        let payload = self.infer(&args[0])?;
                        if payload != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.ok expects bytes".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::ResultBytes,
                            brand: payload.brand,
                            view_full: false,
                        })
                    }
                    "result_bytes.err" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.err expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.err expects i32".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::ResultBytes,
                            brand: TyBrand::Any,
                            view_full: false,
                        })
                    }
                    "result_bytes.is_ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.is_ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.is_ok expects result_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_bytes.err_code" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.err_code expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.err_code expects result_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_bytes.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultBytes
                            || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.unwrap_or expects (result_bytes, bytes default)"
                                    .to_string(),
                            ));
                        }
                        let res = self.infer(&args[0])?;
                        let default = self.infer(&args[1])?;
                        if res != Ty::ResultBytes || default != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.unwrap_or expects (result_bytes, bytes default)"
                                    .to_string(),
                            ));
                        }
                        let out_brand = match res.brand {
                            TyBrand::Any => default.brand,
                            other => tybrand_join(Ty::Bytes, &other, &default.brand),
                        };
                        Ok(TyInfo {
                            ty: Ty::Bytes,
                            brand: out_brand,
                            view_full: false,
                        })
                    }
                    "__internal.result_bytes.unwrap_ok_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.result_bytes.unwrap_ok_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let res = self.infer(&args[0])?;
                        if res != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.result_bytes.unwrap_ok_v1 expects result_bytes"
                                    .to_string(),
                            ));
                        }
                        let out_brand = match res.brand {
                            TyBrand::Brand(b) => TyBrand::Brand(b),
                            TyBrand::Any | TyBrand::None => TyBrand::None,
                        };
                        Ok(TyInfo {
                            ty: Ty::Bytes,
                            brand: out_brand,
                            view_full: false,
                        })
                    }
                    "__internal.bytes.alloc_aligned_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.bytes.alloc_aligned_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.bytes.alloc_aligned_v1 expects (i32 len, i32 align)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "__internal.bytes.clone_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.bytes.clone_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let b = self.infer(&args[0])?;
                        if b != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.bytes.clone_v1 expects bytes".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::Bytes,
                            brand: b.brand,
                            view_full: false,
                        })
                    }
                    "__internal.bytes.drop_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.bytes.drop_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.bytes.drop_v1 expects bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "__internal.stream_xf.plugin_init_v1" => {
                        if args.len() != 12 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.stream_xf.plugin_init_v1 expects 12 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::Bytes
                            || self.infer(&args[3])? != Ty::Bytes
                            || self.infer(&args[4])? != Ty::Bytes
                            || self.infer(&args[5])? != Ty::Bytes
                            || self.infer(&args[6])? != Ty::I32
                            || self.infer(&args[7])? != Ty::I32
                            || self.infer(&args[8])? != Ty::I32
                            || self.infer(&args[9])? != Ty::I32
                            || self.infer(&args[10])? != Ty::I32
                            || self.infer(&args[11])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.stream_xf.plugin_init_v1 arg types mismatch".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "__internal.stream_xf.plugin_step_v1" => {
                        if args.len() != 9 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.stream_xf.plugin_step_v1 expects 9 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::Bytes
                            || self.infer(&args[3])? != Ty::Bytes
                            || self.infer(&args[4])? != Ty::Bytes
                            || self.infer(&args[5])? != Ty::I32
                            || self.infer(&args[6])? != Ty::I32
                            || self.infer(&args[7])? != Ty::I32
                            || self.infer(&args[8])? != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.stream_xf.plugin_step_v1 arg types mismatch".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "__internal.stream_xf.plugin_flush_v1" => {
                        if args.len() != 8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.stream_xf.plugin_flush_v1 expects 8 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::Bytes
                            || self.infer(&args[3])? != Ty::Bytes
                            || self.infer(&args[4])? != Ty::Bytes
                            || self.infer(&args[5])? != Ty::I32
                            || self.infer(&args[6])? != Ty::I32
                            || self.infer(&args[7])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "__internal.stream_xf.plugin_flush_v1 arg types mismatch".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes.into())
                    }
                    "result_bytes_view.ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.ok expects 1 arg".to_string(),
                            ));
                        }
                        let payload = self.infer(&args[0])?;
                        if payload != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes_view.ok expects bytes_view".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::ResultBytesView,
                            brand: payload.brand,
                            view_full: false,
                        })
                    }
                    "result_bytes_view.err" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.err expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes_view.err expects i32".to_string(),
                            ));
                        }
                        Ok(TyInfo {
                            ty: Ty::ResultBytesView,
                            brand: TyBrand::Any,
                            view_full: false,
                        })
                    }
                    "result_bytes_view.is_ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.is_ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultBytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes_view.is_ok expects result_bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_bytes_view.err_code" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.err_code expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultBytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes_view.err_code expects result_bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_bytes_view.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        let res = self.infer(&args[0])?;
                        let default = self.infer(&args[1])?;
                        if res != Ty::ResultBytesView || default != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes_view.unwrap_or expects (result_bytes_view, bytes_view default)"
                                    .to_string(),
                            ));
                        }
                        let out_brand = match res.brand {
                            TyBrand::Any => default.brand,
                            other => tybrand_join(Ty::BytesView, &other, &default.brand),
                        };
                        Ok(TyInfo {
                            ty: Ty::BytesView,
                            brand: out_brand,
                            view_full: false,
                        })
                    }
                    "result_result_bytes.is_ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_result_bytes.is_ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.is_ok expects result_result_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_result_bytes.err_code" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_result_bytes.err_code expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.err_code expects result_result_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "result_result_bytes.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_result_bytes.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultResultBytes
                            || self.infer(&args[1])? != Ty::ResultBytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.unwrap_or expects (result_result_bytes, result_bytes default)"
                                    .to_string(),
                            ));
                        }
                        let outer = self.infer(&args[0])?;
                        let default = self.infer(&args[1])?;
                        if outer != Ty::ResultResultBytes || default != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_result_bytes.unwrap_or expects (result_result_bytes, result_bytes default)"
                                    .to_string(),
                            ));
                        }
                        let out_brand = match outer.brand {
                            TyBrand::Any => default.brand,
                            other => tybrand_join(Ty::ResultBytes, &other, &default.brand),
                        };
                        Ok(TyInfo {
                            ty: Ty::ResultBytes,
                            brand: out_brand,
                            view_full: false,
                        })
                    }
                    "try" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "try expects 1 arg".to_string(),
                            ));
                        }
                        let arg = self.infer(&args[0])?;
                        match arg.ty {
                            Ty::ResultI32 => {
                                if !matches!(self.fn_ret_ty.ty, Ty::ResultI32 | Ty::ResultBytes) {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "try(result_i32) requires function return type result_i32 or result_bytes".to_string(),
                                    ));
                                }
                                Ok(Ty::I32.into())
                            }
                            Ty::ResultBytes => {
                                if !matches!(self.fn_ret_ty.ty, Ty::ResultBytes | Ty::ResultI32) {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "try(result_bytes) requires function return type result_bytes or result_i32".to_string(),
                                    ));
                                }
                                Ok(TyInfo {
                                    ty: Ty::Bytes,
                                    brand: arg.brand,
                                    view_full: false,
                                })
                            }
                            Ty::ResultBytesView => {
                                if !matches!(
                                    self.fn_ret_ty.ty,
                                    Ty::ResultBytesView | Ty::ResultI32
                                ) {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "try(result_bytes_view) requires function return type result_bytes_view or result_i32".to_string(),
                                    ));
                                }
                                Ok(TyInfo {
                                    ty: Ty::BytesView,
                                    brand: arg.brand,
                                    view_full: false,
                                })
                            }
                            other => Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "try expects result_i32, result_bytes, or result_bytes_view, got {other:?}"
                                ),
                            )),
                        }
                    }
                    "map_u32.get" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_u32.get expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.get expects (handle, key, default) all i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "map_u32.set" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_u32.set expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.set expects (handle, key, val) all i32".to_string(),
                            ));
                        }
                        let v = self.infer(&args[2])?;
                        if v != Ty::I32 && !is_task_handle_ty(v.ty) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.set expects (handle, key, val) all i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "map_u32.contains" | "set_u32.contains" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (handle, key)"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "map_u32.remove" | "set_u32.remove" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (handle, key)"),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "set_u32.add" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "set_u32.add expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "set_u32.add expects (handle, key)".to_string(),
                            ));
                        }
                        Ok(Ty::I32.into())
                    }
                    "set_u32.dump_u32le" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "set_u32.dump_u32le expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "set_u32.dump_u32le expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    "map_u32.dump_kv_u32le_u32le" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_u32.dump_kv_u32le_u32le expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.dump_kv_u32le_u32le expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes.into())
                    }
                    _ => {
                        if let Some(f) = self.extern_functions.get(head).cloned() {
                            self.require_ffi_world(head)?;
                            self.require_unsafe_world(head)?;
                            self.require_unsafe_block(head)?;

                            if args.len() != f.params.len() {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Parse,
                                    format!("call {head:?} expects {} args", f.params.len()),
                                ));
                            }
                            for (i, (arg, p)) in args.iter().zip(f.params.iter()).enumerate() {
                                let got = self.infer(arg)?;
                                let want = p.ty;
                                let ok = ty_compat_call_arg_extern(got.ty, want);
                                if !ok {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("call {head:?} arg {i} expects {want:?}"),
                                    ));
                                }
                            }
                            Ok(f.ret_ty.into())
                        } else {
                            match self.functions.get(head).cloned() {
                                Some(sig) => {
                                    if args.len() != sig.params.len() {
                                        return Err(CompilerError::new(
                                            CompileErrorKind::Parse,
                                            format!(
                                                "call {head:?} expects {} args",
                                                sig.params.len()
                                            ),
                                        ));
                                    }
                                    for (i, (arg, want)) in
                                        args.iter().zip(sig.params.iter()).enumerate()
                                    {
                                        let got = self.infer(arg)?;
                                        let ok = tyinfo_compat_call_arg(&got, want);
                                        if !ok {
                                            return Err(CompilerError::new(
                                                CompileErrorKind::Typing,
                                                call_arg_mismatch_message(head, i, &got, want),
                                            ));
                                        }
                                    }
                                    Ok(sig.ret)
                                }
                                None => Err(CompilerError::new(
                                    CompileErrorKind::Unsupported,
                                    format!("unsupported head: {head:?}"),
                                )),
                            }
                        }
                    }
                }
            }
        }
    }

    fn infer_stmt(&mut self, expr: &Expr) -> Result<Ty, CompilerError> {
        let ty = match expr {
            Expr::List { items, .. } => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];
                match head {
                    "begin" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "(begin ...) requires at least 1 expression".to_string(),
                            ));
                        }
                        self.push_scope();
                        let mut result = Ty::I32;
                        for e in args {
                            if self.infer_stmt(e)? == Ty::Never {
                                result = Ty::Never;
                            }
                        }
                        self.pop_scope();
                        result
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "if condition must be i32".to_string(),
                            ));
                        }
                        self.push_scope();
                        let then_ty = self.infer_stmt(&args[1])?;
                        self.pop_scope();

                        self.push_scope();
                        let else_ty = self.infer_stmt(&args[2])?;
                        self.pop_scope();

                        if then_ty == Ty::Never && else_ty == Ty::Never {
                            Ty::Never
                        } else {
                            Ty::I32
                        }
                    }
                    "for" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "for form: (for <i> <start:i32> <end:i32> <body:any>)".to_string(),
                            ));
                        }
                        let var = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "for variable must be an identifier".to_string(),
                            )
                        })?;
                        match self.lookup(var) {
                            Some(v) if v.ty == Ty::I32 => {}
                            Some(_) => {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("for variable must be i32: {var:?}"),
                                ));
                            }
                            None => {
                                self.bind(var.to_string(), Ty::I32.into());
                            }
                        }
                        if self.infer(&args[1])? != Ty::I32 || self.infer(&args[2])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "for bounds must be i32".to_string(),
                            ));
                        }
                        self.push_scope();
                        let _ = self.infer_stmt(&args[3])?;
                        self.pop_scope();
                        Ty::I32
                    }
                    _ => self.infer(expr)?.ty,
                }
            }
            _ => self.infer(expr)?.ty,
        };

        Ok(if ty == Ty::Never { Ty::Never } else { Ty::I32 })
    }
}

impl<'a> Emitter<'a> {
    pub(super) fn infer_option_bytes_view_borrow_from_expr(
        &self,
        fn_name: &str,
        expr: &Expr,
        env: &mut ViewBorrowEnv,
        cache: &mut ViewBorrowFnCache,
        view_cache: &mut ViewBorrowFnCache,
        collector: &mut ViewBorrowCollector,
    ) -> Result<Option<ViewBorrowFrom>, CompilerError> {
        match expr {
            Expr::Int { .. } => Ok(None),
            Expr::Ident { name, .. } => Ok(env.lookup(name)),
            Expr::List { items, .. } => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "begin" | "unsafe" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("({head} ...) requires at least 1 expression"),
                            ));
                        }
                        env.push_scope();
                        for e in &args[..args.len() - 1] {
                            let _ = self.infer_option_bytes_view_borrow_from_expr(
                                fn_name, e, env, cache, view_cache, collector,
                            )?;
                        }
                        let out = self.infer_option_bytes_view_borrow_from_expr(
                            fn_name,
                            &args[args.len() - 1],
                            env,
                            cache,
                            view_cache,
                            collector,
                        )?;
                        env.pop_scope();
                        Ok(out)
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        env.push_scope();
                        let t = self.infer_option_bytes_view_borrow_from_expr(
                            fn_name, &args[1], env, cache, view_cache, collector,
                        )?;
                        env.pop_scope();

                        env.push_scope();
                        let e = self.infer_option_bytes_view_borrow_from_expr(
                            fn_name, &args[2], env, cache, view_cache, collector,
                        )?;
                        env.pop_scope();

                        match (t, e) {
                            (Some(a), Some(b)) => Ok(Some(merge_view_borrow_from(fn_name, a, b)?)),
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "let" | "set" | "set0" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} form: ({head} <name> <expr>)"),
                            ));
                        }
                        let name = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} name must be an identifier"),
                            )
                        })?;

                        let src = self.infer_option_bytes_view_borrow_from_expr(
                            fn_name, &args[1], env, cache, view_cache, collector,
                        )?;
                        if let Some(src) = &src {
                            env.bind(name.to_string(), src.clone());
                        }
                        Ok(src)
                    }
                    "return" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "return form: (return <expr>)".to_string(),
                            ));
                        }
                        let src = self.require_option_bytes_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, view_cache, collector,
                        )?;
                        collector.merge(fn_name, src)?;
                        Ok(None)
                    }
                    "option_bytes_view.none" => Ok(Some(ViewBorrowFrom::Runtime)),
                    "option_bytes_view.some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes_view.some expects 1 arg".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name,
                            &args[0],
                            env,
                            &mut *view_cache.cache,
                            &mut *view_cache.visiting,
                            collector,
                        )?;
                        Ok(Some(src))
                    }
                    _ => {
                        let Some(f) = self.program.functions.iter().find(|f| f.name == head) else {
                            return Ok(None);
                        };
                        if f.ret_ty != Ty::OptionBytesView {
                            return Ok(None);
                        }
                        let (cache_map, cache_visiting) = (&mut *cache.cache, &mut *cache.visiting);
                        let (view_cache_map, view_cache_visiting) =
                            (&mut *view_cache.cache, &mut *view_cache.visiting);
                        let spec = self.option_bytes_view_return_arg_for_fn(
                            head,
                            cache_map,
                            cache_visiting,
                            view_cache_map,
                            view_cache_visiting,
                        )?;
                        match spec {
                            None => Ok(Some(ViewBorrowFrom::Runtime)),
                            Some(idx) => {
                                if args.len() <= idx {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("call {head:?} missing arg {idx}"),
                                    ));
                                }
                                let src = self.require_view_borrow_from_expr(
                                    fn_name,
                                    &args[idx],
                                    env,
                                    &mut *view_cache.cache,
                                    &mut *view_cache.visiting,
                                    collector,
                                )?;
                                Ok(Some(src))
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn infer_result_bytes_view_borrow_from_expr(
        &self,
        fn_name: &str,
        expr: &Expr,
        env: &mut ViewBorrowEnv,
        cache: &mut ViewBorrowFnCache,
        view_cache: &mut ViewBorrowFnCache,
        collector: &mut ViewBorrowCollector,
    ) -> Result<Option<ViewBorrowFrom>, CompilerError> {
        match expr {
            Expr::Int { .. } => Ok(None),
            Expr::Ident { name, .. } => Ok(env.lookup(name)),
            Expr::List { items, .. } => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "begin" | "unsafe" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("({head} ...) requires at least 1 expression"),
                            ));
                        }
                        env.push_scope();
                        for e in &args[..args.len() - 1] {
                            let _ = self.infer_result_bytes_view_borrow_from_expr(
                                fn_name, e, env, cache, view_cache, collector,
                            )?;
                        }
                        let out = self.infer_result_bytes_view_borrow_from_expr(
                            fn_name,
                            &args[args.len() - 1],
                            env,
                            cache,
                            view_cache,
                            collector,
                        )?;
                        env.pop_scope();
                        Ok(out)
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        env.push_scope();
                        let t = self.infer_result_bytes_view_borrow_from_expr(
                            fn_name, &args[1], env, cache, view_cache, collector,
                        )?;
                        env.pop_scope();

                        env.push_scope();
                        let e = self.infer_result_bytes_view_borrow_from_expr(
                            fn_name, &args[2], env, cache, view_cache, collector,
                        )?;
                        env.pop_scope();

                        match (t, e) {
                            (Some(a), Some(b)) => Ok(Some(merge_view_borrow_from(fn_name, a, b)?)),
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "let" | "set" | "set0" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} form: ({head} <name> <expr>)"),
                            ));
                        }
                        let name = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} name must be an identifier"),
                            )
                        })?;

                        let src = self.infer_result_bytes_view_borrow_from_expr(
                            fn_name, &args[1], env, cache, view_cache, collector,
                        )?;
                        if let Some(src) = &src {
                            env.bind(name.to_string(), src.clone());
                        }
                        Ok(src)
                    }
                    "return" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "return form: (return <expr>)".to_string(),
                            ));
                        }
                        let src = self.require_result_bytes_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, view_cache, collector,
                        )?;
                        collector.merge(fn_name, src)?;
                        Ok(None)
                    }
                    "result_bytes_view.ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.ok expects 1 arg".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name,
                            &args[0],
                            env,
                            &mut *view_cache.cache,
                            &mut *view_cache.visiting,
                            collector,
                        )?;
                        Ok(Some(src))
                    }
                    "result_bytes_view.err" => Ok(Some(ViewBorrowFrom::Runtime)),
                    "std.brand.cast_view_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_v1 expects 3 args".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name,
                            &args[2],
                            env,
                            &mut *view_cache.cache,
                            &mut *view_cache.visiting,
                            collector,
                        )?;
                        Ok(Some(src))
                    }
                    _ => {
                        let Some(f) = self.program.functions.iter().find(|f| f.name == head) else {
                            return Ok(None);
                        };
                        if f.ret_ty != Ty::ResultBytesView {
                            return Ok(None);
                        }
                        let (cache_map, cache_visiting) = (&mut *cache.cache, &mut *cache.visiting);
                        let (view_cache_map, view_cache_visiting) =
                            (&mut *view_cache.cache, &mut *view_cache.visiting);
                        let spec = self.result_bytes_view_return_arg_for_fn(
                            head,
                            cache_map,
                            cache_visiting,
                            view_cache_map,
                            view_cache_visiting,
                        )?;
                        match spec {
                            None => Ok(Some(ViewBorrowFrom::Runtime)),
                            Some(idx) => {
                                if args.len() <= idx {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("call {head:?} missing arg {idx}"),
                                    ));
                                }
                                let src = self.require_view_borrow_from_expr(
                                    fn_name,
                                    &args[idx],
                                    env,
                                    &mut *view_cache.cache,
                                    &mut *view_cache.visiting,
                                    collector,
                                )?;
                                Ok(Some(src))
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn infer_view_borrow_from_expr(
        &self,
        fn_name: &str,
        expr: &Expr,
        env: &mut ViewBorrowEnv,
        cache: &mut BTreeMap<String, Option<usize>>,
        visiting: &mut BTreeSet<String>,
        collector: &mut ViewBorrowCollector,
    ) -> Result<Option<ViewBorrowFrom>, CompilerError> {
        match expr {
            Expr::Int { .. } => Ok(None),
            Expr::Ident { name, .. } => {
                if name == "input" {
                    return Ok(Some(ViewBorrowFrom::Runtime));
                }
                Ok(env.lookup(name))
            }
            Expr::List { items, ptr } => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "begin" | "unsafe" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("({head} ...) requires at least 1 expression"),
                            ));
                        }
                        env.push_scope();
                        for e in &args[..args.len() - 1] {
                            let _ = self.infer_view_borrow_from_expr(
                                fn_name, e, env, cache, visiting, collector,
                            )?;
                        }
                        let out = self.infer_view_borrow_from_expr(
                            fn_name,
                            &args[args.len() - 1],
                            env,
                            cache,
                            visiting,
                            collector,
                        )?;
                        env.pop_scope();
                        Ok(out)
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        env.push_scope();
                        let t = self.infer_view_borrow_from_expr(
                            fn_name, &args[1], env, cache, visiting, collector,
                        )?;
                        env.pop_scope();

                        env.push_scope();
                        let e = self.infer_view_borrow_from_expr(
                            fn_name, &args[2], env, cache, visiting, collector,
                        )?;
                        env.pop_scope();

                        match (t, e) {
                            (Some(a), Some(b)) => Ok(Some(merge_view_borrow_from(fn_name, a, b)?)),
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "let" | "set" | "set0" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} form: ({head} <name> <expr>)"),
                            ));
                        }
                        let name = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} name must be an identifier"),
                            )
                        })?;

                        let src = self.infer_view_borrow_from_expr(
                            fn_name, &args[1], env, cache, visiting, collector,
                        )?;
                        if let Some(src) = &src {
                            env.bind(name.to_string(), src.clone());
                        }
                        Ok(src)
                    }
                    "return" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "return form: (return <expr>)".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, visiting, collector,
                        )?;
                        collector.merge(fn_name, src)?;
                        Ok(None)
                    }
                    "view.slice" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.slice expects (bytes_view,i32,i32)".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, visiting, collector,
                        )?;
                        Ok(Some(src))
                    }
                    "try" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "try expects 1 arg".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, visiting, collector,
                        )?;
                        Ok(Some(src))
                    }
                    "std.brand.erase_view_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.erase_view_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, visiting, collector,
                        )?;
                        Ok(Some(src))
                    }
                    "result_bytes_view.ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.ok expects 1 arg".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, visiting, collector,
                        )?;
                        Ok(Some(src))
                    }
                    "result_bytes_view.err" => Ok(Some(ViewBorrowFrom::Runtime)),
                    "bytes.view_lit" => Ok(Some(ViewBorrowFrom::Runtime)),
                    "std.brand.cast_view_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_v1 expects 3 args".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name, &args[2], env, cache, visiting, collector,
                        )?;
                        Ok(Some(src))
                    }
                    "bytes.view" | "bytes.subview" | "vec_u8.as_view" => {
                        let Some(owner_name) = args.first().and_then(Expr::as_ident) else {
                            let ptr = ptr.as_str();
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "{head} requires an identifier owner (bind the value to a local with let first) (ptr={ptr})"
                                ),
                            ));
                        };
                        Ok(Some(ViewBorrowFrom::LocalOwned(owner_name.to_string())))
                    }
                    "std.brand.view_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.view_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let Some(owner_name) = args.first().and_then(Expr::as_ident) else {
                            let ptr = ptr.as_str();
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "std.brand.view_v1 requires an identifier owner (bind the value to a local with let first) (ptr={ptr})"
                                ),
                            ));
                        };
                        Ok(Some(ViewBorrowFrom::LocalOwned(owner_name.to_string())))
                    }
                    "bufread.fill" => Ok(Some(ViewBorrowFrom::Runtime)),
                    _ => {
                        let Some(f) = self.program.functions.iter().find(|f| f.name == head) else {
                            return Ok(None);
                        };
                        if f.ret_ty != Ty::BytesView {
                            return Ok(None);
                        }
                        let spec = self.view_return_arg_for_fn(head, cache, visiting)?;
                        match spec {
                            None => Ok(Some(ViewBorrowFrom::Runtime)),
                            Some(idx) => {
                                if args.len() <= idx {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("call {head:?} missing arg {idx}"),
                                    ));
                                }
                                let src = self.require_view_borrow_from_expr(
                                    fn_name, &args[idx], env, cache, visiting, collector,
                                )?;
                                Ok(Some(src))
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn infer_expr_in_new_scope(&self, expr: &Expr) -> Result<TyInfo, CompilerError> {
        self.infer_expr_in_new_scope_with_task_scope_depth(expr, self.task_scopes.len())
    }

    pub(super) fn infer_expr_in_new_scope_with_task_scope_depth(
        &self,
        expr: &Expr,
        task_scope_depth: usize,
    ) -> Result<TyInfo, CompilerError> {
        let mut functions: BTreeMap<String, FnSig> = BTreeMap::new();
        for f in &self.program.functions {
            functions.insert(
                f.name.clone(),
                FnSig {
                    ret: TyInfo {
                        ty: f.ret_ty,
                        brand: ty_brand_from_opt(&f.ret_brand),
                        view_full: false,
                    },
                    params: f
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
        for f in &self.program.async_functions {
            let (call_ret_ty, call_ret_brand) = match f.ret_ty {
                Ty::Bytes => (Ty::TaskHandleBytesV1, ty_brand_from_opt(&f.ret_brand)),
                Ty::ResultBytes => (Ty::TaskHandleResultBytesV1, ty_brand_from_opt(&f.ret_brand)),
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
            functions.insert(
                f.name.clone(),
                FnSig {
                    ret: TyInfo {
                        ty: call_ret_ty,
                        brand: call_ret_brand,
                        view_full: false,
                    },
                    params: f
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

        let mut infer = InferCtx {
            options: self.options.clone(),
            fn_ret_ty: TyInfo::unbranded(self.fn_ret_ty),
            allow_async_ops: self.allow_async_ops,
            unsafe_depth: self.unsafe_depth,
            task_scope_depth,
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
            functions,
            extern_functions: self.extern_functions.clone(),
        };
        infer.infer(expr)
    }
}
