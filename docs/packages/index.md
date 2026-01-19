# Packages & projects

X07 packages are source-only (x07AST JSON), and projects pin dependencies with a deterministic lockfile.

## Files

- `x07.json` — project definition
- `x07-package.json` — package definition
- `x07.lock.json` — pinned dependency graph + hashes

## Module roots (important)

`x07.json` contains a `module_roots` list. This should list **your source directories** (usually just `src`).

When you run `x07 pkg lock`, dependencies are fetched into `.x07/deps/...` and pinned in `x07.lock.json`. Tooling that accepts `--project` (for example `x07c build --project x07.json`, `x07-host-runner --project x07.json`, `x07-os-runner --project x07.json`) automatically adds the locked dependency module roots from the lockfile.

Do not add `.x07/deps/*/modules` paths to `module_roots` manually.

## Quickstart

```bash
mkdir mypkg
cd mypkg
x07 --init

# Browse packages at https://x07.io/packages
x07 pkg add ext-base64-rs@0.1.1
x07 pkg lock --index sparse+https://registry.x07.io/index/
```

`x07 pkg lock` downloads dependencies into `.x07/deps/…` and writes `x07.lock.json`.
Commit `x07.lock.json` to make builds reproducible.

## Deterministic builds

X07’s lockfile ensures:

- exact versions
- exact content hashes
- reproducible builds

## Dependency resolution

- uses a sparse-index-style registry model
- the lockfile is canonical JSON
