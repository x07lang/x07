# X07 WASM — Phase 6 (Ops + Capabilities + Policy + SLO + Deploy Plans + Provenance)

**Last updated:** 2026-02-27

## Goal

Make Phase 0–5 WASM artifacts safe to run and deploy autonomously by adding:

- operational contracts (ops profiles + deny-by-default capabilities)
- policy cards (assertions + optional RFC-6902 patch mutation)
- SLO-as-code + offline evaluation (promote/rollback/inconclusive)
- progressive delivery plan generation (Argo Rollouts-style YAML emission)
- hash-first pack provenance (SLSA-aligned attest/verify)

## Repos

- `x07-wasm-backend`: Phase 6 CLI + schemas + CI gate + examples + runtime enforcement
- `x07-web-ui`: browser host capability gating + policy snapshot metadata
- `x07-registry-web`: publishable schema set under `x07.io/spec/`
- `x07`: delegation (`x07 wasm ...`) + docs

## Phase 6 checklist

- [x] Ops registry + `x07-wasm ops validate`
- [x] Capabilities schema + `x07-wasm caps validate` + host enforcement for `fs/env/secrets/network/clocks/random` where applicable
- [x] Policy cards schema + `x07-wasm policy validate` + RFC-6902 patch support
- [x] SLO schema + metrics snapshot schema + `x07-wasm slo validate|eval` (pinned exit codes)
- [x] Deploy plan schema + `x07-wasm deploy plan` (JSON plan + YAML outputs)
- [x] Provenance schema + `x07-wasm provenance attest|verify` (digest recompute; tamper negative)
- [x] Phase 6 CI gate scripts: `scripts/ci/check_phase6.sh` + examples + diagnostic allowlists
- [x] Publish Phase 6 schemas to `x07.io/spec/` (repo: `x07-registry-web`)
- [ ] Update `x07/docs/toolchain/wasm.md` and sync `x07-website` docs bundle

## Local gate (target)

From `x07-wasm-backend/`:

```bash
set -euo pipefail
export PATH="${WASI_SDK_DIR}/bin:${PATH}"
bash scripts/ci/check_phase6.sh
```

