# Packages & projects

X07 packages are source-only (x07AST JSON), and projects pin dependencies with a deterministic lockfile.

## Files

- `x07.json` — project definition
- `x07-package.json` — package definition (required only for publishable packages)
- `x07.lock.json` — pinned dependency graph + hashes

## Manifest schema (canonical)

- New and actively maintained projects should use `x07.project@0.3.0`.
- The toolchain still accepts `x07.project@0.2.0` for legacy manifests.
- `project.patch` (transitive dependency overrides) requires `x07.project@0.3.0`.

## Module roots (important)

`x07.json` contains a `module_roots` list. This should list **your source directories** (usually `src` for projects, and `modules` for publishable package repos).

When you run `x07 pkg lock`, dependencies are fetched into `.x07/deps/...` and pinned in `x07.lock.json`. Tooling that accepts `--project` (for example `x07 build --project x07.json` and `x07 run --project x07.json`) automatically adds the locked dependency module roots from the lockfile.

Do not add `.x07/deps/*/modules` paths to `module_roots` manually.

If you see an `unknown module` compile error and you don’t know which package provides it, use:

- `x07 pkg provides <module-id>`

## Local path dependencies

`project.dependencies[*].path` supports two modes:

1) **Registry-vendored**: paths under `.x07/deps/...`

- Canonical for published packages.
- `x07 pkg lock` fetches missing versions into `.x07/deps/...`.
- When index metadata is available, `x07 pkg lock --check` also fails on yanks/advisories unless explicitly allowed.

2) **Local-only**: any other relative path

- Intended for in-repo packages or vendored/unpublished deps.
- The dependency must exist on disk (lock fails with `X07PKG_LOCAL_MISSING_DEP` when missing).
- `x07 pkg lock` does not download into these paths and does not require the sparse index for them.

### Workspace-root paths (`$workspace/...`)

For nested projects where `..` segments are rejected, you can use `$workspace/...` in any path field in `x07.json`.

`$workspace/...` resolves relative to the environment variable `X07_WORKSPACE_ROOT` (when set). If `X07_WORKSPACE_ROOT` is not set, tooling attempts to infer the workspace root from the nearest git repository root.

The resolved path is guarded so it cannot escape the chosen workspace root.

Example dependency entry:

```jsonc
{
  "name": "ext-mcp-auth-core",
  "version": "0.1.0",
  "path": "$workspace/packages/ext/x07-ext-mcp-auth-core/0.1.0"
}
```

## Quickstart

```bash
mkdir mypkg
cd mypkg
x07 init

# Browse packages at https://x07.io/packages
# Add the latest non-yanked version from the index:
x07 pkg add NAME --sync

# Or pin explicitly:
x07 pkg versions NAME
x07 pkg add NAME@VERSION --sync

# Remove a dependency:
x07 pkg remove NAME --sync
```

`x07 init` also writes the agent kit (`AGENT.md`, `x07-toolchain.toml`, `.agent/skills/`, and `.agent/docs/`).

If you are creating a publishable package repo, start with `x07 init --package` instead of `x07 init`
(it creates `x07-package.json`, `modules/`, `tests/`, and the same agent kit). Follow the `next_steps` in the init JSON report
for the canonical pack/login/publish workflow.

`x07 pkg lock` downloads registry dependencies into `.x07/deps/…` and writes `x07.lock.json`.
Commit `x07.lock.json` to make builds reproducible.

In CI, use:

- `x07 pkg lock --project x07.json --check`

When the index can be consulted, `--check` also fails on:

- yanked dependencies (allow only with `--allow-yanked`)
- active advisories (allow only with `--allow-advisories`)

If you need to override a transitive dependency (for example to move off a yanked/advised version), use `project.patch` in `x07.json`:

```jsonc
{
  "schema_version": "x07.project@0.3.0",
  "patch": {
    "some-dep": { "version": "1.2.3" }
  }
}
```

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
