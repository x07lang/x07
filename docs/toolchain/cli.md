# Toolchain CLI

X07 ships multiple small CLIs with JSON-first contracts so both humans and agents can drive it deterministically.

## Key commands

### New project skeleton

- `x07 init`
  - Creates `x07.json` (with `os`/`sandbox` profiles), `x07.lock.json`, a minimal `src/` program, and a `tests/` harness (`tests/tests.json`).
- `x07 init --package`
  - Creates a publishable package repo: `x07-package.json`, a minimal `x07.json` + `x07.lock.json`, publishable `modules/` layout, and a CI-friendly `tests/tests.json`.
  - Not compatible with `--template` (templates are for app scaffolds).
- `x07 init --template verified-core-pure`
  - Creates a certifiable `solve-pure` project with `arch/manifest.x07arch.json`, `arch/boundaries/index.x07boundary.json`, `arch/trust/profiles/verified_core_pure_v1.json`, `.github/workflows/certify.yml`, and a smoke + PBT harness wired for `x07 trust certify`.
- `x07 init --template trusted-sandbox-program`
  - Creates a certifiable `run-os-sandboxed` async project with a certified capsule boundary, capsule attestations, sandbox smoke tests, and a self-hosted VM certification workflow wired for `x07 trust certify`.
- `x07 init --template trusted-network-service`
  - Creates a certifiable `run-os-sandboxed` network service project under `trusted_program_sandboxed_net_v1`, with pinned peer-policy files, attested network capsule metadata, a loopback TCP smoke harness, and a self-hosted VM certification workflow.
- `x07 init --template certified-capsule`
  - Creates a minimal certified-capsule project with capsule contract/effect-log/attestation surfaces plus a self-hosted VM certification workflow.
- `x07 init --template certified-network-capsule`
  - Creates a standalone network capsule project under `trusted_program_sandboxed_net_v1`, with peer-policy evidence, effect-log attestations, a loopback TCP smoke harness, and a self-hosted VM certification workflow.
- `x07 init --template api-cell|event-consumer|scheduled-job|policy-service|workflow-service`
  - Copies the matching service scaffold from `docs/examples/service_*_v1/`, including `arch/service/index.x07service.json`, sandbox policy defaults, and a starter test harness.
  - `workflow-service` is scaffold-only for now; use it to define the authoring shape and bindings before workload packaging is finalized.

### Service tooling

- `x07 service archetypes`
  - Emits the built-in service archetype catalog, including default capabilities and package defaults pinned to the current published package lines.
- `x07 service genpack schema --archetype <id>`
  - Emits an archetype-specific JSON Schema for constrained service generation.
- `x07 service genpack grammar --archetype <id>`
  - Emits an archetype-specific CFG-style grammar for constrained service generation.
- `x07 service validate --manifest <path>`
  - Validates `arch/service/index.x07service.json` and checks it against a built-in archetype when one matches the service shape.

### MCP kit tooling

- `x07 mcp [ARGS...]`
  - Delegates to `x07-mcp` on PATH.
  - Delegated commands exit with code `2` if `x07-mcp` is not installed/discoverable.
- Common subcommands (provided by `x07-mcp`):
  - `x07 mcp inspect initialize|tools|tool-call|prompts|prompt-get|resources|resource-read|tasks|task-poll ...`
  - `x07 mcp catalog templates|reference-servers|conformance-suites|publish-capabilities`
  - `x07 mcp registry gen --in <x07.mcp.json> --out <server.json> [--mcpb <file>] [--schema <schema.json>]`
  - `x07 mcp publish --dry-run --server-json <server.json> --mcpb <file>`
  - `x07 mcp conformance --url <url> [--baseline <path>] [--spawn <server-id> --mode <noauth|oauth>]`
  - `x07 mcp bundle --mcpb --server-dir <servers/<id>> [--out <dist/...mcpb>]`
  - `x07 mcp trust summary --server-dir <servers/<id>> [--x07-root <repo>]`
  - `x07 mcp trust tlog-monitor --server-dir <servers/<id>> [--x07-root <repo>]`
