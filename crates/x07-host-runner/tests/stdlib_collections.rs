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

fn u32le(x: u32) -> [u8; 4] {
    x.to_le_bytes()
}

#[test]
fn small_map_put_get_len_remove() {
    let program = x07_program::entry(
        &["std.small_map"],
        json!([
            "begin",
            ["let", "m", ["std.small_map.empty_bytes_u32"]],
            [
                "set",
                "m",
                ["std.small_map.put_bytes_u32", "m", ["bytes.lit", "a"], 5]
            ],
            [
                "set",
                "m",
                ["std.small_map.put_bytes_u32", "m", ["bytes.lit", "b"], 2]
            ],
            [
                "set",
                "m",
                ["std.small_map.put_bytes_u32", "m", ["bytes.lit", "a"], 7]
            ],
            [
                "set",
                "m",
                ["std.small_map.remove_bytes_u32", "m", ["bytes.lit", "b"]]
            ],
            ["let", "n", ["std.small_map.len_bytes_u32", "m"]],
            [
                "let",
                "va",
                ["std.small_map.get_bytes_u32", "m", ["bytes.lit", "a"]]
            ],
            [
                "let",
                "vb",
                ["std.small_map.get_bytes_u32", "m", ["bytes.lit", "b"]]
            ],
            ["let", "out", ["vec_u8.with_capacity", 12]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "n"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "va"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "vb"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(1), u32le(7), u32le(0)].concat(), "out={out:?}");
}

#[test]
fn hash_fnv1a32_matches_reference() {
    fn fnv1a32(bytes: &[u8]) -> u32 {
        let mut h: u32 = 0x811C_9DC5;
        for &b in bytes {
            h ^= b as u32;
            h = h.wrapping_mul(0x0100_0193);
        }
        h
    }

    let program = x07_program::entry(
        &["std.hash"],
        json!(["codec.write_u32_le", ["std.hash.fnv1a32_bytes", "input"]]),
    );
    let exe = compile_exe(program.as_slice());
    for input in [&b""[..], &b"hello"[..], &b"abc"[..], &b"\x00\xFF\x01"[..]] {
        let out = run_exe(&exe, input);
        assert_eq!(out.as_slice(), fnv1a32(input).to_le_bytes());
    }
}

#[test]
fn hash_set_u32_basic() {
    let program = x07_program::entry(
        &["std.hash_set"],
        json!([
            "begin",
            ["let", "s", ["std.hash_set.new_u32", 16]],
            ["std.hash_set.add_u32", "s", 1],
            ["std.hash_set.add_u32", "s", 2],
            ["std.hash_set.add_u32", "s", 2],
            ["let", "n", ["std.hash_set.len_u32", "s"]],
            ["let", "c1", ["std.hash_set.contains_u32", "s", 1]],
            ["let", "c3", ["std.hash_set.contains_u32", "s", 3]],
            ["let", "out", ["vec_u8.with_capacity", 12]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "n"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "c1"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "c3"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(2), u32le(1), u32le(0)].concat());
}

#[test]
fn hash_set_emit_u32le_sorted_unique() {
    let program = x07_program::entry(
        &["std.hash_set"],
        json!([
            "begin",
            ["let", "s", ["std.hash_set.new_u32", 16]],
            ["std.hash_set.add_u32", "s", 3],
            ["std.hash_set.add_u32", "s", 1],
            ["std.hash_set.add_u32", "s", 2],
            ["std.hash_set.add_u32", "s", 2],
            ["std.hash_set.add_u32", "s", 5],
            ["std.hash_set.emit_u32le", "s"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(1), u32le(2), u32le(3), u32le(5)].concat());
}

#[test]
fn hash_set_view_insert_contains() {
    let program = x07_program::entry(
        &["std.hash_set"],
        json!([
            "begin",
            ["let", "set", ["std.hash_set.view_new", 4]],
            [
                "set",
                "set",
                ["std.hash_set.view_insert", "set", "input", 0, 3]
            ],
            [
                "set",
                "set",
                ["std.hash_set.view_insert", "set", "input", 0, 3]
            ],
            [
                "set",
                "set",
                ["std.hash_set.view_insert", "set", "input", 4, 3]
            ],
            ["let", "n", ["std.hash_set.view_len", "set"]],
            [
                "let",
                "c0",
                ["std.hash_set.view_contains", "set", "input", 0, 3]
            ],
            [
                "let",
                "c1",
                ["std.hash_set.view_contains", "set", "input", 4, 3]
            ],
            [
                "let",
                "c2",
                ["std.hash_set.view_contains", "set", "input", 0, 2]
            ],
            ["let", "out", ["vec_u8.with_capacity", 16]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "n"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "c0"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "c1"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "c2"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"abc_def_");
    assert_eq!(out, [u32le(2), u32le(1), u32le(1), u32le(0)].concat());
}

#[test]
fn hash_map_emit_kv_u32le_u32le_sorted_by_key() {
    let program = x07_program::entry(
        &["std.hash_map"],
        json!([
            "begin",
            ["let", "m", ["std.hash_map.with_capacity_u32", 4]],
            ["std.hash_map.set_u32", "m", 2, 200],
            ["std.hash_map.set_u32", "m", 1, 100],
            ["std.hash_map.set_u32", "m", 3, 300],
            ["std.hash_map.set_u32", "m", 2, 201],
            ["std.hash_map.emit_kv_u32le_u32le", "m"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(
        out,
        [
            u32le(1),
            u32le(100),
            u32le(2),
            u32le(201),
            u32le(3),
            u32le(300)
        ]
        .concat()
    );
}

#[test]
fn vec_u8_extend_zeroes_appends() {
    let program = x07_program::entry(
        &[],
        json!([
            "begin",
            ["let", "v", ["vec_u8.with_capacity", 0]],
            ["set", "v", ["vec_u8.extend_zeroes", "v", 4]],
            ["set", "v", ["vec_u8.push", "v", 7]],
            ["vec_u8.into_bytes", "v"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out.as_slice(), [0, 0, 0, 0, 7]);
}

#[test]
fn btree_map_and_set_u32() {
    let program = x07_program::entry(
        &["std.btree_map", "std.btree_set"],
        json!([
            "begin",
            ["let", "s", ["std.btree_set.empty_u32"]],
            ["set", "s", ["std.btree_set.insert_u32", "s", 3]],
            ["set", "s", ["std.btree_set.insert_u32", "s", 1]],
            ["set", "s", ["std.btree_set.insert_u32", "s", 2]],
            ["set", "s", ["std.btree_set.insert_u32", "s", 2]],
            ["let", "m", ["std.btree_map.empty_u32_u32"]],
            ["set", "m", ["std.btree_map.put_u32_u32", "m", 5, 10]],
            ["set", "m", ["std.btree_map.put_u32_u32", "m", 4, 20]],
            ["set", "m", ["std.btree_map.put_u32_u32", "m", 5, 30]],
            ["let", "sn", ["std.btree_set.len_u32", "s"]],
            ["let", "sc", ["std.btree_set.contains_u32", "s", 2]],
            ["let", "mv", ["std.btree_map.get_u32_u32_or", "m", 5, 0]],
            ["let", "out", ["vec_u8.with_capacity", 12]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "sn"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "sc"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "mv"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(3), u32le(1), u32le(30)].concat());
}

#[test]
fn deque_u32_push_pop_grows() {
    let program = x07_program::entry(
        &["std.deque_u32"],
        json!([
            "begin",
            ["let", "q", ["std.deque_u32.with_capacity", 2]],
            ["set", "q", ["std.deque_u32.push_back", "q", 1]],
            ["set", "q", ["std.deque_u32.push_back", "q", 2]],
            ["set", "q", ["std.deque_u32.push_back", "q", 3]],
            ["let", "a", ["std.deque_u32.front_or", "q", 0]],
            ["set", "q", ["std.deque_u32.pop_front", "q"]],
            ["let", "b", ["std.deque_u32.front_or", "q", 0]],
            ["set", "q", ["std.deque_u32.pop_front", "q"]],
            ["let", "c", ["std.deque_u32.front_or", "q", 0]],
            ["set", "q", ["std.deque_u32.pop_front", "q"]],
            ["let", "d", ["std.deque_u32.front_or", "q", 0]],
            ["set", "q", ["std.deque_u32.pop_front", "q"]],
            ["let", "out", ["vec_u8.with_capacity", 16]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "a"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "b"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "c"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "d"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(1), u32le(2), u32le(3), u32le(0)].concat());
}

#[test]
fn deque_u32_emit_u32le_front_to_back() {
    let program = x07_program::entry(
        &["std.deque_u32"],
        json!([
            "begin",
            ["let", "q", ["std.deque_u32.with_capacity", 2]],
            ["set", "q", ["std.deque_u32.push_back", "q", 10]],
            ["set", "q", ["std.deque_u32.push_back", "q", 20]],
            ["set", "q", ["std.deque_u32.push_back", "q", 30]],
            ["set", "q", ["std.deque_u32.pop_front", "q"]],
            ["set", "q", ["std.deque_u32.push_back", "q", 40]],
            ["std.deque_u32.emit_u32le", "q"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(20), u32le(30), u32le(40)].concat());
}

#[test]
fn heap_u32_push_pop_min() {
    let program = x07_program::entry(
        &["std.heap_u32"],
        json!([
            "begin",
            ["let", "h", ["std.heap_u32.with_capacity", 1]],
            ["set", "h", ["std.heap_u32.push", "h", 3]],
            ["set", "h", ["std.heap_u32.push", "h", 1]],
            ["set", "h", ["std.heap_u32.push", "h", 2]],
            ["let", "a", ["std.heap_u32.min_or", "h", 0]],
            ["set", "h", ["std.heap_u32.pop_min", "h"]],
            ["let", "b", ["std.heap_u32.min_or", "h", 0]],
            ["set", "h", ["std.heap_u32.pop_min", "h"]],
            ["let", "c", ["std.heap_u32.min_or", "h", 0]],
            ["set", "h", ["std.heap_u32.pop_min", "h"]],
            ["let", "d", ["std.heap_u32.min_or", "h", 0]],
            ["set", "h", ["std.heap_u32.pop_min", "h"]],
            ["let", "out", ["vec_u8.with_capacity", 16]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "a"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "b"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "c"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "d"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(1), u32le(2), u32le(3), u32le(0)].concat());
}

#[test]
fn heap_u32_emit_u32le_sorted() {
    let program = x07_program::entry(
        &["std.heap_u32"],
        json!([
            "begin",
            ["let", "h", ["std.heap_u32.with_capacity", 1]],
            ["set", "h", ["std.heap_u32.push", "h", 3]],
            ["set", "h", ["std.heap_u32.push", "h", 1]],
            ["set", "h", ["std.heap_u32.push", "h", 2]],
            ["set", "h", ["std.heap_u32.push", "h", 2]],
            ["std.heap_u32.emit_u32le", "h"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(1), u32le(2), u32le(2), u32le(3)].concat());
}

#[test]
fn bitset_intersection_count() {
    let program = x07_program::entry(
        &["std.bitset"],
        json!([
            "begin",
            ["let", "a", ["std.bitset.new", 16]],
            ["let", "b", ["std.bitset.new", 16]],
            ["set", "a", ["std.bitset.set", "a", 1]],
            ["set", "a", ["std.bitset.set", "a", 2]],
            ["set", "a", ["std.bitset.set", "a", 10]],
            ["set", "b", ["std.bitset.set", "b", 2]],
            ["set", "b", ["std.bitset.set", "b", 3]],
            ["set", "b", ["std.bitset.set", "b", 10]],
            [
                "codec.write_u32_le",
                ["std.bitset.intersection_count", "a", "b"]
            ]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out.as_slice(), u32le(2));
}

#[test]
fn slab_alloc_free_reuse() {
    let program = x07_program::entry(
        &["std.slab"],
        json!([
            "begin",
            ["let", "s", ["std.slab.new_u32", 3]],
            ["let", "h1", ["std.slab.free_head_u32", "s"]],
            ["set", "s", ["std.slab.alloc_u32", "s"]],
            ["let", "h2", ["std.slab.free_head_u32", "s"]],
            ["set", "s", ["std.slab.alloc_u32", "s"]],
            ["set", "s", ["std.slab.set_u32", "s", "h1", 11]],
            ["set", "s", ["std.slab.set_u32", "s", "h2", 22]],
            ["set", "s", ["std.slab.free_u32", "s", "h1"]],
            ["let", "h3", ["std.slab.free_head_u32", "s"]],
            ["set", "s", ["std.slab.alloc_u32", "s"]],
            ["let", "v3", ["std.slab.get_u32", "s", "h3", 0]],
            ["let", "out", ["vec_u8.with_capacity", 16]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "h1"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "h2"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "h3"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "v3"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(1), u32le(2), u32le(1), u32le(11)].concat());
}

#[test]
fn lru_cache_put_get_evicts() {
    let program = x07_program::entry(
        &["std.lru_cache"],
        json!([
            "begin",
            ["let", "c", ["std.lru_cache.new_u32", 2]],
            ["set", "c", ["std.lru_cache.put_u32", "c", 1, 10]],
            ["set", "c", ["std.lru_cache.put_u32", "c", 2, 20]],
            ["let", "v1", ["std.lru_cache.peek_u32_or", "c", 1, 0]],
            ["set", "c", ["std.lru_cache.touch_u32", "c", 1]],
            ["set", "c", ["std.lru_cache.put_u32", "c", 3, 30]],
            ["let", "v2", ["std.lru_cache.peek_u32_or", "c", 2, 0]],
            ["let", "v3", ["std.lru_cache.peek_u32_or", "c", 3, 0]],
            ["let", "n", ["std.lru_cache.len_u32", "c"]],
            ["let", "out", ["vec_u8.with_capacity", 16]],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "v1"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "v2"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "v3"]]
            ],
            [
                "set",
                "out",
                ["vec_u8.extend_bytes", "out", ["codec.write_u32_le", "n"]]
            ],
            ["vec_u8.into_bytes", "out"]
        ]),
    );
    let exe = compile_exe(program.as_slice());
    let out = run_exe(&exe, b"");
    assert_eq!(out, [u32le(10), u32le(0), u32le(30), u32le(2)].concat());
}
