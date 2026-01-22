# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**X07** is an experimental toolchain for machine-authored programs: a deterministic compiler + runner for x07AST JSON programs (`solve(bytes_view)->bytes`).

Track B model:

- Programs are **x07AST JSON** (`*.x07.json`, `x07.x07ast@0.1.0`) with expressions encoded as json-sexpr (`["head", ...]`).
- The toolchain enforces an agent-friendly loop: `x07 fmt` → `x07 lint`/`x07 fix` → `x07 ast apply-patch` → `x07 run` / `x07 test` (JSON Patch is RFC 6902).
- The built-in test harness (`x07 test`) runs manifest-declared tests deterministically and emits a strict `x07test` JSON report.
- The compiler (`crates/x07c`) compiles a program to self-contained C.
- The runner (`crates/x07-host-runner`) compiles the C to a native executable and runs it deterministically.
- The standalone OS runner (`crates/x07-os-runner`) compiles+runs programs in standalone-only worlds (`run-os*`), which are not used by benchmark suites.

Execution details and solver ABI: `../dev-docs/x07-internal-docs/spec/x07-c-backend.md`.

## Stdlib and builtins (canonical surface)

- Compiler-embedded stdlib: `stdlib/std/0.1.1/`.
- Removed builtin aliases: `vec_u8.new` and `str.*` (strings are `bytes`).
  - Use `vec_u8.with_capacity` and finalize with `std.vec.as_bytes` (wraps the builtin `vec_u8.into_bytes`).
- Added performance builtins:
  - `vec_u8.extend_bytes_range` for appending `bytes[start..start+len]` into a vec.
  - `bytes.cmp_range` for lexicographic range compare (`-1/0/1`).
- Arithmetic builtins:
  - `/` and `%` are unsigned u32 ops (mod 2^32): `/` by 0 yields 0, and `%` by 0 yields the numerator.
- Views + debug borrow checks:
  - `bytes_view` type and `view.*` builtins for explicit, zero-copy scanning.
  - View builtins: `bytes.view`, `bytes.subview`, `view.len`, `view.get_u8`, `view.slice`, `view.to_bytes`, `view.eq`, `view.cmp_range`, `vec_u8.as_view`.
- Deterministic concurrency + streaming I/O:
  - `defasync` (async function), `await` / `task.join.bytes` (wait for bytes).
  - Task builtins: `task.spawn`, `task.join.bytes`, `task.yield`, `task.sleep`, `task.cancel`.
  - Channel builtins: `chan.bytes.new`, `chan.bytes.send`, `chan.bytes.recv`, `chan.bytes.close`.
  - Streaming I/O builtins: `io.read`, `bufread.new`, `bufread.fill`, `bufread.consume`.
  - World adapters: `fs.open_read`, `rr.fetch`, `rr.send`, `kv.get_async`, `kv.get_stream` (streaming adapters return `iface` readers).
- Stdlib parity helpers:
  - `std.csv`: `sum_second_col_i32_status_le`, `sum_second_col_i32le_or_err`.
  - `std.prng`: `x07rand32_v1_stream` (X07RAND32 v1; pure).
  - `std.bit`: `popcount_u32` (pure).
  - `std.regex-lite`: `count_matches_u32le` (letters + `.` + `*` subset).
  - `std.fs`: `read`, `read_async`, `read_task`, `open_read`, `list_dir`, `list_dir_sorted`.
    - Phase H3: `std.fs.read*` binds through `std.world.fs` (fixture-backed in `solve-fs`, OS-backed in `run-os*`).
  - `std.bytes`: bytes_view helpers `max_u8`, `sum_u8`, `count_u8`, `starts_with`, `ends_with`.
  - `std.text.ascii`: `split_u8`, `split_lines_view` (X7SL v1 slice lists; access via `std.text.slices`).
- Testing helpers:
  - `std.test`: deterministic assertions + `X7TEST_STATUS_V1` encoder (`std.test.code_assert_*`, `std.test.assert_*`, `std.test.status_from_result_i32`).
- Collections (Phase H2 part2):
  - `std.small_map` / `std.small_set`: sorted packed bytes collections (deterministic, compact; O(n) inserts).
  - `std.hash`: deterministic hashes (`fnv1a32_*`, `mix32`).
  - `std.hash_map` / `std.hash_set`: u32 map/set wrappers; `std.hash_set` also provides a view-key set for `input`-range keys.
  - `std.btree_map` / `std.btree_set`: ordered u32 collections (binary search + packed bytes storage).
  - `std.deque_u32`: growable ring-buffer deque for u32.
  - `std.heap_u32`: min-heap priority queue for u32.
  - Collection emitters: `std.*.emit_*` functions return canonical deterministic encodings (hash emitters canonicalize by sorting). Spec: `../dev-docs/x07-internal-docs/spec/stdlib-emitters-v1.md`.
  - `std.bitset`: dense bitset with `intersection_count`.
  - `std.slab`: handle-based pool for u32 values.
  - `std.lru_cache`: fixed-capacity LRU cache for u32 keys/values (`peek_u32_opt`/`peek_u32_or` + `touch_u32` + `put_u32`; mutating ops return updated bytes).

