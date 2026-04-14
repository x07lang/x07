use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;

use serde_json::Value;
use sha2::{Digest, Sha256};
use x07_contracts::{
    PACKAGE_MANIFEST_SCHEMA_VERSION, PROJECT_LOCKFILE_SCHEMA_VERSION,
    PROJECT_MANIFEST_SCHEMA_VERSION, X07AST_SCHEMA_VERSION, X07C_REPORT_SCHEMA_VERSION,
    X07DIAG_SCHEMA_VERSION, X07TEST_SCHEMA_VERSION, X07_AGENT_CONTEXT_SCHEMA_VERSION,
    X07_ARCH_REPORT_SCHEMA_VERSION, X07_CONTRACT_REPRO_SCHEMA_VERSION,
    X07_OS_RUNNER_REPORT_SCHEMA_VERSION, X07_PATCHSET_SCHEMA_VERSION,
    X07_PKG_ADVISORY_SCHEMA_VERSION, X07_POLICY_INIT_REPORT_SCHEMA_VERSION,
    X07_REVIEW_DIFF_SCHEMA_VERSION, X07_RUN_REPORT_SCHEMA_VERSION, X07_TRUST_REPORT_SCHEMA_VERSION,
    X07_VERIFY_COVERAGE_SCHEMA_VERSION, X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_VERSION,
    X07_VERIFY_PROOF_SUMMARY_SCHEMA_VERSION, X07_VERIFY_REPORT_SCHEMA_VERSION,
    X07_VERIFY_SUMMARY_SCHEMA_VERSION,
};
use x07_runner_common::sandbox_backend::{ENV_ACCEPT_WEAKER_ISOLATION, ENV_SANDBOX_BACKEND};
use x07c::{json_patch, project};

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
static MCP_NATIVE_BACKENDS_READY: Once = Once::new();
static RUNNER_BINARIES_READY: Once = Once::new();

fn repo_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn ensure_mcp_native_backends_staged() {
    MCP_NATIVE_BACKENDS_READY.call_once(|| {
        let root = repo_root();
        for script in [
            "scripts/ci/ensure_ext_stdio_backend.sh",
            "scripts/ci/ensure_ext_rand_backend.sh",
            "scripts/ci/ensure_ext_jsonschema_backend.sh",
        ] {
            let script_path = root.join(script);
            let out = Command::new(&script_path)
                .current_dir(&root)
                .output()
                .unwrap_or_else(|e| panic!("run {}: {e}", script_path.display()));
            assert!(
                out.status.success(),
                "script failed: {}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
                script_path.display(),
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr),
            );
        }
    });
}

fn ensure_runner_binaries_staged() {
    RUNNER_BINARIES_READY.call_once(|| {
        let root = repo_root();
        let out = Command::new("cargo")
            .current_dir(&root)
            .args(["build", "-p", "x07-host-runner", "-p", "x07-os-runner"])
            .output()
            .expect("build runner binaries");
        assert!(
            out.status.success(),
            "runner build failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    });
}

fn run_x07(args: &[&str]) -> std::process::Output {
    ensure_runner_binaries_staged();
    let exe = env!("CARGO_BIN_EXE_x07");
    Command::new(exe)
        .env(ENV_SANDBOX_BACKEND, "os")
        .env(ENV_ACCEPT_WEAKER_ISOLATION, "1")
        .args(args)
        .output()
        .expect("run x07")
}

fn run_x07_in_dir(dir: &Path, args: &[&str]) -> std::process::Output {
    ensure_runner_binaries_staged();
    let exe = env!("CARGO_BIN_EXE_x07");
    Command::new(exe)
        .current_dir(dir)
        .env(ENV_SANDBOX_BACKEND, "os")
        .env(ENV_ACCEPT_WEAKER_ISOLATION, "1")
        .args(args)
        .output()
        .expect("run x07")
}

#[cfg(unix)]
fn run_x07_in_dir_with_path_prefixes(
    dir: &Path,
    args: &[&str],
    path_prefixes: &[PathBuf],
) -> std::process::Output {
    ensure_runner_binaries_staged();
    let exe = env!("CARGO_BIN_EXE_x07");
    let mut paths = path_prefixes.to_vec();
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    Command::new(exe)
        .current_dir(dir)
        .env(ENV_SANDBOX_BACKEND, "os")
        .env(ENV_ACCEPT_WEAKER_ISOLATION, "1")
        .env("PATH", std::env::join_paths(paths).expect("join PATH"))
        .args(args)
        .output()
        .expect("run x07")
}

#[cfg(unix)]
fn run_x07_in_dir_with_fake_prove_solvers(dir: &Path, args: &[&str]) -> std::process::Output {
    let solver_dir = dir.join("bin");
    write_fake_prove_solvers(&solver_dir);
    run_x07_in_dir_with_path_prefixes(dir, args, &[solver_dir])
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

fn write_fake_file_index_config(index_dir: &Path) -> String {
    std::fs::create_dir_all(index_dir.join("dl")).expect("create dl dir");
    std::fs::create_dir_all(index_dir.join("api")).expect("create api dir");
    let index_url = file_url_for_dir(index_dir);

    let cfg = serde_json::json!({
        "dl": format!("{index_url}dl/"),
        "api": format!("{index_url}api/"),
        "auth-required": false,
    });
    write_bytes(
        &index_dir.join("config.json"),
        serde_json::to_vec_pretty(&cfg).unwrap().as_slice(),
    );

    index_url
}

fn write_index_entries_ndjson(index_dir: &Path, package_name: &str, entries: &[Value]) {
    let rel = sparse_index_rel_path(package_name);
    let index_file = index_dir.join(rel);
    let mut ndjson = String::new();
    for e in entries {
        ndjson.push_str(&serde_json::to_string(e).unwrap());
        ndjson.push('\n');
    }
    write_bytes(&index_file, ndjson.as_bytes());
}

fn write_minimal_pkg_manifest(dir: &Path, name: &str, version: &str, requires_packages: &[&str]) {
    std::fs::create_dir_all(dir).expect("create package dir");
    let mut doc = serde_json::json!({
        "schema_version": PACKAGE_MANIFEST_SCHEMA_VERSION,
        "name": name,
        "version": version,
        "module_root": "modules",
        "modules": [],
    });
    if !requires_packages.is_empty() {
        doc["meta"] = serde_json::json!({
            "requires_packages": requires_packages,
        });
    }
    write_bytes(
        &dir.join("x07-package.json"),
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );
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

#[test]
fn x07_check_schema_discovery() {
    let out = run_x07(&["check", "--json-schema-id"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "x07.tool.check.report@0.1.0\n"
    );

    let out = run_x07(&["check", "--json-schema"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let schema: Value = serde_json::from_slice(&out.stdout).expect("parse schema JSON");
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        "x07.tool.check.report@0.1.0"
    );
    assert_eq!(schema["properties"]["command"]["const"], "x07.check");
}

#[test]
fn x07_prove_schema_discovery() {
    let out = run_x07(&["prove", "--json-schema-id"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "x07.tool.prove.report@0.2.0\n"
    );

    let out = run_x07(&["prove", "--json-schema"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let schema: Value = serde_json::from_slice(&out.stdout).expect("parse schema JSON");
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        "x07.tool.prove.report@0.2.0"
    );
    assert_eq!(schema["properties"]["command"]["const"], "x07.prove");
}

#[test]
fn x07_prove_check_schema_discovery() {
    let out = run_x07(&["prove", "check", "--json-schema-id"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "x07.tool.prove.check.report@0.2.0\n"
    );

    let out = run_x07(&["prove", "check", "--json-schema"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let schema: Value = serde_json::from_slice(&out.stdout).expect("parse schema JSON");
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        "x07.tool.prove.check.report@0.2.0"
    );
    assert_eq!(schema["properties"]["command"]["const"], "x07.prove.check");
}

#[test]
fn x07_check_valid_project_no_emit() {
    let root = fresh_tmp_dir(&repo_root(), "x07_check_valid");
    std::fs::create_dir_all(&root).expect("create root");

    let project = serde_json::to_vec(&serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "world": "solve-pure",
        "entry": "src/main.x07.json",
        "module_roots": ["src"],
        "dependencies": [],
        "lockfile": "x07.lock.json"
    }))
    .expect("serialize x07.json");
    write_bytes(&root.join("x07.json"), &project);

    write_lockfile_for_project_bytes(&root, &project);

    let entry = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["bytes.alloc", 0]
    }))
    .expect("serialize entry x07AST");
    write_bytes(&root.join("src/main.x07.json"), &entry);

    let out = run_x07_in_dir(&root, &["check", "--project", "x07.json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty(), "expected empty stderr");
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07DIAG_SCHEMA_VERSION);
    assert_eq!(v["ok"], true);

    let mut saw_c = false;
    for entry in walkdir::WalkDir::new(&root).into_iter().flatten() {
        if entry.file_type().is_file()
            && entry.path().extension().and_then(|e| e.to_str()) == Some("c")
        {
            saw_c = true;
            break;
        }
    }
    assert!(!saw_c, "expected no *.c outputs under {}", root.display());

    std::fs::remove_dir_all(&root).expect("cleanup tmp dir");
}

#[test]
fn x07_check_project_wide_typecheck_across_modules() {
    let root = fresh_tmp_dir(&repo_root(), "x07_check_type");
    std::fs::create_dir_all(&root).expect("create root");

    let project = serde_json::to_vec(&serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "world": "solve-pure",
        "entry": "src/main.x07.json",
        "module_roots": ["src"],
        "dependencies": [],
        "lockfile": "x07.lock.json"
    }))
    .expect("serialize x07.json");
    write_bytes(&root.join("x07.json"), &project);

    write_lockfile_for_project_bytes(&root, &project);

    let foo = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "foo",
        "imports": [],
        "decls": [
            { "kind": "export", "names": ["foo.ret_i32"] },
            { "kind": "defn", "name": "foo.ret_i32", "params": [], "result": "i32", "body": 0 }
        ]
    }))
    .expect("serialize foo module");
    write_bytes(&root.join("src/foo.x07.json"), &foo);

    let entry = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": ["foo"],
        "decls": [],
        "solve": ["foo.ret_i32"]
    }))
    .expect("serialize entry x07AST");
    write_bytes(&root.join("src/main.x07.json"), &entry);

    let out = run_x07_in_dir(&root, &["check", "--project", "x07.json"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected failure; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty(), "expected empty stderr");
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07DIAG_SCHEMA_VERSION);
    assert_eq!(v["ok"], false);

    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(
        diags.iter().any(|d| {
            d["severity"] == "error"
                && d["stage"] == "type"
                && d["loc"]["kind"] == "x07ast"
                && d["loc"]["ptr"] == "/solve"
        }),
        "expected type error at /solve; diagnostics:\n{}",
        serde_json::to_string_pretty(&v["diagnostics"]).unwrap()
    );

    std::fs::remove_dir_all(&root).expect("cleanup tmp dir");
}

#[test]
fn x07_check_stdlib_call_arg_mismatch_is_type_error_not_internal() {
    let root = fresh_tmp_dir(&repo_root(), "x07_check_stdlib_call_arg_mismatch");
    std::fs::create_dir_all(&root).expect("create root");

    let project = serde_json::to_vec(&serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "world": "solve-pure",
        "entry": "src/main.x07.json",
        "module_roots": ["src"],
        "dependencies": [],
        "lockfile": "x07.lock.json"
    }))
    .expect("serialize x07.json");
    write_bytes(&root.join("x07.json"), &project);
    write_lockfile_for_project_bytes(&root, &project);

    let entry = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": ["std.bytes", "std.codec"],
        "decls": [],
        "solve": ["std.codec.write_u32_le", ["std.bytes.len", 0]]
    }))
    .expect("serialize entry x07AST");
    write_bytes(&root.join("src/main.x07.json"), &entry);

    let out = run_x07_in_dir(&root, &["check", "--project", "x07.json"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected failure; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07DIAG_SCHEMA_VERSION);
    assert_eq!(v["ok"], false);

    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(
        !diags.iter().any(|d| d["code"] == "X07-INTERNAL-0001"),
        "did not expect internal error diagnostics; got:\n{}",
        serde_json::to_string_pretty(diags).unwrap()
    );
    let d = diags
        .iter()
        .find(|d| d["code"] == "X07-TYPE-CALL-0002" && d["stage"] == "type")
        .unwrap_or_else(|| {
            panic!(
                "expected X07-TYPE-CALL-0002 at stage=type; got:\n{}",
                serde_json::to_string_pretty(diags).unwrap()
            )
        });
    assert_eq!(d["data"]["callee"], "std.bytes.len");
    assert_eq!(d["data"]["arg_index"], 0);
    assert_eq!(d["data"]["expected"], "bytes_view");
    assert_eq!(d["data"]["got"], "i32");

    std::fs::remove_dir_all(&root).expect("cleanup tmp dir");
}

#[test]
fn x07_check_unknown_callee_in_stdlib_is_type_error_not_codegen() {
    let root = fresh_tmp_dir(&repo_root(), "x07_check_unknown_callee_in_stdlib");
    std::fs::create_dir_all(&root).expect("create root");

    let project = serde_json::to_vec(&serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "world": "solve-pure",
        "entry": "src/main.x07.json",
        "module_roots": ["src"],
        "dependencies": [],
        "lockfile": "x07.lock.json"
    }))
    .expect("serialize x07.json");
    write_bytes(&root.join("x07.json"), &project);
    write_lockfile_for_project_bytes(&root, &project);

    let entry = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": ["std.bytes", "std.codec"],
        "decls": [],
        "solve": ["std.codec.write_u32_le", ["std.bytes.lenn", ["bytes.lit", "abc"]]]
    }))
    .expect("serialize entry x07AST");
    write_bytes(&root.join("src/main.x07.json"), &entry);

    let out = run_x07_in_dir(&root, &["check", "--project", "x07.json"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected failure; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07DIAG_SCHEMA_VERSION);
    assert_eq!(v["ok"], false);

    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(
        !diags.iter().any(|d| d["code"] == "X07-INTERNAL-0001"),
        "did not expect internal error diagnostics; got:\n{}",
        serde_json::to_string_pretty(diags).unwrap()
    );
    let d = diags
        .iter()
        .find(|d| d["code"] == "X07-TYPE-CALL-0001" && d["stage"] == "type")
        .unwrap_or_else(|| {
            panic!(
                "expected X07-TYPE-CALL-0001 at stage=type; got:\n{}",
                serde_json::to_string_pretty(diags).unwrap()
            )
        });
    assert_eq!(d["data"]["callee"], "std.bytes.lenn");

    std::fs::remove_dir_all(&root).expect("cleanup tmp dir");
}

#[test]
fn x07_check_surfaces_move_errors() {
    let root = fresh_tmp_dir(&repo_root(), "x07_check_move");
    std::fs::create_dir_all(&root).expect("create root");

    let project = serde_json::to_vec(&serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "world": "solve-pure",
        "entry": "src/main.x07.json",
        "module_roots": ["src"],
        "dependencies": [],
        "lockfile": "x07.lock.json"
    }))
    .expect("serialize x07.json");
    write_bytes(&root.join("x07.json"), &project);

    write_lockfile_for_project_bytes(&root, &project);

    let entry = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["begin", ["let", "x", ["bytes.lit", "a"]], ["bytes.concat", "x", "x"]]
    }))
    .expect("serialize entry x07AST");
    write_bytes(&root.join("src/main.x07.json"), &entry);

    let out = run_x07_in_dir(&root, &["check", "--project", "x07.json"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected failure; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty(), "expected empty stderr");
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07DIAG_SCHEMA_VERSION);
    assert_eq!(v["ok"], false);

    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(
        diags.iter().any(|d| d["code"]
            .as_str()
            .is_some_and(|c| c.starts_with("X07-MOVE-"))),
        "expected X07-MOVE-* diagnostic; diagnostics:\n{}",
        serde_json::to_string_pretty(&v["diagnostics"]).unwrap()
    );

    std::fs::remove_dir_all(&root).expect("cleanup tmp dir");
}

#[test]
fn x07_check_backend_use_after_move_has_quickfix_and_can_apply_it() {
    let root = fresh_tmp_dir(&repo_root(), "x07_check_backend_move_qf");
    std::fs::create_dir_all(&root).expect("create root");

    let project = serde_json::to_vec(&serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "world": "solve-pure",
        "entry": "src/main.x07.json",
        "module_roots": ["src"],
        "dependencies": [],
        "lockfile": "x07.lock.json"
    }))
    .expect("serialize x07.json");
    write_bytes(&root.join("x07.json"), &project);

    write_lockfile_for_project_bytes(&root, &project);

    let entry = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            { "kind": "export", "names": ["main.id_bytes"] },
            {
                "kind": "defn",
                "name": "main.id_bytes",
                "params": [{ "name": "b", "ty": "bytes" }],
                "result": "bytes",
                "body": "b"
            }
        ],
        "solve": [
            "begin",
            ["let", "x", ["bytes.lit", "a"]],
            ["let", "y", ["main.id_bytes", "x"]],
            ["bytes.concat", "y", "x"]
        ]
    }))
    .expect("serialize entry x07AST");
    let program_path = root.join("src/main.x07.json");
    write_bytes(&program_path, &entry);

    let out = run_x07_in_dir(&root, &["check", "--project", "x07.json"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected failure; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty(), "expected empty stderr");

    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07DIAG_SCHEMA_VERSION);
    assert_eq!(v["ok"], false);

    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    let diag = diags
        .iter()
        .find(|d| d["code"] == "X07-MOVE-0901")
        .expect("expected X07-MOVE-0901 diagnostic");
    let q = diag["quickfix"]
        .as_object()
        .expect("expected quickfix object");
    assert_eq!(q["kind"], "json_patch");
    assert_eq!(q["note"], "Copy before move");

    let patch_ops: Vec<x07c::diagnostics::PatchOp> =
        serde_json::from_value(q["patch"].clone()).expect("parse patch ops");
    assert_eq!(patch_ops.len(), 1, "expected a single patch op");
    match &patch_ops[0] {
        x07c::diagnostics::PatchOp::Replace { path, value: _ } => {
            assert_eq!(path, "/solve/2/2/1");
        }
        other => panic!("expected replace op, got: {other:?}"),
    }

    let mut doc: Value =
        serde_json::from_slice(&std::fs::read(&program_path).expect("read program"))
            .expect("parse program");
    json_patch::apply_patch(&mut doc, &patch_ops).expect("apply patch");
    write_bytes(
        &program_path,
        serde_json::to_string_pretty(&doc)
            .expect("encode patched program")
            .as_bytes(),
    );

    let out2 = run_x07_in_dir(&root, &["check", "--project", "x07.json"]);
    assert_eq!(
        out2.status.code(),
        Some(0),
        "expected ok after patch; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out2.stdout),
        String::from_utf8_lossy(&out2.stderr)
    );
    assert!(
        out2.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out2.stderr)
    );
    let v2 = parse_json_stdout(&out2);
    assert_eq!(v2["schema_version"], X07DIAG_SCHEMA_VERSION);
    assert_eq!(v2["ok"], true);

    std::fs::remove_dir_all(&root).expect("cleanup tmp dir");
}

#[test]
fn arch_check_suggests_and_applies_patches_for_rr_sorting_and_sanitizer_defaults() {
    let root = fresh_tmp_dir(&repo_root(), "arch_check_suggest");
    std::fs::create_dir_all(&root).expect("create root");

    let manifest = r#"{
  "schema_version": "x07.arch.manifest@0.3.0",
  "repo": { "id": "tmp_arch_check", "root": "." },
  "externals": { "allowed_import_prefixes": ["std.", "ext."], "allowed_exact": [] },
  "nodes": [],
  "rules": [],
  "checks": {
    "deny_cycles": false,
    "deny_orphans": false,
    "enforce_visibility": false,
    "enforce_world_caps": false
  },
  "tool_budgets": {
    "max_modules": 5000,
    "max_edges": 50000,
    "max_diags": 2000,
    "contracts_budgets": { "max_contract_files": 2000, "max_contract_bytes": 67108864 }
  },
  "contracts_v1": {
    "rr": {
      "index_path": "arch/rr/index.x07rr.json",
      "gen_dir": "arch/rr",
      "require_policy_for_os_calls": true
    },
    "canonical_json": { "mode": "jcs_rfc8785_v1" }
  }
}
"#;
    write_bytes(
        &root.join("arch/manifest.x07arch.json"),
        manifest.as_bytes(),
    );

    let rr_index = r#"{
  "schema_version": "x07.arch.rr.index@0.1.0",
  "policies": [
    {
      "id": "p1",
      "policy_path": "arch/rr/policies/p1.policy.json",
      "sanitize_id": "sanitize_none_v1",
      "sanitize_path": "arch/rr/sanitizers/sanitize_none_v1.sanitize.json",
      "worlds_allowed": ["solve-rr", "run-os"],
      "kinds_allowed": ["rr", "http"],
      "ops_allowed": ["std.rr.fetch_v1", "std.net.http.fetch_v1"],
      "cassette_brand": "std.rr.cassette_v1"
    }
  ],
  "defaults": {
    "record_modes_allowed": ["off", "record_missing_v1", "record_v1", "replay_v1", "rewrite_v1"]
  }
}
"#;
    write_bytes(&root.join("arch/rr/index.x07rr.json"), rr_index.as_bytes());

    let policy = r#"{
  "schema_version": "x07.arch.rr.policy@0.1.0",
  "id": "p1",
  "v": 1,
  "mode_default": "replay_v1",
  "match_mode": "lookup_v1",
  "budgets": {
    "max_cassette_bytes": 1024,
    "max_entries": 16,
    "max_req_bytes": 1024,
    "max_resp_bytes": 1024,
    "max_key_bytes": 256
  }
}
"#;
    write_bytes(
        &root.join("arch/rr/policies/p1.policy.json"),
        policy.as_bytes(),
    );

    let sanitizer_missing_fields = r#"{
  "schema_version": "x07.arch.rr.sanitize@0.1.0",
  "id": "sanitize_none_v1"
}
"#;
    write_bytes(
        &root.join("arch/rr/sanitizers/sanitize_none_v1.sanitize.json"),
        sanitizer_missing_fields.as_bytes(),
    );

    let out = run_x07_in_dir(
        &root,
        &["arch", "check", "--manifest", "arch/manifest.x07arch.json"],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07_ARCH_REPORT_SCHEMA_VERSION);
    let patches = v["suggested_patches"]
        .as_array()
        .expect("suggested_patches[]");
    assert!(
        patches
            .iter()
            .any(|p| p["path"] == "arch/rr/index.x07rr.json"),
        "missing index patch; got: {patches:?}"
    );
    assert!(
        patches
            .iter()
            .any(|p| p["path"] == "arch/rr/sanitizers/sanitize_none_v1.sanitize.json"),
        "missing sanitizer patch; got: {patches:?}"
    );

    let out2 = run_x07_in_dir(
        &root,
        &[
            "arch",
            "check",
            "--manifest",
            "arch/manifest.x07arch.json",
            "--write",
        ],
    );
    assert_eq!(
        out2.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out2.stderr)
    );

    let index_doc: Value = serde_json::from_slice(
        &std::fs::read(root.join("arch/rr/index.x07rr.json")).expect("read index"),
    )
    .expect("parse index json");
    assert_eq!(
        index_doc["policies"][0]["worlds_allowed"],
        serde_json::json!(["run-os", "solve-rr"])
    );
    assert_eq!(
        index_doc["policies"][0]["kinds_allowed"],
        serde_json::json!(["http", "rr"])
    );
    assert_eq!(
        index_doc["policies"][0]["ops_allowed"],
        serde_json::json!(["std.net.http.fetch_v1", "std.rr.fetch_v1"])
    );

    let sanitize_doc: Value = serde_json::from_slice(
        &std::fs::read(root.join("arch/rr/sanitizers/sanitize_none_v1.sanitize.json"))
            .expect("read sanitizer"),
    )
    .expect("parse sanitizer json");
    assert_eq!(sanitize_doc["v"], 1);
    assert_eq!(sanitize_doc["redact_headers"], serde_json::json!([]));
    assert_eq!(sanitize_doc["redact_token"], "");
}

