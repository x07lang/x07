use std::path::PathBuf;

use serde_json::json;
use x07_host_runner::{compile_program_with_options, run_artifact_file, RunnerConfig};
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

fn os_module_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("locate workspace root")
        .join("stdlib/os/0.2.0/modules")
}

fn compile_options(world: WorldId) -> x07c::compile::CompileOptions {
    x07c::compile::CompileOptions {
        world,
        enable_fs: false,
        enable_rr: false,
        enable_kv: false,
        module_roots: vec![os_module_root()],
        arch_root: None,
        emit_main: true,
        freestanding: false,
        optimize: true,
        contract_mode: x07c::compile::ContractMode::RuntimeTrap,
        allow_unsafe: None,
        allow_ffi: None,
    }
}

fn u32_le(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

#[test]
fn proc_caps_v1_pack_encodes_bytes() {
    let cfg = config();
    let program = x07_program::entry(
        &["std.os.process.caps_v1"],
        json!(["std.os.process.caps_v1.finish", 1, 2, 3, 4]),
    );

    let compile = compile_program_with_options(
        program.as_slice(),
        &cfg,
        None,
        &compile_options(WorldId::SolvePure),
        &[],
    )
    .expect("compile ok");
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

    let mut expected = Vec::new();
    expected.push(1);
    expected.extend_from_slice(&u32_le(1));
    expected.extend_from_slice(&u32_le(2));
    expected.extend_from_slice(&u32_le(3));
    expected.extend_from_slice(&u32_le(4));
    assert_eq!(res.solve_output, expected);
}

#[test]
fn proc_req_v1_new_and_finish_encodes_minimal_request() {
    let cfg = config();
    let program = x07_program::entry(
        &["std.os.process.req_v1"],
        json!([
            "begin",
            ["let", "exe", ["bytes.lit", "tool"]],
            ["let", "reqb", ["std.os.process.req_v1.new", "exe"]],
            ["std.os.process.req_v1.finish", "reqb"]
        ]),
    );

    let compile = compile_program_with_options(
        program.as_slice(),
        &cfg,
        None,
        &compile_options(WorldId::SolvePure),
        &[],
    )
    .expect("compile ok");
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

    let mut expected = Vec::new();
    expected.push(1);
    expected.push(0);
    expected.extend_from_slice(&u32_le(1));
    expected.extend_from_slice(&u32_le(4));
    expected.extend_from_slice(b"tool");
    expected.extend_from_slice(&u32_le(0));
    expected.extend_from_slice(&u32_le(0));
    expected.extend_from_slice(&u32_le(0));
    assert_eq!(res.solve_output, expected);
}

#[test]
fn proc_req_v1_builder_encodes_args_env_cwd_and_stdin() {
    let cfg = config();
    let program = x07_program::entry(
        &["std.os.process.req_v1"],
        json!([
            "begin",
            ["let", "exe", ["bytes.lit", "tool"]],
            ["let", "reqb", ["std.os.process.req_v1.new", "exe"]],
            [
                "set",
                "reqb",
                ["std.os.process.req_v1.arg", "reqb", ["bytes.lit", "arg1"]]
            ],
            [
                "set",
                "reqb",
                ["std.os.process.req_v1.arg", "reqb", ["bytes.lit", "arg2"]]
            ],
            [
                "set",
                "reqb",
                [
                    "std.os.process.req_v1.env",
                    "reqb",
                    ["bytes.lit", "KEY"],
                    ["bytes.lit", "VAL"]
                ]
            ],
            [
                "set",
                "reqb",
                ["std.os.process.req_v1.cwd", "reqb", ["bytes.lit", "cwd"]]
            ],
            [
                "set",
                "reqb",
                [
                    "std.os.process.req_v1.stdin",
                    "reqb",
                    ["bytes.lit", "stdin"]
                ]
            ],
            ["std.os.process.req_v1.finish", "reqb"]
        ]),
    );

    let compile = compile_program_with_options(
        program.as_slice(),
        &cfg,
        None,
        &compile_options(WorldId::SolvePure),
        &[],
    )
    .expect("compile ok");
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

    let mut expected = Vec::new();
    expected.push(1);
    expected.push(0);
    expected.extend_from_slice(&u32_le(3));
    expected.extend_from_slice(&u32_le(4));
    expected.extend_from_slice(b"tool");
    expected.extend_from_slice(&u32_le(4));
    expected.extend_from_slice(b"arg1");
    expected.extend_from_slice(&u32_le(4));
    expected.extend_from_slice(b"arg2");
    expected.extend_from_slice(&u32_le(1));
    expected.extend_from_slice(&u32_le(3));
    expected.extend_from_slice(b"KEY");
    expected.extend_from_slice(&u32_le(3));
    expected.extend_from_slice(b"VAL");
    expected.extend_from_slice(&u32_le(3));
    expected.extend_from_slice(b"cwd");
    expected.extend_from_slice(&u32_le(5));
    expected.extend_from_slice(b"stdin");
    assert_eq!(res.solve_output, expected);
}

#[test]
fn result_bytes_v1_decode_helpers_work_for_err_and_ok_docs() {
    let cfg = config();

    let err_program = x07_program::entry(
        &["std.os.process"],
        json!([
            "begin",
            ["let", "doc", "input"],
            ["let", "is_err", ["std.os.process.is_err", "doc"]],
            ["let", "code", ["std.os.process.err_code", "doc"]],
            [
                "if",
                [
                    "&",
                    ["=", "is_err", 1],
                    ["=", "code", ["std.os.process.code_output_limit"]]
                ],
                ["bytes.lit", "ok"],
                ["bytes.lit", "bad"]
            ]
        ]),
    );

    let ok_program = x07_program::entry(
        &["std.bytes", "std.os.process"],
        json!([
            "begin",
            ["let", "doc", "input"],
            ["let", "want_stdout", ["bytes.lit", "abc"]],
            ["let", "want_stderr", ["bytes.lit", "xy"]],
            ["let", "is_err", ["std.os.process.is_err", "doc"]],
            ["let", "ver", ["std.os.process.resp_ver", "doc"]],
            ["let", "exit_code", ["std.os.process.resp_exit_code", "doc"]],
            ["let", "flags", ["std.os.process.resp_flags", "doc"]],
            ["let", "stdout", ["std.os.process.resp_stdout", "doc"]],
            ["let", "stderr", ["std.os.process.resp_stderr", "doc"]],
            [
                "if",
                [
                    "&",
                    ["=", "is_err", 0],
                    [
                        "&",
                        ["=", "ver", 1],
                        [
                            "&",
                            ["=", "exit_code", 42],
                            [
                                "&",
                                ["=", "flags", 7],
                                [
                                    "&",
                                    [
                                        "std.bytes.eq",
                                        ["bytes.view", "stdout"],
                                        ["bytes.view", "want_stdout"]
                                    ],
                                    [
                                        "std.bytes.eq",
                                        ["bytes.view", "stderr"],
                                        ["bytes.view", "want_stderr"]
                                    ]
                                ]
                            ]
                        ]
                    ]
                ],
                ["bytes.lit", "ok"],
                ["bytes.lit", "bad"]
            ]
        ]),
    );

    for (name, program, input) in [
        ("err", err_program, {
            let mut doc = Vec::new();
            doc.push(0u8);
            doc.extend_from_slice(&u32_le(5));
            doc.extend_from_slice(&u32_le(0));
            doc
        }),
        ("ok", ok_program, {
            let mut doc = Vec::new();
            doc.push(1u8);
            doc.push(1u8);
            doc.extend_from_slice(&u32_le(42));
            doc.extend_from_slice(&u32_le(7));
            doc.extend_from_slice(&u32_le(3));
            doc.extend_from_slice(b"abc");
            doc.extend_from_slice(&u32_le(2));
            doc.extend_from_slice(b"xy");
            doc
        }),
    ] {
        let compile = compile_program_with_options(
            program.as_slice(),
            &cfg,
            None,
            &compile_options(WorldId::RunOs),
            &[],
        )
        .unwrap_or_else(|e| panic!("compile ok ({name}): {e:#}"));
        assert!(
            compile.ok,
            "compile_error ({name})={:?}\nstdout:\n{}\nstderr:\n{}",
            compile.compile_error,
            String::from_utf8_lossy(&compile.stdout),
            String::from_utf8_lossy(&compile.stderr)
        );
        let exe = compile.compiled_exe.expect("compiled exe");

        let res = run_artifact_file(&cfg, &exe, &input).expect("runner ok");
        assert!(res.ok, "trap ({name})={:?}", res.trap);
        assert_eq!(res.solve_output, b"ok", "decode failed for {name}");
    }
}
