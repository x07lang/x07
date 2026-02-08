# Toolchain CLI

X07 ships multiple small CLIs with JSON-first contracts so both humans and agents can drive it deterministically.

## Key commands

### New project skeleton

- `x07 init`
  - Creates `x07.json` (with `os`/`sandbox` profiles), `x07.lock.json`, a minimal `src/` program, and a `tests/` harness (`tests/tests.json`).
- `x07 init --package`
  - Creates a publishable package repo: `x07-package.json`, a minimal `x07.json` + `x07.lock.json`, publishable `modules/` layout, and a CI-friendly `tests/tests.json`.
  - Not compatible with `--template` (templates are for app scaffolds).

### Doctor (platform prerequisites)

- `x07 doctor`
  - Checks host prerequisites for OS worlds (C compiler + common native deps).
  - Emits a JSON report to stdout.

### Guide (built-in language + stdlib reference)

- `x07 guide`
  - Prints the built-in language + stdlib reference guide (Markdown).

### Formatting (x07AST JSON)

- `x07 fmt --input <path> --check`
- `x07 fmt --input <path> --write`

### Linting + diagnostics (`x07diag` JSON)

- `x07 lint --input <path>`
  - Prints an `x07diag` JSON report to stdout.
- `x07 fix --input <path> --write`
  - Applies quickfixes (JSON Patch) and rewrites the file deterministically.

### Architecture check (repo contracts)

- `x07 arch check`
  - Verifies the repo’s `*.x07.json` import graph against `arch/manifest.x07arch.json`.
  - Emits an `x07.arch.report@0.1.0` report and optional JSON Patch suggestions.

See: [Architecture check](arch-check.md).

### Review artifacts (semantic diff)

- `x07 review diff --from <path> --to <path> --html-out <path>`
- `x07 review diff --from <path> --to <path> --html-out <path> --json-out <path>`
  - Produces an intent-level semantic diff for x07AST/project/arch/policy changes.
  - Supports CI gates via `--fail-on world-capability|budget-increase|allow-unsafe|allow-ffi`.
  - JSON schema: `spec/x07-review.diff.schema.json` (`schema_version: "x07.review.diff@0.1.0"`).

See: [Review & trust artifacts](review-trust.md).

### Trust artifacts (CI security posture summary)

- `x07 trust report --project x07.json --out <path>`
- `x07 trust report --project x07.json --profile sandbox --out <path> --html-out <path>`
  - Emits a machine-readable trust summary (budgets/caps, declared+used capabilities, nondeterminism flags, SBOM placeholders).
  - Supports observed merges via `--run-report`, `--bundle-report`, `--x07test`.
  - Supports CI gates via `--strict` and `--fail-on ...`.
  - JSON schema: `spec/x07-trust.report.schema.json` (`schema_version: "x07.trust.report@0.1.0"`).

See: [Review & trust artifacts](review-trust.md).

### Diagnostics catalog / coverage

- `x07 diag catalog --catalog catalog/diagnostics.json --format both`
  - Validates `catalog/diagnostics.json` against `x07.diag.catalog@0.1.0`.
  - Emits canonical JSON and generated docs (default: `docs/toolchain/diagnostic-codes.md`).
- `x07 diag check`
  - Scans source diagnostic codes and fails on catalog drift.
  - Writes extracted scan data to `target/x07diag/extracted_codes.json`.
- `x07 diag coverage`
  - Emits `x07.diag.coverage@0.1.0` quickfix coverage report JSON.
  - Supports threshold gating with `--min-coverage`.
- `x07 diag explain <CODE>`
  - Prints summary, origins, quickfix policy, and agent strategy for one code.
- `x07 diag sarif --in <x07diag.json> --out <results.sarif>`
  - Converts `x07diag` to SARIF v2.1.0 for code-scanning UIs.

### Patching (RFC 6902 JSON Patch)

- `x07 ast apply-patch --in <path> --patch <patch.json> --out <path> --validate`
- `x07 patch apply --in <patchset.json> --repo-root . --write`
  - Applies a multi-file patchset (`schema_version: "x07.patchset@0.1.0"`).
  - Without `--write`, validates and reports only (dry run).

### x07AST schema + grammar generation pack

- `x07 ast schema`
  - Emits the canonical `x07ast.schema.json` document on stdout (raw JSON bytes + trailing newline).
- `x07 ast schema --out <path>`
  - Writes the schema document bytes to file.
- `x07 ast schema --pretty --out <path>`
  - Writes a pretty-printed schema document to file.
