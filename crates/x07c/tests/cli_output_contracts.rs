use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use x07_contracts::X07C_REPORT_SCHEMA_VERSION;

mod x07_program;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    base.join(format!("{prefix}_{pid}_{n}"))
}

#[test]
fn cli_lint_report_json_is_stable() {
    let dir = temp_dir("x07c_cli_report_json");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let ok_path = dir.join("ok.x07.json");
    let ok_program = x07_program::entry(&[], Vec::new(), json!(["bytes.alloc", 0]));
    std::fs::write(&ok_path, ok_program).expect("write ok program");

    let bin = env!("CARGO_BIN_EXE_x07c");
    let ok_out = Command::new(bin)
        .arg("lint")
        .arg("--input")
        .arg(&ok_path)
        .arg("--world")
        .arg("solve-pure")
        .arg("--report-json")
        .output()
        .expect("run x07c lint --report-json");

    assert!(
        ok_out.status.success(),
        "status={}\nstderr={}",
        ok_out.status,
        String::from_utf8_lossy(&ok_out.stderr)
    );

    let v: serde_json::Value = serde_json::from_slice(&ok_out.stdout).expect("parse report json");
    assert_eq!(
        v.get("schema_version").and_then(|s| s.as_str()),
        Some(X07C_REPORT_SCHEMA_VERSION)
    );
    assert_eq!(v.get("command").and_then(|s| s.as_str()), Some("lint"));
    assert_eq!(v.get("ok").and_then(|b| b.as_bool()), Some(true));
    assert_eq!(v.get("diagnostics_count").and_then(|n| n.as_u64()), Some(0));
    assert_eq!(v.get("exit_code").and_then(|n| n.as_u64()), Some(0));

    let bad_path = dir.join("bad.x07.json");
    let bad_program = x07_program::entry(&["std.os.env"], Vec::new(), json!(["bytes.alloc", 0]));
    std::fs::write(&bad_path, bad_program).expect("write bad program");

    let bad_out = Command::new(bin)
        .arg("lint")
        .arg("--input")
        .arg(&bad_path)
        .arg("--world")
        .arg("solve-pure")
        .arg("--report-json")
        .output()
        .expect("run x07c lint --report-json (bad)");

    assert!(
        !bad_out.status.success(),
        "expected non-zero exit for lint errors"
    );
    assert_eq!(bad_out.status.code(), Some(1));

    let v: serde_json::Value = serde_json::from_slice(&bad_out.stdout).expect("parse report json");
    assert_eq!(v.get("ok").and_then(|b| b.as_bool()), Some(false));
    assert_eq!(v.get("exit_code").and_then(|n| n.as_u64()), Some(1));
    assert!(
        v.get("diagnostics_count")
            .and_then(|n| n.as_u64())
            .unwrap_or(0)
            > 0
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_specrows_is_json() {
    let bin = env!("CARGO_BIN_EXE_x07c");
    let out = Command::new(bin)
        .arg("--cli-specrows")
        .output()
        .expect("run x07c --cli-specrows");

    assert!(
        out.status.success(),
        "status={}\nstderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("parse specrows json");
    assert_eq!(
        v.get("schema_version").and_then(|s| s.as_str()),
        Some("x07cli.specrows@0.1.0")
    );
    assert!(v.get("rows").is_some_and(|r| r.is_array()));
}
