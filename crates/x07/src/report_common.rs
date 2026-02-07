use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use jsonschema::Draft;
use serde_json::Value;
use x07c::diagnostics;

const X07DIAG_SCHEMA_BYTES: &[u8] = include_bytes!("../../../spec/x07diag.schema.json");

#[derive(Debug, Clone, Default)]
pub(crate) struct SensitiveScan {
    pub(crate) namespaces: BTreeSet<String>,
    pub(crate) op_counts: BTreeMap<String, u64>,
    pub(crate) budget_scopes: Vec<BudgetScopeHit>,
    pub(crate) uses_os_time: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BudgetScopeHit {
    pub(crate) kind: String,
    pub(crate) ptr: String,
    pub(crate) label: Option<String>,
    pub(crate) mode: Option<String>,
    pub(crate) limits: BTreeMap<String, Option<u64>>,
    pub(crate) arch_profile_id: Option<String>,
}

const SENSITIVE_NAMESPACES: &[&str] = &[
    "std.fs.",
    "std.rr.",
    "std.kv.",
    "std.db.",
    "std.msg.",
    "std.os.env.",
    "std.os.fs.",
    "std.os.net.",
    "std.os.process.",
    "std.os.time.",
    "os.db.",
    "os.msg.",
    "ext.http.",
    "ext.db.",
    "ext.msg.",
];

pub(crate) fn scan_sensitive(value: &Value) -> SensitiveScan {
    let mut out = SensitiveScan::default();
    scan_sensitive_at(value, "", &mut out);
    out
}

fn scan_sensitive_at(value: &Value, ptr: &str, out: &mut SensitiveScan) {
    match value {
        Value::Array(items) => {
            if let Some(op) = items.first().and_then(Value::as_str) {
                *out.op_counts.entry(op.to_string()).or_insert(0) += 1;

                for ns in SENSITIVE_NAMESPACES {
                    if op.starts_with(ns) {
                        out.namespaces.insert((*ns).to_string());
                        if *ns == "std.os.time." {
                            out.uses_os_time = true;
                        }
                        break;
                    }
                }

                if op == "budget.scope_v1" {
                    out.budget_scopes.push(parse_budget_scope_v1(items, ptr));
                } else if op == "budget.scope_from_arch_v1" {
                    out.budget_scopes
                        .push(parse_budget_scope_from_arch_v1(items, ptr));
                }
            }

            for (idx, item) in items.iter().enumerate() {
                let child_ptr = format!("{ptr}/{idx}");
                scan_sensitive_at(item, &child_ptr, out);
            }
        }
        Value::Object(obj) => {
            for (k, v) in obj {
                let child_ptr = format!("{ptr}/{}", escape_json_pointer(k));
                scan_sensitive_at(v, &child_ptr, out);
            }
        }
        _ => {}
    }
}

fn parse_budget_scope_v1(items: &[Value], ptr: &str) -> BudgetScopeHit {
    let mut hit = BudgetScopeHit {
        kind: "inline_v1".to_string(),
        ptr: ptr.to_string(),
        ..BudgetScopeHit::default()
    };

    if let Some(Value::Object(cfg)) = items.get(1) {
        hit.label = cfg.get("label").and_then(Value::as_str).map(str::to_string);
        hit.mode = cfg.get("mode").and_then(Value::as_str).map(str::to_string);
        if let Some(Value::Object(limits)) = cfg.get("limits") {
            for key in [
                "fuel",
                "alloc_bytes",
                "alloc_calls",
                "realloc_calls",
                "free_calls",
                "memcpy_bytes",
                "sched_ticks",
            ] {
                let val = limits.get(key).and_then(Value::as_u64);
                hit.limits.insert(key.to_string(), val);
            }
        }
    }

    hit
}

fn parse_budget_scope_from_arch_v1(items: &[Value], ptr: &str) -> BudgetScopeHit {
    let mut hit = BudgetScopeHit {
        kind: "from_arch_v1".to_string(),
        ptr: ptr.to_string(),
        ..BudgetScopeHit::default()
    };

    if let Some(v) = items.get(1) {
        if let Some(s) = v.as_str() {
            hit.arch_profile_id = Some(s.to_string());
        } else if let Value::Object(obj) = v {
            if let Some(id) = obj.get("profile_id").and_then(Value::as_str) {
                hit.arch_profile_id = Some(id.to_string());
            } else if let Some(id) = obj.get("id").and_then(Value::as_str) {
                hit.arch_profile_id = Some(id.to_string());
            }
        }
    }

    hit
}

pub(crate) fn validate_schema(
    schema_bytes: &[u8],
    schema_name: &str,
    value: &Value,
) -> Result<Vec<diagnostics::Diagnostic>> {
    let schema_json: Value =
        serde_json::from_slice(schema_bytes).with_context(|| format!("parse {schema_name}"))?;
    let x07diag_schema_json: Value =
        serde_json::from_slice(X07DIAG_SCHEMA_BYTES).context("parse spec/x07diag.schema.json")?;
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_resource(
            "x07diag.schema.json",
            jsonschema::Resource::from_contents(x07diag_schema_json.clone()),
        )
        .with_resource(
            "https://x07.io/spec/x07diag.schema.json",
            jsonschema::Resource::from_contents(x07diag_schema_json),
        )
        .build(&schema_json)
        .with_context(|| format!("build {schema_name} validator"))?;

    let mut out = Vec::new();
    for error in validator.iter_errors(value) {
        let mut data = std::collections::BTreeMap::new();
        data.insert(
            "schema_path".to_string(),
            Value::String(error.schema_path().to_string()),
        );
        out.push(diagnostics::Diagnostic {
            code: "X07-SCHEMA-0001".to_string(),
            severity: diagnostics::Severity::Error,
            stage: diagnostics::Stage::Parse,
            message: error.to_string(),
            loc: Some(diagnostics::Location::X07Ast {
                ptr: error.instance_path().to_string(),
            }),
            notes: Vec::new(),
            related: Vec::new(),
            data,
            quickfix: None,
        });
    }
    Ok(out)
}

pub(crate) fn canonical_pretty_json_bytes(v: &Value) -> Result<Vec<u8>> {
    let mut v = v.clone();
    x07c::x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

pub(crate) fn read_json_file(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).with_context(|| format!("read: {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse JSON: {}", path.display()))
}

pub(crate) fn html_escape(s: impl AsRef<str>) -> String {
    s.as_ref()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn escape_json_pointer(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}
