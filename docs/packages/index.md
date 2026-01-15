# Packages & projects

X07 packages are source-only (x07AST JSON), and projects pin dependencies with a deterministic lockfile.

## Files

- `x07.json` — project definition
- `x07-package.json` — package definition
- `x07.lock.json` — pinned dependency graph + hashes

## Deterministic builds

X07’s lockfile ensures:

- exact versions
- exact content hashes
- reproducible builds

## Dependency resolution

- uses a sparse-index-style registry model
- the lockfile is canonical JSON
