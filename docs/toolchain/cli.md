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

### Running generated executables

Use `x07 run` as the canonical entry point for execution.

#### CLI args (`argv_v1`) passthrough

If your program expects `argv_v1` input, pass process args after `--` and `x07 run` will encode them into input bytes:

- `x07 run --profile os -- tool --help`

#### Profiles (recommended)

`x07 run` supports intent-driven **profiles** defined in your project manifest (`x07.json`):

- `default_profile`: which profile `x07 run` uses by default
- `profiles.<name>`: a profile definition (world + optional defaults)

Common profile names are:

- `test` → deterministic `solve-*` world
- `os` → `run-os`
- `sandbox` → `run-os-sandboxed` + policy

Examples:

- `x07 run` (uses `x07.json.default_profile`)
- `x07 run --profile os`
- `x07 run --profile sandbox`

World IDs (`--world solve-pure`, `--world run-os`, ...) remain available as an advanced escape hatch.

#### Policies (run-os-sandboxed)

`run-os-sandboxed` requires an explicit policy file. Use `x07 policy init` to generate a schema-valid starting point, then extend it for your app (filesystem roots, env keys, subprocess allowlists, resource limits):

- `x07 policy init --template cli`
- `x07 policy init --template http-client`
- `x07 policy init --template web-service`
- `x07 policy init --template fs-tool`
- `x07 policy init --template sqlite-app`
- `x07 policy init --template postgres-client`
- `x07 policy init --template worker`

For net-enabled templates, the base policy starts with `net.allow_hosts: []` (deny-by-default destinations). Use `--allow-host` / `--deny-host` to materialize a derived policy under `.x07/policies/_generated/`:

- `x07 run --world run-os-sandboxed --policy .x07/policies/base/http-client.sandbox.base.policy.json --allow-host example.com:443`
- `x07 run --world run-os-sandboxed --policy .x07/policies/base/http-client.sandbox.base.policy.json --allow-host example.com:80,443 --deny-host example.com:80`

Derived policies are written under `.x07/policies/_generated/` and passed to `x07-os-runner`.

The generated `main` uses a simple byte framing:

- stdin: `u32_le(len)` followed by `len` bytes
- stdout: `u32_le(len)` followed by `len` bytes
- stderr: JSON stats (fuel/heap/fs/rr/kv/scheduler)

`x07 run`, `x07-host-runner`, and `x07-os-runner` handle this framing automatically.

## JSON outputs (agent-friendly)

Commands that emit structured outputs must guarantee:

- valid JSON
- schema-valid output
- stable ordering (canonical JSON writer)

For machine-first discovery and debugging:

- `x07 --cli-specrows` emits a deterministic CLI surface description.
- Many commands support `--report-json` for tool-style wrapper reports.

See [Diagnostics & repair](diagnostics-repair.md).
