use serde_json::json;
use x07_host_runner::{compile_program, RunnerConfig};
use x07_worlds::WorldId;

mod x07_program;

fn config() -> RunnerConfig {
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
        cpu_time_limit_seconds: 20,
        debug_borrow_checks: false,
    }
}

#[test]
fn unknown_module_includes_pkg_add_hint_when_available() {
    let cfg = config();

    let program = x07_program::entry(&["std.text.ws"], json!(["view.to_bytes", "input"]));
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(!compile.ok);

    let msg = compile.compile_error.unwrap_or_default();
    assert!(
        msg.contains("x07 pkg add ext-text@0.1.1 --sync"),
        "compile_error={msg:?}"
    );
}
