use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BudgetScopeModeV1 {
    TrapV1,
    ResultErrV1,
    StatsOnlyV1,
    YieldV1,
}

#[derive(Debug, Clone)]
pub(super) struct BudgetScopeCfgV1 {
    pub(super) mode: BudgetScopeModeV1,
    pub(super) label: String,
    pub(super) alloc_bytes: u64,
    pub(super) alloc_calls: u64,
    pub(super) realloc_calls: u64,
    pub(super) memcpy_bytes: u64,
    pub(super) sched_ticks: u64,
    pub(super) fuel: u64,
}

pub(super) fn parse_budget_scope_cfg_v1(expr: &Expr) -> Result<BudgetScopeCfgV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "budget.cfg_v1 must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("budget.cfg_v1") {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "budget scope cfg must be budget.cfg_v1".to_string(),
        ));
    }

    let mut mode: Option<BudgetScopeModeV1> = None;
    let mut label: Option<String> = None;

    let mut alloc_bytes: Option<u64> = None;
    let mut alloc_calls: Option<u64> = None;
    let mut realloc_calls: Option<u64> = None;
    let mut memcpy_bytes: Option<u64> = None;
    let mut sched_ticks: Option<u64> = None;
    let mut fuel: Option<u64> = None;

    for field in items.iter().skip(1) {
        let Expr::List { items: kv, .. } = field else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "budget.cfg_v1 field must be a pair".to_string(),
            ));
        };
        if kv.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "budget.cfg_v1 field must be a pair".to_string(),
            ));
        }
        let Some(key) = kv[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "budget.cfg_v1 key must be an identifier".to_string(),
            ));
        };

        match key {
            "mode" => {
                let Some(v) = kv[1].as_ident() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.cfg_v1 mode must be an identifier".to_string(),
                    ));
                };
                let m = match v {
                    "trap_v1" => BudgetScopeModeV1::TrapV1,
                    "result_err_v1" => BudgetScopeModeV1::ResultErrV1,
                    "stats_only_v1" => BudgetScopeModeV1::StatsOnlyV1,
                    "yield_v1" => BudgetScopeModeV1::YieldV1,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("budget.cfg_v1 mode is not supported: {v:?}"),
                        ));
                    }
                };
                if mode.replace(m).is_some() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.cfg_v1 has duplicate mode".to_string(),
                    ));
                }
            }
            "label" => {
                let Expr::List {
                    items: label_items, ..
                } = &kv[1]
                else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.cfg_v1 label must be bytes.lit".to_string(),
                    ));
                };
                if label_items.first().and_then(Expr::as_ident) != Some("bytes.lit")
                    || label_items.len() != 2
                {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.cfg_v1 label must be bytes.lit".to_string(),
                    ));
                }
                let Some(v) = label_items[1].as_ident() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.cfg_v1 label bytes.lit expects a text string".to_string(),
                    ));
                };
                if label.replace(v.to_string()).is_some() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.cfg_v1 has duplicate label".to_string(),
                    ));
                }
            }
            "alloc_bytes" | "alloc_calls" | "realloc_calls" | "memcpy_bytes" | "sched_ticks"
            | "fuel" => {
                let Expr::Int { value, .. } = &kv[1] else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "budget.cfg_v1 value must be an integer".to_string(),
                    ));
                };
                if *value < 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("budget.cfg_v1 {key} must be >= 0"),
                    ));
                }
                let v = *value as u64;
                match key {
                    "alloc_bytes" => {
                        if alloc_bytes.replace(v).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "budget.cfg_v1 has duplicate alloc_bytes".to_string(),
                            ));
                        }
                    }
                    "alloc_calls" => {
                        if alloc_calls.replace(v).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "budget.cfg_v1 has duplicate alloc_calls".to_string(),
                            ));
                        }
                    }
                    "realloc_calls" => {
                        if realloc_calls.replace(v).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "budget.cfg_v1 has duplicate realloc_calls".to_string(),
                            ));
                        }
                    }
                    "memcpy_bytes" => {
                        if memcpy_bytes.replace(v).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "budget.cfg_v1 has duplicate memcpy_bytes".to_string(),
                            ));
                        }
                    }
                    "sched_ticks" => {
                        if sched_ticks.replace(v).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "budget.cfg_v1 has duplicate sched_ticks".to_string(),
                            ));
                        }
                    }
                    "fuel" => {
                        if fuel.replace(v).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "budget.cfg_v1 has duplicate fuel".to_string(),
                            ));
                        }
                    }
                    _ => unreachable!(),
                }
            }
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("budget.cfg_v1 unknown field: {key}"),
                ));
            }
        }
    }

    Ok(BudgetScopeCfgV1 {
        mode: mode.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "budget.cfg_v1 is missing mode".to_string(),
            )
        })?,
        label: label.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "budget.cfg_v1 is missing label".to_string(),
            )
        })?,
        alloc_bytes: alloc_bytes.unwrap_or(0),
        alloc_calls: alloc_calls.unwrap_or(0),
        realloc_calls: realloc_calls.unwrap_or(0),
        memcpy_bytes: memcpy_bytes.unwrap_or(0),
        sched_ticks: sched_ticks.unwrap_or(0),
        fuel: fuel.unwrap_or(0),
    })
}

pub(super) fn load_budget_profile_cfg_from_arch_v1(
    options: &CompileOptions,
    profile_id: &str,
) -> Result<BudgetScopeCfgV1, CompilerError> {
    let Some(arch_root) = options.arch_root.as_ref() else {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            "budget.scope_from_arch_v1 requires compile_options.arch_root".to_string(),
        ));
    };

    let profile_path = arch_root
        .join("arch")
        .join("budgets")
        .join("profiles")
        .join(format!("{profile_id}.budget.json"));

    let bytes = match std::fs::read(&profile_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let builtin = match profile_id {
                "stream_xf_plugin_v1" => Some(include_str!(
                    "../../../arch/budgets/profiles/stream_xf_plugin_v1.budget.json"
                )),
                _ => None,
            };
            if let Some(doc) = builtin {
                doc.as_bytes().to_vec()
            } else {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("read budget profile file {}: {err}", profile_path.display()),
                ));
            }
        }
        Err(err) => {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("read budget profile file {}: {err}", profile_path.display()),
            ));
        }
    };

    let doc: Value = serde_json::from_slice(&bytes).map_err(|err| {
        CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "parse budget profile JSON {}: {err}",
                profile_path.display()
            ),
        )
    })?;

    let schema_version = doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema_version != X07_BUDGET_PROFILE_SCHEMA_VERSION {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "budget profile schema_version mismatch: got {schema_version:?} expected {X07_BUDGET_PROFILE_SCHEMA_VERSION:?}"
            ),
        ));
    }

    let doc_id = doc.get("id").and_then(Value::as_str).unwrap_or("");
    if doc_id != profile_id {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("budget profile id mismatch: got {doc_id:?} expected {profile_id:?}"),
        ));
    }

    let cfg = doc.get("cfg").and_then(Value::as_object).ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Parse,
            "budget profile cfg must be an object".to_string(),
        )
    })?;

    let mode_s = cfg.get("mode").and_then(Value::as_str).unwrap_or("");
    let mode = match mode_s {
        "trap_v1" => BudgetScopeModeV1::TrapV1,
        "result_err_v1" => BudgetScopeModeV1::ResultErrV1,
        "stats_only_v1" => BudgetScopeModeV1::StatsOnlyV1,
        "yield_v1" => BudgetScopeModeV1::YieldV1,
        _ => {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("budget profile cfg.mode is not supported: {mode_s:?}"),
            ));
        }
    };

    let label = cfg.get("label").and_then(Value::as_str).unwrap_or("");
    if label.is_empty() {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            "budget profile cfg.label must be a non-empty string".to_string(),
        ));
    }

    Ok(BudgetScopeCfgV1 {
        mode,
        label: label.to_string(),
        alloc_bytes: cfg.get("alloc_bytes").and_then(Value::as_u64).unwrap_or(0),
        alloc_calls: cfg.get("alloc_calls").and_then(Value::as_u64).unwrap_or(0),
        realloc_calls: cfg
            .get("realloc_calls")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        memcpy_bytes: cfg.get("memcpy_bytes").and_then(Value::as_u64).unwrap_or(0),
        sched_ticks: cfg.get("sched_ticks").and_then(Value::as_u64).unwrap_or(0),
        fuel: cfg.get("fuel").and_then(Value::as_u64).unwrap_or(0),
    })
}

pub(super) fn parse_bytes_lit_ascii(expr: &Expr, what: &str) -> Result<String, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{what} must be bytes.lit"),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("bytes.lit") || items.len() != 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{what} must be bytes.lit"),
        ));
    }
    let Some(s) = items[1].as_ident() else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{what} must be bytes.lit"),
        ));
    };
    Ok(s.to_string())
}

pub(super) fn parse_i32_lit(expr: &Expr, what: &str) -> Result<i32, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{what} must be i32.lit"),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("i32.lit") || items.len() != 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{what} must be i32.lit"),
        ));
    }
    let Expr::Int { value, .. } = items[1] else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{what} must be i32.lit"),
        ));
    };
    Ok(value)
}

pub(super) fn load_rr_cfg_v1_from_arch_v1(
    options: &CompileOptions,
    policy_id: &str,
    cassette_path: &str,
    mode_i32: i32,
) -> Result<Vec<u8>, CompilerError> {
    let Some(arch_root) = options.arch_root.as_ref() else {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            "std.rr.with_policy_v1 requires compile_options.arch_root".to_string(),
        ));
    };

    let index_path = arch_root.join("arch").join("rr").join("index.x07rr.json");
    let index_bytes = std::fs::read(&index_path).map_err(|err| {
        CompilerError::new(
            CompileErrorKind::Parse,
            format!("read rr index file {}: {err}", index_path.display()),
        )
    })?;
    let index_doc: Value = serde_json::from_slice(&index_bytes).map_err(|err| {
        CompilerError::new(
            CompileErrorKind::Parse,
            format!("parse rr index JSON {}: {err}", index_path.display()),
        )
    })?;

    let index_schema_version = index_doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if index_schema_version != X07_ARCH_RR_INDEX_SCHEMA_VERSION {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "rr index schema_version mismatch: got {index_schema_version:?} expected {X07_ARCH_RR_INDEX_SCHEMA_VERSION:?}"
            ),
        ));
    }

    let mut record_modes_allowed: Vec<String> = Vec::new();
    if let Some(defaults) = index_doc.get("defaults").and_then(Value::as_object) {
        if let Some(modes) = defaults
            .get("record_modes_allowed")
            .and_then(Value::as_array)
        {
            for v in modes {
                if let Some(s) = v.as_str() {
                    record_modes_allowed.push(s.to_string());
                }
            }
        }
    }
    if record_modes_allowed.is_empty() {
        record_modes_allowed.push("replay_v1".to_string());
    }

    let mode_s = match mode_i32 {
        0 => "off",
        1 => "record_v1",
        2 => "replay_v1",
        3 => "record_missing_v1",
        4 => "rewrite_v1",
        _ => {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.rr.with_policy_v1 mode must be one of: 0(off),1(record),2(replay),3(record_missing),4(rewrite)"
                    .to_string(),
            ));
        }
    };
    if !record_modes_allowed.iter().any(|m| m == mode_s) {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "std.rr.with_policy_v1 mode {mode_s:?} is not allowed by arch/rr index defaults.record_modes_allowed"
            ),
        ));
    }

    let policies = index_doc
        .get("policies")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "rr index policies must be an array".to_string(),
            )
        })?;

    let mut policy_path_rel: Option<String> = None;
    for p in policies {
        let Some(obj) = p.as_object() else {
            continue;
        };
        let Some(id) = obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        if id != policy_id {
            continue;
        }
        if let Some(path) = obj.get("policy_path").and_then(Value::as_str) {
            policy_path_rel = Some(path.to_string());
        }
        break;
    }
    let Some(policy_path_rel) = policy_path_rel else {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("rr policy {policy_id:?} not found in rr index policies"),
        ));
    };

    let policy_path = arch_root.join(&policy_path_rel);
    let policy_bytes = std::fs::read(&policy_path).map_err(|err| {
        CompilerError::new(
            CompileErrorKind::Parse,
            format!("read rr policy file {}: {err}", policy_path.display()),
        )
    })?;
    let policy_doc: Value = serde_json::from_slice(&policy_bytes).map_err(|err| {
        CompilerError::new(
            CompileErrorKind::Parse,
            format!("parse rr policy JSON {}: {err}", policy_path.display()),
        )
    })?;

    let policy_schema_version = policy_doc
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if policy_schema_version != X07_ARCH_RR_POLICY_SCHEMA_VERSION {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "rr policy schema_version mismatch: got {policy_schema_version:?} expected {X07_ARCH_RR_POLICY_SCHEMA_VERSION:?}"
            ),
        ));
    }

    let doc_id = policy_doc.get("id").and_then(Value::as_str).unwrap_or("");
    if doc_id != policy_id {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("rr policy id mismatch: got {doc_id:?} expected {policy_id:?}"),
        ));
    }

    let match_mode_s = policy_doc
        .get("match_mode")
        .and_then(Value::as_str)
        .unwrap_or("");
    let match_mode_u8: u8 = match match_mode_s {
        "lookup_v1" => 0,
        "transcript_v1" => 1,
        _ => {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("rr policy match_mode is not supported: {match_mode_s:?}"),
            ));
        }
    };

    let budgets = policy_doc
        .get("budgets")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "rr policy budgets must be an object".to_string(),
            )
        })?;

    fn get_u64_field(obj: &serde_json::Map<String, Value>, k: &str) -> Result<u64, CompilerError> {
        obj.get(k).and_then(Value::as_u64).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                format!("rr policy budgets.{k} must be an integer"),
            )
        })
    }

    let max_cassette_bytes = get_u64_field(budgets, "max_cassette_bytes")?;
    let max_entries_u64 = get_u64_field(budgets, "max_entries")?;
    let max_req_bytes_u64 = get_u64_field(budgets, "max_req_bytes")?;
    let max_resp_bytes_u64 = get_u64_field(budgets, "max_resp_bytes")?;
    let max_key_bytes_u64 = get_u64_field(budgets, "max_key_bytes")?;

    let max_entries = u32::try_from(max_entries_u64).map_err(|_| {
        CompilerError::new(
            CompileErrorKind::Parse,
            "rr policy budgets.max_entries out of u32 range".to_string(),
        )
    })?;
    let max_req_bytes = u32::try_from(max_req_bytes_u64).map_err(|_| {
        CompilerError::new(
            CompileErrorKind::Parse,
            "rr policy budgets.max_req_bytes out of u32 range".to_string(),
        )
    })?;
    let max_resp_bytes = u32::try_from(max_resp_bytes_u64).map_err(|_| {
        CompilerError::new(
            CompileErrorKind::Parse,
            "rr policy budgets.max_resp_bytes out of u32 range".to_string(),
        )
    })?;
    let max_key_bytes = u32::try_from(max_key_bytes_u64).map_err(|_| {
        CompilerError::new(
            CompileErrorKind::Parse,
            "rr policy budgets.max_key_bytes out of u32 range".to_string(),
        )
    })?;

    let cassette_bytes = cassette_path.as_bytes();
    let cassette_len = u32::try_from(cassette_bytes.len()).map_err(|_| {
        CompilerError::new(
            CompileErrorKind::Parse,
            "std.rr.with_policy_v1 cassette_path is too long".to_string(),
        )
    })?;

    let mut out = Vec::new();
    out.extend_from_slice(b"X7RC");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.push(mode_i32 as u8);
    out.push(match_mode_u8);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&max_cassette_bytes.to_le_bytes());
    out.extend_from_slice(&max_entries.to_le_bytes());
    out.extend_from_slice(&max_req_bytes.to_le_bytes());
    out.extend_from_slice(&max_resp_bytes.to_le_bytes());
    out.extend_from_slice(&max_key_bytes.to_le_bytes());
    out.extend_from_slice(&cassette_len.to_le_bytes());
    out.extend_from_slice(cassette_bytes);
    Ok(out)
}

impl<'a> Emitter<'a> {
    pub(super) fn emit_budget_scope_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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

