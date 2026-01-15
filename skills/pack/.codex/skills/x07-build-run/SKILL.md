---
name: x07-build-run
description: Compile and run deterministic solve-world X07 programs via x07-host-runner, producing a machine-readable JSON report.
metadata:
  short-description: Compile + run (solve worlds)
  version: 0.1.0
  kind: docs
---

# x07-build-run

Use this skill to compile and run X07 programs in deterministic solve worlds.

## Canonical commands

- Run a single program (solve-pure):
  - `x07-host-runner --program src/main.x07.json --world solve-pure --module-root src`

- Run a project (uses `x07.json` + `x07.lock.json`):
  - `x07-host-runner --project x07.json`

- Provide binary stdin:
  - `x07-host-runner --program src/main.x07.json --world solve-pure --module-root src --input input.bin`

- Compile only (no execution):
  - `x07-host-runner --program src/main.x07.json --world solve-pure --module-root src --compile-only`

## Output contract

`x07-host-runner` prints pretty JSON including `schema_version`, `mode`, and base64-encoded output bytes.
Use the process exit code for pass/fail and parse the JSON for details.
