use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::ast::Expr;
use crate::compile::{CompileErrorKind, CompileOptions, CompilerError};
use crate::program::{AsyncFunctionDef, FunctionDef, FunctionParam, Program};
use crate::types::Ty;
use crate::validate;
use crate::x07ast;

pub fn elaborate_stream_pipes(
    program: &mut Program,
    options: &CompileOptions,
    module_metas: &BTreeMap<String, BTreeMap<String, Value>>,
) -> Result<(), CompilerError> {
    let fn_sigs = PipeFnSigsV1::build(program);
    let mut existing_names: BTreeSet<String> = BTreeSet::new();
    for f in &program.functions {
        existing_names.insert(f.name.clone());
    }
    for f in &program.async_functions {
        existing_names.insert(f.name.clone());
    }
    for f in &program.extern_functions {
        existing_names.insert(f.name.clone());
    }

    let mut elab = Elaborator {
        options,
        module_metas,
        fn_sigs: &fn_sigs,
        existing_names,
        helpers: BTreeMap::new(),
        new_helpers: Vec::new(),
        new_async_helpers: Vec::new(),
        stream_plugin_registry: None,
    };

    program.solve = elab.rewrite_expr(program.solve.clone(), "main", RewriteCtx::Solve)?;
    for f in &mut program.functions {
        let module_id = function_module_id(&f.name)?;
        f.body = elab.rewrite_expr(f.body.clone(), module_id, RewriteCtx::Defn)?;
    }
    for f in &mut program.async_functions {
        let module_id = function_module_id(&f.name)?;
        f.body = elab.rewrite_expr(f.body.clone(), module_id, RewriteCtx::Defasync)?;
    }

    program.functions.extend(elab.new_helpers);
    program.async_functions.extend(elab.new_async_helpers);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RewriteCtx {
    Solve,
    Defn,
    Defasync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipeHelperKind {
    Defn,
    Defasync,
}

#[derive(Debug, Clone)]
struct PipeHelperInfo {
    name: String,
    kind: PipeHelperKind,
}

#[derive(Debug, Clone)]
struct PipeFnSigV1 {
    params: Vec<FunctionParam>,
    ret_ty: Ty,
    ret_brand: Option<String>,
}

#[derive(Debug, Clone)]
struct PipeFnSigsV1 {
    defns: BTreeMap<String, PipeFnSigV1>,
    defasyncs: BTreeMap<String, PipeFnSigV1>,
}

impl PipeFnSigsV1 {
    fn build(program: &Program) -> Self {
        let mut defns: BTreeMap<String, PipeFnSigV1> = BTreeMap::new();
        for f in &program.functions {
            let _ = defns.insert(
                f.name.clone(),
                PipeFnSigV1 {
                    params: f.params.clone(),
                    ret_ty: f.ret_ty,
                    ret_brand: f.ret_brand.clone(),
                },
            );
        }
        for f in &program.extern_functions {
            let _ = defns.insert(
                f.name.clone(),
                PipeFnSigV1 {
                    params: f.params.clone(),
                    ret_ty: f.ret_ty,
                    ret_brand: None,
                },
            );
        }

        let mut defasyncs: BTreeMap<String, PipeFnSigV1> = BTreeMap::new();
        for f in &program.async_functions {
            let _ = defasyncs.insert(
                f.name.clone(),
                PipeFnSigV1 {
                    params: f.params.clone(),
                    ret_ty: f.ret_ty,
                    ret_brand: f.ret_brand.clone(),
                },
            );
        }

        Self { defns, defasyncs }
    }

    fn defn(&self, fn_id: &str) -> Option<&PipeFnSigV1> {
        self.defns.get(fn_id)
    }

    fn defasync(&self, fn_id: &str) -> Option<&PipeFnSigV1> {
        self.defasyncs.get(fn_id)
    }
}

struct Elaborator<'a> {
    options: &'a CompileOptions,
    module_metas: &'a BTreeMap<String, BTreeMap<String, Value>>,
    fn_sigs: &'a PipeFnSigsV1,
    existing_names: BTreeSet<String>,
    helpers: BTreeMap<(String, String), PipeHelperInfo>,
    new_helpers: Vec<FunctionDef>,
    new_async_helpers: Vec<AsyncFunctionDef>,
    stream_plugin_registry: Option<StreamPluginRegistryV1>,
}

impl Elaborator<'_> {
    fn rewrite_expr(
        &mut self,
        expr: Expr,
        module_id: &str,
        ctx: RewriteCtx,
    ) -> Result<Expr, CompilerError> {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => Ok(expr),
            Expr::List { items, ptr } => {
                if items.first().and_then(Expr::as_ident) == Some("std.stream.pipe_v1") {
                    return self.rewrite_pipe(Expr::List { items, ptr }, module_id, ctx);
                }

                let mut new_items = Vec::with_capacity(items.len());
                for item in items {
                    new_items.push(self.rewrite_expr(item, module_id, ctx)?);
                }
                Ok(Expr::List {
                    items: new_items,
                    ptr,
                })
            }
        }
    }

    fn rewrite_pipe(
        &mut self,
        expr: Expr,
        module_id: &str,
        ctx: RewriteCtx,
    ) -> Result<Expr, CompilerError> {
        let h8 = hash_pipe_without_expr_bodies(&expr)?;
        let mut parsed = parse_pipe_v1(&expr)?;
        self.resolve_stream_plugins_v1(&mut parsed)?;
        self.typecheck_and_rewrite_item_brands_v1(&mut parsed, module_id)?;
        let needs_async = parsed
            .chain
            .iter()
            .any(|xf| matches!(xf.kind, PipeXfV1::ParMapStreamV1 { .. }));

        let helper_full = format!("{module_id}.__std_stream_pipe_v1_{h8}");
        let helper_key = (module_id.to_string(), h8.clone());

        let helper = if let Some(info) = self.helpers.get(&helper_key) {
            info.clone()
        } else {
            if self.existing_names.contains(&helper_full) {
                return Err(CompilerError::new(
                    CompileErrorKind::Parse,
                    format!("pipe helper name collision: {helper_full:?}"),
                ));
            }
            validate_pipe_world_caps(&parsed, self.options)?;
            let body = gen_pipe_helper_body(&parsed, self.options)?;
            let kind = if needs_async {
                PipeHelperKind::Defasync
            } else {
                PipeHelperKind::Defn
            };
            match kind {
                PipeHelperKind::Defn => {
                    self.new_helpers.push(FunctionDef {
                        name: helper_full.clone(),
                        params: parsed
                            .params
                            .iter()
                            .enumerate()
                            .map(|(idx, p)| FunctionParam {
                                name: format!("p{idx}"),
                                ty: p.ty,
                                brand: None,
                            })
                            .collect(),
                        ret_ty: Ty::Bytes,
                        ret_brand: None,
                        body,
                    });
                }
                PipeHelperKind::Defasync => {
                    self.new_async_helpers.push(AsyncFunctionDef {
                        name: helper_full.clone(),
                        params: parsed
                            .params
                            .iter()
                            .enumerate()
                            .map(|(idx, p)| FunctionParam {
                                name: format!("p{idx}"),
                                ty: p.ty,
                                brand: None,
                            })
                            .collect(),
                        ret_ty: Ty::Bytes,
                        ret_brand: None,
                        body,
                    });
                }
            }
            let helper = PipeHelperInfo {
                name: helper_full.clone(),
                kind,
            };
            self.existing_names.insert(helper_full.clone());
            self.helpers.insert(helper_key, helper.clone());
            helper
        };

        if helper.kind == PipeHelperKind::Defasync && ctx == RewriteCtx::Defn {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "std.stream.pipe_v1 with concurrency stages is only allowed in solve or defasync"
                    .to_string(),
            ));
        }

        let mut begin_items: Vec<Expr> = vec![expr_ident("begin")];
        let mut arg_names: Vec<String> = Vec::with_capacity(parsed.params.len());
        for (idx, param) in parsed.params.iter().enumerate() {
            let arg_name = format!("__std_stream_pipe_v1_{h8}_arg{idx}");
            let arg_expr = self.rewrite_expr(param.expr.clone(), module_id, ctx)?;
            begin_items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(arg_name.clone()),
                arg_expr,
            ]));
            arg_names.push(arg_name);
        }

        let mut call_items: Vec<Expr> = Vec::with_capacity(1 + arg_names.len());
        call_items.push(expr_ident(helper.name));
        for name in arg_names {
            call_items.push(expr_ident(name));
        }
        let call_expr = expr_list(call_items);
        begin_items.push(match helper.kind {
            PipeHelperKind::Defn => call_expr,
            PipeHelperKind::Defasync => expr_list(vec![expr_ident("await"), call_expr]),
        });
        Ok(expr_list(begin_items))
    }

    fn typecheck_and_rewrite_item_brands_v1(
        &self,
        pipe: &mut PipeParsed,
        module_id: &str,
    ) -> Result<(), CompilerError> {
        let registry = match pipe.cfg.brand_registry_ref_v1.as_deref() {
            Some(registry_module_id) => {
                load_brand_registry_v1(registry_module_id, self.module_metas)?
            }
            None => load_brand_registry_optional_v1(module_id, self.module_metas)?,
        };

        // Resolve require_brand_v1 validators (always, even if typecheck is disabled).
        for xf in pipe.chain.iter_mut() {
            let PipeXfV1::RequireBrandV1 {
                brand_id,
                validator_id,
                ..
            } = &mut xf.kind
            else {
                continue;
            };

            let resolved = match validator_id.as_deref() {
                Some(v) => v.to_string(),
                None => registry.get(brand_id).cloned().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_BRAND_REGISTRY_MISSING: brand {brand_id:?} is missing meta.brands_v1.validate (ptr={})",
                            xf.ptr
                        ),
                    )
                })?,
            };
            ensure_brand_validator_sig_v1(&resolved, self.fn_sigs, &xf.ptr)?;
            *validator_id = Some(resolved);
        }

        if pipe.cfg.typecheck_item_brands_v1 == 0 {
            return Ok(());
        }

        let auto = pipe.cfg.auto_require_brand_v1 != 0;
        let verify_produced = pipe.cfg.verify_produced_brands_v1 != 0;

        let mut cur_brand: Option<String> = pipe.src.out_item_brand.clone();
        let mut new_chain: Vec<PipeXfDescV1> = Vec::with_capacity(pipe.chain.len());

        if verify_produced {
            if let Some(b) = &cur_brand {
                let validator_id = registry.get(b).cloned().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_BRAND_REGISTRY_MISSING: brand {b:?} is missing meta.brands_v1.validate (ptr={})",
                            pipe.src.ptr
                        ),
                    )
                })?;
                ensure_brand_validator_sig_v1(&validator_id, self.fn_sigs, &pipe.src.ptr)?;
                new_chain.push(PipeXfDescV1 {
                    kind: PipeXfV1::RequireBrandV1 {
                        brand_id: b.clone(),
                        validator_id: Some(validator_id),
                        max_item_bytes: 0,
                    },
                    in_item_brand: None,
                    out_item_brand: None,
                    ptr: pipe.src.ptr.clone(),
                });
            }
        }

        for xf in pipe.chain.iter() {
            let req_in = infer_xf_req_in_v1(xf, self.fn_sigs)?;
            match &req_in {
                PipeItemBrandInV1::Any => {}
                PipeItemBrandInV1::Same => {
                    if cur_brand.is_none() {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!(
                                "E_PIPE_BRAND_REQUIRED: stage requires branded items, got unbranded (ptr={})",
                                xf.ptr
                            ),
                        ));
                    }
                }
                PipeItemBrandInV1::Brand(want) => match &cur_brand {
                    Some(got) if got == want => {}
                    Some(got) => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!(
                                "E_PIPE_BRAND_MISMATCH: expected bytes_view@{want}, got bytes_view@{got} (ptr={})",
                                xf.ptr
                            ),
                        ));
                    }
                    None => {
                        if auto {
                            let validator_id = registry.get(want).cloned().ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!(
                                        "E_PIPE_BRAND_REQUIRED: expected bytes_view@{want}, got unbranded; missing meta.brands_v1.validate and auto_require_brand_v1 cannot insert require_brand (ptr={})",
                                        xf.ptr
                                    ),
                                )
                            })?;
                            ensure_brand_validator_sig_v1(&validator_id, self.fn_sigs, &xf.ptr)?;
                            new_chain.push(PipeXfDescV1 {
                                kind: PipeXfV1::RequireBrandV1 {
                                    brand_id: want.clone(),
                                    validator_id: Some(validator_id),
                                    max_item_bytes: 0,
                                },
                                in_item_brand: None,
                                out_item_brand: None,
                                ptr: xf.ptr.clone(),
                            });
                            cur_brand = Some(want.clone());
                        } else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "E_PIPE_BRAND_REQUIRED: expected bytes_view@{want}, got unbranded (ptr={})",
                                    xf.ptr
                                ),
                            ));
                        }
                    }
                },
            }

            let mut xf_out = xf.clone();
            if let PipeXfV1::RequireBrandV1 {
                brand_id,
                validator_id,
                ..
            } = &mut xf_out.kind
            {
                let resolved = validator_id.as_deref().or_else(|| registry.get(brand_id).map(|s| s.as_str())).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_BRAND_REGISTRY_MISSING: brand {brand_id:?} is missing meta.brands_v1.validate (ptr={})",
                            xf.ptr
                        ),
                    )
                })?;
                ensure_brand_validator_sig_v1(resolved, self.fn_sigs, &xf.ptr)?;
                *validator_id = Some(resolved.to_string());
            }

            new_chain.push(xf_out.clone());

            let (out, produced_claim) = infer_xf_out_v1(&xf_out, self.fn_sigs)?;
            match &out {
                PipeItemBrandOutV1::Same => {}
                PipeItemBrandOutV1::None => cur_brand = None,
                PipeItemBrandOutV1::Brand(b) => cur_brand = Some(b.clone()),
            }

            if verify_produced && produced_claim {
                let Some(b) = &cur_brand else {
                    continue;
                };
                let validator_id = registry.get(b).cloned().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_BRAND_REGISTRY_MISSING: brand {b:?} is missing meta.brands_v1.validate (ptr={})",
                            xf.ptr
                        ),
                    )
                })?;
                ensure_brand_validator_sig_v1(&validator_id, self.fn_sigs, &xf.ptr)?;
                new_chain.push(PipeXfDescV1 {
                    kind: PipeXfV1::RequireBrandV1 {
                        brand_id: b.clone(),
                        validator_id: Some(validator_id),
                        max_item_bytes: 0,
                    },
                    in_item_brand: None,
                    out_item_brand: None,
                    ptr: xf.ptr.clone(),
                });
            }
        }

        if let Some(req) = &pipe.sink.in_item_brand {
            match req {
                PipeItemBrandInV1::Any => {}
                PipeItemBrandInV1::Same => {
                    return Err(CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: sink in_item_brand cannot be \"same\"".to_string(),
                    ));
                }
                PipeItemBrandInV1::Brand(want) => match &cur_brand {
                    Some(got) if got == want => {}
                    Some(got) => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!(
                                "E_PIPE_BRAND_MISMATCH: sink expected bytes_view@{want}, got bytes_view@{got} (ptr={})",
                                pipe.sink.ptr
                            ),
                        ));
                    }
                    None => {
                        if auto {
                            let validator_id = registry.get(want).cloned().ok_or_else(|| {
                                CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!(
                                        "E_PIPE_BRAND_REQUIRED: sink expected bytes_view@{want}, got unbranded; missing meta.brands_v1.validate and auto_require_brand_v1 cannot insert require_brand (ptr={})",
                                        pipe.sink.ptr
                                    ),
                                )
                            })?;
                            ensure_brand_validator_sig_v1(
                                &validator_id,
                                self.fn_sigs,
                                &pipe.sink.ptr,
                            )?;
                            new_chain.push(PipeXfDescV1 {
                                kind: PipeXfV1::RequireBrandV1 {
                                    brand_id: want.clone(),
                                    validator_id: Some(validator_id),
                                    max_item_bytes: 0,
                                },
                                in_item_brand: None,
                                out_item_brand: None,
                                ptr: pipe.sink.ptr.clone(),
                            });
                        } else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "E_PIPE_BRAND_REQUIRED: sink expected bytes_view@{want}, got unbranded (ptr={})",
                                    pipe.sink.ptr
                                ),
                            ));
                        }
                    }
                },
            }
        }

        pipe.chain = new_chain;
        Ok(())
    }

    fn resolve_stream_plugins_v1(&mut self, pipe: &mut PipeParsed) -> Result<(), CompilerError> {
        let needs_registry = pipe
            .chain
            .iter()
            .any(|xf| matches!(xf.kind, PipeXfV1::PluginV1 { resolved: None, .. }));
        if !needs_registry {
            return Ok(());
        }

        let Some(arch_root) = self.options.arch_root.as_ref() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "E_PIPE_PLUGIN_NEEDS_ARCH_ROOT: std.stream.xf.plugin_v1 requires compile_options.arch_root"
                    .to_string(),
            ));
        };

        if self.stream_plugin_registry.is_none() {
            self.stream_plugin_registry = Some(StreamPluginRegistryV1::load(arch_root)?);
        }
        let registry = self.stream_plugin_registry.as_ref().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: missing stream plugin registry".to_string(),
            )
        })?;

        let world = self.options.world.as_str();

        for xf in pipe.chain.iter_mut() {
            let PipeXfV1::PluginV1 {
                plugin_id,
                resolved,
                ..
            } = &mut xf.kind
            else {
                continue;
            };
            if resolved.is_some() {
                continue;
            }

            let p = registry.by_id.get(plugin_id).ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_NOT_FOUND: stream plugin_id {plugin_id:?} is not declared in arch/stream/plugins/index.x07sp.json (ptr={})",
                        xf.ptr
                    ),
                )
            })?;

            if !p.worlds_allowed.is_empty() && !p.worlds_allowed.iter().any(|w| w == world) {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!(
                        "E_PIPE_PLUGIN_WORLD_VIOLATION: stream plugin_id {plugin_id:?} is not allowed in world {world} (ptr={})",
                        xf.ptr
                    ),
                ));
            }

            if p.determinism == StreamPluginDeterminismV1::NondetOsOnlyV1
                && !matches!(world, "run-os" | "run-os-sandboxed")
            {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!(
                        "E_PIPE_PLUGIN_WORLD_VIOLATION: OS-only stream plugin_id {plugin_id:?} is not allowed in solve worlds (ptr={})",
                        xf.ptr
                    ),
                ));
            }

            xf.in_item_brand = Some(p.in_item_brand.clone());
            xf.out_item_brand = Some(p.out_item_brand.clone());
            *resolved = Some(p.clone());
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct StreamPluginRegistryV1 {
    by_id: BTreeMap<String, StreamPluginResolvedV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamPluginDeterminismV1 {
    DeterministicV1,
    NondetOsOnlyV1,
}

#[derive(Debug, Clone)]
struct StreamPluginCfgV1 {
    max_bytes: u32,
    canon_mode: StreamPluginCfgCanonModeV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamPluginCfgCanonModeV1 {
    NoneV1,
    CanonJsonV1,
}

#[derive(Debug, Clone)]
struct StreamPluginLimitsV1 {
    max_out_bytes_per_step: u32,
    max_out_items_per_step: u32,
    max_out_buf_bytes: u32,
}

#[derive(Debug, Clone)]
struct StreamPluginBudgetsV1 {
    state_bytes: u32,
    scratch_bytes: u32,
}

#[derive(Debug, Clone)]
struct StreamPluginResolvedV1 {
    plugin_id: String,
    native_backend_id: String,
    abi_major: u32,
    export_symbol: String,
    budget_profile_id: String,
    determinism: StreamPluginDeterminismV1,
    worlds_allowed: Vec<String>,
    in_item_brand: PipeItemBrandInV1,
    out_item_brand: PipeItemBrandOutV1,
    budgets: StreamPluginBudgetsV1,
    cfg: StreamPluginCfgV1,
    limits: StreamPluginLimitsV1,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamPluginsIndexFileV1 {
    schema_version: String,
    #[serde(default)]
    plugins: Vec<StreamPluginsIndexEntryV1>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamPluginsIndexEntryV1 {
    plugin_id: String,
    plugin_spec_path: String,
    budget_profile_id: String,
    native_backend_id: String,
    abi_major: u32,
    export_symbol: String,
    determinism: String,
    #[serde(default)]
    worlds_allowed: Vec<String>,
    in_item_brand: String,
    out_item_brand: String,
    state_bytes: u32,
    scratch_bytes: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamPluginSpecFileV1 {
    schema_version: String,
    plugin_id: String,
    v: u32,
    abi: StreamPluginSpecAbiV1,
    determinism: String,
    #[serde(default)]
    worlds_allowed: Vec<String>,
    brands: StreamPluginSpecBrandsV1,
    budgets: StreamPluginSpecBudgetsV1,
    cfg: StreamPluginSpecCfgV1,
    limits: StreamPluginSpecLimitsV1,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamPluginSpecAbiV1 {
    native_backend_id: String,
    abi_major: u32,
    export_symbol: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamPluginSpecBrandsV1 {
    in_item_brand: String,
    out_item_brand: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamPluginSpecBudgetsV1 {
    state_bytes: u32,
    scratch_bytes: u32,
    #[serde(default)]
    max_expand_ratio: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamPluginSpecCfgV1 {
    max_bytes: u32,
    canon_mode: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct StreamPluginSpecLimitsV1 {
    max_out_bytes_per_step: u32,
    max_out_items_per_step: u32,
    max_out_buf_bytes: u32,
}

const BUILTIN_STREAM_PLUGINS_INDEX_V1: &str =
    include_str!("../../../arch/stream/plugins/index.x07sp.json");

fn builtin_stream_plugin_spec_source_v1(plugin_spec_path: &str) -> Option<&'static str> {
    match plugin_spec_path {
        "arch/stream/plugins/specs/xf.deframe_u32le_v1.x07sp-plugin.json" => Some(include_str!(
            "../../../arch/stream/plugins/specs/xf.deframe_u32le_v1.x07sp-plugin.json"
        )),
        "arch/stream/plugins/specs/xf.frame_u32le_v1.x07sp-plugin.json" => Some(include_str!(
            "../../../arch/stream/plugins/specs/xf.frame_u32le_v1.x07sp-plugin.json"
        )),
        "arch/stream/plugins/specs/xf.json_canon_stream_v1.x07sp-plugin.json" => {
            Some(include_str!(
                "../../../arch/stream/plugins/specs/xf.json_canon_stream_v1.x07sp-plugin.json"
            ))
        }
        "arch/stream/plugins/specs/xf.split_lines_v1.x07sp-plugin.json" => Some(include_str!(
            "../../../arch/stream/plugins/specs/xf.split_lines_v1.x07sp-plugin.json"
        )),
        "arch/stream/plugins/specs/xf.test_emit_limits_v1.x07sp-plugin.json" => Some(include_str!(
            "../../../arch/stream/plugins/specs/xf.test_emit_limits_v1.x07sp-plugin.json"
        )),
        _ => None,
    }
}

impl StreamPluginRegistryV1 {
    fn load(arch_root: &std::path::Path) -> Result<Self, CompilerError> {
        let index_path = arch_root
            .join("arch")
            .join("stream")
            .join("plugins")
            .join("index.x07sp.json");

        let (index_bytes, index_display_path, fs_plugin_root) = match std::fs::read(&index_path) {
            Ok(bytes) => (
                bytes,
                index_path.display().to_string(),
                Some(arch_root.to_path_buf()),
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
                BUILTIN_STREAM_PLUGINS_INDEX_V1.as_bytes().to_vec(),
                "<builtin>/arch/stream/plugins/index.x07sp.json".to_string(),
                None,
            ),
            Err(e) => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_INDEX_READ_FAILED: failed to read {}: {e}",
                        index_path.display()
                    ),
                ));
            }
        };

        let index: StreamPluginsIndexFileV1 =
            serde_json::from_slice(&index_bytes).map_err(|e| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_INDEX_INVALID: failed to parse {}: {e}",
                        index_display_path
                    ),
                )
            })?;

        if index.schema_version != x07_contracts::X07_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_VERSION {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "E_PIPE_PLUGIN_INDEX_INVALID: expected schema_version {:?} got {:?} in {}",
                    x07_contracts::X07_ARCH_STREAM_PLUGINS_INDEX_SCHEMA_VERSION,
                    index.schema_version,
                    index_display_path
                ),
            ));
        }

        let mut by_id: BTreeMap<String, StreamPluginResolvedV1> = BTreeMap::new();

        for entry in &index.plugins {
            validate::validate_symbol(&entry.plugin_id)
                .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
            validate::validate_symbol(&entry.native_backend_id)
                .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
            validate::validate_symbol(&entry.budget_profile_id)
                .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
            validate::validate_local_name(&entry.export_symbol)
                .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

            let (spec_bytes, spec_display_path) = if let Some(plugin_root) = fs_plugin_root.as_ref()
            {
                let spec_path = plugin_root.join(&entry.plugin_spec_path);
                let spec_bytes = std::fs::read(&spec_path).map_err(|e| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_PLUGIN_SPEC_READ_FAILED: failed to read {}: {e}",
                            spec_path.display()
                        ),
                    )
                })?;
                (spec_bytes, spec_path.display().to_string())
            } else {
                let Some(spec_src) = builtin_stream_plugin_spec_source_v1(&entry.plugin_spec_path)
                else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_PLUGIN_SPEC_READ_FAILED: unknown builtin stream plugin spec path {:?}",
                            entry.plugin_spec_path
                        ),
                    ));
                };
                (
                    spec_src.as_bytes().to_vec(),
                    format!("<builtin>/{}", entry.plugin_spec_path),
                )
            };

            let spec: StreamPluginSpecFileV1 =
                serde_json::from_slice(&spec_bytes).map_err(|e| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_PLUGIN_SPEC_INVALID: failed to parse {}: {e}",
                            spec_display_path
                        ),
                    )
                })?;

            if spec.schema_version != x07_contracts::X07_ARCH_STREAM_PLUGIN_SCHEMA_VERSION {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_INVALID: expected schema_version {:?} got {:?} in {}",
                        x07_contracts::X07_ARCH_STREAM_PLUGIN_SCHEMA_VERSION,
                        spec.schema_version,
                        spec_display_path
                    ),
                ));
            }

            if spec.plugin_id != entry.plugin_id {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_MISMATCH: plugin_id mismatch: index has {:?} spec has {:?} (spec_path={})",
                        entry.plugin_id,
                        spec.plugin_id,
                        spec_display_path
                    ),
                ));
            }

            if spec.abi.native_backend_id != entry.native_backend_id
                || spec.abi.abi_major != entry.abi_major
                || spec.abi.export_symbol != entry.export_symbol
            {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_MISMATCH: abi mismatch between index and spec (plugin_id={:?} spec_path={})",
                        entry.plugin_id,
                        spec_display_path
                    ),
                ));
            }

            if spec.worlds_allowed != entry.worlds_allowed {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_MISMATCH: worlds_allowed mismatch between index and spec (plugin_id={:?} spec_path={})",
                        entry.plugin_id,
                        spec_display_path
                    ),
                ));
            }

            if spec.determinism != entry.determinism {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_MISMATCH: determinism mismatch between index and spec (plugin_id={:?} spec_path={})",
                        entry.plugin_id,
                        spec_display_path
                    ),
                ));
            }

            if spec.brands.in_item_brand != entry.in_item_brand
                || spec.brands.out_item_brand != entry.out_item_brand
            {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_MISMATCH: brands mismatch between index and spec (plugin_id={:?} spec_path={})",
                        entry.plugin_id,
                        spec_display_path
                    ),
                ));
            }

            if spec.budgets.state_bytes != entry.state_bytes
                || spec.budgets.scratch_bytes != entry.scratch_bytes
            {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_MISMATCH: budgets mismatch between index and spec (plugin_id={:?} spec_path={})",
                        entry.plugin_id,
                        spec_display_path
                    ),
                ));
            }

            let determinism = match spec.determinism.as_str() {
                "deterministic_v1" => StreamPluginDeterminismV1::DeterministicV1,
                "nondet_os_only_v1" => StreamPluginDeterminismV1::NondetOsOnlyV1,
                other => {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_PLUGIN_SPEC_INVALID: unknown determinism {:?} (plugin_id={:?} spec_path={})",
                            other,
                            entry.plugin_id,
                            spec_display_path
                        ),
                    ));
                }
            };

            let canon_mode = match spec.cfg.canon_mode.as_str() {
                "none_v1" => StreamPluginCfgCanonModeV1::NoneV1,
                "canon_json_v1" => StreamPluginCfgCanonModeV1::CanonJsonV1,
                other => {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!(
                            "E_PIPE_PLUGIN_SPEC_INVALID: unknown cfg.canon_mode {:?} (plugin_id={:?} spec_path={})",
                            other,
                            entry.plugin_id,
                            spec_display_path
                        ),
                    ));
                }
            };

            let in_item_brand = parse_item_brand_in(
                &Expr::Ident {
                    name: spec.brands.in_item_brand.clone(),
                    ptr: "".to_string(),
                },
                "in_item_brand",
            )?;
            let out_item_brand = parse_item_brand_out(
                &Expr::Ident {
                    name: spec.brands.out_item_brand.clone(),
                    ptr: "".to_string(),
                },
                "out_item_brand",
            )?;

            if spec.v == 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_INVALID: v must be >= 1 (plugin_id={:?} spec_path={})",
                        entry.plugin_id, spec_display_path
                    ),
                ));
            }
            if !spec.budgets.max_expand_ratio.is_finite() || spec.budgets.max_expand_ratio < 0.0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_SPEC_INVALID: budgets.max_expand_ratio must be >= 0 and finite (plugin_id={:?} spec_path={})",
                        entry.plugin_id,
                        spec_display_path
                    ),
                ));
            }

            let resolved = StreamPluginResolvedV1 {
                plugin_id: entry.plugin_id.clone(),
                native_backend_id: spec.abi.native_backend_id,
                abi_major: spec.abi.abi_major,
                export_symbol: spec.abi.export_symbol,
                budget_profile_id: entry.budget_profile_id.clone(),
                determinism,
                worlds_allowed: spec.worlds_allowed,
                in_item_brand,
                out_item_brand,
                budgets: StreamPluginBudgetsV1 {
                    state_bytes: spec.budgets.state_bytes,
                    scratch_bytes: spec.budgets.scratch_bytes,
                },
                cfg: StreamPluginCfgV1 {
                    max_bytes: spec.cfg.max_bytes,
                    canon_mode,
                },
                limits: StreamPluginLimitsV1 {
                    max_out_bytes_per_step: spec.limits.max_out_bytes_per_step,
                    max_out_items_per_step: spec.limits.max_out_items_per_step,
                    max_out_buf_bytes: spec.limits.max_out_buf_bytes,
                },
            };

            if by_id.insert(entry.plugin_id.clone(), resolved).is_some() {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "E_PIPE_PLUGIN_DUPLICATE_ID: duplicate plugin_id {:?} in {}",
                        entry.plugin_id, index_display_path
                    ),
                ));
            }
        }

        Ok(Self { by_id })
    }
}

fn load_brand_registry_v1(
    module_id: &str,
    module_metas: &BTreeMap<String, BTreeMap<String, Value>>,
) -> Result<BTreeMap<String, String>, CompilerError> {
    let meta = module_metas.get(module_id).ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Typing,
            format!("brand_registry_ref_v1 refers to unknown module_id: {module_id:?}"),
        )
    })?;
    load_brand_registry_v1_from_meta_v1(meta)
}

fn load_brand_registry_optional_v1(
    module_id: &str,
    module_metas: &BTreeMap<String, BTreeMap<String, Value>>,
) -> Result<BTreeMap<String, String>, CompilerError> {
    let Some(meta) = module_metas.get(module_id) else {
        return Ok(BTreeMap::new());
    };
    load_brand_registry_v1_from_meta_v1(meta)
}