#[test]
fn arch_check_rr_sanitizer_schema_version_diag_includes_expected_and_got() {
    let root = fresh_tmp_dir(&repo_root(), "arch_check_sanitize_schema");
    std::fs::create_dir_all(&root).expect("create root");

    let manifest = r#"{
  "schema_version": "x07.arch.manifest@0.3.0",
  "repo": { "id": "tmp_arch_check", "root": "." },
  "externals": { "allowed_import_prefixes": ["std.", "ext."], "allowed_exact": [] },
  "nodes": [],
  "rules": [],
  "checks": {
    "deny_cycles": false,
    "deny_orphans": false,
    "enforce_visibility": false,
    "enforce_world_caps": false
  },
  "tool_budgets": {
    "max_modules": 5000,
    "max_edges": 50000,
    "max_diags": 2000,
    "contracts_budgets": { "max_contract_files": 2000, "max_contract_bytes": 67108864 }
  },
  "contracts_v1": {
    "rr": {
      "index_path": "arch/rr/index.x07rr.json",
      "gen_dir": "arch/rr",
      "require_policy_for_os_calls": true
    },
    "canonical_json": { "mode": "jcs_rfc8785_v1" }
  }
}
"#;
    write_bytes(
        &root.join("arch/manifest.x07arch.json"),
        manifest.as_bytes(),
    );

    let rr_index = r#"{
  "schema_version": "x07.arch.rr.index@0.1.0",
  "policies": [
    {
      "id": "p1",
      "policy_path": "arch/rr/policies/p1.policy.json",
      "sanitize_id": "sanitize_none_v1",
      "sanitize_path": "arch/rr/sanitizers/sanitize_none_v1.sanitize.json",
      "worlds_allowed": ["run-os", "solve-rr"],
      "kinds_allowed": ["rr"],
      "ops_allowed": ["std.rr.fetch_v1"],
      "cassette_brand": "std.rr.cassette_v1"
    }
  ],
  "defaults": {
    "record_modes_allowed": ["off", "record_missing_v1", "record_v1", "replay_v1", "rewrite_v1"]
  }
}
"#;
    write_bytes(&root.join("arch/rr/index.x07rr.json"), rr_index.as_bytes());

    let policy = r#"{
  "schema_version": "x07.arch.rr.policy@0.1.0",
  "id": "p1",
  "v": 1,
  "mode_default": "replay_v1",
  "match_mode": "lookup_v1",
  "budgets": {
    "max_cassette_bytes": 1024,
    "max_entries": 16,
    "max_req_bytes": 1024,
    "max_resp_bytes": 1024,
    "max_key_bytes": 256
  }
}
"#;
    write_bytes(
        &root.join("arch/rr/policies/p1.policy.json"),
        policy.as_bytes(),
    );

    let sanitizer_wrong_schema = r#"{
  "schema_version": "x07.arch.rr.sanitizer@0.1.0",
  "id": "sanitize_none_v1",
  "v": 1,
  "redact_headers": [],
  "redact_token": ""
}
"#;
    write_bytes(
        &root.join("arch/rr/sanitizers/sanitize_none_v1.sanitize.json"),
        sanitizer_wrong_schema.as_bytes(),
    );

    let out = run_x07_in_dir(
        &root,
        &["arch", "check", "--manifest", "arch/manifest.x07arch.json"],
    );
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07_ARCH_REPORT_SCHEMA_VERSION);
    let diags = v["diags"].as_array().expect("diags[]");
    let d = diags
        .iter()
        .find(|d| d["code"] == "E_ARCH_RR_SANITIZER_SCHEMA_VERSION")
        .expect("schema mismatch diag");
    assert_eq!(d["data"]["expected"], "x07.arch.rr.sanitize@0.1.0");
    assert_eq!(d["data"]["got"], "x07.arch.rr.sanitizer@0.1.0");
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
                        "fuel": 2000000,
                        "timeout_ms": 2000,
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
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["summary"]["passed"], 43);
    assert_eq!(v["summary"]["failed"], 0);
    assert_eq!(v["summary"]["errors"], 0);
    assert_eq!(v["summary"]["xfail_failed"], 1);

    let tests = v["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 44);
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
            "smoke/stdlib_bytes_views",
            "smoke/stdlib_json_encode",
            "smoke/stdlib_parse_dec",
            "smoke/stdlib_path_helpers",
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
            "smoke/value_containers_hash_map_bytes_bytes_determinism",
            "smoke/value_containers_ty_clone_drop_bytes",
            "smoke/value_containers_ty_ops_bytes",
            "smoke/value_containers_vec_bytes_basic",
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
fn x07_test_manifest_rejects_runtime_attestation_outside_sandbox() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_test_runtime_attest_manifest");
    write_json(
        &dir.join("tests.json"),
        &serde_json::json!({
            "schema_version": "x07.tests_manifest@0.2.0",
            "tests": [
                {
                    "id": "smoke/runtime",
                    "world": "solve-pure",
                    "entry": "app.main_v1",
                    "expect": "pass",
                    "require_runtime_attestation": true
                }
            ]
        }),
    );

    let out = run_x07_in_dir(&dir, &["test", "--manifest", "tests.json"]);
    assert_eq!(out.status.code(), Some(12));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("X07TEST_RUNTIME_ATTEST_REQUIRED"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn x07_test_manifest_rejects_invalid_required_capsules() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_test_capsule_manifest");
    write_json(
        &dir.join("tests.json"),
        &serde_json::json!({
            "schema_version": "x07.tests_manifest@0.2.0",
            "tests": [
                {
                    "id": "smoke/capsule",
                    "world": "solve-pure",
                    "entry": "app.main_v1",
                    "expect": "pass",
                    "required_capsules": [""]
                }
            ]
        }),
    );

    let out = run_x07_in_dir(&dir, &["test", "--manifest", "tests.json"]);
    assert_eq!(out.status.code(), Some(12));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("X07TEST_CAPSULE_EVIDENCE_MISSING"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn x07_test_rejects_unsupported_async_entry_returns() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_test_async_entry");
    write_json(
        &dir.join("app.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "app",
            "imports": [],
            "decls": [
                {
                    "kind": "export",
                    "names": ["app.worker_v1"]
                },
                {
                    "kind": "defasync",
                    "name": "app.worker_v1",
                    "params": [],
                    "result": "bytes",
                    "body": ["bytes.alloc", 0]
                }
            ]
        }),
    );
    write_json(
        &dir.join("tests.json"),
        &serde_json::json!({
            "schema_version": "x07.tests_manifest@0.2.0",
            "tests": [
                {
                    "id": "smoke/async",
                    "world": "solve-pure",
                    "entry": "app.worker_v1",
                    "expect": "pass",
                    "returns": "result_i32"
                }
            ]
        }),
    );

    let out = run_x07_in_dir(&dir, &["test", "--manifest", "tests.json"]);
    assert_eq!(out.status.code(), Some(12));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("X07TEST_ASYNC_ENTRY_UNSUPPORTED"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn x07_test_filter_exact_zero_tests_errors_unless_allow_empty() {
    let root = repo_root();
    let manifest = root.join("tests/tests.json");
    assert!(manifest.is_file(), "missing {}", manifest.display());

    let out = run_x07(&[
        "test",
        "--manifest",
        manifest.to_str().unwrap(),
        "--filter",
        "definitely_not_a_real_test_id",
        "--exact",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("0 tests selected"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--allow-empty"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_x07(&[
        "test",
        "--manifest",
        manifest.to_str().unwrap(),
        "--filter",
        "definitely_not_a_real_test_id",
        "--exact",
        "--allow-empty",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["tests"].as_array().unwrap().len(), 0);
}

#[test]
fn x07_test_solve_fuel_override_is_applied() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_test_solve_fuel_override");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let module = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "app",
        "imports": ["std.test"],
        "decls": [
            { "kind": "export", "names": ["app.pass"] },
            {
                "kind": "defn",
                "name": "app.pass",
                "params": [],
                "result": "result_i32",
                "body": ["begin", ["std.test.pass"]]
            }
        ]
    });
    write_bytes(
        &dir.join("app.x07.json"),
        serde_json::to_string_pretty(&module).unwrap().as_bytes(),
    );

    let manifest = serde_json::json!({
        "schema_version": "x07.tests_manifest@0.2.0",
        "tests": [
            {
                "id": "fuel_low",
                "world": "solve-pure",
                "entry": "app.pass",
                "expect": "pass",
                "solve_fuel": 1
            }
        ]
    });
    let manifest_path = dir.join("tests.json");
    write_bytes(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    );

    let out = run_x07(&["test", "--manifest", manifest_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(12),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let v = parse_json_stdout(&out);
    let tests = v["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 1);
    let diags = tests[0]["diags"].as_array().expect("diags[]");
    assert!(
        diags
            .iter()
            .any(|d| d.get("code").and_then(Value::as_str) == Some("X07T_RUN_TRAP")),
        "expected X07T_RUN_TRAP diag, got:\n{}",
        serde_json::to_string_pretty(diags).unwrap()
    );
}

#[test]
fn x07_test_run_trap_includes_fs_open_path() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_test_trap_fs_path");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    std::fs::create_dir_all(dir.join("fixture")).expect("create fixture dir");

    let module = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "app",
        "imports": ["std.fs", "std.test"],
        "decls": [
            { "kind": "export", "names": ["app.read_missing"] },
            {
                "kind": "defn",
                "name": "app.read_missing",
                "params": [],
                "result": "result_i32",
                "body": [
                    "begin",
                    ["let", "_b", ["std.fs.read", ["bytes.lit", "definitely_missing.txt"]]],
                    ["std.test.pass"]
                ]
            }
        ]
    });
    write_bytes(
        &dir.join("app.x07.json"),
        serde_json::to_string_pretty(&module).unwrap().as_bytes(),
    );

    let manifest = serde_json::json!({
        "schema_version": "x07.tests_manifest@0.2.0",
        "tests": [
            {
                "id": "missing_file_traps",
                "world": "solve-fs",
                "entry": "app.read_missing",
                "fixture_root": "fixture",
                "expect": "pass"
            }
        ]
    });
    let manifest_path = dir.join("tests.json");
    write_bytes(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    );

    let out = run_x07(&["test", "--manifest", manifest_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(12),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let v = parse_json_stdout(&out);
    let tests = v["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 1);
    let diags = tests[0]["diags"].as_array().expect("diags[]");
    let trap = diags
        .iter()
        .find(|d| d.get("code").and_then(Value::as_str) == Some("X07T_RUN_TRAP"))
        .and_then(|d| d.get("details"))
        .and_then(|d| d.get("trap"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        trap.contains("path=definitely_missing.txt"),
        "expected trap to include attempted path, got:\n{trap}"
    );
}

#[test]
fn x07_test_assert_bytes_eq_emits_details() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "test_assert_bytes_eq_details");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let module = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "app",
        "imports": ["std.test"],
        "decls": [
            { "kind": "export", "names": ["app.fail_bytes_eq"] },
            {
                "kind": "defn",
                "name": "app.fail_bytes_eq",
                "params": [],
                "result": "result_i32",
                "body": [
                    "begin",
                    ["let", "got_v", ["vec_u8.with_capacity", 2]],
                    ["set", "got_v", ["vec_u8.push", "got_v", 255]],
                    ["set", "got_v", ["vec_u8.push", "got_v", 97]],
                    ["let", "got", ["vec_u8.into_bytes", "got_v"]],
                    ["let", "exp_v", ["vec_u8.with_capacity", 2]],
                    ["set", "exp_v", ["vec_u8.push", "exp_v", 255]],
                    ["set", "exp_v", ["vec_u8.push", "exp_v", 98]],
                    ["let", "expected", ["vec_u8.into_bytes", "exp_v"]],
                    [
                        "try",
                        [
                            "std.test.assert_bytes_eq",
                            "got",
                            "expected",
                            ["std.test.code_assert_bytes_eq"]
                        ]
                    ],
                    ["std.test.pass"]
                ]
            }
        ]
    });
    write_bytes(
        &dir.join("app.x07.json"),
        serde_json::to_string_pretty(&module)
            .expect("encode module")
            .as_bytes(),
    );

    let manifest_path = dir.join("tests.json");
    let manifest = serde_json::json!({
        "schema_version": "x07.tests_manifest@0.2.0",
        "tests": [
            {
                "id": "smoke/assert_bytes_eq_details",
                "world": "solve-pure",
                "entry": "app.fail_bytes_eq",
                "expect": "pass"
            }
        ]
    });
    write_bytes(
        &manifest_path,
        serde_json::to_string_pretty(&manifest)
            .expect("encode manifest")
            .as_bytes(),
    );

    let out = run_x07_in_dir(&dir, &["test", "--manifest", "tests.json"]);
    assert_eq!(
        out.status.code(),
        Some(10),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["summary"]["failed"], 1);

    let tests = v["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0]["id"], "smoke/assert_bytes_eq_details");
    assert_eq!(tests[0]["status"], "fail");
    let diags = tests[0]["diags"].as_array().expect("diags[]");
    let d = diags
        .iter()
        .find(|d| d["code"] == "X07T_ASSERT_BYTES_EQ")
        .expect("expected X07T_ASSERT_BYTES_EQ diag");
    let details = d["details"].as_object().expect("expected details object");
    assert_eq!(details["prefix_max_bytes"], 64);
    assert_eq!(details["got"]["len"], 2);
    assert_eq!(details["expected"]["len"], 2);
    assert_eq!(details["got"]["prefix_hex"], "ff61");
    assert_eq!(details["expected"]["prefix_hex"], "ff62");
    assert!(
        details["got"]["prefix_utf8_lossy"]
            .as_str()
            .expect("got.prefix_utf8_lossy")
            .contains('\u{FFFD}'),
        "expected got.prefix_utf8_lossy to contain U+FFFD: {:?}",
        details["got"]["prefix_utf8_lossy"]
    );

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
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
fn x07_test_contract_violation_emits_repro_and_report_fields() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "test_contract_repro");
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let module = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "contracts_fixture",
        "imports": ["std.test"],
        "decls": [
            { "kind": "export", "names": ["contracts_fixture.contract_fail"] },
            {
                "kind": "defn",
                "name": "contracts_fixture.contract_fail",
                "params": [],
                "result": "result_i32",
                "requires": [{ "id": "req1", "expr": 0, "witness": [42] }],
                "body": ["std.test.pass"]
            }
        ]
    });
    write_bytes(
        &dir.join("contracts_fixture.x07.json"),
        serde_json::to_string_pretty(&module).unwrap().as_bytes(),
    );

    let manifest = serde_json::json!({
        "schema_version": "x07.tests_manifest@0.2.0",
        "tests": [
            {
                "id": "contracts/requires_violation",
                "world": "solve-pure",
                "entry": "contracts_fixture.contract_fail",
                "expect": "pass"
            }
        ]
    });
    let manifest_path = dir.join("tests.json");
    write_bytes(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    );

    let artifact_dir = dir.join("artifacts");
    let stdlib_lock = root.join("stdlib.lock");

    let out = run_x07_in_dir(
        &dir,
        &[
            "test",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--artifact-dir",
            artifact_dir.to_str().unwrap(),
            "--stdlib-lock",
            stdlib_lock.to_str().unwrap(),
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(12),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_stdout(&out);
    assert_eq!(v["schema_version"], X07TEST_SCHEMA_VERSION);
    assert_eq!(v["summary"]["run_failures"], 1);
    assert_eq!(v["tests"].as_array().expect("tests[]").len(), 1);

    let t = &v["tests"][0];
    assert_eq!(t["status"], "error");
    assert_eq!(t["failure_kind"], "contract_violation");

    let expected_repro_path = artifact_dir
        .join("contract")
        .join(format!("id_{}", sha256_hex("req1".as_bytes())))
        .join("repro.json");
    let expected_repro_path_str = expected_repro_path.display().to_string();
    assert_eq!(
        t["contract_repro_path"]
            .as_str()
            .expect("contract_repro_path"),
        expected_repro_path_str
    );
    assert!(
        expected_repro_path.is_file(),
        "missing {}",
        expected_repro_path.display()
    );

    let repro_bytes = std::fs::read(&expected_repro_path).expect("read repro.json");
    let repro: Value = serde_json::from_slice(&repro_bytes).expect("parse repro.json");
    assert_eq!(repro["schema_version"], X07_CONTRACT_REPRO_SCHEMA_VERSION);
    assert_eq!(repro["world"], "solve-pure");
    assert_eq!(repro["source"]["mode"], "x07test");
    assert_eq!(repro["source"]["test_id"], "contracts/requires_violation");
    assert_eq!(
        repro["source"]["test_entry"],
        "contracts_fixture.contract_fail"
    );
    assert_eq!(repro["runner"]["solve_fuel"], 400_000_000);
    assert_eq!(repro["runner"]["max_memory_bytes"], 64 * 1024 * 1024);
    assert_eq!(repro["runner"]["max_output_bytes"], 1024 * 1024);
    assert_eq!(repro["runner"]["cpu_time_limit_seconds"], 5);
    assert_eq!(repro["contract"]["contract_kind"], "requires");
    assert_eq!(repro["contract"]["fn"], "contracts_fixture.contract_fail");
    assert_eq!(repro["contract"]["clause_id"], "req1");
    assert_eq!(repro["input_bytes_b64"], "");
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
fn x07_run_project_with_only_path_deps_does_not_require_registry() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_project_path_deps_no_registry");
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

    let pkg_dir = dir.join("pkgs/local/1.0.0");
    write_minimal_pkg_manifest(&pkg_dir, "local", "1.0.0", &[]);
    std::fs::create_dir_all(pkg_dir.join("modules")).expect("create module_root dir");

    let proj_path = dir.join("x07.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
        .expect("parse x07.json");
    let obj = doc.as_object_mut().expect("x07.json must be object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![
            serde_json::json!({"name":"local","version":"1.0.0","path":"pkgs/local/1.0.0"}),
        ]),
    );
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );

    let lock_path = dir.join("x07.lock.json");
    if lock_path.is_file() {
        std::fs::remove_file(&lock_path).expect("remove old lockfile");
    }

    let exe = env!("CARGO_BIN_EXE_x07");
    let unreachable_index = "sparse+http://127.0.0.1:1/index/";

    // Path-only deps should not require consulting the registry index, even when the lockfile is
    // missing and dependency hydration runs.
    let out = Command::new(exe)
        .current_dir(&dir)
        .env(ENV_SANDBOX_BACKEND, "os")
        .env(ENV_ACCEPT_WEAKER_ISOLATION, "1")
        .env("X07_PKG_INDEX_URL", unreachable_index)
        .args([
            "run",
            "--project",
            ".",
            "--report",
            "wrapped",
            "--cpu-time-limit-seconds",
            "30",
        ])
        .output()
        .expect("run x07");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["schema_version"], X07_RUN_REPORT_SCHEMA_VERSION);
    assert!(lock_path.is_file(), "expected lockfile to be created");

    std::fs::remove_file(&lock_path).expect("remove lockfile");

    // `--offline` should also work for path-only projects (and still should not consult the
    // registry index).
    let out = Command::new(exe)
        .current_dir(&dir)
        .env(ENV_SANDBOX_BACKEND, "os")
        .env(ENV_ACCEPT_WEAKER_ISOLATION, "1")
        .env("X07_PKG_INDEX_URL", unreachable_index)
        .args([
            "run",
            "--project",
            ".",
            "--offline",
            "--report",
            "wrapped",
            "--cpu-time-limit-seconds",
            "30",
        ])
        .output()
        .expect("run x07 --offline");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["schema_version"], X07_RUN_REPORT_SCHEMA_VERSION);
    assert!(lock_path.is_file(), "expected lockfile to be created");
}

#[test]
fn x07_run_contract_violation_emits_repro() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_contract_repro");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": ["std.test"],
        "decls": [
            { "kind": "export", "names": ["main.contract_fail"] },
            {
                "kind": "defn",
                "name": "main.contract_fail",
                "params": [],
                "result": "result_i32",
                "requires": [{ "id": "req1", "expr": 0, "witness": [42] }],
                "body": ["std.test.pass"]
            }
        ],
        "solve": ["begin", ["main.contract_fail"], ["bytes.alloc", 0]]
    }))
    .expect("serialize x07AST");
    let program_path = dir.join("main.x07.json");
    write_bytes(&program_path, &program);

    let out = run_x07_in_dir(
        &dir,
        &[
            "run",
            "--program",
            program_path.to_str().unwrap(),
            "--world",
            "solve-pure",
        ],
    );
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

    let repro_path = dir
        .join(".x07")
        .join("artifacts")
        .join("contract")
        .join(format!("id_{}", sha256_hex("req1".as_bytes())))
        .join("repro.json");
    assert!(repro_path.is_file(), "missing {}", repro_path.display());

    let repro_bytes = std::fs::read(&repro_path).expect("read repro.json");
    let repro: Value = serde_json::from_slice(&repro_bytes).expect("parse repro.json");
    assert_eq!(repro["schema_version"], X07_CONTRACT_REPRO_SCHEMA_VERSION);
    assert_eq!(repro["world"], "solve-pure");
    assert_eq!(repro["source"]["mode"], "x07run");
    assert_eq!(repro["source"]["target_kind"], "program");
    assert_eq!(repro["contract"]["contract_kind"], "requires");
    assert_eq!(repro["contract"]["clause_id"], "req1");
}

#[test]
fn x07_verify_bmc_missing_cbmc_emits_tool_missing_report() {
    let dir = fresh_os_tmp_dir("x07_verify_bmc_missing_cbmc");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let module = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "verify_fixture",
        "imports": [],
        "decls": [
            {"kind":"export", "names":["verify_fixture.f"]},
            {
                "kind": "defn",
                "name": "verify_fixture.f",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "requires": [{"id":"r0","expr":["=","x","x"]}],
                "body": "x"
            }
        ]
    }))
    .expect("serialize x07AST module");
    write_bytes(&dir.join("verify_fixture.x07.json"), &module);

    let empty_path = dir.join("empty_path");
    std::fs::create_dir_all(&empty_path).expect("create empty PATH dir");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&dir)
        .env("PATH", empty_path.to_str().unwrap())
        .args(["verify", "--bmc", "--entry", "verify_fixture.f"])
        .output()
        .expect("run x07 verify");

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

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "bmc");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "tool_missing");
    assert_eq!(v["diagnostics_count"], 1);
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_ECBMC_MISSING");
    assert!(
        v["artifacts"]["driver_path"].as_str().is_some(),
        "expected artifacts.driver_path"
    );
    assert!(
        v["artifacts"]["c_path"].as_str().is_some(),
        "expected artifacts.c_path"
    );
}

#[test]
fn x07_verify_prove_without_contracts_emits_unsupported_report() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_unsupported");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let module = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "verify_fixture",
        "imports": [],
        "decls": [
            {"kind":"export", "names":["verify_fixture.f"]},
            {
                "kind": "defn",
                "name": "verify_fixture.f",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "body": "x"
            }
        ]
    }))
    .expect("serialize x07AST module");
    write_bytes(&dir.join("verify_fixture.x07.json"), &module);

    let out = run_x07_in_dir(&dir, &["verify", "--prove", "--entry", "verify_fixture.f"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "unsupported");
    assert_eq!(v["diagnostics_count"], 1);
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_NO_CONTRACTS");
    assert_eq!(
        v["result"]["details"],
        "target function has no requires/ensures/invariant clauses"
    );
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_simple_contract_returns_proven() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_simple");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result","x"]}],
                    "body": "x"
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
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
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["kind"], "proven");
    assert!(v["artifacts"]["smt2_path"].as_str().is_some());
    assert!(v["artifacts"]["z3_out_path"].as_str().is_some());
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_self_recursive_with_decreases_returns_proven() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_recursive");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [{"name":"n","ty":"i32"}],
                    "result": "u32",
                    "requires": [
                        {"id":"r0","expr":[">=","n",0]},
                        {"id":"r1","expr":["<=","n",3]}
                    ],
                    "ensures": [{"id":"e0","expr":[">=","__result",0]}],
                    "decreases": [{"id":"d0","expr":"n"}],
                    "body": ["if",["=","n",0],0,["verify_fixture.f",["-","n",1]]]
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
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
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["kind"], "proven");
    assert_eq!(v["proof_summary"]["engine"], "cbmc_z3");
    assert_eq!(v["proof_summary"]["recursion_kind"], "self_recursive");
    assert_eq!(v["proof_summary"]["has_decreases"], true);
    assert_eq!(v["proof_summary"]["decreases_count"], 1);
    assert_eq!(v["proof_summary"]["bounded_by_unwind"], true);

    let coverage = run_x07_in_dir(
        &dir,
        &[
            "verify",
            "--coverage",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        coverage.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&coverage.stderr)
    );
    assert!(
        coverage.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&coverage.stderr)
    );

    let coverage_doc: Value =
        serde_json::from_slice(&coverage.stdout).expect("parse verify coverage JSON");
    assert_eq!(coverage_doc["coverage"]["summary"]["recursive_defn"], 1);
    assert_eq!(
        coverage_doc["coverage"]["summary"]["supported_recursive_defn"],
        1
    );
    assert_eq!(
        coverage_doc["coverage"]["summary"]["unsupported_recursive_defn"],
        0
    );
    let functions = coverage_doc["coverage"]["functions"]
        .as_array()
        .expect("functions[]");
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0]["status"], "supported_recursive");
    assert_eq!(
        functions[0]["support_summary"]["recursion_kind"],
        "self_recursive"
    );
    assert_eq!(functions[0]["support_summary"]["decreases_count"], 1);
    assert_eq!(functions[0]["support_summary"]["prove_supported"], true);
}

#[test]
fn x07_verify_prove_self_recursive_without_decreases_is_unsupported() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_recursive_missing_decreases");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [{"name":"n","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":[">=","n",0]}],
                    "ensures": [{"id":"e0","expr":[">=","__result",0]}],
                    "body": ["if",["=","n",0],0,["verify_fixture.f",["-","n",1]]]
                }
            ]
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "unsupported");
    assert_eq!(
        v["result"]["details"],
        "self-recursive targets must declare decreases[] to use x07 verify"
    );
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_RECURSIVE_DECREASES_REQUIRED");
}

#[test]
fn x07_verify_prove_mutual_recursion_is_unsupported() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_mutual_recursion");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result","x"]}],
                    "body": ["verify_fixture.g","x"]
                },
                {
                    "kind": "defn",
                    "name": "verify_fixture.g",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r1","expr":["=","x","x"]}],
                    "ensures": [{"id":"e1","expr":["=","__result","x"]}],
                    "body": ["verify_fixture.f","x"]
                }
            ]
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "unsupported");
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_UNSUPPORTED_MUTUAL_RECURSION");
    assert!(
        v["result"]["details"]
            .as_str()
            .is_some_and(|details| details.contains("mutual recursion")),
        "expected mutual recursion details, got:\n{}",
        serde_json::to_string_pretty(&v).unwrap()
    );
}

#[test]
fn x07_verify_prove_self_recursive_without_obvious_decrease_is_unsupported() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_recursive_non_decreasing");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [{"name":"n","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":[">=","n",0]}],
                    "ensures": [{"id":"e0","expr":[">=","__result",0]}],
                    "decreases": [{"id":"d0","expr":"n"}],
                    "body": ["if",["=","n",0],0,["verify_fixture.f","n"]]
                }
            ]
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "unsupported");
    assert_eq!(
        v["result"]["details"],
        "recursive self-call does not obviously decrease \"n\""
    );
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_RECURSION_TERMINATION_FAILED");
}

