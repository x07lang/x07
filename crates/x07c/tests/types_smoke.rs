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
fn compile_accepts_try_result_bytes_view() {
    let program = x07_program::entry(
        &[],
        vec![
            x07_program::defn(
                "main.validate",
                &[("v", "bytes_view")],
                "result_i32",
                json!(["result_i32.ok", 0]),
            ),
            x07_program::defn(
                "main.cast",
                &[("v", "bytes_view")],
                "result_bytes_view",
                json!([
                    "std.brand.cast_view_v1",
                    "main.brand_v1",
                    "main.validate",
                    "v"
                ]),
            ),
            x07_program::defn(
                "main.use_try",
                &[("v", "bytes_view")],
                "result_i32",
                json!([
                    "begin",
                    ["let", "r", ["main.cast", "v"]],
                    ["let", "ok_view", ["try", "r"]],
                    ["result_i32.ok", ["view.len", "ok_view"]]
                ]),
            ),
        ],
        json!(["bytes.alloc", 0]),
    );
    compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}

#[test]
fn compile_rejects_move_while_borrowed_result_bytes_view() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.bad",
            &[("b", "bytes")],
            "bytes",
            json!([
                "begin",
                ["let", "r", ["result_bytes_view.ok", ["bytes.view", "b"]]],
                ["let", "moved", "b"],
                "moved"
            ]),
        )],
        json!(["bytes.alloc", 0]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject move while borrowed");
    assert_eq!(err.kind, CompileErrorKind::Typing);
    assert!(
        err.message.contains("move while borrowed"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_rejects_option_bytes_view_unwrap_or_borrow_mismatch() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.bad",
            &[],
            "i32",
            json!([
                "begin",
                ["let", "a", ["bytes.alloc", 1]],
                ["let", "b", ["bytes.alloc", 1]],
                [
                    "let",
                    "opt",
                    ["option_bytes_view.some", ["bytes.view", "a"]]
                ],
                ["let", "def", ["bytes.view", "b"]],
                ["bytes.len", ["option_bytes_view.unwrap_or", "opt", "def"]]
            ]),
        )],
        json!(["bytes.alloc", 0]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject borrow mismatch");
    assert_eq!(err.kind, CompileErrorKind::Typing);
    assert!(
        err.message.contains("single borrow source"),
        "unexpected error message: {}",
        err.message
    );
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

#[test]
fn compile_accepts_task_scope_cancel_all_in_defasync() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defasync(
            "main.f",
            &[],
            "bytes",
            json!([
                "task.scope_v1",
                ["task.scope.cfg_v1"],
                ["begin", ["task.scope.cancel_all_v1"], ["bytes.alloc", 0]]
            ]),
        )],
        json!(["bytes.alloc", 0]),
    );
    compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}