fn load_brand_registry_v1_from_meta_v1(
    meta: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, String>, CompilerError> {
    let Some(v) = meta.get("brands_v1") else {
        return Ok(BTreeMap::new());
    };
    let Value::Object(map) = v else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "meta.brands_v1 must be a JSON object".to_string(),
        ));
    };

    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for (brand_id, entry) in map {
        validate::validate_symbol(brand_id)
            .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
        let Value::Object(entry_obj) = entry else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("meta.brands_v1[{brand_id:?}] must be an object"),
            ));
        };
        let Some(validate_v) = entry_obj.get("validate") else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("meta.brands_v1[{brand_id:?}] missing validate"),
            ));
        };
        let Some(validator_id) = validate_v.as_str() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("meta.brands_v1[{brand_id:?}].validate must be a string"),
            ));
        };
        validate::validate_symbol(validator_id)
            .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
        if out
            .insert(brand_id.clone(), validator_id.to_string())
            .is_some()
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("meta.brands_v1 duplicate entry for {brand_id:?}"),
            ));
        }
    }

    Ok(out)
}

fn ensure_brand_validator_sig_v1(
    validator_id: &str,
    fn_sigs: &PipeFnSigsV1,
    ptr: &str,
) -> Result<(), CompilerError> {
    let sig = fn_sigs.defn(validator_id).ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Typing,
            format!("unknown identifier: {validator_id:?} (ptr={ptr})"),
        )
    })?;
    if sig.params.len() != 1
        || sig.params[0].ty != Ty::BytesView
        || sig.params[0].brand.is_some()
        || sig.ret_ty != Ty::ResultI32
        || sig.ret_brand.is_some()
    {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "E_PIPE_BRAND_VALIDATOR_SIG: {validator_id:?} must have signature (bytes_view) -> result_i32 (ptr={ptr})"
            ),
        ));
    }
    Ok(())
}

fn infer_xf_req_in_v1(
    xf: &PipeXfDescV1,
    fn_sigs: &PipeFnSigsV1,
) -> Result<PipeItemBrandInV1, CompilerError> {
    if let Some(v) = &xf.in_item_brand {
        return Ok(v.clone());
    }
    match &xf.kind {
        PipeXfV1::MapBytes { fn_id } => {
            if let Some(sig) = fn_sigs.defn(fn_id) {
                if sig.params.len() == 1 && sig.params[0].ty == Ty::BytesView {
                    if let Some(b) = &sig.params[0].brand {
                        return Ok(PipeItemBrandInV1::Brand(b.clone()));
                    }
                }
            }
            Ok(PipeItemBrandInV1::Any)
        }
        PipeXfV1::ParMapStreamV1 { cfg } => {
            if let Some(sig) = fn_sigs.defasync(&cfg.mapper_defasync) {
                if sig.params.len() >= 2 && sig.params[1].ty == Ty::Bytes {
                    if let Some(b) = &sig.params[1].brand {
                        return Ok(PipeItemBrandInV1::Brand(b.clone()));
                    }
                }
            }
            Ok(PipeItemBrandInV1::Any)
        }
        PipeXfV1::RequireBrandV1 { .. } => Ok(PipeItemBrandInV1::Any),
        PipeXfV1::Filter { .. }
        | PipeXfV1::Take { .. }
        | PipeXfV1::SplitLines { .. }
        | PipeXfV1::FrameU32Le
        | PipeXfV1::MapInPlaceBufV1 { .. }
        | PipeXfV1::JsonCanonStreamV1 { .. }
        | PipeXfV1::DeframeU32LeV1 { .. }
        | PipeXfV1::PluginV1 { .. } => Ok(PipeItemBrandInV1::Any),
    }
}

fn infer_xf_out_v1(
    xf: &PipeXfDescV1,
    fn_sigs: &PipeFnSigsV1,
) -> Result<(PipeItemBrandOutV1, bool), CompilerError> {
    if let Some(v) = &xf.out_item_brand {
        return Ok((v.clone(), matches!(v, PipeItemBrandOutV1::Brand(_))));
    }
    match &xf.kind {
        PipeXfV1::RequireBrandV1 { brand_id, .. } => {
            Ok((PipeItemBrandOutV1::Brand(brand_id.clone()), false))
        }
        PipeXfV1::Filter { .. } | PipeXfV1::Take { .. } => Ok((PipeItemBrandOutV1::Same, false)),
        PipeXfV1::SplitLines { .. }
        | PipeXfV1::FrameU32Le
        | PipeXfV1::MapInPlaceBufV1 { .. }
        | PipeXfV1::JsonCanonStreamV1 { .. }
        | PipeXfV1::DeframeU32LeV1 { .. }
        | PipeXfV1::PluginV1 { .. } => Ok((PipeItemBrandOutV1::None, false)),
        PipeXfV1::MapBytes { fn_id } => {
            let out_brand = fn_sigs
                .defn(fn_id)
                .and_then(|sig| {
                    if sig.ret_ty == Ty::Bytes {
                        sig.ret_brand.clone()
                    } else {
                        None
                    }
                })
                .map(PipeItemBrandOutV1::Brand)
                .unwrap_or(PipeItemBrandOutV1::None);
            let produced_claim = matches!(&out_brand, PipeItemBrandOutV1::Brand(_));
            Ok((out_brand, produced_claim))
        }
        PipeXfV1::ParMapStreamV1 { cfg } => {
            let out_brand = fn_sigs
                .defasync(&cfg.mapper_defasync)
                .and_then(|sig| {
                    if matches!(sig.ret_ty, Ty::Bytes | Ty::ResultBytes) {
                        sig.ret_brand.clone()
                    } else {
                        None
                    }
                })
                .map(PipeItemBrandOutV1::Brand)
                .unwrap_or(PipeItemBrandOutV1::None);
            let produced_claim = matches!(&out_brand, PipeItemBrandOutV1::Brand(_));
            Ok((out_brand, produced_claim))
        }
    }
}

#[derive(Debug, Clone)]
struct PipeParam {
    ty: Ty,
    expr: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PipeItemBrandInV1 {
    Any,
    Same,
    Brand(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PipeItemBrandOutV1 {
    Same,
    None,
    Brand(String),
}

#[derive(Debug, Clone)]
struct PipeSrcDescV1 {
    kind: PipeSrcV1,
    out_item_brand: Option<String>,
    ptr: String,
}

#[derive(Debug, Clone)]
struct PipeXfDescV1 {
    kind: PipeXfV1,
    in_item_brand: Option<PipeItemBrandInV1>,
    out_item_brand: Option<PipeItemBrandOutV1>,
    ptr: String,
}

#[derive(Debug, Clone)]
struct PipeSinkDescV1 {
    kind: PipeSinkV1,
    in_item_brand: Option<PipeItemBrandInV1>,
    ptr: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PipeParsed {
    cfg: PipeCfgV1,
    src: PipeSrcDescV1,
    chain: Vec<PipeXfDescV1>,
    sink: PipeSinkDescV1,
    params: Vec<PipeParam>,
}

fn parse_pipe_v1(expr: &Expr) -> Result<PipeParsed, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.pipe_v1 must be a list".to_string(),
        ));
    };
    if items.len() != 5 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.pipe_v1 expects 4 arguments".to_string(),
        ));
    }

    let cfg = &items[1];
    let src = &items[2];
    let chain = &items[3];
    let sink = &items[4];

    let mut params: Vec<PipeParam> = Vec::new();

    let cfg = parse_cfg_v1(cfg)?;
    let mut src = parse_src_v1(src, &mut params)?;
    let mut chain = parse_chain_v1(chain, &mut params)?;

    // Desugaring: src.net_tcp_read_u32frames_v1 := src.net_tcp_read_stream_handle_v1 + xf.deframe_u32le_v1
    match std::mem::replace(&mut src.kind, PipeSrcV1::Bytes { bytes_param: 0 }) {
        PipeSrcV1::NetTcpReadU32Frames {
            stream_handle_param,
            caps_param,
            max_frame_bytes,
            allow_empty,
            on_timeout,
            on_eof,
        } => {
            let out_item_brand = src.out_item_brand.take().map(PipeItemBrandOutV1::Brand);
            let ptr = src.ptr.clone();
            chain.insert(
                0,
                PipeXfDescV1 {
                    kind: PipeXfV1::DeframeU32LeV1 {
                        cfg: DeframeU32LeCfgV1 {
                            max_frame_bytes,
                            max_frames: 0,
                            allow_empty,
                            on_truncated: DeframeOnTruncatedV1::Err,
                        },
                    },
                    in_item_brand: None,
                    out_item_brand,
                    ptr,
                },
            );
            src.kind = PipeSrcV1::NetTcpReadStreamHandle {
                stream_handle_param,
                caps_param,
                on_timeout,
                on_eof,
            };
            src.out_item_brand = None;
        }
        other => {
            src.kind = other;
        }
    }

    let sink = parse_sink_v1(sink, &mut params)?;

    Ok(PipeParsed {
        cfg,
        src,
        chain,
        sink,
        params,
    })
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PipeCfgV1 {
    chunk_max_bytes: i32,
    bufread_cap_bytes: i32,
    max_in_bytes: i32,
    max_out_bytes: i32,
    max_items: i32,
    max_steps: Option<i32>,
    emit_payload: Option<i32>,
    emit_stats: Option<i32>,
    allow_nondet_v1: i32,
    typecheck_item_brands_v1: i32,
    auto_require_brand_v1: i32,
    brand_registry_ref_v1: Option<String>,
    verify_produced_brands_v1: i32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum PipeSrcV1 {
    FsOpenRead {
        path_param: usize,
    },
    RrSend {
        key_param: usize,
    },
    Bytes {
        bytes_param: usize,
    },
    DbRowsDoc {
        conn_param: usize,
        sql_param: usize,
        params_doc_param: usize,
        qcaps_doc_param: usize,
    },
    NetTcpReadStreamHandle {
        stream_handle_param: usize,
        caps_param: usize,
        on_timeout: NetOnTimeoutV1,
        on_eof: NetOnEofV1,
    },
    NetTcpReadU32Frames {
        stream_handle_param: usize,
        caps_param: usize,
        max_frame_bytes: i32,
        allow_empty: i32,
        on_timeout: NetOnTimeoutV1,
        on_eof: NetOnEofV1,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetOnTimeoutV1 {
    Err,
    Stop,
    StopIfClean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetOnEofV1 {
    LeaveOpen,
    ShutdownRead,
    Close,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum PipeXfV1 {
    MapBytes {
        fn_id: String,
    },
    Filter {
        fn_id: String,
    },
    RequireBrandV1 {
        brand_id: String,
        validator_id: Option<String>,
        max_item_bytes: i32,
    },
    Take {
        n_param: usize,
    },
    SplitLines {
        delim_param: usize,
        max_line_bytes_param: usize,
    },
    FrameU32Le,
    MapInPlaceBufV1 {
        scratch_cap_bytes: i32,
        clear_before_each: i32,
        fn_id: String,
    },
    JsonCanonStreamV1 {
        cfg: JsonCanonStreamCfgV1,
    },
    DeframeU32LeV1 {
        cfg: DeframeU32LeCfgV1,
    },
    PluginV1 {
        plugin_id: String,
        cfg_param: usize,
        strict_brands: i32,
        strict_cfg_canon: i32,
        resolved: Option<StreamPluginResolvedV1>,
    },
    ParMapStreamV1 {
        cfg: ParMapStreamCfgV1,
    },
}

#[derive(Debug, Clone)]
struct ParMapStreamCfgV1 {
    max_inflight: i32,
    max_item_bytes: i32,
    max_inflight_in_bytes: i32,
    max_out_item_bytes: i32,
    ctx_param: Option<usize>,
    mapper_defasync: String,
    scope_cfg: Expr,
    unordered: bool,
    result_bytes: bool,
}

#[derive(Debug, Clone, Copy)]
enum DeframeOnTruncatedV1 {
    Err,
    Drop,
}

#[derive(Debug, Clone, Copy)]
struct JsonCanonStreamCfgV1 {
    max_depth: i32,
    max_total_json_bytes: i32,
    max_object_members: i32,
    max_object_total_bytes: i32,
    emit_chunk_max_bytes: i32,
}

#[derive(Debug, Clone)]
struct DeframeU32LeCfgV1 {
    max_frame_bytes: i32,
    max_frames: i32,
    allow_empty: i32,
    on_truncated: DeframeOnTruncatedV1,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum PipeSinkV1 {
    CollectBytes,
    HashFnv1a32,
    Null,
    WorldFsWriteFile {
        path_param: usize,
    },
    U32Frames {
        inner: Box<PipeSinkDescV1>,
    },
    WorldFsWriteStream {
        path_param: usize,
        caps_param: usize,
        cfg: WorldFsWriteStreamCfgV1,
    },
    WorldFsWriteStreamHashFnv1a32 {
        path_param: usize,
        caps_param: usize,
        cfg: WorldFsWriteStreamCfgV1,
    },
    NetTcpWriteStreamHandle {
        stream_handle_param: usize,
        caps_param: usize,
        cfg: NetTcpWriteStreamHandleCfgV1,
    },
    NetTcpConnectWrite {
        addr_param: usize,
        caps_param: usize,
        cfg: NetTcpWriteStreamHandleCfgV1,
    },
}

#[derive(Debug, Clone, Copy)]
struct WorldFsWriteStreamCfgV1 {
    buf_cap_bytes: i32,
    flush_min_bytes: i32,
    max_flushes: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetSinkOnFinishV1 {
    LeaveOpen,
    ShutdownWrite,
    Close,
}

#[derive(Debug, Clone, Copy)]
struct NetTcpWriteStreamHandleCfgV1 {
    buf_cap_bytes: i32,
    flush_min_bytes: i32,
    max_flushes: i32,
    max_write_calls: i32,
    on_finish: NetSinkOnFinishV1,
}

fn parse_cfg_v1(expr: &Expr) -> Result<PipeCfgV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.cfg_v1 must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("std.stream.cfg_v1") {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe cfg must be std.stream.cfg_v1".to_string(),
        ));
    }

    let mut chunk_max_bytes = None;
    let mut bufread_cap_bytes = None;
    let mut max_in_bytes = None;
    let mut max_out_bytes = None;
    let mut max_items = None;

    let mut max_steps = None;
    let mut emit_payload = None;
    let mut emit_stats = None;
    let mut allow_nondet_v1 = None;
    let mut typecheck_item_brands_v1 = None;
    let mut auto_require_brand_v1 = None;
    let mut brand_registry_ref_v1 = None;
    let mut verify_produced_brands_v1 = None;

    for field in items.iter().skip(1) {
        let Expr::List { items: kv, .. } = field else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cfg field must be a pair".to_string(),
            ));
        };
        if kv.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cfg field must be a pair".to_string(),
            ));
        }
        let Some(key) = kv[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cfg key must be an identifier".to_string(),
            ));
        };
        match key {
            "chunk_max_bytes" => {
                chunk_max_bytes = Some(expect_i32(&kv[1], "chunk_max_bytes must be an integer")?)
            }
            "bufread_cap_bytes" => {
                bufread_cap_bytes =
                    Some(expect_i32(&kv[1], "bufread_cap_bytes must be an integer")?)
            }
            "max_in_bytes" => {
                max_in_bytes = Some(expect_i32(&kv[1], "max_in_bytes must be an integer")?)
            }
            "max_out_bytes" => {
                max_out_bytes = Some(expect_i32(&kv[1], "max_out_bytes must be an integer")?)
            }
            "max_items" => max_items = Some(expect_i32(&kv[1], "max_items must be an integer")?),
            "max_steps" => max_steps = Some(expect_i32(&kv[1], "max_steps must be an integer")?),
            "emit_payload" => {
                emit_payload = Some(expect_i32(&kv[1], "emit_payload must be an integer")?)
            }
            "emit_stats" => emit_stats = Some(expect_i32(&kv[1], "emit_stats must be an integer")?),
            "allow_nondet_v1" => {
                let value = expect_i32(&kv[1], "allow_nondet_v1 must be an integer")?;
                if value != 0 && value != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "allow_nondet_v1 must be 0 or 1".to_string(),
                    ));
                }
                allow_nondet_v1 = Some(value);
            }
            "typecheck_item_brands_v1" => {
                let value = expect_i32(&kv[1], "typecheck_item_brands_v1 must be an integer")?;
                if value != 0 && value != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "typecheck_item_brands_v1 must be 0 or 1".to_string(),
                    ));
                }
                typecheck_item_brands_v1 = Some(value);
            }
            "auto_require_brand_v1" => {
                let value = expect_i32(&kv[1], "auto_require_brand_v1 must be an integer")?;
                if value != 0 && value != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "auto_require_brand_v1 must be 0 or 1".to_string(),
                    ));
                }
                auto_require_brand_v1 = Some(value);
            }
            "brand_registry_ref_v1" => {
                let module_id =
                    expect_bytes_lit_text(&kv[1], "brand_registry_ref_v1 must be bytes")?;
                validate::validate_module_id(&module_id)
                    .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
                brand_registry_ref_v1 = Some(module_id);
            }
            "verify_produced_brands_v1" => {
                let value = expect_i32(&kv[1], "verify_produced_brands_v1 must be an integer")?;
                if value != 0 && value != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "verify_produced_brands_v1 must be 0 or 1".to_string(),
                    ));
                }
                verify_produced_brands_v1 = Some(value);
            }
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown cfg field: {key}"),
                ));
            }
        }
    }

    Ok(PipeCfgV1 {
        chunk_max_bytes: chunk_max_bytes.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing chunk_max_bytes".to_string(),
            )
        })?,
        bufread_cap_bytes: bufread_cap_bytes.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing bufread_cap_bytes".to_string(),
            )
        })?,
        max_in_bytes: max_in_bytes.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing max_in_bytes".to_string(),
            )
        })?,
        max_out_bytes: max_out_bytes.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing max_out_bytes".to_string(),
            )
        })?,
        max_items: max_items.ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "cfg missing max_items".to_string(),
            )
        })?,
        max_steps,
        emit_payload,
        emit_stats,
        allow_nondet_v1: allow_nondet_v1.unwrap_or(0),
        typecheck_item_brands_v1: typecheck_item_brands_v1.unwrap_or(1),
        auto_require_brand_v1: auto_require_brand_v1.unwrap_or(0),
        brand_registry_ref_v1,
        verify_produced_brands_v1: verify_produced_brands_v1.unwrap_or(0),
    })
}

fn parse_src_v1(expr: &Expr, params: &mut Vec<PipeParam>) -> Result<PipeSrcDescV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe src must be a list".to_string(),
        ));
    };
    let ptr = expr.ptr().to_string();
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe src must start with an identifier".to_string(),
        ));
    };

    match head {
        "std.stream.src.fs_open_read_v1" => {
            if items.len() < 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 1 argument"),
                ));
            }
            let path_param = parse_expr_v1(params, Ty::BytesView, &items[1])?;
            let fields = parse_kv_fields(head, &items[2..])?;
            for k in fields.keys() {
                if k != "out_item_brand" {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} unknown field: {k}"),
                    ));
                }
            }
            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_brand_id(v, "out_item_brand"))
                .transpose()?;
            Ok(PipeSrcDescV1 {
                kind: PipeSrcV1::FsOpenRead { path_param },
                out_item_brand,
                ptr,
            })
        }
        "std.stream.src.rr_send_v1" => {
            if items.len() < 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 1 argument"),
                ));
            }
            let key_param = parse_expr_v1(params, Ty::BytesView, &items[1])?;
            let fields = parse_kv_fields(head, &items[2..])?;
            for k in fields.keys() {
                if k != "out_item_brand" {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} unknown field: {k}"),
                    ));
                }
            }
            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_brand_id(v, "out_item_brand"))
                .transpose()?;
            Ok(PipeSrcDescV1 {
                kind: PipeSrcV1::RrSend { key_param },
                out_item_brand,
                ptr,
            })
        }
        "std.stream.src.bytes_v1" => {
            if items.len() < 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 1 argument"),
                ));
            }
            let bytes_param = parse_expr_v1(params, Ty::Bytes, &items[1])?;
            let fields = parse_kv_fields(head, &items[2..])?;
            for k in fields.keys() {
                if k != "out_item_brand" {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} unknown field: {k}"),
                    ));
                }
            }
            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_brand_id(v, "out_item_brand"))
                .transpose()?;
            Ok(PipeSrcDescV1 {
                kind: PipeSrcV1::Bytes { bytes_param },
                out_item_brand,
                ptr,
            })
        }
        "std.stream.src.db_rows_doc_v1" => {
            if items.len() < 5 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 4 arguments"),
                ));
            }
            let conn_param = parse_expr_v1(params, Ty::I32, &items[1])?;
            let sql_param = parse_expr_v1(params, Ty::BytesView, &items[2])?;
            let params_doc_param = parse_expr_v1(params, Ty::Bytes, &items[3])?;
            let qcaps_doc_param = parse_expr_v1(params, Ty::Bytes, &items[4])?;
            let fields = parse_kv_fields(head, &items[5..])?;
            for k in fields.keys() {
                if k != "out_item_brand" {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} unknown field: {k}"),
                    ));
                }
            }
            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_brand_id(v, "out_item_brand"))
                .transpose()?;
            Ok(PipeSrcDescV1 {
                kind: PipeSrcV1::DbRowsDoc {
                    conn_param,
                    sql_param,
                    params_doc_param,
                    qcaps_doc_param,
                },
                out_item_brand,
                ptr,
            })
        }
        "std.stream.src.net_tcp_read_stream_handle_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "stream_handle" | "caps" | "on_timeout" | "on_eof" | "out_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }
            let stream_handle = fields.get("stream_handle").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing stream_handle"),
                )
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;

            // Canonical evaluation order (spec): stream_handle then caps.
            let stream_handle_param = parse_expr_v1(params, Ty::I32, stream_handle)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let on_timeout = match fields.get("on_timeout") {
                None => NetOnTimeoutV1::Err,
                Some(v) => match v.as_ident() {
                    Some("err") => NetOnTimeoutV1::Err,
                    Some("stop") => NetOnTimeoutV1::Stop,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_timeout must be \"err\" or \"stop\""),
                        ));
                    }
                },
            };

            let on_eof = match fields.get("on_eof") {
                None => NetOnEofV1::LeaveOpen,
                Some(v) => match v.as_ident() {
                    Some("leave_open") => NetOnEofV1::LeaveOpen,
                    Some("shutdown_read") => NetOnEofV1::ShutdownRead,
                    Some("close") => NetOnEofV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_eof must be \"leave_open\", \"shutdown_read\", or \"close\""),
                        ));
                    }
                },
            };
            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_brand_id(v, "out_item_brand"))
                .transpose()?;
            Ok(PipeSrcDescV1 {
                kind: PipeSrcV1::NetTcpReadStreamHandle {
                    stream_handle_param,
                    caps_param,
                    on_timeout,
                    on_eof,
                },
                out_item_brand,
                ptr,
            })
        }
        "std.stream.src.net_tcp_read_u32frames_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "stream_handle" | "caps" | "max_frame_bytes" | "allow_empty" | "on_timeout"
                    | "on_eof" | "out_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }
            let stream_handle = fields.get("stream_handle").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing stream_handle"),
                )
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;
            let max_frame_bytes = fields.get("max_frame_bytes").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing max_frame_bytes"),
                )
            })?;

            // Canonical evaluation order: stream_handle then caps.
            let stream_handle_param = parse_expr_v1(params, Ty::I32, stream_handle)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let max_frame_bytes =
                expect_i32(max_frame_bytes, "max_frame_bytes must be an integer")?;
            let allow_empty = fields
                .get("allow_empty")
                .map(|v| expect_i32(v, "allow_empty must be an integer"))
                .transpose()?
                .unwrap_or(1);

            let on_timeout = match fields.get("on_timeout") {
                None => NetOnTimeoutV1::Err,
                Some(v) => match v.as_ident() {
                    Some("err") => NetOnTimeoutV1::Err,
                    Some("stop_if_clean") => NetOnTimeoutV1::StopIfClean,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_timeout must be \"err\" or \"stop_if_clean\""),
                        ));
                    }
                },
            };

            let on_eof = match fields.get("on_eof") {
                None => NetOnEofV1::LeaveOpen,
                Some(v) => match v.as_ident() {
                    Some("leave_open") => NetOnEofV1::LeaveOpen,
                    Some("shutdown_read") => NetOnEofV1::ShutdownRead,
                    Some("close") => NetOnEofV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_eof must be \"leave_open\", \"shutdown_read\", or \"close\""),
                        ));
                    }
                },
            };

            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_brand_id(v, "out_item_brand"))
                .transpose()?;
            Ok(PipeSrcDescV1 {
                kind: PipeSrcV1::NetTcpReadU32Frames {
                    stream_handle_param,
                    caps_param,
                    max_frame_bytes,
                    allow_empty,
                    on_timeout,
                    on_eof,
                },
                out_item_brand,
                ptr,
            })
        }
        _ => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("unsupported pipe src: {head}"),
        )),
    }
}

fn parse_chain_v1(
    expr: &Expr,
    params: &mut Vec<PipeParam>,
) -> Result<Vec<PipeXfDescV1>, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe chain must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("std.stream.chain_v1") {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe chain must be std.stream.chain_v1".to_string(),
        ));
    }
    let mut xfs = Vec::with_capacity(items.len().saturating_sub(1));
    for xf in items.iter().skip(1) {
        xfs.push(parse_xf_v1(xf, params)?);
    }
    Ok(xfs)
}

