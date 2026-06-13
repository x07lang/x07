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

Note: `await` / `task.join.*` — and likewise `task.scope_v1` and all
`task.scope.*` ops — are only allowed in `solve` and inside `defasync` bodies,
**not** inside a plain `defn`. This is enforced by the compiler. See
[Concurrency and certification](#concurrency-and-certification-kernel-and-shell)
for why this shapes the trust story.

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

### Static vs dynamic fan-out

There are two ways to fan work out across child tasks, and they differ in
whether the number of children is known at authoring time:

- **Static arity (named slots).** Use
  `task.scope.async_let_bytes_v1` (or `…_result_bytes_v1`) to start a fixed,
  small set of children, then `task.scope.await_slot_bytes_v1` each slot by id.
  The slot ids are distinct named handles, so the child count is fixed in the
  source. Reach for this when you know exactly how many subtasks there are
  (e.g. two halves, three stages).
  See `docs/examples/14_task_scope_slots.x07.json`.

- **Dynamic arity (channels).** Use `task.scope.start_soon_v1` in a loop to
  start a runtime-determined number of children, and a bytes channel
  (`chan.bytes.new` / `chan.bytes.send` / `chan.bytes.recv`) to collect their
  results. `start_soon_v1` does not return a handle (no per-child slot), so the
  worker count can be a runtime value. Reach for this for map/reduce-style work
  where the number of workers (or chunks) is computed.
  See `docs/examples/12_async_mapreduce.x07.json`.

Both run under the same `task.scope_v1` nursery and the same deterministic
scheduler, so both are reproducible.

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

## Concurrency and certification: kernel and shell

`task.scope_v1`, every `task.scope.*` op, and `await` / `task.join.*` are
allowed **only** in `solve` and inside `defasync` bodies — never in a plain
`defn`. The compiler enforces this (`x07 doc --builtin task.scope_v1` states it
directly: *"Only in solve/defasync contexts"*).

This has a direct consequence for the trust story. XTAL certifies a `defn`
named by the project's `operational_entry_symbol`, and a certifiable pure entry
runs under a restricted language subset with `allow_defasync: false`. So a
concurrent program's **orchestration cannot itself be the certified entry** —
the orchestration lives in `solve` (or in `defasync` helpers), which the pure
certificate deliberately excludes.

The canonical resolution is the **kernel/shell** split:

- **Kernel** — the deterministic, pure logic, written as a `defn` in a module
  and named by `operational_entry_symbol`. This is what XTAL certifies (with
  `requires` / `ensures` contracts and proof objects). It contains no
  `task.scope_v1` and no `defasync`.
- **Shell** — the `task.scope_v1` orchestration in the `solve` body (plus any
  `defasync` workers). The shell calls the kernel and is validated against it
  by golden end-to-end examples.

The two produce **byte-identical output**: the concurrent shell exists only to
schedule the work; the kernel decides the result. You certify the kernel and
test the shell. See the worked
`docs/examples/verified_core_pure_v1/` project (a pure `defn` entry under
`world: solve-pure`) and the
[Kernel/shell in production](../guides/kernel-shell-production.md) guide.

A practical corollary, combined with [generics](generics.md): a generic numeric
reduction is not expressible (no generic arithmetic), so a parallel map/reduce
keeps its per-chunk reducer **monomorphic** (concrete `i32` / `u32`) — exactly
what `docs/examples/12_async_mapreduce.x07.json` does.

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