#[cfg(unix)]
#[test]
fn x07_verify_coverage_reuses_imported_summary_and_emits_summary_artifact() {
    let dir = fresh_os_tmp_dir("x07_verify_imported_summary");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main","verify_fixture.helper"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.helper",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result","x"]}],
                    "body": "x"
                },
                {
                    "kind": "defn",
                    "name": "verify_fixture.main",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r1","expr":["=","x","x"]}],
                    "ensures": [{"id":"e1","expr":["=","__result","x"]}],
                    "body": ["verify_fixture.helper","x"]
                }
            ]
        }),
    );

    let helper_out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.helper",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        helper_out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&helper_out.stderr)
    );
    let helper_report: Value =
        serde_json::from_slice(&helper_out.stdout).expect("parse helper prove JSON");
    let helper_summary_path = helper_report["artifacts"]["verify_proof_summary_path"]
        .as_str()
        .expect("helper verify proof summary path");
    assert!(
        Path::new(helper_summary_path).is_file(),
        "missing {}",
        helper_summary_path
    );
    let helper_summary: Value = serde_json::from_slice(
        &std::fs::read(helper_summary_path).expect("read helper verify proof summary"),
    )
    .expect("parse helper verify proof summary");
    assert_eq!(
        helper_summary["schema_version"],
        X07_VERIFY_PROOF_SUMMARY_SCHEMA_VERSION
    );

    let main_out = run_x07_in_dir(
        &dir,
        &[
            "verify",
            "--coverage",
            "--entry",
            "verify_fixture.main",
            "--project",
            "x07.json",
            "--proof-summary",
            helper_summary_path,
        ],
    );
    assert_eq!(
        main_out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&main_out.stderr)
    );
    let main_report: Value =
        serde_json::from_slice(&main_out.stdout).expect("parse main coverage JSON");
    assert_eq!(
        main_report["schema_version"],
        X07_VERIFY_REPORT_SCHEMA_VERSION
    );
    assert_eq!(
        main_report["coverage"]["summary"]["imported_proof_summary_defn"],
        1
    );
    let functions = main_report["coverage"]["functions"]
        .as_array()
        .expect("functions[]");
    assert!(
        functions.iter().any(|function| {
            function["symbol"] == "verify_fixture.helper"
                && function["status"] == "imported_proof_summary"
        }),
        "expected imported summary status, got:\n{}",
        serde_json::to_string_pretty(&main_report).unwrap()
    );
    let main_summary_path = main_report["artifacts"]["verify_coverage_summary_path"]
        .as_str()
        .expect("main verify summary path");
    let main_summary: Value = serde_json::from_slice(
        &std::fs::read(main_summary_path).expect("read main verify summary"),
    )
    .expect("parse main verify summary");
    assert_eq!(
        main_summary["schema_version"],
        X07_VERIFY_SUMMARY_SCHEMA_VERSION
    );
    assert!(
        main_summary["imported_summaries"]
            .as_array()
            .expect("imported_summaries[]")
            .iter()
            .any(|summary| {
                summary["path"] == helper_summary_path
                    && summary["symbols"]
                        .as_array()
                        .expect("summary symbols[]")
                        .iter()
                        .any(|symbol| symbol == "verify_fixture.helper")
            }),
        "expected imported summary inventory, got:\n{}",
        serde_json::to_string_pretty(&main_summary).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_rejects_mismatched_imported_summary() {
    let dir = fresh_os_tmp_dir("x07_verify_imported_summary_mismatch");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    let initial_module = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "verify_fixture",
        "imports": [],
        "decls": [
            {"kind":"export", "names":["verify_fixture.main","verify_fixture.helper"]},
            {
                "kind": "defn",
                "name": "verify_fixture.helper",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "requires": [{"id":"r0","expr":["=","x","x"]}],
                "ensures": [{"id":"e0","expr":["=","__result","x"]}],
                "body": "x"
            },
            {
                "kind": "defn",
                "name": "verify_fixture.main",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "requires": [{"id":"r1","expr":["=","x","x"]}],
                "ensures": [{"id":"e1","expr":["=","__result","x"]}],
                "body": ["verify_fixture.helper","x"]
            }
        ]
    });
    write_json(&dir.join("verify_fixture.x07.json"), &initial_module);

    let helper_out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.helper",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        helper_out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&helper_out.stderr)
    );
    let helper_report: Value =
        serde_json::from_slice(&helper_out.stdout).expect("parse helper prove JSON");
    let helper_summary_path = helper_report["artifacts"]["verify_proof_summary_path"]
        .as_str()
        .expect("helper verify proof summary path")
        .to_string();

    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main","verify_fixture.helper"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.helper",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result",["+","x",1]]}],
                    "body": ["+","x",1]
                },
                {
                    "kind": "defn",
                    "name": "verify_fixture.main",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r1","expr":["=","x","x"]}],
                    "ensures": [{"id":"e1","expr":["=","__result",["+","x",1]]}],
                    "body": ["verify_fixture.helper","x"]
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.main",
            "--project",
            "x07.json",
            "--summary",
            &helper_summary_path,
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "error");
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_SUMMARY_MISMATCH");
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_check_accepts_defn_proof_bundle() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_check_defn");
    let solver_dir = dir.join("bin");
    write_fake_prove_solvers(&solver_dir);

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.main",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result","x"]}],
                    "body": "x"
                }
            ]
        }),
    );

    let proof_path = dir.join("proof.json");
    let prove_out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.main",
            "--project",
            "x07.json",
            "--emit-proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        std::slice::from_ref(&solver_dir),
    );
    assert_eq!(
        prove_out.status.code(),
        Some(0),
        "stdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&prove_out.stdout),
        String::from_utf8_lossy(&prove_out.stderr)
    );
    let prove_report = parse_json_stdout(&prove_out);
    assert_eq!(prove_report["result"]["kind"], "proven");
    assert!(proof_path.is_file(), "missing proof object");

    let out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "prove",
            "check",
            "--proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        &[solver_dir],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(
        v["schema_version"],
        X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_VERSION
    );
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"], "accepted");
    assert_eq!(v["symbol"], "verify_fixture.main");
    assert_eq!(v["entry_symbol"], "verify_fixture.main");
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_check_accepts_defasync_proof_bundle() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_check_defasync");
    let solver_dir = dir.join("bin");
    write_fake_prove_solvers(&solver_dir);

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main"]},
                {
                    "kind": "defasync",
                    "name": "verify_fixture.main",
                    "params": [],
                    "result": "bytes",
                    "protocol": {
                        "await_invariant": [{"id":"a0","expr":["=",0,0]}],
                        "scope_invariant": [{"id":"s0","expr":["=",0,0]}],
                        "cancellation_ensures": [
                            {"id":"c0","expr":["=",["view.len",["bytes.view","__result"]],0]}
                        ]
                    },
                    "body": ["begin", ["task.yield"], ["bytes.alloc", 0]]
                }
            ]
        }),
    );

    let proof_path = dir.join("proof.json");
    let prove_out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.main",
            "--project",
            "x07.json",
            "--emit-proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        std::slice::from_ref(&solver_dir),
    );
    assert_eq!(
        prove_out.status.code(),
        Some(0),
        "stdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&prove_out.stdout),
        String::from_utf8_lossy(&prove_out.stderr)
    );

    let out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "prove",
            "check",
            "--proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        &[solver_dir],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(
        v["schema_version"],
        X07_VERIFY_PROOF_CHECK_REPORT_SCHEMA_VERSION
    );
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"], "accepted");
    assert_eq!(v["symbol"], "verify_fixture.main");
    assert!(v["validated_scheduler_model_digest"].as_str().is_some());
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_check_rejects_source_edit_after_proof() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_check_source_edit");
    let solver_dir = dir.join("bin");
    write_fake_prove_solvers(&solver_dir);

    write_verify_project_files(&dir);
    let source_path = dir.join("verify_fixture.x07.json");
    write_json(
        &source_path,
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.main",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result","x"]}],
                    "body": "x"
                }
            ]
        }),
    );

    let proof_path = dir.join("proof.json");
    let prove_out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.main",
            "--project",
            "x07.json",
            "--emit-proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        std::slice::from_ref(&solver_dir),
    );
    assert_eq!(prove_out.status.code(), Some(0));

    write_json(
        &source_path,
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.main",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result",["+","x",1]]}],
                    "body": ["+","x",1]
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "prove",
            "check",
            "--proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        &[solver_dir],
    );
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"], "rejected");
    assert_eq!(proof_check_diag_code(&v), "X07PROOF_ESOURCE_REPLAY_FAILED");
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_check_rejects_imported_proof_summary_substitution() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_check_imported_summary_swap");
    let solver_dir = dir.join("bin");
    write_fake_prove_solvers(&solver_dir);

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main","verify_fixture.helper","verify_fixture.helper_alt"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.helper",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result","x"]}],
                    "body": "x"
                },
                {
                    "kind": "defn",
                    "name": "verify_fixture.helper_alt",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r1","expr":["=","x","x"]}],
                    "ensures": [{"id":"e1","expr":["=","__result",["+","x",1]]}],
                    "body": ["+","x",1]
                },
                {
                    "kind": "defn",
                    "name": "verify_fixture.main",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r2","expr":["=","x","x"]}],
                    "ensures": [{"id":"e2","expr":["=","__result","x"]}],
                    "body": ["verify_fixture.helper","x"]
                }
            ]
        }),
    );

    let helper_out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.helper",
            "--project",
            "x07.json",
        ],
        std::slice::from_ref(&solver_dir),
    );
    assert_eq!(helper_out.status.code(), Some(0));
    let helper_report = parse_json_stdout(&helper_out);
    let helper_summary_path =
        prove_report_artifact_path(&helper_report, "verify_proof_summary_path");

    let helper_alt_out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.helper_alt",
            "--project",
            "x07.json",
        ],
        std::slice::from_ref(&solver_dir),
    );
    assert_eq!(helper_alt_out.status.code(), Some(0));
    let helper_alt_report = parse_json_stdout(&helper_alt_out);
    let helper_alt_summary_path =
        prove_report_artifact_path(&helper_alt_report, "verify_proof_summary_path");

    let proof_path = dir.join("proof.json");
    let main_out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.main",
            "--project",
            "x07.json",
            "--summary",
            helper_summary_path
                .to_str()
                .expect("utf-8 helper summary path"),
            "--emit-proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        std::slice::from_ref(&solver_dir),
    );
    assert_eq!(main_out.status.code(), Some(0));

    std::fs::copy(
        &helper_alt_summary_path,
        bundled_imported_summary_path(&proof_path),
    )
    .expect("substitute bundled imported proof summary");

    let out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "prove",
            "check",
            "--proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        &[solver_dir],
    );
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"], "rejected");
    assert_eq!(
        proof_check_diag_code(&v),
        "X07PROOF_EIMPORTED_SUMMARY_MISMATCH"
    );
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_check_rejects_primitive_manifest_change() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_check_primitive_manifest");
    let solver_dir = dir.join("bin");
    write_fake_prove_solvers(&solver_dir);

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.main",
                    "params": [{"name":"x","ty":"i32"}],
                    "result": "i32",
                    "requires": [{"id":"r0","expr":["=","x","x"]}],
                    "ensures": [{"id":"e0","expr":["=","__result","x"]}],
                    "body": "x"
                }
            ]
        }),
    );

    let proof_path = dir.join("proof.json");
    let prove_out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.main",
            "--project",
            "x07.json",
            "--emit-proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        std::slice::from_ref(&solver_dir),
    );
    assert_eq!(prove_out.status.code(), Some(0));

    let mut proof_object: Value =
        serde_json::from_slice(&std::fs::read(&proof_path).expect("read proof object"))
            .expect("parse proof object");
    proof_object["primitive_manifest_digest"] = Value::String(format!("sha256:{}", "0".repeat(64)));
    write_json(&proof_path, &proof_object);

    let out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "prove",
            "check",
            "--proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        &[solver_dir],
    );
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"], "rejected");
    assert_eq!(proof_check_diag_code(&v), "X07PROOF_ESOURCE_REPLAY_FAILED");
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_check_rejects_async_scheduler_model_change() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_check_scheduler_model");
    let solver_dir = dir.join("bin");
    write_fake_prove_solvers(&solver_dir);

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.main"]},
                {
                    "kind": "defasync",
                    "name": "verify_fixture.main",
                    "params": [],
                    "result": "bytes",
                    "protocol": {
                        "await_invariant": [{"id":"a0","expr":["=",0,0]}],
                        "scope_invariant": [{"id":"s0","expr":["=",0,0]}],
                        "cancellation_ensures": [
                            {"id":"c0","expr":["=",["view.len",["bytes.view","__result"]],0]}
                        ]
                    },
                    "body": ["begin", ["task.yield"], ["bytes.alloc", 0]]
                }
            ]
        }),
    );

    let proof_path = dir.join("proof.json");
    let prove_out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.main",
            "--project",
            "x07.json",
            "--emit-proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        std::slice::from_ref(&solver_dir),
    );
    assert_eq!(prove_out.status.code(), Some(0));

    let mut proof_object: Value =
        serde_json::from_slice(&std::fs::read(&proof_path).expect("read proof object"))
            .expect("parse proof object");
    proof_object["scheduler_model_digest"] = Value::String(format!("sha256:{}", "0".repeat(64)));
    write_json(&proof_path, &proof_object);

    let out = run_x07_in_dir_with_path_prefixes(
        &dir,
        &[
            "prove",
            "check",
            "--proof",
            proof_path.to_str().expect("utf-8 proof path"),
        ],
        &[solver_dir],
    );
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"], "rejected");
    assert_eq!(
        proof_check_diag_code(&v),
        "X07PROOF_ESCHEDULER_MODEL_MISMATCH"
    );
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_supports_views_and_result_views() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_rich_signature");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [
                        {"name":"doc","ty":"bytes_view"},
                        {"name":"maybe_doc","ty":"option_bytes_view"},
                        {"name":"maybe_result","ty":"result_bytes_view"}
                    ],
                    "result": "u32",
                    "requires": [{"id":"r0","expr":["=",0,0]}],
                    "ensures": [{"id":"e0","expr":[">=","__result",0]}],
                    "body": 0
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--max-bytes-len",
            "4",
            "--entry",
            "verify_fixture.f",
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
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["kind"], "proven");
}

#[test]
fn x07_verify_prove_rejects_nested_result_param_with_explicit_diag() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_nested_result_param");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [
                        {"name":"nested","ty":"result_result_bytes"}
                    ],
                    "result": "u32",
                    "requires": [{"id":"r0","expr":["=",0,0]}],
                    "ensures": [{"id":"e0","expr":[">=","__result",0]}],
                    "body": 0
                }
            ]
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "unsupported");
    assert!(v["result"]["details"]
        .as_str()
        .expect("details string")
        .contains("param type \"result_result_bytes\""));
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_UNSUPPORTED_RICH_TYPE");
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_supports_vec_param_and_result_via_std_vec() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_vec_param");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": ["std.vec"],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [
                        {"name":"raw","ty":"vec_u8"}
                    ],
                    "result": "vec_u8",
                    "requires": [{"id":"r0","expr":["=",0,0]}],
                    "ensures": [{"id":"e0","expr":["=",0,0]}],
                    "body": "raw"
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--max-bytes-len",
            "4",
            "--entry",
            "verify_fixture.f",
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
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["kind"], "proven");
    assert_eq!(v["diagnostics_count"], 0);
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_supports_schema_record_brand_view_param() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_schema_record_brand");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::create_dir_all(dir.join("schema")).expect("create schema dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("schema").join("event_line_v1.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "schema.event_line_v1",
            "imports": [],
            "meta": {
                "brands_v1": {
                    "schema.event_line_v1": {
                        "validate": "schema.event_line_v1.validate_doc_v1"
                    }
                }
            },
            "decls": [
                {"kind":"export", "names":["schema.event_line_v1.validate_doc_v1", "schema.event_line_v1.first_byte_v1"]},
                {
                    "kind": "defn",
                    "name": "schema.event_line_v1.validate_doc_v1",
                    "params": [
                        {"name":"doc","ty":"bytes_view"}
                    ],
                    "result": "result_i32",
                    "body": ["if",[">",["view.len","doc"],0],["result_i32.ok",0],["result_i32.err",1]]
                },
                {
                    "kind": "defn",
                    "name": "schema.event_line_v1.first_byte_v1",
                    "params": [
                        {"name":"doc","ty":"bytes_view","brand":"schema.event_line_v1"}
                    ],
                    "result": "i32",
                    "body": ["view.get_u8","doc",0]
                }
            ]
        }),
    );
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": ["schema.event_line_v1"],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [
                        {"name":"doc","ty":"bytes_view","brand":"schema.event_line_v1"}
                    ],
                    "result": "u32",
                    "requires": [{"id":"r0","expr":[">",["view.len","doc"],0]}],
                    "ensures": [{"id":"e0","expr":[">=","__result",0]}],
                    "body": ["schema.event_line_v1.first_byte_v1","doc"]
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
            "--max-bytes-len",
            "4",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["kind"], "proven");
    assert_eq!(v["diagnostics_count"], 0);
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_supports_schema_variant_brand_view_param() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_schema_variant_brand");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::create_dir_all(dir.join("schema")).expect("create schema dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("schema").join("choice_v1.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "schema.choice_v1",
            "imports": [],
            "meta": {
                "brands_v1": {
                    "schema.choice_v1": {
                        "validate": "schema.choice_v1.validate_doc_v1"
                    }
                }
            },
            "decls": [
                {"kind":"export", "names":["schema.choice_v1.validate_doc_v1", "schema.choice_v1.variant_tag_v1", "schema.choice_v1.payload_byte_v1"]},
                {
                    "kind": "defn",
                    "name": "schema.choice_v1.validate_doc_v1",
                    "params": [{"name":"doc","ty":"bytes_view"}],
                    "result": "result_i32",
                    "body": [
                        "if",
                        [">",["view.len","doc"],1],
                        [
                            "if",
                            ["<",["view.get_u8","doc",0],2],
                            ["result_i32.ok",0],
                            ["result_i32.err",2]
                        ],
                        ["result_i32.err",1]
                    ]
                },
                {
                    "kind": "defn",
                    "name": "schema.choice_v1.variant_tag_v1",
                    "params": [{"name":"doc","ty":"bytes_view","brand":"schema.choice_v1"}],
                    "result": "i32",
                    "body": ["view.get_u8","doc",0]
                },
                {
                    "kind": "defn",
                    "name": "schema.choice_v1.payload_byte_v1",
                    "params": [{"name":"doc","ty":"bytes_view","brand":"schema.choice_v1"}],
                    "result": "i32",
                    "body": ["view.get_u8","doc",1]
                }
            ]
        }),
    );
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": ["schema.choice_v1"],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defn",
                    "name": "verify_fixture.f",
                    "params": [{"name":"doc","ty":"bytes_view","brand":"schema.choice_v1"}],
                    "result": "u32",
                    "requires": [{"id":"r0","expr":[">",["view.len","doc"],1]}],
                    "ensures": [{"id":"e0","expr":[">=","__result",0]}],
                    "body": [
                        "if",
                        ["=",["schema.choice_v1.variant_tag_v1","doc"],0],
                        ["schema.choice_v1.payload_byte_v1","doc"],
                        0
                    ]
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
            "--max-bytes-len",
            "4",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["kind"], "proven");
    assert_eq!(v["diagnostics_count"], 0);
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_defasync_protocol_returns_proven() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_defasync");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defasync",
                    "name": "verify_fixture.f",
                    "params": [],
                    "result": "bytes",
                    "protocol": {
                        "await_invariant": [{"id":"a0","expr":["=",0,0]}],
                        "scope_invariant": [{"id":"s0","expr":["=",0,0]}],
                        "cancellation_ensures": [
                            {"id":"c0","expr":["=",["view.len",["bytes.view","__result"]],0]}
                        ]
                    },
                    "body": ["begin", ["task.yield"], ["bytes.alloc", 0]]
                }
            ]
        }),
    );

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["kind"], "proven");
    assert!(v["artifacts"]["smt2_path"].as_str().is_some());
    assert!(v["artifacts"]["z3_out_path"].as_str().is_some());

    let c_path = PathBuf::from(
        v["artifacts"]["c_path"]
            .as_str()
            .expect("c_path in verify report"),
    );
    let c_src = std::fs::read_to_string(&c_path).expect("read generated C");
    assert!(
        c_src.contains("rt_task_spawn(ctx"),
        "expected async prove driver to spawn tasks, got:\n{c_src}"
    );
    assert!(
        c_src.contains("rt_task_cancel(ctx"),
        "expected async prove driver to cancel tasks, got:\n{c_src}"
    );
    assert!(
        c_src.contains("\\\"contract_kind\\\":\\\"await_invariant\\\""),
        "expected generated C to encode await_invariant proof checks, got:\n{c_src}"
    );
    assert!(
        c_src.contains("\\\"contract_kind\\\":\\\"cancellation_ensures\\\""),
        "expected generated C to encode cancellation proof checks, got:\n{c_src}"
    );
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_defasync_scope_invariant_failure_emits_async_diag() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = fresh_os_tmp_dir("x07_verify_prove_defasync_scope_fail");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let bin_dir = dir.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");

    let cbmc_stub = bin_dir.join("cbmc");
    let cbmc_src = r#"#!/usr/bin/env python3
import json
import pathlib
import sys

args = sys.argv[1:]
if "--help" in args:
    print("cbmc help --no-standard-checks")
    sys.exit(0)

if "--smt2" in args:
    out = None
    for i, tok in enumerate(args):
        if tok == "--outfile" and i + 1 < len(args):
            out = args[i + 1]
            break
    if not out:
        sys.exit(4)
    pathlib.Path(out).write_text("(set-logic QF_AUFBV)\n(assert true)\n(check-sat)\n", encoding="utf-8")
    sys.exit(0)

payload = (
    "X07T_CONTRACT_V1 "
    "{\"clause_id\":\"s0\",\"clause_index\":0,"
    "\"clause_ptr\":\"/decls/1/protocol/scope_invariant/0/expr\","
    "\"contract_kind\":\"scope_invariant\",\"fn\":\"verify_fixture.f\",\"witness\":[]}"
)
print(json.dumps([
    {"program": "CBMC fake 1.0"},
    {"result": [{"status": "FAILURE", "description": payload, "trace": []}]}
]))
"#;
    write_bytes(&cbmc_stub, cbmc_src.as_bytes());
    std::fs::set_permissions(&cbmc_stub, std::fs::Permissions::from_mode(0o755))
        .expect("chmod cbmc");

    let z3_stub = bin_dir.join("z3");
    let z3_src = r#"#!/usr/bin/env python3
print("sat")
"#;
    write_bytes(&z3_stub, z3_src.as_bytes());
    std::fs::set_permissions(&z3_stub, std::fs::Permissions::from_mode(0o755)).expect("chmod z3");

    write_verify_project_files(&dir);
    write_json(
        &dir.join("verify_fixture.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "verify_fixture",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["verify_fixture.f"]},
                {
                    "kind": "defasync",
                    "name": "verify_fixture.f",
                    "params": [],
                    "result": "bytes",
                    "protocol": {
                        "await_invariant": [{"id":"a0","expr":["=",0,0]}],
                        "scope_invariant": [{"id":"s0","expr":["=",0,1]}],
                        "cancellation_ensures": [
                            {"id":"c0","expr":["=",["view.len",["bytes.view","__result"]],0]}
                        ]
                    },
                    "body": ["begin", ["task.yield"], ["bytes.alloc", 0]]
                }
            ]
        }),
    );

    let exe = env!("CARGO_BIN_EXE_x07");
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir.clone()];
    paths.extend(std::env::split_paths(&existing));
    let out = Command::new(exe)
        .current_dir(&dir)
        .env("PATH", std::env::join_paths(paths).expect("join PATH"))
        .args([
            "verify",
            "--prove",
            "--entry",
            "verify_fixture.f",
            "--project",
            "x07.json",
        ])
        .output()
        .expect("run x07 verify prove");
    assert_eq!(
        out.status.code(),
        Some(10),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "counterexample_found");
    assert_eq!(v["result"]["contract"]["contract_kind"], "scope_invariant");
    assert_eq!(v["diagnostics_count"], 1);
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_SCOPE_INVARIANT_FAILED");
    assert!(v["artifacts"]["cex_path"].as_str().is_some());
}

#[test]
fn x07_verify_coverage_defasync_reports_scheduler_model() {
    let dir = fresh_os_tmp_dir("x07_verify_coverage_defasync");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let module = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "verify_fixture",
        "imports": [],
        "decls": [
            {"kind":"export", "names":["verify_fixture.f"]},
            {
                "kind": "defasync",
                "name": "verify_fixture.f",
                "params": [],
                "result": "bytes",
                "protocol": {
                    "await_invariant": [{"id":"a0","expr":["=",0,0]}],
                    "scope_invariant": [{"id":"s0","expr":["=",0,0]}],
                    "cancellation_ensures": [
                        {"id":"c0","expr":["=",["view.len",["bytes.view","__result"]],0]}
                    ]
                },
                "body": ["begin", ["task.yield"], ["bytes.alloc", 0]]
            }
        ]
    }))
    .expect("serialize x07AST module");
    write_bytes(&dir.join("verify_fixture.x07.json"), &module);

    let out = run_x07_in_dir(
        &dir,
        &["verify", "--coverage", "--entry", "verify_fixture.f"],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "coverage");
    assert_eq!(v["result"]["kind"], "coverage_report");
    assert_eq!(v["coverage"]["summary"]["reachable_async"], 1);
    assert_eq!(v["coverage"]["summary"]["supported_async"], 1);
    assert_eq!(v["coverage"]["summary"]["trusted_scheduler_models"], 1);
    let functions = v["coverage"]["functions"].as_array().expect("functions[]");
    assert!(
        functions
            .iter()
            .any(|f| f["symbol"] == "verify_fixture.f" && f["status"] == "supported_async"),
        "expected defasync target in coverage graph"
    );
    assert!(
        functions.iter().any(|f| {
            f["symbol"] == "x07.verify.scheduler_model.deterministic_task_scope_v1"
                && f["status"] == "trusted_scheduler_model"
        }),
        "expected trusted scheduler model in coverage graph"
    );
}

#[test]
fn x07_verify_coverage_emits_schema_shaped_report() {
    let dir = fresh_os_tmp_dir("x07_verify_coverage");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let module = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "verify_fixture",
        "imports": [],
        "decls": [
            {"kind":"export", "names":["verify_fixture.f"]},
            {
                "kind": "defn",
                "name": "verify_fixture.f",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "requires": [{"id":"r0","expr":["=","x","x"]}],
                "body": "x"
            }
        ]
    }))
    .expect("serialize x07AST module");
    write_bytes(&dir.join("verify_fixture.x07.json"), &module);

    let out = run_x07_in_dir(
        &dir,
        &["verify", "--coverage", "--entry", "verify_fixture.f"],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "coverage");
    assert_eq!(v["result"]["kind"], "coverage_report");
    assert_eq!(
        v["coverage"]["schema_version"],
        X07_VERIFY_COVERAGE_SCHEMA_VERSION
    );
    assert_eq!(v["coverage"]["entry"], "verify_fixture.f");
    assert_eq!(v["coverage"]["summary"]["reachable_defn"], 1);
    assert_eq!(v["coverage"]["summary"]["supported_defn"], 1);
    assert_eq!(v["coverage"]["summary"]["uncovered_defn"], 0);
    assert_eq!(v["coverage"]["summary"]["unsupported_defn"], 0);
    let functions = v["coverage"]["functions"].as_array().expect("functions[]");
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0]["symbol"], "verify_fixture.f");
    assert_eq!(functions[0]["kind"], "defn");
    assert_eq!(functions[0]["status"], "supported");
    assert!(functions[0]["source_path"].as_str().is_some());
}

#[test]
fn x07_verify_coverage_with_invalid_project_returns_error_report() {
    let dir = fresh_os_tmp_dir("x07_verify_coverage_invalid_project");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let module = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "verify_fixture",
        "imports": [],
        "decls": [
            {"kind":"export", "names":["verify_fixture.f"]},
            {
                "kind": "defn",
                "name": "verify_fixture.f",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "requires": [{"id":"r0","expr":["=","x","x"]}],
                "body": "x"
            }
        ]
    }))
    .expect("serialize x07AST module");
    write_bytes(&dir.join("verify_fixture.x07.json"), &module);

    let out = Command::new(env!("CARGO_BIN_EXE_x07"))
        .current_dir(&dir)
        .args([
            "verify",
            "--coverage",
            "--entry",
            "verify_fixture.f",
            "--project",
            dir.to_str().expect("utf-8 temp dir"),
        ])
        .output()
        .expect("run x07 verify coverage");

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

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["schema_version"], X07_VERIFY_REPORT_SCHEMA_VERSION);
    assert_eq!(v["mode"], "coverage");
    assert_eq!(v["ok"], false);
    assert_eq!(v["result"]["kind"], "error");
    assert_eq!(v["diagnostics_count"], 1);
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert_eq!(diags[0]["code"], "X07V_EPROJECT");
}

fn scaffold_trust_profile_fixture(dir: &Path) {
    std::fs::create_dir_all(dir.join("src")).expect("create src");
    std::fs::create_dir_all(dir.join("arch/boundaries")).expect("create boundaries");
    std::fs::create_dir_all(dir.join("arch/trust/profiles")).expect("create trust profiles");

    write_json(
        &dir.join("x07.json"),
        &serde_json::json!({
            "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
            "world": "solve-pure",
            "entry": "src/main.x07.json",
            "module_roots": ["src"],
            "dependencies": [],
            "lockfile": "x07.lock.json"
        }),
    );
    write_json(
        &dir.join("x07.lock.json"),
        &serde_json::json!({
            "schema_version": PROJECT_LOCKFILE_SCHEMA_VERSION,
            "dependencies": []
        }),
    );
    write_json(
        &dir.join("src/main.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["main.id_v1"]},
                {
                    "kind":"defn",
                    "name":"main.id_v1",
                    "params":[{"name":"x","ty":"i32"}],
                    "result":"i32",
                    "requires":[{"id":"r0","expr":["=","x","x"]}],
                    "body":"x"
                }
            ],
            "solve":["main.id_v1", 7]
        }),
    );
    write_json(
        &dir.join("arch/manifest.x07arch.json"),
        &serde_json::json!({
            "schema_version": "x07.arch.manifest@0.3.0",
            "repo": {"id": "cli-fixture", "root": "."},
            "externals": {"allowed_import_prefixes": ["std."], "allowed_exact": []},
            "nodes": [
                {
                    "id": "app_core",
                    "match": {"module_prefixes": ["main"]},
                    "world": "solve-pure",
                    "trust_zone": "verified_core",
                    "visibility": {"mode": "public", "visible_to": []},
                    "imports": {
                        "deny_prefixes": ["std.os."],
                        "allow_prefixes": ["main", "std."]
                    },
                    "contracts": {"smoke_entry": "main.id_v1"}
                }
            ],
            "rules": [
                {"kind": "deny_cycles_v1", "id": "deny_cycles.nodes_v1", "scope": "nodes"}
            ],
            "checks": {
                "deny_cycles": true,
                "deny_orphans": true,
                "enforce_visibility": true,
                "enforce_world_caps": true,
                "allowlist_mode": {
                    "enabled": true,
                    "default_allow_external": false,
                    "default_allow_internal": false
                },
                "brand_boundary_v1": {"enabled": true},
                "world_of_imported_v1": {"enabled": true}
            },
            "contracts_v1": {
                "boundaries": {
                    "index_path": "arch/boundaries/index.x07boundary.json",
                    "enforce": "error"
                }
            }
        }),
    );
    write_json(
        &dir.join("arch/boundaries/index.x07boundary.json"),
        &serde_json::json!({
            "schema_version": "x07.arch.boundaries.index@0.1.0",
            "boundaries": [
                {
                    "id": "main.id_v1",
                    "symbol": "main.id_v1",
                    "node_id": "app_core",
                    "kind": "public_function",
                    "worlds_allowed": ["solve-pure"],
                    "input": {"params": [{"name": "x", "ty": "i32"}]},
                    "output": {"ty": "i32"},
                    "smoke": {"entry": "main.id_v1", "tests": ["smoke_main"]},
                    "pbt": {"required": false, "tests": []},
                    "verify": {"required": true, "mode": "prove"}
                }
            ]
        }),
    );
    write_json(
        &dir.join("arch/trust/profiles/verified_core_pure_v1.json"),
        &serde_json::json!({
            "schema_version": "x07.trust.profile@0.4.0",
            "id": "verified_core_pure_v1",
            "claims": ["human_can_review_certificate_not_source"],
            "entrypoints": ["main.id_v1"],
            "worlds_allowed": ["solve-pure"],
            "language_subset": {
                "allow_defasync": false,
                "allow_recursion": false,
                "allow_extern": false,
                "allow_unsafe": false,
                "allow_ffi": false,
                "allow_dynamic_dispatch": false
            },
            "arch_requirements": {
                "manifest_min_version": "x07.arch.manifest@0.3.0",
                "require_allowlist_mode": true,
                "require_deny_cycles": true,
                "require_deny_orphans": true,
                "require_visibility": true,
                "require_world_caps": true,
                "require_brand_boundaries": true
            },
            "evidence_requirements": {
                "require_boundary_index": true,
                "require_schema_derive_check": true,
                "require_smoke_harnesses": true,
                "require_unit_tests": true,
                "require_pbt": "public_boundaries_only",
                "require_proof_mode": "prove",
                "require_proof_coverage": "all_reachable_defn",
                "require_async_proof_coverage": false,
                "require_per_symbol_prove_reports_defn": true,
                "require_per_symbol_prove_reports_async": false,
                "allow_coverage_summary_imports": false,
                "require_capsule_attestations": false,
                "require_runtime_attestation": false,
                "require_effect_log_digests": false,
                "require_peer_policies": false,
                "require_network_capsules": false,
                "require_dependency_closure_attestation": false,
                "require_compile_attestation": true,
                "require_trust_report_clean": true,
                "require_sbom": true
            },
            "sandbox_requirements": {
                "sandbox_backend": "any",
                "forbid_weaker_isolation": false,
                "network_mode": "any",
                "network_enforcement": "any"
            }
        }),
    );
}

