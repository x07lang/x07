# Agent context packs

`x07 agent context` produces a single deterministic artifact that an agent can consume to repair a failing project with minimal token usage.

## CLI

```bash
x07 agent context --diag <path> --project <path> [--enclosure decl|defn|module] [--closure locals|types|imports|all] [--max-nodes N] [--max-bytes BYTES]
```

- `--diag` accepts either:
  - a raw `x07diag` document (`schema_version: "x07.x07diag@0.1.0"`), or
  - a tool wrapper report (`schema_version: "x07.tool.*.report@0.1.0"`) where `diagnostics[]` is extracted.
- `--project` is the project manifest (`x07.json`) used to locate the entry module.
- Slicing controls are forwarded to the same deterministic engine as `x07 ast slice`.
- With global `--out <path>`, the context pack is written to `<path>` and stdout stays empty.

## Artifact schema

- Payload schema: `spec/x07-agent.context.schema.json` (`schema_version: "x07.agent.context@0.1.0"`).
- Wrapper schema (`--json`): `spec/x07-tool-agent-context.report.schema.json` (`schema_version: "x07.tool.agent.context.report@0.1.0"`).

## Structure (high level)

The context pack contains:

- `toolchain`: tool name + version (for reproducibility).
- `project`: `root`, `world`, and `entry` (the input program path).
- `focus`: a deterministic focus selection (`diag_code`, `loc_ptr`).
- `diagnostics`: the embedded `x07diag` payload.
- `ast`: the entry module slice (`slice_ast`) plus `slice_meta` describing omissions, pointer remaps, and truncation.
- `digests`: stable sha256 digests for all inputs used to build the pack.

## Pointer remapping

AST slices may re-index decls (for example, moving the focused decl to `decls[0]`). When that happens, `ast.slice_meta.ptr_remap[]` provides a deterministic rewrite map from pointers into the source AST to pointers into the sliced AST.

## Truncation

If `--max-nodes` or `--max-bytes` forces truncation, the slice sets:

- `ast.slice_meta.truncated=true`
- `ast.slice_meta.truncation` (stats and reason)

and emits diagnostic code `X07-AST-SLICE-0001` in the slice report.
