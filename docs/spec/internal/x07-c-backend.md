Spec-ID: x07.spec.internal.x07-c-backend@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# C Backend (Native Execution)

X07’s execution pipeline is:

1. The **language core** (compiler built-ins + embedded stdlib modules) defines the surface (no macro/rewrite layer).
2. **Program** (UTF-8 x07AST JSON, usually `*.x07.json`) is compiled to **C source**.
3. The runner compiles the C source to a **native executable** (“solver artifact”).
4. The runner executes the artifact under deterministic limits and collects output + metrics.

Notes:
- The canonical LLM-facing surface is x07AST JSON (`spec/x07ast.schema.json`), with expressions encoded as JSON S-expressions (json-sexpr).
- The compiler accepts only x07AST JSON (tool contracts are JSON-first).

## Solver I/O ABI (bytes → bytes)

The solver artifact is a native executable that:

- Reads from stdin: `u32_le input_len` then `input_len` bytes.
- Writes to stdout: `u32_le output_len` then `output_len` bytes.
- Writes metrics to stderr as a single JSON line at exit, including deterministic memory stats.
  - Example shape:
    - `{"fuel_used":123,"heap_used":456,"fs_read_file_calls":7,"rr_open_calls":1,"rr_close_calls":1,"rr_stats_calls":0,"rr_next_calls":3,"rr_next_miss_calls":0,"rr_append_calls":0,"kv_get_calls":2,"kv_set_calls":0,"mem_stats":{"alloc_calls":1,"realloc_calls":2,"free_calls":1,"bytes_alloc_total":64,"bytes_freed_total":64,"live_bytes":0,"peak_live_bytes":64,"live_allocs":0,"peak_live_allocs":3,"memcpy_bytes":64},"sched_stats":{"tasks_spawned":3,"spawn_calls":3,"join_calls":3,"yield_calls":0,"sleep_calls":3,"chan_send_calls":0,"chan_recv_calls":0,"ctx_switches":10,"wake_events":3,"blocked_waits":0,"virtual_time_end":55,"sched_trace_hash":"0x..."},"debug_stats":{"borrow_violations":0}}`

Notes:
- The compiled program sees `input` as a `bytes_view` (borrowed) and must return owned `bytes`.
- `mem_stats` are reset after reading the input payload, so input bytes are excluded from memory gates/scoring.
- In debug-borrow builds, `debug_stats.borrow_violations` is emitted and can be gated by benchmark suites.
- ABI v2 (C-facing value layouts) is specified under `docs/spec/abi/` and `crates/x07c/include/x07_abi_v2.h`.

## Worlds (capability profiles)

- `solve-pure`: pure compute only.
- `solve-fs`: deterministic read-only fixture filesystem mounted as current directory (`.`).
  - `["fs.read", path_bytes]` reads a fixture file with a safe relative-path policy.
  - Phase G2 streaming adapters:
    - `["fs.open_read", path_bytes]` → `iface` reader
    - `["io.read", reader_iface, max_i32]` → next chunk (bytes)
- `solve-rr`: deterministic replay/record backed by cassette files under `./.x07_rr/`.
  - Cassette files are `*.rrbin` and contain u32-le framed RR `entry_v1` records (DataModel docs).
  - Core builtins:
    - `["rr.open_v1", cfg_bytes_view]` → `result_i32` (ok = handle)
    - `["rr.close_v1", handle_i32]` → `i32`
    - `["rr.stats_v1", handle_i32]` → `bytes` (JSON)
    - `["rr.next_v1", handle_i32, kind_bytes_view, op_bytes_view, key_bytes_view]` → `result_bytes` (ok = entry bytes)
    - `["rr.append_v1", handle_i32, entry_bytes_view]` → `result_i32`
  - Entry helpers:
    - `["rr.entry_resp_v1", entry_bytes_view]` → `bytes`
    - `["rr.entry_err_v1", entry_bytes_view]` → `i32`
  - Programs typically use `std.rr.with_policy_v1` / `std.rr.with_v1` to manage `rr.open_v1`/`rr.close_v1` as a structured scope.
- `solve-kv`: deterministic key/value store (seeded per case; reset per case).
  - `["kv.get", key_bytes]` returns the value bytes or empty if missing.
  - `["kv.get_async", key_bytes]` returns the value bytes or empty if missing.
  - `["kv.get_stream", key_bytes]` returns an `iface` reader for streaming reads.
  - `["kv.set", key_bytes, val_bytes]` sets a value and returns 1 if inserted, 0 if updated.
  - The seed is loaded from `./.x07_kv/seed.evkv` at process start.
  - Phase G2 latency index: runner may also materialize `./.x07_kv/latency.evkvlat` (used to advance virtual time for `kv.get`).
