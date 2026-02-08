use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;
use sha2::{Digest, Sha256};
use x07_contracts::{
    X07AST_SCHEMA_VERSION, X07C_REPORT_SCHEMA_VERSION, X07TEST_SCHEMA_VERSION,
    X07_OS_RUNNER_REPORT_SCHEMA_VERSION, X07_PATCHSET_SCHEMA_VERSION,
    X07_POLICY_INIT_REPORT_SCHEMA_VERSION, X07_REVIEW_DIFF_SCHEMA_VERSION,
    X07_RUN_REPORT_SCHEMA_VERSION, X07_TRUST_REPORT_SCHEMA_VERSION,
};
use x07c::json_patch;

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

fn run_x07_in_dir(dir: &Path, args: &[&str]) -> std::process::Output {
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

fn assert_x07c_report_error(out: &std::process::Output, expected_code: &str) {
    assert_eq!(
        out.status.code(),
        Some(1),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(out);
    assert_eq!(v["schema_version"], X07C_REPORT_SCHEMA_VERSION);
    assert_eq!(v["ok"], false);
    assert_eq!(v["exit_code"], 1);
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["code"], expected_code);
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, bytes).expect("write file");
}

fn file_url_for_dir(dir: &std::path::Path) -> String {
    let abs = dir.canonicalize().expect("canonicalize");
    format!("file://{}/", abs.display())
}

fn sparse_index_rel_path(package_name: &str) -> String {
    let name = package_name.trim();
    assert_eq!(name, name.to_ascii_lowercase(), "name must be lowercase");
    assert!(!name.is_empty(), "name must be non-empty");
    let shard = match name.len() {
        1 => "1".to_string(),
        2 => "2".to_string(),
        3 => format!("3/{}", &name[0..1]),
        _ => format!("{}/{}", &name[0..2], &name[2..4]),
    };
    format!("{shard}/{name}")
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

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn fixtures_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir.join("tests").join("fixtures")
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    if dst.exists() {
        std::fs::remove_dir_all(dst).expect("remove old dst dir");
    }
    std::fs::create_dir_all(dst).expect("create dst dir");

    for entry in walkdir::WalkDir::new(src).into_iter().flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(src).expect("strip prefix");
        let out = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&out).expect("create nested dir");
        } else if entry.file_type().is_file() {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).expect("create parent");
            }
            std::fs::copy(path, &out).expect("copy fixture file");
        }
    }
}

fn write_pbt_fixture_first_byte_zero(dir: &Path) -> (PathBuf, String) {
    let test_id = "prop/first_byte_zero".to_string();
    let module = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "app",
        "imports": ["std.test"],
        "decls": [
            { "kind": "export", "names": ["app.prop_first_byte_zero"] },
            {
                "kind": "defn",
                "name": "app.prop_first_byte_zero",
                "params": [{ "name": "b", "ty": "bytes" }],
                "result": "bytes",
                "body": [
                    "begin",
                    ["let", "v", ["bytes.view", "b"]],
                    [
                        "if",
                        ["=", ["view.len", "v"], 0],
                        ["std.test.status_ok"],
                        [
                            "if",
                            ["=", ["view.get_u8", "v", 0], 0],
                            ["std.test.status_ok"],
                            ["std.test.status_fail", 1]
                        ]
                    ]
                ]
            }
        ]
    });
    write_bytes(
        &dir.join("app.x07.json"),
        serde_json::to_string_pretty(&module).unwrap().as_bytes(),
    );

    let manifest_path = dir.join("tests.json");
    let manifest = serde_json::json!({
        "schema_version": "x07.tests_manifest@0.2.0",
        "tests": [
            {
                "id": test_id,
                "world": "solve-pure",
                "entry": "app.prop_first_byte_zero",
                "expect": "pass",
                "pbt": {
                    "cases": 25,
                    "max_shrinks": 256,
                    "params": [
                        { "name": "b", "gen": { "kind": "bytes", "max_len": 16 } }
                    ],
                    "case_budget": {
                        "fuel": 200000,
                        "timeout_ms": 250,
                        "max_mem_bytes": 67108864,
                        "max_output_bytes": 1048576
                    }
                }
            }
        ]
    });
    write_bytes(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    );

    (manifest_path, test_id)
}

