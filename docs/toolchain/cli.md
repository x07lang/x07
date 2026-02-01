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

### Patching (RFC 6902 JSON Patch)

- `x07 ast apply-patch --in <path> --patch <patch.json> --out <path> --validate`

### Testing (`x07test` JSON)

- `x07 test --manifest tests/tests.json`
  - Runs each test in its declared world.
  - Prints an `x07test` JSON report (or writes it with `--report-out`).

### Doc (module exports)

- `x07 doc <module-id>`
- `x07 doc <module-id>.<exported_symbol>`
  - Prints exported symbol signatures from a module file (useful for agents exploring unfamiliar modules).

### Schema derive (generate modules + tests)

- `x07 schema derive --input <schema.x07schema.json> --out-dir <dir> --write`
- `x07 schema derive --input <schema.x07schema.json> --out-dir <dir> --check`
- `x07 schema derive --report-json ...`

See: [Schema derive](schema-derive.md).

### State machines (generate step functions + tests)

- `x07 sm check --input <sm.json>`
- `x07 sm gen --input <sm.json> --out <dir> --write`
- `x07 sm gen --input <sm.json> --out <dir> --check`

See: [State machines](state-machines.md).

### Packages (pack/lock/publish)

- `x07 pkg add <name>@<version>`
- `x07 pkg add <name>@<version> --sync`
- `x07 pkg lock --project x07.json`
- `x07 pkg provides <module-id>`
- `x07 pkg pack --package <dir> --out <path>`
- `x07 pkg login --index <registry_url>`
- `x07 pkg publish --package <dir> --index <registry_url>`

Notes:

- `x07 pkg add` edits `x07.json` only (no network) unless you pass `--sync`.
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

Canonical minimal sequence (keep the loop simple; prefer `x07 run`):

1. Discover CLIs: `x07 --cli-specrows`
2. Iterate: `x07 run` (auto-repair by default; use `--repair=off` when debugging)
3. Validate: `x07 test`
4. Debug/repair explicitly (when needed): `x07 fmt` / `x07 lint` / `x07 fix` / `x07 ast apply-patch`

See: [Agent quickstart](../getting-started/agent-quickstart.md).
