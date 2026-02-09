use serde_json::json;

use x07c::diagnostics::Location;
use x07c::lint;

mod typecheck_testutil;

#[test]
fn lint_checks_contract_expr_and_witness() {
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
                "requires": [
                    {
                        "expr": ["i32.eq", ["view.len", ["bytes.view", ["bytes.lit", "hi"]]], 0]
                    },
                    {
                        "expr": 1,
                        "witness": [["bytes.view", ["bytes.lit", "hi"]]]
                    }
                ],
                "body": 0
            }
        ],
        "solve": 0,
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");

    let mut borrow_ptrs: Vec<String> = Vec::new();
    for d in &report.diagnostics {
        if d.code != "X07-BORROW-0001" {
            continue;
        }
        let Some(Location::X07Ast { ptr }) = d.loc.as_ref() else {
            panic!("expected X07Ast location for X07-BORROW-0001: {d:?}");
        };
        borrow_ptrs.push(ptr.clone());
    }

    assert!(
        borrow_ptrs
            .iter()
            .any(|p| p == "/decls/0/requires/0/expr/1/1/1"),
        "expected borrow error in contract expr, got: {borrow_ptrs:?}"
    );
    assert!(
        borrow_ptrs
            .iter()
            .any(|p| p == "/decls/0/requires/1/witness/0/1"),
        "expected borrow error in contract witness, got: {borrow_ptrs:?}"
    );
}

#[test]
fn lint_checks_contracts_in_defasync() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defasync",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "requires": [{
                    "expr": 1,
                    "witness": [["bytes.view", ["bytes.lit", "hi"]]]
                }],
                "body": 0
            }
        ],
        "solve": 0,
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = lint::lint_file(&file, lint::LintOptions::default());
    assert!(!report.ok, "expected lint errors");

    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-BORROW-0001")
        .expect("expected X07-BORROW-0001 diagnostic");
    let Some(Location::X07Ast { ptr }) = d.loc.as_ref() else {
        panic!("expected X07Ast location for X07-BORROW-0001: {d:?}");
    };
    assert_eq!(ptr, "/decls/0/requires/0/witness/0/1");
}
