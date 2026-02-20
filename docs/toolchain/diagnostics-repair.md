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
- `data` (optional structured payload; machine-first)
- optional `quickfix` (JSON Patch)

Agents should never scrape text; they should parse the structured payload.

## Ownership provenance (`mem_provenance`)

Some ownership/move/borrow diagnostics include `diagnostics[].data.mem_provenance`, a small structured graph that traces the owned value through moves/borrows and highlights the violating use.

- Schema: `spec/x07-mem.provenance-graph.schema.json` (`schema_version: "x07.mem.provenance_graph@0.1.0"`)

## Repair loop

`x07 run`, `x07 build`, and `x07 bundle` run the canonical repair loop automatically by default (format → lint → quickfix, repeatable).

For explicit control (or when you want the raw reports), use `x07 fmt` / `x07 lint` / `x07 fix` / `x07 ast apply-patch` and see [Repair loop](repair-loop.md).

## Whole-project check

- `x07 check --project x07.json`
  - Resolves the full import graph (including locked dependencies).
  - Runs schema validation + lint + project-wide typecheck + backend-check.
  - Non-mutating: does not run the repair loop and does not emit C or invoke any native compiler.

## Output contracts (agent-friendly)

- Raw diagnostics output: `x07diag` (schema: `spec/x07diag.schema.json`)
- Tool reports (`--json`): `spec/x07-tool-<scope>.report.schema.json` (base: `spec/x07-tool.report.schema.json`)
- Tool streaming events (`--jsonl`): `spec/x07-tool.events.schema.json` (`schema_version: "x07.tool.events@0.1.0"`)
- Deterministic AST slice wrapper (`x07 --json ast slice ...`): `spec/x07-tool-ast-slice.report.schema.json` (`schema_version: "x07.tool.ast.slice.report@0.1.0"`)
- Deterministic context pack artifact (`x07 agent context ...`): `spec/x07-agent.context.schema.json` (`schema_version: "x07.agent.context@0.1.0"`)
- Context pack wrapper (`x07 --json agent context ...`): `spec/x07-tool-agent-context.report.schema.json` (`schema_version: "x07.tool.agent.context.report@0.1.0"`)
- Diagnostics catalog (source of truth): `catalog/diagnostics.json` (schema: `spec/x07-diag.catalog.schema.json`)
- Diagnostics coverage report: `x07.diag.coverage@0.1.0` (schema: `spec/x07-diag.coverage.schema.json`)

Agents should treat these as APIs: parse JSON, key off `schema_version`, and avoid scraping text.

## Catalog workflows

- Validate and regenerate canonical artifacts:
  - `x07 diag catalog --catalog catalog/diagnostics.json --format both`
- Guard against source/catalog drift:
  - `x07 diag check`
- Measure quickfix coverage:
  - `x07 diag coverage --severity error,warning --min-coverage <threshold>`
  - CI gate target (roadmap): `x07 diag coverage --severity error,warning --min-coverage 0.90`
