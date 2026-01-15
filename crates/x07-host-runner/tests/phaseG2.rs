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

fn base_cfg(world: WorldId) -> RunnerConfig {
    RunnerConfig {
        world,
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
fn async_bytes_lit_inside_defasync_works() {
    let cfg = base_cfg(WorldId::SolvePure);

    let program = x07_program::entry_with_decls(
        &[],
        vec![x07_program::defasync(
            "main.lit",
            &[],
            "bytes",
            json!(["bytes.lit", "hi"]),
        )],
        json!([
            "begin",
            ["let", "t", ["task.spawn", ["main.lit"]]],
            ["task.join.bytes", "t"]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, b"hi");
}

#[test]
fn try_join_and_channel_try_ops_work_in_defn() {
    let cfg = base_cfg(WorldId::SolvePure);

    let program = x07_program::entry_with_decls(
        &[],
        vec![
            x07_program::defasync("main.ret_empty", &[], "bytes", json!(["bytes.alloc", 0])),
            x07_program::defasync(
                "main.ret_non_empty",
                &[],
                "bytes",
                json!(["bytes.lit", "hi"]),
            ),
            x07_program::defn(
                "main.check_task_not_done",
                &[("t", "i32")],
                "i32",
                json!([
                    "begin",
                    ["let", "finished", ["task.is_finished", "t"]],
                    [
                        "let",
                        "code",
                        ["result_bytes.err_code", ["task.try_join.bytes", "t"]]
                    ],
                    [
                        "if",
                        ["=", "finished", 0],
                        ["if", ["=", "code", 1], 1, 0],
                        0
                    ]
                ]),
            ),
            x07_program::defn(
                "main.check_task_done_empty",
                &[("t", "i32")],
                "i32",
                json!([
                    "begin",
                    ["let", "finished", ["task.is_finished", "t"]],
                    [
                        "let",
                        "out",
                        [
                            "result_bytes.unwrap_or",
                            ["task.try_join.bytes", "t"],
                            ["bytes.lit", "ERR"]
                        ]
                    ],
                    [
                        "if",
                        ["=", "finished", 1],
                        ["if", ["=", ["bytes.len", "out"], 0], 1, 0],
                        0
                    ]
                ]),
            ),
            x07_program::defn(
                "main.check_task_canceled",
                &[("t", "i32")],
                "i32",
                json!([
                    "begin",
                    ["let", "finished", ["task.is_finished", "t"]],
                    [
                        "let",
                        "code",
                        ["result_bytes.err_code", ["task.try_join.bytes", "t"]]
                    ],
                    [
                        "if",
                        ["=", "finished", 1],
                        ["if", ["=", "code", 2], 1, 0],
                        0
                    ]
                ]),
            ),
            x07_program::defn(
                "main.check_chan_try",
                &[],
                "i32",
                json!([
                    "begin",
                    ["let", "ch", ["chan.bytes.new", 1]],
                    [
                        "let",
                        "code0",
                        ["result_bytes.err_code", ["chan.bytes.try_recv", "ch"]]
                    ],
                    ["let", "cond0", ["=", "code0", 1]],
                    [
                        "let",
                        "s1",
                        ["chan.bytes.try_send", "ch", ["bytes.alloc", 0]]
                    ],
                    ["let", "cond1", ["=", "s1", 1]],
                    ["let", "m", ["bytes.lit", "x"]],
                    ["let", "s_full", ["chan.bytes.try_send", "ch", "m"]],
                    ["let", "cond_full", ["=", "s_full", 0]],
                    [
                        "let",
                        "out1",
                        [
                            "result_bytes.unwrap_or",
                            ["chan.bytes.try_recv", "ch"],
                            ["bytes.lit", "ERR"]
                        ]
                    ],
                    ["let", "cond_r1", ["=", ["bytes.len", "out1"], 0]],
                    ["let", "s2", ["chan.bytes.try_send", "ch", "m"]],
                    ["let", "cond2", ["=", "s2", 1]],
                    [
                        "let",
                        "out2",
                        [
                            "result_bytes.unwrap_or",
                            ["chan.bytes.try_recv", "ch"],
                            ["bytes.lit", "ERR"]
                        ]
                    ],
                    ["let", "cond_r2", ["bytes.eq", "out2", ["bytes.lit", "x"]]],
                    ["let", "c1", ["chan.bytes.close", "ch"]],
                    ["let", "cond_close", ["=", "c1", 1]],
                    [
                        "let",
                        "s_closed",
                        ["chan.bytes.try_send", "ch", ["bytes.lit", "y"]]
                    ],
                    ["let", "cond_s_closed", ["=", "s_closed", 2]],
                    [
                        "let",
                        "code_closed",
                        ["result_bytes.err_code", ["chan.bytes.try_recv", "ch"]]
                    ],
                    ["let", "cond_r_closed", ["=", "code_closed", 2]],
                    [
                        "&",
                        "cond0",
                        [
                            "&",
                            "cond1",
                            [
                                "&",
                                "cond_full",
                                [
                                    "&",
                                    "cond_r1",
                                    [
                                        "&",
                                        "cond2",
                                        [
                                            "&",
                                            "cond_r2",
                                            [
                                                "&",
                                                "cond_close",
                                                ["&", "cond_s_closed", "cond_r_closed"]
                                            ]
                                        ]
                                    ]
                                ]
                            ]
                        ]
                    ]
                ]),
            ),
        ],
        json!([
            "begin",
            ["let", "t_empty", ["task.spawn", ["main.ret_empty"]]],
            [
                "let",
                "ok_not_done",
                ["main.check_task_not_done", "t_empty"]
            ],
            ["for", "i", 0, 10, ["task.yield"]],
            [
                "let",
                "ok_done_empty",
                ["main.check_task_done_empty", "t_empty"]
            ],
            ["let", "t_cancel", ["task.spawn", ["main.ret_non_empty"]]],
            ["task.cancel", "t_cancel"],
            [
                "let",
                "ok_canceled",
                ["main.check_task_canceled", "t_cancel"]
            ],
            ["let", "ok_chan", ["main.check_chan_try"]],
            [
                "let",
                "ok_all",
                [
                    "&",
                    ["&", ["&", "ok_not_done", "ok_done_empty"], "ok_canceled"],
                    "ok_chan"
                ]
            ],
            ["if", "ok_all", ["bytes.lit", "ok"], ["bytes.lit", "fail"]]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, b"ok");
}

#[test]
fn solve_fs_read_respects_latency_index() {
    let fixture = create_temp_dir("x07_phaseG2_fs");
    std::fs::create_dir_all(fixture.join("root").join("data")).expect("mkdir data");
    std::fs::write(fixture.join("root/data/a.txt"), b"hello").expect("write a.txt");
    std::fs::write(
        fixture.join("latency.json"),
        br#"{"format":"x07.fs.latency@0.1.0","default_ticks":0,"paths":{"data/a.txt":30}}"#,
    )
    .expect("write latency.json");

    let mut cfg = base_cfg(WorldId::SolveFs);
    cfg.fixture_fs_dir = Some(fixture.clone());
    cfg.fixture_fs_root = Some(PathBuf::from("root"));
    cfg.fixture_fs_latency_index = Some(PathBuf::from("latency.json"));

    let program = x07_program::entry(&[], json!(["fs.read", ["bytes.lit", "data/a.txt"]]));
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
    assert_eq!(res.solve_output, b"hello");
    assert_eq!(res.fs_read_file_calls, Some(1));
    assert_eq!(
        res.sched_stats.as_ref().map(|s| s.virtual_time_end),
        Some(30)
    );

    rm_rf(&fixture);
}

#[test]
fn solve_fs_open_read_and_io_read_respect_latency_index() {
    let fixture = create_temp_dir("x07_phaseG2_fs");
    std::fs::create_dir_all(fixture.join("root").join("data")).expect("mkdir data");
    std::fs::write(fixture.join("root/data/a.txt"), b"hello").expect("write a.txt");
    std::fs::write(
        fixture.join("latency.json"),
        br#"{"format":"x07.fs.latency@0.1.0","default_ticks":0,"paths":{"data/a.txt":30}}"#,
    )
    .expect("write latency.json");

    let mut cfg = base_cfg(WorldId::SolveFs);
    cfg.fixture_fs_dir = Some(fixture.clone());
    cfg.fixture_fs_root = Some(PathBuf::from("root"));
    cfg.fixture_fs_latency_index = Some(PathBuf::from("latency.json"));

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "r", ["fs.open_read", ["bytes.lit", "data/a.txt"]]],
            ["io.read", "r", 100]
        ]),
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
    assert_eq!(res.solve_output, b"hello");
    assert_eq!(res.fs_read_file_calls, Some(1));
    assert_eq!(
        res.sched_stats.as_ref().map(|s| s.virtual_time_end),
        Some(30)
    );

    rm_rf(&fixture);
}

#[test]
fn solve_fs_concurrent_io_read_makespan_is_max_latency() {
    let fixture = create_temp_dir("x07_phaseG2_fs");
    std::fs::create_dir_all(fixture.join("root").join("data")).expect("mkdir data");
    std::fs::write(fixture.join("root/data/a.txt"), b"A").expect("write a.txt");
    std::fs::write(fixture.join("root/data/b.txt"), b"B").expect("write b.txt");
    std::fs::write(
        fixture.join("latency.json"),
        br#"{"format":"x07.fs.latency@0.1.0","default_ticks":0,"paths":{"data/a.txt":30,"data/b.txt":10}}"#,
    )
    .expect("write latency.json");

    let mut cfg = base_cfg(WorldId::SolveFs);
    cfg.fixture_fs_dir = Some(fixture.clone());
    cfg.fixture_fs_root = Some(PathBuf::from("root"));
    cfg.fixture_fs_latency_index = Some(PathBuf::from("latency.json"));

    let program = x07_program::entry_with_decls(
        &["std.bytes"],
        vec![x07_program::defasync(
            "main.read_file",
            &[("path", "bytes")],
            "bytes",
            json!([
                "begin",
                ["let", "r", ["fs.open_read", "path"]],
                ["io.read", "r", 16]
            ]),
        )],
        json!([
            "begin",
            [
                "let",
                "t1",
                [
                    "task.spawn",
                    ["main.read_file", ["bytes.lit", "data/a.txt"]]
                ]
            ],
            [
                "let",
                "t2",
                [
                    "task.spawn",
                    ["main.read_file", ["bytes.lit", "data/b.txt"]]
                ]
            ],
            ["let", "a", ["task.join.bytes", "t1"]],
            ["let", "b", ["task.join.bytes", "t2"]],
            ["std.bytes.concat", "a", "b"]
        ]),
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
    assert_eq!(res.solve_output, b"AB");
    assert_eq!(
        res.sched_stats.as_ref().map(|s| s.virtual_time_end),
        Some(30)
    );
    assert_eq!(res.sched_stats.as_ref().map(|s| s.tasks_spawned), Some(2));

    rm_rf(&fixture);
}

#[test]
fn solve_fs_bufread_fill_and_consume_work() {
    let fixture = create_temp_dir("x07_phaseG2_fs");
    std::fs::create_dir_all(fixture.join("root").join("data")).expect("mkdir data");
    std::fs::write(fixture.join("root/data/a.txt"), b"abc").expect("write a.txt");
    std::fs::write(
        fixture.join("latency.json"),
        br#"{"format":"x07.fs.latency@0.1.0","default_ticks":0,"paths":{"data/a.txt":30}}"#,
    )
    .expect("write latency.json");

    let mut cfg = base_cfg(WorldId::SolveFs);
    cfg.fixture_fs_dir = Some(fixture.clone());
    cfg.fixture_fs_root = Some(PathBuf::from("root"));
    cfg.fixture_fs_latency_index = Some(PathBuf::from("latency.json"));

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "r", ["fs.open_read", ["bytes.lit", "data/a.txt"]]],
            ["let", "br", ["bufread.new", "r", 2]],
            ["let", "v1", ["bufread.fill", "br"]],
            ["let", "a", ["view.get_u8", "v1", 0]],
            ["let", "b", ["view.get_u8", "v1", 1]],
            ["bufread.consume", "br", ["view.len", "v1"]],
            ["let", "v2", ["bufread.fill", "br"]],
            ["let", "c", ["view.get_u8", "v2", 0]],
            ["bufread.consume", "br", ["view.len", "v2"]],
            ["let", "out", ["bytes.alloc", 3]],
            ["set", "out", ["bytes.set_u8", "out", 0, "a"]],
            ["set", "out", ["bytes.set_u8", "out", 1, "b"]],
            ["set", "out", ["bytes.set_u8", "out", 2, "c"]],
            "out"
        ]),
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
    assert_eq!(res.solve_output, b"abc");
    assert_eq!(
        res.sched_stats.as_ref().map(|s| s.virtual_time_end),
        Some(30)
    );

    rm_rf(&fixture);
}

#[test]
fn solve_rr_send_and_io_read_respect_latency_index() {
    let fixture = create_temp_dir("x07_phaseG2_rr");
    std::fs::create_dir_all(fixture.join("bodies")).expect("mkdir bodies");
    std::fs::write(fixture.join("bodies/A.bin"), b"HELLO").expect("write A.bin");
    std::fs::write(
        fixture.join("index.json"),
        br#"{"format":"x07.rr.fixture_index@0.1.0","default_latency_ticks":0,"requests":{"A":{"latency_ticks":50,"body_file":"bodies/A.bin"}}}"#,
    )
    .expect("write index.json");

    let mut cfg = base_cfg(WorldId::SolveRr);
    cfg.fixture_rr_dir = Some(fixture.clone());
    cfg.fixture_rr_index = Some(PathBuf::from("index.json"));

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "r", ["rr.send", ["bytes.lit", "A"]]],
            ["io.read", "r", 100]
        ]),
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
    assert_eq!(res.solve_output, b"HELLO");
    assert_eq!(res.rr_request_calls, Some(1));
    assert_eq!(
        res.sched_stats.as_ref().map(|s| s.virtual_time_end),
        Some(50)
    );

    rm_rf(&fixture);
}

