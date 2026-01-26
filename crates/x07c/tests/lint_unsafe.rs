use serde_json::json;
use x07c::{lint, x07ast};

#[test]
fn lint_flags_unsafe_ops_in_solve_world() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.2.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["unsafe", 0]
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
            .any(|d| d.code == "X07-WORLD-UNSAFE-0001"),
        "expected X07-WORLD-UNSAFE-0001 diagnostic"
    );
}

#[test]
fn lint_flags_extern_decls_in_solve_world() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.2.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "extern",
                "abi": "C",
                "name": "main.ext_add",
                "link_name": "ext_add",
                "params": [{"name":"a","ty":"i32"},{"name":"b","ty":"i32"}],
                "result": "i32"
            }
        ],
        "solve": ["bytes.alloc", 0]
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
            .any(|d| d.code == "X07-WORLD-FFI-0001"),
        "expected X07-WORLD-FFI-0001 diagnostic"
    );
}

#[test]
fn lint_flags_raw_pointer_types_in_signatures_in_solve_world() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.2.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [{"name":"p","ty":"ptr_const_u8"}],
                "result": "i32",
                "body": 0
            }
        ],
        "solve": ["bytes.alloc", 0]
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
            .any(|d| d.code == "X07-WORLD-UNSAFE-0002"),
        "expected X07-WORLD-UNSAFE-0002 diagnostic"
    );
}
