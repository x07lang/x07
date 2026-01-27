# Agent gate examples

These example projects are used by the CI gate to exercise the agent-first workflow:

- `x07 fmt` / `x07 lint` / `x07 fix` / `x07 test`
- `x07 run` with profiles and lockfile module roots
- `x07 run -- <args...>` argv passthrough (`argv_v1`)
- `run-os-sandboxed` policy templates + `--allow-host` derived policies

They are intentionally small and deterministic.

## Lanes

- Canonical (agents): `x07 run`, `x07 test`, `x07 lint`, `x07 fix`
- Expert (debug-only): direct runner binaries (`x07-host-runner`, `x07-os-runner`)

## Projects

- `cli-newline`: newline input payload parsing (has `tests/`)
- `cli-ext-cli`: `ext-cli` parsing + lockfile deps (has `tests/`)
- `web-crawler-local`: `run-os-sandboxed` HTTP crawl against a local fixture site (policy-gated)

## Run the gate locally

- `python3 scripts/ci/check_repair_corpus.py`
- `./scripts/ci/check_agent_examples.sh`
