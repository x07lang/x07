# X07 WASM — Phase 0

**Last updated:** 2026-02-24

## Goal

Ship `solve-pure` WASM artifacts using the existing freestanding C backend:

- `x07 build --freestanding` → C + header exporting `x07_solve_v2`
- C → `wasm32` reactor module via `clang` + `wasm-ld`
- Deterministic runner using Wasmtime (correct sret ABI, budgets, incident bundles)
- Machine-readable, schema-validated reports

## Repos

- `x07-wasm-backend`: Phase 0 CLI + schemas + examples + CI
- `x07`: optional `x07 wasm ...` delegation + docs
- `x07-registry-web`: publish Phase 0 schemas under `x07.io/spec/`
- `x07-web-ui`: commit spec-only WIT world `x07:web-ui@0.1.0`

## Phase 0 checklist

- [x] Define Phase 0 schemas + wasm profiles (draft 2020-12, pinned `schema_version`)
- [x] Implement `x07-wasm profile validate` (self-validating report output)
- [x] Implement `x07-wasm cli specrows check` (zero external validators)
- [x] Implement `x07-wasm doctor` (toolchain discovery report)
- [x] Implement `x07-wasm build` end-to-end (x07 → C → wasm → inspect → manifest + report)
- [x] Implement `x07-wasm run` end-to-end (Wasmtime + sret ABI + budgets + incidents)
- [x] Add pinned example `examples/solve_pure_echo/` (golden vectors + freestanding smoke)
- [x] Add `examples/json_patch/` and `examples/task_sched/`
- [x] Add CI gates (linux + macOS): validate → build → run → upload reports + incidents
- [x] Add optional `x07 wasm ...` delegation in `x07/`
- [x] Publish new Phase 0 schemas to `x07-registry-web/static/spec/`
- [x] Commit spec-only WIT world `x07:web-ui@0.1.0`

## Local smoke loop (target)

From `x07-wasm-backend/`:

```bash
set -euo pipefail
mkdir -p build/wasm dist .x07-wasm/incidents

cargo run -p x07-wasm -- profile validate --json --report-out build/wasm/profile.validate.json --quiet-json
cargo run -p x07-wasm -- cli specrows check --json --report-out build/wasm/cli.specrows.check.json --quiet-json

cargo run -p x07-wasm -- build --project examples/solve_pure_echo/x07.json --profile wasm_release \
  --out dist/echo.wasm --artifact-out dist/echo.wasm.manifest.json \
  --json --report-out build/wasm/build.echo.json --quiet-json

cargo run -p x07-wasm -- run --wasm dist/echo.wasm --input examples/solve_pure_echo/tests/in.bin \
  --json --report-out build/wasm/run.echo.json --quiet-json \
  --output-out dist/echo.out.bin
```
