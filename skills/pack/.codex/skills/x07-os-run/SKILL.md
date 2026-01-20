---
name: x07-os-run
description: Expert backend for run-os / run-os-sandboxed execution via x07-os-runner. Prefer `x07 run --os` as the canonical agent entry point.
metadata:
  short-description: Run OS-world programs
  version: 0.1.0
  kind: docs
---

# x07-os-run

Prefer `x07 run --os` (or `x07 run --world run-os*`) for normal execution. Use `x07-os-runner` directly only when you need runner-specific flags (policy debugging, auto-FFI toggles, explicit compiled artifact paths) or when you are debugging runner behavior.

Use this skill when you need real OS I/O (fs/net/process/time) via `run-os` or policy-enforced execution via `run-os-sandboxed`.

## Canonical commands (recommended: `x07 run`)

- Run the current project (unsandboxed):
  - `x07 run --os`
  - (equivalently) `x07 run --world run-os`

- Run a project explicitly (unsandboxed):
  - `x07 run --project x07.json --world run-os`

- Run sandboxed (requires an explicit policy):
  - `x07 run --project x07.json --world run-os-sandboxed --policy run-os-policy.json`

- Run a single program (when not using a project manifest):
  - `x07 run --program src/main.x07.json --world run-os --module-root src`

## Expert backend commands (`x07-os-runner`)

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

- `x07 run` in `run-os*` worlds prints an `x07-os-runner.report@...` JSON report to stdout (pass-through).
- `x07-os-runner` prints the same report shape when invoked directly.

In both cases:
- Use the process exit code for pass/fail.
- Parse the JSON for `schema_version`, `mode`, `world`, and base64-encoded output bytes.
