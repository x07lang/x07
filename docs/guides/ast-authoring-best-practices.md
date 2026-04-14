# AST Authoring Best Practices

This guide covers writing and mechanically editing `*.x07.json` (`x07AST`) and using the toolchain to catch common AST-shape mistakes early.

## Canonical Workflow

1. Keep AST JSON canonical while editing:

```bash
x07 fmt --input program.x07.json --write
x07 lint --input program.x07.json
x07 fix --input program.x07.json --write
```

2. Use `x07 check` in two modes:

```bash
# Fast feedback for AST authoring: schema/shape + lint only.
x07 check --project x07.json --ast

# Full check: lint + typecheck + backend-check.
x07 check --project x07.json
```

Notes:

- `x07 check --ast` runs schema validation + shape validation + lint and emits an `x07diag` report.
- `x07 check` also runs project-wide typecheck and backend-check, and surfaces codegen errors in *unreachable* declarations (fail-fast).

3. Try it on the runnable example project:

- `docs/examples/ast-authoring-best-practices/`

```bash
x07 check --project docs/examples/ast-authoring-best-practices/x07.json --ast
x07 check --project docs/examples/ast-authoring-best-practices/x07.json
printf 'hello' | x07 run --repair=off --project docs/examples/ast-authoring-best-practices/x07.json --stdin
```

## Common Shape Errors

### Binary operators are binary

Operators like `+` are binary in x07AST:

- Valid form: `["+", <a>, <b>]`
- Suggested fix for n-ary uses: nest left-associatively (for example `["+", ["+", a, b], c]`)

`x07 check --ast` reports this as `X07-ARITY-BINOP-0001`, and a quickfix may be available for some cases.

### `for` loops have a strict shape

Valid form:

`["for", "i", <start:i32>, <end:i32>, <body:any>]`

If you need multiple body expressions, wrap them in `begin`:

`["for", "i", start, end, ["begin", ...]]`

### `bytes` vs `bytes_view`: move vs borrow

- `bytes` is an owned buffer (move-only).
- `bytes_view` is a borrowed view into an owned buffer.

Common pitfalls (and fixes):

- **Borrowing a view from a temporary** triggers `X07-BORROW-0001`.

  - Bad: `["bytes.view", <expr>]` where `<expr>` is not an identifier.
  - Fix: bind the owner with `let` first (or run `x07 fix` if a quickfix is offered).

- **Borrowing a view from a literal**: prefer `bytes.view_lit`.

  - Bad: `["bytes.view", ["bytes.lit", "..."]]`
  - Good: `["bytes.view_lit", "..."]`

- **Asserts that consume inputs**: `std.test.assert_bytes_eq(a: bytes, b: bytes, ...)` moves both values.

  - If you need to reuse values after the assert, use `std.test.assert_view_eq(["bytes.view", "a"], ["bytes.view", "b"], ...)`.

- **Borrow-union returns**: returning a `bytes_view` from a conditional where branches borrow from different owners is often rejected.

  - Fix: return an owned `bytes` instead (copy in each branch with `view.to_bytes`) or restructure so the view always borrows from the same owner.

A common fix for move errors is to copy before moving a value you still need:

`["view.to_bytes", ["bytes.view", "x"]]`

### Dependency roots (imports)

If `x07 check` fails with an import error (“module not found”), the usual causes are:

- missing `x07.json.module_roots` entry for the directory that contains the module
- missing or stale `x07.lock.json` (or a dependency path that doesn’t exist on disk)

Canonical debugging loop:

```bash
x07 pkg tree --project x07.json
x07 pkg provides <module-id> --project x07.json
x07 pkg lock --project x07.json --check --offline
```

## Expert Notes

- Diagnostic codes are documented in `docs/toolchain/diagnostic-codes.md` and searchable via `x07 diag explain <CODE>`.
- For project-level check behavior and report outputs, see `docs/toolchain/cli.md` (`x07 check`).
