use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
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

fn cc_command() -> (OsString, Vec<String>) {
    let cc = std::env::var_os("X07_CC").unwrap_or_else(|| OsStr::new("cc").to_os_string());
    let args = std::env::var("X07_CC_ARGS")
        .unwrap_or_default()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    (cc, args)
}

fn compile_c_artifact(source: &str) -> (PathBuf, PathBuf) {
    let dir = create_temp_dir("x07_test_c");
    let src_path = dir.join("prog.c");
    let mut exe_path = dir.join("prog");
    if cfg!(windows) {
        exe_path.set_extension("exe");
    }
    std::fs::write(&src_path, source).expect("write C source");

    let (cc, cc_args) = cc_command();
    let out = Command::new(&cc)
        .args(cc_args)
        .arg("-std=c99")
        .arg(&src_path)
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("invoke cc");
    assert!(
        out.status.success(),
        "cc failed\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    (dir, exe_path)
}

fn base_config() -> RunnerConfig {
    RunnerConfig {
        world: WorldId::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_rr_index: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    }
}

#[test]
fn success_without_metrics_is_failure() {
    let (dir, exe) = compile_c_artifact(
        r#"
          #include <stdint.h>
          #include <stdio.h>

          int main(void) {
            uint8_t buf[7];
            uint32_t len = 3;
            buf[0] = (uint8_t)(len & 0xFF);
            buf[1] = (uint8_t)((len >> 8) & 0xFF);
            buf[2] = (uint8_t)((len >> 16) & 0xFF);
            buf[3] = (uint8_t)((len >> 24) & 0xFF);
            buf[4] = 'a';
            buf[5] = 'b';
            buf[6] = 'c';
            fwrite(buf, 1, sizeof(buf), stdout);
            fflush(stdout);
            return 0;
          }
        "#,
    );

    let cfg = base_config();

    let res = run_artifact_file(&cfg, &exe, b"ignored").expect("runner ok");
    assert!(!res.ok);
    assert_eq!(res.exit_status, 0);
    assert_eq!(res.solve_output, b"abc");
    assert_eq!(
        res.trap.as_deref(),
        Some("missing metrics json line on stderr")
    );

    rm_rf(&dir);
}

#[test]
fn fake_metrics_json_is_rejected() {
    let (dir, exe) = compile_c_artifact(
        r#"
          #include <stdint.h>
          #include <stdio.h>

          int main(void) {
            uint8_t buf[7];
            uint32_t len = 3;
            buf[0] = (uint8_t)(len & 0xFF);
            buf[1] = (uint8_t)((len >> 8) & 0xFF);
            buf[2] = (uint8_t)((len >> 16) & 0xFF);
            buf[3] = (uint8_t)((len >> 24) & 0xFF);
            buf[4] = 'a';
            buf[5] = 'b';
            buf[6] = 'c';
            fwrite(buf, 1, sizeof(buf), stdout);
            fputs("{}\n", stderr);
            fflush(stdout);
            fflush(stderr);
            return 0;
          }
        "#,
    );

    let cfg = base_config();

    let res = run_artifact_file(&cfg, &exe, b"ignored").expect("runner ok");
    assert!(!res.ok);
    assert_eq!(res.exit_status, 0);
    assert_eq!(res.solve_output, b"abc");
    assert_eq!(
        res.trap.as_deref(),
        Some("missing metrics json line on stderr")
    );

    rm_rf(&dir);
}

#[test]
fn wall_timeout_kills_blocked_process() {
    let (dir, exe) = compile_c_artifact(
        r#"
          #include <unistd.h>

          int main(void) {
            for (;;) {
              sleep(1);
            }
          }
        "#,
    );

    let mut cfg = base_config();
    cfg.cpu_time_limit_seconds = 1;

    let res = run_artifact_file(&cfg, &exe, b"ignored").expect("runner ok");
    assert!(!res.ok);
    assert_eq!(res.trap.as_deref(), Some("wall timeout"));
    assert_ne!(res.exit_status, 0);

    rm_rf(&dir);
}

#[test]
fn stdout_cap_does_not_hang() {
    let (dir, exe) = compile_c_artifact(
        r#"
          #include <stdint.h>
          #include <stdio.h>

          int main(void) {
            static uint8_t buf[65536];
            for (uint32_t i = 0; i < (uint32_t)sizeof(buf); i++) buf[i] = (uint8_t)i;
            for (int i = 0; i < 32; i++) {
              fwrite(buf, 1, sizeof(buf), stdout);
            }
            fflush(stdout);
            return 0;
          }
        "#,
    );

    let mut cfg = base_config();
    cfg.cpu_time_limit_seconds = 5;
    cfg.max_output_bytes = 64;

    let res = run_artifact_file(&cfg, &exe, b"ignored").expect("runner ok");
    assert!(!res.ok);
    assert_eq!(res.exit_status, 0);
    assert_eq!(res.trap.as_deref(), Some("stdout exceeded cap"));

    rm_rf(&dir);
}

#[test]
fn fs_read_rejects_reserved_x07_dirs() {
    let fixture = create_temp_dir("x07_fixture");

    let cfg = RunnerConfig {
        world: WorldId::SolveFs,
        fixture_fs_dir: Some(fixture.clone()),
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_rr_index: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "v", ["vec_u8.with_capacity", 0]],
            ["set", "v", ["vec_u8.push", "v", 46]],
            [
                "set",
                "v",
                ["vec_u8.extend_bytes", "v", ["bytes.lit", "x07_rr"]]
            ],
            ["set", "v", ["vec_u8.push", "v", 47]],
            [
                "set",
                "v",
                ["vec_u8.extend_bytes", "v", ["bytes.lit", "responses"]]
            ],
            ["set", "v", ["vec_u8.push", "v", 47]],
            [
                "set",
                "v",
                ["vec_u8.extend_bytes", "v", ["bytes.lit", "x.bin"]]
            ],
            ["fs.read", ["vec_u8.into_bytes", "v"]]
        ]),
    );
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok);
    assert!(String::from_utf8_lossy(&res.stderr).contains("fs.read unsafe path"));

    rm_rf(&fixture);
}

#[test]
#[cfg(unix)]
fn terminated_by_signal_is_reported_as_trap() {
    let (dir, exe) = compile_c_artifact(
        r#"
          #include <signal.h>

          int main(void) {
            raise(SIGTERM);
          }
        "#,
    );

    let mut cfg = base_config();
    cfg.cpu_time_limit_seconds = 5;

    let res = run_artifact_file(&cfg, &exe, b"ignored").expect("runner ok");
    assert!(!res.ok);
    assert_eq!(res.trap.as_deref(), Some("terminated by signal 15"));

    rm_rf(&dir);
}
