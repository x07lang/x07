use serde_json::json;

use x07_contracts::{X07AST_SCHEMA_VERSION_V0_6_0, X07AST_SCHEMA_VERSION_V0_7_0};
use x07c::x07ast::{canonicalize_x07ast_file, parse_x07ast_json, x07ast_file_to_value};

#[test]
fn defasync_protocol_roundtrips_and_canonicalizes() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_7_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defasync",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "protocol": {
                    "await_invariant": [{"expr": ["=", 1, 1]}],
                    "scope_invariant": [{
                        "expr": 1,
                        "witness": [["bytes.lit", "x"]]
                    }],
                    "cancellation_ensures": [{"expr": ["=", "__result", 0]}]
                },
                "body": 0
            }
        ],
        "solve": 0,
    });

    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");
    let mut file = parse_x07ast_json(&bytes).expect("parse x07AST");
    canonicalize_x07ast_file(&mut file);

    let protocol = file.async_functions[0]
        .protocol
        .as_ref()
        .expect("expected defasync protocol");
    assert_eq!(
        protocol.await_invariant[0].expr.ptr(),
        "/decls/0/protocol/await_invariant/0/expr"
    );
    assert_eq!(
        protocol.scope_invariant[0].witness[0].ptr(),
        "/decls/0/protocol/scope_invariant/0/witness/0"
    );
    assert_eq!(
        protocol.cancellation_ensures[0].expr.ptr(),
        "/decls/0/protocol/cancellation_ensures/0/expr"
    );

    let emitted = x07ast_file_to_value(&file);
    assert_eq!(
        emitted["decls"][0]["protocol"]["await_invariant"][0]["expr"],
        json!(["=", 1, 1])
    );
    assert_eq!(
        emitted["decls"][0]["protocol"]["scope_invariant"][0]["witness"][0],
        json!(["bytes.lit", "x"])
    );
    assert_eq!(
        emitted["decls"][0]["protocol"]["cancellation_ensures"][0]["expr"],
        json!(["=", "__result", 0])
    );
}

#[test]
fn defasync_protocol_requires_v0_7_0() {
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION_V0_6_0,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defasync",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "protocol": {
                    "await_invariant": [{"expr": 1}]
                },
                "body": 0
            }
        ],
        "solve": 0,
    });

    let bytes = serde_json::to_vec(&doc).expect("encode x07AST");
    let err = parse_x07ast_json(&bytes).expect_err("expected protocol version error");
    assert_eq!(err.ptr, "/decls/0/protocol");
    assert!(
        err.message.contains(X07AST_SCHEMA_VERSION_V0_7_0),
        "expected version in error message, got: {:?}",
        err.message
    );
}
