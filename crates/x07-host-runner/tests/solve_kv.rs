use serde_json::json;
use std::path::PathBuf;
use x07_host_runner::{compile_program, run_artifact_file, RunnerConfig};
use x07_worlds::WorldId;

mod x07_program;

#[test]
fn solve_kv_reads_from_seeded_store() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/fixtures/bench/kv/solve-kv/stdlib-parity-suite-kv@0.1.0");

    let cfg = RunnerConfig {
        world: WorldId::SolveKv,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_rr_index: None,
        fixture_kv_dir: Some(fixture),
        fixture_kv_seed: Some(PathBuf::from("seed.json")),
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry(
        &["std.kv"],
        json!(["std.kv.get", ["view.to_bytes", "input"]]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"PING").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"PONG");
    assert_eq!(res.kv_get_calls, Some(1));
    assert_eq!(res.kv_set_calls, Some(0));
}
