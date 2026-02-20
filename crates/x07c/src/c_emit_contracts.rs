use super::*;

pub(super) const CONTRACT_FUEL: u64 = 50_000;
pub(super) const CONTRACT_ALLOC_BYTES: u64 = 1024 * 1024;
pub(super) const CONTRACT_WITNESS_MAX_BYTES: u32 = 256;

#[derive(Debug, Clone, Copy)]
pub(super) struct ContractWitnessC<'a> {
    pub(super) ty: Ty,
    pub(super) c_name: &'a str,
}

pub(super) fn emit_contract_witness_json_v1(
    mut emit: impl FnMut(String),
    ty: Ty,
    c_name: &str,
    max_bytes: u32,
) -> Result<(), CompilerError> {
    match ty {
        Ty::I32 => {
            emit("fputs(\"{\\\"ty\\\":\\\"i32\\\",\\\"value_i32\\\":\", stderr);".to_string());
            emit(format!("fprintf(stderr, \"%d\", (int32_t){});", c_name));
            emit("fputs(\"}\", stderr);".to_string());
            Ok(())
        }
        Ty::Bytes => {
            emit("fputs(\"{\\\"ty\\\":\\\"bytes\\\",\\\"len\\\":\", stderr);".to_string());
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.len);",
                c_name
            ));
            emit("fputs(\",\\\"hex\\\":\\\"\", stderr);".to_string());
            emit(format!(
                "rt_x07_write_hex_trunc(stderr, {}.ptr, {}.len, UINT32_C({max_bytes}));",
                c_name, c_name
            ));
            emit(format!(
                "fprintf(stderr, \"\\\",\\\"truncated\\\":%u}}\", (unsigned)({}.len > UINT32_C({max_bytes})));",
                c_name
            ));
            Ok(())
        }
        Ty::BytesView => {
            emit("fputs(\"{\\\"ty\\\":\\\"bytes_view\\\",\\\"len\\\":\", stderr);".to_string());
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.len);",
                c_name
            ));
            emit("fputs(\",\\\"hex\\\":\\\"\", stderr);".to_string());
            emit(format!(
                "rt_x07_write_hex_trunc(stderr, {}.ptr, {}.len, UINT32_C({max_bytes}));",
                c_name, c_name
            ));
            emit(format!(
                "fprintf(stderr, \"\\\",\\\"truncated\\\":%u}}\", (unsigned)({}.len > UINT32_C({max_bytes})));",
                c_name
            ));
            Ok(())
        }
        Ty::ResultI32 => {
            emit(format!("if ({}.tag == UINT32_C(1)) {{", c_name));
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_i32\\\",\\\"tag\\\":\\\"ok\\\",\\\"ok_i32\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%d\", (int32_t){}.payload.ok);",
                c_name
            ));
            emit("fputs(\"}\", stderr);".to_string());
            emit("} else {".to_string());
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_i32\\\",\\\"tag\\\":\\\"err\\\",\\\"err_code_u32\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.payload.err);",
                c_name
            ));
            emit("fputs(\"}\", stderr);".to_string());
            emit("}".to_string());
            Ok(())
        }
        Ty::ResultBytes => {
            emit(format!("if ({}.tag == UINT32_C(1)) {{", c_name));
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_bytes\\\",\\\"tag\\\":\\\"ok\\\",\\\"ok\\\":{\\\"len\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.payload.ok.len);",
                c_name
            ));
            emit("fputs(\",\\\"hex\\\":\\\"\", stderr);".to_string());
            emit(format!(
                "rt_x07_write_hex_trunc(stderr, {}.payload.ok.ptr, {}.payload.ok.len, UINT32_C({max_bytes}));",
                c_name, c_name
            ));
            emit(format!(
                "fprintf(stderr, \"\\\",\\\"truncated\\\":%u}}}}\", (unsigned)({}.payload.ok.len > UINT32_C({max_bytes})));",
                c_name
            ));
            emit("} else {".to_string());
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_bytes\\\",\\\"tag\\\":\\\"err\\\",\\\"err_code_u32\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.payload.err);",
                c_name
            ));
            emit("fputs(\"}\", stderr);".to_string());
            emit("}".to_string());
            Ok(())
        }
        Ty::ResultBytesView => {
            emit(format!("if ({}.tag == UINT32_C(1)) {{", c_name));
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_bytes_view\\\",\\\"tag\\\":\\\"ok\\\",\\\"ok\\\":{\\\"len\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.payload.ok.len);",
                c_name
            ));
            emit("fputs(\",\\\"hex\\\":\\\"\", stderr);".to_string());
            emit(format!(
                "rt_x07_write_hex_trunc(stderr, {}.payload.ok.ptr, {}.payload.ok.len, UINT32_C({max_bytes}));",
                c_name, c_name
            ));
            emit(format!(
                "fprintf(stderr, \"\\\",\\\"truncated\\\":%u}}}}\", (unsigned)({}.payload.ok.len > UINT32_C({max_bytes})));",
                c_name
            ));
            emit("} else {".to_string());
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_bytes_view\\\",\\\"tag\\\":\\\"err\\\",\\\"err_code_u32\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.payload.err);",
                c_name
            ));
            emit("fputs(\"}\", stderr);".to_string());
            emit("}".to_string());
            Ok(())
        }
        Ty::ResultResultBytes => {
            emit(format!("if ({}.tag == UINT32_C(1)) {{", c_name));
            emit(format!("if ({}.payload.ok.tag == UINT32_C(1)) {{", c_name));
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_result_bytes\\\",\\\"tag\\\":\\\"ok\\\",\\\"ok\\\":{\\\"tag\\\":\\\"ok\\\",\\\"ok\\\":{\\\"len\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.payload.ok.payload.ok.len);",
                c_name
            ));
            emit("fputs(\",\\\"hex\\\":\\\"\", stderr);".to_string());
            emit(format!(
                "rt_x07_write_hex_trunc(stderr, {}.payload.ok.payload.ok.ptr, {}.payload.ok.payload.ok.len, UINT32_C({max_bytes}));",
                c_name, c_name
            ));
            emit(format!(
                "fprintf(stderr, \"\\\",\\\"truncated\\\":%u}}}}}}\", (unsigned)({}.payload.ok.payload.ok.len > UINT32_C({max_bytes})));",
                c_name
            ));
            emit("} else {".to_string());
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_result_bytes\\\",\\\"tag\\\":\\\"ok\\\",\\\"ok\\\":{\\\"tag\\\":\\\"err\\\",\\\"err_code_u32\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.payload.ok.payload.err);",
                c_name
            ));
            emit("fputs(\"}}\", stderr);".to_string());
            emit("}".to_string());
            emit("} else {".to_string());
            emit(
                "fputs(\"{\\\"ty\\\":\\\"result_result_bytes\\\",\\\"tag\\\":\\\"err\\\",\\\"err_code_u32\\\":\", stderr);".to_string(),
            );
            emit(format!(
                "fprintf(stderr, \"%u\", (unsigned){}.payload.err);",
                c_name
            ));
            emit("fputs(\"}\", stderr);".to_string());
            emit("}".to_string());
            Ok(())
        }
        _ => Err(CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: unsupported contract witness ty: {ty:?}"),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_contract_trap_payload_v1(
    mut emit: impl FnMut(String),
    contract_kind: &str,
    fn_name: &str,
    clause_id: &str,
    clause_index: usize,
    clause_ptr: &str,
    witnesses: &[ContractWitnessC<'_>],
    max_bytes: u32,
) -> Result<(), CompilerError> {
    let fn_bytes = fn_name.as_bytes();
    let fn_escaped = c_escape_string(fn_bytes);
    let id_bytes = clause_id.as_bytes();
    let id_escaped = c_escape_string(id_bytes);
    let ptr_bytes = clause_ptr.as_bytes();
    let ptr_escaped = c_escape_string(ptr_bytes);

    emit(format!(
        "fputs(\"X07T_CONTRACT_V1 {{\\\"contract_kind\\\":\\\"{contract_kind}\\\",\\\"fn\\\":\\\"\", stderr);"
    ));
    emit(format!(
        "rt_x07_write_json_escaped_bytes(stderr, (const uint8_t*)\"{fn_escaped}\", UINT32_C({}));",
        fn_bytes.len()
    ));
    emit("fputs(\"\\\",\\\"clause_id\\\":\\\"\", stderr);".to_string());
    emit(format!(
        "rt_x07_write_json_escaped_bytes(stderr, (const uint8_t*)\"{id_escaped}\", UINT32_C({}));",
        id_bytes.len()
    ));
    emit(format!(
        "fprintf(stderr, \"\\\",\\\"clause_index\\\":%u,\\\"clause_ptr\\\":\\\"\", (unsigned)UINT32_C({}));",
        clause_index as u32
    ));
    emit(format!(
        "rt_x07_write_json_escaped_bytes(stderr, (const uint8_t*)\"{ptr_escaped}\", UINT32_C({}));",
        ptr_bytes.len()
    ));
    emit("fputs(\"\\\",\\\"witness\\\":[\", stderr);".to_string());

    for (idx, w) in witnesses.iter().enumerate() {
        if idx != 0 {
            emit("fputc(',', stderr);".to_string());
        }
        emit_contract_witness_json_v1(&mut emit, w.ty, w.c_name, max_bytes)?;
    }

    emit("fputs(\"]}\\n\", stderr);".to_string());
    emit("(void)fflush(stderr);".to_string());
    emit("rt_trap(NULL);".to_string());
    Ok(())
}

pub(super) fn contract_payload_json_v1(
    contract_kind: &str,
    fn_name: &str,
    clause_id: &str,
    clause_index: usize,
    clause_ptr: &str,
) -> Result<String, CompilerError> {
    let payload = serde_json::json!({
        "contract_kind": contract_kind,
        "fn": fn_name,
        "clause_id": clause_id,
        "clause_index": clause_index,
        "clause_ptr": clause_ptr,
        "witness": [],
    });
    let json = serde_json::to_string(&payload).map_err(|err| {
        CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: serialize contract payload JSON: {err}"),
        )
    })?;
    Ok(format!("X07T_CONTRACT_V1 {json}"))
}