        if cfg.mode == BudgetScopeModeV1::ResultErrV1
            && !matches!(
                dest_ty,
                Ty::ResultI32 | Ty::ResultBytes | Ty::ResultBytesView | Ty::ResultResultBytes
            )
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "budget.scope_v1 mode=result_err_v1 returns result_*".to_string(),
            ));
        }

        let body_ty = self.infer_expr_in_new_scope(&args[1])?;
        if body_ty != dest_ty && body_ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("budget.scope_v1 body must evaluate to {dest_ty:?} (or return)"),
            ));
        }

        let mode = match cfg.mode {
            BudgetScopeModeV1::TrapV1 => "RT_BUDGET_MODE_TRAP",
            BudgetScopeModeV1::ResultErrV1 => "RT_BUDGET_MODE_RESULT_ERR",
            BudgetScopeModeV1::StatsOnlyV1 => "RT_BUDGET_MODE_STATS_ONLY",
            BudgetScopeModeV1::YieldV1 => "RT_BUDGET_MODE_YIELD",
        };

        let label_bytes = cfg.label.as_bytes();
        self.tmp_counter += 1;
        let label_name = format!("budget_label_{}", self.tmp_counter);
        let label_escaped = c_escape_string(label_bytes);
        self.line(&format!(
            "static const char {label_name}[] = \"{label_escaped}\";"
        ));

        let scope_name = self.alloc_local("b_scope_")?;
        self.decl_local(Ty::BudgetScopeV1, &scope_name);
        self.line(&format!(
            "rt_budget_scope_init(ctx, &{scope_name}, {mode}, (const uint8_t*){label_name}, UINT32_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}));",
            label_bytes.len(),
            cfg.alloc_bytes,
            cfg.alloc_calls,
            cfg.realloc_calls,
            cfg.memcpy_bytes,
            cfg.sched_ticks,
            cfg.fuel
        ));

        self.cleanup_scopes.push(CleanupScope::Budget {
            c_name: scope_name.clone(),
        });
        self.emit_expr_to(&args[1], dest_ty, dest)?;
        let popped_cleanup = self.cleanup_scopes.pop();
        debug_assert!(matches!(
            popped_cleanup,
            Some(CleanupScope::Budget { c_name }) if c_name == scope_name
        ));

        self.line(&format!("rt_budget_scope_exit_block(ctx, &{scope_name});"));
        if cfg.mode == BudgetScopeModeV1::ResultErrV1 {
            self.line(&format!("if ({scope_name}.violated) {{"));
            self.indent += 1;
            self.emit_overwrite_result_with_err(dest_ty, dest, &format!("{scope_name}.err_code"));
            self.indent -= 1;
            self.line("}");
        }
        Ok(())
    }

    pub(super) fn emit_budget_scope_from_arch_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
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
        if cfg.mode == BudgetScopeModeV1::YieldV1 && !self.allow_async_ops {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "budget.scope_from_arch_v1 mode=yield_v1 is only allowed in solve or defasync"
                    .to_string(),
            ));
        }

        if cfg.mode == BudgetScopeModeV1::ResultErrV1
            && !matches!(
                dest_ty,
                Ty::ResultI32 | Ty::ResultBytes | Ty::ResultBytesView | Ty::ResultResultBytes
            )
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "budget.scope_from_arch_v1 returns result_* for this profile".to_string(),
            ));
        }

        let body_ty = self.infer_expr_in_new_scope(&args[1])?;
        if body_ty != dest_ty && body_ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("budget.scope_from_arch_v1 body must evaluate to {dest_ty:?} (or return)"),
            ));
        }

        let mode = match cfg.mode {
            BudgetScopeModeV1::TrapV1 => "RT_BUDGET_MODE_TRAP",
            BudgetScopeModeV1::ResultErrV1 => "RT_BUDGET_MODE_RESULT_ERR",
            BudgetScopeModeV1::StatsOnlyV1 => "RT_BUDGET_MODE_STATS_ONLY",
            BudgetScopeModeV1::YieldV1 => "RT_BUDGET_MODE_YIELD",
        };

        let label_bytes = cfg.label.as_bytes();
        self.tmp_counter += 1;
        let label_name = format!("budget_label_{}", self.tmp_counter);
        let label_escaped = c_escape_string(label_bytes);
        self.line(&format!(
            "static const char {label_name}[] = \"{label_escaped}\";"
        ));

        let scope_name = self.alloc_local("b_scope_")?;
        self.decl_local(Ty::BudgetScopeV1, &scope_name);
        self.line(&format!(
            "rt_budget_scope_init(ctx, &{scope_name}, {mode}, (const uint8_t*){label_name}, UINT32_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}), UINT64_C({}));",
            label_bytes.len(),
            cfg.alloc_bytes,
            cfg.alloc_calls,
            cfg.realloc_calls,
            cfg.memcpy_bytes,
            cfg.sched_ticks,
            cfg.fuel
        ));

        self.cleanup_scopes.push(CleanupScope::Budget {
            c_name: scope_name.clone(),
        });
        self.emit_expr_to(&args[1], dest_ty, dest)?;
        let popped_cleanup = self.cleanup_scopes.pop();
        debug_assert!(matches!(
            popped_cleanup,
            Some(CleanupScope::Budget { c_name }) if c_name == scope_name
        ));

        self.line(&format!("rt_budget_scope_exit_block(ctx, &{scope_name});"));
        if cfg.mode == BudgetScopeModeV1::ResultErrV1 {
            self.line(&format!("if ({scope_name}.violated) {{"));
            self.indent += 1;
            self.emit_overwrite_result_with_err(dest_ty, dest, &format!("{scope_name}.err_code"));
            self.indent -= 1;
            self.line("}");
        }

        Ok(())
    }

    pub(super) fn emit_fs_read_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.read returns bytes".to_string(),
            ));
        }
        let path = self.emit_expr_as_bytes_view(&args[0])?;
        if path.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.read expects bytes_view path".to_string(),
            ));
        }
        // Phase G2: if a fixture latency index is present, enforce it even for fs.read so
        // benchmark suites can require meaningful concurrency.
        self.line(&format!(
            "{dest} = rt_fs_read_async_block(ctx, {});",
            path.c_name
        ));
        self.release_temp_view_borrow(&path)?;
        Ok(())
    }

    pub(super) fn emit_fs_read_async_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.read_async returns bytes".to_string(),
            ));
        }
        let path = self.emit_expr_as_bytes_view(&args[0])?;
        if path.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.read_async expects bytes_view path".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_fs_read_async_block(ctx, {});",
            path.c_name
        ));
        self.release_temp_view_borrow(&path)?;
        Ok(())
    }

    pub(super) fn emit_fs_open_read_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.open_read returns iface".to_string(),
            ));
        }
        let path = self.emit_expr_as_bytes_view(&args[0])?;
        if path.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.open_read expects bytes_view path".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (iface_t){{ .data = rt_fs_open_read(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
            path.c_name
        ));
        self.release_temp_view_borrow(&path)?;
        Ok(())
    }

    pub(super) fn emit_fs_list_dir_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.list_dir returns bytes".to_string(),
            ));
        }
        let path = self.emit_expr_as_bytes_view(&args[0])?;
        if path.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.list_dir expects bytes_view path".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_fs_list_dir(ctx, {});", path.c_name));
        self.release_temp_view_borrow(&path)?;
        Ok(())
    }

    pub(super) fn emit_os_fs_read_file_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.read_file")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.read_file expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.read_file returns bytes".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        if path.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.read_file expects bytes path".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_fs_read_file(ctx, {});",
            path.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_write_file_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.write_file")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.write_file expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.write_file returns i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let data = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || data.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.write_file expects (bytes path, bytes data)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_fs_write_file(ctx, {}, {});",
            path.c_name, data.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_read_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.read_all_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.read_all_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.read_all_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.read_all_v1 returns result_bytes".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.read_all_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_read_all_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_write_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.write_all_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.write_all_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.write_all_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.write_all_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let data = self.emit_expr(&args[1])?;
        let caps = self.emit_expr(&args[2])?;
        if path.ty != Ty::Bytes || data.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.write_all_v1 expects (bytes path, bytes data, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_write_all_v1({}, {}, {});",
            path.c_name, data.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_stream_open_write_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.stream_open_write_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.stream_open_write_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.stream_open_write_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stream_open_write_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stream_open_write_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_stream_open_write_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_stream_write_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.stream_write_all_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.stream_write_all_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.stream_write_all_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stream_write_all_v1 returns result_i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let data = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || data.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stream_write_all_v1 expects (i32 writer_handle, bytes_view data)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_stream_write_all_v1((int32_t){}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }});",
            handle.c_name, data.c_name, data.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_stream_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.stream_close_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.stream_close_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.stream_close_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stream_close_v1 returns result_i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stream_close_v1 expects i32 writer_handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_stream_close_v1((int32_t){});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_stream_drop_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.stream_drop_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.stream_drop_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.stream_drop_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stream_drop_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stream_drop_v1 expects i32 writer_handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_stream_drop_v1((int32_t){});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_mkdirs_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.mkdirs_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.mkdirs_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.mkdirs_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.mkdirs_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.mkdirs_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_mkdirs_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_remove_file_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.remove_file_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.remove_file_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.remove_file_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.remove_file_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.remove_file_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_remove_file_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_remove_dir_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.remove_dir_all_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.remove_dir_all_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.remove_dir_all_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.remove_dir_all_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.remove_dir_all_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_remove_dir_all_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_rename_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.rename_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.rename_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.rename_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.rename_v1 returns result_i32".to_string(),
            ));
        }
        let src = self.emit_expr(&args[0])?;
        let dst = self.emit_expr(&args[1])?;
        let caps = self.emit_expr(&args[2])?;
        if src.ty != Ty::Bytes || dst.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.rename_v1 expects (bytes src, bytes dst, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_rename_v1({}, {}, {});",
            src.c_name, dst.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_list_dir_sorted_text_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.list_dir_sorted_text_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.list_dir_sorted_text_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.list_dir_sorted_text_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.list_dir_sorted_text_v1 returns result_bytes".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.list_dir_sorted_text_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_list_dir_sorted_text_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_walk_glob_sorted_text_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.walk_glob_sorted_text_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.walk_glob_sorted_text_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.walk_glob_sorted_text_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.walk_glob_sorted_text_v1 returns result_bytes".to_string(),
            ));
        }
        let root = self.emit_expr(&args[0])?;
        let glob = self.emit_expr(&args[1])?;
        let caps = self.emit_expr(&args[2])?;
        if root.ty != Ty::Bytes || glob.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.walk_glob_sorted_text_v1 expects (bytes root, bytes glob, bytes caps)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_walk_glob_sorted_text_v1({}, {}, {});",
            root.c_name, glob.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_fs_stat_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.stat_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.stat_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.stat_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stat_v1 returns result_bytes".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stat_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_stat_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_stdio_read_line_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.stdio.read_line_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_STDIO,
            native::ABI_MAJOR_V1,
            "os.stdio.read_line_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.stdio.read_line_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.stdio.read_line_v1 returns result_bytes".to_string(),
            ));
        }
        let caps = self.emit_expr(&args[0])?;
        if caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.stdio.read_line_v1 expects (bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_stdio_read_line_v1({});",
            caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_stdio_write_stdout_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.stdio.write_stdout_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_STDIO,
            native::ABI_MAJOR_V1,
            "os.stdio.write_stdout_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.stdio.write_stdout_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.stdio.write_stdout_v1 returns result_i32".to_string(),
            ));
        }
        let data = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if data.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.stdio.write_stdout_v1 expects (bytes data, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_stdio_write_stdout_v1({}, {});",
            data.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_stdio_write_stderr_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.stdio.write_stderr_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_STDIO,
            native::ABI_MAJOR_V1,
            "os.stdio.write_stderr_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.stdio.write_stderr_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.stdio.write_stderr_v1 returns result_i32".to_string(),
            ));
        }
        let data = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if data.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.stdio.write_stderr_v1 expects (bytes data, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_stdio_write_stderr_v1({}, {});",
            data.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_stdio_flush_stdout_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.stdio.flush_stdout_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_STDIO,
            native::ABI_MAJOR_V1,
            "os.stdio.flush_stdout_v1",
        )?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.stdio.flush_stdout_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.stdio.flush_stdout_v1 returns result_i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = x07_ext_stdio_flush_stdout_v1();"));
        Ok(())
    }

    pub(super) fn emit_os_stdio_flush_stderr_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.stdio.flush_stderr_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_STDIO,
            native::ABI_MAJOR_V1,
            "os.stdio.flush_stderr_v1",
        )?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.stdio.flush_stderr_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.stdio.flush_stderr_v1 returns result_i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = x07_ext_stdio_flush_stderr_v1();"));
        Ok(())
    }

    pub(super) fn emit_os_rand_bytes_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.rand.bytes_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_RAND,
            native::ABI_MAJOR_V1,
            "os.rand.bytes_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.rand.bytes_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.rand.bytes_v1 returns result_bytes".to_string(),
            ));
        }
        let n = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if n.ty != Ty::I32 || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.rand.bytes_v1 expects (i32 n, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_rand_bytes_v1({}, {});",
            n.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_rand_u64_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.rand.u64_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_RAND,
            native::ABI_MAJOR_V1,
            "os.rand.u64_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.rand.u64_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.rand.u64_v1 returns result_bytes".to_string(),
            ));
        }
        let caps = self.emit_expr(&args[0])?;
        if caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.rand.u64_v1 expects (bytes caps)".to_string(),
            ));
        }
        self.line(&format!("{dest} = x07_ext_rand_u64_v1({});", caps.c_name));
        Ok(())
    }

    pub(super) fn emit_os_db_call_bytes_v1_to(
        &mut self,
        builtin: &str,
        c_fn: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only(builtin)?;
        let backend_id = if builtin.starts_with("os.db.sqlite.") {
            Some(native::BACKEND_ID_EXT_DB_SQLITE)
        } else if builtin.starts_with("os.db.pg.") {
            Some(native::BACKEND_ID_EXT_DB_PG)
        } else if builtin.starts_with("os.db.mysql.") {
            Some(native::BACKEND_ID_EXT_DB_MYSQL)
        } else if builtin.starts_with("os.db.redis.") {
            Some(native::BACKEND_ID_EXT_DB_REDIS)
        } else {
            None
        };
        if let Some(backend_id) = backend_id {
            self.require_native_backend(backend_id, native::ABI_MAJOR_V1, builtin)?;
        }
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{builtin} expects 2 args"),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{builtin} returns bytes"),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if req.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{builtin} expects (bytes req, bytes caps)"),
            ));
        }
        self.line(&format!(
            "{dest} = {c_fn}({}, {});",
            req.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_db_sqlite_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.sqlite.open_v1",
            "x07_ext_db_sqlite_open_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_sqlite_query_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.sqlite.query_v1",
            "x07_ext_db_sqlite_query_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_sqlite_exec_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.sqlite.exec_v1",
            "x07_ext_db_sqlite_exec_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_sqlite_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.sqlite.close_v1",
            "x07_ext_db_sqlite_close_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_pg_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.pg.open_v1",
            "x07_ext_db_pg_open_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_pg_query_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.pg.query_v1",
            "x07_ext_db_pg_query_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_pg_exec_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.pg.exec_v1",
            "x07_ext_db_pg_exec_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_pg_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.pg.close_v1",
            "x07_ext_db_pg_close_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_mysql_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.mysql.open_v1",
            "x07_ext_db_mysql_open_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_mysql_query_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.mysql.query_v1",
            "x07_ext_db_mysql_query_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_mysql_exec_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.mysql.exec_v1",
            "x07_ext_db_mysql_exec_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_mysql_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.mysql.close_v1",
            "x07_ext_db_mysql_close_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_redis_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.redis.open_v1",
            "x07_ext_db_redis_open_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_redis_cmd_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.redis.cmd_v1",
            "x07_ext_db_redis_cmd_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_db_redis_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.redis.close_v1",
            "x07_ext_db_redis_close_v1",
            args,
            dest_ty,
            dest,
        )
    }

    pub(super) fn emit_os_env_get_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.env.get")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.env.get expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.env.get returns bytes".to_string(),
            ));
        }
        let key = self.emit_expr(&args[0])?;
        if key.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.env.get expects bytes key".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_os_env_get(ctx, {});", key.c_name));
        Ok(())
    }

    pub(super) fn emit_os_time_now_unix_ms_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.time.now_unix_ms")?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.now_unix_ms expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.now_unix_ms returns i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_os_time_now_unix_ms(ctx);"));
        Ok(())
    }

    pub(super) fn emit_os_time_now_instant_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.time.now_instant_v1")?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.now_instant_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.now_instant_v1 returns bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_os_time_now_instant_v1(ctx);"));
        Ok(())
    }

    pub(super) fn emit_os_time_sleep_ms_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.time.sleep_ms_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.sleep_ms_v1 expects 1 arg".to_string(),
            ));
        }
        let ms = self.emit_expr(&args[0])?;
        if ms.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.sleep_ms_v1 expects i32 ms".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.sleep_ms_v1 returns i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (int32_t)rt_os_time_sleep_ms_v1(ctx, (int32_t){});",
            ms.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_time_local_tzid_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.time.local_tzid_v1")?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.local_tzid_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.local_tzid_v1 returns bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_os_time_local_tzid_v1(ctx);"));
        Ok(())
    }

    pub(super) fn emit_os_time_tzdb_is_valid_tzid_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_TIME,
            native::ABI_MAJOR_V1,
            "os.time.tzdb_is_valid_tzid_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.tzdb_is_valid_tzid_v1 expects 1 arg".to_string(),
            ));
        }
        let tzid = self.emit_expr(&args[0])?;
        if tzid.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_is_valid_tzid_v1 expects bytes_view tzid".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_is_valid_tzid_v1 returns i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ev_time_tzdb_is_valid_tzid_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }});",
            tzid.c_name, tzid.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_time_tzdb_offset_duration_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_TIME,
            native::ABI_MAJOR_V1,
            "os.time.tzdb_offset_duration_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.tzdb_offset_duration_v1 expects 3 args".to_string(),
            ));
        }
        let tzid = self.emit_expr(&args[0])?;
        if tzid.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_offset_duration_v1 expects bytes_view tzid".to_string(),
            ));
        }
        let unix_s_lo = self.emit_expr(&args[1])?;
        let unix_s_hi = self.emit_expr(&args[2])?;
        if unix_s_lo.ty != Ty::I32 || unix_s_hi.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_offset_duration_v1 expects i32 unix_s_lo/unix_s_hi".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_offset_duration_v1 returns bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ev_time_tzdb_offset_duration_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){}, (int32_t){});",
            tzid.c_name, tzid.c_name, unix_s_lo.c_name, unix_s_hi.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_time_tzdb_snapshot_id_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_TIME,
            native::ABI_MAJOR_V1,
            "os.time.tzdb_snapshot_id_v1",
        )?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.tzdb_snapshot_id_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_snapshot_id_v1 returns bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = ev_time_tzdb_snapshot_id_v1();"));
        Ok(())
    }

    pub(super) fn emit_os_process_exit_to(
        &mut self,
        args: &[Expr],
        _dest_ty: Ty,
        _dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.exit")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.exit expects 1 arg".to_string(),
            ));
        }
        let code = self.emit_expr(&args[0])?;
        if code.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.exit expects i32 exit code".to_string(),
            ));
        }
        self.line(&format!(
            "rt_os_process_exit(ctx, (int32_t){});",
            code.c_name
        ));
        self.line("__builtin_unreachable();");
        Ok(())
    }

    pub(super) fn emit_os_process_spawn_capture_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.spawn_capture_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.spawn_capture_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.spawn_capture_v1 returns i32".to_string(),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if req.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.spawn_capture_v1 expects (bytes req, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_spawn_capture_v1(ctx, {}, {});",
            req.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_spawn_piped_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.spawn_piped_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.spawn_piped_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.spawn_piped_v1 returns i32".to_string(),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if req.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.spawn_piped_v1 expects (bytes req, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_spawn_piped_v1(ctx, {}, {});",
            req.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_try_join_capture_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.try_join_capture_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.try_join_capture_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::OptionBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.try_join_capture_v1 returns option_bytes".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.try_join_capture_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_try_join_capture_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_stdout_read_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.stdout_read_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.stdout_read_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdout_read_v1 returns bytes".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let max = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || max.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdout_read_v1 expects (i32 handle, i32 max)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_stdout_read_v1(ctx, {}, (int32_t){});",
            handle.c_name, max.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_stderr_read_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.stderr_read_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.stderr_read_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stderr_read_v1 returns bytes".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let max = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || max.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stderr_read_v1 expects (i32 handle, i32 max)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_stderr_read_v1(ctx, {}, (int32_t){});",
            handle.c_name, max.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_stdin_write_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.stdin_write_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.stdin_write_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdin_write_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let chunk = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || chunk.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdin_write_v1 expects (i32 handle, bytes chunk)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_stdin_write_v1(ctx, {}, {});",
            handle.c_name, chunk.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_stdin_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.stdin_close_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.stdin_close_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdin_close_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdin_close_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_stdin_close_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_try_wait_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.try_wait_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.try_wait_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.try_wait_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.try_wait_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_try_wait_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_join_exit_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.join_exit_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.join_exit_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.join_exit_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.join_exit_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_join_exit_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_take_exit_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.take_exit_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.take_exit_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.take_exit_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.take_exit_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_take_exit_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_join_capture_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.join_capture_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.join_capture_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.join_capture_v1 returns bytes".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.join_capture_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_join_capture_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_kill_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.kill_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.kill_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.kill_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let sig = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || sig.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.kill_v1 expects (i32 proc_handle, i32 sig)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_kill_v1(ctx, {}, (int32_t){});",
            handle.c_name, sig.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_drop_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.drop_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.drop_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.drop_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.drop_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_drop_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_process_run_capture_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.run_capture_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.run_capture_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.run_capture_v1 returns bytes".to_string(),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if req.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.run_capture_v1 expects (bytes req, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_run_capture_v1(ctx, {}, {});",
            req.c_name, caps.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_os_net_http_request_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.net.http_request")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.net.http_request expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.net.http_request returns bytes".to_string(),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        if req.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.net.http_request expects bytes req".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_net_http_request(ctx, {});",
            req.c_name
        ));
        Ok(())
    }

    pub(super) fn emit_rr_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.open_v1 returns result_i32".to_string(),
            ));
        }
        let cfg = self.emit_expr_as_bytes_view(&args[0])?;
        if cfg.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.open_v1 expects cfg bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_rr_open_v1(ctx, {});", cfg.c_name));
        self.release_temp_view_borrow(&cfg)?;
        Ok(())
    }

    pub(super) fn emit_rr_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.close_v1 returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.close_v1 expects i32 rr_handle_v1".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_rr_close_v1(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_rr_stats_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.stats_v1 returns bytes".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.stats_v1 expects i32 rr_handle_v1".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_rr_stats_v1(ctx, {});", h.c_name));
        Ok(())
    }

    pub(super) fn emit_rr_next_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
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
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.next_v1 returns result_bytes".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.next_v1 expects (i32 rr_handle_v1, bytes_view kind, bytes_view op, bytes_view key)"
                    .to_string(),
            ));
        }
        let kind = self.emit_expr_as_bytes_view(&args[1])?;
        let op = self.emit_expr_as_bytes_view(&args[2])?;
        let key = self.emit_expr_as_bytes_view(&args[3])?;
        if kind.ty != Ty::BytesView || op.ty != Ty::BytesView || key.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.next_v1 expects (i32 rr_handle_v1, bytes_view kind, bytes_view op, bytes_view key)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_rr_next_v1(ctx, {}, {}, {}, {}, NULL, UINT32_C(1));",
            h.c_name, kind.c_name, op.c_name, key.c_name
        ));
        self.release_temp_view_borrow(&kind)?;
        self.release_temp_view_borrow(&op)?;
        self.release_temp_view_borrow(&key)?;
        Ok(())
    }

    pub(super) fn emit_rr_append_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.append_v1 returns result_i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.append_v1 expects i32 rr_handle_v1".to_string(),
            ));
        }
        let entry = self.emit_expr_as_bytes_view(&args[1])?;
        if entry.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.append_v1 expects bytes_view entry".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_rr_append_v1(ctx, {}, {});",
            h.c_name, entry.c_name
        ));
        self.release_temp_view_borrow(&entry)?;
        Ok(())
    }

    pub(super) fn emit_rr_current_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.current_v1 returns i32 rr_handle_v1".to_string(),
            ));
        }
        self.line(&format!("{dest} = ctx->rr_current;"));
        Ok(())
    }

    pub(super) fn emit_rr_entry_resp_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.entry_resp_v1 returns bytes".to_string(),
            ));
        }
        let entry = self.emit_expr_as_bytes_view(&args[0])?;
        if entry.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.entry_resp_v1 expects bytes_view entry".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_rr_entry_resp_v1(ctx, {});",
            entry.c_name
        ));
        self.release_temp_view_borrow(&entry)?;
        Ok(())
    }

    pub(super) fn emit_rr_entry_err_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.entry_err_v1 returns i32".to_string(),
            ));
        }
        let entry = self.emit_expr_as_bytes_view(&args[0])?;
        if entry.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.entry_err_v1 expects bytes_view entry".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_rr_entry_err_v1(ctx, {});",
            entry.c_name
        ));
        self.release_temp_view_borrow(&entry)?;
        Ok(())
    }

    pub(super) fn emit_rr_with_cfg_expr_to(
        &mut self,
        cfg_expr: &Expr,
        body: &Expr,
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let cfg = self.emit_expr_as_bytes_view(cfg_expr)?;
        if cfg.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.rr.with_v1 expects cfg bytes_view".to_string(),
            ));
        }

        let open_res = self.alloc_local("t_rr_open_")?;
        self.decl_local(Ty::ResultI32, &open_res);
        self.line(&format!("{open_res} = rt_rr_open_v1(ctx, {});", cfg.c_name));
        self.release_temp_view_borrow(&cfg)?;

        self.emit_rr_with_open_result_to(open_res.as_str(), body, dest_ty, dest)
    }

    pub(super) fn emit_rr_with_open_result_to(
        &mut self,
        open_res_c_name: &str,
        body: &Expr,
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.line(&format!("if ({open_res_c_name}.tag == UINT32_C(0)) {{"));
        self.indent += 1;
        let ret_c_name = "rr_with_ret";
        match self.fn_ret_ty {
            Ty::ResultI32 => self.line(&format!(
                "result_i32_t {ret_c_name} = (result_i32_t){{ .tag = UINT32_C(0), .payload.err = {open_res_c_name}.payload.err }};"
            )),
            Ty::ResultBytes => self.line(&format!(
                "result_bytes_t {ret_c_name} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {open_res_c_name}.payload.err }};"
            )),
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "std.rr.with_* requires function return type result_i32 or result_bytes (open failure propagation)".to_string(),
                ));
            }
        }
        let cleanup_scopes_snapshot = self.cleanup_scopes.clone();
        for scope in cleanup_scopes_snapshot.iter().rev() {
            self.emit_unwind_cleanup_scope(scope, self.fn_ret_ty, ret_c_name);
        }
        for (ty, c_name) in self.live_owned_drop_list(None) {
            self.emit_drop_var(ty, &c_name);
        }
        self.line(&format!("return {ret_c_name};"));
        self.indent -= 1;
        self.line("}");

        let handle_name = self.alloc_local("t_rr_h_")?;
        self.decl_local(Ty::I32, &handle_name);
        self.line(&format!(
            "{handle_name} = (int32_t){open_res_c_name}.payload.ok;"
        ));

        let prev_name = self.alloc_local("t_rr_prev_")?;
        self.decl_local(Ty::I32, &prev_name);
        self.line(&format!("{prev_name} = ctx->rr_current;"));
        self.line(&format!("ctx->rr_current = {handle_name};"));

        self.cleanup_scopes.push(CleanupScope::Rr {
            handle_c_name: handle_name.clone(),
            prev_c_name: prev_name.clone(),
        });
        self.emit_expr_to(body, dest_ty, dest)?;
        let popped_cleanup = self.cleanup_scopes.pop();
        debug_assert!(matches!(popped_cleanup, Some(CleanupScope::Rr { .. })));
        self.emit_unwind_cleanup_scope(
            &CleanupScope::Rr {
                handle_c_name: handle_name,
                prev_c_name: prev_name,
            },
            dest_ty,
            dest,
        );
        Ok(())
    }

    pub(super) fn emit_kv_get_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get returns bytes".to_string(),
            ));
        }
        let key = self.emit_expr_as_bytes_view(&args[0])?;
        if key.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get expects bytes_view key".to_string(),
            ));
        }
        // Phase G2: if a fixture latency index is present, enforce it even for kv.get so
        // benchmark suites can require meaningful concurrency.
        self.line(&format!(
            "{dest} = rt_kv_get_async_block(ctx, {});",
            key.c_name
        ));
        self.release_temp_view_borrow(&key)?;
        Ok(())
    }

    pub(super) fn emit_kv_get_async_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get_async returns bytes".to_string(),
            ));
        }
        let key = self.emit_expr_as_bytes_view(&args[0])?;
        if key.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get_async expects bytes_view key".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_kv_get_async_block(ctx, {});",
            key.c_name
        ));
        self.release_temp_view_borrow(&key)?;
        Ok(())
    }

    pub(super) fn emit_kv_get_stream_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get_stream returns iface".to_string(),
            ));
        }
        let key = self.emit_expr_as_bytes_view(&args[0])?;
        if key.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get_stream expects bytes_view key".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (iface_t){{ .data = rt_kv_get_stream(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
            key.c_name
        ));
        self.release_temp_view_borrow(&key)?;
        Ok(())
    }

    pub(super) fn emit_kv_set_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
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
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.set returns i32".to_string(),
            ));
        }
        let key = self.emit_expr(&args[0])?;
        let val = self.emit_expr(&args[1])?;
        if key.ty != Ty::Bytes || val.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.set expects (bytes, bytes)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_kv_set(ctx, {}, {});",
            key.c_name, val.c_name
        ));
        // kv.set takes ownership of key/val.
        self.line(&format!("{} = {};", key.c_name, c_empty(Ty::Bytes)));
        self.line(&format!("{} = {};", val.c_name, c_empty(Ty::Bytes)));
        Ok(())
    }
}

