use serde_json::json;

use x07c::typecheck::{typecheck_file_local, TypecheckOptions};

mod typecheck_testutil;

fn has_code(diags: &[x07c::diagnostics::Diagnostic], code: &str) -> bool {
    diags.iter().any(|d| d.code == code)
}

#[test]
fn ensures_binds_result() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "ensures": [{"expr": ["=", "__result", 0]}],
                "body": 0
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn requires_may_not_reference_result() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "requires": [{"expr": "__result"}],
                "body": 0
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        has_code(&report.diagnostics, "X07-CONTRACT-0003"),
        "expected X07-CONTRACT-0003, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn contract_clause_expr_must_typecheck_to_i32() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "requires": [{"expr": ["bytes.lit", "x"]}],
                "body": 0
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        has_code(&report.diagnostics, "X07-CONTRACT-0001"),
        "expected X07-CONTRACT-0001, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn contract_clause_expr_must_be_pure() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [{"name":"p","ty":"bytes_view"}],
                "result": "i32",
                "requires": [{"expr": ["std.world.fs.read_file", "p"]}],
                "body": 0
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        has_code(&report.diagnostics, "X07-CONTRACT-0002"),
        "expected X07-CONTRACT-0002, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn reserved_result_name_is_rejected_in_params() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [{"name":"__result","ty":"i32"}],
                "result": "i32",
                "requires": [{"expr": 1}],
                "body": 0
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());
    assert!(
        has_code(&report.diagnostics, "X07-CONTRACT-0004"),
        "expected X07-CONTRACT-0004, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn reserved_result_name_is_rejected_in_locals() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "requires": [{"expr": 1}],
                "body": ["begin", ["let", "__result", 0], 0]
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());
    assert!(
        has_code(&report.diagnostics, "X07-CONTRACT-0004"),
        "expected X07-CONTRACT-0004, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn witness_type_is_restricted() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "requires": [{
                    "expr": 1,
                    "witness": [["vec_u8.with_capacity", 0]]
                }],
                "body": 0
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        has_code(&report.diagnostics, "X07-CONTRACT-0005"),
        "expected X07-CONTRACT-0005, got: {:?}",
        report.diagnostics
    );
}
