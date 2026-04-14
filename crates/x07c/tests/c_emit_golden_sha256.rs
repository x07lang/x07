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
        "e76f9fba240ab567b0c3727abb269820a806e5f3aa570bef97d23b62c8b30122"
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
        "dc85e23c979bcc3c77fa988135dd35a3303aadb707d6642f4e9d29ca21638b47"
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
        "2157fbacd9d9a0847092cb7ca5c372606c06557c704201ec5f9f1ab1dd9be0b0"
    );
}

#[test]
fn golden_sha256_run_os_os_fs_read_file() {
    let mut options = x07c::world_config::compile_options_for_world(WorldId::RunOs, Vec::new());
    options.profile_fns = false;
    let program = entry(
        Vec::new(),
        json!(["os.fs.read_file", ["bytes.lit", "hello.txt"]]),
    );
    let c = compile(program.as_slice(), options);
    assert_eq!(
        sha256_hex(&c),
        "480bf61ca7e6057d9b6ba2977e7640c94fcf7afe8f968a6f0d5056c009e9fae3"
    );
}

#[test]
fn golden_sha256_run_os_mega_fixture() {
    let mut options = x07c::world_config::compile_options_for_world(WorldId::RunOs, Vec::new());
    options.profile_fns = false;
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
        "948cda74ba5e3206e56ece7da6270d3cc5496363fdcf5678b8be3e275e36fcc3"
    );
}