pub(super) const RUNTIME_C_OS: &str = r#"
// Standalone OS runtime helpers.
//
// These helpers are only compiled into binaries produced for standalone-only
// worlds (run-os, run-os-sandboxed).

static uint32_t rt_os_policy_inited = 0;
static uint32_t rt_os_sandboxed = 0;

static uint32_t rt_os_fs_enabled = 1;
static uint32_t rt_os_net_enabled = 1;
static uint32_t rt_os_env_enabled = 1;
static uint32_t rt_os_time_enabled = 1;
static uint32_t rt_os_proc_enabled = 1;

static uint32_t rt_os_threads_enabled = 1;
static uint32_t rt_os_threads_max_workers = 0;
static uint32_t rt_os_threads_max_blocking = 4;
static uint32_t rt_os_threads_max_queue = 1024;

static uint32_t rt_os_deny_hidden = 0;
static const char* rt_os_fs_read_roots = NULL;
static const char* rt_os_fs_write_roots = NULL;

static const char* rt_os_env_allow_keys = NULL;
static const char* rt_os_env_deny_keys = NULL;

static uint32_t rt_os_time_allow_wall_clock = 1;
static uint32_t rt_os_time_allow_monotonic = 1;
static uint32_t rt_os_time_allow_sleep = 1;
static uint32_t rt_os_time_max_sleep_ms = 0;
static uint32_t rt_os_time_allow_local_tzid = 1;
static uint32_t rt_os_proc_allow_exit = 1;
static uint32_t rt_os_proc_allow_spawn = 1;
static uint32_t rt_os_proc_allow_exec = 1;
static const char* rt_os_proc_allow_execs = NULL;
static const char* rt_os_proc_allow_exec_prefixes = NULL;
static const char* rt_os_proc_allow_args_regex_lite = NULL;
static const char* rt_os_proc_allow_env_keys = NULL;
static uint32_t rt_os_proc_max_live = 0;
static uint32_t rt_os_proc_max_spawns = 0;
static uint32_t rt_os_proc_max_exe_bytes = 4096;
static uint32_t rt_os_proc_max_args = 64;
static uint32_t rt_os_proc_max_arg_bytes = 4096;
static uint32_t rt_os_proc_max_env = 64;
static uint32_t rt_os_proc_max_env_key_bytes = 256;
static uint32_t rt_os_proc_max_env_val_bytes = 4096;
static uint32_t rt_os_proc_max_runtime_ms = 0;
static uint32_t rt_os_proc_max_stdout_bytes = 0;
static uint32_t rt_os_proc_max_stderr_bytes = 0;
static uint32_t rt_os_proc_max_total_bytes = 0;
static uint32_t rt_os_proc_max_stdin_bytes = 0;
static uint32_t rt_os_proc_kill_on_drop = 1;
static uint32_t rt_os_proc_kill_tree = 1;
static uint32_t rt_os_proc_allow_cwd = 0;
static const char* rt_os_proc_allow_cwd_roots = NULL;

static uint32_t rt_os_net_allow_tcp = 1;
static uint32_t rt_os_net_allow_dns = 1;
static const char* rt_os_net_allow_hosts = NULL;

static uint32_t rt_os_env_u32(const char* key, uint32_t def) {
  const char* raw = getenv(key);
  if (!raw || !*raw) return def;
  char* end = NULL;
  errno = 0;
  unsigned long v = strtoul(raw, &end, 10);
  if (errno != 0) return def;
  if (end == raw) return def;
  return (uint32_t)v;
}

