use serde_json::json;
use x07_host_runner::{compile_program, run_artifact_file, RunnerConfig};
use x07_worlds::WorldId;

mod x07_program;

fn cfg() -> RunnerConfig {
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
        cpu_time_limit_seconds: 10,
        debug_borrow_checks: false,
    }
}

#[test]
fn awaiting_join_survives_task_array_reallocations() {
    let cfg = cfg();

    let program = x07_program::entry_with_decls(
        &[],
        vec![
            x07_program::defasync("main.noop", &[], "bytes", json!(["bytes.alloc", 0])),
            x07_program::defasync(
                "main.child_yield",
                &[],
                "bytes",
                json!(["begin", ["task.yield"], ["bytes.lit", "ok"]]),
            ),
            x07_program::defasync(
                "main.parent",
                &[],
                "bytes",
                json!([
                    "begin",
                    [
                        "for",
                        "i",
                        0,
                        40,
                        [
                            "begin",
                            ["let", "t", ["main.noop"]],
                            ["task.spawn", "t"],
                            ["task.cancel", "t"],
                            0
                        ]
                    ],
                    ["let", "h", ["main.child_yield"]],
                    ["task.spawn", "h"],
                    ["await", "h"]
                ]),
            ),
        ],
        json!([
            "begin",
            ["let", "t", ["task.spawn", ["main.parent"]]],
            ["task.join.bytes", "t"]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(
        compile.ok,
        "compile_error={:?}\nstdout:\n{}\nstderr:\n{}",
        compile.compile_error,
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, b"ok");
}
