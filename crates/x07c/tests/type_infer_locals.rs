use serde_json::json;

use x07c::typecheck::{typecheck_file_local, TypecheckOptions};

mod typecheck_testutil;

#[test]
fn infer_locals_let_and_set_smoke() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["begin", ["let", "x", 0], ["set", "x", ["+", "x", 1]], ["bytes.alloc", 0]]
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
fn infer_locals_set_unknown_local_errors() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["begin", ["set", "x", 0], ["bytes.alloc", 0]]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-TYPE-SET-0001"),
        "expected X07-TYPE-SET-0001, got: {:?}",
        report.diagnostics
    );
}
