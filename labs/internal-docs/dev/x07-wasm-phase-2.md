# X07 WASM — Phase 2 (Web UI)

**Last updated:** 2026-02-26

## Goal

Run X07 UI reducers in the browser with deterministic replay and an incident-to-regression flow:

- `std-web-ui@0.1.1`: canonical `std.web_ui.*` packages
- `x07-wasm web-ui ...`: build/serve/test tooling with machine-readable reports
- deterministic trace fixtures and replay harness

## Repos

- `x07-wasm-backend`: Phase 2 CLI commands + schemas + CI gate (`scripts/ci/check_phase2.sh`)
- `x07-web-ui`: canonical `std-web-ui` package, browser host assets, WIT, and examples
- `x07-registry-web`: published Phase 2 schemas under `x07.io/spec/`
- `x07`: optional `x07 wasm ...` delegation + docs

## Phase 2 checklist

- [x] Create canonical `x07-web-ui` repo:
  - [x] `std-web-ui@0.1.1` package exporting `std.web_ui.*` modules
  - [x] canonical browser host (`host/index.html`, `host/app-host.mjs`)
  - [x] canonical WIT contract (`x07:web-ui@0.2.0`, world `web-ui-app`)
  - [x] examples (`web_ui_counter`, `web_ui_form`) with deterministic trace fixtures
- [x] Add Phase 2 schemas to `x07-wasm-backend/spec/schemas/`:
  - [x] `x07.web_ui.*@0.1.0` (dispatch/tree/patchset/frame/trace + profile)
  - [x] `x07.arch.web_ui.index@0.1.0`
  - [x] `x07.wasm.web_ui.*.report@0.1.0`
- [x] Add web-ui profile registry + defaults (`arch/web_ui/index.x07webui.json`)
- [x] Add wasm profiles for web-ui builds (`wasm_web_ui_debug`, `wasm_web_ui_release`)
- [x] Implement `x07-wasm web-ui` command set:
  - [x] `contracts validate`, `profile validate`
  - [x] `build` (core + component + `jco transpile`)
  - [x] `serve` (static server + strict wasm MIME smoke)
  - [x] `test` (trace replay + optional transpiled component replay)
  - [x] `regress-from-incident`
- [x] Vendor/sync canonical host assets and WIT from `x07-web-ui` into `x07-wasm-backend`
- [x] Add Phase 2 CI gate (`x07-wasm-backend/scripts/ci/check_phase2.sh`) + workflow job
- [x] Publish Phase 2 schemas to `x07.io/spec/` (repo: `x07-registry-web`)
- [x] Publish `std-web-ui@0.1.1` to `x07.io` registry

## Local smoke loop (target)

From `x07-wasm-backend/`:

```bash
set -euo pipefail

# Verifies contracts, vendored snapshot, builds both core+component dists, runs replay tests,
# and exercises incident → regression generation.
bash scripts/ci/check_phase2.sh
```

If you want a manual loop:

```bash
set -euo pipefail
mkdir -p build/wasm dist

x07-wasm web-ui contracts validate --json --report-out build/wasm/web-ui.contracts.validate.json --quiet-json
x07-wasm web-ui profile validate --json --report-out build/wasm/web-ui.profile.validate.json --quiet-json

web_ui_repo="../x07-web-ui"
if [[ ! -d "${web_ui_repo}/.git" ]]; then
  git clone https://github.com/x07lang/x07-web-ui.git "${web_ui_repo}"
fi

x07-wasm web-ui build --project "${web_ui_repo}/examples/web_ui_counter/x07.json" --profile web_ui_debug --out-dir dist/web_ui_counter_core --clean \
  --json --report-out build/wasm/web-ui.build.counter.core.json --quiet-json
x07-wasm web-ui test --dist-dir dist/web_ui_counter_core --case "${web_ui_repo}/examples/web_ui_counter/tests/counter.trace.json" \
  --json --report-out build/wasm/web-ui.test.counter.core.json --quiet-json
```