fn write_pbt_fixture_budget_scope_alloc_bytes_trap(dir: &Path) -> (PathBuf, String) {
    let test_id = "prop/budget_scope_alloc_bytes".to_string();
    let module = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "app",
        "imports": ["std.test"],
        "decls": [
            { "kind": "export", "names": ["app.prop_budget_scope_alloc_bytes"] },
            {
                "kind": "defn",
                "name": "app.prop_budget_scope_alloc_bytes",
                "params": [{ "name": "b", "ty": "bytes" }],
                "result": "bytes",
                "body": [
                    "begin",
                    ["let", "_b", "b"],
                    ["let", "_alloc", ["bytes.alloc", 2]],
                    ["std.test.status_ok"]
                ]
            }
        ]
    });
    write_bytes(
        &dir.join("app.x07.json"),
        serde_json::to_string_pretty(&module).unwrap().as_bytes(),
    );

    let manifest_path = dir.join("tests.json");
    let manifest = serde_json::json!({
        "schema_version": "x07.tests_manifest@0.2.0",
        "tests": [
            {
                "id": test_id,
                "world": "solve-pure",
                "entry": "app.prop_budget_scope_alloc_bytes",
                "expect": "pass",
                "pbt": {
                    "cases": 1,
                    "max_shrinks": 1,
                    "params": [
                        { "name": "b", "gen": { "kind": "bytes", "max_len": 0 } }
                    ],
                    "budget_scope": {
                        "alloc_bytes": 1
                    }
                }
            }
        ]
    });
    write_bytes(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    );

    (manifest_path, test_id)
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
    assert_eq!(v["summary"]["passed"], 35);
    assert_eq!(v["summary"]["failed"], 0);
    assert_eq!(v["summary"]["errors"], 0);
    assert_eq!(v["summary"]["xfail_failed"], 1);

    let tests = v["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 36);
    let ids: Vec<&str> = tests
        .iter()
        .map(|t| t["id"].as_str().expect("test.id"))
        .collect();
    assert_eq!(
        ids,
        vec![
            "smoke/budget_scope_result_err_alloc_bytes",
            "smoke/fs_read_hello",
            "smoke/fs_read_hello_run_os_sandboxed",
            "smoke/fs_read_task_hello_run_os_sandboxed",
            "smoke/full_fs_rr_kv",
            "smoke/kv_get_pong",
            "smoke/pure_i32_eq",
            "smoke/pure_xfail_demo",
            "smoke/rr_fetch_pong",
            "smoke/stream_pipe_bytes_budget_items",
            "smoke/stream_pipe_bytes_collect_identity",
            "smoke/stream_pipe_bytes_filter_collect",
            "smoke/stream_pipe_bytes_frame_u32le_collect",
            "smoke/stream_pipe_bytes_hash_fnv1a32",
            "smoke/stream_pipe_bytes_json_canon_collect",
            "smoke/stream_pipe_bytes_json_canon_trailing_data_err",
            "smoke/stream_pipe_bytes_map_bytes_collect",
            "smoke/stream_pipe_bytes_map_in_place_buf_collect",
            "smoke/stream_pipe_bytes_map_in_place_buf_overflow",
            "smoke/stream_pipe_bytes_split_lines_collect",
            "smoke/stream_pipe_bytes_split_lines_line_too_long",
            "smoke/stream_pipe_bytes_take_collect",
            "smoke/stream_pipe_bytes_u32frames_collect",
            "smoke/stream_pipe_deframe_collect_ok",
            "smoke/stream_pipe_deframe_empty_forbidden",
            "smoke/stream_pipe_deframe_frame_too_large",
            "smoke/stream_pipe_deframe_fs_hdr_split",
            "smoke/stream_pipe_deframe_fs_payload_split",
            "smoke/stream_pipe_deframe_max_frames",
            "smoke/stream_pipe_deframe_truncated_drop_ok",
            "smoke/stream_pipe_deframe_truncated_err",
            "smoke/stream_pipe_fs_open_read_collect",
            "smoke/stream_pipe_rr_send_collect",
            "stream_plugins/abi_descriptor_sanity_v1",
            "stream_plugins/emit_limits_enforced_v1",
            "stream_plugins/emit_view_v1"
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
        let world = t["world"].as_str().expect("test.world");
        if world.starts_with("run-os") {
            assert_eq!(t["status"], "error");
            assert!(t.get("compile").is_none(), "unexpected compile section");
            assert!(t.get("run").is_none(), "unexpected run section");
            assert!(
                t["diags"]
                    .as_array()
                    .expect("diags[]")
                    .iter()
                    .any(|d| d["code"] == "ETEST_NO_RUN_UNSUPPORTED"),
                "expected ETEST_NO_RUN_UNSUPPORTED diag"
            );
        } else {
            assert_eq!(t["status"], "skip");
            assert!(t.get("compile").is_some(), "missing compile section");
            assert!(t.get("run").is_none(), "unexpected run section");
        }
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
fn x07_test_manifest_input_b64_affects_run() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "test_manifest_input_b64");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let module = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "app",
        "imports": ["std.test"],
        "decls": [
            { "kind": "export", "names": ["app.check_input_len"] },
            {
                "kind": "defn",
                "name": "app.check_input_len",
                "params": [],
                "result": "bytes",
                "body": ["if", ["=", ["view.len", "input"], 3], ["std.test.status_ok"], ["std.test.status_fail", 123]]
            }
        ]
    });
    write_bytes(
        &dir.join("app.x07.json"),
        serde_json::to_string_pretty(&module).unwrap().as_bytes(),
    );

    let manifest_path = dir.join("tests.json");
    let mk_manifest = |input_b64: &str| {
        serde_json::json!({
            "schema_version": "x07.tests_manifest@0.2.0",
            "tests": [
                {
                    "id": "smoke/input_b64",
                    "world": "solve-pure",
                    "entry": "app.check_input_len",
                    "returns": "bytes_status_v1",
                    "expect": "pass",
                    "input_b64": input_b64
                }
            ]
        })
    };

    write_bytes(
        &manifest_path,
        serde_json::to_string_pretty(&mk_manifest("YWJj"))
            .unwrap()
            .as_bytes(),
    );
    let out = run_x07(&["test", "--manifest", manifest_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["summary"]["passed"], 1);
    assert_eq!(v["summary"]["failed"], 0);

    write_bytes(
        &manifest_path,
        serde_json::to_string_pretty(&mk_manifest(""))
            .unwrap()
            .as_bytes(),
    );
    let out = run_x07(&["test", "--manifest", manifest_path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(10));
    let v = parse_json_stdout(&out);
    assert_eq!(v["summary"]["passed"], 0);
    assert_eq!(v["summary"]["failed"], 1);
}

#[test]
fn x07_test_pbt_emits_repro_and_replays() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "test_pbt_e2e");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let (manifest_path, test_id) = write_pbt_fixture_first_byte_zero(&dir);

    let artifact_dir = dir.join("artifacts");

    let out = run_x07_in_dir(
        &dir,
        &[
            "test",
            "--pbt",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--artifact-dir",
            artifact_dir.to_str().unwrap(),
            "--keep-artifacts",
            "--repeat",
            "2",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(10),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["summary"]["failed"], 1);
    assert!(
        v["tests"][0]["diags"]
            .as_array()
            .expect("diags[]")
            .iter()
            .any(|d| d["code"] == "X07T_EPBT_FAIL"),
        "expected X07T_EPBT_FAIL diag"
    );

    let repro_path = artifact_dir
        .join("pbt")
        .join(format!("id_{}", sha256_hex(test_id.as_bytes())))
        .join("repro.json");
    assert!(repro_path.is_file(), "missing {}", repro_path.display());
    let repro_bytes_1 = std::fs::read(&repro_path).expect("read repro.json");

    let out = run_x07_in_dir(
        &dir,
        &[
            "test",
            "--pbt",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--artifact-dir",
            artifact_dir.to_str().unwrap(),
            "--keep-artifacts",
        ],
    );
    assert_eq!(out.status.code(), Some(10));
    let repro_bytes_2 = std::fs::read(&repro_path).expect("read repro.json");
    assert_eq!(repro_bytes_2, repro_bytes_1);

    let out = run_x07_in_dir(
        &dir,
        &[
            "test",
            "--pbt",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--artifact-dir",
            artifact_dir.to_str().unwrap(),
            "--keep-artifacts",
            "--pbt-repro",
            repro_path.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(10));
    let v = parse_json_stdout(&out);
    assert_eq!(v["summary"]["failed"], 1);
}

#[test]
fn x07_test_pbt_budget_scope_wraps_property_and_fix_preserves_scope() {
    use base64::Engine as _;

    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "test_pbt_budget_scope");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let (manifest_path, test_id) = write_pbt_fixture_budget_scope_alloc_bytes_trap(&dir);
    let artifact_dir = dir.join("artifacts");

    let out = run_x07_in_dir(
        &dir,
        &[
            "test",
            "--pbt",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--artifact-dir",
            artifact_dir.to_str().unwrap(),
            "--keep-artifacts",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(12),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out_dir = artifact_dir
        .join("pbt")
        .join(format!("id_{}", sha256_hex(test_id.as_bytes())));
    let driver_path = out_dir.join("driver.x07.json");
    assert!(driver_path.is_file(), "missing {}", driver_path.display());
    let driver_text = std::fs::read_to_string(&driver_path).expect("read driver");
    assert!(
        driver_text.contains("budget.scope_v1"),
        "expected budget.scope_v1 in driver"
    );

    let repro_path = out_dir.join("repro.json");
    assert!(repro_path.is_file(), "missing {}", repro_path.display());

    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_path.to_str().unwrap(),
            "--tests-manifest",
            manifest_path.to_str().unwrap(),
            "--write",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let repro_doc: Value = serde_json::from_slice(&std::fs::read(&repro_path).expect("read repro"))
        .expect("parse repro JSON");
    let case_b64 = repro_doc["counterexample"]["case_bytes_b64"]
        .as_str()
        .expect("case_bytes_b64")
        .to_string();
    let case_bytes = base64::engine::general_purpose::STANDARD
        .decode(case_b64.as_bytes())
        .expect("decode case bytes");
    let case_tag = format!("c{}", &sha256_hex(&case_bytes)[0..12]);

    let wrapper_path = dir
        .join("repro")
        .join("pbt")
        .join(format!("{case_tag}.x07.json"));
    assert!(wrapper_path.is_file(), "missing {}", wrapper_path.display());
    let wrapper_text = std::fs::read_to_string(&wrapper_path).expect("read wrapper");
    assert!(
        wrapper_text.contains("budget.scope_v1"),
        "expected budget.scope_v1 in wrapper"
    );
}

#[test]
fn x07_fix_from_pbt_generates_regression_test() {
    use base64::Engine as _;

    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "test_fix_from_pbt");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let (manifest_path, test_id) = write_pbt_fixture_first_byte_zero(&dir);
    let artifact_dir = dir.join("artifacts");

    let out = run_x07_in_dir(
        &dir,
        &[
            "test",
            "--pbt",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--artifact-dir",
            artifact_dir.to_str().unwrap(),
            "--keep-artifacts",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(10),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let repro_path = artifact_dir
        .join("pbt")
        .join(format!("id_{}", sha256_hex(test_id.as_bytes())))
        .join("repro.json");
    assert!(repro_path.is_file(), "missing {}", repro_path.display());

    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_path.to_str().unwrap(),
            "--tests-manifest",
            manifest_path.to_str().unwrap(),
            "--write",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let repro_doc: Value = serde_json::from_slice(&std::fs::read(&repro_path).expect("read repro"))
        .expect("parse repro JSON");
    let case_b64 = repro_doc["counterexample"]["case_bytes_b64"]
        .as_str()
        .expect("case_bytes_b64")
        .to_string();

    let manifest_doc: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest after fix"))
            .expect("parse manifest after fix");
    let tests = manifest_doc["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 2);
    let new_test = tests
        .iter()
        .find(|t| {
            t["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("pbt_repro/"))
        })
        .expect("new test entry");

    assert_eq!(new_test["expect"], "pass");
    assert_eq!(new_test["returns"], "bytes_status_v1");
    assert_eq!(new_test["input_b64"], case_b64);

    let entry = new_test["entry"].as_str().expect("entry");
    assert!(
        entry.starts_with("repro.pbt.c") && entry.ends_with(".run"),
        "unexpected entry: {entry}"
    );
    let case_bytes = base64::engine::general_purpose::STANDARD
        .decode(case_b64.as_bytes())
        .expect("decode case bytes");
    let case_tag = format!("c{}", &sha256_hex(&case_bytes)[0..12]);
    assert_eq!(entry, format!("repro.pbt.{case_tag}.run"));

    let wrapper_path = dir
        .join("repro")
        .join("pbt")
        .join(format!("{case_tag}.x07.json"));
    assert!(wrapper_path.is_file(), "missing {}", wrapper_path.display());
    let copied_repro_path = dir
        .join("repro")
        .join("pbt")
        .join(format!("{case_tag}.repro.json"));
    assert!(
        copied_repro_path.is_file(),
        "missing {}",
        copied_repro_path.display()
    );

    let out = run_x07_in_dir(
        &dir,
        &["test", "--manifest", manifest_path.to_str().unwrap()],
    );
    assert_eq!(out.status.code(), Some(10));
    let v = parse_json_stdout(&out);
    assert_eq!(v["summary"]["failed"], 1);
}

#[test]
fn x07_fix_from_pbt_emits_structured_error_diagnostics_for_repro_and_args() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "test_fix_from_pbt_error_diags_repro");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let tests_manifest = dir.join("tests.json");
    write_bytes(
        &tests_manifest,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "x07.tests_manifest@0.2.0",
            "tests": []
        }))
        .unwrap()
        .as_bytes(),
    );

    let repro_missing = dir.join("missing.repro.json");
    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_missing.to_str().unwrap(),
            "--tests-manifest",
            tests_manifest.to_str().unwrap(),
        ],
    );
    assert_x07c_report_error(&out, "X07-PBT-FIX-ARGS-0001");

    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_missing.to_str().unwrap(),
            "--tests-manifest",
            tests_manifest.to_str().unwrap(),
            "--write",
        ],
    );
    assert_x07c_report_error(&out, "X07-PBT-REPRO-READ-0001");

    let repro_invalid_json = dir.join("invalid.repro.json");
    write_bytes(&repro_invalid_json, b"not json\n");
    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_invalid_json.to_str().unwrap(),
            "--tests-manifest",
            tests_manifest.to_str().unwrap(),
            "--write",
        ],
    );
    assert_x07c_report_error(&out, "X07-PBT-REPRO-PARSE-0001");

    let repro_wrong_schema = dir.join("wrong_schema.repro.json");
    write_bytes(
        &repro_wrong_schema,
        b"{\"schema_version\":\"x07.pbt.repro@0.0.0\"}\n",
    );
    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_wrong_schema.to_str().unwrap(),
            "--tests-manifest",
            tests_manifest.to_str().unwrap(),
            "--write",
        ],
    );
    assert_x07c_report_error(&out, "X07-PBT-REPRO-SCHEMA-0001");
}

#[test]
fn x07_fix_from_pbt_emits_structured_error_diagnostics_for_manifest_and_conflicts() {
    use base64::Engine as _;

    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "test_fix_from_pbt_error_diags_manifest");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let (manifest_path, test_id) = write_pbt_fixture_first_byte_zero(&dir);
    let artifact_dir = dir.join("artifacts");

    let out = run_x07_in_dir(
        &dir,
        &[
            "test",
            "--pbt",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--artifact-dir",
            artifact_dir.to_str().unwrap(),
            "--keep-artifacts",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(10),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let repro_path = artifact_dir
        .join("pbt")
        .join(format!("id_{}", sha256_hex(test_id.as_bytes())))
        .join("repro.json");
    assert!(repro_path.is_file(), "missing {}", repro_path.display());

    let missing_manifest = dir.join("missing_tests.json");
    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_path.to_str().unwrap(),
            "--tests-manifest",
            missing_manifest.to_str().unwrap(),
            "--write",
        ],
    );
    assert_x07c_report_error(&out, "X07-PBT-FIX-MANIFEST-0001");

    let manifest_missing_test = dir.join("tests_missing_test.json");
    write_bytes(
        &manifest_missing_test,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "x07.tests_manifest@0.2.0",
            "tests": []
        }))
        .unwrap()
        .as_bytes(),
    );
    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_path.to_str().unwrap(),
            "--tests-manifest",
            manifest_missing_test.to_str().unwrap(),
            "--write",
        ],
    );
    assert_x07c_report_error(&out, "X07-PBT-FIX-TEST-NOT-FOUND-0001");

    let manifest_conflict = dir.join("tests_conflict.json");
    write_bytes(
        &manifest_conflict,
        &std::fs::read(&manifest_path).expect("read manifest for copy"),
    );
    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_path.to_str().unwrap(),
            "--tests-manifest",
            manifest_conflict.to_str().unwrap(),
            "--write",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let repro_doc: Value = serde_json::from_slice(&std::fs::read(&repro_path).expect("read repro"))
        .expect("parse repro JSON");
    let case_b64 = repro_doc["counterexample"]["case_bytes_b64"]
        .as_str()
        .expect("case_bytes_b64")
        .to_string();
    let case_bytes = base64::engine::general_purpose::STANDARD
        .decode(case_b64.as_bytes())
        .expect("decode case bytes");
    let case_tag = format!("c{}", &sha256_hex(&case_bytes)[0..12]);
    let wrapper_path = dir
        .join("repro")
        .join("pbt")
        .join(format!("{case_tag}.x07.json"));
    assert!(wrapper_path.is_file(), "missing {}", wrapper_path.display());

    write_bytes(&wrapper_path, b"not a wrapper module\n");

    let out = run_x07_in_dir(
        &dir,
        &[
            "fix",
            "--from-pbt",
            repro_path.to_str().unwrap(),
            "--tests-manifest",
            manifest_conflict.to_str().unwrap(),
            "--write",
        ],
    );
    assert_x07c_report_error(&out, "X07-PBT-FIX-CONFLICT-0001");
}

