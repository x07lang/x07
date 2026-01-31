use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use x07_contracts::X07AST_SCHEMA_VERSION;
use x07c::compile::{compile_program_to_c, CompileErrorKind, CompileOptions};

mod x07_program;

fn write_temp_program(program: &[u8]) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let mut dir = std::env::temp_dir();
    dir.push("x07c-tests");
    dir.push(format!(
        "refactoring-1-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create temp test dir");

    let path = dir.join("program.x07.json");
    std::fs::write(&path, program).expect("write program");
    path
}

#[test]
fn compile_rejects_import_with_path_separators() {
    let program = x07_program::entry(&["/etc/passwd"], Vec::new(), json!(["bytes.alloc", 0]));
    let err = compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect_err("must reject unsafe module id");
    assert_eq!(err.kind, CompileErrorKind::Parse);
    assert!(
        err.message.contains("invalid module_id"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_is_deterministic_across_processes() {
    let program = x07_program::entry(
        &[],
        Vec::new(),
        json!([
            "begin",
            ["let", "n", ["parse.u32_dec", "input"]],
            [
                "codec.write_u32_le",
                [
                    "+",
                    ["+", ["+", "n", 1], ["+", "n", 1]],
                    ["+", ["+", "n", 2], ["+", "n", 2]]
                ]
            ]
        ]),
    );
    let program_path = write_temp_program(program.as_slice());

    let exe = env!("CARGO_BIN_EXE_x07c");

    let mut last: Option<Vec<u8>> = None;
    for _ in 0..20 {
        let out = Command::new(exe)
            .args([
                "compile",
                "--program",
                program_path.to_str().expect("program path must be UTF-8"),
                "--world",
                "solve-pure",
            ])
            .output()
            .expect("run x07c");

        assert!(
            out.status.success(),
            "x07c failed (status={}):\nstdout:\n{}\nstderr:\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );

        if let Some(prev) = &last {
            assert_eq!(
                prev,
                &out.stdout,
                "non-deterministic stdout across processes:\nprev:\n{}\nnext:\n{}",
                String::from_utf8_lossy(prev),
                String::from_utf8_lossy(&out.stdout),
            );
        }
        last = Some(out.stdout);
    }
}

#[test]
fn compile_accepts_multi_expr_defn_and_defasync_bodies() {
    let program = x07_program::entry(
        &[],
        vec![
            x07_program::defn(
                "main.f",
                &[("x", "bytes")],
                "bytes",
                json!(["begin", ["let", "n", ["bytes.len", "x"]], ["bytes1", "n"]]),
            ),
            x07_program::defasync(
                "main.g",
                &[("x", "bytes")],
                "bytes",
                json!(["begin", ["let", "n", ["bytes.len", "x"]], ["bytes1", "n"]]),
            ),
        ],
        json!(["bytes.alloc", 0]),
    );
    compile_program_to_c(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
}

#[test]
fn compile_accepts_x07ast_json_entry() {
    let program = format!(
        r#"
          {{
            "schema_version": "{X07AST_SCHEMA_VERSION}",
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [],
            "solve": ["bytes.alloc", 0]
          }}
        "#,
    );
    compile_program_to_c(program.as_bytes(), &CompileOptions::default())
        .expect("program must compile");
}

#[test]
fn compile_rejects_legacy_sexpr_source() {
    let program = "(bytes.alloc 0)\n";
    let err = compile_program_to_c(program.as_bytes(), &CompileOptions::default())
        .expect_err("must reject legacy sexpr source");
    assert_eq!(err.kind, CompileErrorKind::Parse);
    assert!(
        err.message.contains("program must be x07AST JSON"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn compile_does_not_load_legacy_sexpr_modules() {
    let program = x07_program::entry(&["foo"], Vec::new(), json!(["bytes.alloc", 0]));
    let program_path = write_temp_program(program.as_slice());
    let dir = program_path.parent().expect("program must have parent dir");

    std::fs::write(dir.join("foo.sexpr"), "(export foo.x)\n").expect("write foo.sexpr");

    let mut options = CompileOptions::default();
    options.module_roots.push(dir.to_path_buf());

    let err =
        compile_program_to_c(program.as_slice(), &options).expect_err("must not load foo.sexpr");
    assert_eq!(err.kind, CompileErrorKind::Parse);
    assert!(
        err.message.contains("unknown module: \"foo\""),
        "unexpected error message: {}",
        err.message
    );
}