fn parse_xf_v1(expr: &Expr, params: &mut Vec<PipeParam>) -> Result<PipeXfDescV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe xf must be a list".to_string(),
        ));
    };
    let ptr = expr.ptr().to_string();
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe xf must start with an identifier".to_string(),
        ));
    };
    let (kind, in_item_brand, out_item_brand) = match head {
        "std.stream.xf.map_bytes_v1" => {
            if items.len() < 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 1 argument"),
                ));
            }
            let fn_id = parse_fn_v1(&items[1])?;
            let fields = parse_kv_fields(head, &items[2..])?;
            let (in_item_brand, out_item_brand) = parse_xf_item_brand_fields(head, &fields)?;
            (PipeXfV1::MapBytes { fn_id }, in_item_brand, out_item_brand)
        }
        "std.stream.xf.filter_v1" => {
            if items.len() < 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 1 argument"),
                ));
            }
            let fn_id = parse_fn_v1(&items[1])?;
            let fields = parse_kv_fields(head, &items[2..])?;
            let (in_item_brand, out_item_brand) = parse_xf_item_brand_fields(head, &fields)?;
            (PipeXfV1::Filter { fn_id }, in_item_brand, out_item_brand)
        }
        "std.stream.xf.require_brand_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "brand" | "validator" | "max_item_bytes" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let brand_id = fields
                .get("brand")
                .ok_or_else(|| {
                    CompilerError::new(CompileErrorKind::Typing, format!("{head} missing brand"))
                })
                .and_then(|v| parse_brand_id(v, "brand"))?;

            let validator_id = fields
                .get("validator")
                .map(|v| {
                    v.as_ident().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} validator must be an identifier"),
                        )
                    })
                })
                .transpose()?
                .map(|validator_id| {
                    validate::validate_symbol(validator_id)
                        .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
                    Ok(validator_id.to_string())
                })
                .transpose()?;

            let max_item_bytes = fields
                .get("max_item_bytes")
                .map(|v| expect_i32(v, "max_item_bytes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_item_bytes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_item_bytes must be >= 0"),
                ));
            }

            (
                PipeXfV1::RequireBrandV1 {
                    brand_id,
                    validator_id,
                    max_item_bytes,
                },
                None,
                None,
            )
        }
        "std.stream.xf.take_v1" => {
            if items.len() < 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 1 argument"),
                ));
            }
            let n_param = parse_expr_v1(params, Ty::I32, &items[1])?;
            let fields = parse_kv_fields(head, &items[2..])?;
            let (in_item_brand, out_item_brand) = parse_xf_item_brand_fields(head, &fields)?;
            (PipeXfV1::Take { n_param }, in_item_brand, out_item_brand)
        }
        "std.stream.xf.split_lines_v1" => {
            if items.len() < 3 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 2 arguments"),
                ));
            }
            let delim_param = parse_expr_v1(params, Ty::I32, &items[1])?;
            let max_line_bytes_param = parse_expr_v1(params, Ty::I32, &items[2])?;
            let fields = parse_kv_fields(head, &items[3..])?;
            let (in_item_brand, out_item_brand) = parse_xf_item_brand_fields(head, &fields)?;
            (
                PipeXfV1::SplitLines {
                    delim_param,
                    max_line_bytes_param,
                },
                in_item_brand,
                out_item_brand,
            )
        }
        "std.stream.xf.frame_u32le_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            let (in_item_brand, out_item_brand) = parse_xf_item_brand_fields(head, &fields)?;
            (PipeXfV1::FrameU32Le, in_item_brand, out_item_brand)
        }
        "std.stream.xf.map_in_place_buf_v1" => {
            // v1.1; no expr_v1 params.
            let mut scratch_cap_bytes: Option<i32> = None;
            let mut clear_before_each: Option<i32> = None;
            let mut fn_id: Option<String> = None;
            let mut in_item_brand: Option<PipeItemBrandInV1> = None;
            let mut out_item_brand: Option<PipeItemBrandOutV1> = None;

            for item in items.iter().skip(1) {
                let Expr::List { items: inner, .. } = item else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} items must be lists"),
                    ));
                };

                if inner.first().and_then(Expr::as_ident) == Some("std.stream.fn_v1") {
                    let f = parse_fn_v1(item)?;
                    if fn_id.replace(f).is_some() {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} has duplicate fn"),
                        ));
                    }
                    continue;
                }

                if inner.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} fields must be pairs"),
                    ));
                }
                let Some(key) = inner[0].as_ident() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} field key must be an identifier"),
                    ));
                };
                match key {
                    "scratch_cap_bytes" => {
                        let x = expect_i32(&inner[1], "scratch_cap_bytes must be an integer")?;
                        if x <= 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} scratch_cap_bytes must be >= 1"),
                            ));
                        }
                        if scratch_cap_bytes.replace(x).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} has duplicate scratch_cap_bytes"),
                            ));
                        }
                    }
                    "clear_before_each" => {
                        let x = expect_i32(&inner[1], "clear_before_each must be an integer")?;
                        if clear_before_each.replace(x).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} has duplicate clear_before_each"),
                            ));
                        }
                    }
                    "fn" => {
                        let f = parse_fn_v1(&inner[1])?;
                        if fn_id.replace(f).is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} has duplicate fn"),
                            ));
                        }
                    }
                    "in_item_brand" => {
                        if in_item_brand.is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} has duplicate in_item_brand"),
                            ));
                        }
                        in_item_brand = Some(parse_item_brand_in(&inner[1], "in_item_brand")?);
                    }
                    "out_item_brand" => {
                        if out_item_brand.is_some() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} has duplicate out_item_brand"),
                            ));
                        }
                        out_item_brand = Some(parse_item_brand_out(&inner[1], "out_item_brand")?);
                    }
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {key}"),
                        ));
                    }
                }
            }

            let scratch_cap_bytes = scratch_cap_bytes.ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing scratch_cap_bytes"),
                )
            })?;
            let fn_id = fn_id.ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing fn"))
            })?;

            (
                PipeXfV1::MapInPlaceBufV1 {
                    scratch_cap_bytes,
                    clear_before_each: clear_before_each.unwrap_or(1),
                    fn_id,
                },
                in_item_brand,
                out_item_brand,
            )
        }
        "std.stream.xf.json_canon_stream_v1" => {
            // v1.1; no expr_v1 params.
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "max_depth"
                    | "max_total_json_bytes"
                    | "max_object_members"
                    | "max_object_total_bytes"
                    | "emit_chunk_max_bytes"
                    | "in_item_brand"
                    | "out_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            fn parse_opt_pos_i32(
                fields: &BTreeMap<String, Expr>,
                key: &str,
                msg: &str,
                head: &str,
            ) -> Result<i32, CompilerError> {
                match fields.get(key) {
                    None => Ok(0),
                    Some(v) => {
                        let x = expect_i32(v, msg)?;
                        if x <= 0 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} {key} must be >= 1"),
                            ));
                        }
                        Ok(x)
                    }
                }
            }

            let max_depth =
                parse_opt_pos_i32(&fields, "max_depth", "max_depth must be an integer", head)?;
            let max_total_json_bytes = parse_opt_pos_i32(
                &fields,
                "max_total_json_bytes",
                "max_total_json_bytes must be an integer",
                head,
            )?;
            let max_object_members = parse_opt_pos_i32(
                &fields,
                "max_object_members",
                "max_object_members must be an integer",
                head,
            )?;
            let max_object_total_bytes = parse_opt_pos_i32(
                &fields,
                "max_object_total_bytes",
                "max_object_total_bytes must be an integer",
                head,
            )?;
            let emit_chunk_max_bytes = parse_opt_pos_i32(
                &fields,
                "emit_chunk_max_bytes",
                "emit_chunk_max_bytes must be an integer",
                head,
            )?;

            let in_item_brand = fields
                .get("in_item_brand")
                .map(|v| parse_item_brand_in(v, "in_item_brand"))
                .transpose()?;
            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_item_brand_out(v, "out_item_brand"))
                .transpose()?;

            (
                PipeXfV1::JsonCanonStreamV1 {
                    cfg: JsonCanonStreamCfgV1 {
                        max_depth,
                        max_total_json_bytes,
                        max_object_members,
                        max_object_total_bytes,
                        emit_chunk_max_bytes,
                    },
                },
                in_item_brand,
                out_item_brand,
            )
        }
        "std.stream.xf.deframe_u32le_v1" => {
            // v1.1 read-side; no expr_v1 params.
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "max_frame_bytes" | "max_frames" | "allow_empty" | "on_truncated"
                    | "in_item_brand" | "out_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let max_frame_bytes = fields
                .get("max_frame_bytes")
                .ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} missing max_frame_bytes"),
                    )
                })
                .and_then(|v| expect_i32(v, "max_frame_bytes must be an integer"))?;

            let max_frames = fields
                .get("max_frames")
                .map(|v| expect_i32(v, "max_frames must be an integer"))
                .transpose()?
                .unwrap_or(0);

            let allow_empty = fields
                .get("allow_empty")
                .map(|v| expect_i32(v, "allow_empty must be an integer"))
                .transpose()?
                .unwrap_or(1);

            let on_truncated = match fields.get("on_truncated") {
                None => DeframeOnTruncatedV1::Err,
                Some(v) => match v.as_ident() {
                    Some("err") => DeframeOnTruncatedV1::Err,
                    Some("drop") => DeframeOnTruncatedV1::Drop,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_truncated must be \"err\" or \"drop\""),
                        ));
                    }
                },
            };

            let in_item_brand = fields
                .get("in_item_brand")
                .map(|v| parse_item_brand_in(v, "in_item_brand"))
                .transpose()?;
            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_item_brand_out(v, "out_item_brand"))
                .transpose()?;

            (
                PipeXfV1::DeframeU32LeV1 {
                    cfg: DeframeU32LeCfgV1 {
                        max_frame_bytes,
                        max_frames,
                        allow_empty,
                        on_truncated,
                    },
                },
                in_item_brand,
                out_item_brand,
            )
        }
        "std.stream.xf.plugin_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "id" | "cfg" | "opts_v1" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let id_expr = fields.get("id").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing id"))
            })?;

            let plugin_id = if let Expr::List { items: inner, .. } = id_expr {
                if inner.first().and_then(Expr::as_ident) == Some("std.stream.expr_v1")
                    && inner.len() == 2
                {
                    expect_bytes_lit_text(&inner[1], "id must be bytes.lit")?
                } else {
                    expect_bytes_lit_text(id_expr, "id must be bytes.lit")?
                }
            } else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} id must be bytes.lit"),
                ));
            };

            validate::validate_symbol(&plugin_id)
                .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;

            let cfg_expr = fields.get("cfg").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing cfg"))
            })?;
            let cfg_param = parse_expr_v1(params, Ty::Bytes, cfg_expr)?;

            let mut strict_brands: i32 = 1;
            let mut strict_cfg_canon: i32 = 1;
            if let Some(opts) = fields.get("opts_v1") {
                let Expr::List {
                    items: opt_items, ..
                } = opts
                else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} opts_v1 must be a list"),
                    ));
                };
                if opt_items.first().and_then(Expr::as_ident) != Some("opts_v1") {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} opts_v1 must start with opts_v1"),
                    ));
                }
                let opt_fields = parse_kv_fields("opts_v1", &opt_items[1..])?;
                for k in opt_fields.keys() {
                    match k.as_str() {
                        "strict_brands" | "strict_cfg_canon" => {}
                        _ => {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} opts_v1 unknown field: {k}"),
                            ));
                        }
                    }
                }
                if let Some(v) = opt_fields.get("strict_brands") {
                    strict_brands = expect_i32(v, "strict_brands must be an integer")?;
                    if strict_brands != 0 && strict_brands != 1 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} strict_brands must be 0 or 1"),
                        ));
                    }
                }
                if let Some(v) = opt_fields.get("strict_cfg_canon") {
                    strict_cfg_canon = expect_i32(v, "strict_cfg_canon must be an integer")?;
                    if strict_cfg_canon != 0 && strict_cfg_canon != 1 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} strict_cfg_canon must be 0 or 1"),
                        ));
                    }
                }
            }

            (
                PipeXfV1::PluginV1 {
                    plugin_id,
                    cfg_param,
                    strict_brands,
                    strict_cfg_canon,
                    resolved: None,
                },
                None,
                None,
            )
        }
        "std.stream.xf.par_map_stream_v1"
        | "std.stream.xf.par_map_stream_result_bytes_v1"
        | "std.stream.xf.par_map_stream_unordered_v1"
        | "std.stream.xf.par_map_stream_unordered_result_bytes_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "max_inflight"
                    | "max_item_bytes"
                    | "max_inflight_in_bytes"
                    | "max_out_item_bytes"
                    | "ctx"
                    | "mapper_defasync"
                    | "scope_cfg"
                    | "in_item_brand"
                    | "out_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let max_inflight = fields
                .get("max_inflight")
                .ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} missing max_inflight"),
                    )
                })
                .and_then(|v| expect_i32(v, "max_inflight must be an integer"))?;
            if max_inflight <= 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_inflight must be >= 1"),
                ));
            }

            let max_item_bytes = fields
                .get("max_item_bytes")
                .ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} missing max_item_bytes"),
                    )
                })
                .and_then(|v| expect_i32(v, "max_item_bytes must be an integer"))?;
            if max_item_bytes <= 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_item_bytes must be >= 1"),
                ));
            }

            let max_inflight_in_bytes = fields
                .get("max_inflight_in_bytes")
                .map(|v| expect_i32(v, "max_inflight_in_bytes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_inflight_in_bytes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_inflight_in_bytes must be >= 0"),
                ));
            }

            let max_out_item_bytes = fields
                .get("max_out_item_bytes")
                .map(|v| expect_i32(v, "max_out_item_bytes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_out_item_bytes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_out_item_bytes must be >= 0"),
                ));
            }

            let ctx_param = match fields.get("ctx") {
                None => None,
                Some(v) => Some(parse_expr_v1(params, Ty::Bytes, v)?),
            };

            let mapper_defasync = fields
                .get("mapper_defasync")
                .ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} missing mapper_defasync"),
                    )
                })
                .and_then(|v| match v {
                    Expr::Ident { name, .. } => Ok(name.clone()),
                    Expr::List { .. } => parse_fn_v1(v),
                    _ => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("{head} mapper_defasync must be an identifier or std.stream.fn_v1"),
                    )),
                })?;

            let scope_cfg = match fields.get("scope_cfg") {
                None => expr_list(vec![expr_ident("task.scope.cfg_v1".to_string())]),
                Some(v) => {
                    let Expr::List {
                        items: cfg_items, ..
                    } = v
                    else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} scope_cfg must be task.scope.cfg_v1"),
                        ));
                    };
                    if cfg_items.first().and_then(Expr::as_ident) != Some("task.scope.cfg_v1") {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} scope_cfg must be task.scope.cfg_v1"),
                        ));
                    }
                    v.clone()
                }
            };

            let in_item_brand = fields
                .get("in_item_brand")
                .map(|v| parse_item_brand_in(v, "in_item_brand"))
                .transpose()?;
            let out_item_brand = fields
                .get("out_item_brand")
                .map(|v| parse_item_brand_out(v, "out_item_brand"))
                .transpose()?;

            (
                PipeXfV1::ParMapStreamV1 {
                    cfg: ParMapStreamCfgV1 {
                        max_inflight,
                        max_item_bytes,
                        max_inflight_in_bytes,
                        max_out_item_bytes,
                        ctx_param,
                        mapper_defasync,
                        scope_cfg,
                        unordered: head.contains("_unordered_"),
                        result_bytes: head.contains("_result_bytes_"),
                    },
                },
                in_item_brand,
                out_item_brand,
            )
        }
        _ => {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unsupported pipe xf: {head}"),
            ))
        }
    };
    Ok(PipeXfDescV1 {
        kind,
        in_item_brand,
        out_item_brand,
        ptr,
    })
}

fn parse_sink_v1(
    expr: &Expr,
    params: &mut Vec<PipeParam>,
) -> Result<PipeSinkDescV1, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe sink must be a list".to_string(),
        ));
    };
    let ptr = expr.ptr().to_string();
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "pipe sink must start with an identifier".to_string(),
        ));
    };

    let (kind, in_item_brand) = match head {
        "std.stream.sink.collect_bytes_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            let in_item_brand = parse_sink_item_brand_fields(head, &fields)?;
            (PipeSinkV1::CollectBytes, in_item_brand)
        }
        "std.stream.sink.hash_fnv1a32_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            let in_item_brand = parse_sink_item_brand_fields(head, &fields)?;
            (PipeSinkV1::HashFnv1a32, in_item_brand)
        }
        "std.stream.sink.null_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            let in_item_brand = parse_sink_item_brand_fields(head, &fields)?;
            (PipeSinkV1::Null, in_item_brand)
        }
        "std.stream.sink.world_fs_write_file_v1" => {
            if items.len() < 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 1 argument"),
                ));
            }
            let path_param = parse_expr_v1(params, Ty::Bytes, &items[1])?;
            let fields = parse_kv_fields(head, &items[2..])?;
            let in_item_brand = parse_sink_item_brand_fields(head, &fields)?;
            (PipeSinkV1::WorldFsWriteFile { path_param }, in_item_brand)
        }
        "std.stream.sink.u32frames_v1" => {
            if items.len() < 2 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 1 argument"),
                ));
            }
            let inner = parse_sink_v1(&items[1], params)?;
            let fields = parse_kv_fields(head, &items[2..])?;
            let in_item_brand = parse_sink_item_brand_fields(head, &fields)?;
            (
                PipeSinkV1::U32Frames {
                    inner: Box::new(inner),
                },
                in_item_brand,
            )
        }
        "std.stream.sink.world_fs_write_stream_v1"
        | "std.stream.sink.world_fs_write_stream_hash_fnv1a32_v1" => {
            if items.len() < 3 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} expects at least 2 arguments"),
                ));
            }
            let path_param = parse_expr_v1(params, Ty::Bytes, &items[1])?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, &items[2])?;
            let fields = parse_kv_fields(head, &items[3..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "buf_cap_bytes" | "flush_min_bytes" | "max_flushes" | "in_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let buf_cap_bytes = match fields.get("buf_cap_bytes") {
                None => 0,
                Some(v) => {
                    let x = expect_i32(v, "buf_cap_bytes must be an integer")?;
                    if x <= 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} buf_cap_bytes must be >= 1"),
                        ));
                    }
                    x
                }
            };

            let flush_min_bytes = match fields.get("flush_min_bytes") {
                None => 0,
                Some(v) => {
                    let x = expect_i32(v, "flush_min_bytes must be an integer")?;
                    if x <= 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} flush_min_bytes must be >= 1"),
                        ));
                    }
                    x
                }
            };

            let max_flushes = match fields.get("max_flushes") {
                None => 0,
                Some(v) => {
                    let x = expect_i32(v, "max_flushes must be an integer")?;
                    if x < 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} max_flushes must be >= 0"),
                        ));
                    }
                    x
                }
            };

            let cfg = WorldFsWriteStreamCfgV1 {
                buf_cap_bytes,
                flush_min_bytes,
                max_flushes,
            };

            let in_item_brand = fields
                .get("in_item_brand")
                .map(|v| parse_item_brand_in(v, "in_item_brand"))
                .transpose()?;
            if matches!(in_item_brand, Some(PipeItemBrandInV1::Same)) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} in_item_brand must be \"any\" or a brand id"),
                ));
            }

            if head == "std.stream.sink.world_fs_write_stream_v1" {
                (
                    PipeSinkV1::WorldFsWriteStream {
                        path_param,
                        caps_param,
                        cfg,
                    },
                    in_item_brand,
                )
            } else {
                (
                    PipeSinkV1::WorldFsWriteStreamHashFnv1a32 {
                        path_param,
                        caps_param,
                        cfg,
                    },
                    in_item_brand,
                )
            }
        }
        "std.stream.sink.net_tcp_write_stream_handle_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "stream_handle" | "caps" | "buf_cap_bytes" | "flush_min_bytes"
                    | "on_finish" | "max_flushes" | "max_write_calls" | "in_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let stream_handle = fields.get("stream_handle").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing stream_handle"),
                )
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;

            // Canonical evaluation order (spec): stream_handle then caps.
            let stream_handle_param = parse_expr_v1(params, Ty::I32, stream_handle)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let buf_cap_bytes = fields
                .get("buf_cap_bytes")
                .map(|v| expect_i32(v, "buf_cap_bytes must be an integer"))
                .transpose()?
                .unwrap_or(65536);
            if buf_cap_bytes <= 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} buf_cap_bytes must be >= 1"),
                ));
            }

            let flush_min_bytes = fields
                .get("flush_min_bytes")
                .map(|v| expect_i32(v, "flush_min_bytes must be an integer"))
                .transpose()?
                .unwrap_or(buf_cap_bytes);
            if flush_min_bytes <= 0 || flush_min_bytes > buf_cap_bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} flush_min_bytes must be in [1..buf_cap_bytes]"),
                ));
            }

            let on_finish = match fields.get("on_finish") {
                None => NetSinkOnFinishV1::LeaveOpen,
                Some(v) => match v.as_ident() {
                    Some("leave_open") => NetSinkOnFinishV1::LeaveOpen,
                    Some("shutdown_write") => NetSinkOnFinishV1::ShutdownWrite,
                    Some("close") => NetSinkOnFinishV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_finish must be \"leave_open\", \"shutdown_write\", or \"close\""),
                        ));
                    }
                },
            };

            let max_flushes = fields
                .get("max_flushes")
                .map(|v| expect_i32(v, "max_flushes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_flushes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_flushes must be >= 0"),
                ));
            }

            let max_write_calls = fields
                .get("max_write_calls")
                .map(|v| expect_i32(v, "max_write_calls must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_write_calls < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_write_calls must be >= 0"),
                ));
            }

            let in_item_brand = fields
                .get("in_item_brand")
                .map(|v| parse_item_brand_in(v, "in_item_brand"))
                .transpose()?;
            if matches!(in_item_brand, Some(PipeItemBrandInV1::Same)) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} in_item_brand must be \"any\" or a brand id"),
                ));
            }

            (
                PipeSinkV1::NetTcpWriteStreamHandle {
                    stream_handle_param,
                    caps_param,
                    cfg: NetTcpWriteStreamHandleCfgV1 {
                        buf_cap_bytes,
                        flush_min_bytes,
                        max_flushes,
                        max_write_calls,
                        on_finish,
                    },
                },
                in_item_brand,
            )
        }
        "std.stream.sink.net_tcp_write_u32frames_v1" => {
            // Convenience wrapper: u32frames(net_tcp_write_stream_handle_v1(...)).
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "stream_handle" | "caps" | "buf_cap_bytes" | "flush_min_bytes"
                    | "on_finish" | "max_flushes" | "max_write_calls" | "in_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let stream_handle = fields.get("stream_handle").ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} missing stream_handle"),
                )
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;

            // Canonical evaluation order (spec): stream_handle then caps.
            let stream_handle_param = parse_expr_v1(params, Ty::I32, stream_handle)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let buf_cap_bytes = fields
                .get("buf_cap_bytes")
                .map(|v| expect_i32(v, "buf_cap_bytes must be an integer"))
                .transpose()?
                .unwrap_or(65536);
            if buf_cap_bytes <= 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} buf_cap_bytes must be >= 1"),
                ));
            }

            let flush_min_bytes = fields
                .get("flush_min_bytes")
                .map(|v| expect_i32(v, "flush_min_bytes must be an integer"))
                .transpose()?
                .unwrap_or(buf_cap_bytes);
            if flush_min_bytes <= 0 || flush_min_bytes > buf_cap_bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} flush_min_bytes must be in [1..buf_cap_bytes]"),
                ));
            }

            let on_finish = match fields.get("on_finish") {
                None => NetSinkOnFinishV1::LeaveOpen,
                Some(v) => match v.as_ident() {
                    Some("leave_open") => NetSinkOnFinishV1::LeaveOpen,
                    Some("shutdown_write") => NetSinkOnFinishV1::ShutdownWrite,
                    Some("close") => NetSinkOnFinishV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_finish must be \"leave_open\", \"shutdown_write\", or \"close\""),
                        ));
                    }
                },
            };

            let max_flushes = fields
                .get("max_flushes")
                .map(|v| expect_i32(v, "max_flushes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_flushes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_flushes must be >= 0"),
                ));
            }

            let max_write_calls = fields
                .get("max_write_calls")
                .map(|v| expect_i32(v, "max_write_calls must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_write_calls < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_write_calls must be >= 0"),
                ));
            }

            let in_item_brand = fields
                .get("in_item_brand")
                .map(|v| parse_item_brand_in(v, "in_item_brand"))
                .transpose()?;
            if matches!(in_item_brand, Some(PipeItemBrandInV1::Same)) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} in_item_brand must be \"any\" or a brand id"),
                ));
            }

            let inner = PipeSinkDescV1 {
                kind: PipeSinkV1::NetTcpWriteStreamHandle {
                    stream_handle_param,
                    caps_param,
                    cfg: NetTcpWriteStreamHandleCfgV1 {
                        buf_cap_bytes,
                        flush_min_bytes,
                        max_flushes,
                        max_write_calls,
                        on_finish,
                    },
                },
                in_item_brand: None,
                ptr: ptr.clone(),
            };

            (
                PipeSinkV1::U32Frames {
                    inner: Box::new(inner),
                },
                in_item_brand,
            )
        }
        "std.stream.sink.net_tcp_connect_write_v1" => {
            let fields = parse_kv_fields(head, &items[1..])?;
            for k in fields.keys() {
                match k.as_str() {
                    "addr" | "caps" | "buf_cap_bytes" | "flush_min_bytes" | "on_finish"
                    | "max_flushes" | "max_write_calls" | "in_item_brand" => {}
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} unknown field: {k}"),
                        ));
                    }
                }
            }

            let addr = fields.get("addr").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing addr"))
            })?;
            let caps = fields.get("caps").ok_or_else(|| {
                CompilerError::new(CompileErrorKind::Typing, format!("{head} missing caps"))
            })?;

            // Canonical evaluation order (spec): addr then caps.
            let addr_param = parse_expr_v1(params, Ty::Bytes, addr)?;
            let caps_param = parse_expr_v1(params, Ty::Bytes, caps)?;

            let buf_cap_bytes = fields
                .get("buf_cap_bytes")
                .map(|v| expect_i32(v, "buf_cap_bytes must be an integer"))
                .transpose()?
                .unwrap_or(65536);
            if buf_cap_bytes <= 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} buf_cap_bytes must be >= 1"),
                ));
            }

            let flush_min_bytes = fields
                .get("flush_min_bytes")
                .map(|v| expect_i32(v, "flush_min_bytes must be an integer"))
                .transpose()?
                .unwrap_or(buf_cap_bytes);
            if flush_min_bytes <= 0 || flush_min_bytes > buf_cap_bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} flush_min_bytes must be in [1..buf_cap_bytes]"),
                ));
            }

            let on_finish = match fields.get("on_finish") {
                None => NetSinkOnFinishV1::Close,
                Some(v) => match v.as_ident() {
                    Some("shutdown_write") => NetSinkOnFinishV1::ShutdownWrite,
                    Some("close") => NetSinkOnFinishV1::Close,
                    _ => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("{head} on_finish must be \"shutdown_write\" or \"close\""),
                        ));
                    }
                },
            };

            let max_flushes = fields
                .get("max_flushes")
                .map(|v| expect_i32(v, "max_flushes must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_flushes < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_flushes must be >= 0"),
                ));
            }

            let max_write_calls = fields
                .get("max_write_calls")
                .map(|v| expect_i32(v, "max_write_calls must be an integer"))
                .transpose()?
                .unwrap_or(0);
            if max_write_calls < 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} max_write_calls must be >= 0"),
                ));
            }

            let in_item_brand = fields
                .get("in_item_brand")
                .map(|v| parse_item_brand_in(v, "in_item_brand"))
                .transpose()?;
            if matches!(in_item_brand, Some(PipeItemBrandInV1::Same)) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} in_item_brand must be \"any\" or a brand id"),
                ));
            }

            (
                PipeSinkV1::NetTcpConnectWrite {
                    addr_param,
                    caps_param,
                    cfg: NetTcpWriteStreamHandleCfgV1 {
                        buf_cap_bytes,
                        flush_min_bytes,
                        max_flushes,
                        max_write_calls,
                        on_finish,
                    },
                },
                in_item_brand,
            )
        }
        _ => {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unsupported pipe sink: {head}"),
            ));
        }
    };

    Ok(PipeSinkDescV1 {
        kind,
        in_item_brand,
        ptr,
    })
}

fn validate_pipe_world_caps(
    pipe: &PipeParsed,
    options: &CompileOptions,
) -> Result<(), CompilerError> {
    fn sink_needs_os(sink: &PipeSinkV1) -> bool {
        match sink {
            PipeSinkV1::CollectBytes | PipeSinkV1::HashFnv1a32 | PipeSinkV1::Null => false,
            PipeSinkV1::WorldFsWriteFile { .. }
            | PipeSinkV1::WorldFsWriteStream { .. }
            | PipeSinkV1::WorldFsWriteStreamHashFnv1a32 { .. }
            | PipeSinkV1::NetTcpWriteStreamHandle { .. }
            | PipeSinkV1::NetTcpConnectWrite { .. } => true,
            PipeSinkV1::U32Frames { inner } => sink_needs_os(&inner.kind),
        }
    }

    let src_needs_os = matches!(
        pipe.src.kind,
        PipeSrcV1::DbRowsDoc { .. } | PipeSrcV1::NetTcpReadStreamHandle { .. }
    );

    let needs_os = src_needs_os || sink_needs_os(&pipe.sink.kind);
    if needs_os && !options.world.is_standalone_only() {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            format!(
                "std.stream.pipe_v1 requires an OS world (run-os, run-os-sandboxed); got {}",
                options.world.as_str()
            ),
        ));
    }
    Ok(())
}

