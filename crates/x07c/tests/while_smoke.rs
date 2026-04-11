use serde_json::json;

use x07c::compile::{compile_program_to_c, CompileOptions};

mod x07_program;

#[test]
fn compile_accepts_while_in_solve() {
    let solve = json!([
        "begin",
        ["let", "i", 0],
        ["while", ["<", "i", 3], ["set0", "i", ["+", "i", 1]]],
        ["bytes.alloc", "i"]
    ]);
    let program = x07_program::entry(&[], vec![], solve);
    let c = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
    assert!(
        c.contains("while ("),
        "expected C emission to contain a while loop, got:\n{}",
        c
    );
}

#[test]
fn compile_accepts_while_in_defasync() {
    let decls = vec![x07_program::defasync(
        "main.worker",
        &[],
        "i32",
        json!([
            "begin",
            ["let", "i", 0],
            ["while", ["<", "i", 3], ["set0", "i", ["+", "i", 1]]],
            "i"
        ]),
    )];
    let program = x07_program::entry(&[], decls, json!(["bytes.alloc", 0]));
    compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("async program must compile");
}