impl<'a> Emitter<'a> {
    pub(super) fn emit_contract_entry_checks(&mut self) -> Result<(), CompilerError> {
        if !self.fn_contracts.has_any() {
            return Ok(());
        }
        if self.fn_contracts.requires.is_empty() && self.fn_contracts.invariant.is_empty() {
            return Ok(());
        }

        let base_scope = self.scopes.first().cloned().unwrap_or_default();
        let requires = self.fn_contracts.requires.clone();
        let invariant = self.fn_contracts.invariant.clone();
        self.with_contract_scope(base_scope, None, move |this| {
            for (idx, c) in requires.iter().enumerate() {
                this.emit_contract_clause_check("requires", ContractClauseKind::Requires, idx, c)?;
            }
            for (idx, c) in invariant.iter().enumerate() {
                this.emit_contract_clause_check(
                    "invariant_entry",
                    ContractClauseKind::Invariant,
                    idx,
                    c,
                )?;
            }
            Ok(())
        })
    }

    pub(super) fn emit_contract_exit_checks(
        &mut self,
        result: &VarRef,
    ) -> Result<(), CompilerError> {
        if !self.fn_contracts.has_any() {
            return Ok(());
        }
        if self.fn_contracts.ensures.is_empty() && self.fn_contracts.invariant.is_empty() {
            return Ok(());
        }

        let base_scope = self.scopes.first().cloned().unwrap_or_default();
        let mut result = result.clone();
        result.ty = self.fn_ret_ty;
        let ensures = self.fn_contracts.ensures.clone();
        let invariant = self.fn_contracts.invariant.clone();
        self.with_contract_scope(base_scope, Some(result), move |this| {
            for (idx, c) in ensures.iter().enumerate() {
                this.emit_contract_clause_check("ensures", ContractClauseKind::Ensures, idx, c)?;
            }
            for (idx, c) in invariant.iter().enumerate() {
                this.emit_contract_clause_check(
                    "invariant_exit",
                    ContractClauseKind::Invariant,
                    idx,
                    c,
                )?;
            }
            Ok(())
        })
    }