static void rt_os_policy_init(ctx_t* ctx) {
  (void)ctx;
  if (rt_os_policy_inited) return;

  const char* w = getenv("X07_WORLD");
  const char* sb = getenv("X07_OS_SANDBOXED");
  if ((w && strcmp(w, "run-os-sandboxed") == 0) || (sb && sb[0] == '1')) {
    rt_os_sandboxed = 1;
  }

  if (rt_os_sandboxed) {
    rt_os_fs_enabled = rt_os_env_u32("X07_OS_FS", 0);
    rt_os_net_enabled = rt_os_env_u32("X07_OS_NET", 0);
    rt_os_env_enabled = rt_os_env_u32("X07_OS_ENV", 0);
    rt_os_time_enabled = rt_os_env_u32("X07_OS_TIME", 0);
    rt_os_proc_enabled = rt_os_env_u32("X07_OS_PROC", 0);

    rt_os_threads_enabled = rt_os_env_u32("X07_OS_THREADS", 1);
    rt_os_threads_max_workers = rt_os_env_u32("X07_OS_THREADS_MAX_WORKERS", 0);
    rt_os_threads_max_blocking = rt_os_env_u32("X07_OS_THREADS_MAX_BLOCKING", 4);
    rt_os_threads_max_queue = rt_os_env_u32("X07_OS_THREADS_MAX_QUEUE", 1024);

    rt_os_deny_hidden = rt_os_env_u32("X07_OS_DENY_HIDDEN", 1);
    rt_os_fs_read_roots = getenv("X07_OS_FS_READ_ROOTS");
    rt_os_fs_write_roots = getenv("X07_OS_FS_WRITE_ROOTS");

    rt_os_env_allow_keys = getenv("X07_OS_ENV_ALLOW_KEYS");
    rt_os_env_deny_keys = getenv("X07_OS_ENV_DENY_KEYS");

    rt_os_time_allow_wall_clock = rt_os_env_u32("X07_OS_TIME_ALLOW_WALL_CLOCK", 0);
    rt_os_time_allow_monotonic = rt_os_env_u32("X07_OS_TIME_ALLOW_MONOTONIC", 0);
    rt_os_time_allow_sleep = rt_os_env_u32("X07_OS_TIME_ALLOW_SLEEP", 0);
    rt_os_time_max_sleep_ms = rt_os_env_u32("X07_OS_TIME_MAX_SLEEP_MS", 0);
    rt_os_time_allow_local_tzid = rt_os_env_u32("X07_OS_TIME_ALLOW_LOCAL_TZID", 0);
    rt_os_proc_allow_exit = rt_os_env_u32("X07_OS_PROC_ALLOW_EXIT", 0);
    rt_os_proc_allow_spawn = rt_os_env_u32("X07_OS_PROC_ALLOW_SPAWN", 0);
    rt_os_proc_allow_exec = rt_os_env_u32("X07_OS_PROC_ALLOW_EXEC", 0);
    rt_os_proc_allow_execs = getenv("X07_OS_PROC_ALLOW_EXECS");
    rt_os_proc_allow_exec_prefixes = getenv("X07_OS_PROC_ALLOW_EXEC_PREFIXES");
    rt_os_proc_allow_args_regex_lite = getenv("X07_OS_PROC_ALLOW_ARGS_REGEX_LITE");
    rt_os_proc_allow_env_keys = getenv("X07_OS_PROC_ALLOW_ENV_KEYS");
    rt_os_proc_max_live = rt_os_env_u32("X07_OS_PROC_MAX_LIVE", 0);
    rt_os_proc_max_spawns = rt_os_env_u32("X07_OS_PROC_MAX_SPAWNS", 0);
    rt_os_proc_max_exe_bytes = rt_os_env_u32("X07_OS_PROC_MAX_EXE_BYTES", 4096);
    rt_os_proc_max_args = rt_os_env_u32("X07_OS_PROC_MAX_ARGS", 64);
    rt_os_proc_max_arg_bytes = rt_os_env_u32("X07_OS_PROC_MAX_ARG_BYTES", 4096);
    rt_os_proc_max_env = rt_os_env_u32("X07_OS_PROC_MAX_ENV", 64);
    rt_os_proc_max_env_key_bytes = rt_os_env_u32("X07_OS_PROC_MAX_ENV_KEY_BYTES", 256);
    rt_os_proc_max_env_val_bytes = rt_os_env_u32("X07_OS_PROC_MAX_ENV_VAL_BYTES", 4096);
    rt_os_proc_max_runtime_ms = rt_os_env_u32("X07_OS_PROC_MAX_RUNTIME_MS", 0);
    rt_os_proc_max_stdout_bytes = rt_os_env_u32("X07_OS_PROC_MAX_STDOUT_BYTES", 0);
    rt_os_proc_max_stderr_bytes = rt_os_env_u32("X07_OS_PROC_MAX_STDERR_BYTES", 0);
    rt_os_proc_max_total_bytes = rt_os_env_u32("X07_OS_PROC_MAX_TOTAL_BYTES", 0);
    rt_os_proc_max_stdin_bytes = rt_os_env_u32("X07_OS_PROC_MAX_STDIN_BYTES", 0);
    rt_os_proc_kill_on_drop = rt_os_env_u32("X07_OS_PROC_KILL_ON_DROP", 1);
    rt_os_proc_kill_tree = rt_os_env_u32("X07_OS_PROC_KILL_TREE", 1);
    rt_os_proc_allow_cwd = rt_os_env_u32("X07_OS_PROC_ALLOW_CWD", 0);
    rt_os_proc_allow_cwd_roots = getenv("X07_OS_PROC_ALLOW_CWD_ROOTS");

    rt_os_net_allow_tcp = rt_os_env_u32("X07_OS_NET_ALLOW_TCP", 0);
    rt_os_net_allow_dns = rt_os_env_u32("X07_OS_NET_ALLOW_DNS", 0);
    rt_os_net_allow_hosts = getenv("X07_OS_NET_ALLOW_HOSTS");
  } else {
    rt_os_fs_enabled = 1;
    rt_os_net_enabled = 1;
    rt_os_env_enabled = 1;
    rt_os_time_enabled = 1;
    rt_os_proc_enabled = 1;
    rt_os_threads_enabled = 1;
    rt_os_threads_max_workers = 0;
    rt_os_threads_max_blocking = 4;
    rt_os_threads_max_queue = 1024;
    rt_os_deny_hidden = 0;
    rt_os_time_allow_wall_clock = 1;
    rt_os_time_allow_monotonic = 1;
    rt_os_time_allow_sleep = 1;
    rt_os_time_max_sleep_ms = 0;
    rt_os_time_allow_local_tzid = 1;
    rt_os_proc_allow_exit = 1;
    rt_os_proc_allow_spawn = 1;
    rt_os_proc_allow_exec = 1;
    rt_os_proc_allow_execs = NULL;
    rt_os_proc_allow_exec_prefixes = NULL;
    rt_os_proc_allow_args_regex_lite = NULL;
    rt_os_proc_allow_env_keys = NULL;
    rt_os_proc_max_live = 0;
    rt_os_proc_max_spawns = 0;
    rt_os_proc_max_exe_bytes = 0;
    rt_os_proc_max_args = 0;
    rt_os_proc_max_arg_bytes = 0;
    rt_os_proc_max_env = 0;
    rt_os_proc_max_env_key_bytes = 0;
    rt_os_proc_max_env_val_bytes = 0;
    rt_os_proc_max_runtime_ms = 0;
    rt_os_proc_max_stdout_bytes = 0;
    rt_os_proc_max_stderr_bytes = 0;
    rt_os_proc_max_total_bytes = 0;
    rt_os_proc_max_stdin_bytes = 0;
    rt_os_proc_kill_on_drop = 1;
    rt_os_proc_kill_tree = 1;
    rt_os_proc_allow_cwd = 1;
    rt_os_proc_allow_cwd_roots = NULL;
    rt_os_net_allow_tcp = 1;
    rt_os_net_allow_dns = 1;
  }

  rt_os_policy_inited = 1;
}

static void rt_os_require(ctx_t* ctx, uint32_t ok, const char* msg) {
  if (!rt_os_sandboxed) return;
  if (!ok) rt_trap(msg);
  (void)ctx;
}

static uint32_t rt_os_split_next(const char** cursor, const char** out_start, size_t* out_len) {
  const char* p = *cursor;
  if (!p) return UINT32_C(0);

  for (;;) {
    while (*p == ';' || *p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
    if (*p == 0) {
      *cursor = p;
      return UINT32_C(0);
    }

    const char* start = p;
    while (*p && *p != ';') p++;
    const char* end = p;
    while (end > start
           && (end[-1] == ' ' || end[-1] == '\t' || end[-1] == '\n' || end[-1] == '\r')) {
      end--;
    }

    *cursor = (*p == ';') ? p + 1 : p;

    if (end == start) {
      p = *cursor;
      continue;
    }

    *out_start = start;
    *out_len = (size_t)(end - start);
    return UINT32_C(1);
  }
}

static uint32_t rt_os_list_contains(const char* list, bytes_t key) {
  if (!list || !*list) return UINT32_C(0);
  const char* cur = list;
  for (;;) {
    const char* s = NULL;
    size_t n = 0;
    if (!rt_os_split_next(&cur, &s, &n)) return UINT32_C(0);
    if (n > (size_t)UINT32_MAX) continue;
    if ((uint32_t)n != key.len) continue;
    if (memcmp(s, key.ptr, n) == 0) return UINT32_C(1);
  }
}

static uint32_t rt_os_list_contains_prefix(const char* list, bytes_t path) {
  if (!list || !*list) return UINT32_C(0);
  const char* cur = list;
  for (;;) {
    const char* s = NULL;
    size_t n = 0;
    if (!rt_os_split_next(&cur, &s, &n)) return UINT32_C(0);
    if (n > (size_t)UINT32_MAX) continue;
    if ((uint32_t)n > path.len) continue;
    if (memcmp(s, path.ptr, n) == 0) return UINT32_C(1);
  }
}

static uint32_t rt_os_regex_lite_match_full(
    const char* pat,
    size_t pat_len,
    bytes_t text
) {
  uint8_t tok_kind[256];
  uint8_t tok_lit[256];
  uint8_t tok_star[256];
  uint32_t k = 0;

  size_t i = 0;
  while (i < pat_len) {
    if (k >= UINT32_C(256)) rt_trap("os.process allow_args_regex_lite pattern too long");

    uint8_t c = (uint8_t)pat[i];
    if (c == (uint8_t)'*') rt_trap("os.process allow_args_regex_lite invalid pattern");

    uint8_t kind = 0;
    uint8_t lit = c;
    if (c == (uint8_t)'\\') {
      if (i + 1 >= pat_len) rt_trap("os.process allow_args_regex_lite invalid pattern");
      lit = (uint8_t)pat[i + 1];
      kind = 0;
      i += 2;
    } else if (c == (uint8_t)'.') {
      kind = 1;
      lit = 0;
      i += 1;
    } else {
      kind = 0;
      lit = c;
      i += 1;
    }

    uint8_t star = 0;
    if (i < pat_len && (uint8_t)pat[i] == (uint8_t)'*') {
      star = 1;
      i += 1;
    }

    tok_kind[k] = kind;
    tok_lit[k] = lit;
    tok_star[k] = star;
    k += 1;
  }

  uint8_t states[257];
  uint8_t next[257];
  for (uint32_t j = 0; j <= k; j++) {
    states[j] = 0;
    next[j] = 0;
  }

  states[0] = 1;
  for (uint32_t j = 0; j < k; j++) {
    if (tok_star[j] && states[j]) states[j + 1] = 1;
  }

  for (uint32_t t = 0; t < text.len; t++) {
    uint8_t ch = text.ptr[t];
    for (uint32_t j = 0; j <= k; j++) next[j] = 0;

    for (uint32_t j = 0; j < k; j++) {
      if (!states[j]) continue;

      uint8_t match = 0;
      if (tok_kind[j] == 1) {
        match = 1;
      } else if (tok_lit[j] == ch) {
        match = 1;
      }

      if (!match) continue;

      if (tok_star[j]) {
        next[j] = 1;
      } else {
        next[j + 1] = 1;
      }
    }

    for (uint32_t j = 0; j < k; j++) {
      if (tok_star[j] && next[j]) next[j + 1] = 1;
    }

    for (uint32_t j = 0; j <= k; j++) states[j] = next[j];
  }

  for (uint32_t j = 0; j < k; j++) {
    if (tok_star[j] && states[j]) states[j + 1] = 1;
  }

  return states[k] ? UINT32_C(1) : UINT32_C(0);
}

static uint32_t rt_os_proc_args_allowed(bytes_t arg) {
  if (!rt_os_proc_allow_args_regex_lite || !*rt_os_proc_allow_args_regex_lite) return UINT32_C(1);
  const char* cur = rt_os_proc_allow_args_regex_lite;
  for (;;) {
    const char* s = NULL;
    size_t n = 0;
    if (!rt_os_split_next(&cur, &s, &n)) return UINT32_C(0);
    if (rt_os_regex_lite_match_full(s, n, arg)) return UINT32_C(1);
  }
}

static char* rt_os_bytes_to_cstr(ctx_t* ctx, bytes_t b, const char* what) {
  for (uint32_t i = 0; i < b.len; i++) {
    if (b.ptr[i] == 0) rt_trap(what);
  }
  char* out = (char*)rt_alloc(ctx, b.len + 1, 1);
  if (b.len != 0) {
    memcpy(out, b.ptr, b.len);
    rt_mem_on_memcpy(ctx, b.len);
  }
  out[b.len] = 0;
  return out;
}

static uint32_t rt_os_path_has_hidden_segment(bytes_t path) {
  if (path.len == 0) return UINT32_C(0);
  uint32_t seg_start = 0;
  for (uint32_t i = 0; i <= path.len; i++) {
    uint8_t b = (i == path.len) ? (uint8_t)'/' : path.ptr[i];
    if (b == (uint8_t)'/') {
      if (i > seg_start && path.ptr[seg_start] == (uint8_t)'.') return UINT32_C(1);
      seg_start = i + 1;
    }
  }
  return UINT32_C(0);
}

static char* rt_os_join_root_and_rel(ctx_t* ctx, const char* root, size_t root_len, bytes_t rel) {
  if (root_len == 0) rt_trap("os.fs root empty");
  if (root_len > (size_t)UINT32_MAX) rt_trap("os.fs root too long");

  uint32_t need_slash = 0;
  char last = root[root_len - 1];
  if (last != '/' && last != '\\') need_slash = 1;

  uint64_t total = (uint64_t)root_len + (uint64_t)need_slash + (uint64_t)rel.len + UINT64_C(1);
  if (total > (uint64_t)UINT32_MAX) rt_trap("os.fs path too long");

  char* out = (char*)rt_alloc(ctx, (uint32_t)total, 1);
  memcpy(out, root, root_len);
  rt_mem_on_memcpy(ctx, (uint32_t)root_len);
  uint32_t off = (uint32_t)root_len;
  if (need_slash) {
    out[off] = '/';
    off += 1;
  }
  if (rel.len != 0) {
    memcpy(out + off, rel.ptr, rel.len);
    rt_mem_on_memcpy(ctx, rel.len);
  }
  out[off + rel.len] = 0;
  return out;
}

static bytes_t rt_os_strip_root_prefix(bytes_t path, const char* root, size_t root_len) {
  // If the caller already included the root prefix in `path` (common for project-relative paths like
  // `out/file.txt`), avoid producing `root/root/...` when we join roots.
  size_t trimmed = root_len;
  while (trimmed > 0 && (root[trimmed - 1] == '/' || root[trimmed - 1] == '\\')) trimmed--;
  if (trimmed == 0) return path;
  if ((uint32_t)trimmed >= path.len) return path;
  if (memcmp(path.ptr, root, trimmed) != 0) return path;
  if (path.ptr[trimmed] != (uint8_t)'/') return path;

  bytes_t out = path;
  out.ptr = path.ptr + trimmed + 1;
  out.len = path.len - (uint32_t)(trimmed + 1);
  return out;
}

static bytes_t rt_os_fs_read_file(ctx_t* ctx, bytes_t path) {
  rt_os_policy_init(ctx);

  FILE* f = NULL;
  char* p = NULL;

  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_fs_enabled, "os.fs disabled by policy");
    if (rt_os_threads_max_blocking == 0) rt_trap("os.threads.blocking disabled by policy");
    bytes_view_t path_view = rt_bytes_view(ctx, path);
    if (!rt_fs_is_safe_rel_path(path_view)) rt_trap("os.fs.read_file unsafe path");
    if (rt_os_deny_hidden && rt_os_path_has_hidden_segment(path)) {
      rt_trap("os.fs.read_file hidden path denied");
    }

    const char* roots = rt_os_fs_read_roots;
    const char* cur = roots;
    const char* root = NULL;
    size_t root_len = 0;
    while (rt_os_split_next(&cur, &root, &root_len)) {
      bytes_t rel = rt_os_strip_root_prefix(path, root, root_len);
      p = rt_os_join_root_and_rel(ctx, root, root_len, rel);
      f = fopen(p, "rb");
      if (f) break;
    }
    if (!f) rt_trap("os.fs.read_file open failed");
  } else {
    p = rt_os_bytes_to_cstr(ctx, path, "os.fs.read_file path contains NUL");
    f = fopen(p, "rb");
    if (!f) rt_trap("os.fs.read_file open failed");
  }

  if (fseek(f, 0, SEEK_END) != 0) rt_trap("os.fs.read_file seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("os.fs.read_file tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("os.fs.read_file file too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("os.fs.read_file seek failed");

  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)end);
  if (out.len != 0) {
    size_t n = fread(out.ptr, 1, out.len, f);
    if (n != out.len) rt_trap("os.fs.read_file short read");
  }
  fclose(f);
  return out;
}

