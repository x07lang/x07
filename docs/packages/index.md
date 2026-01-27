# Packages & projects

X07 packages are source-only (x07AST JSON), and projects pin dependencies with a deterministic lockfile.

## Files

- `x07.json` — project definition
- `x07-package.json` — package definition (required only for publishable packages)
- `x07.lock.json` — pinned dependency graph + hashes

## Module roots (important)

`x07.json` contains a `module_roots` list. This should list **your source directories** (usually `src` for projects, and `modules` for publishable package repos).

When you run `x07 pkg lock`, dependencies are fetched into `.x07/deps/...` and pinned in `x07.lock.json`. Tooling that accepts `--project` (for example `x07 build --project x07.json` and `x07 run --project x07.json`) automatically adds the locked dependency module roots from the lockfile.

Do not add `.x07/deps/*/modules` paths to `module_roots` manually.

If you see an `unknown module` compile error and you don’t know which package provides it, use:

- `x07 pkg provides <module-id>`

## Quickstart

```bash
mkdir mypkg
cd mypkg
x07 init

# Browse packages at https://x07.io/packages
# Pick NAME@VERSION from the registry catalog (or the capability map for canonical picks).
x07 pkg add NAME@VERSION --sync
```

`x07 init` also writes the agent kit (`AGENT.md`, `x07-toolchain.toml`, `.agent/skills/`, and `.agent/docs/README.md`).

If you are creating a publishable package repo, start with `x07 init --package` instead of `x07 init`
(it creates `x07-package.json`, `modules/`, `tests/`, and the same agent kit). Follow the `next_steps` in the init JSON report
for the canonical pack/login/publish workflow.

`x07 pkg lock` downloads dependencies into `.x07/deps/…` and writes `x07.lock.json`.
Commit `x07.lock.json` to make builds reproducible.

Some packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` may add those transitive deps to `x07.json` before locking. Treat this as a convenience, not a contract; prefer the capability map and `x07 init --template ...` so the dependency set is explicit.

When fetching is required, `x07 pkg lock` defaults to the official registry index.
Override with `--index sparse+https://registry.x07.io/index/`, or use `--offline` to forbid network access.

## Deterministic builds

X07’s lockfile ensures:

- exact versions
- exact content hashes
- reproducible builds

## Dependency resolution

- uses a sparse-index-style registry model
- the lockfile is canonical JSON
