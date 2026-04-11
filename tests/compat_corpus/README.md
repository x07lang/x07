# Compat Corpus (Milestone M0)

This directory defines the **compatibility corpus** used as a regression safety net for evolving
the X07 toolchain.

It is intentionally small at first, but designed to be easy to extend.

## What It Checks

For each corpus project listed in `corpus.json`, the CI harness:

- Seeds the project with **offline** dependency sources from `packages/ext/` (versioned directories).
- Runs `x07 pkg lock --offline` to ensure dependency hydration/locking works without network.
- Runs `x07 check --project x07.json` to lint + typecheck + codegen-check the project.
- Optionally runs `x07 run --project x07.json` for selected “agent workflow” projects.

Separately, it runs one or more **fixability** cases (also defined in `corpus.json`) that:

- Lint a known-broken `*.x07.json` that includes a deterministic quickfix.
- Run `x07 fix` in memory (no write) and lint the fixed output.
- Enforce a minimum “fixable rate” so quickfix coverage does not regress.

## Why It Uses `packages/ext/` Instead of Copying

The repo already contains multiple historical versions under `packages/ext/x07-*/<version>/`.
Those versioned directories are treated as “frozen snapshots” for compatibility testing, so the
corpus does not duplicate package sources.

