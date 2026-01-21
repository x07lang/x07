use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;
use x07_contracts::{
    X07TEST_SCHEMA_VERSION, X07_OS_RUNNER_REPORT_SCHEMA_VERSION,
    X07_POLICY_INIT_REPORT_SCHEMA_VERSION,
};

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

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

fn run_x07_in_dir(dir: &PathBuf, args: &[&str]) -> std::process::Output {
    let exe = env!("CARGO_BIN_EXE_x07");
    Command::new(exe)
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run x07")
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

fn fresh_tmp_dir(root: &std::path::Path, name: &str) -> PathBuf {
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    root.join("target").join(format!("{name}_{pid}_{n}"))
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
    assert_eq!(v["summary"]["passed"], 5);
    assert_eq!(v["summary"]["failed"], 0);
    assert_eq!(v["summary"]["errors"], 0);
    assert_eq!(v["summary"]["xfail_failed"], 1);

    let tests = v["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 6);
    let ids: Vec<&str> = tests
        .iter()
        .map(|t| t["id"].as_str().expect("test.id"))
        .collect();
    assert_eq!(
        ids,
        vec![
            "smoke/fs_read_hello",
            "smoke/full_fs_rr_kv",
            "smoke/kv_get_pong",
            "smoke/pure_i32_eq",
            "smoke/pure_xfail_demo",
            "smoke/rr_fetch_pong"
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
fn x07_init_creates_project_skeleton() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_init_project");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["--init"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["command"], "init");

    for rel in [
        "x07.json",
        "x07.lock.json",
        "src/app.x07.json",
        "src/main.x07.json",
        ".gitignore",
    ] {
        assert!(dir.join(rel).is_file(), "missing {}", rel);
    }
    assert!(!dir.join("x07-package.json").exists());

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--project", "x07.json", "--check"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);

    let out = run_x07_in_dir(&dir, &["--init"]);
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "X07INIT_EXISTS");

    let dir2 = fresh_tmp_dir(&root, "tmp_x07_init_package");
    if dir2.exists() {
        std::fs::remove_dir_all(&dir2).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir2).expect("create tmp dir");

    let out = run_x07_in_dir(&dir2, &["--init", "--package"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir2.join("x07-package.json").is_file());
}

#[test]
fn x07_pkg_add_updates_project_manifest() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_add");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["--init"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(&dir, &["pkg", "add", "ext-hex-rs@0.1.0"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["command"], "pkg.add");

    let doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    let deps = doc["dependencies"].as_array().expect("dependencies[]");
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0]["name"], "ext-hex-rs");
    assert_eq!(deps[0]["version"], "0.1.0");
    assert_eq!(deps[0]["path"], ".x07/deps/ext-hex-rs/0.1.0");

    let out = run_x07_in_dir(&dir, &["pkg", "add", "ext-hex-rs@0.1.0"]);
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "X07PKG_DEP_EXISTS");
}

#[test]
fn x07_pkg_pack_includes_ffi_dir() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_pack_ffi");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let package_dir = root.join("packages/ext/x07-ext-curl-c/0.1.3");
    assert!(
        package_dir.join("ffi/curl_shim.c").is_file(),
        "missing fixture file"
    );

    let out_path = dir.join("ext-curl-c-0.1.3.x07pkg");
    let out = run_x07(&[
        "pkg",
        "pack",
        "--package",
        package_dir.to_str().unwrap(),
        "--out",
        out_path.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);

    let bytes = std::fs::read(&out_path).expect("read archive bytes");
    let unpack_dir = dir.join("unpacked");
    x07_pkg::unpack_tar_bytes(&bytes, &unpack_dir).expect("unpack archive");
    assert!(
        unpack_dir.join("ffi/curl_shim.c").is_file(),
        "missing ffi/curl_shim.c in packed archive"
    );
}

#[test]
fn x07_policy_init_cli_template_creates_base_policy() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_policy_init_cli");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["--init"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(&dir, &["policy", "init", "--template", "cli"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07_POLICY_INIT_REPORT_SCHEMA_VERSION);
    assert_eq!(v["template"], "cli");
    assert_eq!(v["status"], "created");
    assert_eq!(v["out"], ".x07/policies/base/cli.sandbox.base.policy.json");
    assert_eq!(v["policy_id"], "sandbox.cli.base");

    let pol_path = dir.join(".x07/policies/base/cli.sandbox.base.policy.json");
    assert!(pol_path.is_file(), "missing {}", pol_path.display());
    let pol: Value =
        serde_json::from_slice(&std::fs::read(&pol_path).unwrap()).expect("parse policy json");
    assert_eq!(pol["schema_version"], "x07.run-os-policy@0.1.0");
    assert_eq!(pol["policy_id"], "sandbox.cli.base");

    let out = run_x07_in_dir(&dir, &["policy", "init", "--template", "cli"]);
    assert_eq!(out.status.code(), Some(0));
    let v = parse_json_stdout(&out);
    assert_eq!(v["status"], "unchanged");

    // exists_different without --force.
    write_bytes(&pol_path, b"{\"not\":\"a policy\"}\n");
    let out = run_x07_in_dir(&dir, &["policy", "init", "--template", "cli"]);
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json_stdout(&out);
    assert_eq!(v["status"], "exists_different");

    let out = run_x07_in_dir(&dir, &["policy", "init", "--template", "cli", "--force"]);
    assert_eq!(out.status.code(), Some(0));
    let v = parse_json_stdout(&out);
    assert_eq!(v["status"], "overwritten");
    let pol: Value =
        serde_json::from_slice(&std::fs::read(&pol_path).unwrap()).expect("parse policy json");
    assert_eq!(pol["policy_id"], "sandbox.cli.base");
}

