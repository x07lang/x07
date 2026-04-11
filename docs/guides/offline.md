# Offline Workflows (Pkg + Registry Mirrors)

This guide covers using `x07 pkg` in environments without network access (CI sandboxes, offline agents) using a local `file://` sparse index mirror and pre-hydrated `.x07/deps`.

## Canonical Offline Workflow

1. Point the project at a local `file://` sparse index mirror and enable offline mode.

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
x07 run --project x07.json --input case.bin
```

4. After a toolchain upgrade, repair incompatible dependencies.

```bash
x07 pkg repair --project x07.json --toolchain current
```

## Expert Notes

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
