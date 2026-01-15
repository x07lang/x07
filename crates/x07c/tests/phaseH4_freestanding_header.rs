use serde_json::json;
use x07_worlds::WorldId;
use x07c::compile::{compile_program_to_c, CompileErrorKind, CompileOptions};

mod x07_program;

#[test]
fn emit_c_header_requires_no_main() {
    let err = x07c::c_emit::emit_c_header(&CompileOptions::default())
        .expect_err("header emission must require emit_main=false");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
}

#[test]
fn freestanding_emits_library_api_and_no_main() {
    let program = x07_program::entry(&[], vec![], json!(["bytes.alloc", 0]));
    let options = CompileOptions {
        emit_main: false,
        freestanding: true,
        ..Default::default()
    };
    let c = compile_program_to_c(program.as_slice(), &options)
        .expect("freestanding solve-pure must compile");
    assert!(c.contains("#define X07_FREESTANDING 1"));
    assert!(c.contains("bytes_t x07_solve_v2("));
    assert!(
        !c.contains("int main(void)"),
        "freestanding output must not contain a main()"
    );

    let h = x07c::c_emit::emit_c_header(&options).expect("emit header");
    assert!(h.contains("bytes_t x07_solve_v2("));
}

#[test]
fn freestanding_is_rejected_outside_solve_pure() {
    let program = x07_program::entry(&[], vec![], json!(["bytes.alloc", 0]));
    let options = CompileOptions {
        world: WorldId::SolveFs,
        enable_fs: true,
        emit_main: false,
        freestanding: true,
        ..Default::default()
    };
    let err = compile_program_to_c(program.as_slice(), &options)
        .expect_err("freestanding must reject non-solve-pure worlds");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message.contains("only --world solve-pure"),
        "unexpected error message: {}",
        err.message
    );
}
