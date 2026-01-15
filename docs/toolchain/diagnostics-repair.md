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

1. Run `x07c lint --input <program.x07.json> --world solve-pure`
2. Parse diagnostics
3. Apply quickfixes (if present), or apply a patch with `x07c apply-patch`
4. Re-run `x07c lint`
5. Then run `x07 test`

This is how you turn LLM output into reliable engineering.
