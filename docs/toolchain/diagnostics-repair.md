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

1. Run `x07 lint --input <program.x07.json> --world solve-pure`
2. Parse diagnostics
3. Apply quickfixes (if present) with `x07 fix --input <program.x07.json> --world solve-pure --write`,
   or apply an explicit patch with `x07 ast apply-patch`
4. Re-run `x07 lint`
5. Then run `x07 test`

This is how you turn LLM output into reliable engineering.