#[test]
fn x07_policy_init_all_templates_create_policies() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_policy_init_all_templates");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["--init"]);
    assert_eq!(out.status.code(), Some(0));

    let cases = [
        (
            "cli",
            ".x07/policies/base/cli.sandbox.base.policy.json",
            "sandbox.cli.base",
        ),
        (
            "http-client",
            ".x07/policies/base/http-client.sandbox.base.policy.json",
            "sandbox.http-client.base",
        ),
        (
            "web-service",
            ".x07/policies/base/web-service.sandbox.base.policy.json",
            "sandbox.web-service.base",
        ),
        (
            "fs-tool",
            ".x07/policies/base/fs-tool.sandbox.base.policy.json",
            "sandbox.fs-tool.base",
        ),
        (
            "sqlite-app",
            ".x07/policies/base/sqlite-app.sandbox.base.policy.json",
            "sandbox.sqlite-app.base",
        ),
        (
            "postgres-client",
            ".x07/policies/base/postgres-client.sandbox.base.policy.json",
            "sandbox.postgres-client.base",
        ),
        (
            "worker",
            ".x07/policies/base/worker.sandbox.base.policy.json",
            "sandbox.worker.base",
        ),
    ];

    for (template, out_rel, policy_id) in cases {
        let out = run_x07_in_dir(&dir, &["policy", "init", "--template", template]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "template={template} stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v = parse_json_stdout(&out);
        assert_eq!(v["schema_version"], X07_POLICY_INIT_REPORT_SCHEMA_VERSION);
        assert_eq!(v["template"], template);
        assert_eq!(v["status"], "created");
        assert_eq!(v["out"], out_rel);
        assert_eq!(v["policy_id"], policy_id);

        let pol_path = dir.join(out_rel);
        assert!(pol_path.is_file(), "missing {}", pol_path.display());
        let pol: Value =
            serde_json::from_slice(&std::fs::read(&pol_path).unwrap()).expect("parse policy json");
        assert_eq!(pol["schema_version"], "x07.run-os-policy@0.1.0");
        assert_eq!(pol["policy_id"], policy_id);
    }
}

fn derived_policy_path_from_stderr(stderr: &[u8]) -> Option<String> {
    let s = String::from_utf8_lossy(stderr);
    for line in s.lines() {
        let prefix = "x07 run: using derived policy ";
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

#[test]
fn x07_run_allow_host_materializes_policy() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_allow_host");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["--init"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(&dir, &["policy", "init", "--template", "http-client"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(
        &dir,
        &[
            "run",
            "--world",
            "run-os-sandboxed",
            "--policy",
            ".x07/policies/base/http-client.sandbox.base.policy.json",
            "--allow-host",
            "example.com:443",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let runner_report = parse_json_stdout(&out);
    assert_eq!(
        runner_report["schema_version"],
        X07_OS_RUNNER_REPORT_SCHEMA_VERSION
    );
    assert_eq!(runner_report["exit_code"], 0);

    let derived_path =
        derived_policy_path_from_stderr(&out.stderr).expect("derived policy stderr line");
    let derived_path = PathBuf::from(derived_path);
    assert!(derived_path.is_file(), "missing {}", derived_path.display());

    let pol: Value = serde_json::from_slice(&std::fs::read(&derived_path).unwrap())
        .expect("parse derived policy json");
    assert_eq!(pol["schema_version"], "x07.run-os-policy@0.1.0");
    assert!(pol["policy_id"].as_str().unwrap_or("").contains(".g"));
    assert!(pol["policy_id"].as_str().unwrap_or("").len() <= 64);
    assert_eq!(pol["net"]["allow_dns"], true);

    let hosts = pol["net"]["allow_hosts"].as_array().expect("allow_hosts[]");
    assert!(hosts.iter().any(|h| h["host"] == "example.com"));
    let entry = hosts
        .iter()
        .find(|h| h["host"] == "example.com")
        .expect("example.com entry");
    assert_eq!(entry["ports"], serde_json::json!([443]));
}

#[test]
fn x07_run_deny_host_removes_allow() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_deny_host");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["--init"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(&dir, &["policy", "init", "--template", "http-client"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(
        &dir,
        &[
            "run",
            "--world",
            "run-os-sandboxed",
            "--policy",
            ".x07/policies/base/http-client.sandbox.base.policy.json",
            "--allow-host",
            "example.com:443",
            "--deny-host",
            "example.com:*",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let derived_path =
        derived_policy_path_from_stderr(&out.stderr).expect("derived policy stderr line");
    let derived_path = PathBuf::from(derived_path);
    let pol: Value = serde_json::from_slice(&std::fs::read(&derived_path).unwrap())
        .expect("parse derived policy json");
    assert_eq!(pol["net"]["allow_hosts"], serde_json::json!([]));
    assert_eq!(pol["net"]["allow_dns"], true);
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
