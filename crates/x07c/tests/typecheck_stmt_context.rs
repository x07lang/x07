use serde_json::json;

use x07c::typecheck::{typecheck_file_local, TypecheckOptions};

mod typecheck_testutil;

#[test]
fn typecheck_allows_if_branch_mismatch_in_stmt_position() {
    // Matches the compiler's statement-like typechecking rules for `begin`/`for` bodies.
    let doc = json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": [
            "begin",
            ["let", "b", ["bytes.alloc", 0]],
            ["if", 1, ["set", "b", ["bytes.alloc", 1]], 0],
            "b"
        ]
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
fn typecheck_rejects_if_branch_mismatch_in_expr_position() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["begin", ["let", "x", ["if", 1, ["bytes.alloc", 0], 0]], ["bytes.alloc", 0]]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-TYPE-IF-0002"),
        "expected X07-TYPE-IF-0002, got: {:?}",
        report.diagnostics
    );
}
