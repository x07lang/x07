use serde_json::json;
use x07_contracts::X07AST_SCHEMA_VERSION;
use x07c::{json_patch, lint, x07ast};

#[test]
fn lint_rejects_bytes_view_of_temporary() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
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
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BORROW-0001")
        .expect("expected X07-BORROW-0001 diagnostic");
    assert!(
        diag.notes
            .iter()
            .any(|note| note.contains("Suggested fix:")),
        "expected X07-BORROW-0001 Suggested fix note"
    );
}

#[test]
fn lint_borrow_subview_note_preserves_start_len_args() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["view.to_bytes", ["bytes.subview", ["bytes.lit", "hello"], 0, 3]]
    });
    let doc_bytes = serde_json::to_vec(&doc).expect("serialize");
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);

    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BORROW-0001")
        .expect("expected X07-BORROW-0001 diagnostic");
    assert!(
        diag.notes
            .iter()
            .any(|note| note.contains("[\"bytes.subview\", <expr>, <start>, <len>]")),
        "expected bytes.subview note to include start/len placeholders"
    );
}

#[test]
fn lint_rejects_bytes_view_escape_from_begin_scope() {
    let mut doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": [
            "begin",
            [
                "let",
                "len",
                [
                    "view.len",
                    [
                        "begin",
                        ["let", "k", ["bytes.lit", "scope"]],
                        ["bytes.view", "k"]
                    ]
                ]
            ],
            ["bytes.lit", "ok"]
        ]
    });
    let doc_bytes = serde_json::to_vec(&doc).expect("serialize");
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);

    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");
    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BORROW-0002")
        .expect("expected X07-BORROW-0002 diagnostic");
    let q = d.quickfix.as_ref().expect("expected quickfix");
    json_patch::apply_patch(&mut doc, &q.patch).expect("apply quickfix");

    let patched = serde_json::to_vec(&doc).expect("serialize patched");
    let mut patched_file = x07ast::parse_x07ast_json(&patched).expect("reparse patched");
    x07ast::canonicalize_x07ast_file(&mut patched_file);
    let patched_report = lint::lint_file(&patched_file, lint::LintOptions::default());
    assert!(
        patched_report.ok,
        "expected lint ok after patch: {:?}",
        patched_report.diagnostics
    );
}

#[test]
fn lint_warns_on_eager_bool_ops_with_trap_prone_view_ops() {
    let mut doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": [
            "if",
            [
                "&",
                ["=", ["view.len", "input"], 1],
                ["=", ["view.get_u8", "input", 0], 47]
            ],
            ["bytes.lit", "ok"],
            ["bytes.lit", "no"]
        ]
    });
    let doc_bytes = serde_json::to_vec(&doc).expect("serialize");
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);

    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(report.ok, "expected lint ok (warnings only)");
    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BOOL-0001")
        .expect("expected X07-BOOL-0001 diagnostic");
    let q = d.quickfix.as_ref().expect("expected quickfix");
    json_patch::apply_patch(&mut doc, &q.patch).expect("apply quickfix");

    let patched = serde_json::to_vec(&doc).expect("serialize patched");
    let mut patched_file = x07ast::parse_x07ast_json(&patched).expect("reparse patched");
    x07ast::canonicalize_x07ast_file(&mut patched_file);
    let patched_report = lint::lint_file(&patched_file, lint::LintOptions::default());
    assert!(
        !patched_report
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-BOOL-0001"),
        "expected X07-BOOL-0001 to be fixed: {:?}",
        patched_report.diagnostics
    );
}