fn gen_pipe_helper_body(
    pipe: &PipeParsed,
    options: &CompileOptions,
) -> Result<Expr, CompilerError> {
    let mut cfg = pipe.cfg.clone();

    let emit_payload = cfg.emit_payload.unwrap_or(1) != 0;
    let emit_stats = cfg.emit_stats.unwrap_or(1) != 0;

    if cfg.chunk_max_bytes <= 0
        || cfg.bufread_cap_bytes <= 0
        || cfg.max_in_bytes <= 0
        || cfg.max_out_bytes <= 0
        || cfg.max_items <= 0
        || cfg.bufread_cap_bytes < cfg.chunk_max_bytes
    {
        return Ok(err_doc_const(E_CFG_INVALID, "stream:cfg_invalid"));
    }

    // Resolve cfg.max_in_bytes to include json_canon_stream_v1's optional max_total_json_bytes.
    {
        let mut json_stage_count = 0;
        let mut max_total_json_bytes = 0;
        for xf in &pipe.chain {
            if let PipeXfV1::JsonCanonStreamV1 { cfg: jcfg } = &xf.kind {
                json_stage_count += 1;
                if json_stage_count > 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "std.stream.pipe_v1 supports at most one std.stream.xf.json_canon_stream_v1".to_string(),
                    ));
                }
                max_total_json_bytes = jcfg.max_total_json_bytes;
            }
        }
        if max_total_json_bytes > 0 && max_total_json_bytes < cfg.max_in_bytes {
            cfg.max_in_bytes = max_total_json_bytes;
        }
    }

    let max_steps = if let Some(ms) = cfg.max_steps.filter(|&v| v > 0) {
        ms
    } else {
        // Conservative bound: ceil(max_in_bytes / chunk_max_bytes) + slack.
        let chunk = cfg.chunk_max_bytes.max(1) as i64;
        let max_in = cfg.max_in_bytes as i64;
        let steps = (max_in + chunk - 1) / chunk + 8;
        i32::try_from(steps.min(i32::MAX as i64)).unwrap_or(i32::MAX)
    };

    let mut sink_shape = sink_shape_v1(&pipe.sink.kind)?;
    match &mut sink_shape.base {
        SinkBaseV1::WorldFsWriteStream { cfg: fs_cfg, .. }
        | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { cfg: fs_cfg, .. } => {
            if fs_cfg.buf_cap_bytes == 0 {
                fs_cfg.buf_cap_bytes = cfg.chunk_max_bytes;
            }
            if fs_cfg.flush_min_bytes == 0 {
                fs_cfg.flush_min_bytes = fs_cfg.buf_cap_bytes;
            }
            if fs_cfg.buf_cap_bytes <= 0 || fs_cfg.flush_min_bytes <= 0 {
                return Ok(err_doc_const(E_CFG_INVALID, "stream:fs_sink_cfg_invalid"));
            }
            if fs_cfg.flush_min_bytes > fs_cfg.buf_cap_bytes {
                return Ok(err_doc_const(E_CFG_INVALID, "stream:fs_sink_cfg_invalid"));
            }
            if fs_cfg.max_flushes == 0 {
                // max_flushes_default = ceil(max_out_bytes / max(1, flush_min_bytes)) + 4
                let out = i64::from(cfg.max_out_bytes.max(1));
                let fmin = i64::from(fs_cfg.flush_min_bytes.max(1));
                let derived = (out + fmin - 1) / fmin + 4;
                fs_cfg.max_flushes =
                    i32::try_from(derived.min(i64::from(i32::MAX))).unwrap_or(i32::MAX);
            }
            if fs_cfg.max_flushes <= 0 {
                return Ok(err_doc_const(E_CFG_INVALID, "stream:fs_sink_cfg_invalid"));
            }
        }
        _ => {}
    }
    let mut stream_plugin_registry: Option<StreamPluginRegistryV1> = None;

    let mut resolve_stream_plugin = |plugin_id: &str,
                                     ptr: &str|
     -> Result<StreamPluginResolvedV1, CompilerError> {
        let Some(arch_root) = options.arch_root.as_ref() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "E_PIPE_STREAM_PLUGINS_NEEDS_ARCH_ROOT: std.stream.pipe_v1 requires compile_options.arch_root for plugin-backed xfs"
                    .to_string(),
            ));
        };

        if stream_plugin_registry.is_none() {
            stream_plugin_registry = Some(StreamPluginRegistryV1::load(arch_root)?);
        }
        let registry = stream_plugin_registry.as_ref().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: missing stream plugin registry".to_string(),
            )
        })?;

        let p = registry.by_id.get(plugin_id).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "E_PIPE_PLUGIN_NOT_FOUND: stream plugin_id {plugin_id:?} is not declared in arch/stream/plugins/index.x07sp.json (ptr={ptr})",
                ),
            )
        })?;

        let world = options.world.as_str();
        if !p.worlds_allowed.is_empty() && !p.worlds_allowed.iter().any(|w| w == world) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "E_PIPE_PLUGIN_WORLD_VIOLATION: stream plugin_id {plugin_id:?} is not allowed in world {world} (ptr={ptr})",
                ),
            ));
        }
        if p.determinism == StreamPluginDeterminismV1::NondetOsOnlyV1
            && !matches!(world, "run-os" | "run-os-sandboxed")
        {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "E_PIPE_PLUGIN_WORLD_VIOLATION: OS-only stream plugin_id {plugin_id:?} is not allowed in solve worlds (ptr={ptr})",
                ),
            ));
        }

        Ok(p.clone())
    };

    let cfg_bytes_i32_le = |stage_idx: usize, words: Vec<Expr>| -> Expr {
        let cfg_var = format!("xf_cfg_{stage_idx}");
        let cap = i32::try_from(words.len().saturating_mul(4)).unwrap_or(i32::MAX);
        let mut stmts = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident(cfg_var.clone()),
            expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(cap)]),
        ])];
        for w in words {
            stmts.push(extend_u32(&cfg_var, w));
        }
        stmts.push(expr_list(vec![
            expr_ident("vec_u8.into_bytes"),
            expr_ident(cfg_var),
        ]));
        expr_list(vec![expr_ident("begin")].into_iter().chain(stmts).collect())
    };

    let mut take_states: Vec<TakeState> = Vec::new();
    let mut require_brand_states: Vec<RequireBrandState> = Vec::new();
    let mut map_in_place_states: Vec<MapInPlaceState> = Vec::new();
    let mut plugin_states: Vec<PluginState> = Vec::new();
    let mut par_map_states: Vec<ParMapState> = Vec::new();
    for (idx, xf) in pipe.chain.iter().enumerate() {
        match &xf.kind {
            PipeXfV1::RequireBrandV1 { .. } => {
                require_brand_states.push(RequireBrandState {
                    stage_idx: idx,
                    item_idx_var: format!("req_brand_item_idx_{idx}"),
                });
            }
            PipeXfV1::Take { n_param } => {
                take_states.push(TakeState {
                    stage_idx: idx,
                    n_param: *n_param,
                    rem_var: format!("take_rem_{idx}"),
                });
            }
            PipeXfV1::MapInPlaceBufV1 {
                scratch_cap_bytes,
                clear_before_each,
                fn_id,
            } => {
                map_in_place_states.push(MapInPlaceState {
                    stage_idx: idx,
                    scratch_cap_bytes: *scratch_cap_bytes,
                    clear_before_each: *clear_before_each,
                    fn_id: fn_id.clone(),
                    scratch_var: format!("scratch_{idx}"),
                });
            }
            PipeXfV1::SplitLines {
                delim_param,
                max_line_bytes_param,
            } => {
                let plugin = resolve_stream_plugin("xf.split_lines_v1", &xf.ptr)?;
                let cfg_init_expr = cfg_bytes_i32_le(
                    idx,
                    vec![
                        param_ident(*delim_param),
                        param_ident(*max_line_bytes_param),
                    ],
                );

                plugin_states.push(PluginState {
                    stage_idx: idx,
                    plugin,
                    cfg_b_var: format!("xf_cfg_b_{idx}"),
                    cfg_init_expr: Some(cfg_init_expr),
                    strict_cfg_canon: 0,
                    err_map: PluginErrMapV1::SplitLinesV1,
                    state_b_var: format!("xf_plugin_state_{idx}"),
                    scratch_b_var: format!("xf_plugin_scratch_{idx}"),
                });
            }
            PipeXfV1::FrameU32Le => {
                let plugin = resolve_stream_plugin("xf.frame_u32le_v1", &xf.ptr)?;
                plugin_states.push(PluginState {
                    stage_idx: idx,
                    plugin,
                    cfg_b_var: format!("xf_cfg_b_{idx}"),
                    cfg_init_expr: Some(expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)])),
                    strict_cfg_canon: 0,
                    err_map: PluginErrMapV1::FrameU32LeV1,
                    state_b_var: format!("xf_plugin_state_{idx}"),
                    scratch_b_var: format!("xf_plugin_scratch_{idx}"),
                });
            }
            PipeXfV1::DeframeU32LeV1 { cfg } => {
                let plugin = resolve_stream_plugin("xf.deframe_u32le_v1", &xf.ptr)?;
                let on_truncated = match cfg.on_truncated {
                    DeframeOnTruncatedV1::Err => 0,
                    DeframeOnTruncatedV1::Drop => 1,
                };
                let cfg_init_expr = cfg_bytes_i32_le(
                    idx,
                    vec![
                        expr_int(cfg.max_frame_bytes),
                        expr_int(cfg.max_frames),
                        expr_int(cfg.allow_empty),
                        expr_int(on_truncated),
                    ],
                );
                plugin_states.push(PluginState {
                    stage_idx: idx,
                    plugin,
                    cfg_b_var: format!("xf_cfg_b_{idx}"),
                    cfg_init_expr: Some(cfg_init_expr),
                    strict_cfg_canon: 0,
                    err_map: PluginErrMapV1::DeframeU32LeV1,
                    state_b_var: format!("xf_plugin_state_{idx}"),
                    scratch_b_var: format!("xf_plugin_scratch_{idx}"),
                });
            }
            PipeXfV1::PluginV1 {
                cfg_param,
                strict_cfg_canon,
                resolved,
                ..
            } => {
                let plugin = resolved.clone().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: stream plugin stage not resolved".to_string(),
                    )
                })?;
                plugin_states.push(PluginState {
                    stage_idx: idx,
                    plugin,
                    cfg_b_var: format!("p{cfg_param}"),
                    cfg_init_expr: None,
                    strict_cfg_canon: *strict_cfg_canon,
                    err_map: PluginErrMapV1::Generic,
                    state_b_var: format!("xf_plugin_state_{idx}"),
                    scratch_b_var: format!("xf_plugin_scratch_{idx}"),
                });
            }
            PipeXfV1::MapBytes { .. } | PipeXfV1::Filter { .. } => {}
            PipeXfV1::ParMapStreamV1 { cfg: pcfg } => {
                par_map_states.push(ParMapState {
                    stage_idx: idx,
                    cfg: pcfg.clone(),
                    ctx_b_var: format!("par_map_ctx_b_{idx}"),
                    ctx_v_var: format!("par_map_ctx_v_{idx}"),
                    slots_var: format!("par_map_slots_{idx}"),
                    lens_var: (pcfg.max_inflight_in_bytes > 0)
                        .then(|| format!("par_map_lens_{idx}")),
                    idxs_var: (pcfg.unordered && pcfg.result_bytes)
                        .then(|| format!("par_map_idxs_{idx}")),
                    head_var: (!pcfg.unordered).then(|| format!("par_map_head_{idx}")),
                    len_var: format!("par_map_len_{idx}"),
                    inflight_bytes_var: (pcfg.max_inflight_in_bytes > 0)
                        .then(|| format!("par_map_inflight_bytes_{idx}")),
                    next_index_var: format!("par_map_next_index_{idx}"),
                });
            }
            PipeXfV1::JsonCanonStreamV1 { cfg: jcfg } => {
                let plugin = resolve_stream_plugin("xf.json_canon_stream_v1", &xf.ptr)?;

                let max_depth = if jcfg.max_depth > 0 {
                    jcfg.max_depth
                } else {
                    64
                };
                let max_total_json_bytes = if jcfg.max_total_json_bytes > 0 {
                    jcfg.max_total_json_bytes
                } else {
                    cfg.max_in_bytes
                };
                let max_object_members = if jcfg.max_object_members > 0 {
                    jcfg.max_object_members
                } else {
                    4096
                };
                let max_object_total_bytes = if jcfg.max_object_total_bytes > 0 {
                    jcfg.max_object_total_bytes
                } else {
                    cfg.max_out_bytes.min(4 * 1024 * 1024)
                };
                let emit_chunk_max_bytes = if jcfg.emit_chunk_max_bytes > 0 {
                    jcfg.emit_chunk_max_bytes
                } else {
                    cfg.chunk_max_bytes
                };

                if max_depth <= 0
                    || max_total_json_bytes <= 0
                    || max_object_members <= 0
                    || max_object_total_bytes <= 0
                    || emit_chunk_max_bytes <= 0
                {
                    return Ok(err_doc_const(
                        E_CFG_INVALID,
                        "stream:json_canon_cfg_invalid",
                    ));
                }

                let cfg_init_expr = cfg_bytes_i32_le(
                    idx,
                    vec![
                        expr_int(max_depth),
                        expr_int(max_total_json_bytes),
                        expr_int(max_object_members),
                        expr_int(max_object_total_bytes),
                        expr_int(emit_chunk_max_bytes),
                    ],
                );

                plugin_states.push(PluginState {
                    stage_idx: idx,
                    plugin,
                    cfg_b_var: format!("xf_cfg_b_{idx}"),
                    cfg_init_expr: Some(cfg_init_expr),
                    strict_cfg_canon: 0,
                    err_map: PluginErrMapV1::JsonCanonStreamV1,
                    state_b_var: format!("xf_plugin_state_{idx}"),
                    scratch_b_var: format!("xf_plugin_scratch_{idx}"),
                });
            }
        }
    }

    if par_map_states.len() > 1 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.pipe_v1 supports at most one par_map stream stage".to_string(),
        ));
    }
    if cfg.allow_nondet_v1 == 0 && par_map_states.iter().any(|s| s.cfg.unordered) {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "X07E_PIPE_NDET_NOT_ALLOWED: unordered par_map stages require allow_nondet_v1=1"
                .to_string(),
        ));
    }

    let has_take = !take_states.is_empty();

    let par_map_scope_cfg = par_map_states.first().map(|s| s.cfg.scope_cfg.clone());

    let sink_vec_var = match sink_shape.base {
        SinkBaseV1::CollectBytes | SinkBaseV1::WorldFsWriteFile { .. } => {
            Some("sink_vec".to_string())
        }
        SinkBaseV1::HashFnv1a32
        | SinkBaseV1::Null
        | SinkBaseV1::WorldFsWriteStream { .. }
        | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { .. }
        | SinkBaseV1::NetTcpWriteStreamHandle { .. }
        | SinkBaseV1::NetTcpConnectWrite { .. } => None,
    };
    let hash_var = match sink_shape.base {
        SinkBaseV1::HashFnv1a32 | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { .. } => {
            Some("hash".to_string())
        }
        _ => None,
    };

    let cg = PipeCodegen {
        cfg: &cfg,
        src_out_item_brand: pipe.src.out_item_brand.clone(),
        chain: pipe.chain.as_slice(),
        emit_payload,
        emit_stats,
        max_steps,
        sink: sink_shape,
        bytes_in_var: "bytes_in".to_string(),
        bytes_out_var: "bytes_out".to_string(),
        items_in_var: "items_in".to_string(),
        items_out_var: "items_out".to_string(),
        stop_var: if has_take {
            Some("stop".to_string())
        } else {
            None
        },
        require_brand_states,
        take_states,
        map_in_place_states,
        plugin_states,
        par_map_states,
        sink_vec_var,
        hash_var,
    };

    let mut items: Vec<Expr> = vec![expr_ident("begin")];

    items.push(let_i32(&cg.bytes_in_var, 0));
    items.push(let_i32(&cg.bytes_out_var, 0));
    items.push(let_i32(&cg.items_in_var, 0));
    items.push(let_i32(&cg.items_out_var, 0));
    if let Some(stop) = &cg.stop_var {
        items.push(let_i32(stop, 0));
    }
    for s in &cg.require_brand_states {
        items.push(let_i32(&s.item_idx_var, 0));
    }

    // Init map_in_place_buf scratch handles.
    for s in &cg.map_in_place_states {
        if s.scratch_cap_bytes <= 0 {
            items.push(expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:scratch_cap_bytes_invalid"),
            ]));
            continue;
        }
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(s.scratch_var.clone()),
            expr_list(vec![
                expr_ident("scratch_u8_fixed_v1.new"),
                expr_int(s.scratch_cap_bytes),
            ]),
        ]));
    }

    // Init take stage counters.
    for t in &cg.take_states {
        let p = param_ident(t.n_param);
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(t.rem_var.clone()),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("<"), p, expr_int(0)]),
                expr_int(0),
                param_ident(t.n_param),
            ]),
        ]));
    }

    // Init plugin stage state + scratch buffers.
    for p in &cg.plugin_states {
        let state_len = i32::try_from(p.plugin.budgets.state_bytes).unwrap_or(i32::MAX);
        let scratch_len = i32::try_from(p.plugin.budgets.scratch_bytes).unwrap_or(i32::MAX);
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(p.state_b_var.clone()),
            expr_list(vec![
                expr_ident("__internal.bytes.alloc_aligned_v1"),
                expr_int(state_len),
                expr_int(16),
            ]),
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(p.scratch_b_var.clone()),
            expr_list(vec![
                expr_ident("__internal.bytes.alloc_aligned_v1"),
                expr_int(scratch_len),
                expr_int(16),
            ]),
        ]));
    }

    // Init plugin cfg bytes (if not provided via pipe params).
    for p in &cg.plugin_states {
        let Some(cfg_expr) = p.cfg_init_expr.clone() else {
            continue;
        };
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(p.cfg_b_var.clone()),
            cfg_expr,
        ]));
    }

    // Init par_map_stream state.
    for p in &cg.par_map_states {
        let ctx_expr = match p.cfg.ctx_param {
            Some(param) => param_ident(param),
            None => expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
        };
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(p.ctx_b_var.clone()),
            ctx_expr,
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(p.ctx_v_var.clone()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident(p.ctx_b_var.clone()),
            ]),
        ]));

        let cap_bytes = p.cfg.max_inflight.checked_mul(4).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                "par_map_stream cap overflow".to_string(),
            )
        })?;
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(p.slots_var.clone()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_int(cap_bytes),
            ]),
        ]));
        items.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(p.slots_var.clone()),
            expr_list(vec![
                expr_ident("vec_u8.extend_zeroes"),
                expr_ident(p.slots_var.clone()),
                expr_int(cap_bytes),
            ]),
        ]));

        if p.cfg.unordered {
            let init_i = format!("pm_init_{stage_idx}", stage_idx = p.stage_idx);
            items.push(expr_list(vec![
                expr_ident("for"),
                expr_ident(init_i.clone()),
                expr_int(0),
                expr_int(p.cfg.max_inflight),
                expr_list(vec![
                    expr_ident("begin"),
                    vec_u8_set_u32_le(
                        &p.slots_var,
                        expr_list(vec![expr_ident("*"), expr_ident(init_i), expr_int(4)]),
                        expr_int(-1),
                    ),
                    expr_int(0),
                ]),
            ]));
        }

        if let Some(lens) = &p.lens_var {
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(lens.clone()),
                expr_list(vec![
                    expr_ident("vec_u8.with_capacity"),
                    expr_int(cap_bytes),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("set"),
                expr_ident(lens.clone()),
                expr_list(vec![
                    expr_ident("vec_u8.extend_zeroes"),
                    expr_ident(lens.clone()),
                    expr_int(cap_bytes),
                ]),
            ]));
            if let Some(inflight) = &p.inflight_bytes_var {
                items.push(let_i32(inflight, 0));
            }
        }
        if let Some(idxs) = &p.idxs_var {
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(idxs.clone()),
                expr_list(vec![
                    expr_ident("vec_u8.with_capacity"),
                    expr_int(cap_bytes),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("set"),
                expr_ident(idxs.clone()),
                expr_list(vec![
                    expr_ident("vec_u8.extend_zeroes"),
                    expr_ident(idxs.clone()),
                    expr_int(cap_bytes),
                ]),
            ]));
        }
        items.push(let_i32(&p.len_var, 0));
        if let Some(head) = &p.head_var {
            items.push(let_i32(head, 0));
        }
        items.push(let_i32(&p.next_index_var, 0));
    }

    // Init sink state.
    if let Some(vec_name) = &cg.sink_vec_var {
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(vec_name.clone()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_int(cg.cfg.max_out_bytes),
            ]),
        ]));
    }
    if let Some(hash_name) = &cg.hash_var {
        items.push(let_i32(hash_name, FNV1A32_OFFSET_BASIS));
    }
    match &cg.sink.base {
        SinkBaseV1::NetTcpWriteStreamHandle {
            stream_handle_param,
            caps_param,
            cfg,
        } => {
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_h".to_string()),
                param_ident(*stream_handle_param),
            ]));
            items.push(let_i32("net_sink_owned", 0));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_caps_b".to_string()),
                param_ident(*caps_param),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_caps".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("net_sink_caps_b".to_string()),
                ]),
            ]));
            emit_net_sink_validate_caps(&mut items);
            emit_net_sink_init_limits_and_buffer(&mut items, cg.cfg.max_out_bytes, *cfg);
        }
        SinkBaseV1::NetTcpConnectWrite {
            addr_param,
            caps_param,
            cfg,
        } => {
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_addr_b".to_string()),
                param_ident(*addr_param),
            ]));
            items.push(let_i32("net_sink_owned", 1));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_addr".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("net_sink_addr_b".to_string()),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_caps_b".to_string()),
                param_ident(*caps_param),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_caps".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("net_sink_caps_b".to_string()),
                ]),
            ]));
            emit_net_sink_validate_caps(&mut items);

            // Connect once (owned).
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_connect_doc".to_string()),
                expr_list(vec![
                    expr_ident("std.net.tcp.connect_v1"),
                    expr_ident("net_sink_addr".to_string()),
                    expr_ident("net_sink_caps".to_string()),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_connect_dv".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("net_sink_connect_doc".to_string()),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("std.net.err.is_err_doc_v1"),
                    expr_ident("net_sink_connect_dv".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_NET_WRITE_FAILED),
                        "stream:net_connect_failed",
                        expr_ident("net_sink_connect_doc".to_string()),
                    ),
                ]),
                expr_int(0),
            ]));
            items.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("net_sink_h".to_string()),
                expr_list(vec![
                    expr_ident("std.net.tcp.connect_stream_handle_v1"),
                    expr_ident("net_sink_connect_dv".to_string()),
                ]),
            ]));
            items.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<="),
                    expr_ident("net_sink_h".to_string()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_connect_doc_invalid"),
                ]),
                expr_int(0),
            ]));
            emit_net_sink_init_limits_and_buffer(&mut items, cg.cfg.max_out_bytes, *cfg);
        }
        _ => {}
    }

    if let SinkBaseV1::WorldFsWriteStream {
        path_param,
        caps_param,
        cfg,
    }
    | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 {
        path_param,
        caps_param,
        cfg,
    } = &cg.sink.base
    {
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_sink_path_b".to_string()),
            param_ident(*path_param),
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_sink_caps_b".to_string()),
            param_ident(*caps_param),
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_open".to_string()),
            expr_list(vec![
                expr_ident("os.fs.stream_open_write_v1"),
                expr_ident("fs_sink_path_b".to_string()),
                expr_ident("fs_sink_caps_b".to_string()),
            ]),
        ]));
        items.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("result_i32.is_ok"),
                expr_ident("fs_open".to_string()),
            ]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("ec".to_string()),
                    expr_list(vec![
                        expr_ident("result_i32.err_code"),
                        expr_ident("fs_open".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(12)]),
                ]),
                extend_u32("pl", expr_ident("ec".to_string())),
                extend_u32("pl", expr_int(0)),
                extend_u32("pl", expr_ident(cg.bytes_out_var.clone())),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_SINK_FS_OPEN_FAILED),
                        "stream:fs_open_failed",
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident("pl".to_string()),
                        ]),
                    ),
                ]),
            ]),
        ]));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_sink_h".to_string()),
            expr_list(vec![
                expr_ident("result_i32.unwrap_or"),
                expr_ident("fs_open".to_string()),
                expr_int(-1),
            ]),
        ]));
        items.push(let_i32("fs_sink_flushes", 0));
        items.push(let_i32("fs_sink_max_flushes", cfg.max_flushes));
        items.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("fs_sink_buf".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_int(cfg.buf_cap_bytes),
            ]),
        ]));
    }

    // Init plugin stages (downstream -> upstream).
    for p in cg.plugin_states.iter().rev() {
        items.push(cg.gen_plugin_init(p.stage_idx)?);
    }

    let main = match &pipe.src.kind {
        PipeSrcV1::Bytes { bytes_param } => cg.gen_run_bytes_source(*bytes_param)?,
        PipeSrcV1::FsOpenRead { path_param } => cg.gen_run_reader_source(
            "fs.open_read",
            param_ident(*path_param),
            cfg.bufread_cap_bytes,
        )?,
        PipeSrcV1::RrSend { key_param } => cg.gen_run_rr_send_source(*key_param)?,
        PipeSrcV1::DbRowsDoc {
            conn_param,
            sql_param,
            params_doc_param,
            qcaps_doc_param,
        } => cg.gen_run_db_rows_doc_source(
            *conn_param,
            *sql_param,
            *params_doc_param,
            *qcaps_doc_param,
        )?,
        PipeSrcV1::NetTcpReadStreamHandle {
            stream_handle_param,
            caps_param,
            on_timeout,
            on_eof,
        } => cg.gen_run_net_tcp_read_stream_handle_source(
            *stream_handle_param,
            *caps_param,
            *on_timeout,
            *on_eof,
        )?,
        PipeSrcV1::NetTcpReadU32Frames { .. } => {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: net_tcp_read_u32frames should have been desugared".to_string(),
            ));
        }
    };
    items.push(main);

    let body = expr_list(items);
    if let Some(cfg_expr) = par_map_scope_cfg {
        Ok(expr_list(vec![expr_ident("task.scope_v1"), cfg_expr, body]))
    } else {
        Ok(body)
    }
}

const E_CFG_INVALID: i32 = 1;
const E_BUDGET_IN_BYTES: i32 = 2;
const E_BUDGET_OUT_BYTES: i32 = 3;
const E_BUDGET_ITEMS: i32 = 4;
const E_LINE_TOO_LONG: i32 = 5;
const E_DB_QUERY_FAILED: i32 = 7;
const E_SCRATCH_OVERFLOW: i32 = 8;
const E_STAGE_FAILED: i32 = 9;
const E_FRAME_TOO_LARGE: i32 = 10;

const E_PARMAP_ITEM_TOO_LARGE: i32 = 100;
const E_PARMAP_OUT_TOO_LARGE: i32 = 101;
const E_PARMAP_CHILD_ERR: i32 = 102;
const E_PARMAP_CHILD_CANCELED: i32 = 103;

const E_JSON_SYNTAX: i32 = 20;
const E_JSON_NOT_IJSON: i32 = 21;
const E_JSON_TOO_DEEP: i32 = 22;
const E_JSON_OBJECT_TOO_LARGE: i32 = 23;
const E_JSON_TRAILING_DATA: i32 = 24;
const E_RR_MISS: i32 = 11;

const E_SINK_FS_OPEN_FAILED: i32 = 40;
const E_SINK_FS_WRITE_FAILED: i32 = 41;
const E_SINK_FS_CLOSE_FAILED: i32 = 42;
const E_SINK_TOO_MANY_FLUSHES: i32 = 43;

const E_NET_CAPS_INVALID: i32 = 60;
const E_NET_READ_FAILED: i32 = 61;
const E_NET_READ_DOC_INVALID: i32 = 62;
const E_NET_BACKEND_OVERRAN_MAX: i32 = 63;
const E_NET_WRITE_FAILED: i32 = 64;
const E_NET_SINK_MAX_FLUSHES: i32 = 65;
const E_NET_SINK_MAX_WRITES: i32 = 66;

const E_BRAND_ITEM_TOO_LARGE: i32 = 70;
const E_BRAND_VALIDATE_FAILED: i32 = 71;

const E_DEFRAME_FRAME_TOO_LARGE: i32 = 80;
const E_DEFRAME_TRUNCATED: i32 = 81;
const E_DEFRAME_EMPTY_FORBIDDEN: i32 = 82;
const E_DEFRAME_MAX_FRAMES: i32 = 83;
const E_DEFRAME_TRUNCATED_TIMEOUT: i32 = 84;

const E_XF_CFG_TOO_LARGE: i32 = 110;
const E_XF_CFG_NON_CANON: i32 = 111;
const E_XF_OUT_INVALID: i32 = 112;

const E_XF_EMIT_BUF_TOO_LARGE: i32 = 113;
const E_XF_EMIT_STEP_BYTES_EXCEEDED: i32 = 114;
const E_XF_EMIT_STEP_ITEMS_EXCEEDED: i32 = 115;
const E_XF_EMIT_LEN_GT_CAP: i32 = 116;

const E_XF_PLUGIN_INVALID: i32 = 117;

const FNV1A32_OFFSET_BASIS: i32 = -2128831035; // 0x811c9dc5
const FNV1A32_PRIME: i32 = 16777619;

#[derive(Clone)]
struct TakeState {
    stage_idx: usize,
    n_param: usize,
    rem_var: String,
}

#[derive(Clone)]
struct RequireBrandState {
    stage_idx: usize,
    item_idx_var: String,
}

#[derive(Clone)]
struct MapInPlaceState {
    stage_idx: usize,
    scratch_cap_bytes: i32,
    clear_before_each: i32,
    fn_id: String,
    scratch_var: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PluginErrMapV1 {
    Generic,
    SplitLinesV1,
    FrameU32LeV1,
    DeframeU32LeV1,
    JsonCanonStreamV1,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PluginCallKindV1 {
    Init,
    Step,
    Flush,
}

#[derive(Clone)]
struct PluginState {
    stage_idx: usize,
    plugin: StreamPluginResolvedV1,
    cfg_b_var: String,
    cfg_init_expr: Option<Expr>,
    strict_cfg_canon: i32,
    err_map: PluginErrMapV1,
    state_b_var: String,
    scratch_b_var: String,
}

#[derive(Clone)]
struct ParMapState {
    stage_idx: usize,
    cfg: ParMapStreamCfgV1,
    ctx_b_var: String,
    ctx_v_var: String,
    slots_var: String,
    lens_var: Option<String>,
    idxs_var: Option<String>,
    head_var: Option<String>,
    len_var: String,
    inflight_bytes_var: Option<String>,
    next_index_var: String,
}

#[derive(Clone)]
enum SinkBaseV1 {
    CollectBytes,
    HashFnv1a32,
    Null,
    WorldFsWriteFile {
        path_param: usize,
    },
    WorldFsWriteStream {
        path_param: usize,
        caps_param: usize,
        cfg: WorldFsWriteStreamCfgV1,
    },
    WorldFsWriteStreamHashFnv1a32 {
        path_param: usize,
        caps_param: usize,
        cfg: WorldFsWriteStreamCfgV1,
    },
    NetTcpWriteStreamHandle {
        stream_handle_param: usize,
        caps_param: usize,
        cfg: NetTcpWriteStreamHandleCfgV1,
    },
    NetTcpConnectWrite {
        addr_param: usize,
        caps_param: usize,
        cfg: NetTcpWriteStreamHandleCfgV1,
    },
}

#[derive(Clone)]
struct SinkShapeV1 {
    framing_u32frames: bool,
    base: SinkBaseV1,
}

fn sink_shape_v1(sink: &PipeSinkV1) -> Result<SinkShapeV1, CompilerError> {
    match sink {
        PipeSinkV1::CollectBytes => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::CollectBytes,
        }),
        PipeSinkV1::HashFnv1a32 => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::HashFnv1a32,
        }),
        PipeSinkV1::Null => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::Null,
        }),
        PipeSinkV1::WorldFsWriteFile { path_param } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::WorldFsWriteFile {
                path_param: *path_param,
            },
        }),
        PipeSinkV1::WorldFsWriteStream {
            path_param,
            caps_param,
            cfg,
        } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::WorldFsWriteStream {
                path_param: *path_param,
                caps_param: *caps_param,
                cfg: *cfg,
            },
        }),
        PipeSinkV1::WorldFsWriteStreamHashFnv1a32 {
            path_param,
            caps_param,
            cfg,
        } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::WorldFsWriteStreamHashFnv1a32 {
                path_param: *path_param,
                caps_param: *caps_param,
                cfg: *cfg,
            },
        }),
        PipeSinkV1::NetTcpWriteStreamHandle {
            stream_handle_param,
            caps_param,
            cfg,
        } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::NetTcpWriteStreamHandle {
                stream_handle_param: *stream_handle_param,
                caps_param: *caps_param,
                cfg: *cfg,
            },
        }),
        PipeSinkV1::NetTcpConnectWrite {
            addr_param,
            caps_param,
            cfg,
        } => Ok(SinkShapeV1 {
            framing_u32frames: false,
            base: SinkBaseV1::NetTcpConnectWrite {
                addr_param: *addr_param,
                caps_param: *caps_param,
                cfg: *cfg,
            },
        }),
        PipeSinkV1::U32Frames { inner } => {
            let inner = sink_shape_v1(&inner.kind)?;
            if inner.framing_u32frames {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "std.stream.sink.u32frames_v1 cannot wrap another u32frames_v1".to_string(),
                ));
            }
            Ok(SinkShapeV1 {
                framing_u32frames: true,
                base: inner.base,
            })
        }
    }
}

struct PipeCodegen<'a> {
    cfg: &'a PipeCfgV1,
    src_out_item_brand: Option<String>,
    chain: &'a [PipeXfDescV1],
    emit_payload: bool,
    emit_stats: bool,
    max_steps: i32,

    sink: SinkShapeV1,
    bytes_in_var: String,
    bytes_out_var: String,
    items_in_var: String,
    items_out_var: String,

    stop_var: Option<String>,
    require_brand_states: Vec<RequireBrandState>,
    take_states: Vec<TakeState>,
    map_in_place_states: Vec<MapInPlaceState>,
    plugin_states: Vec<PluginState>,
    par_map_states: Vec<ParMapState>,

    sink_vec_var: Option<String>,
    hash_var: Option<String>,
}

