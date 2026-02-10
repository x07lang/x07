use serde_json::json;
use x07c::compile::{compile_program_to_c_with_meta, CompileOptions};

mod x07_program;

#[test]
fn ty_intrinsics_do_not_require_explicit_std_imports() {
    let read_at = json!({
        "kind": "defn",
        "name": "main.read_at",
        "type_params": [{"name": "A"}],
        "params": [
            {"name": "b", "ty": "bytes_view"},
            {"name": "off", "ty": "i32"}
        ],
        "result": ["t", "A"],
        "body": ["ty.read_le_at", ["t", "A"], "b", "off"],
    });

    let hash32 = json!({
        "kind": "defn",
        "name": "main.hash32",
        "type_params": [{"name": "A"}],
        "params": [
            {"name": "x", "ty": ["t", "A"]}
        ],
        "result": "i32",
        "body": ["ty.hash32", ["t", "A"], "x"],
    });

    let call = json!({
        "kind": "defn",
        "name": "main.call",
        "params": [],
        "result": "i32",
        "body": [
            "begin",
            ["let", "b", ["bytes.lit", "abcd"]],
            ["let", "v", ["bytes.view", "b"]],
            ["let", "x", ["tapp", "main.read_at", "u32", "v", 0]],
            ["tapp", "main.hash32", "u32", "x"]
        ],
    });

    let program = x07_program::entry(
        &[],
        vec![read_at, hash32, call],
        json!(["begin", ["main.call"], ["bytes.alloc", 0]]),
    );

    compile_program_to_c_with_meta(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}
