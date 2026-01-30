use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;

use x07c::compile::{compile_program_to_c, CompileErrorKind, CompileOptions};

mod x07_program;

fn write_temp_file(rel: &Path, contents: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let mut root = std::env::temp_dir();
    root.push("x07c-tests");
    root.push(format!(
        "internal-only-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create temp dirs");
    std::fs::write(&path, contents).expect("write temp file");
    root
}

#[test]
fn compile_rejects_internal_only_builtins_in_entry_program() {
    let program = x07_program::entry(&[], Vec::new(), json!(["set_u32.dump_u32le", 0]));
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject internal-only builtin in entry program");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message
            .contains("internal-only builtin is not allowed here: set_u32.dump_u32le"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_rejects_internal_only_builtins_in_filesystem_modules() {
    let module = r#"{
  "schema_version":"x07.x07ast@0.2.0",
  "kind":"module",
  "module_id":"app.internal",
  "imports":[],
  "decls":[
    {"kind":"export","names":["app.internal.f"]},
    {"kind":"defn","name":"app.internal.f","params":[{"name":"h","ty":"i32"}],"result":"bytes","body":["set_u32.dump_u32le","h"]}
  ]
}
"#;

    let module_root = write_temp_file(Path::new("app/internal.x07.json"), module);

    let mut options = CompileOptions::default();
    options.module_roots.push(module_root);

    let program = x07_program::entry(&["app.internal"], Vec::new(), json!(["app.internal.f", 0]));
    let err = compile_program_to_c(program.as_slice(), &options)
        .expect_err("must reject internal-only builtin in filesystem module");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message
            .contains("internal-only builtin is not allowed here: set_u32.dump_u32le"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_rejects_internal_only_builtins_in_decl_body() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.f",
            &[("h", "i32")],
            "bytes",
            json!(["set_u32.dump_u32le", "h"]),
        )],
        json!(["main.f", 0]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject internal-only builtins in user-defined functions");
    assert_eq!(err.kind, CompileErrorKind::Unsupported);
    assert!(
        err.message
            .contains("internal-only builtin is not allowed here: set_u32.dump_u32le"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_rejects_reserved_stream_pipe_helper_names() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defn(
            "main.__std_stream_pipe_v1_bad",
            &[],
            "bytes",
            json!(["bytes.alloc", 0]),
        )],
        json!(["bytes.alloc", 0]),
    );
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject reserved helper function name");
    assert_eq!(err.kind, CompileErrorKind::Parse);
    assert!(
        err.message.contains("reserved function name"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_accepts_stream_pipe_helpers_that_use_internal_only_builtins() {
    let program = x07_program::entry(
        &[],
        vec![x07_program::defasync(
            "main.mapper",
            &[("ctx", "bytes_view"), ("item", "bytes")],
            "bytes",
            json!(["begin", "ctx", "item"]),
        )],
        json!([
            "std.stream.pipe_v1",
            [
                "std.stream.cfg_v1",
                ["chunk_max_bytes", 64],
                ["bufread_cap_bytes", 64],
                ["max_in_bytes", 1024],
                ["max_out_bytes", 1024],
                ["max_items", 100]
            ],
            [
                "std.stream.src.bytes_v1",
                ["std.stream.expr_v1", ["bytes.lit", "a\nb\nc\n"]]
            ],
            [
                "std.stream.chain_v1",
                [
                    "std.stream.xf.split_lines_v1",
                    ["std.stream.expr_v1", 10],
                    ["std.stream.expr_v1", 128]
                ],
                [
                    "std.stream.xf.par_map_stream_v1",
                    ["max_inflight", 2],
                    ["max_item_bytes", 64],
                    ["mapper_defasync", ["std.stream.fn_v1", "main.mapper"]]
                ]
            ],
            ["std.stream.sink.collect_bytes_v1"]
        ]),
    );
    compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}