#[test]
fn solve_kv_get_and_get_stream_respect_latency_index() {
    let fixture = create_temp_dir("x07_phaseG2_kv");
    std::fs::write(
        fixture.join("seed.json"),
        br#"{"format":"x07.kv.seed@0.1.0","default_latency_ticks":0,"entries":[{"key_b64":"YQ==","value_b64":"AQIDBA==","latency_ticks":25}]}"#,
    )
    .expect("write seed.json");

    let mut cfg = base_cfg(WorldId::SolveKv);
    cfg.fixture_kv_dir = Some(fixture.clone());
    cfg.fixture_kv_seed = Some(PathBuf::from("seed.json"));

    let program_get = x07_program::entry(&[], json!(["kv.get", ["bytes.lit", "a"]]));
    let compile = compile_program(program_get.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, &[1u8, 2, 3, 4]);
    assert_eq!(res.kv_get_calls, Some(1));
    assert_eq!(
        res.sched_stats.as_ref().map(|s| s.virtual_time_end),
        Some(25)
    );

    let program_stream = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "r", ["kv.get_stream", ["bytes.lit", "a"]]],
            ["io.read", "r", 10]
        ]),
    );
    let compile = compile_program(program_stream.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, &[1u8, 2, 3, 4]);
    assert_eq!(res.kv_get_calls, Some(1));
    assert_eq!(
        res.sched_stats.as_ref().map(|s| s.virtual_time_end),
        Some(25)
    );

    rm_rf(&fixture);
}
