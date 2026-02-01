use serde_json::json;
use std::path::PathBuf;
use x07_host_runner::{
    compile_options_for_world, compile_program_with_options, run_artifact_file, RunnerConfig,
};
use x07_worlds::WorldId;

mod x07_program;

#[test]
fn solve_rr_fetches_response_fixture() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/rr_smoke");
    let arch_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let cfg = RunnerConfig {
        world: WorldId::SolveRr,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: Some(fixture.clone()),
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry_with_decls(
        &["std.rr"],
        vec![x07_program::defn(
            "main.run",
            &[("key", "bytes_view")],
            "result_bytes",
            json!([
                "std.rr.with_policy_v1",
                ["bytes.lit", "smoke_rr_v1"],
                ["bytes.lit", "smoke.rrbin"],
                ["i32.lit", 2],
                [
                    "begin",
                    ["let", "h", ["std.rr.current_v1"]],
                    [
                        "let",
                        "entry",
                        [
                            "try",
                            [
                                "std.rr.next_v1",
                                "h",
                                ["bytes.lit", "rr"],
                                ["bytes.lit", "std.rr.fetch_v1"],
                                "key"
                            ]
                        ]
                    ],
                    [
                        "let",
                        "resp",
                        ["std.rr.entry_resp_v1", ["bytes.view", "entry"]]
                    ],
                    ["result_bytes.ok", "resp"]
                ]
            ]),
        )],
        json!([
            "begin",
            [
                "let",
                "out",
                [
                    "result_bytes.unwrap_or",
                    ["main.run", "input"],
                    ["bytes.alloc", 0]
                ]
            ],
            "out"
        ]),
    );

    let mut compile_options =
        compile_options_for_world(cfg.world, Vec::new()).expect("compile options");
    compile_options.arch_root = Some(arch_root);
    let compile = compile_program_with_options(
        program.as_slice(),
        &cfg,
        None,
        &compile_options,
        &[] as &[String],
    )
    .expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let input = b"C";
    let res = run_artifact_file(&cfg, &exe, input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );

    assert_eq!(res.solve_output, b"PONG");
    assert_eq!(res.rr_open_calls, Some(1));
    assert_eq!(res.rr_close_calls, Some(1));
    assert_eq!(res.rr_next_calls, Some(1));
    assert_eq!(res.rr_next_miss_calls, Some(0));
}

#[test]
fn solve_rr_miss_increments_metric() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/rr_smoke");
    let arch_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let cfg = RunnerConfig {
        world: WorldId::SolveRr,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: Some(fixture.clone()),
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 10_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let program = x07_program::entry_with_decls(
        &["std.rr"],
        vec![x07_program::defn(
            "main.run",
            &[("key", "bytes_view")],
            "result_bytes",
            json!([
                "std.rr.with_policy_v1",
                ["bytes.lit", "smoke_rr_v1"],
                ["bytes.lit", "smoke.rrbin"],
                ["i32.lit", 2],
                [
                    "begin",
                    ["let", "h", ["std.rr.current_v1"]],
                    [
                        "let",
                        "entry",
                        [
                            "try",
                            [
                                "std.rr.next_v1",
                                "h",
                                ["bytes.lit", "rr"],
                                ["bytes.lit", "std.rr.fetch_v1"],
                                "key"
                            ]
                        ]
                    ],
                    [
                        "let",
                        "resp",
                        ["std.rr.entry_resp_v1", ["bytes.view", "entry"]]
                    ],
                    ["result_bytes.ok", "resp"]
                ]
            ]),
        )],
        json!([
            "begin",
            [
                "let",
                "out",
                [
                    "result_bytes.unwrap_or",
                    ["main.run", "input"],
                    ["bytes.alloc", 0]
                ]
            ],
            "out"
        ]),
    );

    let mut compile_options =
        compile_options_for_world(cfg.world, Vec::new()).expect("compile options");
    compile_options.arch_root = Some(arch_root);
    let compile = compile_program_with_options(
        program.as_slice(),
        &cfg,
        None,
        &compile_options,
        &[] as &[String],
    )
    .expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let input = b"Z";
    let res = run_artifact_file(&cfg, &exe, input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );

    assert_eq!(res.solve_output, b"");
    assert_eq!(res.rr_open_calls, Some(1));
    assert_eq!(res.rr_close_calls, Some(1));
    assert_eq!(res.rr_next_calls, Some(1));
    assert_eq!(res.rr_next_miss_calls, Some(1));
}
