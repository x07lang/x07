use serde_json::json;

use x07c::typecheck::{typecheck_file_local, TypecheckOptions};

mod typecheck_testutil;

#[test]
fn unify_error_message_shows_expected_and_got() {
    // A `begin` whose tail is `bytes` in an `i32`-returning function is an
    // ExprCheck mismatch -> X07-TYPE-UNIFY-0001. The message must name both types
    // (it used to be a bare "unification failure").
    let doc = json!({
        "schema_version": "x07.x07ast@0.5.0",
        "kind": "module",
        "module_id": "m",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "m.f",
                "params": [],
                "result": "i32",
                "body": ["begin", ["let", "x", 0], ["bytes.lit", "hi"]]
            }
        ]
    });

    let file = typecheck_testutil::file_from_json(&doc);
    let report = typecheck_file_local(&file, &TypecheckOptions::default());

    let unify = report
        .diagnostics
        .iter()
        .find(|d| d.code == "X07-TYPE-UNIFY-0001")
        .unwrap_or_else(|| {
            panic!(
                "expected X07-TYPE-UNIFY-0001, got: {:?}",
                report.diagnostics
            )
        });
    assert!(
        unify.message.contains("expected `i32`") && unify.message.contains("got `bytes`"),
        "expected an expected/got message, got: {:?}",
        unify.message
    );
}
