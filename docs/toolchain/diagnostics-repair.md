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

A good agent loop:

1. Run `x07 lint --input <program.x07.json> --world solve-pure` (or add `--report-json` for a tool wrapper report)
2. Parse diagnostics
3. Apply quickfixes (if present) with `x07 fix --input <program.x07.json> --world solve-pure --write`,
   or apply an explicit patch with `x07 ast apply-patch --in <program.x07.json> --patch <patch.json> --out <program.x07.json> --validate`
4. Re-run `x07 lint`
5. Then run `x07 test`

This is how you turn LLM output into reliable engineering.

## Output contracts (agent-friendly)

- Raw diagnostics output: `x07diag` (schema: `spec/x07diag.schema.json`)
- Tool wrapper reports (`--report-json`): `x07c.report@0.1.0` (schema: `spec/x07c.report.schema.json`)

Agents should treat these as APIs: parse JSON, key off `schema_version`, and avoid scraping text.
