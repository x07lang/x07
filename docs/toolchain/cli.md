# Toolchain CLI

X07 ships multiple small CLIs with JSON-first contracts so both humans and agents can drive it deterministically.

## Key commands

### New project skeleton

- `x07 --init`
  - Creates `x07.json` (with `test`/`os`/`sandbox` profiles), `x07.lock.json`, a minimal `src/` program, and a `tests/` harness (`tests/tests.json`).
- `x07 --init --package`
  - Also creates `x07-package.json` (required only for publishable packages).

### Doctor (platform prerequisites)

- `x07up doctor --json`
  - Checks toolchain integrity and host prerequisites (C compiler + common native deps).

### Guide (built-in language + stdlib reference)

- `x07 guide`
  - Prints the built-in language + stdlib reference guide (Markdown).

### Formatting (x07AST JSON)

- `x07 fmt --input <path> --check`
- `x07 fmt --input <path> --write`

### Linting + diagnostics (`x07diag` JSON)

- `x07 lint --input <path> --world solve-pure`
  - Prints an `x07diag` JSON report to stdout.
- `x07 fix --input <path> --world solve-pure --write`
  - Applies quickfixes (JSON Patch) and rewrites the file deterministically.

### Patching (RFC 6902 JSON Patch)

- `x07 ast apply-patch --in <path> --patch <patch.json> --out <path> --validate`

### Testing (`x07test` JSON)

- `x07 test --manifest tests/tests.json`
  - Uses deterministic worlds by default.
  - Prints an `x07test` JSON report (or writes it with `--report-out`).

### Doc (module exports)

- `x07 doc <module-id>`
- `x07 doc <module-id>.<exported_symbol>`
  - Prints exported symbol signatures from a module file (useful for agents exploring unfamiliar modules).

### RR (record solve-rr fixtures)

- `x07 rr record --out fixtures/rr <key> <url>`
  - Records a real HTTP response body into a `solve-rr` fixture directory (see `docs/worlds/fixture-worlds.md`).

### Packages (pack/lock/publish)

- `x07 pkg add <name>@<version>`
- `x07 pkg add <name>@<version> --sync`
- `x07 pkg lock --project x07.json`
- `x07 pkg pack --package <dir> --out <path>`
- `x07 pkg login --index <registry_url>`
- `x07 pkg publish --package <dir> --index <registry_url>`

Notes:

- `x07 pkg add` edits `x07.json` only (no network) unless you pass `--sync`.
- `x07 pkg lock` uses the official registry index by default when fetching is required; override with `--index` or use `--offline`.
- Use `x07 pkg lock --check` in CI to fail if `x07.lock.json` is out of date.
- If any dependency declares required helper packages via `meta.requires_packages`, `x07 pkg lock` will add them to `x07.json` (and then lock them).

### Build to C (project)

- `x07 build --project x07.json --out build/program.c`
  - Reads `x07.json` + `x07.lock.json`.
  - Uses `x07.json` `module_roots` plus locked dependency module roots (from the lockfile).

### Running programs (canonical)

Use `x07 run` as the canonical entry point for execution. Prefer intent-driven profiles (`x07.json.default_profile` + `x07.json.profiles`) so most invocations look like:

- `x07 run`
- `x07 run --profile os`
- `x07 run --profile sandbox`

For the complete guide (targets, worlds, input, fixtures, policies, reports), see [Running programs](running-programs.md).

## JSON outputs (agent-friendly)

Commands that emit structured outputs must guarantee:

- valid JSON
- schema-valid output
- stable ordering (canonical JSON writer)

For machine-first discovery and debugging:

- `x07 --cli-specrows` emits a deterministic CLI surface description.
- Many commands support `--report-json` for tool-style wrapper reports.

See [Diagnostics & repair](diagnostics-repair.md).

## Programmatic CLI discovery (`--cli-specrows`)

Agents should not scrape `--help` text. Use `--cli-specrows`:

- `x07 --cli-specrows`
- `x07c --cli-specrows`
- `x07-host-runner --cli-specrows`
- `x07-os-runner --cli-specrows`

Output is a single JSON object:

- `schema_version: "x07cli.specrows@0.1.0"`
- `rows: [...]` (a flat, ordered table describing the CLI surface)

Each row is a small tuple. Examples:

- `["root","help","-h","--help","Print help"]`
- `["root","version","","--version","Print version"]`

Schema: `spec/x07cli.specrows.schema.json`.

## Tool wrapper reports (`--report-json`)

Some commands can emit a tool-style wrapper report for agents:

- `x07 fmt --report-json ...`
- `x07 lint --report-json ...`
- `x07 fix --report-json ...` (requires `--write`)

Wrapper schema: `schema_version: "x07c.report@0.1.0"` (see `spec/x07c.report.schema.json`).

Notes:

- `x07 lint` without `--report-json` prints the raw diagnostics report (`x07diag`, see `spec/x07diag.schema.json`).
- `x07 fix` without `--report-json` prints the fixed x07AST JSON to stdout unless `--write` is set.

## Agent bootstrap recipe

Canonical minimal sequence:

1. Discover CLIs: `x07 --cli-specrows`
2. Format + lint: `x07 fmt` / `x07 lint` (use `--report-json` when you need wrapper reports)
3. Repair: `x07 fix` and/or `x07 ast apply-patch`
4. Execute + validate: `x07 run`, `x07 test`

See: [Agent quickstart](../getting-started/agent-quickstart.md).
