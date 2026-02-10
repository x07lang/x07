# Record/replay (`std.rr`)

X07’s record/replay system (“rr”) turns real-world boundary interactions into **deterministic** fixture inputs by storing them in cassette files that can be replayed later.

This is a core piece of the “agent-first” loop: failures become reproducible, and repairs can be validated without re-running real side effects.

## Cassette location (`.x07_rr/`)

Cassette files live under the repo-local directory:

- `.x07_rr/`

When you pass `cassette_path`, it is interpreted as a path **relative to `.x07_rr/`**.

## Canonical scope: `std.rr.with_policy_v1`

`std.rr.with_policy_v1` is the default, structured way to use rr in programs.

Shape:

```jsonc
["std.rr.with_policy_v1",
  ["bytes.lit", "POLICY_ID"],
  ["bytes.lit", "CASSETTE_PATH"],
  ["i32.lit", 2],
  <body_expr>
]
```

- `POLICY_ID`: selects a pinned policy from `arch/rr/` (via the rr index).
- `CASSETTE_PATH`: cassette file path under `.x07_rr/`.
- mode (`i32.lit`):
  - `0`: `off`
  - `1`: `record_v1`
  - `2`: `replay_v1`
  - `3`: `record_missing_v1`
  - `4`: `rewrite_v1`

`std.rr.with_policy_v1` is a structured scope: it opens the cassette on entry and closes it on exit (even if the body returns early).

## Advanced scope: `std.rr.with_v1`

If you already have a cfg doc in bytes form (for example, built by your program), you can use:

```jsonc
["std.rr.with_v1", cfg_bytes_view_expr, <body_expr>]
```

## Low-level API (`std.rr` module)

Most application code should use rr through higher-level adapters (for example, stream pipes sources/sinks or package-specific wrappers).

If you need explicit control, import `std.rr` and use its handle-based API:

- `std.rr.open_v1(cfg: bytes_view) -> result_i32` (ok = handle id)
- `std.rr.close_v1(handle: i32) -> i32`
- `std.rr.next_v1(handle, kind, op, key) -> result_bytes`
- `std.rr.append_v1(handle, entry) -> result_i32`
- `std.rr.entry_resp_v1(entry) -> bytes`
- `std.rr.entry_err_v1(entry) -> i32`
- `std.rr.stats_v1(handle) -> bytes`
- `std.rr.current_v1() -> i32` (current dynamic-scope handle id, or 0)

Tip: `x07 guide` includes the stable signature list for rr and other built-ins (search for `std.rr`).

## Streaming integration (pipes)

Inside rr scopes, stream pipes can consume cassette entries as streams:

- `std.stream.src.rr_send_v1` (source descriptor for `std.stream.pipe_v1`)

Example (collect a streamed response payload):

```jsonc
["std.rr.with_policy_v1",
  ["bytes.lit","smoke_rr_v1"],
  ["bytes.lit","smoke.rrbin"],
  ["i32.lit",2],
  ["begin",
    ["let","doc",
      ["std.stream.pipe_v1",
        ["std.stream.cfg_v1",
          ["chunk_max_bytes",64],
          ["bufread_cap_bytes",64],
          ["max_in_bytes",1048576],
          ["max_out_bytes",1048576],
          ["max_items",1048576]
        ],
        ["std.stream.src.rr_send_v1", ["std.stream.expr_v1", ["bytes.lit","K"]]],
        ["std.stream.chain_v1"],
        ["std.stream.sink.collect_bytes_v1"]
      ]
    ],
    "doc"
  ]
]
```

## Arch files (pinned contracts)

When using `std.rr.with_policy_v1`, rr policy/config comes from the repo’s `arch/rr/` folder:

- `arch/rr/index.x07rr.json` (`schema_version: "x07.arch.rr.index@0.1.0"`)
- `arch/rr/policies/*.policy.json` (`schema_version: "x07.arch.rr.policy@0.1.0"`)
- `arch/rr/sanitizers/*.sanitize.json` (`schema_version: "x07.arch.rr.sanitize@0.1.0"`)

## Fixture mode (test harness)

When running rr tests through `x07 test` (the test harness):

- In the test manifest (`tests/tests.json`), `fixture_root` is copied into the runner’s `.x07_rr/` directory before the program executes.
- Cassette paths in `std.rr.with_policy_v1` still resolve relative to `.x07_rr/` (so the common case is just the filename).

### Canonical setup

```
tests/
├── tests.json
└── fixtures/
    └── replay/
        └── rr/
            └── smoke.rrbin
```

In `tests/tests.json`:

```json
{
  "schema_version": "x07.tests_manifest@0.1.0",
  "tests": [
    {
      "id": "rr_smoke",
      "world": "solve-rr",
      "entry": "app.rr_smoke_v1",
      "expect": "pass",
      "fixture_root": "fixtures/replay/rr"
    }
  ]
}
```

In the program, `cassette_path` is just the filename (relative to `fixture_root/rr/`):

```jsonc
["std.rr.with_policy_v1",
  ["bytes.lit", "smoke_rr_v1"],
  ["bytes.lit", "smoke.rrbin"],
  ["i32.lit", 2],
  <body_expr>
]
```

This resolves to `tests/fixtures/replay/rr/smoke.rrbin`.

### Common mistake

Using `fixture_root: "."` with `cassette_path: ".x07_rr/smoke.rrbin"` — this fails safety checks because the rr fixture resolver does not allow path traversal into `.x07_rr/`. Always use a dedicated fixture directory (for example `tests/fixtures/replay/rr/`) and keep cassettes under it.

Note: In `solve-full`, `fixture_root` must contain an `rr/` directory (alongside `fs/` and/or `kv/` as needed). For `solve-rr`, `fixture_root` itself is treated as the rr fixture directory.

## Minimal `arch/rr/` example (copy/paste)

For a schema-valid starting point, see:

- `docs/examples/contracts_project/arch/rr/index.x07rr.json`
- `docs/examples/contracts_project/arch/rr/policies/smoke_rr_v1.policy.json`
- `docs/examples/contracts_project/arch/rr/sanitizers/sanitize_none_v1.sanitize.json`