fn scaffold_sandbox_trust_profile_fixture(dir: &Path, world: &str, net_enabled: bool) {
    std::fs::create_dir_all(dir.join("src")).expect("create src");
    std::fs::create_dir_all(dir.join("arch/boundaries")).expect("create boundaries");
    std::fs::create_dir_all(dir.join("arch/capsules")).expect("create capsules");
    std::fs::create_dir_all(dir.join("arch/trust/profiles")).expect("create trust profiles");
    std::fs::create_dir_all(dir.join("policy")).expect("create policy");

    let mut project = serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "world": world,
        "entry": "src/main.x07.json",
        "module_roots": ["src"],
        "dependencies": [],
        "lockfile": "x07.lock.json"
    });
    if world == "run-os-sandboxed" {
        project["default_profile"] = Value::String("sandbox".to_string());
        project["profiles"] = serde_json::json!({
            "sandbox": {
                "world": "run-os-sandboxed",
                "policy": "policy/run-os.json"
            }
        });
    }
    write_json(&dir.join("x07.json"), &project);
    write_json(
        &dir.join("x07.lock.json"),
        &serde_json::json!({
            "schema_version": PROJECT_LOCKFILE_SCHEMA_VERSION,
            "dependencies": []
        }),
    );
    write_json(
        &dir.join("src/main.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [
                {"kind":"export", "names":["main.id_v1"]},
                {
                    "kind":"defn",
                    "name":"main.id_v1",
                    "params":[{"name":"x","ty":"i32"}],
                    "result":"i32",
                    "requires":[{"id":"r0","expr":["=","x","x"]}],
                    "body":"x"
                }
            ],
            "solve":["main.id_v1", 7]
        }),
    );
    write_json(
        &dir.join("arch/manifest.x07arch.json"),
        &serde_json::json!({
            "schema_version": "x07.arch.manifest@0.3.0",
            "repo": {"id": "cli-sandbox-fixture", "root": "."},
            "externals": {"allowed_import_prefixes": ["std."], "allowed_exact": []},
            "nodes": [
                {
                    "id": "app_core",
                    "match": {"module_prefixes": ["main"]},
                    "world": world,
                    "trust_zone": "verified_core",
                    "visibility": {"mode": "public", "visible_to": []},
                    "imports": {
                        "deny_prefixes": ["std.os."],
                        "allow_prefixes": ["main", "std."]
                    },
                    "contracts": {"smoke_entry": "main.id_v1"}
                }
            ],
            "rules": [
                {"kind": "deny_cycles_v1", "id": "deny_cycles.nodes_v1", "scope": "nodes"}
            ],
            "checks": {
                "deny_cycles": true,
                "deny_orphans": true,
                "enforce_visibility": true,
                "enforce_world_caps": true,
                "allowlist_mode": {
                    "enabled": true,
                    "default_allow_external": false,
                    "default_allow_internal": false
                },
                "brand_boundary_v1": {"enabled": true},
                "world_of_imported_v1": {"enabled": true}
            },
            "contracts_v1": {
                "boundaries": {
                    "index_path": "arch/boundaries/index.x07boundary.json",
                    "enforce": "error"
                }
            }
        }),
    );
    write_json(
        &dir.join("arch/boundaries/index.x07boundary.json"),
        &serde_json::json!({
            "schema_version": "x07.arch.boundaries.index@0.1.0",
            "boundaries": [
                {
                    "id": "main.id_v1",
                    "symbol": "main.id_v1",
                    "node_id": "app_core",
                    "kind": "public_function",
                    "from_zone": "verified_core",
                    "to_zone": "verified_core",
                    "worlds_allowed": [world],
                    "input": {"params": [{"name": "x", "ty": "i32"}]},
                    "output": {"ty": "i32"},
                    "smoke": {"entry": "main.id_v1", "tests": ["smoke_main"]},
                    "pbt": {"required": false, "tests": []},
                    "verify": {"required": true, "mode": "prove"}
                }
            ]
        }),
    );
    write_json(
        &dir.join("arch/capsules/index.x07capsule.json"),
        &serde_json::json!({
            "schema_version": "x07.capsule.index@0.1.0",
            "capsules": [
                {
                    "id": "capsule.echo_v1",
                    "worlds_allowed": [world],
                    "capabilities": ["net"],
                    "contract_path": "capsule.echo.contract.json",
                    "attestation_path": "capsule.echo.attest.json"
                }
            ]
        }),
    );
    write_json(
        &dir.join("arch/capsules/capsule.echo.contract.json"),
        &serde_json::json!({
            "schema_version": "x07.capsule.contract@0.2.0",
            "id": "capsule.echo_v1",
            "worlds_allowed": [world],
            "capabilities": ["net"],
            "language": {
                "allow_unsafe": false,
                "allow_ffi": false
            },
            "input": {
                "shape": {
                    "brand": "capsule.echo.in_v1"
                }
            },
            "output": {
                "shape": {
                    "brand": "capsule.echo.out_v1"
                }
            },
            "error_spaces": ["capsule.echo.error_v1"],
            "effect_log": {
                "schema_path": "capsule.echo.effect_log.json",
                "redaction": "metadata_only",
                "replay_safe": true
            },
            "replay": {
                "mode": "deterministic"
            },
            "conformance": {
                "tests": ["smoke_main"],
                "report_path": null
            },
            "network": null
        }),
    );
    write_json(
        &dir.join("arch/capsules/capsule.echo.attest.json"),
        &serde_json::json!({
            "schema_version": "x07.capsule.attest@0.2.0",
            "capsule_id": "capsule.echo_v1",
            "contract_digest": format!("sha256:{}", "0".repeat(64)),
            "module_digests": [],
            "lockfile_digest": format!("sha256:{}", "1".repeat(64)),
            "conformance_report_digest": format!("sha256:{}", "2".repeat(64)),
            "peer_policy_digests": [],
            "request_contract_digest": null,
            "response_contract_digest": null
        }),
    );
    write_json(
        &dir.join("arch/capsules/capsule.echo.effect_log.json"),
        &serde_json::json!({
            "schema_version": "x07.effect.log@0.2.0",
            "capsule_id": "capsule.echo_v1",
            "events": []
        }),
    );
    if world == "run-os-sandboxed" {
        write_json(
            &dir.join("policy/run-os.json"),
            &serde_json::json!({
                "schema_version": "x07.run-os-policy@0.1.0",
                "policy_id": "sandbox_fixture",
                "limits": {
                    "cpu_ms": 5000,
                    "wall_ms": 6000,
                    "mem_bytes": 33554432,
                    "fds": 16,
                    "procs": 8
                },
                "fs": {
                    "enabled": false,
                    "read_roots": [],
                    "write_roots": [],
                    "deny_hidden": true
                },
                "net": {
                    "enabled": net_enabled,
                    "allow_dns": net_enabled,
                    "allow_tcp": net_enabled,
                    "allow_udp": false,
                    "allow_hosts": if net_enabled {
                        serde_json::json!([{"host": "127.0.0.1", "ports": [4317]}])
                    } else {
                        serde_json::json!([])
                    }
                },
                "env": {
                    "enabled": false,
                    "allow_keys": [],
                    "deny_keys": []
                },
                "time": {
                    "enabled": false,
                    "allow_monotonic": false,
                    "allow_wall_clock": false,
                    "allow_sleep": false,
                    "max_sleep_ms": 0,
                    "allow_local_tzid": false
                },
                "language": {
                    "allow_unsafe": false,
                    "allow_ffi": false
                },
                "process": {
                    "enabled": false,
                    "allow_spawn": false,
                    "max_live": 0,
                    "max_spawns": 0,
                    "allow_exec": false,
                    "allow_exit": false
                }
            }),
        );
    }
}

#[test]
fn x07_trust_profile_check_accepts_matching_profile() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_profile_check");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    scaffold_trust_profile_fixture(&dir);

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--profile",
            "arch/trust/profiles/verified_core_pure_v1.json",
            "--project",
            "x07.json",
            "--entry",
            "main.id_v1",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse trust profile report");
    assert_eq!(v["schema_version"], "x07.trust.profile.check@0.1.0");
    assert_eq!(v["ok"], true);
    assert_eq!(v["profile"], "verified_core_pure_v1");
    assert_eq!(v["entry"], "main.id_v1");
}

#[test]
fn x07_trust_profile_check_accepts_sandboxed_local_profile() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_profile_check_sandbox_ok");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    scaffold_sandbox_trust_profile_fixture(&dir, "run-os-sandboxed", false);
    write_json(
        &dir.join("arch/trust/profiles/trusted_program_sandboxed_local_v1.json"),
        &serde_json::json!({
            "schema_version": "x07.trust.profile@0.4.0",
            "id": "trusted_program_sandboxed_local_v1",
            "claims": ["human_can_review_certificate_not_source"],
            "entrypoints": ["main.id_v1"],
            "worlds_allowed": ["run-os-sandboxed"],
            "language_subset": {
                "allow_defasync": true,
                "allow_recursion": false,
                "allow_extern": false,
                "allow_unsafe": false,
                "allow_ffi": false,
                "allow_dynamic_dispatch": false
            },
            "arch_requirements": {
                "manifest_min_version": "x07.arch.manifest@0.3.0",
                "require_allowlist_mode": true,
                "require_deny_cycles": true,
                "require_deny_orphans": true,
                "require_visibility": true,
                "require_world_caps": true,
                "require_brand_boundaries": true
            },
            "evidence_requirements": {
                "require_boundary_index": true,
                "require_schema_derive_check": true,
                "require_smoke_harnesses": true,
                "require_unit_tests": true,
                "require_pbt": "public_boundaries_only",
                "require_proof_mode": "prove",
                "require_proof_coverage": "all_reachable_defn",
                "require_async_proof_coverage": true,
                "require_per_symbol_prove_reports_defn": true,
                "require_per_symbol_prove_reports_async": true,
                "allow_coverage_summary_imports": false,
                "require_capsule_attestations": true,
                "require_runtime_attestation": true,
                "require_effect_log_digests": true,
                "require_peer_policies": false,
                "require_network_capsules": false,
                "require_dependency_closure_attestation": false,
                "require_compile_attestation": true,
                "require_trust_report_clean": true,
                "require_sbom": true
            },
            "sandbox_requirements": {
                "sandbox_backend": "vm",
                "forbid_weaker_isolation": true,
                "network_mode": "none",
                "network_enforcement": "none"
            }
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--profile",
            "arch/trust/profiles/trusted_program_sandboxed_local_v1.json",
            "--project",
            "x07.json",
            "--entry",
            "main.id_v1",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse trust profile report");
    assert_eq!(v["ok"], true);
    assert_eq!(v["profile"], "trusted_program_sandboxed_local_v1");
}

#[test]
fn x07_trust_profile_check_rejects_run_os_for_sandboxed_local_profile() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_profile_check_sandbox_run_os");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    scaffold_sandbox_trust_profile_fixture(&dir, "run-os", false);
    write_json(
        &dir.join("arch/trust/profiles/trusted_program_sandboxed_local_v1.json"),
        &serde_json::json!({
            "schema_version": "x07.trust.profile@0.4.0",
            "id": "trusted_program_sandboxed_local_v1",
            "claims": ["human_can_review_certificate_not_source"],
            "entrypoints": ["main.id_v1"],
            "worlds_allowed": ["run-os-sandboxed"],
            "language_subset": {
                "allow_defasync": true,
                "allow_recursion": false,
                "allow_extern": false,
                "allow_unsafe": false,
                "allow_ffi": false,
                "allow_dynamic_dispatch": false
            },
            "arch_requirements": {
                "manifest_min_version": "x07.arch.manifest@0.3.0",
                "require_allowlist_mode": true,
                "require_deny_cycles": true,
                "require_deny_orphans": true,
                "require_visibility": true,
                "require_world_caps": true,
                "require_brand_boundaries": true
            },
            "evidence_requirements": {
                "require_boundary_index": true,
                "require_schema_derive_check": true,
                "require_smoke_harnesses": true,
                "require_unit_tests": true,
                "require_pbt": "public_boundaries_only",
                "require_proof_mode": "prove",
                "require_proof_coverage": "all_reachable_defn",
                "require_async_proof_coverage": true,
                "require_per_symbol_prove_reports_defn": true,
                "require_per_symbol_prove_reports_async": true,
                "allow_coverage_summary_imports": false,
                "require_capsule_attestations": true,
                "require_runtime_attestation": true,
                "require_effect_log_digests": true,
                "require_peer_policies": false,
                "require_network_capsules": false,
                "require_dependency_closure_attestation": false,
                "require_compile_attestation": true,
                "require_trust_report_clean": true,
                "require_sbom": true
            },
            "sandbox_requirements": {
                "sandbox_backend": "vm",
                "forbid_weaker_isolation": true,
                "network_mode": "none",
                "network_enforcement": "none"
            }
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--profile",
            "arch/trust/profiles/trusted_program_sandboxed_local_v1.json",
            "--project",
            "x07.json",
            "--entry",
            "main.id_v1",
        ],
    );
    assert_eq!(out.status.code(), Some(20));
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse trust profile report");
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_SANDBOX_BACKEND_REQUIRED"));
}

#[test]
fn x07_trust_profile_check_rejects_network_enabled_local_sandbox_policy() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_profile_check_sandbox_net");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    scaffold_sandbox_trust_profile_fixture(&dir, "run-os-sandboxed", true);
    write_json(
        &dir.join("arch/trust/profiles/trusted_program_sandboxed_local_v1.json"),
        &serde_json::json!({
            "schema_version": "x07.trust.profile@0.4.0",
            "id": "trusted_program_sandboxed_local_v1",
            "claims": ["human_can_review_certificate_not_source"],
            "entrypoints": ["main.id_v1"],
            "worlds_allowed": ["run-os-sandboxed"],
            "language_subset": {
                "allow_defasync": true,
                "allow_recursion": false,
                "allow_extern": false,
                "allow_unsafe": false,
                "allow_ffi": false,
                "allow_dynamic_dispatch": false
            },
            "arch_requirements": {
                "manifest_min_version": "x07.arch.manifest@0.3.0",
                "require_allowlist_mode": true,
                "require_deny_cycles": true,
                "require_deny_orphans": true,
                "require_visibility": true,
                "require_world_caps": true,
                "require_brand_boundaries": true
            },
            "evidence_requirements": {
                "require_boundary_index": true,
                "require_schema_derive_check": true,
                "require_smoke_harnesses": true,
                "require_unit_tests": true,
                "require_pbt": "public_boundaries_only",
                "require_proof_mode": "prove",
                "require_proof_coverage": "all_reachable_defn",
                "require_async_proof_coverage": true,
                "require_per_symbol_prove_reports_defn": true,
                "require_per_symbol_prove_reports_async": true,
                "allow_coverage_summary_imports": false,
                "require_capsule_attestations": true,
                "require_runtime_attestation": true,
                "require_effect_log_digests": true,
                "require_peer_policies": false,
                "require_network_capsules": false,
                "require_dependency_closure_attestation": false,
                "require_compile_attestation": true,
                "require_trust_report_clean": true,
                "require_sbom": true
            },
            "sandbox_requirements": {
                "sandbox_backend": "vm",
                "forbid_weaker_isolation": true,
                "network_mode": "none",
                "network_enforcement": "none"
            }
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--profile",
            "arch/trust/profiles/trusted_program_sandboxed_local_v1.json",
            "--project",
            "x07.json",
            "--entry",
            "main.id_v1",
        ],
    );
    assert_eq!(out.status.code(), Some(20));
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse trust profile report");
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_NETWORK_MODE_FORBIDDEN"));
}

#[test]
fn x07_trust_profile_check_rejects_weakened_sandbox_profile_contract() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_profile_check_sandbox_contract");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    write_json(
        &dir.join("bad_profile.json"),
        &serde_json::json!({
            "schema_version": "x07.trust.profile@0.4.0",
            "id": "trusted_program_sandboxed_local_v1",
            "claims": ["human_can_review_certificate_not_source"],
            "entrypoints": ["main.id_v1"],
            "worlds_allowed": ["run-os", "run-os-sandboxed"],
            "language_subset": {
                "allow_defasync": true,
                "allow_recursion": false,
                "allow_extern": false,
                "allow_unsafe": false,
                "allow_ffi": false,
                "allow_dynamic_dispatch": false
            },
            "arch_requirements": {
                "manifest_min_version": "x07.arch.manifest@0.3.0",
                "require_allowlist_mode": true,
                "require_deny_cycles": true,
                "require_deny_orphans": true,
                "require_visibility": true,
                "require_world_caps": true,
                "require_brand_boundaries": true
            },
            "evidence_requirements": {
                "require_boundary_index": true,
                "require_schema_derive_check": true,
                "require_smoke_harnesses": true,
                "require_unit_tests": true,
                "require_pbt": "public_boundaries_only",
                "require_proof_mode": "prove",
                "require_proof_coverage": "all_reachable_defn",
                "require_async_proof_coverage": false,
                "require_per_symbol_prove_reports_defn": true,
                "require_per_symbol_prove_reports_async": true,
                "allow_coverage_summary_imports": false,
                "require_capsule_attestations": false,
                "require_runtime_attestation": false,
                "require_effect_log_digests": false,
                "require_peer_policies": false,
                "require_network_capsules": false,
                "require_dependency_closure_attestation": false,
                "require_compile_attestation": true,
                "require_trust_report_clean": true,
                "require_sbom": true
            },
            "sandbox_requirements": {
                "sandbox_backend": "os",
                "forbid_weaker_isolation": false,
                "network_mode": "allowlist",
                "network_enforcement": "unsupported"
            }
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &["trust", "profile", "check", "--profile", "bad_profile.json"],
    );
    assert_eq!(out.status.code(), Some(20));
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse trust profile report");
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_ASYNC_PROOF_REQUIRED"));
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_CAPSULE_ATTEST_REQUIRED"));
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_RUNTIME_ATTEST_REQUIRED"));
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_SANDBOX_BACKEND_REQUIRED"));
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_NETWORK_MODE_FORBIDDEN"));
}

#[test]
fn x07_trust_profile_check_rejects_weakened_network_sandbox_profile_contract() {
    let root = repo_root();
    let dir = fresh_tmp_dir(
        &root,
        "tmp_x07_trust_profile_check_network_sandbox_contract",
    );
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    scaffold_sandbox_trust_profile_fixture(&dir, "run-os-sandboxed", true);
    write_json(
        &dir.join("bad_profile.json"),
        &serde_json::json!({
            "schema_version": "x07.trust.profile@0.4.0",
            "id": "trusted_program_sandboxed_net_v1",
            "claims": ["human_can_review_certificate_not_source"],
            "entrypoints": ["main.id_v1"],
            "worlds_allowed": ["run-os", "run-os-sandboxed"],
            "language_subset": {
                "allow_defasync": true,
                "allow_recursion": false,
                "allow_extern": false,
                "allow_unsafe": false,
                "allow_ffi": false,
                "allow_dynamic_dispatch": false
            },
            "arch_requirements": {
                "manifest_min_version": "x07.arch.manifest@0.3.0",
                "require_allowlist_mode": true,
                "require_deny_cycles": true,
                "require_deny_orphans": true,
                "require_visibility": true,
                "require_world_caps": true,
                "require_brand_boundaries": true
            },
            "evidence_requirements": {
                "require_boundary_index": true,
                "require_schema_derive_check": true,
                "require_smoke_harnesses": true,
                "require_unit_tests": true,
                "require_pbt": "public_boundaries_only",
                "require_proof_mode": "prove",
                "require_proof_coverage": "all_reachable_defn",
                "require_async_proof_coverage": true,
                "require_per_symbol_prove_reports_defn": true,
                "require_per_symbol_prove_reports_async": true,
                "allow_coverage_summary_imports": false,
                "require_capsule_attestations": true,
                "require_runtime_attestation": true,
                "require_effect_log_digests": true,
                "require_peer_policies": false,
                "require_network_capsules": false,
                "require_dependency_closure_attestation": false,
                "require_compile_attestation": true,
                "require_trust_report_clean": true,
                "require_sbom": true
            },
            "sandbox_requirements": {
                "sandbox_backend": "os",
                "forbid_weaker_isolation": false,
                "network_mode": "allowlist",
                "network_enforcement": "unsupported"
            }
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--profile",
            "bad_profile.json",
            "--project",
            "x07.json",
            "--entry",
            "main.id_v1",
        ],
    );
    assert_eq!(out.status.code(), Some(20));
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse trust profile report");
    let diags = v["diagnostics"].as_array().expect("diagnostics[]");
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_BACKEND_NOT_CERTIFIABLE"));
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_NETWORK_PROFILE_REQUIRED"));
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_PEER_POLICY_REQUIRED"));
    assert!(diags
        .iter()
        .any(|diag| diag["code"] == "X07TP_DEP_CLOSURE_REQUIRED"));
}

#[test]
fn x07_verify_coverage_walks_reachable_functions_and_primitives() {
    let dir = fresh_os_tmp_dir("x07_verify_coverage_graph");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let module = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "verify_fixture",
        "imports": [],
        "decls": [
            {"kind":"export", "names":["verify_fixture.f", "verify_fixture.g"]},
            {
                "kind": "defn",
                "name": "verify_fixture.g",
                "params": [],
                "result": "bytes",
                "requires": [{"id":"r0","expr":["=",0,0]}],
                "body": ["bytes.empty"]
            },
            {
                "kind": "defn",
                "name": "verify_fixture.f",
                "params": [],
                "result": "bytes",
                "requires": [{"id":"r1","expr":["=",1,1]}],
                "body": ["verify_fixture.g"]
            }
        ]
    }))
    .expect("serialize x07AST module");
    write_bytes(&dir.join("verify_fixture.x07.json"), &module);

    let out = run_x07_in_dir(
        &dir,
        &["verify", "--coverage", "--entry", "verify_fixture.f"],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    let summary = &v["coverage"]["summary"];
    assert_eq!(summary["reachable_defn"], 2);
    assert_eq!(summary["supported_defn"], 2);
    assert_eq!(summary["trusted_primitives"], 1);

    let functions = v["coverage"]["functions"].as_array().expect("functions[]");
    assert!(
        functions
            .iter()
            .any(|f| f["symbol"] == "verify_fixture.f" && f["status"] == "supported"),
        "expected verify_fixture.f in coverage graph"
    );
    assert!(
        functions
            .iter()
            .any(|f| f["symbol"] == "verify_fixture.g" && f["status"] == "supported"),
        "expected verify_fixture.g in coverage graph"
    );
    assert!(
        functions
            .iter()
            .any(|f| f["symbol"] == "bytes.empty" && f["status"] == "trusted_primitive"),
        "expected bytes.empty trusted primitive in coverage graph"
    );
}

#[cfg(unix)]
#[test]
fn x07_verify_prove_stubs_trusted_imported_primitives_in_generated_c() {
    let dir = fresh_os_tmp_dir("x07_verify_prove_trusted_stub");
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let module = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "verify_fixture",
        "imports": ["std.codec"],
        "decls": [
            {"kind":"export", "names":["verify_fixture.f"]},
            {
                "kind": "defn",
                "name": "verify_fixture.f",
                "params": [],
                "result": "i32",
                "requires": [{"id":"r0","expr":["=",0,0]}],
                "ensures": [{"id":"e0","expr":["=","__result",0]}],
                "body": ["begin",
                    ["let","payload",["bytes.lit","ABCD"]],
                    ["let","payload_v",["bytes.view","payload"]],
                    ["let","_",["std.codec.read_u32_le","payload_v",0]],
                    0
                ]
            }
        ]
    }))
    .expect("serialize x07AST module");
    write_bytes(&dir.join("verify_fixture.x07.json"), &module);

    let out = run_x07_in_dir_with_fake_prove_solvers(
        &dir,
        &[
            "verify",
            "--prove",
            "--allow-imported-stubs",
            "--entry",
            "verify_fixture.f",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse verify report JSON");
    assert_eq!(v["mode"], "prove");
    assert_eq!(v["result"]["kind"], "proven");

    let c_path = PathBuf::from(
        v["artifacts"]["c_path"]
            .as_str()
            .expect("c_path in verify report"),
    );
    let c_src = std::fs::read_to_string(&c_path).expect("read generated C");
    assert!(
        c_src.contains("static uint32_t user_std_codec_read_u32_le"),
        "expected generated C to contain trusted primitive symbol, got:\n{c_src}"
    );
    assert!(
        c_src.contains(
            "static uint32_t user_std_codec_read_u32_le(ctx_t* ctx, bytes_view_t input, bytes_view_t p0, uint32_t p1) {\n  (void)ctx;\n  (void)input;\n  (void)p0;\n  (void)p1;\n  return UINT32_C(0);\n}"
        ),
        "expected trusted primitive body to be stubbed in generated C, got:\n{c_src}"
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

    let has_ast_slice = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("ast.slice")
    });
    assert!(has_ast_slice, "missing ast.slice in --cli-specrows output");

    let has_agent_context = rows.iter().any(|row| {
        row.as_array()
            .and_then(|cols| cols.first())
            .and_then(|v| v.as_str())
            == Some("agent.context")
    });
    assert!(
        has_agent_context,
        "missing agent.context in --cli-specrows output"
    );
}

fn write_json(path: &Path, doc: &Value) {
    let bytes = serde_json::to_vec_pretty(doc).expect("serialize JSON");
    write_bytes(path, &bytes);
}

fn write_verify_project_files(dir: &Path) {
    let project_doc = serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "world": "solve-pure",
        "entry": "verify_fixture.x07.json",
        "module_roots": ["."],
        "dependencies": [],
        "lockfile": "x07.lock.json"
    });
    let project_bytes = serde_json::to_vec(&project_doc).expect("serialize x07.json");
    write_json(&dir.join("x07.json"), &project_doc);
    write_lockfile_for_project_bytes(dir, &project_bytes);
}

fn write_lockfile_for_project_bytes(dir: &Path, project_bytes: &[u8]) {
    let project_path = dir.join("x07.json");
    let manifest = project::parse_project_manifest_bytes(project_bytes, &project_path)
        .expect("parse x07.json");
    let lock = project::compute_lockfile(&project_path, &manifest).expect("compute lockfile");
    let bytes = serde_json::to_vec_pretty(&lock).expect("serialize x07.lock.json");
    write_bytes(&dir.join("x07.lock.json"), &bytes);
}