impl PipeCodegen<'_> {
    fn has_flush_stages(&self) -> bool {
        !self.plugin_states.is_empty() || !self.par_map_states.is_empty()
    }

    fn apply_src_out_item_brand(&self, item: Expr) -> Expr {
        match &self.src_out_item_brand {
            None => item,
            Some(brand_id) => expr_list(vec![
                expr_ident("__internal.brand.assume_view_v1"),
                expr_ident(brand_id.clone()),
                item,
            ]),
        }
    }

    fn apply_out_item_brand(&self, stage_idx: usize, item: Expr) -> Expr {
        match self.chain[stage_idx].out_item_brand.as_ref() {
            None | Some(PipeItemBrandOutV1::Same) => item,
            Some(PipeItemBrandOutV1::None) => {
                expr_list(vec![expr_ident("std.brand.erase_view_v1"), item])
            }
            Some(PipeItemBrandOutV1::Brand(brand_id)) => expr_list(vec![
                expr_ident("__internal.brand.assume_view_v1"),
                expr_ident(brand_id.clone()),
                item,
            ]),
        }
    }

    fn require_brand_state(&self, stage_idx: usize) -> Result<&RequireBrandState, CompilerError> {
        self.require_brand_states
            .iter()
            .find(|s| s.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing require_brand state".to_string(),
                )
            })
    }

    fn plugin_state(&self, stage_idx: usize) -> Result<&PluginState, CompilerError> {
        self.plugin_states
            .iter()
            .find(|s| s.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing plugin state".to_string(),
                )
            })
    }

    fn gen_run_bytes_source_expr(&self, item_b: Expr) -> Result<Expr, CompilerError> {
        let mut stmts = vec![
            expr_list(vec![
                expr_ident("let"),
                expr_ident("item_v".to_string()),
                expr_list(vec![expr_ident("bytes.view"), item_b]),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("item_len".to_string()),
                expr_list(vec![
                    expr_ident("view.len"),
                    expr_ident("item_v".to_string()),
                ]),
            ]),
        ];
        stmts.push(set_add_i32(
            &self.bytes_in_var,
            expr_ident("item_len".to_string()),
        ));
        stmts.push(set_add_i32(&self.items_in_var, expr_int(1)));
        stmts.push(self.budget_check_in()?);
        stmts.push(self.gen_process_from(
            0,
            self.apply_src_out_item_brand(expr_ident("item_v".to_string())),
        )?);

        if let Some(stop) = &self.stop_var {
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("="), expr_ident(stop.clone()), expr_int(1)]),
                self.gen_return_ok()?,
                expr_int(0),
            ]));
        }

        if self.has_flush_stages() {
            stmts.push(self.gen_flush_from(0)?);
        }
        stmts.push(self.gen_return_ok()?);

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_run_bytes_source(&self, bytes_param: usize) -> Result<Expr, CompilerError> {
        self.gen_run_bytes_source_expr(param_ident(bytes_param))
    }

    fn gen_run_rr_send_source(&self, key_param: usize) -> Result<Expr, CompilerError> {
        let stmts = vec![
            expr_list(vec![
                expr_ident("let"),
                expr_ident("rr_h".to_string()),
                expr_list(vec![expr_ident("rr.current_v1")]),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("rr_entry_res".to_string()),
                expr_list(vec![
                    expr_ident("rr.next_v1"),
                    expr_ident("rr_h".to_string()),
                    expr_list(vec![expr_ident("bytes.lit"), expr_ident("rr".to_string())]),
                    expr_list(vec![
                        expr_ident("bytes.lit"),
                        expr_ident("std.stream.src.rr_send_v1".to_string()),
                    ]),
                    param_ident(key_param),
                ]),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("rr_entry".to_string()),
                expr_list(vec![
                    expr_ident("result_bytes.unwrap_or"),
                    expr_ident("rr_entry_res".to_string()),
                    expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
                ]),
            ]),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_list(vec![
                        expr_ident("bytes.len"),
                        expr_ident("rr_entry".to_string()),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_RR_MISS, "stream:rr_miss"),
                ]),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("rr_resp".to_string()),
                expr_list(vec![
                    expr_ident("rr.entry_resp_v1"),
                    expr_list(vec![
                        expr_ident("bytes.view"),
                        expr_ident("rr_entry".to_string()),
                    ]),
                ]),
            ]),
            self.gen_run_bytes_source_expr(expr_ident("rr_resp".to_string()))?,
        ];

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_run_reader_source(
        &self,
        open_head: &str,
        open_arg: Expr,
        bufread_cap_bytes: i32,
    ) -> Result<Expr, CompilerError> {
        let mut stmts = Vec::new();
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("reader".to_string()),
            expr_list(vec![expr_ident(open_head.to_string()), open_arg]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("br".to_string()),
            expr_list(vec![
                expr_ident("bufread.new"),
                expr_ident("reader".to_string()),
                expr_int(bufread_cap_bytes),
            ]),
        ]));

        let loop_body = self.gen_reader_loop_body()?;
        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("step".to_string()),
            expr_int(0),
            expr_int(self.max_steps),
            loop_body,
        ]));
        stmts.push(expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_CFG_INVALID, "stream:max_steps_exceeded"),
        ]));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_run_db_rows_doc_source(
        &self,
        conn_param: usize,
        sql_param: usize,
        params_doc_param: usize,
        qcaps_doc_param: usize,
    ) -> Result<Expr, CompilerError> {
        let mut stmts = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident("resp".to_string()),
            expr_list(vec![
                expr_ident("std.db.query_v1"),
                param_ident(conn_param),
                expr_list(vec![expr_ident("view.to_bytes"), param_ident(sql_param)]),
                param_ident(params_doc_param),
                expr_list(vec![expr_ident("bytes.view"), param_ident(qcaps_doc_param)]),
            ]),
        ])];
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("rows_doc".to_string()),
            expr_list(vec![
                expr_ident("std.db.query_rows_doc_v1"),
                expr_ident("resp".to_string()),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("doc".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("rows_doc".to_string()),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("="),
                expr_list(vec![
                    expr_ident("ext.data_model.doc_is_err"),
                    expr_ident("doc".to_string()),
                ]),
                expr_int(1),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_with_payload(
                    expr_int(E_DB_QUERY_FAILED),
                    "stream:db_query_failed",
                    expr_list(vec![
                        expr_ident("codec.write_u32_le"),
                        expr_list(vec![
                            expr_ident("ext.data_model.doc_error_code"),
                            expr_ident("doc".to_string()),
                        ]),
                    ]),
                ),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("!="),
                expr_list(vec![
                    expr_ident("ext.data_model.root_kind"),
                    expr_ident("doc".to_string()),
                ]),
                expr_int(5),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:db_rows_doc_invalid"),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("root".to_string()),
            expr_list(vec![
                expr_ident("ext.data_model.root_offset"),
                expr_ident("doc".to_string()),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("k_rows".to_string()),
            expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident("rows".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("rows_off".to_string()),
            expr_list(vec![
                expr_ident("ext.data_model.map_find"),
                expr_ident("doc".to_string()),
                expr_ident("root".to_string()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident("k_rows".to_string()),
                ]),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident("rows_off".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:db_rows_doc_missing_rows"),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("rows_count".to_string()),
            expr_list(vec![
                expr_ident("ext.data_model.seq_len"),
                expr_ident("doc".to_string()),
                expr_ident("rows_off".to_string()),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident("rows_count".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:db_rows_doc_rows_not_seq"),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("pos".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident("rows_off".to_string()),
                expr_int(5),
            ]),
        ]));

        let mut loop_body = vec![expr_list(vec![
            expr_ident("let"),
            expr_ident("end".to_string()),
            expr_list(vec![
                expr_ident("ext.data_model.skip_value"),
                expr_ident("doc".to_string()),
                expr_ident("pos".to_string()),
            ]),
        ])];
        loop_body.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident("end".to_string()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_CFG_INVALID, "stream:db_rows_doc_row_bad"),
            ]),
            expr_int(0),
        ]));
        loop_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("row".to_string()),
            expr_list(vec![
                expr_ident("view.slice"),
                expr_ident("doc".to_string()),
                expr_ident("pos".to_string()),
                expr_list(vec![
                    expr_ident("-"),
                    expr_ident("end".to_string()),
                    expr_ident("pos".to_string()),
                ]),
            ]),
        ]));

        loop_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("row_len".to_string()),
            expr_list(vec![expr_ident("view.len"), expr_ident("row".to_string())]),
        ]));
        loop_body.push(set_add_i32(
            &self.bytes_in_var,
            expr_ident("row_len".to_string()),
        ));
        loop_body.push(set_add_i32(&self.items_in_var, expr_int(1)));
        loop_body.push(self.budget_check_in()?);
        loop_body.push(self.gen_process_from(
            0,
            self.apply_src_out_item_brand(expr_ident("row".to_string())),
        )?);

        loop_body.push(expr_list(vec![
            expr_ident("set"),
            expr_ident("pos".to_string()),
            expr_ident("end".to_string()),
        ]));

        if let Some(stop) = &self.stop_var {
            loop_body.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("="), expr_ident(stop.clone()), expr_int(1)]),
                self.gen_return_ok()?,
                expr_int(0),
            ]));
        }

        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("_".to_string()),
            expr_int(0),
            expr_ident("rows_count".to_string()),
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(loop_body)
                    .collect(),
            ),
        ]));

        if self.has_flush_stages() {
            stmts.push(self.gen_flush_from(0)?);
        }
        stmts.push(self.gen_return_ok()?);

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_run_net_tcp_read_stream_handle_source(
        &self,
        stream_handle_param: usize,
        caps_param: usize,
        on_timeout: NetOnTimeoutV1,
        on_eof: NetOnEofV1,
    ) -> Result<Expr, CompilerError> {
        let h_var = "net_h".to_string();
        let caps_b_var = "net_caps_b".to_string();
        let caps_v_var = "net_caps".to_string();
        let read_max_var = "net_read_max".to_string();

        let cleanup_stmts: Vec<Expr> = match on_eof {
            NetOnEofV1::LeaveOpen => Vec::new(),
            NetOnEofV1::ShutdownRead => vec![expr_list(vec![
                expr_ident("std.net.tcp.stream_shutdown_v1"),
                expr_ident(h_var.clone()),
                expr_list(vec![expr_ident("std.net.tcp.shutdown_read_v1")]),
            ])],
            NetOnEofV1::Close => vec![
                expr_list(vec![
                    expr_ident("std.net.tcp.stream_close_v1"),
                    expr_ident(h_var.clone()),
                ]),
                expr_list(vec![
                    expr_ident("std.net.tcp.stream_drop_v1"),
                    expr_ident(h_var.clone()),
                ]),
            ],
        };

        let end_ok_with_flush = {
            let mut end_stmts = Vec::new();
            if self.has_flush_stages() {
                end_stmts.push(self.gen_flush_from(0)?);
            }
            end_stmts.extend(cleanup_stmts.clone());
            end_stmts.push(self.gen_return_ok()?);
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(end_stmts)
                    .collect(),
            )
        };

        let end_ok_no_flush = {
            let mut end_stmts = Vec::new();
            end_stmts.extend(cleanup_stmts.clone());
            end_stmts.push(self.gen_return_ok()?);
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(end_stmts)
                    .collect(),
            )
        };

        let timeout_stop_if_clean_expr = if on_timeout == NetOnTimeoutV1::StopIfClean {
            let p0 = self.plugin_state(0)?;
            if p0.plugin.plugin_id != "xf.deframe_u32le_v1" {
                return Err(CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: on_timeout=stop_if_clean without deframe at stage 0"
                        .to_string(),
                ));
            }

            let r_var = "net_timeout_r".to_string();
            let out_b_var = "net_timeout_out_b".to_string();
            let ec_var = "net_timeout_ec".to_string();

            let abi_major = i32::try_from(p0.plugin.abi_major).unwrap_or(i32::MAX);
            let max_out_bytes_per_step =
                i32::try_from(p0.plugin.limits.max_out_bytes_per_step).unwrap_or(i32::MAX);
            let max_out_items_per_step =
                i32::try_from(p0.plugin.limits.max_out_items_per_step).unwrap_or(i32::MAX);
            let max_out_buf_bytes =
                i32::try_from(p0.plugin.limits.max_out_buf_bytes).unwrap_or(i32::MAX);

            let budget_profile = expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident(p0.plugin.budget_profile_id.clone()),
            ]);

            let call = expr_list(vec![
                expr_ident("__internal.stream_xf.plugin_flush_v1"),
                expr_list(vec![
                    expr_ident("bytes.lit"),
                    expr_ident(p0.plugin.native_backend_id.clone()),
                ]),
                expr_int(abi_major),
                expr_list(vec![
                    expr_ident("bytes.lit"),
                    expr_ident(p0.plugin.export_symbol.clone()),
                ]),
                expr_ident(p0.state_b_var.clone()),
                expr_ident(p0.scratch_b_var.clone()),
                expr_int(max_out_bytes_per_step),
                expr_int(max_out_items_per_step),
                expr_int(max_out_buf_bytes),
            ]);

            let end_ok = {
                let mut end_stmts = vec![self.gen_flush_from(1)?];
                end_stmts.extend(cleanup_stmts.clone());
                end_stmts.push(self.gen_return_ok()?);
                expr_list(
                    vec![expr_ident("begin")]
                        .into_iter()
                        .chain(end_stmts)
                        .collect(),
                )
            };

            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(r_var.clone()),
                    expr_list(vec![
                        expr_ident("budget.scope_from_arch_v1"),
                        budget_profile,
                        call,
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_list(vec![
                            expr_ident("result_bytes.is_ok"),
                            expr_ident(r_var.clone()),
                        ]),
                        expr_int(1),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(out_b_var.clone()),
                            expr_list(vec![
                                expr_ident("__internal.result_bytes.unwrap_ok_v1"),
                                expr_ident(r_var.clone()),
                            ]),
                        ]),
                        self.gen_plugin_process_output_blob(0, &out_b_var, None)?,
                        end_ok,
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(ec_var.clone()),
                            expr_list(vec![
                                expr_ident("result_bytes.err_code"),
                                expr_ident(r_var.clone()),
                            ]),
                        ]),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_ident(ec_var),
                                expr_int(E_DEFRAME_TRUNCATED),
                            ]),
                            expr_list(vec![
                                expr_ident("return"),
                                err_doc_const(
                                    E_DEFRAME_TRUNCATED_TIMEOUT,
                                    "stream:deframe_truncated_timeout",
                                ),
                            ]),
                            expr_list(vec![
                                expr_ident("return"),
                                err_doc_const(E_STAGE_FAILED, "stream:stage_failed"),
                            ]),
                        ]),
                    ]),
                ]),
            ])
        } else {
            expr_int(0)
        };

        let mut stmts = vec![
            expr_list(vec![
                expr_ident("let"),
                expr_ident(h_var.clone()),
                param_ident(stream_handle_param),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(caps_b_var.clone()),
                param_ident(caps_param),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(caps_v_var.clone()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident(caps_b_var.clone()),
                ]),
            ]),
        ];

        // Minimal caps validation (strict): len>=24, ver==1, reserved==0.
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_list(vec![expr_ident("view.len"), expr_ident(caps_v_var.clone())]),
                expr_int(24),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("!="),
                expr_list(vec![
                    expr_ident("codec.read_u32_le"),
                    expr_ident(caps_v_var.clone()),
                    expr_int(0),
                ]),
                expr_int(1),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("!="),
                expr_list(vec![
                    expr_ident("codec.read_u32_le"),
                    expr_ident(caps_v_var.clone()),
                    expr_int(20),
                ]),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
            ]),
            expr_int(0),
        ]));

        // Compute effective read max: min(cfg.chunk_max_bytes, caps.max_read_bytes) unless caps=0 (use cfg).
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("caps_max_read".to_string()),
            expr_list(vec![
                expr_ident("codec.read_u32_le"),
                expr_ident(caps_v_var.clone()),
                expr_int(12),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(read_max_var.clone()),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<="),
                    expr_ident("caps_max_read".to_string()),
                    expr_int(0),
                ]),
                expr_int(self.cfg.chunk_max_bytes),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<u"),
                        expr_ident("caps_max_read".to_string()),
                        expr_int(self.cfg.chunk_max_bytes),
                    ]),
                    expr_ident("caps_max_read".to_string()),
                    expr_int(self.cfg.chunk_max_bytes),
                ]),
            ]),
        ]));

        let mut loop_body = Vec::new();
        loop_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("doc".to_string()),
            expr_list(vec![
                expr_ident("std.net.tcp.stream_read_v1"),
                expr_ident(h_var.clone()),
                expr_ident(read_max_var.clone()),
                expr_ident(caps_v_var.clone()),
            ]),
        ]));
        loop_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("dv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("doc".to_string()),
            ]),
        ]));
        loop_body.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("std.net.err.is_err_doc_v1"),
                expr_ident("dv".to_string()),
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("code".to_string()),
                    expr_list(vec![
                        expr_ident("std.net.err.err_code_v1"),
                        expr_ident("dv".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("code".to_string()),
                        expr_list(vec![expr_ident("std.net.err.code_timeout_v1")]),
                    ]),
                    match on_timeout {
                        NetOnTimeoutV1::Err => expr_list(vec![
                            expr_ident("return"),
                            err_doc_with_payload(
                                expr_int(E_NET_READ_FAILED),
                                "stream:net_timeout",
                                expr_ident("doc".to_string()),
                            ),
                        ]),
                        NetOnTimeoutV1::Stop => end_ok_with_flush.clone(),
                        NetOnTimeoutV1::StopIfClean => timeout_stop_if_clean_expr,
                    },
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_NET_READ_FAILED),
                        "stream:net_read_failed",
                        expr_ident("doc".to_string()),
                    ),
                ]),
            ]),
            // OK doc path
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("doc_len".to_string()),
                    expr_list(vec![expr_ident("view.len"), expr_ident("dv".to_string())]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_ident("doc_len".to_string()),
                        expr_int(8),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            expr_ident("dv".to_string()),
                            expr_int(0),
                        ]),
                        expr_int(1),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            expr_ident("dv".to_string()),
                            expr_int(1),
                        ]),
                        expr_int(1),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            expr_ident("dv".to_string()),
                            expr_int(2),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            expr_ident("dv".to_string()),
                            expr_int(3),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl_len".to_string()),
                    expr_list(vec![
                        expr_ident("codec.read_u32_le"),
                        expr_ident("dv".to_string()),
                        expr_int(4),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_ident("pl_len".to_string()),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("!="),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_int(8),
                            expr_ident("pl_len".to_string()),
                        ]),
                        expr_ident("doc_len".to_string()),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_READ_DOC_INVALID, "stream:net_read_doc_invalid"),
                    ]),
                    expr_int(0),
                ]),
                // EOF
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("pl_len".to_string()),
                        expr_int(0),
                    ]),
                    end_ok_with_flush.clone(),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_ident("pl_len".to_string()),
                        expr_ident(read_max_var.clone()),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_NET_BACKEND_OVERRAN_MAX, "stream:net_backend_overran_max"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("chunk".to_string()),
                    expr_list(vec![
                        expr_ident("view.slice"),
                        expr_ident("dv".to_string()),
                        expr_int(8),
                        expr_ident("pl_len".to_string()),
                    ]),
                ]),
                set_add_i32(&self.bytes_in_var, expr_ident("pl_len".to_string())),
                set_add_i32(&self.items_in_var, expr_int(1)),
                self.budget_check_in()?,
                self.gen_process_from(
                    0,
                    self.apply_src_out_item_brand(expr_ident("chunk".to_string())),
                )?,
            ]),
        ]));
        if let Some(stop) = &self.stop_var {
            loop_body.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("="), expr_ident(stop.clone()), expr_int(1)]),
                end_ok_no_flush,
                expr_int(0),
            ]));
        }
        loop_body.push(expr_int(0));

        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident("step".to_string()),
            expr_int(0),
            expr_int(self.max_steps),
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(loop_body)
                    .collect(),
            ),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_CFG_INVALID, "stream:max_steps_exceeded"),
        ]));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_reader_loop_body(&self) -> Result<Expr, CompilerError> {
        let mut body = Vec::new();

        body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("chunk0".to_string()),
            expr_list(vec![
                expr_ident("bufread.fill"),
                expr_ident("br".to_string()),
            ]),
        ]));
        body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("n0".to_string()),
            expr_list(vec![
                expr_ident("view.len"),
                expr_ident("chunk0".to_string()),
            ]),
        ]));
        body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("n".to_string()),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident(">u"),
                    expr_ident("n0".to_string()),
                    expr_int(self.cfg.chunk_max_bytes),
                ]),
                expr_int(self.cfg.chunk_max_bytes),
                expr_ident("n0".to_string()),
            ]),
        ]));

        // EOF: flush and return OK.
        let mut eof_stmts = Vec::new();
        if self.has_flush_stages() {
            eof_stmts.push(self.gen_flush_from(0)?);
        }
        eof_stmts.push(self.gen_return_ok()?);
        body.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("="),
                expr_ident("n".to_string()),
                expr_int(0),
            ]),
            expr_list(
                vec![expr_ident("begin")]
                    .into_iter()
                    .chain(eof_stmts)
                    .collect(),
            ),
            expr_int(0),
        ]));

        body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("chunk".to_string()),
            expr_list(vec![
                expr_ident("view.slice"),
                expr_ident("chunk0".to_string()),
                expr_int(0),
                expr_ident("n".to_string()),
            ]),
        ]));

        body.push(set_add_i32(&self.bytes_in_var, expr_ident("n".to_string())));
        body.push(set_add_i32(&self.items_in_var, expr_int(1)));
        body.push(self.budget_check_in()?);

        body.push(self.gen_process_from(
            0,
            self.apply_src_out_item_brand(expr_ident("chunk".to_string())),
        )?);

        body.push(expr_list(vec![
            expr_ident("bufread.consume"),
            expr_ident("br".to_string()),
            expr_ident("n".to_string()),
        ]));

        if let Some(stop) = &self.stop_var {
            body.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![expr_ident("="), expr_ident(stop.clone()), expr_int(1)]),
                self.gen_return_ok()?,
                expr_int(0),
            ]));
        }

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(body).collect(),
        ))
    }

    fn budget_check_in(&self) -> Result<Expr, CompilerError> {
        Ok(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident(self.bytes_in_var.clone()),
                expr_int(self.cfg.max_in_bytes),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BUDGET_IN_BYTES, "stream:budget_in_bytes"),
            ]),
            expr_int(0),
        ]))
    }

    fn gen_process_from(&self, stage_idx: usize, item: Expr) -> Result<Expr, CompilerError> {
        if stage_idx >= self.chain.len() {
            return self.emit_item(item);
        }
        match &self.chain[stage_idx].kind {
            PipeXfV1::MapBytes { fn_id } => {
                let mapped_b = format!("mapped_{stage_idx}");
                let mapped_v = format!("mappedv_{stage_idx}");
                Ok(expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(mapped_b.clone()),
                        expr_list(vec![expr_ident(fn_id.clone()), item]),
                    ]),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(mapped_v.clone()),
                        expr_list(vec![expr_ident("bytes.view"), expr_ident(mapped_b)]),
                    ]),
                    self.gen_process_from(
                        stage_idx + 1,
                        self.apply_out_item_brand(stage_idx, expr_ident(mapped_v)),
                    )?,
                ]))
            }
            PipeXfV1::Filter { fn_id } => {
                let keep = format!("keep_{stage_idx}");
                Ok(expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(keep.clone()),
                        expr_list(vec![expr_ident(fn_id.clone()), item.clone()]),
                    ]),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![expr_ident("="), expr_ident(keep), expr_int(0)]),
                        expr_int(0),
                        self.gen_process_from(
                            stage_idx + 1,
                            self.apply_out_item_brand(stage_idx, item),
                        )?,
                    ]),
                ]))
            }
            PipeXfV1::RequireBrandV1 {
                brand_id,
                validator_id,
                max_item_bytes,
            } => self.gen_require_brand_v1(
                stage_idx,
                item,
                brand_id,
                validator_id.as_deref(),
                *max_item_bytes,
            ),
            PipeXfV1::Take { .. } => {
                let t = self
                    .take_states
                    .iter()
                    .find(|t| t.stage_idx == stage_idx)
                    .ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Internal,
                            "internal error: missing take state".to_string(),
                        )
                    })?;
                let stop = self.stop_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: take stage without stop var".to_string(),
                    )
                })?;
                Ok(expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<="),
                        expr_ident(t.rem_var.clone()),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("set"),
                            expr_ident(stop.clone()),
                            expr_int(1),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("set"),
                            expr_ident(t.rem_var.clone()),
                            expr_list(vec![
                                expr_ident("-"),
                                expr_ident(t.rem_var.clone()),
                                expr_int(1),
                            ]),
                        ]),
                        self.gen_process_from(
                            stage_idx + 1,
                            self.apply_out_item_brand(stage_idx, item),
                        )?,
                    ]),
                ]))
            }
            PipeXfV1::FrameU32Le => self.gen_plugin_step(stage_idx, item),
            PipeXfV1::MapInPlaceBufV1 { .. } => self.gen_map_in_place_buf(stage_idx, item),
            PipeXfV1::SplitLines { .. } => self.gen_split_lines(stage_idx, item),
            PipeXfV1::DeframeU32LeV1 { .. } => self.gen_deframe_u32le(stage_idx, item),
            PipeXfV1::JsonCanonStreamV1 { .. } => {
                self.gen_json_canon_stream_process(stage_idx, item)
            }
            PipeXfV1::PluginV1 { .. } => self.gen_plugin_step(stage_idx, item),
            PipeXfV1::ParMapStreamV1 { .. } => self.gen_par_map_stream_process(stage_idx, item),
        }
    }

    fn gen_plugin_init(&self, stage_idx: usize) -> Result<Expr, CompilerError> {
        let p = self.plugin_state(stage_idx)?;

        let canon_mode = match p.plugin.cfg.canon_mode {
            StreamPluginCfgCanonModeV1::NoneV1 => 0,
            StreamPluginCfgCanonModeV1::CanonJsonV1 => 1,
        };

        let abi_major = i32::try_from(p.plugin.abi_major).unwrap_or(i32::MAX);
        let cfg_max_bytes = i32::try_from(p.plugin.cfg.max_bytes).unwrap_or(i32::MAX);
        let max_out_bytes_per_step =
            i32::try_from(p.plugin.limits.max_out_bytes_per_step).unwrap_or(i32::MAX);
        let max_out_items_per_step =
            i32::try_from(p.plugin.limits.max_out_items_per_step).unwrap_or(i32::MAX);
        let max_out_buf_bytes =
            i32::try_from(p.plugin.limits.max_out_buf_bytes).unwrap_or(i32::MAX);

        let budget_profile = expr_list(vec![
            expr_ident("bytes.lit"),
            expr_ident(p.plugin.budget_profile_id.clone()),
        ]);

        let call = expr_list(vec![
            expr_ident("__internal.stream_xf.plugin_init_v1"),
            expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident(p.plugin.native_backend_id.clone()),
            ]),
            expr_int(abi_major),
            expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident(p.plugin.export_symbol.clone()),
            ]),
            expr_ident(p.state_b_var.clone()),
            expr_ident(p.scratch_b_var.clone()),
            expr_ident(p.cfg_b_var.clone()),
            expr_int(cfg_max_bytes),
            expr_int(canon_mode),
            expr_int(p.strict_cfg_canon),
            expr_int(max_out_bytes_per_step),
            expr_int(max_out_items_per_step),
            expr_int(max_out_buf_bytes),
        ]);

        self.gen_plugin_call_and_process_outputs(
            p,
            PluginCallKindV1::Init,
            budget_profile,
            call,
            None,
        )
    }

    fn gen_plugin_step(&self, stage_idx: usize, item: Expr) -> Result<Expr, CompilerError> {
        let p = self.plugin_state(stage_idx)?;
        let in_item_var = format!("xf_in_{stage_idx}");

        let abi_major = i32::try_from(p.plugin.abi_major).unwrap_or(i32::MAX);
        let max_out_bytes_per_step =
            i32::try_from(p.plugin.limits.max_out_bytes_per_step).unwrap_or(i32::MAX);
        let max_out_items_per_step =
            i32::try_from(p.plugin.limits.max_out_items_per_step).unwrap_or(i32::MAX);
        let max_out_buf_bytes =
            i32::try_from(p.plugin.limits.max_out_buf_bytes).unwrap_or(i32::MAX);

        let budget_profile = expr_list(vec![
            expr_ident("bytes.lit"),
            expr_ident(p.plugin.budget_profile_id.clone()),
        ]);

        let call = expr_list(vec![
            expr_ident("__internal.stream_xf.plugin_step_v1"),
            expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident(p.plugin.native_backend_id.clone()),
            ]),
            expr_int(abi_major),
            expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident(p.plugin.export_symbol.clone()),
            ]),
            expr_ident(p.state_b_var.clone()),
            expr_ident(p.scratch_b_var.clone()),
            expr_int(max_out_bytes_per_step),
            expr_int(max_out_items_per_step),
            expr_int(max_out_buf_bytes),
            expr_ident(in_item_var.clone()),
        ]);

        Ok(expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(in_item_var.clone()),
                item,
            ]),
            self.gen_plugin_call_and_process_outputs(
                p,
                PluginCallKindV1::Step,
                budget_profile,
                call,
                Some(in_item_var.as_str()),
            )?,
        ]))
    }

    fn gen_plugin_flush(&self, stage_idx: usize) -> Result<Expr, CompilerError> {
        let p = self.plugin_state(stage_idx)?;

        let abi_major = i32::try_from(p.plugin.abi_major).unwrap_or(i32::MAX);
        let max_out_bytes_per_step =
            i32::try_from(p.plugin.limits.max_out_bytes_per_step).unwrap_or(i32::MAX);
        let max_out_items_per_step =
            i32::try_from(p.plugin.limits.max_out_items_per_step).unwrap_or(i32::MAX);
        let max_out_buf_bytes =
            i32::try_from(p.plugin.limits.max_out_buf_bytes).unwrap_or(i32::MAX);

        let budget_profile = expr_list(vec![
            expr_ident("bytes.lit"),
            expr_ident(p.plugin.budget_profile_id.clone()),
        ]);

        let call = expr_list(vec![
            expr_ident("__internal.stream_xf.plugin_flush_v1"),
            expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident(p.plugin.native_backend_id.clone()),
            ]),
            expr_int(abi_major),
            expr_list(vec![
                expr_ident("bytes.lit"),
                expr_ident(p.plugin.export_symbol.clone()),
            ]),
            expr_ident(p.state_b_var.clone()),
            expr_ident(p.scratch_b_var.clone()),
            expr_int(max_out_bytes_per_step),
            expr_int(max_out_items_per_step),
            expr_int(max_out_buf_bytes),
        ]);

        let mut stmts: Vec<Expr> = vec![self.gen_plugin_call_and_process_outputs(
            p,
            PluginCallKindV1::Flush,
            budget_profile,
            call,
            None,
        )?];
        stmts.push(self.gen_flush_from(stage_idx + 1)?);
        stmts.push(expr_int(0));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_plugin_call_and_process_outputs(
        &self,
        p: &PluginState,
        call_kind: PluginCallKindV1,
        budget_profile: Expr,
        call: Expr,
        in_item_var: Option<&str>,
    ) -> Result<Expr, CompilerError> {
        let stage_idx = p.stage_idx;
        let r_var = format!("xf_r_{stage_idx}");
        let out_b_var = format!("xf_out_b_{stage_idx}");
        let ec_var = format!("xf_ec_{stage_idx}");

        let stage_idx_i32 = i32::try_from(stage_idx).unwrap_or(i32::MAX);

        let payload = expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("pl".to_string()),
                expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(16)]),
            ]),
            extend_u32("pl", expr_int(stage_idx_i32)),
            extend_u32("pl", expr_ident(self.bytes_in_var.clone())),
            extend_u32("pl", expr_ident(self.items_in_var.clone())),
            extend_u32("pl", expr_ident(ec_var.clone())),
            expr_list(vec![
                expr_ident("vec_u8.into_bytes"),
                expr_ident("pl".to_string()),
            ]),
        ]);

        let fallback_doc = err_doc_with_payload(
            expr_ident(ec_var.clone()),
            "stream:xf_plugin_error",
            payload.clone(),
        );

        let plugin_err_doc = match p.err_map {
            PluginErrMapV1::Generic => fallback_doc.clone(),
            PluginErrMapV1::SplitLinesV1 => match call_kind {
                PluginCallKindV1::Init => expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident(ec_var.clone()),
                        expr_int(E_CFG_INVALID),
                    ]),
                    err_doc_const(E_CFG_INVALID, "stream:split_lines_max_line_bytes"),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("="),
                            expr_ident(ec_var.clone()),
                            expr_int(E_LINE_TOO_LONG),
                        ]),
                        err_doc_const(E_LINE_TOO_LONG, "stream:line_too_long"),
                        fallback_doc.clone(),
                    ]),
                ]),
                PluginCallKindV1::Step | PluginCallKindV1::Flush => expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident(ec_var.clone()),
                        expr_int(E_LINE_TOO_LONG),
                    ]),
                    err_doc_const(E_LINE_TOO_LONG, "stream:line_too_long"),
                    fallback_doc.clone(),
                ]),
            },
            PluginErrMapV1::FrameU32LeV1 => expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident(ec_var.clone()),
                    expr_int(E_FRAME_TOO_LARGE),
                ]),
                err_doc_const(E_FRAME_TOO_LARGE, "stream:frame_too_large"),
                fallback_doc.clone(),
            ]),
            PluginErrMapV1::DeframeU32LeV1 => {
                let base = expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident(ec_var.clone()),
                        expr_int(E_DEFRAME_FRAME_TOO_LARGE),
                    ]),
                    err_doc_const(E_DEFRAME_FRAME_TOO_LARGE, "stream:deframe_frame_too_large"),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("="),
                            expr_ident(ec_var.clone()),
                            expr_int(E_DEFRAME_TRUNCATED),
                        ]),
                        err_doc_const(E_DEFRAME_TRUNCATED, "stream:deframe_truncated"),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_ident(ec_var.clone()),
                                expr_int(E_DEFRAME_EMPTY_FORBIDDEN),
                            ]),
                            err_doc_const(
                                E_DEFRAME_EMPTY_FORBIDDEN,
                                "stream:deframe_empty_forbidden",
                            ),
                            expr_list(vec![
                                expr_ident("if"),
                                expr_list(vec![
                                    expr_ident("="),
                                    expr_ident(ec_var.clone()),
                                    expr_int(E_DEFRAME_MAX_FRAMES),
                                ]),
                                err_doc_const(E_DEFRAME_MAX_FRAMES, "stream:deframe_max_frames"),
                                fallback_doc.clone(),
                            ]),
                        ]),
                    ]),
                ]);

                if call_kind == PluginCallKindV1::Init {
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("="),
                            expr_ident(ec_var.clone()),
                            expr_int(E_CFG_INVALID),
                        ]),
                        err_doc_const(E_CFG_INVALID, "stream:deframe_max_frame_bytes"),
                        base,
                    ])
                } else {
                    base
                }
            }
            PluginErrMapV1::JsonCanonStreamV1 => match call_kind {
                PluginCallKindV1::Init => expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident(ec_var.clone()),
                        expr_int(E_CFG_INVALID),
                    ]),
                    err_doc_const(E_CFG_INVALID, "stream:json_canon_cfg_invalid"),
                    fallback_doc.clone(),
                ]),
                PluginCallKindV1::Step => {
                    let st_v_var = format!("xf_json_state_v_{stage_idx}");
                    let kind_var = format!("xf_json_err_kind_{stage_idx}");
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("="),
                            expr_ident(ec_var.clone()),
                            expr_int(E_BUDGET_IN_BYTES),
                        ]),
                        expr_list(vec![
                            expr_ident("begin"),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident(st_v_var.clone()),
                                expr_list(vec![
                                    expr_ident("bytes.view"),
                                    expr_ident(p.state_b_var.clone()),
                                ]),
                            ]),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident(kind_var.clone()),
                                expr_list(vec![
                                    expr_ident("codec.read_u32_le"),
                                    expr_ident(st_v_var),
                                    expr_int(20),
                                ]),
                            ]),
                            expr_list(vec![
                                expr_ident("if"),
                                expr_list(vec![expr_ident("="), expr_ident(kind_var), expr_int(1)]),
                                err_doc_const(E_BUDGET_IN_BYTES, "stream:json_input_too_large"),
                                err_doc_const(
                                    E_BUDGET_IN_BYTES,
                                    "stream:json_max_total_json_bytes",
                                ),
                            ]),
                        ]),
                        fallback_doc.clone(),
                    ])
                }
                PluginCallKindV1::Flush => {
                    let st_v_var = format!("xf_json_state_v_{stage_idx}");
                    let off_var = format!("xf_json_err_off_{stage_idx}");
                    let pl_var = format!("xf_json_pl_{stage_idx}");
                    let plb_var = format!("xf_json_plb_{stage_idx}");

                    let mk_doc = |msg_id: &'static str, code: i32| -> Expr {
                        expr_list(vec![
                            expr_ident("begin"),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident(st_v_var.clone()),
                                expr_list(vec![
                                    expr_ident("bytes.view"),
                                    expr_ident(p.state_b_var.clone()),
                                ]),
                            ]),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident(off_var.clone()),
                                expr_list(vec![
                                    expr_ident("codec.read_u32_le"),
                                    expr_ident(st_v_var.clone()),
                                    expr_int(24),
                                ]),
                            ]),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident(pl_var.clone()),
                                expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(8)]),
                            ]),
                            extend_u32(&pl_var, expr_ident(off_var.clone())),
                            extend_u32(&pl_var, expr_int(stage_idx_i32)),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident(plb_var.clone()),
                                expr_list(vec![
                                    expr_ident("vec_u8.into_bytes"),
                                    expr_ident(pl_var.clone()),
                                ]),
                            ]),
                            err_doc_with_payload(
                                expr_int(code),
                                msg_id,
                                expr_ident(plb_var.clone()),
                            ),
                        ])
                    };

                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("="),
                            expr_ident(ec_var.clone()),
                            expr_int(E_CFG_INVALID),
                        ]),
                        err_doc_const(E_CFG_INVALID, "stream:json_doc_invalid"),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_ident(ec_var.clone()),
                                expr_int(E_JSON_SYNTAX),
                            ]),
                            mk_doc("stream:json_syntax", E_JSON_SYNTAX),
                            expr_list(vec![
                                expr_ident("if"),
                                expr_list(vec![
                                    expr_ident("="),
                                    expr_ident(ec_var.clone()),
                                    expr_int(E_JSON_NOT_IJSON),
                                ]),
                                mk_doc("stream:json_not_ijson", E_JSON_NOT_IJSON),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident("="),
                                        expr_ident(ec_var.clone()),
                                        expr_int(E_JSON_TOO_DEEP),
                                    ]),
                                    mk_doc("stream:json_too_deep", E_JSON_TOO_DEEP),
                                    expr_list(vec![
                                        expr_ident("if"),
                                        expr_list(vec![
                                            expr_ident("="),
                                            expr_ident(ec_var.clone()),
                                            expr_int(E_JSON_OBJECT_TOO_LARGE),
                                        ]),
                                        mk_doc(
                                            "stream:json_object_too_large",
                                            E_JSON_OBJECT_TOO_LARGE,
                                        ),
                                        expr_list(vec![
                                            expr_ident("if"),
                                            expr_list(vec![
                                                expr_ident("="),
                                                expr_ident(ec_var.clone()),
                                                expr_int(E_JSON_TRAILING_DATA),
                                            ]),
                                            mk_doc(
                                                "stream:json_trailing_data",
                                                E_JSON_TRAILING_DATA,
                                            ),
                                            fallback_doc.clone(),
                                        ]),
                                    ]),
                                ]),
                            ]),
                        ]),
                    ])
                }
            },
        };

        let err_doc = expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident(ec_var.clone()),
                expr_int(0),
            ]),
            err_doc_with_payload(
                expr_int(E_STAGE_FAILED),
                "stream:xf_budget_scope_violation",
                payload.clone(),
            ),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident(ec_var.clone()),
                    expr_int(E_XF_CFG_TOO_LARGE),
                ]),
                err_doc_with_payload(
                    expr_int(E_XF_CFG_TOO_LARGE),
                    "stream:xf_cfg_too_large",
                    payload.clone(),
                ),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident(ec_var.clone()),
                        expr_int(E_XF_CFG_NON_CANON),
                    ]),
                    err_doc_with_payload(
                        expr_int(E_XF_CFG_NON_CANON),
                        "stream:xf_cfg_non_canon",
                        payload.clone(),
                    ),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("="),
                            expr_ident(ec_var.clone()),
                            expr_int(E_XF_OUT_INVALID),
                        ]),
                        err_doc_with_payload(
                            expr_int(E_XF_OUT_INVALID),
                            "stream:xf_out_invalid",
                            payload.clone(),
                        ),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_ident(ec_var.clone()),
                                expr_int(E_XF_EMIT_BUF_TOO_LARGE),
                            ]),
                            err_doc_with_payload(
                                expr_int(E_XF_EMIT_BUF_TOO_LARGE),
                                "stream:xf_emit_buf_too_large",
                                payload.clone(),
                            ),
                            expr_list(vec![
                                expr_ident("if"),
                                expr_list(vec![
                                    expr_ident("="),
                                    expr_ident(ec_var.clone()),
                                    expr_int(E_XF_EMIT_STEP_BYTES_EXCEEDED),
                                ]),
                                err_doc_with_payload(
                                    expr_int(E_XF_EMIT_STEP_BYTES_EXCEEDED),
                                    "stream:xf_emit_step_bytes_exceeded",
                                    payload.clone(),
                                ),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident("="),
                                        expr_ident(ec_var.clone()),
                                        expr_int(E_XF_EMIT_STEP_ITEMS_EXCEEDED),
                                    ]),
                                    err_doc_with_payload(
                                        expr_int(E_XF_EMIT_STEP_ITEMS_EXCEEDED),
                                        "stream:xf_emit_step_items_exceeded",
                                        payload.clone(),
                                    ),
                                    expr_list(vec![
                                        expr_ident("if"),
                                        expr_list(vec![
                                            expr_ident("="),
                                            expr_ident(ec_var.clone()),
                                            expr_int(E_XF_EMIT_LEN_GT_CAP),
                                        ]),
                                        err_doc_with_payload(
                                            expr_int(E_XF_EMIT_LEN_GT_CAP),
                                            "stream:xf_emit_len_gt_cap",
                                            payload.clone(),
                                        ),
                                        expr_list(vec![
                                            expr_ident("if"),
                                            expr_list(vec![
                                                expr_ident("="),
                                                expr_ident(ec_var.clone()),
                                                expr_int(E_XF_PLUGIN_INVALID),
                                            ]),
                                            err_doc_with_payload(
                                                expr_int(E_XF_PLUGIN_INVALID),
                                                "stream:xf_plugin_invalid",
                                                payload.clone(),
                                            ),
                                            plugin_err_doc,
                                        ]),
                                    ]),
                                ]),
                            ]),
                        ]),
                    ]),
                ]),
            ]),
        ]);

        Ok(expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(r_var.clone()),
                expr_list(vec![
                    expr_ident("budget.scope_from_arch_v1"),
                    budget_profile,
                    call,
                ]),
            ]),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_list(vec![
                        expr_ident("result_bytes.is_ok"),
                        expr_ident(r_var.clone()),
                    ]),
                    expr_int(1),
                ]),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(out_b_var.clone()),
                        expr_list(vec![
                            expr_ident("__internal.result_bytes.unwrap_ok_v1"),
                            expr_ident(r_var.clone()),
                        ]),
                    ]),
                    self.gen_plugin_process_output_blob(stage_idx, &out_b_var, in_item_var)?,
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(ec_var.clone()),
                        expr_list(vec![
                            expr_ident("result_bytes.err_code"),
                            expr_ident(r_var.clone()),
                        ]),
                    ]),
                    expr_list(vec![expr_ident("return"), err_doc]),
                ]),
            ]),
        ]))
    }

    fn gen_plugin_process_output_blob(
        &self,
        stage_idx: usize,
        out_b_var: &str,
        in_item_var: Option<&str>,
    ) -> Result<Expr, CompilerError> {
        let p = self.plugin_state(stage_idx)?;

        let out_v_var = format!("xf_out_v_{stage_idx}");
        let out_len_var = format!("xf_out_len_{stage_idx}");
        let count_var = format!("xf_out_count_{stage_idx}");
        let pos_var = format!("xf_out_pos_{stage_idx}");
        let tag_var = format!("xf_out_tag_{stage_idx}");
        let view_off_var = format!("xf_out_view_off_{stage_idx}");
        let item_len_var = format!("xf_out_item_len_{stage_idx}");
        let item_var = format!("xf_out_item_{stage_idx}");
        let loop_i_var = format!("xf_out_i_{stage_idx}");
        let scratch_v_var = format!("xf_out_scratch_v_{stage_idx}");
        let scratch_len_var = format!("xf_out_scratch_len_{stage_idx}");
        let in_len_var = format!("xf_out_in_len_{stage_idx}");

        let err_out_invalid = || err_doc_const(E_XF_OUT_INVALID, "stream:xf_out_invalid");

        let mut stmts: Vec<Expr> = vec![
            expr_list(vec![
                expr_ident("let"),
                expr_ident(out_v_var.clone()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident(out_b_var.to_string()),
                ]),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(out_len_var.clone()),
                expr_list(vec![expr_ident("view.len"), expr_ident(out_v_var.clone())]),
            ]),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<"),
                    expr_ident(out_len_var.clone()),
                    expr_int(4),
                ]),
                expr_list(vec![expr_ident("return"), err_out_invalid()]),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(count_var.clone()),
                expr_list(vec![
                    expr_ident("codec.read_u32_le"),
                    expr_ident(out_v_var.clone()),
                    expr_int(0),
                ]),
            ]),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<"),
                    expr_ident(count_var.clone()),
                    expr_int(0),
                ]),
                expr_list(vec![expr_ident("return"), err_out_invalid()]),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(pos_var.clone()),
                expr_int(4),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(scratch_v_var.clone()),
                expr_list(vec![
                    expr_ident("bytes.view"),
                    expr_ident(p.scratch_b_var.clone()),
                ]),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(scratch_len_var.clone()),
                expr_list(vec![
                    expr_ident("view.len"),
                    expr_ident(scratch_v_var.clone()),
                ]),
            ]),
        ];
        if let Some(in_item_var) = in_item_var {
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(in_len_var.clone()),
                expr_list(vec![
                    expr_ident("view.len"),
                    expr_ident(in_item_var.to_string()),
                ]),
            ]));
        }

        let emit_inline = || -> Result<Expr, CompilerError> {
            Ok(expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_list(vec![
                            expr_ident("-"),
                            expr_ident(out_len_var.clone()),
                            expr_ident(pos_var.clone()),
                        ]),
                        expr_int(4),
                    ]),
                    expr_list(vec![expr_ident("return"), err_out_invalid()]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(item_len_var.clone()),
                    expr_list(vec![
                        expr_ident("codec.read_u32_le"),
                        expr_ident(out_v_var.clone()),
                        expr_ident(pos_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_ident(item_len_var.clone()),
                        expr_int(0),
                    ]),
                    expr_list(vec![expr_ident("return"), err_out_invalid()]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident(pos_var.clone()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident(pos_var.clone()),
                        expr_int(4),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_list(vec![
                            expr_ident("-"),
                            expr_ident(out_len_var.clone()),
                            expr_ident(pos_var.clone()),
                        ]),
                        expr_ident(item_len_var.clone()),
                    ]),
                    expr_list(vec![expr_ident("return"), err_out_invalid()]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(item_var.clone()),
                    expr_list(vec![
                        expr_ident("view.slice"),
                        expr_ident(out_v_var.clone()),
                        expr_ident(pos_var.clone()),
                        expr_ident(item_len_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident(pos_var.clone()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident(pos_var.clone()),
                        expr_ident(item_len_var.clone()),
                    ]),
                ]),
                self.gen_process_from(
                    stage_idx + 1,
                    self.apply_out_item_brand(stage_idx, expr_ident(item_var.clone())),
                )?,
                expr_int(0),
            ]))
        };

        let emit_view = |base_v: Expr, base_len: Expr| -> Result<Expr, CompilerError> {
            Ok(expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_list(vec![
                            expr_ident("-"),
                            expr_ident(out_len_var.clone()),
                            expr_ident(pos_var.clone()),
                        ]),
                        expr_int(8),
                    ]),
                    expr_list(vec![expr_ident("return"), err_out_invalid()]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(view_off_var.clone()),
                    expr_list(vec![
                        expr_ident("codec.read_u32_le"),
                        expr_ident(out_v_var.clone()),
                        expr_ident(pos_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident(pos_var.clone()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident(pos_var.clone()),
                        expr_int(4),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(item_len_var.clone()),
                    expr_list(vec![
                        expr_ident("codec.read_u32_le"),
                        expr_ident(out_v_var.clone()),
                        expr_ident(pos_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident(pos_var.clone()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident(pos_var.clone()),
                        expr_int(4),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("<"),
                            expr_ident(view_off_var.clone()),
                            expr_int(0),
                        ]),
                        expr_int(1),
                        expr_list(vec![
                            expr_ident("<"),
                            expr_ident(item_len_var.clone()),
                            expr_int(0),
                        ]),
                    ]),
                    expr_list(vec![expr_ident("return"), err_out_invalid()]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<"),
                        expr_list(vec![
                            expr_ident("-"),
                            base_len,
                            expr_ident(view_off_var.clone()),
                        ]),
                        expr_ident(item_len_var.clone()),
                    ]),
                    expr_list(vec![expr_ident("return"), err_out_invalid()]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(item_var.clone()),
                    expr_list(vec![
                        expr_ident("view.slice"),
                        base_v,
                        expr_ident(view_off_var.clone()),
                        expr_ident(item_len_var.clone()),
                    ]),
                ]),
                self.gen_process_from(
                    stage_idx + 1,
                    self.apply_out_item_brand(stage_idx, expr_ident(item_var.clone())),
                )?,
                expr_int(0),
            ]))
        };

        let view_branch = if let Some(in_item_var) = in_item_var {
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident(tag_var.clone()),
                    expr_int(1),
                ]),
                emit_view(
                    expr_ident(in_item_var.to_string()),
                    expr_ident(in_len_var.clone()),
                )?,
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident(tag_var.clone()),
                        expr_int(2),
                    ]),
                    emit_view(
                        expr_ident(scratch_v_var.clone()),
                        expr_ident(scratch_len_var.clone()),
                    )?,
                    expr_list(vec![expr_ident("return"), err_out_invalid()]),
                ]),
            ])
        } else {
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident(tag_var.clone()),
                    expr_int(2),
                ]),
                emit_view(
                    expr_ident(scratch_v_var.clone()),
                    expr_ident(scratch_len_var.clone()),
                )?,
                expr_list(vec![expr_ident("return"), err_out_invalid()]),
            ])
        };

        let loop_body = expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<"),
                    expr_list(vec![
                        expr_ident("-"),
                        expr_ident(out_len_var.clone()),
                        expr_ident(pos_var.clone()),
                    ]),
                    expr_int(8),
                ]),
                expr_list(vec![expr_ident("return"), err_out_invalid()]),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("let"),
                expr_ident(tag_var.clone()),
                expr_list(vec![
                    expr_ident("codec.read_u32_le"),
                    expr_ident(out_v_var.clone()),
                    expr_ident(pos_var.clone()),
                ]),
            ]),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<"),
                    expr_ident(tag_var.clone()),
                    expr_int(0),
                ]),
                expr_list(vec![expr_ident("return"), err_out_invalid()]),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("set"),
                expr_ident(pos_var.clone()),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident(pos_var.clone()),
                    expr_int(4),
                ]),
            ]),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident(tag_var.clone()),
                    expr_int(0),
                ]),
                emit_inline()?,
                view_branch,
            ]),
            expr_int(0),
        ]);

        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident(loop_i_var),
            expr_int(0),
            expr_ident(count_var.clone()),
            loop_body,
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("!="),
                expr_ident(pos_var.clone()),
                expr_ident(out_len_var.clone()),
            ]),
            expr_list(vec![expr_ident("return"), err_out_invalid()]),
            expr_int(0),
        ]));
        stmts.push(expr_int(0));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_require_brand_v1(
        &self,
        stage_idx: usize,
        item: Expr,
        brand_id: &str,
        validator_id: Option<&str>,
        max_item_bytes: i32,
    ) -> Result<Expr, CompilerError> {
        let Some(validator_id) = validator_id else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: require_brand stage missing validator_id".to_string(),
            ));
        };

        let s = self.require_brand_state(stage_idx)?;
        let item_idx_var = format!("req_brand_item_idx_{stage_idx}");

        let mut stmts = Vec::new();

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(item_idx_var.clone()),
            expr_ident(s.item_idx_var.clone()),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(s.item_idx_var.clone()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident(s.item_idx_var.clone()),
                expr_int(1),
            ]),
        ]));

        let item_n_var = format!("req_brand_item_n_{stage_idx}");
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(item_n_var.clone()),
            expr_list(vec![expr_ident("view.len"), item.clone()]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident(item_n_var.clone()),
                expr_int(0),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BRAND_ITEM_TOO_LARGE, "stream:brand_item_too_large"),
            ]),
            expr_int(0),
        ]));
        if max_item_bytes > 0 {
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident(">u"),
                    expr_ident(item_n_var.clone()),
                    expr_int(max_item_bytes),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_BRAND_ITEM_TOO_LARGE, "stream:brand_item_too_large"),
                ]),
                expr_int(0),
            ]));
        }

        let r_var = format!("req_brand_r_{stage_idx}");
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(r_var.clone()),
            expr_list(vec![expr_ident(validator_id.to_string()), item.clone()]),
        ]));

        let ok_branch = {
            let branded_item = expr_list(vec![
                expr_ident("__internal.brand.assume_view_v1"),
                expr_ident(brand_id.to_string()),
                item,
            ]);
            self.gen_process_from(
                stage_idx + 1,
                self.apply_out_item_brand(stage_idx, branded_item),
            )?
        };

        let err_branch = {
            let ec_var = format!("req_brand_ec_{stage_idx}");
            let pl_var = format!("req_brand_pl_{stage_idx}");
            let brand_b_var = format!("req_brand_brandb_{stage_idx}");
            let brand_v_var = format!("req_brand_brandv_{stage_idx}");
            let brand_len_var = format!("req_brand_brandlen_{stage_idx}");
            let validator_b_var = format!("req_brand_vb_{stage_idx}");
            let validator_v_var = format!("req_brand_vv_{stage_idx}");
            let validator_len_var = format!("req_brand_vlen_{stage_idx}");

            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(ec_var.clone()),
                    expr_list(vec![
                        expr_ident("result_i32.err_code"),
                        expr_ident(r_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(brand_b_var.clone()),
                    expr_list(vec![
                        expr_ident("bytes.lit"),
                        expr_ident(brand_id.to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(brand_v_var.clone()),
                    expr_list(vec![
                        expr_ident("bytes.view"),
                        expr_ident(brand_b_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(brand_len_var.clone()),
                    expr_list(vec![
                        expr_ident("view.len"),
                        expr_ident(brand_v_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(validator_b_var.clone()),
                    expr_list(vec![
                        expr_ident("bytes.lit"),
                        expr_ident(validator_id.to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(validator_v_var.clone()),
                    expr_list(vec![
                        expr_ident("bytes.view"),
                        expr_ident(validator_b_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(validator_len_var.clone()),
                    expr_list(vec![
                        expr_ident("view.len"),
                        expr_ident(validator_v_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(pl_var.clone()),
                    expr_list(vec![
                        expr_ident("vec_u8.with_capacity"),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_int(16),
                            expr_list(vec![
                                expr_ident("+"),
                                expr_ident(brand_len_var.clone()),
                                expr_ident(validator_len_var.clone()),
                            ]),
                        ]),
                    ]),
                ]),
                extend_u32(&pl_var, expr_ident(brand_len_var.clone())),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident(pl_var.clone()),
                    expr_list(vec![
                        expr_ident("vec_u8.extend_bytes"),
                        expr_ident(pl_var.clone()),
                        expr_ident(brand_v_var.clone()),
                    ]),
                ]),
                extend_u32(&pl_var, expr_ident(validator_len_var.clone())),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident(pl_var.clone()),
                    expr_list(vec![
                        expr_ident("vec_u8.extend_bytes"),
                        expr_ident(pl_var.clone()),
                        expr_ident(validator_v_var.clone()),
                    ]),
                ]),
                extend_u32(&pl_var, expr_ident(ec_var.clone())),
                extend_u32(&pl_var, expr_ident(item_idx_var)),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_BRAND_VALIDATE_FAILED),
                        "stream:brand_validate_failed",
                        expr_list(vec![expr_ident("vec_u8.into_bytes"), expr_ident(pl_var)]),
                    ),
                ]),
            ])
        };

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![expr_ident("result_i32.is_ok"), expr_ident(r_var)]),
            ok_branch,
            err_branch,
        ]));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_map_in_place_buf(&self, stage_idx: usize, item: Expr) -> Result<Expr, CompilerError> {
        let s = self
            .map_in_place_states
            .iter()
            .find(|s| s.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing map_in_place state".to_string(),
                )
            })?;

        let scratch = expr_ident(s.scratch_var.clone());

        let mut stmts = Vec::new();
        if s.clear_before_each != 0 {
            stmts.push(expr_list(vec![
                expr_ident("set"),
                scratch.clone(),
                expr_list(vec![
                    expr_ident("scratch_u8_fixed_v1.clear"),
                    scratch.clone(),
                ]),
            ]));
        }

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("r".to_string()),
            expr_list(vec![expr_ident(s.fn_id.clone()), item, scratch.clone()]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("result_i32.is_ok"),
                expr_ident("r".to_string()),
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("out".to_string()),
                    expr_list(vec![
                        expr_ident("scratch_u8_fixed_v1.as_view"),
                        scratch.clone(),
                    ]),
                ]),
                self.gen_process_from(
                    stage_idx + 1,
                    self.apply_out_item_brand(stage_idx, expr_ident("out".to_string())),
                )?,
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("ec".to_string()),
                    expr_list(vec![
                        expr_ident("result_i32.err_code"),
                        expr_ident("r".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_ident("ec".to_string()),
                        expr_int(E_SCRATCH_OVERFLOW),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_SCRATCH_OVERFLOW, "stream:scratch_overflow"),
                    ]),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(16)]),
                ]),
                extend_u32("pl", expr_int(i32::try_from(stage_idx).unwrap_or(i32::MAX))),
                extend_u32("pl", expr_ident(self.bytes_in_var.clone())),
                extend_u32("pl", expr_ident(self.items_in_var.clone())),
                extend_u32("pl", expr_ident("ec".to_string())),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_STAGE_FAILED),
                        "stream:stage_failed",
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident("pl".to_string()),
                        ]),
                    ),
                ]),
            ]),
        ]));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_split_lines(&self, stage_idx: usize, chunk: Expr) -> Result<Expr, CompilerError> {
        self.gen_plugin_step(stage_idx, chunk)
    }

    fn gen_deframe_u32le(&self, stage_idx: usize, chunk: Expr) -> Result<Expr, CompilerError> {
        self.gen_plugin_step(stage_idx, chunk)
    }

    fn gen_json_canon_stream_process(
        &self,
        stage_idx: usize,
        item: Expr,
    ) -> Result<Expr, CompilerError> {
        self.gen_plugin_step(stage_idx, item)
    }

    fn gen_json_canon_stream_flush(&self, stage_idx: usize) -> Result<Expr, CompilerError> {
        self.gen_plugin_flush(stage_idx)
    }

    fn par_map_state(&self, stage_idx: usize) -> Result<&ParMapState, CompilerError> {
        self.par_map_states
            .iter()
            .find(|s| s.stage_idx == stage_idx)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: missing par_map_stream state".to_string(),
                )
            })
    }

    fn gen_par_map_ordered_emit_one(
        &self,
        stage_idx: usize,
        p: &ParMapState,
    ) -> Result<Expr, CompilerError> {
        let head_var = p.head_var.as_ref().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: ordered par_map missing head var".to_string(),
            )
        })?;

        let slot_id_var = format!("pm_slot_id_{stage_idx}");
        let off_var = format!("pm_off_{stage_idx}");
        let bad_idx_var = format!("pm_idx_{stage_idx}");
        let out_b_var = format!("pm_out_b_{stage_idx}");
        let out_v_var = format!("pm_out_v_{stage_idx}");

        let mut stmts = Vec::new();

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(bad_idx_var.clone()),
            expr_list(vec![
                expr_ident("-"),
                expr_ident(p.next_index_var.clone()),
                expr_ident(p.len_var.clone()),
            ]),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(off_var.clone()),
            expr_list(vec![
                expr_ident("*"),
                expr_ident(head_var.clone()),
                expr_int(4),
            ]),
        ]));

        if let Some(inflight_bytes) = &p.inflight_bytes_var {
            let lens = p.lens_var.as_ref().expect("lens var for inflight bytes");
            let in_len_var = format!("pm_in_len_{stage_idx}");
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(in_len_var.clone()),
                vec_u8_read_u32_le(lens, expr_ident(off_var.clone())),
            ]));
            stmts.push(expr_list(vec![
                expr_ident("set"),
                expr_ident(inflight_bytes.clone()),
                expr_list(vec![
                    expr_ident("-"),
                    expr_ident(inflight_bytes.clone()),
                    expr_ident(in_len_var),
                ]),
            ]));
        }

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(slot_id_var.clone()),
            expr_list(vec![
                expr_ident("task.scope.slot_from_i32_v1"),
                vec_u8_read_u32_le(&p.slots_var, expr_ident(off_var.clone())),
            ]),
        ]));

        if p.cfg.result_bytes {
            let rb_var = format!("pm_rb_{stage_idx}");
            let ec_var = format!("pm_ec_{stage_idx}");
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(rb_var.clone()),
                expr_list(vec![
                    expr_ident("task.scope.await_slot_result_bytes_v1"),
                    expr_ident(slot_id_var.clone()),
                ]),
            ]));
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_list(vec![
                        expr_ident("result_bytes.is_ok"),
                        expr_ident(rb_var.clone()),
                    ]),
                    expr_int(1),
                ]),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(out_b_var.clone()),
                        expr_list(vec![
                            expr_ident("__internal.result_bytes.unwrap_ok_v1"),
                            expr_ident(rb_var.clone()),
                        ]),
                    ]),
                    self.par_map_emit_one_downstream(stage_idx, p, &out_b_var, &out_v_var)?,
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(ec_var.clone()),
                        expr_list(vec![
                            expr_ident("result_bytes.err_code"),
                            expr_ident(rb_var),
                        ]),
                    ]),
                    expr_list(vec![expr_ident("task.scope.cancel_all_v1")]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_with_payload(
                            expr_list(vec![
                                expr_ident("if"),
                                expr_list(vec![
                                    expr_ident("="),
                                    expr_ident(ec_var.clone()),
                                    expr_int(2),
                                ]),
                                expr_int(E_PARMAP_CHILD_CANCELED),
                                expr_int(E_PARMAP_CHILD_ERR),
                            ]),
                            "stream:par_map_child_err",
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("pl".to_string()),
                                    expr_list(vec![
                                        expr_ident("vec_u8.with_capacity"),
                                        expr_int(8),
                                    ]),
                                ]),
                                extend_u32("pl", expr_ident(ec_var)),
                                extend_u32("pl", expr_ident(bad_idx_var)),
                                expr_list(vec![
                                    expr_ident("vec_u8.into_bytes"),
                                    expr_ident("pl".to_string()),
                                ]),
                            ]),
                        ),
                    ]),
                ]),
            ]));
        } else {
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(out_b_var.clone()),
                expr_list(vec![
                    expr_ident("task.scope.await_slot_bytes_v1"),
                    expr_ident(slot_id_var.clone()),
                ]),
            ]));
            stmts.push(self.par_map_emit_one_downstream(stage_idx, p, &out_b_var, &out_v_var)?);
        }

        stmts.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(head_var.clone()),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident(head_var.clone()),
                        expr_int(1),
                    ]),
                    expr_int(p.cfg.max_inflight),
                ]),
                expr_int(0),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident(head_var.clone()),
                    expr_int(1),
                ]),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(p.len_var.clone()),
            expr_list(vec![
                expr_ident("-"),
                expr_ident(p.len_var.clone()),
                expr_int(1),
            ]),
        ]));
        stmts.push(expr_int(0));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_par_map_unordered_emit_one(
        &self,
        stage_idx: usize,
        p: &ParMapState,
    ) -> Result<Expr, CompilerError> {
        let found_var = format!("pm_found_{stage_idx}");
        let poll_var = format!("pm_poll_{stage_idx}");
        let scan_i = format!("pm_scan_{stage_idx}");
        let old_len_var = format!("pm_old_len_{stage_idx}");
        let slot_off_var = format!("pm_slot_off_{stage_idx}");
        let slot_id_var = format!("pm_slot_id_{stage_idx}");
        let last_idx_var = format!("pm_last_idx_{stage_idx}");
        let last_off_var = format!("pm_last_off_{stage_idx}");
        let sel_len_var = format!("pm_sel_len_{stage_idx}");
        let out_b_var = format!("pm_out_b_{stage_idx}");
        let out_v_var = format!("pm_out_v_{stage_idx}");

        let poll_end = i32::MAX;
        let sentinel = expr_int(-1);

        let cancel_and_return = |doc: Expr| {
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![expr_ident("task.scope.cancel_all_v1")]),
                expr_list(vec![expr_ident("return"), doc]),
            ])
        };

        let mut scan_body: Vec<Expr> = Vec::new();
        scan_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(slot_off_var.clone()),
            expr_list(vec![
                expr_ident("*"),
                expr_ident(scan_i.clone()),
                expr_int(4),
            ]),
        ]));
        scan_body.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(slot_id_var.clone()),
            expr_list(vec![
                expr_ident("task.scope.slot_from_i32_v1"),
                vec_u8_read_u32_le(&p.slots_var, expr_ident(slot_off_var.clone())),
            ]),
        ]));

        let remove_slot = {
            let mut r: Vec<Expr> = Vec::new();
            r.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(last_idx_var.clone()),
                expr_list(vec![
                    expr_ident("-"),
                    expr_ident(old_len_var.clone()),
                    expr_int(1),
                ]),
            ]));

            if let Some(inflight) = &p.inflight_bytes_var {
                let lens = p.lens_var.as_ref().expect("lens var");
                r.push(expr_list(vec![
                    expr_ident("let"),
                    expr_ident(sel_len_var.clone()),
                    vec_u8_read_u32_le(lens, expr_ident(slot_off_var.clone())),
                ]));
                r.push(expr_list(vec![
                    expr_ident("set"),
                    expr_ident(inflight.clone()),
                    expr_list(vec![
                        expr_ident("-"),
                        expr_ident(inflight.clone()),
                        expr_ident(sel_len_var.clone()),
                    ]),
                ]));
            }

            let mut remove_last = Vec::new();
            remove_last.push(vec_u8_set_u32_le(
                &p.slots_var,
                expr_ident(slot_off_var.clone()),
                sentinel.clone(),
            ));
            if let Some(lens) = &p.lens_var {
                remove_last.push(vec_u8_set_u32_le(
                    lens,
                    expr_ident(slot_off_var.clone()),
                    expr_int(0),
                ));
            }
            if let Some(idxs) = &p.idxs_var {
                remove_last.push(vec_u8_set_u32_le(
                    idxs,
                    expr_ident(slot_off_var.clone()),
                    expr_int(0),
                ));
            }
            remove_last.push(expr_list(vec![
                expr_ident("set"),
                expr_ident(p.len_var.clone()),
                expr_ident(last_idx_var.clone()),
            ]));
            remove_last.push(expr_int(0));

            let mut remove_swap = vec![expr_list(vec![
                expr_ident("let"),
                expr_ident(last_off_var.clone()),
                expr_list(vec![
                    expr_ident("*"),
                    expr_ident(last_idx_var.clone()),
                    expr_int(4),
                ]),
            ])];
            remove_swap.push(expr_list(vec![
                expr_ident("let"),
                expr_ident("pm_last_slot".to_string()),
                vec_u8_read_u32_le(&p.slots_var, expr_ident(last_off_var.clone())),
            ]));
            remove_swap.push(vec_u8_set_u32_le(
                &p.slots_var,
                expr_ident(slot_off_var.clone()),
                expr_ident("pm_last_slot".to_string()),
            ));
            remove_swap.push(vec_u8_set_u32_le(
                &p.slots_var,
                expr_ident(last_off_var.clone()),
                sentinel.clone(),
            ));
            if let Some(lens) = &p.lens_var {
                remove_swap.push(expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pm_last_len".to_string()),
                    vec_u8_read_u32_le(lens, expr_ident(last_off_var.clone())),
                ]));
                remove_swap.push(vec_u8_set_u32_le(
                    lens,
                    expr_ident(slot_off_var.clone()),
                    expr_ident("pm_last_len".to_string()),
                ));
                remove_swap.push(vec_u8_set_u32_le(
                    lens,
                    expr_ident(last_off_var.clone()),
                    expr_int(0),
                ));
            }
            if let Some(idxs) = &p.idxs_var {
                remove_swap.push(expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pm_last_idx".to_string()),
                    vec_u8_read_u32_le(idxs, expr_ident(last_off_var.clone())),
                ]));
                remove_swap.push(vec_u8_set_u32_le(
                    idxs,
                    expr_ident(slot_off_var.clone()),
                    expr_ident("pm_last_idx".to_string()),
                ));
                remove_swap.push(vec_u8_set_u32_le(
                    idxs,
                    expr_ident(last_off_var),
                    expr_int(0),
                ));
            }
            remove_swap.push(expr_list(vec![
                expr_ident("set"),
                expr_ident(p.len_var.clone()),
                expr_ident(last_idx_var.clone()),
            ]));
            remove_swap.push(expr_int(0));

            r.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident(scan_i.clone()),
                    expr_ident(last_idx_var.clone()),
                ]),
                expr_list(
                    vec![expr_ident("begin")]
                        .into_iter()
                        .chain(remove_last)
                        .collect(),
                ),
                expr_list(
                    vec![expr_ident("begin")]
                        .into_iter()
                        .chain(remove_swap)
                        .collect(),
                ),
            ]));
            r.push(expr_int(0));

            expr_list(vec![expr_ident("begin")].into_iter().chain(r).collect())
        };

        let take_and_emit = if p.cfg.result_bytes {
            let idxs = p.idxs_var.as_ref().expect("idxs var");
            let r_var = format!("pm_try_r_{stage_idx}");
            let inner_var = format!("pm_inner_{stage_idx}");
            let idx_val_var = format!("pm_idx_val_{stage_idx}");
            let ec_var = format!("pm_ec_{stage_idx}");

            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(r_var.clone()),
                    expr_list(vec![
                        expr_ident("task.scope.try_await_slot.result_bytes_v1"),
                        expr_ident(slot_id_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_list(vec![
                            expr_ident("result_result_bytes.is_ok"),
                            expr_ident(r_var.clone()),
                        ]),
                        expr_int(1),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(inner_var.clone()),
                            expr_list(vec![
                                expr_ident("result_result_bytes.unwrap_or"),
                                expr_ident(r_var.clone()),
                                expr_list(vec![expr_ident("result_bytes.err"), expr_int(0)]),
                            ]),
                        ]),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(idx_val_var.clone()),
                            vec_u8_read_u32_le(idxs, expr_ident(slot_off_var.clone())),
                        ]),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_list(vec![
                                    expr_ident("result_bytes.is_ok"),
                                    expr_ident(inner_var.clone()),
                                ]),
                                expr_int(1),
                            ]),
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident(out_b_var.clone()),
                                    expr_list(vec![
                                        expr_ident("__internal.result_bytes.unwrap_ok_v1"),
                                        expr_ident(inner_var.clone()),
                                    ]),
                                ]),
                                remove_slot,
                                self.par_map_emit_one_downstream(
                                    stage_idx, p, &out_b_var, &out_v_var,
                                )?,
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident(found_var.clone()),
                                    expr_int(1),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident(scan_i.clone()),
                                    expr_ident(old_len_var.clone()),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident(poll_var.clone()),
                                    expr_int(poll_end),
                                ]),
                                expr_int(0),
                            ]),
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident(ec_var.clone()),
                                    expr_list(vec![
                                        expr_ident("result_bytes.err_code"),
                                        expr_ident(inner_var),
                                    ]),
                                ]),
                                expr_list(vec![expr_ident("task.scope.cancel_all_v1")]),
                                expr_list(vec![
                                    expr_ident("return"),
                                    err_doc_with_payload(
                                        expr_list(vec![
                                            expr_ident("if"),
                                            expr_list(vec![
                                                expr_ident("="),
                                                expr_ident(ec_var.clone()),
                                                expr_int(2),
                                            ]),
                                            expr_int(E_PARMAP_CHILD_CANCELED),
                                            expr_int(E_PARMAP_CHILD_ERR),
                                        ]),
                                        "stream:par_map_child_err",
                                        expr_list(vec![
                                            expr_ident("begin"),
                                            expr_list(vec![
                                                expr_ident("let"),
                                                expr_ident("pl".to_string()),
                                                expr_list(vec![
                                                    expr_ident("vec_u8.with_capacity"),
                                                    expr_int(8),
                                                ]),
                                            ]),
                                            extend_u32("pl", expr_ident(ec_var.clone())),
                                            extend_u32("pl", expr_ident(idx_val_var.clone())),
                                            expr_list(vec![
                                                expr_ident("vec_u8.into_bytes"),
                                                expr_ident("pl".to_string()),
                                            ]),
                                        ]),
                                    ),
                                ]),
                            ]),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(ec_var.clone()),
                            expr_list(vec![
                                expr_ident("result_result_bytes.err_code"),
                                expr_ident(r_var),
                            ]),
                        ]),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_ident(ec_var.clone()),
                                expr_int(2),
                            ]),
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident(idx_val_var.clone()),
                                    vec_u8_read_u32_le(idxs, expr_ident(slot_off_var.clone())),
                                ]),
                                expr_list(vec![expr_ident("task.scope.cancel_all_v1")]),
                                expr_list(vec![
                                    expr_ident("return"),
                                    err_doc_with_payload(
                                        expr_int(E_PARMAP_CHILD_CANCELED),
                                        "stream:par_map_child_canceled",
                                        expr_list(vec![
                                            expr_ident("begin"),
                                            expr_list(vec![
                                                expr_ident("let"),
                                                expr_ident("pl".to_string()),
                                                expr_list(vec![
                                                    expr_ident("vec_u8.with_capacity"),
                                                    expr_int(8),
                                                ]),
                                            ]),
                                            extend_u32("pl", expr_int(2)),
                                            extend_u32("pl", expr_ident(idx_val_var)),
                                            expr_list(vec![
                                                expr_ident("vec_u8.into_bytes"),
                                                expr_ident("pl".to_string()),
                                            ]),
                                        ]),
                                    ),
                                ]),
                            ]),
                            expr_int(0),
                        ]),
                        expr_int(0),
                    ]),
                ]),
                expr_int(0),
            ])
        } else {
            let r_var = format!("pm_try_r_{stage_idx}");
            let ec_var = format!("pm_ec_{stage_idx}");
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(r_var.clone()),
                    expr_list(vec![
                        expr_ident("task.scope.try_await_slot.bytes_v1"),
                        expr_ident(slot_id_var.clone()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("="),
                        expr_list(vec![
                            expr_ident("result_bytes.is_ok"),
                            expr_ident(r_var.clone()),
                        ]),
                        expr_int(1),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(out_b_var.clone()),
                            expr_list(vec![
                                expr_ident("__internal.result_bytes.unwrap_ok_v1"),
                                expr_ident(r_var.clone()),
                            ]),
                        ]),
                        remove_slot,
                        self.par_map_emit_one_downstream(stage_idx, p, &out_b_var, &out_v_var)?,
                        expr_list(vec![
                            expr_ident("set"),
                            expr_ident(found_var.clone()),
                            expr_int(1),
                        ]),
                        expr_list(vec![
                            expr_ident("set"),
                            expr_ident(scan_i.clone()),
                            expr_ident(old_len_var.clone()),
                        ]),
                        expr_list(vec![
                            expr_ident("set"),
                            expr_ident(poll_var.clone()),
                            expr_int(poll_end),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("let"),
                            expr_ident(ec_var.clone()),
                            expr_list(vec![expr_ident("result_bytes.err_code"), expr_ident(r_var)]),
                        ]),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![expr_ident("="), expr_ident(ec_var), expr_int(2)]),
                            cancel_and_return(err_doc_const(
                                E_PARMAP_CHILD_CANCELED,
                                "stream:par_map_child_canceled",
                            )),
                            expr_int(0),
                        ]),
                        expr_int(0),
                    ]),
                ]),
                expr_int(0),
            ])
        };

        scan_body.push(take_and_emit);
        scan_body.push(expr_int(0));

        let poll_body = vec![
            expr_list(vec![
                expr_ident("let"),
                expr_ident(old_len_var.clone()),
                expr_ident(p.len_var.clone()),
            ]),
            expr_list(vec![
                expr_ident("for"),
                expr_ident(scan_i.clone()),
                expr_int(0),
                expr_ident(old_len_var.clone()),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("="),
                            expr_ident(found_var.clone()),
                            expr_int(0),
                        ]),
                        expr_list(
                            vec![expr_ident("begin")]
                                .into_iter()
                                .chain(scan_body)
                                .collect(),
                        ),
                        expr_int(0),
                    ]),
                    expr_int(0),
                ]),
            ]),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident(found_var.clone()),
                    expr_int(0),
                ]),
                expr_list(vec![expr_ident("task.sleep"), expr_int(1)]),
                expr_int(0),
            ]),
            expr_int(0),
        ];

        Ok(expr_list(vec![
            expr_ident("begin"),
            let_i32(&found_var, 0),
            expr_list(vec![
                expr_ident("for"),
                expr_ident(poll_var.clone()),
                expr_int(0),
                expr_int(poll_end),
                expr_list(
                    vec![expr_ident("begin")]
                        .into_iter()
                        .chain(poll_body)
                        .collect(),
                ),
            ]),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident(found_var.clone()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_CFG_INVALID, "stream:max_steps_exceeded"),
                ]),
                expr_int(0),
            ]),
            expr_int(0),
        ]))
    }

    fn par_map_emit_one_downstream(
        &self,
        stage_idx: usize,
        p: &ParMapState,
        out_b_var: &str,
        out_v_var: &str,
    ) -> Result<Expr, CompilerError> {
        let mut stmts = Vec::new();
        if p.cfg.max_out_item_bytes > 0 {
            let out_n = format!("pm_out_n_{stage_idx}");
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(out_n.clone()),
                expr_list(vec![
                    expr_ident("bytes.len"),
                    expr_ident(out_b_var.to_string()),
                ]),
            ]));
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<"),
                    expr_ident(out_n.clone()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![expr_ident("task.scope.cancel_all_v1")]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_PARMAP_OUT_TOO_LARGE, "stream:par_map_out_too_large"),
                    ]),
                ]),
                expr_int(0),
            ]));
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident(">u"),
                    expr_ident(out_n),
                    expr_int(p.cfg.max_out_item_bytes),
                ]),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![expr_ident("task.scope.cancel_all_v1")]),
                    expr_list(vec![
                        expr_ident("return"),
                        err_doc_const(E_PARMAP_OUT_TOO_LARGE, "stream:par_map_out_too_large"),
                    ]),
                ]),
                expr_int(0),
            ]));
        }
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(out_v_var.to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident(out_b_var.to_string()),
            ]),
        ]));
        stmts.push(self.gen_process_from(
            stage_idx + 1,
            self.apply_out_item_brand(stage_idx, expr_ident(out_v_var.to_string())),
        )?);
        stmts.push(expr_int(0));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_par_map_stream_process(
        &self,
        stage_idx: usize,
        item: Expr,
    ) -> Result<Expr, CompilerError> {
        let p = self.par_map_state(stage_idx)?;

        let item_n_var = format!("pm_item_n_{stage_idx}");
        let item_b_var = format!("pm_item_b_{stage_idx}");

        let cancel_and_return = |doc: Expr| {
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![expr_ident("task.scope.cancel_all_v1")]),
                expr_list(vec![expr_ident("return"), doc]),
            ])
        };

        let mut stmts = Vec::new();
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(item_n_var.clone()),
            expr_list(vec![expr_ident("view.len"), item.clone()]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<"),
                expr_ident(item_n_var.clone()),
                expr_int(0),
            ]),
            cancel_and_return(err_doc_const(
                E_PARMAP_ITEM_TOO_LARGE,
                "stream:par_map_item_too_large",
            )),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident(item_n_var.clone()),
                expr_int(p.cfg.max_item_bytes),
            ]),
            cancel_and_return(err_doc_const(
                E_PARMAP_ITEM_TOO_LARGE,
                "stream:par_map_item_too_large",
            )),
            expr_int(0),
        ]));
        if p.cfg.max_inflight_in_bytes > 0 {
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident(">u"),
                    expr_ident(item_n_var.clone()),
                    expr_int(p.cfg.max_inflight_in_bytes),
                ]),
                cancel_and_return(err_doc_const(
                    E_PARMAP_ITEM_TOO_LARGE,
                    "stream:par_map_item_too_large",
                )),
                expr_int(0),
            ]));
        }

        let drain_one = if p.cfg.unordered {
            self.gen_par_map_unordered_emit_one(stage_idx, p)?
        } else {
            self.gen_par_map_ordered_emit_one(stage_idx, p)?
        };

        let drain_i = format!("pm_drain_{stage_idx}");
        let bytes_backpressure = if p.cfg.max_inflight_in_bytes > 0 {
            let inflight_bytes = p.inflight_bytes_var.as_ref().expect("inflight bytes var");
            expr_list(vec![
                expr_ident(">u"),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident(inflight_bytes.clone()),
                    expr_ident(item_n_var.clone()),
                ]),
                expr_int(p.cfg.max_inflight_in_bytes),
            ])
        } else {
            expr_int(0)
        };

        let drain_cond = expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("="),
                expr_ident(p.len_var.clone()),
                expr_int(p.cfg.max_inflight),
            ]),
            expr_int(1),
            bytes_backpressure,
        ]);

        stmts.push(expr_list(vec![
            expr_ident("for"),
            expr_ident(drain_i),
            expr_int(0),
            expr_int(p.cfg.max_inflight),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![expr_ident("if"), drain_cond, drain_one, expr_int(0)]),
                expr_int(0),
            ]),
        ]));

        if p.cfg.max_inflight_in_bytes > 0 {
            let inflight_bytes = p.inflight_bytes_var.as_ref().expect("inflight bytes var");
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident(">u"),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident(inflight_bytes.clone()),
                        expr_ident(item_n_var.clone()),
                    ]),
                    expr_int(p.cfg.max_inflight_in_bytes),
                ]),
                cancel_and_return(err_doc_const(
                    E_PARMAP_ITEM_TOO_LARGE,
                    "stream:par_map_item_too_large",
                )),
                expr_int(0),
            ]));
        }

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(item_b_var.clone()),
            expr_list(vec![
                expr_ident("__internal.brand.view_to_bytes_preserve_brand_v1"),
                item,
            ]),
        ]));

        let slot_var = format!("pm_slot_{stage_idx}");
        let async_let_head = if p.cfg.result_bytes {
            "task.scope.async_let_result_bytes_v1"
        } else {
            "task.scope.async_let_bytes_v1"
        };
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(slot_var.clone()),
            expr_list(vec![
                expr_ident(async_let_head.to_string()),
                expr_list(vec![
                    expr_ident(p.cfg.mapper_defasync.clone()),
                    expr_ident(p.ctx_v_var.clone()),
                    expr_ident(item_b_var.clone()),
                ]),
            ]),
        ]));

        let slot_off_var = format!("pm_slot_off_{stage_idx}");
        if p.cfg.unordered {
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(slot_off_var.clone()),
                expr_list(vec![
                    expr_ident("*"),
                    expr_ident(p.len_var.clone()),
                    expr_int(4),
                ]),
            ]));
        } else {
            let head_var = p.head_var.as_ref().expect("head for ordered par_map");
            let sum_var = format!("pm_sum_{stage_idx}");
            let tail_var = format!("pm_tail_{stage_idx}");
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(sum_var.clone()),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident(head_var.clone()),
                    expr_ident(p.len_var.clone()),
                ]),
            ]));
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(tail_var.clone()),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<u"),
                        expr_ident(sum_var.clone()),
                        expr_int(p.cfg.max_inflight),
                    ]),
                    expr_ident(sum_var.clone()),
                    expr_list(vec![
                        expr_ident("-"),
                        expr_ident(sum_var),
                        expr_int(p.cfg.max_inflight),
                    ]),
                ]),
            ]));
            stmts.push(expr_list(vec![
                expr_ident("let"),
                expr_ident(slot_off_var.clone()),
                expr_list(vec![expr_ident("*"), expr_ident(tail_var), expr_int(4)]),
            ]));
        }

        stmts.push(vec_u8_set_u32_le(
            &p.slots_var,
            expr_ident(slot_off_var.clone()),
            expr_list(vec![
                expr_ident("task.scope.slot_to_i32_v1"),
                expr_ident(slot_var),
            ]),
        ));
        if let Some(lens) = &p.lens_var {
            let inflight_bytes = p.inflight_bytes_var.as_ref().expect("inflight bytes var");
            stmts.push(vec_u8_set_u32_le(
                lens,
                expr_ident(slot_off_var.clone()),
                expr_ident(item_n_var.clone()),
            ));
            stmts.push(expr_list(vec![
                expr_ident("set"),
                expr_ident(inflight_bytes.clone()),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident(inflight_bytes.clone()),
                    expr_ident(item_n_var.clone()),
                ]),
            ]));
        }
        if let Some(idxs) = &p.idxs_var {
            stmts.push(vec_u8_set_u32_le(
                idxs,
                expr_ident(slot_off_var.clone()),
                expr_ident(p.next_index_var.clone()),
            ));
        }

        stmts.push(set_add_i32(&p.len_var, expr_int(1)));
        stmts.push(set_add_i32(&p.next_index_var, expr_int(1)));
        stmts.push(expr_int(0));

        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_par_map_stream_flush(&self, stage_idx: usize) -> Result<Expr, CompilerError> {
        let p = self.par_map_state(stage_idx)?;

        let drain_one = if p.cfg.unordered {
            self.gen_par_map_unordered_emit_one(stage_idx, p)?
        } else {
            self.gen_par_map_ordered_emit_one(stage_idx, p)?
        };

        let drain_i = format!("pm_flush_{stage_idx}");
        let mut stmts = vec![expr_list(vec![
            expr_ident("for"),
            expr_ident(drain_i),
            expr_int(0),
            expr_int(p.cfg.max_inflight),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_ident(p.len_var.clone()),
                        expr_int(0),
                    ]),
                    drain_one.clone(),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
        ])];
        stmts.push(self.gen_flush_from(stage_idx + 1)?);
        stmts.push(expr_int(0));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_flush_from(&self, stage_idx: usize) -> Result<Expr, CompilerError> {
        if stage_idx >= self.chain.len() {
            return Ok(expr_int(0));
        }
        match &self.chain[stage_idx].kind {
            PipeXfV1::JsonCanonStreamV1 { .. } => self.gen_json_canon_stream_flush(stage_idx),
            PipeXfV1::ParMapStreamV1 { .. } => self.gen_par_map_stream_flush(stage_idx),
            PipeXfV1::PluginV1 { .. } => self.gen_plugin_flush(stage_idx),
            PipeXfV1::SplitLines { .. } | PipeXfV1::DeframeU32LeV1 { .. } => {
                self.gen_plugin_flush(stage_idx)
            }
            _ => self.gen_flush_from(stage_idx + 1),
        }
    }

    fn gen_net_sink_return_err(&self, doc: Expr) -> Expr {
        expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("="),
                    expr_ident("net_sink_owned".to_string()),
                    expr_int(1),
                ]),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("std.net.tcp.stream_close_v1"),
                        expr_ident("net_sink_h".to_string()),
                    ]),
                    expr_list(vec![
                        expr_ident("std.net.tcp.stream_drop_v1"),
                        expr_ident("net_sink_h".to_string()),
                    ]),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
            expr_list(vec![expr_ident("return"), doc]),
        ])
    }

    fn gen_net_sink_flush(&self, cfg: NetTcpWriteStreamHandleCfgV1) -> Expr {
        let buf = expr_ident("net_sink_buf".to_string());
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<="),
                expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                expr_int(0),
            ]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bv".to_string()),
                    expr_list(vec![expr_ident("vec_u8.as_view"), buf.clone()]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident("net_sink_flushes".to_string()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident("net_sink_flushes".to_string()),
                        expr_int(1),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_ident("net_sink_flushes".to_string()),
                        expr_ident("net_sink_max_flushes".to_string()),
                    ]),
                    self.gen_net_sink_return_err(err_doc_const(
                        E_NET_SINK_MAX_FLUSHES,
                        "stream:net_sink_max_flushes",
                    )),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pos".to_string()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("end".to_string()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident("bl".to_string()),
                        expr_int(1),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("for"),
                    expr_ident("_".to_string()),
                    expr_int(0),
                    expr_ident("end".to_string()),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">=u"),
                                expr_ident("pos".to_string()),
                                expr_ident("bl".to_string()),
                            ]),
                            expr_int(0),
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("remain".to_string()),
                                    expr_list(vec![
                                        expr_ident("-"),
                                        expr_ident("bl".to_string()),
                                        expr_ident("pos".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("seg_len".to_string()),
                                    expr_list(vec![
                                        expr_ident("if"),
                                        expr_list(vec![
                                            expr_ident("<u"),
                                            expr_ident("remain".to_string()),
                                            expr_ident("net_sink_mw".to_string()),
                                        ]),
                                        expr_ident("remain".to_string()),
                                        expr_ident("net_sink_mw".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("seg".to_string()),
                                    expr_list(vec![
                                        expr_ident("view.slice"),
                                        expr_ident("bv".to_string()),
                                        expr_ident("pos".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("doc".to_string()),
                                    expr_list(vec![
                                        expr_ident("std.net.io.write_all_v1"),
                                        expr_ident("net_sink_h".to_string()),
                                        expr_ident("seg".to_string()),
                                        expr_ident("net_sink_caps".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("dv".to_string()),
                                    expr_list(vec![
                                        expr_ident("bytes.view"),
                                        expr_ident("doc".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident("std.net.err.is_err_doc_v1"),
                                        expr_ident("dv".to_string()),
                                    ]),
                                    self.gen_net_sink_return_err(err_doc_with_payload(
                                        expr_int(E_NET_WRITE_FAILED),
                                        "stream:net_write_failed",
                                        expr_ident("doc".to_string()),
                                    )),
                                    expr_int(0),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident("net_sink_write_calls".to_string()),
                                    expr_list(vec![
                                        expr_ident("+"),
                                        expr_ident("net_sink_write_calls".to_string()),
                                        expr_int(1),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident(">u"),
                                        expr_ident("net_sink_write_calls".to_string()),
                                        expr_ident("net_sink_max_write_calls".to_string()),
                                    ]),
                                    self.gen_net_sink_return_err(err_doc_const(
                                        E_NET_SINK_MAX_WRITES,
                                        "stream:net_sink_max_writes",
                                    )),
                                    expr_int(0),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident("pos".to_string()),
                                    expr_list(vec![
                                        expr_ident("+"),
                                        expr_ident("pos".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_int(0),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<u"),
                        expr_ident("pos".to_string()),
                        expr_ident("bl".to_string()),
                    ]),
                    self.gen_net_sink_return_err(err_doc_const(
                        E_CFG_INVALID,
                        "stream:net_write_loop_overflow",
                    )),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident("net_sink_buf".to_string()),
                    expr_list(vec![
                        expr_ident("vec_u8.with_capacity"),
                        expr_int(cfg.buf_cap_bytes),
                    ]),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn gen_net_sink_write_direct(&self, data: Expr, data_len: Expr) -> Expr {
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![expr_ident("<="), data_len.clone(), expr_int(0)]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident("net_sink_flushes".to_string()),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_ident("net_sink_flushes".to_string()),
                        expr_int(1),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_ident("net_sink_flushes".to_string()),
                        expr_ident("net_sink_max_flushes".to_string()),
                    ]),
                    self.gen_net_sink_return_err(err_doc_const(
                        E_NET_SINK_MAX_FLUSHES,
                        "stream:net_sink_max_flushes",
                    )),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pos".to_string()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("end".to_string()),
                    expr_list(vec![expr_ident("+"), data_len.clone(), expr_int(1)]),
                ]),
                expr_list(vec![
                    expr_ident("for"),
                    expr_ident("_".to_string()),
                    expr_int(0),
                    expr_ident("end".to_string()),
                    expr_list(vec![
                        expr_ident("begin"),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">=u"),
                                expr_ident("pos".to_string()),
                                data_len.clone(),
                            ]),
                            expr_int(0),
                            expr_list(vec![
                                expr_ident("begin"),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("remain".to_string()),
                                    expr_list(vec![
                                        expr_ident("-"),
                                        data_len.clone(),
                                        expr_ident("pos".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("seg_len".to_string()),
                                    expr_list(vec![
                                        expr_ident("if"),
                                        expr_list(vec![
                                            expr_ident("<u"),
                                            expr_ident("remain".to_string()),
                                            expr_ident("net_sink_mw".to_string()),
                                        ]),
                                        expr_ident("remain".to_string()),
                                        expr_ident("net_sink_mw".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("seg".to_string()),
                                    expr_list(vec![
                                        expr_ident("view.slice"),
                                        data.clone(),
                                        expr_ident("pos".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("doc".to_string()),
                                    expr_list(vec![
                                        expr_ident("std.net.io.write_all_v1"),
                                        expr_ident("net_sink_h".to_string()),
                                        expr_ident("seg".to_string()),
                                        expr_ident("net_sink_caps".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("let"),
                                    expr_ident("dv".to_string()),
                                    expr_list(vec![
                                        expr_ident("bytes.view"),
                                        expr_ident("doc".to_string()),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident("std.net.err.is_err_doc_v1"),
                                        expr_ident("dv".to_string()),
                                    ]),
                                    self.gen_net_sink_return_err(err_doc_with_payload(
                                        expr_int(E_NET_WRITE_FAILED),
                                        "stream:net_write_failed",
                                        expr_ident("doc".to_string()),
                                    )),
                                    expr_int(0),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident("net_sink_write_calls".to_string()),
                                    expr_list(vec![
                                        expr_ident("+"),
                                        expr_ident("net_sink_write_calls".to_string()),
                                        expr_int(1),
                                    ]),
                                ]),
                                expr_list(vec![
                                    expr_ident("if"),
                                    expr_list(vec![
                                        expr_ident(">u"),
                                        expr_ident("net_sink_write_calls".to_string()),
                                        expr_ident("net_sink_max_write_calls".to_string()),
                                    ]),
                                    self.gen_net_sink_return_err(err_doc_const(
                                        E_NET_SINK_MAX_WRITES,
                                        "stream:net_sink_max_writes",
                                    )),
                                    expr_int(0),
                                ]),
                                expr_list(vec![
                                    expr_ident("set"),
                                    expr_ident("pos".to_string()),
                                    expr_list(vec![
                                        expr_ident("+"),
                                        expr_ident("pos".to_string()),
                                        expr_ident("seg_len".to_string()),
                                    ]),
                                ]),
                                expr_int(0),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident("<u"),
                        expr_ident("pos".to_string()),
                        data_len.clone(),
                    ]),
                    self.gen_net_sink_return_err(err_doc_const(
                        E_CFG_INVALID,
                        "stream:net_write_loop_overflow",
                    )),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn gen_net_sink_push(
        &self,
        data: Expr,
        data_len: Expr,
        cfg: NetTcpWriteStreamHandleCfgV1,
    ) -> Expr {
        let buf = expr_ident("net_sink_buf".to_string());
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![expr_ident("<="), data_len.clone(), expr_int(0)]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_ident("bl".to_string()),
                            data_len.clone(),
                        ]),
                        expr_int(cfg.buf_cap_bytes),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        self.gen_net_sink_flush(cfg),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">=u"),
                                data_len.clone(),
                                expr_int(cfg.buf_cap_bytes),
                            ]),
                            self.gen_net_sink_write_direct(data.clone(), data_len.clone()),
                            expr_list(vec![
                                expr_ident("set"),
                                buf.clone(),
                                expr_list(vec![
                                    expr_ident("vec_u8.extend_bytes_range"),
                                    buf.clone(),
                                    data.clone(),
                                    expr_int(0),
                                    data_len.clone(),
                                ]),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        buf.clone(),
                        expr_list(vec![
                            expr_ident("vec_u8.extend_bytes_range"),
                            buf.clone(),
                            data.clone(),
                            expr_int(0),
                            data_len.clone(),
                        ]),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">=u"),
                        expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                        expr_int(cfg.flush_min_bytes),
                    ]),
                    self.gen_net_sink_flush(cfg),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn gen_fs_sink_write_direct(&self, data: Expr, hash: bool) -> Expr {
        let mut stmts = vec![expr_list(vec![
            expr_ident("set"),
            expr_ident("fs_sink_flushes".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident("fs_sink_flushes".to_string()),
                expr_int(1),
            ]),
        ])];
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident("fs_sink_flushes".to_string()),
                expr_ident("fs_sink_max_flushes".to_string()),
            ]),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("os.fs.stream_close_v1"),
                    expr_ident("fs_sink_h".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("os.fs.stream_drop_v1"),
                    expr_ident("fs_sink_h".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(12)]),
                ]),
                extend_u32("pl", expr_int(0)),
                extend_u32("pl", expr_ident("fs_sink_flushes".to_string())),
                extend_u32("pl", expr_ident(self.bytes_out_var.clone())),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_SINK_TOO_MANY_FLUSHES),
                        "stream:fs_too_many_flushes",
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident("pl".to_string()),
                        ]),
                    ),
                ]),
            ]),
            expr_int(0),
        ]));

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("wr".to_string()),
            expr_list(vec![
                expr_ident("os.fs.stream_write_all_v1"),
                expr_ident("fs_sink_h".to_string()),
                data.clone(),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("result_i32.is_ok"),
                expr_ident("wr".to_string()),
            ]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("os.fs.stream_close_v1"),
                    expr_ident("fs_sink_h".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("os.fs.stream_drop_v1"),
                    expr_ident("fs_sink_h".to_string()),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("ec".to_string()),
                    expr_list(vec![
                        expr_ident("result_i32.err_code"),
                        expr_ident("wr".to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("pl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(12)]),
                ]),
                extend_u32("pl", expr_ident("ec".to_string())),
                extend_u32("pl", expr_ident("fs_sink_flushes".to_string())),
                extend_u32("pl", expr_ident(self.bytes_out_var.clone())),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_with_payload(
                        expr_int(E_SINK_FS_WRITE_FAILED),
                        "stream:fs_write_failed",
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident("pl".to_string()),
                        ]),
                    ),
                ]),
            ]),
        ]));

        if hash {
            let hash_name = self.hash_var.as_ref().expect("hash state");
            stmts.push(self.gen_fnv1a_update(data, hash_name));
        }

        stmts.push(expr_int(0));
        expr_list(vec![expr_ident("begin")].into_iter().chain(stmts).collect())
    }

    fn gen_fs_sink_flush(&self, hash: bool) -> Expr {
        let buf = expr_ident("fs_sink_buf".to_string());
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident("<="),
                expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                expr_int(0),
            ]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bv".to_string()),
                    expr_list(vec![expr_ident("vec_u8.as_view"), buf.clone()]),
                ]),
                self.gen_fs_sink_write_direct(expr_ident("bv".to_string()), hash),
                expr_list(vec![
                    expr_ident("set"),
                    expr_ident("fs_sink_buf".to_string()),
                    expr_list(vec![
                        expr_ident("vec_u8.clear"),
                        expr_ident("fs_sink_buf".to_string()),
                    ]),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn gen_fs_sink_push(
        &self,
        data: Expr,
        data_len: Expr,
        cfg: WorldFsWriteStreamCfgV1,
        hash: bool,
    ) -> Expr {
        let buf = expr_ident("fs_sink_buf".to_string());
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![expr_ident("<="), data_len.clone(), expr_int(0)]),
            expr_int(0),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("bl".to_string()),
                    expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">u"),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_ident("bl".to_string()),
                            data_len.clone(),
                        ]),
                        expr_int(cfg.buf_cap_bytes),
                    ]),
                    expr_list(vec![
                        expr_ident("begin"),
                        self.gen_fs_sink_flush(hash),
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident(">=u"),
                                data_len.clone(),
                                expr_int(cfg.buf_cap_bytes),
                            ]),
                            self.gen_fs_sink_write_direct(data.clone(), hash),
                            expr_list(vec![
                                expr_ident("set"),
                                buf.clone(),
                                expr_list(vec![
                                    expr_ident("vec_u8.extend_bytes_range"),
                                    buf.clone(),
                                    data.clone(),
                                    expr_int(0),
                                    data_len.clone(),
                                ]),
                            ]),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        buf.clone(),
                        expr_list(vec![
                            expr_ident("vec_u8.extend_bytes_range"),
                            buf.clone(),
                            data.clone(),
                            expr_int(0),
                            data_len.clone(),
                        ]),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    expr_list(vec![
                        expr_ident(">=u"),
                        expr_list(vec![expr_ident("vec_u8.len"), buf.clone()]),
                        expr_int(cfg.flush_min_bytes),
                    ]),
                    self.gen_fs_sink_flush(hash),
                    expr_int(0),
                ]),
                expr_int(0),
            ]),
        ])
    }

    fn emit_item(&self, item: Expr) -> Result<Expr, CompilerError> {
        let len_var = "emit_len".to_string();
        let mut stmts = Vec::new();
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(len_var.clone()),
            expr_list(vec![expr_ident("view.len"), item.clone()]),
        ]));
        if self.sink.framing_u32frames {
            stmts.push(expr_list(vec![
                expr_ident("if"),
                expr_list(vec![
                    expr_ident("<"),
                    expr_ident(len_var.clone()),
                    expr_int(0),
                ]),
                expr_list(vec![
                    expr_ident("return"),
                    err_doc_const(E_FRAME_TOO_LARGE, "stream:frame_too_large"),
                ]),
                expr_int(0),
            ]));
        }

        // items_out += 1 (budgeted)
        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("new_items_out".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident(self.items_out_var.clone()),
                expr_int(1),
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident("new_items_out".to_string()),
                expr_int(self.cfg.max_items),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BUDGET_ITEMS, "stream:budget_items"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(self.items_out_var.clone()),
            expr_ident("new_items_out".to_string()),
        ]));

        let bytes_inc = if self.sink.framing_u32frames {
            expr_list(vec![
                expr_ident("+"),
                expr_int(4),
                expr_ident(len_var.clone()),
            ])
        } else {
            expr_ident(len_var.clone())
        };

        stmts.push(expr_list(vec![
            expr_ident("let"),
            expr_ident("new_bytes_out".to_string()),
            expr_list(vec![
                expr_ident("+"),
                expr_ident(self.bytes_out_var.clone()),
                bytes_inc,
            ]),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">u"),
                expr_ident("new_bytes_out".to_string()),
                expr_int(self.cfg.max_out_bytes),
            ]),
            expr_list(vec![
                expr_ident("return"),
                err_doc_const(E_BUDGET_OUT_BYTES, "stream:budget_out_bytes"),
            ]),
            expr_int(0),
        ]));
        stmts.push(expr_list(vec![
            expr_ident("set"),
            expr_ident(self.bytes_out_var.clone()),
            expr_ident("new_bytes_out".to_string()),
        ]));

        // Feed to sink.
        match &self.sink.base {
            SinkBaseV1::CollectBytes | SinkBaseV1::WorldFsWriteFile { .. } => {
                let vec_name = self.sink_vec_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing sink_vec".to_string(),
                    )
                })?;
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(expr_list(vec![
                        expr_ident("set"),
                        expr_ident(vec_name.clone()),
                        expr_list(vec![
                            expr_ident("vec_u8.extend_bytes"),
                            expr_ident(vec_name.clone()),
                            expr_ident("hdr".to_string()),
                        ]),
                    ]));
                }
                stmts.push(expr_list(vec![
                    expr_ident("set"),
                    expr_ident(vec_name.clone()),
                    expr_list(vec![
                        expr_ident("vec_u8.extend_bytes_range"),
                        expr_ident(vec_name.clone()),
                        item,
                        expr_int(0),
                        expr_ident(len_var),
                    ]),
                ]));
            }
            SinkBaseV1::HashFnv1a32 => {
                let hash_name = self.hash_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing hash state".to_string(),
                    )
                })?;
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(self.gen_fnv1a_update(
                        expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident("hdr".to_string()),
                        ]),
                        hash_name,
                    ));
                }
                stmts.push(self.gen_fnv1a_update(item, hash_name));
            }
            SinkBaseV1::Null => {}
            SinkBaseV1::WorldFsWriteStream { cfg, .. } => {
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(self.gen_fs_sink_push(
                        expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident("hdr".to_string()),
                        ]),
                        expr_int(4),
                        *cfg,
                        false,
                    ));
                }
                stmts.push(self.gen_fs_sink_push(item, expr_ident(len_var.clone()), *cfg, false));
            }
            SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { cfg, .. } => {
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(self.gen_fs_sink_push(
                        expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident("hdr".to_string()),
                        ]),
                        expr_int(4),
                        *cfg,
                        true,
                    ));
                }
                stmts.push(self.gen_fs_sink_push(item, expr_ident(len_var.clone()), *cfg, true));
            }
            SinkBaseV1::NetTcpWriteStreamHandle { cfg, .. }
            | SinkBaseV1::NetTcpConnectWrite { cfg, .. } => {
                if self.sink.framing_u32frames {
                    stmts.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident("hdr".to_string()),
                        expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(len_var.clone()),
                        ]),
                    ]));
                    stmts.push(self.gen_net_sink_push(
                        expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident("hdr".to_string()),
                        ]),
                        expr_int(4),
                        *cfg,
                    ));
                }
                stmts.push(self.gen_net_sink_push(item, expr_ident(len_var.clone()), *cfg));
            }
        }

        stmts.push(expr_int(0));
        Ok(expr_list(
            vec![expr_ident("begin")].into_iter().chain(stmts).collect(),
        ))
    }

    fn gen_fnv1a_update(&self, data: Expr, hash_name: &str) -> Expr {
        expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("n".to_string()),
                expr_list(vec![expr_ident("view.len"), data.clone()]),
            ]),
            expr_list(vec![
                expr_ident("for"),
                expr_ident("i".to_string()),
                expr_int(0),
                expr_ident("n".to_string()),
                expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident("c".to_string()),
                        expr_list(vec![
                            expr_ident("view.get_u8"),
                            data.clone(),
                            expr_ident("i".to_string()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        expr_ident(hash_name.to_string()),
                        expr_list(vec![
                            expr_ident("^"),
                            expr_ident(hash_name.to_string()),
                            expr_ident("c".to_string()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("set"),
                        expr_ident(hash_name.to_string()),
                        expr_list(vec![
                            expr_ident("*"),
                            expr_ident(hash_name.to_string()),
                            expr_int(FNV1A32_PRIME),
                        ]),
                    ]),
                    expr_int(0),
                ]),
            ]),
            expr_int(0),
        ])
    }

    fn gen_return_ok(&self) -> Result<Expr, CompilerError> {
        let payload_expr = match &self.sink.base {
            SinkBaseV1::CollectBytes => {
                let vec_name = self.sink_vec_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing sink_vec".to_string(),
                    )
                })?;
                expr_list(vec![
                    expr_ident("vec_u8.into_bytes"),
                    expr_ident(vec_name.clone()),
                ])
            }
            SinkBaseV1::HashFnv1a32 => {
                let hash_name = self.hash_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing hash state".to_string(),
                    )
                })?;
                expr_list(vec![
                    expr_ident("codec.write_u32_le"),
                    expr_ident(hash_name.clone()),
                ])
            }
            SinkBaseV1::Null => expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
            SinkBaseV1::WorldFsWriteFile { path_param } => {
                let vec_name = self.sink_vec_var.as_ref().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing sink_vec".to_string(),
                    )
                })?;
                let file_bytes = "file_bytes".to_string();
                let rc = "rc".to_string();
                return Ok(expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(file_bytes.clone()),
                        expr_list(vec![
                            expr_ident("vec_u8.into_bytes"),
                            expr_ident(vec_name.clone()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(rc.clone()),
                        expr_list(vec![
                            expr_ident("os.fs.write_file"),
                            param_ident(*path_param),
                            expr_ident(file_bytes),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![expr_ident("!="), expr_ident(rc.clone()), expr_int(0)]),
                        expr_list(vec![
                            expr_ident("return"),
                            err_doc_with_payload(
                                expr_int(E_SINK_FS_WRITE_FAILED),
                                "stream:fs_write_failed",
                                expr_list(vec![expr_ident("codec.write_u32_le"), expr_ident(rc)]),
                            ),
                        ]),
                        expr_int(0),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        ok_doc(
                            self,
                            expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
                        ),
                    ]),
                ]));
            }
            SinkBaseV1::WorldFsWriteStream { .. }
            | SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { .. } => {
                let hash_payload = match &self.sink.base {
                    SinkBaseV1::WorldFsWriteStreamHashFnv1a32 { .. } => {
                        let hash_name = self.hash_var.as_ref().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Internal,
                                "internal error: missing hash state".to_string(),
                            )
                        })?;
                        Some(expr_list(vec![
                            expr_ident("codec.write_u32_le"),
                            expr_ident(hash_name.clone()),
                        ]))
                    }
                    _ => None,
                };

                return Ok(expr_list(vec![
                    expr_ident("begin"),
                    self.gen_fs_sink_flush(hash_payload.is_some()),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident("cl".to_string()),
                        expr_list(vec![
                            expr_ident("os.fs.stream_close_v1"),
                            expr_ident("fs_sink_h".to_string()),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("os.fs.stream_drop_v1"),
                        expr_ident("fs_sink_h".to_string()),
                    ]),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("result_i32.is_ok"),
                            expr_ident("cl".to_string()),
                        ]),
                        expr_int(0),
                        expr_list(vec![
                            expr_ident("begin"),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident("ec".to_string()),
                                expr_list(vec![
                                    expr_ident("result_i32.err_code"),
                                    expr_ident("cl".to_string()),
                                ]),
                            ]),
                            expr_list(vec![
                                expr_ident("let"),
                                expr_ident("pl".to_string()),
                                expr_list(vec![expr_ident("vec_u8.with_capacity"), expr_int(12)]),
                            ]),
                            extend_u32("pl", expr_ident("ec".to_string())),
                            extend_u32("pl", expr_ident("fs_sink_flushes".to_string())),
                            extend_u32("pl", expr_ident(self.bytes_out_var.clone())),
                            expr_list(vec![
                                expr_ident("return"),
                                err_doc_with_payload(
                                    expr_int(E_SINK_FS_CLOSE_FAILED),
                                    "stream:fs_close_failed",
                                    expr_list(vec![
                                        expr_ident("vec_u8.into_bytes"),
                                        expr_ident("pl".to_string()),
                                    ]),
                                ),
                            ]),
                        ]),
                    ]),
                    expr_list(vec![
                        expr_ident("return"),
                        ok_doc(
                            self,
                            hash_payload.unwrap_or_else(|| {
                                expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)])
                            }),
                        ),
                    ]),
                ]));
            }
            SinkBaseV1::NetTcpWriteStreamHandle { cfg, .. }
            | SinkBaseV1::NetTcpConnectWrite { cfg, .. } => {
                let shutdown_write = expr_list(vec![
                    expr_ident("std.net.tcp.stream_shutdown_v1"),
                    expr_ident("net_sink_h".to_string()),
                    expr_list(vec![expr_ident("std.net.tcp.shutdown_write_v1")]),
                ]);
                let close_drop = expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("std.net.tcp.stream_close_v1"),
                        expr_ident("net_sink_h".to_string()),
                    ]),
                    expr_list(vec![
                        expr_ident("std.net.tcp.stream_drop_v1"),
                        expr_ident("net_sink_h".to_string()),
                    ]),
                    expr_int(0),
                ]);

                let finish = match cfg.on_finish {
                    NetSinkOnFinishV1::LeaveOpen => expr_int(0),
                    NetSinkOnFinishV1::ShutdownWrite => expr_list(vec![
                        expr_ident("begin"),
                        shutdown_write,
                        expr_list(vec![
                            expr_ident("if"),
                            expr_list(vec![
                                expr_ident("="),
                                expr_ident("net_sink_owned".to_string()),
                                expr_int(1),
                            ]),
                            close_drop.clone(),
                            expr_int(0),
                        ]),
                        expr_int(0),
                    ]),
                    NetSinkOnFinishV1::Close => close_drop,
                };

                return Ok(expr_list(vec![
                    expr_ident("begin"),
                    self.gen_net_sink_flush(*cfg),
                    finish,
                    expr_list(vec![
                        expr_ident("return"),
                        ok_doc(
                            self,
                            expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
                        ),
                    ]),
                ]));
            }
        };

        Ok(expr_list(vec![
            expr_ident("return"),
            ok_doc(self, payload_expr),
        ]))
    }
}

