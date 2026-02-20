use super::*;

fn expr_uses_stream_xf_plugin_json_jcs(expr: &Expr) -> bool {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => false,
        Expr::List { items, .. } => {
            if items
                .first()
                .and_then(Expr::as_ident)
                .is_some_and(|h| h == "__internal.stream_xf.plugin_init_v1")
            {
                let export_symbol = items
                    .get(3)
                    .and_then(|e| {
                        let Expr::List { items, .. } = e else {
                            return None;
                        };
                        if items.len() != 2
                            || items.first().and_then(Expr::as_ident) != Some("bytes.lit")
                        {
                            return None;
                        }
                        items.get(1).and_then(Expr::as_ident)
                    })
                    .unwrap_or_default();
                if export_symbol == "x07_xf_json_canon_stream_v1" {
                    return true;
                }

                let canon_mode = items.get(8).and_then(|e| match e {
                    Expr::Int { value, .. } => Some(*value),
                    _ => None,
                });
                let strict_cfg_canon = items.get(9).and_then(|e| match e {
                    Expr::Int { value, .. } => Some(*value),
                    _ => None,
                });
                if canon_mode == Some(1) && strict_cfg_canon == Some(1) {
                    return true;
                }
            }
            items.iter().any(expr_uses_stream_xf_plugin_json_jcs)
        }
    }
}

pub(super) fn program_uses_stream_xf_plugin_json_jcs(program: &Program) -> bool {
    if expr_uses_stream_xf_plugin_json_jcs(&program.solve) {
        return true;
    }
    for f in &program.functions {
        if expr_uses_stream_xf_plugin_json_jcs(&f.body) {
            return true;
        }
    }
    for f in &program.async_functions {
        if expr_uses_stream_xf_plugin_json_jcs(&f.body) {
            return true;
        }
    }
    false
}