#[cfg(unix)]
fn write_fake_prove_solvers(bin_dir: &Path) {
    std::fs::create_dir_all(bin_dir).expect("create fake solver bin dir");
    let cbmc_path = bin_dir.join("cbmc");
    let cbmc_src = r#"#!/usr/bin/env python3
import pathlib
import sys

args = sys.argv[1:]
if "--help" in args:
    print("cbmc help --no-standard-checks")
    sys.exit(0)

if "--smt2" in args:
    out = None
    for i, tok in enumerate(args):
        if tok == "--outfile" and i + 1 < len(args):
            out = args[i + 1]
            break
    if out is None:
        print("missing --outfile", file=sys.stderr)
        sys.exit(4)
    pathlib.Path(out).write_text(
        "(set-logic QF_AUFBV)\n(assert true)\n(check-sat)\n",
        encoding="utf-8",
    )
    sys.exit(0)

print("[]")
"#;
    write_bytes(&cbmc_path, cbmc_src.as_bytes());
    std::fs::set_permissions(&cbmc_path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake cbmc");

    let z3_path = bin_dir.join("z3");
    let z3_src = r#"#!/usr/bin/env python3
print("unsat")
"#;
    write_bytes(&z3_path, z3_src.as_bytes());
    std::fs::set_permissions(&z3_path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake z3");
}

#[cfg(unix)]
fn prove_report_artifact_path(report: &Value, key: &str) -> PathBuf {
    PathBuf::from(
        report["artifacts"][key]
            .as_str()
            .unwrap_or_else(|| panic!("missing prove artifact path for {key}")),
    )
}

#[cfg(unix)]
fn bundled_imported_summary_path(proof_path: &Path) -> PathBuf {
    let imported_dir = proof_path
        .parent()
        .expect("proof bundle dir")
        .join("imported_proof_summaries");
    let mut entries = std::fs::read_dir(&imported_dir)
        .expect("read imported proof summaries dir")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(entries.len(), 1, "expected one imported proof summary");
    entries.remove(0)
}

#[cfg(unix)]
fn proof_check_diag_code(report: &Value) -> &str {
    report["diagnostics"]
        .as_array()
        .and_then(|diags| diags.first())
        .and_then(|diag| diag["code"].as_str())
        .expect("proof-check diagnostic code")
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
      "schema_version": "x07.arch.manifest@0.3.0",
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
        "trust_zone".to_string(),
        Value::String("verified_core".to_string()),
    );
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
fn x07_schema_derive_emits_boundary_stub() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_schema_boundary_stub");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let fixture = root.join("tests/fixtures/schema_derive/example_020.x07schema.json");
    let schema_bytes = std::fs::read(&fixture).expect("read schema fixture");
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
            "--emit-boundary-stub",
            "--write",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stub_path = std::fs::read_dir(dir.join("arch/boundaries"))
        .expect("read boundary stub dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".stub.x07boundary.json"))
        })
        .expect("locate generated boundary stub");
    let stub_doc: Value =
        serde_json::from_slice(&std::fs::read(&stub_path).expect("read boundary stub"))
            .expect("parse boundary stub");
    assert_eq!(
        stub_doc["schema_version"],
        "x07.arch.boundaries.index@0.1.0"
    );
    assert!(
        stub_doc["boundaries"]
            .as_array()
            .is_some_and(|boundaries| !boundaries.is_empty()),
        "expected generated boundary stub entries"
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
            "--emit-boundary-stub",
            "--check",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
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
fn x07_lint_resolves_builtin_import_sigs() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_lint_builtin_import_sigs");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    // Regression: `x07 lint` must typecheck stdlib imports, otherwise calls like
    // `std.deque.emit_le` get an inferred return type that diverges from `x07 check`.
    let program = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": ["std.deque"],
        "decls": [],
        "solve": ["begin",
            ["let", "dq", ["tapp", "std.deque.with_capacity", ["tys", "u32"], 2]],
            ["set", "dq", ["tapp", "std.deque.push_back", ["tys", "u32"], "dq", 1]],
            ["let", "out", ["tapp", "std.deque.emit_le", ["tys", "u32"], ["bytes.view", "dq"]]],
            ["bytes.concat", ["bytes.lit", "a"], "out"]
        ]
    }))
    .expect("serialize x07AST");
    let program_path = dir.join("main.x07.json");
    write_bytes(&program_path, &program);

    let out = run_x07(&["lint", "--input", program_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lint_report = parse_json_stdout(&out);
    assert_eq!(lint_report["ok"], true);
    assert!(
        lint_report["diagnostics"]
            .as_array()
            .expect("diagnostics[]")
            .is_empty(),
        "expected lint to be clean, got:\n{}",
        serde_json::to_string_pretty(&lint_report).expect("pretty report")
    );
}

#[test]
fn x07_fmt_accepts_positional_paths() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_fmt_positional");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program_doc = serde_json::json!({
        "kind": "entry",
        "schema_version": X07AST_SCHEMA_VERSION,
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["bytes.alloc", 0],
    });
    let program_path = dir.join("main.x07.json");
    write_bytes(
        &program_path,
        serde_json::to_vec_pretty(&program_doc)
            .expect("encode pretty x07AST json")
            .as_slice(),
    );

    let out = run_x07(&["fmt", "--check", program_path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("file is not formatted:"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_x07(&["fmt", "--write", program_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_x07(&["fmt", "--check", program_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn x07_fmt_pretty_writes_multiline_output() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_fmt_pretty");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program_doc = serde_json::json!({
        "kind": "entry",
        "schema_version": X07AST_SCHEMA_VERSION,
        "module_id": "main",
        "imports": [],
        "decls": [],
        "solve": ["bytes.alloc", 0],
    });
    let program_path = dir.join("main.x07.json");
    write_bytes(
        &program_path,
        serde_json::to_vec_pretty(&program_doc)
            .expect("encode pretty x07AST json")
            .as_slice(),
    );

    let out = run_x07(&["fmt", "--write", program_path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07(&["fmt", "--write", "--pretty", program_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let contents = std::fs::read_to_string(&program_path).expect("read formatted file");
    assert!(
        contents.contains("\n  \""),
        "expected multi-line JSON output, got:\n{contents}"
    );

    let out = run_x07(&["fmt", "--check", "--pretty", program_path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07(&["fmt", "--check", program_path.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
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
fn x07_fix_suggest_generics_does_not_merge_different_contracts() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_fix_suggest_generics_contracts_mismatch");
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
                "requires": [{ "expr": 1 }],
                "params": [{ "name": "x", "ty": "u32" }],
                "result": "u32",
                "body": "x"
            },
            {
                "kind": "defn",
                "name": "main.id_i32",
                "requires": [{ "expr": 0 }],
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

    let patch_bytes = std::fs::read(&patchset_path).expect("read patchset");
    let patch_doc: Value = serde_json::from_slice(&patch_bytes).expect("parse patchset JSON");
    assert_eq!(patch_doc["schema_version"], X07_PATCHSET_SCHEMA_VERSION);
    assert_eq!(patch_doc["patches"].as_array().expect("patches[]").len(), 0);
}

#[test]
fn x07_fix_suggest_generics_moves_contracts_to_base_and_keeps_schema() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_fix_suggest_generics_contracts_preserve");
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
                "requires": [{ "expr": 1 }],
                "ensures": [{ "expr": 1 }],
                "invariant": [{ "expr": 1 }],
                "params": [{ "name": "x", "ty": "u32" }],
                "result": "u32",
                "body": "x"
            },
            {
                "kind": "defn",
                "name": "main.id_i32",
                "requires": [{ "expr": 1 }],
                "ensures": [{ "expr": 1 }],
                "invariant": [{ "expr": 1 }],
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
    assert_eq!(file.schema_version, X07AST_SCHEMA_VERSION);

    let base = file
        .functions
        .iter()
        .find(|f| f.name == "main.id")
        .expect("missing base defn");
    assert_eq!(base.requires.len(), 1);
    assert_eq!(base.ensures.len(), 1);
    assert_eq!(base.invariant.len(), 1);

    for wrapper_name in ["main.id_u32", "main.id_i32"] {
        let w = file
            .functions
            .iter()
            .find(|f| f.name == wrapper_name)
            .unwrap_or_else(|| panic!("missing wrapper {wrapper_name:?}"));
        assert!(w.requires.is_empty(), "expected wrapper requires cleared");
        assert!(w.ensures.is_empty(), "expected wrapper ensures cleared");
        assert!(w.invariant.is_empty(), "expected wrapper invariant cleared");
    }

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
fn x07_check_run_os_sandboxed_respects_policy_language_toggles() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_check_policy_language_toggles");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(dir.join("src")).expect("create src dir");
    std::fs::create_dir_all(dir.join("policy")).expect("create policy dir");

    let policy_doc = serde_json::json!({
        "schema_version": "x07.run-os-policy@0.1.0",
        "policy_id": "x07_check_policy_language_toggles",
        "limits": {
            "cpu_ms": 1000,
            "wall_ms": 1000,
            "mem_bytes": 1048576,
            "fds": 16,
            "procs": 8
        },
        "fs": {
            "enabled": false,
            "read_roots": [],
            "write_roots": [],
            "deny_hidden": true
        },
        "net": {
            "enabled": false,
            "allow_dns": false,
            "allow_tcp": false,
            "allow_udp": false,
            "allow_hosts": []
        },
        "env": {
            "enabled": false,
            "allow_keys": [],
            "deny_keys": []
        },
        "time": {
            "enabled": false,
            "allow_monotonic": false,
            "allow_wall_clock": false,
            "allow_sleep": false,
            "max_sleep_ms": 0,
            "allow_local_tzid": false
        },
        "language": {
            "allow_unsafe": true,
            "allow_ffi": true
        },
        "process": {
            "enabled": false,
            "allow_spawn": false,
            "max_live": 0,
            "max_spawns": 0,
            "allow_exec": false,
            "allow_exit": false
        }
    });
    write_json(&dir.join("policy/run-os.json"), &policy_doc);

    let program_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "extern",
                "abi": "C",
                "link_name": "x07_test_dummy",
                "name": "main._ffi.test",
                "params": [{"name":"p","ty":"ptr_const_u8"}],
                "result": "i32"
            }
        ],
        "solve": ["bytes.lit", "ok"]
    });
    write_json(&dir.join("src/main.x07.json"), &program_doc);

    let project_doc = serde_json::json!({
        "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
        "compat": "0.5",
        "world": "run-os-sandboxed",
        "entry": "src/main.x07.json",
        "module_roots": ["src"],
        "dependencies": [],
        "lockfile": "x07.lock.json",
        "default_profile": "sandbox",
        "profiles": {
            "sandbox": {
                "world": "run-os-sandboxed",
                "policy": "policy/run-os.json"
            }
        }
    });
    let project_bytes = serde_json::to_vec(&project_doc).expect("serialize x07.json");
    write_json(&dir.join("x07.json"), &project_doc);
    write_lockfile_for_project_bytes(&dir, &project_bytes);

    let out = run_x07_in_dir(&dir, &["check", "--project", "x07.json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["schema_version"], X07DIAG_SCHEMA_VERSION);
    assert_eq!(report["ok"], true);
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
fn x07_init_verified_core_pure_template_creates_certifiable_project() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_init_verified_core_pure");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init", "--template", "verified-core-pure"]);
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
        vec!["Generated a certifiable solve-pure trust template."]
    );
    assert_eq!(
        v["next_steps"]
            .as_array()
            .expect("next_steps[]")
            .iter()
            .map(|v| v.as_str().expect("next_steps[] string"))
            .collect::<Vec<_>>(),
        vec![
            "x07 trust profile check --profile arch/trust/profiles/verified_core_pure_v1.json --project x07.json --entry example.main",
            "x07 test --all --manifest tests/tests.json",
            "x07 trust certify --project x07.json --profile arch/trust/profiles/verified_core_pure_v1.json --entry example.main --out-dir target/cert",
        ]
    );

    for rel in [
        "README.md",
        "x07.json",
        "x07.lock.json",
        "src/example.x07.json",
        "src/main.x07.json",
        "tests/tests.json",
        "tests/core.x07.json",
        "arch/manifest.x07arch.json",
        "arch/boundaries/index.x07boundary.json",
        "arch/trust/profiles/verified_core_pure_v1.json",
        "x07-toolchain.toml",
        "AGENT.md",
        ".agent/docs/index.md",
        ".agent/skills/README.md",
        ".gitignore",
    ] {
        assert!(dir.join(rel).is_file(), "missing {}", rel);
    }

    let proj_doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    assert_eq!(proj_doc["schema_version"], PROJECT_MANIFEST_SCHEMA_VERSION);
    assert_eq!(proj_doc["world"], "solve-pure");
    assert_eq!(proj_doc["entry"], "src/main.x07.json");

    let lock_doc: Value =
        serde_json::from_slice(&std::fs::read(dir.join("x07.lock.json")).unwrap())
            .expect("parse x07.lock.json");
    assert_eq!(lock_doc["schema_version"], PROJECT_LOCKFILE_SCHEMA_VERSION);

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--project",
            "x07.json",
            "--profile",
            "arch/trust/profiles/verified_core_pure_v1.json",
            "--entry",
            "example.main",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["profile"], "verified_core_pure_v1");
    assert_eq!(report["entry"], "example.main");

    let out = run_x07_in_dir(&dir, &["test", "--all", "--manifest", "tests/tests.json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["summary"]["failed"], 0);
    assert_eq!(report["summary"]["errors"], 0);
    assert_eq!(report["summary"]["passed"], 2);

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_init_trusted_sandbox_program_template_creates_capsule_backed_project() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_init_trusted_sandbox_program");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init", "--template", "trusted-sandbox-program"]);
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
        vec!["Generated a sandboxed trusted-program template with capsule evidence."]
    );
    assert_eq!(
        v["next_steps"]
            .as_array()
            .expect("next_steps[]")
            .iter()
            .map(|v| v.as_str().expect("next_steps[] string"))
            .collect::<Vec<_>>(),
        vec![
            "x07 trust profile check --project x07.json --profile arch/trust/profiles/trusted_program_sandboxed_local_v1.json --entry example.main",
            "x07 trust capsule check --project x07.json --index arch/capsules/index.x07capsule.json",
            "x07 test --all --manifest tests/tests.json",
            "x07 trust certify --project x07.json --profile arch/trust/profiles/trusted_program_sandboxed_local_v1.json --entry example.main --out-dir target/cert",
        ]
    );

    for rel in [
        "README.md",
        "x07.json",
        "x07.lock.json",
        "src/capsule.x07.json",
        "src/example.x07.json",
        "src/main.x07.json",
        "tests/tests.json",
        "tests/core.x07.json",
        "arch/manifest.x07arch.json",
        "arch/boundaries/index.x07boundary.json",
        "arch/capsules/index.x07capsule.json",
        "arch/capsules/capsule.main.contract.json",
        "arch/capsules/capsule.main.effect_log.json",
        "arch/capsules/capsule.main.conformance.json",
        "arch/capsules/capsule.main.attest.json",
        "arch/trust/profiles/trusted_program_sandboxed_local_v1.json",
        "policy/run-os.json",
        ".github/workflows/certify.yml",
        "x07-toolchain.toml",
        "AGENT.md",
        ".agent/docs/index.md",
        ".agent/skills/README.md",
        ".gitignore",
    ] {
        assert!(dir.join(rel).is_file(), "missing {}", rel);
    }

    let proj_doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    assert_eq!(proj_doc["schema_version"], PROJECT_MANIFEST_SCHEMA_VERSION);
    assert_eq!(proj_doc["world"], "run-os-sandboxed");
    assert_eq!(proj_doc["default_profile"], "sandbox");
    assert_eq!(proj_doc["entry"], "src/main.x07.json");

    let tests_doc: Value =
        serde_json::from_slice(&std::fs::read(dir.join("tests/tests.json")).unwrap())
            .expect("parse tests/tests.json");
    let tests = tests_doc["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 2);
    assert_eq!(tests[0]["sandbox_smoke"], true);
    assert_eq!(tests[0]["require_runtime_attestation"], true);
    assert_eq!(
        tests[0]["required_capsules"]
            .as_array()
            .expect("required_capsules[]"),
        &vec![Value::String("capsule.main_v1".to_string())]
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--project",
            "x07.json",
            "--profile",
            "arch/trust/profiles/trusted_program_sandboxed_local_v1.json",
            "--entry",
            "example.main",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["profile"], "trusted_program_sandboxed_local_v1");
    assert_eq!(report["entry"], "example.main");

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "capsule",
            "check",
            "--project",
            "x07.json",
            "--index",
            "arch/capsules/index.x07capsule.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["checked_capsules"], 1);

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_init_trusted_network_service_template_creates_network_capsule_project() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_init_trusted_network_service");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init", "--template", "trusted-network-service"]);
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
        vec!["Generated a sandboxed network-service template with peer-policy capsule evidence."]
    );
    assert_eq!(
        v["next_steps"]
            .as_array()
            .expect("next_steps[]")
            .iter()
            .map(|v| v.as_str().expect("next_steps[] string"))
            .collect::<Vec<_>>(),
        vec![
            "x07 trust profile check --project x07.json --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json --entry example.main",
            "x07 trust capsule check --project x07.json --index arch/capsules/index.x07capsule.json",
            "python3 tests/tcp_echo_server.py --host 127.0.0.1 --port 30030",
            "x07 test --all --manifest tests/tests.json",
            "x07 trust certify --project x07.json --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json --entry example.main --out-dir target/cert",
        ]
    );

    for rel in [
        "README.md",
        "x07.json",
        "x07.lock.json",
        "src/capsule.x07.json",
        "src/example.x07.json",
        "src/main.x07.json",
        "tests/tests.json",
        "tests/core.x07.json",
        "tests/policy/run-os.json",
        "tests/tcp_echo_server.py",
        "arch/manifest.x07arch.json",
        "arch/boundaries/index.x07boundary.json",
        "arch/capsules/index.x07capsule.json",
        "arch/capsules/capsule.main.contract.json",
        "arch/capsules/capsule.main.effect_log.json",
        "arch/capsules/capsule.main.conformance.json",
        "arch/capsules/capsule.main.attest.json",
        "arch/capsules/peers/loopback_tcp_v1.peer.json",
        "arch/trust/profiles/trusted_program_sandboxed_net_v1.json",
        "policy/run-os.json",
        ".github/workflows/certify.yml",
        "x07-toolchain.toml",
        "AGENT.md",
        ".agent/docs/index.md",
        ".agent/skills/README.md",
        ".gitignore",
    ] {
        assert!(dir.join(rel).is_file(), "missing {}", rel);
    }

    let proj_doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    assert_eq!(proj_doc["schema_version"], PROJECT_MANIFEST_SCHEMA_VERSION);
    assert_eq!(proj_doc["world"], "run-os-sandboxed");
    assert_eq!(proj_doc["default_profile"], "sandbox");
    assert_eq!(proj_doc["entry"], "src/main.x07.json");

    let tests_doc: Value =
        serde_json::from_slice(&std::fs::read(dir.join("tests/tests.json")).unwrap())
            .expect("parse tests/tests.json");
    let tests = tests_doc["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 2);
    assert_eq!(tests[0]["id"], "smoke/tcp_echo");
    assert_eq!(tests[0]["sandbox_smoke"], true);
    assert_eq!(tests[0]["require_runtime_attestation"], true);
    assert_eq!(
        tests[0]["required_capsules"]
            .as_array()
            .expect("required_capsules[]"),
        &vec![Value::String("capsule.main_v1".to_string())]
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--project",
            "x07.json",
            "--profile",
            "arch/trust/profiles/trusted_program_sandboxed_net_v1.json",
            "--entry",
            "example.main",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["profile"], "trusted_program_sandboxed_net_v1");
    assert_eq!(report["entry"], "example.main");

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "capsule",
            "check",
            "--project",
            "x07.json",
            "--index",
            "arch/capsules/index.x07capsule.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["checked_capsules"], 1);

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_init_certified_capsule_template_creates_attested_capsule_project() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_init_certified_capsule");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init", "--template", "certified-capsule"]);
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
        vec!["Generated a certified capsule template with attestation scaffolding."]
    );
    assert_eq!(
        v["next_steps"]
            .as_array()
            .expect("next_steps[]")
            .iter()
            .map(|v| v.as_str().expect("next_steps[] string"))
            .collect::<Vec<_>>(),
        vec![
            "x07 trust profile check --project x07.json --profile arch/trust/profiles/certified_capsule_v1.json --entry capsule.main",
            "x07 trust capsule check --project x07.json --index arch/capsules/index.x07capsule.json",
            "x07 test --all --manifest tests/tests.json",
            "x07 trust certify --project x07.json --profile arch/trust/profiles/certified_capsule_v1.json --entry capsule.main --out-dir target/cert",
        ]
    );

    for rel in [
        "README.md",
        "x07.json",
        "x07.lock.json",
        "src/capsule.x07.json",
        "src/main.x07.json",
        "tests/tests.json",
        "tests/core.x07.json",
        "arch/manifest.x07arch.json",
        "arch/boundaries/index.x07boundary.json",
        "arch/capsules/index.x07capsule.json",
        "arch/capsules/capsule.main.contract.json",
        "arch/capsules/capsule.main.effect_log.json",
        "arch/capsules/capsule.main.conformance.json",
        "arch/capsules/capsule.main.attest.json",
        "arch/trust/profiles/certified_capsule_v1.json",
        "policy/run-os.json",
        ".github/workflows/certify.yml",
        "x07-toolchain.toml",
        "AGENT.md",
        ".agent/docs/index.md",
        ".agent/skills/README.md",
        ".gitignore",
    ] {
        assert!(dir.join(rel).is_file(), "missing {}", rel);
    }

    let proj_doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    assert_eq!(proj_doc["schema_version"], PROJECT_MANIFEST_SCHEMA_VERSION);
    assert_eq!(proj_doc["world"], "run-os-sandboxed");
    assert_eq!(proj_doc["default_profile"], "sandbox");
    assert_eq!(proj_doc["entry"], "src/main.x07.json");

    let tests_doc: Value =
        serde_json::from_slice(&std::fs::read(dir.join("tests/tests.json")).unwrap())
            .expect("parse tests/tests.json");
    let tests = tests_doc["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0]["sandbox_smoke"], true);
    assert_eq!(tests[0]["require_runtime_attestation"], true);
    assert_eq!(
        tests[0]["required_capsules"]
            .as_array()
            .expect("required_capsules[]"),
        &vec![Value::String("capsule.main_v1".to_string())]
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--project",
            "x07.json",
            "--profile",
            "arch/trust/profiles/certified_capsule_v1.json",
            "--entry",
            "capsule.main",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["profile"], "certified_capsule_v1");
    assert_eq!(report["entry"], "capsule.main");

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "capsule",
            "check",
            "--project",
            "x07.json",
            "--index",
            "arch/capsules/index.x07capsule.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["checked_capsules"], 1);

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
}

#[test]
fn x07_init_certified_network_capsule_template_creates_network_attested_capsule_project() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_init_certified_network_capsule");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init", "--template", "certified-network-capsule"]);
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
        vec!["Generated a certified network capsule template with peer-policy attestation scaffolding."]
    );
    assert_eq!(
        v["next_steps"]
            .as_array()
            .expect("next_steps[]")
            .iter()
            .map(|v| v.as_str().expect("next_steps[] string"))
            .collect::<Vec<_>>(),
        vec![
            "x07 trust profile check --project x07.json --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json --entry capsule.main",
            "x07 trust capsule check --project x07.json --index arch/capsules/index.x07capsule.json",
            "python3 tests/tcp_echo_server.py --host 127.0.0.1 --port 30030",
            "x07 test --all --manifest tests/tests.json",
            "x07 trust certify --project x07.json --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json --entry capsule.main --out-dir target/cert",
        ]
    );

    for rel in [
        "README.md",
        "x07.json",
        "x07.lock.json",
        "src/capsule.x07.json",
        "src/main.x07.json",
        "tests/tests.json",
        "tests/core.x07.json",
        "tests/policy/run-os.json",
        "tests/tcp_echo_server.py",
        "arch/manifest.x07arch.json",
        "arch/boundaries/index.x07boundary.json",
        "arch/capsules/index.x07capsule.json",
        "arch/capsules/capsule.main.contract.json",
        "arch/capsules/capsule.main.effect_log.json",
        "arch/capsules/capsule.main.conformance.json",
        "arch/capsules/capsule.main.attest.json",
        "arch/capsules/peers/loopback_tcp_v1.peer.json",
        "arch/trust/profiles/trusted_program_sandboxed_net_v1.json",
        "policy/run-os.json",
        ".github/workflows/certify.yml",
        "x07-toolchain.toml",
        "AGENT.md",
        ".agent/docs/index.md",
        ".agent/skills/README.md",
        ".gitignore",
    ] {
        assert!(dir.join(rel).is_file(), "missing {}", rel);
    }

    let proj_doc: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    assert_eq!(proj_doc["schema_version"], PROJECT_MANIFEST_SCHEMA_VERSION);
    assert_eq!(proj_doc["world"], "run-os-sandboxed");
    assert_eq!(proj_doc["default_profile"], "sandbox");
    assert_eq!(proj_doc["entry"], "src/main.x07.json");

    let tests_doc: Value =
        serde_json::from_slice(&std::fs::read(dir.join("tests/tests.json")).unwrap())
            .expect("parse tests/tests.json");
    let tests = tests_doc["tests"].as_array().expect("tests[]");
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0]["id"], "smoke/capsule_tcp_echo");
    assert_eq!(tests[0]["sandbox_smoke"], true);
    assert_eq!(tests[0]["require_runtime_attestation"], true);
    assert_eq!(
        tests[0]["required_capsules"]
            .as_array()
            .expect("required_capsules[]"),
        &vec![Value::String("capsule.main_v1".to_string())]
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--project",
            "x07.json",
            "--profile",
            "arch/trust/profiles/trusted_program_sandboxed_net_v1.json",
            "--entry",
            "capsule.main",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["profile"], "trusted_program_sandboxed_net_v1");
    assert_eq!(report["entry"], "capsule.main");

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "capsule",
            "check",
            "--project",
            "x07.json",
            "--index",
            "arch/capsules/index.x07capsule.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["checked_capsules"], 1);

    std::fs::remove_dir_all(&dir).expect("cleanup tmp dir");
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
            "Edit x07-package.json: set description/docs/license; verify meta.x07c_compat; bump version",
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
    assert_eq!(pkg_doc["license"], "MIT OR Apache-2.0");
    assert!(
        pkg_doc["meta"]["x07c_compat"]
            .as_str()
            .unwrap_or("")
            .contains(x07c::X07C_VERSION),
        "expected meta.x07c_compat to mention x07c version {}",
        x07c::X07C_VERSION
    );

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
fn x07_pkg_verify_accepts_signed_file_index_fixture() {
    let root = repo_root();
    let work = root.join("docs/examples/packaging-integrity");
    let index_dir = work.join("signed-index");
    assert!(index_dir.is_dir());

    let index_url = format!("sparse+{}", file_url_for_dir(&index_dir));
    let argv: Vec<String> = vec![
        "pkg".to_string(),
        "verify".to_string(),
        "integrity-demo@0.1.0".to_string(),
        "--index".to_string(),
        index_url,
        "--offline".to_string(),
    ];
    let args: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
    let out = run_x07_in_dir(&work, &args);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["command"], "pkg.verify");
    assert_eq!(report["result"]["name"], "integrity-demo");
    assert_eq!(report["result"]["version"], "0.1.0");
    assert_eq!(report["result"]["signature"]["ok"], true);
}

#[test]
fn x07_pkg_check_semver_accepts_compatible_change_fixture() {
    let root = repo_root();
    let work = root.join("docs/examples/packaging-integrity/semver");
    let old_dir = work.join("old");
    let new_dir = work.join("new-compatible");
    assert!(old_dir.is_dir());
    assert!(new_dir.is_dir());

    let out = run_x07(&[
        "pkg",
        "check-semver",
        "--old",
        old_dir.to_str().unwrap(),
        "--new",
        new_dir.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["command"], "pkg.check-semver");
    let breaks = report["result"]["breaking_changes"]
        .as_array()
        .map(|v| v.len())
        .unwrap_or(0);
    assert_eq!(breaks, 0);
}

#[test]
fn x07_pkg_check_semver_detects_breaking_change_fixture() {
    let root = repo_root();
    let work = root.join("docs/examples/packaging-integrity/semver");
    let old_dir = work.join("old");
    let new_dir = work.join("new-breaking");
    assert!(old_dir.is_dir());
    assert!(new_dir.is_dir());

    let out = run_x07(&[
        "pkg",
        "check-semver",
        "--old",
        old_dir.to_str().unwrap(),
        "--new",
        new_dir.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], false);
    assert_eq!(report["command"], "pkg.check-semver");
    assert!(
        report["result"]["breaking_changes"]
            .as_array()
            .is_some_and(|v| !v.is_empty()),
        "expected breaking_changes to be non-empty"
    );
    assert_eq!(report["error"]["code"], "X07PKG_SEMVER_BREAKING");
}

#[test]
fn x07_info_offline_reads_local_dep_manifest() {
    let root = repo_root();
    let parent = fresh_tmp_dir(&root, "tmp_x07_info_offline");
    if parent.exists() {
        std::fs::remove_dir_all(&parent).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&parent).expect("create tmp dir");

    let work = parent.join("work");
    std::fs::create_dir_all(&work).expect("create work dir");

    let demo_pkg = root.join("docs/examples/packaging-integrity/pkg/integrity-demo");
    assert!(demo_pkg.is_dir());
    let dep_dir = work.join(".x07/deps/integrity-demo/0.1.0");
    std::fs::create_dir_all(dep_dir.parent().unwrap()).expect("create deps parent dir");
    copy_dir_recursive(&demo_pkg, &dep_dir);
    assert!(dep_dir.join("x07-package.json").is_file());

    let index_dir = root.join("docs/examples/packaging-integrity/signed-index");
    assert!(index_dir.is_dir());
    let index_url = format!("sparse+{}", file_url_for_dir(&index_dir));

    let out = run_x07_in_dir(
        &work,
        &[
            "info",
            "integrity-demo@0.1.0",
            "--index",
            &index_url,
            "--offline",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["command"], "pkg.info");
    assert_eq!(report["result"]["name"], "integrity-demo");
    assert_eq!(report["result"]["version"], "0.1.0");
    assert!(report["result"]["package_manifest"].is_object());

    std::fs::remove_dir_all(&parent).expect("cleanup tmp dir");
}

#[test]
fn x07_pkg_pack_requires_publish_metadata() {
    let root = repo_root();
    let parent = fresh_tmp_dir(&root, "tmp_x07_pkg_pack_metadata");
    if parent.exists() {
        std::fs::remove_dir_all(&parent).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&parent).expect("create tmp dir");

    let dir = parent.join("demo-pkg");
    std::fs::create_dir_all(&dir).expect("create package dir");
    std::fs::create_dir_all(dir.join("modules/demo")).expect("create module dir");
    let module_bytes = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "demo.api",
        "imports": [],
        "decls": []
    }))
    .expect("serialize module");
    write_bytes(&dir.join("modules/demo/api.x07.json"), &module_bytes);
    let manifest_bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": PACKAGE_MANIFEST_SCHEMA_VERSION,
        "name": "demo-pkg",
        "version": "0.1.0",
        "description": "demo package",
        "docs": "demo docs",
        "module_root": "modules",
        "modules": ["demo.api"],
        "meta": {
            "x07c_compat": format!(">={}, <0.3.0", x07c::X07C_VERSION)
        }
    }))
    .expect("serialize x07-package.json");
    write_bytes(&dir.join("x07-package.json"), &manifest_bytes);

    let out = run_x07_in_dir(
        &dir,
        &[
            "pkg",
            "pack",
            "--package",
            ".",
            "--out",
            "dist/demo-pkg-0.1.0.x07pkg",
        ],
    );
    assert_eq!(out.status.code(), Some(20));
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], false);
    assert_eq!(report["command"], "pkg.pack");
    assert_eq!(report["error"]["code"], "X07PKG_PACK_FAILED");
    let msg = report["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("license"),
        "expected error to mention license, got: {msg}"
    );

    std::fs::remove_dir_all(&parent).expect("cleanup tmp dir");
}

#[test]
fn x07_pkg_publish_reports_pack_failures_as_json() {
    let root = repo_root();
    let parent = fresh_tmp_dir(&root, "tmp_x07_pkg_publish_pack_fail");
    if parent.exists() {
        std::fs::remove_dir_all(&parent).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&parent).expect("create tmp dir");

    let dir = parent.join("demo-pkg");
    std::fs::create_dir_all(&dir).expect("create package dir");
    std::fs::create_dir_all(dir.join("modules/demo")).expect("create module dir");
    let module_bytes = serde_json::to_vec(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "demo.api",
        "imports": [],
        "decls": []
    }))
    .expect("serialize module");
    write_bytes(&dir.join("modules/demo/api.x07.json"), &module_bytes);
    let manifest_bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": PACKAGE_MANIFEST_SCHEMA_VERSION,
        "name": "demo-pkg",
        "version": "0.1.0",
        "description": "demo package",
        "docs": "demo docs",
        "module_root": "modules",
        "modules": ["demo.api"],
        "meta": {
            "x07c_compat": format!(">={}, <0.3.0", x07c::X07C_VERSION)
        }
    }))
    .expect("serialize x07-package.json");
    write_bytes(&dir.join("x07-package.json"), &manifest_bytes);

    let out = run_x07_in_dir(
        &dir,
        &[
            "pkg",
            "publish",
            "--index",
            "sparse+https://registry.x07.io/index/",
            "--package",
            ".",
        ],
    );
    assert_eq!(out.status.code(), Some(20));
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], false);
    assert_eq!(report["command"], "pkg.publish");
    assert_eq!(report["error"]["code"], "X07PKG_PUBLISH_PACK");
    let msg = report["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("license"),
        "expected error to mention license, got: {msg}"
    );

    std::fs::remove_dir_all(&parent).expect("cleanup tmp dir");
}

