# Guide: CLI patterns (ext-cli v2)

This guide is the companion to [CLI apps](cli-apps.md). It focuses on `ext-cli` patterns that keep CLIs consistent, typed, and easy to debug.

See the runnable reference project: `docs/examples/agent-gate/cli-ext-cli/`.

## Canonical (specrows JSON + `x07 cli specrows`)

1. Author your CLI as `x07cli.specrows@0.1.0` JSON (see [Specrows model](#specrows-model)).
2. Validate semantics (CI-friendly JSON report on stdout):

   ```bash
   x07 cli specrows check --in path/to/cli.specrows.json
   ```

3. Canonicalize in-place (stable formatting + implied defaults):

   ```bash
   x07 cli specrows fmt --in path/to/cli.specrows.json --write
   ```

4. Parse argv and produce deterministic help/errors with `ext-cli`:
   - help: `ext.cli.render_help(...)`
   - errors: `ext.cli.err_doc_v2(...)` (message + actionable hint + usage excerpt)

## Specrows model

`ext-cli` consumes a declarative CLI spec called **specrows**:

- `schema_version: "x07cli.specrows@0.1.0"`
- `rows: [...]` (flat table; one row per CLI surface element)

Each row begins with a **scope**:

- `"root"` for the top-level command
- a dot-separated subcommand path like `"pkg"` or `"pkg.add"`

Common row shapes:

- `["SCOPE","about","TEXT"]`
- `["SCOPE","flag","-v","--verbose","KEY","DESC"]`
- `["SCOPE","opt","-n","--name","KEY","VALUE_KIND","DESC",{...meta}]`
- `["SCOPE","arg","POS_NAME","KEY","DESC",{...meta}]`

Example spec (JSON):

```json
{
  "schema_version": "x07cli.specrows@0.1.0",
  "app": { "name": "tool", "version": "0.1.0" },
  "rows": [
    ["root", "about", "Example CLI using ext-cli"],
    ["root", "opt", "", "--url", "url", "STR", "URL to fetch", { "required": true }],
    ["root", "opt", "", "--depth", "depth", "U32", "Max depth", { "required": true }],
    ["root", "opt", "", "--out", "out", "PATH", "Output path", { "required": true }]
  ]
}
```

## Validate + canonicalize specrows

For deterministic semantic validation (and stable formatting), use the toolchain validator:

```bash
# Validate (machine-readable JSON report on stdout):
x07 cli specrows check --in path/to/cli.specrows.json

# Canonicalize rows + implied defaults (writes canonical JSON in-place):
x07 cli specrows fmt --in path/to/cli.specrows.json --write
```

The validator is stricter than JSON Schema and enforces rules like per-scope uniqueness (`--help` reserved, no duplicate keys) and typed default validity (`U32`, `I32`, `BOOL`, `BYTES_HEX`, `ENUM`).

## Typed options (`opt` rows)

For `opt` rows, `VALUE_KIND` controls both validation and the bytes encoding returned by `ext.cli.matches_get(..., kind=2)`.

Supported value kinds:

- `STR` (raw token bytes)
- `PATH` (raw token bytes)
- `U32` (4-byte little-endian)
- `I32` (4-byte little-endian)
- `BOOL` (1 byte: `0` or `1`)
- `ENUM` (raw token bytes; validated against `meta.enum`)
- `BYTES` (raw token bytes)
- `BYTES_HEX` (decoded bytes; input token must be even-length hex)

Notes:

- For `ENUM`, provide `meta.enum` (list of allowed values). The parser rejects values not in the list.
- For `U32`, decode using `std.codec.read_u32_le(val, 0)`. For `I32`, the value is 4-byte little-endian two’s complement (convert from the `U32` bits if you need a signed `i32`).

Examples (`opt` rows):

- `["root","opt","","--depth","depth","U32","Max depth",{"required":true}]`
- `["root","opt","","--dry-run","dry_run","BOOL","Dry run",{"default":false}]`
- `["root","opt","","--mode","mode","ENUM","Mode",{"enum":["fast","safe"],"default":"safe"}]`
- `["root","opt","","--token","token","BYTES_HEX","Token",{"required":true}]`

## Built-in help output

Avoid hardcoding usage strings. Use:

- `ext.cli.render_help(spec_json_or_specbin, scope_bytes) -> bytes`

Typical pattern:

1. Parse argv with `ext.cli.parse_specrows(...)`.
2. If the help flag is present, return `ext.cli.render_help(...)`.

If you already parsed successfully, prefer deriving the current scope from the parse result:

- `ext.cli.matches_cmd(doc) -> bytes` (returns `"root"`, `"pkg"`, `"pkg.add"`, …)

Help output is deterministic and structured into sections (`options:`, `args:`, `commands:`). Example shape:

```text
Example CLI using ext-cli
usage: tool --url <STR> --depth <U32> --out <PATH>

options:
  --help            Show help
  --url <STR>       URL to fetch
  --depth <U32>     Max depth
  --out <PATH>      Output path
```

## Canonical error handling

`ext-cli` parse results are always bytes:

- OK doc: `ext.cli.is_ok(doc) == 1`
- ERR doc: `ext.cli.is_ok(doc) == 0`

For human-readable output:

- `ext.cli.err_msg(doc)` (stable machine message)
- `ext.cli.err_hint(doc)` (actionable “expected form” hints)
- `ext.cli.err_usage(doc)` (generated usage/help excerpt)

For a canonical machine-readable map:

- `ext.cli.err_doc_v2(doc) -> bytes` returning a `ext.data_model` document:
  - `{ code, message, hint, usage, arg?, scope? }`

Example `err_doc_v2` shape (conceptual):

```json
{
  "code": 1002,
  "message": "missing_required_opt",
  "hint": "Expected: --url <STR>.",
  "usage": "usage: tool --url <STR> --depth <U32> --out <PATH>\n\noptions:\n  ...\n",
  "arg": "url",
  "scope": "root"
}
```

## Subcommands

Define subcommands by scoping rows:

- root command rows use scope `"root"`
- subcommand rows use scope `"pkg"`
- nested subcommand rows use scope `"pkg.add"`

Example scopes in `rows`:

```json
[
  ["root", "about", "tool"],
  ["pkg", "about", "Package commands"],
  ["pkg.add", "about", "Add a package"],
  ["pkg.add", "opt", "", "--name", "name", "STR", "Package name", { "required": true }]
]
```

At runtime:

- `ext.cli.matches_cmd(doc)` reports the selected scope path
- `ext.cli.render_help(spec, scope)` renders help for the selected scope and lists immediate subcommands (if any)

## Repeated options

For repeated options, set `meta.multiple=true` on the `opt` row.

In that case, `ext.cli.matches_get(..., kind=2)` returns a **multi** encoding:

- `u32_le(count)`
- then `count` items of:
  - `u32_le(len)`
  - `len` bytes of the per-item typed encoding (based on `VALUE_KIND`)

Example (`--tag` repeatable):

- `["root","opt","","--tag","tag","STR","Tag",{"multiple":true}]`

## Env var defaults (`parse_specrows_v2`)

To let an option default from an environment variable, set `meta.env` on the `opt` row and call:

- `ext.cli.parse_specrows_v2(spec_json, argv_v1, env_v1)`

Example (`--url` defaults from `TOOL_URL`):

- `["root","opt","","--url","url","STR","URL",{"env":"TOOL_URL","required":true}]`

`env_v1` encoding:

- `u32_le(count)`
- then `count` entries of:
  - `u32_le(klen)` + key bytes
  - `u32_le(vlen)` + value bytes

In `run-os` worlds you can fetch variables with `std.os.env.get(key)`, then encode the subset of keys you care about into `env_v1`.

## Expert (specbin + runner-level debugging)

- Specrows can be compiled to a compact binary form (specbin) for distribution or embedding:

  ```bash
  x07 cli specrows compile --in path/to/cli.specrows.json --out path/to/cli.specrows.bin
  ```

- If you need to debug outside `x07 run`, you can run the low-level runners directly:
  - `x07-host-runner` (solve worlds)
  - `x07-os-runner` (run-os worlds)
