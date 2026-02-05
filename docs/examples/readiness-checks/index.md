# Readiness check example implementations

This directory contains **reference implementations** for each readiness prompt in
[`docs/agent/readiness-checks.md`](../../agent/readiness-checks.md).

Each example is a complete X07 project directory you can copy into a fresh repo (or use in place):

- `PROMPT.md`: the exact readiness prompt text
- `README.md`: how to run + what to expect
- `x07.json`, `x07.lock.json`, `src/`, `tests/` (and `policy/`, `arch/`, `gen/` when needed)

After copying an example, hydrate deps from the committed lockfile before running:

- `x07 pkg lock --project x07.json`

## Projects

- Prompt 1: [`x07-core-conformance/README.md`](x07-core-conformance/README.md)
- Prompt 2: [`x07-schema-types-pipes-lab/README.md`](x07-schema-types-pipes-lab/README.md)
- Prompt 3: [`x07-data-interop-lab/README.md`](x07-data-interop-lab/README.md)
- Prompt 4: [`x07-artifact-integrity-pipeline/README.md`](x07-artifact-integrity-pipeline/README.md)
- Prompt 5: [`x07-sandbox-web-stack/README.md`](x07-sandbox-web-stack/README.md)
- Prompt 6: [`x07-db-fs-indexer/README.md`](x07-db-fs-indexer/README.md)
- Prompt 7: [`x07-rr-pipes-smoke/README.md`](x07-rr-pipes-smoke/README.md)
- Prompt 8: [`x07-sm-arch-contracts-smoke/README.md`](x07-sm-arch-contracts-smoke/README.md)
- Prompt 9: [`x07-messaging-rr-lab/README.md`](x07-messaging-rr-lab/README.md)