fn ok_doc(cg: &PipeCodegen<'_>, payload: Expr) -> Expr {
    let payload = if cg.emit_payload {
        payload
    } else {
        expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)])
    };

    let bytes_in = if cg.emit_stats {
        expr_ident(cg.bytes_in_var.clone())
    } else {
        expr_int(0)
    };
    let bytes_out = if cg.emit_stats {
        expr_ident(cg.bytes_out_var.clone())
    } else {
        expr_int(0)
    };
    let items_in = if cg.emit_stats {
        expr_ident(cg.items_in_var.clone())
    } else {
        expr_int(0)
    };
    let items_out = if cg.emit_stats {
        expr_ident(cg.items_out_var.clone())
    } else {
        expr_int(0)
    };

    expr_list(vec![
        expr_ident("begin"),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("plb".to_string()),
            payload,
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("plv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("plb".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("pl_len".to_string()),
            expr_list(vec![expr_ident("view.len"), expr_ident("plv".to_string())]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_list(vec![
                    expr_ident("+"),
                    expr_int(21),
                    expr_ident("pl_len".to_string()),
                ]),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.push"),
                expr_ident("out".to_string()),
                expr_int(1),
            ]),
        ]),
        extend_u32("out", bytes_in),
        extend_u32("out", bytes_out),
        extend_u32("out", items_in),
        extend_u32("out", items_out),
        extend_u32("out", expr_ident("pl_len".to_string())),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes"),
                expr_ident("out".to_string()),
                expr_ident("plv".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("vec_u8.into_bytes"),
            expr_ident("out".to_string()),
        ]),
    ])
}

