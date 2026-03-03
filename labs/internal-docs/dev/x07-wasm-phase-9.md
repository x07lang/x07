# X07 WASM — Phase 9 (Device host: desktop runner + device run/package)

**Last updated:** 2026-03-03

## Goal

Make Phase 8 device bundles runnable and packageable on desktop by landing:

- a single system WebView desktop host runner (`x07-device-host-desktop`)
- `x07-wasm device run` wiring (host delegation, report pass-through)
- `x07-wasm device package --target desktop` (self-contained desktop payload)

## Repos

- `x07-device-host`: desktop host runner + embedded host assets
- `x07-wasm-backend`: Phase 9 schemas + CLI + CI gates
- `x07-registry-web`: publishable schema set under `x07.io/spec/`
- `x07`: delegation (`x07 wasm ...`) + docs + release tags

## Phase 9 checklist

- [x] Desktop host runner (`x07-device-host-desktop`) mounts `ui/reducer.wasm` in a system WebView.
- [x] `x07-wasm device run` delegates to the host and validates/prints the same JSON report.
- [x] `x07-wasm device package --target desktop` emits `package.manifest.json` + payload (dir or deterministic zip).
- [x] Phase 9 CI gate scripts (`scripts/ci/check_phase9.sh`).
- [x] Host ABI drift is CI-gated even without the host runner (vendored snapshot sync + tampered bundle negative test).
- [x] Publish Phase 9 schemas to `x07.io/spec/` (`x07-registry-web`).
- [x] Update `x07/docs/toolchain/wasm.md` and sync `x07-website` docs bundle.

## Local gate (target)

From `x07-wasm-backend/`:

```bash
set -euo pipefail
bash scripts/ci/check_phase9.sh
```
