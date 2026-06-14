# Syntax & x07AST (JSON)

X07 uses a structured syntax called **x07AST**.

- The canonical on-disk format is JSON (`*.x07.json`).
- [x07text](x07text.md) is the lossless, human/agent-readable text projection
  of that JSON. `x07 ast from-text` re-canonicalizes x07text back to
  byte-identical JSON, so you can read and author in x07text without giving up
  the JSON-first source of truth.
- Structural edits (JSON Patch / quickfix / MCP patch) still operate on the
  canonical JSON.

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

x07text keeps these guarantees: it is a 1:1 surface over the JSON, so
whole-file conversion re-canonicalizes everything and text editing cannot
accumulate the paren/brace drift above. See the
[x07text projection](#x07text-projection) section below.

## Core expression model

x07AST is an expression tree:

- lists: `["head", arg1, arg2, ...]`
- identifiers: `"std.bytes.len"` (string)
- integers: `123` (number)

The entrypoint expression returns **bytes**.

## Boolean operators (i32)

X07 represents booleans as `i32` (0 = false, nonzero = true).

- `&` / `|`: eager bitwise ops (both sides are evaluated).
- `&&` / `||`: short-circuit logical ops (RHS is evaluated only when needed; result is `0` or `1`).

## Schema version

The root JSON object must include `schema_version`.

Current schema version:

- `x07.x07ast@0.8.0`

The toolchain also accepts legacy schema versions:

- `x07.x07ast@0.7.0` (programs with async protocol contracts but without function `decreases`)
- `x07.x07ast@0.6.0` (programs with loop contracts but without async protocol contracts)
- `x07.x07ast@0.5.0` (programs with function contracts but without loop contracts)
- `x07.x07ast@0.4.0` (programs without contracts)
- `x07.x07ast@0.3.0` (concrete-only programs)

Tooling (`x07 ast init`, `x07 fmt`, `x07 lint`) keeps the AST compatible with the current schema.

## Generics (v0.4)

x07AST v0.4 adds:

- `type_ref` in signatures (either legacy concrete type tokens or structured type expressions)
- `type_params` on `defn` / `defasync`
- `tapp` at call sites
- `ty.*` intrinsics for type-dependent ops inside generic bodies

See [Generics](generics.md).

## Contracts (v0.5)

x07AST v0.5 adds first-class function contracts:

- Optional fields on `defn` / `defasync` declarations:
  - `requires`: array of preconditions
  - `ensures`: array of postconditions
  - `invariant`: array of function-level invariants
- Each clause is an object:
  - `id` (optional string)
  - `expr` (expression; must typecheck to `i32`)
  - `witness` (optional array of expressions; evaluated only on failure)

Notes:

- Contract expressions and witnesses must be contract-pure (no `world.*`, `task.*`, mutation forms, etc.).
- `__result` is reserved and is only available inside `ensures` expressions.

### Contract-pure allowlist

Contract expressions and witnesses may only call contract-pure builtins/operators:

- Operators: `+`, `-`, `*`, `/`, `%`, `=`, `!=`, `<`, `<=`, `>`, `>=`, `<u`, `<=u`, `>u`, `>=u`, `<<u`, `>>u`, `&`, `|`, `^`, `&&`, `||`.
- Builtins: `bytes.lit`, `bytes.view_lit`, `i32.lit`, `bytes.view`, `bytes.subview`, `bytes.len`, `bytes.get_u8`, `bytes.eq`, `bytes.cmp_range`, `view.len`, `view.get_u8`, `view.slice`, `view.to_bytes`.
- Also allowed: any builtin head with prefix `option_*` or `result_*`.

Module calls (like `foo.bar`) are not allowed in contracts.

## Loop contracts (v0.6)

x07AST v0.6 adds `loop_contracts` on `defn` declarations so proof mode can reason about `for` loops without widening the certifiable subset to recursion.

Each item points at a loop body by JSON Pointer and declares:

- `invariant[]`: clauses that must hold at loop entry and every iteration
- `decreases[]`: lexicographic rank terms used to prove termination

Minimal shape (x07text projection of the `loop_contracts` value):

```clojure
; x07text
({:decreases ({:expr (- n i)}) :invariant ({:expr (>= i 0) :id i_nonneg}) :ptr /decls/0/body/4}
)
```

Canonical JSON:

```json
"loop_contracts": [
  {
    "ptr": "/decls/0/body/4",
    "invariant": [
      { "id": "i_nonneg", "expr": [">=", "i", 0] }
    ],
    "decreases": [
      { "expr": ["-", "n", "i"] }
    ]
  }
]
```

Use `x07 verify --prove` with `loop_contracts` when the certified surface needs loops; combine them with `decreases` on `defn` when the same pure core also uses self-recursion.

## Function termination clauses (v0.8)

x07AST v0.8 adds `decreases` on `defn` declarations so pure self-recursive functions can stay inside the certifiable proof subset.

`decreases` uses the same contract-clause object shape as `requires` / `ensures` / `invariant` and declares the lexicographic rank that must descend on recursive self-calls.

Minimal shape (x07text projection of the `defn`):

```clojure
; x07text
{
  :kind defn
  :name main.count_down
  :body (if (= n 0) 0 (main.count_down (- n 1)))
  :decreases ({:expr n :id d0})
  :params ({:name n :ty i32})
  :requires ({:expr (>= n 0) :id r0})
  :result i32
}
```

Canonical JSON:

```json
{
  "kind": "defn",
  "name": "main.count_down",
  "params": [{"name":"n","ty":"i32"}],
  "result": "i32",
  "requires": [{"id":"r0","expr":[">=","n",0]}],
  "decreases": [{"id":"d0","expr":"n"}],
  "body": ["if",["=","n",0],0,["main.count_down",["-","n",1]]]
}
```

Mutual recursion and recursive `defasync` targets remain outside the current certifiable subset.

## Async protocol contracts (v0.7)

x07AST v0.7 adds `protocol` on `defasync` declarations so async code can declare proof obligations across suspension points and task scopes.

`protocol` may include:

- `await_invariant[]`
- `scope_invariant[]`
- `cancellation_ensures[]`

These clauses use the same contract-clause object shape as `requires` / `ensures` / `invariant`, and they must remain contract-pure.

## Branded bytes annotations

X07 can attach a nominal **brand** to bytes-like parameters and results to model “validated bytes of encoding X” (compile-time only).

In `defn`/`defasync` declarations:

- `params[]` entries may include optional `brand` when `ty` is bytes-like.
- `result_brand` may be provided when `result` is bytes-like.

Example (x07text projection of the `defn`):

```clojure
; x07text
{
  :kind defn
  :name main.parse_x7sl
  :body (std.text.slices.cast_bytes_v1 b)
  :params ({:name b :ty bytes})
  :result result_bytes
  :result_brand std.text.slices.x7sl_v1
}
```

Canonical JSON:

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

## x07text projection

The `; x07text` blocks above are the lossless text projection of the canonical
JSON shown next to them. Conversion is exact in both directions:

- `x07 ast to-text` renders a `*.x07.json` file as x07text.
- `x07 ast from-text` parses x07text back into canonical JCS JSON —
  byte-identical to `x07 fmt` output — so the on-disk source stays JSON.

Author in whichever surface you prefer; the JSON remains the source of truth and
structural tooling (JSON Patch, quickfix, MCP patch) keeps operating on it. See
[x07text: the text projection](x07text.md) for the full format reference.

## Tooling

For a full project skeleton (`x07.json`, lockfile, `src/`, `tests/`), use `x07 init`. The `x07 ast` subcommands are for working with individual `*.x07.json` files.

- `x07 ast init` / `x07 ast validate`:
  - generates and validates `*.x07.json` skeletons
- `x07 ast to-text` / `x07 ast from-text`:
  - render canonical JSON as x07text and re-canonicalize x07text back to
    byte-identical JSON
- `x07 fmt`:
  - canonicalizes JSON (JCS) and ensures stable emission
- `x07 lint`:
  - checks schema + semantic constraints
  - prints `x07diag` (machine-readable diagnostics)
- `x07 fix`:
  - applies quickfixes (JSON Patch) and rewrites `*.x07.json` deterministically
- `x07 ast apply-patch`:
  - applies RFC 6902 JSON Patch deterministically (agent-safe edits)
- `x07 ast schema`:
  - emits the canonical JSON Schema for constrained/structured generation engines
- `x07 ast grammar --cfg`:
  - emits GBNF variants plus semantic supplement for grammar-constrained decoding

## Common mistakes (and the canonical fixes)

### 1) Using multi-step logic where a single expression is required

Fix: wrap with `["begin", ...]`.

### 2) Returning the wrong type from the entrypoint

Fix: ensure the final expression is bytes.

### 3) Calling OS-only builtins in fixture worlds

Fix: keep OS builtins behind OS adapters and world checks.
