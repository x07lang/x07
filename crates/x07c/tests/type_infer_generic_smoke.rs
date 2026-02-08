use serde_json::json;

use x07c::typecheck::{typecheck_file_local, TypecheckOptions};

mod typecheck_testutil;

#[test]
fn infer_generic_call_collects_rewrite() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.id",
                "type_params": [{"name": "A"}],
                "params": [{"name": "x", "ty": ["t", "A"]}],
                "result": ["t", "A"],
                "body": "x"
            }
        ],
        "solve": ["begin", ["let", "x", ["main.id", 0]], ["bytes.alloc", 0]]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report1 = typecheck_file_local(&file, &TypecheckOptions::default());
    let report2 = typecheck_file_local(&file, &TypecheckOptions::default());

    assert_eq!(report1, report2, "typecheck must be deterministic");
    assert!(
        report1.tapp_rewrites.iter().any(|r| r.callee == "main.id"),
        "expected tapp rewrite for main.id, got: {:?}",
        report1.tapp_rewrites
    );
    assert!(
        report1
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-TAPP-ELAB-0001"),
        "expected X07-TAPP-ELAB-0001, got: {:?}",
        report1.diagnostics
    );
}

#[test]
fn infer_generic_call_reports_unresolved_type_params() {
    let doc = json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.only_a",
                "type_params": [{"name": "A"}, {"name": "B"}],
                "params": [{"name": "x", "ty": ["t", "A"]}],
                "result": ["t", "A"],
                "body": "x"
            }
        ],
        "solve": ["begin", ["let", "x", ["main.only_a", 0]], ["bytes.alloc", 0]]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-TAPP-INFER-0001")
        .expect("expected X07-TAPP-INFER-0001");
    let unresolved = d
        .data
        .get("unresolved_type_params")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        unresolved.iter().any(|v| v.as_str() == Some("B")),
        "expected unresolved type params to include \"B\", got: {:?}",
        unresolved
    );
    assert!(
        !report
            .tapp_rewrites
            .iter()
            .any(|r| r.callee == "main.only_a"),
        "expected no tapp rewrite when inference fails, got: {:?}",
        report.tapp_rewrites
    );
}