- `x07 init --template mcp-server|mcp-server-stdio|mcp-server-http|mcp-server-http-tasks`
  - Delegates template generation to `x07-mcp scaffold init ... --machine json`.
  - Creates the agent kit and a worker base policy at `.x07/policies/base/worker.sandbox.base.policy.json`.
  - Prefer `mcp-server-http-tasks` for task-aware HTTP/SSE servers that expose long-running tool calls, progress, or `tasks/*` APIs.
  - Official lifecycle control still flows through the official `io.x07/x07lang-mcp` server via capability-gated tools such as `lp.query_v1` and `lp.control_v1`; `x07` does not duplicate platform runtime actions here.
  - Forge-style clients should treat the public `lp.control.action.result@0.1.0`, `lp.deploy.query.result@0.1.0`, `lp.environment.list.result@0.1.0`, `lp.incident.query.result@0.2.0`, and `lp.regression.run.result@0.2.0` outputs as the stable lifecycle read surface.

### WASM tooling

- `x07 wasm [ARGS...]`
  - Delegates to `x07-wasm` on PATH.
  - Delegated commands exit with code `2` if `x07-wasm` is not installed/discoverable.
- Common subcommands (provided by `x07-wasm`):
  - `x07 wasm doctor`
  - `x07 wasm build`
  - `x07 wasm run`
  - `x07 wasm profile validate`
  - `x07 wasm cli specrows check`
  - `x07 wasm wit validate`
  - `x07 wasm component profile validate`
  - `x07 wasm component build`
  - `x07 wasm component compose`
  - `x07 wasm component targets`
  - `x07 wasm serve`
  - `x07 wasm component run`
  - `x07 wasm web-ui contracts validate`
  - `x07 wasm web-ui profile validate`
  - `x07 wasm web-ui build`
  - `x07 wasm web-ui serve`
  - `x07 wasm web-ui test`
  - `x07 wasm web-ui regress-from-incident`
  - `x07 wasm device index validate`
  - `x07 wasm device profile validate`
  - `x07 wasm device build`
  - `x07 wasm device verify`
  - `x07 wasm device run`
  - `x07 wasm device package`
  - `x07 wasm app contracts validate`
  - `x07 wasm app profile validate`
  - `x07 wasm app build`
  - `x07 wasm app serve`
  - `x07 wasm app test`
  - `x07 wasm app pack`
  - `x07 wasm app verify`
  - `x07 wasm app regress from-incident`
  - `x07 wasm deploy plan`
  - `x07 wasm slo eval`

For platform-facing remote deploy workflows, the supported wasm contract surface is:

- `x07 wasm app pack`
- `x07 wasm app verify`
- `x07 wasm deploy plan`
- `x07 wasm slo eval`
- `x07 wasm app regress from-incident`

See: [WASM](wasm.md).

### Doctor (platform prerequisites)

- `x07 doctor`
  - Checks host prerequisites for OS worlds (C compiler + common native deps).
  - Emits a JSON report to stdout.

### Guide (built-in language + stdlib reference)

- `x07 guide`
  - Prints the built-in language + stdlib reference guide (Markdown).

### Formatting (x07AST JSON)

- `x07 fmt --check <path>...`
- `x07 fmt --write <path>...`
- `x07 fmt --input <path> --check`
- `x07 fmt --input <path> --write`

Add `--pretty` to produce deterministic multi-line formatting intended for human editing/review (default: canonical one-line JSON).

`--input` may be repeated. Positional `<path>` inputs may also be repeated. Each `<path>` may be a file or a directory; directory inputs are scanned recursively for `*.x07.json`.

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
- `x07 migrate --to 0.5 --check --input <path>`
  - Runs migration lints and exits non-zero when mechanical rewrites are required.
- `x07 migrate --to 0.5 --write --input <path>`
  - Applies mechanical migrations in-place via deterministic quickfix patches.

See: [PBT repro → regression test](pbt-fix-from-repro.md).

### Project manifest migrations (`x07.json`)

- `x07 project migrate --check --project x07.json`
  - Exits non-zero when `x07.json` is on a legacy schema line.
- `x07 project migrate --write --project x07.json`
  - Rewrites `schema_version` to the current manifest line (`x07.project@0.5.0`) and inserts `compat: "0.5"` when upgrading from a legacy schema line.

Notes:

- `x07 project migrate` is for **`x07.json` schema** migrations.
- `x07 migrate` is for **`*.x07.json` code** migrations (compat updates).

### AST slicing (deterministic context views)

