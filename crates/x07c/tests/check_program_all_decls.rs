use serde_json::json;
use x07_contracts::X07AST_SCHEMA_VERSION;
use x07c::compile::{check_program, compile_program_to_c, CompileOptions};

#[test]
fn check_program_checks_unreachable_function_bodies() {
    // REGRESSION: `x07 check` backend-check must fail fast on latent codegen errors, even when the
    // invalid function is unreachable from `solve`.
    let doc = json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.bad",
                "params": [],
                "result": "bytes",
                "body": ["begin",
                    ["let", "x", ["bytes.lit", "a"]],
                    ["let", "y", "x"],
                    "x"
                ]
            }
        ],
        "solve": ["bytes.lit", "ok"]
    });

    let program_bytes = serde_json::to_vec(&doc).expect("serialize program");

    // The normal compilation path performs dead-code elimination, so this should succeed.
    compile_program_to_c(&program_bytes, &CompileOptions::default()).expect("compile to C");

    // The backend-check used by `x07 check` must validate all declarations.
    let err = check_program(&program_bytes, &CompileOptions::default())
        .expect_err("expected backend-check failure");
    assert!(
        err.message.contains("use after move"),
        "unexpected error: {:?}",
        err
    );
    assert!(
        err.message.contains("fn=main.bad"),
        "expected fn name in error: {:?}",
        err
    );
}
