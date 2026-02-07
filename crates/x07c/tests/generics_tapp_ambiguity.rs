use serde_json::json;
use x07c::compile::{compile_program_to_c_with_meta, CompileOptions};

mod x07_program;

#[test]
fn tapp_parses_inline_type_vars_for_arity_2() {
    let pick_first = json!({
        "kind": "defn",
        "name": "main.pick_first",
        "type_params": [{"name": "A"}, {"name": "B"}],
        "params": [
            {"name": "a", "ty": ["t", "A"]},
            {"name": "b", "ty": ["t", "B"]}
        ],
        "result": ["t", "A"],
        "body": "a",
    });

    let outer = json!({
        "kind": "defn",
        "name": "main.outer",
        "type_params": [{"name": "K"}, {"name": "V"}],
        "params": [
            {"name": "k", "ty": ["t", "K"]},
            {"name": "v", "ty": ["t", "V"]}
        ],
        "result": ["t", "K"],
        "body": ["tapp", "main.pick_first", ["t", "K"], ["t", "V"], "k", "v"],
    });

    let test = json!({
        "kind": "defn",
        "name": "main.test",
        "params": [],
        "result": "i32",
        "body": ["tapp", "main.outer", "i32", "i32", 1, 2],
    });

    let program = x07_program::entry(
        &[],
        vec![pick_first, outer, test],
        json!(["begin", ["main.test"], ["bytes.alloc", 0]]),
    );

    let out = compile_program_to_c_with_meta(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
    let mono_map = out.mono_map.expect("mono map must be emitted");

    assert!(
        mono_map.items.iter().any(|it| {
            it.generic == "main.outer"
                && it.type_args == vec![json!("i32"), json!("i32")]
                && it
                    .specialized
                    .starts_with("main.outer__x07_mono_v1__i32__i32__h")
        }),
        "missing specialization for main.outer<i32,i32>: {:?}",
        mono_map.items
    );
    assert!(
        mono_map.items.iter().any(|it| {
            it.generic == "main.pick_first"
                && it.type_args == vec![json!("i32"), json!("i32")]
                && it
                    .specialized
                    .starts_with("main.pick_first__x07_mono_v1__i32__i32__h")
        }),
        "missing specialization for main.pick_first<i32,i32>: {:?}",
        mono_map.items
    );
}
