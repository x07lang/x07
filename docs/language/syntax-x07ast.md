# Syntax & x07AST (JSON)

X07 uses a structured syntax called **x07AST**.

- Humans can work with a pretty form.
- Agents should operate directly on the JSON AST (no fragile text edits).

## Why JSON AST?

Text-based syntaxes (even simple ones) often fail under autonomous patching:

- parentheses drift
- braces drift
- indentation drift
- accidental truncation

A JSON AST:

- is easy to validate,
- is easy to patch with JSON Patch,
- is easy to autoformat deterministically,
- makes tooling (lint/repair) reliable.

## Core expression model

x07AST is an expression tree:

- lists: `["head", arg1, arg2, ...]`
- identifiers: `"std.bytes.len"` (string)
- integers: `123` (number)

The entrypoint expression returns **bytes**.

## Schema version

The root JSON object must include `schema_version`.

Current schema version:

- `x07.x07ast@0.3.0`

Tooling (`x07 ast init`, `x07 fmt`, `x07 lint`) keeps the AST compatible with the current schema.

## Branded bytes annotations

X07 can attach a nominal **brand** to bytes-like parameters and results to model “validated bytes of encoding X” (compile-time only).

In `defn`/`defasync` declarations:

- `params[]` entries may include optional `brand` when `ty` is bytes-like.
- `result_brand` may be provided when `result` is bytes-like.

Example:

```jsonc
{
  "kind": "defn",
  "name": "main.parse_x7sl",
  "params": [{"name":"b","ty":"bytes"}],
  "result": "result_bytes",
  "result_brand": "std.text.slices.x7sl_v1",
  "body": ["std.text.slices.cast_bytes_v1","b"]
}
```

## Tooling

For a full project skeleton (`x07.json`, lockfile, `src/`, `tests/`), use `x07 init`. The `x07 ast` subcommands are for working with individual `*.x07.json` files.

- `x07 ast init` / `x07 ast validate`:
  - generates and validates `*.x07.json` skeletons
- `x07 fmt`:
  - canonicalizes JSON (JCS) and ensures stable emission
- `x07 lint`:
  - checks schema + semantic constraints
  - prints `x07diag` (machine-readable diagnostics)
- `x07 fix`:
  - applies quickfixes (JSON Patch) and rewrites `*.x07.json` deterministically
- `x07 ast apply-patch`:
  - applies RFC 6902 JSON Patch deterministically (agent-safe edits)

## Common mistakes (and the canonical fixes)

### 1) Using multi-step logic where a single expression is required

Fix: wrap with `["begin", ...]`.

### 2) Returning the wrong type from the entrypoint

Fix: ensure the final expression is bytes.

### 3) Calling OS-only builtins in fixture worlds

Fix: keep OS builtins behind OS adapters and world checks.