impl<'a> Emitter<'a> {
    pub(super) fn emit_json_jcs_canon_doc_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_MATH,
            native::ABI_MAJOR_V1,
            "json.jcs.canon_doc_v1",
        )?;
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "json.jcs.canon_doc_v1 expects 4 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "json.jcs.canon_doc_v1 returns bytes".to_string(),
            ));
        }
        let input = self.emit_expr_as_bytes_view(&args[0])?;
        if input.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "json.jcs.canon_doc_v1 expects bytes_view".to_string(),
            ));
        }
        let max_depth = self.emit_expr(&args[1])?;
        let max_object_members = self.emit_expr(&args[2])?;
        let max_object_total_bytes = self.emit_expr(&args[3])?;
        if max_depth.ty != Ty::I32
            || max_object_members.ty != Ty::I32
            || max_object_total_bytes.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "json.jcs.canon_doc_v1 expects (bytes_view, i32, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_json_jcs_canon_doc_v1(ctx, {}, (uint32_t){}, (uint32_t){}, (uint32_t){});",
            input.c_name,
            max_depth.c_name,
            max_object_members.c_name,
            max_object_total_bytes.c_name
        ));
        self.release_temp_view_borrow(&input)?;
        Ok(())
    }

    fn lookup_borrowed_bytes_ident_arg(
        &self,
        head: &str,
        arg: &Expr,
    ) -> Result<VarRef, CompilerError> {
        let Some(name) = arg.as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects bytes identifier"),
            ));
        };
        let Some(var) = self.lookup(name).cloned() else {
            return Err(self.err(
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
            return Err(self.err(
                CompileErrorKind::Typing,
                format!("use after move: {name:?} moved_ptr={moved_ptr}"),
            ));
        }
        if var.ty != Ty::Bytes {
            return Err(self.err(
                CompileErrorKind::Typing,
                format!("{head} expects bytes identifier"),
            ));
        }
        Ok(var)
    }

    pub(super) fn emit_internal_stream_xf_plugin_init_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 12 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.stream_xf.plugin_init_v1 expects 12 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.stream_xf.plugin_init_v1 returns result_bytes".to_string(),
            ));
        }

        let backend_id = self
            .parse_bytes_lit_text_arg("__internal.stream_xf.plugin_init_v1 backend_id", &args[0])?;
        crate::validate::validate_symbol(&backend_id)
            .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
        let abi_major_i32 =
            self.parse_i32_lit_arg("__internal.stream_xf.plugin_init_v1 abi_major", &args[1])?;
        let abi_major = u32::try_from(abi_major_i32).unwrap_or(0);
        if abi_major == 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.stream_xf.plugin_init_v1 abi_major must be >= 1".to_string(),
            ));
        }
        let export_symbol = self.parse_bytes_lit_text_arg(
            "__internal.stream_xf.plugin_init_v1 export_symbol",
            &args[2],
        )?;
        crate::validate::validate_local_name(&export_symbol)
            .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

        self.require_native_backend(&backend_id, abi_major, &export_symbol)?;
        let needs_json_jcs = export_symbol == "x07_xf_json_canon_stream_v1"
            || matches!(
                (&args[7], &args[8]),
                (Expr::Int { value: 1, .. }, Expr::Int { value: 1, .. })
            );
        if needs_json_jcs {
            self.require_native_backend(
                native::BACKEND_ID_MATH,
                native::ABI_MAJOR_V1,
                "json.jcs.canon_doc_v1",
            )?;
        }

        let state_b = self.lookup_borrowed_bytes_ident_arg(
            "__internal.stream_xf.plugin_init_v1 state",
            &args[3],
        )?;
        let scratch_b = self.lookup_borrowed_bytes_ident_arg(
            "__internal.stream_xf.plugin_init_v1 scratch",
            &args[4],
        )?;
        let cfg_b = self
            .lookup_borrowed_bytes_ident_arg("__internal.stream_xf.plugin_init_v1 cfg", &args[5])?;
        let cfg_max_bytes = self.emit_expr(&args[6])?;
        let canon_mode = self.emit_expr(&args[7])?;
        let strict_cfg_canon = self.emit_expr(&args[8])?;
        let max_out_bytes_per_step = self.emit_expr(&args[9])?;
        let max_out_items_per_step = self.emit_expr(&args[10])?;
        let max_out_buf_bytes = self.emit_expr(&args[11])?;
        if cfg_max_bytes.ty != Ty::I32
            || canon_mode.ty != Ty::I32
            || strict_cfg_canon.ty != Ty::I32
            || max_out_bytes_per_step.ty != Ty::I32
            || max_out_items_per_step.ty != Ty::I32
            || max_out_buf_bytes.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.stream_xf.plugin_init_v1 arg types mismatch".to_string(),
            ));
        }

        self.line(&format!("extern x07_stream_xf_plugin_v1 {export_symbol};"));
        self.line(&format!(
            "{dest} = rt_stream_xf_plugin_init_v1(ctx, &{export_symbol}, UINT32_C({abi_major}), {}, {}, {}, (uint32_t){}, (uint32_t){}, (uint32_t){}, (uint32_t){}, (uint32_t){}, (uint32_t){});",
            state_b.c_name,
            scratch_b.c_name,
            cfg_b.c_name,
            cfg_max_bytes.c_name,
            canon_mode.c_name,
            strict_cfg_canon.c_name,
            max_out_bytes_per_step.c_name,
            max_out_items_per_step.c_name,
            max_out_buf_bytes.c_name,
        ));
        Ok(())
    }

    pub(super) fn emit_internal_stream_xf_plugin_step_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 9 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.stream_xf.plugin_step_v1 expects 9 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.stream_xf.plugin_step_v1 returns result_bytes".to_string(),
            ));
        }

        let backend_id = self
            .parse_bytes_lit_text_arg("__internal.stream_xf.plugin_step_v1 backend_id", &args[0])?;
        crate::validate::validate_symbol(&backend_id)
            .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
        let abi_major_i32 =
            self.parse_i32_lit_arg("__internal.stream_xf.plugin_step_v1 abi_major", &args[1])?;
        let abi_major = u32::try_from(abi_major_i32).unwrap_or(0);
        if abi_major == 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.stream_xf.plugin_step_v1 abi_major must be >= 1".to_string(),
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
        let max_out_bytes_per_step = self.emit_expr(&args[5])?;
        let max_out_items_per_step = self.emit_expr(&args[6])?;
        let max_out_buf_bytes = self.emit_expr(&args[7])?;
        let input = self.emit_expr(&args[8])?;
        if max_out_bytes_per_step.ty != Ty::I32
            || max_out_items_per_step.ty != Ty::I32
            || max_out_buf_bytes.ty != Ty::I32
            || input.ty != Ty::BytesView
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.stream_xf.plugin_step_v1 arg types mismatch".to_string(),
            ));
        }

        self.line(&format!("extern x07_stream_xf_plugin_v1 {export_symbol};"));
        self.line(&format!(
            "{dest} = rt_stream_xf_plugin_step_v1(ctx, &{export_symbol}, UINT32_C({abi_major}), {}, {}, (uint32_t){}, (uint32_t){}, (uint32_t){}, {});",
            state_b.c_name,
            scratch_b.c_name,
            max_out_bytes_per_step.c_name,
            max_out_items_per_step.c_name,
            max_out_buf_bytes.c_name,
            input.c_name,
        ));

        self.release_temp_view_borrow(&input)?;
        Ok(())
    }

    pub(super) fn emit_internal_stream_xf_plugin_flush_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 8 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.stream_xf.plugin_flush_v1 expects 8 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
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
        let abi_major_i32 =
            self.parse_i32_lit_arg("__internal.stream_xf.plugin_flush_v1 abi_major", &args[1])?;
        let abi_major = u32::try_from(abi_major_i32).unwrap_or(0);
        if abi_major == 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.stream_xf.plugin_flush_v1 abi_major must be >= 1".to_string(),
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
        let max_out_bytes_per_step = self.emit_expr(&args[5])?;
        let max_out_items_per_step = self.emit_expr(&args[6])?;
        let max_out_buf_bytes = self.emit_expr(&args[7])?;
        if max_out_bytes_per_step.ty != Ty::I32
            || max_out_items_per_step.ty != Ty::I32
            || max_out_buf_bytes.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.stream_xf.plugin_flush_v1 arg types mismatch".to_string(),
            ));
        }

        self.line(&format!("extern x07_stream_xf_plugin_v1 {export_symbol};"));
        self.line(&format!(
            "{dest} = rt_stream_xf_plugin_flush_v1(ctx, &{export_symbol}, UINT32_C({abi_major}), {}, {}, (uint32_t){}, (uint32_t){}, (uint32_t){});",
            state_b.c_name,
            scratch_b.c_name,
            max_out_bytes_per_step.c_name,
            max_out_items_per_step.c_name,
            max_out_buf_bytes.c_name,
        ));
        Ok(())
    }
}