static uint32_t rt_os_fs_write_file(ctx_t* ctx, bytes_t path, bytes_t data) {
  rt_os_policy_init(ctx);

  FILE* f = NULL;
  char* p = NULL;
  int last_errno = 0;

  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_fs_enabled, "os.fs disabled by policy");
    if (rt_os_threads_max_blocking == 0) rt_trap("os.threads.blocking disabled by policy");
    bytes_view_t path_view = rt_bytes_view(ctx, path);
    if (!rt_fs_is_safe_rel_path(path_view)) rt_trap("os.fs.write_file unsafe path");
    if (rt_os_deny_hidden && rt_os_path_has_hidden_segment(path)) {
      rt_trap("os.fs.write_file hidden path denied");
    }

    const char* roots = rt_os_fs_write_roots;
    const char* cur = roots;
    const char* root = NULL;
    size_t root_len = 0;
    while (rt_os_split_next(&cur, &root, &root_len)) {
      bytes_t rel = rt_os_strip_root_prefix(path, root, root_len);
      p = rt_os_join_root_and_rel(ctx, root, root_len, rel);
      f = fopen(p, "wb");
      if (f) break;
      last_errno = errno;
    }
    if (!f) return (uint32_t)(last_errno ? last_errno : EACCES);
  } else {
    p = rt_os_bytes_to_cstr(ctx, path, "os.fs.write_file path contains NUL");
    f = fopen(p, "wb");
    if (!f) {
      last_errno = errno;
      return (uint32_t)(last_errno ? last_errno : 1);
    }
  }

  if (data.len != 0) {
    size_t n = fwrite(data.ptr, 1, data.len, f);
    if (n != data.len) {
      last_errno = errno;
      fclose(f);
      return (uint32_t)(last_errno ? last_errno : 1);
    }
  }
  if (fclose(f) != 0) {
    last_errno = errno;
    return (uint32_t)(last_errno ? last_errno : 1);
  }
  return UINT32_C(0);
}

static bytes_t rt_os_env_get(ctx_t* ctx, bytes_t key) {
  rt_os_policy_init(ctx);

  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_env_enabled, "os.env disabled by policy");
    if (rt_os_list_contains(rt_os_env_deny_keys, key)) rt_trap("os.env.get key denied");
    if (!rt_os_list_contains(rt_os_env_allow_keys, key)) rt_trap("os.env.get key not allowed");
  }

  char* k = rt_os_bytes_to_cstr(ctx, key, "os.env.get key contains NUL");
  const char* v = getenv(k);
  if (!v) return rt_bytes_empty(ctx);
  size_t n = strlen(v);
  if (n > (size_t)UINT32_MAX) rt_trap("os.env.get value too large");
  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)n);
  if (out.len != 0) {
    memcpy(out.ptr, v, n);
    rt_mem_on_memcpy(ctx, out.len);
  }
  return out;
}

static uint32_t rt_os_time_now_unix_ms(ctx_t* ctx) {
  rt_os_policy_init(ctx);
  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_time_enabled, "os.time disabled by policy");
    rt_os_require(ctx, rt_os_time_allow_wall_clock, "os.time.now_unix_ms disabled by policy");
  }

  struct timespec ts;
  if (timespec_get(&ts, TIME_UTC) != TIME_UTC) rt_trap("os.time.now_unix_ms failed");
  uint64_t ms = (uint64_t)ts.tv_sec * UINT64_C(1000) + (uint64_t)(ts.tv_nsec / 1000000L);
  return (uint32_t)ms;
}

#define RT_OS_TIME_CODE_POLICY_DENIED UINT32_C(300)
#define RT_OS_TIME_CODE_INTERNAL UINT32_C(301)

#define RT_OS_TIME_TZID_CODE_POLICY_DENIED UINT32_C(1)
#define RT_OS_TIME_TZID_CODE_INTERNAL UINT32_C(2)

static bytes_t rt_os_time_duration_err(ctx_t* ctx, uint32_t code) {
  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(9));
  out.ptr[0] = 0;
  rt_write_u32_le(out.ptr + 1, code);
  rt_write_u32_le(out.ptr + 5, UINT32_C(0));
  return out;
}

static bytes_t rt_os_time_local_tzid_err(ctx_t* ctx, uint32_t code) {
  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(5));
  out.ptr[0] = 0;
  rt_write_u32_le(out.ptr + 1, code);
  return out;
}

static bytes_t rt_os_time_now_instant_v1(ctx_t* ctx) {
  rt_os_policy_init(ctx);
  if (rt_os_sandboxed) {
    if (!rt_os_time_enabled) return rt_os_time_duration_err(ctx, RT_OS_TIME_CODE_POLICY_DENIED);
    if (!rt_os_time_allow_wall_clock) return rt_os_time_duration_err(ctx, RT_OS_TIME_CODE_POLICY_DENIED);
  }

  struct timespec ts;
  if (timespec_get(&ts, TIME_UTC) != TIME_UTC) {
    return rt_os_time_duration_err(ctx, RT_OS_TIME_CODE_INTERNAL);
  }

  int64_t unix_s = (int64_t)ts.tv_sec;
  uint64_t unix_s_bits = (uint64_t)unix_s;

  if (ts.tv_nsec < 0 || ts.tv_nsec >= 1000000000L) {
    return rt_os_time_duration_err(ctx, RT_OS_TIME_CODE_INTERNAL);
  }
  uint32_t nanos = (uint32_t)ts.tv_nsec;

  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(14));
  out.ptr[0] = 1;
  out.ptr[1] = 1;
  rt_write_u32_le(out.ptr + 2, (uint32_t)(unix_s_bits & UINT64_C(0xFFFFFFFF)));
  rt_write_u32_le(out.ptr + 6, (uint32_t)((unix_s_bits >> 32) & UINT64_C(0xFFFFFFFF)));
  rt_write_u32_le(out.ptr + 10, nanos);
  return out;
}

static uint32_t rt_os_time_sleep_ms_v1(ctx_t* ctx, int32_t ms) {
  rt_os_policy_init(ctx);
  if (ms < 0) return UINT32_C(0);

  if (rt_os_sandboxed) {
    if (!rt_os_time_enabled) return UINT32_C(0);
    if (!rt_os_time_allow_sleep) return UINT32_C(0);
    if ((uint32_t)ms > rt_os_time_max_sleep_ms) return UINT32_C(0);
  }

  uint32_t ms_u = (uint32_t)ms;
  struct timespec req;
  req.tv_sec = (time_t)(ms_u / UINT32_C(1000));
  req.tv_nsec = (long)((ms_u % UINT32_C(1000)) * UINT32_C(1000000));

  for (;;) {
    if (nanosleep(&req, &req) == 0) return UINT32_C(1);
    if (errno == EINTR) continue;
    return UINT32_C(0);
  }
}

static bytes_t rt_os_time_local_tzid_v1(ctx_t* ctx) {
  rt_os_policy_init(ctx);
  if (rt_os_sandboxed) {
    if (!rt_os_time_enabled) return rt_os_time_local_tzid_err(ctx, RT_OS_TIME_TZID_CODE_POLICY_DENIED);
    if (!rt_os_time_allow_local_tzid) return rt_os_time_local_tzid_err(ctx, RT_OS_TIME_TZID_CODE_POLICY_DENIED);
  }

  // v1: if the platform tzid can't be determined portably, return empty.
  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(6));
  out.ptr[0] = 1;
  out.ptr[1] = 1;
  rt_write_u32_le(out.ptr + 2, UINT32_C(0));
  return out;
}

static uint64_t rt_os_now_ms(void) {
  struct timespec ts;
  if (timespec_get(&ts, TIME_UTC) != TIME_UTC) rt_trap("os.process.run_capture_v1 clock failed");
  return (uint64_t)ts.tv_sec * UINT64_C(1000) + (uint64_t)(ts.tv_nsec / 1000000L);
}

static void rt_os_close_fd(int fd) {
  if (fd < 0) return;
  for (;;) {
    if (close(fd) == 0) return;
    if (errno == EINTR) continue;
    return;
  }
}

static int rt_os_set_nonblock(int fd) {
  int flags = fcntl(fd, F_GETFL, 0);
  if (flags < 0) return -1;
  if (fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0) return -1;
  return 0;
}

static int rt_os_set_cloexec(int fd) {
  int flags = fcntl(fd, F_GETFD, 0);
  if (flags < 0) return -1;
  if (fcntl(fd, F_SETFD, flags | FD_CLOEXEC) < 0) return -1;
  return 0;
}

#define RT_OS_PROC_CODE_POLICY_DENIED UINT32_C(1)
#define RT_OS_PROC_CODE_INVALID_REQUEST UINT32_C(2)
#define RT_OS_PROC_CODE_SPAWN_FAILED UINT32_C(3)
#define RT_OS_PROC_CODE_TIMEOUT UINT32_C(4)
#define RT_OS_PROC_CODE_OUTPUT_LIMIT UINT32_C(5)

#define RT_OS_PROC_STATE_EMPTY UINT32_C(0)
#define RT_OS_PROC_STATE_RUNNING UINT32_C(1)
#define RT_OS_PROC_STATE_DONE UINT32_C(2)

#define RT_OS_PROC_MODE_CAPTURE UINT32_C(1)
#define RT_OS_PROC_MODE_PIPED UINT32_C(2)

#define RT_OS_PROC_POLL_KIND_STDIN UINT32_C(1)
#define RT_OS_PROC_POLL_KIND_STDOUT UINT32_C(2)
#define RT_OS_PROC_POLL_KIND_STDERR UINT32_C(3)

struct rt_os_proc_s {
  uint32_t state;
  uint32_t mode;
  uint16_t gen;
  uint16_t _pad;

  pid_t pid;
  pid_t pgid;
  int stdin_fd;
  int stdout_fd;
  int stderr_fd;

  uint32_t stdin_closed;
  uint32_t stdin_close_requested;
  uint32_t stdout_closed;
  uint32_t stderr_closed;

  uint32_t exited;
  int status;

  uint32_t fail_code;
  uint32_t kill_sent;

  uint32_t max_stdout;
  uint32_t max_stderr;
  uint32_t max_total;
  uint32_t timeout_ms;
  uint64_t start_ms;

  bytes_t stdin_buf;
  uint32_t stdin_off;

  bytes_t stdout_buf;
  uint32_t stdout_off;
  uint32_t stdout_len;

  bytes_t stderr_buf;
  uint32_t stderr_off;
  uint32_t stderr_len;

  bytes_t result;
  uint32_t result_taken;
  uint32_t exit_taken;

  uint32_t join_wait_head;
  uint32_t join_wait_tail;

  uint32_t exit_wait_head;
  uint32_t exit_wait_tail;
};

static void rt_os_procs_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->os_procs_cap) return;
  rt_os_proc_t* old_items = ctx->os_procs;
  uint32_t old_cap = ctx->os_procs_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_os_proc_t);
  uint32_t new_cap = ctx->os_procs_cap ? ctx->os_procs_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_os_proc_t* items = (rt_os_proc_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_os_proc_t),
    (uint32_t)_Alignof(rt_os_proc_t)
  );
  if (old_items && ctx->os_procs_len) {
    uint32_t bytes = ctx->os_procs_len * (uint32_t)sizeof(rt_os_proc_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_os_proc_t));
  }
  ctx->os_procs = items;
  ctx->os_procs_cap = new_cap;
}

static uint16_t rt_os_proc_next_gen(uint16_t gen) {
  gen = (uint16_t)(gen + 1);
  if (gen == 0) gen = 1;
  return gen;
}

static uint32_t rt_os_proc_handle_u32(uint32_t idx, uint16_t gen) {
  uint32_t idx16 = idx + 1;
  if (idx16 == 0 || idx16 > UINT32_C(0xFFFF)) rt_trap("os.process handle overflow");
  if (gen == 0) rt_trap("os.process handle overflow");
  return ((uint32_t)gen << 16) | idx16;
}

static int32_t rt_os_proc_handle_i32(uint32_t idx, uint16_t gen) {
  return (int32_t)rt_os_proc_handle_u32(idx, gen);
}

static uint32_t rt_os_proc_handle_is_sentinel(int32_t handle, uint32_t* out_code) {
  uint32_t h = (uint32_t)handle;
  if ((h & UINT32_C(0xFFFF)) != 0) return UINT32_C(0);
  uint32_t code = h >> 16;
  if (code == 0) return UINT32_C(0);
  if (out_code) *out_code = code;
  return UINT32_C(1);
}

static void rt_os_proc_init_entry(ctx_t* ctx, rt_os_proc_t* p, uint16_t gen) {
  memset(p, 0, sizeof(*p));
  p->state = RT_OS_PROC_STATE_EMPTY;
  p->gen = gen ? gen : 1;
  p->pid = (pid_t)-1;
  p->pgid = (pid_t)-1;
  p->stdin_fd = -1;
  p->stdout_fd = -1;
  p->stderr_fd = -1;
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdout_buf = rt_bytes_empty(ctx);
  p->stderr_buf = rt_bytes_empty(ctx);
  p->result = rt_bytes_empty(ctx);
}

static uint32_t rt_os_proc_find_free_slot(ctx_t* ctx) {
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    if (ctx->os_procs[i].state == RT_OS_PROC_STATE_EMPTY) return i;
  }
  return UINT32_MAX;
}

static uint32_t rt_os_proc_alloc_slot(ctx_t* ctx, uint32_t allow_extend) {
  uint32_t idx = rt_os_proc_find_free_slot(ctx);
  if (idx != UINT32_MAX) {
    rt_os_proc_t* p = &ctx->os_procs[idx];
    uint16_t gen = p->gen ? p->gen : 1;
    rt_os_proc_init_entry(ctx, p, gen);
    return idx;
  }
  if (!allow_extend) rt_trap("os.process out of proc slots");
  idx = ctx->os_procs_len;
  rt_os_procs_ensure_cap(ctx, idx + 1);
  ctx->os_procs_len += 1;
  rt_os_proc_init_entry(ctx, &ctx->os_procs[idx], 1);
  return idx;
}

static rt_os_proc_t* rt_os_proc_from_handle(ctx_t* ctx, int32_t handle, uint32_t* out_idx) {
  uint32_t h = (uint32_t)handle;
  uint32_t idx16 = h & UINT32_C(0xFFFF);
  uint16_t gen = (uint16_t)(h >> 16);
  if (idx16 == 0 || gen == 0) rt_trap("os.process invalid handle");
  uint32_t idx = idx16 - 1;
  if (idx >= ctx->os_procs_len) rt_trap("os.process invalid handle");
  rt_os_proc_t* p = &ctx->os_procs[idx];
  if (p->state == RT_OS_PROC_STATE_EMPTY) rt_trap("os.process invalid handle");
  if (p->gen != gen) rt_trap("os.process invalid handle");
  if (out_idx) *out_idx = idx;
  return p;
}

static void rt_os_proc_close_fds(rt_os_proc_t* p) {
  rt_os_close_fd(p->stdin_fd);
  rt_os_close_fd(p->stdout_fd);
  rt_os_close_fd(p->stderr_fd);
  p->stdin_fd = -1;
  p->stdout_fd = -1;
  p->stderr_fd = -1;
  p->stdin_closed = 1;
  p->stdout_closed = 1;
  p->stderr_closed = 1;
}

static void rt_os_proc_drop_buffers(ctx_t* ctx, rt_os_proc_t* p) {
  rt_bytes_drop(ctx, &p->stdin_buf);
  rt_bytes_drop(ctx, &p->stdout_buf);
  rt_bytes_drop(ctx, &p->stderr_buf);
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdout_buf = rt_bytes_empty(ctx);
  p->stderr_buf = rt_bytes_empty(ctx);
  p->stdin_off = 0;
  p->stdout_off = 0;
  p->stdout_len = 0;
  p->stderr_off = 0;
  p->stderr_len = 0;
}

static void rt_os_proc_drop_result(ctx_t* ctx, rt_os_proc_t* p) {
  rt_bytes_drop(ctx, &p->result);
  p->result = rt_bytes_empty(ctx);
  p->result_taken = 0;
}

static void rt_os_wake_wait_list(
    ctx_t* ctx,
    uint32_t* head,
    uint32_t* tail,
    uint32_t wait_kind,
    uint32_t reason_id
) {
  uint32_t w = *head;
  uint32_t wt = *tail;
  (void)wt;
  *head = 0;
  *tail = 0;
  while (w != 0) {
    rt_task_t* waiter = rt_task_ptr(ctx, w);
    uint32_t next = waiter->wait_next;
    waiter->wait_next = 0;
    rt_sched_wake(ctx, w, wait_kind, reason_id);
    w = next;
  }
}

static void rt_os_proc_wake_waiters(ctx_t* ctx, rt_os_proc_t* p, uint32_t reason_id) {
  rt_os_wake_wait_list(
    ctx,
    &p->join_wait_head,
    &p->join_wait_tail,
    RT_WAIT_OS_PROC_JOIN,
    reason_id
  );
}

static void rt_os_proc_wake_exit_waiters(ctx_t* ctx, rt_os_proc_t* p, uint32_t reason_id) {
  rt_os_wake_wait_list(
    ctx,
    &p->exit_wait_head,
    &p->exit_wait_tail,
    RT_WAIT_OS_PROC_EXIT,
    reason_id
  );
}

