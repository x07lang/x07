use serde_json::json;
use x07_worlds::WorldId;

use x07c::diagnostics::{PatchOp, QuickfixKind, Severity, Stage};
use x07c::{json_patch, lint, world_config, x07ast};

#[test]
fn lint_quickfix_inserts_tapp_for_inferred_generic_call() {
    let mut doc = json!({
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
        "solve": ["main.id", ["bytes.lit", "hello"]]
    });

    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");
    let mut file = x07ast::parse_x07ast_json(&bytes).expect("parse x07AST");
    x07ast::canonicalize_x07ast_file(&mut file);

    let opts = world_config::lint_options_for_world(WorldId::SolvePure);
    let report = lint::lint_file(&file, opts);

    assert!(!report.ok, "expected lint report to be not ok");
    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-TAPP-ELAB-0001")
        .expect("expected X07-TAPP-ELAB-0001");

    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.stage, Stage::Rewrite);

    let q = d.quickfix.as_ref().expect("expected quickfix");
    assert_eq!(q.kind, QuickfixKind::JsonPatch);
    assert_eq!(q.patch.len(), 2, "expected [test, replace] patch ops");
    assert!(
        matches!(q.patch[0], PatchOp::Test { .. }),
        "expected first op to be test, got: {:?}",
        q.patch[0]
    );
    assert!(
        matches!(q.patch[1], PatchOp::Replace { .. }),
        "expected second op to be replace, got: {:?}",
        q.patch[1]
    );

    json_patch::apply_patch(&mut doc, &q.patch).expect("apply quickfix");

    let solve = doc
        .get("solve")
        .and_then(|v| v.as_array())
        .expect("solve array");
    assert_eq!(solve.first().and_then(|v| v.as_str()), Some("tapp"));
    assert_eq!(solve.get(1).and_then(|v| v.as_str()), Some("main.id"));
    assert_eq!(
        solve
            .get(2)
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str()),
        Some("tys")
    );

    let bytes2 = serde_json::to_vec(&doc).expect("encode patched x07AST");
    let mut file2 = x07ast::parse_x07ast_json(&bytes2).expect("parse patched x07AST");
    x07ast::canonicalize_x07ast_file(&mut file2);

    let report2 = lint::lint_file(&file2, opts);
    assert!(report2.ok, "expected patched lint report to be ok");
    assert!(
        !report2
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-TAPP-ELAB-0001"),
        "expected tapp elab diag to be fixed, got: {:?}",
        report2.diagnostics
    );
}
