use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use x07_host_runner::{compile_program, run_artifact_file, RunnerConfig};
use x07_worlds::WorldId;

mod x07_program;

fn create_temp_dir(prefix: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    let pid = std::process::id();
    for _ in 0..10_000 {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!("{prefix}_{pid}_{n}"));
        if std::fs::create_dir(&path).is_ok() {
            return path;
        }
    }
    panic!("failed to create temp dir under {}", base.display());
}

fn rm_rf(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn solve_fs_can_read_fixture_file() {
    let fixture = create_temp_dir("x07_fixture");
    std::fs::write(fixture.join("config.bin"), b"\x01\x02\x03").expect("write fixture file");

    let cfg = RunnerConfig {
        world: WorldId::SolveFs,
        fixture_fs_dir: Some(fixture.clone()),
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry(&[], json!(["fs.read", ["bytes.lit", "config.bin"]]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"\x01\x02\x03");

    rm_rf(&fixture);
}

#[test]
fn solve_fs_rejects_absolute_paths() {
    let fixture = create_temp_dir("x07_fixture");
    std::fs::write(fixture.join("config.bin"), b"\x00").expect("write fixture file");

    let cfg = RunnerConfig {
        world: WorldId::SolveFs,
        fixture_fs_dir: Some(fixture.clone()),
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry(&[], json!(["fs.read", ["bytes.lit", "/etc/passwd"]]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok);
    assert!(String::from_utf8_lossy(&res.stderr).contains("fs.read unsafe path"));

    rm_rf(&fixture);
}

#[test]
fn solve_fs_can_list_dir_sorted() {
    let fixture = create_temp_dir("x07_fixture");
    std::fs::create_dir(fixture.join("cfg")).expect("create cfg dir");
    std::fs::write(fixture.join("cfg").join("b.txt"), b"b").expect("write b.txt");
    std::fs::write(fixture.join("cfg").join("a.txt"), b"a").expect("write a.txt");

    let cfg = RunnerConfig {
        world: WorldId::SolveFs,
        fixture_fs_dir: Some(fixture.clone()),
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry(&[], json!(["fs.list_dir", ["bytes.lit", "cfg"]]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"a.txt\nb.txt\n");
    assert_eq!(res.fs_list_dir_calls, Some(1));

    rm_rf(&fixture);
}

#[test]
fn solve_fs_list_dir_rejects_absolute_paths() {
    let fixture = create_temp_dir("x07_fixture");
    std::fs::create_dir(fixture.join("cfg")).expect("create cfg dir");
    std::fs::write(fixture.join("cfg").join("a.txt"), b"a").expect("write a.txt");

    let cfg = RunnerConfig {
        world: WorldId::SolveFs,
        fixture_fs_dir: Some(fixture.clone()),
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry(&[], json!(["fs.list_dir", ["bytes.lit", "/etc"]]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok);
    assert!(String::from_utf8_lossy(&res.stderr).contains("fs.list_dir unsafe path"));

    rm_rf(&fixture);
}

#[test]
fn builtin_fs_module_can_be_imported() {
    let fixture = create_temp_dir("x07_fixture");
    std::fs::write(fixture.join("config.bin"), b"\x01\x02\x03").expect("write fixture file");

    let cfg = RunnerConfig {
        world: WorldId::SolveFs,
        fixture_fs_dir: Some(fixture.clone()),
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry(
        &["std.fs"],
        json!(["std.fs.read", ["bytes.lit", "config.bin"]]),
    );
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"\x01\x02\x03");

    rm_rf(&fixture);
}
