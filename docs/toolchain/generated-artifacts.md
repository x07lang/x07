# Generated artifacts

X07 has multiple deterministic generators (for example `x07 schema derive`, `x07 sm gen`, and `x07 xtal tests gen-from-spec`). If a project commits generated outputs, CI should enforce that those outputs:

- exist,
- match the pinned generator invocation byte-for-byte (no drift), and
- are reproducible (no nondeterminism).

## Contract: `arch/gen/index.x07gen.json`

The generator index is a contracts-as-data file that declares which generators exist and what outputs they own.

- Schema: `x07.arch.gen.index@0.1.0` (see `docs/spec/schemas/x07-arch.gen.index.schema.json`)
- Default path: `arch/gen/index.x07gen.json`

Each `generators[]` entry declares:

- `id`: stable identifier
- `mode`: `double_run_v1` (default) or `single_run_v1`
- `check_argv`: pinned generator argv (relative to `x07`, not a shell)
- `write_argv`: pinned generator argv (relative to `x07`, not a shell)
- `outputs`: one or more output roots (files or directories; repo-relative)

Minimal example:

```json
{
  "schema_version": "x07.arch.gen.index@0.1.0",
  "generators": [
    {
      "id": "xtal.toy_sorter",
      "mode": "double_run_v1",
      "check_argv": ["xtal", "tests", "gen-from-spec", "--project", "x07.json", "--check"],
      "write_argv": ["xtal", "tests", "gen-from-spec", "--project", "x07.json", "--write"],
      "outputs": ["gen/xtal/"]
    }
  ]
}
```

## Commands

- `x07 gen verify --index arch/gen/index.x07gen.json`
  - Non-mutating.
  - Checks that declared `outputs` exist.
  - Runs each generator’s `check_argv` in a clean temporary copy of the project (against the committed outputs).
  - Runs each generator’s `write_argv` in clean temporary copies to verify drift and determinism.
  - In `double_run_v1`, runs `write_argv` twice and compares the two output trees (determinism gate).
- `x07 gen write --index arch/gen/index.x07gen.json`
  - Mutating.
  - Removes declared outputs, then runs each generator’s `write_argv` in the project root.

## Workflow

1. Edit source-of-truth inputs (`arch/**`, `spec/**`, `*.x07schema.json`, …).
2. Regenerate outputs: `x07 gen write --index arch/gen/index.x07gen.json`
3. Commit both inputs and outputs.
4. CI runs: `x07 gen verify --index arch/gen/index.x07gen.json`
