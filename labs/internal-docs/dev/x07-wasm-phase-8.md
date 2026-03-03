# X07 WASM — Phase 8 (Device bundles: pinned host ABI + web-ui reducer wasm)

**Last updated:** 2026-03-03

## Goal

Enable a single “system WebView host” execution model (desktop + mobile) for `std.web_ui` reducers by introducing:

- device profile registry + device profile schema (contracts-as-data)
- device bundle manifest pinned to a host ABI hash
- `x07-wasm device ...` CLI tooling (validate/build/verify)
- signed device provenance (`x07-wasm device provenance attest|verify`)

Phase 8 only defines the bundle and its verification rules; the actual desktop/mobile hosts are Phase 9+.

## Repos

- `x07-device-host`: pinned host assets + deterministic host ABI hash (used by device bundles)
- `x07-wasm-backend`: Phase 8 schemas + CLI + CI gate + fixtures
- `x07-registry-web`: publishable schema set under `x07.io/spec/`
- `x07`: delegation (`x07 wasm ...`) + docs

## Phase 8 checklist

- [x] Host assets + ABI hash crate (`x07-device-host`)
- [x] Device schemas + `x07-wasm device index|profile validate` (`x07-wasm-backend`)
- [x] Device bundle build + verify (`x07-wasm-backend`)
- [x] Device provenance attest/verify (`x07-wasm-backend`)
- [x] Phase 8 CI gate scripts (`scripts/ci/check_phase8.sh`)
- [x] Publish Phase 8 schemas to `x07.io/spec/` (`x07-registry-web`)
- [x] Update `x07/docs/toolchain/wasm.md` and sync `x07-website` docs bundle

## Local gate (target)

From `x07-wasm-backend/`:

```bash
set -euo pipefail
bash scripts/ci/check_phase8.sh
```
