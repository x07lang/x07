# Compatibility contract

This page is the single source of truth for what “compatibility” means in the X07 ecosystem.

The goal is to make it safe and measurable to evolve the toolchain without repeatedly breaking
real projects and dependencies.

## Definitions

X07 has multiple versioned surfaces:

- **Toolchain**: `x07` / `x07c` SemVer releases.
- **Project manifests**: `x07.json` (`x07.project@…`) and lockfiles (`x07.lock@…`).
- **Programs/modules**: `*.x07.json` (`x07.x07ast@…`).
- **Contracts/schemas**: JSON schemas under `spec/` (for example `x07diag`, `x07patch`, `x07test`).
- **Stable function contracts**: `_v1` suffixes and other explicitly versioned ABIs/encodings.

## Compat Selection (`project.compat`, `--compat`, `X07_COMPAT`)

`x07.project@0.5.0` introduces an optional `project.compat` field that pins language/toolchain
compatibility semantics for a project.

Example:

```jsonc
{
  "schema_version": "x07.project@0.5.0",
  "compat": "0.5",
  "world": "run-os",
  "entry": "src/main.x07.json",
  "module_roots": ["src"]
}
```

Resolution order (highest priority first):

- CLI: `--compat ...`
- Environment: `X07_COMPAT=...`
- Project: `x07.json` `compat`
- Default: `current`

When a behavior change is mechanically migratable, prefer `x07 migrate --check/--write` to
explicitize required rewrites.

## What Must Stay Compatible

The following are treated as “compat-critical” surfaces:

- Older `x07.project@…` manifests should remain readable and runnable for as long as we claim support
  in `docs/packages/index.md`.
- Older `x07.lock@…` lockfiles should remain readable, or fail with a deterministic error that tells
  the user what to do next.
- Older `x07.x07ast@…` programs and modules used by official packages should continue to compile.
- Call-argument compatibility: in call-argument position, `bytes` (and in some APIs, `vec_u8`) is
  accepted where a parameter expects `bytes_view` and is treated as an implicit view for that call.
  This is a call-site-only rule (`bytes` is not a general subtype of `bytes_view`).
- Diagnostic codes are stable identifiers. New codes are additive; existing code meanings should not
  be silently repurposed.
- Versioned contracts (`*_v1`, `MAGIC+VERSION` envelopes, `spec/` schemas) do not change in-place.

## What Is Allowed To Change

Some surfaces are intentionally allowed to evolve:

- Non-versioned CLI text output (use `--json`/reports for machine stability).
- Performance characteristics (as long as determinism and resource bounds remain intact).
- Internal implementation details that are not part of a documented public contract.

## Guardrails (Milestone M0)

Compatibility is enforced by CI guardrails:

- **Compat corpus**: a curated set of projects and fixability cases that must keep working across
  toolchain changes.
  - Config: `tests/compat_corpus/corpus.json`
  - Harness: `scripts/ci/check_compat_corpus.sh`
- **Diagnostics catalog coverage**: every emitted diagnostic code must be documented and discoverable.
  (See `x07 diag coverage` and the CI gate described in this milestone.)

## How To Introduce A Behavior Change

Any behavior change that can affect downstream packages/projects must:

1. Be documented here first (including the motivation and the migration story).
2. Have a regression surface in the compat corpus (a project or a fixability case).
3. Either:
   - be behind a compatibility switch, or
   - be mechanically migratable (via `x07 fix` and `x07 migrate`).

## Change → Migration Mapping (Appendix)

When a breaking or near-breaking change is introduced, add an entry here with:

- the exact behavior change
- what breaks (and how to detect it)
- the mechanical rewrite (when possible)
- the compatibility switch (when applicable)
- links to the relevant diagnostic codes

### Control Flow: `while` (compat `0.5`)

Behavior:

- Adds the core form `["while", cond, body]` (returns `i32` `0`) to reduce recursion-as-loop pressure.

Migration:

- No migration required (additive).

### Recursion: Termination Evidence (compat `0.5`)

Behavior:

- Direct self-recursive `defn` targets require `decreases[]` only when the function declares any contract clauses (`requires`/`ensures`/`invariant`/`decreases`).
- Functions with no contract clauses may recurse without `decreases[]` boilerplate.
- `decreases[]` is allowed as the only contract clause.

Detection:

- `X07-CONTRACT-0011` indicates a recursive self-call is missing `decreases[]` on a contract-bearing function.

Mechanical rewrite:

- Add `decreases[]` to the recursive `defn`.
- When the recursive call matches a common pattern (for example `n -> n-1`), `x07 fix --write` / `x07 migrate --write --to 0.5` can insert an inferred `decreases[]` clause.
