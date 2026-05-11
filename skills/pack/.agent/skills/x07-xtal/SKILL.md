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
- For deterministic multi-file edits across JSON specs, `src/**`, and manifests, use an `x07.patchset@0.1.0` and apply it with `x07 patch apply --in <patchset.json> --repo-root . --write`.
- `x07 patch apply` currently patches JSON documents only. Edit `*.examples.jsonl` streams directly, then run `x07 xtal tests gen-from-spec --write` so the example digests are captured.
- Run `x07 fmt` only on x07AST files. Run `x07 xtal spec fmt --input <spec.x07spec.json> --write` for XTAL spec files.
- Use `x07 xtal dev` proof flags (`--unwind`, `--max-bytes-len`, `--input-len-bytes`, `--z3-timeout-seconds`, `--z3-memory-mb`, `--proof-policy`) when measuring timeout warnings in the full inner loop.
- Patchsets use top-level `patches`, not `ops`:
  - `{"schema_version":"x07.patchset@0.1.0","patches":[{"path":"spec/foo.x07spec.json","patch":[{"op":"add","path":"/operations/-","value":{...}}]}]}`
- When a repair emits `target/xtal/repair/patchset.json`, apply it via:
  - `x07 patch apply --in target/xtal/repair/patchset.json --repo-root . --write`
- If a patch touches `spec/**`, do not apply unless explicitly allowed (`--allow-spec-change`) and approved by project policy.
- When adding `brand` or `result_brand` ids to public operation signatures, keep the implementation signature, spec signature, and `meta.brands_v1` validators in sync. Include new result brands too, even when the brand is only used on outputs.
- When composing helpers that take owned `bytes` inside loops, bind public byte params once with `bytes.view` and pass copies via `view.to_bytes(<view>)`. Passing the original `bytes` directly can move it and break later postconditions with `use after move`.
