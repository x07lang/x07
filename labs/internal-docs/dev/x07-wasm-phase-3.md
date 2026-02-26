# X07 WASM — Phase 3 (App Bundle)

**Last updated:** 2026-02-26

## Goal

Ship a single, CI-gateable, agent-friendly full-stack loop that produces and consumes only machine-readable artifacts:

> **app profile → app build → app serve → app test (trace) → incident bundle → regression generation**

Phase 3 ties together:

- Phase 2 web-ui reducers (`x07.web_ui.*`)
- Phase 1 `wasi:http/proxy` backend components

## Repos

- `x07-wasm-backend`: Phase 3 CLI (`x07-wasm app ...`), schemas, `arch/app/*`, example app, CI gate
- `x07-web-ui`: canonical `std-web-ui` package + browser host (incl. HTTP effects)
- `x07-registry-web`: publishable schema set under `x07.io/spec/`
- `x07`: optional `x07 wasm ...` delegation + docs

## Phase 3 checklist

- [x] Add Phase 3 schemas to `x07-wasm-backend/spec/schemas/` (app profile/bundle/trace + HTTP envelopes + reports)
- [x] Add `arch/app/*` registry + pinned profiles
- [x] Implement `x07-wasm app contracts validate` + `app profile validate`
- [x] Implement `x07-wasm app build|serve|test|regress from-incident`
- [x] Add example `examples/app_fullstack_hello/` and CI gate (`x07-wasm-backend/scripts/ci/check_phase3.sh`)
- [x] Migrate Phase 1 HTTP envelopes to `x07.http.{request,response}.envelope@0.1.0` end-to-end
- [ ] Publish Phase 3 schemas to `x07.io/spec/` (repo: `x07-registry-web`)
- [ ] Publish `std-web-ui@0.1.2` to `x07.io` registry (repo: `x07-web-ui`)

## Local smoke loop (target)

From `x07-wasm-backend/`:

```bash
set -euo pipefail
bash scripts/ci/check_phase3.sh
```