- `x07 ast slice --in <path> --ptr <json_pointer> [--enclosure decl|defn|module] [--closure locals|types|imports|all] [--max-nodes N] [--max-bytes BYTES]`
  - Emits a minimal, semantically-closed x07AST slice around `--ptr` (plus `slice_meta` describing omissions, remaps, and truncation).
  - Pointers refer to the canonical x07AST view (run `x07 fmt` or use pointers produced by toolchain diagnostics).
  - If the focus decl is re-indexed to `decls[0]`, `slice_meta.ptr_remap[]` records the pointer rewrite.
  - If bounds force truncation, `slice_meta.truncated=true` and diagnostic `X07-AST-SLICE-0001` is emitted.
  - With global `--out <path>`, writes the canonical `slice_ast` to `<path>` and omits it from the stdout report to avoid duplication.
  - Tool wrapper schema (`--json`): `spec/x07-tool-ast-slice.report.schema.json` (`schema_version: "x07.tool.ast.slice.report@0.1.0"`).

### AST editing (targeted structural edits)

- `x07 ast edit insert-stmts --in <path> --defn <name> --stmt-file <path> [--stmt-file <path>...] [--validate]`
  - Inserts statement expression(s) into a single function body by name (decl `kind=defn|defasync`).
- `x07 ast edit insert-stmts --in <path> --ptr <json_pointer> --stmt-file <path> [--stmt-file <path>...] [--validate]`
  - Uses a JSON Pointer target directly.
  - If the target is a `["begin", ...]` body, inserts statements before the tail expression.
  - Otherwise rewrites the body to `["begin", <stmts...>, <original_expr>]`.
- `x07 ast edit apply-quickfix --in <path> --ptr <json_pointer> [--code <diag_code>] [--validate]`
  - Applies exactly one lint quickfix (JSON Patch) selected by pointer (and optional diagnostic code).

All edit commands write canonical one-line JSON. With global `--out <path>`, they write to that path; otherwise they edit in place.

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
  - Supports CI gates via `--fail-on world-capability|budget-increase|allow-unsafe|allow-ffi|proof-coverage-decrease|recursion-proof-coverage-decrease|async-proof-coverage-decrease|summary-downgrade|assumption-surface-widen|dev-only-assumption-introduced|bounded-proof-introduced|coverage-summary-imported|operational-entry-diverges|boundary-relaxation|trusted-subset-expansion|capsule-contract-relaxation|capsule-set-change|sandbox-policy-widen|runtime-attestation-regression|weaker-isolation-enabled|network-allowlist-widen|peer-policy-relaxation|capsule-network-surface-widen|package-set-change|lockfile-hash-change|advisory-allowance-enabled`.
  - JSON schema: `spec/x07-review.diff.schema.json` (`schema_version: "x07.review.diff@0.5.0"`).

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
- `x07 trust profile check --profile arch/trust/profiles/verified_core_pure_v1.json --project x07.json --entry <sym>`
  - Validates a certification profile against the current project posture.
  - Built-in profile line: `verified_core_pure_v1`, `trusted_program_sandboxed_local_v1`, `trusted_program_sandboxed_net_v1`, and `certified_capsule_v1` (`x07.trust.profile@0.4.0`).
  - Strong profiles validate `project.operational_entry_symbol`, optional `project.certification_entry_symbol`, per-symbol prove requirements, and coverage-import policy before certification starts.
- `x07 trust capsule check --index arch/capsules/index.x07capsule.json --project x07.json`
  - Validates a capsule index plus referenced contracts/attestations.
- `x07 trust capsule attest --contract <path> --module <path>... --lockfile x07.lock.json --conformance-report <path> --out <path>`
  - Emits a deterministic `x07.capsule.attest@0.2.0` artifact for a certified capsule.
- `x07 trust certify --project x07.json --profile arch/trust/profiles/verified_core_pure_v1.json --entry <sym> --out-dir target/cert`
  - Emits a certificate bundle with boundary coverage, schema-derive drift reports, verify coverage, prove reports, proof inventory, proof assumptions, proof-check acceptance metadata, formal-verification scope fields, compile attestation evidence, dependency-closure evidence, and any observed capsule/runtime/peer-policy evidence references (`x07.trust.certificate@0.7.0`).
  - Strong profiles reject surrogate certification entries, coverage-only imports, developer-only imported stubs, and bounded recursive proof usage.

- `x07 prove check --proof <path>`
  - Independently checks a proof object emitted by `x07 verify --prove --emit-proof`.
  - JSON schema: `spec/x07-verify.proof-check.report.schema.json` (`schema_version: "x07.verify.proof_check.report@0.2.0"`).

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
- `x07 explain <CODE>`
  - Alias for `x07 diag explain <CODE>`.
