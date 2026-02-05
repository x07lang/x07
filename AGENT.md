# Agent Guide (tool-agnostic)

## Canonical entry points

- Use `x07` as the facade for end-user workflows:
  - `x07 run` (compile+run; defaults to safe repair loops where applicable)
  - `x07 build` (build artifacts)
  - `x07 bundle` (ship a normal CLI executable)
  - `x07 test` (manifest-driven tests)
  - `x07 fmt` / `x07 lint` / `x07 fix` / `x07 ast apply-patch` (authoring + repair tools)
- CI gates:
  - fast: `./scripts/ci/check_canaries.sh`
  - full: `./scripts/ci/check_all.sh`

## Docs layout

- `docs/`: published end-user docs (bundled into releases and synced to x07lang.org)
  - external package contracts: `docs/{db,fs,math,net,os,text,time}/`
- `labs/internal-docs/`: toolchain/language development notes (not published)

## Repo layout

- `crates/`: Rust workspace crates (CLI + compiler + runners + shipped native extensions)
- `docs/`: end-user documentation (published)
- `labs/internal-docs/`: internal specs + design notes (not published)
- `docs/examples/`: public examples
- `ci/`: release-blocking fixtures and suites
- `skills/`: released agent skills pack (installed via `x07up`)
- `schemas/`, `spec/`: contracts
- `stdlib/`, `packages/`: shipped stdlib + packages
- `worlds/`: capability worlds (deterministic fixture worlds + OS worlds)
- `labs/` (optional): benchmarks, perf, fuzz, and eval tooling; never required for release CI

## Surface facts that matter when editing programs

- Canonical solver format: x07AST JSON (`*.x07.json`, `x07.x07ast@0.3.0`) with json-sexpr expressions (`["head", ...]`).
- Built-in stdlib version: `stdlib/std/0.1.1/`.
- Systems-only surface is world-gated: `unsafe`, raw pointers, and `extern "C"` are available only in `run-os*` worlds (not in `solve-*` worlds).

## Package publishing (registry)

- Credentials are stored in `~/.x07/credentials.json` under `tokens["sparse+https://registry.x07.io/index/"]`.
  - Prefer stdin to avoid leaking tokens into shell history:
    - `printf '%s' "$X07_TOKEN" | x07 pkg login --index sparse+https://registry.x07.io/index/ --token-stdin`
- Sync ext package pins + example lockfiles:
  - Check: `python3 scripts/publish_ext_packages.py sync`
  - Write: `python3 scripts/publish_ext_packages.py sync --write`
- Publish missing ext versions from `catalog/capabilities.json` (plus transitive `meta.requires_packages`):
  - Check: `python3 scripts/publish_ext_packages.py --check`
  - Publish: `python3 scripts/publish_ext_packages.py`
- Sparse index reads are cached (~5 minutes); prefer verifying publishes via the registry API (`GET /v1/packages/<name>`).
