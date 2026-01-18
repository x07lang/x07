# Toolchain CLI

X07 ships multiple small CLIs with JSON-first contracts so both humans and agents can drive it deterministically.

## Key commands

### New project skeleton

- `x07 --init`
  - Creates `x07.json`, `x07-package.json`, `x07.lock.json`, and a minimal `src/` program.

### Formatting (x07AST JSON)

- `x07c fmt --input <path> --check`
- `x07c fmt --input <path> --write`

### Linting + diagnostics (`x07diag` JSON)

- `x07c lint --input <path> --world solve-pure`
  - Prints an `x07diag` JSON report to stdout.
- `x07c fix --input <path> --world solve-pure --write`
  - Applies quickfixes (JSON Patch) and rewrites the file deterministically.

### Patching (RFC 6902 JSON Patch)

- `x07c apply-patch --program <path> --patch <patch.json> --out <path>`

### Testing (`x07test` JSON)

- `x07 test --manifest tests/tests.json`
  - Uses deterministic worlds by default.
  - Prints an `x07test` JSON report (or writes it with `--report-out`).

### Packages (pack/lock/publish)

- `x07 pkg add <name>@<version>`
- `x07 pkg lock --project x07.json --index <registry_url>`
- `x07 pkg pack --package <dir> --out <path>`
- `x07 pkg login --index <registry_url>`
- `x07 pkg publish --package <dir> --index <registry_url>`

## JSON outputs (agent-friendly)

Commands that emit structured outputs must guarantee:

- valid JSON
- schema-valid output
- stable ordering (canonical JSON writer)

See [Diagnostics & repair](diagnostics-repair.md).
