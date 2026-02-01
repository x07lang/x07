use serde_json::json;
use x07_host_runner::{compile_program, run_artifact_file, RunnerConfig};
use x07_worlds::WorldId;

mod x07_program;

fn pure_cfg() -> RunnerConfig {
    RunnerConfig {
        world: WorldId::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 5_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 1024 * 1024,
        cpu_time_limit_seconds: 20,
        debug_borrow_checks: false,
    }
}

#[test]
fn core_for_loop_can_count_up() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "x", 0],
            ["for", "i", 0, 3, ["set", "x", ["+", "x", 1]]],
            ["bytes1", "x"]
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
    assert_eq!(res.solve_output, b"\x03");
}

#[test]
fn core_bytes_eq_does_not_require_return() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "a", ["bytes.lit", "abc"]],
            ["let", "b", ["bytes.lit", "abc"]],
            ["bytes1", ["bytes.eq", "a", "b"]]
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
    assert_eq!(res.solve_output, b"\x01");
}

#[test]
fn core_shifts_and_bitwise_ops_are_available() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "x", ["<<u", 1, 3]],
            ["let", "y", ["&", 45, 51]],
            ["let", "out", ["vec_u8.with_capacity", 2]],
            ["set", "out", ["vec_u8.push", "out", "x"]],
            ["set", "out", ["vec_u8.push", "out", "y"]],
            ["vec_u8.into_bytes", "out"]
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
    assert_eq!(res.solve_output, b"\x08\x21");
}

#[test]
fn core_if_else_works() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "out", ["vec_u8.with_capacity", 1]],
            ["let", "c", ["=", 0, 1]],
            [
                "if",
                "c",
                ["set", "out", ["vec_u8.push", "out", 1]],
                ["set", "out", ["vec_u8.push", "out", 2]]
            ],
            ["vec_u8.into_bytes", "out"]
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
    assert_eq!(res.solve_output, b"\x02");
}

#[test]
fn core_div_and_mod_are_available_and_safe_on_zero() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "a", ["/", 10, 3]],
            ["let", "b", ["%", 10, 3]],
            ["let", "c", ["/", 10, 0]],
            ["let", "d", ["%", 10, 0]],
            ["let", "e", [">>u", 8, 3]],
            ["let", "out", ["vec_u8.with_capacity", 5]],
            ["set", "out", ["vec_u8.push", "out", "a"]],
            ["set", "out", ["vec_u8.push", "out", "b"]],
            ["set", "out", ["vec_u8.push", "out", "c"]],
            ["set", "out", ["vec_u8.push", "out", "d"]],
            ["set", "out", ["vec_u8.push", "out", "e"]],
            ["vec_u8.into_bytes", "out"]
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
    assert_eq!(res.solve_output, b"\x03\x01\x00\x0a\x01");
}

#[test]
fn core_i32_helpers_work() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "out", ["vec_u8.with_capacity", 16]],
            ["let", "a", -5],
            ["let", "b", 3],
            ["let", "min_ab", ["if", ["<", "a", "b"], "a", "b"]],
            ["let", "max_ab", ["if", [">", "a", "b"], "a", "b"]],
            ["let", "abs_a", ["if", ["<", "a", 0], ["-", 0, "a"], "a"]],
            [
                "let",
                "clamped_a",
                ["if", ["<", "a", -3], -3, ["if", [">", "a", 10], 10, "a"]]
            ],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", "min_ab"]
                ]
            ],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", "max_ab"]
                ]
            ],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", "abs_a"]
                ]
            ],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", "clamped_a"]
                ]
            ],
            ["vec_u8.into_bytes", "out"]
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
    assert_eq!(
        res.solve_output,
        b"\xfb\xff\xff\xff\x03\x00\x00\x00\x05\x00\x00\x00\xfd\xff\xff\xff"
    );
}