static bytes_t rt_os_proc_make_err(ctx_t* ctx, uint32_t code) {
  bytes_t out = rt_bytes_alloc(ctx, 9);
  out.ptr[0] = 0;
  rt_write_u32_le(out.ptr + 1, code);
  rt_write_u32_le(out.ptr + 5, 0);
  return out;
}

static uint32_t rt_os_proc_exit_code_from_status(int status) {
  uint32_t exit_code = UINT32_C(1);
  if (WIFEXITED(status)) {
    exit_code = (uint32_t)WEXITSTATUS(status);
  } else if (WIFSIGNALED(status)) {
    exit_code = UINT32_C(128) + (uint32_t)WTERMSIG(status);
  }
  return exit_code;
}

static bytes_t rt_os_proc_build_ok_doc(
    ctx_t* ctx,
    uint32_t exit_code,
    uint32_t flags,
    bytes_t stdout_buf,
    uint32_t stdout_len,
    bytes_t stderr_buf,
    uint32_t stderr_len
) {
  if (stdout_len > UINT32_MAX - UINT32_C(18) || stderr_len > UINT32_MAX - UINT32_C(18) - stdout_len) {
    return rt_os_proc_make_err(ctx, RT_OS_PROC_CODE_OUTPUT_LIMIT);
  }

  uint32_t out_len = UINT32_C(18) + stdout_len + stderr_len;
  bytes_t out = rt_bytes_alloc(ctx, out_len);
  uint8_t* p = out.ptr;
  p[0] = 1;                 // ok tag
  p[1] = 1;                 // ProcRespV1 ver
  rt_write_u32_le(p + 2, exit_code);
  rt_write_u32_le(p + 6, flags);
  rt_write_u32_le(p + 10, stdout_len);
  if (stdout_len != 0) {
    memcpy(p + 14, stdout_buf.ptr, stdout_len);
    rt_mem_on_memcpy(ctx, stdout_len);
  }
  rt_write_u32_le(p + 14 + stdout_len, stderr_len);
  if (stderr_len != 0) {
    memcpy(p + 18 + stdout_len, stderr_buf.ptr, stderr_len);
    rt_mem_on_memcpy(ctx, stderr_len);
  }
  return out;
}

static void rt_os_proc_try_wait(rt_os_proc_t* p) {
  if (p->pid == (pid_t)-1) return;
  if (p->exited) return;
  for (;;) {
    pid_t r = waitpid(p->pid, &p->status, WNOHANG);
    if (r == p->pid) {
      p->exited = 1;
      return;
    }
    if (r == 0) {
      return;
    }
    if (r < 0) {
      if (errno == EINTR) continue;
      p->exited = 1;
      p->status = 0;
      return;
    }
  }
}

static pid_t rt_os_proc_kill_target(rt_os_proc_t* p) {
  if (p->pid == (pid_t)-1) return (pid_t)-1;
  if (rt_os_proc_kill_tree && p->pgid > (pid_t)1) {
    return (pid_t)(-p->pgid);
  }
  return p->pid;
}

static void rt_os_proc_send_kill(rt_os_proc_t* p, int32_t sig) {
  if (p->pid == (pid_t)-1) return;
  if (p->kill_sent) return;
  pid_t target = rt_os_proc_kill_target(p);
  if (target == (pid_t)-1 || target == (pid_t)0) return;
  (void)kill(target, sig);
  p->kill_sent = 1;
}

static void rt_os_proc_mark_done(ctx_t* ctx, rt_os_proc_t* p, uint32_t idx, bytes_t result) {
  if (p->state == RT_OS_PROC_STATE_RUNNING) {
    if (ctx->os_procs_live == 0) rt_trap("os.process live underflow");
    ctx->os_procs_live -= 1;
  }

  rt_os_proc_drop_result(ctx, p);
  p->state = RT_OS_PROC_STATE_DONE;
  p->result = result;
  p->result_taken = 0;

  uint32_t reason_id = rt_os_proc_handle_u32(idx, p->gen);
  rt_os_proc_wake_waiters(ctx, p, reason_id);
}

static void rt_os_proc_finish_ok(ctx_t* ctx, rt_os_proc_t* p, uint32_t idx) {
  uint32_t exit_code = rt_os_proc_exit_code_from_status(p->status);
  bytes_t doc = rt_os_proc_build_ok_doc(
    ctx,
    exit_code,
    UINT32_C(0),
    p->stdout_buf,
    p->stdout_len,
    p->stderr_buf,
    p->stderr_len
  );

  rt_os_proc_drop_buffers(ctx, p);
  rt_os_proc_mark_done(ctx, p, idx, doc);
}

static void rt_os_proc_finish_err(ctx_t* ctx, rt_os_proc_t* p, uint32_t idx, uint32_t code) {
  rt_os_proc_drop_buffers(ctx, p);
  rt_os_proc_mark_done(ctx, p, idx, rt_os_proc_make_err(ctx, code));
}

static void rt_os_proc_finish_piped(ctx_t* ctx, rt_os_proc_t* p) {
  if (p->state == RT_OS_PROC_STATE_RUNNING) {
    if (ctx->os_procs_live == 0) rt_trap("os.process live underflow");
    ctx->os_procs_live -= 1;
  }
  rt_os_proc_drop_result(ctx, p);
  p->state = RT_OS_PROC_STATE_DONE;
  p->result_taken = 0;
  rt_bytes_drop(ctx, &p->stdin_buf);
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdin_off = 0;
}

static void rt_os_proc_kill_and_reap(rt_os_proc_t* p) {
  if (p->pid == (pid_t)-1) return;
  pid_t target = rt_os_proc_kill_target(p);
  if (target == (pid_t)-1 || target == (pid_t)0) return;
  (void)kill(target, SIGKILL);
  for (;;) {
    int status = 0;
    pid_t r = waitpid(p->pid, &status, 0);
    if (r == p->pid) break;
    if (r < 0 && errno == EINTR) continue;
    break;
  }
}

