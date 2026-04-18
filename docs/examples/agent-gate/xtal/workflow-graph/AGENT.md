# AGENT.md — workflow-graph

This example is intended for agent-driven XTAL work.

Canonical loop:

1. `x07 fmt --input <file> --write`
2. `x07 lint --input <file>`
3. `x07 fix --input <file> --write`
4. `x07 check --project x07.json`
5. `x07 xtal dev --project x07.json`

Prefer the XTAL wrappers over ad-hoc shell glue:

- `x07 xtal tests gen-from-spec --project x07.json --write`
- `x07 xtal impl check --project x07.json`
- `x07 xtal verify --project x07.json`
- `x07 xtal repair --project x07.json`

Do not edit `gen/xtal/**` directly. Regenerate it from `spec/**`.
