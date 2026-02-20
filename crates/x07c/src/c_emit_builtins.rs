use super::*;

impl<'a> Emitter<'a> {
    pub(super) fn emit_expr(&mut self, expr: &Expr) -> Result<VarRef, CompilerError> {
        let prev_ptr = self.current_ptr.replace(expr.ptr().to_string());
        let out = (|| {
            let ty = self.infer_expr_in_new_scope(expr)?;
            let (storage_ty, name) = match ty.ty {
                Ty::I32
                | Ty::TaskHandleBytesV1
                | Ty::TaskHandleResultBytesV1
                | Ty::TaskSlotV1
                | Ty::TaskSelectEvtV1 => (ty.ty, self.alloc_local("t_i32_")?),
                Ty::TaskScopeV1 => (Ty::TaskScopeV1, self.alloc_local("t_scope_")?),
                Ty::BudgetScopeV1 => (Ty::BudgetScopeV1, self.alloc_local("t_budget_scope_")?),
                Ty::Bytes => (Ty::Bytes, self.alloc_local("t_bytes_")?),
                Ty::BytesView => (Ty::BytesView, self.alloc_local("t_view_")?),
                Ty::VecU8 => (Ty::VecU8, self.alloc_local("t_vec_u8_")?),
                Ty::OptionI32 | Ty::OptionTaskSelectEvtV1 => {
                    (ty.ty, self.alloc_local("t_opt_i32_")?)
                }
                Ty::OptionBytes => (Ty::OptionBytes, self.alloc_local("t_opt_bytes_")?),
                Ty::OptionBytesView => (Ty::OptionBytesView, self.alloc_local("t_opt_view_")?),
                Ty::ResultI32 => (Ty::ResultI32, self.alloc_local("t_res_i32_")?),
                Ty::ResultBytes => (Ty::ResultBytes, self.alloc_local("t_res_bytes_")?),
                Ty::ResultBytesView => (Ty::ResultBytesView, self.alloc_local("t_res_view_")?),
                Ty::ResultResultBytes => {
                    (Ty::ResultResultBytes, self.alloc_local("t_res_res_bytes_")?)
                }
                Ty::Iface => (Ty::Iface, self.alloc_local("t_iface_")?),
                Ty::PtrConstU8 => (Ty::PtrConstU8, self.alloc_local("t_ptr_")?),
                Ty::PtrMutU8 => (Ty::PtrMutU8, self.alloc_local("t_ptr_")?),
                Ty::PtrConstVoid => (Ty::PtrConstVoid, self.alloc_local("t_ptr_")?),
                Ty::PtrMutVoid => (Ty::PtrMutVoid, self.alloc_local("t_ptr_")?),
                Ty::PtrConstI32 => (Ty::PtrConstI32, self.alloc_local("t_ptr_")?),
                Ty::PtrMutI32 => (Ty::PtrMutI32, self.alloc_local("t_ptr_")?),
                Ty::Never => (Ty::I32, self.alloc_local("t_never_")?),
            };
            self.decl_local(storage_ty, &name);
            self.emit_expr_to(expr, storage_ty, &name)?;

            let mut v = self.make_var_ref(ty.ty, name.clone(), true);
            v.brand = ty.brand.clone();
            if is_view_like_ty(ty.ty) {
                let borrow_of = self.borrow_of_view_like_expr(ty.ty, expr)?;
                let borrow_ptr = borrow_of.as_ref().map(|_| expr.ptr().to_string());
                if let Some(owner) = &borrow_of {
                    self.inc_borrow_count(owner)?;
                }
                v.borrow_of = borrow_of;
                v.borrow_ptr = borrow_ptr;
            }

            if is_owned_ty(ty.ty) || is_view_like_ty(ty.ty) {
                self.bind(format!("#tmp:{name}"), v.clone());
            }

            Ok(v)
        })();
        self.current_ptr = prev_ptr;
        out
    }