- `x07 ast grammar --cfg`
  - Emits a machine-readable grammar bundle JSON:
    - `schema_version: "x07.ast.grammar_bundle@0.1.0"`
    - `variants[]` (`min`, `pretty`) with GBNF content
    - `semantic_supplement`
    - per-artifact sha256 hashes
- `x07 ast grammar --cfg --out-dir <dir>`
  - Materializes:
    - `x07ast.schema.json`
    - `x07ast.min.gbnf`
    - `x07ast.pretty.gbnf`
    - `x07ast.semantic.json`
    - `manifest.json`

Cookbook integrations:

- [XGrammar recipe](../genpack/xgrammar.md)
- [Outlines recipe](../genpack/outlines.md)

### Testing (`x07test` JSON)

- `x07 test --manifest tests/tests.json`
  - Runs each test in its declared world.
  - Prints an `x07test` JSON report (or writes it with `--report-out`).

### Agent correctness benchmarks (`x07bench` JSON)

- `x07 bench list --suite labs/x07bench/suites/core_v0/suite.json`
- `x07 bench validate --suite labs/x07bench/suites/core_v0/suite.json`
- `x07 bench eval --suite labs/x07bench/suites/core_v0/suite.json --predictions <predictions.jsonl>`
- `x07 bench eval --suite labs/x07bench/suites/core_v0/suite.json --oracle`
- `x07 bench eval --suite labs/x07bench/suites/core_v0/suite.json --oracle --runner docker`

Reports use `schema_version: "x07.bench.report@0.1.0"`.

See: [Benchmarks](benchmarks.md).

### Doc (module exports)

- `x07 doc <module-id>`
- `x07 doc <module-id>.<exported_symbol>`
  - Prints exported symbol signatures from a module file (useful for agents exploring unfamiliar modules).
- `x07 doc --json <query>`
  - Emits a stable machine-readable report (`schema_version: "x07.doc.report@0.1.0"`).
  - Exit codes:
    - `0` on `ok=true`
    - `1` on `ok=false` (for example: not found or query errors)
    - `2` on tool failures (for example: filesystem read/write errors)
- `x07 doc --json <query> --report-out <path> --quiet-json`
  - Writes the JSON report to a file and suppresses stdout.
- `x07 doc --json spec:<path-or-id>`
  - Resolves published spec docs and schemas via `docs/spec/` / `.agent/docs/spec/`.

Doc report schema: `docs/spec/schemas/x07-doc.report.schema.json`.

### Schema derive (generate modules + tests)

- `x07 schema derive --input <schema.x07schema.json> --out-dir <dir> --write`
- `x07 schema derive --input <schema.x07schema.json> --out-dir <dir> --check`
- `x07 schema derive --json ...`

See: [Schema derive](schema-derive.md).

### State machines (generate step functions + tests)

- `x07 sm check --input <sm.json>`
- `x07 sm gen --input <sm.json> --out <dir> --write`
- `x07 sm gen --input <sm.json> --out <dir> --check`

See: [State machines](state-machines.md).

### Packages (pack/lock/publish)

- `x07 pkg add <name>`
- `x07 pkg add <name> --sync`
- `x07 pkg add <name>@<version>`
- `x07 pkg add <name>@<version> --sync`
- `x07 pkg remove <name>`
- `x07 pkg remove <name> --sync`
- `x07 pkg versions <name>`
- `x07 pkg lock --project x07.json`
- `x07 pkg provides <module-id>`
- `x07 pkg pack --package <dir> --out <path>`
- `x07 pkg login --index <registry_url>`
- `x07 pkg publish --package <dir> --index <registry_url>`

Notes:

