# Budget scopes (`budget.scope_v1`)

Budget scopes are a compiler/runtime primitive for **local** resource contracts. They let you put deterministic caps around a specific region of code so agent edits can’t accidentally blow up cost elsewhere.

## AST shape

```jsonc
["budget.scope_v1",
  ["budget.cfg_v1",
    ["mode", "trap_v1"],
    ["label", ["bytes.lit", "parse_headers"]],

    // optional caps (0 = unlimited):
    ["alloc_bytes", 65536],
    ["alloc_calls", 0],
    ["realloc_calls", 0],
    ["memcpy_bytes", 1048576],
    ["sched_ticks", 0],
    ["fuel", 0]
  ],
  <body_expr>
]
```

Notes:

- `budget.cfg_v1` is a descriptor; use it only as the first argument to `budget.scope_v1`.
- `mode` and `label` are required. All caps are optional.

## Modes (`budget.cfg_v1.mode`)

- `trap_v1`: on budget violation, trap at scope exit (deterministic trap code).
- `result_err_v1`: on budget violation, overwrite the scope result with a budget error code.
  - Only valid when the scope body evaluates to a `result_*` type.
  - Budget violations take precedence over any existing error code.
- `stats_only_v1`: track stats but never trap / return budget errors.
- `yield_v1`: uses `sched_ticks` as a deterministic yield trigger (instead of trapping). Other caps still trap.

## Arch-driven scopes (`budget.scope_from_arch_v1`)

To keep budgets pinned and reviewable, store them as data under `arch/budgets/` and reference them by id:

```jsonc
["budget.scope_from_arch_v1",
  ["bytes.lit", "hot_parse_v1"],
  <body_expr>
]
```

This loads:

- `arch/budgets/profiles/hot_parse_v1.budget.json` (`schema_version: "x07.budget.profile@0.1.0"`)

`x07 arch check` can:

- validate the budgets index: `arch/budgets/index.x07budgets.json` (`schema_version: "x07.arch.budgets.index@0.1.0"`)
- enforce that selected boundaries are wrapped in `budget.scope_from_arch_v1`
- enforce that `profile_id` is declared in the budgets index

