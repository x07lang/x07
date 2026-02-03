use serde_json::json;
use x07_host_runner::{compile_program, run_artifact_file, RunnerConfig};
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

#[test]
fn compile_fuel_is_reported_and_deterministic() {
    let cfg = config();

    let program = x07_program::entry(&[], json!(["view.to_bytes", "input"]));

    let compile1 = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile1.ok, "compile_error={:?}", compile1.compile_error);
    let fuel1 = compile1.fuel_used.expect("compile fuel");
    assert!(fuel1 > 0);

    let compile2 = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile2.ok, "compile_error={:?}", compile2.compile_error);
    let fuel2 = compile2.fuel_used.expect("compile fuel");
    assert_eq!(fuel1, fuel2);
}

#[test]
fn solve_pure_echoes_bytes() {
    let cfg = config();

    let program = x07_program::entry(&[], json!(["view.to_bytes", "input"]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let input = b"hello x07";
    let res = run_artifact_file(&cfg, &exe, input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.exit_status, 0);
    assert_eq!(res.solve_output, input);
    assert!(res.fuel_used.is_some());
}

#[test]
fn solve_pure_is_deterministic_across_runs() {
    let cfg = config();

    let program = x07_program::entry(&[], json!(["view.to_bytes", "input"]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let input = b"determinism-check";
    let first = run_artifact_file(&cfg, &exe, input).expect("runner ok");
    assert!(first.ok);

    for _ in 0..10 {
        let res = run_artifact_file(&cfg, &exe, input).expect("runner ok");
        assert!(res.ok);
        assert_eq!(res.exit_status, 0);
        assert_eq!(res.solve_output, first.solve_output);
        assert_eq!(res.fuel_used, first.fuel_used);
    }
}

#[test]
fn fuel_limit_traps() {
    let mut cfg = config();
    cfg.solve_fuel = 0;

    let program = x07_program::entry(&[], json!(["view.to_bytes", "input"]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok);
    assert!(String::from_utf8_lossy(&res.stderr).contains("fuel exhausted"));
}

#[test]
fn for_body_accepts_begin_expression() {
    let cfg = config();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "x", 0],
            ["for", "i", 0, 1, ["begin", ["set", "x", 1], 0]],
            ["bytes1", "x"]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, [1u8]);
}

#[test]
fn return_is_allowed_in_if_branch_mixed_with_i32() {
    let cfg = config();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["if", 1, ["return", ["bytes1", 7]], 0],
            ["bytes1", 0]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, [7u8]);
}

#[test]
fn signed_less_than_detects_underflow() {
    let cfg = config();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "x", 0],
            ["set", "x", ["-", "x", 1]],
            ["bytes1", ["if", ["<", "x", 0], 1, 0]]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, [1u8]);
}

#[test]
fn user_defined_functions_work() {
    let cfg = config();

    let program = x07_program::entry_with_decls(
        &[],
        vec![x07_program::defn(
            "main.add",
            &[("a", "i32"), ("b", "i32")],
            "i32",
            json!(["+", "a", "b"]),
        )],
        json!([
            "begin",
            ["let", "out", ["bytes.alloc", 1]],
            ["set", "out", ["bytes.set_u8", "out", 0, ["main.add", 7, 5]]],
            "out"
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, [12u8]);
}

#[test]
fn builtin_modules_can_be_imported() {
    let cfg = config();

    let program = x07_program::entry(&["std.bytes"], json!(["std.bytes.reverse", "input"]));

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"abc").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, b"cba");
}

#[test]
fn builtin_std_option_can_be_imported_and_used() {
    let cfg = config();

    let program = x07_program::entry(
        &["std.option"],
        json!([
            "begin",
            ["let", "opt", ["std.option.some_i32_le", 42]],
            [
                "if",
                ["std.option.is_some", ["bytes.view", "opt"]],
                ["std.option.payload", ["bytes.view", "opt"]],
                ["bytes.lit", "bad"]
            ]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, [42u8, 0, 0, 0]);
}

#[test]
fn builtin_std_bytes_copy_and_slice_work() {
    let cfg = config();

    let program = x07_program::entry(
        &["std.bytes"],
        json!(["std.bytes.slice", ["std.bytes.copy", "input"], 1, 10]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"abcd").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, b"bcd");
}

#[test]
fn bytes_view_builtins_work() {
    let cfg = config();

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "v", ["view.slice", "input", 1, 2]],
            ["view.to_bytes", "v"]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"abcd").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, b"bc");
}

#[test]
fn debug_borrow_checks_surface_borrow_violations() {
    let mut cfg = config();
    cfg.debug_borrow_checks = true;

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "h", ["vec_u8.with_capacity", 1]],
            ["set", "h", ["vec_u8.push", "h", 7]],
            ["let", "v", ["vec_u8.as_view", "h"]],
            ["set", "h", ["vec_u8.reserve_exact", "h", 1000]],
            ["bytes1", ["view.get_u8", "v", 0]]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(!compile.ok);
    let msg = compile.compile_error.unwrap_or_default();
    assert!(msg.contains("set while borrowed"), "compile_error={msg:?}");
}

#[test]
fn debug_borrow_checks_detect_stale_bytes_after_view_as_bytes() {
    let mut cfg = config();
    cfg.debug_borrow_checks = true;

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "h", ["vec_u8.with_capacity", 1]],
            ["set", "h", ["vec_u8.push", "h", 7]],
            ["let", "b", ["view.to_bytes", ["vec_u8.as_view", "h"]]],
            ["set", "h", ["vec_u8.reserve_exact", "h", 1000]],
            ["bytes1", ["bytes.get_u8", "b", 0]]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, [7u8]);

    let dbg = res.debug_stats.expect("debug_stats");
    assert_eq!(dbg.borrow_violations, 0, "debug_stats={dbg:?}");
}

#[test]
fn debug_borrow_checks_detect_stale_output_bytes() {
    let mut cfg = config();
    cfg.debug_borrow_checks = true;

    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "h", ["vec_u8.with_capacity", 1]],
            ["set", "h", ["vec_u8.push", "h", 7]],
            ["let", "b", ["view.to_bytes", ["vec_u8.as_view", "h"]]],
            ["set", "h", ["vec_u8.reserve_exact", "h", 1000]],
            "b"
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(res.ok, "trap={:?}", res.trap);
    assert_eq!(res.solve_output, [7u8]);

    let dbg = res.debug_stats.expect("debug_stats");
    assert_eq!(dbg.borrow_violations, 0, "debug_stats={dbg:?}");
}

#[test]
fn using_fs_module_in_solve_pure_fails() {
    let cfg = config();

    let program = x07_program::entry(
        &["std.fs"],
        json!([
            "std.fs.read",
            ["bytes.view", ["bytes.lit", "fixtures/bytes/hello.txt"]]
        ]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(!compile.ok);
    let msg = compile.compile_error.unwrap_or_default();
    assert!(
        msg.contains("fs.read is disabled")
            || msg.contains("fs.read_async is disabled")
            || msg.contains("fs.open_read is disabled")
            || msg.contains("fs.list_dir is disabled"),
        "compile_error={msg:?}"
    );
}
