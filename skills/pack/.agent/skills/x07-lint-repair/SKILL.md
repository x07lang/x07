---
name: x07-lint-repair
description: Lint X07 x07AST JSON and converge via quickfixes (`x07 fix`) or explicit JSON Patch (`x07 ast apply-patch`).
metadata:
  short-description: Lint + fix + patch loop
  version: 0.1.0
  kind: docs
---

# x07-lint-repair

Use this skill when an X07 file fails linting and the agent needs a repeatable converge loop.

Note: `x07 run`, `x07 build`, and `x07 bundle` run the same auto-repair loop by default (`--repair=...`). Use this skill when you want raw diagnostics or tighter control.

## Canonical converge loop

1. Lint (machine-readable JSON report):
   - `x07 lint --input src/main.x07.json --json`

2. Apply tool-provided quickfixes (when diagnostics include JSON Patch quickfixes):
   - `x07 fix --input src/main.x07.json --write --json`

3. If a custom fix is needed, apply an explicit JSON Patch (RFC 6902):
   - `x07 ast apply-patch --in src/main.x07.json --patch /tmp/repair.patch.json --validate`

4. Canonicalize after patching:
   - `x07 fmt --input src/main.x07.json --write --json`

Repeat (max 3 iterations). If still failing, stop and change strategy (reduce scope, regenerate the x07AST cleanly, or ask for clarification).

## Migration (compat upgrades)

If diagnostics indicate a compatibility or migration issue (for example `bytes`/`bytes_view` coercions or recursion termination boilerplate in older code), prefer deterministic rewrites:

- `x07 migrate --to 0.5 --check --input <path>`
- `x07 migrate --to 0.5 --write --input <path>`

For one-off debugging, you can temporarily override compat on execution/test entry points:

- `x07 run --compat 0.5`
- `x07 test --compat 0.5 --manifest tests/tests.json`

Related:

- If the failure came from `x07 test --pbt` (a counterexample repro), use `x07 fix --from-pbt <repro.json> --write` to generate a deterministic regression test.
