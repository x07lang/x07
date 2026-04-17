---
name: x07-xtal
description: Spec-first workflows and incident improvement via `x07 xtal ...`, producing deterministic artifacts under `target/xtal/`.
metadata:
  short-description: XTAL workflow (canonical)
  version: 0.1.0
  kind: docs
---

# x07-xtal

Use this skill when operating a project that has an XTAL manifest (`arch/xtal/xtal.json`) and specs under `spec/`.

XTAL’s contract is: **decide based on deterministic artifacts** (summaries, diagnostics, patchsets), not on log scraping.

## Canonical workflows

- Inner loop (spec checks + verify; optional bounded repair):
  - `x07 xtal dev`
  - With repair: `x07 xtal dev --repair-on-fail`
  - Prechecks only (fast): `x07 xtal dev --prechecks-only`

- Release certification (trust bundle):
  - `x07 xtal certify --all`
  - With review diff: `x07 xtal certify --all --baseline <cert_dir>`

- Incident intake (normalize + improvement loop by default):
  - `x07 xtal ingest --input <violation.json|repro.json|events.jsonl|dir>`
  - Normalize only: `x07 xtal ingest --input <...> --normalize-only`

Advanced building blocks (use when you need to isolate a step):

- `x07 xtal verify`
- `x07 xtal repair`
- `x07 xtal improve`
- `x07 xtal tasks run --input <...>`

## Rules for agents

- Never edit `gen/**` directly. Use `x07 xtal tests gen-from-spec --write` or update spec/examples and regenerate.
- Always read `target/xtal/**/summary.json` (and `target/xtal/xtal.*.diag.json`) to determine next actions.
- When a repair emits `target/xtal/repair/patchset.json`, apply it via:
  - `x07 patch apply --in target/xtal/repair/patchset.json --repo-root . --write`
- If a patch touches `spec/**`, do not apply unless explicitly allowed (`--allow-spec-change`) and approved by project policy.

