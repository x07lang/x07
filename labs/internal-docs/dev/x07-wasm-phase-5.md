# X07 WASM — Phase 5 (Track-1 Hardening)

**Last updated:** 2026-02-27

## Goal

Ship a CI-gateable hardening layer after Phase 4:

- toolchain pin validation (versions as data)
- deterministic host runtime limits (fuel/memory/table/wasm stack)
- deployable app pack artifacts (content-addressed + verifiable headers policy)
- (recommended) core-wasm HTTP reducer loop with deterministic trace replay and incident→regression flow

## Repos

- `x07-wasm-backend`: Phase 5 CLI + schemas + CI gate + examples
- `x07-web-ui`: `std-web-ui@0.1.3` (effects: storage/nav/timer) + browser host
- `x07-registry-web`: publishable schema set under `x07.io/spec/`
- `x07`: delegation (`x07 wasm ...`) + docs

## Phase 5 checklist

- [x] Toolchain lock registry + `x07-wasm toolchain validate`
- [x] Runtime limits schema (`x07.wasm.runtime.limits@0.1.0`) + enforce in `run|serve|component run`
- [x] Web UI: require `dist/wasm.profile.json` for replay/test and type `frame.effects` via `x07.web_ui.effect@0.1.0`
- [x] App deploy artifacts: `app pack` + `app verify` with digest recompute + required headers policy
- [x] Core-wasm HTTP reducer: contracts validate + runner (`http serve|test|regress from-incident`)
- [x] Add Phase 5 CI gate (`x07-wasm-backend/scripts/ci/check_phase5.sh`) + examples
- [ ] Publish updated schemas to `x07.io/spec/` (repo: `x07-registry-web`)
- [ ] Publish `std-web-ui@0.1.3` to the `x07.io` registry (repo: `x07-web-ui`)

## Local gate (target)

From `x07-wasm-backend/`:

```bash
set -euo pipefail
export PATH="${WASI_SDK_DIR}/bin:${PATH}"
bash scripts/ci/check_phase5.sh
```

