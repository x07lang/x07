use serde_json::json;
use x07_contracts::X07AST_SCHEMA_VERSION;
use x07_worlds::WorldId;

use x07c::diagnostics::Location;
use x07c::diagnostics::QuickfixKind;
use x07c::json_patch;
use x07c::lint;
use x07c::world_config;
use x07c::x07ast;

#[test]
fn lint_rejects_undefined_type_var_in_signature() {
    let mut doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.bad",
                "type_params": [{"name":"A"}],
                "params": [{"name":"x","ty":["t","B"]}],
                "result": ["t","B"],
                "body": "x"
            }
        ],
        "solve": ["bytes.alloc", 0]
    });
    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");

    let mut file = x07ast::parse_x07ast_json(&bytes).expect("parse x07AST");
    x07ast::canonicalize_x07ast_file(&mut file);

    let opts = world_config::lint_options_for_world(WorldId::SolvePure);
    let report = lint::lint_file(&file, opts);

    assert!(!report.ok, "expected lint report to be not ok");
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-GENERICS-0001")
        .expect("expected X07-GENERICS-0001");
    let quickfix = diag.quickfix.as_ref().expect("expected quickfix");
    assert_eq!(quickfix.kind, QuickfixKind::JsonPatch);
    assert!(
        !quickfix.patch.is_empty(),
        "expected non-empty quickfix patch"
    );

    json_patch::apply_patch(&mut doc, &quickfix.patch).expect("apply quickfix");
    let bytes2 = serde_json::to_vec(&doc).expect("encode patched x07AST");
    let mut file2 = x07ast::parse_x07ast_json(&bytes2).expect("parse patched x07AST");
    x07ast::canonicalize_x07ast_file(&mut file2);

    let report2 = lint::lint_file(&file2, opts);
    assert!(report2.ok, "expected patched lint report to be ok");
}

#[test]
fn lint_warns_on_unused_type_param() {
    let mut doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.unused",
                "type_params": [{"name":"A"}],
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "body": "x"
            }
        ],
        "solve": ["bytes.alloc", 0]
    });
    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");

    let mut file = x07ast::parse_x07ast_json(&bytes).expect("parse x07AST");
    x07ast::canonicalize_x07ast_file(&mut file);

    let opts = world_config::lint_options_for_world(WorldId::SolvePure);
    let report = lint::lint_file(&file, opts);

    assert!(report.ok, "expected lint report to be ok");
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-GENERICS-0002")
        .expect("expected X07-GENERICS-0002");
    let quickfix = diag.quickfix.as_ref().expect("expected quickfix");
    assert_eq!(quickfix.kind, QuickfixKind::JsonPatch);
    assert!(
        !quickfix.patch.is_empty(),
        "expected non-empty quickfix patch"
    );

    json_patch::apply_patch(&mut doc, &quickfix.patch).expect("apply quickfix");
    let bytes2 = serde_json::to_vec(&doc).expect("encode patched x07AST");
    let mut file2 = x07ast::parse_x07ast_json(&bytes2).expect("parse patched x07AST");
    x07ast::canonicalize_x07ast_file(&mut file2);

    let report2 = lint::lint_file(&file2, opts);
    assert!(report2.ok, "expected patched lint report to be ok");
    assert!(
        !report2
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-GENERICS-0002"),
        "expected unused type param warning to be fixed, got: {:?}",
        report2.diagnostics
    );
}

#[test]
fn lint_does_not_warn_on_type_param_used_only_in_contract() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "type_params": [{"name":"A"}],
                "params": [],
                "result": "i32",
                "requires": [{"expr": ["tapp", "main.g", ["t", "A"]]}],
                "body": 0
            },
            {
                "kind": "defn",
                "name": "main.g",
                "type_params": [{"name":"T"}],
                "params": [],
                "result": "i32",
                "body": ["ty.size_bytes", ["t", "T"]]
            }
        ],
        "solve": ["bytes.alloc", 0]
    });
    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");

    let mut file = x07ast::parse_x07ast_json(&bytes).expect("parse x07AST");
    x07ast::canonicalize_x07ast_file(&mut file);

    let opts = world_config::lint_options_for_world(WorldId::SolvePure);
    let report = lint::lint_file(&file, opts);

    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-GENERICS-0002"
                && matches!(d.loc.as_ref(), Some(Location::X07Ast { ptr }) if ptr == "/decls/0/type_params/0/name")),
        "expected type param A to be treated as used, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn lint_reports_undefined_type_var_in_contract() {
    let mut doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "requires": [{"expr": ["tapp", "main.g", ["t", "B"]]}],
                "body": 0
            },
            {
                "kind": "defn",
                "name": "main.g",
                "type_params": [{"name":"T"}],
                "params": [],
                "result": "i32",
                "body": ["ty.size_bytes", ["t", "T"]]
            }
        ],
        "solve": ["bytes.alloc", 0]
    });
    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");

    let mut file = x07ast::parse_x07ast_json(&bytes).expect("parse x07AST");
    x07ast::canonicalize_x07ast_file(&mut file);

    let opts = world_config::lint_options_for_world(WorldId::SolvePure);
    let report = lint::lint_file(&file, opts);

    assert!(!report.ok, "expected lint report to be not ok");
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-GENERICS-0001")
        .expect("expected X07-GENERICS-0001");
    let Some(Location::X07Ast { ptr }) = diag.loc.as_ref() else {
        panic!("expected x07ast location, got: {:?}", diag.loc);
    };
    assert_eq!(ptr, "/decls/0/requires/0/expr/2");
    let quickfix = diag.quickfix.as_ref().expect("expected quickfix");
    assert_eq!(quickfix.kind, QuickfixKind::JsonPatch);
    assert!(
        !quickfix.patch.is_empty(),
        "expected non-empty quickfix patch"
    );

    json_patch::apply_patch(&mut doc, &quickfix.patch).expect("apply quickfix");
    let bytes2 = serde_json::to_vec(&doc).expect("encode patched x07AST");
    let mut file2 = x07ast::parse_x07ast_json(&bytes2).expect("parse patched x07AST");
    x07ast::canonicalize_x07ast_file(&mut file2);

    let report2 = lint::lint_file(&file2, opts);
    assert!(
        !report2
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-GENERICS-0001"),
        "expected undefined type var to be fixed, got: {:?}",
        report2.diagnostics
    );
    assert!(
        !report2
            .diagnostics
            .iter()
            .any(|d| d.code == "X07-GENERICS-0002"),
        "expected no unused type params after quickfix, got: {:?}",
        report2.diagnostics
    );
}