- `x07 diag sarif --in <x07diag.json> --out <results.sarif>`
  - Converts `x07diag` to SARIF v2.1.0 for code-scanning UIs.

### Repro bundles

- `x07 repro compile --project x07.json`
  - Writes a self-contained compile repro directory bundle (project + lock + deps snapshot) and emits a `diagnostics.json` report by running `x07 check` inside the bundle.

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
- `x07 verify --prove --entry <sym>`
- `x07 verify --coverage --entry <sym>`
  - `--project <path>` (or one/more `--module-root <dir>`)
  - `--unwind <n>` (CBMC loop unwinding bound)
  - `--max-bytes-len <n>` (bound for `bytes` / `bytes_view` params)
  - `--proof-summary <path>` (import reviewed proof-summary artifacts from `x07 verify --prove`)
  - `--summary <path>` (deprecated alias for `--proof-summary`)
  - `--allow-imported-stubs` (developer-only proof mode)
  - `--emit-proof <path>` (emit proof object and proof-check artifacts for independent checking)

Notes:

- Direct `--prove` inputs currently support `i32`, `u32`, raw `bytes`, raw `bytes_view`, raw `vec_u8`, `option_i32`, `option_bytes`, `option_bytes_view`, `result_i32`, `result_bytes`, and `result_bytes_view`.
- Direct prove inputs accept unbranded `bytes` / `bytes_view` / `vec_u8`, first-order `option_*` / `result_*`, and branded `bytes_view` carriers whose brand resolves through reachable `meta.brands_v1.validate`.
- That means schema-derived record and tagged-union documents can be proved directly as `bytes_view@brand` inputs, with the generated verify driver running the validator before it constructs the branded view seen by the proof target.
- Owned branded `bytes` and nested result carriers are still rejected explicitly.
- `x07 verify` supports the certifiable subset of reachable `defn` and `defasync` targets. Pure self-recursive `defn` targets are supported when they declare `decreases[]`; mutual recursion, recursive `defasync`, `while` loops, and `for` loops with non-literal bounds remain unsupported.
- `--prove` is the certifiable mode for accepted trust certificates; unsupported targets return `result.kind = "unsupported"`.
- `--prove` reports include `proof_summary` for the solver engine, recursion kind, `decreases` usage, unwind-bounded recursion, and the reachable dependency symbol set.
- `--coverage` emits a reachable-closure support artifact under `coverage` using `spec/x07-verify.coverage.schema.json`, including `supported*` counters plus per-function `support_summary`, alongside `trusted_scheduler_model` and `capsule_boundary` statuses when they apply.
- `--coverage` and `--prove` emit reusable coverage/support summaries (`x07.verify.summary@0.2.0`, `summary_kind = "coverage_support"`). These are posture artifacts only and do not satisfy proof requirements.
- Successful `--prove` runs emit reusable proof summaries (`x07.verify.proof_summary@0.2.0`, `summary_kind = "proof"`). Pass them back with `--proof-summary <path>` when a reviewed reachable dependency sits outside the currently loaded module roots.
- Coverage/support summaries are rejected anywhere proof imports are required.
- When `--emit-proof <path>` is set, `x07 verify --prove` also emits a proof object plus a proof-check report that `x07 prove check` can semantically replay independently.
- Async `--prove` failures emit `x07.verify.cex@0.2.0`, including `await_invariant`, `scope_invariant`, and `cancellation_ensures` counterexamples when those checks fail.
- Artifacts are written under `.x07/artifacts/verify/<mode>/<entry>/` (driver module, emitted C, CBMC output, counterexample/SMT artifacts when present).
- Async proof coverage is lowered through the trusted scheduler model catalog at `catalog/verify_scheduler_model.json`.

Report schema: `spec/x07-verify.report.schema.json` (`schema_version: "x07.verify.report@0.8.0"`).

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
- `x07 schema derive --input <schema.x07schema.json> --out-dir <dir> --emit-boundary-stub --write`
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
- `x07 pkg versions <name> --refresh`
- `x07 pkg lock --project x07.json`
- `x07 pkg attest-closure --project x07.json --out <path>`
- `x07 pkg provides <module-id>`
- `x07 pkg pack --package <dir> --out <path>`
- `x07 pkg login --index <registry_url>`
- `x07 pkg publish --package <dir> --index <registry_url>`

