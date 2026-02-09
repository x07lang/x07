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

fn parse_contract_trap(trap: &str) -> serde_json::Value {
    const PREFIX: &str = "X07T_CONTRACT_V1 ";
    assert!(
        trap.starts_with(PREFIX),
        "expected trap prefix {PREFIX:?}, got: {trap:?}"
    );
    let payload = &trap[PREFIX.len()..];
    serde_json::from_str(payload).expect("parse contract trap payload JSON")
}

#[test]
fn requires_violation_traps_with_payload() {
    let cfg = config();

    let program = x07_program::entry_v0_5_with_decls(
        &[],
        vec![json!({
            "kind": "defn",
            "name": "main.f",
            "params": [],
            "result": "i32",
            "requires": [{
                "id": "req1",
                "expr": 0,
                "witness": [42],
            }],
            "body": 0,
        })],
        json!(["bytes1", ["main.f"]]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(
        compile.ok,
        "compile_error={:?}\nstderr:\n{}",
        compile.compile_error,
        String::from_utf8_lossy(&compile.stderr)
    );
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok, "expected trap, got ok");
    let trap = res.trap.as_deref().expect("trap text");
    let payload = parse_contract_trap(trap);

    assert_eq!(payload["contract_kind"], "requires");
    assert_eq!(payload["fn"], "main.f");
    assert_eq!(payload["clause_id"], "req1");
    assert_eq!(payload["clause_index"], 0);
    assert_eq!(payload["clause_ptr"], "/decls/0/requires/0/expr");
    assert_eq!(payload["witness"][0]["ty"], "i32");
    assert_eq!(payload["witness"][0]["value_i32"], 42);
}

#[test]
fn ensures_violation_on_return_stmt_traps_with_payload() {
    let cfg = config();

    let program = x07_program::entry_v0_5_with_decls(
        &[],
        vec![json!({
            "kind": "defn",
            "name": "main.f",
            "params": [],
            "result": "i32",
            "ensures": [{
                "id": "ens1",
                "expr": ["=", "__result", 0],
                "witness": ["__result"],
            }],
            "body": ["return", 7],
        })],
        json!(["bytes1", ["main.f"]]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(
        compile.ok,
        "compile_error={:?}\nstderr:\n{}",
        compile.compile_error,
        String::from_utf8_lossy(&compile.stderr)
    );
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok, "expected trap, got ok");
    let trap = res.trap.as_deref().expect("trap text");
    let payload = parse_contract_trap(trap);

    assert_eq!(payload["contract_kind"], "ensures");
    assert_eq!(payload["fn"], "main.f");
    assert_eq!(payload["clause_id"], "ens1");
    assert_eq!(payload["clause_index"], 0);
    assert_eq!(payload["clause_ptr"], "/decls/0/ensures/0/expr");
    assert_eq!(payload["witness"][0]["ty"], "i32");
    assert_eq!(payload["witness"][0]["value_i32"], 7);
}

#[test]
fn ensures_violation_on_try_early_return_traps() {
    let cfg = config();

    let program = x07_program::entry_v0_5_with_decls(
        &[],
        vec![json!({
            "kind": "defn",
            "name": "main.f",
            "params": [],
            "result": "result_i32",
            "ensures": [{
                "id": "ens_try",
                "expr": 0,
            }],
            "body": ["begin", ["try", ["result_i32.err", 7]], ["result_i32.ok", 0]],
        })],
        json!(["begin", ["main.f"], ["bytes.alloc", 0]]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(
        compile.ok,
        "compile_error={:?}\nstderr:\n{}",
        compile.compile_error,
        String::from_utf8_lossy(&compile.stderr)
    );
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok, "expected trap, got ok");
    let trap = res.trap.as_deref().expect("trap text");
    let payload = parse_contract_trap(trap);

    assert_eq!(payload["contract_kind"], "ensures");
    assert_eq!(payload["fn"], "main.f");
    assert_eq!(payload["clause_id"], "ens_try");
    assert_eq!(payload["clause_index"], 0);
    assert_eq!(payload["clause_ptr"], "/decls/0/ensures/0/expr");
}

#[test]
fn defasync_requires_violation_traps_with_payload() {
    let cfg = config();

    let program = x07_program::entry_v0_5_with_decls(
        &[],
        vec![json!({
            "kind": "defasync",
            "name": "main.af",
            "params": [],
            "result": "bytes",
            "requires": [{
                "id": "req_async",
                "expr": 0,
                "witness": [123],
            }],
            "body": ["bytes.alloc", 0],
        })],
        json!(["begin", ["await", ["main.af"]], ["bytes.alloc", 0]]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(
        compile.ok,
        "compile_error={:?}\nstderr:\n{}",
        compile.compile_error,
        String::from_utf8_lossy(&compile.stderr)
    );
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok, "expected trap, got ok");
    let trap = res.trap.as_deref().expect("trap text");
    let payload = parse_contract_trap(trap);

    assert_eq!(payload["contract_kind"], "requires");
    assert_eq!(payload["fn"], "main.af");
    assert_eq!(payload["clause_id"], "req_async");
    assert_eq!(payload["clause_index"], 0);
    assert_eq!(payload["clause_ptr"], "/decls/0/requires/0/expr");
    assert_eq!(payload["witness"][0]["ty"], "i32");
    assert_eq!(payload["witness"][0]["value_i32"], 123);
}

#[test]
fn defasync_ensures_violation_traps_with_payload() {
    let cfg = config();

    let program = x07_program::entry_v0_5_with_decls(
        &[],
        vec![json!({
            "kind": "defasync",
            "name": "main.af",
            "params": [],
            "result": "bytes",
            "ensures": [{
                "id": "ens_async",
                "expr": ["=", ["bytes.len", ["bytes.view", "__result"]], 1],
                "witness": [["bytes.view", "__result"]],
            }],
            "body": ["bytes.alloc", 0],
        })],
        json!(["begin", ["await", ["main.af"]], ["bytes.alloc", 0]]),
    );

    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ok");
    assert!(
        compile.ok,
        "compile_error={:?}\nstderr:\n{}",
        compile.compile_error,
        String::from_utf8_lossy(&compile.stderr)
    );
    let exe = compile.compiled_exe.expect("compiled exe");

    let res = run_artifact_file(&cfg, &exe, b"").expect("runner ok");
    assert!(!res.ok, "expected trap, got ok");
    let trap = res.trap.as_deref().expect("trap text");
    let payload = parse_contract_trap(trap);

    assert_eq!(payload["contract_kind"], "ensures");
    assert_eq!(payload["fn"], "main.af");
    assert_eq!(payload["clause_id"], "ens_async");
    assert_eq!(payload["clause_index"], 0);
    assert_eq!(payload["clause_ptr"], "/decls/0/ensures/0/expr");
    assert_eq!(payload["witness"][0]["ty"], "bytes_view");
    assert_eq!(payload["witness"][0]["len"], 0);
}