static void rt_os_spawn_free_argv_env(
    ctx_t* ctx,
    char** argv,
    uint32_t* argv_sizes,
    uint32_t argv_count,
    char** envp,
    uint32_t* env_sizes,
    uint32_t env_count
) {
  if (argv) {
    for (uint32_t i = 0; i < argv_count; i++) {
      if (argv[i]) rt_free(ctx, argv[i], argv_sizes ? argv_sizes[i] : 0, 1);
    }
    rt_free(ctx, argv, (argv_count + 1) * (uint32_t)sizeof(char*), (uint32_t)_Alignof(char*));
  }
  if (argv_sizes) {
    rt_free(ctx, argv_sizes, argv_count * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  }

  if (envp) {
    for (uint32_t i = 0; i < env_count; i++) {
      if (envp[i]) rt_free(ctx, envp[i], env_sizes ? env_sizes[i] : 0, 1);
    }
    rt_free(ctx, envp, (env_count + 1) * (uint32_t)sizeof(char*), (uint32_t)_Alignof(char*));
  }
  if (env_sizes) {
    rt_free(ctx, env_sizes, env_count * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  }
}


static int32_t rt_os_process_spawn_impl(ctx_t* ctx, bytes_t req, bytes_t caps, uint32_t mode) {
  rt_os_policy_init(ctx);

  if (rt_os_sandboxed && rt_os_proc_max_spawns != 0 && ctx->os_procs_spawned >= rt_os_proc_max_spawns) {
    return (int32_t)((uint32_t)RT_OS_PROC_CODE_POLICY_DENIED << 16);
  }

  uint32_t idx = rt_os_proc_alloc_slot(ctx, UINT32_C(1));
  rt_os_proc_t* proc = &ctx->os_procs[idx];
  uint16_t gen = proc->gen;
  proc->mode = mode;

  uint32_t err = 0;

  uint32_t max_stdout = 0;
  uint32_t max_stderr = 0;
  uint32_t timeout_ms = 0;
  uint32_t max_total = 0;

  uint32_t off = 0;
  uint32_t argv_count = 0;
  uint32_t env_count = 0;

  bytes_t argv0 = rt_bytes_empty(ctx);
  bytes_t stdin_view = rt_bytes_empty(ctx);
  bytes_t cwd_view = rt_bytes_empty(ctx);
  char* cwd_cstr = NULL;
  uint32_t cwd_cstr_size = 0;

  char** argv = NULL;
  uint32_t* argv_sizes = NULL;
  char** envp = NULL;
  uint32_t* env_sizes = NULL;

  int stdin_pipe[2] = {-1, -1};
  int stdout_pipe[2] = {-1, -1};
  int stderr_pipe[2] = {-1, -1};
  int exec_pipe[2] = {-1, -1};

  int stdin_fd = -1;
  int stdout_fd = -1;
  int stderr_fd = -1;

  pid_t pid = (pid_t)-1;
  pid_t pgid = (pid_t)-1;
  int status = 0;

  bytes_t stdin_copy = rt_bytes_empty(ctx);
  bytes_t stdout_buf = rt_bytes_empty(ctx);
  bytes_t stderr_buf = rt_bytes_empty(ctx);

  uint32_t cwd_len = 0;
  uint32_t stdin_len = 0;

  if (rt_os_sandboxed) {
    if (!rt_os_proc_enabled || !rt_os_proc_allow_spawn) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }

    if (rt_os_proc_max_spawns != 0) {
      if (ctx->os_procs_spawned >= rt_os_proc_max_spawns) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
      ctx->os_procs_spawned += 1;
    }

    if (rt_os_proc_max_live != 0 && ctx->os_procs_live >= rt_os_proc_max_live) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
  }

  // Parse caps (ProcCapsV1).
  if (caps.len < UINT32_C(17) || caps.ptr[0] != UINT32_C(1)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  max_stdout = rt_read_u32_le(caps.ptr + 1);
  max_stderr = rt_read_u32_le(caps.ptr + 5);
  timeout_ms = rt_read_u32_le(caps.ptr + 9);
  max_total = rt_read_u32_le(caps.ptr + 13);
  if (max_total == 0) {
    uint64_t sum = (uint64_t)max_stdout + (uint64_t)max_stderr;
    max_total = (sum > (uint64_t)UINT32_MAX) ? UINT32_MAX : (uint32_t)sum;
  }
  if (rt_os_sandboxed && rt_os_proc_max_runtime_ms != 0) {
    if (timeout_ms == 0) {
      timeout_ms = rt_os_proc_max_runtime_ms;
    } else if (timeout_ms > rt_os_proc_max_runtime_ms) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
  }
  if (rt_os_sandboxed) {
    if (rt_os_proc_max_stdout_bytes != 0 && max_stdout > rt_os_proc_max_stdout_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (rt_os_proc_max_stderr_bytes != 0 && max_stderr > rt_os_proc_max_stderr_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (rt_os_proc_max_total_bytes != 0 && max_total > rt_os_proc_max_total_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
  }

  // Parse request (ProcReqV1).
  if (req.len < UINT32_C(6) || req.ptr[0] != UINT32_C(1)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }

  // v1 currently requires flags=0 (no env inheritance/clear toggles yet).
  if (req.ptr[1] != UINT32_C(0)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }

  off = UINT32_C(2);
  argv_count = rt_read_u32_le(req.ptr + off);
  off += 4;
  if (argv_count == 0 || argv_count > UINT32_C(128)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  if (rt_os_sandboxed && rt_os_proc_max_args != 0 && argv_count > rt_os_proc_max_args) {
    err = RT_OS_PROC_CODE_POLICY_DENIED;
    goto cleanup;
  }

  argv = (char**)rt_alloc(
      ctx,
      (argv_count + 1) * (uint32_t)sizeof(char*),
      (uint32_t)_Alignof(char*)
  );
  argv_sizes = (uint32_t*)rt_alloc(
      ctx,
      argv_count * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
  );
  for (uint32_t i = 0; i < argv_count; i++) argv[i] = NULL;

  for (uint32_t i = 0; i < argv_count; i++) {
    if (off > req.len || req.len - off < UINT32_C(4)) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    uint32_t n = rt_read_u32_le(req.ptr + off);
    off += 4;
    if (rt_os_sandboxed && rt_os_proc_max_arg_bytes != 0 && n > rt_os_proc_max_arg_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (off > req.len || req.len - off < n) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    bytes_t b;
    b.ptr = req.ptr + off;
    b.len = n;
    if (rt_os_sandboxed && i != 0) {
      if (!rt_os_proc_args_allowed(b)) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
    }
    if (i == 0) argv0 = b;
    off += n;
    argv[i] = rt_os_bytes_to_cstr(ctx, b, "os.process.spawn_capture_v1 argv contains NUL");
    argv_sizes[i] = n + 1;
  }
  argv[argv_count] = NULL;

  if (argv0.len == 0) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  if (rt_os_sandboxed && rt_os_proc_max_exe_bytes != 0 && argv0.len > rt_os_proc_max_exe_bytes) {
    err = RT_OS_PROC_CODE_POLICY_DENIED;
    goto cleanup;
  }

  if (rt_os_sandboxed) {
    uint32_t allow = UINT32_C(0);
    if (rt_os_list_contains(rt_os_proc_allow_execs, argv0)) allow = UINT32_C(1);
    if (!allow && rt_os_list_contains_prefix(rt_os_proc_allow_exec_prefixes, argv0)) {
      allow = UINT32_C(1);
    }
    if (!allow) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
  }

  // env
  if (off > req.len || req.len - off < UINT32_C(4)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  env_count = rt_read_u32_le(req.ptr + off);
  off += 4;
  if (env_count > UINT32_C(256)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  if (rt_os_sandboxed && rt_os_proc_max_env != 0 && env_count > rt_os_proc_max_env) {
    err = RT_OS_PROC_CODE_POLICY_DENIED;
    goto cleanup;
  }

  envp = (char**)rt_alloc(
      ctx,
      (env_count + 1) * (uint32_t)sizeof(char*),
      (uint32_t)_Alignof(char*)
  );
  env_sizes = (uint32_t*)rt_alloc(
      ctx,
      env_count * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
  );
  for (uint32_t i = 0; i < env_count; i++) envp[i] = NULL;

  for (uint32_t i = 0; i < env_count; i++) {
    if (off > req.len || req.len - off < UINT32_C(4)) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    uint32_t klen = rt_read_u32_le(req.ptr + off);
    off += 4;
    if (rt_os_sandboxed && rt_os_proc_max_env_key_bytes != 0 && klen > rt_os_proc_max_env_key_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (off > req.len || req.len - off < klen) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    bytes_t k;
    k.ptr = req.ptr + off;
    k.len = klen;
    off += klen;

    if (off > req.len || req.len - off < UINT32_C(4)) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    uint32_t vlen = rt_read_u32_le(req.ptr + off);
    off += 4;
    if (rt_os_sandboxed && rt_os_proc_max_env_val_bytes != 0 && vlen > rt_os_proc_max_env_val_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (off > req.len || req.len - off < vlen) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    bytes_t v;
    v.ptr = req.ptr + off;
    v.len = vlen;
    off += vlen;

    if (k.len == 0) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    if (rt_os_sandboxed) {
      if (!rt_os_list_contains(rt_os_proc_allow_env_keys, k)) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
    }

    // key=value\0
    uint64_t need64 = (uint64_t)k.len + UINT64_C(1) + (uint64_t)v.len + UINT64_C(1);
    if (need64 > (uint64_t)UINT32_MAX) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    uint32_t need = (uint32_t)need64;

    char* kv = (char*)rt_alloc(ctx, need, 1);
    // Validate no NULs.
    for (uint32_t j = 0; j < k.len; j++) {
      if (k.ptr[j] == 0) rt_trap("os.process.spawn_capture_v1 env key contains NUL");
    }
    for (uint32_t j = 0; j < v.len; j++) {
      if (v.ptr[j] == 0) rt_trap("os.process.spawn_capture_v1 env val contains NUL");
    }

    if (k.len != 0) {
      memcpy(kv, k.ptr, k.len);
      rt_mem_on_memcpy(ctx, k.len);
    }
    kv[k.len] = '=';
    if (v.len != 0) {
      memcpy(kv + k.len + 1, v.ptr, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
    kv[k.len + 1 + v.len] = 0;

    envp[i] = kv;
    env_sizes[i] = need;
  }
  envp[env_count] = NULL;

  // cwd override (v1 currently requires cwd_len=0).
  if (off > req.len || req.len - off < UINT32_C(4)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  cwd_len = rt_read_u32_le(req.ptr + off);
  off += 4;
  if (cwd_len != 0) {
    if (off > req.len || req.len - off < cwd_len) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    cwd_view.ptr = req.ptr + off;
    cwd_view.len = cwd_len;
    off += cwd_len;
    for (uint32_t j = 0; j < cwd_view.len; j++) {
      if (cwd_view.ptr[j] == 0) rt_trap("os.process.spawn_capture_v1 cwd contains NUL");
    }
    if (rt_os_sandboxed) {
      if (!rt_os_proc_allow_cwd) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
      if (!rt_os_proc_allow_cwd_roots || !*rt_os_proc_allow_cwd_roots) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
      bytes_view_t path_view = rt_bytes_view(ctx, cwd_view);
      if (!rt_fs_is_safe_rel_path(path_view)) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
      if (rt_os_deny_hidden && rt_os_path_has_hidden_segment(cwd_view)) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
    } else {
      cwd_cstr = rt_os_bytes_to_cstr(ctx, cwd_view, "os.process.spawn_capture_v1 cwd contains NUL");
      cwd_cstr_size = cwd_view.len + 1;
    }
  }

  // stdin bytes
  if (off > req.len || req.len - off < UINT32_C(4)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  stdin_len = rt_read_u32_le(req.ptr + off);
  off += 4;
  if (rt_os_sandboxed && rt_os_proc_max_stdin_bytes != 0 && stdin_len > rt_os_proc_max_stdin_bytes) {
    err = RT_OS_PROC_CODE_POLICY_DENIED;
    goto cleanup;
  }
  if (off > req.len || req.len - off < stdin_len) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  stdin_view.ptr = req.ptr + off;
  stdin_view.len = stdin_len;
  off += stdin_len;
  if (off != req.len) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }

  stdin_copy = rt_bytes_alloc(ctx, stdin_len);
  if (stdin_len != 0) {
    memcpy(stdin_copy.ptr, stdin_view.ptr, stdin_len);
    rt_mem_on_memcpy(ctx, stdin_len);
  }

  if (pipe(stdin_pipe) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (pipe(stdout_pipe) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (pipe(stderr_pipe) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (pipe(exec_pipe) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (rt_os_set_cloexec(stdin_pipe[0]) != 0
      || rt_os_set_cloexec(stdin_pipe[1]) != 0
      || rt_os_set_cloexec(stdout_pipe[0]) != 0
      || rt_os_set_cloexec(stdout_pipe[1]) != 0
      || rt_os_set_cloexec(stderr_pipe[0]) != 0
      || rt_os_set_cloexec(stderr_pipe[1]) != 0
      || rt_os_set_cloexec(exec_pipe[0]) != 0
      || rt_os_set_cloexec(exec_pipe[1]) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  pid = fork();
  if (pid < 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (pid == 0) {
    // Child.
    rt_os_close_fd(exec_pipe[0]);
    exec_pipe[0] = -1;

    if (dup2(stdin_pipe[0], STDIN_FILENO) < 0
        || dup2(stdout_pipe[1], STDOUT_FILENO) < 0
        || dup2(stderr_pipe[1], STDERR_FILENO) < 0) {
      int e = errno ? errno : 1;
      (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
      _exit(127);
    }

    rt_os_close_fd(stdin_pipe[0]);
    rt_os_close_fd(stdin_pipe[1]);
    rt_os_close_fd(stdout_pipe[0]);
    rt_os_close_fd(stdout_pipe[1]);
    rt_os_close_fd(stderr_pipe[0]);
    rt_os_close_fd(stderr_pipe[1]);
    stdin_pipe[0] = stdin_pipe[1] = -1;
    stdout_pipe[0] = stdout_pipe[1] = -1;
    stderr_pipe[0] = stderr_pipe[1] = -1;

    if (rt_os_proc_kill_tree) {
      if (setpgid(0, 0) != 0) {
        int e = errno ? errno : 1;
        (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
        _exit(127);
      }
    }

    if (cwd_len != 0) {
      if (rt_os_sandboxed) {
        uint32_t ok = 0;
        const char* cur = rt_os_proc_allow_cwd_roots;
        const char* root = NULL;
        size_t root_len = 0;
        while (rt_os_split_next(&cur, &root, &root_len)) {
          char* p = rt_os_join_root_and_rel(ctx, root, root_len, cwd_view);
          if (chdir(p) == 0) {
            ok = 1;
            break;
          }
        }
        if (!ok) {
          int e = errno ? errno : 1;
          (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
          _exit(127);
        }
      } else {
        if (chdir(cwd_cstr) != 0) {
          int e = errno ? errno : 1;
          (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
          _exit(127);
        }
      }
    }

    execve(argv[0], argv, envp);
    int e = errno ? errno : 1;
    (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
    _exit(127);
  }

  // Parent.
  if (rt_os_proc_kill_tree) pgid = pid;

  rt_os_close_fd(stdin_pipe[0]);
  stdin_pipe[0] = -1;
  rt_os_close_fd(stdout_pipe[1]);
  stdout_pipe[1] = -1;
  rt_os_close_fd(stderr_pipe[1]);
  stderr_pipe[1] = -1;
  rt_os_close_fd(exec_pipe[1]);
  exec_pipe[1] = -1;

  int child_errno = 0;
  ssize_t er = read(exec_pipe[0], &child_errno, (uint32_t)sizeof(child_errno));
  rt_os_close_fd(exec_pipe[0]);
  exec_pipe[0] = -1;
  if (er > 0) {
    (void)waitpid(pid, &status, 0);
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  stdin_fd = stdin_pipe[1];
  stdout_fd = stdout_pipe[0];
  stderr_fd = stderr_pipe[0];

  if (rt_os_set_nonblock(stdin_fd) != 0
      || rt_os_set_nonblock(stdout_fd) != 0
      || rt_os_set_nonblock(stderr_fd) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  stdout_buf = rt_bytes_alloc(ctx, max_stdout);
  stderr_buf = rt_bytes_alloc(ctx, max_stderr);

  proc->state = RT_OS_PROC_STATE_RUNNING;
  proc->mode = mode;
  proc->pid = pid;
  proc->pgid = pgid;
  proc->stdin_fd = stdin_fd;
  proc->stdout_fd = stdout_fd;
  proc->stderr_fd = stderr_fd;

  proc->stdin_closed = 0;
  proc->stdout_closed = 0;
  proc->stderr_closed = 0;

  proc->exited = 0;
  proc->status = 0;

  proc->fail_code = 0;
  proc->kill_sent = 0;

  proc->max_stdout = max_stdout;
  proc->max_stderr = max_stderr;
  proc->max_total = max_total;
  proc->timeout_ms = timeout_ms;
  proc->start_ms = rt_os_now_ms();

  proc->stdin_buf = stdin_copy;
  proc->stdin_off = 0;
  proc->stdout_buf = stdout_buf;
  proc->stdout_len = 0;
  proc->stderr_buf = stderr_buf;
  proc->stderr_len = 0;

  stdin_copy = rt_bytes_empty(ctx);
  stdout_buf = rt_bytes_empty(ctx);
  stderr_buf = rt_bytes_empty(ctx);
  rt_os_spawn_free_argv_env(ctx, argv, argv_sizes, argv_count, envp, env_sizes, env_count);
  argv = NULL;
  argv_sizes = NULL;
  envp = NULL;
  env_sizes = NULL;
  if (cwd_cstr) {
    rt_free(ctx, cwd_cstr, cwd_cstr_size, 1);
    cwd_cstr = NULL;
    cwd_cstr_size = 0;
  }
  stdin_fd = -1;
  stdout_fd = -1;
  stderr_fd = -1;

  if (mode == RT_OS_PROC_MODE_CAPTURE && proc->stdin_buf.len == 0) {
    rt_os_close_fd(proc->stdin_fd);
    proc->stdin_fd = -1;
    proc->stdin_closed = 1;
  }

  ctx->os_procs_live += 1;
  return rt_os_proc_handle_i32(idx, gen);


cleanup:
  rt_os_close_fd(stdin_fd);
  rt_os_close_fd(stdout_fd);
  rt_os_close_fd(stderr_fd);
  rt_os_close_fd(stdin_pipe[0]);
  rt_os_close_fd(stdin_pipe[1]);
  rt_os_close_fd(stdout_pipe[0]);
  rt_os_close_fd(stdout_pipe[1]);
  rt_os_close_fd(stderr_pipe[0]);
  rt_os_close_fd(stderr_pipe[1]);
  rt_os_close_fd(exec_pipe[0]);
  rt_os_close_fd(exec_pipe[1]);

  if (pid != (pid_t)-1 && err != 0) {
    (void)kill(pid, SIGKILL);
    (void)waitpid(pid, &status, 0);
  }

  rt_os_spawn_free_argv_env(ctx, argv, argv_sizes, argv_count, envp, env_sizes, env_count);
  if (cwd_cstr) rt_free(ctx, cwd_cstr, cwd_cstr_size, 1);

  rt_bytes_drop(ctx, &stdin_copy);
  rt_bytes_drop(ctx, &stdout_buf);
  rt_bytes_drop(ctx, &stderr_buf);

  if (err == 0) err = RT_OS_PROC_CODE_SPAWN_FAILED;
  rt_os_proc_mark_done(ctx, proc, idx, rt_os_proc_make_err(ctx, err));
  return rt_os_proc_handle_i32(idx, gen);
}

static int32_t rt_os_process_spawn_capture_v1(ctx_t* ctx, bytes_t req, bytes_t caps) {
  return rt_os_process_spawn_impl(ctx, req, caps, RT_OS_PROC_MODE_CAPTURE);
}

static int32_t rt_os_process_spawn_piped_v1(ctx_t* ctx, bytes_t req, bytes_t caps) {
  return rt_os_process_spawn_impl(ctx, req, caps, RT_OS_PROC_MODE_PIPED);
}

static option_bytes_t rt_os_process_try_join_capture_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    return (option_bytes_t){
      .tag = UINT32_C(1),
      .payload = rt_os_proc_make_err(ctx, sentinel_code),
    };
  }
  (void)rt_os_process_poll_all(ctx, 0);

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_CAPTURE) {
    rt_trap("os.process.try_join_capture_v1 invalid proc mode");
  }

  if (p->state != RT_OS_PROC_STATE_DONE) {
    return (option_bytes_t){ .tag = UINT32_C(0), .payload = rt_bytes_empty(ctx) };
  }
  if (p->result_taken) rt_trap("os.process join already taken");
  p->result_taken = 1;
  bytes_t out = p->result;
  p->result = rt_bytes_empty(ctx);
  return (option_bytes_t){ .tag = UINT32_C(1), .payload = out };
}

static uint32_t rt_os_process_join_capture_poll(ctx_t* ctx, int32_t handle, bytes_t* out) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    bytes_t doc = rt_os_proc_make_err(ctx, sentinel_code);
    if (out) {
      *out = doc;
    } else {
      rt_bytes_drop(ctx, &doc);
    }
    return UINT32_C(1);
  }

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_CAPTURE) {
    rt_trap("os.process.join_capture_v1 invalid proc mode");
  }

  if (p->state == RT_OS_PROC_STATE_DONE) {
    if (p->result_taken) rt_trap("os.process join already taken");
    p->result_taken = 1;
    if (out) {
      *out = p->result;
    } else {
      rt_bytes_drop(ctx, &p->result);
    }
    p->result = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("os.process.join.poll from main");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  uint32_t hid = (uint32_t)handle;
  if (me->wait_kind == RT_WAIT_OS_PROC_JOIN && me->wait_id == hid) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("os.process.join while already waiting");

  me->wait_kind = RT_WAIT_OS_PROC_JOIN;
  me->wait_id = hid;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_OS_PROC_JOIN << 32) | (uint64_t)hid);
  rt_wait_list_push(ctx, &p->join_wait_head, &p->join_wait_tail, cur);
  return UINT32_C(0);
}

static bytes_t rt_os_process_join_capture_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    return rt_os_proc_make_err(ctx, sentinel_code);
  }

  for (;;) {
    uint32_t idx = 0;
    rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
    (void)idx;

    if (p->mode != RT_OS_PROC_MODE_CAPTURE) {
      rt_trap("os.process.join_capture_v1 invalid proc mode");
    }

    if (p->state == RT_OS_PROC_STATE_DONE) {
      if (p->result_taken) rt_trap("os.process join already taken");
      p->result_taken = 1;
      bytes_t out = p->result;
      p->result = rt_bytes_empty(ctx);
      return out;
    }

    if (!rt_sched_step(ctx)) rt_sched_deadlock();
  }
}

static bytes_t rt_os_process_stdout_read_v1(ctx_t* ctx, int32_t handle, int32_t max) {
  rt_os_policy_init(ctx);
  if (max <= 0) return rt_bytes_empty(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return rt_bytes_empty(ctx);

  (void)rt_os_process_poll_all(ctx, 0);

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_PIPED) rt_trap("os.process.stdout_read_v1 invalid proc mode");



  uint32_t avail = p->stdout_len;
  if (avail == 0) {
    return rt_bytes_empty(ctx);
  }

  uint32_t want = (uint32_t)max;
  uint32_t n = (avail < want) ? avail : want;
  bytes_t out = rt_bytes_alloc(ctx, n);
  if (n != 0) {
    memcpy(out.ptr, p->stdout_buf.ptr + p->stdout_off, n);
    rt_mem_on_memcpy(ctx, n);
  }
  p->stdout_off += n;
  p->stdout_len -= n;
  if (p->stdout_len == 0) p->stdout_off = 0;

  return out;
}

static bytes_t rt_os_process_stderr_read_v1(ctx_t* ctx, int32_t handle, int32_t max) {
  rt_os_policy_init(ctx);
  if (max <= 0) return rt_bytes_empty(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return rt_bytes_empty(ctx);

  (void)rt_os_process_poll_all(ctx, 0);

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_PIPED) rt_trap("os.process.stderr_read_v1 invalid proc mode");


  uint32_t avail = p->stderr_len;
  if (avail == 0) {
    return rt_bytes_empty(ctx);
  }

  uint32_t want = (uint32_t)max;
  uint32_t n = (avail < want) ? avail : want;
  bytes_t out = rt_bytes_alloc(ctx, n);
  if (n != 0) {
    memcpy(out.ptr, p->stderr_buf.ptr + p->stderr_off, n);
    rt_mem_on_memcpy(ctx, n);
  }
  p->stderr_off += n;
  p->stderr_len -= n;
  if (p->stderr_len == 0) p->stderr_off = 0;

  return out;
}

static void rt_os_proc_stdin_append(ctx_t* ctx, rt_os_proc_t* p, bytes_t chunk) {
  if (chunk.len == 0) return;

  uint32_t pending = 0;
  if (p->stdin_off < p->stdin_buf.len) {
    pending = p->stdin_buf.len - p->stdin_off;
  }

  uint64_t total64 = (uint64_t)pending + (uint64_t)chunk.len;
  if (total64 > (uint64_t)UINT32_MAX) rt_trap("os.process.stdin_write_v1 pending overflow");
  uint32_t total = (uint32_t)total64;

  bytes_t b = rt_bytes_alloc(ctx, total);
  if (pending != 0) {
    memcpy(b.ptr, p->stdin_buf.ptr + p->stdin_off, pending);
    rt_mem_on_memcpy(ctx, pending);
  }
  memcpy(b.ptr + pending, chunk.ptr, chunk.len);
  rt_mem_on_memcpy(ctx, chunk.len);

  rt_bytes_drop(ctx, &p->stdin_buf);
  p->stdin_buf = b;
  p->stdin_off = 0;
}

static int32_t rt_os_process_stdin_write_v1(ctx_t* ctx, int32_t handle, bytes_t chunk) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 0;

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_PIPED) rt_trap("os.process.stdin_write_v1 invalid proc mode");
  if (p->state != RT_OS_PROC_STATE_RUNNING) return 0;

  if (rt_os_sandboxed && rt_os_proc_max_stdin_bytes != 0) {
    uint64_t total64 = 0;
    (void)rt_os_process_poll_all(ctx, 0);
    if (p->stdin_fd < 0 || p->stdin_closed) return 0;

    uint32_t pending = 0;
    if (p->stdin_off < p->stdin_buf.len) pending = p->stdin_buf.len - p->stdin_off;
    total64 = (uint64_t)pending + (uint64_t)chunk.len;
    if (total64 > (uint64_t)rt_os_proc_max_stdin_bytes) {
      rt_trap("os.process.stdin_write_v1 pending exceeds policy max_stdin_bytes");
    }
  }

  (void)rt_os_process_poll_all(ctx, 0);
  if (p->stdin_fd < 0 || p->stdin_closed) return 0;

  rt_os_proc_stdin_append(ctx, p, chunk);
  (void)rt_os_process_poll_all(ctx, 0);
  return (p->stdin_fd < 0 || p->stdin_closed) ? 0 : 1;
}

static int32_t rt_os_process_stdin_close_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 0;

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_PIPED) rt_trap("os.process.stdin_close_v1 invalid proc mode");
  if (p->stdin_fd < 0 || p->stdin_closed) return 0;

  rt_os_close_fd(p->stdin_fd);
  p->stdin_fd = -1;
  p->stdin_closed = 1;
  rt_bytes_drop(ctx, &p->stdin_buf);
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdin_off = 0;
  return 1;
}

static int32_t rt_os_process_try_wait_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 1;
  (void)rt_os_process_poll_all(ctx, 0);

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;
  return (p->state != RT_OS_PROC_STATE_RUNNING || p->exited) ? 1 : 0;
}

static uint32_t rt_os_process_join_exit_poll(ctx_t* ctx, int32_t handle, int32_t* out) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    if (out) *out = 1;
    return UINT32_C(1);
  }

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->state != RT_OS_PROC_STATE_RUNNING || p->exited) {
    if (out) *out = 1;
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("os.process.join_exit.poll from main");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  uint32_t hid = (uint32_t)handle;
  if (me->wait_kind == RT_WAIT_OS_PROC_EXIT && me->wait_id == hid) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("os.process.join_exit while already waiting");

  me->wait_kind = RT_WAIT_OS_PROC_EXIT;
  me->wait_id = hid;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_OS_PROC_EXIT << 32) | (uint64_t)hid);
  rt_wait_list_push(ctx, &p->exit_wait_head, &p->exit_wait_tail, cur);
  return UINT32_C(0);
}

static int32_t rt_os_process_join_exit_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) return 1;

  for (;;) {
    uint32_t idx = 0;
    rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
    (void)idx;

    if (p->state != RT_OS_PROC_STATE_RUNNING || p->exited) return 1;

    if (!rt_sched_step(ctx)) rt_sched_deadlock();
  }
}

static int32_t rt_os_process_take_exit_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    return (int32_t)(0 - (int32_t)sentinel_code);
  }

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->exit_taken) rt_trap("os.process.take_exit_v1 already taken");

  if (p->fail_code != 0) {
    p->exit_taken = 1;
    return (int32_t)(0 - (int32_t)p->fail_code);
  }

  if (p->state != RT_OS_PROC_STATE_RUNNING && p->result.len != 0 && p->result.ptr[0] == 0) {
    uint32_t code = (p->result.len >= 5) ? rt_read_u32_le(p->result.ptr + 1) : UINT32_C(0);
    if (code == 0) rt_trap("os.process.take_exit_v1 invalid error doc");
    p->exit_taken = 1;
    return (int32_t)(0 - (int32_t)code);
  }

  if (!p->exited) rt_trap("os.process.take_exit_v1 before exit");
  p->exit_taken = 1;
  return (int32_t)rt_os_proc_exit_code_from_status(p->status);
}

static int32_t rt_os_process_kill_v1(ctx_t* ctx, int32_t handle, int32_t sig) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 0;

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->state != RT_OS_PROC_STATE_RUNNING) return 0;
  if (p->pid == (pid_t)-1) return 0;
  pid_t target = rt_os_proc_kill_target(p);
  if (target == (pid_t)-1 || target == (pid_t)0) return 0;
  if (kill(target, (int)sig) != 0) return 0;
  return 1;
}

static int32_t rt_os_process_drop_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 0;

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);

  if (p->join_wait_head != 0 || p->exit_wait_head != 0) rt_trap("os.process.drop_v1 while tasks waiting");

  uint32_t was_running = (p->state == RT_OS_PROC_STATE_RUNNING) ? 1 : 0;
  if (was_running) {
    if (rt_os_sandboxed && !rt_os_proc_kill_on_drop) return 0;
    if (ctx->os_procs_live == 0) rt_trap("os.process live underflow");
    ctx->os_procs_live -= 1;
  }

  if (was_running) {
    rt_os_proc_close_fds(p);
    rt_os_proc_drop_buffers(ctx, p);
    rt_os_proc_drop_result(ctx, p);
    rt_os_proc_kill_and_reap(p);
  } else {
    rt_os_proc_close_fds(p);
    rt_os_proc_drop_buffers(ctx, p);
    rt_os_proc_drop_result(ctx, p);
  }

  uint16_t next_gen = rt_os_proc_next_gen(p->gen);
  rt_os_proc_init_entry(ctx, p, next_gen);
  return 1;
}

