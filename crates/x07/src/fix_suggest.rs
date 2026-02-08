use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use x07_contracts::{X07AST_SCHEMA_VERSION_V0_4_0, X07_PATCHSET_SCHEMA_VERSION};
use x07c::ast::Expr;
use x07c::diagnostics;
use x07c::x07ast::{self, AstFunctionDef, TypeParam, TypeRef, X07AstKind};

const SUPPORTED_TAGS: [&str; 4] = ["bytes", "bytes_view", "i32", "u32"];
const TYPE_PARAM_NAME: &str = "A";

#[derive(Debug, Clone, Serialize)]
struct PatchTargetOut {
    path: String,
    patch: Vec<diagnostics::PatchOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PatchSetOut {
    schema_version: &'static str,
    patches: Vec<PatchTargetOut>,
}

#[derive(Debug, Clone)]
struct Variant {
    idx: usize,
    tag: String,
}

pub(crate) fn suggest_generics_patchset(input_path: &Path, bytes: &[u8]) -> Result<Value> {
    let mut file = x07ast::parse_x07ast_json(bytes).map_err(|e| anyhow::anyhow!("{e}"))?;

    let original_version = file.schema_version.clone();
    let mut any_change = false;
    let mut suggested_bases: Vec<String> = Vec::new();

    let mut all_names: BTreeSet<String> = BTreeSet::new();
    for f in &file.functions {
        all_names.insert(f.name.clone());
    }
    for f in &file.async_functions {
        all_names.insert(f.name.clone());
    }
    for f in &file.extern_functions {
        all_names.insert(f.name.clone());
    }

    let mut groups: BTreeMap<String, Vec<Variant>> = BTreeMap::new();
    for (idx, f) in file.functions.iter().enumerate() {
        if !f.type_params.is_empty() {
            continue;
        }
        let Some((base, tag)) = infer_base_and_tag(&f.name) else {
            continue;
        };
        groups.entry(base).or_default().push(Variant { idx, tag });
    }

    let mut processed_idxs: BTreeSet<usize> = BTreeSet::new();

    for (base, mut variants) in groups {
        if variants.len() < 2 {
            continue;
        }
        variants.sort_by(|a, b| a.tag.cmp(&b.tag).then_with(|| a.idx.cmp(&b.idx)));
        variants.dedup_by(|a, b| a.tag == b.tag);

        if variants.len() < 2 {
            continue;
        }

        if all_names.contains(&base) {
            continue;
        }

        if variants.iter().any(|v| processed_idxs.contains(&v.idx)) {
            continue;
        }

        let Some(reference) = variants.first() else {
            continue;
        };
        let ref_def = file
            .functions
            .get(reference.idx)
            .cloned()
            .context("internal: missing reference function")?;

        if !def_tag_uses_supported(&ref_def, &reference.tag) {
            continue;
        }

        let ref_norm = normalize_def(&ref_def, &reference.tag);
        let mut ok = true;

        for v in &variants[1..] {
            let Some(def) = file.functions.get(v.idx) else {
                ok = false;
                break;
            };
            if !def_tag_uses_supported(def, &v.tag) {
                ok = false;
                break;
            }
            let norm = normalize_def(def, &v.tag);
            if norm != ref_norm {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }

        let base_def = build_generic_base(&ref_def, &base, &reference.tag)?;

        for v in &variants {
            let Some(orig) = file.functions.get(v.idx) else {
                ok = false;
                break;
            };
            let wrapper = build_wrapper(orig, &base, &v.tag);
            file.functions[v.idx] = wrapper;
            processed_idxs.insert(v.idx);
        }

        if !ok {
            continue;
        }

        if file.kind == X07AstKind::Module
            && !file.exports.is_empty()
            && variants
                .iter()
                .filter_map(|v| file.functions.get(v.idx))
                .any(|f| file.exports.contains(&f.name))
        {
            file.exports.insert(base.clone());
        }

        file.functions.push(base_def);
        all_names.insert(base.clone());
        suggested_bases.push(base);
        any_change = true;
    }

    if any_change {
        file.schema_version = X07AST_SCHEMA_VERSION_V0_4_0.to_string();
    } else {
        file.schema_version = original_version;
    }

    x07ast::canonicalize_x07ast_file(&mut file);
    let mut new_doc = x07ast::x07ast_file_to_value(&file);
    x07ast::canon_value_jcs(&mut new_doc);

    let patches = if any_change {
        let note = if suggested_bases.is_empty() {
            None
        } else {
            suggested_bases.sort();
            suggested_bases.dedup();
            Some(format!(
                "suggested generic bases: {}",
                suggested_bases.join(", ")
            ))
        };
        vec![PatchTargetOut {
            path: input_path.display().to_string(),
            patch: vec![diagnostics::PatchOp::Replace {
                path: "".to_string(),
                value: new_doc,
            }],
            note,
        }]
    } else {
        Vec::new()
    };

    let patchset = PatchSetOut {
        schema_version: X07_PATCHSET_SCHEMA_VERSION,
        patches,
    };

    Ok(serde_json::to_value(patchset)?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedDef {
    params: Vec<(String, Option<String>, TypeRef)>,
    result: TypeRef,
    result_brand: Option<String>,
    body: Expr,
}

fn normalize_def(def: &AstFunctionDef, tag: &str) -> NormalizedDef {
    let params = def
        .params
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                p.brand.clone(),
                normalize_type_ref(&p.ty, tag),
            )
        })
        .collect();
    NormalizedDef {
        params,
        result: normalize_type_ref(&def.result, tag),
        result_brand: def.result_brand.clone(),
        body: normalize_expr(&def.body, tag),
    }
}

fn normalize_type_ref(tr: &TypeRef, tag: &str) -> TypeRef {
    match tr {
        TypeRef::Named(s) => {
            if s == tag {
                TypeRef::Var("__x07_fix_type".to_string())
            } else {
                TypeRef::Named(s.clone())
            }
        }
        TypeRef::Var(v) => TypeRef::Var(v.clone()),
        TypeRef::App { head, args } => TypeRef::App {
            head: head.clone(),
            args: args.iter().map(|a| normalize_type_ref(a, tag)).collect(),
        },
    }
}

fn normalize_expr(expr: &Expr, tag: &str) -> Expr {
    match expr {
        Expr::Int { value, ptr } => Expr::Int {
            value: *value,
            ptr: ptr.clone(),
        },
        Expr::Ident { name, ptr } => {
            if name == tag {
                Expr::Ident {
                    name: "__TYPE__".to_string(),
                    ptr: ptr.clone(),
                }
            } else {
                Expr::Ident {
                    name: name.clone(),
                    ptr: ptr.clone(),
                }
            }
        }
        Expr::List { items, ptr } => Expr::List {
            items: items.iter().map(|e| normalize_expr(e, tag)).collect(),
            ptr: ptr.clone(),
        },
    }
}

fn infer_base_and_tag(sym: &str) -> Option<(String, String)> {
    let mut found: Option<(String, String)> = None;
    for tag in SUPPORTED_TAGS {
        if let Some(base) = strip_symbol_tag(sym, tag) {
            if found.is_some() {
                return None;
            }
            found = Some((base, tag.to_string()));
        }
    }
    found
}

fn strip_symbol_tag(sym: &str, tag: &str) -> Option<String> {
    let suffix = format!("_{tag}");
    let mut parts: Vec<&str> = sym.split('.').collect();
    let mut changed = 0usize;
    for part in parts.iter_mut() {
        if part.ends_with(&suffix) {
            let new_len = part.len().saturating_sub(suffix.len());
            let stripped = &part[..new_len];
            if stripped.is_empty() {
                return None;
            }
            *part = stripped;
            changed = changed.saturating_add(1);
        }
    }
    (changed == 1).then(|| parts.join("."))
}

fn def_tag_uses_supported(def: &AstFunctionDef, tag: &str) -> bool {
    for p in &def.params {
        if !type_ref_uses_supported(&p.ty) {
            return false;
        }
    }
    if !type_ref_uses_supported(&def.result) {
        return false;
    }
    expr_tag_uses_supported(&def.body, tag, false)
}

fn type_ref_uses_supported(tr: &TypeRef) -> bool {
    match tr {
        TypeRef::Named(_) => true,
        TypeRef::Var(_) => false,
        TypeRef::App { head: _, args } => args.iter().all(type_ref_uses_supported),
    }
}

fn expr_tag_uses_supported(expr: &Expr, tag: &str, in_type_pos: bool) -> bool {
    match expr {
        Expr::Int { .. } => true,
        Expr::Ident { name, .. } => name != tag || in_type_pos,
        Expr::List { items, .. } => {
            let Some(head) = items.first() else {
                return true;
            };
            let head_name = head.as_ident().unwrap_or("");
            if head_name == "tapp" {
                if items.len() >= 3 {
                    if let Expr::List { items: tys, .. } = &items[2] {
                        if tys.first().and_then(Expr::as_ident) == Some("tys") {
                            let mut ok = expr_tag_uses_supported(&items[0], tag, false);
                            ok &= expr_tag_uses_supported(&items[1], tag, false);
                            ok &= expr_tag_uses_supported(&tys[0], tag, false);
                            for item in tys.iter().skip(1) {
                                ok &= expr_tag_uses_supported(item, tag, true);
                            }
                            for item in items.iter().skip(3) {
                                ok &= expr_tag_uses_supported(item, tag, false);
                            }
                            return ok;
                        }
                    }
                }

                let mut ok = true;
                let mut tapp_type_prefix = true;
                for (idx, item) in items.iter().enumerate() {
                    if idx < 2 {
                        ok &= expr_tag_uses_supported(item, tag, false);
                        continue;
                    }
                    let is_type_arg = if tapp_type_prefix {
                        match item.as_ident() {
                            Some(s) if SUPPORTED_TAGS.contains(&s) => true,
                            _ => {
                                tapp_type_prefix = false;
                                false
                            }
                        }
                    } else {
                        false
                    };
                    ok &= expr_tag_uses_supported(item, tag, is_type_arg);
                }
                return ok;
            }
            if head_name.starts_with("ty.") {
                if items.len() < 2 {
                    return false;
                }
                if !items[1]
                    .as_ident()
                    .is_some_and(|s| SUPPORTED_TAGS.contains(&s))
                {
                    return false;
                }
                let mut ok = expr_tag_uses_supported(&items[0], tag, false);
                ok &= expr_tag_uses_supported(&items[1], tag, true);
                for item in &items[2..] {
                    ok &= expr_tag_uses_supported(item, tag, false);
                }
                return ok;
            }
            items.iter().all(|e| expr_tag_uses_supported(e, tag, false))
        }
    }
}

fn build_generic_base(def: &AstFunctionDef, base: &str, tag: &str) -> Result<AstFunctionDef> {
    let type_param = TypeParam {
        name: TYPE_PARAM_NAME.to_string(),
        bound: None,
    };
    let mut out = def.clone();
    out.name = base.to_string();
    out.type_params = vec![type_param];
    for p in &mut out.params {
        p.ty = replace_type_named_with_var(&p.ty, tag, TYPE_PARAM_NAME);
    }
    out.result = replace_type_named_with_var(&out.result, tag, TYPE_PARAM_NAME);
    out.body = rewrite_expr_tag_to_typevar(&out.body, tag, TYPE_PARAM_NAME)?;
    Ok(out)
}

fn replace_type_named_with_var(tr: &TypeRef, tag: &str, var: &str) -> TypeRef {
    match tr {
        TypeRef::Named(s) => {
            if s == tag {
                TypeRef::Var(var.to_string())
            } else {
                TypeRef::Named(s.clone())
            }
        }
        TypeRef::Var(v) => TypeRef::Var(v.clone()),
        TypeRef::App { head, args } => TypeRef::App {
            head: head.clone(),
            args: args
                .iter()
                .map(|a| replace_type_named_with_var(a, tag, var))
                .collect(),
        },
    }
}

fn type_var_expr(name: &str, ptr: &str) -> Expr {
    Expr::List {
        items: vec![
            Expr::Ident {
                name: "t".to_string(),
                ptr: ptr.to_string(),
            },
            Expr::Ident {
                name: name.to_string(),
                ptr: ptr.to_string(),
            },
        ],
        ptr: ptr.to_string(),
    }
}

fn rewrite_expr_tag_to_typevar(expr: &Expr, tag: &str, var: &str) -> Result<Expr> {
    match expr {
        Expr::Int { value, ptr } => Ok(Expr::Int {
            value: *value,
            ptr: ptr.clone(),
        }),
        Expr::Ident { name, ptr } => Ok(Expr::Ident {
            name: name.clone(),
            ptr: ptr.clone(),
        }),
        Expr::List { items, ptr } => {
            let Some(head) = items.first() else {
                return Ok(Expr::List {
                    items: Vec::new(),
                    ptr: ptr.clone(),
                });
            };
            let head_name = head.as_ident().unwrap_or("");
            if head_name == "tapp" {
                if items.len() >= 3 {
                    if let Expr::List {
                        items: tys,
                        ptr: tys_ptr,
                    } = &items[2]
                    {
                        if tys.first().and_then(Expr::as_ident) == Some("tys") {
                            let mut out_items = Vec::with_capacity(items.len());
                            out_items.push(items[0].clone());
                            out_items.push(items[1].clone());

                            let mut out_tys = Vec::with_capacity(tys.len());
                            out_tys.push(tys[0].clone());
                            for ty_arg in tys.iter().skip(1) {
                                if let Expr::Ident { name, ptr: tptr } = ty_arg {
                                    if name == tag {
                                        out_tys.push(type_var_expr(var, tptr));
                                        continue;
                                    }
                                }
                                out_tys.push(ty_arg.clone());
                            }
                            out_items.push(Expr::List {
                                items: out_tys,
                                ptr: tys_ptr.clone(),
                            });

                            for item in items.iter().skip(3) {
                                out_items.push(rewrite_expr_tag_to_typevar(item, tag, var)?);
                            }
                            return Ok(Expr::List {
                                items: out_items,
                                ptr: ptr.clone(),
                            });
                        }
                    }
                }

                if items.len() < 3 {
                    return Ok(expr.clone());
                }
                let mut out_items = Vec::with_capacity(items.len());
                out_items.push(items[0].clone());
                out_items.push(items[1].clone());

                let mut i = 2usize;
                while i < items.len() {
                    let item = &items[i];
                    let Some(t) = item.as_ident() else {
                        break;
                    };
                    if !SUPPORTED_TAGS.contains(&t) {
                        break;
                    }
                    if t == tag {
                        out_items.push(type_var_expr(var, item.ptr()));
                    } else {
                        out_items.push(item.clone());
                    }
                    i += 1;
                }
                for item in &items[i..] {
                    out_items.push(rewrite_expr_tag_to_typevar(item, tag, var)?);
                }
                return Ok(Expr::List {
                    items: out_items,
                    ptr: ptr.clone(),
                });
            }
            if head_name.starts_with("ty.") {
                if items.len() < 2 {
                    return Ok(expr.clone());
                }
                let mut out_items = Vec::with_capacity(items.len());
                out_items.push(items[0].clone());
                if let Expr::Ident { name, ptr: tptr } = &items[1] {
                    if name == tag {
                        out_items.push(type_var_expr(var, tptr));
                    } else {
                        out_items.push(items[1].clone());
                    }
                } else {
                    return Err(anyhow::anyhow!(
                        "unsupported non-ident type argument in intrinsic at {}",
                        items[1].ptr()
                    ));
                }
                for item in &items[2..] {
                    out_items.push(rewrite_expr_tag_to_typevar(item, tag, var)?);
                }
                return Ok(Expr::List {
                    items: out_items,
                    ptr: ptr.clone(),
                });
            }

            Ok(Expr::List {
                items: items
                    .iter()
                    .map(|e| rewrite_expr_tag_to_typevar(e, tag, var))
                    .collect::<Result<Vec<_>, _>>()?,
                ptr: ptr.clone(),
            })
        }
    }
}

fn build_wrapper(def: &AstFunctionDef, base: &str, tag: &str) -> AstFunctionDef {
    let mut out = def.clone();
    out.type_params = Vec::new();
    let mut items: Vec<Expr> = Vec::with_capacity(def.params.len() + 3);
    items.push(Expr::Ident {
        name: "tapp".to_string(),
        ptr: def.body.ptr().to_string(),
    });
    items.push(Expr::Ident {
        name: base.to_string(),
        ptr: def.body.ptr().to_string(),
    });
    items.push(Expr::List {
        items: vec![
            Expr::Ident {
                name: "tys".to_string(),
                ptr: def.body.ptr().to_string(),
            },
            Expr::Ident {
                name: tag.to_string(),
                ptr: def.body.ptr().to_string(),
            },
        ],
        ptr: def.body.ptr().to_string(),
    });
    for p in &def.params {
        items.push(Expr::Ident {
            name: p.name.clone(),
            ptr: def.body.ptr().to_string(),
        });
    }
    out.body = Expr::List {
        items,
        ptr: def.body.ptr().to_string(),
    };
    out
}
