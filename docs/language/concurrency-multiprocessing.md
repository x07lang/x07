# Concurrency & multiprocessing

X07 intentionally separates:

- **deterministic concurrency** (for tests and fixture worlds)
- **OS threads** (for OS-world blocking/I/O concurrency; policy-gated)
- **OS multiprocessing** (for production parallelism)

## Deterministic concurrency (async, single-core)

In fixture worlds, X07’s async system:

- is cooperative (tasks yield at known points)
- runs on a deterministic scheduler
- uses virtual time (ticks) instead of wall-clock time

This enables:
- deterministic pipelines,
- repeatable performance measurements,
- reproducible repair loops.

### Async functions (`defasync`) and task handles

Async functions are defined with `defasync`.

- A `defasync` returns an awaited type: `bytes` or `result_bytes`.
- Calling a `defasync` returns an opaque task handle (`i32` in x07AST).
  - The handle’s “kind” is determined by the awaited return type (`bytes` vs `result_bytes`).

Task ops:

- `["task.spawn", task_handle] -> i32` (stats/registration; optional for most code)
- `["await", <bytes task handle>] -> bytes` (alias of `task.join.bytes`)
- `["task.try_join.bytes", <bytes task handle>] -> result_bytes` (`err=1` not finished; `err=2` canceled)
- `["task.join.bytes", <bytes task handle>] -> bytes`
- `["task.try_join.result_bytes", <result_bytes task handle>] -> result_result_bytes` (`err=1` not finished; `err=2` canceled)
- `["task.join.result_bytes", <result_bytes task handle>] -> result_bytes`
- `["task.is_finished", task_handle] -> i32` (0/1)
- `["task.cancel", task_handle] -> i32`
- `["task.yield"] -> i32`
- `["task.sleep", ticks_i32] -> i32` (virtual time ticks)

Note: `await` / `task.join.*` are only allowed in `solve` and inside `defasync` bodies (not inside `defn`).

### Channels (bytes payloads)

- `["chan.bytes.new", cap_i32] -> i32`
- `["chan.bytes.try_send", chan_handle, bytes_view] -> i32` (0 full; 1 sent; 2 closed)
- `["chan.bytes.send", chan_handle, bytes] -> i32`
- `["chan.bytes.try_recv", chan_handle] -> result_bytes` (`err=1` empty; `err=2` closed)
- `["chan.bytes.recv", chan_handle] -> bytes`
- `["chan.bytes.close", chan_handle] -> i32`

### Structured concurrency (`task.scope_v1`)

`task.scope_v1` is a structured concurrency scope (“nursery”) that guarantees: **tasks started inside it cannot outlive it**.

Shape:

```jsonc
["task.scope_v1",
  ["task.scope.cfg_v1",
    ["max_children", <u32>],
    ["max_ticks", <u64>],
    ["max_blocked_waits", <u64>],
    ["max_join_polls", <u64>],
    ["max_slot_result_bytes", <u32>]
  ],
  <body_expr>
]
```

Inside a scope:

- `["task.scope.start_soon_v1", <immediate_defasync_call_expr>] -> i32`
  - The call expression must be an immediate `defasync` call (compile-time enforced).
  - The child task handle is not returned (prevents orphan-task patterns).
- `["task.scope.cancel_all_v1"] -> i32` (cancels all children; deterministic order)
- `["task.scope.wait_all_v1"] -> i32` (joins+drops all children so far; keeps scope open)

### Scoped slots (`async_let`)

Slots are scope-owned handles for child task results. They are not raw task handles, and they must not escape `task.scope_v1`.

- `["task.scope.async_let_bytes_v1", <immediate_defasync_call_expr>] -> i32` (slot id)
- `["task.scope.async_let_result_bytes_v1", <immediate_defasync_call_expr>] -> i32` (slot id)
- `["task.scope.await_slot_bytes_v1", slot_id] -> bytes`
- `["task.scope.await_slot_result_bytes_v1", slot_id] -> result_bytes`
- `["task.scope.try_await_slot.bytes_v1", slot_id] -> result_bytes` (`err=1` not ready; `err=2` canceled)
- `["task.scope.try_await_slot.result_bytes_v1", slot_id] -> result_result_bytes` (`err=1` not ready; `err=2` canceled)
- `["task.scope.slot_is_finished_v1", slot_id] -> i32` (0/1)

### Scoped select (`task.scope.select_*_v1`)

Select waits for one of several scope-owned events deterministically:

- `["task.scope.select_v1", <cfg_v1>, <cases_v1>] -> i32` (select evt id)
- `["task.scope.select_try_v1", <cfg_v1>, <cases_v1>] -> option_i32` (optional select evt id)

```jsonc
["task.scope.select.cfg_v1",
  ["max_cases", <u32>],
  ["policy", "priority_v1" | "rr_v1"],
  ["poll_sleep_ticks", <u32>],
  ["max_polls", <u32>],
  ["timeout_ticks", <u32>]
]
```

```jsonc
["task.scope.select.cases_v1",
  ["task.scope.select.case_slot_bytes_v1", slot_id],
  ["task.scope.select.case_chan_recv_bytes_v1", <i32_chan_handle>]
]
```

The returned select event handle (`evt_id: i32`) is scope-owned:

- `["task.select_evt.tag_v1", evt_id] -> i32`
- `["task.select_evt.case_index_v1", evt_id] -> i32` (0-based index in `cases_v1`)
- `["task.select_evt.src_id_v1", evt_id] -> i32` (slot id or chan id)
- `["task.select_evt.take_bytes_v1", evt_id] -> bytes` (moves payload bytes; only valid for “bytes-ready” events)
- `["task.select_evt.drop_v1", evt_id] -> i32` (drops payload, if any)

Event tags (stable):

- `1`: slot bytes ready (payload = bytes)
- `2`: slot canceled (no payload)
- `3`: chan recv bytes ready (payload = bytes)
- `4`: chan closed (no payload)
- `5`: timeout (no payload)

## OS threads (policy-gated)

In OS worlds, X07 can use threads for blocking and I/O-heavy work. In `run-os-sandboxed`, this is gated by policy.

The `threads` policy section controls thread-backed blocking operations. Setting:

- `threads.max_blocking = 0`

disables blocking operations and produces a stable trap:

- `os.threads.blocking disabled by policy`

## OS multiprocessing (multi-core)

In OS worlds, X07 can spawn subprocesses:

- the OS schedules them across cores
- policies limit what can be spawned, how many, and resource bounds

In `run-os-sandboxed`, process spawning must be explicitly enabled by policy. If your program needs to spawn helper processes for parallel work, start from:

- `x07 policy init --template worker-parallel`

If you don’t need process spawning, prefer the stricter `worker` template.

This provides a safe default path to “real parallel work” without bringing nondeterminism into the deterministic test substrate.

Guideline:
- Keep core logic testable in fixture worlds.
- Use subprocess adapters at the edge for CPU-parallel work.