static uint32_t rt_os_process_poll_all(ctx_t* ctx, int poll_timeout_ms) {
  rt_os_policy_init(ctx);
  uint32_t had_live = ctx->os_procs_live ? UINT32_C(1) : UINT32_C(0);
  if (!had_live) {
    if (poll_timeout_ms > 0) (void)poll(NULL, 0, poll_timeout_ms);
    return UINT32_C(0);
  }

  if (poll_timeout_ms < 0) poll_timeout_ms = 0;

  // Update exit status and enforce timeouts.
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
    uint32_t was_exited = p->exited;
    rt_os_proc_try_wait(p);
    if (!was_exited && p->exited) {
      uint32_t reason_id = rt_os_proc_handle_u32(i, p->gen);
      rt_os_proc_wake_exit_waiters(ctx, p, reason_id);
    }

    if (p->fail_code == 0 && p->timeout_ms != 0) {
      uint64_t now_ms = rt_os_now_ms();
      if (now_ms >= p->start_ms && now_ms - p->start_ms > (uint64_t)p->timeout_ms) {
        p->fail_code = RT_OS_PROC_CODE_TIMEOUT;
        rt_os_proc_send_kill(p, SIGKILL);
        rt_os_proc_close_fds(p);
        rt_os_proc_drop_buffers(ctx, p);
      }
    }
  }

  // Build poll set.
  uint32_t nfds = 0;
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
    if (p->stdin_fd >= 0 && !p->stdin_closed && p->stdin_off < p->stdin_buf.len) nfds += 1;
    if (p->stdout_fd >= 0 && !p->stdout_closed) nfds += 1;
    if (p->stderr_fd >= 0 && !p->stderr_closed) nfds += 1;
  }

  struct pollfd* fds = NULL;
  uint32_t* tags = NULL;
  if (nfds != 0) {
    fds = (struct pollfd*)rt_alloc(ctx, nfds * (uint32_t)sizeof(struct pollfd), (uint32_t)_Alignof(struct pollfd));
    tags = (uint32_t*)rt_alloc(ctx, nfds * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));

    uint32_t at = 0;
    for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
      rt_os_proc_t* p = &ctx->os_procs[i];
      if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
      if (p->stdin_fd >= 0 && !p->stdin_closed && p->stdin_off < p->stdin_buf.len) {
        fds[at].fd = p->stdin_fd;
        fds[at].events = POLLOUT;
        fds[at].revents = 0;
        tags[at] = (i << 2) | RT_OS_PROC_POLL_KIND_STDIN;
        at += 1;
      }
      if (p->stdout_fd >= 0 && !p->stdout_closed) {
        fds[at].fd = p->stdout_fd;
        fds[at].events = POLLIN;
        fds[at].revents = 0;
        tags[at] = (i << 2) | RT_OS_PROC_POLL_KIND_STDOUT;
        at += 1;
      }
      if (p->stderr_fd >= 0 && !p->stderr_closed) {
        fds[at].fd = p->stderr_fd;
        fds[at].events = POLLIN;
        fds[at].revents = 0;
        tags[at] = (i << 2) | RT_OS_PROC_POLL_KIND_STDERR;
        at += 1;
      }
    }

    int pr = poll(fds, (nfds_t)nfds, poll_timeout_ms);
    if (pr < 0) {
      if (errno != EINTR) rt_trap("os.process.poll_all poll failed");
    }

    for (uint32_t i = 0; i < nfds; i++) {
      short re = fds[i].revents;
      if (re == 0) continue;

      uint32_t tag = tags[i];
      uint32_t pidx = tag >> 2;
      uint32_t kind = tag & 3;
      if (pidx >= ctx->os_procs_len) continue;
      rt_os_proc_t* p = &ctx->os_procs[pidx];
      if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
      if (p->fail_code != 0) continue;

      if (kind == RT_OS_PROC_POLL_KIND_STDIN) {
        if (p->stdin_fd < 0 || p->stdin_closed) continue;
        if (p->stdin_off >= p->stdin_buf.len) {
          rt_bytes_drop(ctx, &p->stdin_buf);
          p->stdin_buf = rt_bytes_empty(ctx);
          p->stdin_off = 0;
          if (p->mode == RT_OS_PROC_MODE_CAPTURE) {
            rt_os_close_fd(p->stdin_fd);
            p->stdin_fd = -1;
            p->stdin_closed = 1;
          }
          continue;
        }

        ssize_t n = write(p->stdin_fd, p->stdin_buf.ptr + p->stdin_off, p->stdin_buf.len - p->stdin_off);
        if (n > 0) {
          p->stdin_off += (uint32_t)n;
          if (p->stdin_off >= p->stdin_buf.len) {
            rt_bytes_drop(ctx, &p->stdin_buf);
            p->stdin_buf = rt_bytes_empty(ctx);
            p->stdin_off = 0;
            if (p->mode == RT_OS_PROC_MODE_CAPTURE) {
              rt_os_close_fd(p->stdin_fd);
              p->stdin_fd = -1;
              p->stdin_closed = 1;
            }
          }
          continue;
        }
        if (n < 0) {
          if (errno == EINTR || errno == EAGAIN) continue;
          if (errno == EPIPE) {
            rt_os_close_fd(p->stdin_fd);
            p->stdin_fd = -1;
            p->stdin_closed = 1;
            rt_bytes_drop(ctx, &p->stdin_buf);
            p->stdin_buf = rt_bytes_empty(ctx);
            continue;
          }
          p->fail_code = RT_OS_PROC_CODE_SPAWN_FAILED;
          rt_os_proc_send_kill(p, SIGKILL);
          rt_os_proc_close_fds(p);
          rt_os_proc_drop_buffers(ctx, p);
          continue;
        }
      }

      if (kind == RT_OS_PROC_POLL_KIND_STDOUT) {
        if (p->stdout_fd < 0 || p->stdout_closed) continue;

        uint32_t total_len = p->stdout_len + p->stderr_len;
        uint32_t total_rem = (total_len < p->max_total) ? (p->max_total - total_len) : 0;
        uint32_t rem_total = (p->stdout_len < p->max_stdout) ? (p->max_stdout - p->stdout_len) : 0;
        if (rem_total > total_rem) rem_total = total_rem;
        if (rem_total == 0) {
          p->fail_code = RT_OS_PROC_CODE_OUTPUT_LIMIT;
          rt_os_proc_send_kill(p, SIGKILL);
          rt_os_proc_close_fds(p);
          rt_os_proc_drop_buffers(ctx, p);
          continue;
        }

        if (p->stdout_off != 0 && p->stdout_off + p->stdout_len == p->max_stdout) {
          memmove(p->stdout_buf.ptr, p->stdout_buf.ptr + p->stdout_off, p->stdout_len);
          rt_mem_on_memcpy(ctx, p->stdout_len);
          p->stdout_off = 0;
        }
        uint32_t cont_rem = p->max_stdout - (p->stdout_off + p->stdout_len);
        uint32_t rem = rem_total;
        if (rem > cont_rem) rem = cont_rem;

        ssize_t n = read(p->stdout_fd, p->stdout_buf.ptr + p->stdout_off + p->stdout_len, rem);
        if (n > 0) {
          p->stdout_len += (uint32_t)n;
          continue;
        }
        if (n == 0) {
          rt_os_close_fd(p->stdout_fd);
          p->stdout_fd = -1;
          p->stdout_closed = 1;
          continue;
        }
        if (errno == EINTR || errno == EAGAIN) continue;
        p->fail_code = RT_OS_PROC_CODE_SPAWN_FAILED;
        rt_os_proc_send_kill(p, SIGKILL);
        rt_os_proc_close_fds(p);
        rt_os_proc_drop_buffers(ctx, p);
        continue;
      }

      if (kind == RT_OS_PROC_POLL_KIND_STDERR) {
        if (p->stderr_fd < 0 || p->stderr_closed) continue;

        uint32_t total_len = p->stdout_len + p->stderr_len;
        uint32_t total_rem = (total_len < p->max_total) ? (p->max_total - total_len) : 0;
        uint32_t rem_total = (p->stderr_len < p->max_stderr) ? (p->max_stderr - p->stderr_len) : 0;
        if (rem_total > total_rem) rem_total = total_rem;
        if (rem_total == 0) {
          p->fail_code = RT_OS_PROC_CODE_OUTPUT_LIMIT;
          rt_os_proc_send_kill(p, SIGKILL);
          rt_os_proc_close_fds(p);
          rt_os_proc_drop_buffers(ctx, p);
          continue;
        }

        if (p->stderr_off != 0 && p->stderr_off + p->stderr_len == p->max_stderr) {
          memmove(p->stderr_buf.ptr, p->stderr_buf.ptr + p->stderr_off, p->stderr_len);
          rt_mem_on_memcpy(ctx, p->stderr_len);
          p->stderr_off = 0;
        }
        uint32_t cont_rem = p->max_stderr - (p->stderr_off + p->stderr_len);
        uint32_t rem = rem_total;
        if (rem > cont_rem) rem = cont_rem;

        ssize_t n = read(p->stderr_fd, p->stderr_buf.ptr + p->stderr_off + p->stderr_len, rem);
        if (n > 0) {
          p->stderr_len += (uint32_t)n;
          continue;
        }
        if (n == 0) {
          rt_os_close_fd(p->stderr_fd);
          p->stderr_fd = -1;
          p->stderr_closed = 1;
          continue;
        }
        if (errno == EINTR || errno == EAGAIN) continue;
        p->fail_code = RT_OS_PROC_CODE_SPAWN_FAILED;
        rt_os_proc_send_kill(p, SIGKILL);
        rt_os_proc_close_fds(p);
        rt_os_proc_drop_buffers(ctx, p);
        continue;
      }
    }
  } else {
    (void)poll(NULL, 0, poll_timeout_ms);
  }

  // Finalize completed processes.
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state != RT_OS_PROC_STATE_RUNNING) continue;

    uint32_t was_exited = p->exited;
    rt_os_proc_try_wait(p);
    if (!was_exited && p->exited) {
      uint32_t reason_id = rt_os_proc_handle_u32(i, p->gen);
      rt_os_proc_wake_exit_waiters(ctx, p, reason_id);
    }

    if (p->fail_code != 0) {
      rt_os_proc_send_kill(p, SIGKILL);
      if (p->exited) {
        rt_os_proc_close_fds(p);
        rt_os_proc_finish_err(ctx, p, i, p->fail_code);
      }
      continue;
    }

    if (p->exited && p->stdin_closed && p->stdout_closed && p->stderr_closed) {
      rt_os_proc_close_fds(p);
      if (p->mode == RT_OS_PROC_MODE_CAPTURE) {
        rt_os_proc_finish_ok(ctx, p, i);
      } else {
        rt_os_proc_finish_piped(ctx, p);
      }
    }
  }

  if (fds) rt_free(ctx, fds, nfds * (uint32_t)sizeof(struct pollfd), (uint32_t)_Alignof(struct pollfd));
  if (tags) rt_free(ctx, tags, nfds * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));

  return UINT32_C(1);
}

static void rt_os_process_cleanup(ctx_t* ctx) {
  rt_os_policy_init(ctx);
  if (!ctx->os_procs) {
    ctx->os_procs_len = 0;
    ctx->os_procs_cap = 0;
    ctx->os_procs_live = 0;
    ctx->os_procs_spawned = 0;
    return;
  }

  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state == RT_OS_PROC_STATE_EMPTY) continue;

    if (p->state == RT_OS_PROC_STATE_RUNNING) {
      rt_os_proc_close_fds(p);
      rt_os_proc_drop_buffers(ctx, p);
      rt_os_proc_drop_result(ctx, p);
      rt_os_proc_kill_and_reap(p);
    } else {
      rt_os_proc_close_fds(p);
      rt_os_proc_drop_buffers(ctx, p);
      rt_os_proc_drop_result(ctx, p);
    }

    uint16_t next_gen = rt_os_proc_next_gen(p->gen);
    rt_os_proc_init_entry(ctx, p, next_gen);
  }

  if (ctx->os_procs_cap) {
    rt_free(
      ctx,
      ctx->os_procs,
      ctx->os_procs_cap * (uint32_t)sizeof(rt_os_proc_t),
      (uint32_t)_Alignof(rt_os_proc_t)
    );
  }
  ctx->os_procs = NULL;
  ctx->os_procs_len = 0;
  ctx->os_procs_cap = 0;
  ctx->os_procs_live = 0;
  ctx->os_procs_spawned = 0;
}

static bytes_t rt_os_process_run_capture_v1(ctx_t* ctx, bytes_t req, bytes_t caps) {
  int32_t h = rt_os_process_spawn_capture_v1(ctx, req, caps);
  bytes_t out = rt_os_process_join_capture_v1(ctx, h);
  (void)rt_os_process_drop_v1(ctx, h);
  return out;
}

static __attribute__((noreturn)) void rt_os_process_exit(ctx_t* ctx, int32_t code) {
  rt_os_policy_init(ctx);
  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_proc_enabled, "os.process disabled by policy");
    rt_os_require(ctx, rt_os_proc_allow_exit, "os.process.exit disabled by policy");
  }
  (void)ctx;
  exit((int)code);
}

static bytes_t rt_os_net_http_request(ctx_t* ctx, bytes_t req) {
  rt_os_policy_init(ctx);
  (void)req;
  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_net_enabled, "os.net disabled by policy");
    rt_os_require(ctx, rt_os_net_allow_tcp, "os.net.http_request disabled by policy");
    rt_os_require(ctx, rt_os_net_allow_dns, "os.net.http_request disabled by policy");
    rt_os_require(ctx, rt_os_net_allow_hosts && *rt_os_net_allow_hosts, "os.net.allow_hosts required");
  }
  rt_trap("os.net.http_request not implemented");
}
"#;