- `x07 pkg add <name>@<version>` edits `x07.json` only (no network) unless you pass `--sync`.
- `x07 pkg add <name>` consults the index to resolve a version (network unless you use a file-based index).
- `x07 pkg lock` uses the official registry index by default when fetching is required; override with `--index` or use `--offline`.
- Use `x07 pkg lock --check` in CI to fail if `x07.lock.json` is out of date.
- Some packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` can add them to `x07.json` before locking; do not rely on this for correctness (prefer the capability map and templates, which list the full canonical set explicitly).

### Build to C (project)

- `x07 build --project x07.json --out build/program.c`
  - Reads `x07.json` + `x07.lock.json`.
  - Uses `x07.json` `module_roots` plus locked dependency module roots (from the lockfile).
- `x07 build --project x07.json --out build/program.c --emit-c-header build/x07.h --freestanding`
  - Emits an embeddable `x07_solve_v2` entry point.

See: [Embedding in C](embedding-in-c.md).

### Bundle a native executable (distribution)

- `x07 bundle --project x07.json --profile os --out dist/mytool`
  - Produces a native CLI executable (standard `argc/argv`, raw stdout).
  - Runs without the X07 toolchain installed at runtime.
- `x07 bundle --project x07.json --profile sandbox --out dist/mytool`
  - Bundles a policy-enforced OS-world executable (requires a base policy via profile or `--policy`).

Bundle report schema: `spec/x07-bundle.report.schema.json` (`schema_version: "x07.bundle.report@0.1.0"`).

### Running programs (canonical)

Use `x07 run` as the canonical entry point for execution. Prefer intent-driven profiles (`x07.json.default_profile` + `x07.json.profiles`) so most invocations look like:

- `x07 run`
- `x07 run --profile os`
- `x07 run --profile sandbox`

`x07 run` runs the canonical auto-repair loop by default (format → lint → quickfix, repeatable). Control it with:

- `x07 run --repair=off`
- `x07 run --repair=memory`
- `x07 run --repair=write` (default)

For the complete guide (targets, worlds, input, policies, reports), see [Running programs](running-programs.md).

## JSON outputs (agent-friendly)

All command scopes support machine schema discovery:

- `x07 <scope> --json-schema`
- `x07 <scope> --json-schema-id`

Machine report output:

- `x07 <scope> --json`
- `x07 <scope> --json=pretty`
- `x07 <scope> --jsonl`
- `x07 <scope> --json --report-out <path>`
- `x07 <scope> --json --report-out <path> --quiet-json`
- `x07 <scope> --out <path>` (redirect primary output, when applicable)

Schemas:

- Tool report base schema: `spec/x07-tool.report.schema.json` (`schema_version: "x07.tool.report@0.1.0"`)
- Tool report per-scope schemas:
  - Root: `spec/x07-tool-root.report.schema.json`
  - Wrapped scopes: `spec/x07-tool-<scope>.report.schema.json` (`schema_version: "x07.tool.<scope>.report@0.1.0"`)
  - `x07 doc` (native): `spec/x07-doc.report.schema.json` (`schema_version: "x07.doc.report@0.1.0"`)
- JSONL events schema: `spec/x07-tool.events.schema.json` (`schema_version: "x07.tool.events@0.1.0"`)

Structured commands must guarantee:

- valid JSON
- schema-valid output
- stable ordering (canonical JSON writer)

Review/trust artifact commands also follow this contract:

- `x07 review diff`
- `x07 trust report`

For machine-first discovery and debugging:

- `x07 --cli-specrows` emits a deterministic CLI surface description.
- `--report-json` is accepted as a hidden alias for `--json` during migration.

See [Diagnostics & repair](diagnostics-repair.md).

## Programmatic CLI discovery (`--cli-specrows`)

Agents should not scrape `--help` text. Use `--cli-specrows`:

- `x07 --cli-specrows`
- `x07c --cli-specrows`
- `x07-os-runner --cli-specrows`

Output is a single JSON object:

- `schema_version: "x07cli.specrows@0.1.0"`
- `rows: [...]` (a flat, ordered table describing the CLI surface)

Each row is a small tuple. Examples:

- `["root","help","-h","--help","Print help"]`
- `["root","version","","--version","Print version"]`

Schema: `spec/x07cli.specrows.schema.json`.

## Legacy alias (`--report-json`)

Compatibility mode is still available for existing automation:

- `x07 <scope> --report-json` is equivalent to `x07 <scope> --json` (same report schema + output shape).

Notes:

- `x07 lint` without `--json` prints the raw diagnostics report (`x07diag`, see `spec/x07diag.schema.json`).
- `x07 fix` without `--json` prints the fixed x07AST JSON to stdout unless `--write` is set.
- `x07 fix --suggest-generics` emits a suggested `x07.patchset@0.1.0` to stdout (or `--out <PATH>` in `--json` workflows).

## Agent bootstrap recipe

Canonical minimal sequence (keep the loop simple; prefer `x07 run`):

1. Discover CLIs: `x07 --cli-specrows`
2. Iterate: `x07 run` (auto-repair by default; use `--repair=off` when debugging)
3. Validate: `x07 test`
4. Debug/repair explicitly (when needed): `x07 fmt` / `x07 lint` / `x07 fix` / `x07 ast apply-patch`

See: [Agent quickstart](../getting-started/agent-quickstart.md).
