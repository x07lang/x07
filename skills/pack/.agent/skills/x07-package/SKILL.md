---
name: x07-package
description: Manage X07 project dependencies and lockfiles for reproducible builds (lock, publish). Designed for autonomous agents.
metadata:
  short-description: X07 deps + lockfile management
  version: 0.1.0
  kind: docs
---

# x07-package

This skill documents the single canonical workflow for dependency management in X07 using the built-in package manager (`x07 pkg ...`).

## When to use

Use this skill when:
- adding/updating dependencies,
- generating/verifying lockfiles,
- publishing a package to an index/registry.

## Canonical commands

- Create a publishable package repo (for `x07 pkg publish`):
  - `x07 init --package`

- Add a dependency entry to `x07.json` and sync the lockfile:
  - `x07 pkg add <name>@<version> --sync`

- Generate or update a project lockfile:
  - `x07 pkg lock --project x07.json`

- Non-mutating whole-project validation (no emit):
  - `x07 check --project x07.json`

- Reverse-lookup which package provides a module:
  - `x07 pkg provides <module-id>`

- Verify a lockfile is up to date (CI mode):
  - `x07 pkg lock --project x07.json --check`
  - If you intentionally accept risk in CI: add `--allow-yanked` and/or `--allow-advisories`

- Pack a package directory deterministically:
  - `x07 pkg pack --package <dir> --out <out.x07pkg>`

- Login (store credentials for an index):
  - Interactive: `x07 pkg login --index <url>`
  - Non-interactive: `printf '%s' "$X07_TOKEN" | x07 pkg login --index <url> --token-stdin`

- Publish:
  - `x07 pkg publish --package <dir> --index <url>`

## Notes

- Official registry index URL: `sparse+https://registry.x07.io/index/`
- Publishing to the official registry requires non-empty `description` and `docs` in `x07-package.json`.
- The lockfile path is controlled by `x07.json` (`lockfile`) and defaults to `x07.lock.json`.
- When fetching is required, `x07 pkg lock` defaults to the official registry index; override with `--index <url>`.
- Canonical project manifests use `x07.project@0.3.0`. `x07.project@0.2.0` is accepted for legacy manifests, but `project.patch` requires `@0.3.0`.
- In `--check` mode, when the index can be consulted, lock validation also fails on yanked dependencies and active advisories unless explicitly allowed.
- Use `project.patch` in `x07.json` to override transitive dependency versions (for example, moving off yanked/advised versions).
- Official packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` may add and fetch these transitive deps (and update `x07.json`).
- If dependencies are already present on disk, `x07 pkg lock` can run without `--index` using `--offline`.

See also: https://x07lang.org/docs/packages/publishing-by-example/
