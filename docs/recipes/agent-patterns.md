# Agent patterns (canonical recipes)

This page collects the "do this every time" patterns that trip up agents most frequently. Each section is self-contained: read only the section you need.

## 1) bytes vs bytes_view

X07 has two string-like types: `bytes` (owned) and `bytes_view` (borrowed reference). Most friction comes from mixing them incorrectly.

### Bind then view

`bytes.view` requires an **identifier** (a named local), not an arbitrary expression. Always bind the bytes-producing expression to a local first:

```json
["let", "raw", ["bytes.lit", "hello"]],
["let", "v", ["bytes.view", "raw"]]
```

This fails:

```json
["bytes.view", ["bytes.lit", "hello"]]
```

The error message says "requires an identifier" — the fix is always to bind first.

### Parameter passing (bytes_view arguments)

When a function expects `bytes_view`, the caller must pass an identifier. Bind the result to a local first:

```json
["let", "payload", ["bytes.concat", ["bytes.lit", "prefix:"], "name"]],
["let", "pv", ["bytes.view", "payload"]],
["my_module.process_v1", "pv"]
```

### if branches and ownership

Create borrows only in the branch that needs them. Don't create a `bytes_view` before the `if` and then move the owner inside a branch:

```json
["let", "data", ["bytes.lit", "value"]],
["if", ["=", "flag", 1],
  ["begin",
    ["let", "dv", ["bytes.view", "data"]],
    ["some_fn", "dv"]
  ],
  ["begin",
    ["consume_bytes", "data"]
  ]
]
```

The borrow (`dv`) exists only in the branch that needs it. The other branch can move `data` freely.

### view.to_bytes at the ownership boundary

When you need to return owned `bytes` from a `bytes_view`, use `view.to_bytes` at exactly the point where ownership is required:

```json
["let", "line", ["std.text.ascii.kth_line_view", "b", 0]],
["view.to_bytes", "line"]
```

Don't call `view.to_bytes` early — keep data as `bytes_view` as long as possible to avoid copies.

## 2) Testing: naming, assertions, fixture paths

### Symbol naming rule

In `module_id` and function names, dots before the last segment are the **module id**, not namespacing. Use flat names within a module:

- `smoke.pure_add` — module `smoke`, function `pure_add`
- `smoke.tests.pure_add` — module `smoke.tests`, function `pure_add`

If you want test functions in the same module as production code, name them `smoke.test_add_v1`, not `smoke.tests.add_v1` (the latter implies a separate module `smoke.tests`).

### assert_bytes_eq consumes inputs

`std.test.assert_bytes_eq` **moves** (consumes) both arguments. After the assertion, neither value is usable:

```json
["std.test.assert_bytes_eq", "actual", "expected", ["std.test.code_assert_bytes_eq"]]
```

If you need to reuse a value after assertion, use `std.test.assert_view_eq` instead — it takes `bytes_view` arguments and does not consume them:

```json
["let", "av", ["bytes.view", "actual"]],
["let", "ev", ["bytes.view", "expected"]],
["std.test.assert_view_eq", "av", "ev", ["std.test.code_assert_view_eq"]]
```

### Test harness cwd

`x07 test` sets the working directory to the **manifest directory** (the directory containing `tests.json`), not the project root.

If your manifest is at `tests/tests.json`, then fixture reads resolve relative to `tests/`:

- Correct: `fixtures/input.bin` (resolves to `tests/fixtures/input.bin`)
- Incorrect: `tests/fixtures/input.bin` (resolves to `tests/tests/fixtures/input.bin`)

## 3) Reserved identifiers in defn params

`input` is **reserved** in `defn` parameter lists — it conflicts with the entry-point input binding. The compiler rejects it silently or with an opaque error.

Rename to any other identifier:

- `in_bytes`
- `event_data`
- `payload`
- `req`

```json
{
  "kind": "defn",
  "name": "app.handle_v1",
  "params": ["in_bytes"],
  "result": "result_bytes",
  "body": ["begin", "..."]
}
```

## 4) Structured concurrency: task.scope_v1 + collecting results

### Fixed locals (small, known fan-out)

When you know the number of concurrent tasks at compile time, use named locals:

```json
["task.scope_v1",
  ["begin",
    ["let", "h1", ["my.task_a"]],
    ["let", "h2", ["my.task_b"]],
    ["task.spawn", "h1"],
    ["task.spawn", "h2"],
    ["let", "r1", ["await", "h1"]],
    ["let", "r2", ["await", "h2"]],
    ["bytes.concat", "r1", "r2"]
  ]
]
```

### Channel-based fanout (dynamic or large N)

For dynamic fan-out, use `task.scope.start_soon_v1` with channels:

```json
["task.scope_v1",
  ["begin",
    ["let", "ch", ["chan.bytes.make", 16]],
    ["task.scope.start_soon_v1", ["my.worker_v1", "item1", "ch"]],
    ["task.scope.start_soon_v1", ["my.worker_v1", "item2", "ch"]],
    ["chan.bytes.close_send", "ch"],
    ["let", "results", ["chan.bytes.collect", "ch"]],
    "results"
  ]
]
```

Each worker sends its result into the channel; the parent collects all results after closing the send side.

## 5) JSON report construction

Don't concatenate `bytes.lit` fragments manually to build JSON — escaping bugs produce `"ERR"` from `std.json.canonicalize_small`.

### Canonical approach: build-then-canonicalize

Build each field value as bytes, then assemble and canonicalize:

```json
["let", "key", ["bytes.lit", "status"]],
["let", "val", ["bytes.lit", "ok"]],
["let", "raw", ["bytes.concat",
  ["bytes.lit", "{\""],
  "key",
  ["bytes.lit", "\":\""],
  "val",
  ["bytes.lit", "\"}"]
]],
["std.json.canonicalize_small", ["bytes.view", "raw"]]
```

### If using ext-data-model

Build a map with the DataModel API, then emit canonical JSON. This avoids all manual escaping:

```json
["let", "doc", ["ext.dm.map_new"]],
["ext.dm.map_set", "doc", ["bytes.lit", "status"], ["bytes.lit", "ok"]],
["ext.dm.to_json_canonical", "doc"]
```

Prefer the DataModel approach for any JSON with dynamic or user-supplied values.

## 6) Exit codes

Use `std.process.set_exit_code_v1` for nonzero exit on failure. This works in `run-os` and `run-os-sandboxed`:

```json
["if", ["=", "success", 0],
  ["begin",
    ["std.process.set_exit_code_v1", 1],
    ["bytes.lit", "FAIL"]
  ],
  ["bytes.lit", "OK"]
]
```

The exit code appears in the runner report's `exit_code` field.
