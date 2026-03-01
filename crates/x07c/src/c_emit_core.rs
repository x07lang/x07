use super::*;

impl<'a> Emitter<'a> {
    pub(super) fn emit_program(&mut self) -> Result<(), CompilerError> {
        self.compute_view_return_args()?;
        if self.options.freestanding {
            self.push_str("#define X07_FREESTANDING 1\n");
        }
        if self.options.world.is_standalone_only() {
            self.push_str("#define X07_STANDALONE 1\n");
        }
        self.emit_runtime_preamble()?;
        if self.options.world.is_standalone_only() {
            self.push_str(c_emit_worlds::RUNTIME_C_OS);
        }
        self.push_char('\n');

        self.emit_extern_function_prototypes();
        self.emit_async_function_prototypes();
        self.emit_user_function_prototypes();
        self.emit_async_functions()?;
        self.emit_user_functions()?;
        self.emit_solve()?;

        if self.options.emit_main {
            self.push_str(RUNTIME_C_MAIN);
        } else {
            self.push_str(RUNTIME_C_LIB);
        }
        Ok(())
    }

    pub(super) fn check_program(&mut self) -> Result<(), CompilerError> {
        self.compute_view_return_args()?;
        for f in &self.program.async_functions {
            self.emit_async_function(f)?;
        }
        for f in &self.program.functions {
            self.emit_user_function(f)?;
        }
        self.emit_solve()?;
        Ok(())
    }

    pub(super) fn emit_runtime_preamble(&mut self) -> Result<(), CompilerError> {
        const JSON_JCS_START: &str = "\n// --- X07_JSON_JCS_START";
        const JSON_JCS_END: &str = "\n// --- X07_JSON_JCS_END";

        const STREAM_XF_PLUGIN_START: &str = "\n// --- X07_STREAM_XF_PLUGIN_START";
        const STREAM_XF_PLUGIN_END: &str = "\n// --- X07_STREAM_XF_PLUGIN_END";

        let uses_stream_xf_plugin =
            program_uses_head(self.program, "__internal.bytes.alloc_aligned_v1")
                || program_uses_head(self.program, "__internal.stream_xf.plugin_init_v1")
                || program_uses_head(self.program, "__internal.stream_xf.plugin_step_v1")
                || program_uses_head(self.program, "__internal.stream_xf.plugin_flush_v1");

        let uses_json_jcs = program_uses_head(self.program, "json.jcs.canon_doc_v1")
            || program_uses_stream_xf_plugin_json_jcs(self.program);

        if self.options.enable_fs || self.options.enable_rr || self.options.enable_kv {
            let mut preamble = RUNTIME_C_PREAMBLE.to_string();

            if !uses_json_jcs {
                preamble =
                    trim_preamble_section(&preamble, JSON_JCS_START, JSON_JCS_END, "json.jcs")?;
            }
            if !uses_stream_xf_plugin {
                preamble = trim_preamble_section(
                    &preamble,
                    STREAM_XF_PLUGIN_START,
                    STREAM_XF_PLUGIN_END,
                    "stream_xf_plugin",
                )?;
            }

            self.push_str(&preamble);
            if program_uses_contracts(self.program)
                && self.options.contract_mode == ContractMode::RuntimeTrap
            {
                self.push_str(CONTRACT_RUNTIME_HELPERS_C);
            }
            if self.options.contract_mode == ContractMode::VerifyBmc {
                self.push_str(
                    "\nvoid __CPROVER_assume(int);\nvoid __CPROVER_assert(int, const char*);\n",
                );
            }
            return Ok(());
        }

        const FIXTURE_START: &str = "\n#if X07_ENABLE_FS\nstatic bytes_t rt_fs_read";
        const FIXTURE_END: &str = "\nstatic uint32_t rt_codec_read_u32_le";

        let mut preamble = RUNTIME_C_PREAMBLE.to_string();
        if !uses_json_jcs {
            preamble = trim_preamble_section(&preamble, JSON_JCS_START, JSON_JCS_END, "json.jcs")?;
        }
        if !uses_stream_xf_plugin {
            preamble = trim_preamble_section(
                &preamble,
                STREAM_XF_PLUGIN_START,
                STREAM_XF_PLUGIN_END,
                "stream_xf_plugin",
            )?;
        }

        let (head, rest) = preamble.split_once(FIXTURE_START).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: runtime preamble missing fixture start marker".to_string(),
            )
        })?;
        let (_, tail) = rest.split_once(FIXTURE_END).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: runtime preamble missing fixture end marker".to_string(),
            )
        })?;

        self.push_str(head);
        self.push_str(RUNTIME_C_PURE_STUBS);
        self.push_str(FIXTURE_END);
        self.push_str(tail);
        if program_uses_contracts(self.program)
            && self.options.contract_mode == ContractMode::RuntimeTrap
        {
            self.push_str(CONTRACT_RUNTIME_HELPERS_C);
        }
        if self.options.contract_mode == ContractMode::VerifyBmc {
            self.push_str(
                "\nvoid __CPROVER_assume(int);\nvoid __CPROVER_assert(int, const char*);\n",
            );
        }
        Ok(())
    }

    pub(super) fn emit_extern_function_prototypes(&mut self) {
        for f in &self.program.extern_functions {
            let ret = if f.ret_is_void {
                "void".to_string()
            } else {
                c_ret_ty(f.ret_ty).to_string()
            };
            self.line(&format!(
                "extern {ret} {}({});",
                f.link_name,
                c_extern_param_list(&f.params)
            ));
        }
        if !self.program.extern_functions.is_empty() {
            self.push_char('\n');
        }
    }

    pub(super) fn emit_user_function_prototypes(&mut self) {
        for f in &self.program.functions {
            self.line(&format!(
                "static {} {}(ctx_t* ctx, bytes_view_t input{});",
                c_ret_ty(f.ret_ty),
                self.fn_c_name(&f.name),
                c_param_list_user(&f.params)
            ));
        }
        if !self.program.functions.is_empty() {
            self.push_char('\n');
        }
    }

    pub(super) fn emit_user_functions(&mut self) -> Result<(), CompilerError> {
        for f in &self.program.functions {
            self.emit_user_function(f)?;
            self.push_char('\n');
        }
        Ok(())
    }

    pub(super) fn emit_user_function(&mut self, f: &FunctionDef) -> Result<(), CompilerError> {
        self.reset_fn_state();
        self.current_fn_name = Some(f.name.clone());
        self.fn_ret_ty = f.ret_ty;
        self.fn_contracts = FnContractsV1 {
            requires: f.requires.clone(),
            ensures: f.ensures.clone(),
            invariant: f.invariant.clone(),
        };
        self.allow_async_ops = false;
        self.emit_source_line_for_symbol(&f.name);

        if f.ret_ty != Ty::I32
            && f.ret_ty != Ty::Bytes
            && f.ret_ty != Ty::BytesView
            && f.ret_ty != Ty::VecU8
            && f.ret_ty != Ty::OptionI32
            && f.ret_ty != Ty::OptionBytes
            && f.ret_ty != Ty::OptionBytesView
            && f.ret_ty != Ty::ResultI32
            && f.ret_ty != Ty::ResultBytes
            && f.ret_ty != Ty::ResultBytesView
            && f.ret_ty != Ty::Iface
        {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!("invalid function return type: {:?}", f.ret_ty),
            ));
        }

        self.line(&format!(
            "static {} {}(ctx_t* ctx, bytes_view_t input{}) {{",
            c_ret_ty(f.ret_ty),
            self.fn_c_name(&f.name),
            c_param_list_user(&f.params)
        ));
        self.indent += 1;

        for (i, p) in f.params.iter().enumerate() {
            let mut v = self.make_var_ref(p.ty, format!("p{i}"), false);
            v.brand = ty_brand_from_opt(&p.brand);
            self.bind(p.name.clone(), v);
        }

        self.emit_contract_entry_checks()?;

        let result_ty = self.infer_expr_in_new_scope(&f.body)?;
        let want_ret_ty = TyInfo {
            ty: f.ret_ty,
            brand: ty_brand_from_opt(&f.ret_brand),
            view_full: false,
        };
        if result_ty != Ty::Never && !tyinfo_compat_assign(&result_ty, &want_ret_ty) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "function {:?} must evaluate to {:?} (or return), got {:?}",
                    f.name, f.ret_ty, result_ty
                ),
            ));
        }

        self.line(&format!(
            "{} out = {};",
            c_ret_ty(f.ret_ty),
            c_zero(f.ret_ty)
        ));
        self.emit_expr_to(&f.body, f.ret_ty, "out")?;

        let mut out_var = self.make_var_ref(f.ret_ty, "out".to_string(), false);
        out_var.brand = ty_brand_from_opt(&f.ret_brand);
        self.emit_contract_exit_checks(&out_var)?;

        for (ty, c_name) in self.live_owned_drop_list(None) {
            self.emit_drop_var(ty, &c_name);
        }
        self.line("return out;");

        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn emit_solve(&mut self) -> Result<(), CompilerError> {
        self.reset_fn_state();
        self.current_fn_name = Some("solve".to_string());
        self.fn_ret_ty = Ty::Bytes;
        self.allow_async_ops = true;
        self.emit_source_line_for_module("main");

        let result_ty = self.infer_expr_in_new_scope(&self.program.solve)?;
        if result_ty != Ty::Bytes && result_ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("solve expression must evaluate to bytes (or return), got {result_ty:?}"),
            ));
        }

        self.push_str("static bytes_t solve(ctx_t* ctx, bytes_view_t input) {\n");
        self.indent += 1;
        self.line("bytes_t out = rt_bytes_empty(ctx);");
        self.emit_expr_to(&self.program.solve, Ty::Bytes, "out")?;
        for (ty, c_name) in self.live_owned_drop_list(None) {
            self.emit_drop_var(ty, &c_name);
        }
        self.line("return out;");
        self.indent -= 1;
        self.push_str("}\n\n");
        Ok(())
    }

    pub(super) fn reset_fn_state(&mut self) {
        self.indent = 0;
        self.tmp_counter = 0;
        self.local_count = 0;
        self.scopes.clear();
        self.scopes.push(BTreeMap::new());
        self.task_scopes.clear();
        self.cleanup_scopes.clear();
        self.allow_async_ops = false;
        self.unsafe_depth = 0;
        self.current_fn_name = None;
        self.current_ptr = None;
        self.fn_contracts = FnContractsV1::default();
    }

    pub(super) fn emit_source_line_for_symbol(&mut self, sym: &str) {
        let module_id = sym.rsplit_once('.').map(|(m, _)| m).unwrap_or(sym);
        self.emit_source_line_for_module(module_id);
    }

    pub(super) fn emit_source_line_for_module(&mut self, module_id: &str) {
        let file = module_id.replace('.', "/") + ".x07.json";
        self.line(&format!("#line 1 \"{}\"", c_escape_c_string(&file)));
    }

    pub(super) fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    pub(super) fn pop_scope(&mut self) -> Result<(), CompilerError> {
        let Some(mut scope) = self.scopes.pop() else {
            return Ok(());
        };

        // Release borrows from views in this scope first so owned values can be dropped safely.
        let mut release = Vec::<String>::new();
        for v in scope.values() {
            if is_view_like_ty(v.ty) {
                if let Some(owner) = &v.borrow_of {
                    release.push(owner.clone());
                }
            }
        }
        for owner in release {
            let mut found = false;
            for v in scope.values_mut() {
                if v.c_name == owner {
                    v.borrow_count = v.borrow_count.saturating_sub(1);
                    found = true;
                    break;
                }
            }
            if !found {
                self.dec_borrow_count(&owner)?;
            }
        }

        // Drop owned values in this scope. Moves clear the source to an empty value, so dropping
        // moved-from locals is safe and required to avoid leaks across control-flow merges.
        for v in scope.values() {
            if !is_owned_ty(v.ty) {
                continue;
            }
            if v.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("drop while borrowed: {:?}", v.c_name),
                ));
            }
            self.emit_drop_var(v.ty, &v.c_name);
        }

        Ok(())
    }

    pub(super) fn bind(&mut self, name: String, var: VarRef) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, var);
        }
    }

    pub(super) fn lookup(&self, name: &str) -> Option<&VarRef> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    pub(super) fn lookup_mut(&mut self, name: &str) -> Option<&mut VarRef> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                return scope.get_mut(name);
            }
        }
        None
    }

    pub(super) fn alloc_local(&mut self, prefix: &str) -> Result<String, CompilerError> {
        let max_locals = language::limits::max_locals();
        if self.local_count >= max_locals {
            let msg = match &self.current_fn_name {
                Some(name) => format!(
                    "max locals exceeded: {} (fn={}) (hint: split this function body (extract helper defn/defasync) or raise X07_MAX_LOCALS)",
                    max_locals, name
                ),
                None => format!(
                    "max locals exceeded: {} (hint: split this function body (extract helper defn/defasync) or raise X07_MAX_LOCALS)",
                    max_locals
                ),
            };
            return Err(CompilerError::new(CompileErrorKind::Budget, msg));
        }
        self.local_count += 1;
        self.tmp_counter += 1;
        Ok(format!("{prefix}{}", self.tmp_counter))
    }

    pub(super) fn decl_local(&mut self, ty: Ty, name: &str) {
        match ty {
            Ty::I32
            | Ty::TaskHandleBytesV1
            | Ty::TaskHandleResultBytesV1
            | Ty::TaskSlotV1
            | Ty::TaskSelectEvtV1
            | Ty::Never => self.line(&format!("uint32_t {name} = UINT32_C(0);")),
            Ty::TaskScopeV1 => self.line(&format!("rt_scope_t {name} = (rt_scope_t){{0}};")),
            Ty::BudgetScopeV1 => self.line(&format!(
                "rt_budget_scope_t {name} = (rt_budget_scope_t){{0}};"
            )),
            Ty::Bytes => self.line(&format!("bytes_t {name} = rt_bytes_empty(ctx);")),
            Ty::BytesView => self.line(&format!("bytes_view_t {name} = rt_view_empty(ctx);")),
            Ty::VecU8 => self.line(&format!("vec_u8_t {name} = (vec_u8_t){{0}};")),
            Ty::OptionI32 | Ty::OptionTaskSelectEvtV1 => {
                self.line(&format!("option_i32_t {name} = (option_i32_t){{0}};"))
            }
            Ty::OptionBytes => {
                self.line(&format!("option_bytes_t {name} = (option_bytes_t){{0}};"))
            }
            Ty::OptionBytesView => self.line(&format!(
                "option_bytes_view_t {name} = (option_bytes_view_t){{0}};"
            )),
            Ty::ResultI32 => self.line(&format!("result_i32_t {name} = (result_i32_t){{0}};")),
            Ty::ResultBytes => {
                self.line(&format!("result_bytes_t {name} = (result_bytes_t){{0}};"))
            }
            Ty::ResultBytesView => self.line(&format!(
                "result_bytes_view_t {name} = (result_bytes_view_t){{0}};"
            )),
            Ty::ResultResultBytes => self.line(&format!(
                "result_result_bytes_t {name} = (result_result_bytes_t){{0}};"
            )),
            Ty::Iface => self.line(&format!("iface_t {name} = (iface_t){{0}};")),
            Ty::PtrConstU8 => self.line(&format!("const uint8_t* {name} = NULL;")),
            Ty::PtrMutU8 => self.line(&format!("uint8_t* {name} = NULL;")),
            Ty::PtrConstVoid => self.line(&format!("const void* {name} = NULL;")),
            Ty::PtrMutVoid => self.line(&format!("void* {name} = NULL;")),
            Ty::PtrConstI32 => self.line(&format!("const uint32_t* {name} = NULL;")),
            Ty::PtrMutI32 => self.line(&format!("uint32_t* {name} = NULL;")),
        }
    }

    pub(super) fn line(&mut self, s: &str) {
        if self.suppress_output {
            return;
        }
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    pub(super) fn open_block(&mut self) {
        self.line("{");
        self.indent += 1;
    }

    pub(super) fn close_block(&mut self) {
        self.indent = self.indent.saturating_sub(1);
        self.line("}");
    }
}

impl<'a> Emitter<'a> {
    pub(super) fn new(program: &'a Program, options: CompileOptions) -> Self {
        let mut fn_c_names = BTreeMap::new();
        for f in &program.functions {
            fn_c_names.insert(f.name.clone(), c_user_fn_name(&f.name));
        }
        let mut async_fn_new_names = BTreeMap::new();
        for f in &program.async_functions {
            async_fn_new_names.insert(f.name.clone(), c_async_new_name(&f.name));
        }
        let extern_functions = program
            .extern_functions
            .iter()
            .map(|d| (d.name.clone(), d.clone()))
            .collect::<BTreeMap<_, _>>();
        Self {
            program,
            options,
            out: String::new(),
            suppress_output: false,
            indent: 0,
            tmp_counter: 0,
            local_count: 0,
            scopes: vec![BTreeMap::new()],
            task_scopes: Vec::new(),
            cleanup_scopes: Vec::new(),
            fn_c_names,
            async_fn_new_names,
            extern_functions,
            fn_view_return_arg: BTreeMap::new(),
            fn_option_bytes_view_return_arg: BTreeMap::new(),
            fn_result_bytes_view_return_arg: BTreeMap::new(),
            fn_ret_ty: Ty::Bytes,
            allow_async_ops: false,
            unsafe_depth: 0,
            current_fn_name: None,
            current_ptr: None,
            native_requires: BTreeMap::new(),
            fn_contracts: FnContractsV1::default(),
        }
    }

    pub(super) fn require_native_backend(
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
            return Err(self.err(
                CompileErrorKind::Internal,
                format!("native backend ABI mismatch for {backend_id}: got {abi_major} expected {expected}"),
            ));
        }

        Ok(())
    }

    pub(super) fn native_requires(&self) -> Vec<NativeBackendReq> {
        self.native_requires
            .iter()
            .map(|(backend_id, acc)| NativeBackendReq {
                backend_id: backend_id.clone(),
                abi_major: acc.abi_major,
                features: acc.features.iter().cloned().collect(),
            })
            .collect()
    }

    pub(super) fn push_str(&mut self, s: &str) {
        if self.suppress_output {
            return;
        }
        self.out.push_str(s);
    }

    pub(super) fn push_char(&mut self, c: char) {
        if self.suppress_output {
            return;
        }
        self.out.push(c);
    }

    pub(super) fn make_var_ref(&self, ty: Ty, c_name: String, is_temp: bool) -> VarRef {
        VarRef {
            ty,
            brand: TyBrand::None,
            c_name,
            moved: false,
            moved_ptr: None,
            borrow_count: 0,
            borrow_of: None,
            borrow_ptr: None,
            borrow_is_full: false,
            is_temp,
        }
    }

    pub(super) fn err(&self, kind: CompileErrorKind, message: String) -> CompilerError {
        let ptr = self.current_ptr.as_deref().filter(|p| !p.is_empty());
        match (&self.current_fn_name, ptr) {
            (Some(name), Some(ptr)) => {
                CompilerError::new(kind, format!("{message} (fn={name} ptr={ptr})"))
            }
            (Some(name), None) => CompilerError::new(kind, format!("{message} (fn={name})")),
            (None, Some(ptr)) => CompilerError::new(kind, format!("{message} (ptr={ptr})")),
            (None, None) => CompilerError::new(kind, message),
        }
    }

    pub(super) fn lookup_mut_by_c_name(&mut self, c_name: &str) -> Option<&mut VarRef> {
        for scope in self.scopes.iter_mut().rev() {
            for v in scope.values_mut() {
                if v.c_name == c_name {
                    return Some(v);
                }
            }
        }
        None
    }

    pub(super) fn inc_borrow_count(&mut self, owner_c_name: &str) -> Result<(), CompilerError> {
        let Some(owner) = self.lookup_mut_by_c_name(owner_c_name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("borrow of unknown owner: {:?}", owner_c_name),
            ));
        };
        owner.borrow_count = owner.borrow_count.saturating_add(1);
        Ok(())
    }

    pub(super) fn dec_borrow_count(&mut self, owner_c_name: &str) -> Result<(), CompilerError> {
        let Some(owner) = self.lookup_mut_by_c_name(owner_c_name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("borrow release of unknown owner: {:?}", owner_c_name),
            ));
        };
        if owner.borrow_count == 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("borrow underflow for owner: {:?}", owner_c_name),
            ));
        }
        owner.borrow_count -= 1;
        Ok(())
    }

    pub(super) fn find_any_borrower_for_owner(
        &self,
        owner_c_name: &str,
    ) -> Option<(String, VarRef)> {
        for scope in self.scopes.iter().rev() {
            for (name, v) in scope {
                if !is_view_like_ty(v.ty) {
                    continue;
                }
                if v.borrow_of.as_deref() != Some(owner_c_name) {
                    continue;
                }
                return Some((name.clone(), v.clone()));
            }
        }
        None
    }

    pub(super) fn borrowed_by_diag_suffix(&self, owner_c_name: &str) -> String {
        let Some((borrower_name, borrower)) = self.find_any_borrower_for_owner(owner_c_name) else {
            return String::new();
        };
        let borrower_name = borrower_name
            .strip_prefix("#tmp:")
            .unwrap_or(borrower_name.as_str());
        let ptr = borrower
            .borrow_ptr
            .as_deref()
            .filter(|p| !p.is_empty())
            .unwrap_or("<unknown>");
        format!(" borrowed_by={borrower_name:?} borrowed_ptr={ptr}")
    }

    pub(super) fn release_temp_view_borrow(&mut self, view: &VarRef) -> Result<(), CompilerError> {
        if !is_view_like_ty(view.ty) || !view.is_temp {
            return Ok(());
        }
        let Some(owner) = &view.borrow_of else {
            return Ok(());
        };
        self.dec_borrow_count(owner)?;
        if let Some(tmp) = self.lookup_mut_by_c_name(&view.c_name) {
            tmp.borrow_of = None;
            tmp.borrow_ptr = None;
        }
        Ok(())
    }

    pub(super) fn emit_drop_var(&mut self, ty: Ty, c_name: &str) {
        match ty {
            Ty::Bytes => self.line(&format!("rt_bytes_drop(ctx, &{c_name});")),
            Ty::VecU8 => self.line(&format!("rt_vec_u8_drop(ctx, &{c_name});")),
            Ty::OptionBytes => {
                self.line(&format!("if ({c_name}.tag) {{"));
                self.indent += 1;
                self.line(&format!("rt_bytes_drop(ctx, &{c_name}.payload);"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{c_name}.tag = UINT32_C(0);"));
            }
            Ty::ResultBytes => {
                self.line(&format!("if ({c_name}.tag) {{"));
                self.indent += 1;
                self.line(&format!("rt_bytes_drop(ctx, &{c_name}.payload.ok);"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{c_name}.tag = UINT32_C(0);"));
                self.line(&format!("{c_name}.payload.err = UINT32_C(0);"));
            }
            Ty::ResultResultBytes => {
                self.line(&format!("if ({c_name}.tag) {{"));
                self.indent += 1;
                self.line(&format!("if ({c_name}.payload.ok.tag) {{"));
                self.indent += 1;
                self.line(&format!(
                    "rt_bytes_drop(ctx, &{c_name}.payload.ok.payload.ok);"
                ));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{c_name}.payload.ok.tag = UINT32_C(0);"));
                self.line(&format!("{c_name}.payload.ok.payload.err = UINT32_C(0);"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{c_name}.tag = UINT32_C(0);"));
                self.line(&format!("{c_name}.payload.err = UINT32_C(0);"));
            }
            Ty::TaskScopeV1 => {
                self.line(&format!("rt_scope_dispose_on_drop(ctx, &{c_name});"));
            }
            Ty::BudgetScopeV1 => {
                self.line(&format!("rt_budget_scope_dispose_on_drop(ctx, &{c_name});"));
            }
            Ty::TaskSelectEvtV1 => {
                self.line(&format!(
                    "if ({c_name} != 0) rt_select_evt_drop(ctx, {c_name});"
                ));
                self.line(&format!("{c_name} = UINT32_C(0);"));
            }
            Ty::OptionTaskSelectEvtV1 => {
                self.line(&format!("if ({c_name}.tag) {{"));
                self.indent += 1;
                self.line(&format!("rt_select_evt_drop(ctx, {c_name}.payload);"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{c_name}.tag = UINT32_C(0);"));
                self.line(&format!("{c_name}.payload = UINT32_C(0);"));
            }
            _ => {}
        }
    }

    pub(super) fn emit_overwrite_result_with_err(&mut self, ty: Ty, c_name: &str, err_code: &str) {
        match ty {
            Ty::ResultI32 => {
                self.line(&format!(
                    "{c_name} = (result_i32_t){{ .tag = UINT32_C(0), .payload.err = {err_code} }};"
                ));
            }
            Ty::ResultBytes => {
                self.line(&format!("if ({c_name}.tag) {{"));
                self.indent += 1;
                self.line(&format!("rt_bytes_drop(ctx, &{c_name}.payload.ok);"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "{c_name} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {err_code} }};"
                ));
            }
            Ty::ResultBytesView => {
                self.line(&format!(
                    "{c_name} = (result_bytes_view_t){{ .tag = UINT32_C(0), .payload.err = {err_code} }};"
                ));
            }
            Ty::ResultResultBytes => {
                self.line(&format!("if ({c_name}.tag) {{"));
                self.indent += 1;
                self.line(&format!("if ({c_name}.payload.ok.tag) {{"));
                self.indent += 1;
                self.line(&format!(
                    "rt_bytes_drop(ctx, &{c_name}.payload.ok.payload.ok);"
                ));
                self.indent -= 1;
                self.line("}");
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "{c_name} = (result_result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {err_code} }};"
                ));
            }
            _ => {}
        }
    }

    pub(super) fn borrow_of_view_expr(&self, expr: &Expr) -> Result<Option<String>, CompilerError> {
        match expr {
            Expr::Ident { name, .. } => {
                if name == "input" {
                    return Ok(None);
                }
                let Some(v) = self.lookup(name) else {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                if v.ty != Ty::BytesView {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("expected bytes_view, got {:?} for {name:?}", v.ty),
                    ));
                }
                Ok(v.borrow_of.clone())
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
                    "begin" | "unsafe" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("({head} ...) requires at least 1 expression"),
                            ));
                        }
                        self.borrow_of_view_expr(&args[args.len() - 1])
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        let t = self.borrow_of_view_expr(&args[1])?;
                        let e = self.borrow_of_view_expr(&args[2])?;
                        match (t, e) {
                            (Some(a), Some(b)) => {
                                if a != b {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "bytes_view must have a single borrow source across branches"
                                            .to_string(),
                                    ));
                                }
                                Ok(Some(a))
                            }
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "std.brand.erase_view_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.erase_view_v1 expects 1 arg".to_string(),
                            ));
                        }
                        self.borrow_of_view_expr(&args[0])
                    }
                    "__internal.brand.assume_view_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "__internal.brand.assume_view_v1 expects 2 args".to_string(),
                            ));
                        }
                        self.borrow_of_view_expr(&args[1])
                    }
                    "bytes.view_lit" => Ok(None),
                    "std.brand.view_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.view_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let Some(owner_name) = args.first().and_then(Expr::as_ident) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                "std.brand.view_v1 requires an identifier owner (bind the value to a local with let first)"
                                    .to_string(),
                            ));
                        };
                        let Some(owner) = self.lookup(owner_name) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {owner_name:?}"),
                            ));
                        };
                        if owner.ty != Ty::Bytes {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                "std.brand.view_v1 expects bytes owner".to_string(),
                            ));
                        }
                        Ok(Some(owner.c_name.clone()))
                    }
                    "bytes.view" | "bytes.subview" => {
                        let Some(owner_name) = args.first().and_then(Expr::as_ident) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!(
                                    "{head} requires an identifier owner (bind the value to a local with let first)"
                                ),
                            ));
                        };
                        let Some(owner) = self.lookup(owner_name) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {owner_name:?}"),
                            ));
                        };
                        if owner.ty != Ty::Bytes {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes owner"),
                            ));
                        }
                        Ok(Some(owner.c_name.clone()))
                    }
                    "vec_u8.as_view" => {
                        let Some(owner_name) = args.first().and_then(Expr::as_ident) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                "vec_u8.as_view requires an identifier owner (bind the value to a local with let first)"
                                    .to_string(),
                            ));
                        };
                        let Some(owner) = self.lookup(owner_name) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {owner_name:?}"),
                            ));
                        };
                        if owner.ty != Ty::VecU8 {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                "vec_u8.as_view expects vec_u8 owner".to_string(),
                            ));
                        }
                        Ok(Some(owner.c_name.clone()))
                    }
                    "view.slice" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.slice expects (bytes_view,i32,i32)".to_string(),
                            ));
                        }
                        self.borrow_of_view_expr(&args[0])
                    }
                    "option_bytes_view.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes_view.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        let a = self.borrow_of_as_bytes_view(&args[0])?;
                        let b = self.borrow_of_as_bytes_view(&args[1])?;
                        match (a, b) {
                            (Some(a), Some(b)) => {
                                if a != b {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "bytes_view must have a single borrow source across branches"
                                            .to_string(),
                                    ));
                                }
                                Ok(Some(a))
                            }
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "result_bytes_view.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        let a = self.borrow_of_as_bytes_view(&args[0])?;
                        let b = self.borrow_of_as_bytes_view(&args[1])?;
                        match (a, b) {
                            (Some(a), Some(b)) => {
                                if a != b {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "bytes_view must have a single borrow source across branches"
                                            .to_string(),
                                    ));
                                }
                                Ok(Some(a))
                            }
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "try" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "try expects 1 arg".to_string(),
                            ));
                        }
                        self.borrow_of_as_bytes_view(&args[0])
                    }
                    // Views that borrow from runtime state (not from a user-owned buffer).
                    "bufread.fill" => Ok(None),
                    "scratch_u8_fixed_v1.as_view" => Ok(None),
                    _ if self.fn_view_return_arg.contains_key(head) => {
                        let Some(spec) = self.fn_view_return_arg.get(head) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Internal,
                                format!(
                                    "internal error: missing view-return analysis for {head:?}"
                                ),
                            ));
                        };
                        match spec {
                            Some(idx) => {
                                if args.len() <= *idx {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!(
                                            "call {head:?} needs arg {idx} to infer bytes_view borrow source"
                                        ),
                                    ));
                                }
                                self.borrow_of_as_bytes_view(&args[*idx])
                            }
                            None => Ok(None),
                        }
                    }
                    _ => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "cannot infer borrow source for bytes_view expression".to_string(),
                    )),
                }
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cannot infer borrow source for bytes_view expression".to_string(),
            )),
        }
    }

    pub(super) fn borrow_of_option_bytes_view_expr(
        &self,
        expr: &Expr,
    ) -> Result<Option<String>, CompilerError> {
        match expr {
            Expr::Ident { name, .. } => {
                let Some(v) = self.lookup(name) else {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                if v.ty != Ty::OptionBytesView {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("expected option_bytes_view, got {:?} for {name:?}", v.ty),
                    ));
                }
                Ok(v.borrow_of.clone())
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
                    "begin" | "unsafe" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("({head} ...) requires at least 1 expression"),
                            ));
                        }
                        self.borrow_of_option_bytes_view_expr(&args[args.len() - 1])
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        let t = self.borrow_of_option_bytes_view_expr(&args[1])?;
                        let e = self.borrow_of_option_bytes_view_expr(&args[2])?;
                        match (t, e) {
                            (Some(a), Some(b)) => {
                                if a != b {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "option_bytes_view must have a single borrow source across branches"
                                            .to_string(),
                                    ));
                                }
                                Ok(Some(a))
                            }
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "option_bytes_view.none" => Ok(None),
                    "option_bytes_view.some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes_view.some expects 1 arg".to_string(),
                            ));
                        }
                        self.borrow_of_view_expr(&args[0])
                    }
                    _ if self.fn_option_bytes_view_return_arg.contains_key(head) => {
                        let Some(spec) = self.fn_option_bytes_view_return_arg.get(head) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Internal,
                                format!(
                                    "internal error: missing option_bytes_view return analysis for {head:?}"
                                ),
                            ));
                        };
                        match spec {
                            Some(idx) => {
                                if args.len() <= *idx {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!(
                                            "call {head:?} needs arg {idx} to infer option_bytes_view borrow source"
                                        ),
                                    ));
                                }
                                self.borrow_of_as_bytes_view(&args[*idx])
                            }
                            None => Ok(None),
                        }
                    }
                    _ => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "cannot infer borrow source for option_bytes_view expression".to_string(),
                    )),
                }
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cannot infer borrow source for option_bytes_view expression".to_string(),
            )),
        }
    }

    pub(super) fn borrow_of_result_bytes_view_expr(
        &self,
        expr: &Expr,
    ) -> Result<Option<String>, CompilerError> {
        match expr {
            Expr::Ident { name, .. } => {
                let Some(v) = self.lookup(name) else {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                if v.ty != Ty::ResultBytesView {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("expected result_bytes_view, got {:?} for {name:?}", v.ty),
                    ));
                }
                Ok(v.borrow_of.clone())
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
                    "begin" | "unsafe" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("({head} ...) requires at least 1 expression"),
                            ));
                        }
                        self.borrow_of_result_bytes_view_expr(&args[args.len() - 1])
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        let t = self.borrow_of_result_bytes_view_expr(&args[1])?;
                        let e = self.borrow_of_result_bytes_view_expr(&args[2])?;
                        match (t, e) {
                            (Some(a), Some(b)) => {
                                if a != b {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "result_bytes_view must have a single borrow source across branches"
                                            .to_string(),
                                    ));
                                }
                                Ok(Some(a))
                            }
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "result_bytes_view.ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes_view.ok expects 1 arg".to_string(),
                            ));
                        }
                        self.borrow_of_view_expr(&args[0])
                    }
                    "result_bytes_view.err" => Ok(None),
                    "std.brand.cast_view_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "std.brand.cast_view_v1 expects 3 args".to_string(),
                            ));
                        }
                        self.borrow_of_view_expr(&args[2])
                    }
                    _ if self.fn_result_bytes_view_return_arg.contains_key(head) => {
                        let Some(spec) = self.fn_result_bytes_view_return_arg.get(head) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Internal,
                                format!(
                                    "internal error: missing result_bytes_view return analysis for {head:?}"
                                ),
                            ));
                        };
                        match spec {
                            Some(idx) => {
                                if args.len() <= *idx {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!(
                                            "call {head:?} needs arg {idx} to infer result_bytes_view borrow source"
                                        ),
                                    ));
                                }
                                self.borrow_of_as_bytes_view(&args[*idx])
                            }
                            None => Ok(None),
                        }
                    }
                    _ => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "cannot infer borrow source for result_bytes_view expression".to_string(),
                    )),
                }
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cannot infer borrow source for result_bytes_view expression".to_string(),
            )),
        }
    }

    pub(super) fn borrow_of_view_like_expr(
        &self,
        ty: Ty,
        expr: &Expr,
    ) -> Result<Option<String>, CompilerError> {
        match ty {
            Ty::BytesView => self.borrow_of_view_expr(expr),
            Ty::OptionBytesView => self.borrow_of_option_bytes_view_expr(expr),
            Ty::ResultBytesView => self.borrow_of_result_bytes_view_expr(expr),
            other => Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("internal error: borrow_of_view_like_expr unexpected type: {other:?}"),
            )),
        }
    }

    pub(super) fn borrow_of_as_bytes_view(
        &self,
        expr: &Expr,
    ) -> Result<Option<String>, CompilerError> {
        match expr {
            Expr::Ident { name, .. } => {
                if name == "input" {
                    return Ok(None);
                }
                let Some(v) = self.lookup(name) else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                match v.ty {
                    Ty::BytesView | Ty::OptionBytesView | Ty::ResultBytesView => {
                        Ok(v.borrow_of.clone())
                    }
                    Ty::Bytes | Ty::VecU8 => Ok(Some(v.c_name.clone())),
                    other => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("expected bytes/bytes_view/vec_u8, got {other:?}"),
                    )),
                }
            }
            _ => {
                let ty = self.infer_expr_in_new_scope(expr)?;
                match ty.ty {
                    Ty::BytesView | Ty::OptionBytesView | Ty::ResultBytesView => {
                        self.borrow_of_view_like_expr(ty.ty, expr)
                    }
                    Ty::Bytes | Ty::VecU8 => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "bytes_view borrow source requires an identifier owner".to_string(),
                    )),
                    other => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("expected bytes/bytes_view/vec_u8, got {other:?}"),
                    )),
                }
            }
        }
    }

    pub(super) fn compute_view_return_args(&mut self) -> Result<(), CompilerError> {
        let mut cache_view: BTreeMap<String, Option<usize>> = BTreeMap::new();
        let mut visiting_view: BTreeSet<String> = BTreeSet::new();
        for f in &self.program.functions {
            if f.ret_ty != Ty::BytesView {
                continue;
            }
            self.view_return_arg_for_fn(&f.name, &mut cache_view, &mut visiting_view)
                .map_err(|e| {
                    CompilerError::new(
                        e.kind,
                        format!("{} (bytes_view return analysis: {:?})", e.message, f.name),
                    )
                })?;
        }
        self.fn_view_return_arg = cache_view.clone();

        let mut cache_view_seed = cache_view;
        let mut visiting_view_seed: BTreeSet<String> = BTreeSet::new();

        let mut cache_opt: BTreeMap<String, Option<usize>> = BTreeMap::new();
        let mut visiting_opt: BTreeSet<String> = BTreeSet::new();
        for f in &self.program.functions {
            if f.ret_ty != Ty::OptionBytesView {
                continue;
            }
            self.option_bytes_view_return_arg_for_fn(
                &f.name,
                &mut cache_opt,
                &mut visiting_opt,
                &mut cache_view_seed,
                &mut visiting_view_seed,
            )
            .map_err(|e| {
                CompilerError::new(
                    e.kind,
                    format!(
                        "{} (option_bytes_view return analysis: {:?})",
                        e.message, f.name
                    ),
                )
            })?;
        }
        self.fn_option_bytes_view_return_arg = cache_opt;

        let mut cache_res: BTreeMap<String, Option<usize>> = BTreeMap::new();
        let mut visiting_res: BTreeSet<String> = BTreeSet::new();
        for f in &self.program.functions {
            if f.ret_ty != Ty::ResultBytesView {
                continue;
            }
            self.result_bytes_view_return_arg_for_fn(
                &f.name,
                &mut cache_res,
                &mut visiting_res,
                &mut cache_view_seed,
                &mut visiting_view_seed,
            )
            .map_err(|e| {
                CompilerError::new(
                    e.kind,
                    format!(
                        "{} (result_bytes_view return analysis: {:?})",
                        e.message, f.name
                    ),
                )
            })?;
        }
        self.fn_result_bytes_view_return_arg = cache_res;
        Ok(())
    }

    pub(super) fn view_return_arg_for_fn(
        &self,
        fn_name: &str,
        cache: &mut BTreeMap<String, Option<usize>>,
        visiting: &mut BTreeSet<String>,
    ) -> Result<Option<usize>, CompilerError> {
        if let Some(spec) = cache.get(fn_name) {
            return Ok(*spec);
        }
        if !visiting.insert(fn_name.to_string()) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("recursive bytes_view return borrow analysis: {fn_name:?}"),
            ));
        }

        let Some(f) = self.program.functions.iter().find(|f| f.name == fn_name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!("internal error: missing function def for {fn_name:?}"),
            ));
        };
        if f.ret_ty != Ty::BytesView {
            visiting.remove(fn_name);
            return Ok(None);
        }

        let mut env = ViewBorrowEnv::new();
        for (idx, p) in f.params.iter().enumerate() {
            match p.ty {
                Ty::BytesView | Ty::OptionBytesView | Ty::ResultBytesView => {
                    env.bind(p.name.clone(), ViewBorrowFrom::Param(idx))
                }
                Ty::Bytes | Ty::VecU8 => {
                    env.bind(p.name.clone(), ViewBorrowFrom::LocalOwned(p.name.clone()))
                }
                _ => {}
            }
        }

        let mut collector = ViewBorrowCollector::default();
        let body_src = self.infer_view_borrow_from_expr(
            fn_name,
            &f.body,
            &mut env,
            cache,
            visiting,
            &mut collector,
        )?;
        if let Some(src) = body_src {
            collector.merge(fn_name, src)?;
        }

        let Some(final_src) = collector.src else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("function {fn_name:?} returns bytes_view but no borrow source could be inferred"),
            ));
        };

        let spec = match final_src {
            ViewBorrowFrom::Runtime => None,
            ViewBorrowFrom::Param(i) => Some(i),
            ViewBorrowFrom::LocalOwned(owner) => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "function {fn_name:?} returns bytes_view borrowed from local owned buffer {owner:?}"
                    ),
                ));
            }
        };
        cache.insert(fn_name.to_string(), spec);
        visiting.remove(fn_name);
        Ok(spec)
    }

    pub(super) fn option_bytes_view_return_arg_for_fn(
        &self,
        fn_name: &str,
        cache: &mut BTreeMap<String, Option<usize>>,
        visiting: &mut BTreeSet<String>,
        view_cache: &mut BTreeMap<String, Option<usize>>,
        view_visiting: &mut BTreeSet<String>,
    ) -> Result<Option<usize>, CompilerError> {
        let mut cache = ViewBorrowFnCache::new(cache, visiting);
        let mut view_cache = ViewBorrowFnCache::new(view_cache, view_visiting);

        if let Some(spec) = cache.cache.get(fn_name) {
            return Ok(*spec);
        }
        if !cache.visiting.insert(fn_name.to_string()) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("recursive option_bytes_view return borrow analysis: {fn_name:?}"),
            ));
        }

        let Some(f) = self.program.functions.iter().find(|f| f.name == fn_name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!("internal error: missing function def for {fn_name:?}"),
            ));
        };
        if f.ret_ty != Ty::OptionBytesView {
            cache.visiting.remove(fn_name);
            return Ok(None);
        }

        let mut env = ViewBorrowEnv::new();
        for (idx, p) in f.params.iter().enumerate() {
            match p.ty {
                Ty::BytesView | Ty::OptionBytesView | Ty::ResultBytesView => {
                    env.bind(p.name.clone(), ViewBorrowFrom::Param(idx))
                }
                Ty::Bytes | Ty::VecU8 => {
                    env.bind(p.name.clone(), ViewBorrowFrom::LocalOwned(p.name.clone()))
                }
                _ => {}
            }
        }

        let mut collector = ViewBorrowCollector::default();
        let body_src = self.infer_option_bytes_view_borrow_from_expr(
            fn_name,
            &f.body,
            &mut env,
            &mut cache,
            &mut view_cache,
            &mut collector,
        )?;
        if let Some(src) = body_src {
            collector.merge(fn_name, src)?;
        }

        let final_src = collector.src.unwrap_or(ViewBorrowFrom::Runtime);
        let spec = match final_src {
            ViewBorrowFrom::Runtime => None,
            ViewBorrowFrom::Param(i) => Some(i),
            ViewBorrowFrom::LocalOwned(owner) => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "function {fn_name:?} returns option_bytes_view borrowed from local owned buffer {owner:?}"
                    ),
                ));
            }
        };
        cache.cache.insert(fn_name.to_string(), spec);
        cache.visiting.remove(fn_name);
        Ok(spec)
    }

    pub(super) fn require_option_bytes_view_borrow_from_expr(
        &self,
        fn_name: &str,
        expr: &Expr,
        env: &mut ViewBorrowEnv,
        cache: &mut ViewBorrowFnCache,
        view_cache: &mut ViewBorrowFnCache,
        collector: &mut ViewBorrowCollector,
    ) -> Result<ViewBorrowFrom, CompilerError> {
        if let Some(src) = self.infer_option_bytes_view_borrow_from_expr(
            fn_name, expr, env, cache, view_cache, collector,
        )? {
            return Ok(src);
        }
        Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("cannot infer option_bytes_view borrow source in function {fn_name:?}"),
        ))
    }

    pub(super) fn result_bytes_view_return_arg_for_fn(
        &self,
        fn_name: &str,
        cache: &mut BTreeMap<String, Option<usize>>,
        visiting: &mut BTreeSet<String>,
        view_cache: &mut BTreeMap<String, Option<usize>>,
        view_visiting: &mut BTreeSet<String>,
    ) -> Result<Option<usize>, CompilerError> {
        let mut cache = ViewBorrowFnCache::new(cache, visiting);
        let mut view_cache = ViewBorrowFnCache::new(view_cache, view_visiting);

        if let Some(spec) = cache.cache.get(fn_name) {
            return Ok(*spec);
        }
        if !cache.visiting.insert(fn_name.to_string()) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("recursive result_bytes_view return borrow analysis: {fn_name:?}"),
            ));
        }

        let Some(f) = self.program.functions.iter().find(|f| f.name == fn_name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!("internal error: missing function def for {fn_name:?}"),
            ));
        };
        if f.ret_ty != Ty::ResultBytesView {
            cache.visiting.remove(fn_name);
            return Ok(None);
        }

        let mut env = ViewBorrowEnv::new();
        for (idx, p) in f.params.iter().enumerate() {
            match p.ty {
                Ty::BytesView | Ty::OptionBytesView | Ty::ResultBytesView => {
                    env.bind(p.name.clone(), ViewBorrowFrom::Param(idx))
                }
                Ty::Bytes | Ty::VecU8 => {
                    env.bind(p.name.clone(), ViewBorrowFrom::LocalOwned(p.name.clone()))
                }
                _ => {}
            }
        }

        let mut collector = ViewBorrowCollector::default();
        let body_src = self.infer_result_bytes_view_borrow_from_expr(
            fn_name,
            &f.body,
            &mut env,
            &mut cache,
            &mut view_cache,
            &mut collector,
        )?;
        if let Some(src) = body_src {
            collector.merge(fn_name, src)?;
        }

        let final_src = collector.src.unwrap_or(ViewBorrowFrom::Runtime);
        let spec = match final_src {
            ViewBorrowFrom::Runtime => None,
            ViewBorrowFrom::Param(i) => Some(i),
            ViewBorrowFrom::LocalOwned(owner) => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "function {fn_name:?} returns result_bytes_view borrowed from local owned buffer {owner:?}"
                    ),
                ));
            }
        };
        cache.cache.insert(fn_name.to_string(), spec);
        cache.visiting.remove(fn_name);
        Ok(spec)
    }

    pub(super) fn require_result_bytes_view_borrow_from_expr(
        &self,
        fn_name: &str,
        expr: &Expr,
        env: &mut ViewBorrowEnv,
        cache: &mut ViewBorrowFnCache,
        view_cache: &mut ViewBorrowFnCache,
        collector: &mut ViewBorrowCollector,
    ) -> Result<ViewBorrowFrom, CompilerError> {
        if let Some(src) = self.infer_result_bytes_view_borrow_from_expr(
            fn_name, expr, env, cache, view_cache, collector,
        )? {
            return Ok(src);
        }
        Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("cannot infer result_bytes_view borrow source in function {fn_name:?}"),
        ))
    }

    pub(super) fn require_view_borrow_from_expr(
        &self,
        fn_name: &str,
        expr: &Expr,
        env: &mut ViewBorrowEnv,
        cache: &mut BTreeMap<String, Option<usize>>,
        visiting: &mut BTreeSet<String>,
        collector: &mut ViewBorrowCollector,
    ) -> Result<ViewBorrowFrom, CompilerError> {
        if let Some(src) =
            self.infer_view_borrow_from_expr(fn_name, expr, env, cache, visiting, collector)?
        {
            return Ok(src);
        }

        match expr {
            Expr::Ident { name, .. } if name != "input" => {
                Ok(ViewBorrowFrom::LocalOwned(name.to_string()))
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("cannot infer bytes_view borrow source in function {fn_name:?}"),
            )),
        }
    }

    pub(super) fn recompute_borrow_counts(&mut self) -> Result<(), CompilerError> {
        for scope in self.scopes.iter_mut() {
            for v in scope.values_mut() {
                v.borrow_count = 0;
            }
        }

        let mut borrows = Vec::<String>::new();
        for scope in &self.scopes {
            for v in scope.values() {
                if is_view_like_ty(v.ty) {
                    if let Some(owner) = &v.borrow_of {
                        borrows.push(owner.clone());
                    }
                }
            }
        }
        for owner in borrows {
            self.inc_borrow_count(&owner)?;
        }

        for scope in &self.scopes {
            for v in scope.values() {
                if is_owned_ty(v.ty) && v.moved && v.borrow_count != 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "borrow of moved value".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    pub(super) fn live_owned_drop_list(&self, skip_c_name: Option<&str>) -> Vec<(Ty, String)> {
        let mut to_drop = Vec::new();
        for scope in self.scopes.iter().rev() {
            for var in scope.values() {
                if !is_owned_ty(var.ty) {
                    continue;
                }
                if skip_c_name.is_some_and(|skip| skip == var.c_name) {
                    continue;
                }
                to_drop.push((var.ty, var.c_name.clone()));
            }
        }
        to_drop
    }

    pub(super) fn merge_if_states(
        &self,
        before: &[BTreeMap<String, VarRef>],
        then_state: &[BTreeMap<String, VarRef>],
        else_state: &[BTreeMap<String, VarRef>],
    ) -> Result<Vec<BTreeMap<String, VarRef>>, CompilerError> {
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
                v.borrow_count = 0;
                if is_view_like_ty(v.ty) {
                    v.borrow_of = match (t.borrow_of.clone(), e.borrow_of.clone()) {
                        (Some(a), Some(b)) => {
                            if a != b {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!(
                                        "{:?} must have a single borrow source across branches",
                                        v.ty
                                    ),
                                ));
                            }
                            Some(a)
                        }
                        (Some(a), None) | (None, Some(a)) => Some(a),
                        (None, None) => None,
                    };
                    v.borrow_ptr = match (t.borrow_ptr.clone(), e.borrow_ptr.clone()) {
                        (Some(a), Some(_b)) => Some(a),
                        (Some(a), None) | (None, Some(a)) => Some(a),
                        (None, None) => None,
                    };
                }

                merged.insert(name.clone(), v);
            }
            merged_scopes.push(merged);
        }
        Ok(merged_scopes)
    }

    pub(super) fn require_standalone_only(&self, head: &str) -> Result<(), CompilerError> {
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
}

const RUNTIME_C_PREAMBLE: &str = r#"
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif

#ifndef X07_FREESTANDING
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>
#include <poll.h>
#include <spawn.h>
#include <sys/mman.h>
#include <sys/wait.h>
#ifndef MAP_ANON
#define MAP_ANON MAP_ANONYMOUS
#endif
#else
#include <stddef.h>
#include <stdint.h>

void* memcpy(void* dst, const void* src, size_t n);
void* memmove(void* dst, const void* src, size_t n);
void* memset(void* dst, int c, size_t n);
int memcmp(const void* a, const void* b, size_t n);
char* getenv(const char* name);
int snprintf(char* s, size_t n, const char* fmt, ...);
#endif

#ifndef X07_MEM_CAP
#define X07_MEM_CAP (64u * 1024u * 1024u)
#endif

#ifndef X07_FUEL_INIT
#define X07_FUEL_INIT 50000000ULL
#endif

#ifndef X07_ENABLE_FS
#define X07_ENABLE_FS 0
#endif

#ifndef X07_ENABLE_RR
#define X07_ENABLE_RR 0
#endif

#ifndef X07_ENABLE_KV
#define X07_ENABLE_KV 0
#endif

#define X07_ENABLE_STREAMING_FILE_IO (X07_ENABLE_FS || X07_ENABLE_RR || X07_ENABLE_KV)

#ifdef X07_FREESTANDING
#if X07_ENABLE_FS || X07_ENABLE_RR || X07_ENABLE_KV
#error "X07_FREESTANDING requires X07_ENABLE_FS/RR/KV=0"
#endif
#endif

#ifdef X07_DEBUG_BORROW
#ifndef X07_DBG_ALLOC_CAP
#define X07_DBG_ALLOC_CAP 65536u
#endif
#ifndef X07_DBG_BORROW_CAP
#define X07_DBG_BORROW_CAP 65536u
#endif
#endif

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} bytes_t;

typedef struct {
  uint8_t* ptr;
  uint32_t len;
#ifdef X07_DEBUG_BORROW
  uint64_t aid;
  uint64_t bid;
  uint32_t off_bytes;
#endif
} bytes_view_t;

typedef struct {
  uint32_t tag;
  uint32_t payload;
} option_i32_t;

typedef struct {
  uint32_t tag;
  bytes_t payload;
} option_bytes_t;

typedef struct {
  uint32_t tag;
  bytes_view_t payload;
} option_bytes_view_t;

typedef struct {
  uint32_t tag;
  union {
    uint32_t ok;
    uint32_t err;
  } payload;
} result_i32_t;

typedef struct {
  uint32_t tag;
  union {
    bytes_t ok;
    uint32_t err;
  } payload;
} result_bytes_t;

typedef struct {
  uint32_t tag;
  union {
    bytes_view_t ok;
    uint32_t err;
  } payload;
} result_bytes_view_t;

typedef struct {
  uint32_t tag;
  union {
    result_bytes_t ok;
    uint32_t err;
  } payload;
} result_result_bytes_t;

#define RT_TASK_OUT_KIND_BYTES UINT32_C(1)
#define RT_TASK_OUT_KIND_RESULT_BYTES UINT32_C(2)

typedef struct {
  uint32_t kind;
  union {
    bytes_t bytes;
    result_bytes_t result_bytes;
  } payload;
} rt_task_out_t;

typedef struct {
  uint32_t data;
  uint32_t vtable;
} iface_t;

static __attribute__((noreturn)) void rt_trap(const char* msg);
static __attribute__((noreturn)) void rt_trap_path(const char* msg, const char* path);

#define RT_IFACE_VTABLE_IO_READER UINT32_C(1)
#define RT_IFACE_VTABLE_EXT_IO_READER_MIN UINT32_C(2)
#define RT_IFACE_VTABLE_EXT_IO_READER_MAX UINT32_C(64)

typedef uint32_t (*rt_ext_io_reader_read_fn_t)(uint32_t data, uint8_t* dst, uint32_t cap);
typedef void (*rt_ext_io_reader_drop_fn_t)(uint32_t data);

typedef struct {
  rt_ext_io_reader_read_fn_t read;
  rt_ext_io_reader_drop_fn_t drop;
} rt_ext_io_reader_vtable_t;

static rt_ext_io_reader_vtable_t rt_ext_io_reader_vtables[
  RT_IFACE_VTABLE_EXT_IO_READER_MAX - RT_IFACE_VTABLE_EXT_IO_READER_MIN + 1
];
static uint32_t rt_ext_io_reader_vtables_len = 0;

// External packages register IO reader vtables at runtime to enable `iface` streaming
// through `io.read` / `bufread.*` without adding new builtins.
uint32_t x07_rt_register_io_reader_vtable_v1(
  rt_ext_io_reader_read_fn_t read,
  rt_ext_io_reader_drop_fn_t drop
) {
  if (!read) return 0;

  for (uint32_t i = 0; i < rt_ext_io_reader_vtables_len; i++) {
    rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[i];
    if (vt->read == read && vt->drop == drop) {
      return RT_IFACE_VTABLE_EXT_IO_READER_MIN + i;
    }
  }

  uint32_t cap = (uint32_t)(sizeof(rt_ext_io_reader_vtables) / sizeof(rt_ext_io_reader_vtables[0]));
  if (rt_ext_io_reader_vtables_len >= cap) return 0;

  rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[rt_ext_io_reader_vtables_len];
  vt->read = read;
  vt->drop = drop;

  uint32_t id = RT_IFACE_VTABLE_EXT_IO_READER_MIN + rt_ext_io_reader_vtables_len;
  rt_ext_io_reader_vtables_len += 1;
  return id;
}

static uint32_t rt_ext_io_reader_read_into(uint32_t vtable, uint32_t data, uint8_t* dst, uint32_t cap) {
  if (vtable < RT_IFACE_VTABLE_EXT_IO_READER_MIN || vtable > RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    rt_trap("ext io reader invalid vtable");
  }
  uint32_t idx = vtable - RT_IFACE_VTABLE_EXT_IO_READER_MIN;
  if (idx >= rt_ext_io_reader_vtables_len) {
    rt_trap("ext io reader unregistered vtable");
  }
  rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[idx];
  if (!vt->read) {
    rt_trap("ext io reader missing read fn");
  }
  uint32_t got = vt->read(data, dst, cap);
  if (got > cap) {
    rt_trap("ext io reader returned too many bytes");
  }
  return got;
}

typedef struct {
  bytes_t key;
  bytes_t val;
} kv_entry_t;

typedef struct {
  bytes_t path;
  uint32_t ticks;
} fs_latency_entry_t;

typedef struct {
  uint32_t used;

  uint32_t latency_ticks;

  uint32_t kind_off;
  uint32_t kind_len;

  uint32_t op_off;
  uint32_t op_len;

  uint32_t key_off;
  uint32_t key_len;

  uint32_t payload_off;
  uint32_t payload_len;
} rr_entry_desc_t;

typedef struct {
  bytes_t path;
  bytes_t blob;
  rr_entry_desc_t* entries;
  uint32_t entries_len;
  uint32_t entries_cap;

  uint64_t file_bytes;
  void* append_f;
} rr_cassette_t;

typedef struct {
  uint32_t alive;
  uint8_t mode;
  uint8_t match_mode;
  uint16_t reserved;

  uint64_t max_cassette_bytes;
  uint32_t max_entries;
  uint32_t max_req_bytes;
  uint32_t max_resp_bytes;
  uint32_t max_key_bytes;

  uint32_t transcript_cassette;
  uint32_t transcript_idx;

  rr_cassette_t* cassettes;
  uint32_t cassettes_len;
  uint32_t cassettes_cap;
} rr_handle_t;

typedef struct {
  bytes_t key;
  uint32_t ticks;
} kv_latency_entry_t;

typedef struct {
  uint8_t* mem;
  uint32_t cap;
  uint32_t free_head;
} heap_t;

// Heap allocator is fixed-capacity and deterministic.
// It uses a singly-linked free list of blocks (sorted by address) with coalescing.
#define RT_HEAP_ALIGN UINT32_C(16)
#define RT_HEAP_MAGIC_FREE UINT32_C(0x45564f46) // "X07F"
#define RT_HEAP_MAGIC_USED UINT32_C(0x45564f55) // "X07U"
#define RT_HEAP_NULL_OFF UINT32_MAX

typedef struct {
  uint32_t size;     // total block size including header, multiple of RT_HEAP_ALIGN
  uint32_t next_off; // free: free-list next (offset from heap base), used: allocation epoch id
  uint32_t magic;    // RT_HEAP_MAGIC_FREE / RT_HEAP_MAGIC_USED
  uint32_t req_size; // requested payload size in bytes
} heap_hdr_t;

static uint32_t rt_heap_align_u32(uint32_t x) {
  return (x + (RT_HEAP_ALIGN - 1u)) & ~(RT_HEAP_ALIGN - 1u);
}

typedef struct {
  uint64_t alloc_calls;
  uint64_t realloc_calls;
  uint64_t free_calls;

  uint64_t bytes_alloc_total;
  uint64_t bytes_freed_total;

  uint64_t live_bytes;
  uint64_t peak_live_bytes;

  uint64_t live_allocs;
  uint64_t peak_live_allocs;

  uint64_t memcpy_bytes;
} mem_stats_t;

typedef struct {
  void* ctx;
  void* (*alloc)(void* ctx, uint32_t size, uint32_t align);
  void* (*realloc)(void* ctx, void* ptr, uint32_t old_size, uint32_t new_size, uint32_t align);
  void (*free)(void* ctx, void* ptr, uint32_t size, uint32_t align);
} allocator_v1_t;

__attribute__((weak)) allocator_v1_t x07_custom_allocator(void) {
  return (allocator_v1_t){0};
}

#ifdef X07_DEBUG_BORROW
typedef struct {
  uint8_t* base_ptr;
  uint32_t size_bytes;
  uint32_t alive;
  uint64_t borrow_id;
} dbg_alloc_rec_t;

typedef struct {
  uint64_t alloc_id;
  uint32_t off_bytes;
  uint32_t len_bytes;
  uint32_t active;
} dbg_borrow_rec_t;
#endif

typedef struct {
  uint64_t tasks_spawned;
  uint64_t spawn_calls;
  uint64_t join_calls;
  uint64_t yield_calls;
  uint64_t sleep_calls;
  uint64_t chan_send_calls;
  uint64_t chan_recv_calls;
  uint64_t ctx_switches;
  uint64_t wake_events;
  uint64_t blocked_waits;
  uint64_t virtual_time_end;
  uint64_t sched_trace_hash;
} sched_stats_t;

typedef struct rt_task_s rt_task_t;
typedef struct rt_timer_ev_s rt_timer_ev_t;
typedef struct rt_chan_bytes_s rt_chan_bytes_t;
typedef struct rt_select_evt_s rt_select_evt_t;
typedef struct rt_io_reader_s rt_io_reader_t;
typedef struct rt_bufread_s rt_bufread_t;
typedef struct rt_scratch_u8_fixed_s rt_scratch_u8_fixed_t;
typedef struct rt_os_proc_s rt_os_proc_t;

#define X07_ASSERT_BYTES_EQ_PREFIX_MAX 64

typedef struct {
  uint64_t fuel_init;
  uint64_t fuel;
  int32_t exit_code;
  uint32_t budget_fuel_depth;
  heap_t heap;
  allocator_v1_t allocator;
  uint32_t allocator_is_custom;

  // Total heap usage counters (all allocations, including input/runtime metadata).
  uint64_t heap_live_bytes;
  uint64_t heap_peak_live_bytes;
  uint64_t heap_live_allocs;
  uint64_t heap_peak_live_allocs;

  // Epoch id for mem_stats tracking (0 means "not started yet").
  uint32_t mem_epoch_id;
  mem_stats_t mem_stats;
#ifdef X07_DEBUG_BORROW
  dbg_alloc_rec_t* dbg_allocs;
  uint32_t dbg_allocs_len;
  uint32_t dbg_allocs_cap;

  dbg_borrow_rec_t* dbg_borrows;
  uint32_t dbg_borrows_len;
  uint32_t dbg_borrows_cap;

  uint64_t dbg_borrow_violations;
#endif
  uint64_t fs_read_file_calls;
  uint64_t fs_list_dir_calls;
  uint64_t rr_open_calls;
  uint64_t rr_close_calls;
  uint64_t rr_stats_calls;
  uint64_t rr_next_calls;
  uint64_t rr_next_miss_calls;
  uint64_t rr_append_calls;

  int32_t rr_current;

  rr_handle_t* rr_handles;
  uint32_t rr_handles_len;
  uint32_t rr_handles_cap;
  uint64_t kv_get_calls;
  uint64_t kv_set_calls;

  // Phase G2 fixture-backed latency indices (loaded lazily).
  uint32_t fs_latency_loaded;
  uint32_t fs_latency_default_ticks;
  fs_latency_entry_t* fs_latency_entries;
  uint32_t fs_latency_len;
  bytes_t fs_latency_blob;

  uint32_t kv_latency_loaded;
  uint32_t kv_latency_default_ticks;
  kv_latency_entry_t* kv_latency_entries;
  uint32_t kv_latency_len;
  bytes_t kv_latency_blob;

  kv_entry_t* kv_items;
  uint32_t kv_len;
  uint32_t kv_cap;

  void** map_u32_items;
  uint32_t map_u32_len;
  uint32_t map_u32_cap;

  void** vec_value_items;
  uint32_t vec_value_len;
  uint32_t vec_value_cap;

  void** map_value_items;
  uint32_t map_value_len;
  uint32_t map_value_cap;

  // Phase G2 scheduler + concurrency (deterministic, single-thread cooperative).
  uint32_t sched_current_task;
  uint64_t sched_now_ticks;
  uint64_t sched_seq;
  sched_stats_t sched_stats;

  rt_task_t* sched_tasks;
  uint32_t sched_tasks_len;
  uint32_t sched_tasks_cap;

  uint32_t sched_ready_head;
  uint32_t sched_ready_tail;

  rt_timer_ev_t* sched_timers;
  uint32_t sched_timers_len;
  uint32_t sched_timers_cap;

  rt_chan_bytes_t* sched_chans;
  uint32_t sched_chans_len;
  uint32_t sched_chans_cap;

  rt_select_evt_t* sched_select_evts;
  uint32_t sched_select_evts_len;
  uint32_t sched_select_evts_cap;

  // Phase G2 streaming I/O (deterministic, fixture-backed).
  rt_io_reader_t* io_readers;
  uint32_t io_readers_len;
  uint32_t io_readers_cap;

  rt_bufread_t* bufreads;
  uint32_t bufreads_len;
  uint32_t bufreads_cap;

  // Deterministic fixed-capacity scratch buffers.
  rt_scratch_u8_fixed_t* scratch_u8_fixed;
  uint32_t scratch_u8_fixed_len;
  uint32_t scratch_u8_fixed_cap;

  // Standalone OS process table (run-os*, non-deterministic).
  rt_os_proc_t* os_procs;
  uint32_t os_procs_len;
  uint32_t os_procs_cap;
  uint32_t os_procs_live;
  uint32_t os_procs_spawned;

  const char* trap_ptr;

  uint32_t last_bytes_eq_valid;
  uint32_t last_bytes_eq_a_len;
  uint32_t last_bytes_eq_b_len;
  uint8_t last_bytes_eq_a_prefix[X07_ASSERT_BYTES_EQ_PREFIX_MAX];
  uint8_t last_bytes_eq_b_prefix[X07_ASSERT_BYTES_EQ_PREFIX_MAX];
} ctx_t;

// Global ctx pointer for native extension backends that need to allocate bytes via the runtime.
static ctx_t* rt_ext_ctx = NULL;

// Native math backend entrypoints (linked from deps/x07/libx07_math.*).
bytes_t ev_math_f64_add_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_sub_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_mul_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_div_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_neg_v1(bytes_t x);
bytes_t ev_math_f64_abs_v1(bytes_t x);
bytes_t ev_math_f64_min_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_max_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_sqrt_v1(bytes_t x);
bytes_t ev_math_f64_sin_v1(bytes_t x);
bytes_t ev_math_f64_cos_v1(bytes_t x);
bytes_t ev_math_f64_tan_v1(bytes_t x);
bytes_t ev_math_f64_exp_v1(bytes_t x);
bytes_t ev_math_f64_ln_v1(bytes_t x);
bytes_t ev_math_f64_pow_v1(bytes_t x, bytes_t y);
bytes_t ev_math_f64_atan2_v1(bytes_t y, bytes_t x);
bytes_t ev_math_f64_floor_v1(bytes_t x);
bytes_t ev_math_f64_ceil_v1(bytes_t x);
bytes_t ev_math_f64_fmt_shortest_v1(bytes_t x);
result_bytes_t ev_math_f64_parse_v1(bytes_t s);
bytes_t ev_math_f64_from_i32_v1(int32_t x);
result_i32_t ev_math_f64_to_i32_trunc_v1(bytes_t x);
bytes_t ev_math_f64_to_bits_u64le_v1(bytes_t x);

// Native time backend entrypoints (linked from deps/x07/libx07_time.*).
uint32_t ev_time_tzdb_is_valid_tzid_v1(bytes_t tzid);
bytes_t ev_time_tzdb_offset_duration_v1(bytes_t tzid, int32_t unix_s_lo, int32_t unix_s_hi);
bytes_t ev_time_tzdb_snapshot_id_v1(void);

// Native ext-fs backend entrypoints (linked from deps/x07/libx07_ext_fs.*).
result_bytes_t x07_ext_fs_read_all_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_write_all_v1(bytes_t path, bytes_t data, bytes_t caps);
result_i32_t x07_ext_fs_append_all_v1(bytes_t path, bytes_t data, bytes_t caps);
result_i32_t x07_ext_fs_mkdirs_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_remove_file_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_remove_dir_all_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_rename_v1(bytes_t src, bytes_t dst, bytes_t caps);
result_bytes_t x07_ext_fs_list_dir_sorted_text_v1(bytes_t path, bytes_t caps);
result_bytes_t x07_ext_fs_walk_glob_sorted_text_v1(bytes_t root, bytes_t glob, bytes_t caps);
result_bytes_t x07_ext_fs_stat_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_stream_open_write_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_stream_write_all_v1(int32_t writer_handle, bytes_t data);
result_i32_t x07_ext_fs_stream_close_v1(int32_t writer_handle);
int32_t x07_ext_fs_stream_drop_v1(int32_t writer_handle);

// Native ext-stdio backend entrypoints (linked from deps/x07/libx07_ext_stdio.*).
result_bytes_t x07_ext_stdio_read_line_v1(bytes_t caps);
result_i32_t x07_ext_stdio_write_stdout_v1(bytes_t data, bytes_t caps);
result_i32_t x07_ext_stdio_write_stderr_v1(bytes_t data, bytes_t caps);
result_i32_t x07_ext_stdio_flush_stdout_v1(void);
result_i32_t x07_ext_stdio_flush_stderr_v1(void);

// Native ext-rand backend entrypoints (linked from deps/x07/libx07_ext_rand.*).
result_bytes_t x07_ext_rand_bytes_v1(int32_t n, bytes_t caps);
result_bytes_t x07_ext_rand_u64_v1(bytes_t caps);

// Native ext-db-sqlite backend entrypoints (linked from deps/x07/libx07_ext_db_sqlite.*).
bytes_t x07_ext_db_sqlite_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-pg backend entrypoints (linked from deps/x07/libx07_ext_db_pg.*).
bytes_t x07_ext_db_pg_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-mysql backend entrypoints (linked from deps/x07/libx07_ext_db_mysql.*).
bytes_t x07_ext_db_mysql_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-redis backend entrypoints (linked from deps/x07/libx07_ext_db_redis.*).
bytes_t x07_ext_db_redis_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_redis_cmd_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_redis_close_v1(bytes_t req, bytes_t caps);

// Native ext-regex backend entrypoints (linked from deps/x07/libx07_ext_regex.*).
bytes_t x07_ext_regex_compile_opts_v1(bytes_t pat, int32_t opts_u32);
bytes_t x07_ext_regex_exec_from_v1(bytes_t compiled, bytes_t text, int32_t start_i32);
bytes_t x07_ext_regex_exec_caps_from_v1(bytes_t compiled, bytes_t text, int32_t start_i32);
bytes_t x07_ext_regex_find_all_x7sl_v1(bytes_t compiled, bytes_t text, int32_t max_matches_i32);
bytes_t x07_ext_regex_split_v1(bytes_t compiled, bytes_t text, int32_t max_parts_i32);
bytes_t x07_ext_regex_replace_all_v1(bytes_t compiled, bytes_t text, bytes_t repl, int32_t cap_limit_i32);

// Native ext-jsonschema backend entrypoints (linked from deps/x07/libx07_ext_jsonschema.*).
bytes_t x07_ext_jsonschema_compile_v1(bytes_t schema_json);
bytes_t x07_ext_jsonschema_validate_v1(bytes_t compiled, bytes_t instance_json);

#ifdef X07_STANDALONE
static uint32_t rt_os_process_poll_all(ctx_t* ctx, int poll_timeout_ms);
static void rt_os_process_cleanup(ctx_t* ctx);
#else
static uint32_t rt_os_process_poll_all(ctx_t* ctx, int poll_timeout_ms) {
  (void)ctx;
  (void)poll_timeout_ms;
  return UINT32_C(0);
}
static void rt_os_process_cleanup(ctx_t* ctx) {
  (void)ctx;
}
#endif

static __attribute__((noreturn)) void rt_trap(const char* msg) {

#ifndef X07_FREESTANDING
  if (msg) (void)write(STDERR_FILENO, msg, strlen(msg));
  if (rt_ext_ctx && rt_ext_ctx->trap_ptr) {
    const char* p = rt_ext_ctx->trap_ptr;
    (void)write(STDERR_FILENO, " ptr=", 5);
    (void)write(STDERR_FILENO, p, strlen(p));
  }
  if (msg || (rt_ext_ctx && rt_ext_ctx->trap_ptr)) (void)write(STDERR_FILENO, "\n", 1);
#else
  (void)msg;
#endif
  __builtin_trap();
}

static __attribute__((noreturn)) void rt_trap_path(const char* msg, const char* path) {

#ifndef X07_FREESTANDING
  if (msg) (void)write(STDERR_FILENO, msg, strlen(msg));
  if (path) {
    (void)write(STDERR_FILENO, " path=", 6);
    (void)write(STDERR_FILENO, path, strlen(path));
  }
  if (rt_ext_ctx && rt_ext_ctx->trap_ptr) {
    const char* p = rt_ext_ctx->trap_ptr;
    (void)write(STDERR_FILENO, " ptr=", 5);
    (void)write(STDERR_FILENO, p, strlen(p));
  }
  if (msg || path || (rt_ext_ctx && rt_ext_ctx->trap_ptr)) (void)write(STDERR_FILENO, "\n", 1);
#else
  (void)msg;
  (void)path;
#endif
  __builtin_trap();
}

static void rt_fuel(ctx_t* ctx, uint64_t amount) {
  if (ctx->fuel < amount) {
    if (ctx->budget_fuel_depth != 0) rt_trap("X07T_BUDGET_EXCEEDED_FUEL");
    rt_trap("fuel exhausted");
  }
  ctx->fuel -= amount;
}

static uint32_t rt_align_u32(uint32_t x, uint32_t align) {
  return (x + (align - 1u)) & ~(align - 1u);
}

static uint16_t rt_read_u16_le(const uint8_t* p) {
  return (uint16_t)p[0] | ((uint16_t)p[1] << 8);
}

static uint32_t rt_read_u32_le(const uint8_t* p) {
  return (uint32_t)p[0]
       | ((uint32_t)p[1] << 8)
       | ((uint32_t)p[2] << 16)
       | ((uint32_t)p[3] << 24);
}

static void rt_write_u32_le(uint8_t* p, uint32_t x) {
  p[0] = (uint8_t)(x & UINT32_C(0xFF));
  p[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  p[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  p[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
}

static void rt_heap_init(ctx_t* ctx) {
  if (!ctx->heap.mem) rt_trap("heap mem is NULL");
  uint32_t cap = ctx->heap.cap;
  cap &= ~(RT_HEAP_ALIGN - 1u);
  if (cap < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) rt_trap("heap too small");
  ctx->heap.cap = cap;

  ctx->heap.free_head = 0;
  heap_hdr_t* h = (heap_hdr_t*)(ctx->heap.mem);
  h->size = cap;
  h->next_off = RT_HEAP_NULL_OFF;
  h->magic = RT_HEAP_MAGIC_FREE;
  h->req_size = 0;
}

static heap_hdr_t* rt_heap_hdr_at(ctx_t* ctx, uint32_t off) {
  if (off == RT_HEAP_NULL_OFF) return (heap_hdr_t*)NULL;
  if (off > ctx->heap.cap || ctx->heap.cap - off < (uint32_t)sizeof(heap_hdr_t)) rt_trap("heap corrupt");
  return (heap_hdr_t*)(ctx->heap.mem + off);
}

static uint32_t rt_heap_off_of(ctx_t* ctx, heap_hdr_t* h) {
  uintptr_t base = (uintptr_t)(ctx->heap.mem);
  uintptr_t p = (uintptr_t)h;
  if (p < base) rt_trap("heap ptr underflow");
  uintptr_t off = p - base;
  if (off > (uintptr_t)UINT32_MAX) rt_trap("heap ptr overflow");
  if ((uint32_t)off > ctx->heap.cap) rt_trap("heap ptr oob");
  return (uint32_t)off;
}

static uint32_t rt_heap_is_pow2_u32(uint32_t x) {
  return x != 0 && (x & (x - 1u)) == 0;
}

static void* rt_heap_alloc(ctx_t* ctx, uint32_t size, uint32_t align) {
  if (size == 0) return (void*)ctx->heap.mem;
  if (align == 0) rt_trap("alloc align=0");
  if (!rt_heap_is_pow2_u32(align)) rt_trap("alloc align not pow2");
  if (align > RT_HEAP_ALIGN) rt_trap("alloc align too large");

  uint32_t payload = rt_heap_align_u32(size);
  uint32_t need = (uint32_t)sizeof(heap_hdr_t) + payload;
  need = rt_heap_align_u32(need);
  if (need < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) {
    need = (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN;
  }
  if (need > ctx->heap.cap) return NULL;

  uint32_t prev_off = RT_HEAP_NULL_OFF;
  uint32_t off = ctx->heap.free_head;
  while (off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* h = rt_heap_hdr_at(ctx, off);
    if (h->magic != RT_HEAP_MAGIC_FREE) rt_trap("heap free list corrupt");
    if (h->size >= need) {
      uint32_t next_off = h->next_off;

      // Remove from free list.
      if (prev_off == RT_HEAP_NULL_OFF) {
        ctx->heap.free_head = next_off;
      } else {
        heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
        prev->next_off = next_off;
      }

      uint32_t remaining = h->size - need;
      if (remaining >= (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) {
        uint32_t rem_off = off + need;
        heap_hdr_t* rem = rt_heap_hdr_at(ctx, rem_off);
        rem->size = remaining;
        rem->next_off = next_off;
        rem->magic = RT_HEAP_MAGIC_FREE;
        rem->req_size = 0;

        if (prev_off == RT_HEAP_NULL_OFF) {
          ctx->heap.free_head = rem_off;
        } else {
          heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
          prev->next_off = rem_off;
        }
        h->size = need;
      } else {
        // Don't split; keep whole block.
        need = h->size;
      }

      h->next_off = ctx->mem_epoch_id;
      h->magic = RT_HEAP_MAGIC_USED;
      h->req_size = size;

      void* ptr = (void*)(ctx->heap.mem + off + (uint32_t)sizeof(heap_hdr_t));
      memset(ptr, 0, payload);
      return ptr;
    }
    prev_off = off;
    off = h->next_off;
  }
  return NULL;
}

static void rt_heap_free(ctx_t* ctx, void* ptr) {
  if (!ptr) return;
  if ((uint8_t*)ptr == ctx->heap.mem) return;
  uint8_t* p = (uint8_t*)ptr;
  if (p < ctx->heap.mem + (uint32_t)sizeof(heap_hdr_t)) rt_trap("free oob");
  heap_hdr_t* h = (heap_hdr_t*)(p - (uint32_t)sizeof(heap_hdr_t));
  uint32_t off = rt_heap_off_of(ctx, h);
  if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("double free or corrupt heap");
  uint32_t size = h->size;
  if (size < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) rt_trap("free corrupt size");
  if ((size & (RT_HEAP_ALIGN - 1u)) != 0) rt_trap("free corrupt size");
  if (off > ctx->heap.cap || ctx->heap.cap - off < size) rt_trap("free oob");

  // Insert into free list (sorted by address).
  uint32_t prev_off = RT_HEAP_NULL_OFF;
  uint32_t cur_off = ctx->heap.free_head;
  while (cur_off != RT_HEAP_NULL_OFF && cur_off < off) {
    heap_hdr_t* cur = rt_heap_hdr_at(ctx, cur_off);
    prev_off = cur_off;
    cur_off = cur->next_off;
  }

  h->magic = RT_HEAP_MAGIC_FREE;
  h->next_off = cur_off;
  h->req_size = 0;

  if (prev_off == RT_HEAP_NULL_OFF) {
    ctx->heap.free_head = off;
  } else {
    heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
    prev->next_off = off;
  }

  // Coalesce with next.
  if (cur_off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* next = rt_heap_hdr_at(ctx, cur_off);
    if (off + h->size == cur_off) {
      h->size += next->size;
      h->next_off = next->next_off;
    }
  }

  // Coalesce with prev.
  if (prev_off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
    if (prev_off + prev->size == off) {
      prev->size += h->size;
      prev->next_off = h->next_off;
    }
  }
}

static void rt_mem_epoch_reset(ctx_t* ctx) {
  ctx->mem_epoch_id += 1;
  if (ctx->mem_epoch_id == 0) ctx->mem_epoch_id = 1;

  ctx->mem_stats.alloc_calls = 0;
  ctx->mem_stats.realloc_calls = 0;
  ctx->mem_stats.free_calls = 0;
  ctx->mem_stats.bytes_alloc_total = 0;
  ctx->mem_stats.bytes_freed_total = 0;
  ctx->mem_stats.live_bytes = 0;
  ctx->mem_stats.peak_live_bytes = 0;
  ctx->mem_stats.live_allocs = 0;
  ctx->mem_stats.peak_live_allocs = 0;
  ctx->mem_stats.memcpy_bytes = 0;
}

static void rt_mem_on_alloc(ctx_t* ctx, uint32_t size, uint32_t is_realloc) {
  ctx->heap_live_bytes += (uint64_t)size;
  ctx->heap_live_allocs += 1;
  if (ctx->heap_live_bytes > ctx->heap_peak_live_bytes) {
    ctx->heap_peak_live_bytes = ctx->heap_live_bytes;
  }
  if (ctx->heap_live_allocs > ctx->heap_peak_live_allocs) {
    ctx->heap_peak_live_allocs = ctx->heap_live_allocs;
  }

  if (ctx->mem_epoch_id == 0) return;

  if (is_realloc) {
    ctx->mem_stats.realloc_calls += 1;
  } else {
    ctx->mem_stats.alloc_calls += 1;
  }
  ctx->mem_stats.bytes_alloc_total += (uint64_t)size;
  ctx->mem_stats.live_bytes += (uint64_t)size;
  ctx->mem_stats.live_allocs += 1;
  if (ctx->mem_stats.live_bytes > ctx->mem_stats.peak_live_bytes) {
    ctx->mem_stats.peak_live_bytes = ctx->mem_stats.live_bytes;
  }
  if (ctx->mem_stats.live_allocs > ctx->mem_stats.peak_live_allocs) {
    ctx->mem_stats.peak_live_allocs = ctx->mem_stats.live_allocs;
  }
}

static void rt_mem_on_free(ctx_t* ctx, uint32_t size, uint32_t is_epoch, uint32_t strict) {
  if (ctx->heap_live_bytes < (uint64_t)size) rt_trap("mem free underflow");
  if (ctx->heap_live_allocs == 0) rt_trap("mem free underflow");
  ctx->heap_live_bytes -= (uint64_t)size;
  ctx->heap_live_allocs -= 1;

  if (ctx->mem_epoch_id == 0) return;
  if (!is_epoch) return;

  ctx->mem_stats.free_calls += 1;
  ctx->mem_stats.bytes_freed_total += (uint64_t)size;
  if (ctx->mem_stats.live_bytes < (uint64_t)size || ctx->mem_stats.live_allocs == 0) {
    if (strict) rt_trap("mem epoch underflow");
    return;
  }
  ctx->mem_stats.live_bytes -= (uint64_t)size;
  ctx->mem_stats.live_allocs -= 1;
}

static void rt_mem_on_memcpy(ctx_t* ctx, uint32_t size) {
  if (ctx->mem_epoch_id == 0) return;
  ctx->mem_stats.memcpy_bytes += (uint64_t)size;
}

static uint32_t rt_mem_epoch_pause(ctx_t* ctx) {
  uint32_t saved = ctx->mem_epoch_id;
  ctx->mem_epoch_id = 0;
  return saved;
}

static void rt_mem_epoch_resume(ctx_t* ctx, uint32_t saved) {
  ctx->mem_epoch_id = saved;
}

static void* rt_alloc_raw(ctx_t* ctx, uint32_t size, uint32_t align) {
  void* ptr = rt_heap_alloc(ctx, size, align);
  if (!ptr && size) rt_trap("out of memory");
  return ptr;
}

static void* rt_default_alloc(void* alloc_ctx, uint32_t size, uint32_t align) {
  return rt_alloc_raw((ctx_t*)alloc_ctx, size, align);
}

static void* rt_default_realloc(
    void* alloc_ctx,
    void* ptr,
    uint32_t old_size,
    uint32_t new_size,
    uint32_t align
) {
  (void)ptr;
  (void)old_size;
  return rt_alloc_raw((ctx_t*)alloc_ctx, new_size, align);
}

static void rt_default_free(void* alloc_ctx, void* ptr, uint32_t size, uint32_t align) {
  (void)size;
  (void)align;
  rt_heap_free((ctx_t*)alloc_ctx, ptr);
}

static void rt_allocator_init(ctx_t* ctx) {
  ctx->allocator.ctx = ctx;
  ctx->allocator.alloc = rt_default_alloc;
  ctx->allocator.realloc = rt_default_realloc;
  ctx->allocator.free = rt_default_free;
  ctx->allocator_is_custom = 0;

  allocator_v1_t custom = x07_custom_allocator();
  if (custom.alloc || custom.realloc || custom.free) {
    if (!custom.alloc || !custom.realloc || !custom.free) rt_trap("custom allocator missing hooks");
    ctx->allocator = custom;
    ctx->allocator_is_custom = 1;
  }
}

static void* rt_alloc(ctx_t* ctx, uint32_t size, uint32_t align) {
  if (size == 0) return (void*)ctx->heap.mem;
  if (!ctx->allocator.alloc) rt_trap("allocator.alloc missing");
  void* ptr = ctx->allocator.alloc(ctx->allocator.ctx, size, align);
  if (!ptr && size) rt_trap("allocator.alloc failed");
  rt_mem_on_alloc(ctx, size, 0);
  return ptr;
}

static void* rt_alloc_realloc(
    ctx_t* ctx,
    void* old_ptr,
    uint32_t old_size,
    uint32_t new_size,
    uint32_t align
) {
  if (new_size == 0) return (void*)ctx->heap.mem;
  if (!ctx->allocator.realloc) rt_trap("allocator.realloc missing");
  void* new_ptr =
      ctx->allocator.realloc(ctx->allocator.ctx, old_ptr, old_size, new_size, align);
  if (!new_ptr && new_size) rt_trap("allocator.realloc failed");
  rt_mem_on_alloc(ctx, new_size, old_size != 0);
  return new_ptr;
}

static void rt_free(ctx_t* ctx, void* ptr, uint32_t size, uint32_t align) {
  if (!ptr) return;
  if (size == 0) return;
  uint32_t is_epoch = 1;
  uint32_t strict = ctx->allocator_is_custom ? 0 : 1;
  if (!ctx->allocator_is_custom && (uint8_t*)ptr != ctx->heap.mem) {
    uint8_t* p = (uint8_t*)ptr;
    if (p < ctx->heap.mem + (uint32_t)sizeof(heap_hdr_t)) rt_trap("free oob");
    heap_hdr_t* h = (heap_hdr_t*)(p - (uint32_t)sizeof(heap_hdr_t));
    if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("free corrupt heap");
    is_epoch = (h->next_off == ctx->mem_epoch_id);
  }
  if (!ctx->allocator.free) rt_trap("allocator.free missing");
  ctx->allocator.free(ctx->allocator.ctx, ptr, size, align);
  rt_mem_on_free(ctx, size, is_epoch, strict);
}

static void rt_mem_free_all(ctx_t* ctx) {
  // Bulk reset used at process end.
  ctx->mem_stats.free_calls += 1;
  ctx->mem_stats.bytes_freed_total += ctx->mem_stats.live_bytes;
  ctx->mem_stats.live_bytes = 0;
  ctx->mem_stats.live_allocs = 0;

  ctx->heap_live_bytes = 0;
  ctx->heap_live_allocs = 0;
  rt_heap_init(ctx);
}

#ifdef X07_DEBUG_BORROW
static void rt_dbg_init(ctx_t* ctx) {
  ctx->dbg_allocs_len = 0;
  ctx->dbg_allocs_cap = X07_DBG_ALLOC_CAP;
  ctx->dbg_allocs =
      (dbg_alloc_rec_t*)rt_alloc(
          ctx,
          ctx->dbg_allocs_cap * (uint32_t)sizeof(dbg_alloc_rec_t),
          (uint32_t)_Alignof(dbg_alloc_rec_t)
      );

  ctx->dbg_borrows_len = 0;
  ctx->dbg_borrows_cap = X07_DBG_BORROW_CAP;
  ctx->dbg_borrows =
      (dbg_borrow_rec_t*)rt_alloc(
          ctx,
          ctx->dbg_borrows_cap * (uint32_t)sizeof(dbg_borrow_rec_t),
          (uint32_t)_Alignof(dbg_borrow_rec_t)
      );

  ctx->dbg_borrow_violations = 0;
}

static uint64_t rt_dbg_borrow_acquire(
    ctx_t* ctx,
    uint64_t alloc_id,
    uint32_t off_bytes,
    uint32_t len_bytes
);

static uint64_t rt_dbg_alloc_register(ctx_t* ctx, uint8_t* base_ptr, uint32_t size_bytes) {
  if (size_bytes == 0) return 0;
  if (!ctx->dbg_allocs || ctx->dbg_allocs_cap == 0) return 0;
  if (ctx->dbg_allocs_len >= ctx->dbg_allocs_cap) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t idx = ctx->dbg_allocs_len++;
  dbg_alloc_rec_t* rec = &ctx->dbg_allocs[idx];
  rec->base_ptr = base_ptr;
  rec->size_bytes = size_bytes;
  rec->alive = 1;
  rec->borrow_id = rt_dbg_borrow_acquire(ctx, (uint64_t)idx + 1, 0, size_bytes);
  return (uint64_t)idx + 1;
}

static void rt_dbg_alloc_kill(ctx_t* ctx, uint64_t alloc_id) {
  if (alloc_id == 0) return;
  uint64_t idx = alloc_id - 1;
  if (!ctx->dbg_allocs || idx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return;
  }
  ctx->dbg_allocs[idx].alive = 0;
}

static uint64_t rt_dbg_alloc_find(
    ctx_t* ctx,
    uint8_t* ptr,
    uint32_t len_bytes,
    uint32_t* out_off_bytes
) {
  if (out_off_bytes) *out_off_bytes = 0;
  if (len_bytes == 0) return 0;
  if (!ctx->dbg_allocs) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uintptr_t p = (uintptr_t)ptr;
  // Search newest-to-oldest so re-used pointers resolve to the latest allocation record.
  for (uint32_t i = ctx->dbg_allocs_len; i > 0; i--) {
    uint32_t idx = i - 1;
    dbg_alloc_rec_t* rec = &ctx->dbg_allocs[idx];
    uintptr_t base = (uintptr_t)rec->base_ptr;
    uintptr_t end = base + (uintptr_t)rec->size_bytes;
    if (end < base) continue;
    if (p >= base && p < end) {
      uint32_t off = (uint32_t)(p - base);
      if (off > rec->size_bytes || rec->size_bytes - off < len_bytes) {
        ctx->dbg_borrow_violations += 1;
        return 0;
      }
      if (out_off_bytes) *out_off_bytes = off;
      return (uint64_t)idx + 1;
    }
  }
  ctx->dbg_borrow_violations += 1;
  return 0;
}

static uint64_t rt_dbg_alloc_try_find(
    ctx_t* ctx,
    uint8_t* ptr,
    uint32_t len_bytes,
    uint32_t* out_off_bytes
) {
  if (out_off_bytes) *out_off_bytes = 0;
  if (len_bytes == 0) return 0;
  if (!ctx->dbg_allocs) return 0;
  uintptr_t p = (uintptr_t)ptr;
  // Search newest-to-oldest so re-used pointers resolve to the latest allocation record.
  for (uint32_t i = ctx->dbg_allocs_len; i > 0; i--) {
    uint32_t idx = i - 1;
    dbg_alloc_rec_t* rec = &ctx->dbg_allocs[idx];
    uintptr_t base = (uintptr_t)rec->base_ptr;
    uintptr_t end = base + (uintptr_t)rec->size_bytes;
    if (end < base) continue;
    if (p >= base && p < end) {
      uint32_t off = (uint32_t)(p - base);
      if (off > rec->size_bytes || rec->size_bytes - off < len_bytes) return 0;
      if (out_off_bytes) *out_off_bytes = off;
      return (uint64_t)idx + 1;
    }
  }
  return 0;
}

static uint64_t rt_dbg_alloc_borrow_id(ctx_t* ctx, uint64_t alloc_id) {
  if (alloc_id == 0) return 0;
  uint64_t idx = alloc_id - 1;
  if (!ctx->dbg_allocs || idx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t bid = ctx->dbg_allocs[idx].borrow_id;
  if (bid == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return bid;
}

static uint64_t rt_dbg_borrow_acquire(
    ctx_t* ctx,
    uint64_t alloc_id,
    uint32_t off_bytes,
    uint32_t len_bytes
) {
  if (len_bytes == 0) return 0;
  if (alloc_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t aidx = alloc_id - 1;
  if (!ctx->dbg_allocs || aidx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aidx];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes > a->size_bytes || a->size_bytes - off_bytes < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (!ctx->dbg_borrows || ctx->dbg_borrows_cap == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (ctx->dbg_borrows_len >= ctx->dbg_borrows_cap) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t idx = ctx->dbg_borrows_len++;
  dbg_borrow_rec_t* b = &ctx->dbg_borrows[idx];
  b->alloc_id = alloc_id;
  b->off_bytes = off_bytes;
  b->len_bytes = len_bytes;
  b->active = 1;
  return (uint64_t)idx + 1;
}

static void rt_dbg_borrow_release(ctx_t* ctx, uint64_t borrow_id) {
  if (borrow_id == 0) return;
  uint64_t idx = borrow_id - 1;
  if (!ctx->dbg_borrows || idx >= (uint64_t)ctx->dbg_borrows_len) {
    ctx->dbg_borrow_violations += 1;
    return;
  }
  ctx->dbg_borrows[idx].active = 0;
}

static uint32_t rt_dbg_borrow_check(
    ctx_t* ctx,
    uint64_t borrow_id,
    uint32_t off_bytes,
    uint32_t len_bytes
) {
  if (len_bytes == 0) return 1;
  if (borrow_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t idx = borrow_id - 1;
  if (!ctx->dbg_borrows || idx >= (uint64_t)ctx->dbg_borrows_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_borrow_rec_t* b = &ctx->dbg_borrows[idx];
  if (!b->active) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t alloc_id = b->alloc_id;
  if (alloc_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t aidx = alloc_id - 1;
  if (!ctx->dbg_allocs || aidx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aidx];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes < b->off_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t rel = off_bytes - b->off_bytes;
  if (rel > b->len_bytes || b->len_bytes - rel < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes > a->size_bytes || a->size_bytes - off_bytes < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return 1;
}
#endif

static inline bytes_t rt_bytes_empty(ctx_t* ctx) {
  bytes_t out;
  out.ptr = ctx->heap.mem;
  out.len = UINT32_C(0);
  return out;
}

static inline bytes_view_t rt_view_empty(ctx_t* ctx) {
  bytes_view_t out;
  out.ptr = ctx->heap.mem;
  out.len = UINT32_C(0);
#ifdef X07_DEBUG_BORROW
  out.aid = 0;
  out.bid = 0;
  out.off_bytes = 0;
#endif
  return out;
}

static inline rt_task_out_t rt_task_out_empty(ctx_t* ctx) {
  rt_task_out_t out;
  out.kind = RT_TASK_OUT_KIND_BYTES;
  out.payload.bytes = rt_bytes_empty(ctx);
  return out;
}

static bytes_t rt_bytes_alloc(ctx_t* ctx, uint32_t len) {
  if (len == 0) return rt_bytes_empty(ctx);
  bytes_t out;
  out.len = len;
  out.ptr = (uint8_t*)rt_alloc(ctx, len, 1);
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_alloc_register(ctx, out.ptr, len);
#endif
  return out;
}

// Native extension hook: allocate bytes using the currently-running ctx allocator.
bytes_t ev_bytes_alloc(uint32_t len) {
  if (!rt_ext_ctx) rt_trap(NULL);
  return rt_bytes_alloc(rt_ext_ctx, len);
}

// Native extension hook: trap without returning.
__attribute__((noreturn)) void ev_trap(int32_t code) {
  (void)code;
  rt_trap(NULL);
}

static bytes_t rt_bytes_from_literal(ctx_t* ctx, const uint8_t* ptr, uint32_t len) {
  bytes_t out = rt_bytes_alloc(ctx, len);
  if (len != 0) {
    memcpy(out.ptr, ptr, len);
    rt_mem_on_memcpy(ctx, len);
  }
  return out;
}

static bytes_view_t rt_view_from_literal(ctx_t* ctx, const uint8_t* ptr, uint32_t len) {
  if (len == 0) return rt_view_empty(ctx);
  bytes_view_t out;
  out.ptr = (uint8_t*)ptr;
  out.len = len;
#ifdef X07_DEBUG_BORROW
  uint32_t off = 0;
  uint64_t aid = rt_dbg_alloc_try_find(ctx, (uint8_t*)ptr, len, &off);
  if (aid == 0) {
    aid = rt_dbg_alloc_register(ctx, (uint8_t*)ptr, len);
    off = 0;
  }
  out.aid = aid;
  out.off_bytes = off;
  out.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#endif
  return out;
}

static bytes_t rt_bytes_clone(ctx_t* ctx, bytes_t src) {
  bytes_t out = rt_bytes_alloc(ctx, src.len);
  if (src.len != 0) {
    memcpy(out.ptr, src.ptr, src.len);
    rt_mem_on_memcpy(ctx, src.len);
  }
  return out;
}

static void rt_bytes_drop(ctx_t* ctx, bytes_t* b) {
  if (!b) return;
  if (b->len == 0) {
    b->ptr = ctx->heap.mem;
    return;
  }
#ifdef X07_DEBUG_BORROW
  uint64_t aid = rt_dbg_alloc_find(ctx, b->ptr, b->len, NULL);
  rt_dbg_alloc_kill(ctx, aid);
#endif
  uint32_t size = b->len;
  // Heap allocator stores the requested size in the allocation header; use it for exact accounting.
  if (ctx->allocator.free == rt_default_free) {
    heap_hdr_t* h = (heap_hdr_t*)(b->ptr - (uint32_t)sizeof(heap_hdr_t));
    if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("bytes.drop corrupt header");
    if (h->req_size == 0) rt_trap("bytes.drop corrupt header");
    size = h->req_size;
  }
  rt_free(ctx, b->ptr, size, 1);
  b->ptr = ctx->heap.mem;
  b->len = UINT32_C(0);
}

static void rt_task_out_drop(ctx_t* ctx, rt_task_out_t* out) {
  if (!out) return;
  if (out->kind == RT_TASK_OUT_KIND_BYTES) {
    rt_bytes_drop(ctx, &out->payload.bytes);
    out->payload.bytes = rt_bytes_empty(ctx);
    out->kind = RT_TASK_OUT_KIND_BYTES;
    return;
  }
  if (out->kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
    if (out->payload.result_bytes.tag) {
      rt_bytes_drop(ctx, &out->payload.result_bytes.payload.ok);
    }
    out->payload.result_bytes.tag = UINT32_C(0);
    out->payload.result_bytes.payload.err = UINT32_C(0);
    out->kind = RT_TASK_OUT_KIND_BYTES;
    out->payload.bytes = rt_bytes_empty(ctx);
    return;
  }
  rt_trap("task.out.drop invalid kind");
}

static bytes_t rt_view_to_bytes(ctx_t* ctx, bytes_view_t v) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, v.bid, v.off_bytes, v.len);
#endif
  bytes_t out = rt_bytes_alloc(ctx, v.len);
  if (v.len != 0) {
    memcpy(out.ptr, v.ptr, v.len);
    rt_mem_on_memcpy(ctx, v.len);
  }
  return out;
}

#ifdef X07_DEBUG_BORROW
static uint32_t rt_dbg_bytes_check(ctx_t* ctx, bytes_t b) {
  if (b.len == 0) return 1;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, NULL);
  if (aid == 0) return 0;
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aid - 1];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return 1;
}
#endif

static uint32_t rt_bytes_get_u8(ctx_t* ctx, bytes_t b, uint32_t idx) {
  if (idx >= b.len) rt_trap("bytes.get_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, b)) return 0;
#else
  (void)ctx;
#endif
  return (uint32_t)b.ptr[idx];
}

static bytes_t rt_bytes_set_u8(ctx_t* ctx, bytes_t b, uint32_t idx, uint32_t v) {
  if (idx >= b.len) rt_trap("bytes.set_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, b)) return b;
#else
  (void)ctx;
#endif
  b.ptr[idx] = (uint8_t)(v & UINT32_C(0xFF));
  return b;
}

static bytes_t rt_bytes_copy(ctx_t* ctx, bytes_t src, bytes_t dst) {
  if (dst.len < src.len) rt_trap("bytes.copy dst too small");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, src)) return dst;
  if (!rt_dbg_bytes_check(ctx, dst)) return dst;
#endif
  if (src.len != 0) {
    memcpy(dst.ptr, src.ptr, src.len);
    rt_mem_on_memcpy(ctx, src.len);
  }
  return dst;
}

static bytes_t rt_bytes_slice(ctx_t* ctx, bytes_t b, uint32_t start, uint32_t len) {
  if (start > b.len) rt_trap("bytes.slice oob");
  if (len > b.len - start) rt_trap("bytes.slice oob");

#ifdef X07_DEBUG_BORROW
  // Bulk ops bypass per-byte checks; validate once up front.
  uint32_t ok = rt_dbg_bytes_check(ctx, b);
  if (!ok && b.len != 0) {
    return rt_bytes_alloc(ctx, len);
  }
#endif

  bytes_t out = rt_bytes_alloc(ctx, len);
  if (len != 0) {
    memcpy(out.ptr, b.ptr + start, len);
    rt_mem_on_memcpy(ctx, len);
  }
  return out;
}

static bytes_t rt_bytes_concat(ctx_t* ctx, bytes_t a, bytes_t b) {
  uint32_t la = a.len;
  uint32_t lb = b.len;
  if (UINT32_MAX - la < lb) rt_trap("bytes.concat overflow");
#ifdef X07_DEBUG_BORROW
  uint32_t ok_a = rt_dbg_bytes_check(ctx, a);
  uint32_t ok_b = rt_dbg_bytes_check(ctx, b);
  if ((!ok_a && la != 0) || (!ok_b && lb != 0)) {
    return rt_bytes_alloc(ctx, la + lb);
  }
#endif

  bytes_t out = rt_bytes_alloc(ctx, la + lb);
  if (la != 0) {
    memcpy(out.ptr, a.ptr, la);
    rt_mem_on_memcpy(ctx, la);
  }
  if (lb != 0) {
    memcpy(out.ptr + la, b.ptr, lb);
    rt_mem_on_memcpy(ctx, lb);
  }
  return out;
}

static uint32_t rt_bytes_eq(ctx_t* ctx, bytes_t a, bytes_t b) {
  uint32_t a_prefix_len = 0;
  uint32_t b_prefix_len = 0;
  ctx->last_bytes_eq_valid = 0;
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, a)) return UINT32_C(0);
  if (!rt_dbg_bytes_check(ctx, b)) return UINT32_C(0);
#endif
  if (a.len != b.len) goto mismatch;
  if (a.len == 0) return UINT32_C(1);
#ifdef X07_DEBUG_BORROW
  // rt_dbg_bytes_check already ran above.
#endif
  if (memcmp(a.ptr, b.ptr, a.len) == 0) return UINT32_C(1);

mismatch:
  ctx->last_bytes_eq_valid = 1;
  ctx->last_bytes_eq_a_len = a.len;
  ctx->last_bytes_eq_b_len = b.len;
  a_prefix_len =
      (a.len < X07_ASSERT_BYTES_EQ_PREFIX_MAX) ? a.len : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
  b_prefix_len =
      (b.len < X07_ASSERT_BYTES_EQ_PREFIX_MAX) ? b.len : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
  if (a_prefix_len) memcpy(ctx->last_bytes_eq_a_prefix, a.ptr, a_prefix_len);
  if (b_prefix_len) memcpy(ctx->last_bytes_eq_b_prefix, b.ptr, b_prefix_len);
  return UINT32_C(0);
}

static uint32_t rt_bytes_cmp_range(
    ctx_t* ctx,
    bytes_t a,
    uint32_t a_off,
    uint32_t a_len,
    bytes_t b,
    uint32_t b_off,
    uint32_t b_len
) {
  if (a_off > a.len || a.len - a_off < a_len) rt_trap("bytes.cmp_range oob");
  if (b_off > b.len || b.len - b_off < b_len) rt_trap("bytes.cmp_range oob");

#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, a)) return UINT32_C(0);
  if (!rt_dbg_bytes_check(ctx, b)) return UINT32_C(0);
#else
  (void)ctx;
#endif

  uint32_t m = (a_len < b_len) ? a_len : b_len;
  if (m) {
    int cmp = memcmp(a.ptr + a_off, b.ptr + b_off, m);
    if (cmp < 0) return UINT32_MAX;
    if (cmp > 0) return UINT32_C(1);
  }
  if (a_len < b_len) return UINT32_MAX;
  if (a_len > b_len) return UINT32_C(1);
  return UINT32_C(0);
}

static bytes_view_t rt_bytes_view(ctx_t* ctx, bytes_t b) {
  bytes_view_t out;
  out.len = b.len;
#ifdef X07_DEBUG_BORROW
  if (b.len == 0) return rt_view_empty(ctx);
  uint32_t off = 0;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, &off);
  out.ptr = b.ptr;
  out.aid = aid;
  out.off_bytes = off;
  out.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#else
  out.ptr = (b.len == 0) ? ctx->heap.mem : b.ptr;
#endif
  return out;
}

static bytes_view_t rt_bytes_subview(ctx_t* ctx, bytes_t b, uint32_t start, uint32_t len) {
  if (start > b.len) rt_trap("bytes.subview oob");
  if (len > b.len - start) rt_trap("bytes.subview oob");
  bytes_view_t out;
  out.len = len;
#ifdef X07_DEBUG_BORROW
  if (len == 0) {
    return rt_view_empty(ctx);
  }
  uint32_t base_off = 0;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, &base_off);
  uint32_t off = base_off + start;
  out.ptr = b.ptr + start;
  out.aid = aid;
  out.off_bytes = off;
  out.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#else
  out.ptr = (len == 0) ? ctx->heap.mem : (b.ptr + start);
#endif
  return out;
}

static uint32_t rt_view_get_u8(ctx_t* ctx, bytes_view_t v, uint32_t idx) {
  if (idx >= v.len) rt_trap("view.get_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, v.bid, v.off_bytes + idx, 1)) return 0;
#else
  (void)ctx;
#endif
  return (uint32_t)v.ptr[idx];
}

static bytes_view_t rt_view_slice(ctx_t* ctx, bytes_view_t v, uint32_t start, uint32_t len) {
  if (start > v.len) rt_trap("view.slice oob");
  if (len > v.len - start) rt_trap("view.slice oob");
  bytes_view_t out;
  out.ptr = v.ptr + start;
  out.len = len;
#ifdef X07_DEBUG_BORROW
  out.aid = v.aid;
  out.bid = v.bid;
  out.off_bytes = v.off_bytes + start;
#else
  (void)ctx;
#endif
  return out;
}

static uint32_t rt_view_eq(ctx_t* ctx, bytes_view_t a, bytes_view_t b) {
  uint32_t a_prefix_len = 0;
  uint32_t b_prefix_len = 0;
  ctx->last_bytes_eq_valid = 0;
  if (a.len != b.len) goto mismatch;
  if (a.len == 0) return UINT32_C(1);
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, a.bid, a.off_bytes, a.len)) return UINT32_C(0);
  if (!rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b.len)) return UINT32_C(0);
#endif
  if (memcmp(a.ptr, b.ptr, a.len) == 0) return UINT32_C(1);

mismatch:
  a_prefix_len =
      (a.len < X07_ASSERT_BYTES_EQ_PREFIX_MAX) ? a.len : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
  b_prefix_len =
      (b.len < X07_ASSERT_BYTES_EQ_PREFIX_MAX) ? b.len : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
#ifdef X07_DEBUG_BORROW
  if (a_prefix_len && !rt_dbg_borrow_check(ctx, a.bid, a.off_bytes, a_prefix_len)) return UINT32_C(0);
  if (b_prefix_len && !rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b_prefix_len)) return UINT32_C(0);
#endif
  ctx->last_bytes_eq_valid = 1;
  ctx->last_bytes_eq_a_len = a.len;
  ctx->last_bytes_eq_b_len = b.len;
  if (a_prefix_len) memcpy(ctx->last_bytes_eq_a_prefix, a.ptr, a_prefix_len);
  if (b_prefix_len) memcpy(ctx->last_bytes_eq_b_prefix, b.ptr, b_prefix_len);
  return UINT32_C(0);
}

static uint32_t rt_view_cmp_range(
    ctx_t* ctx,
    bytes_view_t a,
    uint32_t a_off,
    uint32_t a_len,
    bytes_view_t b,
    uint32_t b_off,
    uint32_t b_len
) {
  if (a_off > a.len || a.len - a_off < a_len) rt_trap("view.cmp_range oob");
  if (b_off > b.len || b.len - b_off < b_len) rt_trap("view.cmp_range oob");

#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, a.bid, a.off_bytes + a_off, a_len)) return UINT32_C(0);
  if (!rt_dbg_borrow_check(ctx, b.bid, b.off_bytes + b_off, b_len)) return UINT32_C(0);
#else
  (void)ctx;
#endif

  uint32_t m = (a_len < b_len) ? a_len : b_len;
  if (m) {
    int cmp = memcmp(a.ptr + a_off, b.ptr + b_off, m);
    if (cmp < 0) return UINT32_MAX;
    if (cmp > 0) return UINT32_C(1);
  }
  if (a_len < b_len) return UINT32_MAX;
  if (a_len > b_len) return UINT32_C(1);
  return UINT32_C(0);
}

// Phase G2: deterministic single-thread cooperative scheduler + channels.

#define RT_WAIT_NONE UINT32_C(0)
#define RT_WAIT_JOIN UINT32_C(1)
#define RT_WAIT_SLEEP UINT32_C(2)
#define RT_WAIT_CHAN_SEND UINT32_C(3)
#define RT_WAIT_CHAN_RECV UINT32_C(4)
#define RT_WAIT_OS_PROC_JOIN UINT32_C(5)
#define RT_WAIT_OS_PROC_EXIT UINT32_C(6)

#define RT_TRACE_SWITCH UINT64_C(1)
#define RT_TRACE_BLOCK UINT64_C(2)
#define RT_TRACE_WAKE UINT64_C(3)
#define RT_TRACE_COMPLETE UINT64_C(4)

struct rt_task_s {
  uint32_t alive;
  uint32_t done;
  uint32_t canceled;

  uint32_t in_ready;
  uint32_t ready_next;

  uint32_t wait_kind;
  uint32_t wait_id;
  uint32_t wait_next;

  uint32_t join_wait_head;
  uint32_t join_wait_tail;

  uint32_t (*poll)(ctx_t* ctx, void* fut, rt_task_out_t* out);
  void (*drop)(ctx_t* ctx, void* fut);
  void* fut;
  rt_task_out_t out;
  uint32_t out_taken;
};

struct rt_timer_ev_s {
  uint64_t wake_time;
  uint64_t seq;
  uint32_t task_id;
};

struct rt_chan_bytes_s {
  uint32_t alive;
  uint32_t closed;
  uint32_t cap;

  bytes_t* buf;
  uint32_t head;
  uint32_t tail;
  uint32_t len;

  uint32_t send_wait_head;
  uint32_t send_wait_tail;
  uint32_t recv_wait_head;
  uint32_t recv_wait_tail;
};

struct rt_select_evt_s {
  uint32_t alive;
  uint32_t taken;

  uint64_t scope_key;

  uint32_t tag;
  uint32_t case_index;
  uint32_t src_id;

  bytes_t payload;
};

// Phase G2: deterministic streaming I/O reader handles + BufRead-like buffering.

#define RT_IO_READER_KIND_FILE UINT32_C(1)
#define RT_IO_READER_KIND_BYTES UINT32_C(2)

struct rt_io_reader_s {
  uint32_t alive;
  uint32_t kind;
  uint32_t eof;
  uint32_t pending_ticks;

#if X07_ENABLE_STREAMING_FILE_IO
  FILE* f;
#endif

  bytes_t bytes;
  uint32_t pos;
};

struct rt_bufread_s {
  uint32_t alive;
  iface_t reader;
  uint32_t eof;
  uint32_t direct_bytes;

  bytes_t buf;
  uint32_t start;
  uint32_t end;
};

static void rt_sched_trace_init(ctx_t* ctx) {
  if (ctx->sched_stats.sched_trace_hash == 0) {
    ctx->sched_stats.sched_trace_hash = UINT64_C(1469598103934665603);
  }
}

static void rt_sched_trace_u64(ctx_t* ctx, uint64_t x) {
  rt_sched_trace_init(ctx);
  ctx->sched_stats.sched_trace_hash ^= x;
  ctx->sched_stats.sched_trace_hash *= UINT64_C(1099511628211);
}

static void rt_sched_trace_event(ctx_t* ctx, uint64_t tag, uint64_t a, uint64_t b) {
  rt_sched_trace_u64(ctx, tag);
  rt_sched_trace_u64(ctx, a);
  rt_sched_trace_u64(ctx, b);
}

static rt_task_t* rt_task_ptr(ctx_t* ctx, uint32_t task_id) {
  if (task_id == 0 || task_id > ctx->sched_tasks_len) rt_trap("task invalid handle");
  rt_task_t* t = &ctx->sched_tasks[task_id - 1];
  if (!t->alive) rt_trap("task invalid handle");
  return t;
}

static rt_chan_bytes_t* rt_chan_bytes_ptr(ctx_t* ctx, uint32_t chan_id) {
  if (chan_id == 0 || chan_id > ctx->sched_chans_len) rt_trap("chan invalid handle");
  rt_chan_bytes_t* c = &ctx->sched_chans[chan_id - 1];
  if (!c->alive) rt_trap("chan invalid handle");
  return c;
}

static rt_select_evt_t* rt_select_evt_ptr(ctx_t* ctx, uint32_t evt_id) {
  if (evt_id == 0 || evt_id > ctx->sched_select_evts_len) rt_trap("X07T_SELECT_EVT_INVALID");
  rt_select_evt_t* e = &ctx->sched_select_evts[evt_id - 1];
  if (!e->alive) rt_trap("X07T_SELECT_EVT_INVALID");
  return e;
}

static rt_io_reader_t* rt_io_reader_ptr(ctx_t* ctx, uint32_t reader_id) {
  if (reader_id == 0 || reader_id > ctx->io_readers_len) rt_trap("io.reader invalid handle");
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  if (!r->alive) rt_trap("io.reader invalid handle");
  return r;
}

static rt_bufread_t* rt_bufread_ptr(ctx_t* ctx, uint32_t br_id) {
  if (br_id == 0 || br_id > ctx->bufreads_len) rt_trap("bufread invalid handle");
  rt_bufread_t* br = &ctx->bufreads[br_id - 1];
  if (!br->alive) rt_trap("bufread invalid handle");
  return br;
}

static void rt_sched_tasks_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_tasks_cap) return;
  rt_task_t* old_items = ctx->sched_tasks;
  uint32_t old_cap = ctx->sched_tasks_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_task_t);
  uint32_t new_cap = ctx->sched_tasks_cap ? ctx->sched_tasks_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_task_t* items = (rt_task_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_task_t),
    (uint32_t)_Alignof(rt_task_t)
  );
  if (old_items && ctx->sched_tasks_len) {
    uint32_t bytes = ctx->sched_tasks_len * (uint32_t)sizeof(rt_task_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_task_t));
  }
  ctx->sched_tasks = items;
  ctx->sched_tasks_cap = new_cap;
}

static void rt_sched_chans_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_chans_cap) return;
  rt_chan_bytes_t* old_items = ctx->sched_chans;
  uint32_t old_cap = ctx->sched_chans_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_chan_bytes_t);
  uint32_t new_cap = ctx->sched_chans_cap ? ctx->sched_chans_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_chan_bytes_t* items = (rt_chan_bytes_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_chan_bytes_t),
    (uint32_t)_Alignof(rt_chan_bytes_t)
  );
  if (old_items && ctx->sched_chans_len) {
    uint32_t bytes = ctx->sched_chans_len * (uint32_t)sizeof(rt_chan_bytes_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_chan_bytes_t));
  }
  ctx->sched_chans = items;
  ctx->sched_chans_cap = new_cap;
}

static void rt_sched_select_evts_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_select_evts_cap) return;
  rt_select_evt_t* old_items = ctx->sched_select_evts;
  uint32_t old_cap = ctx->sched_select_evts_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_select_evt_t);
  uint32_t new_cap = ctx->sched_select_evts_cap ? ctx->sched_select_evts_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_select_evt_t* items = (rt_select_evt_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_select_evt_t),
    (uint32_t)_Alignof(rt_select_evt_t)
  );
  if (old_items && ctx->sched_select_evts_len) {
    uint32_t bytes = ctx->sched_select_evts_len * (uint32_t)sizeof(rt_select_evt_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_select_evt_t));
  }
  ctx->sched_select_evts = items;
  ctx->sched_select_evts_cap = new_cap;
}

static void rt_sched_timers_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_timers_cap) return;
  rt_timer_ev_t* old_items = ctx->sched_timers;
  uint32_t old_cap = ctx->sched_timers_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_timer_ev_t);
  uint32_t new_cap = ctx->sched_timers_cap ? ctx->sched_timers_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_timer_ev_t* items = (rt_timer_ev_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_timer_ev_t),
    (uint32_t)_Alignof(rt_timer_ev_t)
  );
  if (old_items && ctx->sched_timers_len) {
    uint32_t bytes = ctx->sched_timers_len * (uint32_t)sizeof(rt_timer_ev_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_timer_ev_t));
  }
  ctx->sched_timers = items;
  ctx->sched_timers_cap = new_cap;
}

static void rt_io_readers_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->io_readers_cap) return;
  rt_io_reader_t* old_items = ctx->io_readers;
  uint32_t old_cap = ctx->io_readers_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_io_reader_t);
  uint32_t new_cap = ctx->io_readers_cap ? ctx->io_readers_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_io_reader_t* items = (rt_io_reader_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_io_reader_t),
    (uint32_t)_Alignof(rt_io_reader_t)
  );
  if (old_items && ctx->io_readers_len) {
    uint32_t bytes = ctx->io_readers_len * (uint32_t)sizeof(rt_io_reader_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_io_reader_t));
  }
  ctx->io_readers = items;
  ctx->io_readers_cap = new_cap;
}

static void rt_bufreads_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->bufreads_cap) return;
  rt_bufread_t* old_items = ctx->bufreads;
  uint32_t old_cap = ctx->bufreads_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_bufread_t);
  uint32_t new_cap = ctx->bufreads_cap ? ctx->bufreads_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_bufread_t* items = (rt_bufread_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_bufread_t),
    (uint32_t)_Alignof(rt_bufread_t)
  );
  if (old_items && ctx->bufreads_len) {
    uint32_t bytes = ctx->bufreads_len * (uint32_t)sizeof(rt_bufread_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_bufread_t));
  }
  ctx->bufreads = items;
  ctx->bufreads_cap = new_cap;
}

static void rt_ready_push(ctx_t* ctx, uint32_t task_id) {
  if (task_id == 0) return;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return;
  if (t->in_ready) return;
  t->in_ready = 1;
  t->ready_next = 0;
  if (ctx->sched_ready_tail == 0) {
    ctx->sched_ready_head = task_id;
    ctx->sched_ready_tail = task_id;
    return;
  }
  rt_task_t* tail = rt_task_ptr(ctx, ctx->sched_ready_tail);
  tail->ready_next = task_id;
  ctx->sched_ready_tail = task_id;
}

static uint32_t rt_ready_pop(ctx_t* ctx) {
  for (;;) {
    uint32_t task_id = ctx->sched_ready_head;
    if (task_id == 0) return 0;
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    ctx->sched_ready_head = t->ready_next;
    if (ctx->sched_ready_head == 0) ctx->sched_ready_tail = 0;
    t->ready_next = 0;
    t->in_ready = 0;
    if (t->done) continue;
    return task_id;
  }
}

static void rt_wait_list_push(ctx_t* ctx, uint32_t* head, uint32_t* tail, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  t->wait_next = 0;
  if (*tail == 0) {
    *head = task_id;
    *tail = task_id;
    return;
  }
  rt_task_t* last = rt_task_ptr(ctx, *tail);
  last->wait_next = task_id;
  *tail = task_id;
}

static uint32_t rt_wait_list_pop(ctx_t* ctx, uint32_t* head, uint32_t* tail) {
  for (;;) {
    uint32_t task_id = *head;
    if (task_id == 0) return 0;
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    *head = t->wait_next;
    if (*head == 0) *tail = 0;
    t->wait_next = 0;
    if (t->done) continue;
    return task_id;
  }
}

static void rt_sched_wake(ctx_t* ctx, uint32_t task_id, uint32_t reason_kind, uint32_t reason_id) {
  if (task_id == 0) return;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return;
  t->wait_kind = RT_WAIT_NONE;
  t->wait_id = 0;
  rt_ready_push(ctx, task_id);
  ctx->sched_stats.wake_events += 1;
  rt_sched_trace_event(
    ctx,
    RT_TRACE_WAKE,
    (uint64_t)task_id,
    ((uint64_t)reason_kind << 32) | (uint64_t)reason_id
  );
}

static uint32_t rt_timer_less(rt_timer_ev_t a, rt_timer_ev_t b) {
  if (a.wake_time < b.wake_time) return 1;
  if (a.wake_time > b.wake_time) return 0;
  return (a.seq < b.seq) ? 1 : 0;
}

static void rt_timer_push(ctx_t* ctx, uint64_t wake_time, uint32_t task_id) {
  rt_sched_timers_ensure_cap(ctx, ctx->sched_timers_len + 1);
  uint32_t i = ctx->sched_timers_len++;
  rt_timer_ev_t ev;
  ev.wake_time = wake_time;
  ev.seq = ctx->sched_seq++;
  ev.task_id = task_id;

  while (i > 0) {
    uint32_t p = (i - 1) / 2;
    rt_timer_ev_t parent = ctx->sched_timers[p];
    if (!rt_timer_less(ev, parent)) break;
    ctx->sched_timers[i] = parent;
    i = p;
  }
  ctx->sched_timers[i] = ev;
}

static uint32_t rt_timer_pop(ctx_t* ctx, rt_timer_ev_t* out) {
  if (ctx->sched_timers_len == 0) return 0;
  if (out) *out = ctx->sched_timers[0];
  ctx->sched_timers_len -= 1;
  if (ctx->sched_timers_len == 0) return 1;

  rt_timer_ev_t ev = ctx->sched_timers[ctx->sched_timers_len];
  uint32_t i = 0;
  for (;;) {
    uint32_t l = i * 2 + 1;
    uint32_t r = l + 1;
    if (l >= ctx->sched_timers_len) break;
    uint32_t m = l;
    if (r < ctx->sched_timers_len && rt_timer_less(ctx->sched_timers[r], ctx->sched_timers[l])) {
      m = r;
    }
    if (!rt_timer_less(ctx->sched_timers[m], ev)) break;
    ctx->sched_timers[i] = ctx->sched_timers[m];
    i = m;
  }
  ctx->sched_timers[i] = ev;
  return 1;
}

static uint64_t rt_timer_peek_wake(ctx_t* ctx) {
  if (ctx->sched_timers_len == 0) return UINT64_MAX;
  return ctx->sched_timers[0].wake_time;
}

static uint32_t rt_sched_step(ctx_t* ctx) {
  uint32_t task_id = rt_ready_pop(ctx);
  if (task_id != 0) {
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    if (t->done) return UINT32_C(1);

    ctx->sched_stats.ctx_switches += 1;
    rt_sched_trace_event(ctx, RT_TRACE_SWITCH, (uint64_t)task_id, ctx->sched_now_ticks);

    uint32_t prev = ctx->sched_current_task;
    ctx->sched_current_task = task_id;

    rt_task_out_t out = rt_task_out_empty(ctx);
    uint32_t done = t->poll(ctx, t->fut, &out);

    ctx->sched_current_task = prev;
    t = rt_task_ptr(ctx, task_id);

    if (done) {
      t->done = 1;
      t->out = out;
      t->out_taken = 0;
      if (t->drop && t->fut) {
        t->drop(ctx, t->fut);
      }
      t->drop = NULL;
      t->fut = NULL;
      rt_sched_trace_event(ctx, RT_TRACE_COMPLETE, (uint64_t)task_id, ctx->sched_now_ticks);

      uint32_t w = t->join_wait_head;
      uint32_t wt = t->join_wait_tail;
      (void)wt;
      t->join_wait_head = 0;
      t->join_wait_tail = 0;
      while (w != 0) {
        rt_task_t* waiter = rt_task_ptr(ctx, w);
        uint32_t next = waiter->wait_next;
        waiter->wait_next = 0;
        rt_sched_wake(ctx, w, RT_WAIT_JOIN, task_id);
        w = next;
      }
      return UINT32_C(1);
    }

    if (!t->in_ready && t->wait_kind == RT_WAIT_NONE) {
      const char* dbg = getenv("X07_DEBUG_SCHED");
      if (dbg && dbg[0] == '1' && dbg[1] == '\0') {
        char msg[192];
        uint32_t st = 0;
        if (t->fut) memcpy(&st, t->fut, sizeof(uint32_t));
        (void)snprintf(
          msg,
          sizeof(msg),
          "task pending without block (task_id=%u poll=%p state=%u)",
          (unsigned)task_id,
          (void*)t->poll,
          (unsigned)st
        );
        rt_trap(msg);
      }
      char msg[96];
      (void)snprintf(
        msg,
        sizeof(msg),
        "task pending without block (task_id=%u)",
        (unsigned)task_id
      );
      rt_trap(msg);
    }
    (void)rt_os_process_poll_all(ctx, 0);
    return UINT32_C(1);
  }

  rt_timer_ev_t ev;
  while (rt_timer_pop(ctx, &ev)) {
    if (ev.task_id == 0) continue;
    rt_task_t* t = rt_task_ptr(ctx, ev.task_id);
    if (t->done) continue;
    if (ev.wake_time > ctx->sched_now_ticks) ctx->sched_now_ticks = ev.wake_time;
    ctx->sched_stats.virtual_time_end = ctx->sched_now_ticks;
    rt_sched_wake(ctx, ev.task_id, RT_WAIT_SLEEP, 0);
    return UINT32_C(1);
  }

  if (rt_os_process_poll_all(ctx, 50)) return UINT32_C(1);
  return UINT32_C(0);
}

static __attribute__((noreturn)) void rt_sched_deadlock(void) {
  rt_trap("scheduler deadlock");
}

static uint32_t rt_task_create(
    ctx_t* ctx,
    uint32_t (*poll)(ctx_t* ctx, void* fut, rt_task_out_t* out),
    void (*drop)(ctx_t* ctx, void* fut),
    void* fut
) {
  rt_sched_tasks_ensure_cap(ctx, ctx->sched_tasks_len + 1);
  uint32_t task_id = ctx->sched_tasks_len + 1;
  rt_task_t* t = &ctx->sched_tasks[task_id - 1];
  memset(t, 0, sizeof(*t));
  t->alive = 1;
  t->poll = poll;
  t->drop = drop;
  t->fut = fut;
  t->out = rt_task_out_empty(ctx);
  t->out_taken = 0;
  ctx->sched_tasks_len += 1;

  ctx->sched_stats.tasks_spawned += 1;
  rt_ready_push(ctx, task_id);
  return task_id;
}

static uint32_t rt_task_spawn(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.spawn_calls += 1;
  (void)rt_task_ptr(ctx, task_id);
  return task_id;
}

static uint32_t rt_task_cancel(ctx_t* ctx, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return UINT32_C(0);
  t->canceled = 1;
  t->done = 1;
  if (t->drop && t->fut) {
    t->drop(ctx, t->fut);
  }
  t->drop = NULL;
  t->fut = NULL;
  rt_task_out_drop(ctx, &t->out);
  t->out = rt_task_out_empty(ctx);
  t->out_taken = 0;
  rt_sched_trace_event(ctx, RT_TRACE_COMPLETE, (uint64_t)task_id, ctx->sched_now_ticks);

  uint32_t w = t->join_wait_head;
  t->join_wait_head = 0;
  t->join_wait_tail = 0;
  while (w != 0) {
    rt_task_t* waiter = rt_task_ptr(ctx, w);
    uint32_t next = waiter->wait_next;
    waiter->wait_next = 0;
    rt_sched_wake(ctx, w, RT_WAIT_JOIN, task_id);
    w = next;
  }
  return UINT32_C(1);
}

static uint32_t rt_task_join_bytes_poll(ctx_t* ctx, uint32_t task_id, bytes_t* out) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) {
    if (t->out_taken) rt_trap("join already taken");
    t->out_taken = 1;
    if (t->canceled) {
      if (out) *out = rt_bytes_empty(ctx);
      t->out = rt_task_out_empty(ctx);
      return UINT32_C(1);
    }
    if (t->out.kind != RT_TASK_OUT_KIND_BYTES) rt_trap("task.join.bytes kind mismatch");
    if (out) {
      *out = t->out.payload.bytes;
    } else {
      rt_bytes_drop(ctx, &t->out.payload.bytes);
    }
    t->out = rt_task_out_empty(ctx);
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("join.poll from main");
  if (cur == task_id) rt_trap("join self");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_JOIN && me->wait_id == task_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("join while already waiting");

  me->wait_kind = RT_WAIT_JOIN;
  me->wait_id = task_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_JOIN << 32) | task_id);
  rt_wait_list_push(ctx, &t->join_wait_head, &t->join_wait_tail, cur);
  return UINT32_C(0);
}

static bytes_t rt_task_join_bytes_block(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  while (!t->done) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    t = rt_task_ptr(ctx, task_id);
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) return rt_bytes_empty(ctx);
  if (t->out.kind != RT_TASK_OUT_KIND_BYTES) rt_trap("task.join.bytes kind mismatch");
  bytes_t out_b = t->out.payload.bytes;
  t->out = rt_task_out_empty(ctx);
  return out_b;
}

static uint32_t rt_task_is_finished(ctx_t* ctx, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  return t->done ? UINT32_C(1) : UINT32_C(0);
}

static result_bytes_t rt_task_try_join_bytes(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (!t->done) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(1) };
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  }
  if (t->out.kind != RT_TASK_OUT_KIND_BYTES) rt_trap("task.try_join.bytes kind mismatch");
  bytes_t out_b = t->out.payload.bytes;
  t->out = rt_task_out_empty(ctx);
  return (result_bytes_t){ .tag = UINT32_C(1), .payload.ok = out_b };
}

static uint32_t rt_task_join_result_bytes_poll(ctx_t* ctx, uint32_t task_id, result_bytes_t* out) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) {
    if (t->out_taken) rt_trap("join already taken");
    t->out_taken = 1;
    if (t->canceled) {
      if (out) *out = (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
      t->out = rt_task_out_empty(ctx);
      return UINT32_C(1);
    }
    if (t->out.kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("task.join.result_bytes kind mismatch");
    if (out) {
      *out = t->out.payload.result_bytes;
    } else {
      if (t->out.payload.result_bytes.tag) {
        rt_bytes_drop(ctx, &t->out.payload.result_bytes.payload.ok);
      }
    }
    t->out = rt_task_out_empty(ctx);
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("join.poll from main");
  if (cur == task_id) rt_trap("join self");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_JOIN && me->wait_id == task_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("join while already waiting");

  me->wait_kind = RT_WAIT_JOIN;
  me->wait_id = task_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_JOIN << 32) | task_id);
  rt_wait_list_push(ctx, &t->join_wait_head, &t->join_wait_tail, cur);
  return UINT32_C(0);
}

static result_bytes_t rt_task_join_result_bytes_block(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  while (!t->done) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    t = rt_task_ptr(ctx, task_id);
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  if (t->out.kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("task.join.result_bytes kind mismatch");
  result_bytes_t out_rb = t->out.payload.result_bytes;
  t->out = rt_task_out_empty(ctx);
  return out_rb;
}

static result_result_bytes_t rt_task_try_join_result_bytes(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (!t->done) {
    return (result_result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(1) };
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) {
    return (result_result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  }
  if (t->out.kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("task.try_join.result_bytes kind mismatch");
  result_bytes_t out_rb = t->out.payload.result_bytes;
  t->out = rt_task_out_empty(ctx);
  return (result_result_bytes_t){ .tag = UINT32_C(1), .payload.ok = out_rb };
}

static void rt_task_yield(ctx_t* ctx) {
  ctx->sched_stats.yield_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("yield from main");
  rt_task_t* me = rt_task_ptr(ctx, cur);
  me->wait_kind = RT_WAIT_NONE;
  me->wait_id = 0;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_NONE << 32) | RT_WAIT_NONE);
  rt_ready_push(ctx, cur);
}

static uint32_t rt_task_yield_block(ctx_t* ctx) {
  ctx->sched_stats.yield_calls += 1;
  (void)rt_sched_step(ctx);
  return UINT32_C(0);
}

static void rt_task_sleep(ctx_t* ctx, uint32_t ticks) {
  ctx->sched_stats.sleep_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("sleep from main");
  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (ticks == 0) {
    rt_ready_push(ctx, cur);
    return;
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("sleep while already waiting");
  me->wait_kind = RT_WAIT_SLEEP;
  me->wait_id = 0;
  ctx->sched_stats.blocked_waits += 1;
  uint64_t wake_time = ctx->sched_now_ticks + (uint64_t)ticks;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_SLEEP << 32) | (uint64_t)ticks);
  rt_timer_push(ctx, wake_time, cur);
}

static uint32_t rt_task_sleep_block(ctx_t* ctx, uint32_t ticks) {
  ctx->sched_stats.sleep_calls += 1;
  if (ticks == 0) return UINT32_C(0);
  uint64_t target = ctx->sched_now_ticks + (uint64_t)ticks;
  while (ctx->sched_now_ticks < target) {
    if (ctx->sched_ready_head != 0) {
      if (!rt_sched_step(ctx)) break;
      continue;
    }
    uint64_t next = rt_timer_peek_wake(ctx);
    if (next == UINT64_MAX || next > target) {
      ctx->sched_now_ticks = target;
      ctx->sched_stats.virtual_time_end = ctx->sched_now_ticks;
      break;
    }
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
  }
  return UINT32_C(0);
}

typedef struct {
  uint32_t task_id;
  uint32_t kind;
  uint32_t state;
  uint32_t gen;
} rt_scope_slot_t;

typedef struct {
  uint32_t max_children;
  uint64_t max_ticks;
  uint64_t max_blocked_waits;
  uint64_t max_join_polls;
  uint32_t max_slot_result_bytes;

  uint64_t key;

  uint8_t cancel_requested;

  uint64_t snap_ticks;
  uint64_t snap_blocked_waits;
  uint64_t snap_join_polls;
  uint64_t snap_tasks_spawned;

  uint32_t child_cap;
  uint32_t child_len;
  uint32_t* child_task_ids;
  uint32_t* child_task_kinds;

  uint32_t reg_cap;
  uint32_t reg_len;
  uint32_t* reg_task_ids;

  uint32_t slot_cap;
  uint32_t slot_len;
  rt_scope_slot_t* slots;

  uint32_t join_phase;
  uint32_t join_slot_i;
  uint32_t join_child_i;

  uint32_t select_rr_cursor;
} rt_scope_t;

typedef struct {
  uint32_t active;
  uint32_t mode;
  uint32_t violated;
  uint32_t err_code;
  const uint8_t* label_ptr;
  uint32_t label_len;

  uint8_t yielded;
  uint8_t fuel_clamped;
  uint16_t reserved16;

  uint64_t max_alloc_bytes;
  uint64_t max_alloc_calls;
  uint64_t max_realloc_calls;
  uint64_t max_memcpy_bytes;
  uint64_t max_sched_ticks;
  uint64_t max_fuel;

  uint64_t snap_alloc_bytes;
  uint64_t snap_alloc_calls;
  uint64_t snap_realloc_calls;
  uint64_t snap_memcpy_bytes;
  uint64_t snap_sched_ticks;

  uint64_t snap_fuel_saved;
  uint64_t snap_fuel_start;
} rt_budget_scope_t;

#define RT_BUDGET_MODE_TRAP UINT32_C(0)
#define RT_BUDGET_MODE_RESULT_ERR UINT32_C(1)
#define RT_BUDGET_MODE_STATS_ONLY UINT32_C(2)
#define RT_BUDGET_MODE_YIELD UINT32_C(3)

#define RT_ERR_BUDGET_ALLOC_BYTES UINT32_C(0x80000001)
#define RT_ERR_BUDGET_ALLOC_CALLS UINT32_C(0x80000002)
#define RT_ERR_BUDGET_REALLOC_CALLS UINT32_C(0x80000003)
#define RT_ERR_BUDGET_MEMCPY_BYTES UINT32_C(0x80000004)
#define RT_ERR_BUDGET_SCHED_TICKS UINT32_C(0x80000005)

static void rt_budget_scope_drop(ctx_t* ctx, rt_budget_scope_t* s) {
  if (!s->active) return;
  if (s->fuel_clamped) {
    uint64_t consumed = s->snap_fuel_start - ctx->fuel;
    ctx->fuel = s->snap_fuel_saved - consumed;
    if (ctx->budget_fuel_depth == 0) rt_trap("budget fuel depth underflow");
    ctx->budget_fuel_depth -= 1;
  }
  s->active = UINT32_C(0);
}

static void rt_budget_scope_dispose_on_drop(ctx_t* ctx, rt_budget_scope_t* s) {
  rt_budget_scope_drop(ctx, s);
}

static void rt_budget_scope_init(
  ctx_t* ctx,
  rt_budget_scope_t* s,
  uint32_t mode,
  const uint8_t* label_ptr,
  uint32_t label_len,
  uint64_t max_alloc_bytes,
  uint64_t max_alloc_calls,
  uint64_t max_realloc_calls,
  uint64_t max_memcpy_bytes,
  uint64_t max_sched_ticks,
  uint64_t max_fuel
) {
  memset(s, 0, sizeof(*s));
  s->active = UINT32_C(1);
  s->mode = mode;
  s->label_ptr = label_ptr;
  s->label_len = label_len;
  s->max_alloc_bytes = max_alloc_bytes;
  s->max_alloc_calls = max_alloc_calls;
  s->max_realloc_calls = max_realloc_calls;
  s->max_memcpy_bytes = max_memcpy_bytes;
  s->max_sched_ticks = max_sched_ticks;
  s->max_fuel = max_fuel;

  s->snap_alloc_bytes = ctx->mem_stats.bytes_alloc_total;
  s->snap_alloc_calls = ctx->mem_stats.alloc_calls;
  s->snap_realloc_calls = ctx->mem_stats.realloc_calls;
  s->snap_memcpy_bytes = ctx->mem_stats.memcpy_bytes;
  s->snap_sched_ticks = ctx->sched_now_ticks;

  s->snap_fuel_saved = ctx->fuel;
  s->snap_fuel_start = ctx->fuel;
  if (max_fuel != 0 && ctx->fuel > max_fuel) {
    s->snap_fuel_start = max_fuel;
    ctx->fuel = max_fuel;
    s->fuel_clamped = 1;
    ctx->budget_fuel_depth += 1;
  }
}

static void rt_budget_scope_check_exit(ctx_t* ctx, rt_budget_scope_t* s) {
  if (!s->active) return;
  s->violated = UINT32_C(0);
  s->err_code = UINT32_C(0);

  uint64_t alloc_bytes = ctx->mem_stats.bytes_alloc_total - s->snap_alloc_bytes;
  if (s->max_alloc_bytes != 0 && alloc_bytes > s->max_alloc_bytes) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_ALLOC_BYTES;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_ALLOC_BYTES");
  }

  uint64_t alloc_calls = ctx->mem_stats.alloc_calls - s->snap_alloc_calls;
  if (s->max_alloc_calls != 0 && alloc_calls > s->max_alloc_calls) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_ALLOC_CALLS;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_ALLOC_CALLS");
  }

  uint64_t realloc_calls = ctx->mem_stats.realloc_calls - s->snap_realloc_calls;
  if (s->max_realloc_calls != 0 && realloc_calls > s->max_realloc_calls) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_REALLOC_CALLS;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_REALLOC_CALLS");
  }

  uint64_t memcpy_bytes = ctx->mem_stats.memcpy_bytes - s->snap_memcpy_bytes;
  if (s->max_memcpy_bytes != 0 && memcpy_bytes > s->max_memcpy_bytes) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_MEMCPY_BYTES;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_MEMCPY_BYTES");
  }

  uint64_t sched_ticks = ctx->sched_now_ticks - s->snap_sched_ticks;
  if (s->max_sched_ticks != 0 && sched_ticks > s->max_sched_ticks) {
    if (s->mode == RT_BUDGET_MODE_STATS_ONLY || s->mode == RT_BUDGET_MODE_YIELD) return;
    if (s->mode == RT_BUDGET_MODE_RESULT_ERR) {
      s->violated = UINT32_C(1);
      s->err_code = RT_ERR_BUDGET_SCHED_TICKS;
      return;
    }
    rt_trap("X07T_BUDGET_EXCEEDED_SCHED_TICKS");
  }
}

static uint32_t rt_budget_scope_exit_poll(ctx_t* ctx, rt_budget_scope_t* s) {
  if (!s->active) return UINT32_C(1);

  if (s->mode == RT_BUDGET_MODE_YIELD && !s->yielded && s->max_sched_ticks != 0) {
    uint64_t ticks = ctx->sched_now_ticks - s->snap_sched_ticks;
    if (ticks > s->max_sched_ticks) {
      s->yielded = 1;
      rt_task_yield(ctx);
      s->snap_sched_ticks = ctx->sched_now_ticks;
      return UINT32_C(0);
    }
  }

  rt_budget_scope_check_exit(ctx, s);
  rt_budget_scope_drop(ctx, s);
  return UINT32_C(1);
}

static void rt_budget_scope_exit_block(ctx_t* ctx, rt_budget_scope_t* s) {
  if (!s->active) return;

  if (s->mode == RT_BUDGET_MODE_YIELD && !s->yielded && s->max_sched_ticks != 0) {
    uint64_t ticks = ctx->sched_now_ticks - s->snap_sched_ticks;
    if (ticks > s->max_sched_ticks) {
      s->yielded = 1;
      (void)rt_task_yield_block(ctx);
      s->snap_sched_ticks = ctx->sched_now_ticks;
    }
  }

  rt_budget_scope_check_exit(ctx, s);
  rt_budget_scope_drop(ctx, s);
}

#define RT_SCOPE_SLOT_EMPTY UINT32_C(0)
#define RT_SCOPE_SLOT_PENDING UINT32_C(1)
#define RT_SCOPE_SLOT_TAKEN UINT32_C(2)
#define RT_SCOPE_SLOT_CONSUMED UINT32_C(3)

static void rt_scope_init(
  ctx_t* ctx,
  rt_scope_t* s,
  uint32_t max_children,
  uint64_t max_ticks,
  uint64_t max_blocked_waits,
  uint64_t max_join_polls,
  uint32_t max_slot_result_bytes
) {
  memset(s, 0, sizeof(*s));
  s->max_children = max_children;
  s->max_ticks = max_ticks;
  s->max_blocked_waits = max_blocked_waits;
  s->max_join_polls = max_join_polls;
  s->max_slot_result_bytes = max_slot_result_bytes;
  s->key = (uint64_t)(uintptr_t)s;

  s->child_cap = max_children;
  s->reg_cap = max_children;
  s->slot_cap = max_children;
  s->slot_len = max_children;

  if (max_children != 0) {
    s->child_task_ids = (uint32_t*)rt_alloc(
      ctx,
      max_children * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
    s->child_task_kinds = (uint32_t*)rt_alloc(
      ctx,
      max_children * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
    s->reg_task_ids = (uint32_t*)rt_alloc(
      ctx,
      max_children * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
    s->slots = (rt_scope_slot_t*)rt_alloc(
      ctx,
      max_children * (uint32_t)sizeof(rt_scope_slot_t),
      (uint32_t)_Alignof(rt_scope_slot_t)
    );
    memset(s->slots, 0, max_children * (uint32_t)sizeof(rt_scope_slot_t));
  }

  s->snap_ticks = ctx->sched_now_ticks;
  s->snap_blocked_waits = ctx->sched_stats.blocked_waits;
  s->snap_join_polls = ctx->sched_stats.join_calls;
  s->snap_tasks_spawned = ctx->sched_stats.tasks_spawned;
}

static void rt_scope_drop(ctx_t* ctx, rt_scope_t* s) {
  if (s->key != 0 && ctx->sched_select_evts_len != 0) {
    for (uint32_t i = 0; i < ctx->sched_select_evts_len; i++) {
      rt_select_evt_t* e = &ctx->sched_select_evts[i];
      if (!e->alive) continue;
      if (e->scope_key != s->key) continue;
      rt_bytes_drop(ctx, &e->payload);
      e->payload = rt_bytes_empty(ctx);
      e->taken = 1;
      e->alive = 0;
    }
  }
  if (s->child_cap && s->child_task_ids) {
    rt_free(
      ctx,
      s->child_task_ids,
      s->child_cap * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
  }
  if (s->child_cap && s->child_task_kinds) {
    rt_free(
      ctx,
      s->child_task_kinds,
      s->child_cap * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
  }
  if (s->reg_cap && s->reg_task_ids) {
    rt_free(
      ctx,
      s->reg_task_ids,
      s->reg_cap * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
  }
  if (s->slot_cap && s->slots) {
    rt_free(
      ctx,
      s->slots,
      s->slot_cap * (uint32_t)sizeof(rt_scope_slot_t),
      (uint32_t)_Alignof(rt_scope_slot_t)
    );
  }
  memset(s, 0, sizeof(*s));
}

static void rt_scope_register_task(rt_scope_t* s, uint32_t task_id) {
  if (s->reg_len >= s->reg_cap) rt_trap("X07T_SCOPE_BUDGET_CHILDREN_EXCEEDED");
  s->reg_task_ids[s->reg_len] = task_id;
  s->reg_len += 1;
}

static void rt_scope_unregister_task(rt_scope_t* s, uint32_t task_id) {
  for (uint32_t i = 0; i < s->reg_len; i++) {
    if (s->reg_task_ids[i] != task_id) continue;
    for (uint32_t j = i + 1; j < s->reg_len; j++) {
      s->reg_task_ids[j - 1] = s->reg_task_ids[j];
    }
    s->reg_len -= 1;
    if (s->reg_len < s->reg_cap) s->reg_task_ids[s->reg_len] = 0;
    return;
  }
}

static uint32_t rt_scope_start_soon(ctx_t* ctx, rt_scope_t* s, uint32_t task_id, uint32_t kind) {
  (void)ctx;
  if (s->cancel_requested) rt_trap("X07T_SCOPE_START_AFTER_CANCEL");
  rt_scope_register_task(s, task_id);
  if (s->child_len >= s->child_cap) rt_trap("X07T_SCOPE_BUDGET_CHILDREN_EXCEEDED");
  s->child_task_ids[s->child_len] = task_id;
  s->child_task_kinds[s->child_len] = kind;
  s->child_len += 1;
  return UINT32_C(1);
}

static uint32_t rt_scope_async_let(ctx_t* ctx, rt_scope_t* s, uint32_t task_id, uint32_t kind) {
  (void)ctx;
  if (s->cancel_requested) rt_trap("X07T_SCOPE_START_AFTER_CANCEL");
  rt_scope_register_task(s, task_id);
  for (uint32_t i = 0; i < s->slot_len; i++) {
    rt_scope_slot_t* slot = &s->slots[i];
    if (slot->state != RT_SCOPE_SLOT_EMPTY && slot->state != RT_SCOPE_SLOT_CONSUMED) continue;
    slot->gen += 1;
    slot->task_id = task_id;
    slot->kind = kind;
    slot->state = RT_SCOPE_SLOT_PENDING;
    uint32_t handle = (slot->gen << 16) | i;
    return handle;
  }
  rt_trap("X07T_SCOPE_BUDGET_CHILDREN_EXCEEDED");
}

static uint32_t rt_scope_slot_is_finished(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state == RT_SCOPE_SLOT_PENDING || slot->state == RT_SCOPE_SLOT_TAKEN) {
    return rt_task_is_finished(ctx, slot->task_id);
  }
  if (slot->state == RT_SCOPE_SLOT_CONSUMED) return UINT32_C(1);
  rt_trap("X07T_SCOPE_SLOT_INVALID");
}

static uint32_t rt_scope_slot_task_for_await(
  rt_scope_t* s,
  uint32_t slot_id,
  uint32_t expected_kind
) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state == RT_SCOPE_SLOT_PENDING) {
    slot->state = RT_SCOPE_SLOT_TAKEN;
  } else if (slot->state != RT_SCOPE_SLOT_TAKEN) {
    rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  }
  if (slot->kind != expected_kind) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  if (slot->task_id == 0) rt_trap("X07T_SCOPE_SLOT_INVALID");
  return slot->task_id;
}

static void rt_scope_check_slot_result_size(rt_scope_t* s, bytes_t out) {
  if (s->max_slot_result_bytes != 0 && out.len > s->max_slot_result_bytes) {
    rt_trap("X07T_SCOPE_SLOT_RESULT_TOO_LARGE");
  }
}

static uint32_t rt_scope_await_slot_bytes_poll(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id, bytes_t* out) {
  uint32_t task_id = rt_scope_slot_task_for_await(s, slot_id, RT_TASK_OUT_KIND_BYTES);
  uint32_t done = rt_task_join_bytes_poll(ctx, task_id, out);
  if (!done) return UINT32_C(0);
  rt_scope_check_slot_result_size(s, *out);
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  slot->state = RT_SCOPE_SLOT_CONSUMED;
  slot->task_id = 0;
  rt_scope_unregister_task(s, task_id);
  return UINT32_C(1);
}

static bytes_t rt_scope_await_slot_bytes_block(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state != RT_SCOPE_SLOT_PENDING) rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  if (slot->kind != RT_TASK_OUT_KIND_BYTES) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  slot->state = RT_SCOPE_SLOT_TAKEN;
  uint32_t task_id = slot->task_id;
  bytes_t out = rt_task_join_bytes_block(ctx, task_id);
  rt_scope_check_slot_result_size(s, out);
  slot->state = RT_SCOPE_SLOT_CONSUMED;
  slot->task_id = 0;
  rt_scope_unregister_task(s, task_id);
  return out;
}

static uint32_t rt_scope_await_slot_result_bytes_poll(
  ctx_t* ctx,
  rt_scope_t* s,
  uint32_t slot_id,
  result_bytes_t* out
) {
  uint32_t task_id = rt_scope_slot_task_for_await(s, slot_id, RT_TASK_OUT_KIND_RESULT_BYTES);
  uint32_t done = rt_task_join_result_bytes_poll(ctx, task_id, out);
  if (!done) return UINT32_C(0);
  if (out->tag) rt_scope_check_slot_result_size(s, out->payload.ok);
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  slot->state = RT_SCOPE_SLOT_CONSUMED;
  slot->task_id = 0;
  rt_scope_unregister_task(s, task_id);
  return UINT32_C(1);
}

static result_bytes_t rt_scope_await_slot_result_bytes_block(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state != RT_SCOPE_SLOT_PENDING) rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  if (slot->kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  slot->state = RT_SCOPE_SLOT_TAKEN;
  uint32_t task_id = slot->task_id;
  result_bytes_t out = rt_task_join_result_bytes_block(ctx, task_id);
  if (out.tag) rt_scope_check_slot_result_size(s, out.payload.ok);
  slot->state = RT_SCOPE_SLOT_CONSUMED;
  slot->task_id = 0;
  rt_scope_unregister_task(s, task_id);
  return out;
}

static result_bytes_t rt_scope_try_await_slot_bytes(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state != RT_SCOPE_SLOT_PENDING) rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  if (slot->kind != RT_TASK_OUT_KIND_BYTES) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  uint32_t task_id = slot->task_id;
  result_bytes_t r = rt_task_try_join_bytes(ctx, task_id);
  if (r.tag) {
    rt_scope_check_slot_result_size(s, r.payload.ok);
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
    rt_scope_unregister_task(s, task_id);
    return r;
  }
  if (r.payload.err == UINT32_C(2)) {
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
    rt_scope_unregister_task(s, task_id);
  }
  return r;
}

static result_result_bytes_t rt_scope_try_await_slot_result_bytes(ctx_t* ctx, rt_scope_t* s, uint32_t slot_id) {
  uint32_t slot_idx = slot_id & UINT32_C(0xffff);
  uint32_t slot_gen = slot_id >> 16;
  if (slot_idx >= s->slot_len) rt_trap("X07T_SCOPE_SLOT_OOB");
  rt_scope_slot_t* slot = &s->slots[slot_idx];
  if (slot->gen != slot_gen) rt_trap("X07T_SCOPE_SLOT_INVALID");
  if (slot->state != RT_SCOPE_SLOT_PENDING) rt_trap("X07T_SCOPE_SLOT_ALREADY_CONSUMED");
  if (slot->kind != RT_TASK_OUT_KIND_RESULT_BYTES) rt_trap("X07T_SCOPE_SLOT_KIND_MISMATCH");
  uint32_t task_id = slot->task_id;
  result_result_bytes_t r = rt_task_try_join_result_bytes(ctx, task_id);
  if (r.tag) {
    if (r.payload.ok.tag) rt_scope_check_slot_result_size(s, r.payload.ok.payload.ok);
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
    rt_scope_unregister_task(s, task_id);
    return r;
  }
  if (r.payload.err == UINT32_C(2)) {
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
    rt_scope_unregister_task(s, task_id);
  }
  return r;
}

static uint32_t rt_scope_wait_all_count(rt_scope_t* s) {
  uint32_t slots_pending = 0;
  for (uint32_t i = 0; i < s->slot_len; i++) {
    if (s->slots[i].state == RT_SCOPE_SLOT_PENDING) slots_pending += 1;
  }
  return s->child_len + slots_pending;
}

static void rt_scope_reset_active(rt_scope_t* s) {
  s->child_len = 0;
  s->reg_len = 0;
  s->cancel_requested = 0;
}

static uint32_t rt_scope_cancel_all(ctx_t* ctx, rt_scope_t* s) {
  uint32_t canceled = 0;
  for (uint32_t i = s->reg_len; i > 0; i--) {
    canceled += rt_task_cancel(ctx, s->reg_task_ids[i - 1]);
  }
  s->cancel_requested = 1;
  return canceled;
}

static uint32_t rt_scope_join_drop_remaining_poll(ctx_t* ctx, rt_scope_t* s) {
  if (s->join_phase == 0) {
    s->join_phase = 1;
    s->join_slot_i = 0;
    s->join_child_i = 0;
  }

  if (s->join_phase == 1) {
    while (s->join_slot_i < s->slot_len) {
      uint32_t slot_id = s->join_slot_i;
      rt_scope_slot_t* slot = &s->slots[slot_id];
      if (slot->state != RT_SCOPE_SLOT_PENDING) {
        s->join_slot_i += 1;
        continue;
      }

      if (slot->kind == RT_TASK_OUT_KIND_BYTES) {
        bytes_t out = rt_bytes_empty(ctx);
        uint32_t task_id = slot->task_id;
        uint32_t done = rt_task_join_bytes_poll(ctx, task_id, &out);
        if (!done) return UINT32_C(0);
        rt_scope_check_slot_result_size(s, out);
        rt_bytes_drop(ctx, &out);
        rt_scope_unregister_task(s, task_id);
      } else if (slot->kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
        result_bytes_t out = (result_bytes_t){0};
        uint32_t task_id = slot->task_id;
        uint32_t done = rt_task_join_result_bytes_poll(ctx, task_id, &out);
        if (!done) return UINT32_C(0);
        if (out.tag) {
          rt_scope_check_slot_result_size(s, out.payload.ok);
          rt_bytes_drop(ctx, &out.payload.ok);
        }
        rt_scope_unregister_task(s, task_id);
      } else {
        rt_trap("scope slot kind invalid");
      }

      slot->state = RT_SCOPE_SLOT_CONSUMED;
      slot->task_id = 0;
      s->join_slot_i += 1;
    }
    s->join_phase = 2;
  }

  if (s->join_phase == 2) {
    while (s->join_child_i < s->child_len) {
      uint32_t task_id = s->child_task_ids[s->join_child_i];
      uint32_t kind = s->child_task_kinds[s->join_child_i];
      if (kind == RT_TASK_OUT_KIND_BYTES) {
        uint32_t done = rt_task_join_bytes_poll(ctx, task_id, NULL);
        if (!done) return UINT32_C(0);
      } else if (kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
        uint32_t done = rt_task_join_result_bytes_poll(ctx, task_id, NULL);
        if (!done) return UINT32_C(0);
      } else {
        rt_trap("scope child kind invalid");
      }
      s->join_child_i += 1;
    }
    s->join_phase = 0;
    s->join_slot_i = 0;
    s->join_child_i = 0;
    return UINT32_C(1);
  }

  rt_trap("scope join invalid state");
}

static void rt_scope_join_drop_remaining_block(ctx_t* ctx, rt_scope_t* s) {
  for (uint32_t slot_id = 0; slot_id < s->slot_len; slot_id++) {
    rt_scope_slot_t* slot = &s->slots[slot_id];
    if (slot->state != RT_SCOPE_SLOT_PENDING) continue;
    if (slot->kind == RT_TASK_OUT_KIND_BYTES) {
      uint32_t task_id = slot->task_id;
      bytes_t out = rt_task_join_bytes_block(ctx, task_id);
      rt_scope_check_slot_result_size(s, out);
      rt_bytes_drop(ctx, &out);
      rt_scope_unregister_task(s, task_id);
    } else if (slot->kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
      uint32_t task_id = slot->task_id;
      result_bytes_t out = rt_task_join_result_bytes_block(ctx, task_id);
      if (out.tag) {
        rt_scope_check_slot_result_size(s, out.payload.ok);
        rt_bytes_drop(ctx, &out.payload.ok);
      }
      rt_scope_unregister_task(s, task_id);
    } else {
      rt_trap("scope slot kind invalid");
    }
    slot->state = RT_SCOPE_SLOT_CONSUMED;
    slot->task_id = 0;
  }

  for (uint32_t i = 0; i < s->child_len; i++) {
    uint32_t task_id = s->child_task_ids[i];
    uint32_t kind = s->child_task_kinds[i];
    if (kind == RT_TASK_OUT_KIND_BYTES) {
      bytes_t out = rt_task_join_bytes_block(ctx, task_id);
      rt_bytes_drop(ctx, &out);
    } else if (kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
      result_bytes_t out = rt_task_join_result_bytes_block(ctx, task_id);
      if (out.tag) {
        rt_bytes_drop(ctx, &out.payload.ok);
      }
    } else {
      rt_trap("scope child kind invalid");
    }
  }
}

static void rt_scope_budget_check_exit(ctx_t* ctx, rt_scope_t* s) {
  uint64_t ticks = ctx->sched_now_ticks - s->snap_ticks;
  if (s->max_ticks != 0 && ticks > s->max_ticks) rt_trap("X07T_SCOPE_BUDGET_TICKS_EXCEEDED");

  uint64_t blocked = ctx->sched_stats.blocked_waits - s->snap_blocked_waits;
  if (s->max_blocked_waits != 0 && blocked > s->max_blocked_waits) {
    rt_trap("X07T_SCOPE_BUDGET_BLOCKED_WAITS_EXCEEDED");
  }

  uint64_t joins = ctx->sched_stats.join_calls - s->snap_join_polls;
  if (s->max_join_polls != 0 && joins > s->max_join_polls) rt_trap("X07T_SCOPE_BUDGET_JOIN_POLLS_EXCEEDED");
}

static uint32_t rt_scope_exit_poll(ctx_t* ctx, rt_scope_t* s) {
  uint32_t done = rt_scope_join_drop_remaining_poll(ctx, s);
  if (!done) return UINT32_C(0);
  rt_scope_budget_check_exit(ctx, s);
  rt_scope_drop(ctx, s);
  return UINT32_C(1);
}

static void rt_scope_exit_block(ctx_t* ctx, rt_scope_t* s) {
  rt_scope_join_drop_remaining_block(ctx, s);
  rt_scope_budget_check_exit(ctx, s);
  rt_scope_drop(ctx, s);
}

static void rt_scope_dispose_on_drop(ctx_t* ctx, rt_scope_t* s) {
  // Best-effort scope cleanup used by async task drop (task cancel / task complete):
  // - cancel unfinished children in deterministic order (reverse registration)
  // - drop any finished but unconsumed outputs (without blocking)
  // - free scope buffers
  if (s->reg_len != 0) {
    for (uint32_t i = s->reg_len; i > 0; i--) {
      uint32_t task_id = s->reg_task_ids[i - 1];
      if (task_id == 0) continue;
      rt_task_t* t = rt_task_ptr(ctx, task_id);
      if (!t->done) {
        (void)rt_task_cancel(ctx, task_id);
        continue;
      }
      if (t->out_taken || t->canceled) continue;
      if (t->out.kind == RT_TASK_OUT_KIND_BYTES) {
        (void)rt_task_join_bytes_poll(ctx, task_id, NULL);
      } else if (t->out.kind == RT_TASK_OUT_KIND_RESULT_BYTES) {
        (void)rt_task_join_result_bytes_poll(ctx, task_id, NULL);
      } else {
        rt_trap("scope.drop invalid out kind");
      }
    }
  }
  rt_scope_drop(ctx, s);
}

static uint32_t rt_select_evt_new(
  ctx_t* ctx,
  uint64_t scope_key,
  uint32_t tag,
  uint32_t case_index,
  uint32_t src_id,
  bytes_t payload
) {
  // Reuse a free slot if possible.
  for (uint32_t i = 0; i < ctx->sched_select_evts_len; i++) {
    rt_select_evt_t* e = &ctx->sched_select_evts[i];
    if (e->alive) continue;
    memset(e, 0, sizeof(*e));
    e->alive = 1;
    e->tag = tag;
    e->case_index = case_index;
    e->src_id = src_id;
    e->scope_key = scope_key;
    e->payload = payload;
    return i + 1;
  }

  if (ctx->sched_select_evts_len == UINT32_MAX) rt_trap("select.evt.new overflow");
  uint32_t need = ctx->sched_select_evts_len + 1;
  rt_sched_select_evts_ensure_cap(ctx, need);
  uint32_t evt_id = need;
  rt_select_evt_t* e = &ctx->sched_select_evts[evt_id - 1];
  memset(e, 0, sizeof(*e));
  e->alive = 1;
  e->tag = tag;
  e->case_index = case_index;
  e->src_id = src_id;
  e->scope_key = scope_key;
  e->payload = payload;
  ctx->sched_select_evts_len = need;
  return evt_id;
}

static uint32_t rt_select_evt_tag(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  return e->tag;
}

static uint32_t rt_select_evt_case_index(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  return e->case_index;
}

static uint32_t rt_select_evt_src_id(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  return e->src_id;
}

static bytes_t rt_select_evt_take_bytes(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  if (e->taken) rt_trap("X07T_SELECT_EVT_ALREADY_TAKEN");
  e->taken = 1;
  bytes_t out = e->payload;
  e->payload = rt_bytes_empty(ctx);
  e->alive = 0;
  return out;
}

static void rt_select_evt_drop(ctx_t* ctx, uint32_t evt_id) {
  rt_select_evt_t* e = rt_select_evt_ptr(ctx, evt_id);
  if (e->taken) rt_trap("X07T_SELECT_EVT_ALREADY_TAKEN");
  rt_bytes_drop(ctx, &e->payload);
  e->payload = rt_bytes_empty(ctx);
  e->taken = 1;
  e->alive = 0;
}

static uint32_t rt_chan_bytes_new(ctx_t* ctx, uint32_t cap) {
  if (cap == 0) rt_trap("chan cap=0");
  rt_sched_chans_ensure_cap(ctx, ctx->sched_chans_len + 1);
  uint32_t chan_id = ctx->sched_chans_len + 1;
  rt_chan_bytes_t* c = &ctx->sched_chans[chan_id - 1];
  memset(c, 0, sizeof(*c));
  c->alive = 1;
  c->closed = 0;
  c->cap = cap;
  c->buf = (bytes_t*)rt_alloc(
    ctx,
    cap * (uint32_t)sizeof(bytes_t),
    (uint32_t)_Alignof(bytes_t)
  );
  c->head = 0;
  c->tail = 0;
  c->len = 0;
  ctx->sched_chans_len += 1;
  return chan_id;
}

static uint32_t rt_chan_bytes_send_poll(ctx_t* ctx, uint32_t chan_id, bytes_t msg) {
  ctx->sched_stats.chan_send_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("chan.send.poll from main");

  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) rt_trap("chan.send closed");

  if (c->len < c->cap) {
    c->buf[c->tail] = msg;
    c->tail = (c->tail + 1) % c->cap;
    c->len += 1;

    uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
    return UINT32_C(1);
  }

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_CHAN_SEND && me->wait_id == chan_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("chan.send while already waiting");
  me->wait_kind = RT_WAIT_CHAN_SEND;
  me->wait_id = chan_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_CHAN_SEND << 32) | chan_id);
  rt_wait_list_push(ctx, &c->send_wait_head, &c->send_wait_tail, cur);
  return UINT32_C(0);
}

static uint32_t rt_chan_bytes_recv_poll(ctx_t* ctx, uint32_t chan_id, bytes_t* out) {
  ctx->sched_stats.chan_recv_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("chan.recv.poll from main");

  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->len != 0) {
    bytes_t msg = c->buf[c->head];
    c->head = (c->head + 1) % c->cap;
    c->len -= 1;
    if (out) *out = msg;

    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
    return UINT32_C(1);
  }

  if (c->closed) {
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_CHAN_RECV && me->wait_id == chan_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("chan.recv while already waiting");
  me->wait_kind = RT_WAIT_CHAN_RECV;
  me->wait_id = chan_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_CHAN_RECV << 32) | chan_id);
  rt_wait_list_push(ctx, &c->recv_wait_head, &c->recv_wait_tail, cur);
  return UINT32_C(0);
}

static uint32_t rt_chan_bytes_send_block(ctx_t* ctx, uint32_t chan_id, bytes_t msg) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) rt_trap("chan.send closed");
  while (c->len >= c->cap) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    c = rt_chan_bytes_ptr(ctx, chan_id);
  }
  c->buf[c->tail] = msg;
  c->tail = (c->tail + 1) % c->cap;
  c->len += 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  return UINT32_C(1);
}

static uint32_t rt_chan_bytes_try_send_view(ctx_t* ctx, uint32_t chan_id, bytes_view_t msg) {
  ctx->sched_stats.chan_send_calls += 1;
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) return UINT32_C(2);
  if (c->len >= c->cap) return UINT32_C(0);

  c->buf[c->tail] = rt_view_to_bytes(ctx, msg);
  c->tail = (c->tail + 1) % c->cap;
  c->len += 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  return UINT32_C(1);
}

static result_bytes_t rt_chan_bytes_try_recv(ctx_t* ctx, uint32_t chan_id) {
  ctx->sched_stats.chan_recv_calls += 1;
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);

  if (c->len != 0) {
    bytes_t msg = c->buf[c->head];
    c->head = (c->head + 1) % c->cap;
    c->len -= 1;
    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
    return (result_bytes_t){ .tag = UINT32_C(1), .payload.ok = msg };
  }

  if (c->closed) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  }
  return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(1) };
}

static bytes_t rt_chan_bytes_recv_block(ctx_t* ctx, uint32_t chan_id) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  while (c->len == 0 && !c->closed) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    c = rt_chan_bytes_ptr(ctx, chan_id);
  }
  if (c->len == 0) return rt_bytes_empty(ctx);
  bytes_t msg = c->buf[c->head];
  c->head = (c->head + 1) % c->cap;
  c->len -= 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
  return msg;
}

static uint32_t rt_chan_bytes_close(ctx_t* ctx, uint32_t chan_id) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) return UINT32_C(0);
  c->closed = 1;
  for (;;) {
    uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
    if (w == 0) break;
    rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  }
  for (;;) {
    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w == 0) break;
    rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
  }
  return UINT32_C(1);
}

#if X07_ENABLE_STREAMING_FILE_IO
static uint32_t rt_io_reader_new_file(ctx_t* ctx, FILE* f, uint32_t pending_ticks) {
  if (!f) rt_trap("io.reader null file");
  rt_io_readers_ensure_cap(ctx, ctx->io_readers_len + 1);
  uint32_t reader_id = ctx->io_readers_len + 1;
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  memset(r, 0, sizeof(*r));
  r->alive = 1;
  r->kind = RT_IO_READER_KIND_FILE;
  r->eof = 0;
  r->pending_ticks = pending_ticks;
  r->f = f;
  r->bytes = rt_bytes_empty(ctx);
  r->pos = 0;
  ctx->io_readers_len += 1;
  return reader_id;
}
#else
static uint32_t rt_io_reader_new_file(ctx_t* ctx, void* f, uint32_t pending_ticks) {
  (void)ctx;
  (void)f;
  (void)pending_ticks;
  rt_trap("io.reader file disabled");
}
#endif

static uint32_t rt_io_reader_new_bytes(ctx_t* ctx, bytes_t b, uint32_t pending_ticks) {
  rt_io_readers_ensure_cap(ctx, ctx->io_readers_len + 1);
  uint32_t reader_id = ctx->io_readers_len + 1;
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  memset(r, 0, sizeof(*r));
  r->alive = 1;
  r->kind = RT_IO_READER_KIND_BYTES;
  r->eof = 0;
  r->pending_ticks = pending_ticks;
#if X07_ENABLE_STREAMING_FILE_IO
  r->f = NULL;
#endif
  r->bytes = b;
  r->pos = 0;
  ctx->io_readers_len += 1;
  return reader_id;
}

static uint32_t rt_io_read_poll(ctx_t* ctx, uint32_t reader_id, uint32_t max, bytes_t* out) {
  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader_id);
  if (max == 0 || r->eof) {
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep(ctx, ticks);
    return UINT32_C(0);
  }

  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    uint32_t n = (rem < max) ? rem : max;
    if (n) {
      r->pos = pos + n;
      bytes_t slice = (bytes_t){b.ptr + pos, n};
      if (out) *out = rt_bytes_clone(ctx, slice);
      return UINT32_C(1);
    }
    r->eof = 1;
    rt_bytes_drop(ctx, &r->bytes);
    r->bytes = rt_bytes_empty(ctx);
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

#if X07_ENABLE_STREAMING_FILE_IO
  if (r->kind != RT_IO_READER_KIND_FILE) rt_trap("io.read bad reader kind");
  if (!r->f) {
    r->eof = 1;
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  int c = fgetc(r->f);
  if (c == EOF) {
    fclose(r->f);
    r->f = NULL;
    r->eof = 1;
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  chunk.ptr[0] = (uint8_t)c;
  uint32_t got = 1;
  if (max > 1) {
    size_t n = fread(chunk.ptr + 1, 1, (size_t)(max - 1), r->f);
    if (n > (size_t)(UINT32_MAX - 1)) rt_trap("io.read too large");
    got += (uint32_t)n;
  }
  chunk.len = got;
  if (out) *out = chunk;
  return UINT32_C(1);
#else
  rt_trap("io.read bad reader kind");
#endif
}

static bytes_t rt_io_read_block(ctx_t* ctx, uint32_t reader_id, uint32_t max) {
  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader_id);
  if (max == 0 || r->eof) return rt_bytes_empty(ctx);

  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep_block(ctx, ticks);
  }

  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    uint32_t n = (rem < max) ? rem : max;
    if (n) {
      r->pos = pos + n;
      bytes_t slice = (bytes_t){b.ptr + pos, n};
      return rt_bytes_clone(ctx, slice);
    }
    r->eof = 1;
    rt_bytes_drop(ctx, &r->bytes);
    r->bytes = rt_bytes_empty(ctx);
    return rt_bytes_empty(ctx);
  }

#if X07_ENABLE_STREAMING_FILE_IO
  if (r->kind != RT_IO_READER_KIND_FILE) rt_trap("io.read bad reader kind");
  if (!r->f) {
    r->eof = 1;
    return rt_bytes_empty(ctx);
  }

  int c = fgetc(r->f);
  if (c == EOF) {
    fclose(r->f);
    r->f = NULL;
    r->eof = 1;
    return rt_bytes_empty(ctx);
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  chunk.ptr[0] = (uint8_t)c;
  uint32_t got = 1;
  if (max > 1) {
    size_t n = fread(chunk.ptr + 1, 1, (size_t)(max - 1), r->f);
    if (n > (size_t)(UINT32_MAX - 1)) rt_trap("io.read too large");
    got += (uint32_t)n;
  }
  chunk.len = got;
  return chunk;
#else
  rt_trap("io.read bad reader kind");
#endif
}

static bytes_t rt_iface_io_read_block(ctx_t* ctx, iface_t reader, uint32_t max) {
  if (max == 0) return rt_bytes_empty(ctx);
  if (reader.vtable == RT_IFACE_VTABLE_IO_READER) {
    return rt_io_read_block(ctx, reader.data, max);
  }
  if (reader.vtable < RT_IFACE_VTABLE_EXT_IO_READER_MIN || reader.vtable > RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    rt_trap("io.read bad iface vtable");
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, chunk.ptr, max);
  if (got == 0) {
    rt_bytes_drop(ctx, &chunk);
    return rt_bytes_empty(ctx);
  }
  chunk.len = got;
  return chunk;
}

static uint32_t rt_bufread_new(ctx_t* ctx, iface_t reader, uint32_t cap) {
  if (cap == 0) rt_trap("bufread cap=0");
  if (reader.vtable == RT_IFACE_VTABLE_IO_READER) {
    (void)rt_io_reader_ptr(ctx, reader.data);
  } else if (reader.vtable >= RT_IFACE_VTABLE_EXT_IO_READER_MIN && reader.vtable <= RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    // External IO reader: validated lazily on first read.
  } else {
    rt_trap("bufread.new bad iface vtable");
  }

  rt_bufreads_ensure_cap(ctx, ctx->bufreads_len + 1);
  uint32_t br_id = ctx->bufreads_len + 1;
  rt_bufread_t* br = &ctx->bufreads[br_id - 1];
  memset(br, 0, sizeof(*br));
  br->alive = 1;
  br->reader = reader;
  br->eof = 0;
  br->direct_bytes = 0;
  br->buf = rt_bytes_alloc(ctx, cap);
  br->start = 0;
  br->end = 0;
  ctx->bufreads_len += 1;
  return br_id;
}

static uint32_t rt_bufread_fill_poll(ctx_t* ctx, uint32_t br_id, bytes_view_t* out) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  if (br->start > br->end) rt_trap("bufread corrupt");
  uint32_t avail = br->end - br->start;
  if (avail != 0) {
    if (out) {
      if (br->direct_bytes) {
        iface_t reader = br->reader;
        if (reader.vtable != RT_IFACE_VTABLE_IO_READER) rt_trap("bufread corrupt");
        rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
        if (r->kind != RT_IO_READER_KIND_BYTES) rt_trap("bufread corrupt");
        *out = rt_bytes_subview(ctx, r->bytes, br->start, avail);
      } else {
        *out = rt_bytes_subview(ctx, br->buf, br->start, avail);
      }
    }
    return UINT32_C(1);
  }
  if (br->eof) {
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }

  iface_t reader = br->reader;
  if (reader.vtable != RT_IFACE_VTABLE_IO_READER) {
    uint32_t cap = br->buf.len;
    uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, br->buf.ptr, cap);
    br->direct_bytes = 0;
    br->start = 0;
    br->end = got;
    if (got == 0) {
      br->eof = 1;
      if (out) *out = rt_view_empty(ctx);
      return UINT32_C(1);
    }
    if (out) *out = rt_bytes_subview(ctx, br->buf, 0, got);
    return UINT32_C(1);
  }

  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
  if (r->eof) {
    br->eof = 1;
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }
  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep(ctx, ticks);
    return UINT32_C(0);
  }

  uint32_t cap = br->buf.len;
  uint32_t got = 0;
  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    got = (rem < cap) ? rem : cap;
    if (got) {
      br->direct_bytes = 1;
      br->start = pos;
      br->end = pos + got;
      r->pos = pos + got;
      if (out) *out = rt_bytes_subview(ctx, b, br->start, got);
      return UINT32_C(1);
    } else {
      r->eof = 1;
    }
  }
#if X07_ENABLE_STREAMING_FILE_IO
  else if (r->kind == RT_IO_READER_KIND_FILE) {
    if (!r->f) {
      r->eof = 1;
    } else {
      int c = fgetc(r->f);
      if (c == EOF) {
        fclose(r->f);
        r->f = NULL;
        r->eof = 1;
      } else {
        br->buf.ptr[0] = (uint8_t)c;
        got = 1;
        if (cap > 1) {
          size_t n = fread(br->buf.ptr + 1, 1, (size_t)(cap - 1), r->f);
          if (n > (size_t)(UINT32_MAX - 1)) rt_trap("bufread.fill too large");
          got += (uint32_t)n;
        }
      }
    }
  }
#endif
  else {
    rt_trap("bufread bad reader kind");
  }

  br->direct_bytes = 0;
  br->start = 0;
  br->end = got;
  if (got == 0) {
    br->eof = 1;
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }

  if (out) *out = rt_bytes_subview(ctx, br->buf, 0, got);
  return UINT32_C(1);
}

static bytes_view_t rt_bufread_fill_block(ctx_t* ctx, uint32_t br_id) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  for (;;) {
    if (br->start > br->end) rt_trap("bufread corrupt");
    uint32_t avail = br->end - br->start;
    if (avail != 0) {
      if (br->direct_bytes) {
        iface_t reader = br->reader;
        if (reader.vtable != RT_IFACE_VTABLE_IO_READER) rt_trap("bufread corrupt");
        rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
        if (r->kind != RT_IO_READER_KIND_BYTES) rt_trap("bufread corrupt");
        return rt_bytes_subview(ctx, r->bytes, br->start, avail);
      }
      return rt_bytes_subview(ctx, br->buf, br->start, avail);
    }
    if (br->eof) return rt_view_empty(ctx);

    iface_t reader = br->reader;
    if (reader.vtable != RT_IFACE_VTABLE_IO_READER) {
      uint32_t cap = br->buf.len;
      uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, br->buf.ptr, cap);
      br->direct_bytes = 0;
      br->start = 0;
      br->end = got;
      if (got == 0) {
        br->eof = 1;
        return rt_view_empty(ctx);
      }
      return rt_bytes_subview(ctx, br->buf, 0, got);
    }

    rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
    if (r->eof) {
      br->eof = 1;
      return rt_view_empty(ctx);
    }
    if (r->pending_ticks != 0) {
      uint32_t ticks = r->pending_ticks;
      r->pending_ticks = 0;
      rt_task_sleep_block(ctx, ticks);
      continue;
    }

    uint32_t cap = br->buf.len;
    uint32_t got = 0;
    if (r->kind == RT_IO_READER_KIND_BYTES) {
      bytes_t b = r->bytes;
      uint32_t pos = r->pos;
      if (pos > b.len) pos = b.len;
      uint32_t rem = b.len - pos;
      got = (rem < cap) ? rem : cap;
      if (got) {
        br->direct_bytes = 1;
        br->start = pos;
        br->end = pos + got;
        r->pos = pos + got;
        return rt_bytes_subview(ctx, b, br->start, got);
      } else {
        r->eof = 1;
      }
    }
#if X07_ENABLE_STREAMING_FILE_IO
    else if (r->kind == RT_IO_READER_KIND_FILE) {
      if (!r->f) {
        r->eof = 1;
      } else {
        int c = fgetc(r->f);
        if (c == EOF) {
          fclose(r->f);
          r->f = NULL;
          r->eof = 1;
        } else {
          br->buf.ptr[0] = (uint8_t)c;
          got = 1;
          if (cap > 1) {
            size_t n = fread(br->buf.ptr + 1, 1, (size_t)(cap - 1), r->f);
            if (n > (size_t)(UINT32_MAX - 1)) rt_trap("bufread.fill too large");
            got += (uint32_t)n;
          }
        }
      }
    }
#endif
    else {
      rt_trap("bufread bad reader kind");
    }

    br->direct_bytes = 0;
    br->start = 0;
    br->end = got;
    if (got == 0) {
      br->eof = 1;
      return rt_view_empty(ctx);
    }

    return rt_bytes_subview(ctx, br->buf, 0, got);
  }
}

static uint32_t rt_bufread_consume(ctx_t* ctx, uint32_t br_id, uint32_t n) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  if (br->start > br->end) rt_trap("bufread corrupt");
  uint32_t avail = br->end - br->start;
  if (n > avail) rt_trap("bufread.consume oob");
  br->start += n;
  if (br->start == br->end) {
    br->start = 0;
    br->end = 0;
    br->direct_bytes = 0;
  }
  return UINT32_C(0);
}

static uint32_t rt_fs_is_safe_rel_path(bytes_view_t path) {
  if (path.len == 0) return UINT32_C(0);
  if (path.ptr[0] == (uint8_t)'/') return UINT32_C(0);

  uint32_t seg_start = 0;
  for (uint32_t i = 0; i <= path.len; i++) {
    uint8_t b = (i == path.len) ? (uint8_t)'/' : path.ptr[i];
    if (i < path.len) {
      if (b == 0 || b == (uint8_t)'\\') return UINT32_C(0);
    }
    if (b == (uint8_t)'/') {
      uint32_t seg_len = i - seg_start;
      if (seg_len == 0) return UINT32_C(0);
      if (seg_len == 1 && path.ptr[seg_start] == (uint8_t)'.') return UINT32_C(0);
      if (seg_len == 2
          && path.ptr[seg_start] == (uint8_t)'.'
          && path.ptr[seg_start + 1] == (uint8_t)'.') return UINT32_C(0);
      if (seg_len >= 5 && memcmp(path.ptr + seg_start, ".x07_", 5) == 0) return UINT32_C(0);
      seg_start = i + 1;
    }
  }
  return UINT32_C(1);
}

#if X07_ENABLE_FS
static bytes_t rt_fs_read(ctx_t* ctx, bytes_view_t path) {
  if (!X07_ENABLE_FS) rt_trap("fs disabled");
  if (!rt_fs_is_safe_rel_path(path)) rt_trap("fs.read unsafe path");
  ctx->fs_read_file_calls += 1;

  char* p = (char*)rt_alloc(ctx, path.len + 1, 1);
  memcpy(p, path.ptr, path.len);
  rt_mem_on_memcpy(ctx, path.len);
  p[path.len] = 0;

  FILE* f = fopen(p, "rb");
  if (!f) rt_trap_path("fs.read open failed", p);
  rt_free(ctx, p, path.len + 1, 1);

  if (fseek(f, 0, SEEK_END) != 0) rt_trap("fs.read seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("fs.read tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("fs.read file too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("fs.read seek failed");

  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)end);
  if (out.len != 0) {
    size_t n = fread(out.ptr, 1, out.len, f);
    if (n != out.len) rt_trap("fs.read short read");
  }
  fclose(f);
  return out;
}

static int rt_fs_name_cmp(const void* a, const void* b) {
  const char* const* pa = (const char* const*)a;
  const char* const* pb = (const char* const*)b;
  return strcmp(*pa, *pb);
}

static bytes_t rt_fs_list_dir(ctx_t* ctx, bytes_view_t path) {
  if (!X07_ENABLE_FS) rt_trap("fs disabled");
  if (!rt_fs_is_safe_rel_path(path)) rt_trap("fs.list_dir unsafe path");
  ctx->fs_list_dir_calls += 1;

  char* p = (char*)rt_alloc(ctx, path.len + 1, 1);
  memcpy(p, path.ptr, path.len);
  rt_mem_on_memcpy(ctx, path.len);
  p[path.len] = 0;

  DIR* dir = opendir(p);
  if (!dir) rt_trap_path("fs.list_dir open failed", p);

  uint32_t count = 0;
  for (;;) {
    struct dirent* ent = readdir(dir);
    if (!ent) break;
    const char* name = ent->d_name;
    if (!name) continue;
    if (name[0] == '.' && name[1] == 0) continue;
    if (name[0] == '.' && name[1] == '.' && name[2] == 0) continue;
    if (strncmp(name, ".x07_", 5) == 0) continue;
    count += 1;
  }
  closedir(dir);

  if (count == 0) {
    rt_free(ctx, p, path.len + 1, 1);
    bytes_t out;
    out.len = 0;
    out.ptr = (uint8_t*)rt_alloc(ctx, 0, 1);
    return out;
  }

  uint32_t names_cap = count;
  char** names = (char**)rt_alloc(ctx, count * (uint32_t)sizeof(char*), 8);

  dir = opendir(p);
  if (!dir) rt_trap_path("fs.list_dir open failed", p);

  uint32_t idx = 0;
  for (;;) {
    struct dirent* ent = readdir(dir);
    if (!ent) break;
    const char* name = ent->d_name;
    if (!name) continue;
    if (name[0] == '.' && name[1] == 0) continue;
    if (name[0] == '.' && name[1] == '.' && name[2] == 0) continue;
    if (strncmp(name, ".x07_", 5) == 0) continue;

    size_t len = strlen(name);
    if (len > (size_t)UINT32_MAX) rt_trap("fs.list_dir name too long");
    char* copy = (char*)rt_alloc(ctx, (uint32_t)len + 1, 1);
    memcpy(copy, name, len + 1);
    rt_mem_on_memcpy(ctx, (uint32_t)len + 1);
    if (idx < count) names[idx] = copy;
    idx += 1;
  }
  closedir(dir);

  if (idx < count) count = idx;
  qsort(names, count, sizeof(char*), rt_fs_name_cmp);

  uint64_t out_len_u64 = 0;
  for (uint32_t i = 0; i < count; i++) {
    size_t len = strlen(names[i]);
    if (len > (size_t)UINT32_MAX) rt_trap("fs.list_dir name too long");
    out_len_u64 += (uint64_t)len + 1;
    if (out_len_u64 > (uint64_t)UINT32_MAX) rt_trap("fs.list_dir output too large");
  }

  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)out_len_u64);
  uint32_t off = 0;
  for (uint32_t i = 0; i < count; i++) {
    uint32_t len = (uint32_t)strlen(names[i]);
    if (len) {
      memcpy(out.ptr + off, names[i], len);
      rt_mem_on_memcpy(ctx, len);
      off += len;
    }
    out.ptr[off] = (uint8_t)'\n';
    off += 1;
  }

  for (uint32_t i = 0; i < count; i++) {
    size_t len = strlen(names[i]);
    if (len > (size_t)UINT32_MAX) rt_trap("fs.list_dir name too long");
    rt_free(ctx, names[i], (uint32_t)len + 1, 1);
  }
  rt_free(ctx, names, names_cap * (uint32_t)sizeof(char*), 8);
  rt_free(ctx, p, path.len + 1, 1);
  return out;
}

static void rt_fs_latency_load(ctx_t* ctx) {
  if (ctx->fs_latency_loaded) return;
  ctx->fs_latency_loaded = 1;
  ctx->fs_latency_default_ticks = 0;
  ctx->fs_latency_entries = NULL;
  ctx->fs_latency_len = 0;
  ctx->fs_latency_blob = rt_bytes_empty(ctx);

  FILE* f = fopen(".x07_fs/latency.evfslat", "rb");
  if (!f) return;
  if (fseek(f, 0, SEEK_END) != 0) rt_trap("fs latency seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("fs latency tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("fs latency too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("fs latency seek failed");

  bytes_t blob = rt_bytes_alloc(ctx, (uint32_t)end);
  if (blob.len != 0) {
    size_t got = fread(blob.ptr, 1, blob.len, f);
    if (got != blob.len) rt_trap("fs latency short read");
  }
  fclose(f);

  if (blob.len < 16) rt_trap("fs latency too short");
  if (memcmp(blob.ptr, "X7FL", 4) != 0) rt_trap("fs latency bad magic");
  uint16_t ver = rt_read_u16_le(blob.ptr + 4);
  if (ver != 1) rt_trap("fs latency bad version");

  uint32_t default_ticks = rt_read_u32_le(blob.ptr + 8);
  uint32_t count = rt_read_u32_le(blob.ptr + 12);

  fs_latency_entry_t* entries = NULL;
  if (count != 0) {
    entries = (fs_latency_entry_t*)rt_alloc(
      ctx,
      count * (uint32_t)sizeof(fs_latency_entry_t),
      (uint32_t)_Alignof(fs_latency_entry_t)
    );
  }

  uint32_t off = 16;
  for (uint32_t i = 0; i < count; i++) {
    if (off > blob.len || blob.len - off < 4) rt_trap("fs latency truncated path_len");
    uint32_t plen = rt_read_u32_le(blob.ptr + off);
    off += 4;
    if (off > blob.len || blob.len - off < plen) rt_trap("fs latency truncated path");
    entries[i].path = (bytes_t){blob.ptr + off, plen};
    off += plen;
    if (off > blob.len || blob.len - off < 4) rt_trap("fs latency truncated ticks");
    entries[i].ticks = rt_read_u32_le(blob.ptr + off);
    off += 4;
  }
  if (off != blob.len) rt_trap("fs latency trailing bytes");

  ctx->fs_latency_default_ticks = default_ticks;
  ctx->fs_latency_entries = entries;
  ctx->fs_latency_len = count;
  ctx->fs_latency_blob = blob;
}

static uint32_t rt_fs_latency_ticks(ctx_t* ctx, bytes_view_t path) {
  (void)ctx;
  rt_fs_latency_load(ctx);
  for (uint32_t i = 0; i < ctx->fs_latency_len; i++) {
    bytes_t p = ctx->fs_latency_entries[i].path;
    if (p.len != path.len) continue;
    if (p.len == 0) return ctx->fs_latency_entries[i].ticks;
    if (memcmp(p.ptr, path.ptr, p.len) == 0) return ctx->fs_latency_entries[i].ticks;
  }
  return ctx->fs_latency_default_ticks;
}

static uint32_t rt_fs_open_read(ctx_t* ctx, bytes_view_t path) {
  if (!X07_ENABLE_FS) rt_trap("fs disabled");
  if (!rt_fs_is_safe_rel_path(path)) rt_trap("fs.open_read unsafe path");
  ctx->fs_read_file_calls += 1;

  char* p = (char*)rt_alloc(ctx, path.len + 1, 1);
  memcpy(p, path.ptr, path.len);
  rt_mem_on_memcpy(ctx, path.len);
  p[path.len] = 0;

  FILE* f = fopen(p, "rb");
  if (!f) rt_trap_path("fs.open_read open failed", p);
  rt_free(ctx, p, path.len + 1, 1);

  uint32_t ticks = rt_fs_latency_ticks(ctx, path);
  return rt_io_reader_new_file(ctx, f, ticks);
}

static bytes_t rt_fs_read_async_block(ctx_t* ctx, bytes_view_t path) {
  uint32_t ticks = rt_fs_latency_ticks(ctx, path);
  if (ticks != 0) {
    rt_task_sleep_block(ctx, ticks);
  }
  return rt_fs_read(ctx, path);
}
#endif

static uint32_t rt_sha256_rotr(uint32_t x, uint32_t n) {
  return (x >> n) | (x << (32u - n));
}

static uint32_t rt_sha256_load_u32_be(const uint8_t* p) {
  return ((uint32_t)p[0] << 24)
       | ((uint32_t)p[1] << 16)
       | ((uint32_t)p[2] << 8)
       | (uint32_t)p[3];
}

static void rt_sha256_store_u32_be(uint8_t* out, uint32_t x) {
  out[0] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
  out[1] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  out[2] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  out[3] = (uint8_t)(x & UINT32_C(0xFF));
}

static void rt_sha256_compress(uint32_t state[8], const uint8_t block[64]) {
  static const uint32_t K[64] = {
    UINT32_C(0x428a2f98), UINT32_C(0x71374491), UINT32_C(0xb5c0fbcf), UINT32_C(0xe9b5dba5),
    UINT32_C(0x3956c25b), UINT32_C(0x59f111f1), UINT32_C(0x923f82a4), UINT32_C(0xab1c5ed5),
    UINT32_C(0xd807aa98), UINT32_C(0x12835b01), UINT32_C(0x243185be), UINT32_C(0x550c7dc3),
    UINT32_C(0x72be5d74), UINT32_C(0x80deb1fe), UINT32_C(0x9bdc06a7), UINT32_C(0xc19bf174),
    UINT32_C(0xe49b69c1), UINT32_C(0xefbe4786), UINT32_C(0x0fc19dc6), UINT32_C(0x240ca1cc),
    UINT32_C(0x2de92c6f), UINT32_C(0x4a7484aa), UINT32_C(0x5cb0a9dc), UINT32_C(0x76f988da),
    UINT32_C(0x983e5152), UINT32_C(0xa831c66d), UINT32_C(0xb00327c8), UINT32_C(0xbf597fc7),
    UINT32_C(0xc6e00bf3), UINT32_C(0xd5a79147), UINT32_C(0x06ca6351), UINT32_C(0x14292967),
    UINT32_C(0x27b70a85), UINT32_C(0x2e1b2138), UINT32_C(0x4d2c6dfc), UINT32_C(0x53380d13),
    UINT32_C(0x650a7354), UINT32_C(0x766a0abb), UINT32_C(0x81c2c92e), UINT32_C(0x92722c85),
    UINT32_C(0xa2bfe8a1), UINT32_C(0xa81a664b), UINT32_C(0xc24b8b70), UINT32_C(0xc76c51a3),
    UINT32_C(0xd192e819), UINT32_C(0xd6990624), UINT32_C(0xf40e3585), UINT32_C(0x106aa070),
    UINT32_C(0x19a4c116), UINT32_C(0x1e376c08), UINT32_C(0x2748774c), UINT32_C(0x34b0bcb5),
    UINT32_C(0x391c0cb3), UINT32_C(0x4ed8aa4a), UINT32_C(0x5b9cca4f), UINT32_C(0x682e6ff3),
    UINT32_C(0x748f82ee), UINT32_C(0x78a5636f), UINT32_C(0x84c87814), UINT32_C(0x8cc70208),
    UINT32_C(0x90befffa), UINT32_C(0xa4506ceb), UINT32_C(0xbef9a3f7), UINT32_C(0xc67178f2),
  };

  uint32_t w[64];
  for (uint32_t i = 0; i < 16; i++) {
    w[i] = rt_sha256_load_u32_be(block + (i * 4));
  }
  for (uint32_t i = 16; i < 64; i++) {
    uint32_t s0 = rt_sha256_rotr(w[i - 15], 7) ^ rt_sha256_rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
    uint32_t s1 = rt_sha256_rotr(w[i - 2], 17) ^ rt_sha256_rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
    w[i] = w[i - 16] + s0 + w[i - 7] + s1;
  }

  uint32_t a = state[0];
  uint32_t b = state[1];
  uint32_t c = state[2];
  uint32_t d = state[3];
  uint32_t e = state[4];
  uint32_t f = state[5];
  uint32_t g = state[6];
  uint32_t h = state[7];

  for (uint32_t i = 0; i < 64; i++) {
    uint32_t S1 = rt_sha256_rotr(e, 6) ^ rt_sha256_rotr(e, 11) ^ rt_sha256_rotr(e, 25);
    uint32_t ch = (e & f) ^ ((~e) & g);
    uint32_t temp1 = h + S1 + ch + K[i] + w[i];
    uint32_t S0 = rt_sha256_rotr(a, 2) ^ rt_sha256_rotr(a, 13) ^ rt_sha256_rotr(a, 22);
    uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
    uint32_t temp2 = S0 + maj;

    h = g;
    g = f;
    f = e;
    e = d + temp1;
    d = c;
    c = b;
    b = a;
    a = temp1 + temp2;
  }

  state[0] += a;
  state[1] += b;
  state[2] += c;
  state[3] += d;
  state[4] += e;
  state[5] += f;
  state[6] += g;
  state[7] += h;
}

static void rt_sha256(const uint8_t* data, uint32_t len, uint8_t out[32]) {
  uint32_t state[8] = {
    UINT32_C(0x6a09e667),
    UINT32_C(0xbb67ae85),
    UINT32_C(0x3c6ef372),
    UINT32_C(0xa54ff53a),
    UINT32_C(0x510e527f),
    UINT32_C(0x9b05688c),
    UINT32_C(0x1f83d9ab),
    UINT32_C(0x5be0cd19),
  };

  uint32_t off = 0;
  while (len - off >= 64) {
    rt_sha256_compress(state, data + off);
    off += 64;
  }

  uint64_t bit_len = (uint64_t)len * UINT64_C(8);
  uint8_t block[128];
  memset(block, 0, sizeof(block));
  uint32_t rem = len - off;
  if (rem) memcpy(block, data + off, rem);
  block[rem] = UINT8_C(0x80);

  if (rem < 56) {
    for (uint32_t i = 0; i < 8; i++) {
      block[56 + i] = (uint8_t)((bit_len >> (56 - (i * 8))) & UINT64_C(0xFF));
    }
    rt_sha256_compress(state, block);
  } else {
    for (uint32_t i = 0; i < 8; i++) {
      block[120 + i] = (uint8_t)((bit_len >> (56 - (i * 8))) & UINT64_C(0xFF));
    }
    rt_sha256_compress(state, block);
    rt_sha256_compress(state, block + 64);
  }

  for (uint32_t i = 0; i < 8; i++) {
    rt_sha256_store_u32_be(out + (i * 4), state[i]);
  }
}

static void rt_hex_bytes(const uint8_t* bytes, uint32_t len, char* out) {
  static const char LUT[16] = "0123456789abcdef";
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = bytes[i];
    out[i * 2 + 0] = LUT[b >> 4];
    out[i * 2 + 1] = LUT[b & 0x0F];
  }
  out[len * 2] = 0;
}

#if X07_ENABLE_RR
#define RT_RR_MODE_OFF UINT8_C(0)
#define RT_RR_MODE_RECORD_V1 UINT8_C(1)
#define RT_RR_MODE_REPLAY_V1 UINT8_C(2)
#define RT_RR_MODE_RECORD_MISSING_V1 UINT8_C(3)
#define RT_RR_MODE_REWRITE_V1 UINT8_C(4)

#define RT_RR_MATCH_LOOKUP_V1 UINT8_C(0)
#define RT_RR_MATCH_TRANSCRIPT_V1 UINT8_C(1)

#define RT_RR_ERR_CFG_INVALID UINT32_C(2000)
#define RT_RR_ERR_CFG_UNSUPPORTED UINT32_C(2001)
#define RT_RR_ERR_OPEN_FAILED UINT32_C(2002)
#define RT_RR_ERR_BUDGET_CASSETTE_BYTES UINT32_C(2003)
#define RT_RR_ERR_BUDGET_ENTRIES UINT32_C(2004)
#define RT_RR_ERR_BUDGET_REQ_BYTES UINT32_C(2005)
#define RT_RR_ERR_BUDGET_RESP_BYTES UINT32_C(2006)
#define RT_RR_ERR_BUDGET_KEY_BYTES UINT32_C(2007)
#define RT_RR_ERR_ENTRY_INVALID UINT32_C(2008)
#define RT_RR_ERR_MISS UINT32_C(2009)
#define RT_RR_ERR_KIND_MISMATCH UINT32_C(2010)
#define RT_RR_ERR_OP_MISMATCH UINT32_C(2011)
#define RT_RR_ERR_MODE_NO_REPLAY UINT32_C(2012)
#define RT_RR_ERR_MODE_NO_APPEND UINT32_C(2013)
#define RT_RR_ERR_TRUNCATED UINT32_C(2014)

static int rt_rr_cmp_bytes(const uint8_t* a, uint32_t a_len, const uint8_t* b, uint32_t b_len) {
  uint32_t m = (a_len < b_len) ? a_len : b_len;
  if (m) {
    int cmp = memcmp(a, b, m);
    if (cmp < 0) return -1;
    if (cmp > 0) return 1;
  }
  if (a_len < b_len) return -1;
  if (a_len > b_len) return 1;
  return 0;
}

static uint32_t rt_dm_skip_value_depth(const uint8_t* doc, uint32_t n, uint32_t off, uint32_t depth) {
  if (depth > 64) return 0;
  if (off >= n) return 0;
  uint8_t tag = doc[off];

  if (tag == UINT8_C(0)) return off + 1;
  if (tag == UINT8_C(1)) {
    if (n - off < 2) return 0;
    return off + 2;
  }
  if (tag == UINT8_C(2) || tag == UINT8_C(3)) {
    if (n - off < 5) return 0;
    uint32_t len = rt_read_u32_le(doc + off + 1);
    if (len > n - off - 5) return 0;
    return off + 5 + len;
  }
  if (tag == UINT8_C(4)) {
    if (n - off < 5) return 0;
    uint32_t count = rt_read_u32_le(doc + off + 1);
    uint32_t pos = off + 5;
    for (uint32_t i = 0; i < count; i++) {
      uint32_t next = rt_dm_skip_value_depth(doc, n, pos, depth + 1);
      if (next == 0) return 0;
      pos = next;
    }
    return pos;
  }
  if (tag == UINT8_C(5)) {
    if (n - off < 5) return 0;
    uint32_t count = rt_read_u32_le(doc + off + 1);
    uint32_t pos = off + 5;
    for (uint32_t i = 0; i < count; i++) {
      if (n - pos < 4) return 0;
      uint32_t klen = rt_read_u32_le(doc + pos);
      pos += 4;
      if (klen > n - pos) return 0;
      pos += klen;
      uint32_t next = rt_dm_skip_value_depth(doc, n, pos, depth + 1);
      if (next == 0) return 0;
      pos = next;
    }
    return pos;
  }

  return 0;
}

static uint32_t rt_dm_get_string_range(
  const uint8_t* doc,
  uint32_t n,
  uint32_t off,
  uint32_t* out_start,
  uint32_t* out_len
) {
  if (off >= n) return 0;
  if (doc[off] != UINT8_C(3)) return 0;
  if (n - off < 5) return 0;
  uint32_t len = rt_read_u32_le(doc + off + 1);
  if (len > n - off - 5) return 0;
  *out_start = off + 5;
  *out_len = len;
  return 1;
}

static uint32_t rt_dm_get_number_range(
  const uint8_t* doc,
  uint32_t n,
  uint32_t off,
  uint32_t* out_start,
  uint32_t* out_len
) {
  if (off >= n) return 0;
  if (doc[off] != UINT8_C(2)) return 0;
  if (n - off < 5) return 0;
  uint32_t len = rt_read_u32_le(doc + off + 1);
  if (len > n - off - 5) return 0;
  *out_start = off + 5;
  *out_len = len;
  return 1;
}

static uint32_t rt_rr_parse_entry_v1(ctx_t* ctx, rr_handle_t* h, const uint8_t* doc, uint32_t n, uint32_t blob_off, rr_entry_desc_t* out) {
  (void)ctx;
  if (n < 2) return RT_RR_ERR_ENTRY_INVALID;
  if (doc[0] != UINT8_C(1)) return RT_RR_ERR_ENTRY_INVALID;
  uint32_t map_off = 1;
  if (doc[map_off] != UINT8_C(5)) return RT_RR_ERR_ENTRY_INVALID;
  if (n - map_off < 5) return RT_RR_ERR_ENTRY_INVALID;
  uint32_t count = rt_read_u32_le(doc + map_off + 1);
  uint32_t pos = map_off + 5;

  const uint8_t* prev_key_ptr = NULL;
  uint32_t prev_key_len = 0;

  uint32_t found_kind = 0;
  uint32_t found_op = 0;
  uint32_t found_key = 0;
  uint32_t found_req = 0;
  uint32_t found_resp = 0;
  uint32_t found_err = 0;

  uint32_t key_bytes_len = 0;
  uint32_t req_bytes_len = 0;
  uint32_t resp_bytes_len = 0;

  out->latency_ticks = 0;

  for (uint32_t i = 0; i < count; i++) {
    if (n - pos < 4) return RT_RR_ERR_ENTRY_INVALID;
    uint32_t klen = rt_read_u32_le(doc + pos);
    pos += 4;
    if (klen > n - pos) return RT_RR_ERR_ENTRY_INVALID;
    const uint8_t* kptr = doc + pos;
    if (i != 0) {
      if (rt_rr_cmp_bytes(prev_key_ptr, prev_key_len, kptr, klen) >= 0) {
        return RT_RR_ERR_ENTRY_INVALID;
      }
    }
    prev_key_ptr = kptr;
    prev_key_len = klen;
    pos += klen;

    uint32_t v_off = pos;
    uint32_t v_end = rt_dm_skip_value_depth(doc, n, v_off, 0);
    if (v_end == 0) return RT_RR_ERR_ENTRY_INVALID;

    if (klen == 4 && memcmp(kptr, "kind", 4) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_string_range(doc, n, v_off, &start, &len)) return RT_RR_ERR_ENTRY_INVALID;
      out->kind_off = blob_off + start;
      out->kind_len = len;
      found_kind = 1;
    } else if (klen == 2 && memcmp(kptr, "op", 2) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_string_range(doc, n, v_off, &start, &len)) return RT_RR_ERR_ENTRY_INVALID;
      out->op_off = blob_off + start;
      out->op_len = len;
      found_op = 1;
    } else if (klen == 3 && memcmp(kptr, "key", 3) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_string_range(doc, n, v_off, &start, &len)) return RT_RR_ERR_ENTRY_INVALID;
      if (len > h->max_key_bytes) return RT_RR_ERR_BUDGET_KEY_BYTES;
      out->key_off = blob_off + start;
      out->key_len = len;
      key_bytes_len = len;
      found_key = 1;
    } else if (klen == 3 && memcmp(kptr, "req", 3) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_string_range(doc, n, v_off, &start, &len)) return RT_RR_ERR_ENTRY_INVALID;
      if (len > h->max_req_bytes) return RT_RR_ERR_BUDGET_REQ_BYTES;
      req_bytes_len = len;
      found_req = 1;
    } else if (klen == 4 && memcmp(kptr, "resp", 4) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_string_range(doc, n, v_off, &start, &len)) return RT_RR_ERR_ENTRY_INVALID;
      if (len > h->max_resp_bytes) return RT_RR_ERR_BUDGET_RESP_BYTES;
      resp_bytes_len = len;
      found_resp = 1;
    } else if (klen == 3 && memcmp(kptr, "err", 3) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_number_range(doc, n, v_off, &start, &len)) return RT_RR_ERR_ENTRY_INVALID;
      found_err = 1;
    } else if (klen == 13 && memcmp(kptr, "latency_ticks", 13) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_number_range(doc, n, v_off, &start, &len)) return RT_RR_ERR_ENTRY_INVALID;
      if (len == 0) return RT_RR_ERR_ENTRY_INVALID;
      uint32_t acc = 0;
      for (uint32_t j = 0; j < len; j++) {
        uint8_t c = doc[start + j];
        if (c < (uint8_t)'0' || c > (uint8_t)'9') return RT_RR_ERR_ENTRY_INVALID;
        uint32_t d = (uint32_t)(c - (uint8_t)'0');
        if (acc > (UINT32_MAX - d) / 10) return RT_RR_ERR_ENTRY_INVALID;
        acc = acc * 10 + d;
      }
      out->latency_ticks = acc;
    }

    pos = v_end;
  }
  if (pos != n) return RT_RR_ERR_ENTRY_INVALID;
  if (!found_kind || !found_op || !found_key || !found_req || !found_resp || !found_err) return RT_RR_ERR_ENTRY_INVALID;
  if (key_bytes_len > h->max_key_bytes) return RT_RR_ERR_BUDGET_KEY_BYTES;
  if (req_bytes_len > h->max_req_bytes) return RT_RR_ERR_BUDGET_REQ_BYTES;
  if (resp_bytes_len > h->max_resp_bytes) return RT_RR_ERR_BUDGET_RESP_BYTES;
  return 0;
}

static void rt_rr_handles_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->rr_handles_cap) return;
  rr_handle_t* old_items = ctx->rr_handles;
  uint32_t old_cap = ctx->rr_handles_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rr_handle_t);
  uint32_t new_cap = ctx->rr_handles_cap ? ctx->rr_handles_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rr_handle_t* items = (rr_handle_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rr_handle_t),
    (uint32_t)_Alignof(rr_handle_t)
  );
  if (old_items && ctx->rr_handles_len) {
    uint32_t bytes = ctx->rr_handles_len * (uint32_t)sizeof(rr_handle_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rr_handle_t));
  }
  ctx->rr_handles = items;
  ctx->rr_handles_cap = new_cap;
}

static rr_handle_t* rt_rr_handle_ptr(ctx_t* ctx, int32_t handle_i32) {
  if (handle_i32 <= 0) rt_trap("rr invalid handle");
  uint32_t handle = (uint32_t)handle_i32;
  if (!ctx->rr_handles || handle > ctx->rr_handles_len) rt_trap("rr invalid handle");
  rr_handle_t* h = &ctx->rr_handles[handle - 1];
  if (!h->alive) rt_trap("rr invalid handle");
  return h;
}

static void rt_rr_entries_ensure_cap(ctx_t* ctx, rr_cassette_t* c, uint32_t need) {
  if (need <= c->entries_cap) return;
  rr_entry_desc_t* old_items = c->entries;
  uint32_t old_cap = c->entries_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rr_entry_desc_t);
  uint32_t new_cap = c->entries_cap ? c->entries_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rr_entry_desc_t* items = (rr_entry_desc_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rr_entry_desc_t),
    (uint32_t)_Alignof(rr_entry_desc_t)
  );
  if (old_items && c->entries_len) {
    uint32_t bytes = c->entries_len * (uint32_t)sizeof(rr_entry_desc_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rr_entry_desc_t));
  }
  c->entries = items;
  c->entries_cap = new_cap;
}

static result_i32_t rt_rr_open_v1(ctx_t* ctx, bytes_view_t cfg) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  ctx->rr_open_calls += 1;

#ifdef X07_DEBUG_BORROW
  if (cfg.len != 0 && !rt_dbg_borrow_check(ctx, cfg.bid, cfg.off_bytes, cfg.len)) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  }
#endif

  if (cfg.len < 40) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  }
  if (memcmp(cfg.ptr, "X7RC", 4) != 0) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  }
  uint16_t ver = rt_read_u16_le(cfg.ptr + 4);
  if (ver != 1) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_UNSUPPORTED };
  }

  uint8_t mode = cfg.ptr[8];
  uint8_t match_mode = cfg.ptr[9];
  if (mode > RT_RR_MODE_REWRITE_V1) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  }
  if (match_mode != RT_RR_MATCH_LOOKUP_V1 && match_mode != RT_RR_MATCH_TRANSCRIPT_V1) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  }

  uint64_t max_cassette_bytes =
    ((uint64_t)cfg.ptr[12])
    | ((uint64_t)cfg.ptr[13] << 8)
    | ((uint64_t)cfg.ptr[14] << 16)
    | ((uint64_t)cfg.ptr[15] << 24)
    | ((uint64_t)cfg.ptr[16] << 32)
    | ((uint64_t)cfg.ptr[17] << 40)
    | ((uint64_t)cfg.ptr[18] << 48)
    | ((uint64_t)cfg.ptr[19] << 56);

  uint32_t max_entries = rt_read_u32_le(cfg.ptr + 20);
  uint32_t max_req_bytes = rt_read_u32_le(cfg.ptr + 24);
  uint32_t max_resp_bytes = rt_read_u32_le(cfg.ptr + 28);
  uint32_t max_key_bytes = rt_read_u32_le(cfg.ptr + 32);
  uint32_t cassette_len = rt_read_u32_le(cfg.ptr + 36);
  if (cassette_len > cfg.len - 40) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  }
  if (40 + cassette_len != cfg.len) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  }

  bytes_view_t cassette_path = rt_view_slice(ctx, cfg, 40, cassette_len);
  if (!rt_fs_is_safe_rel_path(cassette_path)) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  }

  rt_rr_handles_ensure_cap(ctx, ctx->rr_handles_len + 1);
  uint32_t handle_id = ctx->rr_handles_len + 1;
  rr_handle_t* h = &ctx->rr_handles[handle_id - 1];
  memset(h, 0, sizeof(*h));
  h->alive = 1;
  h->mode = mode;
  h->match_mode = match_mode;
  h->max_cassette_bytes = max_cassette_bytes;
  h->max_entries = max_entries;
  h->max_req_bytes = max_req_bytes;
  h->max_resp_bytes = max_resp_bytes;
  h->max_key_bytes = max_key_bytes;
  h->transcript_cassette = 0;
  h->transcript_idx = 0;

  h->cassettes_len = 0;
  h->cassettes_cap = 1;
  h->cassettes = (rr_cassette_t*)rt_alloc(ctx, (uint32_t)sizeof(rr_cassette_t), (uint32_t)_Alignof(rr_cassette_t));
  memset(h->cassettes, 0, (uint32_t)sizeof(rr_cassette_t));
  h->cassettes_len = 1;

  rr_cassette_t* c = &h->cassettes[0];
  c->path = rt_view_to_bytes(ctx, cassette_path);
  c->blob = rt_bytes_empty(ctx);
  c->entries = NULL;
  c->entries_len = 0;
  c->entries_cap = 0;
  c->file_bytes = 0;
  c->append_f = NULL;

  // Replay modes load entries from the cassette file.
  if (mode == RT_RR_MODE_REPLAY_V1 || mode == RT_RR_MODE_RECORD_MISSING_V1) {
    uint32_t saved_epoch = rt_mem_epoch_pause(ctx);

    const uint32_t prefix_len = 8; // ".x07_rr/"
    if (cassette_path.len > UINT32_MAX - prefix_len) {
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
    }
    uint32_t total = prefix_len + cassette_path.len;
    char* path = (char*)rt_alloc(ctx, total + 1, 1);
    memcpy(path, ".x07_rr/", prefix_len);
    rt_mem_on_memcpy(ctx, prefix_len);
    memcpy(path + prefix_len, cassette_path.ptr, cassette_path.len);
    rt_mem_on_memcpy(ctx, cassette_path.len);
    path[total] = 0;

    FILE* f = fopen(path, "rb");
    if (!f) {
      rt_free(ctx, path, total + 1, 1);
      if (mode == RT_RR_MODE_REPLAY_V1) {
        rt_mem_epoch_resume(ctx, saved_epoch);
        return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
      }
      // record_missing: allow empty cassette when missing.
      ctx->rr_handles_len += 1;
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_i32_t){ .tag = UINT32_C(1), .payload.ok = handle_id };
    }
    rt_free(ctx, path, total + 1, 1);

    if (fseek(f, 0, SEEK_END) != 0) {
      fclose(f);
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
    }
    long end = ftell(f);
    if (end < 0) {
      fclose(f);
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
    }
    if ((uint64_t)end > max_cassette_bytes) {
      fclose(f);
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_BUDGET_CASSETTE_BYTES };
    }
    if ((uint64_t)end > (uint64_t)UINT32_MAX) {
      fclose(f);
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
    }
    if (fseek(f, 0, SEEK_SET) != 0) {
      fclose(f);
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
    }

    bytes_t blob = rt_bytes_alloc(ctx, (uint32_t)end);
    if (blob.len != 0) {
      size_t got = fread(blob.ptr, 1, blob.len, f);
      if (got != blob.len) {
        fclose(f);
        rt_bytes_drop(ctx, &blob);
        rt_mem_epoch_resume(ctx, saved_epoch);
        return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
      }
    }
    fclose(f);

    c->blob = blob;
    c->file_bytes = (uint64_t)blob.len;

    uint32_t pos = 0;
    while (pos != blob.len) {
      if (blob.len - pos < 4) {
        return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_TRUNCATED };
      }
      uint32_t plen = rt_read_u32_le(blob.ptr + pos);
      pos += 4;
      if (plen > blob.len - pos) {
        return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_TRUNCATED };
      }
      uint32_t payload_off = pos;
      pos += plen;

      if (c->entries_len + 1 > max_entries) {
        rt_mem_epoch_resume(ctx, saved_epoch);
        return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_BUDGET_ENTRIES };
      }
      rt_rr_entries_ensure_cap(ctx, c, c->entries_len + 1);
      rr_entry_desc_t* e = &c->entries[c->entries_len];
      memset(e, 0, sizeof(*e));
      e->payload_off = payload_off;
      e->payload_len = plen;
      uint32_t err = rt_rr_parse_entry_v1(ctx, h, blob.ptr + payload_off, plen, payload_off, e);
      if (err != 0) {
        rt_mem_epoch_resume(ctx, saved_epoch);
        return (result_i32_t){ .tag = UINT32_C(0), .payload.err = err };
      }
      c->entries_len += 1;
    }

    rt_mem_epoch_resume(ctx, saved_epoch);
  }

  ctx->rr_handles_len += 1;
  return (result_i32_t){ .tag = UINT32_C(1), .payload.ok = handle_id };
}

static int32_t rt_rr_close_v1(ctx_t* ctx, int32_t handle_i32) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  ctx->rr_close_calls += 1;
  if (handle_i32 <= 0) return 0;
  uint32_t handle = (uint32_t)handle_i32;
  if (!ctx->rr_handles || handle > ctx->rr_handles_len) return 0;
  rr_handle_t* h = &ctx->rr_handles[handle - 1];
  if (!h->alive) return 0;

  for (uint32_t j = 0; j < h->cassettes_len; j++) {
    rr_cassette_t* c = &h->cassettes[j];
    if (c->append_f) {
      fclose((FILE*)c->append_f);
      c->append_f = NULL;
    }
    if (c->entries && c->entries_cap) {
      rt_free(
        ctx,
        c->entries,
        c->entries_cap * (uint32_t)sizeof(rr_entry_desc_t),
        (uint32_t)_Alignof(rr_entry_desc_t)
      );
    }
    c->entries = NULL;
    c->entries_len = 0;
    c->entries_cap = 0;
    rt_bytes_drop(ctx, &c->blob);
    c->blob = rt_bytes_empty(ctx);
    rt_bytes_drop(ctx, &c->path);
    c->path = rt_bytes_empty(ctx);
    c->file_bytes = 0;
  }

  if (h->cassettes && h->cassettes_cap) {
    rt_free(
      ctx,
      h->cassettes,
      h->cassettes_cap * (uint32_t)sizeof(rr_cassette_t),
      (uint32_t)_Alignof(rr_cassette_t)
    );
  }
  h->cassettes = NULL;
  h->cassettes_len = 0;
  h->cassettes_cap = 0;
  h->alive = 0;

  if (ctx->rr_current == handle_i32) {
    ctx->rr_current = 0;
  }
  return 1;
}

static bytes_t rt_rr_stats_v1(ctx_t* ctx, int32_t handle_i32) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  ctx->rr_stats_calls += 1;
  rr_handle_t* h = rt_rr_handle_ptr(ctx, handle_i32);
  uint32_t entries_total = 0;
  uint32_t used_total = 0;
  uint32_t bytes_total = 0;
  for (uint32_t i = 0; i < h->cassettes_len; i++) {
    rr_cassette_t* c = &h->cassettes[i];
    entries_total += c->entries_len;
    if (c->blob.len > UINT32_MAX - bytes_total) {
      bytes_total = UINT32_MAX;
    } else {
      bytes_total += c->blob.len;
    }
    for (uint32_t j = 0; j < c->entries_len; j++) {
      if (c->entries[j].used) used_total += 1;
    }
  }

  char buf[256];
  int n = snprintf(
    buf,
    sizeof(buf),
    "{\"v\":1,\"mode\":%u,\"match_mode\":%u,\"cassettes\":%u,\"entries\":%u,\"used\":%u,\"bytes\":%u}",
    (unsigned)h->mode,
    (unsigned)h->match_mode,
    (unsigned)h->cassettes_len,
    (unsigned)entries_total,
    (unsigned)used_total,
    (unsigned)bytes_total
  );
  if (n < 0) rt_trap("rr.stats_v1 snprintf failed");
  if ((size_t)n >= sizeof(buf)) n = (int)(sizeof(buf) - 1);
  return rt_bytes_from_literal(ctx, (const uint8_t*)buf, (uint32_t)n);
}

static result_bytes_t rt_rr_next_v1(ctx_t* ctx, int32_t handle_i32, bytes_view_t kind, bytes_view_t op, bytes_view_t key, uint32_t* out_latency_ticks, uint32_t do_sleep) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  ctx->rr_next_calls += 1;

#ifdef X07_DEBUG_BORROW
  if (kind.len != 0 && !rt_dbg_borrow_check(ctx, kind.bid, kind.off_bytes, kind.len)) return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_ENTRY_INVALID };
  if (op.len != 0 && !rt_dbg_borrow_check(ctx, op.bid, op.off_bytes, op.len)) return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_ENTRY_INVALID };
  if (key.len != 0 && !rt_dbg_borrow_check(ctx, key.bid, key.off_bytes, key.len)) return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_ENTRY_INVALID };
#endif

  if (out_latency_ticks) *out_latency_ticks = UINT32_C(0);

  rr_handle_t* h = rt_rr_handle_ptr(ctx, handle_i32);

  if (h->mode == RT_RR_MODE_OFF || h->mode == RT_RR_MODE_RECORD_V1 || h->mode == RT_RR_MODE_REWRITE_V1) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_MODE_NO_REPLAY };
  }

  if (key.len > h->max_key_bytes) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_BUDGET_KEY_BYTES };
  }

  if (h->match_mode == RT_RR_MATCH_TRANSCRIPT_V1) {
    // Consume entries sequentially.
    for (;;) {
      if (h->transcript_cassette >= h->cassettes_len) {
        ctx->rr_next_miss_calls += 1;
        return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_MISS };
      }
      rr_cassette_t* c = &h->cassettes[h->transcript_cassette];
      if (h->transcript_idx >= c->entries_len) {
        h->transcript_cassette += 1;
        h->transcript_idx = 0;
        continue;
      }
      rr_entry_desc_t* e = &c->entries[h->transcript_idx];
      h->transcript_idx += 1;

      const uint8_t* ekind = c->blob.ptr + e->kind_off;
      const uint8_t* eop = c->blob.ptr + e->op_off;

      if (e->kind_len != kind.len || memcmp(ekind, kind.ptr, kind.len) != 0) {
        ctx->rr_next_miss_calls += 1;
        return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_KIND_MISMATCH };
      }
      if (e->op_len != op.len || memcmp(eop, op.ptr, op.len) != 0) {
        ctx->rr_next_miss_calls += 1;
        return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OP_MISMATCH };
      }

      if (out_latency_ticks) *out_latency_ticks = e->latency_ticks;
      if (do_sleep && e->latency_ticks != 0) {
        rt_task_sleep_block(ctx, e->latency_ticks);
      }

      uint32_t saved_epoch = rt_mem_epoch_pause(ctx);
      bytes_t out = rt_bytes_alloc(ctx, e->payload_len);
      if (e->payload_len) {
        memcpy(out.ptr, c->blob.ptr + e->payload_off, e->payload_len);
        rt_mem_on_memcpy(ctx, e->payload_len);
      }
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_bytes_t){ .tag = UINT32_C(1), .payload.ok = out };
    }
  }

  // lookup_v1: earliest unused entry matching (kind, op, key) within earliest cassette.
  for (uint32_t ci = 0; ci < h->cassettes_len; ci++) {
    rr_cassette_t* c = &h->cassettes[ci];
    uint32_t best = UINT32_MAX;
    for (uint32_t i = 0; i < c->entries_len; i++) {
      rr_entry_desc_t* e = &c->entries[i];
      if (e->used) continue;
      if (e->kind_len != kind.len) continue;
      if (e->op_len != op.len) continue;
      if (e->key_len != key.len) continue;
      if (e->kind_len && memcmp(c->blob.ptr + e->kind_off, kind.ptr, kind.len) != 0) continue;
      if (e->op_len && memcmp(c->blob.ptr + e->op_off, op.ptr, op.len) != 0) continue;
      if (e->key_len && memcmp(c->blob.ptr + e->key_off, key.ptr, key.len) != 0) continue;
      best = i;
      break;
    }
    if (best != UINT32_MAX) {
      rr_entry_desc_t* e = &c->entries[best];
      e->used = 1;
      if (out_latency_ticks) *out_latency_ticks = e->latency_ticks;
      if (do_sleep && e->latency_ticks != 0) {
        rt_task_sleep_block(ctx, e->latency_ticks);
      }

      uint32_t saved_epoch = rt_mem_epoch_pause(ctx);
      bytes_t out = rt_bytes_alloc(ctx, e->payload_len);
      if (e->payload_len) {
        memcpy(out.ptr, c->blob.ptr + e->payload_off, e->payload_len);
        rt_mem_on_memcpy(ctx, e->payload_len);
      }
      rt_mem_epoch_resume(ctx, saved_epoch);
      return (result_bytes_t){ .tag = UINT32_C(1), .payload.ok = out };
    }
  }

  ctx->rr_next_miss_calls += 1;
  return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_MISS };
}

static uint32_t rt_rr_parse_i32_dec(const uint8_t* p, uint32_t n, int32_t* out) {
  if (n == 0) return 0;
  uint32_t i = 0;
  int neg = 0;
  if (p[0] == (uint8_t)'-') {
    neg = 1;
    i = 1;
    if (n == 1) return 0;
  }
  int32_t acc = 0;
  for (; i < n; i++) {
    uint8_t c = p[i];
    if (c < (uint8_t)'0' || c > (uint8_t)'9') return 0;
    int32_t d = (int32_t)(c - (uint8_t)'0');
    if (acc > (INT32_MAX - d) / 10) return 0;
    acc = acc * 10 + d;
  }
  *out = neg ? -acc : acc;
  return 1;
}

static bytes_t rt_rr_entry_resp_v1(ctx_t* ctx, bytes_view_t entry) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");

#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, entry.bid, entry.off_bytes, entry.len);
#endif

  if (entry.len < 2) rt_trap("rr.entry_resp_v1 invalid entry");
  if (entry.ptr[0] != UINT8_C(1)) rt_trap("rr.entry_resp_v1 invalid entry");
  uint32_t map_off = 1;
  if (map_off >= entry.len || entry.ptr[map_off] != UINT8_C(5)) rt_trap("rr.entry_resp_v1 invalid entry");
  if (entry.len - map_off < 5) rt_trap("rr.entry_resp_v1 invalid entry");
  uint32_t count = rt_read_u32_le(entry.ptr + map_off + 1);
  uint32_t pos = map_off + 5;
  for (uint32_t i = 0; i < count; i++) {
    if (entry.len - pos < 4) rt_trap("rr.entry_resp_v1 invalid entry");
    uint32_t klen = rt_read_u32_le(entry.ptr + pos);
    pos += 4;
    if (klen > entry.len - pos) rt_trap("rr.entry_resp_v1 invalid entry");
    const uint8_t* kptr = entry.ptr + pos;
    pos += klen;
    uint32_t v_off = pos;
    uint32_t v_end = rt_dm_skip_value_depth(entry.ptr, entry.len, v_off, 0);
    if (v_end == 0) rt_trap("rr.entry_resp_v1 invalid entry");
    if (klen == 4 && memcmp(kptr, "resp", 4) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_string_range(entry.ptr, entry.len, v_off, &start, &len)) rt_trap("rr.entry_resp_v1 invalid entry");

      uint32_t saved_epoch = rt_mem_epoch_pause(ctx);
      bytes_t out = rt_bytes_alloc(ctx, len);
      if (len) {
        memcpy(out.ptr, entry.ptr + start, len);
        rt_mem_on_memcpy(ctx, len);
      }
      rt_mem_epoch_resume(ctx, saved_epoch);
      return out;
    }
    pos = v_end;
  }
  rt_trap("rr.entry_resp_v1 missing resp");
}

static int32_t rt_rr_entry_err_v1(ctx_t* ctx, bytes_view_t entry) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");

#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, entry.bid, entry.off_bytes, entry.len);
#endif

  if (entry.len < 2) rt_trap("rr.entry_err_v1 invalid entry");
  if (entry.ptr[0] != UINT8_C(1)) rt_trap("rr.entry_err_v1 invalid entry");
  uint32_t map_off = 1;
  if (map_off >= entry.len || entry.ptr[map_off] != UINT8_C(5)) rt_trap("rr.entry_err_v1 invalid entry");
  if (entry.len - map_off < 5) rt_trap("rr.entry_err_v1 invalid entry");
  uint32_t count = rt_read_u32_le(entry.ptr + map_off + 1);
  uint32_t pos = map_off + 5;
  for (uint32_t i = 0; i < count; i++) {
    if (entry.len - pos < 4) rt_trap("rr.entry_err_v1 invalid entry");
    uint32_t klen = rt_read_u32_le(entry.ptr + pos);
    pos += 4;
    if (klen > entry.len - pos) rt_trap("rr.entry_err_v1 invalid entry");
    const uint8_t* kptr = entry.ptr + pos;
    pos += klen;
    uint32_t v_off = pos;
    uint32_t v_end = rt_dm_skip_value_depth(entry.ptr, entry.len, v_off, 0);
    if (v_end == 0) rt_trap("rr.entry_err_v1 invalid entry");
    if (klen == 3 && memcmp(kptr, "err", 3) == 0) {
      uint32_t start = 0;
      uint32_t len = 0;
      if (!rt_dm_get_number_range(entry.ptr, entry.len, v_off, &start, &len)) rt_trap("rr.entry_err_v1 invalid entry");
      int32_t out = 0;
      if (!rt_rr_parse_i32_dec(entry.ptr + start, len, &out)) rt_trap("rr.entry_err_v1 invalid err");
      return out;
    }
    pos = v_end;
  }
  rt_trap("rr.entry_err_v1 missing err");
}

static result_i32_t rt_rr_append_v1(ctx_t* ctx, int32_t handle_i32, bytes_view_t entry) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  ctx->rr_append_calls += 1;

#ifdef X07_DEBUG_BORROW
  if (entry.len != 0 && !rt_dbg_borrow_check(ctx, entry.bid, entry.off_bytes, entry.len)) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_ENTRY_INVALID };
  }
#endif

  rr_handle_t* h = rt_rr_handle_ptr(ctx, handle_i32);
  if (h->mode != RT_RR_MODE_RECORD_V1 && h->mode != RT_RR_MODE_RECORD_MISSING_V1 && h->mode != RT_RR_MODE_REWRITE_V1) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_MODE_NO_APPEND };
  }

  if (h->cassettes_len == 0) return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
  rr_cassette_t* c = &h->cassettes[h->cassettes_len - 1];

  // Validate entry doc.
  rr_entry_desc_t desc;
  memset(&desc, 0, sizeof(desc));
  desc.payload_off = 0;
  desc.payload_len = entry.len;
  uint32_t err = rt_rr_parse_entry_v1(ctx, h, entry.ptr, entry.len, 0, &desc);
  if (err != 0) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = err };
  }

  if (c->entries_len + 1 > h->max_entries) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_BUDGET_ENTRIES };
  }

  uint64_t new_bytes = c->file_bytes + 4 + (uint64_t)entry.len;
  if (new_bytes > h->max_cassette_bytes) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_BUDGET_CASSETTE_BYTES };
  }

  if (!c->append_f) {
    bytes_view_t cassette_path = rt_bytes_view(ctx, c->path);
    const uint32_t prefix_len = 8; // ".x07_rr/"
    if (cassette_path.len > UINT32_MAX - prefix_len) {
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_CFG_INVALID };
    }
    uint32_t total = prefix_len + cassette_path.len;
    char* path = (char*)rt_alloc(ctx, total + 1, 1);
    memcpy(path, ".x07_rr/", prefix_len);
    rt_mem_on_memcpy(ctx, prefix_len);
    memcpy(path + prefix_len, cassette_path.ptr, cassette_path.len);
    rt_mem_on_memcpy(ctx, cassette_path.len);
    path[total] = 0;

    const char* open_mode = "ab";
    if (h->mode == RT_RR_MODE_REWRITE_V1) {
      open_mode = "wb";
      h->mode = RT_RR_MODE_RECORD_V1;
      c->file_bytes = 0;
      if (c->entries_len) {
        c->entries_len = 0;
      }
    }

    FILE* f = fopen(path, open_mode);
    rt_free(ctx, path, total + 1, 1);
    if (!f) {
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
    }
    c->append_f = f;
  }

  // Append frame.
  uint8_t hdr[4];
  rt_write_u32_le(hdr, entry.len);
  if (fwrite(hdr, 1, 4, (FILE*)c->append_f) != 4) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
  }
  if (entry.len != 0) {
    if (fwrite(entry.ptr, 1, entry.len, (FILE*)c->append_f) != entry.len) {
      return (result_i32_t){ .tag = UINT32_C(0), .payload.err = RT_RR_ERR_OPEN_FAILED };
    }
  }
  fflush((FILE*)c->append_f);

  // Update in-memory list (blob is not updated).
  rt_rr_entries_ensure_cap(ctx, c, c->entries_len + 1);
  rr_entry_desc_t* e = &c->entries[c->entries_len];
  memset(e, 0, sizeof(*e));
  *e = desc;
  e->used = 0;
  c->entries_len += 1;
  c->file_bytes = new_bytes;

  return (result_i32_t){ .tag = UINT32_C(1), .payload.ok = UINT32_C(0) };
}
#endif

static uint32_t rt_kv_u32_le(const uint8_t* p) {
  return (uint32_t)p[0]
       | ((uint32_t)p[1] << 8)
       | ((uint32_t)p[2] << 16)
       | ((uint32_t)p[3] << 24);
}

#if X07_ENABLE_KV
static void rt_kv_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->kv_cap) return;
  kv_entry_t* old_items = ctx->kv_items;
  uint32_t old_cap = ctx->kv_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(kv_entry_t);
  uint32_t new_cap = ctx->kv_cap ? ctx->kv_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  kv_entry_t* items = (kv_entry_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(kv_entry_t),
    (uint32_t)_Alignof(kv_entry_t)
  );
  if (old_items && ctx->kv_len) {
    uint32_t bytes = ctx->kv_len * (uint32_t)sizeof(kv_entry_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(kv_entry_t));
  }
  ctx->kv_items = items;
  ctx->kv_cap = new_cap;
}

static uint32_t rt_kv_find(ctx_t* ctx, bytes_view_t key) {
#ifdef X07_DEBUG_BORROW
  if (key.len != 0 && !rt_dbg_borrow_check(ctx, key.bid, key.off_bytes, key.len)) {
    return UINT32_MAX;
  }
#endif
  for (uint32_t i = 0; i < ctx->kv_len; i++) {
    bytes_t k = ctx->kv_items[i].key;
    if (k.len != key.len) continue;
    if (k.len == 0) return i;
    if (memcmp(k.ptr, key.ptr, k.len) == 0) return i;
  }
  return UINT32_MAX;
}

static void rt_kv_init(ctx_t* ctx) {
  if (!X07_ENABLE_KV) return;

  FILE* f = fopen(".x07_kv/seed.evkv", "rb");
  if (!f) rt_trap("kv seed open failed");
  if (fseek(f, 0, SEEK_END) != 0) rt_trap("kv seed seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("kv seed tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("kv seed too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("kv seed seek failed");

  bytes_t seed = rt_bytes_alloc(ctx, (uint32_t)end);
  if (seed.len != 0) {
    size_t got = fread(seed.ptr, 1, seed.len, f);
    if (got != seed.len) rt_trap("kv seed short read");
  }
  fclose(f);

  if (seed.len < 10) rt_trap("kv seed too short");
  if (memcmp(seed.ptr, "X7KV", 4) != 0) rt_trap("kv seed bad magic");
  uint32_t ver = (uint32_t)seed.ptr[4] | ((uint32_t)seed.ptr[5] << 8);
  if (ver != 1) rt_trap("kv seed bad version");

  uint32_t count = rt_kv_u32_le(seed.ptr + 6);
  ctx->kv_items = NULL;
  ctx->kv_len = 0;
  ctx->kv_cap = 0;
  if (count != 0) {
    ctx->kv_items = (kv_entry_t*)rt_alloc(
      ctx,
      count * (uint32_t)sizeof(kv_entry_t),
      (uint32_t)_Alignof(kv_entry_t)
    );
    ctx->kv_cap = count;
  }

  uint32_t off = 10;
  for (uint32_t i = 0; i < count; i++) {
    if (off > seed.len || seed.len - off < 4) rt_trap("kv seed truncated klen");
    uint32_t klen = rt_kv_u32_le(seed.ptr + off);
    off += 4;
    if (off > seed.len || seed.len - off < klen) rt_trap("kv seed truncated key");
    bytes_t key = rt_bytes_alloc(ctx, klen);
    if (klen) {
      memcpy(key.ptr, seed.ptr + off, klen);
      rt_mem_on_memcpy(ctx, klen);
    }
    off += klen;

    if (off > seed.len || seed.len - off < 4) rt_trap("kv seed truncated vlen");
    uint32_t vlen = rt_kv_u32_le(seed.ptr + off);
    off += 4;
    if (off > seed.len || seed.len - off < vlen) rt_trap("kv seed truncated value");
    bytes_t val = rt_bytes_alloc(ctx, vlen);
    if (vlen) {
      memcpy(val.ptr, seed.ptr + off, vlen);
      rt_mem_on_memcpy(ctx, vlen);
    }
    off += vlen;

    ctx->kv_items[ctx->kv_len++] = (kv_entry_t){key, val};
  }
  if (off != seed.len) rt_trap("kv seed trailing bytes");
  rt_bytes_drop(ctx, &seed);
}

static void rt_kv_latency_load(ctx_t* ctx) {
  if (ctx->kv_latency_loaded) return;
  ctx->kv_latency_loaded = 1;
  ctx->kv_latency_default_ticks = 0;
  ctx->kv_latency_entries = NULL;
  ctx->kv_latency_len = 0;
  ctx->kv_latency_blob = rt_bytes_empty(ctx);

  FILE* f = fopen(".x07_kv/latency.evkvlat", "rb");
  if (!f) return;
  if (fseek(f, 0, SEEK_END) != 0) rt_trap("kv latency seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("kv latency tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("kv latency too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("kv latency seek failed");

  bytes_t blob = rt_bytes_alloc(ctx, (uint32_t)end);
  if (blob.len != 0) {
    size_t got = fread(blob.ptr, 1, blob.len, f);
    if (got != blob.len) rt_trap("kv latency short read");
  }
  fclose(f);

  if (blob.len < 16) rt_trap("kv latency too short");
  if (memcmp(blob.ptr, "X7KL", 4) != 0) rt_trap("kv latency bad magic");
  uint16_t ver = rt_read_u16_le(blob.ptr + 4);
  if (ver != 1) rt_trap("kv latency bad version");

  uint32_t default_ticks = rt_read_u32_le(blob.ptr + 8);
  uint32_t count = rt_read_u32_le(blob.ptr + 12);

  kv_latency_entry_t* entries = NULL;
  if (count != 0) {
    entries = (kv_latency_entry_t*)rt_alloc(
      ctx,
      count * (uint32_t)sizeof(kv_latency_entry_t),
      (uint32_t)_Alignof(kv_latency_entry_t)
    );
  }

  uint32_t off = 16;
  for (uint32_t i = 0; i < count; i++) {
    if (off > blob.len || blob.len - off < 4) rt_trap("kv latency truncated key_len");
    uint32_t klen = rt_read_u32_le(blob.ptr + off);
    off += 4;
    if (off > blob.len || blob.len - off < klen) rt_trap("kv latency truncated key");
    entries[i].key = (bytes_t){blob.ptr + off, klen};
    off += klen;
    if (off > blob.len || blob.len - off < 4) rt_trap("kv latency truncated ticks");
    entries[i].ticks = rt_read_u32_le(blob.ptr + off);
    off += 4;
  }
  if (off != blob.len) rt_trap("kv latency trailing bytes");

  ctx->kv_latency_default_ticks = default_ticks;
  ctx->kv_latency_entries = entries;
  ctx->kv_latency_len = count;
  ctx->kv_latency_blob = blob;
}

static uint32_t rt_kv_latency_ticks(ctx_t* ctx, bytes_view_t key) {
  if (!X07_ENABLE_KV) rt_trap("kv disabled");
  rt_kv_latency_load(ctx);
#ifdef X07_DEBUG_BORROW
  if (key.len != 0 && !rt_dbg_borrow_check(ctx, key.bid, key.off_bytes, key.len)) {
    return ctx->kv_latency_default_ticks;
  }
#endif
  for (uint32_t i = 0; i < ctx->kv_latency_len; i++) {
    bytes_t k = ctx->kv_latency_entries[i].key;
    if (k.len != key.len) continue;
    if (k.len == 0) return ctx->kv_latency_entries[i].ticks;
    if (memcmp(k.ptr, key.ptr, k.len) == 0) return ctx->kv_latency_entries[i].ticks;
  }
  return ctx->kv_latency_default_ticks;
}

static bytes_t rt_kv_get(ctx_t* ctx, bytes_view_t key) {
  if (!X07_ENABLE_KV) rt_trap("kv disabled");
  ctx->kv_get_calls += 1;
  uint32_t idx = rt_kv_find(ctx, key);
  if (idx == UINT32_MAX) return rt_bytes_empty(ctx);
  return rt_bytes_clone(ctx, ctx->kv_items[idx].val);
}

static bytes_t rt_kv_get_async_block(ctx_t* ctx, bytes_view_t key) {
  uint32_t ticks = rt_kv_latency_ticks(ctx, key);
  if (ticks != 0) {
    rt_task_sleep_block(ctx, ticks);
  }
  return rt_kv_get(ctx, key);
}

static uint32_t rt_kv_get_stream(ctx_t* ctx, bytes_view_t key) {
  if (!X07_ENABLE_KV) rt_trap("kv disabled");
  ctx->kv_get_calls += 1;
  uint32_t idx = rt_kv_find(ctx, key);
  bytes_t val =
      (idx == UINT32_MAX) ? rt_bytes_empty(ctx) : rt_bytes_clone(ctx, ctx->kv_items[idx].val);
  uint32_t ticks = rt_kv_latency_ticks(ctx, key);
  return rt_io_reader_new_bytes(ctx, val, ticks);
}

static uint32_t rt_kv_set(ctx_t* ctx, bytes_t key, bytes_t val) {
  if (!X07_ENABLE_KV) rt_trap("kv disabled");
  ctx->kv_set_calls += 1;

  uint32_t idx = rt_kv_find(ctx, rt_bytes_view(ctx, key));
  if (idx != UINT32_MAX) {
    rt_bytes_drop(ctx, &key);
    rt_bytes_drop(ctx, &ctx->kv_items[idx].val);
    ctx->kv_items[idx].val = val;
    return UINT32_C(0);
  }

  rt_kv_ensure_cap(ctx, ctx->kv_len + 1);
  ctx->kv_items[ctx->kv_len++] = (kv_entry_t){key, val};
  return UINT32_C(1);
}
#else
static void rt_kv_init(ctx_t* ctx) {
  (void)ctx;
}
#endif

static uint32_t rt_codec_read_u32_le(ctx_t* ctx, bytes_view_t buf, uint32_t offset) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes, buf.len);
#else
  (void)ctx;
#endif
  if (offset > buf.len || buf.len - offset < 4) rt_trap("codec.read_u32_le oob");
  return (uint32_t)buf.ptr[offset]
       | ((uint32_t)buf.ptr[offset + 1] << 8)
       | ((uint32_t)buf.ptr[offset + 2] << 16)
       | ((uint32_t)buf.ptr[offset + 3] << 24);
}

static bytes_t rt_codec_write_u32_le(ctx_t* ctx, uint32_t x) {
  bytes_t out = rt_bytes_alloc(ctx, 4);
  out.ptr[0] = (uint8_t)(x & UINT32_C(0xFF));
  out.ptr[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  out.ptr[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  out.ptr[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
  return out;
}

static bytes_t rt_fmt_u32_to_dec(ctx_t* ctx, uint32_t x) {
  uint8_t scratch[16];
  uint32_t n = 0;
  if (x == 0) {
    bytes_t out = rt_bytes_alloc(ctx, 1);
    out.ptr[0] = (uint8_t)'0';
    return out;
  }
  while (x > 0) {
    uint32_t digit = x % 10;
    x /= 10;
    scratch[n++] = (uint8_t)('0' + digit);
  }
  bytes_t out = rt_bytes_alloc(ctx, n);
  for (uint32_t i = 0; i < n; i++) {
    out.ptr[i] = scratch[n - 1 - i];
  }
  return out;
}

static bytes_t rt_fmt_s32_to_dec(ctx_t* ctx, uint32_t x) {
  if ((x & UINT32_C(0x80000000)) == 0) {
    return rt_fmt_u32_to_dec(ctx, x);
  }
  uint32_t mag = (~x) + UINT32_C(1);
  bytes_t digits = rt_fmt_u32_to_dec(ctx, mag);
  bytes_t out = rt_bytes_alloc(ctx, digits.len + 1);
  out.ptr[0] = (uint8_t)'-';
  memcpy(out.ptr + 1, digits.ptr, digits.len);
  rt_mem_on_memcpy(ctx, digits.len);
  return out;
}

static uint32_t rt_parse_u32_dec_slice(ctx_t* ctx, uint8_t* ptr, uint32_t len) {
  if (len == 0) rt_trap("parse.u32_dec empty");
  uint32_t acc = 0;
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = ptr[i];
    if (b < (uint8_t)'0' || b > (uint8_t)'9') rt_trap("parse.u32_dec non-digit");
    uint32_t digit = (uint32_t)(b - (uint8_t)'0');
    if (acc > (UINT32_MAX - digit) / 10) rt_trap("parse.u32_dec overflow");
    acc = acc * 10 + digit;
  }
  return acc;
}

static uint32_t rt_parse_u32_dec(ctx_t* ctx, bytes_view_t buf) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes, buf.len);
#endif
  return rt_parse_u32_dec_slice(ctx, buf.ptr, buf.len);
}

static uint32_t rt_parse_u32_dec_at(ctx_t* ctx, bytes_view_t buf, uint32_t offset) {
  if (offset > buf.len) rt_trap("parse.u32_dec_at oob");
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes + offset, buf.len - offset);
#endif
  return rt_parse_u32_dec_slice(ctx, buf.ptr + offset, buf.len - offset);
}

static uint32_t rt_prng_lcg_next_u32(uint32_t state) {
  return state * UINT32_C(1103515245) + UINT32_C(12345);
}

typedef struct {
  uint8_t* data;
  uint32_t len;
  uint32_t cap;
#ifdef X07_DEBUG_BORROW
  uint64_t dbg_aid;
#endif
} vec_u8_t;

static vec_u8_t rt_vec_u8_new(ctx_t* ctx, uint32_t cap) {
  vec_u8_t v;
  v.len = 0;
  v.cap = cap;
  v.data = (cap == 0) ? ctx->heap.mem : (uint8_t*)rt_alloc(ctx, cap, 1);
#ifdef X07_DEBUG_BORROW
  v.dbg_aid = (cap == 0) ? 0 : rt_dbg_alloc_register(ctx, v.data, cap);
#endif
  return v;
}

static void rt_vec_u8_drop(ctx_t* ctx, vec_u8_t* v) {
  if (!v) return;
  if (v->cap == 0) {
    v->data = ctx->heap.mem;
    v->len = 0;
    return;
  }
#ifdef X07_DEBUG_BORROW
  rt_dbg_alloc_kill(ctx, v->dbg_aid);
  v->dbg_aid = 0;
#endif
  rt_free(ctx, v->data, v->cap, 1);
  v->data = ctx->heap.mem;
  v->len = 0;
  v->cap = 0;
}

static uint32_t rt_vec_u8_len(ctx_t* ctx, vec_u8_t v) {
  (void)ctx;
  return v.len;
}

static uint32_t rt_vec_u8_cap(ctx_t* ctx, vec_u8_t v) {
  (void)ctx;
  return v.cap;
}

static vec_u8_t rt_vec_u8_clear(ctx_t* ctx, vec_u8_t v) {
  (void)ctx;
  v.len = 0;
  return v;
}

static uint32_t rt_vec_u8_get(ctx_t* ctx, vec_u8_t v, uint32_t idx) {
  (void)ctx;
  if (idx >= v.len) rt_trap("vec_u8.get oob");
  return (uint32_t)v.data[idx];
}

static vec_u8_t rt_vec_u8_set(ctx_t* ctx, vec_u8_t v, uint32_t idx, uint32_t val) {
  (void)ctx;
  if (idx >= v.len) rt_trap("vec_u8.set oob");
  v.data[idx] = (uint8_t)(val & UINT32_C(0xFF));
  return v;
}

static vec_u8_t rt_vec_u8_push(ctx_t* ctx, vec_u8_t v, uint32_t val) {
  if (v.len == v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? (v.cap * 2) : 1;
    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }
  v.data[v.len++] = (uint8_t)(val & UINT32_C(0xFF));
  return v;
}

static vec_u8_t rt_vec_u8_reserve_exact(ctx_t* ctx, vec_u8_t v, uint32_t additional) {
  if (additional > UINT32_MAX - v.len) rt_trap("vec_u8.reserve_exact overflow");
  uint32_t need = v.len + additional;
  if (need <= v.cap) return v;

  uint8_t* old_data = v.cap ? v.data : NULL;
  uint32_t old_cap = v.cap;
  uint8_t* data = (uint8_t*)rt_alloc_realloc(
      ctx,
      old_data,
      old_cap,
      need,
      1
  );
  if (v.data && v.len) {
    memcpy(data, v.data, v.len);
    rt_mem_on_memcpy(ctx, v.len);
  }
#ifdef X07_DEBUG_BORROW
  rt_dbg_alloc_kill(ctx, v.dbg_aid);
  v.dbg_aid = rt_dbg_alloc_register(ctx, data, need);
#endif
  if (old_data && old_cap) {
    rt_free(ctx, old_data, old_cap, 1);
  }
  v.data = data;
  v.cap = need;
  return v;
}

static vec_u8_t rt_vec_u8_extend_zeroes(ctx_t* ctx, vec_u8_t v, uint32_t n) {
  if (n > UINT32_MAX - v.len) rt_trap("vec_u8.extend_zeroes overflow");
  uint32_t need = v.len + n;
  if (need > v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? v.cap : 1;
    while (new_cap < need) {
      if (new_cap > UINT32_MAX / 2) {
        new_cap = need;
        break;
      }
      new_cap *= 2;
    }

    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }

  if (n) {
    memset(v.data + v.len, 0, n);
  }
  v.len += n;
  return v;
}

static vec_u8_t rt_vec_u8_extend_bytes(ctx_t* ctx, vec_u8_t v, bytes_view_t b) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b.len);
#endif
  if (b.len > UINT32_MAX - v.len) rt_trap("vec_u8.extend_bytes overflow");
  uint32_t need = v.len + b.len;
  if (need > v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? v.cap : 1;
    while (new_cap < need) {
      if (new_cap > UINT32_MAX / 2) {
        new_cap = need;
        break;
      }
      new_cap *= 2;
    }

    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }

  if (b.len) {
    memcpy(v.data + v.len, b.ptr, b.len);
    rt_mem_on_memcpy(ctx, b.len);
  }
  v.len += b.len;
  return v;
}

static vec_u8_t rt_vec_u8_extend_bytes_range(
    ctx_t* ctx,
    vec_u8_t v,
    bytes_view_t b,
    uint32_t start,
    uint32_t len
) {
  if (start > b.len || b.len - start < len) rt_trap("vec_u8.extend_bytes_range oob");
  bytes_view_t sub;
  sub.ptr = b.ptr + start;
  sub.len = len;
#ifdef X07_DEBUG_BORROW
  sub.aid = b.aid;
  sub.bid = b.bid;
  if (UINT32_MAX - b.off_bytes < start) rt_trap("vec_u8.extend_bytes_range off overflow");
  sub.off_bytes = b.off_bytes + start;
#endif
  return rt_vec_u8_extend_bytes(ctx, v, sub);
}

static bytes_view_t rt_vec_u8_as_view(ctx_t* ctx, vec_u8_t v) {
  bytes_view_t out;
  out.len = v.len;
#ifdef X07_DEBUG_BORROW
  if (out.len == 0) {
    out.ptr = ctx->heap.mem;
    out.aid = 0;
    out.bid = 0;
    out.off_bytes = 0;
    return out;
  }
  out.ptr = v.data;
  out.aid = v.dbg_aid;
  out.off_bytes = 0;
  out.bid = rt_dbg_alloc_borrow_id(ctx, out.aid);
#else
  out.ptr = (out.len == 0) ? ctx->heap.mem : v.data;
#endif
  return out;
}

static bytes_t rt_vec_u8_into_bytes(ctx_t* ctx, vec_u8_t* v) {
  if (!v) return rt_bytes_empty(ctx);
  if (v->len == 0) {
    rt_vec_u8_drop(ctx, v);
    return rt_bytes_empty(ctx);
  }

  bytes_t out;
  out.ptr = v->data;
  out.len = v->len;

  v->data = ctx->heap.mem;
  v->len = 0;
  v->cap = 0;
#ifdef X07_DEBUG_BORROW
  v->dbg_aid = 0;
#endif
  return out;
}

#ifndef X07_JSON_JCS_ENABLED
#define X07_JSON_JCS_ENABLED 0
#endif

// --- X07_JSON_JCS_START
//
// JSON Canonicalization Scheme (RFC 8785) runtime canonicalizer.
// Used by `std.stream.xf.json_canon_stream_v1` via the compiler builtin head:
// `json.jcs.canon_doc_v1(bytes_view, max_depth, max_object_members, max_object_total_bytes) -> bytes`.
//
// Return format:
// - ok:  [0]=1, followed by canonical JSON UTF-8 bytes.
// - err: [0]=0, [1..5)=u32_le code, [5..9)=u32_le byte offset in input.
//
#undef X07_JSON_JCS_ENABLED
#define X07_JSON_JCS_ENABLED 1

#define RT_JSON_JCS_E_JSON_SYNTAX UINT32_C(20)
#define RT_JSON_JCS_E_JSON_NOT_IJSON UINT32_C(21)
#define RT_JSON_JCS_E_JSON_TOO_DEEP UINT32_C(22)
#define RT_JSON_JCS_E_JSON_OBJECT_TOO_LARGE UINT32_C(23)
#define RT_JSON_JCS_E_JSON_TRAILING_DATA UINT32_C(24)

typedef struct {
  const uint8_t* buf;
  uint32_t len;
  uint32_t max_depth;
  uint32_t max_object_members;
  uint32_t max_object_total_bytes;
  uint32_t err_code;
  uint32_t err_off;
} rt_json_jcs_state_t;

static bytes_t rt_json_jcs_err_doc(ctx_t* ctx, uint32_t code, uint32_t off) {
  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(9));
  out.ptr[0] = UINT8_C(0);
  out.ptr[1] = (uint8_t)(code & UINT32_C(0xFF));
  out.ptr[2] = (uint8_t)((code >> 8) & UINT32_C(0xFF));
  out.ptr[3] = (uint8_t)((code >> 16) & UINT32_C(0xFF));
  out.ptr[4] = (uint8_t)((code >> 24) & UINT32_C(0xFF));
  out.ptr[5] = (uint8_t)(off & UINT32_C(0xFF));
  out.ptr[6] = (uint8_t)((off >> 8) & UINT32_C(0xFF));
  out.ptr[7] = (uint8_t)((off >> 16) & UINT32_C(0xFF));
  out.ptr[8] = (uint8_t)((off >> 24) & UINT32_C(0xFF));
  return out;
}

static uint32_t rt_json_jcs_fail(rt_json_jcs_state_t* st, uint32_t code, uint32_t off) {
  if (!st->err_code) {
    st->err_code = code;
    st->err_off = off;
  }
  return UINT32_MAX;
}

static uint32_t rt_json_jcs_is_ws(uint8_t c) {
  return (c == UINT8_C(0x20) || c == UINT8_C(0x09) || c == UINT8_C(0x0A) || c == UINT8_C(0x0D))
    ? UINT32_C(1)
    : UINT32_C(0);
}

static uint32_t rt_json_jcs_skip_ws(rt_json_jcs_state_t* st, uint32_t pos) {
  while (pos < st->len) {
    if (!rt_json_jcs_is_ws(st->buf[pos])) break;
    pos += 1;
  }
  return pos;
}

static int rt_json_jcs_hex_val(uint8_t c) {
  if (c >= (uint8_t)'0' && c <= (uint8_t)'9') return (int)(c - (uint8_t)'0');
  if (c >= (uint8_t)'a' && c <= (uint8_t)'f') return (int)(c - (uint8_t)'a') + 10;
  if (c >= (uint8_t)'A' && c <= (uint8_t)'F') return (int)(c - (uint8_t)'A') + 10;
  return -1;
}

static uint32_t rt_json_jcs_read_u16_hex4(rt_json_jcs_state_t* st, uint32_t pos, uint32_t* out_u16) {
  if (!out_u16) return 0;
  if (pos > st->len || st->len - pos < 4) return 0;
  int h0 = rt_json_jcs_hex_val(st->buf[pos + 0]);
  int h1 = rt_json_jcs_hex_val(st->buf[pos + 1]);
  int h2 = rt_json_jcs_hex_val(st->buf[pos + 2]);
  int h3 = rt_json_jcs_hex_val(st->buf[pos + 3]);
  if (h0 < 0 || h1 < 0 || h2 < 0 || h3 < 0) return 0;
  *out_u16 = (uint32_t)((h0 << 12) | (h1 << 8) | (h2 << 4) | h3);
  return 1;
}

static uint32_t rt_json_jcs_is_noncharacter(uint32_t cp) {
  if (cp >= UINT32_C(0xFDD0) && cp <= UINT32_C(0xFDEF)) return 1;
  uint32_t low = cp & UINT32_C(0xFFFF);
  return (low == UINT32_C(0xFFFE) || low == UINT32_C(0xFFFF)) ? 1 : 0;
}

static uint32_t rt_json_jcs_decode_utf8(
    rt_json_jcs_state_t* st,
    uint32_t pos,
    uint32_t* out_cp,
    uint32_t* out_next
) {
  if (!out_cp || !out_next) return 0;
  if (pos >= st->len) return 0;
  const uint8_t* s = st->buf;
  uint8_t b0 = s[pos];

  if (b0 < UINT8_C(0x80)) {
    *out_cp = (uint32_t)b0;
    *out_next = pos + 1;
    return 1;
  }

  if (b0 >= UINT8_C(0xC2) && b0 <= UINT8_C(0xDF)) {
    if (st->len - pos < 2) return 0;
    uint8_t b1 = s[pos + 1];
    if ((b1 & UINT8_C(0xC0)) != UINT8_C(0x80)) return 0;
    *out_cp = ((uint32_t)(b0 & UINT8_C(0x1F)) << 6) | (uint32_t)(b1 & UINT8_C(0x3F));
    *out_next = pos + 2;
    return 1;
  }

  if (b0 >= UINT8_C(0xE0) && b0 <= UINT8_C(0xEF)) {
    if (st->len - pos < 3) return 0;
    uint8_t b1 = s[pos + 1];
    uint8_t b2 = s[pos + 2];
    if ((b1 & UINT8_C(0xC0)) != UINT8_C(0x80)) return 0;
    if ((b2 & UINT8_C(0xC0)) != UINT8_C(0x80)) return 0;
    if (b0 == UINT8_C(0xE0) && b1 < UINT8_C(0xA0)) return 0; // overlong
    if (b0 == UINT8_C(0xED) && b1 > UINT8_C(0x9F)) return 0; // surrogate
    uint32_t cp = ((uint32_t)(b0 & UINT8_C(0x0F)) << 12)
                | ((uint32_t)(b1 & UINT8_C(0x3F)) << 6)
                | (uint32_t)(b2 & UINT8_C(0x3F));
    *out_cp = cp;
    *out_next = pos + 3;
    return 1;
  }

  if (b0 >= UINT8_C(0xF0) && b0 <= UINT8_C(0xF4)) {
    if (st->len - pos < 4) return 0;
    uint8_t b1 = s[pos + 1];
    uint8_t b2 = s[pos + 2];
    uint8_t b3 = s[pos + 3];
    if ((b1 & UINT8_C(0xC0)) != UINT8_C(0x80)) return 0;
    if ((b2 & UINT8_C(0xC0)) != UINT8_C(0x80)) return 0;
    if ((b3 & UINT8_C(0xC0)) != UINT8_C(0x80)) return 0;
    if (b0 == UINT8_C(0xF0) && b1 < UINT8_C(0x90)) return 0; // overlong
    if (b0 == UINT8_C(0xF4) && b1 > UINT8_C(0x8F)) return 0; // > U+10FFFF
    uint32_t cp = ((uint32_t)(b0 & UINT8_C(0x07)) << 18)
                | ((uint32_t)(b1 & UINT8_C(0x3F)) << 12)
                | ((uint32_t)(b2 & UINT8_C(0x3F)) << 6)
                | (uint32_t)(b3 & UINT8_C(0x3F));
    if (cp > UINT32_C(0x10FFFF)) return 0;
    *out_cp = cp;
    *out_next = pos + 4;
    return 1;
  }

  return 0;
}

static vec_u8_t rt_json_jcs_push_utf8(ctx_t* ctx, vec_u8_t out, uint32_t cp) {
  if (cp < UINT32_C(0x80)) {
    return rt_vec_u8_push(ctx, out, cp);
  }
  if (cp < UINT32_C(0x800)) {
    out = rt_vec_u8_push(ctx, out, UINT32_C(0xC0) | (cp >> 6));
    out = rt_vec_u8_push(ctx, out, UINT32_C(0x80) | (cp & UINT32_C(0x3F)));
    return out;
  }
  if (cp < UINT32_C(0x10000)) {
    out = rt_vec_u8_push(ctx, out, UINT32_C(0xE0) | (cp >> 12));
    out = rt_vec_u8_push(ctx, out, UINT32_C(0x80) | ((cp >> 6) & UINT32_C(0x3F)));
    out = rt_vec_u8_push(ctx, out, UINT32_C(0x80) | (cp & UINT32_C(0x3F)));
    return out;
  }
  out = rt_vec_u8_push(ctx, out, UINT32_C(0xF0) | (cp >> 18));
  out = rt_vec_u8_push(ctx, out, UINT32_C(0x80) | ((cp >> 12) & UINT32_C(0x3F)));
  out = rt_vec_u8_push(ctx, out, UINT32_C(0x80) | ((cp >> 6) & UINT32_C(0x3F)));
  out = rt_vec_u8_push(ctx, out, UINT32_C(0x80) | (cp & UINT32_C(0x3F)));
  return out;
}

static vec_u8_t rt_json_jcs_push_escaped(ctx_t* ctx, vec_u8_t out, uint32_t cp) {
  static const uint8_t hex[] = "0123456789abcdef";
  if (cp <= UINT32_C(0x1F)) {
    // Use the predefined JSON control escapes where available; otherwise \u00xx.
    if (cp == UINT32_C(0x08)) {
      out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
      out = rt_vec_u8_push(ctx, out, (uint32_t)'b');
      return out;
    }
    if (cp == UINT32_C(0x09)) {
      out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
      out = rt_vec_u8_push(ctx, out, (uint32_t)'t');
      return out;
    }
    if (cp == UINT32_C(0x0A)) {
      out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
      out = rt_vec_u8_push(ctx, out, (uint32_t)'n');
      return out;
    }
    if (cp == UINT32_C(0x0C)) {
      out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
      out = rt_vec_u8_push(ctx, out, (uint32_t)'f');
      return out;
    }
    if (cp == UINT32_C(0x0D)) {
      out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
      out = rt_vec_u8_push(ctx, out, (uint32_t)'r');
      return out;
    }
    out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
    out = rt_vec_u8_push(ctx, out, (uint32_t)'u');
    out = rt_vec_u8_push(ctx, out, (uint32_t)'0');
    out = rt_vec_u8_push(ctx, out, (uint32_t)'0');
    out = rt_vec_u8_push(ctx, out, (uint32_t)hex[(cp >> 4) & UINT32_C(0xF)]);
    out = rt_vec_u8_push(ctx, out, (uint32_t)hex[cp & UINT32_C(0xF)]);
    return out;
  }

  if (cp == UINT32_C(0x22)) { // "
    out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
    out = rt_vec_u8_push(ctx, out, (uint32_t)'"');
    return out;
  }
  if (cp == UINT32_C(0x5C)) { // \
    out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
    out = rt_vec_u8_push(ctx, out, (uint32_t)'\\');
    return out;
  }

  return rt_json_jcs_push_utf8(ctx, out, cp);
}

static vec_u8_t rt_json_jcs_sort_push_u16be(ctx_t* ctx, vec_u8_t out, uint32_t cu) {
  out = rt_vec_u8_push(ctx, out, (cu >> 8) & UINT32_C(0xFF));
  out = rt_vec_u8_push(ctx, out, cu & UINT32_C(0xFF));
  return out;
}

static vec_u8_t rt_json_jcs_sort_push_cp_utf16be(ctx_t* ctx, vec_u8_t out, uint32_t cp) {
  if (cp <= UINT32_C(0xFFFF)) {
    return rt_json_jcs_sort_push_u16be(ctx, out, cp);
  }
  uint32_t x = cp - UINT32_C(0x10000);
  uint32_t hi = UINT32_C(0xD800) + (x >> 10);
  uint32_t lo = UINT32_C(0xDC00) + (x & UINT32_C(0x3FF));
  out = rt_json_jcs_sort_push_u16be(ctx, out, hi);
  out = rt_json_jcs_sort_push_u16be(ctx, out, lo);
  return out;
}

static uint32_t rt_json_jcs_parse_string(
    ctx_t* ctx,
    rt_json_jcs_state_t* st,
    uint32_t pos,
    uint32_t emit_quotes,
    vec_u8_t* out_escaped,
    vec_u8_t* out_sort
) {
  if (!out_escaped) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  if (pos >= st->len || st->buf[pos] != (uint8_t)'"') {
    return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  }
  pos += 1;
  if (emit_quotes) {
    *out_escaped = rt_vec_u8_push(ctx, *out_escaped, (uint32_t)'"');
  }

  for (;;) {
    if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    uint8_t c = st->buf[pos];

    if (c == (uint8_t)'"') {
      pos += 1;
      if (emit_quotes) {
        *out_escaped = rt_vec_u8_push(ctx, *out_escaped, (uint32_t)'"');
      }
      return pos;
    }

    uint32_t cp = 0;
    uint32_t next = pos;
    uint32_t cp_off = pos;

    if (c == (uint8_t)'\\') {
      uint32_t esc_off = pos;
      if (st->len - pos < 2) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
      uint8_t esc = st->buf[pos + 1];
      pos += 2;

      switch (esc) {
        case (uint8_t)'"': cp = UINT32_C(0x22); break;
        case (uint8_t)'\\': cp = UINT32_C(0x5C); break;
        case (uint8_t)'/': cp = UINT32_C(0x2F); break;
        case (uint8_t)'b': cp = UINT32_C(0x08); break;
        case (uint8_t)'f': cp = UINT32_C(0x0C); break;
        case (uint8_t)'n': cp = UINT32_C(0x0A); break;
        case (uint8_t)'r': cp = UINT32_C(0x0D); break;
        case (uint8_t)'t': cp = UINT32_C(0x09); break;
        case (uint8_t)'u': {
          uint32_t u = 0;
          if (!rt_json_jcs_read_u16_hex4(st, pos, &u)) {
            return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, esc_off);
          }
          pos += 4;

          // Reject lone surrogates; accept valid surrogate pairs.
          if (u >= UINT32_C(0xD800) && u <= UINT32_C(0xDBFF)) {
            if (st->len - pos < 6) {
              return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, esc_off);
            }
            if (st->buf[pos] != (uint8_t)'\\' || st->buf[pos + 1] != (uint8_t)'u') {
              return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, esc_off);
            }
            uint32_t u2 = 0;
            if (!rt_json_jcs_read_u16_hex4(st, pos + 2, &u2)) {
              return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, esc_off);
            }
            if (u2 < UINT32_C(0xDC00) || u2 > UINT32_C(0xDFFF)) {
              return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, esc_off);
            }
            pos += 6;
            uint32_t hi = u - UINT32_C(0xD800);
            uint32_t lo = u2 - UINT32_C(0xDC00);
            cp = UINT32_C(0x10000) + ((hi << 10) | lo);
          } else if (u >= UINT32_C(0xDC00) && u <= UINT32_C(0xDFFF)) {
            return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, esc_off);
          } else {
            cp = u;
          }
          break;
        }
        default:
          return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, esc_off);
      }

      if (rt_json_jcs_is_noncharacter(cp)) {
        return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, esc_off);
      }

      cp_off = esc_off;
      next = pos;
    } else if (c < UINT8_C(0x20)) {
      return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    } else if (c < UINT8_C(0x80)) {
      cp = (uint32_t)c;
      next = pos + 1;
      if (rt_json_jcs_is_noncharacter(cp)) {
        return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, pos);
      }
    } else {
      if (!rt_json_jcs_decode_utf8(st, pos, &cp, &next)) {
        return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, pos);
      }
      if (rt_json_jcs_is_noncharacter(cp)) {
        return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, pos);
      }
    }

    // Sorting uses UTF-16 code units of the unescaped string.
    if (out_sort) {
      *out_sort = rt_json_jcs_sort_push_cp_utf16be(ctx, *out_sort, cp);
    }
    *out_escaped = rt_json_jcs_push_escaped(ctx, *out_escaped, cp);
    (void)cp_off;
    pos = next;
  }
}

static uint32_t rt_json_jcs_read_u32_le_at(const uint8_t* p) {
  return (uint32_t)p[0]
       | ((uint32_t)p[1] << 8)
       | ((uint32_t)p[2] << 16)
       | ((uint32_t)p[3] << 24);
}

static void rt_json_jcs_write_u32_le_at(uint8_t* p, uint32_t x) {
  p[0] = (uint8_t)(x & UINT32_C(0xFF));
  p[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  p[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  p[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
}

static void rt_json_jcs_vec_write_u32_le(vec_u8_t* v, uint32_t off, uint32_t x) {
  if (!v) rt_trap("json.jcs vec_write_u32_le null");
  if (off > v->len || v->len - off < 4) rt_trap("json.jcs vec_write_u32_le oob");
  rt_json_jcs_write_u32_le_at(v->data + off, x);
}

static vec_u8_t rt_json_jcs_vec_push_u32_le(ctx_t* ctx, vec_u8_t v, uint32_t x) {
  v = rt_vec_u8_push(ctx, v, x & UINT32_C(0xFF));
  v = rt_vec_u8_push(ctx, v, (x >> 8) & UINT32_C(0xFF));
  v = rt_vec_u8_push(ctx, v, (x >> 16) & UINT32_C(0xFF));
  v = rt_vec_u8_push(ctx, v, (x >> 24) & UINT32_C(0xFF));
  return v;
}

static uint64_t rt_json_jcs_read_u64_le(bytes_t b) {
  if (b.len != 8) rt_trap("json.jcs f64 bad len");
  uint64_t x = 0;
  x |= (uint64_t)b.ptr[0];
  x |= (uint64_t)b.ptr[1] << 8;
  x |= (uint64_t)b.ptr[2] << 16;
  x |= (uint64_t)b.ptr[3] << 24;
  x |= (uint64_t)b.ptr[4] << 32;
  x |= (uint64_t)b.ptr[5] << 40;
  x |= (uint64_t)b.ptr[6] << 48;
  x |= (uint64_t)b.ptr[7] << 56;
  return x;
}

static vec_u8_t rt_json_jcs_push_u32_dec(ctx_t* ctx, vec_u8_t out, uint32_t x) {
  uint8_t scratch[16];
  uint32_t n = 0;
  if (x == 0) {
    return rt_vec_u8_push(ctx, out, (uint32_t)'0');
  }
  while (x) {
    uint32_t d = x % 10;
    x /= 10;
    scratch[n++] = (uint8_t)('0' + d);
  }
  while (n) {
    out = rt_vec_u8_push(ctx, out, (uint32_t)scratch[n - 1]);
    n -= 1;
  }
  return out;
}

static uint32_t rt_json_jcs_emit_number(ctx_t* ctx, rt_json_jcs_state_t* st, bytes_t f, vec_u8_t* out) {
  if (!out) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, 0);
  uint64_t bits = rt_json_jcs_read_u64_le(f);
  uint32_t exp = (uint32_t)((bits >> 52) & UINT64_C(0x7FF));
  if (exp == UINT32_C(0x7FF)) {
    rt_bytes_drop(ctx, &f);
    return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, st->err_off);
  }
  if (bits == UINT64_C(0x8000000000000000)) {
    // -0 must serialize as 0.
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'0');
    rt_bytes_drop(ctx, &f);
    return 1;
  }

  bytes_t shortest = ev_math_f64_fmt_shortest_v1(f);

  // Keep `f` alive until fmt_shortest has read it, then drop.
  rt_bytes_drop(ctx, &f);

  if (shortest.len == 2 && shortest.ptr[0] == (uint8_t)'-' && shortest.ptr[1] == (uint8_t)'0') {
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'0');
    rt_bytes_drop(ctx, &shortest);
    return 1;
  }

  // Parse ryu-style shortest string and re-emit per RFC 8785 number serialization rules.
  uint32_t i = 0;
  uint32_t neg = 0;
  if (shortest.len && shortest.ptr[0] == (uint8_t)'-') {
    neg = 1;
    i = 1;
  }

  uint8_t digits[32];
  uint32_t digits_len = 0;
  uint32_t digits_before_dot = 0;
  uint32_t seen_dot = 0;

  while (i < shortest.len) {
    uint8_t c = shortest.ptr[i];
    if (c == (uint8_t)'e' || c == (uint8_t)'E') break;
    if (c == (uint8_t)'.') {
      seen_dot = 1;
      i += 1;
      continue;
    }
    if (c < (uint8_t)'0' || c > (uint8_t)'9') {
      rt_bytes_drop(ctx, &shortest);
      return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, st->err_off);
    }
    if (digits_len >= (uint32_t)(sizeof(digits) / sizeof(digits[0]))) {
      rt_bytes_drop(ctx, &shortest);
      return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, st->err_off);
    }
    digits[digits_len++] = c;
    if (!seen_dot) digits_before_dot += 1;
    i += 1;
  }

  if (digits_len == 0) {
    rt_bytes_drop(ctx, &shortest);
    return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, st->err_off);
  }

  uint32_t has_e = 0;
  int32_t exp10 = 0;
  if (i < shortest.len && (shortest.ptr[i] == (uint8_t)'e' || shortest.ptr[i] == (uint8_t)'E')) {
    has_e = 1;
    i += 1;
    int32_t sign = 1;
    if (i < shortest.len && (shortest.ptr[i] == (uint8_t)'-' || shortest.ptr[i] == (uint8_t)'+')) {
      if (shortest.ptr[i] == (uint8_t)'-') sign = -1;
      i += 1;
    }
    if (i >= shortest.len) {
      rt_bytes_drop(ctx, &shortest);
      return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, st->err_off);
    }
    int32_t e = 0;
    for (; i < shortest.len; i++) {
      uint8_t c = shortest.ptr[i];
      if (c < (uint8_t)'0' || c > (uint8_t)'9') {
        rt_bytes_drop(ctx, &shortest);
        return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, st->err_off);
      }
      e = e * 10 + (int32_t)(c - (uint8_t)'0');
      if (e > 1000000) break;
    }
    exp10 = sign * e;
  } else if (i != shortest.len) {
    rt_bytes_drop(ctx, &shortest);
    return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, st->err_off);
  }

  if (!has_e) {
    bytes_view_t v = rt_bytes_view(ctx, shortest);
    *out = rt_vec_u8_extend_bytes(ctx, *out, v);
    rt_bytes_drop(ctx, &shortest);
    return 1;
  }

  // Scientific exponent for the value with a single digit before the decimal point.
  int32_t sci_exp = exp10 + (int32_t)digits_before_dot - 1;

  uint32_t use_exp = (sci_exp < -6 || sci_exp >= 21) ? 1 : 0;

  if (neg) *out = rt_vec_u8_push(ctx, *out, (uint32_t)'-');

  if (use_exp) {
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)digits[0]);
    if (digits_len > 1) {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)'.');
      for (uint32_t di = 1; di < digits_len; di++) {
        *out = rt_vec_u8_push(ctx, *out, (uint32_t)digits[di]);
      }
    }
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'e');
    if (sci_exp >= 0) {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)'+');
      *out = rt_json_jcs_push_u32_dec(ctx, *out, (uint32_t)sci_exp);
    } else {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)'-');
      uint32_t mag = (uint32_t)(-sci_exp);
      *out = rt_json_jcs_push_u32_dec(ctx, *out, mag);
    }
    rt_bytes_drop(ctx, &shortest);
    return 1;
  }

  // Decimal form for -6 <= sci_exp < 21: shift the decimal point and avoid exponent notation.
  int32_t new_dp = (int32_t)digits_before_dot + exp10;
  if (new_dp <= 0) {
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'0');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'.');
    for (int32_t z = 0; z < -new_dp; z++) {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)'0');
    }
    for (uint32_t di = 0; di < digits_len; di++) {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)digits[di]);
    }
    rt_bytes_drop(ctx, &shortest);
    return 1;
  }

  if (new_dp >= (int32_t)digits_len) {
    for (uint32_t di = 0; di < digits_len; di++) {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)digits[di]);
    }
    for (int32_t z = 0; z < new_dp - (int32_t)digits_len; z++) {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)'0');
    }
    rt_bytes_drop(ctx, &shortest);
    return 1;
  }

  for (int32_t di = 0; di < new_dp; di++) {
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)digits[(uint32_t)di]);
  }
  *out = rt_vec_u8_push(ctx, *out, (uint32_t)'.');
  for (uint32_t di = (uint32_t)new_dp; di < digits_len; di++) {
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)digits[di]);
  }
  rt_bytes_drop(ctx, &shortest);
  return 1;
}

static uint32_t rt_json_jcs_parse_value(
    ctx_t* ctx,
    rt_json_jcs_state_t* st,
    uint32_t pos,
    uint32_t depth,
    vec_u8_t* out
);

static uint32_t rt_json_jcs_parse_array(
    ctx_t* ctx,
    rt_json_jcs_state_t* st,
    uint32_t pos,
    uint32_t depth,
    vec_u8_t* out
) {
  if (!out) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  if (pos >= st->len || st->buf[pos] != (uint8_t)'[') return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);

  *out = rt_vec_u8_push(ctx, *out, (uint32_t)'[');
  pos += 1;
  pos = rt_json_jcs_skip_ws(st, pos);
  if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  if (st->buf[pos] == (uint8_t)']') {
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)']');
    return pos + 1;
  }

  uint32_t first = 1;
  for (uint32_t _ = 0; _ < st->len + 1; _++) {
    if (!first) {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)',');
    }
    first = 0;
    pos = rt_json_jcs_parse_value(ctx, st, pos, depth, out);
    if (pos == UINT32_MAX) return UINT32_MAX;
    pos = rt_json_jcs_skip_ws(st, pos);
    if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    uint8_t c = st->buf[pos];
    if (c == (uint8_t)',') {
      pos += 1;
      pos = rt_json_jcs_skip_ws(st, pos);
      continue;
    }
    if (c == (uint8_t)']') {
      *out = rt_vec_u8_push(ctx, *out, (uint32_t)']');
      return pos + 1;
    }
    return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  }
  return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
}

static uint32_t rt_json_jcs_parse_number(
    ctx_t* ctx,
    rt_json_jcs_state_t* st,
    uint32_t pos,
    vec_u8_t* out
) {
  if (!out) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  uint32_t start = pos;
  if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  uint8_t c = st->buf[pos];
  if (c == (uint8_t)'-') {
    pos += 1;
    if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, start);
    c = st->buf[pos];
  }
  if (c == (uint8_t)'0') {
    pos += 1;
    if (pos < st->len) {
      uint8_t d = st->buf[pos];
      if (d >= (uint8_t)'0' && d <= (uint8_t)'9') {
        return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, start);
      }
    }
  } else if (c >= (uint8_t)'1' && c <= (uint8_t)'9') {
    pos += 1;
    while (pos < st->len) {
      uint8_t d = st->buf[pos];
      if (d < (uint8_t)'0' || d > (uint8_t)'9') break;
      pos += 1;
    }
  } else {
    return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, start);
  }

  if (pos < st->len && st->buf[pos] == (uint8_t)'.') {
    pos += 1;
    if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, start);
    uint8_t d = st->buf[pos];
    if (d < (uint8_t)'0' || d > (uint8_t)'9') return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, start);
    pos += 1;
    while (pos < st->len) {
      d = st->buf[pos];
      if (d < (uint8_t)'0' || d > (uint8_t)'9') break;
      pos += 1;
    }
  }

  if (pos < st->len && (st->buf[pos] == (uint8_t)'e' || st->buf[pos] == (uint8_t)'E')) {
    pos += 1;
    if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, start);
    uint8_t sgn = st->buf[pos];
    if (sgn == (uint8_t)'+' || sgn == (uint8_t)'-') {
      pos += 1;
      if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, start);
    }
    uint8_t d = st->buf[pos];
    if (d < (uint8_t)'0' || d > (uint8_t)'9') return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, start);
    pos += 1;
    while (pos < st->len) {
      d = st->buf[pos];
      if (d < (uint8_t)'0' || d > (uint8_t)'9') break;
      pos += 1;
    }
  }

  bytes_t slice = (bytes_t){ .ptr = (uint8_t*)(st->buf + start), .len = pos - start };
  result_bytes_t r = ev_math_f64_parse_v1(slice);
  if (r.tag == UINT32_C(0)) {
    return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, start);
  }

  bytes_t f = r.payload.ok;
  st->err_off = start;
  uint32_t ok = rt_json_jcs_emit_number(ctx, st, f, out);
  if (ok == UINT32_MAX) return UINT32_MAX;
  return pos;
}

static uint32_t rt_json_jcs_parse_object(
    ctx_t* ctx,
    rt_json_jcs_state_t* st,
    uint32_t pos,
    uint32_t depth,
    vec_u8_t* out
) {
  if (!out) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  if (pos >= st->len || st->buf[pos] != (uint8_t)'{') return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);

  uint32_t obj_off = pos;
  pos += 1;
  pos = rt_json_jcs_skip_ws(st, pos);
  if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  if (st->buf[pos] == (uint8_t)'}') {
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'{');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'}');
    return pos + 1;
  }

  vec_u8_t members = rt_vec_u8_new(ctx, 0);
  vec_u8_t offsets = rt_vec_u8_new(ctx, 0);
  vec_u8_t sort_tmp = rt_vec_u8_new(ctx, 0);

  uint32_t count = 0;
  uint32_t done = 0;

  for (uint32_t _ = 0; _ < st->len + 1 && !done; _++) {
    pos = rt_json_jcs_skip_ws(st, pos);
    if (pos >= st->len) { rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos); goto fail; }
    if (st->buf[pos] != (uint8_t)'"') { rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos); goto fail; }

    if (st->max_object_members && count >= st->max_object_members) {
      rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_OBJECT_TOO_LARGE, obj_off);
      goto fail;
    }

    uint32_t rec_off = members.len;
    uint32_t key_input_off = pos;
    members = rt_json_jcs_vec_push_u32_le(ctx, members, key_input_off);

    uint32_t key_len_off = members.len;
    members = rt_vec_u8_extend_zeroes(ctx, members, 4);
    uint32_t key_start = members.len;
    sort_tmp = rt_vec_u8_clear(ctx, sort_tmp);
    pos = rt_json_jcs_parse_string(ctx, st, pos, 0, &members, &sort_tmp);
    if (pos == UINT32_MAX) goto fail;
    uint32_t key_len = members.len - key_start;
    rt_json_jcs_vec_write_u32_le(&members, key_len_off, key_len);

    uint32_t sort_len_off = members.len;
    members = rt_vec_u8_extend_zeroes(ctx, members, 4);
    uint32_t sort_start = members.len;
    bytes_view_t sortv = rt_vec_u8_as_view(ctx, sort_tmp);
    members = rt_vec_u8_extend_bytes(ctx, members, sortv);
    uint32_t sort_len = members.len - sort_start;
    rt_json_jcs_vec_write_u32_le(&members, sort_len_off, sort_len);

    pos = rt_json_jcs_skip_ws(st, pos);
    if (pos >= st->len || st->buf[pos] != (uint8_t)':') { rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos); goto fail; }
    pos += 1;
    pos = rt_json_jcs_skip_ws(st, pos);

    uint32_t val_len_off = members.len;
    members = rt_vec_u8_extend_zeroes(ctx, members, 4);
    uint32_t val_start = members.len;
    pos = rt_json_jcs_parse_value(ctx, st, pos, depth, &members);
    if (pos == UINT32_MAX) goto fail;
    uint32_t val_len = members.len - val_start;
    rt_json_jcs_vec_write_u32_le(&members, val_len_off, val_len);

    offsets = rt_json_jcs_vec_push_u32_le(ctx, offsets, rec_off);
    count += 1;

    uint64_t used = (uint64_t)members.len + (uint64_t)offsets.len;
    if (used > (uint64_t)st->max_object_total_bytes) {
      rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_OBJECT_TOO_LARGE, obj_off);
      goto fail;
    }

    pos = rt_json_jcs_skip_ws(st, pos);
    if (pos >= st->len) { rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos); goto fail; }
    uint8_t sep = st->buf[pos];
    if (sep == (uint8_t)',') {
      pos += 1;
      continue;
    }
    if (sep == (uint8_t)'}') {
      pos += 1;
      done = 1;
      break;
    }
    rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    goto fail;
  }

  if (!done) {
    rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    goto fail;
  }

  if (offsets.len != count * 4) rt_trap("json.jcs offsets corrupt");
  if (count == 0) {
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'{');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'}');
    rt_vec_u8_drop(ctx, &sort_tmp);
    rt_vec_u8_drop(ctx, &offsets);
    rt_vec_u8_drop(ctx, &members);
    return pos;
  }

  // Ensure sort scratch fits the object bounds.
  uint64_t used_total = (uint64_t)members.len + (uint64_t)offsets.len + (uint64_t)count * 8ULL;
  if (used_total > (uint64_t)st->max_object_total_bytes) {
    rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_OBJECT_TOO_LARGE, obj_off);
    goto fail;
  }

  uint32_t idx_bytes = count * 4;
  uint32_t align = (uint32_t)_Alignof(uint32_t);
  uint32_t* idxs = (uint32_t*)rt_alloc(ctx, idx_bytes, align);
  uint32_t* tmp = (uint32_t*)rt_alloc(ctx, idx_bytes, align);
  for (uint32_t i = 0; i < count; i++) idxs[i] = i;

  const uint8_t* offp = offsets.data;
  const uint8_t* memb = members.data;

  // Bottom-up mergesort on indices.
  for (uint32_t width = 1; width < count; width *= 2) {
    for (uint32_t i = 0; i < count; i += 2 * width) {
      uint32_t l = i;
      uint32_t m = (i + width < count) ? (i + width) : count;
      uint32_t r = (i + 2 * width < count) ? (i + 2 * width) : count;
      uint32_t a = l;
      uint32_t b = m;
      uint32_t t = l;
      while (a < m && b < r) {
        uint32_t a_off = rt_json_jcs_read_u32_le_at(offp + idxs[a] * 4);
        uint32_t b_off = rt_json_jcs_read_u32_le_at(offp + idxs[b] * 4);

        uint32_t a_key_len = rt_json_jcs_read_u32_le_at(memb + a_off + 4);
        uint32_t b_key_len = rt_json_jcs_read_u32_le_at(memb + b_off + 4);
        uint32_t a_sort_len_off = a_off + 8 + a_key_len;
        uint32_t b_sort_len_off = b_off + 8 + b_key_len;
        uint32_t a_sort_len = rt_json_jcs_read_u32_le_at(memb + a_sort_len_off);
        uint32_t b_sort_len = rt_json_jcs_read_u32_le_at(memb + b_sort_len_off);
        const uint8_t* a_sort = memb + a_sort_len_off + 4;
        const uint8_t* b_sort = memb + b_sort_len_off + 4;

        uint32_t min_len = (a_sort_len < b_sort_len) ? a_sort_len : b_sort_len;
        int cmp = 0;
        if (min_len) cmp = memcmp(a_sort, b_sort, min_len);
        if (cmp < 0 || (cmp == 0 && a_sort_len < b_sort_len)) {
          tmp[t++] = idxs[a++];
        } else {
          tmp[t++] = idxs[b++];
        }
      }
      while (a < m) tmp[t++] = idxs[a++];
      while (b < r) tmp[t++] = idxs[b++];
    }
    memcpy(idxs, tmp, idx_bytes);
    rt_mem_on_memcpy(ctx, idx_bytes);
  }

  // Duplicate detection (I-JSON requirement: no duplicate object member names).
  for (uint32_t i = 1; i < count; i++) {
    uint32_t a = idxs[i - 1];
    uint32_t b = idxs[i];
    uint32_t a_off = rt_json_jcs_read_u32_le_at(offp + a * 4);
    uint32_t b_off = rt_json_jcs_read_u32_le_at(offp + b * 4);
    uint32_t a_key_len = rt_json_jcs_read_u32_le_at(memb + a_off + 4);
    uint32_t b_key_len = rt_json_jcs_read_u32_le_at(memb + b_off + 4);
    uint32_t a_sort_len_off = a_off + 8 + a_key_len;
    uint32_t b_sort_len_off = b_off + 8 + b_key_len;
    uint32_t a_sort_len = rt_json_jcs_read_u32_le_at(memb + a_sort_len_off);
    uint32_t b_sort_len = rt_json_jcs_read_u32_le_at(memb + b_sort_len_off);
    if (a_sort_len != b_sort_len) continue;
    const uint8_t* a_sort = memb + a_sort_len_off + 4;
    const uint8_t* b_sort = memb + b_sort_len_off + 4;
    if (a_sort_len && memcmp(a_sort, b_sort, a_sort_len) != 0) continue;
    uint32_t dup_off = rt_json_jcs_read_u32_le_at(memb + b_off);
    rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_NOT_IJSON, dup_off);
    rt_free(ctx, tmp, idx_bytes, align);
    rt_free(ctx, idxs, idx_bytes, align);
    goto fail;
  }

  bytes_view_t mview = rt_vec_u8_as_view(ctx, members);
  *out = rt_vec_u8_push(ctx, *out, (uint32_t)'{');
  for (uint32_t i = 0; i < count; i++) {
    if (i) *out = rt_vec_u8_push(ctx, *out, (uint32_t)',');
    uint32_t rec_idx = idxs[i];
    uint32_t rec_off = rt_json_jcs_read_u32_le_at(offp + rec_idx * 4);
    uint32_t key_len = rt_json_jcs_read_u32_le_at(memb + rec_off + 4);
    uint32_t key_start = rec_off + 8;
    uint32_t sort_len_off = key_start + key_len;
    uint32_t sort_len = rt_json_jcs_read_u32_le_at(memb + sort_len_off);
    uint32_t val_len_off = sort_len_off + 4 + sort_len;
    uint32_t val_len = rt_json_jcs_read_u32_le_at(memb + val_len_off);
    uint32_t val_start = val_len_off + 4;

    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'"');
    *out = rt_vec_u8_extend_bytes_range(ctx, *out, mview, key_start, key_len);
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'"');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)':');
    *out = rt_vec_u8_extend_bytes_range(ctx, *out, mview, val_start, val_len);
  }
  *out = rt_vec_u8_push(ctx, *out, (uint32_t)'}');

  rt_free(ctx, tmp, idx_bytes, align);
  rt_free(ctx, idxs, idx_bytes, align);
  rt_vec_u8_drop(ctx, &sort_tmp);
  rt_vec_u8_drop(ctx, &offsets);
  rt_vec_u8_drop(ctx, &members);
  return pos;

fail:
  rt_vec_u8_drop(ctx, &sort_tmp);
  rt_vec_u8_drop(ctx, &offsets);
  rt_vec_u8_drop(ctx, &members);
  return UINT32_MAX;
}

static uint32_t rt_json_jcs_parse_value(
    ctx_t* ctx,
    rt_json_jcs_state_t* st,
    uint32_t pos,
    uint32_t depth,
    vec_u8_t* out
) {
  if (!out) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  pos = rt_json_jcs_skip_ws(st, pos);
  if (pos >= st->len) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
  uint8_t c = st->buf[pos];

  if (c == (uint8_t)'{') {
    if (depth >= st->max_depth) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_TOO_DEEP, pos);
    return rt_json_jcs_parse_object(ctx, st, pos, depth + 1, out);
  }
  if (c == (uint8_t)'[') {
    if (depth >= st->max_depth) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_TOO_DEEP, pos);
    return rt_json_jcs_parse_array(ctx, st, pos, depth + 1, out);
  }
  if (c == (uint8_t)'"') {
    return rt_json_jcs_parse_string(ctx, st, pos, 1, out, NULL);
  }
  if (c == (uint8_t)'t') {
    if (st->len - pos < 4) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    if (memcmp(st->buf + pos, "true", 4) != 0) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'t');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'r');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'u');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'e');
    return pos + 4;
  }
  if (c == (uint8_t)'f') {
    if (st->len - pos < 5) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    if (memcmp(st->buf + pos, "false", 5) != 0) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'f');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'a');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'l');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'s');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'e');
    return pos + 5;
  }
  if (c == (uint8_t)'n') {
    if (st->len - pos < 4) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    if (memcmp(st->buf + pos, "null", 4) != 0) return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'n');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'u');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'l');
    *out = rt_vec_u8_push(ctx, *out, (uint32_t)'l');
    return pos + 4;
  }
  if (c == (uint8_t)'-' || (c >= (uint8_t)'0' && c <= (uint8_t)'9')) {
    return rt_json_jcs_parse_number(ctx, st, pos, out);
  }
  return rt_json_jcs_fail(st, RT_JSON_JCS_E_JSON_SYNTAX, pos);
}

static bytes_t rt_json_jcs_canon_doc_v1(
    ctx_t* ctx,
    bytes_view_t input,
    uint32_t max_depth,
    uint32_t max_object_members,
    uint32_t max_object_total_bytes
) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, input.bid, input.off_bytes, input.len);
#endif

  rt_json_jcs_state_t st;
  st.buf = input.ptr;
  st.len = input.len;
  st.max_depth = max_depth ? max_depth : UINT32_C(1);
  st.max_object_members = max_object_members;
  st.max_object_total_bytes = max_object_total_bytes;
  st.err_code = 0;
  st.err_off = 0;

  vec_u8_t out = rt_vec_u8_new(ctx, (input.len < UINT32_MAX) ? (input.len + 1) : input.len);
  out = rt_vec_u8_push(ctx, out, UINT32_C(1)); // tag=ok

  uint32_t pos = rt_json_jcs_skip_ws(&st, 0);
  if (pos >= st.len) {
    rt_vec_u8_drop(ctx, &out);
    return rt_json_jcs_err_doc(ctx, RT_JSON_JCS_E_JSON_SYNTAX, 0);
  }
  pos = rt_json_jcs_parse_value(ctx, &st, pos, 0, &out);
  if (pos == UINT32_MAX) {
    uint32_t code = st.err_code ? st.err_code : RT_JSON_JCS_E_JSON_SYNTAX;
    uint32_t off = st.err_off;
    rt_vec_u8_drop(ctx, &out);
    return rt_json_jcs_err_doc(ctx, code, off);
  }
  pos = rt_json_jcs_skip_ws(&st, pos);
  if (pos != st.len) {
    rt_vec_u8_drop(ctx, &out);
    return rt_json_jcs_err_doc(ctx, RT_JSON_JCS_E_JSON_TRAILING_DATA, pos);
  }

  return rt_vec_u8_into_bytes(ctx, &out);
}

// --- X07_JSON_JCS_END

// --- X07_STREAM_XF_PLUGIN_START
//
// Stream transducer plugin runtime wrapper.
// Used by compiler-internal builtins:
// - __internal.bytes.alloc_aligned_v1
// - __internal.stream_xf.plugin_init_v1
// - __internal.stream_xf.plugin_step_v1
// - __internal.stream_xf.plugin_flush_v1
//
// Output blob format (bytes):
// - u32_le count
// - repeated count times:
//   - u32_le len
//   - len bytes payload
//
// Notes:
// - Plugin callback return codes are normalized into non-negative stream error codes:
//   - rc == 0 => ok
//   - rc < 0 => err_code = -rc
//   - rc > 0 => err_code = rc
//
// - Budget scope violations use RT_ERR_BUDGET_* codes (high bit set); those are handled by
//   the surrounding `budget.scope_from_arch_v1` wrapper.
//
#define RT_XF_ABI_TAG_X7XF UINT32_C(0x46584637) // 'X7XF'
#define RT_XF_ABI_VERSION_1 UINT32_C(1)

#define RT_XF_E_CFG_TOO_LARGE UINT32_C(110)
#define RT_XF_E_CFG_NON_CANON UINT32_C(111)
#define RT_XF_E_OUT_INVALID UINT32_C(112)
#define RT_XF_E_EMIT_BUF_TOO_LARGE UINT32_C(113)
#define RT_XF_E_EMIT_STEP_BYTES_EXCEEDED UINT32_C(114)
#define RT_XF_E_EMIT_STEP_ITEMS_EXCEEDED UINT32_C(115)
#define RT_XF_E_EMIT_LEN_GT_CAP UINT32_C(116)
#define RT_XF_E_PLUGIN_INVALID UINT32_C(117)
#define RT_XF_E_VIEW_NOT_ALLOWED UINT32_C(118)

typedef bytes_t ev_bytes;

typedef struct {
  const uint8_t* ptr;
  uint32_t len;
} x07_bytes_view_v1;

typedef struct {
  uint8_t* ptr;
  uint32_t cap;
  uint32_t len;
} x07_out_buf_v1;

typedef struct {
  uint8_t* ptr;
  uint32_t cap;
  uint32_t used;
} x07_scratch_v1;

typedef struct {
  uint32_t max_out_bytes_per_step;
  uint32_t max_out_items_per_step;
  uint32_t max_out_buf_bytes;
  uint32_t max_state_bytes;
  uint32_t max_cfg_bytes;
  uint32_t max_scratch_bytes;
} x07_xf_budget_v1;

typedef struct x07_xf_emit_v1 {
  void* emit_ctx;
  int32_t (*emit_alloc)(void* emit_ctx, uint32_t cap, x07_out_buf_v1* out);
  int32_t (*emit_commit)(void* emit_ctx, const x07_out_buf_v1* out);
  int32_t (*emit_view)(void* emit_ctx, const uint8_t* ptr, uint32_t len, uint32_t view_kind);
} x07_xf_emit_v1;

typedef struct {
  uint32_t abi_tag;
  uint32_t abi_version;
  const uint8_t* plugin_id;
  uint32_t flags;
  const uint8_t* in_item_brand;
  const uint8_t* out_item_brand;
  uint32_t state_size;
  uint32_t state_align;
  uint32_t scratch_hint;
  uint32_t scratch_max;
  int32_t (*init)(
    void* state,
    x07_scratch_v1* scratch,
    x07_bytes_view_v1 cfg,
    x07_xf_emit_v1 emit,
    x07_xf_budget_v1 budget
  );
  int32_t (*step)(
    void* state,
    x07_scratch_v1* scratch,
    x07_bytes_view_v1 in,
    x07_xf_emit_v1 emit,
    x07_xf_budget_v1 budget
  );
  int32_t (*flush)(
    void* state,
    x07_scratch_v1* scratch,
    x07_xf_emit_v1 emit,
    x07_xf_budget_v1 budget
  );
  void (*drop)(void* state);
} x07_stream_xf_plugin_v1;

static bytes_t rt_bytes_alloc_aligned(ctx_t* ctx, uint32_t len, uint32_t align) {
  if (len == 0) return rt_bytes_empty(ctx);
  if (align == 0) align = 1;
  // Require power-of-two alignment (C ABI).
  if ((align & (align - 1)) != 0) rt_trap("bytes.alloc_aligned align must be power of two");
  bytes_t out;
  out.len = len;
  out.ptr = (uint8_t*)rt_alloc(ctx, len, align);
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_alloc_register(ctx, out.ptr, len);
#endif
  return out;
}

typedef struct {
  ctx_t* ctx;
  vec_u8_t out;
  uint32_t out_count;
  uint32_t emit_bytes;
  uint32_t emit_items;
  uint32_t max_out_bytes_per_step;
  uint32_t max_out_items_per_step;
  uint32_t max_out_buf_bytes;
  uint32_t pending_active;
  bytes_t pending;
  const uint8_t* in_ptr;
  uint32_t in_len;
  const uint8_t* scratch_ptr;
  uint32_t scratch_len;
  uint32_t allow_emit_view;
} rt_stream_xf_emit_ctx_v1;

static void rt_stream_xf_emit_ctx_init(
    ctx_t* ctx,
    rt_stream_xf_emit_ctx_v1* e,
    uint32_t max_out_bytes_per_step,
    uint32_t max_out_items_per_step,
    uint32_t max_out_buf_bytes
) {
  memset(e, 0, sizeof(*e));
  e->ctx = ctx;
  e->max_out_bytes_per_step = max_out_bytes_per_step;
  e->max_out_items_per_step = max_out_items_per_step;
  e->max_out_buf_bytes = max_out_buf_bytes;
  e->pending_active = 0;
  e->pending = rt_bytes_empty(ctx);

  // Reserve output header (u32 count).
  e->out = rt_vec_u8_new(ctx, 4);
  e->out = rt_vec_u8_extend_zeroes(ctx, e->out, 4);
}

static void rt_stream_xf_emit_ctx_drop(ctx_t* ctx, rt_stream_xf_emit_ctx_v1* e) {
  if (!e) return;
  if (e->pending_active) {
    rt_bytes_drop(ctx, &e->pending);
    e->pending_active = 0;
  }
  rt_vec_u8_drop(ctx, &e->out);
}

static int32_t rt_stream_xf_emit_alloc_v1(void* emit_ctx, uint32_t cap, x07_out_buf_v1* out) {
  if (!emit_ctx || !out) return (int32_t)RT_XF_E_OUT_INVALID;
  rt_stream_xf_emit_ctx_v1* e = (rt_stream_xf_emit_ctx_v1*)emit_ctx;
  if (!e->ctx) return (int32_t)RT_XF_E_OUT_INVALID;
  if (e->pending_active) return (int32_t)RT_XF_E_OUT_INVALID;

  if (e->max_out_buf_bytes != 0 && cap > e->max_out_buf_bytes) {
    return (int32_t)RT_XF_E_EMIT_BUF_TOO_LARGE;
  }

  if (e->max_out_items_per_step != 0) {
    uint32_t next_items = e->emit_items + 1;
    if (next_items < e->emit_items || next_items > e->max_out_items_per_step) {
      return (int32_t)RT_XF_E_EMIT_STEP_ITEMS_EXCEEDED;
    }
  }

  if (e->max_out_bytes_per_step != 0) {
    if (cap > UINT32_MAX - e->emit_bytes) {
      return (int32_t)RT_XF_E_EMIT_STEP_BYTES_EXCEEDED;
    }
    uint32_t next_bytes = e->emit_bytes + cap;
    if (next_bytes > e->max_out_bytes_per_step) {
      return (int32_t)RT_XF_E_EMIT_STEP_BYTES_EXCEEDED;
    }
  }

  bytes_t b = rt_bytes_alloc(e->ctx, cap);
  e->pending = b;
  e->pending_active = 1;

  out->ptr = b.ptr;
  out->cap = cap;
  out->len = 0;

  e->emit_items += 1;
  e->emit_bytes += cap;
  return 0;
}

static int32_t rt_stream_xf_emit_commit_v1(void* emit_ctx, const x07_out_buf_v1* out) {
  if (!emit_ctx || !out) return (int32_t)RT_XF_E_OUT_INVALID;
  rt_stream_xf_emit_ctx_v1* e = (rt_stream_xf_emit_ctx_v1*)emit_ctx;
  if (!e->ctx) return (int32_t)RT_XF_E_OUT_INVALID;
  if (!e->pending_active) return (int32_t)RT_XF_E_OUT_INVALID;
  if (out->ptr != e->pending.ptr || out->cap != e->pending.len) return (int32_t)RT_XF_E_OUT_INVALID;
  if (out->len > out->cap) return (int32_t)RT_XF_E_EMIT_LEN_GT_CAP;
  if (out->len > (uint32_t)INT32_MAX) return (int32_t)RT_XF_E_OUT_INVALID;

  // Append: u32 tag(0=inline), u32 len, then payload bytes.
  uint32_t hdr_off = e->out.len;
  e->out = rt_vec_u8_extend_zeroes(e->ctx, e->out, 8);
  rt_write_u32_le(e->out.data + hdr_off, 0);
  rt_write_u32_le(e->out.data + hdr_off + 4, out->len);

  if (out->len != 0) {
    uint32_t pos = e->out.len;
    e->out = rt_vec_u8_extend_zeroes(e->ctx, e->out, out->len);
    memcpy(e->out.data + pos, out->ptr, out->len);
    rt_mem_on_memcpy(e->ctx, out->len);
  }

  e->out_count += 1;
  if (e->out_count > (uint32_t)INT32_MAX) return (int32_t)RT_XF_E_OUT_INVALID;

  rt_bytes_drop(e->ctx, &e->pending);
  e->pending_active = 0;
  return 0;
}

static int32_t rt_stream_xf_emit_view_v1(void* emit_ctx, const uint8_t* ptr, uint32_t len, uint32_t view_kind) {
  if (!emit_ctx) return (int32_t)RT_XF_E_OUT_INVALID;
  rt_stream_xf_emit_ctx_v1* e = (rt_stream_xf_emit_ctx_v1*)emit_ctx;
  if (!e->ctx) return (int32_t)RT_XF_E_OUT_INVALID;
  if (e->pending_active) return (int32_t)RT_XF_E_OUT_INVALID;
  if (!e->allow_emit_view) return (int32_t)RT_XF_E_VIEW_NOT_ALLOWED;

  if (view_kind != 1 && view_kind != 2) return (int32_t)RT_XF_E_OUT_INVALID;

  if (e->max_out_buf_bytes != 0 && len > e->max_out_buf_bytes) {
    return (int32_t)RT_XF_E_EMIT_BUF_TOO_LARGE;
  }

  if (e->max_out_items_per_step != 0) {
    uint32_t next_items = e->emit_items + 1;
    if (next_items < e->emit_items || next_items > e->max_out_items_per_step) {
      return (int32_t)RT_XF_E_EMIT_STEP_ITEMS_EXCEEDED;
    }
  }

  if (e->max_out_bytes_per_step != 0) {
    if (len > UINT32_MAX - e->emit_bytes) {
      return (int32_t)RT_XF_E_EMIT_STEP_BYTES_EXCEEDED;
    }
    uint32_t next_bytes = e->emit_bytes + len;
    if (next_bytes > e->max_out_bytes_per_step) {
      return (int32_t)RT_XF_E_EMIT_STEP_BYTES_EXCEEDED;
    }
  }

  const uint8_t* base = NULL;
  uint32_t base_len = 0;
  if (view_kind == 1) {
    if (!e->in_ptr && e->in_len == 0) return (int32_t)RT_XF_E_VIEW_NOT_ALLOWED;
    base = e->in_ptr;
    base_len = e->in_len;
  } else {
    base = e->scratch_ptr;
    base_len = e->scratch_len;
  }

  uint32_t off = 0;
  if (len != 0) {
    if (!base || base_len == 0) return (int32_t)RT_XF_E_VIEW_NOT_ALLOWED;

    uintptr_t bp = (uintptr_t)base;
    uintptr_t ep = bp + (uintptr_t)base_len;
    uintptr_t p = (uintptr_t)ptr;
    uintptr_t pe = p + (uintptr_t)len;
    if (p < bp || p > ep) return (int32_t)RT_XF_E_OUT_INVALID;
    if (pe < p || pe > ep) return (int32_t)RT_XF_E_OUT_INVALID;
    uintptr_t d = p - bp;
    if (d > UINT32_MAX) return (int32_t)RT_XF_E_OUT_INVALID;
    off = (uint32_t)d;
  }

  uint32_t hdr_off = e->out.len;
  e->out = rt_vec_u8_extend_zeroes(e->ctx, e->out, 12);
  rt_write_u32_le(e->out.data + hdr_off, view_kind);
  rt_write_u32_le(e->out.data + hdr_off + 4, off);
  rt_write_u32_le(e->out.data + hdr_off + 8, len);

  e->out_count += 1;
  if (e->out_count > (uint32_t)INT32_MAX) return (int32_t)RT_XF_E_OUT_INVALID;

  e->emit_items += 1;
  e->emit_bytes += len;
  return 0;
}

static uint32_t rt_stream_xf_norm_err_code(int32_t rc) {
  if (rc == 0) return 0;
  if (rc == INT32_MIN) return RT_XF_E_PLUGIN_INVALID;
  if (rc < 0) return (uint32_t)(-rc);
  return (uint32_t)rc;
}

static uint32_t rt_stream_xf_is_pow2_u32(uint32_t x) {
  return (x != 0 && (x & (x - 1)) == 0) ? 1 : 0;
}

static uint32_t rt_stream_xf_validate_plugin(
    const x07_stream_xf_plugin_v1* p,
    uint32_t abi_major,
    bytes_t state_b,
    bytes_t scratch_b
) {
  if (!p) return RT_XF_E_PLUGIN_INVALID;
  if (p->abi_tag != RT_XF_ABI_TAG_X7XF) return RT_XF_E_PLUGIN_INVALID;
  if (p->abi_version != RT_XF_ABI_VERSION_1) return RT_XF_E_PLUGIN_INVALID;
  if (abi_major != p->abi_version) return RT_XF_E_PLUGIN_INVALID;
  if (!p->plugin_id || p->plugin_id[0] == 0) return RT_XF_E_PLUGIN_INVALID;
  if (!p->init || !p->step || !p->flush) return RT_XF_E_PLUGIN_INVALID;
  if (p->state_align < 8 || !rt_stream_xf_is_pow2_u32(p->state_align) || p->state_align > RT_HEAP_ALIGN) {
    return RT_XF_E_PLUGIN_INVALID;
  }
  if (p->state_size != state_b.len) return RT_XF_E_PLUGIN_INVALID;
  if (p->scratch_max != scratch_b.len) return RT_XF_E_PLUGIN_INVALID;
  if (state_b.len != 0) {
    uintptr_t sp = (uintptr_t)state_b.ptr;
    if ((sp & (uintptr_t)(p->state_align - 1)) != 0) return RT_XF_E_PLUGIN_INVALID;
  }
  return 0;
}

static result_bytes_t rt_stream_xf_result_ok(bytes_t b) {
  result_bytes_t r;
  r.tag = UINT32_C(1);
  r.payload.ok = b;
  return r;
}

static result_bytes_t rt_stream_xf_result_err(uint32_t code) {
  result_bytes_t r;
  r.tag = UINT32_C(0);
  r.payload.err = code;
  return r;
}

static bytes_view_t rt_view_from_ptr(ctx_t* ctx, const uint8_t* ptr, uint32_t len) {
  bytes_view_t v;
  v.ptr = (uint8_t*)ptr;
  v.len = len;
#ifdef X07_DEBUG_BORROW
  if (len == 0) return rt_view_empty(ctx);
  uint32_t off = 0;
  uint64_t aid = rt_dbg_alloc_find(ctx, (uint8_t*)ptr, len, &off);
  v.aid = aid;
  v.off_bytes = off;
  v.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#endif
  return v;
}

// Exported for native stream xf plugins that need to canonicalize JSON documents via the runtime.
ev_bytes x07_json_jcs_canon_doc_v1(
    const uint8_t* input_ptr,
    uint32_t input_len,
    int32_t max_depth,
    int32_t max_object_members,
    int32_t max_object_total_bytes
) {
#if X07_JSON_JCS_ENABLED
  if (!rt_ext_ctx) rt_trap(NULL);
  ctx_t* ctx = rt_ext_ctx;
  uint32_t md = (max_depth > 0) ? (uint32_t)max_depth : 0;
  uint32_t mm = (max_object_members > 0) ? (uint32_t)max_object_members : 0;
  uint32_t mb = (max_object_total_bytes > 0) ? (uint32_t)max_object_total_bytes : 0;
  bytes_view_t in = rt_view_from_ptr(ctx, input_ptr, input_len);
  return rt_json_jcs_canon_doc_v1(ctx, in, md, mm, mb);
#else
  rt_trap("x07_json_jcs_canon_doc_v1 requires json.jcs");
#endif
}

static result_bytes_t rt_stream_xf_plugin_init_v1(
    ctx_t* ctx,
    const x07_stream_xf_plugin_v1* p,
    uint32_t abi_major,
    bytes_t state_b,
    bytes_t scratch_b,
    bytes_t cfg_b,
    uint32_t cfg_max_bytes,
    uint32_t canon_mode,
    uint32_t strict_cfg_canon,
    uint32_t max_out_bytes_per_step,
    uint32_t max_out_items_per_step,
    uint32_t max_out_buf_bytes
) {
  uint32_t v = rt_stream_xf_validate_plugin(p, abi_major, state_b, scratch_b);
  if (v) return rt_stream_xf_result_err(v);
  if (cfg_max_bytes != 0 && cfg_b.len > cfg_max_bytes) return rt_stream_xf_result_err(RT_XF_E_CFG_TOO_LARGE);

  if (canon_mode == 1 && strict_cfg_canon == 1) {
#if X07_JSON_JCS_ENABLED
    bytes_view_t cfg_v = rt_bytes_view(ctx, cfg_b);
    bytes_t canon = rt_json_jcs_canon_doc_v1(ctx, cfg_v, 64, 4096, cfg_max_bytes);
    if (canon.len < 1 || canon.ptr[0] != 1) {
      rt_bytes_drop(ctx, &canon);
      return rt_stream_xf_result_err(RT_XF_E_CFG_NON_CANON);
    }
    uint32_t canon_len = canon.len - 1;
    if (canon_len != cfg_b.len || (canon_len != 0 && memcmp(canon.ptr + 1, cfg_b.ptr, canon_len) != 0)) {
      rt_bytes_drop(ctx, &canon);
      return rt_stream_xf_result_err(RT_XF_E_CFG_NON_CANON);
    }
    rt_bytes_drop(ctx, &canon);
#else
    return rt_stream_xf_result_err(RT_XF_E_CFG_NON_CANON);
#endif
  }

  rt_stream_xf_emit_ctx_v1 emit_ctx;
  rt_stream_xf_emit_ctx_init(ctx, &emit_ctx, max_out_bytes_per_step, max_out_items_per_step, max_out_buf_bytes);
  emit_ctx.in_ptr = NULL;
  emit_ctx.in_len = 0;
  emit_ctx.scratch_ptr = scratch_b.ptr;
  emit_ctx.scratch_len = scratch_b.len;
  emit_ctx.allow_emit_view = 1;
  x07_xf_emit_v1 emit;
  emit.emit_ctx = (void*)&emit_ctx;
  emit.emit_alloc = rt_stream_xf_emit_alloc_v1;
  emit.emit_commit = rt_stream_xf_emit_commit_v1;
  emit.emit_view = rt_stream_xf_emit_view_v1;

  x07_scratch_v1 scratch;
  scratch.ptr = scratch_b.ptr;
  scratch.cap = scratch_b.len;
  scratch.used = 0;

  x07_bytes_view_v1 cfg;
  cfg.ptr = cfg_b.ptr;
  cfg.len = cfg_b.len;

  x07_xf_budget_v1 budget;
  budget.max_out_bytes_per_step = max_out_bytes_per_step;
  budget.max_out_items_per_step = max_out_items_per_step;
  budget.max_out_buf_bytes = max_out_buf_bytes;
  budget.max_state_bytes = state_b.len;
  budget.max_cfg_bytes = cfg_max_bytes;
  budget.max_scratch_bytes = scratch_b.len;

  int32_t rc = p->init(state_b.ptr, &scratch, cfg, emit, budget);
  if (rc != 0) {
    uint32_t err_code = rt_stream_xf_norm_err_code(rc);
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(err_code);
  }
  if (emit_ctx.pending_active) {
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }

  if (emit_ctx.out.len < 4) {
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }
  rt_write_u32_le(emit_ctx.out.data, emit_ctx.out_count);
  bytes_t out_b = rt_vec_u8_into_bytes(ctx, &emit_ctx.out);
  if (out_b.len < 4 || out_b.len > (uint32_t)INT32_MAX) {
    rt_bytes_drop(ctx, &out_b);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }
  return rt_stream_xf_result_ok(out_b);
}

static result_bytes_t rt_stream_xf_plugin_step_v1(
    ctx_t* ctx,
    const x07_stream_xf_plugin_v1* p,
    uint32_t abi_major,
    bytes_t state_b,
    bytes_t scratch_b,
    uint32_t max_out_bytes_per_step,
    uint32_t max_out_items_per_step,
    uint32_t max_out_buf_bytes,
    bytes_view_t input
) {
  uint32_t v = rt_stream_xf_validate_plugin(p, abi_major, state_b, scratch_b);
  if (v) return rt_stream_xf_result_err(v);

  rt_stream_xf_emit_ctx_v1 emit_ctx;
  rt_stream_xf_emit_ctx_init(ctx, &emit_ctx, max_out_bytes_per_step, max_out_items_per_step, max_out_buf_bytes);
  emit_ctx.in_ptr = input.ptr;
  emit_ctx.in_len = input.len;
  emit_ctx.scratch_ptr = scratch_b.ptr;
  emit_ctx.scratch_len = scratch_b.len;
  emit_ctx.allow_emit_view = 1;
  x07_xf_emit_v1 emit;
  emit.emit_ctx = (void*)&emit_ctx;
  emit.emit_alloc = rt_stream_xf_emit_alloc_v1;
  emit.emit_commit = rt_stream_xf_emit_commit_v1;
  emit.emit_view = rt_stream_xf_emit_view_v1;

  x07_scratch_v1 scratch;
  scratch.ptr = scratch_b.ptr;
  scratch.cap = scratch_b.len;
  scratch.used = 0;

  x07_bytes_view_v1 in;
  in.ptr = input.ptr;
  in.len = input.len;

  x07_xf_budget_v1 budget;
  budget.max_out_bytes_per_step = max_out_bytes_per_step;
  budget.max_out_items_per_step = max_out_items_per_step;
  budget.max_out_buf_bytes = max_out_buf_bytes;
  budget.max_state_bytes = state_b.len;
  budget.max_cfg_bytes = 0;
  budget.max_scratch_bytes = scratch_b.len;

  int32_t rc = p->step(state_b.ptr, &scratch, in, emit, budget);
  if (rc != 0) {
    uint32_t err_code = rt_stream_xf_norm_err_code(rc);
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(err_code);
  }
  if (emit_ctx.pending_active) {
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }

  if (emit_ctx.out.len < 4) {
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }
  rt_write_u32_le(emit_ctx.out.data, emit_ctx.out_count);
  bytes_t out_b = rt_vec_u8_into_bytes(ctx, &emit_ctx.out);
  if (out_b.len < 4 || out_b.len > (uint32_t)INT32_MAX) {
    rt_bytes_drop(ctx, &out_b);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }
  return rt_stream_xf_result_ok(out_b);
}

static result_bytes_t rt_stream_xf_plugin_flush_v1(
    ctx_t* ctx,
    const x07_stream_xf_plugin_v1* p,
    uint32_t abi_major,
    bytes_t state_b,
    bytes_t scratch_b,
    uint32_t max_out_bytes_per_step,
    uint32_t max_out_items_per_step,
    uint32_t max_out_buf_bytes
) {
  uint32_t v = rt_stream_xf_validate_plugin(p, abi_major, state_b, scratch_b);
  if (v) return rt_stream_xf_result_err(v);

  rt_stream_xf_emit_ctx_v1 emit_ctx;
  rt_stream_xf_emit_ctx_init(ctx, &emit_ctx, max_out_bytes_per_step, max_out_items_per_step, max_out_buf_bytes);
  emit_ctx.in_ptr = NULL;
  emit_ctx.in_len = 0;
  emit_ctx.scratch_ptr = scratch_b.ptr;
  emit_ctx.scratch_len = scratch_b.len;
  emit_ctx.allow_emit_view = 1;
  x07_xf_emit_v1 emit;
  emit.emit_ctx = (void*)&emit_ctx;
  emit.emit_alloc = rt_stream_xf_emit_alloc_v1;
  emit.emit_commit = rt_stream_xf_emit_commit_v1;
  emit.emit_view = rt_stream_xf_emit_view_v1;

  x07_scratch_v1 scratch;
  scratch.ptr = scratch_b.ptr;
  scratch.cap = scratch_b.len;
  scratch.used = 0;

  x07_xf_budget_v1 budget;
  budget.max_out_bytes_per_step = max_out_bytes_per_step;
  budget.max_out_items_per_step = max_out_items_per_step;
  budget.max_out_buf_bytes = max_out_buf_bytes;
  budget.max_state_bytes = state_b.len;
  budget.max_cfg_bytes = 0;
  budget.max_scratch_bytes = scratch_b.len;

  int32_t rc = p->flush(state_b.ptr, &scratch, emit, budget);
  if (rc != 0) {
    uint32_t err_code = rt_stream_xf_norm_err_code(rc);
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(err_code);
  }
  if (emit_ctx.pending_active) {
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }

  if (emit_ctx.out.len < 4) {
    rt_stream_xf_emit_ctx_drop(ctx, &emit_ctx);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }
  rt_write_u32_le(emit_ctx.out.data, emit_ctx.out_count);
  bytes_t out_b = rt_vec_u8_into_bytes(ctx, &emit_ctx.out);
  if (out_b.len < 4 || out_b.len > (uint32_t)INT32_MAX) {
    rt_bytes_drop(ctx, &out_b);
    return rt_stream_xf_result_err(RT_XF_E_OUT_INVALID);
  }
  return rt_stream_xf_result_ok(out_b);
}

// --- X07_STREAM_XF_PLUGIN_END

struct rt_scratch_u8_fixed_s {
  uint32_t alive;
  vec_u8_t buf;
};

static void rt_scratch_u8_fixed_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->scratch_u8_fixed_cap) return;
  rt_scratch_u8_fixed_t* old_items = ctx->scratch_u8_fixed;
  uint32_t old_cap = ctx->scratch_u8_fixed_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_scratch_u8_fixed_t);
  uint32_t new_cap = ctx->scratch_u8_fixed_cap ? ctx->scratch_u8_fixed_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_scratch_u8_fixed_t* items = (rt_scratch_u8_fixed_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_scratch_u8_fixed_t),
    (uint32_t)_Alignof(rt_scratch_u8_fixed_t)
  );
  if (old_items && ctx->scratch_u8_fixed_len) {
    uint32_t bytes = ctx->scratch_u8_fixed_len * (uint32_t)sizeof(rt_scratch_u8_fixed_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_scratch_u8_fixed_t));
  }
  ctx->scratch_u8_fixed = items;
  ctx->scratch_u8_fixed_cap = new_cap;
}

static rt_scratch_u8_fixed_t* rt_scratch_u8_fixed_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->scratch_u8_fixed_len) rt_trap("scratch_u8_fixed invalid handle");
  rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[handle - 1];
  if (!s->alive) rt_trap("scratch_u8_fixed invalid handle");
  return s;
}

static uint32_t rt_scratch_u8_fixed_new(ctx_t* ctx, uint32_t cap) {
  // Reuse a free slot if possible.
  for (uint32_t i = 0; i < ctx->scratch_u8_fixed_len; i++) {
    rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[i];
    if (s->alive) continue;
    s->alive = 1;
    s->buf = rt_vec_u8_new(ctx, cap);
    return i + 1;
  }

  if (ctx->scratch_u8_fixed_len == UINT32_MAX) rt_trap("scratch_u8_fixed.new overflow");
  uint32_t need = ctx->scratch_u8_fixed_len + 1;
  rt_scratch_u8_fixed_ensure_cap(ctx, need);
  uint32_t handle = need;
  rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[handle - 1];
  s->alive = 1;
  s->buf = rt_vec_u8_new(ctx, cap);
  ctx->scratch_u8_fixed_len = need;
  return handle;
}

static uint32_t rt_scratch_u8_fixed_clear(ctx_t* ctx, uint32_t handle) {
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  s->buf.len = 0;
  return handle;
}

static uint32_t rt_scratch_u8_fixed_len(ctx_t* ctx, uint32_t handle) {
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  return s->buf.len;
}

static uint32_t rt_scratch_u8_fixed_cap(ctx_t* ctx, uint32_t handle) {
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  return s->buf.cap;
}

static bytes_view_t rt_scratch_u8_fixed_as_view(ctx_t* ctx, uint32_t handle) {
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  return rt_vec_u8_as_view(ctx, s->buf);
}

static result_i32_t rt_scratch_u8_fixed_try_write(ctx_t* ctx, uint32_t handle, bytes_view_t b) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b.len);
#endif
  rt_scratch_u8_fixed_t* s = rt_scratch_u8_fixed_ptr(ctx, handle);
  if (b.len > UINT32_MAX - s->buf.len) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(8) };
  }
  uint32_t need = s->buf.len + b.len;
  if (need > s->buf.cap) {
    return (result_i32_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(8) };
  }
  if (b.len) {
    memcpy(s->buf.data + s->buf.len, b.ptr, b.len);
    rt_mem_on_memcpy(ctx, b.len);
  }
  s->buf.len = need;
  return (result_i32_t){ .tag = UINT32_C(1), .payload.ok = b.len };
}

static uint32_t rt_scratch_u8_fixed_drop(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->scratch_u8_fixed_len) return UINT32_C(0);
  rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[handle - 1];
  if (!s->alive) return UINT32_C(0);
  rt_vec_u8_drop(ctx, &s->buf);
  s->alive = 0;
  return UINT32_C(1);
}

typedef struct {
  uint32_t cap;
  uint32_t len;
  uint32_t* keys;
  uint32_t* vals;
} map_u32_t;

static map_u32_t* rt_map_u32_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->map_u32_len) rt_trap("map_u32 invalid handle");
  map_u32_t* m = (map_u32_t*)ctx->map_u32_items[handle - 1];
  if (!m) rt_trap("map_u32 invalid handle");
  return m;
}

static uint32_t rt_is_pow2_u32(uint32_t x) {
  return (x != 0) && ((x & (x - 1)) == 0);
}

static uint32_t rt_map_u32_new(ctx_t* ctx, uint32_t cap) {
  if (!rt_is_pow2_u32(cap)) rt_trap("map_u32.new cap must be power of two");
  if (ctx->map_u32_len == ctx->map_u32_cap) {
    void** old_items = ctx->map_u32_items;
    uint32_t old_cap = ctx->map_u32_cap;
    uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(void*);
    uint32_t new_cap = ctx->map_u32_cap ? (ctx->map_u32_cap * 2) : 8;
    void** items = (void**)rt_alloc_realloc(
      ctx,
      old_items,
      old_bytes_total,
      new_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
    if (old_items && ctx->map_u32_len) {
      uint32_t bytes = ctx->map_u32_len * (uint32_t)sizeof(void*);
      memcpy(items, old_items, bytes);
      rt_mem_on_memcpy(ctx, bytes);
    }
    if (old_items && old_bytes_total) {
      rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(void*));
    }
    ctx->map_u32_items = items;
    ctx->map_u32_cap = new_cap;
  }
  map_u32_t* m = (map_u32_t*)rt_alloc(ctx, (uint32_t)sizeof(map_u32_t), (uint32_t)_Alignof(map_u32_t));
  m->cap = cap;
  m->len = 0;
  m->keys = (uint32_t*)rt_alloc(ctx, cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  m->vals = (uint32_t*)rt_alloc(ctx, cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  memset(m->keys, 0xFF, cap * (uint32_t)sizeof(uint32_t));
  ctx->map_u32_items[ctx->map_u32_len++] = m;
  return ctx->map_u32_len;
}

static uint32_t rt_map_u32_len(ctx_t* ctx, uint32_t handle) {
  return rt_map_u32_ptr(ctx, handle)->len;
}

static uint32_t rt_map_u32_hash(uint32_t key) {
  return key * UINT32_C(2654435769);
}

static uint32_t rt_map_u32_get(ctx_t* ctx, uint32_t handle, uint32_t key, uint32_t default_) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.get key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) return m->vals[idx];
    if (slot == UINT32_C(0xFFFFFFFF)) return default_;
    idx = (idx + 1) & mask;
    if (idx == start) return default_;
  }
}

static uint32_t rt_map_u32_set(ctx_t* ctx, uint32_t handle, uint32_t key, uint32_t val) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.set key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) {
      m->vals[idx] = val;
      return UINT32_C(0);
    }
    if (slot == UINT32_C(0xFFFFFFFF)) {
      m->keys[idx] = key;
      m->vals[idx] = val;
      m->len += 1;
      return UINT32_C(1);
    }
    idx = (idx + 1) & mask;
    if (idx == start) rt_trap("map_u32 full");
  }
}

static uint32_t rt_map_u32_contains(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.contains key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) return UINT32_C(1);
    if (slot == UINT32_C(0xFFFFFFFF)) return UINT32_C(0);
    idx = (idx + 1) & mask;
    if (idx == start) return UINT32_C(0);
  }
}

static uint32_t rt_map_u32_remove(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.remove key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) break;
    if (slot == UINT32_C(0xFFFFFFFF)) return UINT32_C(0);
    idx = (idx + 1) & mask;
    if (idx == start) return UINT32_C(0);
  }

  m->keys[idx] = UINT32_C(0xFFFFFFFF);
  m->vals[idx] = 0;
  m->len -= 1;

  uint32_t j = (idx + 1) & mask;
  while (m->keys[j] != UINT32_C(0xFFFFFFFF)) {
    uint32_t k = m->keys[j];
    uint32_t v = m->vals[j];
    m->keys[j] = UINT32_C(0xFFFFFFFF);
    m->vals[j] = 0;
    m->len -= 1;
    (void)rt_map_u32_set(ctx, handle, k, v);
    j = (j + 1) & mask;
  }

  return UINT32_C(1);
}

static inline void rt_store_u32_le(uint8_t* out, uint32_t x) {
  out[0] = (uint8_t)(x & UINT32_C(0xFF));
  out[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  out[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  out[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
}

static bytes_t rt_set_u32_dump_u32le(ctx_t* ctx, uint32_t handle) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (m->len == 0) return rt_bytes_empty(ctx);
  if (m->len > UINT32_MAX / UINT32_C(4)) rt_trap("set_u32.dump_u32le overflow");
  bytes_t out = rt_bytes_alloc(ctx, m->len * UINT32_C(4));
  uint32_t off = 0;
  for (uint32_t i = 0; i < m->cap; i++) {
    uint32_t k = m->keys[i];
    if (k == UINT32_C(0xFFFFFFFF)) continue;
    if (off > out.len - UINT32_C(4)) rt_trap("set_u32.dump_u32le len mismatch");
    rt_store_u32_le(out.ptr + off, k);
    off += UINT32_C(4);
  }
  if (off != out.len) rt_trap("set_u32.dump_u32le len mismatch");
  return out;
}

static bytes_t rt_map_u32_dump_kv_u32le_u32le(ctx_t* ctx, uint32_t handle) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (m->len == 0) return rt_bytes_empty(ctx);
  if (m->len > UINT32_MAX / UINT32_C(8)) rt_trap("map_u32.dump_kv_u32le_u32le overflow");
  bytes_t out = rt_bytes_alloc(ctx, m->len * UINT32_C(8));
  uint32_t off = 0;
  for (uint32_t i = 0; i < m->cap; i++) {
    uint32_t k = m->keys[i];
    if (k == UINT32_C(0xFFFFFFFF)) continue;
    if (off > out.len - UINT32_C(8)) rt_trap("map_u32.dump_kv_u32le_u32le len mismatch");
    rt_store_u32_le(out.ptr + off, k);
    rt_store_u32_le(out.ptr + off + UINT32_C(4), m->vals[i]);
    off += UINT32_C(8);
  }
  if (off != out.len) rt_trap("map_u32.dump_kv_u32le_u32le len mismatch");
  return out;
}

static uint32_t rt_hash_mix32(uint32_t h) {
  h ^= h >> 16;
  h *= UINT32_C(2246822507);
  h ^= h >> 13;
  h *= UINT32_C(3266489909);
  h ^= h >> 16;
  return h;
}

static uint32_t rt_hash_fnv1a32_view(ctx_t* ctx, bytes_view_t v) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, v.bid, v.off_bytes, v.len);
#else
  (void)ctx;
#endif
  uint32_t h = UINT32_C(2166136261);
  for (uint32_t i = 0; i < v.len; i++) {
    h ^= (uint32_t)v.ptr[i];
    h *= UINT32_C(16777619);
  }
  return h;
}

static int32_t rt_cmp_u32_to_i32(uint32_t x07_cmp) {
  if (x07_cmp == UINT32_MAX) return -1;
  if (x07_cmp == UINT32_C(1)) return 1;
  return 0;
}

typedef struct {
  uint32_t size;
  uint32_t align;
  void (*drop_in_place)(ctx_t* ctx, void* p);
  void (*clone_into)(ctx_t* ctx, void* dst, const void* src);
  uint32_t (*eq)(ctx_t* ctx, const void* a, const void* b);
  uint32_t (*hash32)(ctx_t* ctx, const void* a);
  int32_t (*cmp)(ctx_t* ctx, const void* a, const void* b);
} rt_tyops_v1;

static void rt_tyops_drop_nop(ctx_t* ctx, void* p) {
  (void)ctx;
  (void)p;
}

static void rt_tyops_clone_u32_into(ctx_t* ctx, void* dst, const void* src) {
  (void)ctx;
  *(uint32_t*)dst = *(const uint32_t*)src;
}

static uint32_t rt_tyops_eq_u32(ctx_t* ctx, const void* a, const void* b) {
  (void)ctx;
  return (*(const uint32_t*)a == *(const uint32_t*)b) ? UINT32_C(1) : UINT32_C(0);
}

static uint32_t rt_tyops_hash32_u32(ctx_t* ctx, const void* a) {
  (void)ctx;
  return rt_hash_mix32(*(const uint32_t*)a);
}

static int32_t rt_tyops_cmp_i32(ctx_t* ctx, const void* a, const void* b) {
  (void)ctx;
  int32_t aa = (int32_t)(*(const uint32_t*)a);
  int32_t bb = (int32_t)(*(const uint32_t*)b);
  if (aa < bb) return -1;
  if (aa > bb) return 1;
  return 0;
}

static int32_t rt_tyops_cmp_u32(ctx_t* ctx, const void* a, const void* b) {
  (void)ctx;
  uint32_t aa = *(const uint32_t*)a;
  uint32_t bb = *(const uint32_t*)b;
  if (aa < bb) return -1;
  if (aa > bb) return 1;
  return 0;
}

static void rt_tyops_drop_bytes(ctx_t* ctx, void* p) {
  rt_bytes_drop(ctx, (bytes_t*)p);
}

static void rt_tyops_clone_bytes_into(ctx_t* ctx, void* dst, const void* src) {
  bytes_t b = *(const bytes_t*)src;
  *(bytes_t*)dst = rt_bytes_clone(ctx, b);
}

static uint32_t rt_tyops_eq_bytes(ctx_t* ctx, const void* a, const void* b) {
  bytes_t ba = *(const bytes_t*)a;
  bytes_t bb = *(const bytes_t*)b;
  return rt_view_eq(ctx, rt_bytes_view(ctx, ba), rt_bytes_view(ctx, bb));
}

static uint32_t rt_tyops_hash32_bytes(ctx_t* ctx, const void* a) {
  bytes_t b = *(const bytes_t*)a;
  bytes_view_t v = rt_bytes_view(ctx, b);
  return rt_hash_mix32(rt_hash_fnv1a32_view(ctx, v));
}

static int32_t rt_tyops_cmp_bytes(ctx_t* ctx, const void* a, const void* b) {
  bytes_t ba = *(const bytes_t*)a;
  bytes_t bb = *(const bytes_t*)b;
  uint32_t r = rt_bytes_cmp_range(ctx, ba, 0, ba.len, bb, 0, bb.len);
  return rt_cmp_u32_to_i32(r);
}

static void rt_tyops_clone_bytes_view_into(ctx_t* ctx, void* dst, const void* src) {
  (void)ctx;
  *(bytes_view_t*)dst = *(const bytes_view_t*)src;
}

static uint32_t rt_tyops_eq_bytes_view(ctx_t* ctx, const void* a, const void* b) {
  bytes_view_t va = *(const bytes_view_t*)a;
  bytes_view_t vb = *(const bytes_view_t*)b;
  return rt_view_eq(ctx, va, vb);
}

static uint32_t rt_tyops_hash32_bytes_view(ctx_t* ctx, const void* a) {
  bytes_view_t v = *(const bytes_view_t*)a;
  return rt_hash_mix32(rt_hash_fnv1a32_view(ctx, v));
}

static int32_t rt_tyops_cmp_bytes_view(ctx_t* ctx, const void* a, const void* b) {
  bytes_view_t va = *(const bytes_view_t*)a;
  bytes_view_t vb = *(const bytes_view_t*)b;
  uint32_t r = rt_view_cmp_range(ctx, va, 0, va.len, vb, 0, vb.len);
  return rt_cmp_u32_to_i32(r);
}

static const rt_tyops_v1 RT_TYOPS_V1[5] = {
  {0},
  {
    .size = UINT32_C(4),
    .align = (uint32_t)_Alignof(uint32_t),
    .drop_in_place = rt_tyops_drop_nop,
    .clone_into = rt_tyops_clone_u32_into,
    .eq = rt_tyops_eq_u32,
    .hash32 = rt_tyops_hash32_u32,
    .cmp = rt_tyops_cmp_i32,
  },
  {
    .size = UINT32_C(4),
    .align = (uint32_t)_Alignof(uint32_t),
    .drop_in_place = rt_tyops_drop_nop,
    .clone_into = rt_tyops_clone_u32_into,
    .eq = rt_tyops_eq_u32,
    .hash32 = rt_tyops_hash32_u32,
    .cmp = rt_tyops_cmp_u32,
  },
  {
    .size = (uint32_t)sizeof(bytes_t),
    .align = (uint32_t)_Alignof(bytes_t),
    .drop_in_place = rt_tyops_drop_bytes,
    .clone_into = rt_tyops_clone_bytes_into,
    .eq = rt_tyops_eq_bytes,
    .hash32 = rt_tyops_hash32_bytes,
    .cmp = rt_tyops_cmp_bytes,
  },
  {
    .size = (uint32_t)sizeof(bytes_view_t),
    .align = (uint32_t)_Alignof(bytes_view_t),
    .drop_in_place = rt_tyops_drop_nop,
    .clone_into = rt_tyops_clone_bytes_view_into,
    .eq = rt_tyops_eq_bytes_view,
    .hash32 = rt_tyops_hash32_bytes_view,
    .cmp = rt_tyops_cmp_bytes_view,
  },
};

static const rt_tyops_v1* rt_tyops_v1_get(ctx_t* ctx, uint32_t ty_id) {
  (void)ctx;
  if (ty_id == UINT32_C(1)
      || ty_id == UINT32_C(2)
      || ty_id == UINT32_C(3)
      || ty_id == UINT32_C(4)) {
    return &RT_TYOPS_V1[ty_id];
  }
  rt_trap("value ty_id invalid");
}

typedef struct {
  uint32_t ty_id;
  uint32_t len;
  uint32_t cap;
  const rt_tyops_v1* ops;
  uint8_t* data;
} vec_value_t;

static vec_value_t* rt_vec_value_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->vec_value_len) rt_trap("vec_value invalid handle");
  vec_value_t* v = (vec_value_t*)ctx->vec_value_items[handle - 1];
  if (!v) rt_trap("vec_value invalid handle");
  return v;
}

static void rt_vec_value_grow_exact(ctx_t* ctx, vec_value_t* v, uint32_t new_cap) {
  if (new_cap <= v->cap) return;
  uint32_t esz = v->ops->size;
  uint32_t align = v->ops->align;
  if (esz != 0 && new_cap > UINT32_MAX / esz) rt_trap("vec_value cap overflow");
  uint32_t old_bytes_total = v->cap * esz;
  uint32_t new_bytes_total = new_cap * esz;

  uint8_t* old_data = v->cap ? v->data : NULL;
  uint8_t* data = (uint8_t*)rt_alloc_realloc(ctx, old_data, old_bytes_total, new_bytes_total, align);
  if (old_data && v->len) {
    uint32_t bytes = v->len * esz;
    memcpy(data, v->data, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_data && old_bytes_total) {
    rt_free(ctx, old_data, old_bytes_total, align);
  }
  v->data = data;
  v->cap = new_cap;
}

static void rt_vec_value_reserve_exact_in_place(ctx_t* ctx, vec_value_t* v, uint32_t additional) {
  if (additional > UINT32_MAX - v->len) rt_trap("vec_value.reserve_exact overflow");
  uint32_t need = v->len + additional;
  if (need <= v->cap) return;
  rt_vec_value_grow_exact(ctx, v, need);
}

static void rt_vec_value_grow_for_push(ctx_t* ctx, vec_value_t* v, uint32_t need) {
  uint32_t new_cap = v->cap ? v->cap : 1;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_vec_value_grow_exact(ctx, v, new_cap);
}

static uint32_t rt_vec_value_push_raw(ctx_t* ctx, vec_value_t* v, const void* x) {
  if (v->len == UINT32_MAX) rt_trap("vec_value.push overflow");
  uint32_t need = v->len + 1;
  if (need > v->cap) {
    rt_vec_value_grow_for_push(ctx, v, need);
  }
  uint32_t esz = v->ops->size;
  uint8_t* dst = v->data + (v->len * esz);
  memcpy(dst, x, esz);
  rt_mem_on_memcpy(ctx, esz);
  v->len = need;
  return UINT32_C(0);
}

static uint32_t rt_vec_value_with_capacity_v1(ctx_t* ctx, uint32_t ty_id, uint32_t cap) {
  const rt_tyops_v1* ops = rt_tyops_v1_get(ctx, ty_id);

  if (ctx->vec_value_len == ctx->vec_value_cap) {
    void** old_items = ctx->vec_value_items;
    uint32_t old_cap = ctx->vec_value_cap;
    uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(void*);
    uint32_t new_cap = ctx->vec_value_cap ? (ctx->vec_value_cap * 2) : 8;
    void** items = (void**)rt_alloc_realloc(
      ctx,
      old_items,
      old_bytes_total,
      new_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
    if (old_items && ctx->vec_value_len) {
      uint32_t bytes = ctx->vec_value_len * (uint32_t)sizeof(void*);
      memcpy(items, old_items, bytes);
      rt_mem_on_memcpy(ctx, bytes);
    }
    if (old_items && old_bytes_total) {
      rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(void*));
    }
    ctx->vec_value_items = items;
    ctx->vec_value_cap = new_cap;
  }

  vec_value_t* v = (vec_value_t*)rt_alloc(
    ctx,
    (uint32_t)sizeof(vec_value_t),
    (uint32_t)_Alignof(vec_value_t)
  );
  v->ty_id = ty_id;
  v->ops = ops;
  v->len = 0;
  v->cap = cap;
  if (cap == 0) {
    v->data = ctx->heap.mem;
  } else {
    if (ops->size != 0 && cap > UINT32_MAX / ops->size) rt_trap("vec_value.with_capacity cap overflow");
    uint32_t bytes_total = cap * ops->size;
    v->data = (uint8_t*)rt_alloc(ctx, bytes_total, ops->align);
  }

  ctx->vec_value_items[ctx->vec_value_len++] = v;
  return ctx->vec_value_len;
}

static uint32_t rt_vec_value_len(ctx_t* ctx, uint32_t handle) {
  return rt_vec_value_ptr(ctx, handle)->len;
}

static uint32_t rt_vec_value_reserve_exact(ctx_t* ctx, uint32_t handle, uint32_t additional) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  rt_vec_value_reserve_exact_in_place(ctx, v, additional);
  return handle;
}

static uint32_t rt_vec_value_push_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (!(v->ty_id == UINT32_C(1) || v->ty_id == UINT32_C(2))) rt_trap("vec_value.push_i32_v1 ty mismatch");
  (void)rt_vec_value_push_raw(ctx, v, &x);
  return handle;
}

static uint32_t rt_vec_value_push_bytes_v1(ctx_t* ctx, uint32_t handle, bytes_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(3)) rt_trap("vec_value.push_bytes_v1 ty mismatch");
  (void)rt_vec_value_push_raw(ctx, v, &x);
  return handle;
}

static uint32_t rt_vec_value_push_bytes_view_v1(ctx_t* ctx, uint32_t handle, bytes_view_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(4)) rt_trap("vec_value.push_bytes_view_v1 ty mismatch");
  (void)rt_vec_value_push_raw(ctx, v, &x);
  return handle;
}

static uint32_t rt_vec_value_get_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t idx, uint32_t default_) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (!(v->ty_id == UINT32_C(1) || v->ty_id == UINT32_C(2))) rt_trap("vec_value.get_i32_v1 ty mismatch");
  if (idx >= v->len) return default_;
  uint8_t* src = v->data + (idx * v->ops->size);
  return *(uint32_t*)src;
}

static bytes_t rt_vec_value_get_bytes_v1(ctx_t* ctx, uint32_t handle, uint32_t idx, bytes_t default_) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(3)) rt_trap("vec_value.get_bytes_v1 ty mismatch");
  if (idx >= v->len) return default_;
  rt_bytes_drop(ctx, &default_);
  bytes_t out = rt_bytes_empty(ctx);
  uint8_t* src = v->data + (idx * v->ops->size);
  v->ops->clone_into(ctx, &out, src);
  return out;
}

static bytes_view_t rt_vec_value_get_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t idx,
    bytes_view_t default_
) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(4)) rt_trap("vec_value.get_bytes_view_v1 ty mismatch");
  if (idx >= v->len) return default_;
  uint8_t* src = v->data + (idx * v->ops->size);
  return *(bytes_view_t*)src;
}

static uint32_t rt_vec_value_set_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t idx, uint32_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (!(v->ty_id == UINT32_C(1) || v->ty_id == UINT32_C(2))) rt_trap("vec_value.set_i32_v1 ty mismatch");
  if (idx >= v->len) rt_trap("vec_value.set oob");
  uint8_t* dst = v->data + (idx * v->ops->size);
  *(uint32_t*)dst = x;
  return handle;
}

static uint32_t rt_vec_value_set_bytes_v1(ctx_t* ctx, uint32_t handle, uint32_t idx, bytes_t x) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(3)) rt_trap("vec_value.set_bytes_v1 ty mismatch");
  if (idx >= v->len) rt_trap("vec_value.set oob");
  uint8_t* dst = v->data + (idx * v->ops->size);
  v->ops->drop_in_place(ctx, dst);
  memcpy(dst, &x, v->ops->size);
  rt_mem_on_memcpy(ctx, v->ops->size);
  return handle;
}

static uint32_t rt_vec_value_set_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t idx,
    bytes_view_t x
) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->ty_id != UINT32_C(4)) rt_trap("vec_value.set_bytes_view_v1 ty mismatch");
  if (idx >= v->len) rt_trap("vec_value.set oob");
  uint8_t* dst = v->data + (idx * v->ops->size);
  *(bytes_view_t*)dst = x;
  return handle;
}

static uint32_t rt_vec_value_pop(ctx_t* ctx, uint32_t handle) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  if (v->len == 0) return handle;
  uint32_t idx = v->len - 1;
  uint8_t* dst = v->data + (idx * v->ops->size);
  v->ops->drop_in_place(ctx, dst);
  v->len = idx;
  return handle;
}

static uint32_t rt_vec_value_clear(ctx_t* ctx, uint32_t handle) {
  vec_value_t* v = rt_vec_value_ptr(ctx, handle);
  uint32_t esz = v->ops->size;
  for (uint32_t i = 0; i < v->len; i++) {
    v->ops->drop_in_place(ctx, v->data + (i * esz));
  }
  v->len = 0;
  return handle;
}

typedef struct {
  uint32_t cap;
  uint32_t len;
  uint32_t k_ty_id;
  uint32_t v_ty_id;
  const rt_tyops_v1* k_ops;
  const rt_tyops_v1* v_ops;

  uint8_t* ctrl;    // 0 empty, 1 filled, 2 tombstone
  uint32_t* hashes; // cached hash32(key)
  uint8_t* keys;    // raw key bytes
  uint8_t* vals;    // raw val bytes
} map_value_t;

static map_value_t* rt_map_value_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->map_value_len) rt_trap("map_value invalid handle");
  map_value_t* m = (map_value_t*)ctx->map_value_items[handle - 1];
  if (!m) rt_trap("map_value invalid handle");
  return m;
}

static void rt_map_value_alloc_arrays(ctx_t* ctx, map_value_t* m, uint32_t cap) {
  if (cap == 0) rt_trap("map_value cap=0");
  if (!rt_is_pow2_u32(cap)) rt_trap("map_value cap must be power of two");

  m->cap = cap;
  m->len = 0;

  m->ctrl = (uint8_t*)rt_alloc(ctx, cap, 1);
  memset(m->ctrl, 0, cap);

  if (cap > UINT32_MAX / (uint32_t)sizeof(uint32_t)) rt_trap("map_value hashes overflow");
  uint32_t hashes_bytes = cap * (uint32_t)sizeof(uint32_t);
  m->hashes = (uint32_t*)rt_alloc(ctx, hashes_bytes, (uint32_t)_Alignof(uint32_t));
  memset(m->hashes, 0, hashes_bytes);

  uint32_t ksz = m->k_ops->size;
  uint32_t vsz = m->v_ops->size;
  if (ksz != 0 && cap > UINT32_MAX / ksz) rt_trap("map_value keys overflow");
  if (vsz != 0 && cap > UINT32_MAX / vsz) rt_trap("map_value vals overflow");
  m->keys = (uint8_t*)rt_alloc(ctx, cap * ksz, m->k_ops->align);
  m->vals = (uint8_t*)rt_alloc(ctx, cap * vsz, m->v_ops->align);
}

static void rt_map_value_free_arrays(ctx_t* ctx, map_value_t* m) {
  if (m->ctrl && m->cap) rt_free(ctx, m->ctrl, m->cap, 1);
  if (m->hashes && m->cap) {
    rt_free(
      ctx,
      m->hashes,
      m->cap * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
    );
  }
  if (m->keys && m->cap) rt_free(ctx, m->keys, m->cap * m->k_ops->size, m->k_ops->align);
  if (m->vals && m->cap) rt_free(ctx, m->vals, m->cap * m->v_ops->size, m->v_ops->align);

  m->ctrl = NULL;
  m->hashes = NULL;
  m->keys = NULL;
  m->vals = NULL;
  m->cap = 0;
  m->len = 0;
}

static uint8_t* rt_map_value_key_ptr(map_value_t* m, uint32_t idx) {
  return m->keys + (idx * m->k_ops->size);
}

static uint8_t* rt_map_value_val_ptr(map_value_t* m, uint32_t idx) {
  return m->vals + (idx * m->v_ops->size);
}

static uint32_t rt_map_value_lookup_idx(
    ctx_t* ctx,
    map_value_t* m,
    const void* key,
    uint32_t hash,
    uint32_t* out_idx
) {
  if (m->cap == 0) return 0;
  uint32_t mask = m->cap - 1;
  uint32_t idx = hash & mask;
  uint32_t start = idx;
  for (;;) {
    uint8_t c = m->ctrl[idx];
    if (c == 0) return 0;
    if (c == 1) {
      if (m->hashes[idx] == hash) {
        uint8_t* kptr = rt_map_value_key_ptr(m, idx);
        if (m->k_ops->eq(ctx, kptr, key)) {
          if (out_idx) *out_idx = idx;
          return 1;
        }
      }
    }
    idx = (idx + 1) & mask;
    if (idx == start) return 0;
  }
}

static uint32_t rt_map_value_find_slot(
    ctx_t* ctx,
    map_value_t* m,
    const void* key,
    uint32_t hash,
    uint32_t* out_idx,
    uint32_t* out_found
) {
  if (m->cap == 0) rt_trap("map_value cap=0");
  uint32_t mask = m->cap - 1;
  uint32_t idx = hash & mask;
  uint32_t start = idx;
  uint32_t first_tomb = UINT32_MAX;
  for (;;) {
    uint8_t c = m->ctrl[idx];
    if (c == 0) {
      if (out_found) *out_found = 0;
      if (out_idx) *out_idx = (first_tomb == UINT32_MAX) ? idx : first_tomb;
      return 1;
    }
    if (c == 2) {
      if (first_tomb == UINT32_MAX) first_tomb = idx;
    } else if (c == 1) {
      if (m->hashes[idx] == hash) {
        uint8_t* kptr = rt_map_value_key_ptr(m, idx);
        if (m->k_ops->eq(ctx, kptr, key)) {
          if (out_found) *out_found = 1;
          if (out_idx) *out_idx = idx;
          return 1;
        }
      }
    } else {
      rt_trap("map_value ctrl corrupt");
    }

    idx = (idx + 1) & mask;
    if (idx == start) {
      if (first_tomb != UINT32_MAX) {
        if (out_found) *out_found = 0;
        if (out_idx) *out_idx = first_tomb;
        return 1;
      }
      return 0;
    }
  }
}

static void rt_map_value_rehash(ctx_t* ctx, map_value_t* m, uint32_t new_cap) {
  if (!rt_is_pow2_u32(new_cap)) rt_trap("map_value.rehash cap must be power of two");

  map_value_t tmp = *m;
  rt_map_value_alloc_arrays(ctx, &tmp, new_cap);

  uint32_t ksz = m->k_ops->size;
  uint32_t vsz = m->v_ops->size;

  for (uint32_t i = 0; i < m->cap; i++) {
    if (m->ctrl[i] != 1) continue;
    uint32_t hash = m->hashes[i];

    uint32_t mask = tmp.cap - 1;
    uint32_t idx = hash & mask;
    uint32_t start = idx;
    for (;;) {
      if (tmp.ctrl[idx] == 0) break;
      idx = (idx + 1) & mask;
      if (idx == start) rt_trap("map_value.rehash full");
    }

    tmp.ctrl[idx] = 1;
    tmp.hashes[idx] = hash;
    memcpy(rt_map_value_key_ptr(&tmp, idx), rt_map_value_key_ptr(m, i), ksz);
    rt_mem_on_memcpy(ctx, ksz);
    memcpy(rt_map_value_val_ptr(&tmp, idx), rt_map_value_val_ptr(m, i), vsz);
    rt_mem_on_memcpy(ctx, vsz);
    tmp.len += 1;
  }

  rt_map_value_free_arrays(ctx, m);
  *m = tmp;
}

static void rt_map_value_ensure_room_for_insert(ctx_t* ctx, map_value_t* m) {
  uint64_t lhs = ((uint64_t)m->len + 1ULL) * 10ULL;
  uint64_t rhs = (uint64_t)m->cap * 7ULL;
  if (lhs <= rhs) return;

  if (m->cap > UINT32_MAX / 2) rt_trap("map_value cap overflow");
  rt_map_value_rehash(ctx, m, m->cap * 2);
}

static uint32_t rt_map_value_set_raw(
    ctx_t* ctx,
    map_value_t* m,
    const void* key,
    const void* val,
    uint32_t* out_replaced
) {
  rt_map_value_ensure_room_for_insert(ctx, m);

  uint32_t hash = m->k_ops->hash32(ctx, key);
  uint32_t idx = 0;
  uint32_t found = 0;
  if (!rt_map_value_find_slot(ctx, m, key, hash, &idx, &found)) {
    if (m->cap > UINT32_MAX / 2) rt_trap("map_value cap overflow");
    rt_map_value_rehash(ctx, m, m->cap * 2);
    if (!rt_map_value_find_slot(ctx, m, key, hash, &idx, &found)) {
      rt_trap("map_value.set no slot");
    }
  }

  uint32_t ksz = m->k_ops->size;
  uint32_t vsz = m->v_ops->size;
  uint8_t* vptr = rt_map_value_val_ptr(m, idx);

  if (found) {
    if (out_replaced) *out_replaced = 1;
    m->v_ops->drop_in_place(ctx, vptr);
    memcpy(vptr, val, vsz);
    rt_mem_on_memcpy(ctx, vsz);
    return 0;
  }

  if (out_replaced) *out_replaced = 0;
  m->ctrl[idx] = 1;
  m->hashes[idx] = hash;
  memcpy(rt_map_value_key_ptr(m, idx), key, ksz);
  rt_mem_on_memcpy(ctx, ksz);
  memcpy(vptr, val, vsz);
  rt_mem_on_memcpy(ctx, vsz);
  m->len += 1;
  return 0;
}

static uint32_t rt_map_value_remove_raw(ctx_t* ctx, map_value_t* m, const void* key) {
  uint32_t hash = m->k_ops->hash32(ctx, key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, key, hash, &idx)) return 0;

  uint8_t* kptr = rt_map_value_key_ptr(m, idx);
  uint8_t* vptr = rt_map_value_val_ptr(m, idx);
  m->k_ops->drop_in_place(ctx, kptr);
  m->v_ops->drop_in_place(ctx, vptr);
  m->ctrl[idx] = 2;
  if (m->len == 0) rt_trap("map_value.remove len underflow");
  m->len -= 1;
  return 1;
}

static uint32_t rt_map_value_clear_in_place(ctx_t* ctx, map_value_t* m) {
  for (uint32_t i = 0; i < m->cap; i++) {
    if (m->ctrl[i] != 1) continue;
    m->k_ops->drop_in_place(ctx, rt_map_value_key_ptr(m, i));
    m->v_ops->drop_in_place(ctx, rt_map_value_val_ptr(m, i));
  }
  memset(m->ctrl, 0, m->cap);
  memset(m->hashes, 0, m->cap * (uint32_t)sizeof(uint32_t));
  m->len = 0;
  return 0;
}

static uint32_t rt_map_value_new_v1(ctx_t* ctx, uint32_t k_ty_id, uint32_t v_ty_id, uint32_t cap) {
  if (!rt_is_pow2_u32(cap)) rt_trap("map_value.new cap must be power of two");
  const rt_tyops_v1* k_ops = rt_tyops_v1_get(ctx, k_ty_id);
  const rt_tyops_v1* v_ops = rt_tyops_v1_get(ctx, v_ty_id);
  if (!k_ops->eq || !k_ops->hash32) rt_trap("map_value.new key ops missing");

  if (ctx->map_value_len == ctx->map_value_cap) {
    void** old_items = ctx->map_value_items;
    uint32_t old_cap = ctx->map_value_cap;
    uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(void*);
    uint32_t new_cap = ctx->map_value_cap ? (ctx->map_value_cap * 2) : 8;
    void** items = (void**)rt_alloc_realloc(
      ctx,
      old_items,
      old_bytes_total,
      new_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
    if (old_items && ctx->map_value_len) {
      uint32_t bytes = ctx->map_value_len * (uint32_t)sizeof(void*);
      memcpy(items, old_items, bytes);
      rt_mem_on_memcpy(ctx, bytes);
    }
    if (old_items && old_bytes_total) {
      rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(void*));
    }
    ctx->map_value_items = items;
    ctx->map_value_cap = new_cap;
  }

  map_value_t* m = (map_value_t*)rt_alloc(
    ctx,
    (uint32_t)sizeof(map_value_t),
    (uint32_t)_Alignof(map_value_t)
  );
  memset(m, 0, sizeof(map_value_t));
  m->k_ty_id = k_ty_id;
  m->v_ty_id = v_ty_id;
  m->k_ops = k_ops;
  m->v_ops = v_ops;
  rt_map_value_alloc_arrays(ctx, m, cap);

  ctx->map_value_items[ctx->map_value_len++] = m;
  return ctx->map_value_len;
}

static uint32_t rt_map_value_len(ctx_t* ctx, uint32_t handle) {
  return rt_map_value_ptr(ctx, handle)->len;
}

static uint32_t rt_map_value_clear(ctx_t* ctx, uint32_t handle) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  (void)rt_map_value_clear_in_place(ctx, m);
  return handle;
}

static uint32_t rt_map_value_contains_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.contains_i32_v1 key ty mismatch");
  uint32_t hash = m->k_ops->hash32(ctx, &key);
  return rt_map_value_lookup_idx(ctx, m, &key, hash, NULL);
}

static uint32_t rt_map_value_contains_bytes_v1(ctx_t* ctx, uint32_t handle, bytes_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.contains_bytes_v1 key ty mismatch");
  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t ok = rt_map_value_lookup_idx(ctx, m, &key, hash, NULL);
  rt_bytes_drop(ctx, &key);
  return ok;
}

static uint32_t rt_map_value_contains_bytes_view_v1(ctx_t* ctx, uint32_t handle, bytes_view_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.contains_bytes_view_v1 key ty mismatch");
  uint32_t hash = m->k_ops->hash32(ctx, &key);
  return rt_map_value_lookup_idx(ctx, m, &key, hash, NULL);
}

static uint32_t rt_map_value_remove_i32_v1(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.remove_i32_v1 key ty mismatch");
  (void)rt_map_value_remove_raw(ctx, m, &key);
  return handle;
}

static uint32_t rt_map_value_remove_bytes_v1(ctx_t* ctx, uint32_t handle, bytes_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.remove_bytes_v1 key ty mismatch");
  (void)rt_map_value_remove_raw(ctx, m, &key);
  rt_bytes_drop(ctx, &key);
  return handle;
}

static uint32_t rt_map_value_remove_bytes_view_v1(ctx_t* ctx, uint32_t handle, bytes_view_t key) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.remove_bytes_view_v1 key ty mismatch");
  (void)rt_map_value_remove_raw(ctx, m, &key);
  return handle;
}

static uint32_t rt_map_value_get_i32_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    uint32_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.get key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;
  uint8_t* vptr = rt_map_value_val_ptr(m, idx);
  return *(uint32_t*)vptr;
}

static bytes_t rt_map_value_get_i32_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    bytes_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;

  rt_bytes_drop(ctx, &default_);
  bytes_t out = rt_bytes_empty(ctx);
  m->v_ops->clone_into(ctx, &out, rt_map_value_val_ptr(m, idx));
  return out;
}

static bytes_view_t rt_map_value_get_i32_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    bytes_view_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;
  return *(bytes_view_t*)rt_map_value_val_ptr(m, idx);
}

static uint32_t rt_map_value_get_bytes_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    uint32_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.get key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  uint32_t found = rt_map_value_lookup_idx(ctx, m, &key, hash, &idx);
  rt_bytes_drop(ctx, &key);
  if (!found) return default_;
  return *(uint32_t*)rt_map_value_val_ptr(m, idx);
}

static bytes_t rt_map_value_get_bytes_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    bytes_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  uint32_t found = rt_map_value_lookup_idx(ctx, m, &key, hash, &idx);
  rt_bytes_drop(ctx, &key);
  if (!found) return default_;

  rt_bytes_drop(ctx, &default_);
  bytes_t out = rt_bytes_empty(ctx);
  m->v_ops->clone_into(ctx, &out, rt_map_value_val_ptr(m, idx));
  return out;
}

static bytes_view_t rt_map_value_get_bytes_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    bytes_view_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  uint32_t found = rt_map_value_lookup_idx(ctx, m, &key, hash, &idx);
  rt_bytes_drop(ctx, &key);
  if (!found) return default_;
  return *(bytes_view_t*)rt_map_value_val_ptr(m, idx);
}

static uint32_t rt_map_value_get_bytes_view_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    uint32_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.get key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;
  return *(uint32_t*)rt_map_value_val_ptr(m, idx);
}

static bytes_t rt_map_value_get_bytes_view_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    bytes_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;

  rt_bytes_drop(ctx, &default_);
  bytes_t out = rt_bytes_empty(ctx);
  m->v_ops->clone_into(ctx, &out, rt_map_value_val_ptr(m, idx));
  return out;
}

static bytes_view_t rt_map_value_get_bytes_view_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    bytes_view_t default_
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.get key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.get val ty mismatch");

  uint32_t hash = m->k_ops->hash32(ctx, &key);
  uint32_t idx = 0;
  if (!rt_map_value_lookup_idx(ctx, m, &key, hash, &idx)) return default_;
  return *(bytes_view_t*)rt_map_value_val_ptr(m, idx);
}

static uint32_t rt_map_value_set_i32_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    uint32_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.set key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_i32_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    bytes_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_i32_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    uint32_t key,
    bytes_view_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (!(m->k_ty_id == UINT32_C(1) || m->k_ty_id == UINT32_C(2))) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_bytes_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    uint32_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.set key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.set val ty mismatch");
  uint32_t replaced = 0;
  (void)rt_map_value_set_raw(ctx, m, &key, &val, &replaced);
  if (replaced) rt_bytes_drop(ctx, &key);
  return handle;
}

static uint32_t rt_map_value_set_bytes_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    bytes_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.set val ty mismatch");
  uint32_t replaced = 0;
  (void)rt_map_value_set_raw(ctx, m, &key, &val, &replaced);
  if (replaced) rt_bytes_drop(ctx, &key);
  return handle;
}

static uint32_t rt_map_value_set_bytes_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_t key,
    bytes_view_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(3)) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.set val ty mismatch");
  uint32_t replaced = 0;
  (void)rt_map_value_set_raw(ctx, m, &key, &val, &replaced);
  if (replaced) rt_bytes_drop(ctx, &key);
  return handle;
}

static uint32_t rt_map_value_set_bytes_view_i32_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    uint32_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.set key ty mismatch");
  if (!(m->v_ty_id == UINT32_C(1) || m->v_ty_id == UINT32_C(2))) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_bytes_view_bytes_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    bytes_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(3)) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static uint32_t rt_map_value_set_bytes_view_bytes_view_v1(
    ctx_t* ctx,
    uint32_t handle,
    bytes_view_t key,
    bytes_view_t val
) {
  map_value_t* m = rt_map_value_ptr(ctx, handle);
  if (m->k_ty_id != UINT32_C(4)) rt_trap("map_value.set key ty mismatch");
  if (m->v_ty_id != UINT32_C(4)) rt_trap("map_value.set val ty mismatch");
  (void)rt_map_value_set_raw(ctx, m, &key, &val, NULL);
  return handle;
}

static void rt_ctx_cleanup(ctx_t* ctx) {
#ifdef X07_DEBUG_BORROW
  if (ctx->dbg_borrows && ctx->dbg_borrows_cap) {
    rt_free(
      ctx,
      ctx->dbg_borrows,
      ctx->dbg_borrows_cap * (uint32_t)sizeof(dbg_borrow_rec_t),
      (uint32_t)_Alignof(dbg_borrow_rec_t)
    );
  }
  if (ctx->dbg_allocs && ctx->dbg_allocs_cap) {
    rt_free(
      ctx,
      ctx->dbg_allocs,
      ctx->dbg_allocs_cap * (uint32_t)sizeof(dbg_alloc_rec_t),
      (uint32_t)_Alignof(dbg_alloc_rec_t)
    );
  }
  ctx->dbg_borrows = NULL;
  ctx->dbg_borrows_len = 0;
  ctx->dbg_borrows_cap = 0;
  ctx->dbg_allocs = NULL;
  ctx->dbg_allocs_len = 0;
  ctx->dbg_allocs_cap = 0;
#endif

  rt_os_process_cleanup(ctx);

  for (uint32_t i = 0; i < ctx->bufreads_len; i++) {
    rt_bufread_t* br = &ctx->bufreads[i];
    if (!br->alive) continue;
    rt_bytes_drop(ctx, &br->buf);
    br->buf = rt_bytes_empty(ctx);
    br->alive = 0;
  }
  if (ctx->bufreads && ctx->bufreads_cap) {
    rt_free(
      ctx,
      ctx->bufreads,
      ctx->bufreads_cap * (uint32_t)sizeof(rt_bufread_t),
      (uint32_t)_Alignof(rt_bufread_t)
    );
  }
  ctx->bufreads = NULL;
  ctx->bufreads_len = 0;
  ctx->bufreads_cap = 0;

  for (uint32_t i = 0; i < ctx->scratch_u8_fixed_len; i++) {
    rt_scratch_u8_fixed_t* s = &ctx->scratch_u8_fixed[i];
    if (!s->alive) continue;
    rt_vec_u8_drop(ctx, &s->buf);
    s->alive = 0;
  }
  if (ctx->scratch_u8_fixed && ctx->scratch_u8_fixed_cap) {
    rt_free(
      ctx,
      ctx->scratch_u8_fixed,
      ctx->scratch_u8_fixed_cap * (uint32_t)sizeof(rt_scratch_u8_fixed_t),
      (uint32_t)_Alignof(rt_scratch_u8_fixed_t)
    );
  }
  ctx->scratch_u8_fixed = NULL;
  ctx->scratch_u8_fixed_len = 0;
  ctx->scratch_u8_fixed_cap = 0;

  for (uint32_t i = 0; i < ctx->io_readers_len; i++) {
    rt_io_reader_t* r = &ctx->io_readers[i];
    if (!r->alive) continue;
#if X07_ENABLE_STREAMING_FILE_IO
    if (r->kind == RT_IO_READER_KIND_FILE && r->f) {
      fclose(r->f);
      r->f = NULL;
    }
#endif
    if (r->kind == RT_IO_READER_KIND_BYTES) {
      rt_bytes_drop(ctx, &r->bytes);
      r->bytes = rt_bytes_empty(ctx);
    }
    r->alive = 0;
  }
  if (ctx->io_readers && ctx->io_readers_cap) {
    rt_free(
      ctx,
      ctx->io_readers,
      ctx->io_readers_cap * (uint32_t)sizeof(rt_io_reader_t),
      (uint32_t)_Alignof(rt_io_reader_t)
    );
  }
  ctx->io_readers = NULL;
  ctx->io_readers_len = 0;
  ctx->io_readers_cap = 0;

  for (uint32_t i = 0; i < ctx->sched_chans_len; i++) {
    rt_chan_bytes_t* c = &ctx->sched_chans[i];
    if (!c->alive) continue;
    for (uint32_t j = 0; j < c->len; j++) {
      uint32_t idx = (c->head + j) % c->cap;
      rt_bytes_drop(ctx, &c->buf[idx]);
    }
    if (c->buf && c->cap) {
      rt_free(ctx, c->buf, c->cap * (uint32_t)sizeof(bytes_t), (uint32_t)_Alignof(bytes_t));
    }
    c->buf = NULL;
    c->alive = 0;
  }
  if (ctx->sched_chans && ctx->sched_chans_cap) {
    rt_free(
      ctx,
      ctx->sched_chans,
      ctx->sched_chans_cap * (uint32_t)sizeof(rt_chan_bytes_t),
      (uint32_t)_Alignof(rt_chan_bytes_t)
    );
  }
  ctx->sched_chans = NULL;
  ctx->sched_chans_len = 0;
  ctx->sched_chans_cap = 0;

  for (uint32_t i = 0; i < ctx->sched_select_evts_len; i++) {
    rt_select_evt_t* e = &ctx->sched_select_evts[i];
    if (!e->alive) continue;
    rt_bytes_drop(ctx, &e->payload);
    e->payload = rt_bytes_empty(ctx);
    e->alive = 0;
    e->taken = 0;
  }
  if (ctx->sched_select_evts && ctx->sched_select_evts_cap) {
    rt_free(
      ctx,
      ctx->sched_select_evts,
      ctx->sched_select_evts_cap * (uint32_t)sizeof(rt_select_evt_t),
      (uint32_t)_Alignof(rt_select_evt_t)
    );
  }
  ctx->sched_select_evts = NULL;
  ctx->sched_select_evts_len = 0;
  ctx->sched_select_evts_cap = 0;

  if (ctx->sched_timers && ctx->sched_timers_cap) {
    rt_free(
      ctx,
      ctx->sched_timers,
      ctx->sched_timers_cap * (uint32_t)sizeof(rt_timer_ev_t),
      (uint32_t)_Alignof(rt_timer_ev_t)
    );
  }
  ctx->sched_timers = NULL;
  ctx->sched_timers_len = 0;
  ctx->sched_timers_cap = 0;

  for (uint32_t i = 0; i < ctx->sched_tasks_len; i++) {
    rt_task_t* t = &ctx->sched_tasks[i];
    if (!t->alive) continue;
    if (!t->done) t->canceled = 1;
    t->done = 1;
    if (t->drop && t->fut) {
      t->drop(ctx, t->fut);
    }
    t->drop = NULL;
    t->fut = NULL;
    rt_task_out_drop(ctx, &t->out);
    t->out = rt_task_out_empty(ctx);
    t->out_taken = 0;
    t->alive = 0;
  }
  if (ctx->sched_tasks && ctx->sched_tasks_cap) {
    rt_free(
      ctx,
      ctx->sched_tasks,
      ctx->sched_tasks_cap * (uint32_t)sizeof(rt_task_t),
      (uint32_t)_Alignof(rt_task_t)
    );
  }
  ctx->sched_tasks = NULL;
  ctx->sched_tasks_len = 0;
  ctx->sched_tasks_cap = 0;
  ctx->sched_ready_head = 0;
  ctx->sched_ready_tail = 0;

  if (ctx->fs_latency_entries && ctx->fs_latency_len) {
    rt_free(
      ctx,
      ctx->fs_latency_entries,
      ctx->fs_latency_len * (uint32_t)sizeof(fs_latency_entry_t),
      (uint32_t)_Alignof(fs_latency_entry_t)
    );
  }
  ctx->fs_latency_entries = NULL;
  ctx->fs_latency_len = 0;
  rt_bytes_drop(ctx, &ctx->fs_latency_blob);
  ctx->fs_latency_blob = rt_bytes_empty(ctx);

#if X07_ENABLE_RR
  for (uint32_t i = 0; i < ctx->rr_handles_len; i++) {
    rr_handle_t* h = &ctx->rr_handles[i];
    if (!h->alive) continue;

    for (uint32_t j = 0; j < h->cassettes_len; j++) {
      rr_cassette_t* c = &h->cassettes[j];
      if (c->append_f) {
        fclose((FILE*)c->append_f);
        c->append_f = NULL;
      }
      if (c->entries && c->entries_cap) {
        rt_free(
          ctx,
          c->entries,
          c->entries_cap * (uint32_t)sizeof(rr_entry_desc_t),
          (uint32_t)_Alignof(rr_entry_desc_t)
        );
      }
      c->entries = NULL;
      c->entries_len = 0;
      c->entries_cap = 0;
      rt_bytes_drop(ctx, &c->blob);
      c->blob = rt_bytes_empty(ctx);
      rt_bytes_drop(ctx, &c->path);
      c->path = rt_bytes_empty(ctx);
      c->file_bytes = 0;
    }

    if (h->cassettes && h->cassettes_cap) {
      rt_free(
        ctx,
        h->cassettes,
        h->cassettes_cap * (uint32_t)sizeof(rr_cassette_t),
        (uint32_t)_Alignof(rr_cassette_t)
      );
    }
    h->cassettes = NULL;
    h->cassettes_len = 0;
    h->cassettes_cap = 0;
    h->alive = 0;
  }
  if (ctx->rr_handles && ctx->rr_handles_cap) {
    rt_free(
      ctx,
      ctx->rr_handles,
      ctx->rr_handles_cap * (uint32_t)sizeof(rr_handle_t),
      (uint32_t)_Alignof(rr_handle_t)
    );
  }
  ctx->rr_handles = NULL;
  ctx->rr_handles_len = 0;
  ctx->rr_handles_cap = 0;
  ctx->rr_current = 0;
#endif

  if (ctx->kv_latency_entries && ctx->kv_latency_len) {
    rt_free(
      ctx,
      ctx->kv_latency_entries,
      ctx->kv_latency_len * (uint32_t)sizeof(kv_latency_entry_t),
      (uint32_t)_Alignof(kv_latency_entry_t)
    );
  }
  ctx->kv_latency_entries = NULL;
  ctx->kv_latency_len = 0;
  rt_bytes_drop(ctx, &ctx->kv_latency_blob);
  ctx->kv_latency_blob = rt_bytes_empty(ctx);

#if X07_ENABLE_KV
  for (uint32_t i = 0; i < ctx->kv_len; i++) {
    rt_bytes_drop(ctx, &ctx->kv_items[i].key);
    rt_bytes_drop(ctx, &ctx->kv_items[i].val);
  }
  if (ctx->kv_items && ctx->kv_cap) {
    rt_free(
      ctx,
      ctx->kv_items,
      ctx->kv_cap * (uint32_t)sizeof(kv_entry_t),
      (uint32_t)_Alignof(kv_entry_t)
    );
  }
#endif
  ctx->kv_items = NULL;
  ctx->kv_len = 0;
  ctx->kv_cap = 0;

  for (uint32_t i = 0; i < ctx->map_u32_len; i++) {
    map_u32_t* m = (map_u32_t*)ctx->map_u32_items[i];
    if (!m) continue;
    if (m->keys && m->cap) {
      rt_free(ctx, m->keys, m->cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
    }
    if (m->vals && m->cap) {
      rt_free(ctx, m->vals, m->cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
    }
    rt_free(ctx, m, (uint32_t)sizeof(map_u32_t), (uint32_t)_Alignof(map_u32_t));
  }
  if (ctx->map_u32_items && ctx->map_u32_cap) {
    rt_free(
      ctx,
      ctx->map_u32_items,
      ctx->map_u32_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
  }
  ctx->map_u32_items = NULL;
  ctx->map_u32_len = 0;
  ctx->map_u32_cap = 0;

  for (uint32_t i = 0; i < ctx->vec_value_len; i++) {
    vec_value_t* v = (vec_value_t*)ctx->vec_value_items[i];
    if (!v) continue;
    uint32_t esz = v->ops->size;
    for (uint32_t j = 0; j < v->len; j++) {
      v->ops->drop_in_place(ctx, v->data + (j * esz));
    }
    if (v->data && v->cap) {
      if (esz != 0 && v->cap > UINT32_MAX / esz) rt_trap("vec_value.cleanup overflow");
      rt_free(ctx, v->data, v->cap * esz, v->ops->align);
    }
    rt_free(ctx, v, (uint32_t)sizeof(vec_value_t), (uint32_t)_Alignof(vec_value_t));
  }
  if (ctx->vec_value_items && ctx->vec_value_cap) {
    rt_free(
      ctx,
      ctx->vec_value_items,
      ctx->vec_value_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
  }
  ctx->vec_value_items = NULL;
  ctx->vec_value_len = 0;
  ctx->vec_value_cap = 0;

  for (uint32_t i = 0; i < ctx->map_value_len; i++) {
    map_value_t* m = (map_value_t*)ctx->map_value_items[i];
    if (!m) continue;
    (void)rt_map_value_clear_in_place(ctx, m);
    rt_map_value_free_arrays(ctx, m);
    rt_free(ctx, m, (uint32_t)sizeof(map_value_t), (uint32_t)_Alignof(map_value_t));
  }
  if (ctx->map_value_items && ctx->map_value_cap) {
    rt_free(
      ctx,
      ctx->map_value_items,
      ctx->map_value_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
  }
  ctx->map_value_items = NULL;
  ctx->map_value_len = 0;
  ctx->map_value_cap = 0;
}
"#;
const RUNTIME_C_MAIN: &str = r#"
static int rt_read_exact(int fd, uint8_t* dst, uint32_t len) {
  uint32_t off = 0;
  while (off < len) {
    ssize_t n = read(fd, dst + off, len - off);
    if (n == 0) return -1;
    if (n < 0) {
      if (errno == EINTR) continue;
      return -1;
    }
    off += (uint32_t)n;
  }
  return 0;
}

static int rt_write_exact(int fd, const uint8_t* src, uint32_t len) {
  uint32_t off = 0;
  while (off < len) {
    ssize_t n = write(fd, src + off, len - off);
    if (n < 0) {
      if (errno == EINTR) continue;
      return -1;
    }
    off += (uint32_t)n;
  }
  return 0;
}

int main(void) {
#if defined(SIGPIPE) && defined(SIG_IGN)
  (void)signal(SIGPIPE, SIG_IGN);
#endif

  const uint32_t mem_cap = (uint32_t)(X07_MEM_CAP);
  int mem_is_mmap = 0;
  uint8_t* mem = NULL;
  mem = (uint8_t*)mmap(
    NULL,
    (size_t)mem_cap,
    PROT_READ | PROT_WRITE,
    MAP_PRIVATE | MAP_ANON,
    -1,
    0
  );
  if (mem != (uint8_t*)MAP_FAILED) {
    mem_is_mmap = 1;
  } else {
    mem = (uint8_t*)calloc(1, (size_t)mem_cap);
    if (!mem) rt_trap("calloc failed");
  }

  ctx_t ctx;
  memset(&ctx, 0, sizeof(ctx));
  ctx.fuel_init = (uint64_t)(X07_FUEL_INIT);
  ctx.fuel = ctx.fuel_init;
  ctx.heap.mem = mem;
  ctx.heap.cap = mem_cap;
  rt_heap_init(&ctx);
  rt_allocator_init(&ctx);
  rt_ext_ctx = &ctx;

#ifdef X07_DEBUG_BORROW
  rt_dbg_init(&ctx);
#endif

  rt_kv_init(&ctx);

  uint8_t len_buf[4];
  if (rt_read_exact(STDIN_FILENO, len_buf, 4) != 0) return 2;
  uint32_t in_len = (uint32_t)len_buf[0]
                  | ((uint32_t)len_buf[1] << 8)
                  | ((uint32_t)len_buf[2] << 16)
                  | ((uint32_t)len_buf[3] << 24);

  bytes_t input_bytes = rt_bytes_alloc(&ctx, in_len);
  if (in_len && rt_read_exact(STDIN_FILENO, input_bytes.ptr, in_len) != 0) return 2;

  rt_mem_epoch_reset(&ctx);

  bytes_view_t input = rt_bytes_view(&ctx, input_bytes);
  bytes_t out = solve(&ctx, input);
  int32_t exit_code = ctx.exit_code;
  rt_ext_ctx = NULL;

#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_bytes_check(&ctx, out);
#endif

  uint32_t out_len = out.len;
  uint8_t bytes_eq_payload[13 + (X07_ASSERT_BYTES_EQ_PREFIX_MAX * 2)];
  uint32_t bytes_eq_payload_len = 0;
  if (out_len == 5 && out.ptr && out.ptr[0] == 0 && ctx.last_bytes_eq_valid) {
    uint32_t code = (uint32_t)out.ptr[1]
                  | ((uint32_t)out.ptr[2] << 8)
                  | ((uint32_t)out.ptr[3] << 16)
                  | ((uint32_t)out.ptr[4] << 24);
    if (code == 1003) {
      uint32_t got_len = ctx.last_bytes_eq_a_len;
      uint32_t expected_len = ctx.last_bytes_eq_b_len;
      uint32_t got_prefix_len = (got_len < X07_ASSERT_BYTES_EQ_PREFIX_MAX)
        ? got_len
        : X07_ASSERT_BYTES_EQ_PREFIX_MAX;
      uint32_t expected_prefix_len = (expected_len < X07_ASSERT_BYTES_EQ_PREFIX_MAX)
        ? expected_len
        : X07_ASSERT_BYTES_EQ_PREFIX_MAX;

      bytes_eq_payload[0] = (uint8_t)'X';
      bytes_eq_payload[1] = (uint8_t)'7';
      bytes_eq_payload[2] = (uint8_t)'T';
      bytes_eq_payload[3] = (uint8_t)'1';
      bytes_eq_payload[4] = (uint8_t)X07_ASSERT_BYTES_EQ_PREFIX_MAX;
      bytes_eq_payload[5] = (uint8_t)(got_len & UINT32_C(0xFF));
      bytes_eq_payload[6] = (uint8_t)((got_len >> 8) & UINT32_C(0xFF));
      bytes_eq_payload[7] = (uint8_t)((got_len >> 16) & UINT32_C(0xFF));
      bytes_eq_payload[8] = (uint8_t)((got_len >> 24) & UINT32_C(0xFF));
      bytes_eq_payload[9] = (uint8_t)(expected_len & UINT32_C(0xFF));
      bytes_eq_payload[10] = (uint8_t)((expected_len >> 8) & UINT32_C(0xFF));
      bytes_eq_payload[11] = (uint8_t)((expected_len >> 16) & UINT32_C(0xFF));
      bytes_eq_payload[12] = (uint8_t)((expected_len >> 24) & UINT32_C(0xFF));

      uint32_t off = 13;
      if (got_prefix_len) {
        memcpy(bytes_eq_payload + off, ctx.last_bytes_eq_a_prefix, got_prefix_len);
        off += got_prefix_len;
      }
      if (expected_prefix_len) {
        memcpy(bytes_eq_payload + off, ctx.last_bytes_eq_b_prefix, expected_prefix_len);
        off += expected_prefix_len;
      }
      bytes_eq_payload_len = off;
    }
  }

  uint32_t out_total_len = out_len + bytes_eq_payload_len;
  uint8_t out_len_buf[4] = {
    (uint8_t)(out_total_len & UINT32_C(0xFF)),
    (uint8_t)((out_total_len >> 8) & UINT32_C(0xFF)),
    (uint8_t)((out_total_len >> 16) & UINT32_C(0xFF)),
    (uint8_t)((out_total_len >> 24) & UINT32_C(0xFF)),
  };
  if (rt_write_exact(STDOUT_FILENO, out_len_buf, 4) != 0) return 2;
  if (out_len && rt_write_exact(STDOUT_FILENO, out.ptr, out_len) != 0) return 2;
  if (bytes_eq_payload_len && rt_write_exact(STDOUT_FILENO, bytes_eq_payload, bytes_eq_payload_len) != 0) return 2;

  rt_bytes_drop(&ctx, &out);
  rt_bytes_drop(&ctx, &input_bytes);
  rt_ctx_cleanup(&ctx);

  uint32_t heap_used = (ctx.heap_peak_live_bytes > (uint64_t)UINT32_MAX)
    ? UINT32_MAX
    : (uint32_t)ctx.heap_peak_live_bytes;
  uint64_t fuel_used = ctx.fuel_init - ctx.fuel;

  ctx.sched_stats.virtual_time_end = ctx.sched_now_ticks;
  rt_sched_trace_init(&ctx);
  char sched_trace_hash_str[19];
  (void)snprintf(
    sched_trace_hash_str,
    sizeof(sched_trace_hash_str),
    "0x%016" PRIx64,
    ctx.sched_stats.sched_trace_hash
  );

#ifdef X07_DEBUG_BORROW
  fprintf(
    stderr,
    "{\"fuel_used\":%" PRIu64 ",\"heap_used\":%u,\"fs_read_file_calls\":%" PRIu64 ",\"fs_list_dir_calls\":%" PRIu64 ","
    "\"rr_open_calls\":%" PRIu64 ",\"rr_close_calls\":%" PRIu64 ",\"rr_stats_calls\":%" PRIu64 ","
    "\"rr_next_calls\":%" PRIu64 ",\"rr_next_miss_calls\":%" PRIu64 ",\"rr_append_calls\":%" PRIu64 ","
    "\"kv_get_calls\":%" PRIu64 ",\"kv_set_calls\":%" PRIu64 ","
    "\"sched_stats\":{"
    "\"tasks_spawned\":%" PRIu64 ",\"spawn_calls\":%" PRIu64 ",\"join_calls\":%" PRIu64 ","
    "\"yield_calls\":%" PRIu64 ",\"sleep_calls\":%" PRIu64 ","
    "\"chan_send_calls\":%" PRIu64 ",\"chan_recv_calls\":%" PRIu64 ","
    "\"ctx_switches\":%" PRIu64 ",\"wake_events\":%" PRIu64 ",\"blocked_waits\":%" PRIu64 ","
    "\"virtual_time_end\":%" PRIu64 ",\"sched_trace_hash\":\"%s\"},"
    "\"mem_stats\":{"
    "\"alloc_calls\":%" PRIu64 ",\"realloc_calls\":%" PRIu64 ",\"free_calls\":%" PRIu64 ","
    "\"bytes_alloc_total\":%" PRIu64 ",\"bytes_freed_total\":%" PRIu64 ","
    "\"live_bytes\":%" PRIu64 ",\"peak_live_bytes\":%" PRIu64 ","
    "\"live_allocs\":%" PRIu64 ",\"peak_live_allocs\":%" PRIu64 ","
    "\"memcpy_bytes\":%" PRIu64 "},"
    "\"debug_stats\":{"
    "\"borrow_violations\":%" PRIu64 "}}\n",
    fuel_used,
    heap_used,
    ctx.fs_read_file_calls,
    ctx.fs_list_dir_calls,
    ctx.rr_open_calls,
    ctx.rr_close_calls,
    ctx.rr_stats_calls,
    ctx.rr_next_calls,
    ctx.rr_next_miss_calls,
    ctx.rr_append_calls,
    ctx.kv_get_calls,
    ctx.kv_set_calls,
    ctx.sched_stats.tasks_spawned,
    ctx.sched_stats.spawn_calls,
    ctx.sched_stats.join_calls,
    ctx.sched_stats.yield_calls,
    ctx.sched_stats.sleep_calls,
    ctx.sched_stats.chan_send_calls,
    ctx.sched_stats.chan_recv_calls,
    ctx.sched_stats.ctx_switches,
    ctx.sched_stats.wake_events,
    ctx.sched_stats.blocked_waits,
    ctx.sched_stats.virtual_time_end,
    sched_trace_hash_str,
    ctx.mem_stats.alloc_calls,
    ctx.mem_stats.realloc_calls,
    ctx.mem_stats.free_calls,
    ctx.mem_stats.bytes_alloc_total,
    ctx.mem_stats.bytes_freed_total,
    ctx.mem_stats.live_bytes,
    ctx.mem_stats.peak_live_bytes,
    ctx.mem_stats.live_allocs,
    ctx.mem_stats.peak_live_allocs,
    ctx.mem_stats.memcpy_bytes,
    ctx.dbg_borrow_violations
  );
#else
  fprintf(
    stderr,
    "{\"fuel_used\":%" PRIu64 ",\"heap_used\":%u,\"fs_read_file_calls\":%" PRIu64 ",\"fs_list_dir_calls\":%" PRIu64 ","
    "\"rr_open_calls\":%" PRIu64 ",\"rr_close_calls\":%" PRIu64 ",\"rr_stats_calls\":%" PRIu64 ","
    "\"rr_next_calls\":%" PRIu64 ",\"rr_next_miss_calls\":%" PRIu64 ",\"rr_append_calls\":%" PRIu64 ","
    "\"kv_get_calls\":%" PRIu64 ",\"kv_set_calls\":%" PRIu64 ","
    "\"sched_stats\":{"
    "\"tasks_spawned\":%" PRIu64 ",\"spawn_calls\":%" PRIu64 ",\"join_calls\":%" PRIu64 ","
    "\"yield_calls\":%" PRIu64 ",\"sleep_calls\":%" PRIu64 ","
    "\"chan_send_calls\":%" PRIu64 ",\"chan_recv_calls\":%" PRIu64 ","
    "\"ctx_switches\":%" PRIu64 ",\"wake_events\":%" PRIu64 ",\"blocked_waits\":%" PRIu64 ","
    "\"virtual_time_end\":%" PRIu64 ",\"sched_trace_hash\":\"%s\"},"
    "\"mem_stats\":{"
    "\"alloc_calls\":%" PRIu64 ",\"realloc_calls\":%" PRIu64 ",\"free_calls\":%" PRIu64 ","
    "\"bytes_alloc_total\":%" PRIu64 ",\"bytes_freed_total\":%" PRIu64 ","
    "\"live_bytes\":%" PRIu64 ",\"peak_live_bytes\":%" PRIu64 ","
    "\"live_allocs\":%" PRIu64 ",\"peak_live_allocs\":%" PRIu64 ","
    "\"memcpy_bytes\":%" PRIu64 "}}\n",
    fuel_used,
    heap_used,
    ctx.fs_read_file_calls,
    ctx.fs_list_dir_calls,
    ctx.rr_open_calls,
    ctx.rr_close_calls,
    ctx.rr_stats_calls,
    ctx.rr_next_calls,
    ctx.rr_next_miss_calls,
    ctx.rr_append_calls,
    ctx.kv_get_calls,
    ctx.kv_set_calls,
    ctx.sched_stats.tasks_spawned,
    ctx.sched_stats.spawn_calls,
    ctx.sched_stats.join_calls,
    ctx.sched_stats.yield_calls,
    ctx.sched_stats.sleep_calls,
    ctx.sched_stats.chan_send_calls,
    ctx.sched_stats.chan_recv_calls,
    ctx.sched_stats.ctx_switches,
    ctx.sched_stats.wake_events,
    ctx.sched_stats.blocked_waits,
    ctx.sched_stats.virtual_time_end,
    sched_trace_hash_str,
    ctx.mem_stats.alloc_calls,
    ctx.mem_stats.realloc_calls,
    ctx.mem_stats.free_calls,
    ctx.mem_stats.bytes_alloc_total,
    ctx.mem_stats.bytes_freed_total,
    ctx.mem_stats.live_bytes,
    ctx.mem_stats.peak_live_bytes,
    ctx.mem_stats.live_allocs,
    ctx.mem_stats.peak_live_allocs,
    ctx.mem_stats.memcpy_bytes
  );
#endif
  fflush(stderr);
  if (mem_is_mmap) {
    (void)munmap(mem, (size_t)mem_cap);
  } else {
    free(mem);
  }
  if (exit_code < 0 || exit_code > 255) exit_code = 255;
  return (int)exit_code;
}
"#;
pub(super) const RUNTIME_C_HEADER: &str = r#"
#ifndef X07_PKG_H
#define X07_PKG_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} bytes_t;

typedef struct {
  void* ctx;
  void* (*alloc)(void* ctx, uint32_t size, uint32_t align);
  void* (*realloc)(void* ctx, void* ptr, uint32_t old_size, uint32_t new_size, uint32_t align);
  void (*free)(void* ctx, void* ptr, uint32_t size, uint32_t align);
} allocator_v1_t;

allocator_v1_t x07_custom_allocator(void);

bytes_t x07_solve_v2(
    uint8_t* arena_mem,
    uint32_t arena_cap,
    const uint8_t* input_ptr,
    uint32_t input_len
);

int32_t x07_exit_code_v1(void);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_PKG_H
"#;
const RUNTIME_C_LIB: &str = r#"
static uint8_t rt_dummy_heap_mem[1];
static int32_t rt_last_exit_code = 0;

int32_t x07_exit_code_v1(void) {
  return rt_last_exit_code;
}

bytes_t x07_solve_v2(
    uint8_t* arena_mem,
    uint32_t arena_cap,
    const uint8_t* input_ptr,
    uint32_t input_len
) {
  if (!arena_mem) {
    if (arena_cap != 0) rt_trap("arena_mem is NULL");
    arena_mem = rt_dummy_heap_mem;
  }

  ctx_t ctx;
  memset(&ctx, 0, sizeof(ctx));
  ctx.fuel_init = (uint64_t)(X07_FUEL_INIT);
  ctx.fuel = ctx.fuel_init;
  ctx.heap.mem = arena_mem;
  ctx.heap.cap = arena_cap;
  rt_heap_init(&ctx);
  rt_allocator_init(&ctx);
  rt_ext_ctx = &ctx;

#ifdef X07_DEBUG_BORROW
  rt_dbg_init(&ctx);
#endif

  rt_kv_init(&ctx);

  bytes_t input_bytes = rt_bytes_alloc(&ctx, input_len);
  if (input_len) {
    memcpy(input_bytes.ptr, input_ptr, input_len);
    rt_mem_on_memcpy(&ctx, input_len);
  }

  rt_mem_epoch_reset(&ctx);

  bytes_view_t input = rt_bytes_view(&ctx, input_bytes);
  bytes_t out = solve(&ctx, input);
  rt_last_exit_code = ctx.exit_code;
  rt_ext_ctx = NULL;

#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_bytes_check(&ctx, out);
#endif

  rt_bytes_drop(&ctx, &input_bytes);

  return out;
}
"#;

const RUNTIME_C_PURE_STUBS: &str = r#"
static void rt_kv_init(ctx_t* ctx) {
  (void)ctx;
}

static void rt_hex_bytes(const uint8_t* bytes, uint32_t len, char* out) {
  static const char LUT[16] = "0123456789abcdef";
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = bytes[i];
    out[i * 2 + 0] = LUT[b >> 4];
    out[i * 2 + 1] = LUT[b & 0x0F];
  }
  out[len * 2] = 0;
}
"#;
