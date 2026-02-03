use serde_json::json;
use x07_worlds::WorldId;
use x07c::compile::{compile_program_to_c, CompileErrorKind, CompileOptions};

mod x07_program;

fn run_os_options() -> CompileOptions {
    CompileOptions {
        world: WorldId::parse("run-os").expect("parse run-os world"),
        ..Default::default()
    }
}

#[test]
fn compile_rejects_extern_decls_in_solve_world() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::extern_c(
            "main.ext_add",
            "ext_add",
            &[("a", "i32"), ("b", "i32")],
            "i32",
        )],
        json!(["begin", ["main.ext_add", 1, 2], ["bytes.alloc", 0]]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("extern decl must be rejected in solve world");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message.contains("requires ffi capability"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_rejects_raw_pointer_types_in_solve_world_signatures() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.f",
            &[("p", "ptr_const_u8")],
            "i32",
            json!(0),
        )],
        json!(["begin", ["main.f", 0], ["bytes.alloc", 0]]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("raw pointer types must be rejected in solve world signatures");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message.contains("requires unsafe capability"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_rejects_unsafe_builtins_in_solve_world() {
    let program = x07_program::entry(
        &[],
        Vec::new(),
        json!([
            "begin",
            ["let", "p", ["bytes.as_ptr", "input"]],
            ["bytes.alloc", 0]
        ]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject unsafe builtins in solve world");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message
            .contains("bytes.as_ptr requires unsafe capability"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_requires_unsafe_block_for_ptr_read_u8() {
    let options = run_os_options();
    let program = x07_program::entry(
        &[],
        Vec::new(),
        json!(["bytes.alloc", ["ptr.read_u8", ["view.as_ptr", "input"]]]),
    );
    let err = compile_program_to_c(program.as_slice(), &options)
        .expect_err("ptr.read_u8 must require unsafe block");
    assert_eq!(err.kind, CompileErrorKind::Typing);
    assert!(
        err.message.contains("unsafe-required: ptr.read_u8"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_accepts_unsafe_block_for_ptr_read_u8() {
    let options = run_os_options();
    let program = x07_program::entry(
        &[],
        Vec::new(),
        json!([
            "bytes.alloc",
            ["unsafe", ["ptr.read_u8", ["view.as_ptr", "input"]]]
        ]),
    );
    compile_program_to_c(program.as_slice(), &options).expect("program must compile");
}

#[test]
fn compile_accepts_extern_call_in_unsafe_block() {
    let options = run_os_options();
    let program = x07_program::entry(
        &[],
        vec![x07_program::extern_c(
            "main.ext_add",
            "ext_add",
            &[("a", "i32"), ("b", "i32")],
            "i32",
        )],
        json!(["bytes.alloc", ["unsafe", ["main.ext_add", 1, 2]]]),
    );
    let c = compile_program_to_c(program.as_slice(), &options).expect("must compile");
    assert!(
        c.contains("extern uint32_t ext_add(uint32_t a, uint32_t b);"),
        "missing extern prototype"
    );
    assert!(c.contains("ext_add("), "missing extern call");
}

#[test]
fn compile_rejects_extern_call_outside_unsafe_block() {
    let options = run_os_options();
    let program = x07_program::entry(
        &[],
        vec![x07_program::extern_c(
            "main.ext_add",
            "ext_add",
            &[("a", "i32"), ("b", "i32")],
            "i32",
        )],
        json!(["bytes.alloc", ["main.ext_add", 1, 2]]),
    );
    let err = compile_program_to_c(program.as_slice(), &options)
        .expect_err("extern calls must require unsafe block");
    assert_eq!(err.kind, CompileErrorKind::Typing);
    assert!(
        err.message.contains("unsafe-required: main.ext_add"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_accepts_async_unsafe_ops() {
    let options = run_os_options();
    let program = x07_program::entry(
        &[],
        vec![x07_program::defasync(
            "main.a",
            &[],
            "bytes",
            json!([
                "bytes.alloc",
                ["unsafe", ["ptr.read_u8", ["view.as_ptr", "input"]]]
            ]),
        )],
        json!(["await", ["main.a"]]),
    );
    compile_program_to_c(program.as_slice(), &options).expect("async program must compile");
}

#[test]
fn compile_accepts_async_ptr_cast_and_addr_of() {
    let options = run_os_options();
    let program = x07_program::entry(
        &[],
        vec![x07_program::defasync(
            "main.b",
            &[],
            "bytes",
            json!([
                "begin",
                ["let", "p", ["addr_of", "input"]],
                ["let", "q", ["unsafe", ["ptr.cast", "ptr_const_u8", "p"]]],
                ["bytes.alloc", 0]
            ]),
        )],
        json!(["await", ["main.b"]]),
    );
    compile_program_to_c(program.as_slice(), &options)
        .expect("async ptr.cast/addr_of program must compile");
}

#[test]
fn compile_rejects_extern_param_non_ffi_type() {
    let options = run_os_options();
    let program = x07_program::entry(
        &[],
        vec![x07_program::extern_c(
            "main.bad",
            "bad",
            &[("b", "bytes")],
            "i32",
        )],
        json!(["bytes.alloc", 0]),
    );
    let err = compile_program_to_c(program.as_slice(), &options)
        .expect_err("extern decl must reject non-ffi param types");
    assert_eq!(err.kind, CompileErrorKind::Parse);
    assert!(
        err.message.contains("extern param has unsupported type"),
        "unexpected error message: {}",
        err.message
    );
}
