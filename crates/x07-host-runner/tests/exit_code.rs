use std::path::PathBuf;
use std::process::Command;

use serde_json::json;
use x07_host_runner::{
    compile_bundle_exe, compile_options_for_world, compile_program, run_artifact_file,
    NativeCliWrapperOpts, NativeToolchainConfig, RunnerConfig,
};
use x07_worlds::WorldId;

mod x07_program;

fn config() -> RunnerConfig {
    RunnerConfig {
        world: WorldId::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 20,
        debug_borrow_checks: false,
    }
}

fn make_temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let pid = std::process::id();
    for n in 0..10_000u32 {
        let p = base.join(format!("x07-exit-code-{prefix}-{pid}-{n}"));
        if std::fs::create_dir(&p).is_ok() {
            return p;
        }
    }
    panic!("failed to create temp dir under {}", base.display());
}

#[test]
fn process_set_exit_code_v1_propagates_to_exit_status() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["process.set_exit_code_v1", 7],
            ["bytes.lit", "ok"]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"ignored").expect("runner ok");
    assert!(!res.ok, "expected ok=false for nonzero exit code");
    assert!(res.trap.is_none(), "trap={:?}", res.trap);
    assert_eq!(res.exit_status, 7);
    assert_eq!(res.solve_output, b"ok");
}

#[test]
fn bundle_wrapper_returns_set_exit_code() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["process.set_exit_code_v1", 7],
            ["bytes.lit", "ok"]
        ]),
    );

    let compile_options =
        compile_options_for_world(cfg.world, Vec::new()).expect("compile options");
    let toolchain = NativeToolchainConfig {
        world_tag: compile_options.world.as_str().to_string(),
        fuel_init: cfg.solve_fuel,
        mem_cap_bytes: cfg.max_memory_bytes,
        debug_borrow_checks: cfg.debug_borrow_checks,
        enable_fs: compile_options.enable_fs,
        enable_rr: compile_options.enable_rr,
        enable_kv: compile_options.enable_kv,
        extra_cc_args: Vec::new(),
    };

    let dir = make_temp_dir("bundle");
    let exe_path = dir.join(if cfg!(windows) { "app.exe" } else { "app" });
    let wrapper = NativeCliWrapperOpts {
        argv0: "app".to_string(),
        env: Vec::new(),
        max_output_bytes: Some(1024 * 1024),
        cpu_time_limit_seconds: Some(20),
    };

    let out = compile_bundle_exe(
        program.as_slice(),
        &compile_options,
        &toolchain,
        &exe_path,
        &wrapper,
    )
    .expect("compile bundle ok");
    assert!(
        out.compile.ok,
        "compile_error={:?}",
        out.compile.compile_error
    );

    let run = Command::new(&exe_path).output().expect("run bundle exe");
    assert_eq!(run.status.code(), Some(7));
    assert_eq!(run.stdout, b"ok");

    let _ = std::fs::remove_dir_all(&dir);
}
