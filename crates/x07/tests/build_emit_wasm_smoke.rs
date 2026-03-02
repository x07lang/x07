use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn x07_build_emit_wasm_smoke() {
    let root = workspace_root();
    let dir = root.join("target/tmp_build_emit_wasm_smoke");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("create tmp project dir");

    std::fs::write(
        dir.join("x07.json"),
        r#"{
  "schema_version": "x07.project@0.3.0",
  "world": "solve-pure",
  "entry": "src/main.x07.json",
  "module_roots": ["src"],
  "lockfile": null,
  "dependencies": []
}
"#,
    )
    .expect("write x07.json");

    std::fs::write(
        dir.join("x07.lock.json"),
        r#"{
  "schema_version": "x07.lock@0.3.0",
  "dependencies": []
}
"#,
    )
    .expect("write x07.lock.json");

    std::fs::write(
        dir.join("src/main.x07.json"),
        r#"{"schema_version":"x07.x07ast@0.5.0","kind":"entry","module_id":"main","imports":[],"decls":[],"solve":["view.to_bytes","input"]}"#,
    )
    .expect("write main.x07.json");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out_c = dir.join("out.c");
    let out_wasm = dir.join("out.wasm");

    let out = Command::new(exe)
        .args([
            "build",
            "--project",
            dir.join("x07.json").to_str().unwrap(),
            "--out",
            out_c.to_str().unwrap(),
            "--freestanding",
            "--emit-wasm",
            out_wasm.to_str().unwrap(),
            "--wasm-initial-memory-bytes",
            "131072",
            "--wasm-max-memory-bytes",
            "131072",
            "--wasm-no-growable-memory",
        ])
        .output()
        .expect("run x07 build");

    if !out.status.success() {
        panic!(
            "x07 build failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let wasm = std::fs::read(&out_wasm).expect("read out.wasm");
    assert!(wasm.starts_with(b"\0asm"), "expected wasm magic header");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("validate wasm");
}
