use std::path::{Path, PathBuf};

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

fn compile_exe(program: &[u8]) -> PathBuf {
    let cfg = config();
    let compile = compile_program(program, &cfg, None).expect("compile ok");
    assert!(compile.ok, "compile_error={:?}", compile.compile_error);
    compile.compiled_exe.expect("compiled exe")
}

fn run_exe(exe: &Path, input: &[u8]) -> Vec<u8> {
    let cfg = config();
    let res = run_artifact_file(&cfg, exe, input).expect("runner ok");
    assert!(
        res.ok,
        "trap={:?}\nstderr={:?}",
        res.trap,
        String::from_utf8_lossy(&res.stderr)
    );
    res.solve_output
}

#[test]
fn std_text_normalize_lines_matches_suite_vectors() {
    let program = x07_program::entry(
        &["std.text.ascii"],
        json!(["std.text.ascii.normalize_lines", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b"  a \r\n\tb\t\n\n c  \r\n"), b"a\nb\nc");
    assert_eq!(run_exe(&exe, b" \t\r\n \n"), b"");
    assert_eq!(run_exe(&exe, b"one\nTWO\r\nthree"), b"one\nTWO\nthree");
}

#[test]
fn std_text_tokenize_words_lower_matches_suite_vectors() {
    let program = x07_program::entry(
        &["std.text.ascii"],
        json!(["std.text.ascii.tokenize_words_lower", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b"Hello, WORLD!!"), b"hello world");
    assert_eq!(run_exe(&exe, b"Foo2Bar---BAZ"), b"foo bar baz");
    assert_eq!(run_exe(&exe, b"1234!!!"), b"");
}

#[test]
fn std_text_utf8_validate_or_empty_matches_suite_vectors() {
    let program = x07_program::entry(
        &["std.text.utf8"],
        json!(["std.text.utf8.validate_or_empty", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b"hello"), b"hello");
    assert_eq!(run_exe(&exe, "€".as_bytes()), "€".as_bytes());
    assert_eq!(run_exe(&exe, "Привет".as_bytes()), "Привет".as_bytes());
    assert_eq!(run_exe(&exe, b"\xC3("), b"");
    assert_eq!(run_exe(&exe, b"\xE2\x82"), b"");
}

#[test]
fn std_bytes_view_helpers_work() {
    let program = x07_program::entry(
        &["std.bytes"],
        json!([
            "begin",
            ["let", "v", "input"],
            ["let", "he_b", ["bytes.lit", "he"]],
            ["let", "he_v", ["bytes.view", "he_b"]],
            ["let", "lo_b", ["bytes.lit", "lo"]],
            ["let", "lo_v", ["bytes.view", "lo_b"]],
            ["let", "out", ["vec_u8.with_capacity", 14]],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", ["std.bytes.max_u8", "v"]]
                ]
            ],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", ["std.bytes.sum_u8", "v"]]
                ]
            ],
            [
                "set",
                "out",
                [
                    "vec_u8.extend_bytes",
                    "out",
                    ["codec.write_u32_le", ["std.bytes.count_u8", "v", 108]]
                ]
            ],
            [
                "set",
                "out",
                ["vec_u8.push", "out", ["std.bytes.starts_with", "v", "he_v"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.push", "out", ["std.bytes.ends_with", "v", "lo_v"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(
        run_exe(&exe, b"hello"),
        b"\x6f\x00\x00\x00\x14\x02\x00\x00\x02\x00\x00\x00\x01\x01"
    );
    assert_eq!(
        run_exe(&exe, b""),
        b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
    );
}

#[test]
fn std_text_ascii_split_u8_encodes_slices() {
    let program = x07_program::entry(
        &["std.text.ascii"],
        json!(["std.text.ascii.split_u8", "input", 44]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(
        run_exe(&exe, b"a,b,,c"),
        b"X7SL\x01\x00\x00\x00\x04\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02\x00\x00\x00\x01\x00\x00\x00\x04\x00\x00\x00\x00\x00\x00\x00\x05\x00\x00\x00\x01\x00\x00\x00"
    );
    assert_eq!(
        run_exe(&exe, b"a,"),
        b"X7SL\x01\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00"
    );
}

#[test]
fn std_text_ascii_split_lines_view_encodes_slices() {
    let program = x07_program::entry(
        &["std.text.ascii"],
        json!(["std.text.ascii.split_lines_view", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(
        run_exe(&exe, b"a\r\nb\n\nc\r\n"),
        b"X7SL\x01\x00\x00\x00\x04\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x03\x00\x00\x00\x01\x00\x00\x00\x05\x00\x00\x00\x00\x00\x00\x00\x06\x00\x00\x00\x01\x00\x00\x00"
    );
}

#[test]
fn std_text_slices_accessors_work() {
    let program = x07_program::entry(
        &["std.text.ascii", "std.text.slices"],
        json!([
            "begin",
            ["let", "x7sl", ["std.text.ascii.split_u8", "input", 44]],
            ["let", "c", ["std.text.slices.count_v1", "x7sl"]],
            ["let", "s0", ["std.text.slices.start_v1", "x7sl", 0]],
            ["let", "l0", ["std.text.slices.len_v1", "x7sl", 0]],
            ["let", "s1", ["std.text.slices.start_v1", "x7sl", 1]],
            ["let", "l1", ["std.text.slices.len_v1", "x7sl", 1]],
            ["let", "out", ["vec_u8.with_capacity", 20]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "c"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "s0"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "l0"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "s1"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "l1"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(
        run_exe(&exe, b"a,b"),
        b"\x02\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02\x00\x00\x00\x01\x00\x00\x00"
    );
}

#[test]
fn std_bit_popcount_u32_works() {
    let program = x07_program::entry(
        &["std.bit"],
        json!([
            "begin",
            ["let", "out", ["vec_u8.with_capacity", 4]],
            [
                "set",
                "out",
                ["vec_u8.push", "out", ["std.bit.popcount_u32", 0]]
            ],
            [
                "set",
                "out",
                ["vec_u8.push", "out", ["std.bit.popcount_u32", -252645136]]
            ],
            [
                "set",
                "out",
                ["vec_u8.push", "out", ["std.bit.popcount_u32", -1]]
            ],
            [
                "set",
                "out",
                ["vec_u8.push", "out", ["std.bit.popcount_u32", -2147483648]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b""), b"\x00\x10\x20\x01");
}

#[test]
fn std_json_canonicalize_small_matches_suite_vectors() {
    let program = x07_program::entry(
        &["std.json"],
        json!(["std.json.canonicalize_small", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, br#"{"b":2,"a":1}"#), br#"{"a":1,"b":2}"#);
    assert_eq!(
        run_exe(&exe, br#"{"z":0,"a":{"d":4,"c":3}}"#),
        br#"{"a":{"c":3,"d":4},"z":0}"#
    );
    assert_eq!(
        run_exe(&exe, br#"[{"b":2,"a":1},{"k":true,"j":null}]"#),
        br#"[{"a":1,"b":2},{"j":null,"k":true}]"#
    );
    assert_eq!(run_exe(&exe, br#"{bad"#), b"ERR");
}

#[test]
fn std_json_extract_path_matches_suite_vectors() {
    let program = x07_program::entry(
        &["std.json"],
        json!(["std.json.extract_path_canon_or_err", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(
        run_exe(&exe, b"{\"b\":{\"a\":1,\"c\":2},\"x\":0}\x00b.a"),
        b"1"
    );
    assert_eq!(
        run_exe(&exe, b"{\"b\":{\"a\":1,\"c\":2},\"x\":0}\x00b"),
        b"{\"a\":1,\"c\":2}"
    );
    assert_eq!(
        run_exe(&exe, b"{\"a\":[{\"b\":2,\"a\":1}]}\x00a"),
        b"[{\"a\":1,\"b\":2}]"
    );
    assert_eq!(run_exe(&exe, b"{\"a\":1}\x00a.b"), b"ERR");
}

#[test]
fn std_map_word_freq_sorted_matches_suite_vectors() {
    let program = x07_program::entry(
        &["std.map"],
        json!(["std.map.word_freq_sorted_ascii", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b"a a b"), b"a=2\nb=1");
    assert_eq!(run_exe(&exe, b"Hello hello HELLO"), b"hello=3");
    assert_eq!(run_exe(&exe, b"b a B a."), b"a=2\nb=2");
    assert_eq!(run_exe(&exe, b""), b"");
}

#[test]
fn std_set_unique_lines_sorted_matches_suite_vectors() {
    let program = x07_program::entry(
        &["std.set"],
        json!(["std.set.unique_lines_sorted", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b"b\na\na\n"), b"a\nb");
    assert_eq!(run_exe(&exe, b"  cat\r\nDog\ncat\n"), b"Dog\ncat");
    assert_eq!(run_exe(&exe, b"\n\n\t \n"), b"");
}

#[test]
fn std_parse_i32_status_le_matches_suite_vectors() {
    let program = x07_program::entry(&["std.parse"], json!(["std.parse.i32_status_le", "input"]));
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b"0"), b"\x01\x00\x00\x00\x00");
    assert_eq!(run_exe(&exe, b"-5"), b"\x01\xfb\xff\xff\xff");
    assert_eq!(run_exe(&exe, b"  123  "), b"\x01{\x00\x00\x00");
    assert_eq!(run_exe(&exe, b"2147483647"), b"\x01\xff\xff\xff\x7f");
    assert_eq!(run_exe(&exe, b"2147483648"), b"\x00");
    assert_eq!(run_exe(&exe, b"abc"), b"\x00");
}

#[test]
fn std_result_chain_sum_csv_i32_matches_suite_vectors() {
    let program = x07_program::entry(
        &["std.result"],
        json!(["std.result.chain_sum_csv_i32", "input"]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b"1,2,3"), b"\x01\x06\x00\x00\x00");
    assert_eq!(run_exe(&exe, b" 10 , -2, 5 "), b"\x01\r\x00\x00\x00");
    assert_eq!(run_exe(&exe, b"1,abc,3"), b"\x00");
    assert_eq!(run_exe(&exe, b"2147483647,1"), b"\x00");
}

#[test]
fn std_regex_lite_find_literal_works() {
    let program = x07_program::entry(
        &["std.regex-lite"],
        json!([
            "codec.write_u32_le",
            [
                "std.regex-lite.find_literal",
                "input",
                ["bytes.lit", "needle"]
            ]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    assert_eq!(run_exe(&exe, b"abc needle xyz"), vec![4, 0, 0, 0]);
    assert_eq!(run_exe(&exe, b"nope"), vec![255, 255, 255, 255]);
}

#[test]
fn std_csv_module_is_removed() {
    let program = x07_program::entry(&["std.csv"], json!(["std.csv.sum_i32_status_le", "input"]));
    let cfg = config();
    let compile = compile_program(program.as_slice(), &cfg, None).expect("compile ran");
    assert!(!compile.ok);
    assert!(compile
        .compile_error
        .unwrap_or_default()
        .contains("std.csv"));
}
