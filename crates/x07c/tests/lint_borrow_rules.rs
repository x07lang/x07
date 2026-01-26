use serde_json::json;
use x07c::{lint, x07ast};

#[test]
fn lint_rejects_bytes_view_of_temporary() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.2.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["view.to_bytes", ["bytes.view", ["bytes.lit", "hello"]]]
    });
    let doc_bytes = serde_json::to_vec(&doc).expect("serialize");
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);

    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-BORROW-0001"),
        "expected X07-BORROW-0001 diagnostic"
    );
}
