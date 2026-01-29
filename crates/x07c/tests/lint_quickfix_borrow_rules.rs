use serde_json::json;
use x07c::compile::{compile_program_to_c, CompileOptions};
use x07c::{json_patch, lint, x07ast};

mod x07_program;

fn parse_doc(src: &[u8]) -> serde_json::Value {
    serde_json::from_slice(src).expect("valid JSON")
}

#[test]
fn lint_quickfix_borrow_hoists_owner_to_stmt_scope_and_compiles() {
    let src = x07_program::entry(
        &[],
        vec![],
        json!([
            "begin",
            [
                "let",
                "out",
                ["view.to_bytes", ["bytes.view", ["bytes.lit", "hello"]]]
            ],
            "out"
        ]),
    );
    let mut doc = parse_doc(&src);

    let mut file = x07ast::parse_x07ast_json(&src).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);
    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");

    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BORROW-0001")
        .expect("expected X07-BORROW-0001 diagnostic");
    let q = d.quickfix.as_ref().expect("expected quickfix");
    json_patch::apply_patch(&mut doc, &q.patch).expect("apply quickfix patch");

    let patched = serde_json::to_vec(&doc).expect("serialize patched");
    let mut patched_file = x07ast::parse_x07ast_json(&patched).expect("reparse patched");
    x07ast::canonicalize_x07ast_file(&mut patched_file);
    let patched_report = lint::lint_file(&patched_file, lint::LintOptions::default());
    assert!(
        patched_report.ok,
        "expected lint ok after patch: {:?}",
        patched_report.diagnostics
    );

    compile_program_to_c(&patched, &CompileOptions::default())
        .expect("patched program must compile");
}

#[test]
fn lint_quickfix_borrow_is_omitted_in_if_branch_expr() {
    let src = x07_program::entry(
        &[],
        vec![],
        json!([
            "if",
            1,
            ["view.to_bytes", ["bytes.view", ["bytes.lit", "hello"]]],
            ["bytes.alloc", 0]
        ]),
    );
    let mut file = x07ast::parse_x07ast_json(&src).expect("parse x07ast");
    x07ast::canonicalize_x07ast_file(&mut file);
    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");

    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BORROW-0001")
        .expect("expected X07-BORROW-0001 diagnostic");
    assert!(d.quickfix.is_none(), "expected no quickfix for if branch");
}