Notes:

- `x07 pkg add <name>@<version>` edits `x07.json` only (no network) unless you pass `--sync`.
- `x07 pkg add <name>` consults the index to resolve a version (network unless you use a file-based index).
- `x07 pkg lock` uses the official registry index by default when fetching is required; override with `--index` or use `--offline`.
- `x07 pkg attest-closure` emits `x07.dep.closure.attest@0.1.0` and exits with code `20` when the closure is materialized but yanked/advisory policy fails.
- Use `x07 pkg lock --project x07.json --check` in CI to fail if `x07.lock.json` is out of date.
- When the index can be consulted, `x07 pkg lock --check` also fails on yanked dependencies and active advisories unless you explicitly allow them (`--allow-yanked` / `--allow-advisories`).
- Sparse index reads (including `x07 pkg versions`) may be cached; use `x07 pkg versions --refresh <name>` after publishing to force a cache-busting fetch (HTTP/HTTPS indexes only).
- For transitive dependency overrides, use `project.patch` in `x07.json` (canonical manifest schema: `x07.project@0.5.0`; `x07.project@0.2.0`, `x07.project@0.3.0`, and `x07.project@0.4.0` are accepted for legacy manifests, but the current certification surfaces use `project.operational_entry_symbol` and related `0.4.0` fields which remain present in `0.5.0`).
- Some packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` may add them to `x07.json` before locking; do not rely on this for correctness (prefer the capability map and templates, which list the full canonical set explicitly).

### Project check (no emit)

- `x07 check --project x07.json`
  - Reads `x07.json` + `x07.lock.json` and resolves the full module import graph (including locked dependencies).
  - Runs schema validation + lint + project-wide typecheck + backend-check.
  - Non-mutating: does not run the repair loop and does not write source files.
  - Does not emit C and does not invoke any native compiler.
  - Emits an `x07diag` JSON report to stdout (or writes it with global `--out`).

Tool wrapper schema (`--json`): `spec/x07-tool-check.report.schema.json` (`schema_version: "x07.tool.check.report@0.1.0"`).

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
- `x07 bundle --project x07.json --profile os --solve-fuel <u64> --out dist/mytool`
  - Overrides the initial solve fuel cap (otherwise `profile.solve_fuel`, then the built-in default).
- `x07 bundle --project x07.json --profile sandbox --out dist/mytool`
  - Bundles a VM-backed sandbox bundle by default (requires a base policy via profile or `--policy`).
  - To emit a legacy policy-only bundle (weaker isolation), add: `--sandbox-backend os --i-accept-weaker-isolation`.

Bundle report schema: `spec/x07-bundle.report.schema.json` (`schema_version: "x07.bundle.report@0.4.0"`).

### Running programs (canonical)

Use `x07 run` as the canonical entry point for execution. Prefer intent-driven profiles (`x07.json.default_profile` + `x07.json.profiles`) so most invocations look like:

- `x07 run`
- `x07 run --profile os`
- `x07 run --profile sandbox`

`x07 run` runs the canonical auto-repair loop by default (format → lint → quickfix, repeatable). Control it with:

- `x07 run --repair=off`
- `x07 run --repair=memory`
- `x07 run --repair=write` (default)

For `run-os-sandboxed`, `x07 run --attest-runtime <path>` writes `x07.runtime.attest@0.2.0` and records the reference in the runner and wrapped reports.

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
- Compatibility mode overrides are available on build/compile entry points (`x07 run`, `x07 build`, `x07 check`, `x07 test`) via `--compat ...` or `X07_COMPAT=...`. See: [Compatibility modes](../reference/compat.md).

## Agent bootstrap recipe

Canonical minimal sequence (keep the loop simple; prefer `x07 run`):

1. Discover CLIs: `x07 --cli-specrows`
2. Iterate: `x07 run` (auto-repair by default; use `--repair=off` when debugging)
3. Validate whole project (non-mutating): `x07 check --project x07.json`
4. Validate behavior: `x07 test`
5. Debug/repair explicitly (when needed): `x07 fmt` / `x07 lint` / `x07 fix` / `x07 ast apply-patch`

See: [Agent quickstart](../getting-started/agent-quickstart.md).
- `x07 verify` requires at least one contract clause (`requires` / `ensures` / `invariant` / `decreases`) on the target function.