    pub(super) fn with_contract_scope<T>(
        &mut self,
        base_scope: BTreeMap<String, VarRef>,
        result: Option<VarRef>,
        f: impl FnOnce(&mut Self) -> Result<T, CompilerError>,
    ) -> Result<T, CompilerError> {
        let saved_scopes = self.scopes.clone();
        self.scopes.clear();
        self.scopes.push(base_scope);
        if let Some(result) = result {
            self.bind("__result".to_string(), result);
        }
        let out = f(self);
        self.scopes = saved_scopes;
        out
    }

    pub(super) fn emit_contract_clause_check(
        &mut self,
        contract_kind: &str,
        id_kind: ContractClauseKind,
        clause_index: usize,
        clause: &crate::x07ast::ContractClauseAst,
    ) -> Result<(), CompilerError> {
        let fn_name = self
            .current_fn_name
            .clone()
            .unwrap_or_else(|| "<unknown_fn>".to_string());
        let clause_id = clause_id_or_hash(
            &fn_name,
            id_kind,
            clause_index,
            &clause.expr,
            clause.id.as_deref(),
        );
        let clause_ptr = clause.expr.ptr().to_string();

        if self.options.contract_mode == ContractMode::VerifyBmc {
            let msg = contract_payload_json_v1(
                contract_kind,
                &fn_name,
                &clause_id,
                clause_index,
                &clause_ptr,
            )?;
            let msg_escaped = c_escape_c_string(&msg);

            self.push_scope();
            self.open_block();

            let cond = self.emit_expr(&clause.expr)?;
            if cond.ty != Ty::I32 {
                return Err(self.err(
                    CompileErrorKind::Typing,
                    format!(
                        "contract clause expr must evaluate to i32 (got {:?})",
                        cond.ty
                    ),
                ));
            }

            if contract_kind == "requires" {
                self.line(&format!(
                    "__CPROVER_assume({} != UINT32_C(0));",
                    cond.c_name
                ));
            } else {
                self.line(&format!(
                    "__CPROVER_assert({} != UINT32_C(0), \"{}\");",
                    cond.c_name, msg_escaped
                ));
            }

            self.pop_scope()?;
            self.close_block();
            return Ok(());
        }

        self.push_scope();
        self.open_block();

        let scope_name = self.alloc_local("t_contract_budget_")?;
        self.decl_local(Ty::BudgetScopeV1, &scope_name);

        self.line(&format!(
            "rt_budget_scope_init(ctx, &{scope_name}, RT_BUDGET_MODE_TRAP, (const uint8_t*)\"contract\", UINT32_C(8), UINT64_C({}), UINT64_C(0), UINT64_C(0), UINT64_C(0), UINT64_C(0), UINT64_C({}));",
            CONTRACT_ALLOC_BYTES,
            CONTRACT_FUEL,
        ));

        let cond = self.emit_expr(&clause.expr)?;
        if cond.ty != Ty::I32 {
            return Err(self.err(
                CompileErrorKind::Typing,
                format!(
                    "contract clause expr must evaluate to i32 (got {:?})",
                    cond.ty
                ),
            ));
        }

        self.line(&format!("if ({} == UINT32_C(0)) {{", cond.c_name));
        self.indent += 1;

        let witness_vals = clause
            .witness
            .iter()
            .map(|w| self.emit_expr(w))
            .collect::<Result<Vec<_>, _>>()?;

        let witnesses: Vec<ContractWitnessC<'_>> = witness_vals
            .iter()
            .map(|w| ContractWitnessC {
                ty: w.ty,
                c_name: w.c_name.as_str(),
            })
            .collect();
        emit_contract_trap_payload_v1(
            |s| self.line(&s),
            contract_kind,
            &fn_name,
            &clause_id,
            clause_index,
            &clause_ptr,
            &witnesses,
            CONTRACT_WITNESS_MAX_BYTES,
        )?;
        self.indent -= 1;
        self.line("}");

        self.line(&format!("rt_budget_scope_exit_block(ctx, &{scope_name});"));

        self.pop_scope()?;
        self.close_block();
        Ok(())
    }
}