#[test]
fn core_bytes_any_all_u8_work() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "out", ["vec_u8.with_capacity", 3]],
            ["let", "b0", ["bytes.lit", "abC"]],
            ["let", "v0", ["bytes.view", "b0"]],
            ["let", "n0", ["view.len", "v0"]],
            ["let", "any0", 0],
            [
                "for",
                "i",
                0,
                "n0",
                [
                    "begin",
                    [
                        "if",
                        ["=", "any0", 0],
                        [
                            "if",
                            ["=", ["view.get_u8", "v0", "i"], 67],
                            ["set", "any0", 1],
                            0
                        ],
                        0
                    ],
                    0
                ]
            ],
            ["set", "out", ["vec_u8.push", "out", "any0"]],
            ["let", "b1", ["bytes.lit", "abc"]],
            ["let", "v1", ["bytes.view", "b1"]],
            ["let", "n1", ["view.len", "v1"]],
            ["let", "all1", 1],
            [
                "for",
                "i",
                0,
                "n1",
                [
                    "begin",
                    [
                        "if",
                        ["=", "all1", 1],
                        [
                            "begin",
                            ["let", "c", ["view.get_u8", "v1", "i"]],
                            ["if", ["<u", "c", 97], ["set", "all1", 0], 0],
                            ["if", [">=u", "c", 123], ["set", "all1", 0], 0],
                            0
                        ],
                        0
                    ],
                    0
                ]
            ],
            ["set", "out", ["vec_u8.push", "out", "all1"]],
            ["let", "n2", ["view.len", "v0"]],
            ["let", "all0", 1],
            [
                "for",
                "i",
                0,
                "n2",
                [
                    "begin",
                    [
                        "if",
                        ["=", "all0", 1],
                        [
                            "begin",
                            ["let", "c", ["view.get_u8", "v0", "i"]],
                            ["if", ["<u", "c", 97], ["set", "all0", 0], 0],
                            ["if", [">=u", "c", 123], ["set", "all0", 0], 0],
                            0
                        ],
                        0
                    ],
                    0
                ]
            ],
            ["set", "out", ["vec_u8.push", "out", "all0"]],
            ["vec_u8.into_bytes", "out"]
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
    assert_eq!(res.solve_output, b"\x01\x01\x00");
}

#[test]
fn core_bit_helpers_work() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "out", ["vec_u8.with_capacity", 12]],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", ["^", 0, -1]]
                ]
            ],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", ["|", ["<<u", 1, 1], [">>u", 1, 31]]]
                ]
            ],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", ["|", [">>u", 2, 1], ["<<u", 2, 31]]]
                ]
            ],
            ["vec_u8.into_bytes", "out"]
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
    assert_eq!(
        res.solve_output,
        b"\xff\xff\xff\xff\x02\x00\x00\x00\x01\x00\x00\x00"
    );
}

#[test]
fn core_for_view_and_vec_u8_work() {
    let cfg = pure_cfg();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "b", ["bytes.alloc", 3]],
            ["set", "b", ["bytes.set_u8", "b", 0, 1]],
            ["set", "b", ["bytes.set_u8", "b", 1, 2]],
            ["set", "b", ["bytes.set_u8", "b", 2, 3]],
            ["let", "v", ["bytes.view", "b"]],
            ["let", "sum0", 0],
            [
                "for",
                "i",
                0,
                ["view.len", "v"],
                ["set", "sum0", ["+", "sum0", ["view.get_u8", "v", "i"]]]
            ],
            ["let", "vec", ["vec_u8.with_capacity", 3]],
            ["set", "vec", ["vec_u8.push", "vec", 1]],
            ["set", "vec", ["vec_u8.push", "vec", 2]],
            ["set", "vec", ["vec_u8.push", "vec", 3]],
            ["let", "vv", ["vec_u8.as_view", "vec"]],
            ["let", "sum1", 0],
            [
                "for",
                "i",
                0,
                ["vec_u8.len", "vec"],
                ["set", "sum1", ["+", "sum1", ["view.get_u8", "vv", "i"]]]
            ],
            ["let", "out", ["vec_u8.with_capacity", 2]],
            ["set", "out", ["vec_u8.push", "out", "sum0"]],
            ["set", "out", ["vec_u8.push", "out", "sum1"]],
            ["vec_u8.into_bytes", "out"]
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
    assert_eq!(res.solve_output, b"\x06\x06");
}