- `solve-full`: includes fs + rr + kv.

Standalone-only (not used in deterministic suites; run via `x07-os-runner`):

- `run-os`: real OS access (non-deterministic by design).
- `run-os-sandboxed`: same surface, but restricted by a policy file (see `schemas/run-os-policy.schema.json`).

Phase H3 adds standalone-only OS builtins (compile-time gated to `run-os*`):

- `os.fs.read_file(path: bytes) -> bytes`
- `os.fs.write_file(path: bytes, data: bytes) -> i32` (0 on success; errno-like code on failure)
- `os.env.get(key: bytes) -> bytes` (empty if missing)
- `os.time.now_unix_ms() -> i32` (low 32 bits of unix ms)
- `os.time.now_instant_v1() -> bytes` (InstantDocV1; encoded like DurationDocV1)
- `os.time.sleep_ms_v1(ms: i32) -> i32` (1 on success; 0 on failure/policy denied)
- `os.time.local_tzid_v1() -> bytes` (LocalTzidResultV1 doc; see `docs/time/os-time-v1.md`)
- `os.process.exit(code: i32) -> never`
- `os.net.http_request(req: bytes) -> bytes` (currently traps; reserved for later)
  - OS-world networking today: use `ext-net` (`packages/ext/x07-ext-net/0.1.5/`) via `std.net.*` (see `docs/guides/networking.md` and `docs/net/net-v1.md`).

Pinned tzdb builtins (deterministic; used by `ext.time.tzdb`, see `docs/time/tzdb-v1.md`):

- `os.time.tzdb_is_valid_tzid_v1(tzid: bytes_view) -> i32`
- `os.time.tzdb_offset_duration_v1(tzid: bytes_view, unix_s_lo: i32, unix_s_hi: i32) -> bytes`
- `os.time.tzdb_snapshot_id_v1() -> bytes`

Phase H4 adds standalone-only unsafe + FFI (compile-time gated to `run-os*`):

- `unsafe` blocks (`["unsafe", ...]`) and raw pointer types (`ptr_const_u8`, `ptr_mut_u8`, `ptr_const_void`, `ptr_mut_void`, `ptr_const_i32`, `ptr_mut_i32`).
- Pointer + memory intrinsics (`ptr.*`, `addr_of*`, `memcpy`/`memmove`/`memset`) and `extern` C function declarations/calls (unsafe + `ffi` capability).

## Determinism & limits

Determinism is enforced by:

- A deterministic fuel counter consumed by generated code (`rt_fuel(ctx, 1)` per AST node evaluation).
- A fixed-capacity deterministic heap allocator capped by `X07_MEM_CAP` (compile-time macro), with explicit `free` (used by `bytes`/`vec_u8` drops).
- A fixed environment: no inherited args/env; the process runs in an isolated temp working directory.
- Unix resource limits (`setrlimit`) as kill-switches (CPU time, file size, fd count, core dumps).
- Bounded stdout/stderr capture in the runner to prevent output-spam from exhausting host memory.

## Toolchain & caching

The runner compiles C using `cc` (override via `X07_CC`) and caches artifacts under:

- `target/x07-native-cache/<sha256>/solver`

Cache keys include the C source, compiler version string, and runner compile-time configuration.

To inject extra C toolchain arguments (used for sanitizer gates), set:

- `X07_CC_ARGS="-fsanitize=address,undefined -fno-omit-frame-pointer -g -O1"`

To build smaller native artifacts (useful when linking native extension archives), use:

- `x07-host-runner --cc-profile size ...` (or `x07-os-runner --cc-profile size ...`)

This appends a platform-specific set of size-focused flags to `X07_CC_ARGS` (for example: `-Os` plus linker dead-stripping on macOS, or `--gc-sections` on Linux). `X07_CC_ARGS` remains the escape hatch for custom toolchain flags.

For standalone OS runs that use external FFI packages, prefer `x07-os-runner --auto-ffi` so the runner compiles `ffi/*.c` sources and links `x07-package.json` `meta.ffi_libs` automatically.

To keep the generated C source for inspection, set `X07_KEEP_C=1` and the runner will write:

- `target/x07-native-cache/<sha256>/solver.c`

## Debug borrow checks

To enable debug-only borrow/lifetime instrumentation in the C backend (Phase G1), run the host runner with:

- `--debug-borrow-checks`
