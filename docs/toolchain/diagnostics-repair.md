# Diagnostics & repair (LLM-oriented)

X07 treats tooling output as a first-class API.

## Goals

- Diagnostics are **machine-readable**
- Error codes are **stable** across releases
- Suggested fixes are **JSON Patch** (apply-able deterministically)

## Diagnostic model (`x07diag`)

Diagnostics include:

- `code` (stable ID)
- `severity`
- `message` (human)
- `loc` (where in x07AST or text)
- optional `quickfix` (JSON Patch)

Agents should never scrape text; they should parse the structured payload.

## Repair loop

See [Repair loop](repair-loop.md) for the canonical command sequence (format → lint → quickfix/patch → re-lint → test).

## Output contracts (agent-friendly)

- Raw diagnostics output: `x07diag` (schema: `spec/x07diag.schema.json`)
- Tool wrapper reports (`--report-json`): `x07c.report@0.1.0` (schema: `spec/x07c.report.schema.json`)

Agents should treat these as APIs: parse JSON, key off `schema_version`, and avoid scraping text.