#[test]
fn x07_trust_capsule_check_accepts_relocated_network_examples() {
    let root = repo_root();
    let parent = fresh_tmp_dir(&root, "tmp_x07_trust_capsule_network_examples");
    if parent.exists() {
        std::fs::remove_dir_all(&parent).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&parent).expect("create tmp dir");

    for example in [
        "docs/examples/trusted_network_service_v1",
        "docs/examples/certified_network_capsule_v1",
    ] {
        let src = root.join(example);
        let dir = parent.join(src.file_name().expect("example dir name"));
        copy_dir_recursive(&src, &dir);
        let target_dir = dir.join("target");
        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir).expect("remove copied target dir");
        }

        let out = run_x07_in_dir(&dir, &["pkg", "lock", "--project", "x07.json"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "example={example}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );

        let out = run_x07_in_dir(
            &dir,
            &[
                "trust",
                "capsule",
                "check",
                "--project",
                "x07.json",
                "--index",
                "arch/capsules/index.x07capsule.json",
            ],
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "example={example}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let report = parse_json_stdout(&out);
        assert_eq!(report["ok"], true, "example={example}");
        assert_eq!(report["checked_capsules"], 1, "example={example}");
    }

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
fn x07_init_service_template_writes_locked_dependencies() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_init_service_template_locked");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init", "--template", "api-cell"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["command"], "init");

    let proj: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.json")).unwrap())
        .expect("parse x07.json");
    let deps = proj["dependencies"].as_array().expect("dependencies[]");
    assert!(
        !deps.is_empty(),
        "expected service template dependencies in x07.json"
    );
    assert!(
        deps.iter()
            .all(|dep| dep["path"].as_str().is_some_and(|s| !s.is_empty())),
        "expected every dependency to have a resolved path after init:\n{}",
        serde_json::to_string_pretty(&proj).unwrap()
    );

    let lock: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.lock.json")).unwrap())
        .expect("parse x07.lock.json");
    let lock_deps = lock["dependencies"]
        .as_array()
        .expect("lock.dependencies[]");
    assert!(
        !lock_deps.is_empty(),
        "expected non-empty lockfile after service template init:\n{}",
        serde_json::to_string_pretty(&lock).unwrap()
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
fn x07_pkg_lock_patch_overrides_transitive_requires_packages() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_lock_patch_override_transitive");
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
    write_minimal_pkg_manifest(&deps_root.join("a/1.0.0"), "a", "1.0.0", &["b@1.0.0"]);
    write_minimal_pkg_manifest(&deps_root.join("b/1.0.0"), "b", "1.0.0", &["c@1.0.0"]);
    write_minimal_pkg_manifest(&deps_root.join("c/1.0.0"), "c", "1.0.0", &[]);
    write_minimal_pkg_manifest(&deps_root.join("c/1.0.1"), "c", "1.0.1", &[]);

    let proj_path = dir.join("x07.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
        .expect("parse x07.json");
    let obj = doc.as_object_mut().expect("x07.json must be object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![
            serde_json::json!({"name":"a","version":"1.0.0","path":".x07/deps/a/1.0.0"}),
        ]),
    );
    obj.insert(
        "patch".to_string(),
        serde_json::json!({
            "c": { "version": "1.0.1" }
        }),
    );
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );

    let index_dir = dir.join("fake_index");
    std::fs::create_dir_all(&index_dir).expect("create fake index dir");
    let index_url = write_fake_file_index_config(&index_dir);

    write_index_entries_ndjson(
        &index_dir,
        "a",
        &[
            serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":"a","version":"1.0.0","cksum":"aa","yanked":false}),
        ],
    );
    write_index_entries_ndjson(
        &index_dir,
        "b",
        &[
            serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":"b","version":"1.0.0","cksum":"bb","yanked":false}),
        ],
    );
    write_index_entries_ndjson(
        &index_dir,
        "c",
        &[
            serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":"c","version":"1.0.0","cksum":"c0","yanked":true}),
            serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":"c","version":"1.0.1","cksum":"c1","yanked":false}),
        ],
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--index", index_url.as_str()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["command"], "pkg.lock");

    let updated: Value =
        serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json after lock"))
            .expect("parse x07.json after lock");
    let deps = updated["dependencies"].as_array().expect("dependencies[]");
    assert!(
        deps.iter()
            .any(|d| d["name"] == "c" && d["version"] == "1.0.1"),
        "expected patched c@1.0.1 in dependencies[]; got:\n{}",
        serde_json::to_string_pretty(&updated).unwrap()
    );
    assert!(
        !deps
            .iter()
            .any(|d| d["name"] == "c" && d["version"] == "1.0.0"),
        "expected c@1.0.0 to be replaced by patch"
    );

    let lock: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.lock.json")).unwrap())
        .expect("parse x07.lock.json");
    let lock_deps = lock["dependencies"]
        .as_array()
        .expect("lock.dependencies[]");
    let c_dep = lock_deps
        .iter()
        .find(|d| d["name"] == "c")
        .expect("c dep in lockfile");
    assert_eq!(c_dep["version"], "1.0.1");
    assert_eq!(c_dep["overridden_by"], "c");
    assert_eq!(c_dep["yanked"], false);
}

#[test]
fn x07_pkg_lock_normalizes_missing_dependency_paths() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_lock_normalize_missing_paths");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let deps_root = dir.join(".x07").join("deps");
    write_minimal_pkg_manifest(&deps_root.join("demo/1.0.0"), "demo", "1.0.0", &[]);

    let proj_path = dir.join("x07.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
        .expect("parse x07.json");
    let obj = doc.as_object_mut().expect("x07.json must be object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![serde_json::json!({
            "name": "demo",
            "version": "1.0.0"
        })]),
    );
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--offline"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let updated: Value =
        serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json after lock"))
            .expect("parse x07.json after lock");
    let deps = updated["dependencies"].as_array().expect("dependencies[]");
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0]["name"], "demo");
    assert_eq!(deps[0]["version"], "1.0.0");
    assert_eq!(deps[0]["path"], ".x07/deps/demo/1.0.0");

    let lock: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.lock.json")).unwrap())
        .expect("parse x07.lock.json");
    let lock_deps = lock["dependencies"]
        .as_array()
        .expect("lock.dependencies[]");
    assert_eq!(lock_deps.len(), 1);
    assert_eq!(lock_deps[0]["name"], "demo");
    assert_eq!(lock_deps[0]["path"], ".x07/deps/demo/1.0.0");
}

#[test]
fn x07_pkg_lock_check_hydrates_patched_vendored_deps() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_lock_check_hydrate_patched_vendored");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let deps_root = dir.join(".x07").join("deps");
    write_minimal_pkg_manifest(&deps_root.join("a/1.0.0"), "a", "1.0.0", &["b@1.0.0"]);
    write_minimal_pkg_manifest(&deps_root.join("b/1.0.1"), "b", "1.0.1", &[]);

    let proj_path = dir.join("x07.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
        .expect("parse x07.json");
    let obj = doc.as_object_mut().expect("x07.json must be object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![
            serde_json::json!({"name":"a","version":"1.0.0","path":".x07/deps/a/1.0.0"}),
            serde_json::json!({"name":"b","version":"1.0.1","path":".x07/deps/b/1.0.1"}),
        ]),
    );
    obj.insert(
        "patch".to_string(),
        serde_json::json!({
            "b": { "version": "1.0.1", "path": ".x07/deps/b/1.0.1" }
        }),
    );
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--offline"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    std::fs::remove_dir_all(deps_root.join("b")).expect("remove vendored b dep dir");
    assert!(
        !deps_root.join("b/1.0.1/x07-package.json").is_file(),
        "expected b dep to be missing before hydration"
    );

    let index_dir = dir.join("fake_index");
    std::fs::create_dir_all(&index_dir).expect("create fake index dir");
    let index_url = write_fake_file_index_config(&index_dir);

    let b_pkg = serde_json::json!({
        "schema_version": PACKAGE_MANIFEST_SCHEMA_VERSION,
        "name": "b",
        "version": "1.0.1",
        "module_root": "modules",
        "modules": [],
    });
    let b_pkg_bytes = serde_json::to_vec_pretty(&b_pkg).expect("encode package manifest");
    let b_archive = x07_pkg::build_tar_bytes(&[(PathBuf::from("x07-package.json"), b_pkg_bytes)])
        .expect("build tar");
    let b_cksum = sha256_hex(&b_archive);

    write_bytes(&index_dir.join("dl/b/1.0.1/download"), &b_archive);
    write_index_entries_ndjson(
        &index_dir,
        "a",
        &[
            serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":"a","version":"1.0.0","cksum":"aa","yanked":false}),
        ],
    );
    write_index_entries_ndjson(
        &index_dir,
        "b",
        &[
            serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":"b","version":"1.0.1","cksum":b_cksum,"yanked":false}),
        ],
    );

    let out = run_x07_in_dir(
        &dir,
        &["pkg", "lock", "--check", "--index", index_url.as_str()],
    );
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
        deps_root.join("b/1.0.1/x07-package.json").is_file(),
        "expected b dep to be hydrated under .x07/deps"
    );
}

#[test]
fn x07_run_hydrates_missing_vendored_deps() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_run_hydrate_vendored_dep");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    std::fs::create_dir_all(dir.join("src")).expect("create src dir");

    let dep_name = "demo-dep";
    let dep_version = "1.0.0";
    let dep_rel = format!(".x07/deps/{dep_name}/{dep_version}");
    let dep_dir = dir.join(&dep_rel);
    std::fs::create_dir_all(dep_dir.join("modules/demo")).expect("create dep modules");
    write_json(
        &dep_dir.join("x07-package.json"),
        &serde_json::json!({
            "schema_version": PACKAGE_MANIFEST_SCHEMA_VERSION,
            "name": dep_name,
            "version": dep_version,
            "module_root": "modules",
            "modules": ["demo.main"]
        }),
    );
    write_json(
        &dep_dir.join("modules/demo/main.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "demo.main",
            "imports": [],
            "decls": [
                {
                    "kind": "export",
                    "names": ["demo.main.answer_v1"]
                },
                {
                    "kind": "defn",
                    "name": "demo.main.answer_v1",
                    "params": [{ "name": "b", "ty": "bytes_view" }],
                    "result": "bytes",
                    "body": ["view.to_bytes", "b"]
                }
            ]
        }),
    );

    write_json(
        &dir.join("x07.json"),
        &serde_json::json!({
            "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
            "world": "solve-pure",
            "entry": "src/main.x07.json",
            "module_roots": ["src"],
            "dependencies": [
                { "name": dep_name, "version": dep_version, "path": dep_rel }
            ],
            "lockfile": "x07.lock.json"
        }),
    );
    write_json(
        &dir.join("src/main.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": ["demo.main"],
            "decls": [],
            "solve": ["demo.main.answer_v1", "input"]
        }),
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--offline"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    std::fs::remove_dir_all(dir.join(".x07/deps").join(dep_name)).expect("remove vendored dep dir");
    assert!(
        !dep_dir.join("x07-package.json").is_file(),
        "expected dep to be missing before hydration"
    );

    let index_dir = dir.join("fake_index");
    std::fs::create_dir_all(&index_dir).expect("create fake index dir");
    let index_url = write_fake_file_index_config(&index_dir);

    let pkg_manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": PACKAGE_MANIFEST_SCHEMA_VERSION,
        "name": dep_name,
        "version": dep_version,
        "module_root": "modules",
        "modules": ["demo.main"]
    }))
    .expect("encode package manifest");
    let module_bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "demo.main",
        "imports": [],
        "decls": [
            {
                "kind": "export",
                "names": ["demo.main.answer_v1"]
            },
            {
                "kind": "defn",
                "name": "demo.main.answer_v1",
                "params": [{ "name": "b", "ty": "bytes_view" }],
                "result": "bytes",
                "body": ["view.to_bytes", "b"]
            }
        ]
    }))
    .expect("encode module");
    let archive = x07_pkg::build_tar_bytes(&[
        (PathBuf::from("x07-package.json"), pkg_manifest),
        (PathBuf::from("modules/demo/main.x07.json"), module_bytes),
    ])
    .expect("build package archive");
    let archive_sha = sha256_hex(&archive);
    write_bytes(
        &index_dir.join(format!("dl/{dep_name}/{dep_version}/download")),
        &archive,
    );
    write_index_entries_ndjson(
        &index_dir,
        dep_name,
        &[serde_json::json!({
            "schema_version": "x07.index-entry@0.1.0",
            "name": dep_name,
            "version": dep_version,
            "cksum": archive_sha,
            "yanked": false
        })],
    );

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&dir)
        .env(ENV_SANDBOX_BACKEND, "os")
        .env(ENV_ACCEPT_WEAKER_ISOLATION, "1")
        .env("X07_PKG_INDEX_URL", index_url.as_str())
        .args([
            "run",
            "--project",
            ".",
            "--report",
            "wrapped",
            "--cpu-time-limit-seconds",
            "30",
        ])
        .output()
        .expect("run x07");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["schema_version"], X07_RUN_REPORT_SCHEMA_VERSION);
    assert!(
        dep_dir.join("x07-package.json").is_file(),
        "expected dep to be hydrated under .x07/deps"
    );
}

#[test]
fn x07_pkg_lock_check_fails_on_yanked_dep() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_lock_check_yanked");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let deps_root = dir.join(".x07").join("deps");
    write_minimal_pkg_manifest(&deps_root.join("yankme/1.0.0"), "yankme", "1.0.0", &[]);

    let proj_path = dir.join("x07.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
        .expect("parse x07.json");
    let obj = doc.as_object_mut().expect("x07.json must be object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![
            serde_json::json!({"name":"yankme","version":"1.0.0","path":".x07/deps/yankme/1.0.0"}),
        ]),
    );
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );

    let index_dir = dir.join("fake_index");
    std::fs::create_dir_all(&index_dir).expect("create fake index dir");
    let index_url = write_fake_file_index_config(&index_dir);
    write_index_entries_ndjson(
        &index_dir,
        "yankme",
        &[
            serde_json::json!({"schema_version":"x07.index-entry@0.1.0","name":"yankme","version":"1.0.0","cksum":"00","yanked":true}),
        ],
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--index", index_url.as_str()]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(
        &dir,
        &["pkg", "lock", "--check", "--index", index_url.as_str()],
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
    assert_eq!(v["error"]["code"], "X07PKG_YANKED_DEP");

    let out = run_x07_in_dir(
        &dir,
        &[
            "pkg",
            "lock",
            "--check",
            "--allow-yanked",
            "--index",
            index_url.as_str(),
        ],
    );
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
fn x07_pkg_lock_check_fails_on_advisories() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_lock_check_advisories");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let deps_root = dir.join(".x07").join("deps");
    write_minimal_pkg_manifest(&deps_root.join("advised/1.0.0"), "advised", "1.0.0", &[]);

    let proj_path = dir.join("x07.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
        .expect("parse x07.json");
    let obj = doc.as_object_mut().expect("x07.json must be object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![serde_json::json!({"name":"advised","version":"1.0.0","path":".x07/deps/advised/1.0.0"})]),
    );
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );

    let index_dir = dir.join("fake_index");
    std::fs::create_dir_all(&index_dir).expect("create fake index dir");
    let index_url = write_fake_file_index_config(&index_dir);
    let advisory = serde_json::json!({
        "schema_version": X07_PKG_ADVISORY_SCHEMA_VERSION,
        "id": "00000000-0000-0000-0000-000000000001",
        "package": "advised",
        "version": "1.0.0",
        "kind": "broken",
        "severity": "high",
        "summary": "bad release",
        "created_at_utc": "2026-01-01T00:00:00Z",
    });
    write_index_entries_ndjson(
        &index_dir,
        "advised",
        &[serde_json::json!({
            "schema_version":"x07.index-entry@0.1.0",
            "name":"advised",
            "version":"1.0.0",
            "cksum":"00",
            "yanked":false,
            "advisories":[advisory]
        })],
    );

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--index", index_url.as_str()]);
    assert_eq!(out.status.code(), Some(0));

    let out = run_x07_in_dir(
        &dir,
        &["pkg", "lock", "--check", "--index", index_url.as_str()],
    );
    assert_eq!(out.status.code(), Some(20));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "X07PKG_ADVISED_DEP");

    let out = run_x07_in_dir(
        &dir,
        &[
            "pkg",
            "lock",
            "--check",
            "--allow-advisories",
            "--index",
            index_url.as_str(),
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], true);
}

#[test]
fn x07_pkg_tree_outputs_deterministic_graph() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_tree_deterministic");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let deps_root = dir.join(".x07").join("deps");
    write_minimal_pkg_manifest(&deps_root.join("a/1.0.0"), "a", "1.0.0", &["b@1.0.0"]);
    write_minimal_pkg_manifest(&deps_root.join("b/1.0.0"), "b", "1.0.0", &["c@1.0.0"]);
    write_minimal_pkg_manifest(&deps_root.join("c/1.0.0"), "c", "1.0.0", &[]);

    let proj_path = dir.join("x07.json");
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": PROJECT_MANIFEST_SCHEMA_VERSION,
            "world": "solve-pure",
            "entry": "src/main.x07.json",
            "module_roots": ["src"],
            "dependencies": [
                {"name":"a","version":"1.0.0","path":".x07/deps/a/1.0.0"},
                {"name":"b","version":"1.0.0","path":".x07/deps/b/1.0.0"},
                {"name":"c","version":"1.0.0","path":".x07/deps/c/1.0.0"}
            ]
        }))
        .expect("serialize x07.json")
        .as_slice(),
    );

    let manifest = project::load_project_manifest(&proj_path).expect("load project manifest");
    let lock = project::compute_lockfile(&proj_path, &manifest).expect("compute lockfile");
    let lock_path = project::default_lockfile_path(&proj_path, &manifest);
    write_bytes(
        &lock_path,
        serde_json::to_vec_pretty(&lock)
            .expect("serialize x07.lock.json")
            .as_slice(),
    );

    let out1 = run_x07_in_dir(&dir, &["pkg", "tree", "--project", "x07.json"]);
    assert_eq!(
        out1.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out1.stderr),
        String::from_utf8_lossy(&out1.stdout)
    );
    let report = parse_json_stdout(&out1);
    assert_eq!(report["schema_version"], "x07.pkg.tree.report@0.1.0");
    assert_eq!(report["ok"], true);

    let nodes = report["nodes"].as_array().expect("nodes[]");
    let project_node = nodes
        .iter()
        .find(|n| n["kind"] == "project")
        .expect("project node");
    let declared = project_node["declared_module_roots"]
        .as_array()
        .expect("declared_module_roots[]");
    assert!(!declared.is_empty(), "expected declared module roots");
    let resolved = project_node["resolved_module_roots"]
        .as_array()
        .expect("resolved_module_roots[]");
    assert!(!resolved.is_empty(), "expected resolved module roots");
    let expected_src = dir.join("src").display().to_string();
    assert!(
        resolved
            .iter()
            .any(|r| r.as_str() == Some(expected_src.as_str())),
        "expected resolved_module_roots to contain {expected_src}, got:\n{}",
        serde_json::to_string_pretty(project_node).unwrap()
    );

    let edges = report["edges"].as_array().expect("edges[]");
    assert!(
        edges
            .iter()
            .any(|e| { e["kind"] == "requires" && e["from"] == "a@1.0.0" && e["to"] == "b@1.0.0" }),
        "missing requires edge a -> b"
    );
    assert!(
        edges
            .iter()
            .any(|e| { e["kind"] == "requires" && e["from"] == "b@1.0.0" && e["to"] == "c@1.0.0" }),
        "missing requires edge b -> c"
    );
    assert!(
        edges
            .iter()
            .any(|e| e["kind"] == "root" && e["from"] == "project" && e["to"] == "a@1.0.0"),
        "missing root edge project -> a"
    );

    let out2 = run_x07_in_dir(&dir, &["pkg", "tree", "--project", "x07.json"]);
    assert_eq!(
        out2.status.code(),
        Some(0),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out2.stderr),
        String::from_utf8_lossy(&out2.stdout)
    );
    assert_eq!(
        out1.stdout, out2.stdout,
        "pkg tree output changed between identical runs"
    );
}

#[test]
fn x07_pkg_lock_offline_patch_requires_dep_present() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_pkg_lock_offline_patch_present");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let out = run_x07_in_dir(&dir, &["init"]);
    assert_eq!(out.status.code(), Some(0));

    let deps_root = dir.join(".x07").join("deps");
    write_minimal_pkg_manifest(&deps_root.join("a/1.0.0"), "a", "1.0.0", &["b@1.0.0"]);
    write_minimal_pkg_manifest(&deps_root.join("b/1.0.0"), "b", "1.0.0", &["c@1.0.0"]);
    write_minimal_pkg_manifest(&deps_root.join("c/1.0.0"), "c", "1.0.0", &[]);

    let proj_path = dir.join("x07.json");
    let mut doc: Value = serde_json::from_slice(&std::fs::read(&proj_path).expect("read x07.json"))
        .expect("parse x07.json");
    let obj = doc.as_object_mut().expect("x07.json must be object");
    obj.insert(
        "dependencies".to_string(),
        Value::Array(vec![
            serde_json::json!({"name":"a","version":"1.0.0","path":".x07/deps/a/1.0.0"}),
        ]),
    );
    obj.insert(
        "patch".to_string(),
        serde_json::json!({
            "c": { "version": "1.0.1" }
        }),
    );
    write_bytes(
        &proj_path,
        serde_json::to_vec_pretty(&doc).unwrap().as_slice(),
    );

    let before = std::fs::read(&proj_path).expect("read x07.json before lock");

    let out = run_x07_in_dir(&dir, &["pkg", "lock", "--offline"]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}\nstdout:\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let v = parse_json_stdout(&out);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "X07PKG_OFFLINE_MISSING_DEP");
    let msg = v["error"]["message"]
        .as_str()
        .unwrap_or("<missing error.message>");
    assert!(
        msg.contains("missing:"),
        "expected missing list in error.message, got:\n{msg}"
    );
    assert!(
        msg.contains("c@1.0.1"),
        "expected missing spec in error.message, got:\n{msg}"
    );
    assert!(
        msg.contains(".x07/deps/c/1.0.1"),
        "expected missing path in error.message, got:\n{msg}"
    );
    assert!(
        msg.contains("x07 pkg lock --project"),
        "expected next-step hint in error.message, got:\n{msg}"
    );
    assert!(
        msg.contains("--offline"),
        "expected offline hint in error.message, got:\n{msg}"
    );
    assert!(
        msg.contains("X07_OFFLINE=1"),
        "expected env offline hint in error.message, got:\n{msg}"
    );

    let after = std::fs::read(&proj_path).expect("read x07.json after failed lock");
    assert_eq!(
        after, before,
        "x07.json changed despite offline missing dep"
    );

    write_minimal_pkg_manifest(&deps_root.join("c/1.0.1"), "c", "1.0.1", &[]);

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

    let lock: Value = serde_json::from_slice(&std::fs::read(dir.join("x07.lock.json")).unwrap())
        .expect("parse x07.lock.json");
    let lock_deps = lock["dependencies"]
        .as_array()
        .expect("lock.dependencies[]");
    let c_dep = lock_deps
        .iter()
        .find(|d| d["name"] == "c")
        .expect("c dep in lockfile");
    assert_eq!(c_dep["version"], "1.0.1");
    assert_eq!(c_dep["overridden_by"], "c");
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
fn x07_ast_edit_insert_stmts_wraps_non_begin_body() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_edit_insert_wrap");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "body": 0
            }
        ]
    });
    let program_path = dir.join("main.x07.json");
    write_bytes(
        &program_path,
        serde_json::to_vec(&program_doc)
            .expect("serialize x07AST")
            .as_slice(),
    );

    let stmt_path = dir.join("stmt1.json");
    write_bytes(
        &stmt_path,
        serde_json::to_vec(&serde_json::json!(["let", "x", 1]))
            .expect("serialize stmt")
            .as_slice(),
    );

    let out = run_x07(&[
        "ast",
        "edit",
        "insert-stmts",
        "--in",
        program_path.to_str().unwrap(),
        "--defn",
        "main.f",
        "--stmt-file",
        stmt_path.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["inserted"], 1);

    let edited_bytes = std::fs::read(&program_path).expect("read edited program");
    let edited_doc: Value = serde_json::from_slice(&edited_bytes).expect("parse edited x07AST");
    assert_eq!(
        edited_doc.pointer("/decls/0/body").expect("body"),
        &serde_json::json!(["begin", ["let", "x", 1], 0])
    );
}

#[test]
fn x07_ast_edit_insert_stmts_inserts_before_begin_tail() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_edit_insert_before_tail");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "body": ["begin", ["let", "y", 2], 0]
            }
        ]
    });
    let program_path = dir.join("main.x07.json");
    write_bytes(
        &program_path,
        serde_json::to_vec(&program_doc)
            .expect("serialize x07AST")
            .as_slice(),
    );

    let stmt_path = dir.join("stmt1.json");
    write_bytes(
        &stmt_path,
        serde_json::to_vec(&serde_json::json!(["let", "x", 1]))
            .expect("serialize stmt")
            .as_slice(),
    );

    let out = run_x07(&[
        "ast",
        "edit",
        "insert-stmts",
        "--in",
        program_path.to_str().unwrap(),
        "--defn",
        "main.f",
        "--stmt-file",
        stmt_path.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let edited_bytes = std::fs::read(&program_path).expect("read edited program");
    let edited_doc: Value = serde_json::from_slice(&edited_bytes).expect("parse edited x07AST");
    assert_eq!(
        edited_doc.pointer("/decls/0/body").expect("body"),
        &serde_json::json!(["begin", ["let", "y", 2], ["let", "x", 1], 0])
    );
}

#[test]
fn x07_ast_edit_apply_quickfix_applies_single_borrow_fix() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_edit_apply_quickfix_borrow");
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
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "bytes_view",
                "body": ["bytes.view", ["bytes.lit", "a"]]
            }
        ]
    }))
    .expect("serialize x07AST");
    let program_path = dir.join("main.x07.json");
    write_bytes(&program_path, &program);

    let out = run_x07(&[
        "ast",
        "edit",
        "apply-quickfix",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        "/decls/0/body/1",
        "--code",
        "X07-BORROW-0001",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);

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
fn x07_ast_edit_insert_stmts_errors_on_unknown_defn() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_edit_insert_unknown_defn");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.f",
                "params": [],
                "result": "i32",
                "body": 0
            }
        ]
    });
    let program_path = dir.join("main.x07.json");
    write_bytes(
        &program_path,
        serde_json::to_vec(&program_doc)
            .expect("serialize x07AST")
            .as_slice(),
    );

    let stmt_path = dir.join("stmt1.json");
    write_bytes(
        &stmt_path,
        serde_json::to_vec(&serde_json::json!(["let", "x", 1]))
            .expect("serialize stmt")
            .as_slice(),
    );

    let out = run_x07(&[
        "ast",
        "edit",
        "insert-stmts",
        "--in",
        program_path.to_str().unwrap(),
        "--defn",
        "main.nope",
        "--stmt-file",
        stmt_path.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(20));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no matching defn/defasync"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], false);
}

#[test]
fn x07_ast_edit_apply_quickfix_errors_on_ambiguous_match() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_edit_apply_quickfix_ambiguous");
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
            ["let", "x", ["view.len", ["bytes.view", ["bytes.lit", "a"]]]],
            ["let", "y", ["view.len", ["bytes.view", ["bytes.lit", "b"]]]],
            0
        ]
    }))
    .expect("serialize x07AST");
    let program_path = dir.join("main.x07.json");
    write_bytes(&program_path, &program);

    let out = run_x07(&[
        "ast",
        "edit",
        "apply-quickfix",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        "/solve",
        "--code",
        "X07-BORROW-0001",
    ]);
    assert_eq!(out.status.code(), Some(20));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("ambiguous quickfix selection"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], false);
}

#[test]
fn x07_ast_slice_pointer_remap_preserves_value() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_slice_ptr_remap");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "main",
        "imports": ["std.bytes", "std.text"],
        "decls": [
            {
                "kind": "defn",
                "name": "main.aa_helper",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "body": "x"
            },
            {
                "kind": "defn",
                "name": "main.zz_focus",
                "type_params": [{"name":"A"}],
                "requires": [{"expr": 0}],
                "params": [{"name":"y","ty":"i32"}],
                "result": "i32",
                "body": ["main.aa_helper", "y"]
            }
        ]
    });
    let program_bytes = serde_json::to_vec(&program_doc).expect("serialize x07AST");
    let program_path = dir.join("main.x07.json");
    write_bytes(&program_path, &program_bytes);

    let ptr = "/decls/1/body/1";
    let out = run_x07(&[
        "ast",
        "slice",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        ptr,
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty(), "expected empty stderr");

    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], true);
    assert_eq!(report["slice_meta"]["ptr"], ptr);

    let remaps = report["slice_meta"]["ptr_remap"]
        .as_array()
        .expect("slice_meta.ptr_remap[]");
    assert_eq!(remaps.len(), 1, "expected a single pointer remap");
    assert_eq!(remaps[0]["from"], ptr);
    let new_ptr = remaps[0]["to"].as_str().expect("ptr_remap.to");

    let orig_value = program_doc.pointer(ptr).expect("value at original ptr");
    let slice_ast = report.get("slice_ast").expect("slice_ast");
    let slice_value = slice_ast.pointer(new_ptr).expect("value at remapped ptr");
    assert_eq!(
        slice_value, orig_value,
        "expected remapped pointer value match"
    );
}