    pub(super) fn emit_expr_as_bytes_view(&mut self, expr: &Expr) -> Result<VarRef, CompilerError> {
        let prev_ptr = self.current_ptr.replace(expr.ptr().to_string());
        let out = (|| {
            let ty = self.infer_expr_in_new_scope(expr)?;
            match ty.ty {
                Ty::BytesView => self.emit_expr(expr),
                Ty::Bytes => match expr {
                    Expr::Ident { name, .. } if name != "input" => {
                        let Some(owner) = self.lookup(name).cloned() else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if owner.moved {
                            let moved_ptr = owner
                                .moved_ptr
                                .as_deref()
                                .filter(|p| !p.is_empty())
                                .unwrap_or("<unknown>");
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("use after move: {name:?} moved_ptr={moved_ptr}"),
                            ));
                        }
                        if owner.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("type mismatch for identifier {name:?}"),
                            ));
                        }

                        let tmp = self.alloc_local("t_view_")?;
                        self.decl_local(Ty::BytesView, &tmp);
                        self.line(&format!("{tmp} = rt_bytes_view(ctx, {});", owner.c_name));
                        self.inc_borrow_count(&owner.c_name)?;
                        let mut view = self.make_var_ref(Ty::BytesView, tmp.clone(), true);
                        view.brand = owner.brand.clone();
                        view.borrow_of = Some(owner.c_name);
                        view.borrow_ptr = Some(expr.ptr().to_string());
                        view.borrow_is_full = true;
                        self.bind(format!("#tmp:{tmp}"), view.clone());
                        Ok(view)
                    }
                    _ => {
                        let owner = self.emit_expr(expr)?;
                        if owner.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("expected bytes, got {:?}", owner.ty),
                            ));
                        }
                        let tmp = self.alloc_local("t_view_")?;
                        self.decl_local(Ty::BytesView, &tmp);
                        self.line(&format!("{tmp} = rt_bytes_view(ctx, {});", owner.c_name));
                        self.inc_borrow_count(&owner.c_name)?;
                        let mut view = self.make_var_ref(Ty::BytesView, tmp.clone(), true);
                        view.brand = owner.brand.clone();
                        view.borrow_of = Some(owner.c_name);
                        view.borrow_ptr = Some(expr.ptr().to_string());
                        view.borrow_is_full = true;
                        self.bind(format!("#tmp:{tmp}"), view.clone());
                        Ok(view)
                    }
                },
                Ty::VecU8 => match expr {
                    Expr::Ident { name, .. } => {
                        let Some(owner) = self.lookup(name).cloned() else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
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
                                format!("use after move: {name:?} moved_ptr={moved_ptr}"),
                            ));
                        }
                        if owner.ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("type mismatch for identifier {name:?}"),
                            ));
                        }

                        let tmp = self.alloc_local("t_view_")?;
                        self.decl_local(Ty::BytesView, &tmp);
                        self.line(&format!(
                            "{tmp} = rt_vec_u8_as_view(ctx, {});",
                            owner.c_name
                        ));
                        self.inc_borrow_count(&owner.c_name)?;
                        let mut view = self.make_var_ref(Ty::BytesView, tmp.clone(), true);
                        view.borrow_of = Some(owner.c_name);
                        view.borrow_ptr = Some(expr.ptr().to_string());
                        view.borrow_is_full = true;
                        self.bind(format!("#tmp:{tmp}"), view.clone());
                        Ok(view)
                    }
                    _ => {
                        let owner = self.emit_expr(expr)?;
                        if owner.ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("expected vec_u8, got {:?}", owner.ty),
                            ));
                        }
                        let tmp = self.alloc_local("t_view_")?;
                        self.decl_local(Ty::BytesView, &tmp);
                        self.line(&format!(
                            "{tmp} = rt_vec_u8_as_view(ctx, {});",
                            owner.c_name
                        ));
                        self.inc_borrow_count(&owner.c_name)?;
                        let mut view = self.make_var_ref(Ty::BytesView, tmp.clone(), true);
                        view.borrow_of = Some(owner.c_name);
                        view.borrow_ptr = Some(expr.ptr().to_string());
                        view.borrow_is_full = true;
                        self.bind(format!("#tmp:{tmp}"), view.clone());
                        Ok(view)
                    }
                },
                other => Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("expected bytes/bytes_view/vec_u8, got {other:?}"),
                )),
            }
        })();
        self.current_ptr = prev_ptr;
        out
    }

    pub(super) fn emit_stmt(&mut self, expr: &Expr) -> Result<(), CompilerError> {
        let prev_ptr = self.current_ptr.replace(expr.ptr().to_string());
        let out = (|| match expr {
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
                        self.open_block();
                        for e in args {
                            self.emit_stmt(e)?;
                        }
                        self.pop_scope()?;
                        self.close_block();
                        Ok(())
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }

                        let cond = self.emit_expr(&args[0])?;
                        if cond.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "if condition must be i32".to_string(),
                            ));
                        }

                        let then_ty = self.infer_expr_in_new_scope(&args[1])?;
                        let else_ty = self.infer_expr_in_new_scope(&args[2])?;
                        let scopes_before = self.scopes.clone();

                        self.line(&format!("if ({} != UINT32_C(0)) {{", cond.c_name));
                        self.indent += 1;
                        self.push_scope();
                        self.emit_stmt(&args[1])?;
                        self.pop_scope()?;
                        let scopes_then = self.scopes.clone();
                        self.indent -= 1;
                        self.line("} else {");
                        self.indent += 1;
                        self.scopes = scopes_before.clone();
                        self.push_scope();
                        self.emit_stmt(&args[2])?;
                        self.pop_scope()?;
                        let scopes_else = self.scopes.clone();
                        self.indent -= 1;
                        self.line("}");

                        if then_ty == Ty::Never && else_ty == Ty::Never {
                            self.scopes = scopes_before;
                        } else if then_ty == Ty::Never {
                            self.scopes = scopes_else;
                        } else if else_ty == Ty::Never {
                            self.scopes = scopes_then;
                        } else {
                            self.scopes =
                                self.merge_if_states(&scopes_before, &scopes_then, &scopes_else)?;
                            self.recompute_borrow_counts()?;
                        }
                        Ok(())
                    }
                    "let" => self.emit_let_stmt(args),
                    "set" => self.emit_set_stmt(args),
                    "for" => {
                        let tmp = self.alloc_local("t_i32_")?;
                        self.decl_local(Ty::I32, &tmp);
                        self.emit_for_to(args, Ty::I32, &tmp)
                    }
                    "return" => self.emit_return(args),
                    _ => {
                        let _ = self.emit_expr(expr)?;
                        Ok(())
                    }
                }
            }
            _ => {
                let _ = self.emit_expr(expr)?;
                Ok(())
            }
        })();
        self.current_ptr = prev_ptr;
        out
    }

    pub(super) fn emit_let_stmt(&mut self, args: &[Expr]) -> Result<(), CompilerError> {
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

        if self.scopes.last().and_then(|s| s.get(name)).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("duplicate let binding in same scope: {name:?}"),
            ));
        }

        let expr_ty = self.infer_expr_in_new_scope(&args[1])?;
        let c_name = self.alloc_local("v_")?;
        self.decl_local(expr_ty.ty, &c_name);

        let mut var = self.make_var_ref(expr_ty.ty, c_name.clone(), false);
        var.brand = expr_ty.brand.clone();
        if is_owned_ty(expr_ty.ty) {
            match &args[1] {
                Expr::Ident {
                    name: src_name,
                    ptr: src_ptr,
                } if src_name != "input" => {
                    let Some(src) = self.lookup(src_name).cloned() else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {src_name:?}"),
                        ));
                    };
                    if (TyInfo {
                        ty: src.ty,
                        brand: src.brand.clone(),
                        view_full: false,
                    }) != expr_ty
                    {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("type mismatch in move: {src_name:?}"),
                        ));
                    }
                    if src.moved {
                        let moved_ptr = src
                            .moved_ptr
                            .as_deref()
                            .filter(|p| !p.is_empty())
                            .unwrap_or("<unknown>");
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("use after move: {src_name:?} moved_ptr={moved_ptr}"),
                        ));
                    }
                    if src.borrow_count != 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("move while borrowed: {src_name:?}"),
                        ));
                    }
                    self.line(&format!("{c_name} = {};", src.c_name));
                    self.line(&format!("{} = {};", src.c_name, c_empty(expr_ty.ty)));
                    if let Some(src_mut) = self.lookup_mut(src_name) {
                        src_mut.moved = true;
                        src_mut.moved_ptr = Some(src_ptr.clone());
                    }
                }
                _ => {
                    self.emit_expr_to(&args[1], expr_ty.ty, &c_name)?;
                }
            }
        } else {
            self.emit_expr_to(&args[1], expr_ty.ty, &c_name)?;
        }

        if is_view_like_ty(expr_ty.ty) {
            let borrow_of = self.borrow_of_view_like_expr(expr_ty.ty, &args[1])?;
            let borrow_ptr = borrow_of.as_ref().map(|_| args[1].ptr().to_string());
            if let Some(owner) = &borrow_of {
                self.inc_borrow_count(owner)?;
            }
            var.borrow_of = borrow_of;
            var.borrow_ptr = borrow_ptr;
        }

        self.bind(name.to_string(), var);
        Ok(())
    }

    pub(super) fn emit_set_stmt(&mut self, args: &[Expr]) -> Result<(), CompilerError> {
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
        let Some(dst) = self.lookup(name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("set of unknown variable: {name:?}"),
            ));
        };

        if is_owned_ty(dst.ty) {
            if dst.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "set while borrowed: {name:?}{}",
                        self.borrowed_by_diag_suffix(&dst.c_name)
                    ),
                ));
            }

            let tmp = self.alloc_local("t_set_")?;
            self.decl_local(dst.ty, &tmp);
            self.emit_expr_to(&args[1], dst.ty, &tmp)?;

            // Drop the previous value (unless it was moved-out during RHS evaluation).
            let moved = self.lookup(name).map(|v| v.moved).unwrap_or(false);
            if !moved {
                self.emit_drop_var(dst.ty, &dst.c_name);
            }
            self.line(&format!("{} = {};", dst.c_name, tmp));
            self.line(&format!("{tmp} = {};", c_empty(dst.ty)));

            if let Some(v) = self.lookup_mut(name) {
                v.moved = false;
                v.moved_ptr = None;
            }
        } else if is_view_like_ty(dst.ty) {
            let tmp = self.alloc_local("t_view_")?;
            self.decl_local(dst.ty, &tmp);
            self.emit_expr_to(&args[1], dst.ty, &tmp)?;
            let new_borrow_of = self.borrow_of_view_like_expr(dst.ty, &args[1])?;
            let new_borrow_ptr = new_borrow_of.as_ref().map(|_| args[1].ptr().to_string());

            let old_borrow_of = dst.borrow_of.clone();
            if let Some(owner) = &old_borrow_of {
                self.dec_borrow_count(owner)?;
            }
            if let Some(owner) = &new_borrow_of {
                self.inc_borrow_count(owner)?;
            }
            if let Some(v) = self.lookup_mut(name) {
                v.borrow_of = new_borrow_of;
                v.borrow_ptr = new_borrow_ptr;
            }

            self.line(&format!("{} = {tmp};", dst.c_name));
        } else {
            self.emit_expr_to(&args[1], dst.ty, &dst.c_name)?;
        }
        Ok(())
    }

    pub(super) fn emit_expr_to(
        &mut self,
        expr: &Expr,
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let prev_ptr = self.current_ptr.replace(expr.ptr().to_string());
        let out = (|| {
            self.line("rt_fuel(ctx, 1);");
            match expr {
                Expr::Int { value: i, .. } => {
                    if dest_ty != Ty::I32 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "int literal used where bytes expected".to_string(),
                        ));
                    }
                    let v = *i as u32;
                    self.line(&format!("{dest} = UINT32_C({v});"));
                    Ok(())
                }
                Expr::Ident { name, .. } => self.emit_ident_to(name, dest_ty, dest),
                Expr::List { items, .. } => self.emit_list_to(items, dest_ty, dest),
            }
        })();
        self.current_ptr = prev_ptr;
        out
    }

    pub(super) fn emit_ident_to(
        &mut self,
        name: &str,
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if name == "input" {
            if dest_ty != Ty::BytesView {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "input is bytes_view".to_string(),
                ));
            }
            self.line(&format!("{dest} = input;"));
            return Ok(());
        }

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
        if var.ty != dest_ty && !ty_compat_task_handle_as_i32(var.ty, dest_ty) {
            return Err(self.err(
                CompileErrorKind::Typing,
                format!("type mismatch for identifier {name:?}"),
            ));
        }
        if is_owned_ty(dest_ty) {
            if var.borrow_count != 0 {
                return Err(self.err(
                    CompileErrorKind::Typing,
                    format!("move while borrowed: {name:?}"),
                ));
            }
            self.line(&format!("{dest} = {};", var.c_name));
            self.line(&format!("{} = {};", var.c_name, c_empty(dest_ty)));
            let moved_ptr = self.current_ptr.clone();
            if let Some(v) = self.lookup_mut(name) {
                v.moved = true;
                v.moved_ptr = moved_ptr;
            }
        } else {
            self.line(&format!("{dest} = {};", var.c_name));
        }
        Ok(())
    }

    pub(super) fn emit_list_to(
        &mut self,
        items: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "list head must be an identifier".to_string(),
            )
        })?;
        let args = &items[1..];

        match head {
            "unsafe" => self.emit_unsafe_to(args, dest_ty, dest),
            "begin" => self.emit_begin_to(args, dest_ty, dest),
            "let" => self.emit_let_to(args, dest_ty, dest),
            "set" => self.emit_set_to(args, dest_ty, dest),
            "if" => self.emit_if_to(args, dest_ty, dest),
            "for" => self.emit_for_to(args, dest_ty, dest),
            "budget.scope_v1" => self.emit_budget_scope_v1_to(args, dest_ty, dest),
            "budget.scope_from_arch_v1" => {
                self.emit_budget_scope_from_arch_v1_to(args, dest_ty, dest)
            }
            "task.scope_v1" => self.emit_task_scope_v1_to(args, dest_ty, dest),
            "task.scope.slot_to_i32_v1" => {
                self.emit_task_scope_slot_to_i32_v1_to(args, dest_ty, dest)
            }
            "task.scope.slot_from_i32_v1" => {
                self.emit_task_scope_slot_from_i32_v1_to(args, dest_ty, dest)
            }
            "task.scope.select_v1" | "task.scope.select_try_v1" => {
                self.emit_task_scope_select_v1_to(head, args, dest_ty, dest)
            }
            "task.select_evt.tag_v1" => self.emit_task_select_evt_tag_v1_to(args, dest_ty, dest),
            "task.select_evt.case_index_v1" => {
                self.emit_task_select_evt_case_index_v1_to(args, dest_ty, dest)
            }
            "task.select_evt.src_id_v1" => {
                self.emit_task_select_evt_src_id_v1_to(args, dest_ty, dest)
            }
            "task.select_evt.take_bytes_v1" => {
                self.emit_task_select_evt_take_bytes_v1_to(args, dest_ty, dest)
            }
            "task.select_evt.drop_v1" => self.emit_task_select_evt_drop_v1_to(args, dest_ty, dest),
            "return" => self.emit_return(args),

            "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<u" | ">>u" | "=" | "!=" | "<"
            | "<=" | ">" | ">=" | "<u" | ">=u" | ">u" | "<=u" => {
                self.emit_binop_to(head, args, dest_ty, dest)
            }

            "bytes.len" => self.emit_bytes_len_to(args, dest_ty, dest),
            "bytes.get_u8" => self.emit_bytes_get_u8_to(args, dest_ty, dest),
            "bytes.set_u8" => self.emit_bytes_set_u8_to(args, dest_ty, dest),
            "bytes.alloc" => self.emit_bytes_alloc_to(args, dest_ty, dest),
            "bytes.empty" => self.emit_bytes_empty_to(args, dest_ty, dest),
            "bytes1" => self.emit_bytes1_to(args, dest_ty, dest),
            "bytes.lit" => self.emit_bytes_lit_to(args, dest_ty, dest),
            "bytes.slice" => self.emit_bytes_slice_to(args, dest_ty, dest),
            "bytes.copy" => self.emit_bytes_copy_to(args, dest_ty, dest),
            "bytes.concat" => self.emit_bytes_concat_to(args, dest_ty, dest),
            "bytes.eq" => self.emit_bytes_eq_to(args, dest_ty, dest),
            "bytes.cmp_range" => self.emit_bytes_cmp_range_to(args, dest_ty, dest),
            "bytes.as_ptr" => self.emit_bytes_as_ptr_to(args, dest_ty, dest),
            "bytes.as_mut_ptr" => self.emit_bytes_as_mut_ptr_to(args, dest_ty, dest),

            "math.f64.add_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_add_v1", args, dest_ty, dest)
            }
            "math.f64.sub_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_sub_v1", args, dest_ty, dest)
            }
            "math.f64.mul_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_mul_v1", args, dest_ty, dest)
            }
            "math.f64.div_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_div_v1", args, dest_ty, dest)
            }
            "math.f64.pow_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_pow_v1", args, dest_ty, dest)
            }
            "math.f64.atan2_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_atan2_v1", args, dest_ty, dest)
            }
            "math.f64.min_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_min_v1", args, dest_ty, dest)
            }
            "math.f64.max_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_max_v1", args, dest_ty, dest)
            }

            "math.f64.sqrt_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_sqrt_v1", args, dest_ty, dest)
            }
            "math.f64.neg_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_neg_v1", args, dest_ty, dest)
            }
            "math.f64.abs_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_abs_v1", args, dest_ty, dest)
            }
            "math.f64.sin_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_sin_v1", args, dest_ty, dest)
            }
            "math.f64.cos_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_cos_v1", args, dest_ty, dest)
            }
            "math.f64.tan_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_tan_v1", args, dest_ty, dest)
            }
            "math.f64.exp_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_exp_v1", args, dest_ty, dest)
            }
            "math.f64.log_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_ln_v1", args, dest_ty, dest)
            }
            "math.f64.floor_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_floor_v1", args, dest_ty, dest)
            }
            "math.f64.ceil_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_ceil_v1", args, dest_ty, dest)
            }
            "math.f64.fmt_shortest_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_fmt_shortest_v1", args, dest_ty, dest)
            }
            "math.f64.parse_v1" => self.emit_math_f64_parse_to(args, dest_ty, dest),
            "math.f64.from_i32_v1" => self.emit_math_f64_from_i32_to(args, dest_ty, dest),
            "math.f64.to_i32_trunc_v1" => self.emit_math_f64_to_i32_trunc_to(args, dest_ty, dest),
            "math.f64.to_bits_u64le_v1" => self.emit_math_f64_unop_to(
                head,
                "ev_math_f64_to_bits_u64le_v1",
                args,
                dest_ty,
                dest,
            ),
            "json.jcs.canon_doc_v1" => self.emit_json_jcs_canon_doc_v1_to(args, dest_ty, dest),

            "regex.compile_opts_v1" => self.emit_regex_compile_opts_v1_to(args, dest_ty, dest),
            "regex.exec_from_v1" => self.emit_regex_exec_from_v1_to(args, dest_ty, dest),
            "regex.exec_caps_from_v1" => self.emit_regex_exec_caps_from_v1_to(args, dest_ty, dest),
            "regex.find_all_x7sl_v1" => self.emit_regex_find_all_x7sl_v1_to(args, dest_ty, dest),
            "regex.split_v1" => self.emit_regex_split_v1_to(args, dest_ty, dest),
            "regex.replace_all_v1" => self.emit_regex_replace_all_v1_to(args, dest_ty, dest),

            "jsonschema.compile_v1" => self.emit_jsonschema_compile_v1_to(args, dest_ty, dest),
            "jsonschema.validate_v1" => self.emit_jsonschema_validate_v1_to(args, dest_ty, dest),

            "bytes.view" => self.emit_bytes_view_to(args, dest_ty, dest),
            "bytes.subview" => self.emit_bytes_subview_to(args, dest_ty, dest),
            "view.len" => self.emit_view_len_to(args, dest_ty, dest),
            "view.get_u8" => self.emit_view_get_u8_to(args, dest_ty, dest),
            "view.slice" => self.emit_view_slice_to(args, dest_ty, dest),
            "view.to_bytes" => self.emit_view_to_bytes_to(args, dest_ty, dest),
            "view.as_ptr" => self.emit_view_as_ptr_to(args, dest_ty, dest),
            "view.eq" => self.emit_view_eq_to(args, dest_ty, dest),
            "view.cmp_range" => self.emit_view_cmp_range_to(args, dest_ty, dest),

            "std.brand.erase_bytes_v1" => {
                self.emit_std_brand_erase_bytes_v1_to(args, dest_ty, dest)
            }
            "std.brand.erase_view_v1" => self.emit_std_brand_erase_view_v1_to(args, dest_ty, dest),
            "std.brand.view_v1" => self.emit_std_brand_view_v1_to(args, dest_ty, dest),
            "std.brand.assume_bytes_v1" => {
                self.emit_std_brand_assume_bytes_v1_to(args, dest_ty, dest)
            }
            "std.brand.cast_bytes_v1" => self.emit_std_brand_cast_bytes_v1_to(args, dest_ty, dest),
            "std.brand.cast_view_copy_v1" => {
                self.emit_std_brand_cast_view_copy_v1_to(args, dest_ty, dest)
            }
            "std.brand.cast_view_v1" => self.emit_std_brand_cast_view_v1_to(args, dest_ty, dest),
            "std.brand.to_bytes_preserve_if_full_v1" => {
                self.emit_std_brand_to_bytes_preserve_if_full_v1_to(args, dest_ty, dest)
            }
            "__internal.brand.assume_view_v1" => {
                self.emit_internal_brand_assume_view_v1_to(args, dest_ty, dest)
            }
            "__internal.brand.view_to_bytes_preserve_brand_v1" => {
                self.emit_internal_brand_view_to_bytes_preserve_brand_v1_to(args, dest_ty, dest)
            }
            "__internal.result_bytes.unwrap_ok_v1" => {
                self.emit_internal_result_bytes_unwrap_ok_v1_to(args, dest_ty, dest)
            }
            "__internal.bytes.alloc_aligned_v1" => {
                self.emit_internal_bytes_alloc_aligned_v1_to(args, dest_ty, dest)
            }
            "__internal.bytes.clone_v1" => {
                self.emit_internal_bytes_clone_v1_to(args, dest_ty, dest)
            }
            "__internal.bytes.drop_v1" => self.emit_internal_bytes_drop_v1_to(args, dest_ty, dest),
            "__internal.stream_xf.plugin_init_v1" => {
                self.emit_internal_stream_xf_plugin_init_v1_to(args, dest_ty, dest)
            }
            "__internal.stream_xf.plugin_step_v1" => {
                self.emit_internal_stream_xf_plugin_step_v1_to(args, dest_ty, dest)
            }
            "__internal.stream_xf.plugin_flush_v1" => {
                self.emit_internal_stream_xf_plugin_flush_v1_to(args, dest_ty, dest)
            }

            "vec_u8.as_ptr" => self.emit_vec_u8_as_ptr_to(args, dest_ty, dest),
            "vec_u8.as_mut_ptr" => self.emit_vec_u8_as_mut_ptr_to(args, dest_ty, dest),

            "ptr.null" => self.emit_ptr_null_to(args, dest_ty, dest),
            "ptr.as_const" => self.emit_ptr_as_const_to(args, dest_ty, dest),
            "ptr.cast" => self.emit_ptr_cast_to(args, dest_ty, dest),

            "addr_of" => self.emit_addr_of_to(args, dest_ty, dest),
            "addr_of_mut" => self.emit_addr_of_mut_to(args, dest_ty, dest),

            "ptr.add" => self.emit_ptr_add_to(args, dest_ty, dest),
            "ptr.sub" => self.emit_ptr_sub_to(args, dest_ty, dest),
            "ptr.offset" => self.emit_ptr_offset_to(args, dest_ty, dest),
            "ptr.read_u8" => self.emit_ptr_read_u8_to(args, dest_ty, dest),
            "ptr.write_u8" => self.emit_ptr_write_u8_to(args, dest_ty, dest),
            "ptr.read_i32" => self.emit_ptr_read_i32_to(args, dest_ty, dest),
            "ptr.write_i32" => self.emit_ptr_write_i32_to(args, dest_ty, dest),

            "memcpy" => self.emit_memcpy_to(args, dest_ty, dest),
            "memmove" => self.emit_memmove_to(args, dest_ty, dest),
            "memset" => self.emit_memset_to(args, dest_ty, dest),

            "await" => self.emit_task_await_to(args, dest_ty, dest),
            "task.spawn" => self.emit_task_spawn_to(args, dest_ty, dest),
            "task.is_finished" => self.emit_task_is_finished_to(args, dest_ty, dest),
            "task.try_join.bytes" => self.emit_task_try_join_bytes_to(args, dest_ty, dest),
            "task.try_join.result_bytes" => {
                self.emit_task_try_join_result_bytes_to(args, dest_ty, dest)
            }
            "task.join.bytes" => self.emit_task_join_bytes_to(args, dest_ty, dest),
            "task.join.result_bytes" => self.emit_task_join_result_bytes_to(args, dest_ty, dest),
            "task.yield" => self.emit_task_yield_to(args, dest_ty, dest),
            "task.sleep" => self.emit_task_sleep_to(args, dest_ty, dest),
            "task.cancel" => self.emit_task_cancel_to(args, dest_ty, dest),

            "task.scope.start_soon_v1" => {
                self.emit_task_scope_start_soon_v1_to(args, dest_ty, dest)
            }
            "task.scope.cancel_all_v1" => {
                self.emit_task_scope_cancel_all_v1_to(args, dest_ty, dest)
            }
            "task.scope.wait_all_v1" => self.emit_task_scope_wait_all_v1_to(args, dest_ty, dest),
            "task.scope.async_let_bytes_v1" => self.emit_task_scope_async_let_v1_to(
                head,
                args,
                dest_ty,
                dest,
                Ty::TaskHandleBytesV1,
                "RT_TASK_OUT_KIND_BYTES",
            ),
            "task.scope.async_let_result_bytes_v1" => self.emit_task_scope_async_let_v1_to(
                head,
                args,
                dest_ty,
                dest,
                Ty::TaskHandleResultBytesV1,
                "RT_TASK_OUT_KIND_RESULT_BYTES",
            ),
            "task.scope.await_slot_bytes_v1" => {
                self.emit_task_scope_await_slot_bytes_v1_to(args, dest_ty, dest)
            }
            "task.scope.await_slot_result_bytes_v1" => {
                self.emit_task_scope_await_slot_result_bytes_v1_to(args, dest_ty, dest)
            }
            "task.scope.try_await_slot.bytes_v1" => {
                self.emit_task_scope_try_await_slot_bytes_v1_to(args, dest_ty, dest)
            }
            "task.scope.try_await_slot.result_bytes_v1" => {
                self.emit_task_scope_try_await_slot_result_bytes_v1_to(args, dest_ty, dest)
            }
            "task.scope.slot_is_finished_v1" => {
                self.emit_task_scope_slot_is_finished_v1_to(args, dest_ty, dest)
            }

            "chan.bytes.new" => self.emit_chan_bytes_new_to(args, dest_ty, dest),
            "chan.bytes.try_send" => self.emit_chan_bytes_try_send_to(args, dest_ty, dest),
            "chan.bytes.send" => self.emit_chan_bytes_send_to(args, dest_ty, dest),
            "chan.bytes.try_recv" => self.emit_chan_bytes_try_recv_to(args, dest_ty, dest),
            "chan.bytes.recv" => self.emit_chan_bytes_recv_to(args, dest_ty, dest),
            "chan.bytes.close" => self.emit_chan_bytes_close_to(args, dest_ty, dest),

            "fs.read" => self.emit_fs_read_to(args, dest_ty, dest),
            "fs.read_async" => self.emit_fs_read_async_to(args, dest_ty, dest),
            "fs.open_read" => self.emit_fs_open_read_to(args, dest_ty, dest),
            "fs.list_dir" => self.emit_fs_list_dir_to(args, dest_ty, dest),

            "os.fs.read_file" => self.emit_os_fs_read_file_to(args, dest_ty, dest),
            "os.fs.write_file" => self.emit_os_fs_write_file_to(args, dest_ty, dest),
            "os.fs.read_all_v1" => self.emit_os_fs_read_all_v1_to(args, dest_ty, dest),
            "os.fs.write_all_v1" => self.emit_os_fs_write_all_v1_to(args, dest_ty, dest),
            "os.fs.stream_open_write_v1" => {
                self.emit_os_fs_stream_open_write_v1_to(args, dest_ty, dest)
            }
            "os.fs.stream_write_all_v1" => {
                self.emit_os_fs_stream_write_all_v1_to(args, dest_ty, dest)
            }
            "os.fs.stream_close_v1" => self.emit_os_fs_stream_close_v1_to(args, dest_ty, dest),
            "os.fs.stream_drop_v1" => self.emit_os_fs_stream_drop_v1_to(args, dest_ty, dest),
            "os.fs.mkdirs_v1" => self.emit_os_fs_mkdirs_v1_to(args, dest_ty, dest),
            "os.fs.remove_file_v1" => self.emit_os_fs_remove_file_v1_to(args, dest_ty, dest),
            "os.fs.remove_dir_all_v1" => self.emit_os_fs_remove_dir_all_v1_to(args, dest_ty, dest),
            "os.fs.rename_v1" => self.emit_os_fs_rename_v1_to(args, dest_ty, dest),
            "os.fs.list_dir_sorted_text_v1" => {
                self.emit_os_fs_list_dir_sorted_text_v1_to(args, dest_ty, dest)
            }
            "os.fs.walk_glob_sorted_text_v1" => {
                self.emit_os_fs_walk_glob_sorted_text_v1_to(args, dest_ty, dest)
            }
            "os.fs.stat_v1" => self.emit_os_fs_stat_v1_to(args, dest_ty, dest),

            "os.stdio.read_line_v1" => self.emit_os_stdio_read_line_v1_to(args, dest_ty, dest),
            "os.stdio.write_stdout_v1" => {
                self.emit_os_stdio_write_stdout_v1_to(args, dest_ty, dest)
            }
            "os.stdio.write_stderr_v1" => {
                self.emit_os_stdio_write_stderr_v1_to(args, dest_ty, dest)
            }
            "os.stdio.flush_stdout_v1" => {
                self.emit_os_stdio_flush_stdout_v1_to(args, dest_ty, dest)
            }
            "os.stdio.flush_stderr_v1" => {
                self.emit_os_stdio_flush_stderr_v1_to(args, dest_ty, dest)
            }

            "os.rand.bytes_v1" => self.emit_os_rand_bytes_v1_to(args, dest_ty, dest),
            "os.rand.u64_v1" => self.emit_os_rand_u64_v1_to(args, dest_ty, dest),

            "os.db.sqlite.open_v1" => self.emit_os_db_sqlite_open_v1_to(args, dest_ty, dest),
            "os.db.sqlite.query_v1" => self.emit_os_db_sqlite_query_v1_to(args, dest_ty, dest),
            "os.db.sqlite.exec_v1" => self.emit_os_db_sqlite_exec_v1_to(args, dest_ty, dest),
            "os.db.sqlite.close_v1" => self.emit_os_db_sqlite_close_v1_to(args, dest_ty, dest),
            "os.db.pg.open_v1" => self.emit_os_db_pg_open_v1_to(args, dest_ty, dest),
            "os.db.pg.query_v1" => self.emit_os_db_pg_query_v1_to(args, dest_ty, dest),
            "os.db.pg.exec_v1" => self.emit_os_db_pg_exec_v1_to(args, dest_ty, dest),
            "os.db.pg.close_v1" => self.emit_os_db_pg_close_v1_to(args, dest_ty, dest),
            "os.db.mysql.open_v1" => self.emit_os_db_mysql_open_v1_to(args, dest_ty, dest),
            "os.db.mysql.query_v1" => self.emit_os_db_mysql_query_v1_to(args, dest_ty, dest),
            "os.db.mysql.exec_v1" => self.emit_os_db_mysql_exec_v1_to(args, dest_ty, dest),
            "os.db.mysql.close_v1" => self.emit_os_db_mysql_close_v1_to(args, dest_ty, dest),
            "os.db.redis.open_v1" => self.emit_os_db_redis_open_v1_to(args, dest_ty, dest),
            "os.db.redis.cmd_v1" => self.emit_os_db_redis_cmd_v1_to(args, dest_ty, dest),
            "os.db.redis.close_v1" => self.emit_os_db_redis_close_v1_to(args, dest_ty, dest),
            "os.env.get" => self.emit_os_env_get_to(args, dest_ty, dest),
            "os.time.now_unix_ms" => self.emit_os_time_now_unix_ms_to(args, dest_ty, dest),
            "os.time.now_instant_v1" => self.emit_os_time_now_instant_v1_to(args, dest_ty, dest),
            "os.time.sleep_ms_v1" => self.emit_os_time_sleep_ms_v1_to(args, dest_ty, dest),
            "os.time.local_tzid_v1" => self.emit_os_time_local_tzid_v1_to(args, dest_ty, dest),
            "os.time.tzdb_is_valid_tzid_v1" => {
                self.emit_os_time_tzdb_is_valid_tzid_v1_to(args, dest_ty, dest)
            }
            "os.time.tzdb_offset_duration_v1" => {
                self.emit_os_time_tzdb_offset_duration_v1_to(args, dest_ty, dest)
            }
            "os.time.tzdb_snapshot_id_v1" => {
                self.emit_os_time_tzdb_snapshot_id_v1_to(args, dest_ty, dest)
            }
            "process.set_exit_code_v1" => {
                self.emit_process_set_exit_code_v1_to(args, dest_ty, dest)
            }
            "os.process.exit" => self.emit_os_process_exit_to(args, dest_ty, dest),
            "os.process.spawn_capture_v1" => {
                self.emit_os_process_spawn_capture_v1_to(args, dest_ty, dest)
            }
            "os.process.spawn_piped_v1" => {
                self.emit_os_process_spawn_piped_v1_to(args, dest_ty, dest)
            }
            "os.process.try_join_capture_v1" => {
                self.emit_os_process_try_join_capture_v1_to(args, dest_ty, dest)
            }
            "os.process.join_capture_v1" | "std.os.process.join_capture_v1" => {
                self.emit_os_process_join_capture_v1_to(args, dest_ty, dest)
            }
            "os.process.stdout_read_v1" => {
                self.emit_os_process_stdout_read_v1_to(args, dest_ty, dest)
            }
            "os.process.stderr_read_v1" => {
                self.emit_os_process_stderr_read_v1_to(args, dest_ty, dest)
            }
            "os.process.stdin_write_v1" => {
                self.emit_os_process_stdin_write_v1_to(args, dest_ty, dest)
            }
            "os.process.stdin_close_v1" => {
                self.emit_os_process_stdin_close_v1_to(args, dest_ty, dest)
            }
            "os.process.try_wait_v1" => self.emit_os_process_try_wait_v1_to(args, dest_ty, dest),
            "os.process.join_exit_v1" | "std.os.process.join_exit_v1" => {
                self.emit_os_process_join_exit_v1_to(args, dest_ty, dest)
            }
            "os.process.take_exit_v1" => self.emit_os_process_take_exit_v1_to(args, dest_ty, dest),
            "os.process.kill_v1" => self.emit_os_process_kill_v1_to(args, dest_ty, dest),
            "os.process.drop_v1" => self.emit_os_process_drop_v1_to(args, dest_ty, dest),
            "os.process.run_capture_v1" => {
                self.emit_os_process_run_capture_v1_to(args, dest_ty, dest)
            }
            "os.net.http_request" => self.emit_os_net_http_request_to(args, dest_ty, dest),

            "rr.open_v1" => self.emit_rr_open_v1_to(args, dest_ty, dest),
            "rr.close_v1" => self.emit_rr_close_v1_to(args, dest_ty, dest),
            "rr.stats_v1" => self.emit_rr_stats_v1_to(args, dest_ty, dest),
            "rr.next_v1" => self.emit_rr_next_v1_to(args, dest_ty, dest),
            "rr.append_v1" => self.emit_rr_append_v1_to(args, dest_ty, dest),
            "rr.current_v1" => self.emit_rr_current_v1_to(args, dest_ty, dest),
            "rr.entry_resp_v1" => self.emit_rr_entry_resp_v1_to(args, dest_ty, dest),
            "rr.entry_err_v1" => self.emit_rr_entry_err_v1_to(args, dest_ty, dest),
            "std.rr.with_v1" => self.emit_std_rr_with_v1_to(args, dest_ty, dest),
            "std.rr.with_policy_v1" => self.emit_std_rr_with_policy_v1_to(args, dest_ty, dest),
            "kv.get" => self.emit_kv_get_to(args, dest_ty, dest),
            "kv.get_async" => self.emit_kv_get_async_to(args, dest_ty, dest),
            "kv.get_stream" => self.emit_kv_get_stream_to(args, dest_ty, dest),
            "kv.set" => self.emit_kv_set_to(args, dest_ty, dest),

            "io.open_read_bytes" => self.emit_io_open_read_bytes_to(args, dest_ty, dest),
            "io.read" => self.emit_io_read_to(args, dest_ty, dest),
            "iface.make_v1" => self.emit_iface_make_v1_to(args, dest_ty, dest),
            "bufread.new" => self.emit_bufread_new_to(args, dest_ty, dest),
            "bufread.fill" => self.emit_bufread_fill_to(args, dest_ty, dest),
            "bufread.consume" => self.emit_bufread_consume_to(args, dest_ty, dest),
            "scratch_u8_fixed_v1.new" => self.emit_scratch_u8_fixed_new_to(args, dest_ty, dest),
            "scratch_u8_fixed_v1.clear" => self.emit_scratch_u8_fixed_clear_to(args, dest_ty, dest),
            "scratch_u8_fixed_v1.len" => self.emit_scratch_u8_fixed_len_to(args, dest_ty, dest),
            "scratch_u8_fixed_v1.cap" => self.emit_scratch_u8_fixed_cap_to(args, dest_ty, dest),
            "scratch_u8_fixed_v1.as_view" => {
                self.emit_scratch_u8_fixed_as_view_to(args, dest_ty, dest)
            }
            "scratch_u8_fixed_v1.try_write" => {
                self.emit_scratch_u8_fixed_try_write_to(args, dest_ty, dest)
            }
            "scratch_u8_fixed_v1.drop" => self.emit_scratch_u8_fixed_drop_to(args, dest_ty, dest),

            "codec.read_u32_le" => self.emit_codec_read_u32_le_to(args, dest_ty, dest),
            "codec.write_u32_le" => self.emit_codec_write_u32_le_to(args, dest_ty, dest),
            "fmt.u32_to_dec" => self.emit_fmt_u32_to_dec_to(args, dest_ty, dest),
            "fmt.s32_to_dec" => self.emit_fmt_s32_to_dec_to(args, dest_ty, dest),
            "parse.u32_dec" => self.emit_parse_u32_dec_to(args, dest_ty, dest),
            "parse.u32_dec_at" => self.emit_parse_u32_dec_at_to(args, dest_ty, dest),
            "prng.lcg_next_u32" => self.emit_prng_lcg_next_u32_to(args, dest_ty, dest),

            "vec_u8.with_capacity" => self.emit_vec_u8_new_to(args, dest_ty, dest),
            "vec_u8.len" => self.emit_vec_u8_len_to(args, dest_ty, dest),
            "vec_u8.cap" => self.emit_vec_u8_cap_to(args, dest_ty, dest),
            "vec_u8.clear" => self.emit_vec_u8_clear_to(args, dest_ty, dest),
            "vec_u8.get" => self.emit_vec_u8_get_to(args, dest_ty, dest),
            "vec_u8.set" => self.emit_vec_u8_set_to(args, dest_ty, dest),
            "vec_u8.push" => self.emit_vec_u8_push_to(args, dest_ty, dest),
            "vec_u8.reserve_exact" => self.emit_vec_u8_reserve_exact_to(args, dest_ty, dest),
            "vec_u8.extend_zeroes" => self.emit_vec_u8_extend_zeroes_to(args, dest_ty, dest),
            "vec_u8.extend_bytes" => self.emit_vec_u8_extend_bytes_to(args, dest_ty, dest),
            "vec_u8.extend_bytes_range" => {
                self.emit_vec_u8_extend_bytes_range_to(args, dest_ty, dest)
            }
            "vec_u8.into_bytes" => self.emit_vec_u8_into_bytes_to(args, dest_ty, dest),
            "vec_u8.as_view" => self.emit_vec_u8_as_view_to(args, dest_ty, dest),

            "option_i32.none" => self.emit_option_i32_none_to(args, dest_ty, dest),
            "option_i32.some" => self.emit_option_i32_some_to(args, dest_ty, dest),
            "option_i32.is_some" => self.emit_option_i32_is_some_to(args, dest_ty, dest),
            "option_i32.unwrap_or" => self.emit_option_i32_unwrap_or_to(args, dest_ty, dest),

            "option_bytes.none" => self.emit_option_bytes_none_to(args, dest_ty, dest),
            "option_bytes.some" => self.emit_option_bytes_some_to(args, dest_ty, dest),
            "option_bytes.is_some" => self.emit_option_bytes_is_some_to(args, dest_ty, dest),
            "option_bytes.unwrap_or" => self.emit_option_bytes_unwrap_or_to(args, dest_ty, dest),

            "option_bytes_view.none" => self.emit_option_bytes_view_none_to(args, dest_ty, dest),
            "option_bytes_view.some" => self.emit_option_bytes_view_some_to(args, dest_ty, dest),
            "option_bytes_view.is_some" => {
                self.emit_option_bytes_view_is_some_to(args, dest_ty, dest)
            }
            "option_bytes_view.unwrap_or" => {
                self.emit_option_bytes_view_unwrap_or_to(args, dest_ty, dest)
            }

            "result_i32.ok" => self.emit_result_i32_ok_to(args, dest_ty, dest),
            "result_i32.err" => self.emit_result_i32_err_to(args, dest_ty, dest),
            "result_i32.is_ok" => self.emit_result_i32_is_ok_to(args, dest_ty, dest),
            "result_i32.err_code" => self.emit_result_i32_err_code_to(args, dest_ty, dest),
            "result_i32.unwrap_or" => self.emit_result_i32_unwrap_or_to(args, dest_ty, dest),

            "result_bytes.ok" => self.emit_result_bytes_ok_to(args, dest_ty, dest),
            "result_bytes.err" => self.emit_result_bytes_err_to(args, dest_ty, dest),
            "result_bytes.is_ok" => self.emit_result_bytes_is_ok_to(args, dest_ty, dest),
            "result_bytes.err_code" => self.emit_result_bytes_err_code_to(args, dest_ty, dest),
            "result_bytes.unwrap_or" => self.emit_result_bytes_unwrap_or_to(args, dest_ty, dest),

            "result_bytes_view.ok" => self.emit_result_bytes_view_ok_to(args, dest_ty, dest),
            "result_bytes_view.err" => self.emit_result_bytes_view_err_to(args, dest_ty, dest),
            "result_bytes_view.is_ok" => self.emit_result_bytes_view_is_ok_to(args, dest_ty, dest),
            "result_bytes_view.err_code" => {
                self.emit_result_bytes_view_err_code_to(args, dest_ty, dest)
            }
            "result_bytes_view.unwrap_or" => {
                self.emit_result_bytes_view_unwrap_or_to(args, dest_ty, dest)
            }

            "result_result_bytes.is_ok" => {
                self.emit_result_result_bytes_is_ok_to(args, dest_ty, dest)
            }
            "result_result_bytes.err_code" => {
                self.emit_result_result_bytes_err_code_to(args, dest_ty, dest)
            }
            "result_result_bytes.unwrap_or" => {
                self.emit_result_result_bytes_unwrap_or_to(args, dest_ty, dest)
            }

            "try" => self.emit_try_to(args, dest_ty, dest),

            "vec_value.with_capacity_v1" => {
                self.emit_vec_value_with_capacity_v1_to(args, dest_ty, dest)
            }
            "vec_value.len" => self.emit_vec_value_len_to(args, dest_ty, dest),
            "vec_value.reserve_exact" => self.emit_vec_value_reserve_exact_to(args, dest_ty, dest),
            "vec_value.pop" => self.emit_vec_value_pop_to(args, dest_ty, dest),
            "vec_value.clear" => self.emit_vec_value_clear_to(args, dest_ty, dest),
            h if h.starts_with("vec_value.push_") => {
                self.emit_vec_value_push_v1_to(h, args, dest_ty, dest)
            }
            h if h.starts_with("vec_value.get_") => {
                self.emit_vec_value_get_v1_to(h, args, dest_ty, dest)
            }
            h if h.starts_with("vec_value.set_") => {
                self.emit_vec_value_set_v1_to(h, args, dest_ty, dest)
            }

            "map_value.new_v1" => self.emit_map_value_new_v1_to(args, dest_ty, dest),
            "map_value.len" => self.emit_map_value_len_to(args, dest_ty, dest),
            "map_value.clear" => self.emit_map_value_clear_to(args, dest_ty, dest),
            h if h.starts_with("map_value.contains_") => {
                self.emit_map_value_contains_v1_to(h, args, dest_ty, dest)
            }
            h if h.starts_with("map_value.remove_") => {
                self.emit_map_value_remove_v1_to(h, args, dest_ty, dest)
            }
            h if h.starts_with("map_value.get_") => {
                self.emit_map_value_get_v1_to(h, args, dest_ty, dest)
            }
            h if h.starts_with("map_value.set_") => {
                self.emit_map_value_set_v1_to(h, args, dest_ty, dest)
            }

            "map_u32.new" | "set_u32.new" => self.emit_map_u32_new_to(args, dest_ty, dest),
            "map_u32.len" => self.emit_map_u32_len_to(args, dest_ty, dest),
            "map_u32.get" => self.emit_map_u32_get_to(args, dest_ty, dest),
            "map_u32.set" => self.emit_map_u32_set_to(args, dest_ty, dest),
            "map_u32.contains" | "set_u32.contains" => {
                self.emit_map_u32_contains_to(args, dest_ty, dest)
            }
            "map_u32.remove" | "set_u32.remove" => self.emit_map_u32_remove_to(args, dest_ty, dest),
            "set_u32.add" => self.emit_set_u32_add_to(args, dest_ty, dest),
            "set_u32.dump_u32le" => self.emit_set_u32_dump_u32le_to(args, dest_ty, dest),
            "map_u32.dump_kv_u32le_u32le" => {
                self.emit_map_u32_dump_kv_u32le_u32le_to(args, dest_ty, dest)
            }

            _ => {
                if self.fn_c_names.contains_key(head) {
                    self.emit_user_call_to(head, args, dest_ty, dest)
                } else if self.async_fn_new_names.contains_key(head) {
                    self.emit_async_call_to(head, args, dest_ty, dest)
                } else if self.extern_functions.contains_key(head) {
                    self.emit_extern_call_to(head, args, dest_ty, dest)
                } else {
                    Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!("unsupported head: {head:?}"),
                    ))
                }
            }
        }
    }

    pub(super) fn emit_begin_to(
        &mut self,
        exprs: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if exprs.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "(begin ...) requires at least 1 expression".to_string(),
            ));
        }

        self.push_scope();
        self.open_block();
        for e in &exprs[..exprs.len() - 1] {
            self.emit_stmt(e)?;
        }
        self.emit_expr_to(&exprs[exprs.len() - 1], dest_ty, dest)?;
        self.pop_scope()?;
        self.close_block();
        Ok(())
    }

    pub(super) fn emit_unsafe_to(
        &mut self,
        exprs: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.allow_unsafe() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "unsafe is not allowed in this world".to_string(),
            ));
        }
        let prev = self.unsafe_depth;
        self.unsafe_depth = self.unsafe_depth.saturating_add(1);
        let res = self.emit_begin_to(exprs, dest_ty, dest);
        self.unsafe_depth = prev;
        res
    }

    pub(super) fn emit_let_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
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

        if is_owned_ty(dest_ty) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "let expression cannot produce an owned value; use (begin (let ...) <var>)"
                    .to_string(),
            ));
        }

        if self.scopes.last().and_then(|s| s.get(name)).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("duplicate let binding in same scope: {name:?}"),
            ));
        }

        let expr_ty = self.infer_expr_in_new_scope(&args[1])?;
        if expr_ty != dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("let expression must match context type {dest_ty:?}"),
            ));
        }

        let c_name = self.alloc_local("v_")?;
        self.decl_local(expr_ty.ty, &c_name);
        self.emit_expr_to(&args[1], expr_ty.ty, &c_name)?;
        let mut var = self.make_var_ref(expr_ty.ty, c_name.clone(), false);
        var.brand = expr_ty.brand.clone();
        if is_view_like_ty(expr_ty.ty) {
            let borrow_of = self.borrow_of_view_like_expr(expr_ty.ty, &args[1])?;
            let borrow_ptr = borrow_of.as_ref().map(|_| args[1].ptr().to_string());
            if let Some(owner) = &borrow_of {
                self.inc_borrow_count(owner)?;
            }
            var.borrow_of = borrow_of;
            var.borrow_ptr = borrow_ptr;
        }
        self.bind(name.to_string(), var);
        self.line(&format!("{dest} = {c_name};"));
        Ok(())
    }

    pub(super) fn emit_set_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
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
        let Some(var) = self.lookup(name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("set of unknown variable: {name:?}"),
            ));
        };
        if var.ty != dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("set expression must match context type {dest_ty:?}"),
            ));
        }

        if is_owned_ty(dest_ty) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "set expression cannot produce an owned value; use (begin (set ...) <var>)"
                    .to_string(),
            ));
        }

        self.emit_set_stmt(args)?;
        self.line(&format!("{dest} = {};", var.c_name));
        Ok(())
    }

    pub(super) fn emit_if_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
            ));
        }

        let cond = self.emit_expr(&args[0])?;
        if cond.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "if condition must be i32".to_string(),
            ));
        }

        let then_ty = self.infer_expr_in_new_scope(&args[1])?;
        let else_ty = self.infer_expr_in_new_scope(&args[2])?;
        let dest_info = TyInfo::unbranded(dest_ty);
        let ok = if then_ty == Ty::Never && else_ty == Ty::Never {
            true
        } else if then_ty == Ty::Never {
            tyinfo_compat_assign(&else_ty, &dest_info)
        } else if else_ty == Ty::Never {
            tyinfo_compat_assign(&then_ty, &dest_info)
        } else {
            tyinfo_compat_assign(&then_ty, &dest_info) && tyinfo_compat_assign(&else_ty, &dest_info)
        };
        if !ok {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("if branches must match context type {dest_ty:?}"),
            ));
        }

        let scopes_before = self.scopes.clone();

        self.line(&format!("if ({} != UINT32_C(0)) {{", cond.c_name));
        self.indent += 1;
        self.push_scope();
        self.emit_expr_to(&args[1], dest_ty, dest)?;
        self.pop_scope()?;
        let scopes_then = self.scopes.clone();
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.scopes = scopes_before.clone();
        self.push_scope();
        self.emit_expr_to(&args[2], dest_ty, dest)?;
        self.pop_scope()?;
        let scopes_else = self.scopes.clone();
        self.indent -= 1;
        self.line("}");

        if then_ty == Ty::Never && else_ty == Ty::Never {
            self.scopes = scopes_before;
        } else if then_ty == Ty::Never {
            self.scopes = scopes_else;
        } else if else_ty == Ty::Never {
            self.scopes = scopes_then;
        } else {
            self.scopes = self.merge_if_states(&scopes_before, &scopes_then, &scopes_else)?;
            self.recompute_borrow_counts()?;
        }
        Ok(())
    }

    pub(super) fn emit_for_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "for form: (for <i> <start:i32> <end:i32> <body:any>)".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "for expression returns i32".to_string(),
            ));
        }

        let var_name = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "for variable must be an identifier".to_string(),
            )
        })?;

        let (var_c_name, var_ty) = match self.lookup(var_name) {
            Some(v) => (v.c_name.clone(), v.ty),
            None => {
                let c_name = self.alloc_local("v_")?;
                self.decl_local(Ty::I32, &c_name);
                self.bind(
                    var_name.to_string(),
                    self.make_var_ref(Ty::I32, c_name.clone(), false),
                );
                (c_name, Ty::I32)
            }
        };
        if var_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("for variable must be i32: {var_name:?}"),
            ));
        }

        // start (assigned to var)
        self.emit_expr_to(&args[1], Ty::I32, &var_c_name)?;

        // end (evaluated after start)
        let end_local = self.alloc_local("t_i32_")?;
        self.decl_local(Ty::I32, &end_local);
        self.emit_expr_to(&args[2], Ty::I32, &end_local)?;

        self.line("for (;;) {");
        self.indent += 1;
        self.line(&format!("if ({var_c_name} >= {end_local}) break;"));

        self.push_scope();
        self.open_block();
        self.emit_stmt(&args[3])?;
        self.pop_scope()?;
        self.close_block();

        self.line(&format!("{var_c_name} = {var_c_name} + UINT32_C(1);"));
        self.indent -= 1;
        self.line("}");

        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    pub(super) fn emit_return(&mut self, args: &[Expr]) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "return form: (return <expr>)".to_string(),
            ));
        }
        let scopes_snapshot = self.scopes.clone();
        let task_scopes_snapshot = self.task_scopes.clone();
        let cleanup_scopes_snapshot = self.cleanup_scopes.clone();
        let v = self.emit_expr(&args[0])?;
        if v.ty != self.fn_ret_ty && !ty_compat_task_handle_as_i32(v.ty, self.fn_ret_ty) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("return expression must evaluate to {:?}", self.fn_ret_ty),
            ));
        }

        for scope in cleanup_scopes_snapshot.iter().rev() {
            self.emit_unwind_cleanup_scope(scope, v.ty, &v.c_name);
        }

        self.emit_contract_exit_checks(&v)?;

        for (ty, c_name) in self.live_owned_drop_list(Some(&v.c_name)) {
            self.emit_drop_var(ty, &c_name);
        }
        self.line(&format!("return {};", v.c_name));
        // `return` terminates control flow. Moves/sets performed while evaluating the return
        // expression must not affect the remaining compilation state.
        self.scopes = scopes_snapshot;
        self.task_scopes = task_scopes_snapshot;
        self.cleanup_scopes = cleanup_scopes_snapshot;
        Ok(())
    }

    pub(super) fn emit_unwind_cleanup_scope(
        &mut self,
        scope: &CleanupScope,
        ret_ty: Ty,
        ret_c_name: &str,
    ) {
        match scope {
            CleanupScope::Task { c_name } => {
                self.line(&format!("rt_scope_exit_block(ctx, &{c_name});"));
            }
            CleanupScope::Budget { c_name } => {
                self.line(&format!("rt_budget_scope_exit_block(ctx, &{c_name});"));
                if matches!(
                    ret_ty,
                    Ty::ResultI32 | Ty::ResultBytes | Ty::ResultBytesView | Ty::ResultResultBytes
                ) {
                    self.line(&format!(
                        "if ({c_name}.mode == RT_BUDGET_MODE_RESULT_ERR && {c_name}.violated) {{"
                    ));
                    self.indent += 1;
                    self.emit_overwrite_result_with_err(
                        ret_ty,
                        ret_c_name,
                        &format!("{c_name}.err_code"),
                    );
                    self.indent -= 1;
                    self.line("}");
                }
            }
            CleanupScope::Rr {
                handle_c_name,
                prev_c_name,
            } => {
                self.line(&format!("ctx->rr_current = {prev_c_name};"));
                self.line(&format!("rt_rr_close_v1(ctx, {handle_c_name});"));
            }
        }
    }

    pub(super) fn emit_binop_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns i32"),
            ));
        }
        if (head == "<u" || head == ">=u")
            && matches!(args.get(1), Some(Expr::Int { value: 0, .. }))
        {
            let msg = match head {
                "<u" => {
                    "semantic error: `(<u x 0)` is always false (unsigned comparison). \
Use a signed comparison like `(< x 0)` when checking for negatives, or guard before decrementing."
                }
                ">=u" => {
                    "semantic error: `(>=u x 0)` is always true (unsigned comparison). \
Use a signed comparison like `(>= x 0)` when checking for negatives, or remove the check."
                }
                _ => unreachable!(),
            };
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                msg.to_string(),
            ));
        }
        let a = self.emit_expr(&args[0])?;
        let b = self.emit_expr(&args[1])?;
        if a.ty != Ty::I32 || b.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 args"),
            ));
        }
        match head {
            "+" => self.line(&format!("{dest} = {} + {};", a.c_name, b.c_name)),
            "-" => self.line(&format!("{dest} = {} - {};", a.c_name, b.c_name)),
            "*" => self.line(&format!("{dest} = {} * {};", a.c_name, b.c_name)),
            "/" => self.line(&format!(
                "{dest} = ({} == UINT32_C(0)) ? UINT32_C(0) : ({} / {});",
                b.c_name, a.c_name, b.c_name
            )),
            "%" => self.line(&format!(
                "{dest} = ({} == UINT32_C(0)) ? {} : ({} % {});",
                b.c_name, a.c_name, a.c_name, b.c_name
            )),
            "&" => self.line(&format!("{dest} = {} & {};", a.c_name, b.c_name)),
            "|" => self.line(&format!("{dest} = {} | {};", a.c_name, b.c_name)),
            "^" => self.line(&format!("{dest} = {} ^ {};", a.c_name, b.c_name)),
            "<<u" => self.line(&format!(
                "{dest} = {} << ({} & UINT32_C(31));",
                a.c_name, b.c_name
            )),
            ">>u" => self.line(&format!(
                "{dest} = {} >> ({} & UINT32_C(31));",
                a.c_name, b.c_name
            )),
            "=" => self.line(&format!("{dest} = ({} == {});", a.c_name, b.c_name)),
            "!=" => self.line(&format!("{dest} = ({} != {});", a.c_name, b.c_name)),
            "<" => self.line(&format!(
                "{dest} = (({} ^ UINT32_C(0x80000000)) < ({} ^ UINT32_C(0x80000000)));",
                a.c_name, b.c_name
            )),
            "<=" => self.line(&format!(
                "{dest} = (({} ^ UINT32_C(0x80000000)) >= ({} ^ UINT32_C(0x80000000)));",
                b.c_name, a.c_name
            )),
            ">" => self.line(&format!(
                "{dest} = (({} ^ UINT32_C(0x80000000)) < ({} ^ UINT32_C(0x80000000)));",
                b.c_name, a.c_name
            )),
            ">=" => self.line(&format!(
                "{dest} = (({} ^ UINT32_C(0x80000000)) >= ({} ^ UINT32_C(0x80000000)));",
                a.c_name, b.c_name
            )),
            "<u" => self.line(&format!("{dest} = ({} < {});", a.c_name, b.c_name)),
            ">=u" => self.line(&format!("{dest} = ({} >= {});", a.c_name, b.c_name)),
            ">u" => self.line(&format!("{dest} = ({} < {});", b.c_name, a.c_name)),
            "<=u" => self.line(&format!("{dest} = ({} >= {});", b.c_name, a.c_name)),
            _ => unreachable!(),
        }
        Ok(())
    }

    pub(super) fn emit_user_call_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let f = self
            .program
            .functions
            .iter()
            .find(|f| f.name == head)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    format!("internal error: missing function def for {head:?}"),
                )
            })?;

        if args.len() != f.params.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("call {:?} expects {} args", head, f.params.len()),
            ));
        }
        if dest_ty != f.ret_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("call {:?} returns {:?}", head, f.ret_ty),
            ));
        }

        let mut rendered_args = Vec::with_capacity(args.len());
        let mut arg_vals = Vec::with_capacity(args.len());
        for (i, (arg_expr, param)) in args.iter().zip(f.params.iter()).enumerate() {
            let v = match param.ty {
                Ty::BytesView => self.emit_expr_as_bytes_view(arg_expr)?,
                _ => self.emit_expr(arg_expr)?,
            };
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
            self.fn_c_name(head)
        ));
        for v in arg_vals {
            if is_owned_ty(v.ty) {
                self.line(&format!("{} = {};", v.c_name, c_empty(v.ty)));
            }
            self.release_temp_view_borrow(&v)?;
        }
        Ok(())
    }

    pub(super) fn emit_extern_call_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let f = self.extern_functions.get(head).cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                format!("internal error: missing extern decl for {head:?}"),
            )
        })?;

        if args.len() != f.params.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("call {:?} expects {} args", head, f.params.len()),
            ));
        }
        if dest_ty != f.ret_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("call {:?} returns {:?}", head, f.ret_ty),
            ));
        }

        let mut rendered_args = Vec::with_capacity(args.len());
        for (i, (arg_expr, param)) in args.iter().zip(f.params.iter()).enumerate() {
            let v = self.emit_expr(arg_expr)?;
            let want = param.ty;
            let ok = ty_compat_call_arg_extern(v.ty, want);
            if !ok {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("call {:?} arg {} expects {:?}", head, i, param.ty),
                ));
            }
            rendered_args.push(v.c_name);
        }
        let c_args = rendered_args.join(", ");

        if f.ret_is_void {
            self.line(&format!("{}({c_args});", f.link_name));
            self.line(&format!("{dest} = UINT32_C(0);"));
        } else {
            self.line(&format!("{dest} = {}({c_args});", f.link_name));
        }
        Ok(())
    }

    pub(super) fn fn_c_name(&self, name: &str) -> &str {
        self.fn_c_names
            .get(name)
            .map(|s| s.as_str())
            .unwrap_or("__x07_missing_fn")
    }

    pub(super) fn async_fn_new_c_name(&self, name: &str) -> &str {
        self.async_fn_new_names
            .get(name)
            .map(|s| s.as_str())
            .unwrap_or("__x07_missing_async_fn")
    }

    pub(super) fn emit_bytes_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.len returns i32".to_string(),
            ));
        }
        let v = self.emit_expr_as_bytes_view(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.len expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.len;", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_bytes_get_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.get_u8 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.get_u8 returns i32".to_string(),
            ));
        }
        let v = self.emit_expr_as_bytes_view(&args[0])?;
        let i = self.emit_expr(&args[1])?;
        if v.ty != Ty::BytesView || i.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.get_u8 expects (bytes_view, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_get_u8(ctx, {}, {});",
            v.c_name, i.c_name
        ));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_bytes_set_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.set_u8 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.set_u8 returns bytes".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("bytes.set_u8 while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::Bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
                ));
            }
            let i = self.emit_expr(&args[1])?;
            let v = self.emit_expr(&args[2])?;
            if i.ty != Ty::I32 || v.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_bytes_set_u8(ctx, {}, {}, {});",
                var_c_name, var_c_name, i.c_name, v.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::Bytes)));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
            }
            return Ok(());
        }

        let b = self.emit_expr(&args[0])?;
        let i = self.emit_expr(&args[1])?;
        let v = self.emit_expr(&args[2])?;
        if b.ty != Ty::Bytes || i.ty != Ty::I32 || v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bytes_set_u8(ctx, {}, {}, {});",
            b.c_name, i.c_name, v.c_name
        ));
        if dest != b.c_name.as_str() {
            self.line(&format!("{} = {};", b.c_name, c_empty(Ty::Bytes)));
        }
        Ok(())
    }

    pub(super) fn emit_bytes_alloc_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.alloc expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.alloc returns bytes".to_string(),
            ));
        }
        let len = self.emit_expr(&args[0])?;
        if len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.alloc length must be i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_alloc(ctx, {});", len.c_name));
        Ok(())
    }

    pub(super) fn emit_bytes_empty_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.empty expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.empty returns bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_empty(ctx);"));
        Ok(())
    }

    pub(super) fn emit_bytes1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes1 returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes1 expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_alloc(ctx, UINT32_C(1));"));
        self.line(&format!(
            "{dest} = rt_bytes_set_u8(ctx, {dest}, UINT32_C(0), {});",
            x.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_bytes_lit_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.lit expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
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

        self.tmp_counter += 1;
        let lit_name = format!("lit_{}", self.tmp_counter);
        let escaped = c_escape_string(lit_bytes);
        self.line(&format!("static const char {lit_name}[] = \"{escaped}\";"));
        self.line(&format!(
            "{dest} = rt_bytes_from_literal(ctx, (const uint8_t*){lit_name}, UINT32_C({}));",
            lit_bytes.len()
        ));
        Ok(())
    }

    pub(super) fn emit_bytes_slice_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.slice expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.slice returns bytes".to_string(),
            ));
        }
        let v = self.emit_expr_as_bytes_view(&args[0])?;
        let start = self.emit_expr(&args[1])?;
        let len = self.emit_expr(&args[2])?;
        if v.ty != Ty::BytesView || start.ty != Ty::I32 || len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.slice expects (bytes_view, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_to_bytes(ctx, rt_view_slice(ctx, {}, {}, {}));",
            v.c_name, start.c_name, len.c_name
        ));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_bytes_copy_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.copy expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.copy returns bytes".to_string(),
            ));
        }
        let src = self.emit_expr(&args[0])?;
        let dstb = self.emit_expr(&args[1])?;
        if src.ty != Ty::Bytes || dstb.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.copy expects (bytes, bytes)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bytes_copy(ctx, {}, {});",
            src.c_name, dstb.c_name
        ));
        if dest != dstb.c_name.as_str() {
            self.line(&format!("{} = rt_bytes_empty(ctx);", dstb.c_name));
        }
        Ok(())
    }

    pub(super) fn emit_bytes_concat_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.concat expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.concat returns bytes".to_string(),
            ));
        }
        let a = self.emit_expr(&args[0])?;
        let b = self.emit_expr(&args[1])?;
        if a.ty != Ty::Bytes || b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.concat expects (bytes, bytes)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bytes_concat(ctx, {}, {});",
            a.c_name, b.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_bytes_eq_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.eq expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.eq returns i32".to_string(),
            ));
        }
        let a = self.emit_expr_as_bytes_view(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        if a.ty != Ty::BytesView || b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.eq expects (bytes_view, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_eq(ctx, {}, {});",
            a.c_name, b.c_name
        ));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    pub(super) fn emit_bytes_cmp_range_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 6 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.cmp_range expects 6 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.cmp_range returns i32".to_string(),
            ));
        }
        let a = self.emit_expr_as_bytes_view(&args[0])?;
        let a_off = self.emit_expr(&args[1])?;
        let a_len = self.emit_expr(&args[2])?;
        let b = self.emit_expr_as_bytes_view(&args[3])?;
        let b_off = self.emit_expr(&args[4])?;
        let b_len = self.emit_expr(&args[5])?;
        if a.ty != Ty::BytesView
            || a_off.ty != Ty::I32
            || a_len.ty != Ty::I32
            || b.ty != Ty::BytesView
            || b_off.ty != Ty::I32
            || b_len.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_cmp_range(ctx, {}, {}, {}, {}, {}, {});",
            a.c_name, a_off.c_name, a_len.c_name, b.c_name, b_off.c_name, b_len.c_name
        ));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    pub(super) fn emit_bytes_as_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.as_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrConstU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.as_ptr returns ptr_const_u8".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.as_ptr expects bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.ptr;", b.c_name));
        Ok(())
    }

    pub(super) fn emit_bytes_as_mut_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.as_mut_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrMutU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.as_mut_ptr returns ptr_mut_u8".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.as_mut_ptr expects bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.ptr;", b.c_name));
        Ok(())
    }

    pub(super) fn emit_math_f64_binop_to(
        &mut self,
        head: &str,
        c_fn: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(native::BACKEND_ID_MATH, native::ABI_MAJOR_V1, head)?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns bytes"),
            ));
        }
        let a = self.emit_expr_as_bytes_view(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        if a.ty != Ty::BytesView || b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects (bytes_view, bytes_view)"),
            ));
        }
        let a_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            a.c_name, a.c_name
        );
        let b_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            b.c_name, b.c_name
        );
        self.line(&format!("{dest} = {c_fn}({a_expr}, {b_expr});"));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    pub(super) fn emit_math_f64_unop_to(
        &mut self,
        head: &str,
        c_fn: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(native::BACKEND_ID_MATH, native::ABI_MAJOR_V1, head)?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 1 arg"),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns bytes"),
            ));
        }
        let x = self.emit_expr_as_bytes_view(&args[0])?;
        if x.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects bytes_view"),
            ));
        }
        let x_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            x.c_name, x.c_name
        );
        self.line(&format!("{dest} = {c_fn}({x_expr});"));
        self.release_temp_view_borrow(&x)?;
        Ok(())
    }

    pub(super) fn emit_math_f64_parse_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_MATH,
            native::ABI_MAJOR_V1,
            "math.f64.parse_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "math.f64.parse_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.parse_v1 returns result_bytes".to_string(),
            ));
        }
        let s = self.emit_expr_as_bytes_view(&args[0])?;
        if s.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.parse_v1 expects bytes_view".to_string(),
            ));
        }
        let s_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            s.c_name, s.c_name
        );
        self.line(&format!("{dest} = ev_math_f64_parse_v1({s_expr});"));
        self.release_temp_view_borrow(&s)?;
        Ok(())
    }

    pub(super) fn emit_math_f64_from_i32_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_MATH,
            native::ABI_MAJOR_V1,
            "math.f64.from_i32_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "math.f64.from_i32_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.from_i32_v1 returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.from_i32_v1 expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ev_math_f64_from_i32_v1((int32_t){});",
            x.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_math_f64_to_i32_trunc_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_MATH,
            native::ABI_MAJOR_V1,
            "math.f64.to_i32_trunc_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "math.f64.to_i32_trunc_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.to_i32_trunc_v1 returns result_i32".to_string(),
            ));
        }
        let x = self.emit_expr_as_bytes_view(&args[0])?;
        if x.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.to_i32_trunc_v1 expects bytes_view".to_string(),
            ));
        }
        let x_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            x.c_name, x.c_name
        );
        self.line(&format!("{dest} = ev_math_f64_to_i32_trunc_v1({x_expr});"));
        self.release_temp_view_borrow(&x)?;
        Ok(())
    }

    pub(super) fn emit_bytes_view_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.view expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.view returns bytes_view".to_string(),
            ));
        }
        let Some(b_name) = args[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.view requires an identifier owner (bind the value to a local with let first)"
                    .to_string(),
            ));
        };
        let Some(b) = self.lookup(b_name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {b_name:?}"),
            ));
        };
        if b.moved {
            let moved_ptr = b
                .moved_ptr
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or("<unknown>");
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("use after move: {b_name:?} moved_ptr={moved_ptr}"),
            ));
        }
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.view expects bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_view(ctx, {});", b.c_name));
        Ok(())
    }

    pub(super) fn emit_bytes_subview_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.subview expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.subview returns bytes_view".to_string(),
            ));
        }
        let Some(b_name) = args[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.subview requires an identifier owner (bind the value to a local with let first)"
                    .to_string(),
            ));
        };
        let Some(b) = self.lookup(b_name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {b_name:?}"),
            ));
        };
        if b.moved {
            let moved_ptr = b
                .moved_ptr
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or("<unknown>");
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("use after move: {b_name:?} moved_ptr={moved_ptr}"),
            ));
        }
        let start = self.emit_expr(&args[1])?;
        let len = self.emit_expr(&args[2])?;
        if b.ty != Ty::Bytes || start.ty != Ty::I32 || len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.subview expects (bytes, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bytes_subview(ctx, {}, {}, {});",
            b.c_name, start.c_name, len.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_view_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.len returns i32".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.len expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.len;", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_view_get_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.get_u8 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.get_u8 returns i32".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        let i = self.emit_expr(&args[1])?;
        if v.ty != Ty::BytesView || i.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.get_u8 expects (bytes_view, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_get_u8(ctx, {}, {});",
            v.c_name, i.c_name
        ));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_view_slice_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.slice expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.slice returns bytes_view".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        let start = self.emit_expr(&args[1])?;
        let len = self.emit_expr(&args[2])?;
        if v.ty != Ty::BytesView || start.ty != Ty::I32 || len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.slice expects (bytes_view, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_slice(ctx, {}, {}, {});",
            v.c_name, start.c_name, len.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_view_to_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.to_bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.to_bytes returns bytes".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.to_bytes expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_view_to_bytes(ctx, {});", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_std_brand_erase_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.erase_bytes_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.erase_bytes_v1 returns bytes".to_string(),
            ));
        }
        self.emit_expr_to(&args[0], Ty::Bytes, dest)
    }

    pub(super) fn emit_std_brand_erase_view_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.erase_view_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.erase_view_v1 returns bytes_view".to_string(),
            ));
        }
        self.emit_expr_to(&args[0], Ty::BytesView, dest)
    }

    pub(super) fn emit_std_brand_view_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.view_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.view_v1 returns bytes_view".to_string(),
            ));
        }
        let Some(b_name) = args[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.view_v1 requires an identifier owner (bind the value to a local with let first)"
                    .to_string(),
            ));
        };
        let Some(b) = self.lookup(b_name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {b_name:?}"),
            ));
        };
        if b.moved {
            let moved_ptr = b
                .moved_ptr
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or("<unknown>");
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("use after move: {b_name:?} moved_ptr={moved_ptr}"),
            ));
        }
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.view_v1 expects bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_view(ctx, {});", b.c_name));
        Ok(())
    }

    pub(super) fn emit_std_brand_assume_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if self.unsafe_depth == 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "unsafe-required: std.brand.assume_bytes_v1".to_string(),
            ));
        }
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.assume_bytes_v1 expects 2 args".to_string(),
            ));
        }
        let _brand_id = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.assume_bytes_v1 expects a brand_id string".to_string(),
            )
        })?;
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.assume_bytes_v1 returns bytes".to_string(),
            ));
        }
        self.emit_expr_to(&args[1], Ty::Bytes, dest)
    }

    pub(super) fn emit_std_brand_cast_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_bytes_v1 expects 3 args".to_string(),
            ));
        }
        let _brand_id = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_bytes_v1 expects a brand_id string".to_string(),
            )
        })?;
        let validator_id = args[1].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_bytes_v1 expects a validator symbol".to_string(),
            )
        })?;
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.cast_bytes_v1 returns result_bytes".to_string(),
            ));
        }

        let b = self.emit_expr(&args[2])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.cast_bytes_v1 expects bytes".to_string(),
            ));
        }

        let v = self.alloc_local("t_view_")?;
        self.decl_local(Ty::BytesView, &v);
        self.line(&format!("{v} = rt_bytes_view(ctx, {});", b.c_name));

        let r = self.alloc_local("t_res_i32_")?;
        self.decl_local(Ty::ResultI32, &r);
        self.line(&format!(
            "{r} = {}(ctx, input, {v});",
            self.fn_c_name(validator_id)
        ));

        self.line(&format!("if (!{r}.tag) {{"));
        self.indent += 1;
        self.line(&format!("rt_bytes_drop(ctx, &{});", b.c_name));
        self.line(&format!("{} = rt_bytes_empty(ctx);", b.c_name));
        self.line(&format!(
            "{dest} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {r}.payload.err }};"
        ));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!(
            "{dest} = (result_bytes_t){{ .tag = UINT32_C(1), .payload.ok = {} }};",
            b.c_name
        ));
        self.line(&format!("{} = rt_bytes_empty(ctx);", b.c_name));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn emit_std_brand_cast_view_copy_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_view_copy_v1 expects 3 args".to_string(),
            ));
        }
        let _brand_id = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_view_copy_v1 expects a brand_id string".to_string(),
            )
        })?;
        let validator_id = args[1].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_view_copy_v1 expects a validator symbol".to_string(),
            )
        })?;
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.cast_view_copy_v1 returns result_bytes".to_string(),
            ));
        }

        let v = self.emit_expr(&args[2])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.cast_view_copy_v1 expects bytes_view".to_string(),
            ));
        }

        let r = self.alloc_local("t_res_i32_")?;
        self.decl_local(Ty::ResultI32, &r);
        self.line(&format!(
            "{r} = {}(ctx, input, {});",
            self.fn_c_name(validator_id),
            v.c_name
        ));

        self.line(&format!("if (!{r}.tag) {{"));
        self.indent += 1;
        self.line(&format!(
            "{dest} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {r}.payload.err }};"
        ));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        let out = self.alloc_local("t_bytes_")?;
        self.decl_local(Ty::Bytes, &out);
        self.line(&format!("{out} = rt_view_to_bytes(ctx, {});", v.c_name));
        self.line(&format!(
            "{dest} = (result_bytes_t){{ .tag = UINT32_C(1), .payload.ok = {out} }};"
        ));
        self.indent -= 1;
        self.line("}");
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_std_brand_cast_view_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_view_v1 expects 3 args".to_string(),
            ));
        }
        let _brand_id = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_view_v1 expects a brand_id string".to_string(),
            )
        })?;
        let validator_id = args[1].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.cast_view_v1 expects a validator symbol".to_string(),
            )
        })?;
        if dest_ty != Ty::ResultBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.cast_view_v1 returns result_bytes_view".to_string(),
            ));
        }

        let v = self.emit_expr(&args[2])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.cast_view_v1 expects bytes_view".to_string(),
            ));
        }

        let r = self.alloc_local("t_res_i32_")?;
        self.decl_local(Ty::ResultI32, &r);
        self.line(&format!(
            "{r} = {}(ctx, input, {});",
            self.fn_c_name(validator_id),
            v.c_name
        ));

        self.line(&format!("if (!{r}.tag) {{"));
        self.indent += 1;
        self.line(&format!(
            "{dest} = (result_bytes_view_t){{ .tag = UINT32_C(0), .payload.err = {r}.payload.err }};"
        ));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!(
            "{dest} = (result_bytes_view_t){{ .tag = UINT32_C(1), .payload.ok = {} }};",
            v.c_name
        ));
        self.indent -= 1;
        self.line("}");

        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_std_brand_to_bytes_preserve_if_full_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "std.brand.to_bytes_preserve_if_full_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.to_bytes_preserve_if_full_v1 returns bytes".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.brand.to_bytes_preserve_if_full_v1 expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_view_to_bytes(ctx, {});", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_internal_brand_assume_view_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.brand.assume_view_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.brand.assume_view_v1 returns bytes_view".to_string(),
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

        self.emit_expr_to(&args[1], Ty::BytesView, dest)
    }

    pub(super) fn emit_internal_brand_view_to_bytes_preserve_brand_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.brand.view_to_bytes_preserve_brand_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.brand.view_to_bytes_preserve_brand_v1 returns bytes".to_string(),
            ));
        }
        self.emit_view_to_bytes_to(args, dest_ty, dest)
    }

    pub(super) fn emit_internal_result_bytes_unwrap_ok_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.result_bytes.unwrap_ok_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.result_bytes.unwrap_ok_v1 returns bytes".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.result_bytes.unwrap_ok_v1 expects result_bytes".to_string(),
            ));
        }

        self.line(&format!("if ({}.tag == UINT32_C(1)) {{", res.c_name));
        self.indent += 1;
        self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
        self.line(&format!("{}.payload.ok = rt_bytes_empty(ctx);", res.c_name));
        self.line(&format!("{}.tag = UINT32_C(0);", res.c_name));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!("{dest} = rt_bytes_empty(ctx);"));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn emit_internal_bytes_alloc_aligned_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.bytes.alloc_aligned_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.bytes.alloc_aligned_v1 returns bytes".to_string(),
            ));
        }
        let len = self.emit_expr(&args[0])?;
        let align = self.emit_expr(&args[1])?;
        if len.ty != Ty::I32 || align.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.bytes.alloc_aligned_v1 expects (i32 len, i32 align)".to_string(),
            ));
        }

        self.line(&format!(
            "{dest} = rt_bytes_alloc_aligned(ctx, (uint32_t){}, (uint32_t){});",
            len.c_name, align.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_internal_bytes_clone_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.bytes.clone_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.bytes.clone_v1 returns bytes".to_string(),
            ));
        }
        match &args[0] {
            Expr::Ident { name, .. } if name != "input" => {
                let Some(var) = self.lookup(name).cloned() else {
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
                        "__internal.bytes.clone_v1 expects bytes".to_string(),
                    ));
                }
                self.line(&format!("{dest} = rt_bytes_clone(ctx, {});", var.c_name));
                Ok(())
            }
            _ => {
                let b = self.emit_expr(&args[0])?;
                if b.ty != Ty::Bytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "__internal.bytes.clone_v1 expects bytes".to_string(),
                    ));
                }
                self.line(&format!("{dest} = rt_bytes_clone(ctx, {});", b.c_name));
                Ok(())
            }
        }
    }

    pub(super) fn emit_internal_bytes_drop_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "__internal.bytes.drop_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "__internal.bytes.drop_v1 returns i32".to_string(),
            ));
        }

        match &args[0] {
            Expr::Ident { name, .. } => {
                let Some(var) = self.lookup(name).cloned() else {
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
                if var.borrow_count != 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("__internal.bytes.drop_v1 while borrowed: {name:?}"),
                    ));
                }
                if var.ty != Ty::Bytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "__internal.bytes.drop_v1 expects bytes".to_string(),
                    ));
                }
                self.line(&format!("rt_bytes_drop(ctx, &{});", var.c_name));
                self.line(&format!("{dest} = UINT32_C(0);"));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
                Ok(())
            }
            _ => {
                let b = self.emit_expr(&args[0])?;
                if b.ty != Ty::Bytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "__internal.bytes.drop_v1 expects bytes".to_string(),
                    ));
                }
                self.line(&format!("rt_bytes_drop(ctx, &{});", b.c_name));
                self.line(&format!("{dest} = UINT32_C(0);"));
                Ok(())
            }
        }
    }

    pub(super) fn emit_vec_value_with_capacity_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_value.with_capacity_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.with_capacity_v1 returns i32 handle".to_string(),
            ));
        }
        let ty_id = self.emit_expr(&args[0])?;
        let cap = self.emit_expr(&args[1])?;
        if ty_id.ty != Ty::I32 || cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.with_capacity_v1 expects (i32 ty_id, i32 cap)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_value_with_capacity_v1(ctx, {}, {});",
            ty_id.c_name, cap.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_vec_value_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_value.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.len returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.len expects i32 handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_value_len(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_value_reserve_exact_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_value.reserve_exact expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.reserve_exact returns i32 handle".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let additional = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || additional.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.reserve_exact expects (i32 handle, i32 additional)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_value_reserve_exact(ctx, {}, {});",
            h.c_name, additional.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_vec_value_pop_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_value.pop expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.pop returns i32 handle".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.pop expects i32 handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_value_pop(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_value_clear_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_value.clear expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.clear returns i32 handle".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_value.clear expects i32 handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_value_clear(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_value_push_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let Some(suffix) = parse_value_suffix_single(head, "vec_value.push_") else {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("unsupported head: {head:?}"),
            ));
        };
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns i32 handle"),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 handle"),
            ));
        }
        let want_x_ty = value_suffix_ty(suffix).expect("suffix validated by parse_value_suffix");
        let x = self.emit_expr(&args[1])?;
        if x.ty != want_x_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects ({want_x_ty:?})"),
            ));
        }
        let rt_fn = format!("rt_{}", head.replace('.', "_"));
        self.line(&format!(
            "{dest} = {rt_fn}(ctx, {}, {});",
            h.c_name, x.c_name
        ));
        match want_x_ty {
            Ty::Bytes => self.line(&format!("{} = {};", x.c_name, c_empty(Ty::Bytes))),
            Ty::BytesView => self.release_temp_view_borrow(&x)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn emit_vec_value_get_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let Some(suffix) = parse_value_suffix_single(head, "vec_value.get_") else {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("unsupported head: {head:?}"),
            ));
        };
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 3 args"),
            ));
        }
        let want_out_ty = value_suffix_ty(suffix).expect("suffix validated by parse_value_suffix");
        if dest_ty != want_out_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns {want_out_ty:?}"),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let idx = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || idx.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects (i32 handle, i32 idx, default)"),
            ));
        }
        let default = self.emit_expr(&args[2])?;
        if default.ty != want_out_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects default ({want_out_ty:?})"),
            ));
        }

        let rt_fn = format!("rt_{}", head.replace('.', "_"));
        self.line(&format!(
            "{dest} = {rt_fn}(ctx, {}, {}, {});",
            h.c_name, idx.c_name, default.c_name
        ));
        match want_out_ty {
            Ty::Bytes => {
                if dest != default.c_name.as_str() {
                    self.line(&format!("{} = {};", default.c_name, c_empty(Ty::Bytes)));
                }
            }
            Ty::BytesView => self.release_temp_view_borrow(&default)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn emit_vec_value_set_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let Some(suffix) = parse_value_suffix_single(head, "vec_value.set_") else {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("unsupported head: {head:?}"),
            ));
        };
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 3 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns i32 handle"),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let idx = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || idx.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects (i32 handle, i32 idx, x)"),
            ));
        }
        let want_x_ty = value_suffix_ty(suffix).expect("suffix validated by parse_value_suffix");
        let x = self.emit_expr(&args[2])?;
        if x.ty != want_x_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects x ({want_x_ty:?})"),
            ));
        }
        let rt_fn = format!("rt_{}", head.replace('.', "_"));
        self.line(&format!(
            "{dest} = {rt_fn}(ctx, {}, {}, {});",
            h.c_name, idx.c_name, x.c_name
        ));
        match want_x_ty {
            Ty::Bytes => self.line(&format!("{} = {};", x.c_name, c_empty(Ty::Bytes))),
            Ty::BytesView => self.release_temp_view_borrow(&x)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn emit_map_value_new_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_value.new_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_value.new_v1 returns i32 handle".to_string(),
            ));
        }
        let k_id = self.emit_expr(&args[0])?;
        let v_id = self.emit_expr(&args[1])?;
        let cap = self.emit_expr(&args[2])?;
        if k_id.ty != Ty::I32 || v_id.ty != Ty::I32 || cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_value.new_v1 expects (i32 k_id, i32 v_id, i32 cap_pow2)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_value_new_v1(ctx, {}, {}, {});",
            k_id.c_name, v_id.c_name, cap.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_map_value_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_value.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_value.len returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_value.len expects i32 handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_map_value_len(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_map_value_clear_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_value.clear expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_value.clear returns i32 handle".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_value.clear expects i32 handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_map_value_clear(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_map_value_contains_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let Some(k_suffix) = parse_value_suffix_single(head, "map_value.contains_") else {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("unsupported head: {head:?}"),
            ));
        };
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns i32"),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 handle"),
            ));
        }
        let want_k_ty = value_suffix_ty(k_suffix).expect("suffix validated by parse_value_suffix");
        let key = self.emit_expr(&args[1])?;
        if key.ty != want_k_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects key ({want_k_ty:?})"),
            ));
        }
        let rt_fn = format!("rt_{}", head.replace('.', "_"));
        self.line(&format!(
            "{dest} = {rt_fn}(ctx, {}, {});",
            h.c_name, key.c_name
        ));
        match want_k_ty {
            Ty::Bytes => self.line(&format!("{} = {};", key.c_name, c_empty(Ty::Bytes))),
            Ty::BytesView => self.release_temp_view_borrow(&key)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn emit_map_value_remove_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let Some(k_suffix) = parse_value_suffix_single(head, "map_value.remove_") else {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("unsupported head: {head:?}"),
            ));
        };
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns i32 handle"),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 handle"),
            ));
        }
        let want_k_ty = value_suffix_ty(k_suffix).expect("suffix validated by parse_value_suffix");
        let key = self.emit_expr(&args[1])?;
        if key.ty != want_k_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects key ({want_k_ty:?})"),
            ));
        }
        let rt_fn = format!("rt_{}", head.replace('.', "_"));
        self.line(&format!(
            "{dest} = {rt_fn}(ctx, {}, {});",
            h.c_name, key.c_name
        ));
        match want_k_ty {
            Ty::Bytes => self.line(&format!("{} = {};", key.c_name, c_empty(Ty::Bytes))),
            Ty::BytesView => self.release_temp_view_borrow(&key)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn emit_map_value_get_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let Some((k_suffix, v_suffix)) = parse_value_suffix_pair(head, "map_value.get_") else {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("unsupported head: {head:?}"),
            ));
        };
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 3 args"),
            ));
        }
        let want_k_ty = value_suffix_ty(k_suffix).expect("suffix validated by parse_value_suffix");
        let want_v_ty = value_suffix_ty(v_suffix).expect("suffix validated by parse_value_suffix");
        if dest_ty != want_v_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns {want_v_ty:?}"),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 handle"),
            ));
        }
        let key = self.emit_expr(&args[1])?;
        if key.ty != want_k_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects key ({want_k_ty:?})"),
            ));
        }
        let default = self.emit_expr(&args[2])?;
        if default.ty != want_v_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects default ({want_v_ty:?})"),
            ));
        }
        let rt_fn = format!("rt_{}", head.replace('.', "_"));
        self.line(&format!(
            "{dest} = {rt_fn}(ctx, {}, {}, {});",
            h.c_name, key.c_name, default.c_name
        ));
        match want_k_ty {
            Ty::Bytes => self.line(&format!("{} = {};", key.c_name, c_empty(Ty::Bytes))),
            Ty::BytesView => self.release_temp_view_borrow(&key)?,
            _ => {}
        }
        match want_v_ty {
            Ty::Bytes => {
                if dest != default.c_name.as_str() {
                    self.line(&format!("{} = {};", default.c_name, c_empty(Ty::Bytes)));
                }
            }
            Ty::BytesView => self.release_temp_view_borrow(&default)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn emit_map_value_set_v1_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let Some((k_suffix, v_suffix)) = parse_value_suffix_pair(head, "map_value.set_") else {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("unsupported head: {head:?}"),
            ));
        };
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 3 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns i32 handle"),
            ));
        }
        let want_k_ty = value_suffix_ty(k_suffix).expect("suffix validated by parse_value_suffix");
        let want_v_ty = value_suffix_ty(v_suffix).expect("suffix validated by parse_value_suffix");
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 handle"),
            ));
        }
        let key = self.emit_expr(&args[1])?;
        if key.ty != want_k_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects key ({want_k_ty:?})"),
            ));
        }
        let val = self.emit_expr(&args[2])?;
        if val.ty != want_v_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects val ({want_v_ty:?})"),
            ));
        }
        let rt_fn = format!("rt_{}", head.replace('.', "_"));
        self.line(&format!(
            "{dest} = {rt_fn}(ctx, {}, {}, {});",
            h.c_name, key.c_name, val.c_name
        ));
        match want_k_ty {
            Ty::Bytes => self.line(&format!("{} = {};", key.c_name, c_empty(Ty::Bytes))),
            Ty::BytesView => self.release_temp_view_borrow(&key)?,
            _ => {}
        }
        match want_v_ty {
            Ty::Bytes => self.line(&format!("{} = {};", val.c_name, c_empty(Ty::Bytes))),
            Ty::BytesView => self.release_temp_view_borrow(&val)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn parse_bytes_lit_text_arg(
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

    pub(super) fn parse_i32_lit_arg(&self, head: &str, arg: &Expr) -> Result<i32, CompilerError> {
        let Expr::Int { value, .. } = arg else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects integer literal"),
            ));
        };
        Ok(*value)
    }

    pub(super) fn emit_view_as_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.as_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrConstU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.as_ptr returns ptr_const_u8".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.as_ptr expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.ptr;", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_view_eq_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.eq expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.eq returns i32".to_string(),
            ));
        }
        let a = self.emit_expr(&args[0])?;
        let b = self.emit_expr(&args[1])?;
        if a.ty != Ty::BytesView || b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.eq expects (bytes_view, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_eq(ctx, {}, {});",
            a.c_name, b.c_name
        ));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    pub(super) fn emit_view_cmp_range_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 6 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.cmp_range expects 6 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.cmp_range returns i32".to_string(),
            ));
        }
        let a = self.emit_expr(&args[0])?;
        let a_off = self.emit_expr(&args[1])?;
        let a_len = self.emit_expr(&args[2])?;
        let b = self.emit_expr(&args[3])?;
        let b_off = self.emit_expr(&args[4])?;
        let b_len = self.emit_expr(&args[5])?;
        if a.ty != Ty::BytesView
            || a_off.ty != Ty::I32
            || a_len.ty != Ty::I32
            || b.ty != Ty::BytesView
            || b_off.ty != Ty::I32
            || b_len.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_cmp_range(ctx, {}, {}, {}, {}, {}, {});",
            a.c_name, a_off.c_name, a_len.c_name, b.c_name, b_off.c_name, b_len.c_name
        ));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    pub(super) fn emit_regex_compile_opts_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.compile_opts_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.compile_opts_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.compile_opts_v1 returns bytes".to_string(),
            ));
        }
        let pat = self.emit_expr(&args[0])?;
        let opts = self.emit_expr(&args[1])?;
        if pat.ty != Ty::BytesView || opts.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.compile_opts_v1 expects (bytes_view pat, i32 opts)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_compile_opts_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            pat.c_name, pat.c_name, opts.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_regex_exec_from_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.exec_from_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.exec_from_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.exec_from_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let start = self.emit_expr(&args[2])?;
        if compiled.ty != Ty::BytesView || text.ty != Ty::BytesView || start.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.exec_from_v1 expects (bytes_view compiled, bytes_view text, i32 start)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_exec_from_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, start.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_regex_exec_caps_from_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.exec_caps_from_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.exec_caps_from_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.exec_caps_from_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let start = self.emit_expr(&args[2])?;
        if compiled.ty != Ty::BytesView || text.ty != Ty::BytesView || start.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.exec_caps_from_v1 expects (bytes_view compiled, bytes_view text, i32 start)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_exec_caps_from_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, start.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_regex_find_all_x7sl_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.find_all_x7sl_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.find_all_x7sl_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.find_all_x7sl_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let max_matches = self.emit_expr(&args[2])?;
        if compiled.ty != Ty::BytesView || text.ty != Ty::BytesView || max_matches.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.find_all_x7sl_v1 expects (bytes_view compiled, bytes_view text, i32 max_matches)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_find_all_x7sl_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, max_matches.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_regex_split_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.split_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.split_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.split_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let max_parts = self.emit_expr(&args[2])?;
        if compiled.ty != Ty::BytesView || text.ty != Ty::BytesView || max_parts.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.split_v1 expects (bytes_view compiled, bytes_view text, i32 max_parts)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_split_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, max_parts.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_regex_replace_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.replace_all_v1",
        )?;
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.replace_all_v1 expects 4 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.replace_all_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let repl = self.emit_expr(&args[2])?;
        let cap_limit = self.emit_expr(&args[3])?;
        if compiled.ty != Ty::BytesView
            || text.ty != Ty::BytesView
            || repl.ty != Ty::BytesView
            || cap_limit.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.replace_all_v1 expects (bytes_view compiled, bytes_view text, bytes_view repl, i32 cap_limit)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_replace_all_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, repl.c_name, repl.c_name, cap_limit.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_jsonschema_compile_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_JSONSCHEMA,
            native::ABI_MAJOR_V1,
            "jsonschema.compile_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "jsonschema.compile_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "jsonschema.compile_v1 returns bytes".to_string(),
            ));
        }
        let schema_json = self.emit_expr(&args[0])?;
        if schema_json.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "jsonschema.compile_v1 expects bytes_view schema_json".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_jsonschema_compile_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }});",
            schema_json.c_name, schema_json.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_jsonschema_validate_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_JSONSCHEMA,
            native::ABI_MAJOR_V1,
            "jsonschema.validate_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "jsonschema.validate_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "jsonschema.validate_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let instance_json = self.emit_expr(&args[1])?;
        if compiled.ty != Ty::BytesView || instance_json.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "jsonschema.validate_v1 expects (bytes_view compiled, bytes_view instance_json)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_jsonschema_validate_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }});",
            compiled.c_name,
            compiled.c_name,
            instance_json.c_name,
            instance_json.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_process_set_exit_code_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "process.set_exit_code_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "process.set_exit_code_v1 returns i32".to_string(),
            ));
        }
        let code = self.emit_expr(&args[0])?;
        if code.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "process.set_exit_code_v1 expects i32 code".to_string(),
            ));
        }
        self.line(&format!("ctx->exit_code = (int32_t){};", code.c_name));
        self.line(&format!("{dest} = (int32_t){};", code.c_name));
        Ok(())
    }

    pub(super) fn emit_std_rr_with_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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

        let body_ty = self.infer_expr_in_new_scope(&args[1])?;
        if body_ty != dest_ty && body_ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("std.rr.with_v1 body must evaluate to {dest_ty:?} (or return)"),
            ));
        }

        self.emit_rr_with_cfg_expr_to(&args[0], &args[1], dest_ty, dest)
    }

    pub(super) fn emit_std_rr_with_policy_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
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

        let body_ty = self.infer_expr_in_new_scope(&args[3])?;
        if body_ty != dest_ty && body_ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("std.rr.with_policy_v1 body must evaluate to {dest_ty:?} (or return)"),
            ));
        }

        let policy_id = parse_bytes_lit_ascii(&args[0], "std.rr.with_policy_v1 policy_id")?;
        let cassette_path = parse_bytes_lit_ascii(&args[1], "std.rr.with_policy_v1 cassette_path")?;
        let mode_i32 = parse_i32_lit(&args[2], "std.rr.with_policy_v1 mode")?;

        let cfg = load_rr_cfg_v1_from_arch_v1(&self.options, &policy_id, &cassette_path, mode_i32)?;

        self.tmp_counter += 1;
        let cfg_name = format!("rr_cfg_{}", self.tmp_counter);
        let escaped = c_escape_string(cfg.as_slice());
        self.line(&format!("static const char {cfg_name}[] = \"{escaped}\";"));

        let cfg_bytes_name = self.alloc_local("t_rr_cfg_bytes_")?;
        self.decl_local(Ty::Bytes, &cfg_bytes_name);
        self.line(&format!(
            "{cfg_bytes_name} = rt_bytes_from_literal(ctx, (const uint8_t*){cfg_name}, UINT32_C({}));",
            cfg.len()
        ));

        let cfg_view_name = self.alloc_local("t_rr_cfg_view_")?;
        self.decl_local(Ty::BytesView, &cfg_view_name);
        self.line(&format!(
            "{cfg_view_name} = rt_bytes_view(ctx, {cfg_bytes_name});"
        ));

        let open_res = self.alloc_local("t_rr_open_")?;
        self.decl_local(Ty::ResultI32, &open_res);
        self.line(&format!(
            "{open_res} = rt_rr_open_v1(ctx, {cfg_view_name});"
        ));
        self.line(&format!("rt_bytes_drop(ctx, &{cfg_bytes_name});"));
        self.line(&format!("{cfg_bytes_name} = rt_bytes_empty(ctx);"));

        self.emit_rr_with_open_result_to(open_res.as_str(), &args[3], dest_ty, dest)?;
        Ok(())
    }

    pub(super) fn emit_io_read_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "io.read expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "io.read returns bytes".to_string(),
            ));
        }
        let reader = self.emit_expr(&args[0])?;
        let max = self.emit_expr(&args[1])?;
        if reader.ty != Ty::Iface || max.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "io.read expects (iface, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_iface_io_read_block(ctx, {}, {});",
            reader.c_name, max.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_iface_make_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "iface.make_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "iface.make_v1 returns iface".to_string(),
            ));
        }
        let data = self.emit_expr(&args[0])?;
        let vtable = self.emit_expr(&args[1])?;
        if data.ty != Ty::I32 || vtable.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "iface.make_v1 expects (i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (iface_t){{ .data = {}, .vtable = {} }};",
            data.c_name, vtable.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_io_open_read_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "io.open_read_bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "io.open_read_bytes returns iface".to_string(),
            ));
        }

        let b = self.emit_expr(&args[0])?;
        let b_expr = match b.ty {
            Ty::Bytes => b.c_name.clone(),
            Ty::BytesView => format!("rt_view_to_bytes(ctx, {})", b.c_name),
            Ty::VecU8 => format!("rt_vec_u8_into_bytes(ctx, &{})", b.c_name),
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "io.open_read_bytes expects bytes".to_string(),
                ))
            }
        };

        self.line(&format!(
            "{dest} = (iface_t){{ .data = rt_io_reader_new_bytes(ctx, {b_expr}, UINT32_C(0)), .vtable = RT_IFACE_VTABLE_IO_READER }};",
        ));

        if b.ty == Ty::Bytes {
            self.line(&format!("{} = {};", b.c_name, c_empty(Ty::Bytes)));
        }

        Ok(())
    }

    pub(super) fn emit_bufread_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bufread.new expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.new returns i32".to_string(),
            ));
        }
        let reader = self.emit_expr(&args[0])?;
        let cap = self.emit_expr(&args[1])?;
        if reader.ty != Ty::Iface || cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.new expects (iface, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bufread_new(ctx, {}, {});",
            reader.c_name, cap.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_bufread_fill_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bufread.fill expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.fill returns bytes_view".to_string(),
            ));
        }
        let br = self.emit_expr(&args[0])?;
        if br.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.fill expects i32 bufread handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bufread_fill_block(ctx, {});",
            br.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_bufread_consume_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bufread.consume expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.consume returns i32".to_string(),
            ));
        }
        let br = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if br.ty != Ty::I32 || n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.consume expects (i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bufread_consume(ctx, {}, {});",
            br.c_name, n.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_scratch_u8_fixed_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "scratch_u8_fixed_v1.new expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.new returns i32".to_string(),
            ));
        }
        let cap = self.emit_expr(&args[0])?;
        if cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.new expects i32 cap".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scratch_u8_fixed_new(ctx, {});",
            cap.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_scratch_u8_fixed_clear_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "scratch_u8_fixed_v1.clear expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.clear returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.clear expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scratch_u8_fixed_clear(ctx, {});",
            h.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_scratch_u8_fixed_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "scratch_u8_fixed_v1.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.len returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.len expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scratch_u8_fixed_len(ctx, {});",
            h.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_scratch_u8_fixed_cap_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "scratch_u8_fixed_v1.cap expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.cap returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.cap expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scratch_u8_fixed_cap(ctx, {});",
            h.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_scratch_u8_fixed_as_view_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "scratch_u8_fixed_v1.as_view expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.as_view returns bytes_view".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.as_view expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scratch_u8_fixed_as_view(ctx, {});",
            h.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_scratch_u8_fixed_try_write_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "scratch_u8_fixed_v1.try_write expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.try_write returns result_i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        if h.ty != Ty::I32 || b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.try_write expects (i32 handle, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scratch_u8_fixed_try_write(ctx, {}, {});",
            h.c_name, b.c_name
        ));
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    pub(super) fn emit_scratch_u8_fixed_drop_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "scratch_u8_fixed_v1.drop expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.drop returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "scratch_u8_fixed_v1.drop expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_scratch_u8_fixed_drop(ctx, {});",
            h.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_codec_read_u32_le_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "codec.read_u32_le expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.read_u32_le returns i32".to_string(),
            ));
        }
        let b = self.emit_expr_as_bytes_view(&args[0])?;
        let off = self.emit_expr(&args[1])?;
        if b.ty != Ty::BytesView || off.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.read_u32_le expects (bytes_view, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_codec_read_u32_le(ctx, {}, {});",
            b.c_name, off.c_name
        ));
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    pub(super) fn emit_codec_write_u32_le_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "codec.write_u32_le expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.write_u32_le returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.write_u32_le expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_codec_write_u32_le(ctx, {});",
            x.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_fmt_u32_to_dec_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "fmt.u32_to_dec expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fmt.u32_to_dec returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fmt.u32_to_dec expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_fmt_u32_to_dec(ctx, {});", x.c_name));
        Ok(())
    }

    pub(super) fn emit_fmt_s32_to_dec_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "fmt.s32_to_dec expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fmt.s32_to_dec returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fmt.s32_to_dec expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_fmt_s32_to_dec(ctx, {});", x.c_name));
        Ok(())
    }

    pub(super) fn emit_parse_u32_dec_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "parse.u32_dec expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "parse.u32_dec returns i32".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "parse.u32_dec expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_parse_u32_dec(ctx, {});", b.c_name));
        Ok(())
    }

    pub(super) fn emit_parse_u32_dec_at_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "parse.u32_dec_at expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "parse.u32_dec_at returns i32".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        let off = self.emit_expr(&args[1])?;
        if b.ty != Ty::BytesView || off.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "parse.u32_dec_at expects (bytes_view, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_parse_u32_dec_at(ctx, {}, {});",
            b.c_name, off.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_prng_lcg_next_u32_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "prng.lcg_next_u32 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "prng.lcg_next_u32 returns i32".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "prng.lcg_next_u32 expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_prng_lcg_next_u32({});", x.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_u8_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.with_capacity expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.with_capacity returns vec_u8".to_string(),
            ));
        }
        let cap = self.emit_expr(&args[0])?;
        if cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.with_capacity cap must be i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_u8_new(ctx, {});", cap.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_u8_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.len returns i32".to_string(),
            ));
        }
        if let Some(name) = args[0].as_ident() {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.len expects vec_u8".to_string(),
                ));
            }
            self.line(&format!("{dest} = rt_vec_u8_len(ctx, {});", var.c_name));
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.len expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_u8_len(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_u8_cap_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.cap expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.cap returns i32".to_string(),
            ));
        }
        if let Some(name) = args[0].as_ident() {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.cap expects vec_u8".to_string(),
                ));
            }
            self.line(&format!("{dest} = rt_vec_u8_cap(ctx, {});", var.c_name));
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.cap expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_u8_cap(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_u8_clear_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.clear expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.clear returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.clear while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.clear expects vec_u8".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{var_c_name} = rt_vec_u8_clear(ctx, {var_c_name});",
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
            }
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.clear expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_u8_clear(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_u8_get_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.get expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.get returns i32".to_string(),
            ));
        }
        if let Some(name) = args[0].as_ident() {
            let Some(var) = self.lookup(name).cloned() else {
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
            let idx = self.emit_expr(&args[1])?;
            if var.ty != Ty::VecU8 || idx.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.get expects (vec_u8, i32 index)".to_string(),
                ));
            }
            self.line(&format!(
                "{dest} = rt_vec_u8_get(ctx, {}, {});",
                var.c_name, idx.c_name
            ));
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let idx = self.emit_expr(&args[1])?;
        if h.ty != Ty::VecU8 || idx.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.get expects (vec_u8, i32 index)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_get(ctx, {}, {});",
            h.c_name, idx.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_vec_u8_set_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.set expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.set returns vec_u8".to_string(),
            ));
        }

        if let Expr::Ident { name, .. } = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "vec_u8.set while borrowed: {name:?}{}",
                        self.borrowed_by_diag_suffix(&var.c_name)
                    ),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.set expects (vec_u8, i32 index, i32 value)".to_string(),
                ));
            }
            let idx = self.emit_expr(&args[1])?;
            let val = self.emit_expr(&args[2])?;
            if idx.ty != Ty::I32 || val.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.set expects (vec_u8, i32 index, i32 value)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_set(ctx, {}, {}, {});",
                var_c_name, var_c_name, idx.c_name, val.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
            }
            return Ok(());
        }

        let v = self.emit_expr(&args[0])?;
        let idx = self.emit_expr(&args[1])?;
        let val = self.emit_expr(&args[2])?;
        if v.ty != Ty::VecU8 || idx.ty != Ty::I32 || val.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.set expects (vec_u8, i32 index, i32 value)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_set(ctx, {}, {}, {});",
            v.c_name, idx.c_name, val.c_name
        ));
        if dest != v.c_name.as_str() {
            self.line(&format!("{} = {};", v.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    pub(super) fn emit_vec_u8_push_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.push expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.push returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.push while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.push expects (vec_u8, i32 value)".to_string(),
                ));
            }
            let v = self.emit_expr(&args[1])?;
            if v.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.push expects (vec_u8, i32 value)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_push(ctx, {}, {});",
                var_c_name, var_c_name, v.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
            }
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let v = self.emit_expr(&args[1])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.push expects (vec_u8, i32 value)".to_string(),
            ));
        }
        if v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.push expects (vec_u8, i32 value)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_push(ctx, {}, {});",
            h.c_name, v.c_name
        ));
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    pub(super) fn emit_vec_u8_reserve_exact_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.reserve_exact expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.reserve_exact returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.reserve_exact while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
                ));
            }
            let n = self.emit_expr(&args[1])?;
            if n.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_reserve_exact(ctx, {}, {});",
                var_c_name, var_c_name, n.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
            }
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
            ));
        }
        if n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_reserve_exact(ctx, {}, {});",
            h.c_name, n.c_name
        ));
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    pub(super) fn emit_vec_u8_extend_zeroes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.extend_zeroes expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_zeroes returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.extend_zeroes while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
                ));
            }
            let n = self.emit_expr(&args[1])?;
            if n.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_extend_zeroes(ctx, {}, {});",
                var_c_name, var_c_name, n.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
            }
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if h.ty != Ty::VecU8 || n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_extend_zeroes(ctx, {}, {});",
            h.c_name, n.c_name
        ));
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    pub(super) fn emit_vec_u8_extend_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.extend_bytes expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.extend_bytes while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
                ));
            }
            let b = self.emit_expr_as_bytes_view(&args[1])?;
            if b.ty != Ty::BytesView {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_extend_bytes(ctx, {}, {});",
                var_c_name, var_c_name, b.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
            }
            self.release_temp_view_borrow(&b)?;
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
            ));
        }
        if b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_extend_bytes(ctx, {}, {});",
            h.c_name, b.c_name
        ));
        self.release_temp_view_borrow(&b)?;
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    pub(super) fn emit_vec_u8_extend_bytes_range_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.extend_bytes_range expects 4 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes_range returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
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
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.extend_bytes_range while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)"
                        .to_string(),
                ));
            }
            let b = self.emit_expr_as_bytes_view(&args[1])?;
            let start = self.emit_expr(&args[2])?;
            let len = self.emit_expr(&args[3])?;
            if b.ty != Ty::BytesView || start.ty != Ty::I32 || len.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)"
                        .to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_extend_bytes_range(ctx, {}, {}, {}, {});",
                var_c_name, var_c_name, b.c_name, start.c_name, len.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
            }
            self.release_temp_view_borrow(&b)?;
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        let start = self.emit_expr(&args[2])?;
        let len = self.emit_expr(&args[3])?;
        if b.ty != Ty::BytesView || start.ty != Ty::I32 || len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)"
                    .to_string(),
            ));
        }
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_extend_bytes_range(ctx, {}, {}, {}, {});",
            h.c_name, b.c_name, start.c_name, len.c_name
        ));
        self.release_temp_view_borrow(&b)?;
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    pub(super) fn emit_vec_u8_into_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.into_bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.into_bytes returns bytes".to_string(),
            ));
        }
        match &args[0] {
            Expr::Ident { name, .. } => {
                let Some(var) = self.lookup(name).cloned() else {
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
                if var.borrow_count != 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("vec_u8.into_bytes while borrowed: {name:?}"),
                    ));
                }
                if var.ty != Ty::VecU8 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "vec_u8.into_bytes expects vec_u8".to_string(),
                    ));
                }
                self.line(&format!(
                    "{dest} = rt_vec_u8_into_bytes(ctx, &{});",
                    var.c_name
                ));
                let moved_ptr = self.current_ptr.clone();
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                    v.moved_ptr = moved_ptr;
                }
                Ok(())
            }
            _ => {
                let h = self.emit_expr(&args[0])?;
                if h.ty != Ty::VecU8 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "vec_u8.into_bytes expects vec_u8".to_string(),
                    ));
                }
                self.line(&format!(
                    "{dest} = rt_vec_u8_into_bytes(ctx, &{});",
                    h.c_name
                ));
                Ok(())
            }
        }
    }

    pub(super) fn emit_vec_u8_as_view_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.as_view expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_view returns bytes_view".to_string(),
            ));
        }
        let Some(h_name) = args[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_view requires an identifier owner (bind the value to a local with let first)"
                    .to_string(),
            ));
        };
        let Some(h) = self.lookup(h_name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {h_name:?}"),
            ));
        };
        if h.moved {
            let moved_ptr = h
                .moved_ptr
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or("<unknown>");
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("use after move: {h_name:?} moved_ptr={moved_ptr}"),
            ));
        }
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_view expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_u8_as_view(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_u8_as_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.as_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrConstU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_ptr returns ptr_const_u8".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_ptr expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}).data;", h.c_name));
        Ok(())
    }

    pub(super) fn emit_vec_u8_as_mut_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.as_mut_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrMutU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_mut_ptr returns ptr_mut_u8".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_mut_ptr expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}).data;", h.c_name));
        Ok(())
    }

    pub(super) fn emit_ptr_null_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.null expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::PtrMutVoid {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.null returns ptr_mut_void".to_string(),
            ));
        }
        self.line(&format!("{dest} = NULL;"));
        Ok(())
    }

    pub(super) fn emit_ptr_as_const_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.as_const expects 1 arg".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let ok = matches!(
            (p.ty, dest_ty),
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
                "ptr.as_const expects a raw pointer and returns a const raw pointer".to_string(),
            ));
        }
        self.line(&format!("{dest} = {};", p.c_name));
        Ok(())
    }

    pub(super) fn emit_ptr_cast_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if target != dest_ty || !target.is_ptr_ty() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.cast target type must match the expression type".to_string(),
            ));
        }
        let p = self.emit_expr(&args[1])?;
        if !p.ty.is_ptr_ty() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.cast expects a raw pointer".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}){};", c_ret_ty(target), p.c_name));
        Ok(())
    }

    pub(super) fn emit_addr_of_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_addr_of_common(args, dest_ty, dest, false)
    }

    pub(super) fn emit_addr_of_mut_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_addr_of_common(args, dest_ty, dest, true)
    }

    pub(super) fn emit_addr_of_common(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
        is_mut: bool,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != want {
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
            "input".to_string()
        } else {
            let Some(var) = self.lookup(name).cloned() else {
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
            var.c_name
        };
        let cty = if is_mut { "void*" } else { "const void*" };
        self.line(&format!("{dest} = ({cty})&({lvalue});"));
        Ok(())
    }

    pub(super) fn emit_ptr_add_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_ptr_addsub_common("ptr.add", args, dest_ty, dest, false)
    }

    pub(super) fn emit_ptr_sub_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_ptr_addsub_common("ptr.sub", args, dest_ty, dest, true)
    }

    pub(super) fn emit_ptr_addsub_common(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
        is_sub: bool,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if p.ty != dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} pointer type mismatch"),
            ));
        }
        if n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 offset"),
            ));
        }
        let op = if is_sub { "-" } else { "+" };
        self.line(&format!("{dest} = {} {op} (size_t){};", p.c_name, n.c_name));
        Ok(())
    }

    pub(super) fn emit_ptr_offset_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.offset expects 2 args".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if p.ty != dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.offset pointer type mismatch".to_string(),
            ));
        }
        if n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.offset expects i32 offset".to_string(),
            ));
        }
        self.line(&format!("{dest} = {} + (int32_t){};", p.c_name, n.c_name));
        Ok(())
    }

    pub(super) fn emit_ptr_read_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.read_u8 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.read_u8 returns i32".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        if !matches!(p.ty, Ty::PtrConstU8 | Ty::PtrMutU8) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.read_u8 expects ptr_const_u8 or ptr_mut_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = (uint32_t)(*{});", p.c_name));
        Ok(())
    }

    pub(super) fn emit_ptr_write_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.write_u8 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.write_u8 returns i32".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let v = self.emit_expr(&args[1])?;
        if p.ty != Ty::PtrMutU8 || v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.write_u8 expects (ptr_mut_u8, i32)".to_string(),
            ));
        }
        self.line(&format!("*{} = (uint8_t){};", p.c_name, v.c_name));
        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    pub(super) fn emit_ptr_read_i32_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.read_i32 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.read_i32 returns i32".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        if !matches!(p.ty, Ty::PtrConstI32 | Ty::PtrMutI32) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.read_i32 expects ptr_const_i32 or ptr_mut_i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = *{};", p.c_name));
        Ok(())
    }

    pub(super) fn emit_ptr_write_i32_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.write_i32 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.write_i32 returns i32".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let v = self.emit_expr(&args[1])?;
        if p.ty != Ty::PtrMutI32 || v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.write_i32 expects (ptr_mut_i32, i32)".to_string(),
            ));
        }
        self.line(&format!("*{} = {};", p.c_name, v.c_name));
        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    pub(super) fn emit_memcpy_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_memcpy_common("memcpy", args, dest_ty, dest)
    }

    pub(super) fn emit_memmove_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_memcpy_common("memmove", args, dest_ty, dest)
    }

    pub(super) fn emit_memcpy_common(
        &mut self,
        name: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{name} expects 3 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{name} returns i32"),
            ));
        }
        let dst = self.emit_expr(&args[0])?;
        let src = self.emit_expr(&args[1])?;
        let n = self.emit_expr(&args[2])?;
        if dst.ty != Ty::PtrMutVoid {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{name} expects ptr_mut_void dest"),
            ));
        }
        if !matches!(src.ty, Ty::PtrConstVoid | Ty::PtrMutVoid) || n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{name} expects (ptr_const_void src, i32 len)"),
            ));
        }
        self.line(&format!("rt_mem_on_memcpy(ctx, {});", n.c_name));
        self.line(&format!(
            "(void){name}((void*){}, (const void*){}, (size_t){});",
            dst.c_name, src.c_name, n.c_name
        ));
        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    pub(super) fn emit_memset_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "memset expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "memset returns i32".to_string(),
            ));
        }
        let dst = self.emit_expr(&args[0])?;
        let val = self.emit_expr(&args[1])?;
        let n = self.emit_expr(&args[2])?;
        if dst.ty != Ty::PtrMutVoid || val.ty != Ty::I32 || n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "memset expects (ptr_mut_void, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "(void)memset((void*){}, (int){}, (size_t){});",
            dst.c_name, val.c_name, n.c_name
        ));
        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    pub(super) fn emit_option_i32_none_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_i32.none expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::OptionI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.none returns option_i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_i32_t){{ .tag = UINT32_C(0), .payload = UINT32_C(0) }};"
        ));
        Ok(())
    }

    pub(super) fn emit_option_i32_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_i32.some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::OptionI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.some returns option_i32".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.some expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_i32_t){{ .tag = UINT32_C(1), .payload = {} }};",
            v.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_option_i32_is_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_i32.is_some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.is_some returns i32".to_string(),
            ));
        }
        match &args[0] {
            Expr::Ident { name, .. } if name != "input" => {
                let Some(opt) = self.lookup(name).cloned() else {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                if opt.moved {
                    let moved_ptr = opt
                        .moved_ptr
                        .as_deref()
                        .filter(|p| !p.is_empty())
                        .unwrap_or("<unknown>");
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("use after move: {name:?} moved_ptr={moved_ptr}"),
                    ));
                }
                if opt.ty != Ty::OptionI32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "option_i32.is_some expects option_i32".to_string(),
                    ));
                }
                self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", opt.c_name));
            }
            _ => {
                let opt = self.emit_expr(&args[0])?;
                if opt.ty != Ty::OptionI32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "option_i32.is_some expects option_i32".to_string(),
                    ));
                }
                self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", opt.c_name));
            }
        }
        Ok(())
    }

    pub(super) fn emit_option_i32_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_i32.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.unwrap_or returns i32".to_string(),
            ));
        }
        let opt = self.emit_expr(&args[0])?;
        let default = self.emit_expr(&args[1])?;
        if opt.ty != Ty::OptionI32 || default.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.unwrap_or expects (option_i32, i32 default)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(1)) ? {}.payload : {};",
            opt.c_name, opt.c_name, default.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_option_bytes_none_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes.none expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::OptionBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.none returns option_bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_bytes_t){{ .tag = UINT32_C(0), .payload = rt_bytes_empty(ctx) }};"
        ));
        Ok(())
    }

    pub(super) fn emit_option_bytes_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes.some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::OptionBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.some returns option_bytes".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.some expects bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_bytes_t){{ .tag = UINT32_C(1), .payload = {} }};",
            b.c_name
        ));
        self.line(&format!("{} = rt_bytes_empty(ctx);", b.c_name));
        Ok(())
    }

    pub(super) fn emit_option_bytes_is_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes.is_some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.is_some returns i32".to_string(),
            ));
        }
        match &args[0] {
            Expr::Ident { name, .. } if name != "input" => {
                let Some(opt) = self.lookup(name).cloned() else {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                if opt.moved {
                    let moved_ptr = opt
                        .moved_ptr
                        .as_deref()
                        .filter(|p| !p.is_empty())
                        .unwrap_or("<unknown>");
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("use after move: {name:?} moved_ptr={moved_ptr}"),
                    ));
                }
                if opt.ty != Ty::OptionBytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "option_bytes.is_some expects option_bytes".to_string(),
                    ));
                }
                self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", opt.c_name));
            }
            _ => {
                let opt = self.emit_expr(&args[0])?;
                if opt.ty != Ty::OptionBytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "option_bytes.is_some expects option_bytes".to_string(),
                    ));
                }
                self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", opt.c_name));
            }
        }
        Ok(())
    }

    pub(super) fn emit_option_bytes_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.unwrap_or returns bytes".to_string(),
            ));
        }
        let opt = self.emit_expr(&args[0])?;
        let default_name: String;
        let default_var: VarRef;
        let default_is_ident = matches!(&args[1], Expr::Ident { name, .. } if name != "input");
        let default = if default_is_ident {
            let name = args[1].as_ident().unwrap_or_default();
            let Some(v) = self.lookup(name).cloned() else {
                return Err(self.err(
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
                return Err(self.err(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?} moved_ptr={moved_ptr}"),
                ));
            }
            if v.ty != Ty::Bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "option_bytes.unwrap_or expects (option_bytes, bytes default)".to_string(),
                ));
            }
            if v.borrow_count != 0 {
                return Err(self.err(
                    CompileErrorKind::Typing,
                    format!("move while borrowed: {name:?}"),
                ));
            }
            default_name = v.c_name.clone();
            default_var = v;
            &default_var
        } else {
            let v = self.emit_expr(&args[1])?;
            default_name = v.c_name.clone();
            default_var = v;
            &default_var
        };
        if opt.ty != Ty::OptionBytes || default.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.unwrap_or expects (option_bytes, bytes default)".to_string(),
            ));
        }
        self.line(&format!("if ({}.tag == UINT32_C(1)) {{", opt.c_name));
        self.indent += 1;
        self.line(&format!("{dest} = {}.payload;", opt.c_name));
        self.line(&format!("{}.payload = rt_bytes_empty(ctx);", opt.c_name));
        self.line(&format!("{}.tag = UINT32_C(0);", opt.c_name));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!("{dest} = {default_name};"));
        self.line(&format!("{default_name} = rt_bytes_empty(ctx);"));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn emit_option_bytes_view_none_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes_view.none expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::OptionBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes_view.none returns option_bytes_view".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_bytes_view_t){{ .tag = UINT32_C(0), .payload = rt_view_empty(ctx) }};"
        ));
        Ok(())
    }

    pub(super) fn emit_option_bytes_view_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes_view.some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::OptionBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes_view.some returns option_bytes_view".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes_view.some expects bytes_view".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_bytes_view_t){{ .tag = UINT32_C(1), .payload = {} }};",
            v.c_name
        ));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_option_bytes_view_is_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes_view.is_some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes_view.is_some returns i32".to_string(),
            ));
        }
        let opt = self.emit_expr(&args[0])?;
        if opt.ty != Ty::OptionBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes_view.is_some expects option_bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", opt.c_name));
        self.release_temp_view_borrow(&opt)?;
        Ok(())
    }

    pub(super) fn emit_option_bytes_view_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes_view.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes_view.unwrap_or returns bytes_view".to_string(),
            ));
        }
        let opt = self.emit_expr(&args[0])?;
        if opt.ty != Ty::OptionBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes_view.unwrap_or expects option_bytes_view".to_string(),
            ));
        }

        self.line(&format!("if ({}.tag == UINT32_C(1)) {{", opt.c_name));
        self.indent += 1;
        self.line(&format!("{dest} = {}.payload;", opt.c_name));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.push_scope();
        self.emit_expr_to(&args[1], Ty::BytesView, dest)?;
        self.pop_scope()?;
        self.indent -= 1;
        self.line("}");
        self.release_temp_view_borrow(&opt)?;
        Ok(())
    }

    pub(super) fn emit_result_i32_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.ok returns result_i32".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.ok expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_i32_t){{ .tag = UINT32_C(1), .payload.ok = {} }};",
            v.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_result_i32_err_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.err expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.err returns result_i32".to_string(),
            ));
        }
        let code = self.emit_expr(&args[0])?;
        if code.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.err expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_i32_t){{ .tag = UINT32_C(0), .payload.err = {} }};",
            code.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_result_i32_is_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.is_ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.is_ok returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.is_ok expects result_i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", res.c_name));
        Ok(())
    }

    pub(super) fn emit_result_i32_err_code_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.err_code expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.err_code returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.err_code expects result_i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
            res.c_name, res.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_result_i32_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.unwrap_or returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        let default = self.emit_expr(&args[1])?;
        if res.ty != Ty::ResultI32 || default.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.unwrap_or expects (result_i32, i32 default)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(1)) ? {}.payload.ok : {};",
            res.c_name, res.c_name, default.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_result_bytes_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.ok returns result_bytes".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.ok expects bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_bytes_t){{ .tag = UINT32_C(1), .payload.ok = {} }};",
            b.c_name
        ));
        self.line(&format!("{} = rt_bytes_empty(ctx);", b.c_name));
        Ok(())
    }

    pub(super) fn emit_result_bytes_err_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.err expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.err returns result_bytes".to_string(),
            ));
        }
        let code = self.emit_expr(&args[0])?;
        if code.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.err expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {} }};",
            code.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_result_bytes_is_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.is_ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.is_ok returns i32".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
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
            if var.ty != Ty::ResultBytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "result_bytes.is_ok expects result_bytes".to_string(),
                ));
            }
            self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", var.c_name));
            return Ok(());
        }

        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.is_ok expects result_bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", res.c_name));
        Ok(())
    }

    pub(super) fn emit_result_bytes_err_code_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.err_code expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.err_code returns i32".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
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
            if var.ty != Ty::ResultBytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "result_bytes.err_code expects result_bytes".to_string(),
                ));
            }
            self.line(&format!(
                "{dest} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
                var.c_name, var.c_name
            ));
            return Ok(());
        }

        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.err_code expects result_bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
            res.c_name, res.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_result_bytes_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.unwrap_or returns bytes".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        let default = self.emit_expr(&args[1])?;
        if res.ty != Ty::ResultBytes || default.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.unwrap_or expects (result_bytes, bytes default)".to_string(),
            ));
        }
        self.line(&format!("if ({}.tag == UINT32_C(1)) {{", res.c_name));
        self.indent += 1;
        self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
        self.line(&format!("{}.payload.ok = rt_bytes_empty(ctx);", res.c_name));
        self.line(&format!("{}.tag = UINT32_C(0);", res.c_name));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!("{dest} = {};", default.c_name));
        self.line(&format!("{} = rt_bytes_empty(ctx);", default.c_name));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn emit_result_bytes_view_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes_view.ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.ok returns result_bytes_view".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.ok expects bytes_view".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_bytes_view_t){{ .tag = UINT32_C(1), .payload.ok = {} }};",
            v.c_name
        ));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    pub(super) fn emit_result_bytes_view_err_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes_view.err expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.err returns result_bytes_view".to_string(),
            ));
        }
        let code = self.emit_expr(&args[0])?;
        if code.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.err expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_bytes_view_t){{ .tag = UINT32_C(0), .payload.err = {} }};",
            code.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_result_bytes_view_is_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes_view.is_ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.is_ok returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.is_ok expects result_bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", res.c_name));
        self.release_temp_view_borrow(&res)?;
        Ok(())
    }

    pub(super) fn emit_result_bytes_view_err_code_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes_view.err_code expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.err_code returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.err_code expects result_bytes_view".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
            res.c_name, res.c_name
        ));
        self.release_temp_view_borrow(&res)?;
        Ok(())
    }

    pub(super) fn emit_result_bytes_view_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes_view.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.unwrap_or returns bytes_view".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultBytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes_view.unwrap_or expects result_bytes_view".to_string(),
            ));
        }
        self.line(&format!("if ({}.tag == UINT32_C(1)) {{", res.c_name));
        self.indent += 1;
        self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.push_scope();
        self.emit_expr_to(&args[1], Ty::BytesView, dest)?;
        self.pop_scope()?;
        self.indent -= 1;
        self.line("}");
        self.release_temp_view_borrow(&res)?;
        Ok(())
    }

    pub(super) fn emit_result_result_bytes_is_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_result_bytes.is_ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_result_bytes.is_ok returns i32".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
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
            if var.ty != Ty::ResultResultBytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "result_result_bytes.is_ok expects result_result_bytes".to_string(),
                ));
            }
            self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", var.c_name));
            return Ok(());
        }

        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_result_bytes.is_ok expects result_result_bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", res.c_name));
        Ok(())
    }

    pub(super) fn emit_result_result_bytes_err_code_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_result_bytes.err_code expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_result_bytes.err_code returns i32".to_string(),
            ));
        }
        if let Expr::Ident { name, .. } = &args[0] {
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
            if var.ty != Ty::ResultResultBytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "result_result_bytes.err_code expects result_result_bytes".to_string(),
                ));
            }
            self.line(&format!(
                "{dest} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
                var.c_name, var.c_name
            ));
            return Ok(());
        }

        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_result_bytes.err_code expects result_result_bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
            res.c_name, res.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_result_result_bytes_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_result_bytes.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_result_bytes.unwrap_or returns result_bytes".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        let default = self.emit_expr(&args[1])?;
        if res.ty != Ty::ResultResultBytes || default.ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_result_bytes.unwrap_or expects (result_result_bytes, result_bytes default)"
                    .to_string(),
            ));
        }

        self.line(&format!("if ({}.tag == UINT32_C(1)) {{", res.c_name));
        self.indent += 1;
        self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
        self.line(&format!(
            "{}.payload.ok = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = UINT32_C(0) }};",
            res.c_name
        ));
        self.line(&format!("{}.tag = UINT32_C(0);", res.c_name));
        self.line(&format!("{}.payload.err = UINT32_C(0);", res.c_name));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!("{dest} = {};", default.c_name));
        self.line(&format!(
            "{}.payload.ok = rt_bytes_empty(ctx);",
            default.c_name
        ));
        self.line(&format!("{}.tag = UINT32_C(0);", default.c_name));
        self.line(&format!("{}.payload.err = UINT32_C(0);", default.c_name));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn emit_try_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "try expects 1 arg".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        match res.ty {
            Ty::ResultI32 => {
                if !matches!(self.fn_ret_ty, Ty::ResultI32 | Ty::ResultBytes) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_i32) requires function return type result_i32 or result_bytes"
                            .to_string(),
                    ));
                }
                if dest_ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_i32) returns i32".to_string(),
                    ));
                }
                self.line(&format!("if ({}.tag == UINT32_C(0)) {{", res.c_name));
                self.indent += 1;
                let ret_c_name = "try_ret";
                match self.fn_ret_ty {
                    Ty::ResultI32 => self.line(&format!("result_i32_t {ret_c_name} = {};", res.c_name)),
                    Ty::ResultBytes => self.line(&format!(
                        "result_bytes_t {ret_c_name} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {}.payload.err }};",
                        res.c_name
                    )),
                    other => unreachable!("try(result_i32) invalid fn_ret_ty: {other:?}"),
                }
                let cleanup_scopes_snapshot = self.cleanup_scopes.clone();
                for scope in cleanup_scopes_snapshot.iter().rev() {
                    self.emit_unwind_cleanup_scope(scope, self.fn_ret_ty, ret_c_name);
                }

                let ret = self.make_var_ref(self.fn_ret_ty, ret_c_name.to_string(), false);
                self.emit_contract_exit_checks(&ret)?;

                for (ty, c_name) in self.live_owned_drop_list(Some(&res.c_name)) {
                    self.emit_drop_var(ty, &c_name);
                }
                self.line(&format!("return {ret_c_name};"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
                Ok(())
            }
            Ty::ResultBytes => {
                if !matches!(self.fn_ret_ty, Ty::ResultBytes | Ty::ResultI32) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_bytes) requires function return type result_bytes or result_i32"
                            .to_string(),
                    ));
                }
                if dest_ty != Ty::Bytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_bytes) returns bytes".to_string(),
                    ));
                }
                self.line(&format!("if ({}.tag == UINT32_C(0)) {{", res.c_name));
                self.indent += 1;
                let ret_c_name = "try_ret";
                match self.fn_ret_ty {
                    Ty::ResultBytes => self.line(&format!("result_bytes_t {ret_c_name} = {};", res.c_name)),
                    Ty::ResultI32 => self.line(&format!(
                        "result_i32_t {ret_c_name} = (result_i32_t){{ .tag = UINT32_C(0), .payload.err = {}.payload.err }};",
                        res.c_name
                    )),
                    other => unreachable!("try(result_bytes) invalid fn_ret_ty: {other:?}"),
                }
                let cleanup_scopes_snapshot = self.cleanup_scopes.clone();
                for scope in cleanup_scopes_snapshot.iter().rev() {
                    self.emit_unwind_cleanup_scope(scope, self.fn_ret_ty, ret_c_name);
                }

                let ret = self.make_var_ref(self.fn_ret_ty, ret_c_name.to_string(), false);
                self.emit_contract_exit_checks(&ret)?;

                for (ty, c_name) in self.live_owned_drop_list(Some(&res.c_name)) {
                    self.emit_drop_var(ty, &c_name);
                }
                self.line(&format!("return {ret_c_name};"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
                self.line(&format!("{}.payload.ok = rt_bytes_empty(ctx);", res.c_name));
                self.line(&format!("{}.tag = UINT32_C(0);", res.c_name));
                Ok(())
            }
            Ty::ResultBytesView => {
                if !matches!(self.fn_ret_ty, Ty::ResultBytesView | Ty::ResultI32) {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_bytes_view) requires function return type result_bytes_view or result_i32"
                            .to_string(),
                    ));
                }
                if dest_ty != Ty::BytesView {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_bytes_view) returns bytes_view".to_string(),
                    ));
                }
                self.line(&format!("if ({}.tag == UINT32_C(0)) {{", res.c_name));
                self.indent += 1;
                let ret_c_name = "try_ret";
                match self.fn_ret_ty {
                    Ty::ResultBytesView => {
                        self.line(&format!("result_bytes_view_t {ret_c_name} = {};", res.c_name))
                    }
                    Ty::ResultI32 => self.line(&format!(
                        "result_i32_t {ret_c_name} = (result_i32_t){{ .tag = UINT32_C(0), .payload.err = {}.payload.err }};",
                        res.c_name
                    )),
                    other => unreachable!("try(result_bytes_view) invalid fn_ret_ty: {other:?}"),
                }
                let cleanup_scopes_snapshot = self.cleanup_scopes.clone();
                for scope in cleanup_scopes_snapshot.iter().rev() {
                    self.emit_unwind_cleanup_scope(scope, self.fn_ret_ty, ret_c_name);
                }

                let ret = self.make_var_ref(self.fn_ret_ty, ret_c_name.to_string(), false);
                self.emit_contract_exit_checks(&ret)?;

                for (ty, c_name) in self.live_owned_drop_list(None) {
                    self.emit_drop_var(ty, &c_name);
                }
                self.line(&format!("return {ret_c_name};"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
                self.release_temp_view_borrow(&res)?;
                Ok(())
            }
            other => Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "try expects result_i32, result_bytes, or result_bytes_view, got {other:?}"
                ),
            )),
        }
    }

    pub(super) fn emit_map_u32_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.new expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.new returns i32 handle".to_string(),
            ));
        }
        let cap = self.emit_expr(&args[0])?;
        if cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.new cap must be i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_map_u32_new(ctx, {});", cap.c_name));
        Ok(())
    }

    pub(super) fn emit_map_u32_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.len returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.len expects i32 handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_map_u32_len(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_map_u32_get_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.get expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.get returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        let default = self.emit_expr(&args[2])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 || default.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.get expects (handle, key, default) all i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_get(ctx, {}, {}, {});",
            h.c_name, key.c_name, default.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_map_u32_set_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.set expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.set returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        let val = self.emit_expr(&args[2])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 || (val.ty != Ty::I32 && !is_task_handle_ty(val.ty))
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.set expects (handle, key, val) all i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_set(ctx, {}, {}, {});",
            h.c_name, key.c_name, val.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_map_u32_contains_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.contains expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.contains returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.contains expects (handle, key)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_contains(ctx, {}, {});",
            h.c_name, key.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_map_u32_remove_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.remove expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.remove returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.remove expects (handle, key)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_remove(ctx, {}, {});",
            h.c_name, key.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_set_u32_add_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "set_u32.add expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "set_u32.add returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "set_u32.add expects (handle, key)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_set(ctx, {}, {}, UINT32_C(1));",
            h.c_name, key.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_set_u32_dump_u32le_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "set_u32.dump_u32le expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "set_u32.dump_u32le returns bytes".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "set_u32.dump_u32le expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_set_u32_dump_u32le(ctx, {});",
            h.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_map_u32_dump_kv_u32le_u32le_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.dump_kv_u32le_u32le expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.dump_kv_u32le_u32le returns bytes".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.dump_kv_u32le_u32le expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_dump_kv_u32le_u32le(ctx, {});",
            h.c_name
        ));
        Ok(())
    }
}
