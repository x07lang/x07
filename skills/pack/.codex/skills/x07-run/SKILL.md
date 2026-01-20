---
name: x07-run
description: Canonical execution front door for X07 programs (solve-* and run-os*), producing pass-through runner reports.
metadata:
  short-description: Run programs (canonical)
  version: 0.1.0
  kind: docs
---

# x07-run

Use this skill for normal program execution. `x07 run` dispatches to:

- `x07-host-runner` for deterministic `solve-*` worlds
- `x07-os-runner` for `run-os*` worlds

## Canonical commands

- Run the current project (auto-discovers `x07.json`):
  - `x07 run`

- Run a deterministic fixture world:
  - `x07 run --world solve-fs --fixtures fixtures`

- Run with real OS access (non-deterministic):
  - `x07 run --os`
  - (equivalently) `x07 run --world run-os`

- Run policy-enforced OS world (requires explicit policy):
  - `x07 run --world run-os-sandboxed --policy run-os-policy.json`

## Inputs

Default is empty input bytes. Provide input via:

- file: `x07 run --input input.bin`
- stdin: `cat input.bin | x07 run --stdin`
- base64: `x07 run --input-b64 <BASE64>`

## Output contract

- Default output is a pass-through runner report JSON object on stdout:
  - `x07-host-runner.report@...` for `solve-*`
  - `x07-os-runner.report@...` for `run-os*`
- Parse based on `schema_version`.

Optional wrapper (debuggable resolution envelope):

- `x07 run --report wrapped`
- Wrapper schema: `x07.run.report@0.1.0` (field `report` contains the raw runner report object).