#[test]
fn x07_ast_slice_semantic_closure_all_vs_locals() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_slice_semantic_closure");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "main",
        "imports": ["std.bytes", "std.text"],
        "decls": [
            {
                "kind": "defn",
                "name": "main.aa_helper",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "body": "x"
            },
            {
                "kind": "defn",
                "name": "main.zz_focus",
                "type_params": [{"name":"A"}],
                "requires": [{"expr": 0}],
                "params": [{"name":"y","ty":"i32"}],
                "result": "i32",
                "body": ["+", ["main.aa_helper", 1], ["std.bytes.len", ["bytes.lit", "abc"]]]
            }
        ]
    });
    let program_path = dir.join("main.x07.json");
    write_bytes(
        &program_path,
        &serde_json::to_vec(&program_doc).expect("serialize x07AST"),
    );

    let ptr = "/decls/1/body/1/0";

    let out_all = run_x07(&[
        "ast",
        "slice",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        ptr,
    ]);
    assert_eq!(
        out_all.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out_all.stderr)
    );
    let report_all = parse_json_stdout(&out_all);
    assert_eq!(report_all["ok"], true);
    let slice_all = report_all.get("slice_ast").expect("slice_ast");
    assert_eq!(slice_all["imports"], serde_json::json!(["std.bytes"]));
    let decls_all = slice_all["decls"].as_array().expect("decls[]");
    assert!(
        decls_all
            .iter()
            .any(|d| d.get("name").and_then(Value::as_str) == Some("main.aa_helper")),
        "expected helper decl included for closure=all"
    );

    let out_locals = run_x07(&[
        "ast",
        "slice",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        ptr,
        "--closure",
        "locals",
    ]);
    assert_eq!(
        out_locals.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out_locals.stderr)
    );
    let report_locals = parse_json_stdout(&out_locals);
    assert_eq!(report_locals["ok"], true);

    let slice_locals = report_locals.get("slice_ast").expect("slice_ast");
    assert_eq!(slice_locals["imports"], serde_json::json!([]));
    let decls_locals = slice_locals["decls"].as_array().expect("decls[]");
    assert_eq!(
        decls_locals
            .first()
            .and_then(|d| d.get("name"))
            .and_then(Value::as_str),
        Some("main.zz_focus"),
        "expected focus decl first"
    );
    assert!(
        decls_locals
            .iter()
            .any(|d| d.get("name").and_then(Value::as_str) == Some("main.aa_helper")),
        "expected helper decl included for closure=locals"
    );
    assert!(
        decls_locals[0].get("type_params").is_none(),
        "expected types stripped for closure=locals"
    );
    assert!(
        decls_locals[0].get("requires").is_none(),
        "expected contracts stripped for closure=locals"
    );

    let meta = &report_locals["slice_meta"];
    assert_eq!(meta["omitted"]["imports"], true);
    assert_eq!(meta["omitted"]["types"], true);
    assert_eq!(meta["missing"]["imports"], serde_json::json!(["std.bytes"]));
    let missing_types = meta["missing"]["types"]
        .as_array()
        .expect("missing.types[]");
    assert!(
        missing_types.iter().any(|v| v == "A"),
        "expected missing types to include type param name"
    );
    assert!(
        missing_types.iter().any(|v| v == "contracts"),
        "expected missing types to include contracts sentinel"
    );
}

#[test]
fn x07_ast_slice_is_deterministic_and_wrapper_meta_has_input_digest() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_slice_determinism");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let program_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.focus",
                "params": [{"name":"y","ty":"i32"}],
                "result": "i32",
                "body": ["+", "y", 1]
            }
        ]
    });
    let program_bytes = serde_json::to_vec(&program_doc).expect("serialize x07AST");
    let program_path = dir.join("main.x07.json");
    write_bytes(&program_path, &program_bytes);

    let ptr = "/decls/0/body/1";

    let out1 = run_x07(&[
        "ast",
        "slice",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        ptr,
    ]);
    assert_eq!(out1.status.code(), Some(0));

    let out2 = run_x07(&[
        "ast",
        "slice",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        ptr,
    ]);
    assert_eq!(out2.status.code(), Some(0));
    assert_eq!(
        out1.stdout, out2.stdout,
        "expected byte-identical ast slice output"
    );

    let out_wrapped = run_x07(&[
        "ast",
        "slice",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        ptr,
        "--json",
    ]);
    assert_eq!(
        out_wrapped.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out_wrapped.stderr)
    );
    let report = parse_json_stdout(&out_wrapped);
    assert_eq!(report["schema_version"], "x07.tool.ast.slice.report@0.1.0");
    assert_eq!(report["command"], "x07.ast.slice");
    assert_eq!(report["ok"], true);

    let expected_sha = sha256_hex(&program_bytes);
    let inputs = report["meta"]["inputs"].as_array().expect("meta.inputs[]");
    let program_path_str = program_path.display().to_string();
    let input = inputs
        .iter()
        .find(|d| d.get("path").and_then(Value::as_str) == Some(program_path_str.as_str()))
        .expect("missing input digest for program");
    assert_eq!(input["sha256"], expected_sha);
}

#[test]
fn x07_ast_slice_bounds_emit_truncation_diagnostic_and_respect_max_bytes() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_ast_slice_bounds");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let big = "x".repeat(4096);
    let program_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": "main",
        "imports": ["std.bytes"],
        "decls": [
            {
                "kind": "defn",
                "name": "main.aa_helper",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "body": "x"
            },
            {
                "kind": "defn",
                "name": "main.zz_focus",
                "params": [],
                "result": "i32",
                "body": ["+", ["main.aa_helper", 1], ["std.bytes.len", ["bytes.lit", big]]]
            }
        ]
    });
    let program_path = dir.join("main.x07.json");
    write_bytes(
        &program_path,
        &serde_json::to_vec(&program_doc).expect("serialize x07AST"),
    );

    let ptr = "/decls/1/name";

    let out_nodes = run_x07(&[
        "ast",
        "slice",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        ptr,
        "--max-nodes",
        "1",
    ]);
    assert_eq!(
        out_nodes.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out_nodes.stderr)
    );
    let report_nodes = parse_json_stdout(&out_nodes);
    assert_eq!(report_nodes["ok"], true);
    assert_eq!(report_nodes["slice_meta"]["truncated"], true);
    let diags = report_nodes["diagnostics"]
        .as_array()
        .expect("diagnostics[]");
    assert!(
        diags.iter().any(|d| {
            d.get("code")
                .and_then(Value::as_str)
                .is_some_and(|c| c == "X07-AST-SLICE-0001")
        }),
        "expected truncation diagnostic"
    );
    let decls = report_nodes["slice_ast"]["decls"]
        .as_array()
        .expect("decls[]");
    assert_eq!(decls.len(), 1, "expected max_nodes to drop decls");

    let out_path = dir.join("slice.x07.json");
    let max_bytes = 350usize;
    let out_bytes = run_x07(&[
        "--out",
        out_path.to_str().unwrap(),
        "ast",
        "slice",
        "--in",
        program_path.to_str().unwrap(),
        "--ptr",
        ptr,
        "--max-bytes",
        &max_bytes.to_string(),
    ]);
    assert_eq!(
        out_bytes.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out_bytes.stderr)
    );
    let report_bytes = parse_json_stdout(&out_bytes);
    assert_eq!(report_bytes["ok"], true);
    assert_eq!(report_bytes["slice_meta"]["truncated"], true);
    let diags = report_bytes["diagnostics"]
        .as_array()
        .expect("diagnostics[]");
    assert!(
        diags.iter().any(|d| {
            d.get("code")
                .and_then(Value::as_str)
                .is_some_and(|c| c == "X07-AST-SLICE-0001")
        }),
        "expected truncation diagnostic"
    );

    let mut written = std::fs::read(&out_path).expect("read slice output");
    if written.last() == Some(&b'\n') {
        written.pop();
    }
    assert!(
        written.len() <= max_bytes,
        "expected canonical slice_ast bytes len <= max_bytes (got {})",
        written.len()
    );
}

#[test]
fn x07_agent_context_emits_context_pack_with_valid_slice_and_digests() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_agent_context_pack");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let project_doc = serde_json::json!({
        "schema_version": "x07.project@0.2.0",
        "world": "solve-pure",
        "entry": "main.x07.json",
        "module_roots": ["."]
    });
    let project_bytes = serde_json::to_vec(&project_doc).expect("serialize project");
    let project_path = dir.join("x07.json");
    write_bytes(&project_path, &project_bytes);

    let entry_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "entry",
        "module_id": "main",
        "imports": [],
        "decls": [
            {
                "kind": "defn",
                "name": "main.aa_helper",
                "params": [{"name":"x","ty":"i32"}],
                "result": "i32",
                "body": "x"
            },
            {
                "kind": "defn",
                "name": "main.zz_focus",
                "params": [{"name":"y","ty":"i32"}],
                "result": "i32",
                "body": ["main.aa_helper", "y"]
            }
        ],
        "solve": ["main.zz_focus", 1]
    });
    let entry_bytes = serde_json::to_vec(&entry_doc).expect("serialize entry");
    let entry_path = dir.join("main.x07.json");
    write_bytes(&entry_path, &entry_bytes);

    let ptr = "/decls/1/body/1";
    let diag_doc = serde_json::json!({
        "schema_version": X07DIAG_SCHEMA_VERSION,
        "ok": false,
        "diagnostics": [
            {
                "code": "X07-TEST-0001",
                "severity": "error",
                "stage": "lint",
                "message": "boom",
                "loc": {"kind":"x07ast","ptr": ptr}
            }
        ],
        "meta": {}
    });
    let diag_bytes = serde_json::to_vec(&diag_doc).expect("serialize x07diag");
    let diag_path = dir.join("diag.x07diag.json");
    write_bytes(&diag_path, &diag_bytes);

    let out = run_x07(&[
        "agent",
        "context",
        "--diag",
        diag_path.to_str().unwrap(),
        "--project",
        project_path.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty(), "expected empty stderr");

    let pack = parse_json_stdout(&out);
    assert_eq!(pack["schema_version"], X07_AGENT_CONTEXT_SCHEMA_VERSION);
    assert_eq!(pack["toolchain"]["name"], "x07");
    assert_eq!(pack["toolchain"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(pack["project"]["root"], dir.display().to_string());
    assert_eq!(pack["project"]["world"], "solve-pure");
    assert_eq!(pack["project"]["entry"], "main.x07.json");
    assert_eq!(pack["focus"]["diag_code"], "X07-TEST-0001");
    assert_eq!(pack["focus"]["loc_ptr"], ptr);

    assert_eq!(
        pack["diagnostics"]["schema_version"],
        X07DIAG_SCHEMA_VERSION
    );

    let slice_ast = pack["ast"]["slice_ast"].clone();
    let slice_bytes = serde_json::to_vec(&slice_ast).expect("encode slice_ast");
    x07c::x07ast::parse_x07ast_json(&slice_bytes).expect("parse slice_ast x07AST");

    let inputs = pack["digests"]["inputs"]
        .as_array()
        .expect("digests.inputs[]");
    assert_eq!(inputs.len(), 3, "expected diag+project+entry digests");
    let get_input = |p: &Path| {
        let path_str = p.display().to_string();
        inputs
            .iter()
            .find(|d| d.get("path").and_then(Value::as_str) == Some(path_str.as_str()))
            .expect("missing input digest")
    };
    assert_eq!(get_input(&diag_path)["sha256"], sha256_hex(&diag_bytes));
    assert_eq!(
        get_input(&project_path)["sha256"],
        sha256_hex(&project_bytes)
    );
    assert_eq!(get_input(&entry_path)["sha256"], sha256_hex(&entry_bytes));
    assert_eq!(
        pack["digests"]["outputs"],
        serde_json::json!([]),
        "expected empty outputs digests"
    );
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
    assert_eq!(bundle["x07ast_schema_version"], X07AST_SCHEMA_VERSION);
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
    assert_eq!(manifest["x07ast_schema_version"], X07AST_SCHEMA_VERSION);
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
fn x07_review_diff_fail_on_formal_verification_posture_returns_20() {
    let root = repo_root();
    let fixtures = fixtures_root().join("review");
    let before = fixtures.join("before");
    let after = fixtures.join("after");

    let work = fresh_tmp_dir(&root, "tmp_x07_review_diff_formal_fail_on");
    let before_copy = work.join("before");
    let after_copy = work.join("after");
    copy_dir_recursive(&before, &before_copy);
    copy_dir_recursive(&after, &after_copy);

    let json_out = work.join("review_formal.json");
    let html_out = work.join("review_formal.html");

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
        "proof-coverage-decrease",
        "--fail-on",
        "boundary-relaxation",
        "--fail-on",
        "trusted-subset-expansion",
    ]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report: Value =
        serde_json::from_slice(&std::fs::read(&json_out).expect("read review json"))
            .expect("parse review json");
    assert!(
        !report["highlights"]["proof_changes"]
            .as_array()
            .expect("proof_changes[]")
            .is_empty(),
        "expected proof coverage changes"
    );
    assert!(
        !report["highlights"]["boundary_changes"]
            .as_array()
            .expect("boundary_changes[]")
            .is_empty(),
        "expected boundary contract changes"
    );
    assert!(
        !report["highlights"]["subset_changes"]
            .as_array()
            .expect("subset_changes[]")
            .is_empty(),
        "expected trusted subset changes"
    );
}

#[test]
fn x07_review_diff_fail_on_recursive_proof_and_summary_posture_returns_20() {
    let root = repo_root();
    let fixtures = fixtures_root().join("review");
    let before = fixtures.join("before");
    let after = fixtures.join("after");

    let work = fresh_tmp_dir(&root, "tmp_x07_review_diff_recursive_summary");
    let before_copy = work.join("before");
    let after_copy = work.join("after");
    copy_dir_recursive(&before, &before_copy);
    copy_dir_recursive(&after, &after_copy);

    write_json(
        &before_copy.join("arch/verify.coverage.json"),
        &serde_json::json!({
            "schema_version": "x07.verify.coverage@0.4.0",
            "entry": "app.main",
            "worlds": ["solve-pure"],
            "summary": {
                "reachable_defn": 1,
                "supported_defn": 1,
                "recursive_defn": 1,
                "supported_recursive_defn": 1,
                "imported_proof_summary_defn": 0,
                "termination_proven_defn": 0,
                "unsupported_recursive_defn": 0,
                "reachable_async": 0,
                "supported_async": 0,
                "trusted_primitives": 0,
                "trusted_scheduler_models": 0,
                "capsule_boundaries": 0,
                "uncovered_defn": 0,
                "unsupported_defn": 0
            },
            "functions": [
                {
                    "symbol": "app.main",
                    "kind": "defn",
                    "status": "supported_recursive",
                    "support_summary": {
                        "recursion_kind": "self_recursive",
                        "has_decreases": true,
                        "decreases_count": 1,
                        "prove_supported": true
                    }
                }
            ]
        }),
    );
    write_json(
        &after_copy.join("arch/verify.coverage.json"),
        &serde_json::json!({
            "schema_version": "x07.verify.coverage@0.4.0",
            "entry": "app.main",
            "worlds": ["solve-pure"],
            "summary": {
                "reachable_defn": 1,
                "supported_defn": 0,
                "recursive_defn": 1,
                "supported_recursive_defn": 0,
                "imported_proof_summary_defn": 0,
                "termination_proven_defn": 0,
                "unsupported_recursive_defn": 1,
                "reachable_async": 0,
                "supported_async": 0,
                "trusted_primitives": 0,
                "trusted_scheduler_models": 0,
                "capsule_boundaries": 0,
                "uncovered_defn": 0,
                "unsupported_defn": 1
            },
            "functions": [
                {
                    "symbol": "app.main",
                    "kind": "defn",
                    "status": "unsupported",
                    "support_summary": {
                        "recursion_kind": "self_recursive",
                        "has_decreases": false,
                        "decreases_count": 0,
                        "prove_supported": false
                    }
                }
            ]
        }),
    );

    let json_out = work.join("review_recursive_summary.json");
    let html_out = work.join("review_recursive_summary.html");
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
        "recursion-proof-coverage-decrease",
        "--fail-on",
        "summary-downgrade",
    ]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report: Value =
        serde_json::from_slice(&std::fs::read(&json_out).expect("read review json"))
            .expect("parse review json");
    assert!(
        !report["highlights"]["recursive_proof_changes"]
            .as_array()
            .expect("recursive_proof_changes[]")
            .is_empty(),
        "expected recursive proof coverage changes"
    );
    assert!(
        report["highlights"]["summary_changes"]
            .as_array()
            .expect("summary_changes[]")
            .iter()
            .any(|change| change["kind"] == "proof_summary"),
        "expected proof summary downgrade changes"
    );
    let diag_codes = report["diags"]
        .as_array()
        .expect("diags[]")
        .iter()
        .filter_map(|diag| diag.get("code").and_then(Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(diag_codes.contains("X07RD_RECURSION_PROOF_COVERAGE_DECREASE"));
    assert!(diag_codes.contains("X07RD_SUMMARY_DOWNGRADE"));
}

#[test]
fn x07_review_diff_fail_on_capsule_runtime_and_sandbox_posture_returns_20() {
    let root = repo_root();
    let fixtures = fixtures_root().join("review");
    let before = fixtures.join("before");
    let after = fixtures.join("after");

    let work = fresh_tmp_dir(&root, "tmp_x07_review_diff_capsule_runtime");
    let before_copy = work.join("before");
    let after_copy = work.join("after");
    copy_dir_recursive(&before, &before_copy);
    copy_dir_recursive(&after, &after_copy);

    write_json(
        &before_copy.join("arch/capsules/index.x07capsule.json"),
        &serde_json::json!({
            "schema_version": "x07.capsule.index@0.1.0",
            "capsules": [
                {
                    "id": "capsule.echo_v1",
                    "worlds_allowed": ["run-os-sandboxed"],
                    "capabilities": ["net"],
                    "contract_path": "capsule.echo.contract.json",
                    "attestation_path": "capsule.echo.attest.json"
                }
            ]
        }),
    );
    write_json(
        &before_copy.join("arch/capsules/capsule.echo.contract.json"),
        &serde_json::json!({
            "schema_version": "x07.capsule.contract@0.2.0",
            "id": "capsule.echo_v1",
            "worlds_allowed": ["run-os-sandboxed"],
            "capabilities": ["net"],
            "language": {
                "allow_unsafe": false,
                "allow_ffi": false
            },
            "input": { "shape": { "brand": "capsule.echo.in_v1" } },
            "output": { "shape": { "brand": "capsule.echo.out_v1" } },
            "error_spaces": ["capsule.echo.error_v1"],
            "effect_log": {
                "schema_path": "capsule.echo.effect_log.json",
                "redaction": "metadata_only",
                "replay_safe": true
            },
            "replay": { "mode": "deterministic" },
            "conformance": {
                "tests": ["capsule_echo_smoke"],
                "report_path": null
            },
            "network": null
        }),
    );
    write_json(
        &after_copy.join("arch/capsules/index.x07capsule.json"),
        &serde_json::json!({
            "schema_version": "x07.capsule.index@0.1.0",
            "capsules": [
                {
                    "id": "capsule.echo_v1",
                    "worlds_allowed": ["run-os-sandboxed"],
                    "capabilities": ["net"],
                    "contract_path": "capsule.echo.contract.json",
                    "attestation_path": "capsule.echo.attest.json"
                },
                {
                    "id": "capsule.fs_v1",
                    "worlds_allowed": ["run-os-sandboxed"],
                    "capabilities": ["fs"],
                    "contract_path": "capsule.fs.contract.json",
                    "attestation_path": "capsule.fs.attest.json"
                }
            ]
        }),
    );
    write_json(
        &after_copy.join("arch/capsules/capsule.echo.contract.json"),
        &serde_json::json!({
            "schema_version": "x07.capsule.contract@0.2.0",
            "id": "capsule.echo_v1",
            "worlds_allowed": ["run-os-sandboxed"],
            "capabilities": ["net", "process"],
            "language": {
                "allow_unsafe": true,
                "allow_ffi": true
            },
            "input": { "shape": { "brand": "capsule.echo.in_v1" } },
            "output": { "shape": { "brand": "capsule.echo.out_v1" } },
            "error_spaces": ["capsule.echo.error_v1"],
            "effect_log": {
                "schema_path": "capsule.echo.effect_log.json",
                "redaction": "none",
                "replay_safe": false
            },
            "replay": { "mode": "deterministic" },
            "conformance": {
                "tests": ["capsule_echo_smoke"],
                "report_path": null
            },
            "network": null
        }),
    );
    write_json(
        &before_copy.join("runtime.attest.json"),
        &serde_json::json!({
            "schema_version": "x07.runtime.attest@0.2.0",
            "world": "run-os-sandboxed",
            "sandbox_backend": "vm",
            "artifact_path": "solver",
            "policy_path": "policy.json",
            "input_len_bytes": 0,
            "guest_image_digest": format!("sha256:{}", "0".repeat(64)),
            "effective_policy_digest": format!("sha256:{}", "1".repeat(64)),
            "network_mode": "allowlist",
            "network_enforcement": "vm_boundary_allowlist",
            "allow_dns": true,
            "allow_tcp": true,
            "allow_udp": false,
            "effective_allow_hosts": [{"host": "api.example.com", "ports": [443]}],
            "effective_deny_hosts": [],
            "bundled_binary_digest": format!("sha256:{}", "2".repeat(64)),
            "compile_attestation_digest": format!("sha256:{}", "3".repeat(64)),
            "capsule_attestation_digests": [],
            "effect_log_digests": [],
            "weaker_isolation": false,
            "outcome": { "ok": true, "exit_status": 0 }
        }),
    );
    write_json(
        &after_copy.join("runtime.attest.json"),
        &serde_json::json!({
            "schema_version": "x07.runtime.attest@0.2.0",
            "world": "run-os-sandboxed",
            "sandbox_backend": "os",
            "artifact_path": "solver",
            "policy_path": "policy.json",
            "input_len_bytes": 0,
            "guest_image_digest": format!("sha256:{}", "4".repeat(64)),
            "effective_policy_digest": format!("sha256:{}", "5".repeat(64)),
            "network_mode": "allowlist",
            "network_enforcement": "unsupported",
            "allow_dns": true,
            "allow_tcp": true,
            "allow_udp": false,
            "effective_allow_hosts": [{"host": "api.example.com", "ports": [443]}],
            "effective_deny_hosts": [],
            "bundled_binary_digest": format!("sha256:{}", "6".repeat(64)),
            "compile_attestation_digest": format!("sha256:{}", "7".repeat(64)),
            "capsule_attestation_digests": [],
            "effect_log_digests": [],
            "weaker_isolation": true,
            "outcome": { "ok": true, "exit_status": 0 }
        }),
    );

    let json_out = work.join("review_capsule_runtime.json");
    let html_out = work.join("review_capsule_runtime.html");
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
        "capsule-contract-relaxation",
        "--fail-on",
        "capsule-set-change",
        "--fail-on",
        "sandbox-policy-widen",
        "--fail-on",
        "runtime-attestation-regression",
        "--fail-on",
        "weaker-isolation-enabled",
    ]);
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report: Value =
        serde_json::from_slice(&std::fs::read(&json_out).expect("read review json"))
            .expect("parse review json");
    assert!(
        report["highlights"]["capsule_changes"]
            .as_array()
            .expect("capsule_changes[]")
            .iter()
            .any(|c| c["kind"] == "capsule_contract"),
        "expected capsule contract changes"
    );
    assert!(
        report["highlights"]["capsule_changes"]
            .as_array()
            .expect("capsule_changes[]")
            .iter()
            .any(|c| c["kind"] == "capsule_set"),
        "expected capsule set changes"
    );
    assert!(
        !report["highlights"]["runtime_attestation_changes"]
            .as_array()
            .expect("runtime_attestation_changes[]")
            .is_empty(),
        "expected runtime attestation changes"
    );
    assert!(
        report["highlights"]["sandbox_policy_changes"]
            .as_array()
            .expect("sandbox_policy_changes[]")
            .iter()
            .any(|c| c["kind"] == "weaker_isolation"),
        "expected weaker isolation change"
    );
    let diag_codes = report["diags"]
        .as_array()
        .expect("diags[]")
        .iter()
        .filter_map(|diag| diag.get("code").and_then(Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(diag_codes.contains("X07RD_CAPSULE_CONTRACT_RELAXATION"));
    assert!(diag_codes.contains("X07RD_CAPSULE_SET_CHANGE"));
    assert!(diag_codes.contains("X07RD_SANDBOX_POLICY_WIDEN"));
    assert!(diag_codes.contains("X07RD_RUNTIME_ATTEST_REGRESSION"));
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

    assert_eq!(report["sbom"]["format"], "cyclonedx");
    assert_eq!(report["sbom"]["generated"], true);
    assert_eq!(report["sbom"]["cyclonedx"]["spec_version"], "1.5");
    let sbom_path = report["sbom"]["path"].as_str().expect("sbom.path");
    let expected_sbom_path = trust_json.with_extension("sbom.cdx.json");
    assert_eq!(PathBuf::from(sbom_path), expected_sbom_path);
    assert!(
        expected_sbom_path.is_file(),
        "missing {}",
        expected_sbom_path.display()
    );

    let sbom_bytes_1 = std::fs::read(&expected_sbom_path).expect("read sbom-1");
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
    let sbom_bytes_2 = std::fs::read(&expected_sbom_path).expect("read sbom-2");
    assert_eq!(sbom_bytes_1, sbom_bytes_2, "SBOM must be deterministic");

    let trust_none_json = dir.join("trust_none.json");
    let trust_none_sbom = trust_none_json.with_extension("sbom.cdx.json");
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
            trust_none_json.to_str().unwrap(),
            "--sbom-format",
            "none",
            "--fail-on",
            "sbom-missing",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        trust_none_json.is_file(),
        "missing {}",
        trust_none_json.display()
    );
    assert!(
        !trust_none_sbom.exists(),
        "expected no SBOM artifact for --sbom-format none"
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
fn x07_trust_capsule_attest_overwrites_fixture_and_check_passes() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_capsule");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    scaffold_sandbox_trust_profile_fixture(&dir, "run-os-sandboxed", false);

    write_json(
        &dir.join("target/capsules/echo.conformance.json"),
        &serde_json::json!({
            "schema_version": "x07.conformance.report@0.1.0",
            "ok": true,
            "tests": ["smoke_main"]
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "capsule",
            "attest",
            "--contract",
            "arch/capsules/capsule.echo.contract.json",
            "--module",
            "src/main.x07.json",
            "--lockfile",
            "x07.lock.json",
            "--conformance-report",
            "target/capsules/echo.conformance.json",
            "--out",
            "arch/capsules/capsule.echo.attest.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let attest_stdout = parse_json_stdout(&out);
    assert_eq!(attest_stdout["schema_version"], "x07.capsule.attest@0.2.0");
    assert_eq!(attest_stdout["peer_policy_digests"], serde_json::json!([]));
    assert_eq!(attest_stdout["capsule_id"], "capsule.echo_v1");
    assert!(dir.join("arch/capsules/capsule.echo.attest.json").is_file());

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "capsule",
            "check",
            "--project",
            "x07.json",
            "--index",
            "arch/capsules/index.x07capsule.json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let check_stdout = parse_json_stdout(&out);
    assert_eq!(check_stdout["ok"], true);
    assert_eq!(check_stdout["checked_capsules"], 1);
}

#[test]
fn x07_trust_profile_check_accepts_strict_pure_project() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_profile_check");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(dir.join("src")).expect("create src dir");
    std::fs::create_dir_all(dir.join("arch/trust/profiles")).expect("create trust dir");

    write_json(
        &dir.join("x07.json"),
        &serde_json::json!({
            "schema_version": "x07.project@0.2.0",
            "world": "solve-pure",
            "entry": "src/main.x07.json",
            "module_roots": ["src"],
            "dependencies": []
        }),
    );
    write_json(
        &dir.join("x07.lock.json"),
        &serde_json::json!({
            "schema_version": "x07.lock@0.2.0",
            "dependencies": []
        }),
    );
    write_json(
        &dir.join("src/app.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "app",
            "imports": [],
            "decls": [
                {"kind":"export","names":["app.main"]},
                {
                    "kind": "defn",
                    "name": "app.main",
                    "params": [],
                    "result": "bytes",
                    "requires": [{"id":"r0","expr":["=",0,0]}],
                    "body": ["bytes.lit","OK"]
                }
            ]
        }),
    );
    write_json(
        &dir.join("src/main.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": ["app"],
            "decls": [],
            "solve": ["app.main"]
        }),
    );
    write_json(
        &dir.join("arch/manifest.x07arch.json"),
        &serde_json::json!({
            "schema_version": "x07.arch.manifest@0.3.0",
            "repo": { "id": "fixture", "root": "." },
            "externals": { "allowed_import_prefixes": ["std."], "allowed_exact": [] },
            "nodes": [
                {
                    "id": "app_core",
                    "match": { "module_prefixes": ["app."], "path_globs": ["src/**/*.x07.json"] },
                    "world": "solve-pure",
                    "trust_zone": "verified_core",
                    "visibility": { "mode": "public", "visible_to": [] },
                    "imports": { "deny_prefixes": ["std.os.", "ext."], "allow_prefixes": ["app.", "std."] }
                }
            ],
            "rules": [
                { "kind": "deny_cycles_v1", "id": "deny_cycles.nodes_v1", "scope": "nodes" }
            ],
            "checks": {
                "deny_cycles": true,
                "deny_orphans": true,
                "enforce_visibility": true,
                "enforce_world_caps": true,
                "allowlist_mode": {
                    "enabled": true,
                    "default_allow_external": false,
                    "default_allow_internal": false
                },
                "brand_boundary_v1": { "enabled": true },
                "world_of_imported_v1": { "enabled": true }
            },
            "contracts_v1": {
                "boundaries": {
                    "index_path": "arch/boundaries/index.x07boundary.json",
                    "enforce": "error"
                }
            },
            "tool_budgets": {
                "max_modules": 1000,
                "max_edges": 1000,
                "max_diags": 1000
            }
        }),
    );
    write_json(
        &dir.join("arch/trust/profiles/verified_core_pure_v1.json"),
        &serde_json::json!({
            "schema_version": "x07.trust.profile@0.4.0",
            "id": "verified_core_pure_v1",
            "claims": ["human_can_review_certificate_not_source"],
            "entrypoints": ["app.main"],
            "worlds_allowed": ["solve-pure"],
            "language_subset": {
                "allow_defasync": false,
                "allow_recursion": false,
                "allow_extern": false,
                "allow_unsafe": false,
                "allow_ffi": false,
                "allow_dynamic_dispatch": false
            },
            "arch_requirements": {
                "manifest_min_version": "x07.arch.manifest@0.3.0",
                "require_allowlist_mode": true,
                "require_deny_cycles": true,
                "require_deny_orphans": true,
                "require_visibility": true,
                "require_world_caps": true,
                "require_brand_boundaries": true
            },
            "evidence_requirements": {
                "require_boundary_index": true,
                "require_schema_derive_check": true,
                "require_smoke_harnesses": true,
                "require_unit_tests": true,
                "require_pbt": "public_boundaries_only",
                "require_proof_mode": "prove",
                "require_proof_coverage": "all_reachable_defn",
                "require_async_proof_coverage": false,
                "require_per_symbol_prove_reports_defn": true,
                "require_per_symbol_prove_reports_async": false,
                "allow_coverage_summary_imports": false,
                "require_capsule_attestations": false,
                "require_runtime_attestation": false,
                "require_effect_log_digests": false,
                "require_peer_policies": false,
                "require_network_capsules": false,
                "require_dependency_closure_attestation": false,
                "require_compile_attestation": true,
                "require_trust_report_clean": true,
                "require_sbom": true
            },
            "sandbox_requirements": {
                "sandbox_backend": "any",
                "forbid_weaker_isolation": false,
                "network_mode": "any",
                "network_enforcement": "any"
            }
        }),
    );

    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "profile",
            "check",
            "--project",
            "x07.json",
            "--profile",
            "arch/trust/profiles/verified_core_pure_v1.json",
            "--entry",
            "app.main",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], Value::Bool(true));
    assert_eq!(report["profile"], "verified_core_pure_v1");
    assert_eq!(report["entry"], "app.main");
    assert_eq!(report["exit_code"], 0);
}

fn scaffold_deps_capability_fixture(dir: &Path) -> (String, String, String) {
    std::fs::create_dir_all(dir.join("src")).expect("create src");

    write_json(
        &dir.join("src/main.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [],
            "solve": ["bytes.lit", "ok"]
        }),
    );

    let dep_name = "ext-bad".to_string();
    let dep_version = "0.1.0".to_string();
    let dep_path = format!(".x07/deps/{}/{}", dep_name, dep_version);

    let dep_modules_dir = dir.join(&dep_path).join("modules");
    std::fs::create_dir_all(&dep_modules_dir).expect("create dep modules");
    write_json(
        &dep_modules_dir.join("bad.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "module",
            "module_id": "bad",
            "imports": [],
            "decls": [
                {
                    "kind": "defn",
                    "name": "bad.net",
                    "params": [],
                    "result": ["t", "bytes"],
                    "body": ["std.os.net.fake"]
                }
            ]
        }),
    );

    write_json(
        &dir.join("x07.json"),
        &serde_json::json!({
            "schema_version": "x07.project@0.2.0",
            "world": "solve-pure",
            "entry": "src/main.x07.json",
            "module_roots": ["src"],
            "dependencies": [
                { "name": dep_name, "version": dep_version, "path": dep_path }
            ]
        }),
    );

    write_json(
        &dir.join("x07.lock.json"),
        &serde_json::json!({
            "schema_version": "x07.lock@0.2.0",
            "dependencies": [
                {
                    "name": dep_name,
                    "version": dep_version,
                    "path": dep_path,
                    "package_manifest_sha256": "00",
                    "module_root": "modules",
                    "modules_sha256": {}
                }
            ]
        }),
    );

    (dep_name, dep_version, dep_path)
}

