# Tooling

X07 builds and runs native solver artifacts from generated C, so the only external “tooling” requirement beyond Rust/Python is a working C compiler.

## Requirements

- Rust toolchain with `cargo`
- A C compiler available as `cc` (override with `X07_CC`)
- `clang` (required for the Phase H0 C importer: `x07import c`)
- Python 3 (for benchmark/curriculum tooling and repo maintenance scripts)

## Verify

- Full repo gate: `./scripts/ci/check_all.sh`
- Machine-readable gate (JSON report): `./scripts/ci/run.sh pr --strict`
- Tool presence check: `./scripts/ci/check_tools.sh`
- LLM-first contract smoke: `./scripts/ci/check_llm_contracts.sh`
- Skills pack check: `./scripts/ci/check_skills.sh`
- Language guide sync: `./scripts/ci/check_language_guide_sync.sh`
- Stdlib import drift checks (Phase H0):
  - `./scripts/ci/check_x07import_generated.sh`
  - `./scripts/ci/check_x07import_diagnostics_sync.sh`
- Stdlib lockfile checks: `./scripts/ci/check_stdlib_lock.sh`
- External packages lockfile checks: `./scripts/ci/check_external_packages_lock.sh`

## Phase E projects

- See `docs/spec/modules-packages.md` for the project/package/lockfile workflow.
- See `docs/dev/x07-ast.md` for x07AST JSON Patch workflows (`x07 ast`).

## Fixture snapshots (solve-fs)

- Build/update a stable fixture snapshot:
  - `./scripts/build_fixture_fs_tree.sh <fixture_id> <source_dir>`