#[test]
fn x07_run_project_accepts_dir_path() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_project_dir");
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

    let out = run_x07_in_dir(&dir, &["run", "--project", ".", "--report", "wrapped"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07_RUN_REPORT_SCHEMA_VERSION);
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

    let has_pkg_remove = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("pkg.remove")
    });
    assert!(
        has_pkg_remove,
        "missing pkg.remove in --cli-specrows output"
    );

    let has_pkg_versions = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("pkg.versions")
    });
    assert!(
        has_pkg_versions,
        "missing pkg.versions in --cli-specrows output"
    );

    let has_arch_check = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("arch.check")
    });
    assert!(
        has_arch_check,
        "missing arch.check in --cli-specrows output"
    );

    let has_review_diff = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("review.diff")
    });
    assert!(
        has_review_diff,
        "missing review.diff in --cli-specrows output"
    );

    let has_trust_report = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("trust.report")
    });
    assert!(
        has_trust_report,
        "missing trust.report in --cli-specrows output"
    );

    let has_ast_schema = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("ast.schema")
    });
    assert!(
        has_ast_schema,
        "missing ast.schema in --cli-specrows output"
    );

    let has_ast_grammar = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("ast.grammar")
    });
    assert!(
        has_ast_grammar,
        "missing ast.grammar in --cli-specrows output"
    );
}

fn write_json(path: &Path, doc: &Value) {
    let bytes = serde_json::to_vec_pretty(doc).expect("serialize JSON");
    write_bytes(path, &bytes);
}

fn x07_module_doc(module_id: &str, imports: &[&str], decls: Vec<Value>) -> Value {
    Value::Object(
        [
            (
                "schema_version".to_string(),
                Value::String(X07AST_SCHEMA_VERSION.to_string()),
            ),
            ("kind".to_string(), Value::String("module".to_string())),
            (
                "module_id".to_string(),
                Value::String(module_id.to_string()),
            ),
            (
                "imports".to_string(),
                Value::Array(
                    imports
                        .iter()
                        .map(|s| Value::String((*s).to_string()))
                        .collect(),
                ),
            ),
            ("decls".to_string(), Value::Array(decls)),
        ]
        .into_iter()
        .collect(),
    )
}

fn x07_export_decl(names: &[&str]) -> Value {
    serde_json::json!({"kind":"export","names": names})
}

fn x07_defn_decl(
    name: &str,
    params: Vec<Value>,
    result: &str,
    result_brand: Option<&str>,
) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("kind".to_string(), Value::String("defn".to_string()));
    m.insert("name".to_string(), Value::String(name.to_string()));
    m.insert("params".to_string(), Value::Array(params));
    m.insert("result".to_string(), Value::String(result.to_string()));
    if let Some(b) = result_brand {
        m.insert("result_brand".to_string(), Value::String(b.to_string()));
    }
    m.insert("body".to_string(), Value::Number(0.into()));
    Value::Object(m)
}

fn x07_param(name: &str, ty: &str, brand: Option<&str>) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("name".to_string(), Value::String(name.to_string()));
    m.insert("ty".to_string(), Value::String(ty.to_string()));
    if let Some(b) = brand {
        m.insert("brand".to_string(), Value::String(b.to_string()));
    }
    Value::Object(m)
}

fn arch_manifest_doc(
    nodes: Vec<Value>,
    rules: Vec<Value>,
    checks: Value,
    externals: Value,
) -> Value {
    serde_json::json!({
      "schema_version": "x07.arch.manifest@0.1.0",
      "repo": {"id":"test-repo","root":"."},
      "externals": externals,
      "nodes": nodes,
      "rules": rules,
      "checks": checks,
      "tool_budgets": { "max_modules": 1000, "max_edges": 1000, "max_diags": 2000 }
    })
}

#[allow(clippy::too_many_arguments)]
fn arch_node_doc(
    id: &str,
    module_prefixes: &[&str],
    world: &str,
    visibility_mode: &str,
    visible_to: &[&str],
    deny_prefixes: &[&str],
    allow_prefixes: &[&str],
    smoke_entry: Option<&str>,
) -> Value {
    let mut node = serde_json::Map::new();
    node.insert("id".to_string(), Value::String(id.to_string()));
    node.insert(
        "match".to_string(),
        serde_json::json!({
          "module_prefixes": module_prefixes,
          "path_globs": []
        }),
    );
    node.insert("world".to_string(), Value::String(world.to_string()));
    node.insert(
        "visibility".to_string(),
        serde_json::json!({"mode": visibility_mode, "visible_to": visible_to}),
    );
    node.insert(
        "imports".to_string(),
        serde_json::json!({"deny_prefixes": deny_prefixes, "allow_prefixes": allow_prefixes}),
    );
    if let Some(s) = smoke_entry {
        node.insert(
            "contracts".to_string(),
            serde_json::json!({"smoke_entry": s}),
        );
    }
    Value::Object(node)
}

fn default_arch_checks() -> Value {
    serde_json::json!({
      "deny_cycles": true,
      "deny_orphans": true,
      "enforce_visibility": true,
      "enforce_world_caps": true
    })
}

fn default_arch_externals() -> Value {
    serde_json::json!({"allowed_import_prefixes":["std.","ext."],"allowed_exact":[]})
}

fn diag_codes(v: &Value) -> Vec<String> {
    v["diags"]
        .as_array()
        .expect("diags[]")
        .iter()
        .map(|d| d["code"].as_str().expect("code").to_string())
        .collect()
}