pub(super) const CONTRACT_RUNTIME_HELPERS_C: &str = r#"

static void rt_x07_write_hex_trunc(FILE* f, const uint8_t* p, uint32_t len, uint32_t max) {
  static const char hex[] = "0123456789abcdef";
  uint32_t n = len;
  if (n > max) n = max;
  for (uint32_t i = 0; i < n; i++) {
    uint8_t b = p[i];
    (void)fputc(hex[b >> 4], f);
    (void)fputc(hex[b & 0x0f], f);
  }
}

static void rt_x07_write_json_escaped_bytes(FILE* f, const uint8_t* p, uint32_t len) {
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = p[i];
    switch (b) {
      case '\"':
        (void)fputc('\\', f);
        (void)fputc('\"', f);
        break;
      case '\\':
        (void)fputc('\\', f);
        (void)fputc('\\', f);
        break;
      case '\n':
        (void)fputc('\\', f);
        (void)fputc('n', f);
        break;
      case '\r':
        (void)fputc('\\', f);
        (void)fputc('r', f);
        break;
      case '\t':
        (void)fputc('\\', f);
        (void)fputc('t', f);
        break;
      default:
        if (b < 0x20) {
          (void)fprintf(f, "\\u%04x", (unsigned)b);
        } else {
          (void)fputc((int)b, f);
        }
    }
  }
}

"#;
