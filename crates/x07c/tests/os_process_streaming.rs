use serde_json::json;

use x07c::compile::{compile_program_to_c, CompileErrorKind, CompileOptions};

mod x07_program;

#[test]
fn compile_rejects_spawn_piped_in_solve_world() {
    let program = x07_program::entry(
        &[],
        Vec::new(),
        json!([
            "os.process.spawn_piped_v1",
            ["bytes.alloc", 0],
            ["bytes.alloc", 0]
        ]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("spawn_piped_v1 must be standalone-only");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message
            .contains("os.process.spawn_piped_v1 is standalone-only"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_rejects_join_exit_in_defn() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.f",
            &[("h", "i32")],
            "i32",
            json!(["os.process.join_exit_v1", "h"]),
        )],
        json!(["main.f", 0]),
    );

    let options = CompileOptions {
        world: x07_worlds::WorldId::RunOs,
        ..Default::default()
    };

    let err = compile_program_to_c(program.as_slice(), &options)
        .expect_err("join_exit_v1 must not be allowed in defn");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message
            .contains("os.process.join_exit_v1 is only allowed in solve or defasync"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_accepts_join_exit_in_solve() {
    let program = x07_program::entry(
        &[],
        Vec::new(),
        json!(["begin", ["os.process.join_exit_v1", 0], ["bytes.alloc", 0]]),
    );

    let options = CompileOptions {
        world: x07_worlds::WorldId::RunOs,
        ..Default::default()
    };

    compile_program_to_c(program.as_slice(), &options)
        .expect("join_exit_v1 must be allowed in solve in standalone worlds");
}

#[test]
fn compile_rejects_join_exit_alias_in_defn() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.f",
            &[("h", "i32")],
            "i32",
            json!(["std.os.process.join_exit_v1", "h"]),
        )],
        json!(["main.f", 0]),
    );

    let options = CompileOptions {
        world: x07_worlds::WorldId::RunOs,
        ..Default::default()
    };

    let err = compile_program_to_c(program.as_slice(), &options)
        .expect_err("std.os.process.join_exit_v1 must not be allowed in defn");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message
            .contains("os.process.join_exit_v1 is only allowed in solve or defasync"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_accepts_join_exit_alias_in_solve() {
    let program = x07_program::entry(
        &[],
        Vec::new(),
        json!([
            "begin",
            ["std.os.process.join_exit_v1", 0],
            ["bytes.alloc", 0]
        ]),
    );

    let options = CompileOptions {
        world: x07_worlds::WorldId::RunOs,
        ..Default::default()
    };

    compile_program_to_c(program.as_slice(), &options)
        .expect("std.os.process.join_exit_v1 must be allowed in solve in standalone worlds");
}

#[test]
fn compile_rejects_join_capture_alias_in_defn() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.f",
            &[("h", "i32")],
            "bytes",
            json!(["std.os.process.join_capture_v1", "h"]),
        )],
        json!(["main.f", 0]),
    );

    let options = CompileOptions {
        world: x07_worlds::WorldId::RunOs,
        ..Default::default()
    };

    let err = compile_program_to_c(program.as_slice(), &options)
        .expect_err("std.os.process.join_capture_v1 must not be allowed in defn");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message
            .contains("os.process.join_capture_v1 is only allowed in solve or defasync"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_accepts_join_capture_alias_in_solve() {
    let program = x07_program::entry(
        &[],
        Vec::new(),
        json!([
            "begin",
            ["std.os.process.join_capture_v1", 0],
            ["bytes.alloc", 0]
        ]),
    );

    let options = CompileOptions {
        world: x07_worlds::WorldId::RunOs,
        ..Default::default()
    };

    compile_program_to_c(program.as_slice(), &options)
        .expect("std.os.process.join_capture_v1 must be allowed in solve in standalone worlds");
}
