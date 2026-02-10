# Roadmap apps (full projects)

This directory contains **full X07 application projects** derived from the Roadmap app prompts in [`docs/agent/apps.md`](../../agent/apps.md).

Each project is a complete directory you can copy into a fresh repo:

- `PROMPT.md`: the app prompt/spec
- `README.md`: how to run + what to expect
- `x07.json`, `x07.lock.json`, `src/`, `tests/` (and `arch/`, `policy/`, `ci/` when needed)

After copying an example, hydrate deps from the committed lockfile before running:

- `x07 pkg lock --project x07.json`

## Projects

- [`x07-api-gateway/README.md`](x07-api-gateway/README.md)
- [`x07crawl/README.md`](x07crawl/README.md)
- [`x07dbguard/README.md`](x07dbguard/README.md)