#[test]
fn x07_trust_report_fail_on_deps_capability_emits_structured_diagnostics_in_wrapper() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_deps_capability_deny");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let (_dep_name, _dep_version, _dep_path) = scaffold_deps_capability_fixture(&dir);

    write_json(
        &dir.join("x07.deps.capability-policy.json"),
        &serde_json::json!({
            "schema_version": "x07.deps.capability_policy@0.1.0",
            "policy_id": "deny-net",
            "default": { "deny_sensitive_namespaces": ["std.os.net"] }
        }),
    );

    let trust_json = dir.join("trust.json");
    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "report",
            "--project",
            "x07.json",
            "--out",
            trust_json.to_str().unwrap(),
            "--fail-on",
            "deps-capability",
            "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], false);
    assert_eq!(report["exit_code"], 20);

    let diags = report["diagnostics"].as_array().expect("diagnostics[]");
    let deny = diags
        .iter()
        .find(|d| d["code"] == "E_DEPS_CAP_POLICY_DENY")
        .expect("missing E_DEPS_CAP_POLICY_DENY");
    assert_eq!(deny["severity"], "error");
    assert!(
        deny["data"]["offending_namespaces"]
            .as_array()
            .expect("offending_namespaces[]")
            .iter()
            .any(|v| v == "std.os.net."),
        "expected offending std.os.net."
    );
}

#[test]
fn x07_trust_report_missing_deps_cap_policy_fails_only_under_fail_on() {
    let root = repo_root();
    let dir = fresh_tmp_dir(&root, "tmp_x07_trust_deps_capability_missing_policy");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove old tmp dir");
    }
    std::fs::create_dir_all(&dir).expect("create tmp dir");

    let (_dep_name, _dep_version, _dep_path) = scaffold_deps_capability_fixture(&dir);

    let trust_json = dir.join("trust.json");
    let out = run_x07_in_dir(
        &dir,
        &[
            "trust",
            "report",
            "--project",
            "x07.json",
            "--out",
            trust_json.to_str().unwrap(),
            "--fail-on",
            "deps-capability",
            "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(20),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], false);
    assert_eq!(report["exit_code"], 20);

    let diags = report["diagnostics"].as_array().expect("diagnostics[]");
    let missing = diags
        .iter()
        .find(|d| d["code"] == "W_DEPS_CAP_POLICY_MISSING")
        .expect("missing W_DEPS_CAP_POLICY_MISSING");
    assert_eq!(missing["severity"], "error");
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

#[cfg(unix)]
#[test]
fn x07_wasm_json_emits_native_child_report() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = fresh_os_tmp_dir("x07_wasm_native_json");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let bin_dir = dir.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");

    let stub = bin_dir.join("x07-wasm");
    let stub_src = r#"#!/usr/bin/env python3
import json
import sys

doc = {
    "schema_version": "x07.wasm.doctor.report@0.1.0",
    "command": "x07-wasm.doctor",
    "ok": True,
    "exit_code": 0,
    "diagnostics": [],
    "meta": {
        "argv": sys.argv,
        "tool": {"name": "x07-wasm", "version": "0.1.8"}
    },
    "result": {
        "doctor_ok": True,
        "json_flag_seen": "--json" in sys.argv[1:]
    }
}

print(json.dumps(doc))
sys.exit(0)
"#;
    write_bytes(&stub, stub_src.as_bytes());
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
        .expect("chmod x07-wasm");

    let exe = env!("CARGO_BIN_EXE_x07");
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir.clone()];
    paths.extend(std::env::split_paths(&existing));
    let out = Command::new(exe)
        .current_dir(&dir)
        .env(ENV_SANDBOX_BACKEND, "os")
        .env(ENV_ACCEPT_WEAKER_ISOLATION, "1")
        .env("PATH", std::env::join_paths(paths).expect("join PATH"))
        .args(["wasm", "doctor", "--json"])
        .output()
        .expect("run x07 wasm doctor");

    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v: Value = serde_json::from_slice(&out.stdout).expect("parse native wasm JSON");
    assert_eq!(v["schema_version"], "x07.wasm.doctor.report@0.1.0");
    assert_eq!(v["command"], "x07-wasm.doctor");
    assert_eq!(v["result"]["doctor_ok"], Value::Bool(true));
    assert_eq!(v["result"]["json_flag_seen"], Value::Bool(true));
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

#[test]
fn x07_scope_json_schema_for_assets_embed_dir_is_available() {
    let out = run_x07(&["assets", "embed-dir", "--json-schema"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let schema: Value = serde_json::from_slice(&out.stdout).expect("parse schema");
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        Value::String("x07.tool.assets.embed-dir.report@0.1.0".to_string())
    );
    assert_eq!(
        schema["properties"]["command"]["const"],
        Value::String("x07.assets.embed-dir".to_string())
    );
}

#[test]
fn assets_embed_dir_emits_stable_paths_and_base64_literals() {
    use base64::Engine as _;

    let root = repo_root();
    let tmp = fresh_tmp_dir(&root, "assets_embed_dir");
    let in_dir = tmp.join("in");
    let out_path = tmp.join("out.x07.json");
    std::fs::create_dir_all(in_dir.join("sub")).expect("mkdir in/sub");

    let a_path = in_dir.join("a.txt");
    let b_path = in_dir.join("sub").join("b.bin");
    let a_bytes = b"hello\n".to_vec();
    let b_bytes = vec![0u8, 1u8, 2u8, 3u8, 254u8, 255u8];
    std::fs::write(&a_path, &a_bytes).expect("write a.txt");
    std::fs::write(&b_path, &b_bytes).expect("write b.bin");

    let out = run_x07(&[
        "assets",
        "embed-dir",
        "--in",
        in_dir.to_str().expect("utf-8"),
        "--module-id",
        "my.assets",
        "--out",
        out_path.to_str().expect("utf-8"),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty(), "expected empty stderr");
    assert!(out_path.is_file(), "expected out module");

    let report: Value = serde_json::from_slice(&out.stdout).expect("parse report JSON");
    assert_eq!(report["ok"], Value::Bool(true));
    assert_eq!(
        report["command"],
        Value::String("assets.embed-dir".to_string())
    );
    assert_eq!(report["module_id"], Value::String("my.assets".to_string()));
    assert_eq!(report["file_count"], Value::from(2));
    assert_eq!(
        report["files"],
        Value::Array(vec![
            Value::String("a.txt".to_string()),
            Value::String("sub/b.bin".to_string()),
        ])
    );

    let module_text = std::fs::read_to_string(&out_path).expect("read out module");
    assert!(
        module_text.contains("\"module_id\": \"my.assets\"")
            || module_text.contains("\"module_id\":\"my.assets\""),
        "module_id not present in output"
    );

    let a_b64 = base64::engine::general_purpose::STANDARD.encode(&a_bytes);
    let b_b64 = base64::engine::general_purpose::STANDARD.encode(&b_bytes);
    for needle in ["a.txt", "sub/b.bin", &a_b64, &b_b64] {
        assert!(
            module_text.contains(needle),
            "expected output module to contain {needle:?}"
        );
    }
}

#[test]
fn bundle_stdio_smoke_reads_and_writes() {
    ensure_mcp_native_backends_staged();

    let root = repo_root();
    let tmp = fresh_tmp_dir(&root, "bundle_stdio_smoke");
    std::fs::create_dir_all(&tmp).expect("create tmp dir");

    let out_path = tmp.join("app_stdio_smoke");
    let program_path = root.join("tests/external_os/stdio_smoke_ok/src/main.x07.json");
    let module_root = root.join("packages/ext/x07-ext-stdio/0.1.0/modules");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&root)
        .args([
            "--out",
            out_path.to_str().expect("out_path utf-8"),
            "bundle",
            "--program",
            program_path.to_str().expect("program_path utf-8"),
            "--module-root",
            module_root.to_str().expect("module_root utf-8"),
        ])
        .output()
        .expect("x07 bundle");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out_path.is_file(),
        "missing bundled binary: {}",
        out_path.display()
    );

    let mut child = Command::new(&out_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bundled binary");
    child
        .stdin
        .as_mut()
        .expect("take stdin")
        .write_all(b"hello\n")
        .expect("write stdin");
    drop(child.stdin.take());
    let run_out = child.wait_with_output().expect("wait bundled binary");

    assert_eq!(
        run_out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_out.stdout),
        String::from_utf8_lossy(&run_out.stderr)
    );
    assert_eq!(run_out.stdout, b"ok\ndone");
    assert_eq!(run_out.stderr, b"log\n");
}

#[test]
fn bundle_project_program_override_uses_override_entry() {
    let root = repo_root();
    let tmp = fresh_tmp_dir(&root, "bundle_project_program_override");
    std::fs::create_dir_all(tmp.join("src")).expect("create src dir");

    write_json(
        &tmp.join("x07.json"),
        &serde_json::json!({
            "schema_version": "x07.project@0.4.0",
            "world": "solve-pure",
            "entry": "src/main.x07.json",
            "module_roots": ["src"],
            "dependencies": []
        }),
    );
    write_json(
        &tmp.join("src/main.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [],
            "solve": ["bytes.lit", "main"]
        }),
    );
    write_json(
        &tmp.join("src/worker_main.x07.json"),
        &serde_json::json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "worker_main",
            "imports": [],
            "decls": [],
            "solve": ["bytes.lit", "worker"]
        }),
    );

    let out_path = tmp.join("worker_bundle");
    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&tmp)
        .args([
            "--out",
            out_path.to_str().expect("out_path utf-8"),
            "bundle",
            "--project",
            "x07.json",
            "--program",
            "src/worker_main.x07.json",
        ])
        .output()
        .expect("x07 bundle with project program override");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out_path.is_file(),
        "missing bundled binary: {}",
        out_path.display()
    );

    let run_out = Command::new(&out_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundled binary");
    assert_eq!(
        run_out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_out.stdout),
        String::from_utf8_lossy(&run_out.stderr)
    );
    assert_eq!(run_out.stdout, b"worker");
    assert!(run_out.stderr.is_empty(), "expected empty stderr");
}

#[test]
fn bundle_jsonschema_smoke_validates_ok_and_err() {
    ensure_mcp_native_backends_staged();

    let root = repo_root();
    let tmp = fresh_tmp_dir(&root, "bundle_jsonschema_smoke");
    std::fs::create_dir_all(&tmp).expect("create tmp dir");

    let out_path = tmp.join("app_jsonschema_smoke");
    let program_path = root.join("tests/external_os/jsonschema_smoke_ok/src/main.x07.json");
    let module_root = root.join("packages/ext/x07-ext-jsonschema-rs/0.1.0/modules");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&root)
        .args([
            "--out",
            out_path.to_str().expect("out_path utf-8"),
            "bundle",
            "--program",
            program_path.to_str().expect("program_path utf-8"),
            "--module-root",
            module_root.to_str().expect("module_root utf-8"),
        ])
        .output()
        .expect("x07 bundle");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out_path.is_file(),
        "missing bundled binary: {}",
        out_path.display()
    );

    let run_out = Command::new(&out_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundled binary");
    assert_eq!(
        run_out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run_out.stdout),
        String::from_utf8_lossy(&run_out.stderr)
    );
    assert!(run_out.stderr.is_empty(), "expected empty stderr");
    assert_eq!(run_out.stdout, b"OK");
}

#[test]
fn bundle_rand_smoke_outputs_32_bytes() {
    ensure_mcp_native_backends_staged();

    let root = repo_root();
    let tmp = fresh_tmp_dir(&root, "bundle_rand_smoke");
    std::fs::create_dir_all(&tmp).expect("create tmp dir");

    let out_path = tmp.join("app_rand_smoke");
    let program_path = root.join("tests/external_os/rand_smoke_ok/src/main.x07.json");
    let module_root = root.join("packages/ext/x07-ext-rand/0.1.0/modules");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&root)
        .args([
            "--out",
            out_path.to_str().expect("out_path utf-8"),
            "bundle",
            "--program",
            program_path.to_str().expect("program_path utf-8"),
            "--module-root",
            module_root.to_str().expect("module_root utf-8"),
        ])
        .output()
        .expect("x07 bundle");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out_path.is_file(),
        "missing bundled binary: {}",
        out_path.display()
    );

    let run1 = Command::new(&out_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundled binary");
    assert_eq!(
        run1.status.code(),
        Some(0),
        "stdout.len={} stderr:\n{}",
        run1.stdout.len(),
        String::from_utf8_lossy(&run1.stderr)
    );
    assert!(run1.stderr.is_empty(), "expected empty stderr");
    assert_eq!(run1.stdout.len(), 32, "expected 32 random bytes");

    if run1.stdout.iter().all(|b| *b == 0) {
        let run2 = Command::new(&out_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run bundled binary (retry)");
        assert_eq!(run2.status.code(), Some(0));
        assert!(run2.stderr.is_empty(), "expected empty stderr");
        assert_eq!(run2.stdout.len(), 32, "expected 32 random bytes");
        assert!(
            !run2.stdout.iter().all(|b| *b == 0),
            "unexpected all-zero bytes"
        );
    }
}

#[test]
fn bundle_rand_caps_enforces_bounds() {
    ensure_mcp_native_backends_staged();

    let root = repo_root();
    let tmp = fresh_tmp_dir(&root, "bundle_rand_caps_bounds");
    std::fs::create_dir_all(&tmp).expect("create tmp dir");

    let out_path = tmp.join("app_rand_caps_bounds");
    let program_path = root.join("tests/external_os/rand_caps_bounds/src/main.x07.json");
    let module_root = root.join("packages/ext/x07-ext-rand/0.1.0/modules");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&root)
        .args([
            "--out",
            out_path.to_str().expect("out_path utf-8"),
            "bundle",
            "--program",
            program_path.to_str().expect("program_path utf-8"),
            "--module-root",
            module_root.to_str().expect("module_root utf-8"),
        ])
        .output()
        .expect("x07 bundle");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out_path.is_file(),
        "missing bundled binary: {}",
        out_path.display()
    );

    let run_out = Command::new(&out_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundled binary");
    assert_eq!(
        run_out.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&run_out.stderr)
    );
    assert!(run_out.stderr.is_empty(), "expected empty stderr");
    assert_eq!(run_out.stdout, b"OK");
}

#[test]
fn bundle_sandboxed_env_overrides_allow_policy_override_but_cannot_disable_sandbox() {
    let root = repo_root();
    let tmp = fresh_tmp_dir(&root, "bundle_sandboxed_env_override");
    std::fs::create_dir_all(&tmp).expect("create tmp dir");

    let out_path = tmp.join("app_sandboxed_env_override");
    let program_path = root.join("tests/external_os/bundle_sandbox_env_override/src/main.x07.json");
    let policy_path =
        root.join("tests/external_os/bundle_sandbox_env_override/run-os-policy.base.json");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&root)
        .env(ENV_SANDBOX_BACKEND, "os")
        .env(ENV_ACCEPT_WEAKER_ISOLATION, "1")
        .args([
            "--out",
            out_path.to_str().expect("out_path utf-8"),
            "bundle",
            "--program",
            program_path.to_str().expect("program_path utf-8"),
            "--module-root",
            root.join("stdlib/std/0.1.2/modules")
                .to_str()
                .expect("module_root utf-8"),
            "--world",
            "run-os-sandboxed",
            "--policy",
            policy_path.to_str().expect("policy_path utf-8"),
            "--sandbox-backend",
            "os",
            "--i-accept-weaker-isolation",
        ])
        .output()
        .expect("x07 bundle");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out_path.is_file(),
        "missing bundled binary: {}",
        out_path.display()
    );

    write_bytes(&tmp.join("ok.txt"), b"hello\n");

    let run0 = Command::new(&out_path)
        .current_dir(&tmp)
        .env_remove("X07_WORLD")
        .env_remove("X07_OS_SANDBOXED")
        .env_remove("X07_OS_FS")
        .env_remove("X07_OS_FS_READ_ROOTS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundled binary (base policy)");
    assert_ne!(
        run0.status.code(),
        Some(0),
        "expected base policy to deny fs; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run0.stdout),
        String::from_utf8_lossy(&run0.stderr)
    );

    let run1 = Command::new(&out_path)
        .current_dir(&tmp)
        .env_remove("X07_OS_FS")
        .env_remove("X07_OS_FS_READ_ROOTS")
        .env("X07_WORLD", "run-os")
        .env("X07_OS_SANDBOXED", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundled binary (attempt disable sandbox)");
    assert_ne!(
        run1.status.code(),
        Some(0),
        "expected wrapper to force sandbox on; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run1.stdout),
        String::from_utf8_lossy(&run1.stderr)
    );

    let run2 = Command::new(&out_path)
        .current_dir(&tmp)
        .env_remove("X07_WORLD")
        .env_remove("X07_OS_SANDBOXED")
        .env("X07_OS_FS", "1")
        .env("X07_OS_FS_READ_ROOTS", ".")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundled binary (fs override)");
    assert_eq!(
        run2.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&run2.stderr)
    );
    assert!(run2.stderr.is_empty(), "expected empty stderr");
    assert_eq!(run2.stdout, b"OK");
}

#[cfg(unix)]
#[test]
fn x07_mcp_delegates_to_x07_mcp_and_forwards_exit_code() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = fresh_os_tmp_dir("x07_mcp_delegate");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let bin_dir = dir.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");

    let stub = bin_dir.join("x07-mcp");
    let stub_src = r#"#!/usr/bin/env python3
import json
import sys

print(json.dumps({"args": sys.argv[1:]}))
sys.exit(17)
"#;
    write_bytes(&stub, stub_src.as_bytes());
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod x07-mcp");

    let exe = env!("CARGO_BIN_EXE_x07");
    let mut cmd = Command::new(exe);
    cmd.current_dir(&dir);
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir.clone()];
    paths.extend(std::env::split_paths(&existing));
    cmd.env("PATH", std::env::join_paths(paths).expect("join PATH"));
    cmd.args([
        "mcp",
        "scaffold",
        "init",
        "--template",
        "mcp-server-stdio",
        "--dir",
        "/tmp/example",
    ]);
    let out = cmd.output().expect("run x07 mcp");

    assert_eq!(
        out.status.code(),
        Some(17),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).expect("parse stub JSON");
    assert_eq!(
        v["args"],
        Value::Array(
            [
                "scaffold",
                "init",
                "--template",
                "mcp-server-stdio",
                "--dir",
                "/tmp/example"
            ]
            .into_iter()
            .map(|s| Value::String(s.to_string()))
            .collect()
        )
    );
}

#[cfg(unix)]
#[test]
fn x07_mcp_errors_when_x07_mcp_missing() {
    let dir = fresh_os_tmp_dir("x07_mcp_missing");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let empty_path = dir.join("empty_path");
    std::fs::create_dir_all(&empty_path).expect("create empty PATH dir");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&dir)
        .env("PATH", empty_path.to_str().unwrap())
        .args(["mcp", "--", "anything"])
        .output()
        .expect("run x07 mcp");

    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("x07-mcp not found"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[cfg(unix)]
#[test]
fn x07_init_mcp_templates_delegate_to_x07_mcp_scaffold() {
    use std::os::unix::fs::PermissionsExt as _;

    for template in ["mcp-server-stdio", "mcp-server-http-tasks"] {
        let dir = fresh_os_tmp_dir(&format!(
            "x07_init_mcp_delegate_{}",
            template.replace('-', "_")
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let bin_dir = dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("create bin dir");

        let stub = bin_dir.join("x07-mcp");
        let stub_src = r#"#!/usr/bin/env python3
import json
import os
import sys

def arg_value(argv, key):
    for i, tok in enumerate(argv):
        if tok == key and i + 1 < len(argv):
            return argv[i + 1]
    return None

argv = sys.argv[1:]
if argv[:2] != ["scaffold", "init"]:
    print(json.dumps({"ok": False, "error": {"message": "unexpected argv"}}))
    sys.exit(3)

tpl = arg_value(argv, "--template")
dst = arg_value(argv, "--dir")
ver = arg_value(argv, "--toolchain-version")
mach = arg_value(argv, "--machine")

if not tpl or not dst or not ver or mach != "json":
    print(json.dumps({"ok": False, "error": {"message": "missing required args"}}))
    sys.exit(4)

os.makedirs(dst, exist_ok=True)
with open(os.path.join(dst, "x07.json"), "w", encoding="utf-8") as f:
    f.write("{}\n")
with open(os.path.join(dst, ".toolchain-version"), "w", encoding="utf-8") as f:
    f.write(ver + "\n")
with open(os.path.join(dst, ".template"), "w", encoding="utf-8") as f:
    f.write(tpl + "\n")

print(json.dumps({"ok": True, "created": ["x07.json", ".toolchain-version"], "next_steps": []}))
sys.exit(0)
"#;
        write_bytes(&stub, stub_src.as_bytes());
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
            .expect("chmod x07-mcp");

        let exe = env!("CARGO_BIN_EXE_x07");
        let mut cmd = Command::new(exe);
        cmd.current_dir(&dir);
        let existing = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![bin_dir.clone()];
        paths.extend(std::env::split_paths(&existing));
        cmd.env("PATH", std::env::join_paths(paths).expect("join PATH"));
        cmd.args(["init", "--template", template]);
        let out = cmd.output().expect("run x07 init");

        assert_eq!(
            out.status.code(),
            Some(0),
            "template={template} stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stderr.is_empty(),
            "template={template} expected empty stderr, got:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report = parse_json_stdout(&out);
        assert_eq!(report["ok"], Value::from(true));
        assert!(
            dir.join("x07.json").is_file(),
            "template={template}: x07.json missing"
        );
        assert!(
            dir.join(".toolchain-version").is_file(),
            "template={template}: .toolchain-version missing"
        );
        assert!(
            dir.join(".template").is_file(),
            "template={template}: .template missing"
        );
        assert!(
            dir.join(".x07")
                .join("policies")
                .join("base")
                .join("worker.sandbox.base.policy.json")
                .is_file(),
            "template={template}: worker base policy missing"
        );

        let ver =
            std::fs::read_to_string(dir.join(".toolchain-version")).expect("read version file");
        assert_eq!(ver.trim(), env!("CARGO_PKG_VERSION"));
        let recorded_template =
            std::fs::read_to_string(dir.join(".template")).expect("read template file");
        assert_eq!(recorded_template.trim(), template);
    }
}

#[cfg(unix)]
#[test]
fn x07_init_mcp_template_errors_when_x07_mcp_missing() {
    let dir = fresh_os_tmp_dir("x07_init_mcp_missing");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let empty_path = dir.join("empty_path");
    std::fs::create_dir_all(&empty_path).expect("create empty PATH dir");

    let exe = env!("CARGO_BIN_EXE_x07");
    let out = Command::new(exe)
        .current_dir(&dir)
        .env("PATH", empty_path.to_str().unwrap())
        .args(["init", "--template", "mcp-server-stdio"])
        .output()
        .expect("run x07 init");

    assert_eq!(
        out.status.code(),
        Some(20),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = parse_json_stdout(&out);
    assert_eq!(report["ok"], Value::from(false));
    assert_eq!(report["error"]["code"], Value::from("X07INIT_MCP_MISSING"));
}

#[test]
fn x07_explain_works_from_installed_toolchain_layout() {
    let toolchain_root = fresh_os_tmp_dir("x07_toolchain_root");
    let project_root = fresh_os_tmp_dir("x07_project_root");

    std::fs::create_dir_all(toolchain_root.join("bin")).expect("create toolchain bin dir");
    std::fs::create_dir_all(toolchain_root.join("catalog")).expect("create toolchain catalog dir");
    std::fs::create_dir_all(&project_root).expect("create project root dir");

    let src_exe = PathBuf::from(env!("CARGO_BIN_EXE_x07"));
    let exe_name = src_exe.file_name().expect("exe filename");
    let dst_exe = toolchain_root.join("bin").join(exe_name);
    std::fs::copy(&src_exe, &dst_exe).expect("copy x07 binary");
    #[cfg(unix)]
    {
        let mut perms = std::fs::metadata(&dst_exe)
            .expect("stat x07 binary")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dst_exe, perms).expect("chmod x07 binary");
    }

    let root = repo_root();
    std::fs::copy(root.join("stdlib.lock"), toolchain_root.join("stdlib.lock"))
        .expect("copy stdlib.lock");
    std::fs::copy(
        root.join("catalog").join("diagnostics.json"),
        toolchain_root.join("catalog").join("diagnostics.json"),
    )
    .expect("copy diagnostics catalog");

    let out = Command::new(&dst_exe)
        .current_dir(&project_root)
        .args(["explain", "X07-TOOL-EXEC-0001"])
        .output()
        .expect("run x07 explain");

    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("code: X07-TOOL-EXEC-0001"),
        "unexpected stdout:\n{stdout}"
    );
}
