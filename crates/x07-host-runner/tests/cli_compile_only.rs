use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use x07_contracts::X07_HOST_RUNNER_REPORT_SCHEMA_VERSION;

mod x07_program;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    base.join(format!("{prefix}_{pid}_{n}"))
}

#[test]
fn cli_compile_only_skips_solve() {
    let dir = temp_dir("x07_host_runner_cli_compile_only");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let program_path = dir.join("trap.x07.json");
    let program = x07_program::entry(
        &[],
        json!(["codec.write_u32_le", ["bytes.get_u8", "input", 0]]),
    );
    std::fs::write(&program_path, program).expect("write program");

    let compiled_out = dir.join("artifact");

    let bin = env!("CARGO_BIN_EXE_x07-host-runner");
    let out = Command::new(bin)
        .arg("--program")
        .arg(&program_path)
        .arg("--world")
        .arg("solve-pure")
        .arg("--compiled-out")
        .arg(&compiled_out)
        .arg("--compile-only")
        .output()
        .expect("run x07-host-runner");

    assert!(
        out.status.success(),
        "status={}\nstderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(compiled_out.is_file(), "compiled artifact missing");

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("parse runner json");
    assert_eq!(
        v.get("schema_version").and_then(|s| s.as_str()),
        Some(X07_HOST_RUNNER_REPORT_SCHEMA_VERSION)
    );
    assert_eq!(v.get("exit_code").and_then(|n| n.as_u64()), Some(0));
    assert_eq!(v.get("mode").and_then(|m| m.as_str()), Some("compile"));
    assert_eq!(
        v.get("compile")
            .and_then(|c| c.get("ok"))
            .and_then(|ok| ok.as_bool()),
        Some(true)
    );
    assert!(v.get("solve").is_some_and(|s| s.is_null()));

    let _ = std::fs::remove_dir_all(&dir);
}
