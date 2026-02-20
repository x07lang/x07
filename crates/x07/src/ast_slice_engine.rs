use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::Serialize;
use serde_json::Value;
use x07c::diagnostics;

const TRUNCATION_CODE: &str = "X07-AST-SLICE-0001";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum SliceEnclosure {
    Defn,
    Decl,
    Module,
}

impl SliceEnclosure {
    pub fn as_str(self) -> &'static str {
        match self {
            SliceEnclosure::Defn => "defn",
            SliceEnclosure::Decl => "decl",
            SliceEnclosure::Module => "module",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
pub enum SliceClosure {
    Locals,
    Types,
    Imports,
    All,
}

impl SliceClosure {
    pub fn as_str(self) -> &'static str {
        match self {
            SliceClosure::Locals => "locals",
            SliceClosure::Types => "types",
            SliceClosure::Imports => "imports",
            SliceClosure::All => "all",
        }
    }

    pub fn omit_locals(self) -> bool {
        matches!(self, SliceClosure::Types | SliceClosure::Imports)
    }

    pub fn omit_types(self) -> bool {
        matches!(self, SliceClosure::Locals | SliceClosure::Imports)
    }

    pub fn omit_imports(self) -> bool {
        matches!(self, SliceClosure::Locals | SliceClosure::Types)
    }

    pub fn include_locals(self) -> bool {
        !self.omit_locals()
    }

    pub fn include_types(self) -> bool {
        !self.omit_types()
    }

    pub fn include_imports(self) -> bool {
        !self.omit_imports()
    }
}

#[derive(Debug, Clone)]
pub struct SliceRequest {
    pub ptr: String,
    pub enclosure: SliceEnclosure,
    pub closure: SliceClosure,
    pub max_nodes: Option<usize>,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PtrRemap {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceOmitted {
    pub locals: bool,
    pub types: bool,
    pub imports: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SliceMissing {
    #[serde(default)]
    pub locals: Vec<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceTruncation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_nodes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<usize>,
    pub decls_before: usize,
    pub decls_after: usize,
    pub bytes_before: usize,
    pub bytes_after: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceMeta {
    pub ptr: String,
    pub enclosure: String,
    pub closure: String,
    #[serde(default)]
    pub ptr_remap: Vec<PtrRemap>,
    pub omitted: SliceOmitted,
    #[serde(default)]
    pub missing: SliceMissing,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<SliceTruncation>,
}

#[derive(Debug)]
pub struct SliceOutcome {
    pub slice_ast: Value,
    pub slice_meta: SliceMeta,
    pub diagnostics: Vec<diagnostics::Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Root,
    Solve,
    Decls { idx: usize },
    Other,
}

pub fn slice_x07ast(doc: &Value, req: &SliceRequest) -> Result<SliceOutcome> {
    let ptr = req.ptr.trim().to_string();
    if !ptr.is_empty() && !ptr.starts_with('/') {
        anyhow::bail!("invalid JSON Pointer (expected leading '/'): {ptr:?}");
    }
    if doc.pointer(&ptr).is_none() {
        anyhow::bail!("JSON Pointer not found: {ptr:?}");
    }

    if req.max_nodes == Some(0) {
        anyhow::bail!("--max_nodes must be >= 1");
    }
    if req.max_bytes == Some(0) {
        anyhow::bail!("--max_bytes must be >= 1");
    }

    let root_obj = doc.as_object().context("x07ast root must be an object")?;
    let schema_version = root_obj
        .get("schema_version")
        .and_then(Value::as_str)
        .context("x07ast missing schema_version")?
        .to_string();
    let input_kind = root_obj
        .get("kind")
        .and_then(Value::as_str)
        .context("x07ast missing kind")?
        .to_string();
    let module_id = root_obj
        .get("module_id")
        .and_then(Value::as_str)
        .context("x07ast missing module_id")?
        .to_string();

    let original_imports = root_obj
        .get("imports")
        .and_then(Value::as_array)
        .context("x07ast missing imports[]")?;
    let original_imports: Vec<String> = original_imports
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let original_imports_set: BTreeSet<String> = original_imports.iter().cloned().collect();

    let decls_arr = root_obj
        .get("decls")
        .and_then(Value::as_array)
        .context("x07ast missing decls[]")?;
    let original_decls: Vec<Value> = decls_arr.to_vec();

    let focus = detect_focus(&ptr);
    match req.enclosure {
        SliceEnclosure::Decl | SliceEnclosure::Defn => match focus {
            Focus::Decls { .. } => {}
            _ => anyhow::bail!(
                "--enclosure {} requires --ptr under /decls/<i>",
                req.enclosure.as_str()
            ),
        },
        SliceEnclosure::Module => {}
    }

    let focus_decl_idx = match focus {
        Focus::Decls { idx } => Some(idx),
        _ => None,
    };
    if let Some(idx) = focus_decl_idx {
        if idx >= original_decls.len() {
            anyhow::bail!("--ptr decl index out of bounds: {idx}");
        }
    }

    if req.enclosure == SliceEnclosure::Defn {
        let Some(idx) = focus_decl_idx else {
            anyhow::bail!("--enclosure defn requires --ptr under /decls/<i>");
        };
        let kind = original_decls[idx]
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if kind != "defn" && kind != "defasync" {
            anyhow::bail!("--enclosure defn requires a defn/defasync decl, got kind={kind:?}");
        }
    }

    let meta_value = root_obj.get("meta").cloned();

    let mut decl_name_to_idx: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, decl) in original_decls.iter().enumerate() {
        let Some(obj) = decl.as_object() else {
            continue;
        };
        let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("");
        if !matches!(kind, "defn" | "defasync" | "extern") {
            continue;
        }
        let Some(name) = obj.get("name").and_then(Value::as_str) else {
            continue;
        };
        decl_name_to_idx.insert(name.to_string(), idx);
    }

    let include_locals = req.closure.include_locals();
    let include_types = req.closure.include_types();
    let include_imports = req.closure.include_imports();

    let force_include_imports = ptr.starts_with("/imports");
    let force_include_types = focus_decl_idx.is_some_and(|idx| {
        ptr.starts_with(&format!("/decls/{idx}/type_params"))
            || ptr.starts_with(&format!("/decls/{idx}/requires"))
            || ptr.starts_with(&format!("/decls/{idx}/ensures"))
            || ptr.starts_with(&format!("/decls/{idx}/invariant"))
            || ptr.starts_with(&format!("/decls/{idx}/result_brand"))
            || (ptr.starts_with(&format!("/decls/{idx}/params/")) && ptr.ends_with("/brand"))
    });
    let effective_include_imports = include_imports || force_include_imports;
    let effective_include_types = include_types || force_include_types;

    let mut selected_decl_idxs: BTreeSet<usize> = BTreeSet::new();
    let mut queue: VecDeque<usize> = VecDeque::new();

    if ptr.is_empty() {
        for idx in 0..original_decls.len() {
            selected_decl_idxs.insert(idx);
        }
    } else if let Some(focus_idx) = focus_decl_idx {
        selected_decl_idxs.insert(focus_idx);
        queue.push_back(focus_idx);
    } else if focus == Focus::Solve && include_locals {
        for name in collect_decl_calls_from_solve(doc) {
            if let Some(idx) = decl_name_to_idx.get(&name).copied() {
                if selected_decl_idxs.insert(idx) {
                    queue.push_back(idx);
                }
            }
        }
    }

    if include_locals {
        if let Some(focus_idx) = focus_decl_idx {
            if is_export_decl(&original_decls[focus_idx]) {
                for sym in export_names(&original_decls[focus_idx]) {
                    if let Some(idx) = decl_name_to_idx.get(&sym).copied() {
                        if selected_decl_idxs.insert(idx) {
                            queue.push_back(idx);
                        }
                    }
                }
            }
        }

        while let Some(idx) = queue.pop_front() {
            let decl = &original_decls[idx];
            for name in collect_decl_calls_from_decl(decl) {
                let Some(dep_idx) = decl_name_to_idx.get(&name).copied() else {
                    continue;
                };
                if selected_decl_idxs.insert(dep_idx) {
                    queue.push_back(dep_idx);
                }
            }
        }
    }

    let mut selected_decl_idxs_with_exports = selected_decl_idxs.clone();
    let selected_decl_names: BTreeSet<String> = selected_decl_idxs
        .iter()
        .filter_map(|idx| decl_symbol_name(&original_decls[*idx]))
        .collect();
    for (idx, decl) in original_decls.iter().enumerate() {
        if !is_export_decl(decl) {
            continue;
        }
        if export_names(decl)
            .into_iter()
            .any(|name| selected_decl_names.contains(&name))
        {
            selected_decl_idxs_with_exports.insert(idx);
        }
    }

    let focus_out_first = matches!(req.enclosure, SliceEnclosure::Decl | SliceEnclosure::Defn)
        && focus_decl_idx.is_some();
    let output_decl_idxs: Vec<usize> = if focus_out_first {
        let focus = focus_decl_idx.expect("focus decl idx");
        let mut rest: Vec<usize> = selected_decl_idxs_with_exports
            .iter()
            .copied()
            .filter(|idx| *idx != focus)
            .collect();
        rest.sort();
        let mut out = vec![focus];
        out.extend(rest);
        out
    } else {
        selected_decl_idxs_with_exports.iter().copied().collect()
    };

    let focus_out_idx =
        focus_decl_idx.and_then(|idx| output_decl_idxs.iter().position(|v| *v == idx));

    let required_imports = if force_include_imports {
        original_imports.clone()
    } else {
        minimal_required_imports(
            doc,
            &original_decls,
            &output_decl_idxs,
            &original_imports_set,
        )
    };

    let mut missing = SliceMissing::default();
    if !effective_include_imports {
        missing.imports = required_imports.clone();
    }

    if !include_locals {
        missing.locals = compute_missing_locals(
            doc,
            &original_decls,
            focus,
            &decl_name_to_idx,
            &selected_decl_idxs,
        );
    }

    if !effective_include_types {
        missing.types = compute_missing_types(&original_decls, &output_decl_idxs);
    }

    let mut slice_decls: Vec<Value> = Vec::new();
    for idx in &output_decl_idxs {
        let decl = &original_decls[*idx];
        if is_export_decl(decl) {
            if let Some(filtered) = filter_export_decl(decl, &selected_decl_names) {
                slice_decls.push(filtered);
            }
            continue;
        }

        let mut decl_out = decl.clone();
        if !effective_include_types {
            strip_types_in_decl(&mut decl_out);
        }
        slice_decls.push(decl_out);
    }

    let output_kind = if req.enclosure == SliceEnclosure::Module {
        input_kind.clone()
    } else {
        "module".to_string()
    };

    let mut slice_root = serde_json::Map::new();
    slice_root.insert(
        "schema_version".to_string(),
        Value::String(schema_version.clone()),
    );
    slice_root.insert("kind".to_string(), Value::String(output_kind.clone()));
    slice_root.insert("module_id".to_string(), Value::String(module_id.clone()));

    let mut out_imports: Vec<String> = if effective_include_imports {
        required_imports
    } else {
        Vec::new()
    };
    out_imports.sort();
    out_imports.dedup();
    slice_root.insert(
        "imports".to_string(),
        Value::Array(out_imports.into_iter().map(Value::String).collect()),
    );

    slice_root.insert("decls".to_string(), Value::Array(slice_decls));

    if output_kind == "entry" {
        if let Some(solve) = root_obj.get("solve") {
            slice_root.insert("solve".to_string(), solve.clone());
        } else {
            slice_root.insert("solve".to_string(), Value::Number(0.into()));
        }
    }

    if let Some(Value::Object(meta)) = meta_value {
        if !meta.is_empty() {
            slice_root.insert("meta".to_string(), Value::Object(meta));
        }
    }

    let mut slice_ast = Value::Object(slice_root);

    let mut ptr_remap = Vec::new();
    if let (Some(orig_idx), Some(new_idx)) = (focus_decl_idx, focus_out_idx) {
        if new_idx != orig_idx {
            if let Some(remap) = remap_decl_pointer(&ptr, orig_idx, new_idx) {
                ptr_remap.push(remap);
            }
        }
    }

    let omitted = SliceOmitted {
        locals: !include_locals,
        types: !effective_include_types,
        imports: !effective_include_imports,
    };

    let mut slice_meta = SliceMeta {
        ptr: ptr.clone(),
        enclosure: req.enclosure.as_str().to_string(),
        closure: req.closure.as_str().to_string(),
        ptr_remap,
        omitted,
        missing,
        truncated: false,
        truncation: None,
    };

    let mut diagnostics_out = Vec::new();

    let decls_before = slice_ast
        .get("decls")
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);
    let bytes_before = canonical_len(&slice_ast)?;

    let mut focus_decl_pos = focus_out_idx;
    if let Some(max_nodes) = req.max_nodes {
        if decls_before > max_nodes {
            enforce_max_nodes(&mut slice_ast, max_nodes, focus_decl_pos.as_mut())?;
            slice_meta.truncated = true;
            let decls_after = slice_ast
                .get("decls")
                .and_then(Value::as_array)
                .map(|a| a.len())
                .unwrap_or(0);
            let bytes_after = canonical_len(&slice_ast)?;
            slice_meta.truncation = Some(SliceTruncation {
                max_nodes: req.max_nodes,
                max_bytes: req.max_bytes,
                decls_before,
                decls_after,
                bytes_before,
                bytes_after,
                reason: "max_nodes".to_string(),
            });
            diagnostics_out.push(truncation_diag(
                req,
                decls_before,
                decls_after,
                bytes_before,
                bytes_after,
                "max_nodes",
            )?);
        }
    }

    if let Some(max_bytes) = req.max_bytes {
        let mut current = canonical_len(&slice_ast)?;
        if current > max_bytes {
            let focus_ptr_in_slice = slice_meta
                .ptr_remap
                .first()
                .map(|r| r.to.clone())
                .unwrap_or_else(|| ptr.clone());
            enforce_max_bytes(
                &mut slice_ast,
                max_bytes,
                &focus_ptr_in_slice,
                focus_decl_pos.as_mut(),
            )?;
            current = canonical_len(&slice_ast)?;
            if current > max_bytes {
                anyhow::bail!("failed to truncate slice to --max_bytes={max_bytes}");
            }

            slice_meta.truncated = true;
            let decls_after = slice_ast
                .get("decls")
                .and_then(Value::as_array)
                .map(|a| a.len())
                .unwrap_or(0);
            let bytes_after = current;
            let reason = slice_meta
                .truncation
                .as_ref()
                .map(|t| t.reason.clone())
                .unwrap_or_else(|| "max_bytes".to_string());
            slice_meta.truncation = Some(SliceTruncation {
                max_nodes: req.max_nodes,
                max_bytes: req.max_bytes,
                decls_before,
                decls_after,
                bytes_before,
                bytes_after,
                reason: reason.clone(),
            });
            diagnostics_out.push(truncation_diag(
                req,
                decls_before,
                decls_after,
                bytes_before,
                bytes_after,
                &reason,
            )?);
        }
    }

    x07c::x07ast::canon_value_jcs(&mut slice_ast);

    Ok(SliceOutcome {
        slice_ast,
        slice_meta,
        diagnostics: diagnostics_out,
    })
}

fn detect_focus(ptr: &str) -> Focus {
    let ptr = ptr.trim();
    if ptr.is_empty() {
        return Focus::Root;
    }
    let mut it = ptr.split('/').skip(1);
    let Some(first) = it.next() else {
        return Focus::Root;
    };
    match first {
        "solve" => Focus::Solve,
        "decls" => {
            let Some(raw) = it.next() else {
                return Focus::Other;
            };
            if let Ok(idx) = raw.parse::<usize>() {
                Focus::Decls { idx }
            } else {
                Focus::Other
            }
        }
        _ => Focus::Other,
    }
}

fn decl_symbol_name(decl: &Value) -> Option<String> {
    let obj = decl.as_object()?;
    let kind = obj.get("kind").and_then(Value::as_str)?;
    if !matches!(kind, "defn" | "defasync" | "extern") {
        return None;
    }
    obj.get("name").and_then(Value::as_str).map(str::to_string)
}

fn is_export_decl(decl: &Value) -> bool {
    decl.get("kind").and_then(Value::as_str) == Some("export")
}

fn export_names(decl: &Value) -> Vec<String> {
    let Some(arr) = decl.get("names").and_then(Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn filter_export_decl(decl: &Value, allowed: &BTreeSet<String>) -> Option<Value> {
    let mut out = decl.clone();
    let obj = out.as_object_mut()?;
    let names = obj.get_mut("names").and_then(Value::as_array_mut)?;
    names.retain(|v| v.as_str().is_some_and(|s| allowed.contains(s)));
    if names.is_empty() {
        return None;
    }
    Some(out)
}

fn collect_decl_calls_from_solve(doc: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(solve) = doc.get("solve") else {
        return out;
    };
    scan_calls_in_sexpr(solve, &mut out);
    out
}

fn collect_decl_calls_from_decl(decl: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(obj) = decl.as_object() else {
        return out;
    };
    if let Some(body) = obj.get("body") {
        scan_calls_in_sexpr(body, &mut out);
    }
    for key in ["requires", "ensures", "invariant"] {
        let Some(arr) = obj.get(key).and_then(Value::as_array) else {
            continue;
        };
        for clause in arr {
            let Some(clause_obj) = clause.as_object() else {
                continue;
            };
            if let Some(expr) = clause_obj.get("expr") {
                scan_calls_in_sexpr(expr, &mut out);
            }
            if let Some(witness) = clause_obj.get("witness").and_then(Value::as_array) {
                for w in witness {
                    scan_calls_in_sexpr(w, &mut out);
                }
            }
        }
    }
    out
}

fn scan_calls_in_sexpr(expr: &Value, out: &mut BTreeSet<String>) {
    match expr {
        Value::Array(items) => {
            if let Some(Value::String(head)) = items.first() {
                out.insert(head.clone());
            }
            for item in items {
                scan_calls_in_sexpr(item, out);
            }
        }
        Value::Object(obj) => {
            for v in obj.values() {
                scan_calls_in_sexpr(v, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn minimal_required_imports(
    doc: &Value,
    original_decls: &[Value],
    output_decl_idxs: &[usize],
    original_imports: &BTreeSet<String>,
) -> Vec<String> {
    let mut referenced_modules: BTreeSet<String> = BTreeSet::new();

    if let Some(solve) = doc.get("solve") {
        collect_module_refs_from_sexpr(solve, &mut referenced_modules);
    }

    for idx in output_decl_idxs {
        let decl = &original_decls[*idx];
        collect_module_refs_from_decl(decl, &mut referenced_modules);
    }

    referenced_modules
        .into_iter()
        .filter(|m| original_imports.contains(m))
        .collect()
}

fn collect_module_refs_from_decl(decl: &Value, out: &mut BTreeSet<String>) {
    if is_export_decl(decl) {
        return;
    }
    let Some(obj) = decl.as_object() else {
        return;
    };
    if let Some(body) = obj.get("body") {
        collect_module_refs_from_sexpr(body, out);
    }
    for key in ["requires", "ensures", "invariant"] {
        let Some(arr) = obj.get(key).and_then(Value::as_array) else {
            continue;
        };
        for clause in arr {
            let Some(clause_obj) = clause.as_object() else {
                continue;
            };
            if let Some(expr) = clause_obj.get("expr") {
                collect_module_refs_from_sexpr(expr, out);
            }
            if let Some(witness) = clause_obj.get("witness").and_then(Value::as_array) {
                for w in witness {
                    collect_module_refs_from_sexpr(w, out);
                }
            }
        }
    }
}

fn collect_module_refs_from_sexpr(expr: &Value, out: &mut BTreeSet<String>) {
    match expr {
        Value::Array(items) => {
            if let Some(Value::String(head)) = items.first() {
                if let Some(module_id) = module_id_from_symbol(head) {
                    out.insert(module_id);
                }
            }
            for item in items {
                collect_module_refs_from_sexpr(item, out);
            }
        }
        Value::Object(obj) => {
            for v in obj.values() {
                collect_module_refs_from_sexpr(v, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn module_id_from_symbol(sym: &str) -> Option<String> {
    let mut parts: Vec<&str> = sym.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    parts.pop();
    Some(parts.join("."))
}

fn compute_missing_locals(
    doc: &Value,
    original_decls: &[Value],
    focus: Focus,
    name_to_idx: &BTreeMap<String, usize>,
    selected_decl_idxs: &BTreeSet<usize>,
) -> Vec<String> {
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    match focus {
        Focus::Decls { idx } => {
            if idx < original_decls.len() {
                referenced = collect_decl_calls_from_decl(&original_decls[idx]);
            }
        }
        Focus::Solve => {
            referenced = collect_decl_calls_from_solve(doc);
        }
        _ => {}
    }

    let mut out = Vec::new();
    for name in referenced {
        let Some(idx) = name_to_idx.get(&name).copied() else {
            continue;
        };
        if !selected_decl_idxs.contains(&idx) {
            out.push(name);
        }
    }
    out
}

fn compute_missing_types(original_decls: &[Value], output_decl_idxs: &[usize]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut stripped_contracts = false;

    for idx in output_decl_idxs {
        let decl = &original_decls[*idx];
        let Some(obj) = decl.as_object() else {
            continue;
        };
        if let Some(tps) = obj.get("type_params").and_then(Value::as_array) {
            for tp in tps {
                if let Some(name) = tp.get("name").and_then(Value::as_str) {
                    out.insert(name.to_string());
                }
            }
        }
        for key in ["requires", "ensures", "invariant"] {
            let Some(arr) = obj.get(key).and_then(Value::as_array) else {
                continue;
            };
            if !arr.is_empty() {
                stripped_contracts = true;
            }
        }
    }

    if stripped_contracts {
        out.insert("contracts".to_string());
    }

    out.into_iter().collect()
}

fn strip_types_in_decl(decl: &mut Value) {
    let Some(obj) = decl.as_object_mut() else {
        return;
    };
    let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("");
    if !matches!(kind, "defn" | "defasync") {
        if let Some(params) = obj.get_mut("params").and_then(Value::as_array_mut) {
            for param in params {
                if let Some(pobj) = param.as_object_mut() {
                    pobj.remove("brand");
                }
            }
        }
        return;
    }

    obj.remove("type_params");
    obj.remove("requires");
    obj.remove("ensures");
    obj.remove("invariant");
    obj.remove("result_brand");

    if let Some(params) = obj.get_mut("params").and_then(Value::as_array_mut) {
        for param in params {
            if let Some(pobj) = param.as_object_mut() {
                pobj.remove("brand");
            }
        }
    }
}

fn remap_decl_pointer(ptr: &str, orig_idx: usize, new_idx: usize) -> Option<PtrRemap> {
    let prefix = format!("/decls/{orig_idx}");
    if !ptr.starts_with(&prefix) {
        return None;
    }
    let to = format!("/decls/{new_idx}{}", &ptr[prefix.len()..]);
    Some(PtrRemap {
        from: ptr.to_string(),
        to,
    })
}

fn canonical_len(v: &Value) -> Result<usize> {
    Ok(crate::util::canonical_jcs_bytes(v)?.len())
}

fn enforce_max_nodes(
    ast: &mut Value,
    max_nodes: usize,
    focus_decl_pos: Option<&mut usize>,
) -> Result<()> {
    let Some(decls) = ast.get_mut("decls").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    if decls.len() <= max_nodes {
        return Ok(());
    }

    let mut focus = focus_decl_pos.as_deref().copied();
    while decls.len() > max_nodes {
        let mut remove_idx = decls.len() - 1;
        if let Some(focus_idx) = focus {
            if remove_idx == focus_idx {
                if remove_idx == 0 {
                    break;
                }
                remove_idx -= 1;
            }
        }

        decls.remove(remove_idx);
        if let Some(focus_idx) = focus.as_mut() {
            if remove_idx < *focus_idx {
                *focus_idx -= 1;
            }
        }
    }

    if let (Some(dst), Some(src)) = (focus_decl_pos, focus) {
        *dst = src;
    }
    Ok(())
}

fn enforce_max_bytes(
    ast: &mut Value,
    max_bytes: usize,
    focus_ptr: &str,
    focus_decl_pos: Option<&mut usize>,
) -> Result<()> {
    if canonical_len(ast)? <= max_bytes {
        return Ok(());
    }

    let mut focus = focus_decl_pos.as_deref().copied();
    while canonical_len(ast)? > max_bytes {
        let Some(decls) = ast.get_mut("decls").and_then(Value::as_array_mut) else {
            break;
        };
        if decls.len() <= 1 {
            break;
        }

        let mut remove_idx = decls.len() - 1;
        if let Some(focus_idx) = focus {
            if remove_idx == focus_idx {
                if remove_idx == 0 {
                    break;
                }
                remove_idx -= 1;
            }
        }

        decls.remove(remove_idx);
        if let Some(focus_idx) = focus.as_mut() {
            if remove_idx < *focus_idx {
                *focus_idx -= 1;
            }
        }
    }

    if canonical_len(ast)? <= max_bytes {
        if let (Some(dst), Some(src)) = (focus_decl_pos, focus) {
            *dst = src;
        }
        return Ok(());
    }

    if let Some(focus_idx) = focus {
        prune_focus_decl(ast, focus_idx, focus_ptr);
    } else if focus_ptr.starts_with("/solve") {
        prune_solve(ast, focus_ptr);
    }

    if canonical_len(ast)? > max_bytes {
        if let Some(obj) = ast.as_object_mut() {
            if !focus_ptr.starts_with("/meta") {
                obj.remove("meta");
            }
        }

        if let Some(focus_idx) = focus {
            zero_defn_body(ast, focus_idx);
        } else if focus_ptr.starts_with("/solve") {
            if let Some(obj) = ast.as_object_mut() {
                obj.insert("solve".to_string(), Value::Number(0.into()));
            }
        }
    }

    if let (Some(dst), Some(src)) = (focus_decl_pos, focus) {
        *dst = src;
    }
    Ok(())
}

fn zero_defn_body(ast: &mut Value, focus_decl_idx: usize) {
    let Some(decls) = ast.get_mut("decls").and_then(Value::as_array_mut) else {
        return;
    };
    let Some(decl) = decls.get_mut(focus_decl_idx) else {
        return;
    };
    let Some(obj) = decl.as_object_mut() else {
        return;
    };
    let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("");
    if !matches!(kind, "defn" | "defasync") {
        return;
    }
    for key in ["requires", "ensures", "invariant"] {
        obj.remove(key);
    }
    obj.insert("body".to_string(), Value::Number(0.into()));
}

fn prune_focus_decl(ast: &mut Value, focus_decl_idx: usize, focus_ptr: &str) {
    let Some(decls) = ast.get_mut("decls").and_then(Value::as_array_mut) else {
        return;
    };
    let Some(decl) = decls.get_mut(focus_decl_idx) else {
        return;
    };
    let Some(obj) = decl.as_object_mut() else {
        return;
    };

    // Remove large optional fields unless the focus pointer is inside them.
    for key in ["requires", "ensures", "invariant"] {
        let prefix = format!("/decls/{focus_decl_idx}/{key}");
        if !focus_ptr.starts_with(&prefix) {
            obj.remove(key);
        }
    }

    let body_prefix = format!("/decls/{focus_decl_idx}/body");
    if !focus_ptr.starts_with(&body_prefix) {
        obj.insert("body".to_string(), Value::Number(0.into()));
        return;
    }

    let suffix = focus_ptr[body_prefix.len()..].to_string();
    if let Some(body) = obj.get_mut("body") {
        prune_value_to_pointer(body, &suffix);
    }
}

fn prune_solve(ast: &mut Value, focus_ptr: &str) {
    let Some(solve) = ast.get_mut("solve") else {
        return;
    };
    let suffix = focus_ptr.strip_prefix("/solve").unwrap_or("");
    prune_value_to_pointer(solve, suffix);
}

fn prune_value_to_pointer(value: &mut Value, suffix_ptr: &str) {
    let suffix_ptr = suffix_ptr.trim();
    if suffix_ptr.is_empty() || suffix_ptr == "/" {
        return;
    }
    if !suffix_ptr.starts_with('/') {
        return;
    }

    let tokens: Vec<&str> = suffix_ptr
        .split('/')
        .skip(1)
        .filter(|t| !t.is_empty())
        .collect();
    prune_recursive(value, &tokens);
}

fn prune_recursive(value: &mut Value, tokens: &[&str]) {
    if tokens.is_empty() {
        return;
    }
    match value {
        Value::Array(items) => {
            let Ok(idx) = tokens[0].parse::<usize>() else {
                return;
            };
            if idx >= items.len() {
                return;
            }

            let is_bytes_lit =
                matches!(items.first(), Some(Value::String(head)) if head == "bytes.lit");
            let min_len = if is_bytes_lit { 2 } else { 1 };
            let target_len = (idx + 1).max(min_len);
            if items.len() > target_len {
                items.truncate(target_len);
            }

            let start = if is_bytes_lit {
                2
            } else if matches!(items.first(), Some(Value::String(_))) {
                1
            } else {
                0
            };
            for i in start..idx {
                if i < items.len() {
                    items[i] = Value::Number(0.into());
                }
            }

            if idx < items.len() {
                prune_recursive(&mut items[idx], &tokens[1..]);
            }
        }
        Value::Object(map) => {
            let key = tokens[0];
            if let Some(next) = map.get_mut(key) {
                let keep = std::mem::take(next);
                map.clear();
                map.insert(key.to_string(), keep);
                if let Some(v) = map.get_mut(key) {
                    prune_recursive(v, &tokens[1..]);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn truncation_diag(
    req: &SliceRequest,
    decls_before: usize,
    decls_after: usize,
    bytes_before: usize,
    bytes_after: usize,
    reason: &str,
) -> Result<diagnostics::Diagnostic> {
    let mut data = BTreeMap::new();
    if let Some(n) = req.max_nodes {
        data.insert("max_nodes".to_string(), Value::Number(n.into()));
    }
    if let Some(n) = req.max_bytes {
        data.insert("max_bytes".to_string(), Value::Number(n.into()));
    }
    data.insert(
        "decls_before".to_string(),
        Value::Number((decls_before as u64).into()),
    );
    data.insert(
        "decls_after".to_string(),
        Value::Number((decls_after as u64).into()),
    );
    data.insert(
        "bytes_before".to_string(),
        Value::Number((bytes_before as u64).into()),
    );
    data.insert(
        "bytes_after".to_string(),
        Value::Number((bytes_after as u64).into()),
    );
    data.insert("reason".to_string(), Value::String(reason.to_string()));

    Ok(diagnostics::Diagnostic {
        code: TRUNCATION_CODE.to_string(),
        severity: diagnostics::Severity::Info,
        stage: diagnostics::Stage::Run,
        message: format!("x07 ast slice truncated ({reason})"),
        loc: Some(diagnostics::Location::X07Ast {
            ptr: req.ptr.trim().to_string(),
        }),
        notes: Vec::new(),
        related: Vec::new(),
        data,
        quickfix: None,
    })
}
