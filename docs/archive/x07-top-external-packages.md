# X07 top external packages

This repo keeps “third‑party” functionality in **external packages** under `packages/ext/`.
External packages are pinned and hashed via `locks/external-packages.lock` and validated in CI.
For the production v1 supported set, see `docs/phases/x07-external-packages-v1.md`.

The first wave is intentionally small and targets high‑leverage building blocks:

1. TOML + URL + JSON utilities (pure)
2. X07‑native error patterns (pure)
3. zlib (FFI, OS‑world only)
4. libcurl + OpenSSL (FFI, OS‑world only)
5. sqlite3 (FFI, OS‑world only)

## Determinism tiers

- **pure**: usable in `solve-*` and `run-os*` worlds
- **os-world-only**: usable only in `run-os` / `run-os-sandboxed` (never in deterministic evaluation)

Each external package declares its tier and allowed worlds in `x07-package.json`.

## Naming: package origin suffixes

X07 distinguishes **package names** (dependency selection) from **module IDs** (what code imports).

### Package names / directories (show origin)

- x07import‑from‑Rust packages use `-rs`
  - directory: `packages/ext/x07-ext-<name>-rs/0.1.0/`
  - package name: `ext-<name>-rs`
- C/FFI packages use `-c`
  - directory: `packages/ext/x07-ext-<name>-c/0.1.0/`
  - package name: `ext-<name>-c`
- Handwritten X07 external packages have no suffix
  - directory: `packages/ext/x07-ext-<name>/0.1.0/`
  - package name: `ext-<name>`

### Module IDs (leave canonical)

Module IDs stay canonical under `ext.<name>...` (no `-rs` / `-c` suffix).
This keeps imports stable and avoids baking implementation details into the namespace.

Special case for JSON: if/when we import an API directly derived from `serde_json`, use the
module namespace `ext.serde-json-rs.*` (so it’s distinct from `ext.json.*` JSON utilities).

## Package structure

Every external package is versioned and self‑contained:

```
packages/ext/x07-ext-<name>(-rs|-c)/0.1.0/
  x07-package.json
  modules/
    <module_id_path>.x07.json
  tests/
    tests.json
  ffi/                      # only for -c packages
    *_shim.c
```

## Implemented packages (in this repo)

### Pure (solve-* capable)

- `ext-log` (`packages/ext/x07-ext-log/0.1.0/`) — `ext.log`
- `ext-tracing` (`packages/ext/x07-ext-tracing/0.1.0/`) — `ext.tracing`
- `ext-streams` (`packages/ext/x07-ext-streams/0.1.0/`) — `ext.streams`
- `ext-error` (`packages/ext/x07-ext-error/0.1.0/`)
  - modules: `ext.error.context`, `ext.error.chain`, `ext.error.fmt`
- `ext-data-model` (`packages/ext/x07-ext-data-model/0.1.0/`)
  - modules: `ext.data_model`, `ext.data_model.json`, `ext.data_model.toml`, `ext.data_model.yaml`
  - `ext.data_model.json.emit_canon` — emit canonical JSON bytes from an `ext.data_model` doc
  - `ext.data_model.toml.emit_canon` — emit canonical TOML bytes from an `ext.data_model` doc (strict subset)
  - `ext.data_model.yaml.emit_canon` — emit canonical YAML bytes from an `ext.data_model` doc (YAML 1.2 JSON subset)
- `ext-url-rs` (`packages/ext/x07-ext-url-rs/0.1.0/`)
  - modules: `ext.url.parse`, `ext.url.encode`, `ext.http_types`, `ext.httparse`
- `ext-toml-rs` (`packages/ext/x07-ext-toml-rs/0.1.0/`)
  - modules: `ext.toml`, `ext.toml.data_model`
- `ext-ini-rs` (`packages/ext/x07-ext-ini-rs/0.1.0/`)
  - modules: `ext.ini`, `ext.ini.data_model`
- `ext-json-rs` (`packages/ext/x07-ext-json-rs/0.1.0/`)
  - modules: `ext.json.pointer`, `ext.json.canon`, `ext.json.data_model`
  - note: complements `std.json` (stdlib) with JSON Pointer + a different error surface
- `ext-regex` (`packages/ext/x07-ext-regex/0.2.0/`) — `ext.regex` — native backend (no fallback); contract: `docs/text/regex-v1.md` (details: `docs/phases/x07-regex-replacement.md`)
- `ext-memchr-rs` (`packages/ext/x07-ext-memchr-rs/0.1.0/`) — `ext.memchr` — X07IMPORT — fast byte search primitives
- `ext-aho-corasick-rs` (`packages/ext/x07-ext-aho-corasick-rs/0.1.0/`) — `ext.aho_corasick` — X07IMPORT — fast multi-needle find
- `ext-yaml-rs` (`packages/ext/x07-ext-yaml-rs/0.1.0/`)
  - modules: `ext.yaml`, `ext.yaml.data_model`
- `ext-csv-rs` (`packages/ext/x07-ext-csv-rs/0.1.0/`)
  - modules: `ext.csv`, `ext.csv.data_model`
- `ext-xml-rs` (`packages/ext/x07-ext-xml-rs/0.1.0/`)
  - modules: `ext.xml`, `ext.xml.data_model`
- `ext-semver-rs` (`packages/ext/x07-ext-semver-rs/0.1.0/`)
  - modules: `ext.semver`
- `ext-uuid-rs` (`packages/ext/x07-ext-uuid-rs/0.1.0/`)
  - modules: `ext.uuid`
- `ext-base64-rs` (`packages/ext/x07-ext-base64-rs/0.1.0/`)
  - modules: `ext.base64`
- `ext-hex-rs` (`packages/ext/x07-ext-hex-rs/0.1.0/`)
  - modules: `ext.hex`
- `ext-byteorder-rs` (`packages/ext/x07-ext-byteorder-rs/0.1.0/`)
  - modules: `ext.byteorder`

Note: some “framework-named” packages are intentionally small helpers rather than full ports.
In particular, yielding ops like `task.join.bytes` and `chan.bytes.{send,recv}` are only allowed in `solve` / `defasync`; for non-yielding task/channel ops usable inside `defn`, use `task.is_finished`, `task.try_join.bytes`, `chan.bytes.try_send`, and `chan.bytes.try_recv`.

### OS-world only (run-os / run-os-sandboxed)

- `ext-zlib-c` (`packages/ext/x07-ext-zlib-c/0.1.0/`) — `ext.zlib`
- `ext-openssl-c` (`packages/ext/x07-ext-openssl-c/0.1.0/`) — `ext.openssl.hash`
- `ext-curl-c` (`packages/ext/x07-ext-curl-c/0.1.0/`) — `ext.curl.http`

## Regeneration and verification

### x07import (Rust → x07AST JSON)

- Sources live in `import_sources/rust/` and are wired in `import_sources/manifest.json`.
- CI gate: `./scripts/ci/check_x07import_generated.sh`

### Lockfile (pinned external package hashes)

- Generate/check: `python3 scripts/generate_external_packages_lock.py --check`
- CI gate: `./scripts/ci/check_external_packages_lock.sh`

### OS smoke (FFI packages)

- CI gate: `./scripts/ci/check_external_packages_os_smoke.sh`
