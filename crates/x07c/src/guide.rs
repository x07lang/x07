use x07_contracts::X07AST_SCHEMA_VERSION;

pub fn guide_md() -> String {
    let mut out = String::new();

    out.push_str("# Language Guide (x07AST JSON)\n\n");
    out.push_str("IMPORTANT:\n");
    out.push_str("- Output ONLY one JSON object (no preamble).\n");
    out.push_str("- Do NOT wrap the JSON in Markdown code fences.\n");
    out.push_str("- Do NOT output extra prose.\n");
    out.push_str("- Must satisfy x07AST schema_version `");
    out.push_str(X07AST_SCHEMA_VERSION);
    out.push_str("`.\n\n");
    out.push_str("Program encoding: UTF-8 JSON text.\n\n");
    out.push_str("Entry program object fields:\n");
    out.push_str("- `schema_version`: `");
    out.push_str(X07AST_SCHEMA_VERSION);
    out.push_str("`\n");
    out.push_str("- `kind`: `entry`\n");
    out.push_str("- `module_id`: `main`\n");
    out.push_str("- `imports`: array of module IDs (e.g. `std.bytes`)\n");
    out.push_str("- `decls`: array of decl objects\n");
    out.push_str("- `solve`: expression (must evaluate to bytes)\n\n");

    out.push_str("## Expression Encoding (json-sexpr)\n\n");
    out.push_str("- i32: JSON numbers in range -2147483648..2147483647\n");
    out.push_str("- atom: JSON strings with no whitespace\n");
    out.push_str("- text: JSON strings (whitespace allowed; JSON escapes apply)\n");
    out.push_str("- list: JSON arrays: `[\"head\", arg1, arg2, ...]` (head is an atom string; list must be non-empty)\n\n");

    out.push_str("## Types\n\n");
    out.push_str("- `i32` for integers and conditions (0=false, non-zero=true)\n");
    out.push_str("- `bytes` for owned byte arrays (move-only; outputs and owned buffers)\n");
    out.push_str("- `bytes_view` for borrowed byte views (zero-copy scanning/slicing)\n");
    out.push_str("- `vec_u8` for mutable byte vectors (move-only; capacity-planned builders)\n");
    out.push_str("- `option_i32`, `option_bytes`, `option_bytes_view` for typed optional values\n");
    out.push_str(
        "- `result_i32`, `result_bytes`, `result_bytes_view`, `result_result_bytes` for typed results with deterministic error codes\n",
    );
    out.push_str("- Bytes-like types may carry an optional compile-time brand (see `params[].brand` and `result_brand`)\n");
    out.push_str("  - Brand builtins (`std.brand.*`):\n");
    out.push_str("    - `std.brand.cast_bytes_v1(brand_id, validator_id, b: bytes) -> result_bytes@brand_id`\n");
    out.push_str("    - `std.brand.cast_view_v1(brand_id, validator_id, v: bytes_view) -> result_bytes_view@brand_id`\n");
    out.push_str("    - `std.brand.cast_view_copy_v1(brand_id, validator_id, v: bytes_view) -> result_bytes@brand_id`\n");
    out.push_str(
        "    - `std.brand.assume_bytes_v1(brand_id, b: bytes) -> bytes@brand_id` (unsafe)\n",
    );
    out.push_str("    - `std.brand.erase_bytes_v1(b: bytes@B) -> bytes`, `std.brand.erase_view_v1(v: bytes_view@B) -> bytes_view`\n");
    out.push_str("    - `std.brand.view_v1(b: bytes@B) -> bytes_view@B`\n");
    out.push_str("    - `std.brand.to_bytes_preserve_if_full_v1(v: bytes_view@B) -> bytes`\n");
    out.push_str("- `iface` for interface records (used for streaming readers)\n");
    out.push_str("- Raw pointer types (standalone-only; require unsafe capability): `ptr_const_u8`, `ptr_mut_u8`, `ptr_const_void`, `ptr_mut_void`, `ptr_const_i32`, `ptr_mut_i32`\n\n");
    out.push_str("Move rules (critical):\n");
    out.push_str("- Passing `bytes` / `vec_u8` to a function that expects `bytes` / `vec_u8` **moves** (consumes) the value.\n");
    out.push_str("- `result_i32` / `result_bytes` / `result_result_bytes` values are also move-only; consume them once (use `*_err_code`, `*_unwrap_or`, or `try`).\n");
    out.push_str("- Mutating APIs return the updated value; always bind it with `let`/`set` (example: `[\"set\",\"b\",[\"bytes.set_u8\",\"b\",0,65]]`).\n\n");

    out.push_str("## Builtins\n\n");
    out.push_str("- `input` is the input byte view (bytes_view); refer to it as the atom string `input` in expressions.\n");
    out.push_str("- `iface.make_v1(data: i32, vtable: i32) -> iface` constructs an interface record value.\n\n");

    out.push_str("## Core Forms\n\n");
    out.push_str("- `begin`: `[\"begin\", e1, e2, ...]` evaluates sequentially and returns the last expression\n");
    out.push_str("- `unsafe`: `[\"unsafe\", e1, e2, ...]` evaluates sequentially and returns the last expression; inside it, unsafe-only operations are allowed (standalone-only)\n");
    out.push_str("- `let`: `[\"let\", name, expr]` binds `name` in the current scope\n");
    out.push_str("- `set`: `[\"set\", name, expr]` assigns an existing binding\n");
    out.push_str(
        "- `set0`: `[\"set0\", name, expr]` assigns an existing binding and returns `0` (i32)\n",
    );
    out.push_str(
        "  - Example: `[\"if\", cond, [\"set0\",\"buf\",[\"vec_u8.extend_bytes\",\"buf\",v]], 0]` unifies as `i32`.\n",
    );
    out.push_str("- `if`: `[\"if\", cond, then, else]` branches on non-zero `cond`\n");
    out.push_str("- `for`: `[\"for\", i, start, end, body]` declares `i` (i32) and runs it from `start` to `end-1`\n");
    out.push_str("  - `body` is a single expression; use `begin` for multiple steps.\n");
    out.push_str("- `return`: `[\"return\", expr]` returns early from the current function\n");
    out.push_str("  - In `solve`, the return value must be `bytes`.\n\n");

    out.push_str("## Examples\n\n");
    out.push_str("Echo (returns input):\n");
    out.push_str("```json\n");
    out.push_str("{\"schema_version\":\"");
    out.push_str(X07AST_SCHEMA_VERSION);
    out.push_str(
        "\",\"kind\":\"entry\",\"module_id\":\"main\",\"imports\":[],\"decls\":[],\"solve\":[\"view.to_bytes\",\"input\"]}\n",
    );
    out.push_str("```\n\n");
    out.push_str("Arity reminder:\n");
    out.push_str("- `if` is `[\"if\", cond, then, else]`\n");
    out.push_str("- `for` is `[\"for\", i, start, end, body]`\n");
    out.push_str("- `begin` is `[\"begin\", e1, e2, ...]`\n\n");

    out.push_str("## Modules\n\n");
    out.push_str("Top-level fields:\n\n");
    out.push_str("- `imports`: array of module IDs\n");
    out.push_str("- `decls`: array of declarations (objects)\n");
    out.push_str("- `solve`: expression (entry programs only)\n\n");
    out.push_str("Declaration objects:\n\n");
    out.push_str("- `{\"kind\":\"defn\",\"name\":\"main.f\",\"params\":[{\"name\":\"x\",\"ty\":\"bytes\"}],\"result\":\"bytes\",\"body\":<expr>}`\n");
    out.push_str("- `{\"kind\":\"defasync\",...}` (returns awaited type; calling returns a task handle i32)\n");
    out.push_str("- `{\"kind\":\"extern\",\"abi\":\"C\",\"name\":\"main.c_fn\",\"link_name\":\"c_fn\",\"params\":[{\"name\":\"x\",\"ty\":\"i32\"}],\"result\":\"i32\"}` (standalone-only; `result` may also be `\"void\"`)\n");
    out.push_str(
        "- `{\"kind\":\"export\",\"names\":[\"std.foo.bar\", ...]}` (module files only)\n\n",
    );
    out.push_str("Contracts (optional fields on `defn` / `defasync` in v0.5):\n\n");
    out.push_str("- `requires`: array of preconditions\n");
    out.push_str("- `ensures`: array of postconditions\n");
    out.push_str("- `invariant`: array of function-level invariants\n\n");
    out.push_str("Each clause is an object:\n\n");
    out.push_str("- `id` (optional string)\n");
    out.push_str("- `expr` (expression; must typecheck to `i32`)\n");
    out.push_str("- `witness` (optional array of expressions; evaluated only on failure)\n\n");
    out.push_str("Reserved name:\n\n");
    out.push_str(
        "- `__result` is reserved and is only available inside `ensures` expressions.\n\n",
    );
    out.push_str("Module IDs are dot-separated identifiers like `app.rle` or `std.bytes`.\n\n");
    out.push_str("Module resolution is deterministic:\n\n");
    out.push_str(
        "- Built-in modules: `std.vec`, `std.vec_value`, `std.slice`, `std.bytes`, `std.codec`, `std.parse`, `std.fmt`, `std.prng`, `std.bit`, `std.text.ascii`, `std.text.utf8`, `std.test`, `std.regex-lite`, `std.json`, `std.csv`, `std.map`, `std.set`, `std.u32`, `std.small_map`, `std.small_set`, `std.hash`, `std.hash_map`, `std.hash_map_value`, `std.hash_set`, `std.btree_map`, `std.btree_set`, `std.deque_u32`, `std.heap_u32`, `std.bitset`, `std.slab`, `std.lru_cache`, `std.result`, `std.option`, `std.io`, `std.io.bufread`, `std.fs`, `std.kv`, `std.rr`, `std.world.fs`, `std.path`, `std.os.env`, `std.os.fs`, `std.os.net`, `std.os.process`, `std.os.process.req_v1`, `std.os.process.caps_v1`, `std.os.process_pool`, `std.os.time`\n",
    );
    out.push_str("- Filesystem modules (standalone): `x07 run --program <prog.x07.json> --module-root <dir>` resolves `a.b` to `<dir>/a/b.x07.json`\n\n");
    out.push_str("Standalone binding override:\n\n");
    out.push_str("- In standalone-only worlds (`run-os`, `run-os-sandboxed`), `std.world.*` modules are resolved from `--module-root` only (no built-in fallback).\n");
    out.push_str("- This keeps program source stable (`import std.fs`) while the target world selects the adapter implementation.\n\n");
    out.push_str("Standalone OS builtins:\n\n");
    out.push_str("- These heads are only available in OS worlds (`run-os`, `run-os-sandboxed`):\n");
    out.push_str("  - `os.fs.read_file(path: bytes) -> bytes`\n");
    out.push_str("  - `os.fs.write_file(path: bytes, data: bytes) -> i32`\n");
    out.push_str("  - `os.fs.read_all_v1(path: bytes, caps: bytes) -> result_bytes`\n");
    out.push_str("  - `os.fs.write_all_v1(path: bytes, data: bytes, caps: bytes) -> result_i32`\n");
    out.push_str("  - `os.fs.mkdirs_v1(path: bytes, caps: bytes) -> result_i32`\n");
    out.push_str("  - `os.fs.remove_file_v1(path: bytes, caps: bytes) -> result_i32`\n");
    out.push_str("  - `os.fs.remove_dir_all_v1(path: bytes, caps: bytes) -> result_i32`\n");
    out.push_str("  - `os.fs.rename_v1(src: bytes, dst: bytes, caps: bytes) -> result_i32`\n");
    out.push_str("  - `os.fs.list_dir_sorted_text_v1(path: bytes, caps: bytes) -> result_bytes`\n");
    out.push_str("  - `os.fs.walk_glob_sorted_text_v1(root: bytes, glob: bytes, caps: bytes) -> result_bytes`\n");
    out.push_str("  - `os.fs.stat_v1(path: bytes, caps: bytes) -> result_bytes`\n");
    out.push_str("  - `os.env.get(key: bytes) -> bytes`\n");
    out.push_str("  - `os.time.now_unix_ms() -> i32`\n");
    out.push_str("  - `os.time.now_instant_v1() -> bytes`\n");
    out.push_str("  - `os.time.sleep_ms_v1(ms: i32) -> i32`\n");
    out.push_str("  - `os.time.local_tzid_v1() -> bytes`\n");
    out.push_str("  - `os.process.exit(code: i32) -> never`\n");
    out.push_str("  - `os.process.spawn_capture_v1(req: bytes, caps: bytes) -> i32`\n");
    out.push_str("  - `os.process.spawn_piped_v1(req: bytes, caps: bytes) -> i32`\n");
    out.push_str("  - `os.process.try_join_capture_v1(handle: i32) -> option_bytes`\n");
    out.push_str("  - `os.process.join_capture_v1(handle: i32) -> bytes` (yield boundary)\n");
    out.push_str("  - `os.process.stdout_read_v1(handle: i32, max: i32) -> bytes`\n");
    out.push_str("  - `os.process.stderr_read_v1(handle: i32, max: i32) -> bytes`\n");
    out.push_str("  - `os.process.stdin_write_v1(handle: i32, chunk: bytes) -> i32`\n");
    out.push_str("  - `os.process.stdin_close_v1(handle: i32) -> i32`\n");
    out.push_str("  - `os.process.try_wait_v1(handle: i32) -> i32`\n");
    out.push_str("  - `os.process.join_exit_v1(handle: i32) -> i32` (yield boundary)\n");
    out.push_str("  - `os.process.take_exit_v1(handle: i32) -> i32`\n");
    out.push_str("  - `os.process.kill_v1(handle: i32, sig: i32) -> i32`\n");
    out.push_str("  - `os.process.drop_v1(handle: i32) -> i32`\n");
    out.push_str("  - `os.process.run_capture_v1(req: bytes, caps: bytes) -> bytes`\n");
    out.push_str("    - Note: `req`/`caps` are `bytes` (move-only). To reuse, copy via `std.bytes.copy(req)` / `std.bytes.copy(caps)`.\n");
    out.push_str(
        "  - `os.net.http_request(req: bytes) -> bytes` (currently traps; reserved for later)\n\n",
    );
    out.push_str("Standalone unsafe + FFI:\n\n");
    out.push_str("- Only available in `run-os` / `run-os-sandboxed`.\n");
    out.push_str("- Unsafe block: `[\"unsafe\", ...]`.\n");
    out.push_str("- Pointer creation/casts: `bytes.as_ptr`, `bytes.as_mut_ptr`, `view.as_ptr`, `vec_u8.as_ptr`, `vec_u8.as_mut_ptr`, `ptr.null`, `ptr.as_const`, `ptr.cast`, `addr_of`, `addr_of_mut`.\n");
    out.push_str("- Unsafe-only ops (require `unsafe` block): `ptr.add`, `ptr.sub`, `ptr.offset`, `ptr.read_u8`, `ptr.write_u8`, `ptr.read_i32`, `ptr.write_i32`, `memcpy`, `memmove`, `memset`.\n");
    out.push_str("- `extern` function calls require `unsafe` blocks.\n\n");

    out.push_str("## Functions\n\n");
    out.push_str("- Define with a `decls[]` entry of kind `defn`.\n");
    out.push_str("  - `body` is a single expression; wrap multi-step bodies in `begin`.\n");
    out.push_str("  - `ty` and `ret_ty` are `i32`, `bytes`, `bytes_view`, `vec_u8`, `option_i32`, `option_bytes`, `option_bytes_view`, `result_i32`, `result_bytes`, `result_bytes_view`, `result_result_bytes`, `iface`, `ptr_const_u8`, `ptr_mut_u8`, `ptr_const_void`, `ptr_mut_void`, `ptr_const_i32`, or `ptr_mut_i32`.\n");
    out.push_str("  - Function names must be namespaced and start with the current module ID.\n");
    out.push_str("    - In the entry file, use module `main` (example: `main.helper`).\n");
    out.push_str("  - `input` (bytes_view) is available in all function bodies.\n");
    out.push_str("- Call: `[\"name\", arg1, arg2, ...]`\n\n");

    out.push_str("## Concurrency\n\n");
    out.push_str("Async functions are defined with `defasync`.\n");
    out.push_str("Calling an async function returns an opaque task handle (`i32` in x07AST; type-checked by the compiler).\n");
    out.push_str(
        "To get concurrency, create multiple tasks before waiting on them (and avoid blocking operations outside tasks).\n\n",
    );
    out.push_str("- Define with a `decls[]` entry of kind `defasync`.\n");
    out.push_str("  - `body` is a single expression; wrap multi-step bodies in `begin`.\n");
    out.push_str("  - Awaited return types:\n");
    out.push_str("    - `bytes`\n");
    out.push_str("    - `result_bytes`\n\n");
    out.push_str("Task ops:\n\n");
    out.push_str("- Call: `[\"name\", arg1, arg2, ...]` -> `i32` task handle\n");
    out.push_str("- `[\"await\", <bytes task handle>]` -> `bytes` (alias of `task.join.bytes`)\n");
    out.push_str(
        "- `[\"task.spawn\", task_handle]` -> `i32` (stats/registration; optional for most code)\n",
    );
    out.push_str("- `[\"task.is_finished\", task_handle]` -> `i32` (0/1)\n");
    out.push_str("- `[\"task.try_join.bytes\", <bytes task handle>]` -> `result_bytes` (err=1 not finished; err=2 canceled)\n");
    out.push_str("- `[\"task.join.bytes\", <bytes task handle>]` -> `bytes`\n");
    out.push_str("- `[\"task.try_join.result_bytes\", <result_bytes task handle>]` -> `result_result_bytes` (err=1 not finished; err=2 canceled)\n");
    out.push_str(
        "- `[\"task.join.result_bytes\", <result_bytes task handle>]` -> `result_bytes`\n",
    );
    out.push_str("- `[\"task.yield\"]` -> `i32`\n");
    out.push_str("- `[\"task.sleep\", ticks_i32]` -> `i32` (virtual time ticks)\n");
    out.push_str("- `[\"task.cancel\", task_handle]` -> `i32`\n\n");
    out.push_str(
        "Note: `await` / `task.join.bytes` are only allowed in `solve` expressions and inside `defasync` bodies (not inside `defn`).\n\n",
    );
    out.push_str("Structured concurrency (`task.scope_v1`):\n\n");
    out.push_str("- `[\"task.scope_v1\", [\"task.scope.cfg_v1\", ...], <body>]` evaluates `<body>` and then joins+drops all children started in the scope.\n");
    out.push_str("- `[\"task.scope.start_soon_v1\", <immediate defasync call expr>] -> i32` registers a child task in the current scope.\n");
    out.push_str("- `[\"task.scope.cancel_all_v1\"] -> i32` cancels all registered children.\n");
    out.push_str(
        "- `[\"task.scope.wait_all_v1\"] -> i32` joins+drops all registered children so far.\n\n",
    );
    out.push_str("Scope cfg (`task.scope.cfg_v1`) fields (all optional):\n\n");
    out.push_str("- `[\"max_children\", <u32>]`\n");
    out.push_str("- `[\"max_ticks\", <u64>]`\n");
    out.push_str("- `[\"max_blocked_waits\", <u64>]`\n");
    out.push_str("- `[\"max_join_polls\", <u64>]`\n");
    out.push_str("- `[\"max_slot_result_bytes\", <u32>]`\n\n");
    out.push_str("Scoped slots (`async_let`):\n\n");
    out.push_str("- `[\"task.scope.async_let_bytes_v1\", <immediate defasync call expr>] -> i32` (slot id)\n");
    out.push_str("- `[\"task.scope.async_let_result_bytes_v1\", <immediate defasync call expr>] -> i32` (slot id)\n");
    out.push_str("- `[\"task.scope.await_slot_bytes_v1\", slot_id] -> bytes`\n");
    out.push_str("- `[\"task.scope.await_slot_result_bytes_v1\", slot_id] -> result_bytes`\n");
    out.push_str("- `[\"task.scope.try_await_slot.bytes_v1\", slot_id] -> result_bytes` (err=1 not ready; err=2 canceled)\n");
    out.push_str("- `[\"task.scope.try_await_slot.result_bytes_v1\", slot_id] -> result_result_bytes` (err=1 not ready; err=2 canceled)\n");
    out.push_str("- `[\"task.scope.slot_is_finished_v1\", slot_id] -> i32` (0/1)\n\n");
    out.push_str("Scoped select:\n\n");
    out.push_str("- `[\"task.scope.select_v1\", [\"task.scope.select.cfg_v1\", ...], [\"task.scope.select.cases_v1\", ...]] -> i32` (select evt id)\n");
    out.push_str("- `[\"task.scope.select_try_v1\", [\"task.scope.select.cfg_v1\", ...], [\"task.scope.select.cases_v1\", ...]] -> option_i32` (optional select evt id)\n\n");
    out.push_str("Select event helpers:\n\n");
    out.push_str("- `[\"task.select_evt.tag_v1\", evt_id] -> i32`\n");
    out.push_str("- `[\"task.select_evt.case_index_v1\", evt_id] -> i32`\n");
    out.push_str("- `[\"task.select_evt.src_id_v1\", evt_id] -> i32`\n");
    out.push_str("- `[\"task.select_evt.take_bytes_v1\", evt_id] -> bytes`\n");
    out.push_str("- `[\"task.select_evt.drop_v1\", evt_id] -> i32`\n\n");
    out.push_str("Channels (bytes payloads):\n\n");
    out.push_str("- `[\"chan.bytes.new\", cap_i32]` -> `i32`\n");
    out.push_str("- `[\"chan.bytes.try_send\", chan_handle, bytes_view]` -> `i32` (0 full; 1 sent; 2 closed)\n");
    out.push_str("- `[\"chan.bytes.send\", chan_handle, bytes]` -> `i32`\n");
    out.push_str("- `[\"chan.bytes.try_recv\", chan_handle]` -> `result_bytes` (err=1 empty; err=2 closed)\n");
    out.push_str("- `[\"chan.bytes.recv\", chan_handle]` -> `bytes`\n");
    out.push_str("- `[\"chan.bytes.close\", chan_handle]` -> `i32`\n\n");

    out.push_str("## Built-in Modules (stdlib)\n\n");
    out.push_str("Call module functions using fully-qualified names (e.g. `[\"std.bytes.reverse\",\"b\"]`).\n\n");
    out.push_str("- `std.bytes`\n");
    out.push_str("  - `[\"std.bytes.len\",\"b\"]` -> i32\n");
    out.push_str("  - `[\"std.bytes.get_u8\",\"b\",\"i\"]` -> i32 (0..255)\n");
    out.push_str("  - `[\"std.bytes.set_u8\",\"b\",\"i\",\"v\"]` -> bytes (returns `b`)\n");
    out.push_str("  - `[\"std.bytes.alloc\",\"n\"]` -> bytes (length `n`)\n");
    out.push_str("  - `[\"std.bytes.eq\",\"a\",\"b\"]` -> i32 (1 if equal else 0)\n");
    out.push_str("  - `[\"std.bytes.find_u8\",\"b\",\"target\"]` -> i32 (index, or -1)\n");
    out.push_str("  - `[\"std.bytes.cmp_range\",\"a\",\"a_off\",\"a_len\",\"b\",\"b_off\",\"b_len\"]` -> i32 (-1/0/1)\n");
    out.push_str(
        "  - `[\"std.bytes.max_u8\",\"v\"]` -> i32 (`v` is bytes_view; returns 0 if empty)\n",
    );
    out.push_str("  - `[\"std.bytes.sum_u8\",\"v\"]` -> i32 (`v` is bytes_view; wraps mod 2^32)\n");
    out.push_str("  - `[\"std.bytes.count_u8\",\"v\",\"target\"]` -> i32 (`v` is bytes_view)\n");
    out.push_str("  - `[\"std.bytes.starts_with\",\"a\",\"prefix\"]` -> i32 (both bytes_view)\n");
    out.push_str("  - `[\"std.bytes.ends_with\",\"a\",\"suffix\"]` -> i32 (both bytes_view)\n");
    out.push_str("  - `[\"std.bytes.reverse\",\"b\"]` -> bytes\n");
    out.push_str("  - `[\"std.bytes.concat\",\"a\",\"b\"]` -> bytes\n");
    out.push_str("  - `[\"std.bytes.take\",\"b\",\"n\"]` -> bytes\n");
    out.push_str("  - `[\"std.bytes.drop\",\"b\",\"n\"]` -> bytes\n");
    out.push_str("  - `[\"std.bytes.copy\",\"b\"]` -> bytes\n");
    out.push_str("  - `[\"std.bytes.slice\",\"b\",\"start\",\"len\"]` -> bytes (copy; clamps within bounds)\n");
    out.push_str("- `std.codec`\n");
    out.push_str("  - `[\"std.codec.read_u32_le\",\"b\",\"off\"]` -> i32 (`b` is bytes_view)\n");
    out.push_str("  - `[\"std.codec.write_u32_le\",\"x\"]` -> bytes\n");
    out.push_str("- `std.vec`\n");
    out.push_str("  - `[\"std.vec.with_capacity\",\"cap\"]` -> vec_u8\n");
    out.push_str("  - `[\"std.vec.len\",\"v\"]` -> i32\n");
    out.push_str("  - `[\"std.vec.get\",\"v\",\"i\"]` -> i32 (0..255)\n");
    out.push_str("  - `[\"std.vec.push\",\"v\",\"x\"]` -> vec_u8\n");
    out.push_str("  - `[\"std.vec.reserve_exact\",\"v\",\"additional\"]` -> vec_u8\n");
    out.push_str("  - `[\"std.vec.extend_bytes\",\"v\",\"b\"]` -> vec_u8\n");
    out.push_str("  - `[\"std.vec.as_bytes\",\"v\"]` -> bytes\n");
    out.push_str("- `std.vec_value` (generic; `bytes` convenience wrappers)\n");
    out.push_str("  - `[\"std.vec_value.with_capacity_bytes\",\"cap\"]` -> i32\n");
    out.push_str("  - `[\"std.vec_value.len\",\"v\"]` -> i32\n");
    out.push_str("  - `[\"std.vec_value.reserve_exact\",\"v\",\"additional\"]` -> i32\n");
    out.push_str("  - `[\"std.vec_value.push_bytes\",\"v\",\"x\"]` -> i32\n");
    out.push_str("  - `[\"std.vec_value.get_bytes_or\",\"v\",\"idx\",\"default\"]` -> bytes\n");
    out.push_str("  - `[\"std.vec_value.set_bytes\",\"v\",\"idx\",\"x\"]` -> i32\n");
    out.push_str("  - `[\"std.vec_value.pop\",\"v\"]` -> i32\n");
    out.push_str("  - `[\"std.vec_value.clear\",\"v\"]` -> i32\n");
    out.push_str("- `std.slice`\n");
    out.push_str("  - `[\"std.slice.clamp\",\"b\",\"start\",\"len\"]` -> bytes\n");
    out.push_str("  - `[\"std.slice.cmp_bytes\",\"a\",\"b\"]` -> i32 (-1/0/1)\n");
    out.push_str("- `std.parse`\n");
    out.push_str("  - `[\"std.parse.u32_dec\",\"b\"]` -> i32\n");
    out.push_str("  - `[\"std.parse.u32_dec_at\",\"b\",\"off\"]` -> i32\n");
    out.push_str(
        "  - `[\"std.parse.i32_status_le\",\"b\"]` -> bytes (tag byte 1 + i32_le, or tag byte 0)\n",
    );
    out.push_str(
        "  - `[\"std.parse.i32_status_le_at\",\"b\",\"off\"]` -> bytes (tag byte 1 + i32_le + next_off_u32_le, or tag byte 0)\n",
    );
    out.push_str("- `std.fmt`\n");
    out.push_str("  - `[\"std.fmt.u32_to_dec\",\"x\"]` -> bytes\n");
    out.push_str("  - `[\"std.fmt.s32_to_dec\",\"x\"]` -> bytes\n");
    out.push_str("- `std.prng`\n");
    out.push_str("  - `[\"std.prng.lcg_next_u32\",\"state\"]` -> i32\n");
    out.push_str("  - `[\"std.prng.x07rand32_v1_stream\",\"b\"]` -> bytes\n");
    out.push_str("- `std.bit`\n");
    out.push_str("  - `[\"std.bit.popcount_u32\",\"x\"]` -> i32\n");
    out.push_str("- `std.text.ascii`\n");
    out.push_str("  - `[\"std.text.ascii.normalize_lines\",\"b\"]` -> bytes\n");
    out.push_str("  - `[\"std.text.ascii.tokenize_words_lower\",\"b\"]` -> bytes\n");
    out.push_str(
        "  - `[\"std.text.ascii.split_u8\",\"b\",\"sep_u8\"]` -> bytes (X7SL v1 slice list)\n",
    );
    out.push_str("  - `[\"std.text.ascii.split_lines_view\",\"b\"]` -> bytes (X7SL v1 slice list; splits on `\\n`, strips trailing `\\r`, omits trailing empty line after final `\\n`)\n");
    out.push_str("- `std.text.slices`\n");
    out.push_str("  - `[\"std.text.slices.builder_new_v1\",\"cap_hint\"]` -> vec_u8\n");
    out.push_str(
        "  - `[\"std.text.slices.builder_push_v1\",\"out\",\"start\",\"len\"]` -> vec_u8\n",
    );
    out.push_str(
        "  - `[\"std.text.slices.builder_finish_v1\",\"out\",\"count\"]` -> bytes (X7SL v1)\n",
    );
    out.push_str(
        "  - `[\"std.text.slices.validate_v1\",\"x7sl\"]` -> result_i32 (OK(count) or ERR(code); see `docs/text/x7sl-v1.md`)\n",
    );
    out.push_str("  - `[\"std.text.slices.count_v1\",\"x7sl\"]` -> i32 (count or -1)\n");
    out.push_str("  - `[\"std.text.slices.start_v1\",\"x7sl\",\"idx\"]` -> i32\n");
    out.push_str("  - `[\"std.text.slices.len_v1\",\"x7sl\",\"idx\"]` -> i32\n");
    out.push_str(
        "  - `[\"std.text.slices.view_at_v1\",\"base_view\",\"x7sl\",\"idx\"]` -> bytes_view\n",
    );
    out.push_str(
        "  - `[\"std.text.slices.copy_at_v1\",\"base_view\",\"x7sl\",\"idx\"]` -> bytes\n",
    );
    out.push_str("- `std.text.utf8`\n");
    out.push_str("  - `[\"std.text.utf8.validate_or_empty\",\"b\"]` -> bytes\n");
    out.push_str("- `std.regex-lite`\n");
    out.push_str(
        "  - `[\"std.regex-lite.find_literal\",\"hay\",\"needle\"]` -> i32 (index, or -1)\n",
    );
    out.push_str("  - `[\"std.regex-lite.is_match_literal\",\"hay\",\"needle\"]` -> i32 (0/1)\n");
    out.push_str("  - `[\"std.regex-lite.count_matches_u32le\",\"b\"]` -> bytes\n");
    out.push_str("- `std.json`\n");
    out.push_str("  - `[\"std.json.canonicalize_small\",\"json_bytes\"]` -> bytes (or `ERR`)\n");
    out.push_str("  - `[\"std.json.extract_path_canon_or_err\",\"b\"]` -> bytes\n");
    out.push_str("- `std.csv`\n");
    out.push_str(
        "  - `[\"std.csv.sum_second_col_i32_status_le\",\"csv_bytes\"]` -> bytes (tag byte 1 + i32_le, or tag byte 0)\n",
    );
    out.push_str(
        "  - `[\"std.csv.sum_second_col_i32le_or_err\",\"csv_bytes\"]` -> bytes (i32_le, or `ERR`)\n",
    );
    out.push_str("- `std.map`\n");
    out.push_str("  - `[\"std.map.word_freq_sorted_ascii\",\"b\"]` -> bytes\n");
    out.push_str("- `std.set`\n");
    out.push_str("  - `[\"std.set.unique_lines_sorted\",\"b\"]` -> bytes\n");
    out.push_str("- `std.u32`\n");
    out.push_str("  - `[\"std.u32.read_le_at\",\"b\",\"off\"]` -> i32\n");
    out.push_str("  - `[\"std.u32.write_le_at\",\"b\",\"off\",\"x\"]` -> bytes\n");
    out.push_str("  - `[\"std.u32.push_le\",\"v\",\"x\"]` -> vec_u8\n");
    out.push_str("  - `[\"std.u32.pow2_ceil\",\"x\"]` -> i32\n");
    out.push_str("- `std.small_map`\n");
    out.push_str("  - `[\"std.small_map.empty_bytes_u32\"]` -> bytes\n");
    out.push_str("  - `[\"std.small_map.len_bytes_u32\",\"m\"]` -> i32\n");
    out.push_str("  - `[\"std.small_map.get_bytes_u32\",\"m\",\"key\"]` -> i32 (0 if missing)\n");
    out.push_str("  - `[\"std.small_map.put_bytes_u32\",\"m\",\"key\",\"val\"]` -> bytes\n");
    out.push_str("  - `[\"std.small_map.remove_bytes_u32\",\"m\",\"key\"]` -> bytes\n");
    out.push_str("- `std.small_set`\n");
    out.push_str("  - `[\"std.small_set.empty_bytes\"]` -> bytes\n");
    out.push_str("  - `[\"std.small_set.len_bytes\",\"s\"]` -> i32\n");
    out.push_str("  - `[\"std.small_set.contains_bytes\",\"s\",\"key\"]` -> i32\n");
    out.push_str("  - `[\"std.small_set.insert_bytes\",\"s\",\"key\"]` -> bytes\n");
    out.push_str("- `std.hash`\n");
    out.push_str("  - `[\"std.hash.fnv1a32_bytes\",\"b\"]` -> i32\n");
    out.push_str("  - `[\"std.hash.fnv1a32_range\",\"b\",\"start\",\"len\"]` -> i32\n");
    out.push_str("  - `[\"std.hash.fnv1a32_view\",\"v\"]` -> i32\n");
    out.push_str("  - `[\"std.hash.mix32\",\"x\"]` -> i32\n");
    out.push_str("- `std.hash_map` (u32 keys/values)\n");
    out.push_str("  - `[\"std.hash_map.with_capacity_u32\",\"expected_len\"]` -> i32\n");
    out.push_str("  - `[\"std.hash_map.len_u32\",\"m\"]` -> i32\n");
    out.push_str("  - `[\"std.hash_map.contains_u32\",\"m\",\"key\"]` -> i32\n");
    out.push_str("  - `[\"std.hash_map.get_u32_or\",\"m\",\"key\",\"default\"]` -> i32\n");
    out.push_str("  - `[\"std.hash_map.set_u32\",\"m\",\"key\",\"val\"]` -> i32\n");
    out.push_str("- `std.hash_map_value` (generic; `bytes`/`bytes` convenience wrappers)\n");
    out.push_str("  - `[\"std.hash_map_value.new_bytes_bytes\",\"cap_pow2\"]` -> i32\n");
    out.push_str(
        "  - `[\"std.hash_map_value.with_capacity_bytes_bytes\",\"expected_len\"]` -> i32\n",
    );
    out.push_str("  - `[\"std.hash_map_value.len\",\"m\"]` -> i32\n");
    out.push_str("  - `[\"std.hash_map_value.contains_bytes\",\"m\",\"key\"]` -> i32\n");
    out.push_str(
        "  - `[\"std.hash_map_value.get_bytes_bytes_or\",\"m\",\"key\",\"default\"]` -> bytes\n",
    );
    out.push_str("  - `[\"std.hash_map_value.set_bytes_bytes\",\"m\",\"key\",\"val\"]` -> i32\n");
    out.push_str("  - `[\"std.hash_map_value.remove_bytes\",\"m\",\"key\"]` -> i32\n");
    out.push_str("  - `[\"std.hash_map_value.clear\",\"m\"]` -> i32\n");
    out.push_str("- `std.hash_set`\n");
    out.push_str("  - u32 set: `[\"std.hash_set.new_u32\",\"cap_pow2\"]` -> i32, `[\"std.hash_set.add_u32\",\"s\",\"key\"]` -> i32\n");
    out.push_str("  - view-key set: `[\"std.hash_set.view_new\",\"expected_len\"]` -> vec_u8, `[\"std.hash_set.view_insert\",\"set\",\"base\",\"start\",\"len\"]` -> vec_u8\n");
    out.push_str("- `std.btree_map` (ordered u32->u32)\n");
    out.push_str("  - `[\"std.btree_map.empty_u32_u32\"]` -> bytes\n");
    out.push_str("  - `[\"std.btree_map.len_u32_u32\",\"m\"]` -> i32\n");
    out.push_str("  - `[\"std.btree_map.get_u32_u32_or\",\"m\",\"key\",\"default\"]` -> i32\n");
    out.push_str("  - `[\"std.btree_map.put_u32_u32\",\"m\",\"key\",\"val\"]` -> bytes\n");
    out.push_str("- `std.btree_set` (ordered u32)\n");
    out.push_str("  - `[\"std.btree_set.empty_u32\"]` -> bytes\n");
    out.push_str("  - `[\"std.btree_set.len_u32\",\"s\"]` -> i32\n");
    out.push_str("  - `[\"std.btree_set.contains_u32\",\"s\",\"key\"]` -> i32\n");
    out.push_str("  - `[\"std.btree_set.insert_u32\",\"s\",\"key\"]` -> bytes\n");
    out.push_str("- `std.deque_u32`\n");
    out.push_str("  - `[\"std.deque_u32.with_capacity\",\"cap\"]` -> bytes\n");
    out.push_str("  - `[\"std.deque_u32.len\",\"dq\"]` -> i32\n");
    out.push_str("  - `[\"std.deque_u32.push_back\",\"dq\",\"x\"]` -> bytes\n");
    out.push_str("  - `[\"std.deque_u32.front_or\",\"dq\",\"default\"]` -> i32\n");
    out.push_str("  - `[\"std.deque_u32.pop_front\",\"dq\"]` -> bytes\n");
    out.push_str("  - `[\"std.deque_u32.emit_u32le\",\"dq\"]` -> bytes\n");
    out.push_str("- `std.heap_u32`\n");
    out.push_str("  - `[\"std.heap_u32.with_capacity\",\"cap\"]` -> bytes\n");
    out.push_str("  - `[\"std.heap_u32.len\",\"h\"]` -> i32\n");
    out.push_str("  - `[\"std.heap_u32.push\",\"h\",\"x\"]` -> bytes\n");
    out.push_str("  - `[\"std.heap_u32.min_or\",\"h\",\"default\"]` -> i32\n");
    out.push_str("  - `[\"std.heap_u32.pop_min\",\"h\"]` -> bytes\n");
    out.push_str("  - `[\"std.heap_u32.emit_u32le\",\"h\"]` -> bytes\n");
    out.push_str("- `std.bitset`\n");
    out.push_str("  - `[\"std.bitset.new\",\"bit_len\"]` -> bytes\n");
    out.push_str("  - `[\"std.bitset.set\",\"bs\",\"bit\"]` -> bytes\n");
    out.push_str("  - `[\"std.bitset.test\",\"bs\",\"bit\"]` -> i32\n");
    out.push_str("  - `[\"std.bitset.intersection_count\",\"a\",\"b\"]` -> i32\n");
    out.push_str("- `std.slab` (u32 values)\n");
    out.push_str("  - `[\"std.slab.new_u32\",\"cap\"]` -> bytes\n");
    out.push_str("  - `[\"std.slab.free_head_u32\",\"slab\"]` -> i32 (-1 if none)\n");
    out.push_str("  - `[\"std.slab.alloc_u32\",\"slab\"]` -> bytes\n");
    out.push_str("  - `[\"std.slab.free_u32\",\"slab\",\"handle\"]` -> bytes\n");
    out.push_str("  - `[\"std.slab.get_u32\",\"slab\",\"handle\",\"default\"]` -> i32\n");
    out.push_str("  - `[\"std.slab.set_u32\",\"slab\",\"handle\",\"val\"]` -> bytes\n");
    out.push_str("- `std.lru_cache` (u32 keys/values)\n");
    out.push_str("  - `[\"std.lru_cache.new_u32\",\"cap\"]` -> bytes\n");
    out.push_str("  - `[\"std.lru_cache.len_u32\",\"cache\"]` -> i32\n");
    out.push_str("  - `[\"std.lru_cache.peek_u32_opt\",\"cache\",\"key\"]` -> option_i32\n");
    out.push_str("  - `[\"std.lru_cache.peek_u32_or\",\"cache\",\"key\",\"default\"]` -> i32\n");
    out.push_str("  - `[\"std.lru_cache.touch_u32\",\"cache\",\"key\"]` -> bytes\n");
    out.push_str("  - `[\"std.lru_cache.put_u32\",\"cache\",\"key\",\"val\"]` -> bytes\n");
    out.push_str("- `std.test`\n");
    out.push_str("  - `[\"std.test.pass\"]` -> result_i32\n");
    out.push_str("  - `[\"std.test.fail\",\"code\"]` -> result_i32\n");
    out.push_str("  - `[\"std.test.assert_true\",\"cond\",\"code\"]` -> result_i32\n");
    out.push_str("  - `[\"std.test.assert_i32_eq\",\"a\",\"b\",\"code\"]` -> result_i32\n");
    out.push_str("  - `[\"std.test.assert_bytes_eq\",\"a\",\"b\",\"code\"]` -> result_i32\n");
    out.push_str("  - `[\"std.test.code_assert_true\"]` -> i32\n");
    out.push_str("  - `[\"std.test.code_assert_i32_eq\"]` -> i32\n");
    out.push_str("  - `[\"std.test.code_assert_bytes_eq\"]` -> i32\n");
    out.push_str(
        "  - `[\"std.test.status_from_result_i32\",\"r\"]` -> bytes (5-byte X7TEST_STATUS_V1)\n",
    );
    out.push_str("- `std.result`\n");
    out.push_str("  - `[\"std.result.ok_i32_le\",\"x\"]` -> bytes\n");
    out.push_str("  - `[\"std.result.err0\"]` -> bytes\n");
    out.push_str("  - `[\"std.result.chain_sum_csv_i32\",\"b\"]` -> bytes\n");
    out.push_str("- `std.option`\n");
    out.push_str("  - `[\"std.option.some_i32_le\",\"x\"]` -> bytes\n");
    out.push_str("  - `[\"std.option.none\"]` -> bytes\n");
    out.push_str("- `std.io`\n");
    out.push_str("  - `[\"std.io.read\",\"reader\",\"max\"]` -> bytes (`reader` is `iface`)\n");
    out.push_str("- `std.io.bufread`\n");
    out.push_str(
        "  - `[\"std.io.bufread.new\",\"reader\",\"cap\"]` -> i32 (`reader` is `iface`)\n",
    );
    out.push_str("  - `[\"std.io.bufread.fill\",\"br\"]` -> bytes_view\n");
    out.push_str("  - `[\"std.io.bufread.consume\",\"br\",\"n\"]` -> i32\n");
    out.push_str("- `std.fs` (world-bound via `std.world.fs`)\n");
    out.push_str("  - `[\"std.fs.read\",\"path_bytes\"]` -> bytes\n");
    out.push_str("  - `[\"std.fs.read_async\",\"path_bytes\"]` -> bytes\n");
    out.push_str("  - `[\"std.fs.open_read\",\"path_bytes\"]` -> iface\n");
    out.push_str("  - `[\"std.fs.list_dir\",\"path_bytes\"]` -> bytes\n");
    out.push_str("  - `[\"std.fs.list_dir_sorted\",\"path_bytes\"]` -> bytes\n\n");
    out.push_str("- `std.kv` (world-bound)\n");
    out.push_str("  - `[\"std.kv.get\",\"key\"]` -> bytes (`key` is bytes_view)\n");
    out.push_str("  - `[\"std.kv.get_async\",\"key\"]` -> bytes (`key` is bytes_view)\n");
    out.push_str("  - `[\"std.kv.get_stream\",\"key\"]` -> iface (`key` is bytes_view)\n");
    out.push_str("  - `[\"std.kv.get_task\",\"key\"]` -> i32 task handle (`key` is bytes; await returns bytes)\n");
    out.push_str("  - `[\"std.kv.set\",\"key\",\"val\"]` -> i32 (`key`/`val` are bytes)\n\n");
    out.push_str("- `std.rr` (solve-rr)\n");
    out.push_str("  - `[\"std.rr.open_v1\",\"cfg\"]` -> result_i32 (`cfg` is bytes_view)\n");
    out.push_str("  - `[\"std.rr.close_v1\",\"h\"]` -> i32\n");
    out.push_str("  - `[\"std.rr.stats_v1\",\"h\"]` -> bytes\n");
    out.push_str("  - `[\"std.rr.next_v1\",\"h\",\"kind\",\"op\",\"key\"]` -> result_bytes (all bytes_view)\n");
    out.push_str(
        "  - `[\"std.rr.append_v1\",\"h\",\"entry\"]` -> result_i32 (`entry` is bytes_view)\n",
    );
    out.push_str("  - `[\"std.rr.entry_resp_v1\",\"entry\"]` -> bytes (`entry` is bytes_view)\n");
    out.push_str("  - `[\"std.rr.entry_err_v1\",\"entry\"]` -> i32 (`entry` is bytes_view)\n");
    out.push_str("  - `[\"std.rr.current_v1\"]` -> i32\n\n");
    out.push_str("- `std.world.fs` (adapter module; world-selected)\n");
    out.push_str("  - `[\"std.world.fs.read_file\",\"path_bytes\"]` -> bytes\n");
    out.push_str("  - `[\"std.world.fs.read_file_async\",\"path_bytes\"]` -> bytes\n\n");
    out.push_str("  - `[\"std.world.fs.write_file\",\"path_bytes\",\"data_bytes\"]` -> i32\n\n");
    out.push_str("- `std.os.env` (OS worlds)\n");
    out.push_str("  - `[\"std.os.env.get\",\"key_bytes\"]` -> bytes\n\n");
    out.push_str("- `std.os.fs` (OS worlds)\n");
    out.push_str("  - `[\"std.os.fs.read_file\",\"path_bytes\"]` -> bytes\n");
    out.push_str("  - `[\"std.os.fs.write_file\",\"path_bytes\",\"data_bytes\"]` -> i32\n\n");
    out.push_str("- `std.os.net` (OS worlds)\n");
    out.push_str("  - `[\"std.os.net.http_request\",\"req_bytes\"]` -> bytes\n\n");
    out.push_str("- `std.os.time` (OS worlds)\n");
    out.push_str("  - `[\"std.os.time.now_unix_ms\"]` -> i32\n\n");
    out.push_str("- `std.path`\n");
    out.push_str("  - `[\"std.path.join\",\"a\",\"b\"]` -> bytes\n");
    out.push_str("  - `[\"std.path.basename\",\"p\"]` -> bytes\n");
    out.push_str("  - `[\"std.path.extname\",\"p\"]` -> bytes\n\n");

    out.push_str("## Operators (i32)\n\n");
    out.push_str("- `[\"+\",\"a\",\"b\"]` `[\"-\",\"a\",\"b\"]` `[\"*\",\"a\",\"b\"]` `[\"/\",\"a\",\"b\"]` `[\"%\",\"a\",\"b\"]`\n");
    out.push_str("- `[\"=\",\"a\",\"b\"]` `[\"!=\",\"a\",\"b\"]`\n");
    out.push_str("- Signed comparisons (two’s complement): `[\"<\",\"a\",\"b\"]` `[\"<=\",\"a\",\"b\"]` `[\">\",\"a\",\"b\"]` `[\">=\",\"a\",\"b\"]`\n");
    out.push_str("- Unsigned comparisons: `[\"<u\",\"a\",\"b\"]` `[\">=u\",\"a\",\"b\"]`\n");
    out.push_str(
        "- Shifts: `[\"<<u\",\"a\",\"b\"]` `[\">>u\",\"a\",\"b\"]` (shift amount masked by 31)\n",
    );
    out.push_str("- Bitwise: `[\"&\",\"a\",\"b\"]` `[\"|\",\"a\",\"b\"]` `[\"^\",\"a\",\"b\"]`\n");
    out.push('\n');

    out.push_str("## Integer Semantics\n\n");
    out.push_str("- Integers are 32-bit and arithmetic wraps modulo 2^32.\n");
    out.push_str("- Beware underflow/overflow: `[\"-\",0,1]` becomes `-1` (wrap-around).\n");
    out.push_str("- `/` and `%` are unsigned u32 ops: `/` by 0 yields 0, and `%` by 0 yields the numerator.\n");
    out.push_str("- `[\"<u\",\"x\",0]` is always false and `[\">=u\",\"x\",0]` is always true.\n");
    out.push_str("- For negative checks, use signed comparisons like `[\"<\",\"x\",0]`.\n");
    out.push_str("- For \"can’t go below zero\" counters, guard before decrementing.\n\n");

    out.push_str("## Bytes Ops\n\n");
    out.push_str("Use `std.bytes.*` functions (import `std.bytes`):\n\n");
    out.push_str("- `[\"std.bytes.len\",\"b\"]` -> i32\n");
    out.push_str("- `[\"std.bytes.get_u8\",\"b\",\"i\"]` -> i32 (0..255)\n");
    out.push_str("- `[\"std.bytes.set_u8\",\"b\",\"i\",\"v\"]` -> bytes (returns `b`)\n");
    out.push_str("- `[\"std.bytes.alloc\",\"n\"]` -> bytes (length `n`)\n\n");

    out.push_str("Additional bytes ops:\n\n");
    out.push_str("- `[\"std.bytes.eq\",\"a\",\"b\"]` -> i32 (1 if equal else 0)\n");
    out.push_str("- `[\"std.bytes.find_u8\",\"b\",\"target\"]` -> i32 (index, or -1)\n");
    out.push_str("- `[\"std.bytes.cmp_range\",\"a\",\"a_off\",\"a_len\",\"b\",\"b_off\",\"b_len\"]` -> i32 (-1/0/1)\n\n");
    out.push_str(
        "- `[\"std.bytes.max_u8\",\"v\"]` -> i32 (`v` is bytes_view; returns 0 if empty)\n",
    );
    out.push_str("- `[\"std.bytes.sum_u8\",\"v\"]` -> i32 (`v` is bytes_view; wraps mod 2^32)\n");
    out.push_str("- `[\"std.bytes.count_u8\",\"v\",\"target\"]` -> i32 (`v` is bytes_view)\n");
    out.push_str("- `[\"std.bytes.starts_with\",\"a\",\"prefix\"]` -> i32 (both bytes_view)\n");
    out.push_str("- `[\"std.bytes.ends_with\",\"a\",\"suffix\"]` -> i32 (both bytes_view)\n\n");
    out.push_str(
        "For copy/slice/concat/reverse/take/drop helpers, use `std.bytes.*` module functions (see Modules).\n\n",
    );

    out.push_str("Bytes literals:\n\n");
    out.push_str(
        "- `[\"bytes.lit\",\"text\"]` -> bytes (UTF-8 of the JSON string; whitespace allowed)\n",
    );
    out.push_str("  - Example: `[\"bytes.lit\",\"config.bin\"]` produces `b\"config.bin\"`.\n");
    out.push_str("  - JSON escapes apply (e.g. `\\n`, `\\t`, `\\uXXXX`).\n");
    out.push_str(
        "  - For arbitrary (non-UTF-8) bytes, build a `vec_u8` and convert with `std.vec.as_bytes`.\n\n",
    );

    out.push_str("## Views\n\n");
    out.push_str(
        "Views are explicit, borrowed slices used for scan/trim/split without copying.\n\n",
    );
    out.push_str("- `[\"bytes.view\",\"b\"]` -> bytes_view\n");
    out.push_str("- `[\"bytes.subview\",\"b\",\"start\",\"len\"]` -> bytes_view\n");
    out.push_str("- `[\"view.len\",\"v\"]` -> i32\n");
    out.push_str("- `[\"view.get_u8\",\"v\",\"i\"]` -> i32\n");
    out.push_str("- `[\"view.slice\",\"v\",\"start\",\"len\"]` -> bytes_view\n");
    out.push_str("- `[\"view.to_bytes\",\"v\"]` -> bytes (copy)\n");
    out.push_str("- `[\"view.eq\",\"a\",\"b\"]` -> i32 (1 if equal else 0)\n");
    out.push_str("- `[\"view.cmp_range\",\"a\",\"a_off\",\"a_len\",\"b\",\"b_off\",\"b_len\"]` -> i32 (-1/0/1)\n\n");

    out.push_str("Note: `bytes.view`, `bytes.subview`, and `vec_u8.as_view` require an identifier owner (they cannot borrow from a temporary expression).\n\n");

    out.push_str("## OS Worlds (run-os / run-os-sandboxed)\n\n");
    out.push_str("OS effects are accessed through `std.os.*` modules, which call `os.*` builtins (listed above).\n");
    out.push_str("In sandboxed execution, these calls are gated by policy.\n\n");

    out.push_str("## Record/replay (rr)\n\n");
    out.push_str("In rr-enabled worlds, X07 can replay (and optionally record) external interactions from a cassette file.\n\n");
    out.push_str("Structured scope forms:\n\n");
    out.push_str(
        "- `[\"std.rr.with_v1\", cfg_bytes_view_expr, body_expr]` -> type of `body_expr`\n",
    );
    out.push_str("- `[\"std.rr.with_policy_v1\", [\"bytes.lit\",\"POLICY_ID\"], [\"bytes.lit\",\"CASSETTE_PATH\"], [\"i32.lit\",mode], body_expr]` -> type of `body_expr`\n");
    out.push_str("  - mode: 0=off, 1=record, 2=replay, 3=record_missing, 4=rewrite\n\n");
    out.push_str(
        "Low-level APIs live in the built-in `std.rr` module (see built-in modules below).\n\n",
    );

    out.push_str("## Streaming I/O\n\n");
    out.push_str("Readers are `iface` values returned by world adapters.\n\n");
    out.push_str("- `[\"io.open_read_bytes\",\"b\"]` -> iface (`b` is bytes/bytes_view/vec_u8)\n");
    out.push_str("- `[\"io.read\",\"reader_iface\",\"max_i32\"]` -> bytes\n");
    out.push_str("- `[\"bufread.new\",\"reader_iface\",\"cap_i32\"]` -> i32\n");
    out.push_str("- `[\"bufread.fill\",\"br\"]` -> bytes_view\n");
    out.push_str("- `[\"bufread.consume\",\"br\",\"n_i32\"]` -> i32\n\n");
    out.push_str("For deterministic, budgeted streaming composition, prefer `std.stream.pipe_v1` (see end-user docs).\n\n");
    out.push_str("Pipe shape:\n\n");
    out.push_str(
        "- `[\"std.stream.pipe_v1\", cfg_v1, src_v1, chain_v1, sink_v1]` -> bytes (a stream doc)\n",
    );
    out.push_str("- `cfg_v1`: `[\"std.stream.cfg_v1\", ...]`\n");
    out.push_str("- `src_v1`: `std.stream.src.*_v1` descriptor\n");
    out.push_str("- `chain_v1`: `[\"std.stream.chain_v1\", ...]`\n");
    out.push_str("- `sink_v1`: `std.stream.sink.*_v1` descriptor\n\n");

    out.push_str("## Vectors\n\n");
    out.push_str(
        "`vec_u8` is a mutable byte vector value used for building outputs efficiently.\n\n",
    );
    out.push_str("Use `std.vec.*` to create and manipulate `vec_u8` values:\n\n");
    out.push_str("- `[\"std.vec.with_capacity\",\"cap\"]` -> vec_u8\n");
    out.push_str("- `[\"std.vec.push\",\"v\",\"x\"]` -> vec_u8\n");
    out.push_str("- `[\"std.vec.reserve_exact\",\"v\",\"additional\"]` -> vec_u8\n");
    out.push_str("- `[\"std.vec.extend_bytes\",\"v\",\"b\"]` -> vec_u8\n");
    out.push_str("- `[\"std.vec.as_bytes\",\"v\"]` -> bytes\n\n");

    out.push_str("Additional vec_u8 builtins:\n\n");
    out.push_str("- `[\"vec_u8.extend_bytes_range\",\"v\",\"b\",\"start\",\"len\"]` -> vec_u8 (append subrange of `b`)\n");
    out.push_str(
        "- `[\"vec_u8.as_view\",\"v\"]` -> bytes_view (borrowed view of current vec contents)\n\n",
    );

    out.push_str("## Option / Result\n\n");

    out.push_str("Typed options:\n\n");
    out.push_str("- `[\"option_i32.none\"]` -> option_i32\n");
    out.push_str("- `[\"option_i32.some\",\"x\"]` -> option_i32\n");
    out.push_str("- `[\"option_i32.is_some\",\"o\"]` -> i32\n");
    out.push_str("- `[\"option_i32.unwrap_or\",\"o\",\"default\"]` -> i32\n\n");
    out.push_str("- `[\"option_bytes.none\"]` -> option_bytes\n");
    out.push_str("- `[\"option_bytes.some\",\"b\"]` -> option_bytes\n");
    out.push_str("- `[\"option_bytes.is_some\",\"o\"]` -> i32\n");
    out.push_str("- `[\"option_bytes.unwrap_or\",\"o\",\"default\"]` -> bytes\n\n");

    out.push_str("Typed results:\n\n");
    out.push_str("- `[\"result_i32.ok\",\"x\"]` -> result_i32\n");
    out.push_str("- `[\"result_i32.err\",\"code\"]` -> result_i32\n");
    out.push_str("- `[\"result_i32.is_ok\",\"r\"]` -> i32\n");
    out.push_str("- `[\"result_i32.err_code\",\"r\"]` -> i32\n");
    out.push_str("- `[\"result_i32.unwrap_or\",\"r\",\"default\"]` -> i32\n\n");
    out.push_str("- `[\"result_bytes.ok\",\"b\"]` -> result_bytes\n");
    out.push_str("- `[\"result_bytes.err\",\"code\"]` -> result_bytes\n");
    out.push_str("- `[\"result_bytes.is_ok\",\"r\"]` -> i32\n");
    out.push_str("- `[\"result_bytes.err_code\",\"r\"]` -> i32\n");
    out.push_str("- `[\"result_bytes.unwrap_or\",\"r\",\"default\"]` -> bytes\n\n");

    out.push_str("Propagation sugar:\n\n");
    out.push_str("- `[\"try\",\"r\"]` -> `i32` or `bytes` (requires the current `defn` return type is `result_i32` or `result_bytes`)\n\n");

    out.push_str("## Budget scopes\n\n");
    out.push_str("Budget scopes are special forms that enforce local resource limits (alloc/memcpy/scheduler ticks/fuel).\n\n");
    out.push_str("- `[\"budget.scope_v1\", [\"budget.cfg_v1\", ...], body_expr]` -> type of `body_expr` (or a `result_*` in `mode=result_err_v1`)\n");
    out.push_str("- `[\"budget.scope_from_arch_v1\", [\"bytes.lit\",\"PROFILE_ID\"], body_expr]` -> loads a pinned cfg from `arch/budgets/profiles/PROFILE_ID.budget.json`\n\n");
    out.push_str("`budget.cfg_v1` fields:\n\n");
    out.push_str("- `mode`: `trap_v1` | `result_err_v1` | `stats_only_v1` | `yield_v1`\n");
    out.push_str("- `label`: bytes literal (for diagnostics)\n");
    out.push_str("- Optional caps: `alloc_bytes`, `alloc_calls`, `realloc_calls`, `memcpy_bytes`, `sched_ticks`, `fuel`\n\n");

    out.push_str("## Memory / Performance Tips\n\n");
    out.push_str("- Deterministic suite gates may enforce `mem_stats`: reduce `realloc_calls`, `memcpy_bytes`, and `peak_live_bytes`.\n");
    out.push_str(
        "- Prefer building output with `std.vec` instead of repeated concatenation in a loop.\n",
    );
    out.push_str("- If you can estimate the final size, start with `[\"std.vec.with_capacity\",\"n\"]` to avoid reallocations.\n");
    out.push_str("- When appending many bytes, prefer `[\"std.vec.extend_bytes\",\"v\",\"chunk\"]` over per-byte `[\"std.vec.push\",...]`.\n");
    out.push_str("- Convert once at the end: `[\"std.vec.as_bytes\",\"v\"]` returns bytes without copying.\n");
    out.push_str(
        "- Use `bytes_view` + `view.*` builtins for scanning/slicing without copying.\n\n",
    );
    out.push_str(
        "- For streaming parsing, use `[\"bufread.fill\",\"br\"]` to get a `bytes_view` window, scan it, then `[\"bufread.consume\",\"br\",\"n\"]`.\n\n",
    );

    out.push_str("## Maps / Sets\n\n");
    out.push_str("Open-addressing hash tables with fixed capacity.\n");
    out.push_str("- capacity must be a non-zero power of two\n");
    out.push_str("- key `-1` is reserved and will trap\n\n");
    out.push_str("- `[\"map_u32.new\",\"cap_pow2\"]` -> i32\n");
    out.push_str("- `[\"map_u32.len\",\"m\"]` -> i32\n");
    out.push_str("- `[\"map_u32.contains\",\"m\",\"key\"]` -> i32 (0/1)\n");
    out.push_str("- `[\"map_u32.get\",\"m\",\"key\",\"default\"]` -> i32\n");
    out.push_str(
        "- `[\"map_u32.set\",\"m\",\"key\",\"value\"]` -> i32 (1 if inserted, 0 if updated)\n",
    );
    out.push_str("- `[\"map_u32.remove\",\"m\",\"key\"]` -> i32 (1 if removed, 0 if missing)\n");
    out.push_str("- `[\"set_u32.new\",\"cap_pow2\"]` -> i32\n");
    out.push_str("- `[\"set_u32.contains\",\"s\",\"key\"]` -> i32 (0/1)\n");
    out.push_str(
        "- `[\"set_u32.add\",\"s\",\"key\"]` -> i32 (1 if inserted, 0 if already present)\n",
    );
    out.push_str("- `[\"set_u32.remove\",\"s\",\"key\"]` -> i32 (1 if removed, 0 if missing)\n\n");

    out.push_str("## Collection emitters\n\n");
    out.push_str("Use `emit_*` stdlib functions to produce canonical deterministic encodings:\n\n");
    out.push_str("- `std.hash_set.emit_u32le(set_u32_handle)` -> bytes (sorted ascending u32le)\n");
    out.push_str("- `std.hash_map.emit_kv_u32le_u32le(map_u32_handle)` -> bytes ((k u32le)(v u32le)... sorted by key)\n");
    out.push_str("- `std.btree_set.emit_u32le(btree_set_bytes)` -> bytes (already sorted)\n");
    out.push_str(
        "- `std.btree_map.emit_kv_u32le_u32le(btree_map_bytes)` -> bytes (already sorted)\n",
    );
    out.push_str("- `std.deque_u32.emit_u32le(dq_bytes)` -> bytes (front-to-back)\n");
    out.push_str(
        "- `std.heap_u32.emit_u32le(heap_bytes)` -> bytes (non-decreasing pop-min order; consumes heap_bytes)\n\n",
    );

    out.push_str("## Stdlib (pure)\n\n");
    out.push_str("Prefer calling stdlib helpers through their module namespaces (and include the module in `imports`):\n\n");
    out.push_str("- `std.codec`: `[\"std.codec.read_u32_le\",\"b\",\"off\"]` (`b` is bytes_view), `[\"std.codec.write_u32_le\",\"x\"]`\n");
    out.push_str("- `std.parse`: `[\"std.parse.u32_dec\",\"b\"]`, `[\"std.parse.u32_dec_at\",\"b\",\"off\"]`, `[\"std.parse.i32_status_le\",\"b\"]`\n");
    out.push_str(
        "- `std.fmt`: `[\"std.fmt.u32_to_dec\",\"x\"]`, `[\"std.fmt.s32_to_dec\",\"x\"]`\n",
    );
    out.push_str("- `std.prng`: `[\"std.prng.lcg_next_u32\",\"state\"]`\n\n");
    out.push_str("- `std.bit`: `[\"std.bit.popcount_u32\",\"x\"]`\n\n");

    out.push_str("## Common Templates\n\n");
    out.push_str("- 1-byte output: `[\"begin\",[\"let\",\"out\",[\"bytes.alloc\",1]],[\"set\",\"out\",[\"bytes.set_u8\",\"out\",0,\"x\"]],\"out\"]`\n");
    out.push_str("- Empty output: `[\"bytes.alloc\",0]`\n");
    out.push_str("- Looping: use `bytes.len` once, then `[\"for\",\"i\",0,\"n\",body]`.\n\n");

    out.push_str("### Header + tail pattern\n\n");
    out.push_str("Many tasks use `input[0..k]` as parameters and the remaining bytes as data.\n\n");
    out.push_str("Example (k=1):\n\n");
    out.push_str("- `[\"begin\",[\"let\",\"x\",[\"std.bytes.get_u8\",\"input\",0]],[\"let\",\"n\",[\"std.bytes.len\",\"input\"]],[\"let\",\"v\",[\"std.vec.with_capacity\",[\"-\",\"n\",1]]],[\"for\",\"i\",1,\"n\",[\"std.vec.push\",\"v\",[\"std.bytes.get_u8\",\"input\",\"i\"]]],[\"std.vec.as_bytes\",\"v\"]]`\n\n");

    out
}
