---
name: x07-agent-playbook
description: Agent-first workflow and design rails for building X07 programs with the released toolchain (no repo-only dependencies).
metadata:
  short-description: X07 agent workflow + rails
  version: 0.1.0
  kind: docs
---

# x07-agent-playbook

This skill sets the baseline workflow and constraints for autonomous agents writing X07 programs.
It assumes end-users only have the released toolchain binaries, not the toolchain source repo.

## Tooling

See `references/tooling.md`.

## Single canonical agent loop (edit → format → lint → run)

1. Create or edit x07AST JSON (`*.x07.json`).
2. Canonicalize formatting:
   - `x07c fmt --input src/main.x07.json --write --report-json`
3. Lint (world-gating + structural checks):
   - `x07c lint --input src/main.x07.json --world solve-pure --report-json`
4. Apply tool-provided quickfixes (when available):
   - `x07c fix --input src/main.x07.json --world solve-pure --write --report-json`
5. If a targeted structural change is needed, apply an explicit JSON Patch:
   - `x07 ast apply-patch --in src/main.x07.json --patch /tmp/repair.patch.json --validate`
6. Run in the correct capability world:
   - solve worlds: `x07-host-runner --program src/main.x07.json --world solve-pure --module-root src`
   - OS worlds: `x07-os-runner --program src/main.x07.json --world run-os`
7. If the project uses dependencies, update the lockfile:
   - `x07 pkg lock --project x07.json --index <url>`

Keep each iteration small and deterministic; if a repair loop does not converge quickly, stop and re-evaluate the approach.

## Recommended project layout (single canonical shape)

- `x07.json`: project manifest (`x07.project@0.2.0`)
- `x07.lock.json`: project lockfile (or `lockfile` configured in `x07.json`)
- `src/main.x07.json`: entry
- `src/`: module roots
- `.x07/deps/<name>/<version>/`: fetched dependencies (when using `x07 pkg lock`)

## Agent-first design rails

See `references/design-rails.md`.

For a built-in language/stdlib reference (toolchain-only), use `x07c guide`.
