# Toolchain CLI

X07 ships multiple small CLIs with JSON-first contracts so both humans and agents can drive it deterministically.

## Key commands

### New project skeleton

- `x07 init`
  - Creates `x07.json` (with `os`/`sandbox` profiles), `x07.lock.json`, a minimal `src/` program, and a `tests/` harness (`tests/tests.json`).
- `x07 init --package`
  - Creates a publishable package repo: `x07-package.json`, a minimal `x07.json` + `x07.lock.json`, publishable `modules/` layout, and a CI-friendly `tests/tests.json`.
  - Not compatible with `--template` (templates are for app scaffolds).

### MCP kit tooling

- `x07 mcp [ARGS...]`
  - Delegates to `x07-mcp` on PATH.
  - Delegated commands exit with code `2` if `x07-mcp` is not installed/discoverable.
- Common subcommands (provided by `x07-mcp`):
  - `x07 mcp registry gen --in <x07.mcp.json> --out <server.json> [--mcpb <file>] [--schema <schema.json>]`
  - `x07 mcp publish --dry-run --server-json <server.json> --mcpb <file>`
  - `x07 mcp conformance --url <url> [--baseline <path>] [--spawn <server-id> --mode <noauth|oauth>]`
  - `x07 mcp bundle --mcpb --server-dir <servers/<id>> [--out <dist/...mcpb>]`
- `x07 init --template mcp-server|mcp-server-stdio|mcp-server-http|mcp-server-http-tasks`
  - Delegates template generation to `x07-mcp scaffold init ... --machine json`.
  - Creates the agent kit and a worker base policy at `.x07/policies/base/worker.sandbox.base.policy.json`.

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

`--input` may be repeated. Each `<path>` may be a file or a directory; directory inputs are scanned recursively for `*.x07.json`.

### Assets (embed files)

- `x07 assets embed-dir --in <dir> --module-id <module_id> --out <path>`
  - Generates an `.x07.json` module that embeds all matching files under `<dir>` as base64 payloads.
  - Useful for shipping templates or other static assets inside bundled OS-world CLIs (similar to Rust `include_bytes!`).

### Linting + diagnostics (`x07diag` JSON)

- `x07 lint --input <path>`
  - Prints an `x07diag` JSON report to stdout.
- `x07 fix --input <path> --write`
  - Applies quickfixes (JSON Patch) and rewrites the file deterministically.
- `x07 fix --from-pbt <repro.json> --write`
  - Converts a PBT repro artifact into a deterministic regression test (wrapper module + manifest entry).

See: [PBT repro → regression test](pbt-fix-from-repro.md).

### AST slicing (deterministic context views)

- `x07 ast slice --in <path> --ptr <json_pointer> [--enclosure decl|defn|module] [--closure locals|types|imports|all] [--max-nodes N] [--max-bytes BYTES]`
  - Emits a minimal, semantically-closed x07AST slice around `--ptr` (plus `slice_meta` describing omissions, remaps, and truncation).
  - Pointers refer to the canonical x07AST view (run `x07 fmt` or use pointers produced by toolchain diagnostics).
  - If the focus decl is re-indexed to `decls[0]`, `slice_meta.ptr_remap[]` records the pointer rewrite.
  - If bounds force truncation, `slice_meta.truncated=true` and diagnostic `X07-AST-SLICE-0001` is emitted.
  - With global `--out <path>`, writes the canonical `slice_ast` to `<path>` and omits it from the stdout report to avoid duplication.
  - Tool wrapper schema (`--json`): `spec/x07-tool-ast-slice.report.schema.json` (`schema_version: "x07.tool.ast.slice.report@0.1.0"`).

### Agent context packs

- `x07 agent context --diag <path> --project <path> [--enclosure ...] [--closure ...] [--max-nodes ...] [--max-bytes ...]`
  - Produces a deterministic, portable context artifact (`schema_version: "x07.agent.context@0.1.0"`) that embeds:
    - the diagnostics (focus = first `severity=error`, else first),
    - an AST slice of the project entry module at the focused diagnostic pointer,
    - input digests for traceability.
  - `--diag` accepts either raw `x07diag` (`x07.x07diag@0.1.0`) or a tool wrapper report (`x07.tool.*.report@0.1.0`); diagnostics are extracted deterministically.
  - With global `--out <path>`, writes the canonical context pack JSON to `<path>` and prints nothing to stdout.
  - Artifact schema: `spec/x07-agent.context.schema.json`.
  - Tool wrapper schema (`--json`): `spec/x07-tool-agent-context.report.schema.json` (`schema_version: "x07.tool.agent.context.report@0.1.0"`).

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
  - Emits a machine-readable trust summary (budgets/caps, declared+used capabilities, nondeterminism flags, deterministic SBOM artifacts).
  - Supports observed merges via `--run-report`, `--bundle-report`, `--x07test`.
  - SBOM output: `--sbom-format none|cyclonedx|spdx` (default: `cyclonedx`) writes `*.sbom.cdx.json` (or `*.sbom.spdx.json`) next to the trust report output.
  - Dependency capability policy: `--deps-cap-policy <path>` (safe relative path) + CI gate `--fail-on deps-capability`.
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
- `x07 test --pbt --manifest tests/tests.json`
  - Runs property-based tests only (entries where `pbt` is set).
- `x07 test --all --manifest tests/tests.json`
  - Runs both unit tests and property-based tests.
- `x07 test --pbt --pbt-repro <repro.json> --manifest tests/tests.json`
  - Replays exactly one counterexample artifact (single test + single case).

See: [Property-based testing](pbt.md).

### Verification (contracts)

- `x07 verify --bmc --entry <sym>`
- `x07 verify --smt --entry <sym>`
  - `--project <path>` (or one/more `--module-root <dir>`)
  - `--unwind <n>` (CBMC loop unwinding bound)
  - `--max-bytes-len <n>` (bound for `bytes` / `bytes_view` params)

Notes:

- v0.1 verifies only a selected subset: `defn` targets only (no `defasync`), no recursion, and `for` loops must have literal bounds.
- v0.1 supports params: `i32`, `u32`, `bytes`, `bytes_view` (use a wrapper if you need other types).
- `x07 verify` requires at least one contract clause (`requires` / `ensures` / `invariant`) on the target function.
- Artifacts are written under `.x07/artifacts/verify/<mode>/<entry>/` (driver module, emitted C, CBMC output, counterexample/SMT artifacts when present).

Report schema: `spec/x07-verify.report.schema.json` (`schema_version: "x07.verify.report@0.1.0"`).

### Agent correctness benchmarks (`x07bench` JSON)

- `x07 bench list --suite labs/x07bench/suites/core_v1/suite.json`
- `x07 bench validate --suite labs/x07bench/suites/core_v1/suite.json`
- `x07 bench eval --suite labs/x07bench/suites/core_v1/suite.json --predictions <predictions.jsonl>`
- `x07 bench eval --suite labs/x07bench/suites/core_v1/suite.json --oracle`
- `x07 bench eval --suite labs/x07bench/suites/core_v1/suite.json --oracle --runner docker`

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
  - Bundles a VM-backed sandbox bundle by default (requires a base policy via profile or `--policy`).
  - To emit a legacy policy-only bundle (weaker isolation), add: `--sandbox-backend os --i-accept-weaker-isolation`.

Bundle report schema: `spec/x07-bundle.report.schema.json` (`schema_version: "x07.bundle.report@0.2.0"`).

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
