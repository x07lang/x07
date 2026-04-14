# Packages & projects

X07 packages are source-only (x07AST JSON), and projects pin dependencies with a deterministic lockfile.

## Files

- `x07.json` — project definition
- `x07-package.json` — package definition (required only for publishable packages)
- `x07.lock.json` — pinned dependency graph + hashes

## Manifest schema (canonical)

- New and actively maintained projects should use `x07.project@0.5.0` (adds `project.compat`).
- The toolchain still accepts `x07.project@0.2.0`, `x07.project@0.3.0`, and `x07.project@0.4.0` for legacy manifests.
- `project.patch` (transitive dependency overrides) works on the current manifest line, and certification-facing projects use the `project.operational_entry_symbol` fields introduced in `x07.project@0.4.0` (and still present in `x07.project@0.5.0`).
- To migrate a legacy manifest to the current line, run `x07 project migrate --write --project x07.json`.

## Compatibility mode (`project.compat`)

`x07.project@0.5.0` projects may pin a compatibility mode:

- `project.compat`: for example `"0.5"` or `"strict"`.
- CLI override: `x07 run --compat ...`, `x07 build --compat ...`, `x07 check --compat ...`, `x07 test --compat ...`.
- Environment override: `X07_COMPAT=...` (useful for agent environments).

## Module roots (important)

`x07.json` contains a `module_roots` list. This should list **your source directories** (usually `src` for projects, and `modules` for publishable package repos).

When you run `x07 pkg lock`, dependencies are fetched into `.x07/deps/...` and pinned in `x07.lock.json`. Tooling that accepts `--project` (for example `x07 build --project x07.json` and `x07 run --project x07.json`) automatically adds the locked dependency module roots from the lockfile.

If a dependency path points at a registry-vendored location under `.x07/deps/...` (including `$workspace/.../.x07/deps/...`) and the package is missing locally, project commands auto-sync it by running the equivalent of `x07 pkg lock --project x07.json` before build/run/test. Use `x07 pkg lock --project x07.json --check` in CI when you want missing deps or lock drift to fail instead of mutating the workspace.

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

# Inspect registry metadata:
x07 info NAME

# Verify registry index signatures:
x07 pkg verify NAME@VERSION

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
  "schema_version": "x07.project@0.5.0",
  "compat": "0.5",
  "patch": {
    "some-dep": { "version": "1.2.3" }
  }
}
```

Some packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` may add those transitive deps to `x07.json` before locking. Treat this as a convenience, not a contract; prefer the capability map and `x07 init --template ...` so the dependency set is explicit.

When fetching is required, `x07 pkg lock` defaults to the official registry index.
Override with `--registry <URL>` (alias: `--index <URL>`), or use a project-local default config file:

- `.x07/config.json`
- `x07.config.json`

Example `x07.config.json`:

```jsonc
{
  "schema_version": "x07.config@0.1.0",
  "pkg": {
    "registry": "sparse+file:///ABS/PATH/index/",
    "offline": true
  }
}
```

Use `--offline` (or `pkg.offline: true`) to forbid network access and reuse only existing `.x07/deps` contents.
See also: `docs/guides/offline.md`.

After upgrading the toolchain, if an old lock references incompatible package versions, run:

- `x07 pkg repair --toolchain current`
- add `--offline` and/or `--registry <URL>` as needed

Lockfile notes:

- Default lock schema is `x07.lock@0.4.0` (includes toolchain identity and registry provenance).
- Use `x07 pkg lock --lock-version 0.3` only when interoperating with an older toolchain that cannot read `x07.lock@0.4.0`.

## Deterministic builds

X07’s lockfile ensures:

- exact versions
- exact content hashes
- reproducible builds

## Dependency resolution

- uses a sparse-index-style registry model
- the lockfile is canonical JSON
