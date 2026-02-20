# Agent context packs (internal)

This document defines the deterministic semantics behind:

- `x07 ast slice`
- `x07 agent context`

It is intentionally implementation-oriented and may be more specific than public docs.

## Deterministic AST slicing (`x07 ast slice`)

### Enclosure

- `decl` (default): `--ptr` must be under `/decls/<i>`. The slice emits a schema-valid x07AST module with the focused decl moved to `decls[0]`.
- `defn`: same as `decl`, but errors unless the enclosing decl kind is `defn` or `defasync`.
- `module`: the enclosure is the whole input document (module/entry). Decl re-indexing is not applied.

### Closure categories

`--closure` selects which context categories to include beyond the enclosure:

- `locals`: includes the focused decl and same-module decl dependencies discovered by scanning JSON S-expr call heads, transitively.
- `types`: includes optional type-related fields (type params and contract clauses) and keeps brands.
- `imports`: includes the minimal imports required by referenced symbols (conservative and deterministic).
- `all`: includes `locals + types + imports`.

Implementation notes:

- Dependency discovery treats any JSON S-expr list head (`["head", ...]`) as a potential symbol reference.
- Output decl order is deterministic: focus decl first (for `decl`/`defn`), then the remaining selected decls in ascending original decl index.
- Export decls are included only when they export at least one selected symbol; their `names[]` list is filtered accordingly.

### Omitted / missing metadata

- `slice_meta.omitted.*` indicates categories intentionally excluded by `--closure`.
- `slice_meta.missing.*` provides conservative hints about what was excluded:
  - `missing.imports`: the minimal import set that would have been included.
  - `missing.locals`: referenced same-module symbols not included when `locals` is omitted.
  - `missing.types`: removed type parameter names plus `"contracts"` when any contract clauses were stripped.
- `slice_meta.ptr_remap[]` records pointer rewrites when the focused decl is moved to `decls[0]`.

### Bounds and truncation

Bounds are enforced deterministically:

1. `--max-nodes` drops non-focus decls from the end of `decls[]` until the bound is satisfied.
2. `--max-bytes` drops non-focus decls from the end of `decls[]`, then prunes inside the focus region by eliding sibling subtrees not on the focus pointer path. If needed, the slice falls back to replacing the focus decl `body` (or the module `solve`) with `0`.

When truncation occurs:

- `slice_meta.truncated=true`
- `slice_meta.truncation` is populated with stats and a deterministic reason string
- Diagnostic code `X07-AST-SLICE-0001` is emitted with truncation stats in `diagnostic.data`

## Context pack construction (`x07 agent context`)

### Focus selection

Focus selection is deterministic:

1. Pick the first diagnostic with `severity="error"`.
2. If none exist, pick the first diagnostic.
3. If no diagnostics exist, the command fails.

The focused diagnostic must have `loc.kind="x07ast"` so `focus.loc_ptr` is a JSON Pointer into the entry x07AST.

### Inputs and digests

The context pack embeds:

- the diagnostics payload (`x07.x07diag@0.1.0`), including structured `diagnostics[].data` payloads (for example `mem_provenance`)
- an AST slice (`ast.slice_ast` + `ast.slice_meta`)
- stable input digests for traceability:
  - diag file
  - project manifest (`x07.json`)
  - entry x07AST (`*.x07.json`)

`digests.outputs` is intentionally empty to avoid self-referential hashing (the pack would need to hash itself).

### Determinism invariants

- Artifacts are emitted as canonical JSON (JCS).
- Decl selection, ordering, and import minimization are deterministic for identical inputs.
- `toolchain.version` is embedded so context packs can be traced to the exact CLI behavior.