fn err_doc_const(code: i32, msg: &str) -> Expr {
    err_doc_with_payload(
        expr_int(code),
        msg,
        expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]),
    )
}

fn err_doc_with_payload(code: Expr, msg: &str, payload: Expr) -> Expr {
    expr_list(vec![
        expr_ident("begin"),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("msg".to_string()),
            expr_list(vec![expr_ident("bytes.lit"), expr_ident(msg.to_string())]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("msgv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("msg".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("msg_len".to_string()),
            expr_list(vec![expr_ident("view.len"), expr_ident("msgv".to_string())]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("plb".to_string()),
            payload,
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("plv".to_string()),
            expr_list(vec![
                expr_ident("bytes.view"),
                expr_ident("plb".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("pl_len".to_string()),
            expr_list(vec![expr_ident("view.len"), expr_ident("plv".to_string())]),
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.with_capacity"),
                expr_list(vec![
                    expr_ident("+"),
                    expr_list(vec![
                        expr_ident("+"),
                        expr_int(13),
                        expr_ident("msg_len".to_string()),
                    ]),
                    expr_ident("pl_len".to_string()),
                ]),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.push"),
                expr_ident("out".to_string()),
                expr_int(0),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes"),
                expr_ident("out".to_string()),
                expr_list(vec![expr_ident("codec.write_u32_le"), code]),
            ]),
        ]),
        extend_u32("out", expr_ident("msg_len".to_string())),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes"),
                expr_ident("out".to_string()),
                expr_ident("msgv".to_string()),
            ]),
        ]),
        extend_u32("out", expr_ident("pl_len".to_string())),
        expr_list(vec![
            expr_ident("set"),
            expr_ident("out".to_string()),
            expr_list(vec![
                expr_ident("vec_u8.extend_bytes"),
                expr_ident("out".to_string()),
                expr_ident("plv".to_string()),
            ]),
        ]),
        expr_list(vec![
            expr_ident("vec_u8.into_bytes"),
            expr_ident("out".to_string()),
        ]),
    ])
}

