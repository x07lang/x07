# `x07 arch check`

This document describes the toolchain implementation and invariants for the architecture manifest checker (`x07 arch check`).

## Schemas and constants

Pinned schema versions:

- `x07.arch.manifest@0.1.0` (`spec/x07-arch.manifest.schema.json`)
- `x07.arch.manifest.lock@0.1.0` (`spec/x07-arch.manifest.lock.schema.json`)
- `x07.arch.report@0.1.0` (`spec/x07-arch.report.schema.json`)
- `x07.arch.patchset@0.1.0` (`spec/x07-arch.patchset.schema.json`)
- Contracts-as-data groups used by `manifest.contracts_v1`:
  - Budgets index/profile: `spec/x07-budgets.index.schema.json`, `spec/x07-budgets.profile.schema.json`
  - Archive index/profile: `spec/x07-arch.archive.index.schema.json`, `spec/x07-arch.archive.profile.schema.json`
  - DB index/plan/queries: `spec/x07-arch.db.index.schema.json`, `spec/x07-db.migrate.plan.schema.json`, `spec/x07-arch.db.queries.schema.json`
  - Obs index/registry/exporters: `spec/x07-arch.obs.index.schema.json`, `spec/x07-obs.metrics.registry.schema.json`, `spec/x07-obs.exporter.profile.schema.json`
  - Net index/gRPC services: `spec/x07-arch.net.index.schema.json`, `spec/x07-arch.net.grpc.services.schema.json`
  - Crypto index/JWT profiles: `spec/x07-arch.crypto.index.schema.json`, `spec/x07-arch.crypto.jwt_profiles.schema.json`

Rust constants live in `crates/x07-contracts/src/lib.rs`.

## CLI

Main entry point:

- `x07 arch check`

Key flags:

- `--manifest <path>` (default: `arch/manifest.x07arch.json`)
- `--lock <path>` (default: `arch/manifest.lock.json` only when it exists)
- `--write-lock` (create/update lock deterministically)
- `--emit-patch <path>` (emit multi-file JSON Patch set)
- `--write` (apply suggested patches deterministically and re-run)
- `--format json|text`
- `--out <path>`
- budgets: `--max-modules`, `--max-edges`, `--max-diags`, `--max-bytes-in`

## Determinism invariants

The checker must remain deterministic:

- module scan order: stable lexicographic by repo-relative path
- edges: deduped and ordered via `BTreeSet`
- diagnostics: stable sorted by `(severity, code, node_from, node_to, module_path, import, msg)`
- JSON output: canonicalized (JCS) and pretty-printed with trailing newline

## Exit codes

- `0`: pass
- `2`: errors found
- `3`: input invalid (manifest/lock/schema) and the check could not run
- `4`: tool budget exceeded

## Repair loop behavior

The JSON report includes:

- `diags[]` (`x07diag` diagnostics)
- `suggested_patches[]` (multi-file patch targets; each carries RFC 6902 JSON Patch ops)

Optional outputs:

- `--emit-patch <path>` writes `x07.arch.patchset@0.1.0`
- `--write` applies suggested patches and re-runs (final report reflects post-write state)
- `--write-lock` updates `arch/manifest.lock.json` (after `--write` re-run when both are set)

## Implementation locations

- CLI + checker: `crates/x07/src/arch.rs`
- CLI wiring: `crates/x07/src/main.rs`
- CLI tests: `crates/x07/tests/cli.rs`
