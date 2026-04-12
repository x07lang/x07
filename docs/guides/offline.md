# Offline Workflows (Pkg + Registry Mirrors)

This guide covers using `x07 pkg` and `x07 run` in environments without network access (CI sandboxes, offline agents) using a local `file://` sparse index mirror and pre-hydrated `.x07/deps`.

## Canonical Offline Workflow

1. Enable offline mode (either via project-local config or a global env override).

Env override (equivalent to `--offline` for `x07 pkg` commands that support it, and for `x07 run` dependency hydration):

```bash
export X07_OFFLINE=1
```

Project-local config (preferred for CI and reproducible dev shells):

```jsonc
{
  "schema_version": "x07.config@0.1.0",
  "pkg": {
    "registry": "sparse+file:///ABS/PATH/index/",
    "offline": true
  }
}
```

2. Hydrate `.x07/deps` while you still have network access.

```bash
x07 pkg lock --project x07.json
```

3. Run offline (deterministic: no network).

```bash
x07 pkg lock --project x07.json --offline
x07 run --project x07.json --offline --input case.bin
```

4. After a toolchain upgrade, repair incompatible dependencies.

```bash
x07 pkg repair --project x07.json --toolchain current
```

## Expert Notes

### `x07 run --offline` vs runtime networking

`x07 run --offline` (and `X07_OFFLINE=1`) forbids network access when *hydrating project dependencies* (the implicit `x07 pkg lock` step).

It does **not** change the runtime's OS-world networking policy; use `run-os-sandboxed` policies and allow/deny host lists for runtime network controls.

### Path-only projects (no registry needed)

If a project has only local `path` dependencies that are already present on disk, `x07 pkg lock` and `x07 run` do not need to consult the package registry index.

Still prefer `--offline` / `X07_OFFLINE=1` in CI to fail closed if a missing vendored dependency would otherwise trigger a fetch.

Minimal copy-paste template (local path dep + lockfile): `docs/examples/project-multi-module/`.

### Registry Selection

`x07 pkg` selects the registry index URL in this order:

1. CLI: `--registry <URL>` (alias: `--index <URL>`)
2. Env: `X07_PKG_INDEX_URL`
3. Project-local config: `.x07/config.json` or `x07.config.json`
4. Default: the official registry index

Notes:

- `schema_version` may be omitted; when present it must be `x07.config@0.1.0`.
- `x07 pkg` ignores `x07up` config files (`schema_version: "x07up.*"`) to avoid collisions.

### Local Sparse Index Mirrors (`file://`)

Some commands require a local sparse index directory:

- `x07 pkg list` requires a `file://` index (even when online).
- `x07 pkg versions --offline` and `x07 pkg info --offline` require a `file://` index.

Use a trailing slash and an absolute `file:///...` URL:

```bash
x07 pkg list --registry sparse+file:///ABS/PATH/index/
x07 pkg versions std --offline --registry sparse+file:///ABS/PATH/index/
```

### Hydrating `.x07/deps` For Offline Runs

Offline locking and builds require package contents on disk under `.x07/deps/...`.

Typical flow:

1. Online hydration:

   ```bash
   x07 pkg lock --project x07.json --registry sparse+https://registry.x07.io/index/
   ```

2. Offline usage:

   ```bash
   x07 pkg lock --project x07.json --offline
   x07 run --project x07.json --input case.bin
   ```

If a vendored dependency is missing, offline mode fails deterministically with `X07PKG_OFFLINE_MISSING_DEP`.

### Inspect the resolved closure: `x07 pkg tree`

To print a deterministic dependency graph (including module roots inferred from the lockfile):

```bash
x07 pkg tree --project x07.json
```

This command reads `x07.json` + `x07.lock.json` and never consults the registry index.

### Browsing Offline

With a local `file://` index mirror:

```bash
x07 pkg list --offline
x07 pkg versions NAME --offline
x07 pkg info NAME@VERSION --offline
```

`x07 pkg versions --offline` does not allow `--refresh` (because it cannot consult the network).

### Toolchain Upgrades: Repair

When the toolchain upgrades, older locks may refer to package versions whose `meta.x07c_compat` excludes the running compiler.

Use:

```bash
x07 pkg repair --project x07.json --toolchain current
```

Options:

- Add `--offline` to prefer already-cached compatible versions from `.x07/deps`.
- Add `--registry <URL>` to select a specific index.

`x07 pkg repair` rewrites dependency versions (and `project.patch` when needed) and then regenerates `x07.lock.json` deterministically.
