use serde_json::json;
use std::path::PathBuf;
use x07_host_runner::{compile_program, run_artifact_file, RunnerConfig};
use x07_worlds::WorldId;

mod x07_program;

#[test]
fn solve_rr_fetches_response_fixture() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/fixtures/bench/rr/solve-rr/stdlib-parity-suite-rr@0.1.0");

    let cfg = RunnerConfig {
        world: WorldId::SolveRr,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: Some(fixture.clone()),
        fixture_rr_index: Some(PathBuf::from("index.json")),
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry(
        &["std.rr"],
        json!(["std.rr.fetch", ["view.to_bytes", "input"]]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let input = b"U7";
    let res = run_artifact_file(&cfg, &exe, input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );

    let want_bytes = std::fs::read(fixture.join("bodies/U7.json")).expect("read fixture body");

    assert_eq!(res.solve_output, want_bytes);
    assert_eq!(res.rr_send_calls, Some(0));
    assert_eq!(res.rr_request_calls, Some(1));
}

#[test]
fn solve_rr_defaults_fixture_index_json_when_index_not_specified() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/fixtures/bench/rr/solve-rr/stdlib-parity-suite-rr@0.1.0");

    let cfg = RunnerConfig {
        world: WorldId::SolveRr,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: Some(fixture.clone()),
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
        &["std.rr"],
        json!(["std.rr.fetch", ["view.to_bytes", "input"]]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let input = b"U7";
    let res = run_artifact_file(&cfg, &exe, input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );

    let want_bytes = std::fs::read(fixture.join("bodies/U7.json")).expect("read fixture body");
    assert_eq!(res.solve_output, want_bytes);
}
