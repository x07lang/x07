# Package policy (avoid duplicates)

Official packages under `packages/ext/` are part of the agent-facing surface area. The goal of this policy is to keep the ecosystem predictable for agents: one default choice per capability, and no parallel packages that export overlapping module namespaces.

## Canonical sources

- Registry catalog (authoritative “what exists in the public registry now”): https://registry.x07.io/index/catalog.json
- Capability map (authoritative “one happy path” recommendations): `catalog/capabilities.json`
- Official package manifests (authoritative metadata for official packages): `packages/ext/**/x07-package.json`

## Before adding a new package

1. Search the registry catalog for overlap (same domain, same exports, same intent).
2. If overlap exists:
   - Prefer extending the existing package, or
   - Add a pure helper module that builds on the existing canonical package (do not create a second competing OS integration).
3. Decide how it fits the “one happy path”:
   - Add/update `catalog/capabilities.json` so the canonical package choice is machine-readable.

## Required for new official packages

- `x07-package.json` must include `description` and `docs`.
- `meta.visibility` must be set to one of:
  - `canonical` (recommended default)
  - `advanced` (low-level building block or alternative)
  - `experimental` (not yet stable for the default path)
- If a package is marked `meta.visibility: "canonical"`, it must be referenced from `catalog/capabilities.json`.

## CI enforcement

Core CI checks enforce:

- no module_id collisions across different package names
- capability map references resolve to real packages
- canonical packages are discoverable from the capability map

