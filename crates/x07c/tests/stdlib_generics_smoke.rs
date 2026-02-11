use serde_json::json;
use x07c::compile::{compile_program_to_c_with_meta, CompileOptions};

mod x07_program;

#[test]
fn compile_accepts_generic_std_heap_u32() {
    let program = x07_program::entry(
        &["std.heap"],
        vec![],
        json!([
            "begin",
            ["let", "h", ["tapp", "std.heap.with_capacity", "u32", 4]],
            ["set", "h", ["tapp", "std.heap.push", "u32", "h", 2]],
            ["set", "h", ["tapp", "std.heap.push", "u32", "h", 1]],
            ["tapp", "std.heap.emit_le", "u32", "h"]
        ]),
    );

    let out = compile_program_to_c_with_meta(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
    let mono_map = out.mono_map.expect("mono map must be emitted");

    assert!(
        mono_map.items.iter().any(|it| {
            it.generic == "std.heap.push"
                && it.type_args == vec![json!("u32")]
                && it
                    .specialized
                    .starts_with("std.heap.push__x07_mono_v1__u32__h")
        }),
        "missing specialization for std.heap.push<u32>: {:?}",
        mono_map.items
    );
}

#[test]
fn compile_accepts_generic_std_deque_u32() {
    let program = x07_program::entry(
        &["std.deque"],
        vec![],
        json!([
            "begin",
            ["let", "dq", ["tapp", "std.deque.with_capacity", "u32", 4]],
            ["set", "dq", ["tapp", "std.deque.push_back", "u32", "dq", 1]],
            ["set", "dq", ["tapp", "std.deque.push_back", "u32", "dq", 2]],
            ["tapp", "std.deque.emit_le", "u32", ["bytes.view", "dq"]]
        ]),
    );

    let out = compile_program_to_c_with_meta(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
    let mono_map = out.mono_map.expect("mono map must be emitted");

    assert!(
        mono_map.items.iter().any(|it| {
            it.generic == "std.deque.push_back"
                && it.type_args == vec![json!("u32")]
                && it
                    .specialized
                    .starts_with("std.deque.push_back__x07_mono_v1__u32__h")
        }),
        "missing specialization for std.deque.push_back<u32>: {:?}",
        mono_map.items
    );
}

#[test]
fn compile_accepts_generic_stdlib_containers() {
    let program = x07_program::entry(
        &[
            "std.btree_map",
            "std.btree_set",
            "std.deque",
            "std.hash_map",
            "std.hash_set",
            "std.heap",
            "std.lru_cache",
            "std.slab",
            "std.small_map",
            "std.small_set",
        ],
        vec![],
        json!([
            "begin",
            ["let", "h", ["tapp", "std.heap.with_capacity", "u32", 4]],
            ["set", "h", ["tapp", "std.heap.push", "u32", "h", 2]],
            ["set", "h", ["tapp", "std.heap.push", "u32", "h", 1]],
            ["let", "hb", ["tapp", "std.heap.emit_le", "u32", "h"]],
            ["let", "dq", ["tapp", "std.deque.with_capacity", "u32", 4]],
            ["set", "dq", ["tapp", "std.deque.push_back", "u32", "dq", 1]],
            ["set", "dq", ["tapp", "std.deque.push_back", "u32", "dq", 2]],
            [
                "let",
                "dqb",
                ["tapp", "std.deque.emit_le", "u32", ["bytes.view", "dq"]]
            ],
            ["let", "m", ["std.btree_map.empty"]],
            [
                "set",
                "m",
                ["tapp", "std.btree_map.put", "u32", "u32", "m", 1, 2]
            ],
            [
                "let",
                "mb",
                ["tapp", "std.btree_map.emit_kv_le", "u32", "u32", "m"]
            ],
            ["let", "s", ["std.btree_set.empty"]],
            ["set", "s", ["tapp", "std.btree_set.insert", "u32", "s", 3]],
            ["let", "sb", ["tapp", "std.btree_set.emit_le", "u32", "s"]],
            ["let", "kbytes_view", ["bytes.lit", "k"]],
            ["let", "kview", ["bytes.view", "kbytes_view"]],
            ["let", "kbytes_set", ["bytes.lit", "k"]],
            ["let", "sm", ["std.small_map.empty"]],
            [
                "set",
                "sm",
                ["tapp", "std.small_map.put", "u32", "sm", "kview", 7]
            ],
            ["let", "ss", ["std.small_set.empty"]],
            [
                "set",
                "ss",
                ["tapp", "std.small_set.insert", "bytes", "ss", "kbytes_set"]
            ],
            ["let", "hm", ["std.hash_map.with_capacity", 4]],
            [
                "set",
                "hm",
                ["tapp", "std.hash_map.set", "u32", "u32", "hm", 1, 9]
            ],
            [
                "let",
                "hmb",
                ["tapp", "std.hash_map.emit_kv_le", "u32", "u32", "hm"]
            ],
            ["let", "hs", ["std.hash_set.new", 8]],
            ["set", "hs", ["tapp", "std.hash_set.add", "u32", "hs", 5]],
            ["let", "hsb", ["tapp", "std.hash_set.emit_le", "u32", "hs"]],
            ["let", "slab", ["tapp", "std.slab.new", "u32", 4]],
            ["let", "h0", ["std.slab.free_head", ["bytes.view", "slab"]]],
            ["set", "slab", ["std.slab.alloc", "slab"]],
            [
                "set",
                "slab",
                ["tapp", "std.slab.set", "u32", "slab", "h0", 11]
            ],
            [
                "let",
                "slabv",
                [
                    "tapp",
                    "std.slab.get_or",
                    "u32",
                    ["bytes.view", "slab"],
                    "h0",
                    0
                ]
            ],
            ["let", "c", ["std.lru_cache.new", 4]],
            [
                "set",
                "c",
                ["tapp", "std.lru_cache.put", "u32", "u32", "c", 1, 22]
            ],
            [
                "let",
                "cv",
                [
                    "tapp",
                    "std.lru_cache.peek_or",
                    "u32",
                    "u32",
                    ["bytes.view", "c"],
                    1,
                    0
                ]
            ],
            ["let", "nb", ["codec.write_u32_le", ["+", "slabv", "cv"]]],
            [
                "bytes.concat",
                "hb",
                [
                    "bytes.concat",
                    "dqb",
                    [
                        "bytes.concat",
                        "mb",
                        [
                            "bytes.concat",
                            "sb",
                            ["bytes.concat", "hmb", ["bytes.concat", "hsb", "nb"]]
                        ]
                    ]
                ]
            ]
        ]),
    );

    let out = compile_program_to_c_with_meta(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
    let mono_map = out.mono_map.expect("mono map must be emitted");

    let has = |generic: &str, type_args: Vec<serde_json::Value>| {
        mono_map.items.iter().any(|it| {
            it.generic == generic
                && it.type_args == type_args
                && it
                    .specialized
                    .starts_with(&format!("{generic}__x07_mono_v1__"))
        })
    };

    assert!(has("std.heap.push", vec![json!("u32")]));
    assert!(has("std.deque.push_back", vec![json!("u32")]));
    assert!(has("std.btree_map.put", vec![json!("u32"), json!("u32")]));
    assert!(has("std.btree_set.insert", vec![json!("u32")]));
    assert!(has("std.small_map.put", vec![json!("u32")]));
    assert!(has("std.small_set.insert", vec![json!("bytes")]));
    assert!(has("std.hash_map.set", vec![json!("u32"), json!("u32")]));
    assert!(has("std.hash_set.add", vec![json!("u32")]));
    assert!(has("std.slab.new", vec![json!("u32")]));
    assert!(has("std.lru_cache.put", vec![json!("u32"), json!("u32")]));
}

#[test]
fn compile_accepts_generic_stdlib_value_containers_bytes() {
    let program = x07_program::entry(
        &["std.vec_value", "std.hash_map_value"],
        vec![],
        json!([
            "begin",
            ["let", "v", ["std.vec_value.with_capacity_bytes", 4]],
            [
                "set",
                "v",
                ["std.vec_value.push_bytes", "v", ["bytes.lit", "a"]]
            ],
            [
                "let",
                "x",
                ["std.vec_value.get_bytes_or", "v", 0, ["bytes.lit", "x"]]
            ],
            [
                "let",
                "m",
                ["std.hash_map_value.with_capacity_bytes_bytes", 4]
            ],
            [
                "set",
                "m",
                [
                    "std.hash_map_value.set_bytes_bytes",
                    "m",
                    ["bytes.lit", "k"],
                    ["bytes.lit", "v"]
                ]
            ],
            [
                "let",
                "y",
                [
                    "std.hash_map_value.get_bytes_bytes_or",
                    "m",
                    ["bytes.lit", "k"],
                    ["bytes.lit", "d"]
                ]
            ],
            ["bytes.concat", "x", "y"]
        ]),
    );

    let out = compile_program_to_c_with_meta(program.as_slice(), &CompileOptions::default())
        .expect("program must compile");
    let mono_map = out.mono_map.expect("mono map must be emitted");

    let has = |generic: &str, type_args: Vec<serde_json::Value>| {
        mono_map.items.iter().any(|it| {
            it.generic == generic
                && it.type_args == type_args
                && it
                    .specialized
                    .starts_with(&format!("{generic}__x07_mono_v1__"))
        })
    };

    assert!(has("std.vec_value.push", vec![json!("bytes")]));
    assert!(has("std.vec_value.get_or", vec![json!("bytes")]));
    assert!(has(
        "std.hash_map_value.set",
        vec![json!("bytes"), json!("bytes")]
    ));
    assert!(has(
        "std.hash_map_value.get_or",
        vec![json!("bytes"), json!("bytes")]
    ));
}
