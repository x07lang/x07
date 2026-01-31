#![allow(dead_code)]

use serde_json::{json, Value};
use x07_contracts::X07AST_SCHEMA_VERSION;

pub fn entry(imports: &[&str], decls: Vec<Value>, solve: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": imports,
        "decls": decls,
        "solve": solve,
    }))
    .expect("encode x07AST entry JSON")
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

pub fn extern_c(name: &str, link_name: &str, params: &[(&str, &str)], result: &str) -> Value {
    let params: Vec<Value> = params
        .iter()
        .map(|(name, ty)| json!({ "name": name, "ty": ty }))
        .collect();
    json!({
        "kind": "extern",
        "abi": "C",
        "name": name,
        "link_name": link_name,
        "params": params,
        "result": result,
    })
}
