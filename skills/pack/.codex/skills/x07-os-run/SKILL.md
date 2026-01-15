---
name: x07-os-run
description: Run X07 programs in run-os / run-os-sandboxed worlds using x07-os-runner, producing machine-readable runner reports.
metadata:
  short-description: Run OS-world programs
  version: 0.1.0
  kind: docs
---

# x07-os-run

Use this skill when you need real OS I/O (fs/net/process/time) via `run-os` or policy-enforced execution via `run-os-sandboxed`.

## Canonical commands

- Run a program (unsandboxed):
  - `x07-os-runner --program src/main.x07.json --world run-os --module-root src`

- Run a project (unsandboxed):
  - `x07-os-runner --project x07.json --world run-os`

- Run sandboxed (requires an explicit policy):
  - `x07-os-runner --program src/main.x07.json --world run-os-sandboxed --policy run-os-policy.json --module-root src`

## Policy

Use a minimal allowlist policy and expand it deliberately.
Start from `references/run-os-policy.sample.json`.

## Output contract

`x07-os-runner` prints pretty JSON including `schema_version`, `mode`, `world`, and base64-encoded output bytes.
Use the process exit code for pass/fail and parse the JSON for details.
