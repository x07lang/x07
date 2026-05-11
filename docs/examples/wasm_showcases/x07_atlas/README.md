# `x07 Atlas`

Full-stack showcase app for the `x07-wasm app` pipeline: offline-first project tracking, a real `/api` surface, deterministic trace replay, incident capture, regression generation, pack verification, provenance, deploy planning, and SLO evaluation.

- Prompt: [`PROMPT.md`](PROMPT.md)
- CI gate: `bash scripts/ci/check_showcase_fullstack.sh`
- Frontend reducer: [`frontend/src/app.x07.json`](frontend/src/app.x07.json)
- Backend entry: [`backend/src/app.x07.json`](backend/src/app.x07.json)

## What It Demonstrates

- Offline-first storage via `std.web_ui.effects.storage`
- Multi-page navigation via `std.web_ui.effects.nav`
- Stable `/api/projects` and `/api/projects/{id}/items` JSON flows
- Trace replay for happy-path, validation failure, budget failure, and generated regression coverage
- Pack, verify, signed provenance, deploy plan generation, and offline SLO evaluation

## Run With Released Tools

Install the released toolchain components once:

```sh
x07up component add wasm
```

Run the full example gate from this directory:

```sh
bash scripts/ci/check_showcase_fullstack.sh
```

Useful focused commands:

```sh
x07-wasm app build --profile atlas_dev --out-dir dist/showcase_fullstack/app.atlas_dev --clean --json
x07-wasm app serve --dir dist/showcase_fullstack/app.atlas_dev --mode listen --json
x07-wasm app test --dir dist/showcase_fullstack/app.atlas_dev --trace tests/traces/happy_path.trace.json --json
```

## Run From The Workspace

When you are working inside the multi-repo `x07lang` workspace, prefer the local repo-head binaries:

```sh
PATH="<workspace>/x07/target/debug:<workspace>/x07-wasm-backend/target/debug:$PATH" \
  bash scripts/ci/check_showcase_fullstack.sh
```

## Files To Start With

- App profiles: [`arch/app/profiles/atlas_dev.json`](arch/app/profiles/atlas_dev.json), [`arch/app/profiles/atlas_budget.json`](arch/app/profiles/atlas_budget.json), [`arch/app/profiles/atlas_release.json`](arch/app/profiles/atlas_release.json)
- Ops + provenance: [`arch/app/ops/ops_release.json`](arch/app/ops/ops_release.json), [`arch/provenance/dev.ed25519.public_key.b64`](arch/provenance/dev.ed25519.public_key.b64)
- Trace fixtures: [`tests/traces/happy_path.trace.json`](tests/traces/happy_path.trace.json), [`tests/traces/validation_error.trace.json`](tests/traces/validation_error.trace.json), [`tests/traces/perf_budget.trace.json`](tests/traces/perf_budget.trace.json)
- Generated regression target: [`tests/regress/atlas_incident.trace.json`](tests/regress/atlas_incident.trace.json)
