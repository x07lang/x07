#![allow(dead_code)]

use serde_json::{json, Value};

pub fn entry(imports: &[&str], solve: Value) -> Vec<u8> {
    entry_with_decls(imports, Vec::new(), solve)
}

pub fn entry_with_decls(imports: &[&str], decls: Vec<Value>, solve: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": "x07.x07ast@0.1.0",
        "kind": "entry",
        "module_id": "main",
        "imports": imports,
        "decls": decls,
        "solve": solve,
    }))
    .expect("encode x07AST entry JSON")
}

pub fn export(names: &[&str]) -> Value {
    json!({
        "kind": "export",
        "names": names,
    })
}

pub fn defn(name: &str, params: &[(&str, &str)], result: &str, body: Value) -> Value {
    let params: Vec<Value> = params
        .iter()
        .map(|(name, ty)| json!({ "name": name, "ty": ty }))
        .collect();
    json!({
        "kind": "defn",
        "name": name,
        "params": params,
        "result": result,
        "body": body,
    })
}

pub fn defasync(name: &str, params: &[(&str, &str)], result: &str, body: Value) -> Value {
    let params: Vec<Value> = params
        .iter()
        .map(|(name, ty)| json!({ "name": name, "ty": ty }))
        .collect();
    json!({
        "kind": "defasync",
        "name": name,
        "params": params,
        "result": result,
        "body": body,
    })
}
