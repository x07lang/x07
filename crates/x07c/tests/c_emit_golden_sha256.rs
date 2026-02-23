use std::path::PathBuf;

use serde_json::json;
use sha2::{Digest, Sha256};

use x07_contracts::X07AST_SCHEMA_VERSION;
use x07_worlds::WorldId;
use x07c::compile::{compile_program_to_c, CompileOptions};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .to_path_buf()
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

fn entry(decls: Vec<serde_json::Value>, solve: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": decls,
        "solve": solve,
    }))
    .expect("encode x07AST entry JSON")
}

fn compile(program: &[u8], mut options: CompileOptions) -> String {
    options.arch_root = Some(repo_root());
    let program = program.to_vec();
    std::thread::Builder::new()
        .name("c_emit_golden_compile".to_string())
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            compile_program_to_c(program.as_slice(), &options).expect("program must compile")
        })
        .expect("spawn compile thread")
        .join()
        .expect("join compile thread")
}

fn pipe_json_canon_expr() -> serde_json::Value {
    json!([
        "std.stream.pipe_v1",
        [
            "std.stream.cfg_v1",
            ["chunk_max_bytes", 64],
            ["bufread_cap_bytes", 64],
            ["max_in_bytes", 64],
            ["max_out_bytes", 64],
            ["max_items", 10]
        ],
        [
            "std.stream.src.bytes_v1",
            ["std.stream.expr_v1", ["bytes.lit", "{\"b\":1,\"a\":2}"]]
        ],
        [
            "std.stream.chain_v1",
            [
                "std.stream.xf.json_canon_stream_v1",
                ["max_depth", 64],
                ["max_total_json_bytes", 64],
                ["max_object_members", 64],
                ["max_object_total_bytes", 256],
                ["emit_chunk_max_bytes", 64]
            ]
        ],
        ["std.stream.sink.collect_bytes_v1"]
    ])
}

#[test]
fn golden_sha256_solve_pure_stream_json_canon_pipe() {
    let program = entry(Vec::new(), pipe_json_canon_expr());
    let c = compile(program.as_slice(), CompileOptions::default());
    assert_eq!(
        sha256_hex(&c),
        "7c48482ffe26a6f2e4ca5918cadb59e4a1dbe6d5b3ba62c43033a422b6bbdaeb"
    );
}

#[test]
fn golden_sha256_solve_pure_async_tasks() {
    let program = entry(
        vec![json!({
            "kind": "defasync",
            "name": "main.worker",
            "params": [],
            "result": "bytes",
            "body": [
                "task.scope_v1",
                ["task.scope.cfg_v1"],
                ["begin", ["task.scope.cancel_all_v1"], ["bytes.alloc", 0]]
            ],
        })],
        json!([
            "begin",
            ["let", "t", ["main.worker"]],
            ["task.spawn", "t"],
            ["await", "t"]
        ]),
    );
    let c = compile(program.as_slice(), CompileOptions::default());
    assert_eq!(
        sha256_hex(&c),
        "67596aeb3d4c6a8be81c073bad6dda4edf8c9feec9ce4c19b47c5140a97b018d"
    );
}

#[test]
fn golden_sha256_solve_pure_contracts_runtime_trap() {
    let program = entry(
        vec![json!({
            "kind": "defn",
            "name": "main.f",
            "params": [{"name": "x", "ty": "i32"}],
            "result": "i32",
            "requires": [{"id":"r0", "expr": ["=", "x", "x"]}],
            "ensures": [{"id":"e0", "expr": ["=", "__result", "x"]}],
            "body": "x",
        })],
        json!(["begin", ["main.f", 0], ["bytes.alloc", 0]]),
    );
    let c = compile(program.as_slice(), CompileOptions::default());
    assert_eq!(
        sha256_hex(&c),
        "2b96043356b4bc08c03b7c4921df1e4a25ef9285207de3938ed4032852676865"
    );
}

#[test]
fn golden_sha256_run_os_os_fs_read_file() {
    let options = x07c::world_config::compile_options_for_world(WorldId::RunOs, Vec::new());
    let program = entry(
        Vec::new(),
        json!(["os.fs.read_file", ["bytes.lit", "hello.txt"]]),
    );
    let c = compile(program.as_slice(), options);
    assert_eq!(
        sha256_hex(&c),
        "9fcf17a8adf2526d34d0312bf1261ec12f9fa7958db9fe5547f65d3a43d0ddc2"
    );
}

#[test]
fn golden_sha256_run_os_mega_fixture() {
    let options = x07c::world_config::compile_options_for_world(WorldId::RunOs, Vec::new());
    let program = entry(
        vec![
            json!({
                "kind": "defn",
                "name": "main.f",
                "params": [{"name": "x", "ty": "i32"}],
                "result": "i32",
                "requires": [{"id":"r0", "expr": ["=", "x", "x"]}],
                "ensures": [{"id":"e0", "expr": ["=", "__result", "x"]}],
                "body": "x",
            }),
            json!({
                "kind": "defasync",
                "name": "main.worker",
                "params": [],
                "result": "bytes",
                "body": [
                    "task.scope_v1",
                    ["task.scope.cfg_v1"],
                    ["begin", ["task.scope.cancel_all_v1"], ["bytes.alloc", 0]]
                ],
            }),
        ],
        json!([
            "begin",
            ["let", "ignored", ["main.f", 0]],
            ["let", "doc", pipe_json_canon_expr()],
            ["let", "t", ["main.worker"]],
            ["task.spawn", "t"],
            ["let", "out", ["await", "t"]],
            [
                "let",
                "file",
                ["os.fs.read_file", ["bytes.lit", "hello.txt"]]
            ],
            "doc"
        ]),
    );
    let c = compile(program.as_slice(), options);
    assert_eq!(
        sha256_hex(&c),
        "aa90265a78e6a8fbdb5cf7b2cc8003fcf16a8a8a71247f669f12a6c195c0ca71"
    );
}
