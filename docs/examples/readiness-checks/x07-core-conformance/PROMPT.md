# Prompt 1 — Core stdlib conformance and determinism harness

```text
You are a senior X07 coding agent. Create a repo/workspace named `x07-core-conformance` that serves as the “pure stdlib + core language” conformance harness.

GOAL
Build a single executable program (entrypoint `app.main`) that runs an internal battery of checks and prints ONE canonical JSON report summarizing pass/fail per stdlib module and per capability. The program must be deterministic: given the same input bytes, it produces identical output bytes.

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir myapp
cd myapp
x07 init

TOOLCHAIN + PROJECT SETUP
1) Pin toolchain to the latest.
2) Edit `x07.json` (created by `x07 init`) to set:
   - world = "solve-pure" (deterministic compile/lint/repair world; run OS worlds via profiles)
   - module_roots = ["src"]
   - default_profile = "sandbox"
   - profiles.sandbox.world = "run-os-sandboxed"
   - profiles.sandbox.policy = "policy/sandbox.base.json" (deny all net, deny all filesystem by default).
3) Generate a schema-valid base sandbox policy:
   - `x07 policy init --template worker --out policy/sandbox.base.json --mkdir-out`
4) Add dependencies:
   - ext-cli (for argv_v1 parsing)
5) Run `x07 pkg lock` and commit `x07.lock.json`.

FUNCTIONAL REQUIREMENTS
A) Exercise X07 core memory model patterns explicitly:
   - bytes vs bytes_view for scanning/splitting
   - vec_u8 builder patterns (with_capacity / extend_bytes / as_bytes)
   - option_* and result_* idioms (no sentinel “ERR strings”)
B) Exercise deterministic async concurrency (cooperative tasks) using `task.scope_v1` (no orphan tasks), e.g. a small async pipeline (map-reduce over byte chunks), and prove identical output across runs.
C) Exercise high-level primitives in a pure context:
   - `budget.scope_v1` around a hot loop (use a small nonzero cap and assert it does not trip)
   - `std.stream.pipe_v1` with a `std.stream.src.bytes_v1` source, a `std.stream.xf.split_lines_v1` stage, and a `std.stream.sink.collect_bytes_v1` sink

STDLIB COVERAGE (must call AT LEAST ONE export in each module listed below)
Implement one `check_<module>() -> result_i32` per module and call them from `app.main`, collecting results:
NOTE: `std.csv` is removed from the prod surface; for CSV parsing coverage, use `std.result.chain_sum_csv_i32`.
- std.bit (popcount_u32)
- std.bitset (new/set/test/intersection_count)
- std.btree_map (empty_u32_u32/put_u32_u32/len/get)
- std.btree_set (empty_u32/insert/contains/len)
- std.bytes (alloc/len/slice/concat/reverse/find_u8/count_u8/etc.)
- std.codec (read_u32_le/write_u32_le)
- std.deque_u32 (with_capacity/push_back/pop_front/front_or/len)
- std.fmt (u32_to_dec, s32_to_dec)
- std.hash (fnv1a32_bytes/mix32)
- std.hash_map (new/set/get/contains/len)
- std.hash_set (new/add/contains/len/emit or view_* APIs)
- std.heap_u32 (with_capacity/push/pop_min/min_or/len)
- std.io + std.io.bufread (read_all; bufread.new/fill/consume; read from ext.streams is NOT allowed here)
- std.json (canonicalize_small and extract_path_canon_or_err)
- std.lru_cache (new/put/peek/touch/len)
- std.map (word_freq_sorted_ascii)
- std.option (none/some/is_some/payload/unwrap_i32_le)
- std.parse (u32_dec, i32_status_le etc.)
- std.path (join/basename/extname)
- std.prng (lcg_next_u32 and x07rand32_v1_stream)
- std.regex-lite (find_literal/is_match_literal/count_matches_u32le)
- std.result (ok/err/is_ok/payload/unwrap_i32_le/chain_sum_csv_i32)
- std.set (unique_lines_sorted)
- std.slab (new/alloc/set/get/free/len/cap)
- std.slice (clamp/cmp_bytes)
- std.small_map (inc1_bytes_u32/get/put/remove/len)
- std.small_set (empty/insert/contains/len)
- std.test (assert_* and status_v1 helpers)
- std.text.ascii (tokenize_words_lower/split_lines_view/normalize_lines/etc.)
- std.text.slices (builder_* and view_at/validate)
- std.text.utf8 (is_valid/count_codepoints_or_neg1/validate_or_empty)
- std.u32 (read_le_at/write_le_at/pow2_ceil/push_le)
- std.vec (with_capacity/push/get/len/extend_bytes/as_bytes)

OUTPUT
- Output must be canonical JSON bytes (use std.json canonicalization where appropriate).
- Exit with nonzero if any check fails, and include a stable integer error code per failing module.

DELIVERABLES
- Repo with `src/` modules, `x07.json`, `x07.lock.json`, `policy/`, and a short README explaining how to run:
  - x07 run --profile sandbox -- <args>
  - x07 bundle --profile sandbox --out dist/core-conformance
```

