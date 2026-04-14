# ext-cli

`ext-cli` is the canonical package for deterministic CLI parsing, help generation, and completion.

It consumes **specrows** (`x07cli.specrows@0.1.0`) and an `argv_v1` byte encoding (see [CLI apps](../guides/cli-apps.md)).

See also:

- Patterns: [CLI patterns (ext-cli v2)](../guides/cli-patterns.md)
- Runnable reference: `docs/examples/agent-gate/cli-ext-cli/`

## Exports (module: `ext.cli`)

- Parsing
  - `ext.cli.parse_specrows(spec_json, argv_v1) -> bytes`
  - `ext.cli.parse_specrows_v2(spec_json, argv_v1, env_v1) -> bytes` (env defaults)
  - `ext.cli.parse_compiled(specbin_v1, argv_v1) -> bytes`
  - `ext.cli.parse_compiled_v2(specbin_v1, argv_v1, env_v1) -> bytes`
- Matches (on OK docs)
  - `ext.cli.matches_cmd(doc) -> bytes` (scope path: `"root"`, `"pkg"`, `"pkg.add"`, …)
  - `ext.cli.matches_get(doc, key, kind) -> option_bytes`
- Errors (on ERR docs)
  - `ext.cli.err_code(doc) -> i32`
  - `ext.cli.err_msg(doc) -> bytes`
  - `ext.cli.err_hint(doc) -> bytes`
  - `ext.cli.err_usage(doc) -> bytes`
  - `ext.cli.err_arg(doc) -> bytes` (optional)
  - `ext.cli.err_scope(doc) -> bytes` (optional)
  - `ext.cli.err_doc_v2(doc) -> bytes` (`ext.data_model` map with stable fields)
- Help
  - `ext.cli.render_help(spec_json_or_specbin, scope) -> bytes`

## Specrows helpers (module: `ext.cli.specrows`)

- `ext.cli.specrows.compile(spec_json) -> result_bytes`
  - Returns a compiled doc that `parse_compiled*` and `render_help` accept.
  - Inserts implied `--help` (all scopes) and `--version` (root) rows if missing.
- `ext.cli.specrows.validate(spec_json) -> result_i32`

## `matches_get` kinds and encodings

`ext.cli.matches_get(doc, key, kind)` queries the parse result for a specific key and kind:

- `kind=1`: flags (presence)
- `kind=2`: option/arg values (typed encoding)

Typed encodings for `kind=2`:

- `STR` / `PATH` / `BYTES` / `ENUM`: raw token bytes
- `U32`: 4-byte little-endian
- `I32`: 4-byte little-endian two’s complement
- `BOOL`: 1 byte (`0` or `1`)
- `BYTES_HEX`: decoded bytes

Repeated options (`meta.multiple=true`) use a multi encoding stored under `kind=2`:

- `u32_le(count)`
- then `count` items of `u32_le(len)` + `len` bytes of the per-item typed encoding

## Canonical error map (`err_doc_v2`)

`ext.cli.err_doc_v2(doc)` returns an `ext.data_model` document representing:

- `{ code, message, hint, usage, arg?, scope? }`

This is intended for stable machine handling (JSON rendering, structured logs, etc) without needing to parse human-formatted text.

