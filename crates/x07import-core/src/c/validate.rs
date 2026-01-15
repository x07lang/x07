use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CParam {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone)]
pub struct CFunction {
    pub name: String,
    pub ret_ty: String,
    pub params: Vec<CParam>,
    pub body: Value,
}

pub fn validate_source_text(src_path: &Path, src: &str) -> Result<()> {
    for (idx, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }

        let lno = idx + 1;
        if let Some(rest) = trimmed.strip_prefix("#include") {
            let rest = rest.trim();
            if rest == "<stdint.h>" || rest == "<stddef.h>" || rest == "<stdbool.h>" {
                continue;
            }
            anyhow::bail!(
                "unsupported include directive at {}:{}: {trimmed}",
                src_path.display(),
                lno
            );
        }

        anyhow::bail!(
            "preprocessor directives are not supported at {}:{}: {trimmed}",
            src_path.display(),
            lno
        );
    }
    Ok(())
}

pub fn extract_functions(src_path: &Path, src: &str, tu: &Value) -> Result<Vec<CFunction>> {
    let ordered = parse_function_names(src_path, src)?;
    let expected: BTreeSet<&str> = ordered.iter().map(|(name, _line)| name.as_str()).collect();

    let mut found: BTreeMap<String, CFunction> = BTreeMap::new();

    let mut stack: Vec<&Value> = vec![tu];
    while let Some(node) = stack.pop() {
        if node_kind(node) == Some("FunctionDecl") {
            let Some(name) = node.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            if !expected.contains(name) {
                continue;
            }
            let f = extract_function(node)?;
            if found.insert(f.name.clone(), f).is_some() {
                anyhow::bail!("duplicate C function name: {name}");
            }
        }
        if let Some(inner) = node.get("inner").and_then(|v| v.as_array()) {
            for child in inner.iter().rev() {
                stack.push(child);
            }
        }
    }

    let mut out: Vec<CFunction> = Vec::new();
    for (name, line) in ordered {
        let Some(f) = found.remove(&name) else {
            anyhow::bail!("missing FunctionDecl for {name} (declared in source at line {line})");
        };
        out.push(f);
    }

    Ok(out)
}

fn parse_function_names(src_path: &Path, src: &str) -> Result<Vec<(String, usize)>> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for (idx, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("static inline") {
            continue;
        }
        let lno = idx + 1;
        let Some((before_paren, _after)) = trimmed.split_once('(') else {
            anyhow::bail!(
                "unsupported function signature at {}:{}: expected '('",
                src_path.display(),
                lno
            );
        };
        let mut parts = before_paren.split_whitespace();
        let Some(_static_kw) = parts.next() else {
            continue;
        };
        let Some(_inline_kw) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next_back() else {
            anyhow::bail!(
                "unsupported function signature at {}:{}: missing name",
                src_path.display(),
                lno
            );
        };
        if !is_ident(name) {
            anyhow::bail!(
                "unsupported function name at {}:{}: {name:?}",
                src_path.display(),
                lno
            );
        }
        out.push((name.to_string(), lno));
    }

    if out.is_empty() {
        anyhow::bail!("no static inline functions found in {}", src_path.display());
    }

    Ok(out)
}

fn extract_function(node: &Value) -> Result<CFunction> {
    let storage_class = node
        .get("storageClass")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let inline = node
        .get("inline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if storage_class != "static" || !inline {
        anyhow::bail!(
            "only static inline functions are supported: {}",
            describe_loc(node)
        );
    }

    let name = node
        .get("name")
        .and_then(|v| v.as_str())
        .context("FunctionDecl missing name")?
        .to_string();

    let ty = node
        .get("type")
        .and_then(|t| t.get("qualType"))
        .and_then(|v| v.as_str())
        .context("FunctionDecl missing type.qualType")?;
    let ret_ty = ty
        .split_once('(')
        .map(|(ret, _)| ret.trim().to_string())
        .unwrap_or_else(|| ty.trim().to_string());

    let mut params: Vec<CParam> = Vec::new();
    let mut body: Option<Value> = None;
    if let Some(inner) = node.get("inner").and_then(|v| v.as_array()) {
        for ch in inner {
            match node_kind(ch) {
                Some("ParmVarDecl") => params.push(extract_param(ch)?),
                Some("CompoundStmt") => body = Some(ch.clone()),
                _ => {}
            }
        }
    }

    let Some(body) = body else {
        anyhow::bail!("function missing body: {}", describe_loc(node));
    };

    Ok(CFunction {
        name,
        ret_ty,
        params,
        body,
    })
}

fn extract_param(node: &Value) -> Result<CParam> {
    let name = node
        .get("name")
        .and_then(|v| v.as_str())
        .context("ParmVarDecl missing name")?
        .to_string();

    let ty_obj = node.get("type").context("ParmVarDecl missing type")?;
    let ty = ty_obj
        .get("desugaredQualType")
        .or_else(|| ty_obj.get("qualType"))
        .and_then(|v| v.as_str())
        .context("ParmVarDecl missing type qualType")?
        .to_string();

    Ok(CParam { name, ty })
}

fn describe_loc(node: &Value) -> String {
    let loc = node.get("loc").and_then(|v| v.as_object());
    let file = loc
        .and_then(|m| m.get("file"))
        .and_then(|v| v.as_str())
        .unwrap_or("<unknown>");
    let line = loc
        .and_then(|m| m.get("line"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let col = loc
        .and_then(|m| m.get("col"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    format!("{file}:{line}:{col}")
}

fn node_kind(node: &Value) -> Option<&str> {
    node.get("kind").and_then(|v| v.as_str())
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    for c in chars {
        if !(c == '_' || c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}