#[test]
fn x07_arch_check_pass_minimal() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_ok");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_a = x07_module_doc(
        "app.core.a",
        &["std.vec"],
        vec![
            x07_export_decl(&["app.core.a.f"]),
            x07_defn_decl("app.core.a.f", Vec::new(), "i32", None),
        ],
    );
    write_json(&dir.join("src/app/core/a.x07.json"), &mod_a);

    let manifest = arch_manifest_doc(
        vec![arch_node_doc(
            "core",
            &["app.core."],
            "solve-pure",
            "restricted",
            &[],
            &[],
            &["std.", "ext.", "app.core."],
            None,
        )],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], "x07.arch.report@0.1.0");
    assert!(v["diags"].as_array().expect("diags[]").is_empty());

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_orphan_module_errors_and_suggests_node_patch() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_orphan");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_a = x07_module_doc(
        "app.core.a",
        &[],
        vec![
            x07_export_decl(&["app.core.a.f"]),
            x07_defn_decl("app.core.a.f", Vec::new(), "i32", None),
        ],
    );
    write_json(&dir.join("src/app/core/a.x07.json"), &mod_a);

    let manifest = arch_manifest_doc(
        vec![arch_node_doc(
            "domain",
            &["app.domain."],
            "solve-pure",
            "restricted",
            &[],
            &[],
            &["std.", "ext.", "app.domain."],
            None,
        )],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    let codes = diag_codes(&v);
    assert!(codes.contains(&"E_ARCH_NODE_ORPHAN_MODULE".to_string()));
    assert!(
        v["suggested_patches"]
            .as_array()
            .expect("suggested_patches[]")
            .iter()
            .any(|p| p["path"] == "arch/manifest.x07arch.json"),
        "expected manifest patch suggestion for orphan modules"
    );

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_external_import_not_allowed_suggests_manifest_allow_exact_and_write_fixes() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_external");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_a = x07_module_doc("app.core.a", &["thirdparty.hyper"], Vec::new());
    write_json(&dir.join("src/app/core/a.x07.json"), &mod_a);

    let manifest = arch_manifest_doc(
        vec![arch_node_doc(
            "core",
            &["app.core."],
            "solve-pure",
            "restricted",
            &[],
            &[],
            &["app.", "std.", "ext."],
            None,
        )],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json_stdout(&out);
    let codes = diag_codes(&v);
    assert!(codes.contains(&"E_ARCH_EXTERNAL_IMPORT_NOT_ALLOWED".to_string()));
    assert!(
        v["suggested_patches"]
            .as_array()
            .expect("suggested_patches[]")
            .iter()
            .any(|p| p["path"] == "arch/manifest.x07arch.json"),
        "expected manifest patch suggestion"
    );

    let out = run_x07_in_dir(&dir, &["arch", "check", "--write"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert!(v["diags"].as_array().expect("diags[]").is_empty());

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_emit_patch_writes_patchset() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_emit_patch");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_a = x07_module_doc("app.core.a", &["thirdparty.hyper"], Vec::new());
    write_json(&dir.join("src/app/core/a.x07.json"), &mod_a);

    let manifest = arch_manifest_doc(
        vec![arch_node_doc(
            "core",
            &["app.core."],
            "solve-pure",
            "restricted",
            &[],
            &[],
            &["app.", "std.", "ext."],
            None,
        )],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out = run_x07_in_dir(
        &dir,
        &["arch", "check", "--emit-patch", "arch/patchset.json"],
    );
    assert_eq!(out.status.code(), Some(2));

    let patchset_path = dir.join("arch/patchset.json");
    assert!(
        patchset_path.is_file(),
        "missing {}",
        patchset_path.display()
    );
    let patchset: Value =
        serde_json::from_slice(&std::fs::read(&patchset_path).expect("read patchset"))
            .expect("parse patchset");
    assert_eq!(patchset["schema_version"], "x07.arch.patchset@0.1.0");
    assert!(
        patchset["patches"]
            .as_array()
            .expect("patches[]")
            .iter()
            .any(|p| p["path"] == "arch/manifest.x07arch.json"),
        "expected manifest patch target"
    );

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_allowlist_mode_requires_explicit_internal_allows() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_allowlist");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_a = x07_module_doc("app.a", &["app.b"], Vec::new());
    let mod_b = x07_module_doc("app.b", &[], Vec::new());
    write_json(&dir.join("src/app/a.x07.json"), &mod_a);
    write_json(&dir.join("src/app/b.x07.json"), &mod_b);

    let checks = serde_json::json!({
      "deny_cycles": true,
      "deny_orphans": true,
      "enforce_visibility": true,
      "enforce_world_caps": true,
      "allowlist_mode": { "enabled": true, "default_allow_external": true, "default_allow_internal": false }
    });

    let manifest = arch_manifest_doc(
        vec![
            arch_node_doc(
                "a",
                &["app.a"],
                "solve-pure",
                "public",
                &[],
                &[],
                &["app.", "std.", "ext."],
                None,
            ),
            arch_node_doc(
                "b",
                &["app.b"],
                "solve-pure",
                "public",
                &[],
                &[],
                &["app.", "std.", "ext."],
                None,
            ),
        ],
        vec![],
        checks,
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json_stdout(&out);
    let codes = diag_codes(&v);
    assert!(codes.contains(&"E_ARCH_EDGE_NOT_ALLOWED".to_string()));

    let manifest_patch = v["suggested_patches"]
        .as_array()
        .expect("suggested_patches[]")
        .iter()
        .find(|p| p["path"] == "arch/manifest.x07arch.json")
        .expect("missing manifest suggested patch");
    assert!(
        manifest_patch["patch"]
            .as_array()
            .expect("patch[]")
            .iter()
            .any(|op| op["op"] == "add" && op["path"] == "/rules/-"),
        "expected deps_v1 allow rule suggestion"
    );

    let out = run_x07_in_dir(&dir, &["arch", "check", "--write"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert!(v["diags"].as_array().expect("diags[]").is_empty());

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_write_lock_creates_lock_and_detects_mismatch() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_lock");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_a = x07_module_doc("app.core.a", &[], Vec::new());
    write_json(&dir.join("src/app/core/a.x07.json"), &mod_a);

    let manifest = arch_manifest_doc(
        vec![arch_node_doc(
            "core",
            &["app.core."],
            "solve-pure",
            "restricted",
            &[],
            &[],
            &["app.", "std.", "ext."],
            None,
        )],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    let manifest_path = dir.join("arch/manifest.x07arch.json");
    write_json(&manifest_path, &manifest);

    let out = run_x07_in_dir(&dir, &["arch", "check", "--write-lock"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lock_path = dir.join("arch/manifest.lock.json");
    assert!(lock_path.is_file(), "missing {}", lock_path.display());
    let lock: Value =
        serde_json::from_slice(&std::fs::read(&lock_path).expect("read lock")).expect("parse lock");
    assert_eq!(lock["schema_version"], "x07.arch.manifest.lock@0.1.0");
    assert_eq!(lock["manifest_path"], "arch/manifest.x07arch.json");
    assert_eq!(lock["module_scan"]["include_globs"][0], "**/*.x07.json");

    let mut manifest_v: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest_v["externals"]["allowed_exact"] = serde_json::json!(["thirdparty.hyper"]);
    write_json(&manifest_path, &manifest_v);

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json_stdout(&out);
    let codes = diag_codes(&v);
    assert!(codes.contains(&"E_ARCH_LOCK_MISMATCH".to_string()));

    let out = run_x07_in_dir(&dir, &["arch", "check", "--write-lock"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(out.status.code(), Some(0));

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_smoke_entry_missing() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_smoke");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_a = x07_module_doc("app.core.a", &[], Vec::new());
    write_json(&dir.join("src/app/core/a.x07.json"), &mod_a);

    let manifest = arch_manifest_doc(
        vec![arch_node_doc(
            "core",
            &["app.core."],
            "solve-pure",
            "restricted",
            &[],
            &[],
            &["app.", "std.", "ext."],
            Some("app.core.smoke_v1"),
        )],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json_stdout(&out);
    let codes = diag_codes(&v);
    assert!(codes.contains(&"E_ARCH_SMOKE_MISSING".to_string()));

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_public_bytes_unbranded() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_brand");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let symbol = "app.api.echo";
    let mod_a = x07_module_doc(
        "app.api",
        &[],
        vec![
            x07_export_decl(&[symbol]),
            x07_defn_decl(symbol, vec![x07_param("b", "bytes", None)], "bytes", None),
        ],
    );
    write_json(&dir.join("src/app/api.x07.json"), &mod_a);

    let manifest = arch_manifest_doc(
        vec![arch_node_doc(
            "api",
            &["app.api"],
            "solve-pure",
            "public",
            &[],
            &[],
            &["app.", "std.", "ext."],
            None,
        )],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json_stdout(&out);
    let codes = diag_codes(&v);
    assert!(codes.contains(&"E_ARCH_PUBLIC_BYTES_UNBRANDED".to_string()));

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_world_of_imported_forbidden() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_world");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_core = x07_module_doc("app.core.a", &["app.os.proc"], Vec::new());
    let mod_os = x07_module_doc("app.os.proc", &[], Vec::new());
    write_json(&dir.join("src/app/core/a.x07.json"), &mod_core);
    write_json(&dir.join("src/app/os/proc.x07.json"), &mod_os);

    let manifest = arch_manifest_doc(
        vec![
            arch_node_doc(
                "core",
                &["app.core."],
                "solve-pure",
                "public",
                &[],
                &[],
                &["app.", "std.", "ext."],
                None,
            ),
            arch_node_doc(
                "os",
                &["app.os."],
                "run-os",
                "public",
                &[],
                &[],
                &["app.", "std.", "ext."],
                None,
            ),
        ],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json_stdout(&out);
    let codes = diag_codes(&v);
    assert!(codes.contains(&"E_ARCH_WORLD_EDGE_FORBIDDEN".to_string()));

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_arch_check_is_deterministic() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_arch_determinism");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let mod_a = x07_module_doc("app.core.a", &["thirdparty.hyper"], Vec::new());
    write_json(&dir.join("src/app/core/a.x07.json"), &mod_a);
    let manifest = arch_manifest_doc(
        vec![arch_node_doc(
            "core",
            &["app.core."],
            "solve-pure",
            "restricted",
            &[],
            &[],
            &["app.", "std.", "ext."],
            None,
        )],
        vec![],
        default_arch_checks(),
        default_arch_externals(),
    );
    write_json(&dir.join("arch/manifest.x07arch.json"), &manifest);

    let out1 = run_x07_in_dir(&dir, &["arch", "check"]);
    let out2 = run_x07_in_dir(&dir, &["arch", "check"]);
    assert_eq!(out1.status.code(), out2.status.code());
    assert_eq!(out1.stdout, out2.stdout);

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

fn run_schema_derive_smoke(fixture: &Path) {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_schema_derive");
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

    let proj_path = dir.join("x07.json");
    let mut proj: Value =
        serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
            .expect("parse x07.json");
    let roots = proj["module_roots"].as_array_mut().expect("module_roots[]");
    if !roots.iter().any(|v| v.as_str() == Some("modules")) {
        roots.push(Value::String("modules".to_string()));
    }
    std::fs::write(
        &proj_path,
        serde_json::to_vec_pretty(&proj).expect("serialize x07.json"),
    )
    .expect("write x07.json");

    let out = run_x07_in_dir(&dir, &["pkg", "add", "ext-data-model@0.1.5"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--offline"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    assert!(fixture.is_file(), "missing {}", fixture.display());
    let schema_bytes = std::fs::read(fixture).expect("read schema fixture");
    let schema_path = dir.join("schemas").join("example.x07schema.json");
    write_bytes(&schema_path, &schema_bytes);

    let out = run_x07_in_dir(
        &dir,
        &[
            "schema",
            "derive",
            "--input",
            "schemas/example.x07schema.json",
            "--out-dir",
            ".",
            "--write",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["schema_version"], "x07.schema.derive.report@0.1.0");

    let out = run_x07_in_dir(&dir, &["test", "--manifest", "tests/tests.json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "schema",
            "derive",
            "--input",
            "schemas/example.x07schema.json",
            "--out-dir",
            ".",
            "--check",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let drift_path = dir.join("modules/example/schema/api/req_v1.x07.json");
    let mut drift_bytes = std::fs::read(&drift_path).expect("read generated module");
    drift_bytes.push(b' ');
    std::fs::write(&drift_path, &drift_bytes).expect("write drifted module");

    let out = run_x07_in_dir(
        &dir,
        &[
            "schema",
            "derive",
            "--input",
            "schemas/example.x07schema.json",
            "--out-dir",
            ".",
            "--check",
        ],
    );
    assert_eq!(out.status.code(), Some(1), "expected drift exit code");
}

#[test]
fn x07_schema_derive_smoke() {
    let root = repo_root();
    let fixture = root.join("tests/fixtures/schema_derive/example.x07schema.json");
    run_schema_derive_smoke(&fixture);
}

#[test]
fn x07_schema_derive_rows_smoke() {
    let root = repo_root();
    let fixture = root.join("tests/fixtures/schema_derive/example_rows.x07schema.json");
    run_schema_derive_smoke(&fixture);
}

#[test]
fn x07_schema_derive_020_smoke() {
    let root = repo_root();
    let fixture = root.join("tests/fixtures/schema_derive/example_020.x07schema.json");
    run_schema_derive_smoke(&fixture);
}

#[test]
fn x07_fix_applies_multiple_borrow_quickfixes() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_fix_borrow_quickfixes");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["begin",
            ["let", "x", ["+",
                ["bytes.len", ["bytes.view", ["bytes.lit", "a"]]],
                ["bytes.len", ["bytes.view", ["bytes.lit", "b"]]]
            ]],
            ["let", "y", ["view.to_bytes", ["bytes.view", ["bytes.lit", "c"]]]],
            "y"
        ]
    }))
    .expect("serialize x07AST");
    let program_path = dir.join("main.x07.json");
    write_bytes(&program_path, &program);

    let out = run_x07(&[
        "fix",
        "--input",
        program_path.to_str().unwrap(),
        "--write",
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], "x07.tool.fix.report@0.1.0");
    assert_eq!(v["command"], "x07.fix");
    assert_eq!(v["ok"], true);
    assert_eq!(v["exit_code"], 0);
    assert!(
        v["diagnostics"]
            .as_array()
            .expect("diagnostics[]")
            .is_empty(),
        "expected fix wrapper report to be clean"
    );

    let out = run_x07(&["lint", "--input", program_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lint_report = parse_json_stdout(&out);
    assert_eq!(
        lint_report["ok"], true,
        "expected lint to be green after fix"
    );
}

#[test]
fn x07_fix_suggest_generics_emits_patchset_and_applies() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_fix_suggest_generics");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "main",
        "imports": [],
        "decls": [
            { "kind": "export", "names": ["main.id_u32", "main.id_i32"] },
            {
                "kind": "defn",
                "name": "main.id_u32",
                "params": [{ "name": "x", "ty": "u32" }],
                "result": "u32",
                "body": "x"
            },
            {
                "kind": "defn",
                "name": "main.id_i32",
                "params": [{ "name": "x", "ty": "i32" }],
                "result": "i32",
                "body": "x"
            }
        ]
    }))
    .expect("serialize x07AST");
    let program_path = dir.join("main.x07.json");
    write_bytes(&program_path, &program);

    let patchset_path = dir.join("suggest.patchset.json");
    let out = run_x07(&[
        "fix",
        "--input",
        program_path.to_str().unwrap(),
        "--suggest-generics",
        "--out",
        patchset_path.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], "x07.tool.fix.report@0.1.0");
    assert_eq!(v["command"], "x07.fix");
    assert_eq!(v["ok"], true);

    let patch_bytes = std::fs::read(&patchset_path).expect("read patchset");
    let patch_doc: Value = serde_json::from_slice(&patch_bytes).expect("parse patchset JSON");
    assert_eq!(patch_doc["schema_version"], X07_PATCHSET_SCHEMA_VERSION);
    assert_eq!(patch_doc["patches"].as_array().expect("patches[]").len(), 1);

    let out = run_x07(&[
        "patch",
        "apply",
        "--in",
        patchset_path.to_str().unwrap(),
        "--repo-root",
        root.to_str().unwrap(),
        "--write",
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let patched_bytes = std::fs::read(&program_path).expect("read patched x07AST");
    let file = x07c::x07ast::parse_x07ast_json(&patched_bytes).expect("parse patched x07AST");
    assert!(file.exports.contains("main.id"), "expected base export");

    let base = file
        .functions
        .iter()
        .find(|f| f.name == "main.id")
        .expect("missing base defn");
    assert_eq!(base.type_params.len(), 1);
    assert_eq!(base.type_params[0].name, "A");

    let w_u32 = file
        .functions
        .iter()
        .find(|f| f.name == "main.id_u32")
        .expect("missing u32 wrapper");
    assert_eq!(
        x07c::x07ast::expr_to_value(&w_u32.body),
        serde_json::json!(["tapp", "main.id", ["tys", "u32"], "x"])
    );
    let w_i32 = file
        .functions
        .iter()
        .find(|f| f.name == "main.id_i32")
        .expect("missing i32 wrapper");
    assert_eq!(
        x07c::x07ast::expr_to_value(&w_i32.body),
        serde_json::json!(["tapp", "main.id", ["tys", "i32"], "x"])
    );

    let out = run_x07(&["lint", "--input", program_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lint_report = parse_json_stdout(&out);
    assert_eq!(lint_report["ok"], true);
}

#[test]
fn x07_e2e_fix_inserts_tapp_then_build_succeeds() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_e2e_fix_tapp_build");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    // 1) Scaffold a minimal project (gives us x07.json + x07.lock.json layout).
    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // 2) Overwrite src/main.x07.json with a v0.4.0 entry that has:
    //    - a generic defn main.id<A>(x:A)->A
    //    - a call in solve that omits tapp (build --repair off must fail)
    let program_bytes = serde_json::to_vec(&serde_json::json!({
        "schema_version": "x07.x07ast@0.4.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.id",
                "type_params": [
                    { "name": "A" }
                ],
                "params": [
                    { "name": "x", "ty": ["t", "A"] }
                ],
                "result": ["t", "A"],
                "body": "x"
            }
        ],
        "solve": ["main.id", ["bytes.lit", "hello"]]
    }))
    .expect("serialize x07AST v0.4.0");

    let entry_path = dir.join("src/main.x07.json");
    write_bytes(&entry_path, &program_bytes);

    // 3) Build with repair OFF must fail (missing tapp).
    let out_c = dir.join("target/out.c");
    let out = run_x07_in_dir(
        &dir,
        &[
            "--out",
            out_c.to_str().unwrap(),
            "build",
            "--project",
            "x07.json",
            "--repair",
            "off",
        ],
    );
    assert!(
        !out.status.success(),
        "expected build failure without tapp; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // 4) x07 fix should insert tapp via JSON Patch quickfix.
    let out = run_x07_in_dir(
        &dir,
        &["fix", "--input", "src/main.x07.json", "--write", "--json"],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], "x07.tool.fix.report@0.1.0");
    assert_eq!(v["command"], "x07.fix");
    assert_eq!(v["ok"], true);
    assert!(
        v["diagnostics"]
            .as_array()
            .expect("diagnostics[]")
            .is_empty(),
        "expected fix report to be clean"
    );

    // 5) Verify the file was actually rewritten to tapp(...) form.
    let fixed_bytes = std::fs::read(&entry_path).expect("read fixed entry");
    let fixed_doc: serde_json::Value =
        serde_json::from_slice(&fixed_bytes).expect("parse fixed entry JSON");

    let solve = fixed_doc["solve"].as_array().expect("solve must be array");
    assert_eq!(solve.first().and_then(|v| v.as_str()), Some("tapp"));
    assert_eq!(solve.get(1).and_then(|v| v.as_str()), Some("main.id"));

    let tys = solve
        .get(2)
        .and_then(|v| v.as_array())
        .expect("tapp type args must be array");
    assert_eq!(tys.first().and_then(|v| v.as_str()), Some("tys"));
    assert_eq!(tys.get(1).and_then(|v| v.as_str()), Some("bytes"));

    // 6) Build again with repair OFF must now succeed.
    let out = run_x07_in_dir(
        &dir,
        &[
            "--out",
            out_c.to_str().unwrap(),
            "build",
            "--project",
            "x07.json",
            "--repair",
            "off",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out_c.is_file(),
        "expected out.c to exist after successful build"
    );

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
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
    assert_eq!(
        v["notes"]
            .as_array()
            .expect("notes[]")
            .iter()
            .map(|v| v.as_str().expect("notes[] string"))
            .collect::<Vec<_>>(),
        vec![
            "Agent kit: AGENT.md (self-recovery + canonical commands)",
            "Toolchain pin: x07-toolchain.toml (channel=stable; components=docs+skills)",
            "Project docs: .agent/docs/ (linked to toolchain docs)",
            "Project skills: .agent/skills/ (linked to toolchain skills)",
            "Offline docs: x07up docs path --json",
            "Skills status: x07up skills status --json",
        ]
    );
    assert_eq!(
        v["next_steps"]
            .as_array()
            .expect("next_steps[]")
            .iter()
            .map(|v| v.as_str().expect("next_steps[] string"))
            .collect::<Vec<_>>(),
        vec!["x07 run", "x07 test --manifest tests/tests.json",]
    );

    for rel in [
        "x07.json",
        "x07.lock.json",
        "src/app.x07.json",
        "src/main.x07.json",
        "x07-toolchain.toml",
        "AGENT.md",
        ".agent/docs/index.md",
        ".agent/docs/getting-started/agent-quickstart.md",
        ".agent/docs/agent/readiness-checks.md",
        ".agent/docs/examples/readiness-checks/index.md",
        ".agent/docs/examples/readiness-checks/x07-core-conformance/README.md",
        ".agent/skills/README.md",
        ".agent/skills/x07-agent-playbook/SKILL.md",
        ".gitignore",
    ] {
        assert!(dir.join(rel).is_file(), "missing {}", rel);
    }
    assert!(!dir.join("x07-package.json").exists());
    assert!(!dir.join(".codex").exists(), ".codex must not be created");

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
    assert_eq!(
        v["notes"]
            .as_array()
            .expect("notes[]")
            .iter()
            .map(|v| v.as_str().expect("notes[] string"))
            .collect::<Vec<_>>(),
        vec![
            "Package repo: x07-package.json (publish contract)",
            "Agent kit: AGENT.md (self-recovery + canonical commands)",
            "Toolchain pin: x07-toolchain.toml (channel=stable; components=docs+skills)",
            "Project docs: .agent/docs/ (linked to toolchain docs)",
            "Project skills: .agent/skills/ (linked to toolchain skills)",
            "Offline docs: x07up docs path --json",
            "Skills status: x07up skills status --json",
        ]
    );
    assert_eq!(
        v["next_steps"]
            .as_array()
            .expect("next_steps[]")
            .iter()
            .map(|v| v.as_str().expect("next_steps[] string"))
            .collect::<Vec<_>>(),
        vec![
            "Edit x07-package.json: set description/docs; bump version",
            "x07 test --manifest tests/tests.json",
            "x07 pkg pack --package . --out dist/acme-hello-demo-0.1.0.x07pkg",
            "x07 pkg login --index sparse+https://registry.x07.io/index/",
            "x07 pkg publish --index sparse+https://registry.x07.io/index/ --package .",
        ]
    );

    for rel in [
        "x07.json",
        "x07.lock.json",
        "x07-package.json",
        "x07-toolchain.toml",
        "AGENT.md",
        ".agent/docs/index.md",
        ".agent/docs/getting-started/agent-quickstart.md",
        ".agent/docs/agent/readiness-checks.md",
        ".agent/docs/examples/readiness-checks/index.md",
        ".agent/docs/examples/readiness-checks/x07-core-conformance/README.md",
        ".agent/skills/README.md",
        ".agent/skills/x07-agent-playbook/SKILL.md",
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
fn x07_init_template_lock_failure_has_next_steps() {
    let root = repo_root();
    let parent = fresh_tmp_dir(&root, "tmp_x07_init_template_lock_failure");
    if parent.exists() {
        std::fs::remove_dir_all(&parent).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&parent).expect("create tmp dir");

    // Ensure init cannot copy "official" packages from this workspace (which would mask registry issues).
    let fake_repo_root = parent.join("fake_repo_root");
    std::fs::create_dir_all(fake_repo_root.join("packages").join("ext"))
        .expect("create fake repo packages/ext");

    // Point init's internal pkg lock to an empty local index, so it fails deterministically.
    let index_dir = parent.join("fake_index");
    std::fs::create_dir_all(index_dir.join("dl")).expect("create dl dir");
    std::fs::create_dir_all(index_dir.join("api")).expect("create api dir");
    let index_url = file_url_for_dir(&index_dir);
    let cfg = serde_json::json!({
        "dl": format!("{index_url}dl/"),
        "api": format!("{index_url}api/"),
        "auth-required": false,
    });
    write_bytes(
        &index_dir.join("config.json"),
        serde_json::to_vec_pretty(&cfg).unwrap().as_slice(),
    );

    let out = Command::new(env!("CARGO_BIN_EXE_x07"))
        .current_dir(&parent)
        .env("X07_PKG_INDEX_URL", index_url.as_str())
        .env(
            "X07_REPO_ROOT",
            fake_repo_root.to_str().expect("fake repo root utf-8"),
        )
        .args(["init", "--template", "sqlite-app"])
        .output()
        .expect("run x07 init");

    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["command"], "init");
    assert_eq!(v["error"]["code"], "X07INIT_PKG_LOCK");
    assert!(
        v["next_steps"]
            .as_array()
            .expect("next_steps[]")
            .iter()
            .any(|s| {
                s.as_str()
                    .unwrap_or("")
                    .contains("x07 pkg lock --project x07.json")
            }),
        "expected next_steps to include `x07 pkg lock --project x07.json`, got: {}",
        v["next_steps"]
    );
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
}

#[test]
fn x07_pkg_add_rejects_existing_dep_with_different_version() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_add_dep_exists_different_version");
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

    let before = std::fs::read(dir.join("x07.json")).expect("read x07.json");

    let out = run_x07_in_dir(&dir, &["pkg", "add", "ext-hex-rs@0.1.4"]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["command"], "pkg.add");
    assert_eq!(v["error"]["code"], "X07PKG_DEP_EXISTS");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap_or("")
        .contains("requested ext-hex-rs@0.1.4"));

    let after = std::fs::read(dir.join("x07.json")).expect("read x07.json");
    assert_eq!(
        after, before,
        "x07.json changed despite dep-exists-different-version"
    );
}

#[test]
fn x07_pkg_versions_and_add_unpinned_and_remove() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_versions_add_unpinned_remove");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    // Create a minimal file:// sparse index with 3 versions (latest is yanked).
    let index_dir = dir.join("fake_index");
    std::fs::create_dir_all(index_dir.join("dl")).expect("create dl dir");
    std::fs::create_dir_all(index_dir.join("api")).expect("create api dir");
    let index_url = file_url_for_dir(&index_dir);

    let cfg = serde_json::json!({
        "dl": format!("{index_url}dl/"),
        "api": format!("{index_url}api/"),
        "auth-required": false,
    });
    write_bytes(
        &index_dir.join("config.json"),
        serde_json::to_vec_pretty(&cfg).unwrap().as_slice(),
    );

    let pkg = "hello";
    let rel = sparse_index_rel_path(pkg);
    let index_file = index_dir.join(rel);
    let entries = [
        serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":pkg,"version":"0.1.0","cksum":"00","yanked":false}),
        serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":pkg,"version":"0.2.0","cksum":"11","yanked":false}),
        serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":pkg,"version":"0.3.0","cksum":"22","yanked":true}),
    ];
    let mut ndjson = String::new();
    for e in entries {
        ndjson.push_str(&serde_json::to_string(&e).unwrap());
        ndjson.push('\n');
    }
    write_bytes(&index_file, ndjson.as_bytes());

    // versions: list all versions (including yanked).
    let out = run_x07(&["pkg", "versions", pkg, "--index", index_url.as_str()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["command"], "pkg.versions");
    assert_eq!(v["result"]["name"], pkg);
    assert_eq!(v["result"]["index"], index_url);
    let versions = v["result"]["versions"].as_array().expect("versions[]");
    assert_eq!(
        versions
            .iter()
            .map(|row| row["version"].as_str().expect("version string"))
            .collect::<Vec<_>>(),
        vec!["0.1.0", "0.2.0", "0.3.0"]
    );

    // init + add without pinning a version: should pick latest non-yanked (0.2.0).
    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(&dir, &["pkg", "add", pkg, "--index", index_url.as_str()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["command"], "pkg.add");
    assert_eq!(v["result"]["name"], pkg);
    assert_eq!(v["result"]["version"], "0.2.0");

    let doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    let deps = doc["dependencies"].as_array().expect("dependencies[]");
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0]["name"], pkg);
    assert_eq!(deps[0]["version"], "0.2.0");
    assert_eq!(deps[0]["path"], ".x07/deps/hello/0.2.0");

    // remove.
    let out = run_x07_in_dir(&dir, &["pkg", "remove", pkg]);
    assert_eq!(out.status.code(), Some(0));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["command"], "pkg.remove");
    assert_eq!(v["result"]["name"], pkg);
    assert_eq!(v["result"]["removed"], 1);

    let doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    let deps = doc["dependencies"].as_array().expect("dependencies[]");
    assert_eq!(deps.len(), 0);

    let out = run_x07_in_dir(&dir, &["pkg", "remove", pkg]);
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["command"], "pkg.remove");
    assert_eq!(v["error"]["code"], "X07PKG_DEP_NOT_FOUND");
}

#[test]
fn x07_doc_supports_stdlib_modules_and_symbols() {
    let dir = fresh_os_tmp_dir("x07_doc_stdlib");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let out = run_x07_in_dir(&dir, &["doc", "std.bytes"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("module: std.bytes"),
        "unexpected stdout:\n{stdout}"
    );

    let out = run_x07_in_dir(&dir, &["doc", "std.small_map.len_bytes_u32"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("std.small_map.len_bytes_u32("),
        "unexpected stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("bytes_view"),
        "unexpected stdout:\n{stdout}"
    );
}

#[test]
fn x07_doc_supports_stdlib_os_modules() {
    let dir = fresh_os_tmp_dir("x07_doc_stdlib_os");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let out = run_x07_in_dir(&dir, &["doc", "std.os.env"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("module: std.os.env"),
        "unexpected stdout:\n{stdout}"
    );
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
fn x07_pkg_add_with_closure_is_atomic_on_failure() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_add_with_closure_atomic");
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

    write_bytes(
        &dir.join("deps/baddep/0.1.0/x07-package.json"),
        br#"{
  "schema_version": "x07.package@0.1.0",
  "name": "baddep",
  "version": "0.1.0",
  "module_root": "modules",
  "modules": ["baddep.main"],
  "meta": { "requires_packages": ["NOT_A_VALID_SPEC"] }
}
"#,
    );
    write_bytes(
        &dir.join("deps/newdep/0.1.0/x07-package.json"),
        br#"{
  "schema_version": "x07.package@0.1.0",
  "name": "newdep",
  "version": "0.1.0",
  "module_root": "modules",
  "modules": ["newdep.main"]
}
"#,
    );

    let proj_path = dir.join("x07.json");
    let mut doc: Value =
        serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json")).expect("parse");
    let obj = doc.as_object_mut().expect("x07.json object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![serde_json::json!({
            "name": "baddep",
            "version": "0.1.0",
            "path": "deps/baddep/0.1.0"
        })]),
    );
    write_bytes(
        &proj_path,
        &serde_json::to_vec_pretty(&doc).expect("serialize"),
    );

    let before = std::fs::read(&proj_path).expect("read x07.json before");

    let out = run_x07_in_dir(
        &dir,
        &[
            "pkg",
            "add",
            "newdep@0.1.0",
            "--path",
            "deps/newdep/0.1.0",
            "--with-closure",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("expected NAME@VERSION"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let after = std::fs::read(&proj_path).expect("read x07.json after");
    assert_eq!(
        after, before,
        "x07.json changed despite failed --with-closure"
    );
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
fn x07_pkg_lock_transitive_conflict_includes_required_by_context() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_lock_transitive_conflict_required_by");
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

    let deps_root = dir.join(".x07").join("deps");
    let base_dir = deps_root.join("base").join("0.1.0");
    let meta_dir = deps_root.join("meta").join("0.1.0");
    std::fs::create_dir_all(&base_dir).expect("create base dir");
    std::fs::create_dir_all(&meta_dir).expect("create meta dir");

    // Minimal manifests: only `meta.requires_packages` matters for this test.
    write_bytes(&base_dir.join("x07-package.json"), b"{}\n");
    write_bytes(
        &meta_dir.join("x07-package.json"),
        br#"{"meta":{"requires_packages":["base@0.2.0"]}}
"#,
    );

    let proj_path = dir.join("x07.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
        .expect("parse x07.json");
    let obj = doc.as_object_mut().expect("x07.json must be object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![
            serde_json::json!({"name":"base","version":"0.1.0","path":".x07/deps/base/0.1.0"}),
            serde_json::json!({"name":"meta","version":"0.1.0","path":".x07/deps/meta/0.1.0"}),
        ]),
    );
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected anyhow error exit code; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("required by meta@0.1.0"),
        "stderr missing required-by context:\n{}",
        stderr
    );
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
  "schema_version": "x07.x07ast@0.4.0",
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
fn x07_cli_spec_check_schema_error_includes_row_index_and_scope() {
    let root = repo_root();
    let spec_path = root.join("target/tmp_cli_specrows_schema_err_row_index.json");
    let spec_json = r#"{"schema_version":"x07cli.specrows@0.1.0","app":{"name":"mytool","version":"0.1.0"},"rows":[["root","opt","-o","--output","output","PATH"]]}"#;
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
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    let row_diag = diags
        .iter()
        .find(|d| d.get("code").and_then(Value::as_str) == Some("ECLI_SCHEMA_INVALID"))
        .expect("ECLI_SCHEMA_INVALID diag");
    assert_eq!(row_diag["row_index"], 0);
    assert_eq!(row_diag["scope"], "root");
    let msg = row_diag["message"].as_str().expect("message string");
    assert!(
        msg.contains("expected"),
        "expected helpful row length hint, got: {msg}"
    );
    assert!(
        msg.contains("Shape:"),
        "expected expected-shape hint, got: {msg}"
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

#[test]
fn x07_ast_get_extracts_json_pointer() {
    let root = repo_root();
    let path = root.join("target/tmp_x07_ast_get.json");
    let doc = r#"{"a":[1,2,{"b":"c"}]}"#;
    write_bytes(&path, doc.as_bytes());

    let out = run_x07(&[
        "ast",
        "get",
        "--in",
        path.to_str().unwrap(),
        "--ptr",
        "/a/2/b",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["ptr"], "/a/2/b");
    assert_eq!(v["value"], "c");
}

#[test]
fn x07_ast_validate_reports_quickfix_for_unsupported_schema_version() {
    let root = repo_root();
    let program_path = root.join("target/tmp_x07_ast_validate_bad_schema.json");
    let diag_path = root.join("target/tmp_x07_ast_validate_bad_schema.x07diag.json");
    if program_path.exists() {
        std::fs::remove_file(&program_path).expect("remove old program");
    }
    if diag_path.exists() {
        std::fs::remove_file(&diag_path).expect("remove old x07diag");
    }

    let mut doc = serde_json::json!({
        "schema_version": "x07.x07ast@999.0.0",
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["bytes.alloc", 0]
    });
    write_bytes(
        &program_path,
        serde_json::to_string(&doc).expect("encode doc").as_bytes(),
    );

    let out = run_x07(&[
        "ast",
        "validate",
        "--in",
        program_path.to_str().unwrap(),
        "--x07diag",
        diag_path.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let diag_doc: Value = serde_json::from_slice(&std::fs::read(&diag_path).expect("read x07diag"))
        .expect("parse x07diag");
    let diags = diag_doc["diagnostics"].as_array().expect("diagnostics[]");
    let diag = diags
        .iter()
        .find(|d| d["code"] == "X07-SCHEMA-0002")
        .expect("expected X07-SCHEMA-0002");
    let q = diag["quickfix"]
        .as_object()
        .expect("expected quickfix object");
    assert_eq!(q["kind"], "json_patch");

    let patch_ops: Vec<x07c::diagnostics::PatchOp> =
        serde_json::from_value(q["patch"].clone()).expect("parse patch ops");
    json_patch::apply_patch(&mut doc, &patch_ops).expect("apply patch");
    write_bytes(
        &program_path,
        serde_json::to_string(&doc)
            .expect("encode patched doc")
            .as_bytes(),
    );

    let out2 = run_x07(&[
        "ast",
        "validate",
        "--in",
        program_path.to_str().unwrap(),
        "--x07diag",
        diag_path.to_str().unwrap(),
    ]);
    assert_eq!(
        out2.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out2.stderr)
    );
}

#[test]
fn x07_ast_schema_json_schema_matches_embedded_bytes() {
    let root = repo_root();
    let out = run_x07(&["ast", "schema"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut expected = std::fs::read(root.join("spec/x07ast.schema.json")).expect("read schema");
    if expected.last() != Some(&b'\n') {
        expected.push(b'\n');
    }
    assert_eq!(
        out.stdout, expected,
        "schema output must be byte-for-byte stable"
    );
}

#[test]
fn x07_ast_schema_can_emit_v0_3_snapshot() {
    let root = repo_root();
    let out = run_x07(&["ast", "schema", "--schema-version", "x07.x07ast@0.3.0"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut expected =
        std::fs::read(root.join("spec/x07ast.v0.3.0.schema.json")).expect("read schema");
    if expected.last() != Some(&b'\n') {
        expected.push(b'\n');
    }
    assert_eq!(
        out.stdout, expected,
        "schema output must be byte-for-byte stable"
    );
}

#[test]
fn x07_ast_schema_pretty_writes_json_document() {
    let root = repo_root();
    let out_path = root.join("target/tmp_x07_ast_schema_pretty.json");
    if out_path.exists() {
        std::fs::remove_file(&out_path).expect("remove old out");
    }

    let out = run_x07(&[
        "ast",
        "schema",
        "--pretty",
        "--out",
        out_path.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.is_empty(),
        "stdout should stay empty when --out is used"
    );

    let written = std::fs::read(&out_path).expect("read pretty schema");
    let pretty_doc: Value = serde_json::from_slice(&written).expect("parse pretty schema");
    let canonical_doc: Value = serde_json::from_slice(
        &std::fs::read(root.join("spec/x07ast.schema.json")).expect("read embedded schema"),
    )
    .expect("parse embedded schema");
    assert_eq!(
        pretty_doc, canonical_doc,
        "pretty output must preserve schema content"
    );
    assert!(
        String::from_utf8_lossy(&written).contains('\n'),
        "expected multiline pretty JSON"
    );
}

#[test]
fn x07_ast_grammar_cfg_emits_bundle_and_materializes_out_dir() {
    let root = repo_root();
    let out_dir = fresh_tmp_dir(&root, "tmp_x07_ast_grammar_bundle");
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir).expect("remove old out dir");
    }

    let out = run_x07(&[
        "ast",
        "grammar",
        "--cfg",
        "--out-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let bundle = parse_json_stdout(&out);
    assert_eq!(bundle["schema_version"], "x07.ast.grammar_bundle@0.1.0");
    assert_eq!(bundle["x07ast_schema_version"], "x07.x07ast@0.4.0");
    assert_eq!(bundle["format"], "gbnf_v1");

    let variants = bundle["variants"].as_array().expect("variants[]");
    assert_eq!(variants.len(), 2, "expected min+pretty variants");
    let min_cfg = variants
        .iter()
        .find(|v| v.get("name").and_then(Value::as_str) == Some("min"))
        .and_then(|v| v.get("cfg"))
        .and_then(Value::as_str)
        .expect("min cfg");
    let pretty_cfg = variants
        .iter()
        .find(|v| v.get("name").and_then(Value::as_str) == Some("pretty"))
        .and_then(|v| v.get("cfg"))
        .and_then(Value::as_str)
        .expect("pretty cfg");
    assert!(
        min_cfg.contains("root ::="),
        "min cfg must include root rule"
    );
    assert!(
        pretty_cfg.contains("root ::="),
        "pretty cfg must include root rule"
    );

    let schema_path = out_dir.join("x07ast.schema.json");
    let min_path = out_dir.join("x07ast.min.gbnf");
    let pretty_path = out_dir.join("x07ast.pretty.gbnf");
    let semantic_path = out_dir.join("x07ast.semantic.json");
    let manifest_path = out_dir.join("manifest.json");

    for path in [
        &schema_path,
        &min_path,
        &pretty_path,
        &semantic_path,
        &manifest_path,
    ] {
        assert!(path.is_file(), "missing {}", path.display());
    }

    let mut expected_schema = std::fs::read(root.join("spec/x07ast.schema.json")).expect("schema");
    if expected_schema.last() != Some(&b'\n') {
        expected_schema.push(b'\n');
    }
    let mut expected_min = std::fs::read(root.join("spec/x07ast.min.gbnf")).expect("min gbnf");
    if expected_min.last() != Some(&b'\n') {
        expected_min.push(b'\n');
    }
    let mut expected_pretty =
        std::fs::read(root.join("spec/x07ast.pretty.gbnf")).expect("pretty gbnf");
    if expected_pretty.last() != Some(&b'\n') {
        expected_pretty.push(b'\n');
    }
    let mut expected_semantic =
        std::fs::read(root.join("spec/x07ast.semantic.json")).expect("semantic supplement");
    if expected_semantic.last() != Some(&b'\n') {
        expected_semantic.push(b'\n');
    }

    let written_schema = std::fs::read(&schema_path).expect("read materialized schema");
    let written_min = std::fs::read(&min_path).expect("read materialized min grammar");
    let written_pretty = std::fs::read(&pretty_path).expect("read materialized pretty grammar");
    let written_semantic = std::fs::read(&semantic_path).expect("read materialized semantic");
    assert_eq!(written_schema, expected_schema);
    assert_eq!(written_min, expected_min);
    assert_eq!(written_pretty, expected_pretty);
    assert_eq!(written_semantic, expected_semantic);

    assert_eq!(
        bundle["sha256"]["min_cfg"],
        sha256_hex(min_cfg.as_bytes()),
        "bundle min sha should match variant bytes"
    );
    assert_eq!(
        bundle["sha256"]["pretty_cfg"],
        sha256_hex(pretty_cfg.as_bytes()),
        "bundle pretty sha should match variant bytes"
    );
    assert_eq!(
        bundle["sha256"]["semantic_supplement"],
        sha256_hex(&written_semantic),
        "bundle semantic sha should match semantic bytes"
    );

    let semantic_from_bundle = bundle["semantic_supplement"].clone();
    let semantic_from_file: Value =
        serde_json::from_slice(&written_semantic).expect("parse semantic from file");
    assert_eq!(
        semantic_from_bundle, semantic_from_file,
        "bundle semantic object should match materialized semantic file"
    );

    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    assert_eq!(manifest["schema_version"], "x07.genpack.manifest@0.1.0");
    assert_eq!(manifest["pack"], "x07-genpack-x07ast");
    assert_eq!(manifest["pack_version"], "0.1.0");
    assert_eq!(manifest["x07ast_schema_version"], "x07.x07ast@0.4.0");
    let artifacts = manifest["artifacts"].as_array().expect("artifacts[]");
    assert_eq!(artifacts.len(), 4);
    let get_sha = |name: &str| {
        artifacts
            .iter()
            .find(|a| a.get("name").and_then(Value::as_str) == Some(name))
            .and_then(|a| a.get("sha256"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(get_sha("x07ast.schema.json"), sha256_hex(&written_schema));
    assert_eq!(get_sha("x07ast.min.gbnf"), sha256_hex(&written_min));
    assert_eq!(get_sha("x07ast.pretty.gbnf"), sha256_hex(&written_pretty));
    assert_eq!(
        get_sha("x07ast.semantic.json"),
        sha256_hex(&written_semantic)
    );
}

#[test]
fn x07_run_auto_adds_missing_external_package() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_auto_deps");
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

    let app = r#"{"schema_version":"x07.x07ast@0.3.0","kind":"module","module_id":"app","imports":["ext.json.data_model"],"decls":[{"kind":"export","names":["app.solve"]},{"kind":"defn","name":"app.solve","params":[],"result":"bytes","body":["begin",["let","json",["bytes.lit","{\"x\":1}"]],["ext.json.data_model.parse",["bytes.view","json"]]]}]}"#;
    write_bytes(&dir.join("src/app.x07.json"), app.as_bytes());
    let main = r#"{"schema_version":"x07.x07ast@0.3.0","kind":"entry","module_id":"main","imports":["app"],"decls":[],"solve":["app.solve"]}"#;
    write_bytes(&dir.join("src/main.x07.json"), main.as_bytes());

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&dir)
        .env_remove("X07_INTERNAL_AUTO_DEPS")
        .args(["run", "--project", "x07.json"])
        .output()
        .expect("run x07");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let proj: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    let deps = proj["dependencies"].as_array().expect("dependencies[]");
    assert!(
        deps.iter().any(|d| d["name"] == "ext-json-rs"),
        "expected auto-added ext-json-rs dependency"
    );

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_review_diff_reports_high_signal_changes_and_is_deterministic() {
    let root = repo_root();
    let fixtures = fixtures_root().join("review");
    let before = fixtures.join("before");
    let after = fixtures.join("after");
    assert!(before.is_dir(), "missing {}", before.display());
    assert!(after.is_dir(), "missing {}", after.display());

    let work = fresh_tmp_dir(&root, "tmp_x07_review_diff");
    let before_copy = work.join("before");
    let after_copy = work.join("after");
    copy_dir_recursive(&before, &before_copy);
    copy_dir_recursive(&after, &after_copy);

    let json_out_1 = work.join("review-1.json");
    let html_out_1 = work.join("review-1.html");
    let json_out_2 = work.join("review-2.json");
    let html_out_2 = work.join("review-2.html");

    let out = run_x07(&[
        "review",
        "diff",
        "--from",
        before_copy.to_str().unwrap(),
        "--to",
        after_copy.to_str().unwrap(),
        "--mode",
        "project",
        "--json-out",
        json_out_1.to_str().unwrap(),
        "--html-out",
        html_out_1.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report: Value =
        serde_json::from_slice(&std::fs::read(&json_out_1).expect("read review json"))
            .expect("parse review json");
    assert_eq!(report["schema_version"], X07_REVIEW_DIFF_SCHEMA_VERSION);
    assert!(
        report["highlights"]["world_changes"]
            .as_array()
            .expect("world_changes[]")
            .iter()
            .any(|c| c["kind"] == "project_profile_world" || c["kind"] == "arch_node_world"),
        "expected world changes in highlights"
    );
    assert!(
        !report["highlights"]["budget_changes"]
            .as_array()
            .expect("budget_changes[]")
            .is_empty(),
        "expected budget changes in highlights"
    );
    assert!(
        !report["highlights"]["policy_changes"]
            .as_array()
            .expect("policy_changes[]")
            .is_empty(),
        "expected policy changes in highlights"
    );
    assert!(
        report["highlights"]["policy_changes"]
            .as_array()
            .expect("policy_changes[]")
            .iter()
            .any(|c| c["kind"] == "policy_allowlist"),
        "expected policy allowlist changes in highlights"
    );
    assert!(
        !report["highlights"]["capability_changes"]
            .as_array()
            .expect("capability_changes[]")
            .is_empty(),
        "expected capability changes in highlights"
    );
    assert!(
        report["files"]
            .as_array()
            .expect("files[]")
            .iter()
            .filter(|f| f["kind"] == "x07ast")
            .flat_map(|f| {
                f["module"]["decls_changed"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect::<Vec<Value>>()
            })
            .flat_map(|d| {
                d["body"]["ops"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect::<Vec<Value>>()
            })
            .any(|op| op["op"] == "move"),
        "expected at least one decl move operation"
    );

    let out = run_x07(&[
        "review",
        "diff",
        "--from",
        before_copy.to_str().unwrap(),
        "--to",
        after_copy.to_str().unwrap(),
        "--mode",
        "project",
        "--json-out",
        json_out_2.to_str().unwrap(),
        "--html-out",
        html_out_2.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json_1 = std::fs::read(&json_out_1).expect("read review-1 json");
    let json_2 = std::fs::read(&json_out_2).expect("read review-2 json");
    assert_eq!(json_1, json_2, "review json output must be deterministic");

    let html_1 = std::fs::read(&html_out_1).expect("read review-1 html");
    let html_2 = std::fs::read(&html_out_2).expect("read review-2 html");
    assert_eq!(html_1, html_2, "review html output must be deterministic");
    assert!(
        String::from_utf8_lossy(&html_1).contains("risk summary:"),
        "expected risk summary strip in HTML"
    );
}

#[test]
fn x07_review_diff_fail_on_allow_unsafe_returns_20() {
    let root = repo_root();
    let fixtures = fixtures_root().join("review");
    let before = fixtures.join("before");
    let after = fixtures.join("after");

    let work = fresh_tmp_dir(&root, "tmp_x07_review_diff_fail_on");
    let before_copy = work.join("before");
    let after_copy = work.join("after");
    copy_dir_recursive(&before, &before_copy);
    copy_dir_recursive(&after, &after_copy);

    let json_out = work.join("review.json");
    let html_out = work.join("review.html");

    let out = run_x07(&[
        "review",
        "diff",
        "--from",
        before_copy.to_str().unwrap(),
        "--to",
        after_copy.to_str().unwrap(),
        "--mode",
        "project",
        "--json-out",
        json_out.to_str().unwrap(),
        "--html-out",
        html_out.to_str().unwrap(),
        "--fail-on",
        "allow-unsafe",
    ]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(json_out.is_file(), "missing {}", json_out.display());
    assert!(html_out.is_file(), "missing {}", html_out.display());
}

#[test]
fn x07_trust_report_outputs_expected_schema_and_flags() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_report");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(&dir, &["policy", "init", "--template", "http-client"]);
    assert_eq!(out.status.code(), Some(0));
    std::fs::copy(root.join("stdlib.lock"), dir.join("stdlib.lock")).expect("copy stdlib.lock");

    let project_path = dir.join("x07.json");
    let mut project_doc: Value =
        serde_json::from_slice(&std::fs::read(&project_path).unwrap()).expect("parse x07.json");
    project_doc["profiles"]["sandbox"] = serde_json::json!({
      "world": "run-os-sandboxed",
      "policy": ".x07/policies/base/http-client.sandbox.base.policy.json",
      "solve_fuel": 4096,
      "max_memory_bytes": 8388608
    });
    project_doc["default_profile"] = Value::String("sandbox".to_string());
    write_json(&project_path, &project_doc);

    let entry_path = dir.join("src/main.x07.json");
    let entry_doc = serde_json::json!({
      "schema_version": X07AST_SCHEMA_VERSION,
      "kind": "entry",
      "module_id": "main",
      "imports": [],
      "decls": [],
      "solve": [
        "begin",
        [
          "budget.scope_v1",
          {
            "label": "sandbox",
            "mode": "strict",
            "limits": {
              "fuel": 32
            }
          }
        ],
        ["std.os.time.now_unix_ms_v1"],
        ["bytes.lit", "ok"]
      ]
    });
    write_json(&entry_path, &entry_doc);

    let policy_path = dir.join(".x07/policies/base/http-client.sandbox.base.policy.json");
    let mut policy_doc: Value =
        serde_json::from_slice(&std::fs::read(&policy_path).unwrap()).expect("parse policy");
    policy_doc["language"]["allow_unsafe"] = Value::Bool(true);
    policy_doc["language"]["allow_ffi"] = Value::Bool(true);
    policy_doc["net"]["enabled"] = Value::Bool(true);
    policy_doc["process"]["enabled"] = Value::Bool(true);
    policy_doc["time"]["allow_wall_clock"] = Value::Bool(true);
    write_json(&policy_path, &policy_doc);

    let trust_json = dir.join("trust.json");
    let trust_html = dir.join("trust.html");

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "report",
            "--project",
            "x07.json",
            "--profile",
            "sandbox",
            "--out",
            trust_json.to_str().unwrap(),
            "--html-out",
            trust_html.to_str().unwrap(),
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(trust_json.is_file(), "missing {}", trust_json.display());
    assert!(trust_html.is_file(), "missing {}", trust_html.display());

    let report: Value =
        serde_json::from_slice(&std::fs::read(&trust_json).expect("read trust report"))
            .expect("parse trust report");
    assert_eq!(report["schema_version"], X07_TRUST_REPORT_SCHEMA_VERSION);
    assert_eq!(report["project"]["world"], "run-os-sandboxed");
    assert!(
        report["budgets"]["scopes"]
            .as_array()
            .expect("scopes[]")
            .iter()
            .any(|s| s["kind"] == "inline_v1"),
        "expected budget.scope_v1 detection"
    );
    assert!(
        report["capabilities"]["used"]["namespaces"]
            .as_array()
            .expect("namespaces[]")
            .iter()
            .any(|v| v == "std.os.time."),
        "expected std.os.time namespace detection"
    );
    let flags = report["nondeterminism"]["flags"]
        .as_array()
        .expect("flags[]");
    assert!(
        flags.iter().any(|f| f["kind"] == "allow_unsafe"),
        "expected allow_unsafe flag"
    );
    assert!(
        flags.iter().any(|f| f["kind"] == "allow_ffi"),
        "expected allow_ffi flag"
    );
    assert!(
        flags.iter().any(|f| f["kind"] == "net_enabled"),
        "expected net_enabled flag"
    );
    assert!(
        flags.iter().any(|f| f["kind"] == "process_enabled"),
        "expected process_enabled flag"
    );
    assert!(
        report["sbom"]["components"]
            .as_array()
            .expect("components[]")
            .iter()
            .any(|c| c["kind"] == "stdlib"),
        "expected stdlib sbom components from stdlib.lock"
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "report",
            "--project",
            "x07.json",
            "--profile",
            "sandbox",
            "--out",
            trust_json.to_str().unwrap(),
            "--fail-on",
            "net-enabled",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn x07_tool_wrapper_json_for_guide_scope() {
    let out = run_x07(&["guide", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).expect("parse tool wrapper report");
    assert_eq!(report["schema_version"], "x07.tool.guide.report@0.1.0");
    assert_eq!(report["command"], "x07.guide");
    assert_eq!(report["ok"], Value::Bool(true));
    assert!(
        report["result"]["stdout"]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("# Language Guide"),
        "expected captured guide markdown in wrapped stdout"
    );
}

#[test]
fn x07_patch_apply_dry_run_and_write_modes() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_patch_apply");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let target = dir.join("doc.json");
    write_json(&target, &serde_json::json!({ "a": 1, "b": true }));

    let patchset = dir.join("changes.patchset.json");
    write_json(
        &patchset,
        &serde_json::json!({
            "schema_version": "x07.patchset@0.1.0",
            "patches": [
                {
                    "path": "doc.json",
                    "patch": [
                        { "op": "replace", "path": "/a", "value": 7 }
                    ]
                }
            ]
        }),
    );

    let out = run_x07(&[
        "patch",
        "apply",
        "--in",
        patchset.to_str().unwrap(),
        "--repo-root",
        dir.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).expect("parse dry-run report");
    assert_eq!(report["ok"], Value::Bool(true));
    let unchanged: Value =
        serde_json::from_slice(&std::fs::read(&target).expect("read unchanged target"))
            .expect("parse unchanged target");
    assert_eq!(unchanged["a"], Value::from(1));

    let out = run_x07(&[
        "patch",
        "apply",
        "--in",
        patchset.to_str().unwrap(),
        "--repo-root",
        dir.to_str().unwrap(),
        "--write",
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let changed: Value =
        serde_json::from_slice(&std::fs::read(&target).expect("read changed target"))
            .expect("parse changed target");
    assert_eq!(changed["a"], Value::from(7));
}

#[test]
fn x07_scope_json_schema_for_arch_check_is_available() {
    let out = run_x07(&["arch", "check", "--json-schema"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let schema: Value = serde_json::from_slice(&out.stdout).expect("parse schema");
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        Value::String("x07.tool.arch.check.report@0.1.0".to_string())
    );
    assert_eq!(
        schema["properties"]["command"]["const"],
        Value::String("x07.arch.check".to_string())
    );
}
