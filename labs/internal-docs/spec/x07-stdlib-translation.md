# Stdlib translation (`x07import`)

`x07import` is a deterministic importer that translates a restricted subset of
Rust/C into X07 stdlib module files (`*.x07.json`, x07AST JSON). It is implemented
as the `x07import-cli` crate on top of `x07import-core`, and feeds the checked-in,
pinned stdlib package.

## Current state

The importer ships as two crates:

- `crates/x07import-core/` — Rust/C frontends, x07IR lowering, diagnostics.
- `crates/x07import-cli/` — the `x07import-cli` batch/regeneration CLI.

Generated modules are checked in under `stdlib/std/<version>/modules/**`. The
manifest at `ci/fixtures/x07import/manifest.json` declares which modules are
generated, and `stdlib.lock` pins their hashes so a regenerate is reproducible.

Inputs that must be pinned for reproducibility:

- upstream module source version (git rev/tag)
- the `x07import` tool version
- the x07c (backend) version
- `stdlib.lock` + module hashes

Memory model note (ABI v2): prefer `bytes_view` for read-only byte inputs in
imported signatures (borrowed, zero-copy), and return owned `bytes` (drop-clean).
See `labs/internal-docs/spec/x07-memory-management.md` for the ownership rules.

## CLI commands (current)

Full usage and the supported Rust/C subset live in `docs/x07import/`.

- Regenerate from manifest (writes modules):
  `cargo run -p x07import-cli -- batch --manifest ci/fixtures/x07import/manifest.json`
- Drift check (no writes):
  `./scripts/ci/check_x07import_generated.sh`
- Diagnostics catalog sync:
  `./scripts/ci/check_x07import_diagnostics_sync.sh`
- Stdlib lockfile checks:
  `./scripts/ci/check_stdlib_lock.sh`

## See also

- `docs/x07import/README.md` — commands, frontends, and workflow.
- `docs/x07import/porting-by-example.md` — end-to-end tutorial that produces a publishable package.
- `docs/x07import/diagnostics.md` — generated diagnostics catalog.
- `labs/internal-docs/spec/x07-memory-management.md` — ABI v2 memory model + ownership rules.
