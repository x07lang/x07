use serde_json::json;
use x07c::compile::{compile_program_to_c, CompileErrorKind, CompileOptions};

mod x07_program;

#[test]
fn compile_accepts_result_try() {
    let program = x07_program::entry(
        &[],
        vec![
            x07_program::defn(
                "main.f",
                &[("x", "i32")],
                "result_i32",
                json!([
                    "if",
                    ["<u", "x", 10],
                    ["result_i32.ok", "x"],
                    ["result_i32.err", 1]
                ]),
            ),
            x07_program::defn(
                "main.g",
                &[("x", "i32")],
                "result_i32",
                json!([
                    "begin",
                    ["let", "y", ["try", ["main.f", "x"]]],
                    ["result_i32.ok", ["+", "y", 1]]
                ]),
            ),
        ],
        json!(["bytes.alloc", 0]),
    );
    compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}

#[test]
fn compile_rejects_use_after_move_bytes() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.bad",
            &[("b", "bytes")],
            "bytes",
            json!(["begin", ["let", "moved", "b"], "b", ["bytes.alloc", 0]]),
        )],
        json!(["bytes.alloc", 0]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject use-after-move");
    assert_eq!(err.kind, CompileErrorKind::Typing);
    assert!(
        err.message.contains("use after move"),
        "unexpected error message: {}",
        err.message
    );
    assert!(
        err.message.contains("moved_ptr=/"),
        "expected moved_ptr in error message: {}",
        err.message
    );
    assert!(
        err.message.contains("ptr=/"),
        "expected ptr in error message: {}",
        err.message
    );
}

#[test]
fn compile_rejects_bytes_view_of_temporary_includes_hint_and_ptr() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.bad",
            &[("b", "bytes")],
            "bytes_view",
            json!([
                "begin",
                ["let", "_b", "b"],
                ["bytes.view", ["bytes.alloc", 0]]
            ]),
        )],
        json!(["bytes.alloc", 0]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject bytes.view of temporary");
    assert_eq!(err.kind, CompileErrorKind::Typing);
    assert!(
        err.message
            .contains("bytes.view requires an identifier owner"),
        "unexpected error message: {}",
        err.message
    );
    assert!(
        err.message
            .contains("bind the value to a local with let first"),
        "expected hint in error message: {}",
        err.message
    );
    assert!(
        err.message.contains("ptr=/"),
        "expected ptr in error message: {}",
        err.message
    );
}

#[test]
fn compile_accepts_if_with_moves_in_both_branches() {
    let program = x07_program::entry(
        &["std.bytes"],
        vec![x07_program::defn(
            "main.id",
            &[("b", "bytes")],
            "bytes",
            json!("b"),
        )],
        json!([
            "begin",
            ["let", "b", ["bytes.alloc", 0]],
            ["if", 1, ["main.id", "b"], ["std.bytes.reverse", "b"]]
        ]),
    );
    compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}

#[test]
fn compile_accepts_borrow_then_move_in_defasync() {
    let program = x07_program::entry(
        &[],
        vec![
            x07_program::defn(
                "main.len_view",
                &[("v", "bytes_view")],
                "i32",
                json!(["bytes.len", "v"]),
            ),
            x07_program::defasync(
                "main.task",
                &[("b", "bytes")],
                "bytes",
                json!([
                    "begin",
                    ["let", "n", ["main.len_view", "b"]],
                    ["let", "moved", "b"],
                    ["if", ["=", "n", -1], "moved", ["bytes.alloc", 0]]
                ]),
            ),
        ],
        json!(["bytes.alloc", 0]),
    );
    compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}
