pub fn builtin_module_source(module_id: &str) -> Option<&'static str> {
    match module_id {
        "std.vec" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/vec.x07.json"
        )),
        "std.vec_value" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/vec_value.x07.json"
        )),
        "std.slice" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/slice.x07.json"
        )),
        "std.bytes" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/bytes.x07.json"
        )),
        "std.codec" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/codec.x07.json"
        )),
        "std.parse" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/parse.x07.json"
        )),
        "std.fmt" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/fmt.x07.json"
        )),
        "std.prng" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/prng.x07.json"
        )),
        "std.pbt.case_v1" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/pbt/case_v1.x07.json"
        )),
        "std.pbt.gen_v1" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/pbt/gen_v1.x07.json"
        )),
        "std.pbt.shrink_v1" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/pbt/shrink_v1.x07.json"
        )),
        "std.bit" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/bit.x07.json"
        )),
        "std.text.ascii" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/text/ascii.x07.json"
        )),
        "std.text.slices" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/text/slices.x07.json"
        )),
        "std.text.utf8" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/text/utf8.x07.json"
        )),
        "std.test" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/test.x07.json"
        )),
        "std.regex-lite" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/regex-lite.x07.json"
        )),
        "std.json" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/json.x07.json"
        )),
        "std.http.envelope" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/http/envelope.x07.json"
        )),
        "std.csv" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/csv.x07.json"
        )),
        "std.map" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/map.x07.json"
        )),
        "std.set" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/set.x07.json"
        )),
        "std.u32" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/u32.x07.json"
        )),
        "std.small_map" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/small_map.x07.json"
        )),
        "std.small_set" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/small_set.x07.json"
        )),
        "std.hash" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/hash.x07.json"
        )),
        "std.hash_map" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/hash_map.x07.json"
        )),
        "std.hash_map_value" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/hash_map_value.x07.json"
        )),
        "std.hash_set" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/hash_set.x07.json"
        )),
        "std.btree_map" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/btree_map.x07.json"
        )),
        "std.btree_set" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/btree_set.x07.json"
        )),
        "std.deque" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/deque.x07.json"
        )),
        "std.deque_u32" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/deque_u32.x07.json"
        )),
        "std.heap" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/heap.x07.json"
        )),
        "std.heap_u32" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/heap_u32.x07.json"
        )),
        "std.bitset" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/bitset.x07.json"
        )),
        "std.slab" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/slab.x07.json"
        )),
        "std.lru_cache" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/lru_cache.x07.json"
        )),
        "std.result" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/result.x07.json"
        )),
        "std.option" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/option.x07.json"
        )),
        "std.io" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/io.x07.json"
        )),
        "std.io.bufread" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/io/bufread.x07.json"
        )),
        "std.world.fs" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/world/fs.x07.json"
        )),
        "std.fs" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/fs.x07.json"
        )),
        "std.rr" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/rr.x07.json"
        )),
        "std.kv" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/kv.x07.json"
        )),
        "std.path" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/path.x07.json"
        )),
        "std.process" => Some(include_str!(
            "../../../stdlib/std/0.1.2/modules/std/process.x07.json"
        )),
        _ => None,
    }
}

pub fn builtin_module_ids() -> &'static [&'static str] {
    &[
        "std.vec",
        "std.vec_value",
        "std.slice",
        "std.bytes",
        "std.codec",
        "std.parse",
        "std.fmt",
        "std.prng",
        "std.pbt.case_v1",
        "std.pbt.gen_v1",
        "std.pbt.shrink_v1",
        "std.bit",
        "std.text.ascii",
        "std.text.slices",
        "std.text.utf8",
        "std.test",
        "std.regex-lite",
        "std.json",
        "std.http.envelope",
        "std.csv",
        "std.map",
        "std.set",
        "std.u32",
        "std.small_map",
        "std.small_set",
        "std.hash",
        "std.hash_map",
        "std.hash_map_value",
        "std.hash_set",
        "std.btree_map",
        "std.btree_set",
        "std.deque",
        "std.deque_u32",
        "std.heap",
        "std.heap_u32",
        "std.bitset",
        "std.slab",
        "std.lru_cache",
        "std.result",
        "std.option",
        "std.io",
        "std.io.bufread",
        "std.world.fs",
        "std.fs",
        "std.rr",
        "std.kv",
        "std.path",
        "std.process",
    ]
}
