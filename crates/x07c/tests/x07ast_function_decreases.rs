use serde_json::json;

use x07_contracts::{X07AST_SCHEMA_VERSION_V0_7_0, X07AST_SCHEMA_VERSION_V0_8_0};
use x07c::diagnostics::Location;
use x07c::lint;
use x07c::typecheck::{typecheck_file_local, TypecheckOptions};
use x07c::x07ast::{
    canonicalize_x07ast_file, defn_decreases, parse_x07ast_json, x07ast_file_to_value,
};

mod typecheck_testutil;

#[test]
fn defn_decreases_roundtrips_and_canonicalizes() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.fact",
                "params": [{"name": "n", "ty": "i32"}],
                "result": "i32",
                "decreases": [{
                    "id": "n-descends",
                    "expr": "n",
                    "witness": [["bytes.lit", "ok"]]
                }],
                "body": "n"
            }
        ],
        "solve": 0,
    });

    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");
    let mut file = parse_x07ast_json(&bytes).expect("parse x07AST");
    canonicalize_x07ast_file(&mut file);

    let decreases = defn_decreases(&file, "main.fact")
        .expect("decode decreases")
        .expect("expected decreases");
    assert_eq!(decreases[0].expr.ptr(), "/decls/0/decreases/0/expr");
    assert_eq!(
        decreases[0].witness[0].ptr(),
        "/decls/0/decreases/0/witness/0"
    );

    let emitted = x07ast_file_to_value(&file);
    assert_eq!(
        emitted["decls"][0]["decreases"][0]["id"],
        json!("n-descends")
    );
    assert_eq!(emitted["decls"][0]["decreases"][0]["expr"], json!("n"));
    assert_eq!(
        emitted["decls"][0]["decreases"][0]["witness"][0],
        json!(["bytes.lit", "ok"])
    );
}

#[test]
fn defn_decreases_requires_v0_8_0() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_7_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.fact",
                "params": [],
                "result": "i32",
                "decreases": [{"expr": "n"}],
                "body": 0
            }
        ],
        "solve": 0,
    });

    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");
    let err = parse_x07ast_json(&bytes).expect_err("expected version error");
    assert_eq!(err.ptr, "/decls/0/decreases");
    assert!(
        err.message.contains(X07AST_SCHEMA_VERSION_V0_8_0),
        "expected version in error message, got: {:?}",
        err.message
    );
}

#[test]
fn defasync_decreases_are_rejected() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defasync",
                "name": "main.worker",
                "params": [],
                "result": "i32",
                "decreases": [{"expr": 1}],
                "body": 0
            }
        ],
        "solve": 0,
    });

    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");
    let err = parse_x07ast_json(&bytes).expect_err("expected defasync rejection");
    assert_eq!(err.ptr, "/decls/0/decreases");
    assert!(
        err.message.contains("defn"),
        "expected defn-only error, got: {:?}",
        err.message
    );
}

#[test]
fn lint_checks_defn_decreases() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "decreases": [{
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
    assert_eq!(ptr, "/decls/0/decreases/0/witness/0/1");
}

#[test]
fn typecheck_checks_defn_decreases_expr_type() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "decreases": [{"expr": ["bytes.lit", "x"]}],
                "body": 0
            }
        ],
        "solve": 0,
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diag| diag.code == "X07-CONTRACT-0009"),
        "expected X07-CONTRACT-0009, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn typecheck_rejects_decreases_on_non_recursive_target() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [{"name": "n", "ty": "i32"}],
                "result": "i32",
                "requires": [{"id": "r0", "expr": [">=", "n", 0]}],
                "decreases": [{"id": "d0", "expr": "n"}],
                "body": ["+", "n", 1]
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diag| diag.code == "X07-CONTRACT-0010"),
        "expected X07-CONTRACT-0010, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn typecheck_rejects_recursive_defn_without_decreases() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_8_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.fact",
                "params": [{"name": "n", "ty": "i32"}],
                "result": "i32",
                "requires": [{"id": "r0", "expr": [">=", "n", 0]}],
                "body": [
                    "if",
                    ["<=", "n", 0],
                    0,
                    ["main.fact", ["-", "n", 1]]
                ]
            }
        ],
        "solve": ["bytes.alloc", 0],
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diag| diag.code == "X07-CONTRACT-0011"),
        "expected X07-CONTRACT-0011, got: {:?}",
        report.diagnostics
    );
}
