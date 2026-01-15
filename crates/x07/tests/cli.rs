use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use x07_contracts::X07TEST_SCHEMA_VERSION;

fn repo_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn run_x07(args: &[&str]) -> std::process::Output {
    let exe = env!("CARGO_BIN_EXE_x07");
    Command::new(exe).args(args).output().expect("run x07")
}

fn parse_json_stdout(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).expect("parse stdout JSON")
}

fn write_bytes(path: &PathBuf, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, bytes).expect("write file");
}

#[test]
fn x07_test_smoke_suite() {
    let root = repo_root();
    let manifest = root.join("tests/tests.json");
    assert!(manifest.is_file(), "missing {}", manifest.display());

    // Full run should pass (including expected-failure demo).
    let out = run_x07(&["test", "--manifest", manifest.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["summary"]["passed"], 2);
    assert_eq!(v["summary"]["failed"], 0);
    assert_eq!(v["summary"]["errors"], 0);
    assert_eq!(v["summary"]["xfail_failed"], 1);

    let tests = v["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 3);
    let ids: Vec<&str> = tests
        .iter()
        .map(|t| t["id"].as_str().expect("test.id"))
        .collect();
    assert_eq!(
        ids,
        vec![
            "smoke/fs_read_hello",
            "smoke/pure_i32_eq",
            "smoke/pure_xfail_demo"
        ]
    );

    // --no-run compiles all tests and never runs.
    let out = run_x07(&["test", "--manifest", manifest.to_str().unwrap(), "--no-run"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["summary"]["compile_failures"], 0);
    for t in v["tests"].as_array().expect("tests[]") {
        assert_eq!(t["status"], "skip");
        assert!(t.get("compile").is_some(), "missing compile section");
        assert!(t.get("run").is_none(), "unexpected run section");
    }

    // Missing manifest yields a stable non-zero exit and a report.
    let missing_manifest = root.join("target/tmp_missing_tests.json");
    if missing_manifest.exists() {
        std::fs::remove_file(&missing_manifest).expect("remove old tmp file");
    }
    let out = run_x07(&["test", "--manifest", missing_manifest.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(12));
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);

    // Parallel requires explicit opt-in to non-fail-fast mode.
    let out = run_x07(&[
        "test",
        "--manifest",
        manifest.to_str().unwrap(),
        "--jobs",
        "2",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--jobs >1 requires --no-fail-fast"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn x07_cli_spec_check_ok_and_fmt_inserts_help_version() {
    let root = repo_root();
    let spec_path = root.join("target/tmp_cli_specrows_valid.json");
    let spec_json = r#"{"schema_version":"x07cli.specrows@0.1.0","app":{"name":"mytool","version":"0.1.0"},"rows":[["root","flag","-v","--verbose","verbose","Increase verbosity"]]}"#;
    write_bytes(&spec_path, spec_json.as_bytes());

    let out = run_x07(&["cli", "spec", "check", "--in", spec_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["diagnostics_count"], 0);
    assert_eq!(v["diagnostics"].as_array().unwrap().len(), 0);

    let out = run_x07(&["cli", "spec", "fmt", "--in", spec_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let canon: Value = serde_json::from_slice(&out.stdout).expect("parse fmt stdout JSON");
    let rows = canon["rows"].as_array().expect("rows[]");
    assert!(
        rows.iter()
            .any(|r| r.get(1).and_then(Value::as_str) == Some("help")),
        "expected implied help row"
    );
    assert!(
        rows.iter()
            .any(|r| r.get(1).and_then(Value::as_str) == Some("version")),
        "expected implied version row"
    );
}

#[test]
fn x07_cli_spec_check_schema_error_is_reported() {
    let root = repo_root();
    let spec_path = root.join("target/tmp_cli_specrows_schema_err.json");
    let spec_json = r#"{"schema_version":"x07cli.specrows@0.1.0","app":{"name":"mytool"},"rows":[["root","flag","-v","--verbose","verbose","Increase verbosity"]]}"#;
    write_bytes(&spec_path, spec_json.as_bytes());

    let out = run_x07(&["cli", "spec", "check", "--in", spec_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert!(v["diagnostics_count"].as_u64().unwrap() > 0);
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(
        diags
            .iter()
            .any(|d| d.get("code").and_then(Value::as_str) == Some("ECLI_SCHEMA_INVALID")),
        "expected ECLI_SCHEMA_INVALID diag"
    );
}

#[test]
fn x07_cli_spec_compile_writes_bytes() {
    let root = repo_root();
    let spec_path = root.join("target/tmp_cli_specrows_compile_ok.json");
    let out_path = root.join("target/tmp_cli_specrows_compile_ok.bin");
    if out_path.exists() {
        std::fs::remove_file(&out_path).expect("remove old out");
    }

    let spec_json = r#"{"schema_version":"x07cli.specrows@0.1.0","app":{"name":"mytool","version":"0.1.0"},"rows":[["root","flag","-v","--verbose","verbose","Increase verbosity"]]}"#;
    write_bytes(&spec_path, spec_json.as_bytes());

    let out = run_x07(&[
        "cli",
        "spec",
        "compile",
        "--in",
        spec_path.to_str().unwrap(),
        "--out",
        out_path.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["out"], out_path.to_str().unwrap());

    let compiled = std::fs::read(&out_path).expect("read compiled specbin");
    assert!(!compiled.is_empty(), "expected non-empty specbin");
    let sha = report["sha256"].as_str().expect("sha256");
    assert_eq!(sha.len(), 64, "expected sha256 hex");
}
