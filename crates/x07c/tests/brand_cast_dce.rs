use serde_json::json;
use x07_contracts::X07AST_SCHEMA_VERSION;
use x07c::compile::{compile_program_to_c, CompileOptions};

#[test]
fn dead_code_elimination_keeps_brand_validator_symbols() {
    let program = serde_json::to_vec(&json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.validator_v1",
                "params": [{"name": "doc", "ty": "bytes_view"}],
                "result": "result_i32",
                "body": ["result_i32.ok", 0],
            },
            {
                "kind": "defn",
                "name": "main.cast_v1",
                "params": [{"name": "doc", "ty": "bytes_view"}],
                "result": "result_bytes_view",
                "body": ["std.brand.cast_view_v1", "test.brand_v1", "main.validator_v1", "doc"],
            }
        ],
        "solve": [
            "begin",
            ["let", "b", ["bytes.lit", "x"]],
            ["let", "r", ["main.cast_v1", ["bytes.view", "b"]]],
            [
                "if",
                ["result_bytes_view.is_ok", "r"],
                [
                    "view.to_bytes",
                    [
                        "result_bytes_view.unwrap_or",
                        "r",
                        ["view.slice", ["bytes.view", "b"], 0, 0]
                    ]
                ],
                ["bytes.alloc", 0]
            ]
        ],
    }))
    .expect("encode x07AST entry JSON");

    let options = CompileOptions::default();
    compile_program_to_c(program.as_slice(), &options).expect("compile ok");
}