fn extend_u32(out: &str, x: Expr) -> Expr {
    expr_list(vec![
        expr_ident("set"),
        expr_ident(out.to_string()),
        expr_list(vec![
            expr_ident("vec_u8.extend_bytes"),
            expr_ident(out.to_string()),
            expr_list(vec![expr_ident("codec.write_u32_le"), x]),
        ]),
    ])
}

fn vec_u8_read_u32_le(vec_name: &str, off: Expr) -> Expr {
    expr_list(vec![
        expr_ident("codec.read_u32_le"),
        expr_list(vec![
            expr_ident("vec_u8.as_view"),
            expr_ident(vec_name.to_string()),
        ]),
        off,
    ])
}

fn vec_u8_set_u32_le(vec_name: &str, off: Expr, value: Expr) -> Expr {
    expr_list(vec![
        expr_ident("begin"),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("u32le_off".to_string()),
            off,
        ]),
        expr_list(vec![
            expr_ident("let"),
            expr_ident("u32le".to_string()),
            expr_list(vec![expr_ident("codec.write_u32_le"), value]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident(vec_name.to_string()),
            expr_list(vec![
                expr_ident("vec_u8.set"),
                expr_ident(vec_name.to_string()),
                expr_ident("u32le_off".to_string()),
                expr_list(vec![
                    expr_ident("bytes.get_u8"),
                    expr_ident("u32le".to_string()),
                    expr_int(0),
                ]),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident(vec_name.to_string()),
            expr_list(vec![
                expr_ident("vec_u8.set"),
                expr_ident(vec_name.to_string()),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident("u32le_off".to_string()),
                    expr_int(1),
                ]),
                expr_list(vec![
                    expr_ident("bytes.get_u8"),
                    expr_ident("u32le".to_string()),
                    expr_int(1),
                ]),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident(vec_name.to_string()),
            expr_list(vec![
                expr_ident("vec_u8.set"),
                expr_ident(vec_name.to_string()),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident("u32le_off".to_string()),
                    expr_int(2),
                ]),
                expr_list(vec![
                    expr_ident("bytes.get_u8"),
                    expr_ident("u32le".to_string()),
                    expr_int(2),
                ]),
            ]),
        ]),
        expr_list(vec![
            expr_ident("set"),
            expr_ident(vec_name.to_string()),
            expr_list(vec![
                expr_ident("vec_u8.set"),
                expr_ident(vec_name.to_string()),
                expr_list(vec![
                    expr_ident("+"),
                    expr_ident("u32le_off".to_string()),
                    expr_int(3),
                ]),
                expr_list(vec![
                    expr_ident("bytes.get_u8"),
                    expr_ident("u32le".to_string()),
                    expr_int(3),
                ]),
            ]),
        ]),
        expr_int(0),
    ])
}

fn param_ident(idx: usize) -> Expr {
    expr_ident(format!("p{idx}"))
}

fn let_i32(name: &str, value: i32) -> Expr {
    expr_list(vec![
        expr_ident("let"),
        expr_ident(name.to_string()),
        expr_int(value),
    ])
}

fn set_add_i32(name: &str, delta: Expr) -> Expr {
    expr_list(vec![
        expr_ident("set"),
        expr_ident(name.to_string()),
        expr_list(vec![expr_ident("+"), expr_ident(name.to_string()), delta]),
    ])
}

fn emit_net_sink_validate_caps(items: &mut Vec<Expr>) {
    // Minimal caps validation (strict): len>=24, ver==1, reserved==0, max_write_bytes>0.
    items.push(expr_list(vec![
        expr_ident("if"),
        expr_list(vec![
            expr_ident("<"),
            expr_list(vec![
                expr_ident("view.len"),
                expr_ident("net_sink_caps".to_string()),
            ]),
            expr_int(24),
        ]),
        expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
        ]),
        expr_int(0),
    ]));
    items.push(expr_list(vec![
        expr_ident("if"),
        expr_list(vec![
            expr_ident("!="),
            expr_list(vec![
                expr_ident("codec.read_u32_le"),
                expr_ident("net_sink_caps".to_string()),
                expr_int(0),
            ]),
            expr_int(1),
        ]),
        expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
        ]),
        expr_int(0),
    ]));
    items.push(expr_list(vec![
        expr_ident("if"),
        expr_list(vec![
            expr_ident("!="),
            expr_list(vec![
                expr_ident("codec.read_u32_le"),
                expr_ident("net_sink_caps".to_string()),
                expr_int(20),
            ]),
            expr_int(0),
        ]),
        expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
        ]),
        expr_int(0),
    ]));
    items.push(expr_list(vec![
        expr_ident("let"),
        expr_ident("net_sink_mw".to_string()),
        expr_list(vec![
            expr_ident("codec.read_u32_le"),
            expr_ident("net_sink_caps".to_string()),
            expr_int(16),
        ]),
    ]));
    items.push(expr_list(vec![
        expr_ident("if"),
        expr_list(vec![
            expr_ident("<="),
            expr_ident("net_sink_mw".to_string()),
            expr_int(0),
        ]),
        expr_list(vec![
            expr_ident("return"),
            err_doc_const(E_NET_CAPS_INVALID, "stream:net_caps_invalid"),
        ]),
        expr_int(0),
    ]));
}

fn emit_net_sink_init_limits_and_buffer(
    items: &mut Vec<Expr>,
    pipe_max_out_bytes: i32,
    cfg: NetTcpWriteStreamHandleCfgV1,
) {
    items.push(let_i32("net_sink_flushes", 0));
    items.push(let_i32("net_sink_write_calls", 0));

    // Derive max_flushes/max_write_calls when 0.
    let b = pipe_max_out_bytes;
    let f = cfg.flush_min_bytes.max(1);
    let derived_max_flushes = if cfg.max_flushes > 0 {
        cfg.max_flushes
    } else {
        ((b + f - 1) / f) + 4
    };
    items.push(let_i32("net_sink_max_flushes", derived_max_flushes.max(1)));
    items.push(expr_list(vec![
        expr_ident("let"),
        expr_ident("net_sink_max_write_calls".to_string()),
        expr_list(vec![
            expr_ident("if"),
            expr_list(vec![
                expr_ident(">"),
                expr_int(cfg.max_write_calls),
                expr_int(0),
            ]),
            expr_int(cfg.max_write_calls),
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("d".to_string()),
                    expr_list(vec![
                        expr_ident("if"),
                        expr_list(vec![
                            expr_ident("<u"),
                            expr_ident("net_sink_mw".to_string()),
                            expr_int(f),
                        ]),
                        expr_ident("net_sink_mw".to_string()),
                        expr_int(f),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("+"),
                    expr_list(vec![
                        expr_ident("/"),
                        expr_list(vec![
                            expr_ident("+"),
                            expr_int(b),
                            expr_list(vec![
                                expr_ident("-"),
                                expr_ident("d".to_string()),
                                expr_int(1),
                            ]),
                        ]),
                        expr_ident("d".to_string()),
                    ]),
                    expr_int(8),
                ]),
            ]),
        ]),
    ]));

    items.push(expr_list(vec![
        expr_ident("let"),
        expr_ident("net_sink_buf".to_string()),
        expr_list(vec![
            expr_ident("vec_u8.with_capacity"),
            expr_int(cfg.buf_cap_bytes),
        ]),
    ]));
}

fn parse_expr_v1(
    params: &mut Vec<PipeParam>,
    ty: Ty,
    wrapper: &Expr,
) -> Result<usize, CompilerError> {
    let Expr::List { items, .. } = wrapper else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.expr_v1 wrapper must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("std.stream.expr_v1") || items.len() != 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "expected std.stream.expr_v1 wrapper".to_string(),
        ));
    }
    let expr = items[1].clone();
    let idx = params.len();
    params.push(PipeParam { ty, expr });
    Ok(idx)
}

fn parse_fn_v1(wrapper: &Expr) -> Result<String, CompilerError> {
    let Expr::List { items, .. } = wrapper else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.fn_v1 wrapper must be a list".to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("std.stream.fn_v1") || items.len() != 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "expected std.stream.fn_v1 wrapper".to_string(),
        ));
    }
    let Some(fn_id) = items[1].as_ident() else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            "std.stream.fn_v1 expects a function identifier".to_string(),
        ));
    };
    Ok(fn_id.to_string())
}

fn parse_kv_fields(head: &str, tail: &[Expr]) -> Result<BTreeMap<String, Expr>, CompilerError> {
    let mut out = BTreeMap::new();
    for item in tail {
        let Expr::List { items: kv, .. } = item else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} fields must be pairs"),
            ));
        };
        if kv.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} fields must be pairs"),
            ));
        }
        let Some(key) = kv[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} field key must be an identifier"),
            ));
        };
        if out.insert(key.to_string(), kv[1].clone()).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} has duplicate field: {key}"),
            ));
        }
    }
    Ok(out)
}

fn hash_pipe_without_expr_bodies(pipe_expr: &Expr) -> Result<String, CompilerError> {
    let mut v = x07ast::expr_to_value(pipe_expr);
    scrub_expr_bodies(&mut v);
    canon_stream_descriptor_pairs(&mut v);
    x07ast::canon_value_jcs(&mut v);
    let bytes = serde_json::to_vec(&v).map_err(|e| {
        CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: failed to serialize pipe descriptor: {e}"),
        )
    })?;

    let digest = blake3::hash(&bytes);
    let b = digest.as_bytes();
    Ok(format!("{:02x}{:02x}{:02x}{:02x}", b[0], b[1], b[2], b[3]))
}

fn scrub_expr_bodies(v: &mut Value) {
    match v {
        Value::Array(items) => {
            if items.len() == 2 && items[0].as_str() == Some("std.stream.expr_v1") {
                items[1] = Value::Null;
                return;
            }
            for item in items {
                scrub_expr_bodies(item);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                scrub_expr_bodies(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn canon_stream_descriptor_pairs(v: &mut Value) {
    match v {
        Value::Array(items) => {
            if let Some(head) = items.first().and_then(Value::as_str) {
                match head {
                    "std.stream.cfg_v1"
                    | "task.scope.cfg_v1"
                    | "std.stream.sink.net_tcp_connect_write_v1"
                    | "std.stream.sink.net_tcp_write_u32frames_v1"
                    | "std.stream.sink.net_tcp_write_stream_handle_v1"
                    | "std.stream.src.net_tcp_read_stream_handle_v1"
                    | "std.stream.xf.deframe_u32le_v1"
                    | "std.stream.xf.json_canon_stream_v1"
                    | "std.stream.xf.plugin_v1"
                    | "std.stream.xf.require_brand_v1"
                    | "std.stream.xf.par_map_stream_v1"
                    | "std.stream.xf.par_map_stream_result_bytes_v1"
                    | "std.stream.xf.par_map_stream_unordered_v1"
                    | "std.stream.xf.par_map_stream_unordered_result_bytes_v1" => {
                        canon_pair_tail(items, 1);
                    }
                    "std.stream.xf.map_in_place_buf_v1" => canon_map_in_place_buf(items),
                    "std.stream.sink.world_fs_write_stream_v1"
                    | "std.stream.sink.world_fs_write_stream_hash_fnv1a32_v1" => {
                        canon_pair_tail(items, 3);
                    }
                    _ => {}
                }
            }

            for item in items {
                canon_stream_descriptor_pairs(item);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                canon_stream_descriptor_pairs(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn canon_map_in_place_buf(items: &mut Vec<Value>) {
    // Canonicalize v1.1 positional mix:
    //   [head, ["scratch_cap_bytes",...], ["std.stream.fn_v1",...], ["clear_before_each",...]?]
    // Also accepts the keyed form: ["fn", ["std.stream.fn_v1",...]].
    if items.is_empty() {
        return;
    }
    let mut scratch: Option<Value> = None;
    let mut f: Option<Value> = None;
    let mut clear: Option<Value> = None;
    let mut rest: Vec<Value> = Vec::new();

    for v in items.iter().skip(1) {
        let Some(arr) = v.as_array() else {
            rest.push(v.clone());
            continue;
        };
        match arr.first().and_then(Value::as_str) {
            Some("scratch_cap_bytes") if arr.len() == 2 => {
                if scratch.is_none() {
                    scratch = Some(v.clone());
                } else {
                    rest.push(v.clone());
                }
            }
            Some("clear_before_each") if arr.len() == 2 => {
                if clear.is_none() {
                    clear = Some(v.clone());
                } else {
                    rest.push(v.clone());
                }
            }
            Some("fn") if arr.len() == 2 => {
                if f.is_none() {
                    f = Some(arr[1].clone());
                } else {
                    rest.push(v.clone());
                }
            }
            Some("std.stream.fn_v1") => {
                if f.is_none() {
                    f = Some(v.clone());
                } else {
                    rest.push(v.clone());
                }
            }
            _ => rest.push(v.clone()),
        }
    }

    let mut out: Vec<Value> = Vec::new();
    out.push(items[0].clone());
    if let Some(v) = scratch {
        out.push(v);
    }
    if let Some(v) = f {
        out.push(v);
    }
    if let Some(v) = clear {
        out.push(v);
    }
    out.extend(rest);
    *items = out;
}

fn canon_pair_tail(items: &mut Vec<Value>, start: usize) {
    if items.len() <= start {
        return;
    }
    let mut head: Vec<Value> = items[..start].to_vec();
    let mut tail: Vec<Value> = items[start..].to_vec();
    tail.sort_by(|a, b| {
        let ak = a
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        let bk = b
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        ak.as_bytes().cmp(bk.as_bytes())
    });
    head.extend(tail);
    *items = head;
}

fn function_module_id(full_name: &str) -> Result<&str, CompilerError> {
    let Some((module_id, _name)) = full_name.rsplit_once('.') else {
        return Err(CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: function name missing module prefix: {full_name:?}"),
        ));
    };
    Ok(module_id)
}

fn expect_i32(expr: &Expr, message: &str) -> Result<i32, CompilerError> {
    match expr {
        Expr::Int { value, .. } => Ok(*value),
        _ => Err(CompilerError::new(
            CompileErrorKind::Typing,
            message.to_string(),
        )),
    }
}

fn expect_bytes_lit_text(expr: &Expr, message: &str) -> Result<String, CompilerError> {
    let Expr::List { items, .. } = expr else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            message.to_string(),
        ));
    };
    if items.first().and_then(Expr::as_ident) != Some("bytes.lit") || items.len() != 2 {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            message.to_string(),
        ));
    }
    let Some(text) = items[1].as_ident() else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            message.to_string(),
        ));
    };
    Ok(text.to_string())
}

fn parse_brand_id(expr: &Expr, label: &str) -> Result<String, CompilerError> {
    let Some(brand) = expr.as_ident() else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{label} must be an identifier"),
        ));
    };
    validate::validate_symbol(brand)
        .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
    Ok(brand.to_string())
}

fn parse_item_brand_in(expr: &Expr, label: &str) -> Result<PipeItemBrandInV1, CompilerError> {
    let Some(v) = expr.as_ident() else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{label} must be an identifier"),
        ));
    };
    match v {
        "any" => Ok(PipeItemBrandInV1::Any),
        "same" => Ok(PipeItemBrandInV1::Same),
        other => {
            validate::validate_symbol(other)
                .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
            Ok(PipeItemBrandInV1::Brand(other.to_string()))
        }
    }
}

fn parse_item_brand_out(expr: &Expr, label: &str) -> Result<PipeItemBrandOutV1, CompilerError> {
    let Some(v) = expr.as_ident() else {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{label} must be an identifier"),
        ));
    };
    match v {
        "same" => Ok(PipeItemBrandOutV1::Same),
        "none" => Ok(PipeItemBrandOutV1::None),
        other => {
            validate::validate_symbol(other)
                .map_err(|message| CompilerError::new(CompileErrorKind::Typing, message))?;
            Ok(PipeItemBrandOutV1::Brand(other.to_string()))
        }
    }
}

fn parse_xf_item_brand_fields(
    head: &str,
    fields: &BTreeMap<String, Expr>,
) -> Result<(Option<PipeItemBrandInV1>, Option<PipeItemBrandOutV1>), CompilerError> {
    for k in fields.keys() {
        match k.as_str() {
            "in_item_brand" | "out_item_brand" => {}
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("{head} unknown field: {k}"),
                ));
            }
        }
    }
    let in_item_brand = fields
        .get("in_item_brand")
        .map(|v| parse_item_brand_in(v, "in_item_brand"))
        .transpose()?;
    let out_item_brand = fields
        .get("out_item_brand")
        .map(|v| parse_item_brand_out(v, "out_item_brand"))
        .transpose()?;
    Ok((in_item_brand, out_item_brand))
}

fn parse_sink_item_brand_fields(
    head: &str,
    fields: &BTreeMap<String, Expr>,
) -> Result<Option<PipeItemBrandInV1>, CompilerError> {
    for k in fields.keys() {
        if k != "in_item_brand" {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} unknown field: {k}"),
            ));
        }
    }
    let in_item_brand = fields
        .get("in_item_brand")
        .map(|v| parse_item_brand_in(v, "in_item_brand"))
        .transpose()?;
    if matches!(in_item_brand, Some(PipeItemBrandInV1::Same)) {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!("{head} in_item_brand must be \"any\" or a brand id"),
        ));
    }
    Ok(in_item_brand)
}

fn expr_ident(name: impl Into<String>) -> Expr {
    Expr::Ident {
        name: name.into(),
        ptr: String::new(),
    }
}

fn expr_int(value: i32) -> Expr {
    Expr::Int {
        value,
        ptr: String::new(),
    }
}

fn expr_list(items: Vec<Expr>) -> Expr {
    Expr::List {
        items,
        ptr: String::new(),
    }
}
