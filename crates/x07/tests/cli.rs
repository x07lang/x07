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

fn fresh_os_tmp_dir(name: &str) -> PathBuf {
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{name}_{pid}_{n}"))
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
fn x07_test_json_false_prints_human_output() {
    let root = repo_root();
    let manifest = root.join("tests/tests.json");
    assert!(manifest.is_file(), "missing {}", manifest.display());

    let out = run_x07(&[
        "test",
        "--manifest",
        manifest.to_str().unwrap(),
        "--json",
        "false",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.stdout.is_empty(),
        "expected human-readable stdout when --json=false"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("summary:"),
        "expected summary line in stdout:\n{stdout}"
    );
}

#[test]
fn x07_cli_specrows_includes_nested_subcommands() {
    let out = run_x07(&["--cli-specrows"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    let rows = v["rows"].as_array().expect("rows[]");
    let has_pkg_add = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("pkg.add")
    });
    assert!(has_pkg_add, "missing pkg.add in --cli-specrows output");
}

#[test]
fn x07_test_finds_stdlib_lock_from_exe_when_missing() {
    let root = repo_root();
    let dir = fresh_os_tmp_dir("tmp_x07_test_stdlib_lock");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_x07_in_dir(&dir, &["test"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["summary"]["passed"], 1);

    let stdlib_lock = v["invocation"]["stdlib_lock"]
        .as_str()
        .expect("invocation.stdlib_lock");
    assert_eq!(
        PathBuf::from(stdlib_lock),
        root.join("stdlib.lock"),
        "expected fallback to the toolchain stdlib.lock"
    );

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_init_creates_project_skeleton() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_init_project");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
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

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "X07INIT_EXISTS");
}

#[test]
fn x07_init_creates_package_skeleton() {
    let root = repo_root();
    let parent = fresh_tmp_dir(&root, "tmp_x07_init_package");
    if parent.exists() {
        std::fs::remove_dir_all(&parent).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&parent).expect("create tmp dir");

    let dir = parent.join("acme-hello-demo");
    std::fs::create_dir_all(&dir).expect("create package dir");

    let out = run_x07_in_dir(&dir, &["init", "--package"]);
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
        "x07-package.json",
        "modules/ext/acme_hello_demo.x07.json",
        "modules/ext/acme_hello_demo/tests.x07.json",
        "tests/tests.json",
        ".gitignore",
    ] {
        assert!(dir.join(rel).is_file(), "missing {}", rel);
    }
    assert!(
        !dir.join("src").exists(),
        "package scaffold must not create src/"
    );
    assert!(!dir.join("tests/smoke.x07.json").exists());

    let pkg_doc: Value =
        serde_json::from_slice(&std::fs::read(dir.join("x07-package.json")).unwrap())
            .expect("parse x07-package.json");
    assert_eq!(pkg_doc["name"], "acme-hello-demo");
    assert_eq!(pkg_doc["version"], "0.1.0");
    assert_eq!(pkg_doc["module_root"], "modules");
    assert_eq!(
        pkg_doc["modules"]
            .as_array()
            .expect("x07-package.json modules[]")
            .iter()
            .map(|v| v.as_str().expect("modules[] string"))
            .collect::<Vec<_>>(),
        vec!["ext.acme_hello_demo", "ext.acme_hello_demo.tests"]
    );
    assert!(pkg_doc["description"]
        .as_str()
        .unwrap_or("")
        .contains("x07 init --package"));
    assert!(pkg_doc["docs"]
        .as_str()
        .unwrap_or("")
        .contains("x07 pkg add"));

    let proj_doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    assert_eq!(proj_doc["world"], "run-os");
    assert_eq!(
        proj_doc["entry"],
        "modules/ext/acme_hello_demo/tests.x07.json"
    );
    assert_eq!(
        proj_doc["module_roots"]
            .as_array()
            .expect("x07.json module_roots[]")
            .iter()
            .map(|v| v.as_str().expect("module_roots[] string"))
            .collect::<Vec<_>>(),
        vec!["modules"]
    );

    let out = run_x07_in_dir(&dir, &["test", "--manifest", "tests/tests.json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["summary"]["passed"], 1);

    let out = run_x07_in_dir(
        &dir,
        &[
            "pkg",
            "pack",
            "--package",
            ".",
            "--out",
            "dist/acme-hello-demo-0.1.0.x07pkg",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("dist/acme-hello-demo-0.1.0.x07pkg").is_file());

    std::fs::remove_dir_all(&parent).expect("cleanup tmp dir");
}

#[test]
fn x07_init_package_rejects_template() {
    let root = repo_root();
    let parent = fresh_tmp_dir(&root, "tmp_x07_init_package_template_reject");
    if parent.exists() {
        std::fs::remove_dir_all(&parent).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&parent).expect("create tmp dir");

    let dir = parent.join("acme-hello-demo");
    std::fs::create_dir_all(&dir).expect("create package dir");

    let out = run_x07_in_dir(&dir, &["init", "--package", "--template", "cli"]);
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "X07INIT_ARGS");
    assert!(!dir.join("x07.json").exists());
    assert!(!dir.join("x07-package.json").exists());

    std::fs::remove_dir_all(&parent).expect("cleanup tmp dir");
}

#[test]
fn x07_pkg_add_updates_project_manifest() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_add");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(&dir, &["pkg", "add", "ext-hex-rs@0.1.3"]);
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
    assert_eq!(deps[0]["version"], "0.1.3");
    assert_eq!(deps[0]["path"], ".x07/deps/ext-hex-rs/0.1.3");

    let out = run_x07_in_dir(&dir, &["pkg", "add", "ext-hex-rs@0.1.3"]);
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "X07PKG_DEP_EXISTS");
}

#[test]
fn x07_pkg_add_sync_is_atomic_on_failure() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_add_sync_atomic");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let before = std::fs::read(dir.join("x07.json")).expect("read x07.json");

    // Use an invalid index URL to trigger a deterministic `--sync` failure (no network).
    // Use a missing local version so `--sync` must consult the index (and fail deterministically
    // on invalid URL parsing).
    let out = run_x07_in_dir(
        &dir,
        &[
            "pkg",
            "add",
            "ext-cli@9.9.9",
            "--sync",
            "--index",
            "sparse+https://localhost:99999/index/",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["command"], "pkg.add");
    assert_eq!(v["error"]["code"], "X07PKG_INDEX_CONFIG");

    let after = std::fs::read(dir.join("x07.json")).expect("read x07.json");
    assert_eq!(after, before, "x07.json changed despite failed --sync");
}

#[test]
fn x07_pkg_add_rejects_non_semver_versions() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_add_bad_version");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let before = std::fs::read(dir.join("x07.json")).expect("read x07.json");

    let out = run_x07_in_dir(&dir, &["pkg", "add", "ext-cli@invalid-version"]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["command"], "pkg.add");
    assert_eq!(v["error"]["code"], "X07PKG_SPEC_INVALID");

    let after = std::fs::read(dir.join("x07.json")).expect("read x07.json");
    assert_eq!(after, before, "x07.json changed despite invalid version");
}

