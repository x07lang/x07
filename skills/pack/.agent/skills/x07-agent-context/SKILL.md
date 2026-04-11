---
name: x07-agent-context
description: Produce deterministic, token-efficient context packs (`x07 agent context`) that agents can use to repair a failing X07 project.
metadata:
  short-description: Deterministic context packs for agents
  version: 0.1.0
  kind: docs
---

# x07-agent-context

Use `x07 agent context` to generate a single deterministic artifact that captures:

- the diagnostics (focus = first `severity=error`, else first),
- an AST slice of the project entry at the focused diagnostic pointer,
- stable input digests for traceability.

Canonical docs:

- https://x07lang.org/docs/toolchain/agent-context/
- https://x07lang.org/docs/toolchain/repair-loop/

## When to run it

Run this after a lint/check/build/run failure when you need to hand an agent the *minimum* closed context to fix the problem without guessing.

## Minimal workflow (recommended)

1) Produce a deterministic diagnostics file (example: lint):

- `x07 lint --input src/main.x07.json > .x07/lint.last.json || true`

2) Build the context pack:

- `x07 agent context --diag .x07/lint.last.json --project x07.json --out .x07/agent.context.json`

3) Provide `.x07/agent.context.json` to the agent as the primary input.

## Diagnostics inputs (`--diag`)

`--diag` accepts:

- raw `x07diag` JSON (`schema_version: "x07.x07diag@0.1.0"`) produced by `x07 lint` / `x07 check`, or
- a tool wrapper report (`schema_version: "x07.tool.*.report@0.1.0"`) where `diagnostics[]` is extracted.

## Controlling slice size and closure

The pack embeds an AST slice of the project entry module at the focused diagnostic pointer. Use these knobs to trade off size vs completeness:

- `--enclosure decl|defn|module` (default: `decl`)
- `--closure locals|types|imports|all` (default: `all`)
- `--max-nodes N` (hard bound on `slice_ast.decls`)
- `--max-bytes BYTES` (hard bound on canonical JSON size of `slice_ast`)

Typical presets:

- Small, local fix: `--enclosure defn --closure locals --max-nodes 40 --max-bytes 60000`
- Import/type issues: `--enclosure decl --closure imports --max-nodes 80 --max-bytes 120000`
- Complex context: leave defaults, or bound only `--max-bytes`

## Feeding the pack to an agent

Include:

- the context pack file,
- the exact failing command you ran,
- the desired end state (tests passing, `x07 run` ok, etc).

If the agent edits files, re-run the canonical loop (`x07 fmt` → `x07 lint` → `x07 fix` → rerun) and regenerate the pack if diagnostics move.

