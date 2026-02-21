use serde_json::json;
use x07_host_runner::{compile_and_run_with_options, compile_options_for_world, RunnerConfig};
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

fn compile_and_run(program: &[u8], cfg: &RunnerConfig, optimize: bool) -> Vec<u8> {
    let mut options = compile_options_for_world(cfg.world, Vec::new()).expect("compile options");
    options.optimize = optimize;

    let res = compile_and_run_with_options(program, cfg, b"", None, &options).expect("runner ok");
    assert!(
        res.compile.ok,
        "compile_error={:?}",
        res.compile.compile_error
    );
    let solve = res.solve.expect("solve result");
    assert!(
        solve.ok,
        "trap={:?}\nstderr={:?}",
        solve.trap,
        String::from_utf8_lossy(&solve.stderr)
    );
    solve.solve_output
}

fn assert_optimizer_equiv(program: Vec<u8>) {
    let cfg = config();

    let on = compile_and_run(program.as_slice(), &cfg, true);
    let off = compile_and_run(program.as_slice(), &cfg, false);
    assert_eq!(on, off);
}

#[test]
fn optimizer_semantics_noop_equivalence() {
    // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
    let program = x07_program::entry(&[], json!(["bytes.alloc", 0]));
    assert_optimizer_equiv(program);
}

#[test]
fn optimizer_semantics_strength_reduce_equivalence() {
    // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "x", 7],
            ["let", "n", ["*", "x", 8]],
            ["bytes.alloc", "n"]
        ]),
    );
    assert_optimizer_equiv(program);
}

#[test]
fn optimizer_semantics_dce_equivalence() {
    // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "x", 0],
            ["let", "t", ["+", "x", 1]],
            ["bytes1", "x"]
        ]),
    );
    assert_optimizer_equiv(program);
}

#[test]
fn optimizer_semantics_inlining_equivalence() {
    // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
    let program = x07_program::entry_with_decls(
        &[],
        vec![x07_program::defn(
            "main.inc",
            &[("x", "i32")],
            "i32",
            json!(["+", "x", 1]),
        )],
        json!([
            "begin",
            ["let", "x", 7],
            ["let", "n", ["main.inc", "x"]],
            ["bytes.alloc", "n"]
        ]),
    );
    assert_optimizer_equiv(program);
}

#[test]
fn optimizer_semantics_unroll_equivalence() {
    // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "sum", 0],
            ["for", "i", 0, 4, ["set", "sum", ["+", "sum", "i"]]],
            ["bytes.alloc", "sum"]
        ]),
    );
    assert_optimizer_equiv(program);
}