#[test]
fn x07_pkg_lock_offline_uses_official_packages_when_available() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_lock_offline_official");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    // Add without syncing so the dependency is declared but not present on disk yet.
    let out = run_x07_in_dir(&dir, &["pkg", "add", "ext-hex-rs@0.1.3"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dep_manifest = dir.join(".x07/deps/ext-hex-rs/0.1.3/x07-package.json");
    assert!(
        !dep_manifest.is_file(),
        "expected dep not to be present before pkg lock: {}",
        dep_manifest.display()
    );

    // Offline lock should seed official deps from the workspace when possible (no network).
    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--offline"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);

    assert!(
        dep_manifest.is_file(),
        "expected official dep to be copied into project: {}",
        dep_manifest.display()
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--check", "--offline"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
}

#[test]
fn x07_pkg_pack_includes_ffi_dir() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_pack_ffi");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let package_dir = root.join("packages/ext/x07-ext-curl-c/0.1.4");
    assert!(
        package_dir.join("ffi/curl_shim.c").is_file(),
        "missing fixture file"
    );

    let out_path = dir.join("ext-curl-c-0.1.4.x07pkg");
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

    let out = run_x07_in_dir(&dir, &["init"]);
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

    let out = run_x07_in_dir(&dir, &["init"]);
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

    let out = run_x07_in_dir(&dir, &["init"]);
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
fn x07_run_os_sandboxed_allows_write_under_write_root() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_os_sandboxed_write");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_x07_in_dir(&dir, &["policy", "init", "--template", "cli"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    std::fs::create_dir_all(dir.join("out")).expect("create out dir");

    write_bytes(
        &dir.join("main.x07.json"),
        br#"{
  "schema_version": "x07.x07ast@0.2.0",
  "kind": "entry",
  "module_id": "main",
  "imports": [],
  "decls": [],
  "solve": [
    "begin",
    [
      "let",
      "r",
      [
        "os.fs.write_file",
        ["bytes.lit", "out/test.txt"],
        ["bytes.lit", "hello_world"]
      ]
    ],
    [
      "if",
      ["=", "r", 0],
      ["bytes.lit", "ok"],
      ["bytes.lit", "err"]
    ]
  ]
}
"#,
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "run",
            "--world",
            "run-os-sandboxed",
            "--policy",
            ".x07/policies/base/cli.sandbox.base.policy.json",
            "--program",
            "main.x07.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let path = dir.join("out/test.txt");
    assert!(path.is_file(), "missing {}", path.display());
    let bytes = std::fs::read(&path).expect("read output file");
    assert_eq!(bytes, b"hello_world");
}

#[test]
fn x07_run_errors_include_diagnostic_codes_and_hints() {
    let root = repo_root();

    // Invalid project JSON should carry a stable diagnostic code.
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_bad_project_json");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));
    write_bytes(&dir.join("x07.json"), b"{ this is not json }\n");

    let out = run_x07_in_dir(&dir, &["run"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[X07PROJECT_PARSE]"),
        "stderr missing diagnostic code:\n{stderr}"
    );

    // Corrupt lockfile should carry a stable diagnostic code and recovery hint.
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_bad_lockfile_json");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));
    write_bytes(&dir.join("x07.lock.json"), b"{ this is not json }\n");

    let out = run_x07_in_dir(&dir, &["run"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("[X07LOCK_PARSE]"),
        "stderr missing diagnostic code:\n{stderr}"
    );
    assert!(
        stderr.contains("x07 pkg lock"),
        "stderr missing recovery hint:\n{stderr}"
    );
}

#[test]
fn x07_run_deny_host_removes_allow() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_deny_host");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
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
