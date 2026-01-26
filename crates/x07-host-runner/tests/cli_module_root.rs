use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

mod x07_program;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    base.join(format!("{prefix}_{pid}_{n}"))
}

fn module_file(module_id: &str, decls: Vec<serde_json::Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": "x07.x07ast@0.2.0",
        "kind": "module",
        "module_id": module_id,
        "imports": [],
        "decls": decls,
    }))
    .expect("encode x07AST module JSON")
}

#[test]
fn cli_module_root_allows_importing_user_modules() {
    let dir = temp_dir("x07_host_runner_cli_module_root");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let module_root = dir.join("module_root");
    let module_path = module_root.join("ext").join("foo.x07.json");
    std::fs::create_dir_all(module_path.parent().unwrap()).expect("create module dir");

    let module = module_file(
        "ext.foo",
        vec![
            x07_program::export(&["ext.foo.answer"]),
            x07_program::defn("ext.foo.answer", &[], "i32", json!(7)),
        ],
    );
    std::fs::write(&module_path, module).expect("write module");

    let program_path = dir.join("main.x07.json");
    let program = x07_program::entry(
        &["ext.foo"],
        json!(["codec.write_u32_le", ["ext.foo.answer"]]),
    );
    std::fs::write(&program_path, program).expect("write program");

    let compiled_out = dir.join("artifact");

    let bin = env!("CARGO_BIN_EXE_x07-host-runner");
    let ok_out = Command::new(bin)
        .arg("--program")
        .arg(&program_path)
        .arg("--world")
        .arg("solve-pure")
        .arg("--module-root")
        .arg(&module_root)
        .arg("--compiled-out")
        .arg(&compiled_out)
        .arg("--compile-only")
        .output()
        .expect("run x07-host-runner with module-root");

    assert!(
        ok_out.status.success(),
        "status={}\nstderr={}",
        ok_out.status,
        String::from_utf8_lossy(&ok_out.stderr)
    );

    let v: serde_json::Value = serde_json::from_slice(&ok_out.stdout).expect("parse runner json");
    assert_eq!(
        v.get("compile")
            .and_then(|c| c.get("ok"))
            .and_then(|ok| ok.as_bool()),
        Some(true)
    );

    let bad_out = Command::new(bin)
        .arg("--program")
        .arg(&program_path)
        .arg("--world")
        .arg("solve-pure")
        .arg("--compiled-out")
        .arg(&compiled_out)
        .arg("--compile-only")
        .output()
        .expect("run x07-host-runner without module-root");

    assert!(
        !bad_out.status.success(),
        "expected failure without --module-root"
    );

    let v: serde_json::Value = serde_json::from_slice(&bad_out.stdout).expect("parse runner json");
    assert_eq!(
        v.get("compile")
            .and_then(|c| c.get("ok"))
            .and_then(|ok| ok.as_bool()),
        Some(false)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
