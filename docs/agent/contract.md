# Agent contracts (canonical endpoints)

This page lists the canonical machine-readable endpoints that LLM agents can rely on.
If multiple sources disagree, prefer the canonical sources listed here.

## Agent portal (x07lang.org)

Single front door for agent data (schemas, skills, examples, stdlib index, package indexes):

- Entrypoints: https://x07lang.org/agent/latest/entrypoints.json
- Agent index: https://x07lang.org/agent/latest/index.json
- Manifest: https://x07lang.org/agent/latest/manifest.json
- Schemas index: https://x07lang.org/agent/latest/schemas/index.json
- Skills index: https://x07lang.org/agent/latest/skills/index.json
- Examples index: https://x07lang.org/agent/latest/examples/index.json
- Stdlib index: https://x07lang.org/agent/latest/stdlib/index.json
- Packages index: https://x07lang.org/agent/latest/packages/index.json

For canonical “one happy path” recommendations (templates/docs should derive from this):

- Catalog index: https://x07lang.org/agent/latest/catalog/index.json
- Capability map: https://x07lang.org/agent/latest/catalog/capabilities.json

## Registry catalog (x07.io)

Authoritative machine list of what exists in the public registry right now:

- https://registry.x07.io/index/catalog.json

## Toolchain install contracts (x07up)

Machine entrypoint used by `x07up`:

- Channels: https://x07lang.org/install/channels.json

## Local (offline) contracts

When you have the toolchain installed, prefer local validation + repair:

- `x07 guide` (language + stdlib reference)
- `x07 fmt`, `x07 lint`, `x07 fix` (canonical edit loop)
- `x07 --cli-specrows` (machine-readable CLI surface)