## Types + ABI (stable surface)

- ABI v2 docs: `../dev-docs/x07-internal-docs/spec/abi/` and header `crates/x07c/include/x07_abi_v2.h`.
- Type annotations are supported in `defn` signatures for:
  - `i32`, `bytes`, `bytes_view`, `vec_u8`, `option_i32`, `option_bytes`, `result_i32`, `result_bytes`, `iface`.
  - Standalone-only raw pointers: `ptr_const_u8`, `ptr_mut_u8`, `ptr_const_void`, `ptr_mut_void`, `ptr_const_i32`, `ptr_mut_i32`.
- Option / Result builtins:
  - `option_i32.*`, `option_bytes.*`, `result_i32.*`, `result_bytes.*`.
  - `["try", <result_expr>]` for early-return propagation (requires function return type `result_i32` or `result_bytes`).

## Unsafe + FFI (standalone-only, Phase H4)

- Only enabled in `run-os` / `run-os-sandboxed` worlds.
- `["unsafe", ...]` blocks gate unsafe operations; unsafe-only ops error outside.
- Pointer builtins: `bytes.as_ptr`, `bytes.as_mut_ptr`, `view.as_ptr`, `vec_u8.as_ptr`, `vec_u8.as_mut_ptr`, `ptr.null`, `ptr.as_const`, `ptr.cast`, `addr_of`, `addr_of_mut`, `ptr.add/sub/offset`, `ptr.read/write_{u8,i32}`, `memcpy/memmove/memset`.
- C interop: `decls[]` can include `{"kind":"extern","abi":"C",...}` and extern calls require `unsafe` blocks and `ffi` capability.

## Capability Worlds

- `solve-pure`: pure bytes → bytes, no I/O.
- `solve-fs`: read-only fixture filesystem provided as `.`; file reads via `["fs.read", path_bytes]`.
- `solve-rr`: deterministic fixture-backed request/response (no real network).
- `solve-kv`: deterministic seeded key/value store (reset per case).
- `solve-full`: fs + rr + kv combined.
- Standalone-only (not used by benchmark suites):
  - `run-os`: real OS access (non-deterministic by design).
  - `run-os-sandboxed`: policy-restricted OS access (see `schemas/run-os-policy.schema.json`).

Benchmark suites:

- H1 (solve-pure): `benchmarks/solve-pure/phaseH1-suite.json`
  - Fast gate: `benchmarks/solve-pure/phaseH1-smoke.json`
  - Debug-only gate: `benchmarks/solve-pure/phaseH1-debug-suite.json`
- H2 (stdlib parity): `benchmarks/solve-*/phaseH2-suite.json`
  - Fast gate (solve-full): `benchmarks/solve-full/phaseH2-smoke.json`
- H2 collections (solve-pure): `benchmarks/solve-pure/phaseH2-collections-suite.json`
- Stdlib emitters canary (solve-pure): `benchmarks/solve-pure/emitters-v1-suite.json`

Suite bundles (multi-suite runs):

- H1: `benchmarks/bundles/phaseH1.json`
- H2: `benchmarks/bundles/phaseH2.json`
- H1+H2: `benchmarks/bundles/phaseH1H2.json`

## Development Commands

- `cargo fmt --check`
- `cargo test`
- `cargo test -p x07`
- `cargo clippy --all-targets -- -D warnings`
- `./scripts/ci/check_canaries.sh`
- `./scripts/ci/check_x07import_generated.sh`
- `./scripts/ci/check_x07import_diagnostics_sync.sh`
- `./scripts/ci/check_suites_h1h2.sh`
- `./scripts/ci/check_asan_c_backend.sh`
- `./scripts/ci/check_stdlib_lock.sh`
- `python3 scripts/bench/generate_phase4_curriculum.py --check`
- `python3 scripts/bench/run_bench_suite.py --suite benchmarks/bundles/phaseH1H2.json`
- `cargo run -p x07 -- test --manifest tests/tests.json`

Project workflow (modules/packages):

- `cargo run -p x07c -- lock --project <project/x07.json>`
- `cargo run -p x07c -- build --project <project/x07.json> --out <out.c>`
- `cargo run -p x07-host-runner -- --project <project/x07.json> --world solve-pure --input <case.bin>`

## Repository Structure

```
.
├─ docs/                          # Design docs
├─ benchmarks/                    # Suites + fixtures + reference solutions
├─ crates/
│  ├─ x07c/                   # Compiler (X07 -> C)
│  └─ x07-host-runner/        # Deterministic native runner
└─ scripts/bench/                 # Benchmark tooling
```
