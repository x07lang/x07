use std::path::PathBuf;

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

fn compile_exe(program: &[u8]) -> PathBuf {
    let cfg = config();
    let compile = compile_program(program, &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    compile.compiled_exe.expect("compiled exe")
}

#[test]
fn sum_bytes_outputs_u32_le() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "acc", 0],
            [
                "for",
                "i",
                0,
                ["view.len", "input"],
                ["set", "acc", ["+", "acc", ["view.get_u8", "input", "i"]]]
            ],
            ["codec.write_u32_le", "acc"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let input = [1u8, 2, 3, 4];
    let res = run_artifact_file(&cfg, &exe, &input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, vec![10, 0, 0, 0]);
}

#[test]
fn fmt_and_parse_roundtrip() {
    let cfg = config();
    let program = x07_program::entry(&[], json!(["fmt.u32_to_dec", ["parse.u32_dec", "input"]]));
    let exe = compile_exe(program.as_slice());
    let input = b"12345";
    let res = run_artifact_file(&cfg, &exe, input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"12345");
}

#[test]
fn bytes_view_lit_roundtrips_to_bytes() {
    let cfg = config();
    let program = x07_program::entry(&[], json!(["view.to_bytes", ["bytes.view_lit", "abc"]]));
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"abc");
}

#[test]
fn bytes_view_lit_allows_whitespace() {
    let cfg = config();
    let program = x07_program::entry(&[], json!(["view.to_bytes", ["bytes.view_lit", "a b"]]));
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"a b");
}

#[test]
fn vec_u8_builds_bytes() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "v", ["vec_u8.with_capacity", 0]],
            ["set", "v", ["vec_u8.push", "v", 1]],
            ["set", "v", ["vec_u8.push", "v", 2]],
            ["set", "v", ["vec_u8.push", "v", 3]],
            ["vec_u8.into_bytes", "v"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, vec![1, 2, 3]);
}

#[test]
fn map_u32_set_then_get() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "m", ["map_u32.new", 8]],
            ["map_u32.set", "m", 10, 42],
            ["codec.write_u32_le", ["map_u32.get", "m", 10, 0]]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, vec![42, 0, 0, 0]);
}

#[test]
fn bytes_slice_oob_traps() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!(["view.to_bytes", ["view.slice", "input", 0, 10]]),
    );
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"x").expect("runner ok");
    assert!(!res.ok);
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(stderr.contains("view.slice oob"), "stderr={stderr:?}");
    assert!(stderr.contains("ptr="), "stderr={stderr:?}");
}

#[test]
fn and_short_circuits_rhs_trap() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "if",
            [
                "&&",
                ["=", ["view.len", "input"], 1],
                ["=", ["view.get_u8", "input", 0], 47]
            ],
            ["bytes.lit", "ok"],
            ["bytes.lit", "no"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"no");
}

#[test]
fn or_short_circuits_rhs_trap() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "if",
            ["||", ["=", 1, 1], ["=", ["view.get_u8", "input", 0], 47]],
            ["bytes.lit", "ok"],
            ["bytes.lit", "no"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, b"ok");
}

#[test]
fn bytes_alloc_zero_does_not_count_as_alloc() {
    let cfg = config();
    let program = x07_program::entry(&[], json!(["bytes.alloc", 0]));
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert!(res.solve_output.is_empty());

    let stats = res.mem_stats.expect("mem_stats");
    assert_eq!(stats.alloc_calls, 0);
    assert_eq!(stats.memcpy_bytes, 0);
}

#[test]
fn bytes_slice_copies_and_counts_memcpy() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "b", ["bytes.alloc", 3]],
            ["set", "b", ["bytes.set_u8", "b", 0, 10]],
            ["set", "b", ["bytes.set_u8", "b", 1, 11]],
            ["set", "b", ["bytes.set_u8", "b", 2, 12]],
            ["let", "s", ["view.to_bytes", ["bytes.subview", "b", 1, 2]]],
            ["set", "b", ["bytes.set_u8", "b", 1, 99]],
            "s"
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, vec![11, 12]);

    let stats = res.mem_stats.expect("mem_stats");
    assert_eq!(stats.memcpy_bytes, 2);
}

#[test]
fn bytes_concat_counts_memcpy() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "a", ["bytes.alloc", 2]],
            ["set", "a", ["bytes.set_u8", "a", 0, 1]],
            ["set", "a", ["bytes.set_u8", "a", 1, 2]],
            ["let", "b", ["bytes.alloc", 1]],
            ["set", "b", ["bytes.set_u8", "b", 0, 3]],
            ["bytes.concat", "a", "b"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, vec![1, 2, 3]);

    let stats = res.mem_stats.expect("mem_stats");
    assert_eq!(stats.memcpy_bytes, 3);
}

#[test]
fn bytes_copy_counts_memcpy() {
    let cfg = config();
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "src", ["bytes.alloc", 3]],
            ["set", "src", ["bytes.set_u8", "src", 0, 1]],
            ["set", "src", ["bytes.set_u8", "src", 1, 2]],
            ["set", "src", ["bytes.set_u8", "src", 2, 3]],
            ["let", "dst", ["bytes.alloc", 3]],
            ["bytes.copy", "src", "dst"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(res.solve_output, vec![1, 2, 3]);

    let stats = res.mem_stats.expect("mem_stats");
    assert_eq!(stats.memcpy_bytes, 3);
}
