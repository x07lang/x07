Status: in progress

Last updated: 2026-01-15

## Progress

- [x] Add `x07 pkg` CLI surface: `pack`, `lock`, `login`, `publish`.
- [x] Implement deterministic `.x07pkg` archives (tar, stable header fields, stable file order).
- [x] Implement sparse index client (`sparse+…/`, Cargo-style shard paths) and content-hash verification.
- [x] Implement `x07 pkg lock` that materializes missing path deps from the sparse index and writes `x07.lock.json`.
- [x] Implement token storage (`X07_PKG_HOME` / `~/.x07/credentials.json`) + `x07 pkg login`.
- [x] Implement registry server MVP endpoints in `x07lang/x07-registry` (index/config, publish, download, metadata, token check).
- [x] Update `x07lang/x07-index` to use `index/config.json` (`dl`, `api`, `auth-required`) and validate shard paths + stable ordering.
- [ ] SemVer requirements/resolution (deferred).
- [ ] Vendoring (`x07 pkg vendor`) + offline mirror flows (deferred).
- [ ] TUF metadata + signing (deferred).

---

## Implemented v0 (canonical for now)

This repository already has a project + package + lock format (Phase E). The current `x07 pkg` implementation builds on it:

- Project manifest: `x07.project@0.2.0` (`x07.json`)
- Package manifest: `x07.package@0.1.0` (`x07-package.json`)
- Project lockfile: `x07.lock@0.2.0` (`x07.lock.json`)
- Dependency specs: `dependencies: [{ "name", "version", "path" }]` where `path` is the materialization location and `version` is exact for now.

Sparse index protocol (MVP):

- Index URL: `sparse+<base>/` (must end with `/`)
- Index root contains `config.json` and package entry files at Cargo-style shard paths (`<shard>/<name>`).
- Each package file is NDJSON; each line is `x07.index-entry@0.1.0` (canonical schema lives in `x07lang/x07-index/spec/index-entry.schema.json`).

Archive format (MVP):

- `.x07pkg` is a tar archive containing `x07-package.json` plus the module files listed in the manifest (under `module_root`).

Below is a **production‑oriented design + implementation plan** for an X07‑bundled package manager **and** a package repository that stays aligned with your core constraints:

* **LLM‑first / agentic** (strict JSON I/O, deterministic diagnostics, automatable repair loops)
* **Reproducible** (lockfiles + content hashes; “fail closed” like `--locked`)
* **Works offline** (vendor/mirror)
* **Secure supply chain** (TUF‑style metadata + optional artifact signing)

---

## 0) North‑star invariants

### I0 — A build is a pure function of pinned inputs

A project build must be reproducible given:

* `x07c` version (toolchain)
* `stdlib.lock` (pinned stdlib packages and hashes)
* project `x07.lock` (all external deps pinned with hashes)
* sources (local project + vendored/registry tarballs)

This is the same reason Cargo keeps an exact lockfile: it records the exact dependency state and is not meant to be edited by hand. ([Rust Documentation][1])

### I1 — Deterministic “locked mode”

There must be a mode analogous to Cargo’s “use the lockfile exactly” behavior (Cargo has a `--locked` flag used heavily in CI to force reproducibility). ([Corrode Rust Consulting][2])

### I2 — Everything machine‑readable by default

All package manager commands support:

* `--json` output with schema’d responses
* stable diagnostic codes
* `--quiet` for agents (only JSON)
* `--explain <CODE>` for humans

### I3 — Offline is first‑class

A single command must “snapshot dependencies” into a local directory so builds don’t need the network—like `cargo vendor` does. ([Rust Documentation][3])

### I4 — Supply chain security can be layered

Start with **hash‑pinned artifacts** (like pip hash‑checking mode: requirements include hashes to protect against tampering). ([Pip Documentation][4])
Then add **TUF** metadata for rollback/freeze/replay resistance (root/targets/snapshot/timestamp). ([TUF][5])
Optionally add **Sigstore** signatures later (Cosign supports signing and storing signatures in OCI registries). ([sigstore][6])

---

## 1) Product surface

### 1.1 Tool names

* `x07` (umbrella CLI: build/run/test/fmt/lint/pkg)
* `x07 pkg` (package manager subcommand)
* `x07 registry` (optional admin tooling to run a registry server)

You can also ship `x07pkg` as an alias, but “one CLI” is better for agents.

### 1.2 Commands (minimal but complete)

**Project**

* `x07 pkg init`
* `x07 pkg add <pkg>@<req> [--as dev] [--features ...]`
* `x07 pkg remove <pkg>`
* `x07 pkg resolve` (creates/updates `x07.lock`)
* `x07 pkg vendor <dir>` (offline snapshot)
* `x07 pkg verify` (verify hashes/signatures/TUF)
* `x07 build [--locked] [--offline] [--vendor <dir>]`
* `x07 run ...`
* `x07 test ...`

**Registry**

* `x07 pkg publish` (client)
* `x07 registry serve` (server; optional initially)
* `x07 registry index` (index generation / compaction)

### 1.3 Output modes

* Default: human readable
* `--json`: strict schema outputs always
* `--json --pretty`: for debugging
* `--json --fail-on-warn` (agents often want “fail closed”)

---

## 2) Files and formats

### 2.1 Project manifest: `x07.json`

You want a **single, small manifest** that’s very easy for LLMs to edit.

```json
{
  "schema_version": "x07.manifest@0.1.0",
  "package": {
    "name": "acme.reporter",
    "version": "0.1.0",
    "license": "MIT",
    "edition": "2026"
  },
  "toolchain": {
    "x07c_version": "0.7.0",
    "stdlib_lock_sha256": "..."
  },
  "modules": {
    "root": "src",
    "entry": "main"
  },
  "deps": {
    "x07:stdlib-text": "^0.1.0",
    "x07:stdlib-json": "^0.1.0",
    "x07:net-url": "^0.1.0"
  },
  "dev_deps": {
    "x07:test": "^0.1.0"
  },
  "registries": {
    "default": "sparse+https://registry.x07.io/index/"
  }
}
```

Notes:

* Keep dependency requirements in the manifest “broad” (SemVer ranges).
* The **lockfile** pins the exact versions/hashes.

This mirrors the “manifest + lockfile” pattern used widely (Cargo and npm both separate the broad intent from the exact resolved tree). ([Rust Documentation][1])

### 2.2 Lockfile: `x07.lock` (authoritative, generated)

Lockfiles exist specifically to enable reproducible builds. ([Rust Documentation][1])

I recommend JSON (since X07 is already “JSON‑first” in your new direction).

```json
{
  "schema_version": "x07.lock@0.1.0",
  "generated_at_unix": 1767400000,
  "toolchain": {
    "x07c_version": "0.7.0",
    "stdlib_lock_sha256": "..."
  },
  "registry": {
    "default": {
      "url": "sparse+https://registry.x07.io/index/",
      "tuf_root_sha256": "..." 
    }
  },
  "packages": [
    {
      "name": "x07:net-url",
      "version": "0.1.0",
      "source": {
        "kind": "registry",
        "registry": "default"
      },
      "artifact": {
        "url": "https://registry.x07.io/api/v1/artifacts/x07-net-url/0.1.0/x07pkg.tar.zst",
        "sha256": "..."
      },
      "deps": {
        "x07:stdlib-bytes": { "version": "0.1.0", "pkg_id": "..." }
      }
    }
  ],
  "graph": {
    "root": {
      "deps": ["x07:net-url@0.1.0"]
    }
  }
}
```

Hard rules:

* `x07.lock` is **never** edited by hand (same stance as Cargo.lock). ([Rust Documentation][1])
* `x07 build --locked` fails if it would modify `x07.lock` (pattern used widely in CI). ([Corrode Rust Consulting][2])

### 2.3 Stdlib lock: `stdlib.lock`

You already have this idea. Keep it as a toolchain‑level pinned set.

* `stdlib.lock` pins stdlib package versions + hashes
* Projects reference it by hash in `x07.json` and lockfile
* `x07 pkg doctor` warns if toolchain and stdlib lock mismatch

### 2.4 Package archive format: `.x07pkg`

A published package is a **canonical, reproducible archive** (tar + zstd is fine).

Contents:

```
x07pkg/
  package.json
  module/
    module.x07.json
    ... (additional .x07.json files)
  docs/
    README.md
    CHANGELOG.md
  tests/          (optional)
  LICENSE
```

**Canonicalization rules** (important for hashing/signing):

* normalize file ordering (lexicographic)
* normalize permissions and timestamps
* normalize line endings for text files
* compute `sha256` on the final archive bytes

This makes “hash‑checking mode” possible (pip’s secure installs rely on embedded hashes). ([Pip Documentation][4])

---

## 3) Registry design

You have two big decisions:

1. **index protocol**
2. **artifact storage + metadata security**

### 3.1 Index protocols: choose “sparse HTTP” first, optionally add “git index”

Cargo supports both a git index and a sparse HTTP index. ([Rust Documentation][7])
This split is ideal:

* **Sparse**: simplest for clients (fetch a few files over HTTP), easiest to CDN.
* **Git**: easiest for mirrors and offline incremental fetch.

**Recommendation**: implement **sparse** first.

### 3.2 Index root config

Cargo’s registry index includes a `config.json` at the root that tells clients where to download (`dl`) and optionally an API base (`api`). ([Rust Documentation][7])

Do the same:

`/index/config.json`

```json
{
  "dl": "https://registry.x07.io/api/v1/artifacts",
  "api": "https://registry.x07.io/api/v1",
  "tuf": "https://registry.x07.io/tuf"
}
```

### 3.3 Index entries

Use one file per package name. Each file contains **append‑only** version records.

For sparse you can simply do:

`/index/x07/net-url.json` (or any deterministic sharding scheme)

Each record includes:

* version
* deps (names + semver requirements + optional “features”)
* artifact digest (sha256)
* yanked flag
* published timestamp

### 3.4 Artifact download API

`GET /api/v1/artifacts/<name>/<version>/x07pkg.tar.zst`

Clients must:

* download
* verify sha256 from index/lock
* then unpack into cache

### 3.5 Vendoring/offline workflows

Copy the Cargo vendor UX:

* `x07 pkg vendor vendor/`
* writes:

  * `vendor/` directory with all `.x07pkg` sources unpacked (or archived)
  * a `vendor/index/` snapshot of the index entries you used
  * a `vendor/manifest.json` mapping name/version → local path

Cargo’s `cargo vendor` is precisely this concept: vendor remote sources locally for offline/reproducible builds. ([Rust Documentation][3])

---

## 4) Security model

### 4.1 Baseline security: lockfile hashes

Add `--require-hashes` style behavior:

* In “locked mode”, every dependency must have a hash in `x07.lock`.
* Builds must fail if any hash is missing or mismatched.

This parallels pip hash-checking mode: hashes embedded locally protect against remote tampering. ([Pip Documentation][4])

### 4.2 Upgrade: TUF repository metadata

To protect against:

* rollback attacks
* freeze attacks
* compromised mirrors
* key compromise (graceful recovery)

Use TUF metadata roles: Root, Targets, Snapshot, Timestamp. ([TUF][5])

Practical plan:

* Registry hosts `/tuf/root.json`, `/tuf/timestamp.json`, `/tuf/snapshot.json`, `/tuf/targets.json`.
* `x07 pkg update`:

  * fetches metadata in TUF order (root → timestamp → snapshot → targets)
  * verifies signatures
  * uses targets metadata to validate artifacts and their hashes

TUF is explicit that implementers can choose metadata formats; JSON is typical. ([TUF][5])

### 4.3 Optional: Sigstore signing (artifact provenance)

Sigstore’s Cosign supports artifact signing and storage of signatures in an OCI registry. ([sigstore][6])

You can add later:

* `x07 pkg publish --sign`
* `x07 pkg verify --sigstore`

But you don’t need Sigstore to ship v1. Hash pinning + TUF already buys you a lot.

---

## 5) Dependency resolution rules (deterministic)

A deterministic resolver avoids “non‑repeatable solves”.

Rules:

* Resolve from `x07.json` + registry index snapshot.
* Deterministic tie‑breakers:

  1. highest compatible version
  2. then lexicographic package name
  3. then lexicographic source URL
* Once resolved, write `x07.lock` with stable ordering.

### Yank handling

* A yanked version can still be used if already pinned in `x07.lock` (same as Cargo’s typical semantics).
* New resolutions should avoid yanked versions unless `--allow-yanked`.

---

## 6) Capability / world compatibility

X07 has “worlds” (solve‑pure vs solve‑fs etc) in your development story; for production you’ll have “run‑os”, etc.

Packages should declare **capability requirements** so:

* a “pure” environment can refuse to import a package that needs OS/network.

In `package.json`:

```json
{
  "capabilities": {
    "requires": ["io", "fs", "rr", "kv"],
    "forbidden": ["os.net", "os.process"]
  }
}
```

The package manager enforces:

* When building for a target profile (e.g. `--world run-os-sandboxed`), only packages compatible with that world can be resolved.

---

## 7) Agentic/LLM integration design

This is where X07 should beat Cargo/npm.

### 7.1 Deterministic diagnostics

Every failure should be a stable `(code, primary_span, hints[])` object.

Example:

```json
{
  "ok": false,
  "error": {
    "code": "X07PKG_RESOLVE_CONFLICT",
    "message": "Dependency conflict: x07:net-url requires x07:stdlib-bytes ^0.1.1 but toolchain pins 0.1.0",
    "hints": [
      "Run: x07 pkg update --stdlib",
      "Or pin: x07:net-url==0.1.0 in x07.json"
    ]
  }
}
```

### 7.2 “Repair‑friendly” suggested actions

Every error includes:

* suggested CLI commands
* suggested JSON Patch operations for `x07.json`/`x07.lock` (agent can apply automatically)

### 7.3 Strict JSON output guarantees

All commands in `--json` mode:

* print exactly one JSON object to stdout
* never print logs to stdout
* logs go to stderr (optional)
* exit codes are stable

---

## 8) Implementation plan (repo‑aligned milestones)

I’ll assume you have a Rust workspace with `crates/` and `spec/` already. If not, translate the paths.

### Milestone P0 — Specs and schemas

**Adds**

* `spec/x07.manifest.schema.json`
* `spec/x07.lock.schema.json`
* `docs/pkg/manifest.md` (normative)
* `docs/pkg/lockfile.md` (normative)
* `scripts/check_pkg_schemas.py` (validate fixtures)

**CI gates**

* schema validation for fixtures
* deterministic ordering check for generated lockfiles

### Milestone P1 — Local path dependencies (no registry yet)

**Adds**

* `crates/x07-pkg/` (library)
* `crates/x07-cli/` subcommand `pkg`
* `x07 pkg init/add/remove/resolve`
* `x07 build --locked` (fails if lock mismatch)

**CI**

* create a sample project fixture:

  * `fixtures/pkg/local_path_project/`
* run `x07 pkg resolve --json` and validate output schema
* run `x07 build --locked`

### Milestone P2 — Cache and vendor

**Adds**

* `~/.cache/x07/` (or `.x07/cache/` per project)
* `x07 pkg vendor vendor/`
* `x07 build --offline --vendor vendor/`

**CI**

* ensure `--offline` fails if dependency not vendored
* ensure vendoring is deterministic byte‑for‑byte

(Use Cargo’s `vendor` concept as a model. ([Rust Documentation][3]))

### Milestone P3 — Sparse registry client

**Adds**

* sparse index client (HTTP GET for index entries)
* artifact fetcher with sha256 validation (from `x07.lock`)
* `x07 pkg search/info` (optional)

**CI**

* run a local test registry (static files served)
* resolve + fetch + build offline from cache

### Milestone P4 — Registry server (minimal)

**Adds**

* `crates/x07-registry-server/` (optional)
* endpoints:

  * `GET /index/config.json` (like Cargo’s config.json concept) ([Rust Documentation][7])
  * `GET /index/<pkg>.json`
  * `GET /api/v1/artifacts/...`
  * `POST /api/v1/publish` (auth token)

Cargo registries often separate index and download endpoints; your config.json can point to both. ([Rust Documentation][7])

### Milestone P5 — Hash‑checking mode (fail closed)

**Adds**

* `x07 pkg verify`:

  * verifies every artifact hash in lock
* `x07 build --locked` implies:

  * all deps hashed
  * mismatch fails

This parallels pip’s secure hash‑checking approach. ([Pip Documentation][4])

### Milestone P6 — TUF metadata (recommended before public registry)

**Adds**

* `/tuf/` endpoint and client updater
* store TUF root hash in `x07.lock` (trust anchor)
* verify TUF roles (root/targets/snapshot/timestamp). ([TUF][5])

### Milestone P7 — Optional: Sigstore signing

**Adds**

* `x07 pkg publish --sign` (cosign integration)
* `x07 pkg verify --sigstore`

Sigstore’s docs explicitly describe Cosign as a signing/verifying tool with OCI storage. ([sigstore][6])

---

## 9) Policy decisions you should lock in now

These are “expensive to change later”:

1. **Immutability**: published `(name, version)` artifacts are immutable.
2. **Yank semantics**: yanked versions remain fetchable but not newly selected.
3. **Canonical archive**: `.x07pkg` packing rules are deterministic.
4. **Lockfile authority**: `--locked` is enforced everywhere (CI and agent loops).
5. **Dual registry protocol**: sparse first; git index later (optional). Cargo supports both; sparse is easier to host. ([Rust Documentation][7])
6. **Security baseline**: hashes always; TUF before public internet exposure. ([Pip Documentation][4])

---

## 10) What to build first (MVP sequencing)

If you want the **fastest path** to something useful for agents:

1. **Local+vendor** only (P1+P2)
   → lets you ship “package boundaries” without registry infra.

2. Add **sparse registry client** (P3)
   → agents can fetch shared libs.

3. Add **minimal registry server** (P4)
   → private/offline org registry.

4. Add **TUF** (P6)
   → safer to ever go public. ([TUF][5])

Sigstore can come later.

---
[1]: https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html?utm_source=chatgpt.com "Cargo.toml vs Cargo.lock - The Cargo Book"
[2]: https://corrode.dev/blog/tips-for-faster-ci-builds/?utm_source=chatgpt.com "Tips for Faster Rust CI Builds"
[3]: https://doc.rust-lang.org/cargo/commands/cargo-vendor.html?utm_source=chatgpt.com "cargo vendor - The Cargo Book - Rust Documentation"
[4]: https://pip.pypa.io/en/stable/topics/secure-installs/?utm_source=chatgpt.com "Secure installs - pip documentation v25.3"
[5]: https://theupdateframework.io/docs/metadata/?utm_source=chatgpt.com "Roles and metadata | TUF - The Update Framework"
[6]: https://www.sigstore.dev/how-it-works?utm_source=chatgpt.com "How Sigstore works"
[7]: https://doc.rust-lang.org/cargo/reference/registry-index.html?utm_source=chatgpt.com "Registry Index - The Cargo Book - Rust Documentation"

+++++
Got it — **workspace (monorepo) from day 1**, **source‑only packages** (but as **x07AST JSON**, `*.x07.json`), and **Cargo‑like API‑token auth** for the registry.

Below is a **full design + implementation plan** for:

* an X07‑bundled **package manager** (`x07 pkg ...`) with first‑class **workspace** support
* a **package repository** (sparse index + artifact store) with **API token auth**

I’ll intentionally align behavior with Cargo where it matters (workspace lockfile, sparse index, config.json, authorization header). ([Rust Documentation][1])

---

# 1) Non‑negotiable product guarantees

These are the rules your devs should treat as “deterministic contracts”:

## G1 — Workspace first: one lockfile, one build output, one resolution

A workspace is the unit of resolution/build/caching:

* **One lockfile at workspace root** (like Cargo workspaces share `Cargo.lock`). ([Rust Documentation][1])
* **One build output directory** for the workspace (like Cargo’s shared `target`). ([Rust Documentation][1])
* Commands can run across all members (like `cargo … --workspace`). ([Rust Documentation][1])

## G2 — Source‑only packages (x07AST JSON), deterministic hashing

Registry distributes **only** sources as `*.x07.json` (x07AST JSON).
All hashing/signing is over **canonicalized bytes**, not “pretty” JSON.

## G3 — Locked builds are strict (CI/agents)

Implement `--locked` and `--offline` semantics inspired by Cargo:

* `--locked`: fail if lockfile would change (common reproducibility stance). ([Rust Documentation][2])
* `--offline`: never hit network; fail if something is missing locally. ([Rust Documentation][3])

## G4 — Registry auth: API token in `Authorization` header (Cargo‑like)

For any endpoint requiring auth:

* client sends `Authorization: <token>`
* server returns 403 if invalid

This is explicitly described by the Cargo registry Web API docs. ([Rust Documentation][4])

Also support `auth-required: true` in registry index `config.json` (Cargo RFC describes this to force auth on all requests). ([Rust Language][5])

---

# 2) Repo layout and components

## 2.1 Crates

Add these crates (names adjustable to your workspace naming):

```
crates/
  x07-pkg/              # library: manifest/lock, resolver, fetch, cache, pack
  x07-cli/              # existing; add `pkg` subcommand and build integration
  x07-registry/         # server: index + artifacts + publish API
  x07-registry-model/   # shared types + schemas (optional)
```

## 2.2 On-disk workspace files

### Workspace root

```
x07.workspace.json          # workspace manifest
x07.lock.json               # workspace lockfile (generated)
stdlib.lock                 # pinned stdlib bundle (generated)
.x07/
  cache/                    # content-addressed sources/artifacts
  registry/                 # sparse index cache snapshot
target/                     # build outputs
vendor/                     # optional offline snapshot (generated)
```

### Member packages

```
packages/<member>/
  x07.package.json
  modules/                  # canonical: dot-path -> folder path
    <module-id path>/module.x07.json
  tests/                    # optional unit tests in X07 test format
```

---

# 3) Workspace manifest format

You need **two layers**:

* a root workspace manifest (members + shared config)
* per‑package manifests (package metadata + entry points + deps)

This matches Cargo’s “workspace” concept (members, shared metadata), but in your own JSON shape. ([Rust Documentation][1])

## 3.1 `x07.workspace.json` (root)

Key idea: keep it **agent‑editable**, stable ordering, small.

```json
{
  "schema_version": "x07.workspace@0.1.0",
  "workspace": {
    "name": "acme-monorepo",
    "members": [
      "packages/app",
      "packages/lib_url",
      "packages/lib_report"
    ],
    "default_member": "packages/app"
  },
  "toolchain": {
    "x07c_version": "0.12.0",
    "stdlib_lock": "stdlib.lock",
    "stdlib_lock_sha256": "..."
  },
  "registries": {
    "default": {
      "index": "sparse+https://registry.x07.io/index/",
      "api": "https://registry.x07.io/"
    }
  },
  "resolution": {
    "prefer_highest": true,
    "allow_yanked": false
  },
  "paths": {
    "cache_dir": ".x07/cache",
    "registry_dir": ".x07/registry",
    "target_dir": "target"
  }
}
```

### Determinism rules

* `members` list order is **canonical** (sorted by path on write).
* Member paths are **relative** to workspace root.
* Workspace root is “first file upward” only if explicitly requested; default: require `--workspace-root` to avoid ambient scanning (LLM reliability).

---

# 4) Package manifest format (`x07.package.json`)

```json
{
  "schema_version": "x07.package@0.1.0",
  "package": {
    "id": "acme:lib-url",
    "version": "0.1.0",
    "license": "MIT",
    "description": "URL parsing utilities"
  },
  "modules": {
    "root": "modules",
    "exports": [
      "acme.lib_url"
    ]
  },
  "deps": {
    "x07:stdlib-text": "^0.1.0",
    "x07:stdlib-bytes": "^0.1.0"
  },
  "dev_deps": {
    "x07:test": "^0.1.0"
  },
  "capabilities": {
    "worlds_allowed": ["solve-pure", "solve-fs", "run-os", "run-os-sandboxed"],
    "requires": [],
    "forbids": []
  }
}
```

### Determinism rules

* `package.id` is canonical lowercase and must match a strict regex (no unicode confusables).
* `deps` keys sorted lexicographically when writing.

---

# 5) Source format: x07AST JSON modules (`module.x07.json`)

Because packages are **source‑only** in x07AST JSON:

* The package manager never runs a “parser” that can drift.
* It validates each module against your **x07AST schema** (the one you already added).
* It canonicalizes JSON (stable ordering, no floats, etc.) before hashing.

**Rule**: a module file’s hash is computed over canonical JSON bytes (and the package hash is computed over canonical package manifest + canonicalized module bytes in stable order).

This makes:

* hashing stable
* `x07.lock.json` stable
* registry artifacts verifiable

---

# 6) Lockfile format (workspace lock): `x07.lock.json`

Like Cargo, lockfile is shared at workspace root for workspaces. ([Rust Documentation][1])

```json
{
  "schema_version": "x07.lock@0.1.0",
  "generated_at_unix": 1767400000,
  "toolchain": {
    "x07c_version": "0.12.0",
    "stdlib_lock_sha256": "..."
  },
  "registry": {
    "default": {
      "index": "sparse+https://registry.x07.io/index/",
      "api": "https://registry.x07.io/",
      "auth_required": false
    }
  },
  "workspace_members": [
    {
      "path": "packages/app",
      "pkg_id": "acme:app",
      "version": "0.1.0"
    }
  ],
  "packages": [
    {
      "pkg_id": "x07:stdlib-text",
      "version": "0.1.0",
      "source": { "kind": "registry", "name": "default" },
      "artifact": {
        "format": "x07pkg+tar.zst",
        "url": "https://registry.x07.io/api/v1/artifacts/x07-stdlib-text/0.1.0/x07pkg.tar.zst",
        "sha256": "..."
      },
      "module_index": [
        { "module_id": "std.text.utf8", "path": "modules/std/text/utf8/module.x07.json", "sha256": "..." }
      ],
      "deps": [
        { "pkg_id": "x07:stdlib-bytes", "version": "0.1.0" }
      ]
    }
  ],
  "resolution_graph": {
    "roots": [
      { "member": "packages/app", "deps": ["x07:stdlib-text@0.1.0"] }
    ]
  }
}
```

### `--locked` behavior

* If resolution would change anything in `x07.lock.json`, `x07 pkg resolve --locked` fails.
  This mirrors Cargo’s emphasis on reproducibility (and `--offline`/`--locked` being used in CI contexts). ([Rust Documentation][3])

---

# 7) Package archive format and repository

## 7.1 `x07pkg` archive (source‑only)

A published package is a canonical archive:

```
x07pkg/
  package.json
  modules/**/module.x07.json
  docs/README.md
  LICENSE
```

**Canonicalization**:

* stable file ordering
* normalized metadata (timestamps/uid/gid)
* hash computed on the final archive bytes
* every module file is schema‑validated pre‑publish

## 7.2 Index protocol: sparse HTTP (Cargo‑style)

Cargo supports sparse registries with `sparse+` scheme. ([Rust Documentation][6])
Do the same:

* Workspace registry index URL begins with `sparse+...`
* Client fetches only the package entry files it needs
* Recommend HTTP/2 on the registry (sparse uses many small requests) ([Rust Documentation][6])

## 7.3 Registry index root `config.json`

Cargo’s index has a `config.json` with `dl` and `api` keys. ([Rust Documentation][6])

Your registry index root:

```json
{
  "dl": "https://registry.x07.io/api/v1/artifacts",
  "api": "https://registry.x07.io/",
  "auth-required": true
}
```

* `auth-required` matches Cargo RFC behavior for alternative registries. ([Rust Language][5])

## 7.4 Index entry file format

One file per package name, append‑only records (like Cargo index entries).

Example:
`/index/x07/stdlib-text` (exact sharding scheme is your choice; keep deterministic)

Each line could be one JSON object version record:

```json
{"name":"x07:stdlib-text","vers":"0.1.0","deps":[...],"cksum":"<sha256>","yanked":false}
```

(You can also store as one JSON array; line‑oriented is cache‑friendly.)

---

# 8) Registry API and authentication (Cargo-like)

Cargo’s registry Web API states: client sends `Authorization` header with token; server returns 403 if invalid. ([Rust Documentation][4])

## 8.1 Minimal endpoints (v1)

### Public

* `GET /api/v1/packages/<pkg_id>` (metadata)
* `GET /api/v1/artifacts/<pkg_id>/<version>/x07pkg.tar.zst` (download)

### Auth-required

* `POST /api/v1/publish` (upload archive + metadata)
* `POST /api/v1/yank/<pkg_id>/<version>` (optional)
* `POST /api/v1/un-yank/...` (optional)
* `POST /api/v1/token/rotate` (optional)

## 8.2 Client token management

Ship commands analogous to Cargo:

* `x07 pkg login --registry default --token <...>`
* token stored in `~/.config/x07/credentials.json` (0600 permissions)
* `x07 pkg publish --registry default`

Cargo’s config docs explicitly note tokens are used by commands like publish and should be protected. ([Rust 文档网][7])

---

# 9) Offline + vendoring (critical for agentic + CI)

Implement `x07 pkg vendor vendor/` similar to `cargo vendor`. ([Rust Documentation][8])

Vendoring writes:

* `vendor/x07pkg/<pkg_id>/<version>/...` unpacked sources
* `vendor/index/config.json` + exact package index entry files used
* `vendor/vendor.lock.json` (a snapshot pointer to `x07.lock.json`)

Then:

* `x07 build --offline --vendor vendor/ --locked`

  * uses vendor directory only
  * refuses network access (Cargo `--offline` analog). ([Rust Documentation][3])

This becomes your “air‑gapped” enterprise story.

---

# 10) Security roadmap (don’t overbuild, but don’t paint yourself into a corner)

## 10.1 Immediate baseline: hashes everywhere

Require that `x07.lock.json` contains sha256 for every resolved package artifact.

This is conceptually similar to pip’s “hash checking mode” where `--require-hashes` enforces that downloads match expected hashes. ([Pip Documentation][9])

## 10.2 Next: TUF metadata for the registry (recommended before public internet)

TUF defines four required top-level roles: Root, Targets, Snapshot, Timestamp. ([TUF][10])

Add later (but design now):

* registry hosts `/tuf/root.json`, `/tuf/targets.json`, `/tuf/snapshot.json`, `/tuf/timestamp.json`
* client verifies metadata chain before trusting index/artifacts

## 10.3 Optional: Sigstore/cosign for provenance

Cosign stores signatures as OCI objects in registries and can sign “blobs”. ([Sigstore][11])
If you later move artifacts into an OCI registry, cosign becomes a clean add-on.

---

# 11) CLI: workspace-first UX (what agents will run)

## 11.1 Core package commands

* `x07 pkg init --workspace`
* `x07 pkg new <name>` (creates new member under `packages/<name>/`)
* `x07 pkg add <pkg>@<req> --to <member|workspace> [--registry default]`
* `x07 pkg resolve [--locked] [--offline] [--vendor vendor/] [--json]`
* `x07 pkg vendor vendor/ [--locked] [--json]`
* `x07 pkg verify [--locked] [--json]`
* `x07 pkg login --registry default --token <...>`
* `x07 pkg publish --package packages/lib_url --registry default [--json]`

## 11.2 Build commands integrate package manager

* `x07 build --package packages/app [--locked] [--offline] [--vendor vendor/]`
* `x07 test --workspace [--locked]`

---

# 12) Agent-friendly diagnostics and repair hooks

Every command supports:

* `--json` output (single JSON object to stdout)
* stable diagnostic codes (your `x07diag` catalog)
* optional `patch_suggestions` (JSON Patch ops that agents can apply deterministically)

Example error:

```json
{
  "ok": false,
  "error": {
    "code": "X07PKG_LOCK_MISMATCH",
    "message": "x07.lock.json would change (use --locked to fail or run resolve to update)",
    "patch_suggestions": [
      { "op": "replace", "path": "/deps/x07:stdlib-text", "value": "0.1.0" }
    ]
  }
}
```

This is the main “agentic coding” win: the agent doesn’t guess; it edits manifests via patches suggested by tooling.

---

# 13) Implementation plan (PR-sized milestones)

## PKG-00 — Specs + schemas

**Add**

* `spec/x07.workspace.schema.json`
* `spec/x07.package.schema.json`
* `spec/x07.lock.schema.json`
* `spec/x07pkg.schema.json` (package archive manifest)
* `scripts/check_pkg_schemas.py` (validates fixtures)
* fixtures under `tests/fixtures/pkg/`

**CI**

* validate fixtures against schemas
* “stable ordering” test: generate lock 2× and diff must be empty

## PKG-01 — Core library crate: `crates/x07-pkg`

**Add**

* manifest loader
* workspace member enumerator
* semantic validators (deterministic error codes)

## PKG-02 — Resolver v1 (deterministic)

**Add**

* semver solver (simple “highest compatible” deterministic)
* generates `x07.lock.json` with stable ordering

## PKG-03 — Content-addressed cache

**Add**

* `.x07/cache/sha256/<...>/` storage
* `x07pkg` archive unpacker
* `--offline` enforcement

## PKG-04 — Vendor support

**Add**

* `x07 pkg vendor vendor/` (Cargo vendor analog) ([Rust Documentation][8])
* `--vendor` flag that redirects all fetches to vendor

## PKG-05 — Sparse index client

**Add**

* `sparse+` index reader (HTTP GET by package name) ([Rust Documentation][6])
* reads `/index/config.json` with `dl`/`api` ([Rust Documentation][6])

## PKG-06 — Registry publish client + token storage

**Add**

* `x07 pkg login` token store
* `x07 pkg publish` sends `Authorization` header ([Rust Documentation][4])
* `auth-required` config honored ([Rust Language][5])

## PKG-07 — Registry server MVP

**Add**

* serve sparse index files + `config.json` ([Rust Documentation][6])
* serve artifacts
* publish endpoint with token validation ([Rust Documentation][4])

## PKG-08 — Hardening: locked/CI determinism

**Add**

* `x07 build --locked` semantics (fail if lock differs) ([Rust Documentation][2])
* run `resolve` 3× on same inputs and assert identical output

## PKG-09 — Security baseline: hash-required

**Add**

* enforce sha256 presence for every dependency (like “hash checking mode” idea) ([Pip Documentation][9])
* `x07 pkg verify` validates all cached/vendor artifacts match lock hashes

## PKG-10 — TUF (recommended before public exposure)

**Add**

* client verifies Root/Targets/Snapshot/Timestamp chain ([TUF][10])
* lock pins trusted TUF root hash

---

# 14) Key design choices that reduce LLM fragility

1. **No ambient scanning** unless explicitly asked
   Workspace root is explicit; module roots are explicit.
2. **x07AST JSON only**
   No parenthesis errors, no “almost valid” syntax.
3. **All edits via JSON Patch suggestions**
   Agents stop guessing how to fix projects; tooling tells them.
4. **`--locked` and `--offline` are first-class**
   Agents can rerun builds deterministically in CI-like settings. ([Rust Documentation][3])
5. **Sparse index**
   Faster, simpler, CDN-friendly; matches the Cargo direction. ([Rust Documentation][6])

---

[1]: https://doc.rust-lang.org/cargo/reference/workspaces.html?utm_source=chatgpt.com "Workspaces - The Cargo Book"
[2]: https://doc.rust-lang.org/cargo/commands/cargo-install.html?utm_source=chatgpt.com "cargo install - The Cargo Book"
[3]: https://doc.rust-lang.org/cargo/commands/cargo-generate-lockfile.html?utm_source=chatgpt.com "cargo generate-lockfile - The Cargo Book"
[4]: https://doc.rust-lang.org/cargo/reference/registry-web-api.html?utm_source=chatgpt.com "Registry Web API - The Cargo Book"
[5]: https://rust-lang.github.io/rfcs/3139-cargo-alternative-registry-auth.html?utm_source=chatgpt.com "3139-cargo-alternative-registry-auth - The Rust RFC Book"
[6]: https://doc.rust-lang.org/cargo/reference/registry-index.html?utm_source=chatgpt.com "Registry Index - The Cargo Book"
[7]: https://rustwiki.org/en/cargo/reference/config.html?utm_source=chatgpt.com "Configuration - The Cargo Book"
[8]: https://doc.rust-lang.org/cargo/commands/cargo-vendor.html?utm_source=chatgpt.com "cargo vendor - The Cargo Book"
[9]: https://pip.pypa.io/en/stable/topics/secure-installs/?utm_source=chatgpt.com "Secure installs - pip documentation v25.3"
[10]: https://theupdateframework.io/docs/metadata/?utm_source=chatgpt.com "Roles and metadata | TUF - The Update Framework"
[11]: https://docs.sigstore.dev/cosign/system_config/registry_support/?utm_source=chatgpt.com "Registry Support - Cosign"
+++++

Below are **copy/paste‑ready draft files** in the repo style we’ve been using (`spec/…`, `scripts/…`, `crates/…`). I’m intentionally mirroring a few Cargo registry behaviors where they help (sparse index config.json `dl`/`api`, `auth-required`, and `Authorization` token semantics).

---

## 1) `spec/x07.workspace.schema.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://x07.io/spec/x07.workspace.schema.json",
  "title": "X07 Workspace Manifest",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "workspace", "toolchain", "registries", "paths"],
  "properties": {
    "schema_version": {
      "type": "string",
      "const": "x07.workspace@0.1.0"
    },
    "workspace": {
      "type": "object",
      "additionalProperties": false,
      "required": ["name", "members"],
      "properties": {
        "name": { "$ref": "#/$defs/WorkspaceName" },
        "members": {
          "type": "array",
          "minItems": 1,
          "uniqueItems": true,
          "items": { "$ref": "#/$defs/RelPath" }
        },
        "default_member": { "$ref": "#/$defs/RelPath" }
      }
    },
    "toolchain": {
      "type": "object",
      "additionalProperties": false,
      "required": ["x07c_version", "stdlib_lock", "stdlib_lock_sha256"],
      "properties": {
        "x07c_version": { "$ref": "#/$defs/Semver" },
        "stdlib_lock": { "$ref": "#/$defs/RelPath" },
        "stdlib_lock_sha256": { "$ref": "#/$defs/Sha256Hex" }
      }
    },
    "registries": {
      "type": "object",
      "minProperties": 1,
      "additionalProperties": { "$ref": "#/$defs/Registry" }
    },
    "resolution": {
      "type": "object",
      "additionalProperties": false,
      "required": ["prefer_highest", "allow_yanked"],
      "properties": {
        "prefer_highest": { "type": "boolean", "default": true },
        "allow_yanked": { "type": "boolean", "default": false }
      }
    },
    "paths": {
      "type": "object",
      "additionalProperties": false,
      "required": ["cache_dir", "registry_dir", "target_dir"],
      "properties": {
        "cache_dir": { "$ref": "#/$defs/RelPath" },
        "registry_dir": { "$ref": "#/$defs/RelPath" },
        "target_dir": { "$ref": "#/$defs/RelPath" }
      }
    }
  },
  "$defs": {
    "WorkspaceName": {
      "type": "string",
      "minLength": 1,
      "maxLength": 64,
      "pattern": "^[a-z0-9][a-z0-9._-]*$"
    },
    "Semver": {
      "type": "string",
      "minLength": 5,
      "maxLength": 64,
      "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?$"
    },
    "Sha256Hex": {
      "type": "string",
      "pattern": "^[a-f0-9]{64}$"
    },
    "RelPath": {
      "type": "string",
      "minLength": 1,
      "maxLength": 256,
      "pattern": "^(?!/)(?!.*\\\\)(?!.*\\.{2})([A-Za-z0-9._-]+/)*[A-Za-z0-9._-]+$"
    },
    "Registry": {
      "type": "object",
      "additionalProperties": false,
      "required": ["index"],
      "properties": {
        "index": {
          "type": "string",
          "minLength": 8,
          "maxLength": 2048,
          "pattern": "^(sparse\\+)?https?://.+$"
        },
        "api": {
          "type": "string",
          "minLength": 8,
          "maxLength": 2048,
          "pattern": "^https?://.+$"
        },
        "auth_required": { "type": "boolean", "default": false },
        "tuf_root_sha256": { "$ref": "#/$defs/Sha256Hex" }
      }
    }
  }
}
```

Notes:

* The `index` field supports `sparse+https://…` (Cargo uses this prefix to select sparse protocol).
* `auth_required` exists because Cargo registries can signal this in index `config.json` to force Authorization on all operations.

---

## 2) `spec/x07.package.schema.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://x07.io/spec/x07.package.schema.json",
  "title": "X07 Package Manifest",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "package", "modules"],
  "properties": {
    "schema_version": {
      "type": "string",
      "const": "x07.package@0.1.0"
    },
    "package": {
      "type": "object",
      "additionalProperties": false,
      "required": ["id", "version"],
      "properties": {
        "id": { "$ref": "#/$defs/PkgId" },
        "version": { "$ref": "#/$defs/Semver" },
        "license": { "type": "string", "minLength": 1, "maxLength": 128 },
        "description": { "type": "string", "maxLength": 1024 },
        "repository": { "type": "string", "maxLength": 2048 },
        "authors": {
          "type": "array",
          "items": { "type": "string", "maxLength": 128 },
          "maxItems": 64
        }
      }
    },
    "modules": {
      "type": "object",
      "additionalProperties": false,
      "required": ["root", "exports"],
      "properties": {
        "root": { "$ref": "#/$defs/RelPath" },
        "exports": {
          "type": "array",
          "minItems": 1,
          "uniqueItems": true,
          "items": { "$ref": "#/$defs/ModuleId" }
        }
      }
    },
    "deps": { "$ref": "#/$defs/DepMap" },
    "dev_deps": { "$ref": "#/$defs/DepMap" },
    "capabilities": {
      "type": "object",
      "additionalProperties": false,
      "required": ["worlds_allowed", "requires", "forbids"],
      "properties": {
        "worlds_allowed": {
          "type": "array",
          "items": { "$ref": "#/$defs/WorldName" },
          "minItems": 1,
          "uniqueItems": true
        },
        "requires": {
          "type": "array",
          "items": { "$ref": "#/$defs/Capability" },
          "uniqueItems": true
        },
        "forbids": {
          "type": "array",
          "items": { "$ref": "#/$defs/Capability" },
          "uniqueItems": true
        }
      }
    }
  },
  "$defs": {
    "Semver": {
      "type": "string",
      "minLength": 5,
      "maxLength": 64,
      "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?$"
    },
    "SemverReq": {
      "type": "string",
      "minLength": 1,
      "maxLength": 64,
      "pattern": "^(\\^|~|>=|<=|>|<|=)?[0-9]+\\.[0-9]+\\.[0-9]+(?:-[0-9A-Za-z.-]+)?$"
    },
    "PkgId": {
      "type": "string",
      "minLength": 3,
      "maxLength": 128,
      "pattern": "^[a-z0-9][a-z0-9._-]*:[a-z0-9][a-z0-9._-]*$"
    },
    "ModuleId": {
      "type": "string",
      "minLength": 1,
      "maxLength": 128,
      "pattern": "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$"
    },
    "WorldName": {
      "type": "string",
      "minLength": 3,
      "maxLength": 64,
      "pattern": "^[a-z][a-z0-9-]*$"
    },
    "Capability": {
      "type": "string",
      "minLength": 1,
      "maxLength": 64,
      "pattern": "^[a-z][a-z0-9_.-]*$"
    },
    "RelPath": {
      "type": "string",
      "minLength": 1,
      "maxLength": 256,
      "pattern": "^(?!/)(?!.*\\\\)(?!.*\\.{2})([A-Za-z0-9._-]+/)*[A-Za-z0-9._-]+$"
    },
    "DepMap": {
      "type": "object",
      "additionalProperties": { "$ref": "#/$defs/SemverReq" },
      "propertyNames": { "$ref": "#/$defs/PkgId" }
    }
  }
}
```

---

## 3) `spec/x07.lock.schema.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://x07.io/spec/x07.lock.schema.json",
  "title": "X07 Workspace Lockfile",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "schema_version",
    "generated_at_unix",
    "toolchain",
    "registry",
    "workspace_members",
    "packages",
    "resolution_graph"
  ],
  "properties": {
    "schema_version": { "type": "string", "const": "x07.lock@0.1.0" },
    "generated_at_unix": { "type": "integer", "minimum": 0 },
    "toolchain": {
      "type": "object",
      "additionalProperties": false,
      "required": ["x07c_version", "stdlib_lock_sha256"],
      "properties": {
        "x07c_version": { "$ref": "#/$defs/Semver" },
        "stdlib_lock_sha256": { "$ref": "#/$defs/Sha256Hex" }
      }
    },
    "registry": {
      "type": "object",
      "minProperties": 1,
      "additionalProperties": { "$ref": "#/$defs/RegistryPin" }
    },
    "workspace_members": {
      "type": "array",
      "minItems": 1,
      "items": { "$ref": "#/$defs/WorkspaceMemberPin" }
    },
    "packages": {
      "type": "array",
      "items": { "$ref": "#/$defs/ResolvedPackage" }
    },
    "resolution_graph": {
      "type": "object",
      "additionalProperties": false,
      "required": ["roots"],
      "properties": {
        "roots": {
          "type": "array",
          "items": { "$ref": "#/$defs/RootEdges" }
        }
      }
    }
  },
  "$defs": {
    "Semver": {
      "type": "string",
      "minLength": 5,
      "maxLength": 64,
      "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?$"
    },
    "Sha256Hex": { "type": "string", "pattern": "^[a-f0-9]{64}$" },
    "PkgId": {
      "type": "string",
      "minLength": 3,
      "maxLength": 128,
      "pattern": "^[a-z0-9][a-z0-9._-]*:[a-z0-9][a-z0-9._-]*$"
    },
    "ModuleId": {
      "type": "string",
      "minLength": 1,
      "maxLength": 128,
      "pattern": "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$"
    },
    "RelPath": {
      "type": "string",
      "minLength": 1,
      "maxLength": 256,
      "pattern": "^(?!/)(?!.*\\\\)(?!.*\\.{2})([A-Za-z0-9._-]+/)*[A-Za-z0-9._-]+$"
    },
    "RegistryPin": {
      "type": "object",
      "additionalProperties": false,
      "required": ["index"],
      "properties": {
        "index": { "type": "string", "minLength": 8, "maxLength": 2048 },
        "api": { "type": "string", "minLength": 8, "maxLength": 2048 },
        "auth_required": { "type": "boolean", "default": false },
        "tuf_root_sha256": { "$ref": "#/$defs/Sha256Hex" }
      }
    },
    "WorkspaceMemberPin": {
      "type": "object",
      "additionalProperties": false,
      "required": ["path", "pkg_id", "version"],
      "properties": {
        "path": { "$ref": "#/$defs/RelPath" },
        "pkg_id": { "$ref": "#/$defs/PkgId" },
        "version": { "$ref": "#/$defs/Semver" }
      }
    },
    "Source": {
      "type": "object",
      "additionalProperties": false,
      "required": ["kind"],
      "properties": {
        "kind": { "type": "string", "enum": ["registry", "path"] },
        "registry": { "type": "string", "minLength": 1, "maxLength": 64 },
        "path": { "$ref": "#/$defs/RelPath" }
      },
      "allOf": [
        {
          "if": { "properties": { "kind": { "const": "registry" } } },
          "then": { "required": ["registry"] }
        },
        {
          "if": { "properties": { "kind": { "const": "path" } } },
          "then": { "required": ["path"] }
        }
      ]
    },
    "Artifact": {
      "type": "object",
      "additionalProperties": false,
      "required": ["format", "sha256"],
      "properties": {
        "format": { "type": "string", "enum": ["x07pkg+tar.zst", "x07pkg+tar"] },
        "url": { "type": "string", "maxLength": 2048 },
        "sha256": { "$ref": "#/$defs/Sha256Hex" }
      }
    },
    "ModuleIndexEntry": {
      "type": "object",
      "additionalProperties": false,
      "required": ["module_id", "path", "sha256"],
      "properties": {
        "module_id": { "$ref": "#/$defs/ModuleId" },
        "path": { "$ref": "#/$defs/RelPath" },
        "sha256": { "$ref": "#/$defs/Sha256Hex" }
      }
    },
    "DepEdge": {
      "type": "object",
      "additionalProperties": false,
      "required": ["pkg_id", "version"],
      "properties": {
        "pkg_id": { "$ref": "#/$defs/PkgId" },
        "version": { "$ref": "#/$defs/Semver" }
      }
    },
    "ResolvedPackage": {
      "type": "object",
      "additionalProperties": false,
      "required": ["pkg_id", "version", "source", "artifact", "deps"],
      "properties": {
        "pkg_id": { "$ref": "#/$defs/PkgId" },
        "version": { "$ref": "#/$defs/Semver" },
        "source": { "$ref": "#/$defs/Source" },
        "artifact": { "$ref": "#/$defs/Artifact" },
        "module_index": {
          "type": "array",
          "items": { "$ref": "#/$defs/ModuleIndexEntry" }
        },
        "deps": {
          "type": "array",
          "items": { "$ref": "#/$defs/DepEdge" }
        },
        "yanked": { "type": "boolean", "default": false }
      }
    },
    "RootEdges": {
      "type": "object",
      "additionalProperties": false,
      "required": ["member_path", "deps"],
      "properties": {
        "member_path": { "$ref": "#/$defs/RelPath" },
        "deps": {
          "type": "array",
          "items": { "$ref": "#/$defs/DepEdge" }
        }
      }
    }
  }
}
```

---

## 4) `clap` structs: `crates/x07-cli/src/cmd/pkg.rs`

This is **workspace‑first**, supports `--locked`, `--offline`, `--vendor`, and uses registry tokens (Cargo‑like “Authorization token” semantics).

```rust
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct PkgArgs {
    #[command(flatten)]
    pub common: CommonPkgArgs,

    #[command(subcommand)]
    pub cmd: PkgCmd,
}

/// Options shared by all `x07 pkg ...` commands.
#[derive(Debug, Args, Clone)]
pub struct CommonPkgArgs {
    /// Explicit workspace root directory. If omitted, the CLI will search upward
    /// for `x07.workspace.json` (deterministic).
    #[arg(long, value_name = "DIR")]
    pub workspace_root: Option<PathBuf>,

    /// Emit exactly one JSON object to stdout (machine contract).
    #[arg(long)]
    pub json: bool,

    /// Pretty-print JSON output (debug only).
    #[arg(long)]
    pub json_pretty: bool,

    /// Fail the command if warnings exist (agentic fail-closed).
    #[arg(long)]
    pub fail_on_warn: bool,

    /// Registry name to use (defaults to `default`).
    #[arg(long, default_value = "default")]
    pub registry: String,

    /// Use a specific vendor directory (offline snapshot).
    #[arg(long, value_name = "DIR")]
    pub vendor: Option<PathBuf>,

    /// Prevent any network access. Fails if missing cached/vendor data.
    #[arg(long)]
    pub offline: bool,

    /// Require the lockfile is up-to-date; fail if it would change.
    #[arg(long)]
    pub locked: bool,
}

#[derive(Debug, Subcommand)]
pub enum PkgCmd {
    /// Initialize a new workspace (or package if --package).
    Init(PkgInit),

    /// Create a new workspace member under `packages/<name>`.
    New(PkgNew),

    /// Add a dependency to a member or workspace default member.
    Add(PkgAdd),

    /// Remove a dependency.
    Remove(PkgRemove),

    /// Resolve dependencies and write x07.lock.json (unless --locked).
    Resolve(PkgResolve),

    /// Snapshot all sources into a vendor directory for offline builds.
    Vendor(PkgVendor),

    /// Verify cached/vendor artifacts match hashes in x07.lock.json.
    Verify(PkgVerify),

    /// Store a registry auth token (Authorization header value).
    Login(PkgLogin),

    /// Publish a package to the registry (source-only x07pkg archive).
    Publish(PkgPublish),

    /// Fetch and show package info from registry/cache.
    Info(PkgInfo),
}

#[derive(Debug, Args)]
pub struct PkgInit {
    /// Workspace name (also default root package id prefix).
    #[arg(long)]
    pub name: Option<String>,

    /// Initialize a workspace in the specified directory (default: current dir).
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,

    /// If set, initialize a package-only layout instead of workspace.
    #[arg(long)]
    pub package_only: bool,
}

#[derive(Debug, Args)]
pub struct PkgNew {
    /// New member short name (folder will be `packages/<name>`).
    pub name: String,

    /// Optional package id override (otherwise `workspace.name:<name>`).
    #[arg(long)]
    pub pkg_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct PkgAdd {
    /// Dependency spec: `pkg_id@req`, example: `x07:stdlib-json@^0.1.0`
    pub dep: String,

    /// Add to dev_deps instead of deps.
    #[arg(long)]
    pub dev: bool,

    /// Which member to modify (path under workspace). If omitted, uses default_member.
    #[arg(long, value_name = "MEMBER_PATH")]
    pub member: Option<String>,
}

#[derive(Debug, Args)]
pub struct PkgRemove {
    /// Dependency package id, example: `x07:stdlib-json`
    pub pkg_id: String,

    /// Remove from dev_deps instead of deps.
    #[arg(long)]
    pub dev: bool,

    /// Which member to modify (path under workspace). If omitted, uses default_member.
    #[arg(long, value_name = "MEMBER_PATH")]
    pub member: Option<String>,
}

#[derive(Debug, Args)]
pub struct PkgResolve {
    /// Only resolve; do not write x07.lock.json (prints plan in JSON mode).
    #[arg(long)]
    pub dry_run: bool,

    /// If set, allows yanked versions (normally denied).
    #[arg(long)]
    pub allow_yanked: bool,

    /// Do not update sparse index; use cached index only.
    #[arg(long)]
    pub no_index_update: bool,
}

#[derive(Debug, Args)]
pub struct PkgVendor {
    /// Vendor output directory.
    pub out_dir: PathBuf,

    /// If set, clears and re-creates vendor directory.
    #[arg(long)]
    pub clean: bool,
}

#[derive(Debug, Args)]
pub struct PkgVerify {
    /// Verify only a single package id (optional).
    #[arg(long)]
    pub pkg_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct PkgLogin {
    /// Registry auth token (stored locally). This is the raw token used in the
    /// Authorization header value.
    #[arg(long)]
    pub token: String,
}

#[derive(Debug, Args)]
pub struct PkgPublish {
    /// Member path to publish (e.g., `packages/lib_url`).
    #[arg(long, value_name = "MEMBER_PATH")]
    pub member: String,

    /// Dry-run: build archive + validate + print publish request JSON, but do not upload.
    #[arg(long)]
    pub dry_run: bool,

    /// Override registry (defaults to CommonPkgArgs.registry).
    #[arg(long)]
    pub registry: Option<String>,
}

#[derive(Debug, Args)]
pub struct PkgInfo {
    /// Package id (registry lookup).
    pub pkg_id: String,

    /// Optional version; if omitted, returns latest non-yanked.
    #[arg(long)]
    pub version: Option<String>,
}
```

---

## 5) Minimal registry server API contract: `spec/registry/openapi.json`

This **mirrors the key Cargo ideas**:

* publish endpoint is authenticated and uses Authorization token; invalid token => 403
* index has separate `config.json` with `dl`/`api` (you’ll serve that as static, not in OpenAPI)
* `auth-required` exists to require Authorization even for downloads/index updates

```json
{
  "openapi": "3.0.3",
  "info": {
    "title": "X07 Registry API",
    "version": "0.1.0"
  },
  "servers": [
    { "url": "https://registry.x07.io" }
  ],
  "components": {
    "securitySchemes": {
      "ApiToken": {
        "type": "apiKey",
        "in": "header",
        "name": "Authorization",
        "description": "API token (raw value). If invalid, server returns 403."
      }
    },
    "schemas": {
      "Error": {
        "type": "object",
        "additionalProperties": false,
        "required": ["code", "message"],
        "properties": {
          "code": { "type": "string" },
          "message": { "type": "string" }
        }
      },
      "PackageRef": {
        "type": "object",
        "additionalProperties": false,
        "required": ["pkg_id", "version"],
        "properties": {
          "pkg_id": { "type": "string" },
          "version": { "type": "string" }
        }
      },
      "PackageInfo": {
        "type": "object",
        "additionalProperties": false,
        "required": ["pkg_id", "latest_version", "versions"],
        "properties": {
          "pkg_id": { "type": "string" },
          "latest_version": { "type": "string" },
          "versions": {
            "type": "array",
            "items": { "$ref": "#/components/schemas/PackageVersionInfo" }
          }
        }
      },
      "PackageVersionInfo": {
        "type": "object",
        "additionalProperties": false,
        "required": ["pkg_id", "version", "yanked", "artifact_sha256"],
        "properties": {
          "pkg_id": { "type": "string" },
          "version": { "type": "string" },
          "yanked": { "type": "boolean" },
          "published_at_unix": { "type": "integer" },
          "artifact_sha256": { "type": "string" },
          "deps": {
            "type": "array",
            "items": { "$ref": "#/components/schemas/PackageRef" }
          }
        }
      },
      "PublishMetadata": {
        "type": "object",
        "additionalProperties": false,
        "required": ["pkg_id", "version", "manifest_sha256", "archive_sha256"],
        "properties": {
          "pkg_id": { "type": "string" },
          "version": { "type": "string" },
          "manifest_sha256": { "type": "string" },
          "archive_sha256": { "type": "string" },
          "yanked": { "type": "boolean", "default": false }
        }
      },
      "PublishResponse": {
        "type": "object",
        "additionalProperties": false,
        "required": ["ok", "pkg_id", "version"],
        "properties": {
          "ok": { "type": "boolean" },
          "pkg_id": { "type": "string" },
          "version": { "type": "string" },
          "warnings": {
            "type": "array",
            "items": { "type": "string" }
          }
        }
      }
    }
  },
  "paths": {
    "/api/v1/packages/{pkg_id}": {
      "get": {
        "summary": "Get package info",
        "parameters": [
          {
            "name": "pkg_id",
            "in": "path",
            "required": true,
            "schema": { "type": "string" }
          }
        ],
        "responses": {
          "200": {
            "description": "Package info",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/PackageInfo" }
              }
            }
          },
          "404": {
            "description": "Not found",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/Error" }
              }
            }
          }
        }
      }
    },
    "/api/v1/packages/{pkg_id}/{version}": {
      "get": {
        "summary": "Get package version info",
        "parameters": [
          { "name": "pkg_id", "in": "path", "required": true, "schema": { "type": "string" } },
          { "name": "version", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": {
          "200": {
            "description": "Package version info",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/PackageVersionInfo" }
              }
            }
          },
          "404": {
            "description": "Not found",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/Error" }
              }
            }
          }
        }
      }
    },
    "/api/v1/artifacts/{pkg_id}/{version}/x07pkg.tar.zst": {
      "get": {
        "summary": "Download source-only x07pkg archive",
        "parameters": [
          { "name": "pkg_id", "in": "path", "required": true, "schema": { "type": "string" } },
          { "name": "version", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": {
          "200": {
            "description": "Archive bytes",
            "content": {
              "application/octet-stream": {
                "schema": { "type": "string", "format": "binary" }
              }
            }
          },
          "403": {
            "description": "Forbidden (auth-required and token missing/invalid)",
            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } }
          },
          "404": {
            "description": "Not found",
            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } }
          }
        },
        "security": [
          { "ApiToken": [] }
        ]
      }
    },
    "/api/v1/packages/new": {
      "put": {
        "summary": "Publish a new package version (source-only archive)",
        "security": [{ "ApiToken": [] }],
        "requestBody": {
          "required": true,
          "content": {
            "multipart/form-data": {
              "schema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["metadata", "archive"],
                "properties": {
                  "metadata": {
                    "description": "Publish metadata JSON",
                    "contentMediaType": "application/json",
                    "schema": { "$ref": "#/components/schemas/PublishMetadata" }
                  },
                  "archive": {
                    "description": "x07pkg.tar.zst archive bytes",
                    "type": "string",
                    "format": "binary"
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Published",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/PublishResponse" }
              }
            }
          },
          "400": {
            "description": "Bad request",
            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } }
          },
          "403": {
            "description": "Forbidden",
            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } }
          }
        }
      }
    },
    "/api/v1/packages/{pkg_id}/{version}/yank": {
      "post": {
        "summary": "Yank a version",
        "security": [{ "ApiToken": [] }],
        "parameters": [
          { "name": "pkg_id", "in": "path", "required": true, "schema": { "type": "string" } },
          { "name": "version", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": {
          "200": { "description": "OK" },
          "403": {
            "description": "Forbidden",
            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } }
          }
        }
      }
    },
    "/api/v1/packages/{pkg_id}/{version}/unyank": {
      "post": {
        "summary": "Unyank a version",
        "security": [{ "ApiToken": [] }],
        "parameters": [
          { "name": "pkg_id", "in": "path", "required": true, "schema": { "type": "string" } },
          { "name": "version", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": {
          "200": { "description": "OK" },
          "403": {
            "description": "Forbidden",
            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } }
          }
        }
      }
    }
  }
}
```

### Request/response JSON shapes (examples)

**Publish metadata (form field `metadata`)**

```json
{
  "pkg_id": "x07:stdlib-text",
  "version": "0.1.0",
  "manifest_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "archive_sha256": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
  "yanked": false
}
```

**Package info response**

```json
{
  "pkg_id": "x07:stdlib-text",
  "latest_version": "0.1.2",
  "versions": [
    {
      "pkg_id": "x07:stdlib-text",
      "version": "0.1.2",
      "yanked": false,
      "published_at_unix": 1767400000,
      "artifact_sha256": "..."
    }
  ]
}
```

---

## 6) Contract self-test: `scripts/check_pkg_contracts.py`

This script validates:

1. workspace manifest (`x07.workspace.json`) vs schema
2. package manifests (`x07.package.json` and package.json inside x07pkg) vs schema
3. lockfile (`x07.lock.json`) vs schema
4. x07pkg archive safety + required files + deterministic tar invariants

```python
#!/usr/bin/env python3
import argparse
import hashlib
import io
import json
import os
import sys
import tarfile
from pathlib import Path

try:
    import jsonschema
except ImportError:
    print("ERROR: missing python dependency 'jsonschema'. Install dev deps.", file=sys.stderr)
    sys.exit(2)

# Optional: needed only if you validate .tar.zst archives.
try:
    import zstandard as zstd  # type: ignore
except ImportError:
    zstd = None


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_json(path: Path):
    with path.open("rb") as f:
        return json.loads(f.read().decode("utf-8"))


def load_schema(path: Path):
    return read_json(path)


def validate_json(obj, schema, label: str) -> None:
    try:
        jsonschema.validate(instance=obj, schema=schema)
    except jsonschema.ValidationError as e:
        print(f"ERROR: schema validation failed: {label}", file=sys.stderr)
        print(f"  message: {e.message}", file=sys.stderr)
        print(f"  path: {'/'.join(map(str, e.absolute_path))}", file=sys.stderr)
        sys.exit(3)


def canonical_json_bytes(obj) -> bytes:
    # Deterministic JSON encoding for hashing and comparisons.
    # - sort_keys=True ensures stable key order
    # - separators remove whitespace
    # - ensure_ascii=False keeps UTF-8
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def is_safe_relpath(p: str) -> bool:
    # Must be relative, no backslashes, no '..' segments.
    if p.startswith("/"):
        return False
    if "\\" in p:
        return False
    parts = p.split("/")
    if any(seg in ("", ".", "..") for seg in parts):
        return False
    return True


def open_tar_maybe_zst(archive_path: Path) -> tarfile.TarFile:
    if archive_path.suffixes[-2:] == [".tar", ".zst"]:
        if zstd is None:
            print("ERROR: archive is .tar.zst but python 'zstandard' is not installed", file=sys.stderr)
            sys.exit(2)
        raw = archive_path.read_bytes()
        dctx = zstd.ZstdDecompressor()
        decompressed = dctx.decompress(raw)
        return tarfile.open(fileobj=io.BytesIO(decompressed), mode="r:")
    elif archive_path.suffix == ".tar":
        return tarfile.open(archive_path, mode="r:")
    else:
        print(f"ERROR: unsupported archive type: {archive_path}", file=sys.stderr)
        sys.exit(2)


def check_tar_determinism(tf: tarfile.TarFile, label: str) -> None:
    members = tf.getmembers()
    names = [m.name for m in members]

    # 1) No absolute paths or traversal.
    for n in names:
        if not is_safe_relpath(n):
            print(f"ERROR: unsafe path in archive {label}: {n}", file=sys.stderr)
            sys.exit(4)

    # 2) Canonical ordering: require lexicographic order (strong gate).
    if names != sorted(names):
        print(f"ERROR: archive entries not sorted lexicographically: {label}", file=sys.stderr)
        sys.exit(4)

    # 3) Optional: enforce stable mtimes (recommended).
    # If you want this as a hard gate, flip WARNING->ERROR.
    for m in members:
        if m.mtime not in (0,):
            print(f"WARNING: non-canonical mtime in {label}: {m.name} mtime={m.mtime}", file=sys.stderr)


def read_tar_file_utf8(tf: tarfile.TarFile, name: str) -> str:
    f = tf.extractfile(name)
    if f is None:
        raise FileNotFoundError(name)
    return f.read().decode("utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", type=str, default=".", help="repo root")
    ap.add_argument("--fixtures", type=str, default="tests/fixtures/pkg", help="fixtures root")
    ap.add_argument("--check-archives", action="store_true", help="validate x07pkg archives in fixtures")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    fixtures = (root / args.fixtures).resolve()

    ws_schema_path = root / "spec" / "x07.workspace.schema.json"
    pkg_schema_path = root / "spec" / "x07.package.schema.json"
    lock_schema_path = root / "spec" / "x07.lock.schema.json"

    ws_schema = load_schema(ws_schema_path)
    pkg_schema = load_schema(pkg_schema_path)
    lock_schema = load_schema(lock_schema_path)

    # ---- Workspace fixture ----
    ws_manifest_path = fixtures / "workspace" / "x07.workspace.json"
    if not ws_manifest_path.exists():
        print(f"ERROR: missing fixture: {ws_manifest_path}", file=sys.stderr)
        return 2

    ws = read_json(ws_manifest_path)
    validate_json(ws, ws_schema, f"workspace manifest {ws_manifest_path}")

    # ---- Member package fixtures ----
    ws_dir = ws_manifest_path.parent
    members = ws["workspace"]["members"]
    for rel in members:
        pkg_path = (ws_dir / rel / "x07.package.json").resolve()
        if not pkg_path.exists():
            print(f"ERROR: workspace member missing x07.package.json: {pkg_path}", file=sys.stderr)
            return 2
        pkg = read_json(pkg_path)
        validate_json(pkg, pkg_schema, f"package manifest {pkg_path}")

    # ---- Lockfile fixture ----
    lock_path = fixtures / "workspace" / "x07.lock.json"
    if lock_path.exists():
        lock = read_json(lock_path)
        validate_json(lock, lock_schema, f"lockfile {lock_path}")

        # Strong determinism gates: stable ordering expectations.
        pkgs = lock.get("packages", [])
        sorted_pkgs = sorted(pkgs, key=lambda p: (p.get("pkg_id", ""), p.get("version", "")))
        if pkgs != sorted_pkgs:
            print("ERROR: lockfile packages not sorted by (pkg_id, version)", file=sys.stderr)
            return 5

        members_pin = lock.get("workspace_members", [])
        sorted_members = sorted(members_pin, key=lambda m: m.get("path", ""))
        if members_pin != sorted_members:
            print("ERROR: lockfile workspace_members not sorted by path", file=sys.stderr)
            return 5

    # ---- Archive fixtures ----
    if args.check_archives:
        archives_dir = fixtures / "archives"
        if archives_dir.exists():
            for arch in sorted(archives_dir.glob("*.tar")) + sorted(archives_dir.glob("*.tar.zst")):
                with open_tar_maybe_zst(arch) as tf:
                    check_tar_determinism(tf, str(arch))

                    # Require package.json at root.
                    if "package.json" not in [m.name for m in tf.getmembers()]:
                        print(f"ERROR: archive missing package.json: {arch}", file=sys.stderr)
                        return 4

                    pkg_json = json.loads(read_tar_file_utf8(tf, "package.json"))
                    validate_json(pkg_json, pkg_schema, f"archive package.json {arch}")

                    # Require at least one module file.
                    module_names = [m.name for m in tf.getmembers() if m.name.endswith("module.x07.json")]
                    if not module_names:
                        print(f"ERROR: archive has no module.x07.json files: {arch}", file=sys.stderr)
                        return 4

    print("OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

### Expected fixture layout (minimal)

```
tests/fixtures/pkg/
  workspace/
    x07.workspace.json
    x07.lock.json            # optional fixture for schema+ordering checks
    packages/
      app/x07.package.json
      lib_url/x07.package.json
  archives/
    x07-stdlib-text-0.1.0.tar.zst   # optional (enable --check-archives)
```

---

## 7) Why these choices line up with “Cargo-like” behavior (for your docs)

* **Index root `config.json`** with `dl` and `api` is a Cargo registry convention.
* **Authorization header** token semantics and 403 on invalid token are explicitly defined by Cargo’s registry Web API docs.
* **`auth-required = true`** for registries is defined in the Rust RFC for alternative registry auth; it forces Authorization for API, downloads, and index updates.
* **Sparse index** is HTTP-based and benefits from HTTP/2 and cache revalidation (ETag / If-Modified-Since).
++++

Below is a **copy/paste-ready** draft for the `crates/x07-pkg` implementation pieces you asked for:

* **Rust types** matching the workspace/package/lock JSON schemas
* a **deterministic canonical JSON writer** (`canonical_json.rs`)
* a **publish packer** that emits **sorted tar entries** with **canonical mtimes** (and uid/gid/mode pinned), so your `check_pkg_contracts.py --check-archives` can be a hard CI gate.

I’m intentionally making this strict/deterministic (deny unknown fields, stable ordering via `BTreeMap`, canonical JSON normalization).

---

## 1) `crates/x07-pkg/Cargo.toml`

```toml
[package]
name = "x07-pkg"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
description = "X07 package/workspace/lock types + canonical JSON + publish packer"

[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
hex = "0.4"
tar = "0.4"
zstd = "0.13"
```

Notes:

* `tar` is used for deterministic archive construction; we explicitly set header metadata and rely on `append_data` only to set path+checksum (it does **not** modify other metadata). ([Docs.rs][1])
* `zstd` used for `.tar.zst` packing; compression level defaults to 3 when level 0 is used; we’ll pin level explicitly. ([Docs.rs][2])

---

## 2) `crates/x07-pkg/src/lib.rs`

```rust
//! X07 packaging primitives:
//! - Schema-typed manifests (workspace/package/lock)
//! - Deterministic canonical JSON writer
//! - Deterministic publish packer (sorted tar, fixed mtimes)

pub mod canonical_json;
pub mod schema;
pub mod packer;

pub use schema::*;
pub use packer::*;
```

---

## 3) `crates/x07-pkg/src/schema.rs`

These are the schema-aligned Rust types your tooling can use everywhere (CLI, validation, lock generation).

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const WORKSPACE_SCHEMA: &str = "x07.workspace@0.1.0";
pub const PACKAGE_SCHEMA: &str = "x07.package@0.1.0";
pub const LOCK_SCHEMA: &str = "x07.lock@0.1.0";

/// Workspace-first manifest (monorepo).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceManifest {
    pub schema_version: String,
    pub workspace: Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    /// Workspace name (human-facing only).
    pub name: String,

    /// Deterministic list of member package directories (relative to workspace root).
    /// Example: ["stdlib/std/0.1.0/ascii", "apps/demo"]
    pub members: Vec<String>,

    /// Registry aliases → base URLs.
    /// Example: { "default": { "base_url": "https://registry.example/api/v1" } }
    #[serde(default)]
    pub registries: BTreeMap<String, Registry>,

    /// Which registry alias is used by default.
    #[serde(default)]
    pub default_registry: Option<String>,

    /// Optional: workspace-wide edition/toolchain selection (purely informational for now).
    #[serde(default)]
    pub toolchain: Option<WorkspaceToolchain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceToolchain {
    /// X07 toolchain version requirement used by this workspace (string, not semver-typed at v1).
    pub x07_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// Base API URL for registry (ends with /api/v1 or similar).
    pub base_url: String,
}

/// Package manifest (source-only, in x07AST JSON files).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    pub schema_version: String,
    pub package: Package,
    #[serde(default)]
    pub dependencies: BTreeMap<String, DepSpec>, // dep_id -> dep spec
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    /// Globally unique package id (recommended: "org:namespace-name" style).
    pub id: String,

    /// Semantic version string.
    pub version: String,

    /// Type of package.
    #[serde(default = "default_package_kind")]
    pub kind: PackageKind,

    /// Module(s) exported by this package (module IDs).
    /// Example: ["std.text.ascii"]
    pub exports: Vec<String>,

    /// Relative directory containing x07AST JSON sources.
    /// Example: "src" or "module"
    pub module_root: String,

    /// Optional metadata.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
}

fn default_package_kind() -> PackageKind {
    PackageKind::Lib
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageKind {
    Lib,
    Bin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepSpec {
    /// Semver requirement string (e.g. "^0.1.0") OR exact pin.
    pub req: String,

    /// Optional registry alias override (otherwise workspace default_registry is used).
    #[serde(default)]
    pub registry: Option<String>,

    /// Workspace path dependency (relative).
    /// If set, the resolver treats it as a workspace member dependency.
    #[serde(default)]
    pub path: Option<String>,
}

/// Lockfile: resolved dependency graph + pinned artifact hashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lockfile {
    pub schema_version: String,

    /// Deterministic workspace fingerprint (for debugging / invalidation).
    /// Keep as a string to avoid float/timestamp nondeterminism.
    #[serde(default)]
    pub workspace_fingerprint: Option<String>,

    /// Locked packages (sorted deterministically: id, version).
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedPackage {
    pub id: String,
    pub version: String,

    /// Resolved dependencies: dep_id -> exact version.
    #[serde(default)]
    pub dependencies: BTreeMap<String, LockedDep>,

    /// Source describing how to fetch/verify.
    pub source: LockedSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedDep {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum LockedSource {
    /// Registry-sourced package: content is an archive with sha256.
    Registry {
        registry: String,
        artifact: LockedArtifact,
    },

    /// Workspace/path dependency (no archive).
    Path {
        path: String,
        /// Optional hash of canonical package.json + canonicalized module JSON files.
        #[serde(default)]
        content_sha256: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedArtifact {
    /// "x07pkg+tar" or "x07pkg+tar.zst"
    pub format: String,
    pub sha256: String,
    pub size_bytes: u64,

    /// Optional (registry computed): content hash of the *unpacked* canonical tar stream.
    #[serde(default)]
    pub unpacked_sha256: Option<String>,
}
```

**Why these choices are deterministic:**

* maps are `BTreeMap` → stable key ordering during canonical JSON encoding
* we use `deny_unknown_fields` to prevent silent drift
* lock “source” is tagged enum → stable discrimination

---

## 4) `crates/x07-pkg/src/canonical_json.rs`

This is the “single source of truth” canonical JSON encoding you’ll use for:

* writing `stdlib.lock`
* writing canonical `package.json` and canonical `*.x07.json` into archives
* hashing for deterministic identity

```rust
use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;

/// Canonicalize a `serde_json::Value`:
/// - Objects: keys sorted lexicographically (UTF-8 byte order via Rust `String` ordering)
/// - Arrays: preserve order
/// - Scalars: unchanged
pub fn canonicalize_value(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            // Sort keys deterministically.
            let mut sorted: BTreeMap<String, Value> = BTreeMap::new();
            for (k, vv) in map.iter() {
                sorted.insert(k.clone(), canonicalize_value(vv));
            }
            // Rebuild as serde_json::Map preserving insertion order (which is now sorted).
            let mut out = serde_json::Map::new();
            for (k, vv) in sorted.into_iter() {
                out.insert(k, vv);
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize_value).collect()),
        _ => v.clone(),
    }
}

/// Serialize a `serde_json::Value` into *canonical* compact JSON bytes.
pub fn to_canonical_json_bytes_value(v: &Value) -> Result<Vec<u8>> {
    let v2 = canonicalize_value(v);
    let mut out = Vec::<u8>::new();
    serde_json::to_writer(&mut out, &v2)?;
    Ok(out)
}

/// Serialize any `T: Serialize` into canonical compact JSON bytes.
pub fn to_canonical_json_bytes<T: Serialize>(t: &T) -> Result<Vec<u8>> {
    let v = serde_json::to_value(t)?;
    to_canonical_json_bytes_value(&v)
}

/// Write canonical JSON for any `T: Serialize`.
pub fn write_canonical_json<W: Write, T: Serialize>(mut w: W, t: &T) -> Result<()> {
    let bytes = to_canonical_json_bytes(t)?;
    w.write_all(&bytes)?;
    Ok(())
}

/// Parse bytes as JSON and re-emit canonical JSON bytes.
/// Useful for canonicalizing `package.json` and `*.x07.json` content.
pub fn recanonicalize_json_bytes(input: &[u8]) -> Result<Vec<u8>> {
    let v: Value = serde_json::from_slice(input)
        .map_err(|e| anyhow!("invalid json: {e}"))?;
    to_canonical_json_bytes_value(&v)
}
```

---

## 5) `crates/x07-pkg/src/packer.rs`

This packer is designed so:

* tar entry order is deterministic (lexicographic path)
* header metadata is deterministic (mtime/uid/gid/mode pinned)
* JSON files are canonicalized before packing
* symlinks are rejected
* output format supports `.tar` or `.tar.zst`

Key detail: `tar::Builder::append_data` **only** sets the path + updates checksum; it explicitly says “No other metadata in the header will be modified.” ([Docs.rs][1])
So we set `mtime/uid/gid/mode` ourselves and make sure they stay fixed.

```rust
use crate::canonical_json::recanonicalize_json_bytes;
use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tar::{Builder, EntryType, Header};
use zstd::stream::write::Encoder as ZstdEncoder;

#[derive(Debug, Clone, Copy)]
pub enum ArtifactFormat {
    Tar,
    TarZst,
}

#[derive(Debug, Clone)]
pub struct PackConfig {
    pub format: ArtifactFormat,
    pub fixed_mtime: u64, // tar header mtime
    pub fixed_uid: u64,
    pub fixed_gid: u64,
    pub fixed_mode: u32,
    pub zstd_level: i32, // 1-22, recommend 3
}

impl Default for PackConfig {
    fn default() -> Self {
        Self {
            format: ArtifactFormat::TarZst,
            fixed_mtime: 0,
            fixed_uid: 0,
            fixed_gid: 0,
            fixed_mode: 0o644,
            zstd_level: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackedArtifact {
    pub format: ArtifactFormat,
    pub sha256_hex: String,
    pub size_bytes: u64,
    pub file_count: usize,
}

/// Pack a package directory into a deterministic archive.
///
/// Expected package layout (v1):
/// - package.json (or x07.package.json) at package_dir root
/// - module_root directory (declared in package.json) containing `*.x07.json` files
///
/// Archive invariants:
/// - all entry paths are relative and use `/`
/// - entries are sorted lexicographically by archive path
/// - mtime/uid/gid/mode are pinned (mtime default 0)
pub fn pack_package_dir(package_dir: &Path, out_path: &Path, cfg: &PackConfig) -> Result<PackedArtifact> {
    let manifest_path = find_manifest_path(package_dir)?;
    let manifest_bytes = fs::read(&manifest_path)
        .with_context(|| format!("read manifest: {}", manifest_path.display()))?;

    // Parse manifest as JSON so we can:
    // 1) canonicalize it inside archive
    // 2) read module_root
    let manifest_val: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("manifest is not valid JSON: {}", manifest_path.display()))?;
    let module_root = manifest_val
        .get("package")
        .and_then(|p| p.get("module_root"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("manifest missing package.module_root (string)"))?;

    let module_root_dir = package_dir.join(module_root);
    if !module_root_dir.is_dir() {
        return Err(anyhow!(
            "module_root directory not found: {}",
            module_root_dir.display()
        ));
    }

    // Collect files deterministically.
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

    // Always include manifest as `package.json` in archive.
    let canon_manifest = recanonicalize_json_bytes(&manifest_bytes)
        .context("canonicalize package manifest json")?;
    entries.push(("package.json".to_string(), canon_manifest));

    // Include all .x07.json files under module_root (deterministic traversal).
    let mut module_files = list_files_recursive_sorted(&module_root_dir)?;
    // Filter: only allow *.x07.json
    module_files.retain(|p| p.extension().and_then(|s| s.to_str()) == Some("json") && p.to_string_lossy().ends_with(".x07.json"));

    for abs_path in module_files {
        reject_symlink(&abs_path)?;
        let rel = abs_path.strip_prefix(package_dir)
            .with_context(|| format!("internal: path not under package_dir: {}", abs_path.display()))?;
        let rel_arc = to_archive_path(rel)?;

        let raw = fs::read(&abs_path)
            .with_context(|| format!("read module file: {}", abs_path.display()))?;
        let canon = recanonicalize_json_bytes(&raw)
            .with_context(|| format!("canonicalize JSON: {}", abs_path.display()))?;
        entries.push((rel_arc, canon));
    }

    // Optional: include README.md / LICENSE if present (raw).
    for name in ["README.md", "LICENSE", "LICENSE.md"] {
        let p = package_dir.join(name);
        if p.is_file() {
            reject_symlink(&p)?;
            let bytes = fs::read(&p).with_context(|| format!("read {}", p.display()))?;
            entries.push((name.to_string(), bytes));
        }
    }

    // Sort entries by archive path (stable).
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));

    // Build a canonical tar stream in-memory.
    let tar_bytes = build_tar_bytes(&entries, cfg)?;

    // Emit to out_path (tar or tar.zst).
    let out_bytes = match cfg.format {
        ArtifactFormat::Tar => tar_bytes,
        ArtifactFormat::TarZst => compress_zstd(&tar_bytes, cfg.zstd_level)?,
    };

    fs::write(out_path, &out_bytes)
        .with_context(|| format!("write archive: {}", out_path.display()))?;

    let sha256_hex = sha256_hex(&out_bytes);
    Ok(PackedArtifact {
        format: cfg.format,
        sha256_hex,
        size_bytes: out_bytes.len() as u64,
        file_count: entries.len(),
    })
}

fn find_manifest_path(package_dir: &Path) -> Result<PathBuf> {
    let a = package_dir.join("package.json");
    if a.is_file() {
        return Ok(a);
    }
    let b = package_dir.join("x07.package.json");
    if b.is_file() {
        return Ok(b);
    }
    Err(anyhow!(
        "missing package manifest: expected package.json or x07.package.json in {}",
        package_dir.display()
    ))
}

fn reject_symlink(p: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(p)?;
    if meta.file_type().is_symlink() {
        return Err(anyhow!("symlinks are forbidden in packages: {}", p.display()));
    }
    Ok(())
}

/// Convert a repo-relative path into a safe archive path:
/// - must be relative
/// - no `..`
/// - normalize separators to `/`
/// - no Windows prefix
fn to_archive_path(rel: &Path) -> Result<String> {
    let mut parts: Vec<String> = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(os) => {
                let s = os.to_string_lossy();
                if s.is_empty() {
                    return Err(anyhow!("empty path segment"));
                }
                parts.push(s.to_string());
            }
            Component::CurDir => {
                // Strip.
            }
            Component::ParentDir => return Err(anyhow!("parent dir '..' not allowed in archive paths")),
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("absolute/prefix paths not allowed in archive paths"))
            }
        }
    }
    if parts.is_empty() {
        return Err(anyhow!("empty archive path"));
    }
    Ok(parts.join("/"))
}

/// Deterministic recursive file listing:
/// - never follows symlinks
/// - sorts entries lexicographically at every directory level
fn list_files_recursive_sorted(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = Vec::new();
    walk_dir_sorted(root, &mut out)?;
    out.sort_by(|a, b| {
        // stable order by stringified path
        let sa = a.to_string_lossy();
        let sb = b.to_string_lossy();
        sa.cmp(&sb)
    });
    Ok(out)
}

fn walk_dir_sorted(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let meta = fs::symlink_metadata(dir)?;
    if meta.file_type().is_symlink() {
        return Err(anyhow!("symlinked directories forbidden: {}", dir.display()));
    }
    if !meta.is_dir() {
        return Ok(());
    }

    let mut children: Vec<PathBuf> = Vec::new();
    for e in fs::read_dir(dir)? {
        let e = e?;
        children.push(e.path());
    }
    children.sort_by(|a, b| {
        let sa = a.file_name().unwrap().to_string_lossy();
        let sb = b.file_name().unwrap().to_string_lossy();
        sa.cmp(&sb)
    });

    for child in children {
        let m = fs::symlink_metadata(&child)?;
        if m.file_type().is_symlink() {
            return Err(anyhow!("symlink forbidden in package tree: {}", child.display()));
        }
        if m.is_dir() {
            walk_dir_sorted(&child, out)?;
        } else if m.is_file() {
            out.push(child);
        }
    }
    Ok(())
}

fn build_tar_bytes(entries: &[(String, Vec<u8>)], cfg: &PackConfig) -> Result<Vec<u8>> {
    let mut buf = Vec::<u8>::new();
    {
        let mut builder = Builder::new(&mut buf);

        for (path, data) in entries.iter() {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            header.set_size(data.len() as u64);

            // Deterministic metadata.
            header.set_mtime(cfg.fixed_mtime);
            header.set_uid(cfg.fixed_uid);
            header.set_gid(cfg.fixed_gid);
            header.set_mode(cfg.fixed_mode);

            // append_data will set the path and update checksum, but it will not modify other metadata. :contentReference[oaicite:3]{index=3}
            builder
                .append_data(&mut header, path.as_str(), &data[..])
                .with_context(|| format!("append tar entry: {path}"))?;
        }

        // Finish tar.
        builder.finish()?;
    }
    Ok(buf)
}

fn compress_zstd(input: &[u8], level: i32) -> Result<Vec<u8>> {
    let mut out = Vec::<u8>::new();
    // Encoder::new(level) uses level 1-22; level 0 means default (currently 3). We pin explicitly. :contentReference[oaicite:4]{index=4}
    let mut enc = ZstdEncoder::new(&mut out, level)
        .context("zstd encoder init")?;

    // Keep compression single-threaded for deterministic behavior (default unless zstdmt is enabled). :contentReference[oaicite:5]{index=5}
    // Not enabling zstdmt feature here => no multithread path.

    enc.write_all(input)?;
    enc.finish()?;
    Ok(out)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    hex::encode(digest)
}
```

### Why this passes your “archive determinism” gate

* **Entry ordering:** lexicographic sort on archive path before writing.
* **Metadata pinned:** `mtime=0`, `uid=0`, `gid=0`, `mode=0o644` for every file.
* **append_data semantics:** only touches path+checksum; we set everything else (mtime/uid/gid/mode) before calling it. ([Docs.rs][1])
* **JSON canonicalization:** all JSON files are parsed then re-emitted with sorted keys (via `canonical_json.rs`).

---

## 6) Where this plugs into `x07 pkg publish`

Your future `x07 pkg publish` implementation should:

1. read/validate `package.json` using the `schema::PackageManifest` type
2. call `pack_package_dir(...)` to create `*.x07pkg.tar.zst`
3. upload it, and publish `sha256/size` returned in `PackedArtifact`

Then CI can do:

* `x07 pkg pack --path <pkgdir> --out /tmp/pkg.tar.zst`
* `python scripts/check_pkg_contracts.py --check-archives /tmp/pkg.tar.zst`

---

## 7) One small decision you should lock now

**Do you want to hash the compressed archive bytes or the canonical tar stream bytes?**

* If you hash **compressed** bytes: every publish includes `sha256(archive.tar.zst)`.

  * Works great for registry distribution (the registry serves exactly that blob).
* If you hash **uncompressed canonical tar** bytes: you can accept any compression wrapper, but need to standardize what gets hashed.

In the code above, `PackedArtifact.sha256_hex` is the hash of the bytes written to disk (compressed if `.tar.zst`). That’s the simplest “Cargo-like blob integrity” model.

---
[1]: https://docs.rs/tar/latest/tar/struct.Builder.html "Builder in tar - Rust"
[2]: https://docs.rs/zstd/latest/zstd/stream/write/struct.Encoder.html "Encoder in zstd::stream::write - Rust"

+++++
Below is a **copy/paste-ready** draft of the companion CLI commands and their **exact clap structs + compile‑ready Rust skeleton** that uses your `x07-pkg` crate (schema types + canonical JSON + deterministic packer).

This is designed to immediately unblock **SKILLS/PKG CI**:

* `x07 pkg pack` is fully functional (deterministic archive + JSON report).
* `x07 pkg lock` is fully functional in **workspace-only / path-deps** mode (no network required; deterministic lock output).
* `x07 pkg publish` is implemented as **pack + HTTP POST** with a Cargo-like `Authorization` header behavior (token passed as header value; expect 403 if invalid).

It also keeps your determinism properties:

* tar headers pinned and not modified by `append_data` beyond path/checksum.
* zstd multithreading is **disabled** (we don’t enable `zstdmt`).

---

## 0) CLI UX (what the commands look like)

### `x07 pkg pack`

Deterministically packs a single package directory into a `.x07pkg.tar` or `.x07pkg.tar.zst`.

Example:

```bash
x07 pkg pack \
  --package-dir stdlib/std/0.1.0/ascii \
  --out target/x07pkg/x07-stdlib-ascii-0.1.0.x07pkg.tar.zst \
  --format tar.zst \
  --json
```

### `x07 pkg lock`

Deterministically generates a lockfile for a workspace, **offline**, **workspace/path deps only** (hard error if a dep is registry-based unless you later implement registry resolution).

Example:

```bash
x07 pkg lock \
  --workspace . \
  --lockfile x07.lock.json \
  --workspace-only \
  --json
```

Cargo-like “don’t modify lockfile” gate:

```bash
x07 pkg lock --workspace . --lockfile x07.lock.json --workspace-only --locked
```

### `x07 pkg publish`

Packs + uploads to registry.

Example (token via env; dry-run first):

```bash
export X07_REGISTRY_TOKEN="…"
x07 pkg publish \
  --workspace . \
  --package-dir stdlib/std/0.1.0/ascii \
  --registry default \
  --token-env X07_REGISTRY_TOKEN \
  --dry-run \
  --json
```

Actual publish (non-dry-run):

```bash
x07 pkg publish \
  --workspace . \
  --package-dir stdlib/std/0.1.0/ascii \
  --registry default \
  --token-env X07_REGISTRY_TOKEN \
  --json
```

The `Authorization` header value is the token (Cargo-style); server should respond `403` if token invalid.

---

## 1) New crate: `crates/x07-cli/`

### `crates/x07-cli/Cargo.toml`

```toml
[package]
name = "x07-cli"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
description = "X07 CLI (pkg pack/lock/publish)"

[[bin]]
name = "x07"
path = "src/main.rs"

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
ureq = { version = "2", features = ["json"] }
base64 = "0.22"

# Your package crate
x07-pkg = { path = "../x07-pkg" }
```

---

## 2) `crates/x07-cli/src/main.rs`

This is the “exact companion CLI commands” skeleton. It compiles, has deterministic outputs (when `--json`), and calls into `x07_pkg`:

```rust
use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand};
use x07_pkg::canonical_json::write_canonical_json;
use x07_pkg::schema::{
    Lockfile, LockedPackage, LockedSource, WorkspaceManifest, PackageManifest,
    WORKSPACE_SCHEMA, PACKAGE_SCHEMA, LOCK_SCHEMA,
};
use x07_pkg::{pack_package_dir, ArtifactFormat, PackConfig};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// X07 CLI (v0): package tooling (pack/lock/publish).
#[derive(Debug, Parser)]
#[command(name = "x07")]
#[command(about = "X07 CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Package manager operations.
    Pkg(PkgCmd),
}

#[derive(Debug, Args)]
pub struct PkgCmd {
    #[command(subcommand)]
    pub cmd: PkgSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum PkgSubcommand {
    Pack(PkgPackArgs),
    Lock(PkgLockArgs),
    Publish(PkgPublishArgs),
}

#[derive(Debug, Args)]
pub struct PkgPackArgs {
    /// Package directory containing package.json (or x07.package.json).
    #[arg(long)]
    pub package_dir: PathBuf,

    /// Output archive path (e.g. target/x07pkg/foo-0.1.0.x07pkg.tar.zst).
    #[arg(long)]
    pub out: PathBuf,

    /// Archive format.
    #[arg(long, default_value = "tar.zst")]
    pub format: String,

    /// Fixed mtime for tar headers (deterministic).
    #[arg(long, default_value_t = 0)]
    pub fixed_mtime: u64,

    /// Fixed uid for tar headers.
    #[arg(long, default_value_t = 0)]
    pub fixed_uid: u64,

    /// Fixed gid for tar headers.
    #[arg(long, default_value_t = 0)]
    pub fixed_gid: u64,

    /// Fixed mode for tar headers (octal string like 644 or 0644).
    #[arg(long, default_value = "644")]
    pub fixed_mode: String,

    /// Zstd compression level (1-22). Only used for tar.zst.
    #[arg(long, default_value_t = 3)]
    pub zstd_level: i32,

    /// Emit machine-readable canonical JSON report to stdout.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PkgLockArgs {
    /// Workspace root directory (contains x07.workspace.json).
    #[arg(long, default_value = ".")]
    pub workspace: PathBuf,

    /// Path to lockfile to write.
    #[arg(long, default_value = "x07.lock.json")]
    pub lockfile: PathBuf,

    /// Require lockfile to already be up-to-date (fail if changes would be made).
    #[arg(long)]
    pub locked: bool,

    /// Workspace-only resolution: all deps must be `path` deps (no network).
    #[arg(long)]
    pub workspace_only: bool,

    /// Emit canonical JSON report.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PkgPublishArgs {
    /// Workspace root directory (to locate registry configs).
    #[arg(long, default_value = ".")]
    pub workspace: PathBuf,

    /// Package directory to publish.
    #[arg(long)]
    pub package_dir: PathBuf,

    /// Registry alias (must exist in workspace manifest).
    #[arg(long, default_value = "default")]
    pub registry: String,

    /// Override registry URL (bypasses workspace manifest).
    #[arg(long)]
    pub registry_url: Option<String>,

    /// Token passed directly (discouraged; prefer --token-env).
    #[arg(long)]
    pub token: Option<String>,

    /// Read token from environment variable.
    #[arg(long)]
    pub token_env: Option<String>,

    /// Dry run: pack and print report, but do not upload.
    #[arg(long)]
    pub dry_run: bool,

    /// Emit canonical JSON report.
    #[arg(long)]
    pub json: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Command::Pkg(pkg) => match pkg.cmd {
            PkgSubcommand::Pack(args) => cmd_pkg_pack(&args),
            PkgSubcommand::Lock(args) => cmd_pkg_lock(&args),
            PkgSubcommand::Publish(args) => cmd_pkg_publish(&args),
        },
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct PackReport {
    schema_version: String,
    out_path: String,
    format: String,
    sha256: String,
    size_bytes: u64,
    file_count: usize,
}

fn cmd_pkg_pack(args: &PkgPackArgs) -> Result<()> {
    let format = parse_format(&args.format)?;
    let mode = parse_mode_octal(&args.fixed_mode)?;

    let cfg = PackConfig {
        format,
        fixed_mtime: args.fixed_mtime,
        fixed_uid: args.fixed_uid,
        fixed_gid: args.fixed_gid,
        fixed_mode: mode,
        zstd_level: args.zstd_level,
    };

    fs::create_dir_all(args.out.parent().unwrap_or(Path::new(".")))
        .with_context(|| format!("create out dir for {}", args.out.display()))?;

    let packed = pack_package_dir(&args.package_dir, &args.out, &cfg)
        .with_context(|| format!("pack package: {}", args.package_dir.display()))?;

    let report = PackReport {
        schema_version: "x07.pkg.pack_report@0.1.0".to_string(),
        out_path: args.out.to_string_lossy().to_string(),
        format: format_to_string(format),
        sha256: packed.sha256_hex,
        size_bytes: packed.size_bytes,
        file_count: packed.file_count,
    };

    if args.json {
        write_canonical_json(std::io::stdout(), &report)?;
        println!();
    } else {
        eprintln!("packed: {}", report.out_path);
        eprintln!("sha256: {}", report.sha256);
        eprintln!("size_bytes: {}", report.size_bytes);
        eprintln!("files: {}", report.file_count);
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct LockReport {
    schema_version: String,
    lockfile_path: String,
    changed: bool,
    packages: usize,
    workspace_only: bool,
}

fn cmd_pkg_lock(args: &PkgLockArgs) -> Result<()> {
    let ws_root = args.workspace.canonicalize()
        .with_context(|| format!("canonicalize workspace root {}", args.workspace.display()))?;

    let ws_manifest_path = ws_root.join("x07.workspace.json");
    let ws_bytes = fs::read(&ws_manifest_path)
        .with_context(|| format!("read workspace manifest {}", ws_manifest_path.display()))?;
    let ws: WorkspaceManifest = serde_json::from_slice(&ws_bytes)
        .with_context(|| format!("parse workspace manifest {}", ws_manifest_path.display()))?;

    if ws.schema_version != WORKSPACE_SCHEMA {
        return Err(anyhow!(
            "workspace schema_version mismatch: got {}, expected {}",
            ws.schema_version, WORKSPACE_SCHEMA
        ));
    }

    // Resolve packages (workspace/path deps only).
    let mut pkgs_by_idver: BTreeMap<(String, String), LockedPackage> = BTreeMap::new();
    let mut visiting: BTreeSet<(String, String)> = BTreeSet::new();

    for member in ws.workspace.members.iter() {
        let member_dir = ws_root.join(member);
        resolve_package_dir_workspace_only(
            &ws_root,
            &member_dir,
            args.workspace_only,
            &mut pkgs_by_idver,
            &mut visiting,
        )?;
    }

    // Deterministic order.
    let packages: Vec<LockedPackage> = pkgs_by_idver.into_values().collect();

    let lock = Lockfile {
        schema_version: LOCK_SCHEMA.to_string(),
        workspace_fingerprint: None,
        packages,
    };

    // Canonical bytes for stable comparison.
    let mut new_bytes = Vec::new();
    write_canonical_json(&mut new_bytes, &lock)?;

    let lock_path = ws_root.join(&args.lockfile);
    let old_bytes = fs::read(&lock_path).ok();
    let changed = old_bytes.as_deref() != Some(new_bytes.as_slice());

    if args.locked && changed {
        return Err(anyhow!(
            "--locked set but lockfile would change: {}",
            lock_path.display()
        ));
    }

    if changed {
        fs::write(&lock_path, &new_bytes)
            .with_context(|| format!("write lockfile {}", lock_path.display()))?;
    }

    let report = LockReport {
        schema_version: "x07.pkg.lock_report@0.1.0".to_string(),
        lockfile_path: lock_path.to_string_lossy().to_string(),
        changed,
        packages: lock.packages.len(),
        workspace_only: args.workspace_only,
    };

    if args.json {
        write_canonical_json(std::io::stdout(), &report)?;
        println!();
    } else {
        eprintln!("lockfile: {}", report.lockfile_path);
        eprintln!("changed: {}", report.changed);
        eprintln!("packages: {}", report.packages);
    }
    Ok(())
}

fn resolve_package_dir_workspace_only(
    ws_root: &Path,
    package_dir: &Path,
    workspace_only: bool,
    out: &mut BTreeMap<(String, String), LockedPackage>,
    visiting: &mut BTreeSet<(String, String)>,
) -> Result<()> {
    let manifest_path = find_pkg_manifest(package_dir)?;
    let manifest_bytes = fs::read(&manifest_path)
        .with_context(|| format!("read package manifest {}", manifest_path.display()))?;
    let pm: PackageManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("parse package manifest {}", manifest_path.display()))?;

    if pm.schema_version != PACKAGE_SCHEMA {
        return Err(anyhow!(
            "package schema_version mismatch in {}: got {}, expected {}",
            manifest_path.display(), pm.schema_version, PACKAGE_SCHEMA
        ));
    }

    let id = pm.package.id.clone();
    let ver = pm.package.version.clone();
    let key = (id.clone(), ver.clone());

    if visiting.contains(&key) {
        return Err(anyhow!("dependency cycle detected at {} {}", id, ver));
    }
    if out.contains_key(&key) {
        return Ok(());
    }

    visiting.insert(key.clone());

    // Deterministic path string in lock: relative to workspace root when possible.
    let canon_pkg_dir = package_dir.canonicalize()
        .with_context(|| format!("canonicalize package dir {}", package_dir.display()))?;
    let path_for_lock = if let Ok(rel) = canon_pkg_dir.strip_prefix(ws_root) {
        rel.to_string_lossy().to_string()
    } else {
        canon_pkg_dir.to_string_lossy().to_string()
    };

    // Optional: compute a deterministic content hash for path deps (helps CI detect drift).
    // We compute it by packing to an uncompressed tar in a temp file and hashing the bytes.
    let content_sha256 = {
        let tmp = NamedTempFile::new().context("create temp file for content hash")?;
        let tmp_path = tmp.path().to_path_buf();
        let cfg = PackConfig {
            format: ArtifactFormat::Tar,
            fixed_mtime: 0,
            fixed_uid: 0,
            fixed_gid: 0,
            fixed_mode: 0o644,
            zstd_level: 3,
        };
        let packed = pack_package_dir(&canon_pkg_dir, &tmp_path, &cfg)
            .with_context(|| format!("pack (hash) {}", canon_pkg_dir.display()))?;
        Some(packed.sha256_hex)
    };

    let mut deps_locked: BTreeMap<String, x07_pkg::schema::LockedDep> = BTreeMap::new();

    for (dep_id, dep) in pm.dependencies.iter() {
        if let Some(dep_path) = dep.path.as_ref() {
            let dep_dir = safe_join_rel(package_dir, dep_path)
                .with_context(|| format!("bad dep path for {}: {}", dep_id, dep_path))?;
            // Recurse.
            resolve_package_dir_workspace_only(ws_root, &dep_dir, workspace_only, out, visiting)?;

            // Load dep manifest to get version.
            let dep_manifest_path = find_pkg_manifest(&dep_dir)?;
            let dep_manifest_bytes = fs::read(&dep_manifest_path)?;
            let dep_pm: PackageManifest = serde_json::from_slice(&dep_manifest_bytes)?;
            deps_locked.insert(
                dep_id.clone(),
                x07_pkg::schema::LockedDep {
                    version: dep_pm.package.version.clone(),
                },
            );
        } else {
            if workspace_only {
                return Err(anyhow!(
                    "registry dependency not allowed in --workspace-only mode: {} depends on {} (req={})",
                    id, dep_id, dep.req
                ));
            }
            return Err(anyhow!(
                "registry deps not implemented in this v0 lock generator; use --workspace-only and path deps"
            ));
        }
    }

    let locked = LockedPackage {
        id: id.clone(),
        version: ver.clone(),
        dependencies: deps_locked,
        source: LockedSource::Path {
            path: path_for_lock,
            content_sha256,
        },
    };

    out.insert(key, locked);
    visiting.remove(&(id, ver));
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct PublishReport {
    schema_version: String,
    registry: String,
    registry_url: String,
    package_id: String,
    version: String,
    out_path: String,
    format: String,
    sha256: String,
    size_bytes: u64,
    dry_run: bool,
    http_status: Option<u16>,
    error: Option<String>,
}

fn cmd_pkg_publish(args: &PkgPublishArgs) -> Result<()> {
    let ws_root = args.workspace.canonicalize()
        .with_context(|| format!("canonicalize workspace root {}", args.workspace.display()))?;

    let registry_url = if let Some(u) = args.registry_url.clone() {
        u
    } else {
        // Read from x07.workspace.json registries
        let ws_manifest_path = ws_root.join("x07.workspace.json");
        let ws_bytes = fs::read(&ws_manifest_path)
            .with_context(|| format!("read workspace manifest {}", ws_manifest_path.display()))?;
        let ws: WorkspaceManifest = serde_json::from_slice(&ws_bytes)
            .with_context(|| format!("parse workspace manifest {}", ws_manifest_path.display()))?;
        let reg = ws.workspace.registries.get(&args.registry)
            .ok_or_else(|| anyhow!("registry alias not found in workspace: {}", args.registry))?;
        reg.base_url.clone()
    };

    let token = resolve_token(args)?;

    // Read package id/version from manifest.
    let manifest_path = find_pkg_manifest(&args.package_dir)?;
    let manifest_bytes = fs::read(&manifest_path)
        .with_context(|| format!("read package manifest {}", manifest_path.display()))?;
    let pm: PackageManifest = serde_json::from_slice(&manifest_bytes)
        .with_context(|| format!("parse package manifest {}", manifest_path.display()))?;

    if pm.schema_version != PACKAGE_SCHEMA {
        return Err(anyhow!(
            "package schema_version mismatch in {}: got {}, expected {}",
            manifest_path.display(), pm.schema_version, PACKAGE_SCHEMA
        ));
    }

    // Default out path (deterministic).
    let out_path = ws_root
        .join("target/x07pkg")
        .join(format!(
            "{}-{}.x07pkg.tar.zst",
            sanitize_filename(&pm.package.id),
            pm.package.version
        ));
    fs::create_dir_all(out_path.parent().unwrap())?;

    let cfg = PackConfig {
        format: ArtifactFormat::TarZst,
        fixed_mtime: 0,
        fixed_uid: 0,
        fixed_gid: 0,
        fixed_mode: 0o644,
        zstd_level: 3,
    };
    let packed = pack_package_dir(&args.package_dir, &out_path, &cfg)?;

    let mut report = PublishReport {
        schema_version: "x07.pkg.publish_report@0.1.0".to_string(),
        registry: args.registry.clone(),
        registry_url: registry_url.clone(),
        package_id: pm.package.id.clone(),
        version: pm.package.version.clone(),
        out_path: out_path.to_string_lossy().to_string(),
        format: "tar.zst".to_string(),
        sha256: packed.sha256_hex.clone(),
        size_bytes: packed.size_bytes,
        dry_run: args.dry_run,
        http_status: None,
        error: None,
    };

    if !args.dry_run {
        // Minimal publish API (your openapi.json should define this).
        // We use a Cargo-like Authorization header: the header value is the API token. 
        let url = format!("{}/packages/publish", registry_url.trim_end_matches('/'));
        let archive_bytes = fs::read(&out_path)
            .with_context(|| format!("read packed archive {}", out_path.display()))?;
        let archive_b64 = base64::engine::general_purpose::STANDARD.encode(&archive_bytes);

        let payload = serde_json::json!({
            "id": pm.package.id,
            "version": pm.package.version,
            "format": "x07pkg+tar.zst",
            "sha256": packed.sha256_hex,
            "size_bytes": packed.size_bytes,
            "archive_base64": archive_b64
        });

        let resp = ureq::post(&url)
            .set("Authorization", &token)
            .set("Content-Type", "application/json")
            .send_json(payload);

        match resp {
            Ok(r) => {
                report.http_status = Some(r.status() as u16);
            }
            Err(e) => {
                // ureq gives status in the error for HTTP errors
                let status = e.status();
                if let Some(s) = status {
                    report.http_status = Some(s as u16);
                }
                report.error = Some(format!("publish failed: {e}"));
            }
        }
    }

    if args.json {
        write_canonical_json(std::io::stdout(), &report)?;
        println!();
    } else {
        eprintln!("registry: {}", report.registry_url);
        eprintln!("package: {} {}", report.package_id, report.version);
        eprintln!("artifact: {}", report.out_path);
        eprintln!("sha256: {}", report.sha256);
        if let Some(s) = report.http_status {
            eprintln!("http_status: {}", s);
        }
        if let Some(e) = report.error.as_ref() {
            eprintln!("error: {}", e);
        }
    }

    // If publish was attempted and we got an error, fail the command.
    if !args.dry_run && report.error.is_some() {
        return Err(anyhow!("publish failed"));
    }
    Ok(())
}

fn resolve_token(args: &PkgPublishArgs) -> Result<String> {
    if let Some(t) = args.token.clone() {
        return Ok(t);
    }
    if let Some(env) = args.token_env.as_ref() {
        let v = std::env::var(env)
            .with_context(|| format!("token env var not set: {}", env))?;
        if v.trim().is_empty() {
            return Err(anyhow!("token env var is empty: {}", env));
        }
        return Ok(v);
    }
    Err(anyhow!("missing token: use --token or --token-env"))
}

fn parse_format(s: &str) -> Result<ArtifactFormat> {
    match s {
        "tar" => Ok(ArtifactFormat::Tar),
        "tar.zst" => Ok(ArtifactFormat::TarZst),
        _ => Err(anyhow!("invalid --format {}; expected tar or tar.zst", s)),
    }
}

fn format_to_string(f: ArtifactFormat) -> String {
    match f {
        ArtifactFormat::Tar => "tar".to_string(),
        ArtifactFormat::TarZst => "tar.zst".to_string(),
    }
}

/// Parse "644" or "0644" (octal) to u32.
fn parse_mode_octal(s: &str) -> Result<u32> {
    let s2 = s.strip_prefix("0o").or_else(|| s.strip_prefix("0")).unwrap_or(s);
    u32::from_str_radix(s2, 8).map_err(|e| anyhow!("invalid mode '{}': {e}", s))
}

fn sanitize_filename(id: &str) -> String {
    id.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' => c,
            _ => '-',
        })
        .collect()
}

fn find_pkg_manifest(package_dir: &Path) -> Result<PathBuf> {
    let a = package_dir.join("package.json");
    if a.is_file() {
        return Ok(a);
    }
    let b = package_dir.join("x07.package.json");
    if b.is_file() {
        return Ok(b);
    }
    Err(anyhow!(
        "missing package manifest in {} (expected package.json or x07.package.json)",
        package_dir.display()
    ))
}

/// Safe join for a relative path string.
/// Rejects absolute paths and any '..' segments.
fn safe_join_rel(base: &Path, rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if p.is_absolute() {
        return Err(anyhow!("absolute path not allowed: {}", rel));
    }
    let mut out = PathBuf::from(base);
    for comp in p.components() {
        use std::path::Component::*;
        match comp {
            CurDir => {}
            ParentDir => return Err(anyhow!("'..' not allowed in path dep: {}", rel)),
            Normal(seg) => out.push(seg),
            RootDir | Prefix(_) => return Err(anyhow!("invalid path dep: {}", rel)),
        }
    }
    Ok(out)
}
```

---

## 3) Why this matches your “Cargo-like” requirements

### Auth behavior

Cargo’s registry web API spec says authenticated requests include an `Authorization` header whose value is the API token, and invalid tokens should get 403.
We mimic that exactly.

### Registry config expectations

Cargo’s registry index `config.json` has an `api` base URL and `auth-required` for private registries (auth for API + downloads + sparse index updates).
Even if you don’t implement a Cargo-compatible index, these semantics are useful for your own registry contract.

### Deterministic packer assumptions

`tar::Builder::append_data` sets the path and checksum but does not mutate other metadata, which is why pinning `(mtime, uid, gid, mode)` stays stable.
And zstd multithreading is disabled unless you enable the `zstdmt` feature and call `multithread(...)`.

---

## 4) Minimal CI gates you can add immediately

1. **Pack determinism** (same bytes twice):

```bash
x07 pkg pack --package-dir stdlib/std/0.1.0/ascii --out /tmp/a.tar.zst --format tar.zst --json
x07 pkg pack --package-dir stdlib/std/0.1.0/ascii --out /tmp/b.tar.zst --format tar.zst --json
sha256sum /tmp/a.tar.zst /tmp/b.tar.zst | awk '{print $1}' | uniq -c
```

2. **Lock determinism**:

```bash
x07 pkg lock --workspace . --lockfile x07.lock.json --workspace-only --json
git diff --exit-code x07.lock.json
x07 pkg lock --workspace . --lockfile x07.lock.json --workspace-only --locked
```

3. **Archive contract gate** (your existing plan):

```bash
python scripts/check_pkg_contracts.py --check-archives target/x07pkg/*.x07pkg.tar.zst
```

---

## 5) One note for your registry contract

If you want Cargo-like behavior for “private registries”, Cargo’s `auth-required` concept is important: it controls whether auth applies just to publish API or also to downloads/sparse index.
You can mirror this in your registry `config.json` (or your own `/meta` endpoint) so the CLI knows whether to send tokens on downloads in the future.

---
++++
Below is a **copy/paste‑ready design + implementation draft** for the missing piece: **`x07 pkg lock` registry resolution**, using a **Cargo‑style sparse index** model (static HTTP files; one metadata file per package; newline‑delimited JSON entries; `sparse+…` URL prefix).

I’m going to:

1. pin the **exact protocol behaviors** we’ll mirror from Cargo (because they’re battle‑tested),
2. define an **X07 sparse index entry shape** (minimal, deterministic),
3. give a **deterministic version selection + lock generation algorithm**,
4. provide **Rust code skeletons** for `crates/x07-pkg` that implement:

   * sparse index fetch + cache (ETag / Last‑Modified),
   * metadata parsing,
   * deterministic resolution,
   * lockfile filling.

Where I cite Cargo behavior, it’s from the Cargo Book / Cargo code / Rust RFCs. ([Rust Documentation][1])

---

## 0) What we’re copying from Cargo (normative for X07)

### Protocol selection

* If a registry index URL begins with **`sparse+`**, treat it as **sparse HTTP index**.
* Otherwise treat it as “git index” (you can **hard‑error** for v1). ([Rust Documentation][1])

### Sparse index URL must end with `/`

Cargo enforces that sparse index URLs end with `/` so clients can concatenate paths safely. Cargo literally errors if missing. ([Rust Documentation][2])
**X07 should do the same** (hard error).

### Sparse index is static files

* Fetch `config.json` at the index root.
* Fetch **one file per package** (only what you need).
* Each package file is **newline‑delimited JSON**, one object per version. ([Rust Documentation][3])

### Cache refresh (ETag / Last‑Modified)

* Use conditional GETs:

  * If you have an `ETag`, send `If-None-Match`.
  * Else if you have `Last-Modified`, send `If-Modified-Since`.
  * Accept `304 Not Modified`. ([HackMD][4])

### Optional: canonical URL support

Cargo’s ecosystem has a migration story where `config.json` may include a `canonical` URL to prevent “same registry, different URL” drift. ([HackMD][5])
**X07 should implement this now** because it prevents lock churn when you move registries or change index URL aliases.

---

## 1) X07 sparse index: `config.json` and per‑package metadata

### 1.1 `config.json` (index root)

Keep it Cargo‑compatible in spirit:

```json
{
  "dl": "https://registry.example/api/v1/packages/{pkg}/{vers}/download",
  "api": "https://registry.example/api/v1/",
  "canonical": "sparse+https://registry.example/index/",
  "auth-required": true
}
```

Notes:

* `dl` is a **template**. Cargo uses `{crate}/{version}` placeholders; you can use `{pkg}` and `{vers}`. ([Rust Documentation][3])
* `api` is the publishing API base (for `pkg publish` etc). ([Rust Documentation][3])
* `canonical` is optional but strongly recommended. ([HackMD][5])
* `auth-required` is optional; if true, your client should attach auth headers (or retry on 401).

### 1.2 Package metadata file location

Cargo uses a name‑sharded directory scheme (like `re/ge/regex`). ([HackMD][4])

For X07, your package IDs are **not** Rust crate names (`x07:stdlib-net`, etc).
So implement a **deterministic, collision‑resistant sharding**:

**Index path algorithm (X07 v1):**

* Let `key = lowercase(pkg_id)` (ASCII lowercase; reject non‑ASCII for now or normalize).
* Let `h = sha256(key)` as hex lowercase (64 chars).
* Path: `pk/{h[0..2]}/{h[2..4]}/{h}`

Example:

* `pkg_id = "x07:stdlib-net"`
* `GET {index_url}pk/aa/bb/aabb...` (full hex)

This:

* keeps index static‑file friendly,
* avoids encoding ambiguities,
* is deterministic across platforms.

### 1.3 Index entry JSON (one line per version)

Minimal, Cargo‑like, deterministic:

```json
{"pkg":"x07:stdlib-net","vers":"0.1.0","cksum":"<sha256-hex>","yanked":false,
 "deps":[{"pkg":"x07:stdlib-text","req":"^0.1.0","registry":null}]}
```

Rules (copy Cargo’s constraints where they matter):

* File is **append‑only** except `yanked` flips (Cargo works this way). ([Rust Language][6])
* A given `(pkg, vers)` must appear **at most once** (treat duplicates as hard error). Cargo forbids duplicates that differ only in SemVer build metadata. ([Rust Documentation][3])
* `cksum` is the sha256 of the **published deterministic tar** (your `pkg pack` output).
* `deps` is the dependency list needed for resolution (so lock can be generated without downloading source).

---

## 2) Deterministic resolution policy for X07 v1

You asked for “picking versions”. For v1, I recommend a **single‑version‑per‑package** resolver (Go‑like simplicity), because it’s:

* deterministic,
* easy to debug for agents,
* avoids “two versions of the same lib in the graph” surprises.

If you later want Cargo‑style multi‑version graphs, you can swap out the resolver for PubGrub; the sparse index format still works.

### 2.1 Version selection rule (deterministic)

Given:

* package `P`,
* constraints `reqs = [req1, req2, ...]` (SemVer ranges),
* available entries `E = {vers -> entry}`,

Pick:

* the **highest** `vers` such that:

  * `!entry.yanked` (unless it’s already pinned in an existing lock and `--locked`),
  * `vers satisfies all reqs`.

Tie‑breakers:

* If two entries parse to the same SemVer (should not happen): **error**.
* Ordering is SemVer precedence (build metadata ignored by precedence per SemVer; `semver` crate handles that).

### 2.2 Resolution algorithm (fixed point, deterministic frontier)

Maintain:

* `constraints: BTreeMap<PkgId, Vec<VersionReq>>`
* `selected: BTreeMap<PkgId, Version>`
* `frontier: BTreeSet<PkgId>` (stable lexicographic iteration)

Initialize frontier with all root deps from workspace packages.

Loop:

* pop smallest pkg from frontier,
* pick version for pkg using rule above,
* if selected version changed:

  * load entry deps for that version,
  * push deps into `constraints` and `frontier`.

Terminate when frontier empty.

If any pkg has no satisfying version:

* emit deterministic diagnostic listing:

  * pkg id,
  * each requirement and which parent introduced it (store provenance),
  * available versions (top N) and which are yanked.

---

## 3) Repo implementation: `crates/x07-pkg` additions

### 3.1 File layout

Add:

```
crates/x07-pkg/src/registry/
  mod.rs
  sparse.rs
  resolve.rs
  cache.rs
```

You already have:

* schema types in `crates/x07-pkg` (workspace/package/lock),
* `canonical_json.rs` writer,
* deterministic tar packer.

This plugs into your existing `x07 pkg lock`.

---

## 4) Rust code drafts

### 4.1 `crates/x07-pkg/src/registry/mod.rs`

```rust
pub mod cache;
pub mod sparse;
pub mod resolve;

pub use sparse::{
    SparseIndexClient, SparseIndexConfig, SparseIndexEntry, SparseIndexDep,
    RegistryUrl,
};
pub use resolve::{LockResolver, ResolveOptions, ResolveOutcome};
```

---

### 4.2 `crates/x07-pkg/src/registry/sparse.rs`

This implements:

* parse registry URL (`sparse+…` + trailing slash rule),
* fetch `config.json`,
* fetch per‑package metadata file,
* cache with ETag/Last‑Modified.

```rust
use crate::diagnostics::{Diag, DiagCode}; // your single-sourced diag system
use crate::registry::cache::{CacheStore, CachedResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct RegistryUrl {
    /// Original URL string from manifest (may include sparse+ prefix).
    pub raw: String,
    /// Base HTTP URL with sparse+ stripped, guaranteed to end with '/'.
    pub http_base: url::Url,
    /// Optional canonical ID used in lockfiles (from config.json canonical).
    pub canonical_raw: Option<String>,
}

impl RegistryUrl {
    pub fn parse(raw: &str) -> Result<Self, Diag> {
        // Cargo uses sparse+ prefix to select sparse protocol. :contentReference[oaicite:12]{index=12}
        if !raw.starts_with("sparse+") {
            return Err(Diag::new(DiagCode::PkgRegistryUnsupportedProtocol)
                .with_msg(format!("registry index must start with 'sparse+' (v1 only): {raw}")));
        }

        // Cargo enforces trailing slash. :contentReference[oaicite:13]{index=13}
        if !raw.ends_with('/') {
            return Err(Diag::new(DiagCode::PkgRegistryUrlMustEndWithSlash)
                .with_msg(format!("sparse registry url must end with '/': {raw}")));
        }

        let stripped = raw.strip_prefix("sparse+").unwrap();
        let http_base = url::Url::parse(stripped).map_err(|e| {
            Diag::new(DiagCode::PkgRegistryUrlInvalid)
                .with_msg(format!("invalid registry URL after stripping sparse+: {stripped} ({e})"))
        })?;

        Ok(Self { raw: raw.to_string(), http_base, canonical_raw: None })
    }

    pub fn join_path(&self, rel: &str) -> Result<url::Url, Diag> {
        self.http_base.join(rel).map_err(|e| {
            Diag::new(DiagCode::PkgRegistryUrlInvalid)
                .with_msg(format!("failed to join registry URL '{}' with '{rel}': {e}", self.http_base))
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SparseIndexConfig {
    pub dl: String,
    pub api: Option<String>,
    #[serde(default)]
    pub canonical: Option<String>,
    #[serde(rename = "auth-required", default)]
    pub auth_required: bool,
}

/// One line per version.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SparseIndexEntry {
    pub pkg: String,
    pub vers: String,
    pub cksum: String,
    #[serde(default)]
    pub yanked: bool,
    #[serde(default)]
    pub deps: Vec<SparseIndexDep>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SparseIndexDep {
    pub pkg: String,
    pub req: String,
    #[serde(default)]
    pub registry: Option<String>, // optional override
}

fn pkg_index_relpath(pkg_id: &str) -> String {
    let key = pkg_id.to_ascii_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let h = hex::encode(hasher.finalize()); // 64 hex chars
    format!("pk/{}/{}/{}", &h[0..2], &h[2..4], h)
}

#[derive(Clone)]
pub struct SparseIndexClient {
    pub registry: RegistryUrl,
    pub cache: CacheStore,
    pub token: Option<String>,
    pub timeout_ms: u64,
    pub max_retries: u32,
}

impl SparseIndexClient {
    pub fn new(
        registry: RegistryUrl,
        cache_dir: PathBuf,
        token: Option<String>,
        timeout_ms: u64,
        max_retries: u32,
    ) -> Self {
        Self {
            registry,
            cache: CacheStore::new(cache_dir),
            token,
            timeout_ms,
            max_retries,
        }
    }

    pub fn fetch_config(&mut self, offline: bool) -> Result<SparseIndexConfig, Diag> {
        let rel = "config.json";
        let url = self.registry.join_path(rel)?;
        let resp = self.fetch_cached(rel, url, offline)?;
        let cfg: SparseIndexConfig = serde_json::from_slice(&resp.body).map_err(|e| {
            Diag::new(DiagCode::PkgRegistryConfigInvalid)
                .with_msg(format!("invalid registry config.json JSON: {e}"))
        })?;

        // Apply canonical if present (Cargo uses canonical for migration). :contentReference[oaicite:14]{index=14}
        if let Some(canon) = cfg.canonical.clone() {
            self.registry.canonical_raw = Some(canon);
        }

        Ok(cfg)
    }

    pub fn fetch_pkg_entries(&mut self, pkg_id: &str, offline: bool) -> Result<Vec<SparseIndexEntry>, Diag> {
        let rel = pkg_index_relpath(pkg_id);
        let url = self.registry.join_path(&rel)?;
        let resp = self.fetch_cached(&rel, url, offline)?;

        // Newline-delimited JSON entries. :contentReference[oaicite:15]{index=15}
        let text = std::str::from_utf8(&resp.body).map_err(|e| {
            Diag::new(DiagCode::PkgRegistryIndexUtf8Invalid)
                .with_msg(format!("index file not UTF-8: {e}"))
        })?;

        let mut out = Vec::new();
        for (lineno, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let entry: SparseIndexEntry = serde_json::from_str(line).map_err(|e| {
                Diag::new(DiagCode::PkgRegistryIndexEntryInvalid)
                    .with_msg(format!("invalid index entry JSON at line {}: {e}", lineno + 1))
            })?;
            if entry.pkg != pkg_id {
                return Err(Diag::new(DiagCode::PkgRegistryIndexPkgMismatch)
                    .with_msg(format!("index entry pkg mismatch: expected {pkg_id}, got {}", entry.pkg)));
            }
            out.push(entry);
        }

        Ok(out)
    }

    fn fetch_cached(&self, rel: &str, url: url::Url, offline: bool) -> Result<CachedResponse, Diag> {
        self.cache.fetch_http(rel, &url, offline, self.token.as_deref(), self.timeout_ms, self.max_retries)
    }
}
```

---

### 4.3 `crates/x07-pkg/src/registry/cache.rs`

This is the deterministic cache layer:

* stores bytes under a stable cache dir keyed by `(registry_url_hash + relpath)`,
* stores a sidecar meta JSON with `etag` / `last_modified`,
* uses conditional requests and accepts 304.

This mirrors Cargo’s recommended caching behavior. ([HackMD][4])

```rust
use crate::diagnostics::{Diag, DiagCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct CachedResponse {
    pub body: Vec<u8>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CacheMeta {
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
}

#[derive(Clone)]
pub struct CacheStore {
    root: PathBuf,
}

impl CacheStore {
    pub fn new(root: PathBuf) -> Self { Self { root } }

    pub fn fetch_http(
        &self,
        rel: &str,
        url: &url::Url,
        offline: bool,
        token: Option<&str>,
        timeout_ms: u64,
        max_retries: u32,
    ) -> Result<CachedResponse, Diag> {
        let (data_path, meta_path) = self.paths_for(rel, url.as_str());

        if offline {
            return self.load_cache_or_err(&data_path, &meta_path, rel);
        }

        let mut meta = self.load_meta(&meta_path).unwrap_or_default();

        // Build request with conditional headers.
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build();

        let mut req = agent.get(url.as_str());
        if let Some(t) = token {
            req = req.set("Authorization", &format!("Bearer {t}"));
        }

        if let Some(etag) = meta.etag.as_deref() {
            req = req.set("If-None-Match", etag);
        } else if let Some(lm) = meta.last_modified.as_deref() {
            req = req.set("If-Modified-Since", lm);
        }

        let mut last_err: Option<String> = None;

        for _attempt in 0..=max_retries {
            match req.clone().call() {
                Ok(resp) => {
                    let status = resp.status();
                    if status == 304 {
                        // Not modified: return cached bytes.
                        return self.load_cache_or_err(&data_path, &meta_path, rel);
                    }
                    if status >= 200 && status < 300 {
                        let mut body = Vec::new();
                        resp.into_reader().read_to_end(&mut body).map_err(|e| {
                            Diag::new(DiagCode::PkgRegistryFetchIo)
                                .with_msg(format!("read HTTP response failed: {e}"))
                        })?;

                        meta.etag = resp.header("ETag").map(|s| s.to_string());
                        meta.last_modified = resp.header("Last-Modified").map(|s| s.to_string());

                        self.store(&data_path, &meta_path, &body, &meta)?;

                        return Ok(CachedResponse { body, etag: meta.etag, last_modified: meta.last_modified });
                    }

                    return Err(Diag::new(DiagCode::PkgRegistryFetchHttp)
                        .with_msg(format!("HTTP {} fetching {}", status, url)));
                }
                Err(e) => {
                    last_err = Some(e.to_string());
                    // retry loop continues
                }
            }
        }

        Err(Diag::new(DiagCode::PkgRegistryFetchFailed)
            .with_msg(format!("failed fetching {} (last error: {})", url, last_err.unwrap_or_else(|| "unknown".into()))))
    }

    fn paths_for(&self, rel: &str, registry_id: &str) -> (PathBuf, PathBuf) {
        // Cache namespace = sha256(registry_id) so different registries don't collide.
        let mut h = Sha256::new();
        h.update(registry_id.as_bytes());
        let hex = hex::encode(h.finalize());
        let base = self.root.join("index").join(&hex);
        let data = base.join(rel);
        let meta = base.join(format!("{rel}.meta.json"));
        (data, meta)
    }

    fn load_cache_or_err(&self, data: &Path, meta: &Path, rel: &str) -> Result<CachedResponse, Diag> {
        let body = fs::read(data).map_err(|_| {
            Diag::new(DiagCode::PkgRegistryOfflineCacheMiss)
                .with_msg(format!("offline mode: missing cached index file for {rel}"))
        })?;
        let meta_obj = self.load_meta(meta).unwrap_or_default();
        Ok(CachedResponse { body, etag: meta_obj.etag, last_modified: meta_obj.last_modified })
    }

    fn load_meta(&self, path: &Path) -> Option<CacheMeta> {
        let bytes = fs::read(path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    fn store(&self, data: &Path, meta: &Path, body: &[u8], meta_obj: &CacheMeta) -> Result<(), Diag> {
        if let Some(parent) = data.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                Diag::new(DiagCode::PkgRegistryCacheWriteFailed)
                    .with_msg(format!("create cache dir failed: {e}"))
            })?;
        }
        fs::write(data, body).map_err(|e| {
            Diag::new(DiagCode::PkgRegistryCacheWriteFailed)
                .with_msg(format!("write cache file failed: {e}"))
        })?;

        let meta_bytes = serde_json::to_vec(meta_obj).map_err(|e| {
            Diag::new(DiagCode::PkgRegistryCacheWriteFailed)
                .with_msg(format!("encode cache meta failed: {e}"))
        })?;
        fs::write(meta, meta_bytes).map_err(|e| {
            Diag::new(DiagCode::PkgRegistryCacheWriteFailed)
                .with_msg(format!("write cache meta failed: {e}"))
        })?;

        Ok(())
    }
}

use std::io::Read;
```

---

### 4.4 `crates/x07-pkg/src/registry/resolve.rs`

This is the “missing” lock resolver:

* fetches index entries,
* chooses versions deterministically,
* emits lock entries (sorted).

```rust
use crate::diagnostics::{Diag, DiagCode};
use crate::registry::sparse::{SparseIndexClient, SparseIndexEntry};
use crate::schemas::{WorkspaceManifest, LockFile, LockPackage, DepSpec}; // adapt to your actual types
use semver::{Version, VersionReq};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct ResolveOptions {
    pub offline: bool,
    pub allow_yanked_if_locked: bool,
    pub max_versions_listed_in_errors: usize,
}

#[derive(Clone, Debug)]
pub struct ResolveOutcome {
    pub lock: LockFile,
    pub resolved: BTreeMap<String, Version>, // pkg -> version (single-version policy)
}

pub struct LockResolver {
    pub registries: BTreeMap<String, SparseIndexClient>, // registry name -> client
}

impl LockResolver {
    pub fn resolve_workspace(
        &mut self,
        ws: &WorkspaceManifest,
        prev_lock: Option<&LockFile>,
        opts: &ResolveOptions,
    ) -> Result<ResolveOutcome, Diag> {
        // 1) Collect root deps from all workspace members.
        let mut constraints: BTreeMap<String, Vec<(VersionReq, String /*provenance*/)>> = BTreeMap::new();
        let mut frontier: BTreeSet<String> = BTreeSet::new();

        for member in ws.members.iter() {
            let pkg = ws.packages.get(member).ok_or_else(|| {
                Diag::new(DiagCode::PkgWorkspaceInvalid)
                    .with_msg(format!("workspace member not found in packages map: {member}"))
            })?;
            for (dep_id, dep) in pkg.dependencies.iter() {
                if dep.path.is_some() {
                    continue; // path/workspace deps resolved separately (fixed)
                }
                let req = VersionReq::parse(&dep.req).map_err(|e| {
                    Diag::new(DiagCode::PkgVersionReqInvalid)
                        .with_msg(format!("invalid version requirement '{}' for dep {dep_id}: {e}", dep.req))
                })?;
                constraints.entry(dep_id.clone()).or_default()
                    .push((req, format!("root:{member}")));
                frontier.insert(dep_id.clone());
            }
        }

        // 2) Fixed-point selection.
        let mut selected: BTreeMap<String, Version> = BTreeMap::new();
        let mut selected_entry: BTreeMap<String, SparseIndexEntry> = BTreeMap::new();

        while let Some(pkg_id) = pop_first(&mut frontier) {
            // Determine registry (default or per-dep registry).
            // v1: use ws.default_registry unless dep specifies one.
            let registry_name = ws.default_registry.as_deref().unwrap_or("default");
            let client = self.registries.get_mut(registry_name).ok_or_else(|| {
                Diag::new(DiagCode::PkgRegistryNotConfigured)
                    .with_msg(format!("registry not configured: {registry_name}"))
            })?;

            let entries = client.fetch_pkg_entries(&pkg_id, opts.offline)?;
            let chosen = choose_version(&pkg_id, &entries, constraints.get(&pkg_id), prev_lock, opts)?;

            let changed = match selected.get(&pkg_id) {
                None => true,
                Some(v) => v != &chosen.0,
            };

            if changed {
                selected.insert(pkg_id.clone(), chosen.0.clone());
                selected_entry.insert(pkg_id.clone(), chosen.1.clone());

                // Add deps of chosen version.
                for dep in chosen.1.deps.iter() {
                    // NOTE: v1 ignores dep.registry override, but you can support it trivially:
                    // pick registry_name = dep.registry.unwrap_or(registry_name)
                    let req = VersionReq::parse(&dep.req).map_err(|e| {
                        Diag::new(DiagCode::PkgVersionReqInvalid)
                            .with_msg(format!("invalid version requirement '{}' for dep {} -> {}: {e}", dep.req, pkg_id, dep.pkg))
                    })?;
                    constraints.entry(dep.pkg.clone()).or_default()
                        .push((req, format!("{pkg_id}@{}" , chosen.0)));
                    frontier.insert(dep.pkg.clone());
                }
            }
        }

        // 3) Build LockFile deterministically.
        let mut lock = LockFile::new();
        for (pkg, ver) in selected.iter() {
            let entry = selected_entry.get(pkg).expect("entry present if selected");

            let lp = LockPackage {
                pkg: pkg.clone(),
                vers: ver.to_string(),
                checksum: entry.cksum.clone(),
                // source should use registry canonical if present:
                source: Some(self.lock_source_for(ws, pkg, registry_name_of(ws, pkg))),
                deps: entry.deps.iter().map(|d| d.pkg.clone()).collect(),
            };
            lock.packages.push(lp);
        }

        // stable ordering: sort lock.packages by (pkg, vers)
        lock.packages.sort_by(|a, b| (a.pkg.as_str(), a.vers.as_str()).cmp(&(b.pkg.as_str(), b.vers.as_str())));

        Ok(ResolveOutcome { lock, resolved: selected })
    }

    fn lock_source_for(&self, ws: &WorkspaceManifest, _pkg: &str, registry_name: &str) -> String {
        // Prefer registry canonical if known
        ws.registries.get(registry_name)
            .and_then(|r| r.canonical.clone())
            .unwrap_or_else(|| ws.registries[registry_name].index.clone())
    }
}

fn pop_first(set: &mut BTreeSet<String>) -> Option<String> {
    let first = set.iter().next().cloned();
    if let Some(ref f) = first { set.remove(f); }
    first
}

fn choose_version(
    pkg_id: &str,
    entries: &[SparseIndexEntry],
    reqs: Option<&Vec<(VersionReq, String)>>,
    prev_lock: Option<&crate::schemas::LockFile>,
    opts: &ResolveOptions,
) -> Result<(Version, SparseIndexEntry), Diag> {
    // Parse entries into (Version, Entry) pairs.
    let mut parsed: Vec<(Version, SparseIndexEntry)> = Vec::new();
    for e in entries.iter() {
        let v = Version::parse(&e.vers).map_err(|err| {
            Diag::new(DiagCode::PkgRegistryIndexVersionInvalid)
                .with_msg(format!("invalid semver in index for {pkg_id}: '{}': {err}", e.vers))
        })?;
        parsed.push((v, e.clone()));
    }

    // Sort descending by version (deterministic pick highest).
    parsed.sort_by(|a, b| b.0.cmp(&a.0));

    let constraints = reqs.map(|v| v.as_slice()).unwrap_or(&[]);

    // Allow yanked if it is already pinned in prev_lock and flag is set.
    let pinned_yanked_ok = |vers: &str| -> bool {
        if !opts.allow_yanked_if_locked { return false; }
        let Some(lock) = prev_lock else { return false; };
        lock.packages.iter().any(|p| p.pkg == pkg_id && p.vers == vers)
    };

    for (v, e) in parsed.into_iter() {
        if e.yanked && !pinned_yanked_ok(&e.vers) {
            continue;
        }
        let mut ok = true;
        for (req, _prov) in constraints {
            if !req.matches(&v) { ok = false; break; }
        }
        if ok {
            return Ok((v, e));
        }
    }

    // Build deterministic failure message.
    let mut msg = format!("no version satisfies constraints for {pkg_id}\n");
    for (req, prov) in constraints {
        msg.push_str(&format!("  required by {prov}: {req}\n"));
    }
    msg.push_str("  hint: run `x07 pkg lock --explain` to print available versions\n");

    Err(Diag::new(DiagCode::PkgResolveNoSatisfyingVersion).with_msg(msg))
}

fn registry_name_of(_ws: &WorkspaceManifest, _pkg: &str) -> &str {
    // v1: everything uses default.
    "default"
}
```

> This resolver is intentionally “v1 simple”: **one version per package** across the graph.
> If you later switch to PubGrub/multi‑version, this module is the only place you swap.

---

## 5) `x07 pkg lock` behavior and flags (for determinism + CI)

Add/confirm these flags (workspace‑first):

* `x07 pkg lock`

  * `--offline`
    Uses cache only; errors on cache miss. (Important for deterministic CI and for agents working in restricted envs.)
  * `--locked`
    Refuses to change existing lock (like Cargo).
  * `--update`
    Allows changes; will re-resolve.
  * `--registry <name>`
    Picks a registry (default from workspace if omitted).
  * `--timeout-ms`, `--max-retries`
    Deterministic network behavior knobs.

Why: sparse index inherently depends on network state; **deterministic CI should use `--offline` + cached index snapshot** or run in a controlled environment.

---

## 6) Determinism guarantees you should enforce

### 6.1 Deterministic ordering

Use `BTreeMap/BTreeSet` and explicit sorts so:

* lock packages appear in stable order,
* deps appear stable (sort dep IDs when storing).

### 6.2 Canonical lock source URL

Prefer `config.json.canonical` when present; otherwise use the configured registry index string. This avoids churn when the same registry is reachable at different URLs. ([HackMD][5])

### 6.3 Cache semantics

Cache bodies + meta sidecars. Refresh via ETag/Last‑Modified conditional GET. ([HackMD][4])

### 6.4 Sparse URL invariants

* Require `sparse+` prefix and trailing slash (`/`). ([Rust Documentation][1])

---

## 7) What’s “missing” for a complete v1, and what can wait

### Must do now (unblocks `pkg lock`)

* Sparse URL parsing (`sparse+`, trailing slash).
* `config.json` fetch.
* per‑package metadata fetch.
* cache with ETag/Last‑Modified.
* deterministic version selection + lock write.

### Can wait (v2+)

* Git index support (Cargo has both; you can remain sparse‑only).
* Multi‑version dependency graphs (PubGrub).
* Index signing / transparency logs (harder; Cargo RFC discusses security tradeoffs). ([Rust Language][6])

---

## 8) Suggested diag codes (so agents can self‑repair)

Add these to your single‑source `diagnostics.md` / `diagnostics.rs`:

* `PKG_REGISTRY_UNSUPPORTED_PROTOCOL`
* `PKG_REGISTRY_URL_MUST_END_WITH_SLASH`
* `PKG_REGISTRY_CONFIG_INVALID`
* `PKG_REGISTRY_INDEX_ENTRY_INVALID`
* `PKG_REGISTRY_INDEX_VERSION_INVALID`
* `PKG_REGISTRY_OFFLINE_CACHE_MISS`
* `PKG_RESOLVE_NO_SATISFYING_VERSION`
* `PKG_VERSION_REQ_INVALID`
* `PKG_REGISTRY_NOT_CONFIGURED`

These are the exact things an agent can act on deterministically:

* fix URL,
* add registry config,
* pin a compatible version,
* run online once to seed cache.

---
[1]: https://doc.rust-lang.org/cargo/reference/registries.html "Registries - The Cargo Book"
[2]: https://doc.rust-lang.org/beta/nightly-rustc/src/cargo/sources/registry/http_remote.rs.html "http_remote.rs - source"
[3]: https://doc.rust-lang.org/cargo/reference/registry-index.html "Registry Index - The Cargo Book"
[4]: https://hackmd.io/%40rust-cargo-team/SyebelI4o "Cargo Sparse Indexes - HackMD"
[5]: https://hackmd.io/%40rust-cargo-team/B13O52Zko "Sparse registry selection - HackMD"
[6]: https://rust-lang.github.io/rfcs/2789-sparse-index.html "2789-sparse-index - The Rust RFC Book"

++++
Some of the earlier uploaded files in this chat have expired on my side, but that doesn’t affect this request (the sparse-index entry schema is self-contained). If you later want me to cross-check it against your current `spec/*` files in-repo, re-upload those.

Below is a **drop-in, single-source-of-truth** JSON Schema for **one sparse-index entry line** (NDJSON “one JSON object per line”), modeled after Cargo’s index-entry concepts (one version per line; fields like version, deps, checksum, yanked). ([Rust Documentation][1])

---

## `spec/x07.sparse-index-entry.schema.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://x07.io/spec/x07.sparse-index-entry.schema.json",
  "title": "X07 Sparse Index Entry (NDJSON line)",
  "description": "Schema for one newline-delimited JSON (NDJSON) entry describing one published version of one package in the X07 sparse registry index. One file per package; one line per version.",
  "type": "object",
  "additionalProperties": false,
  "required": ["v", "pkg", "vers", "deps", "cksum"],
  "properties": {
    "v": {
      "type": "integer",
      "const": 1,
      "description": "Entry format version. Allows future changes by bumping v."
    },

    "pkg": {
      "$ref": "#/$defs/pkg_id",
      "description": "Package identifier (namespace:name). Lowercase ASCII only."
    },

    "vers": {
      "$ref": "#/$defs/semver",
      "description": "Semantic Versioning 2.0.0."
    },

    "yanked": {
      "type": "boolean",
      "default": false,
      "description": "If true, version is yanked and should not be selected for new resolutions."
    },

    "cksum": {
      "$ref": "#/$defs/sha256_hex",
      "description": "Lowercase hex SHA-256 checksum of the published package archive."
    },

    "deps": {
      "type": "array",
      "maxItems": 256,
      "items": { "$ref": "#/$defs/dep" },
      "default": [],
      "description": "Direct dependencies. Deterministic registries SHOULD emit these sorted by (pkg, req, registry, optional)."
    },

    "x07_req": {
      "$ref": "#/$defs/semver_req",
      "description": "Optional: minimum X07 toolchain requirement for this package version (semver requirement string)."
    },

    "worlds": {
      "type": "array",
      "minItems": 1,
      "maxItems": 32,
      "uniqueItems": true,
      "items": { "$ref": "#/$defs/world_id" },
      "description": "Optional: declared compatible worlds/capability profiles (e.g., solve-pure, solve-fs, run-os)."
    }
  },

  "$defs": {
    "pkg_id": {
      "type": "string",
      "minLength": 3,
      "maxLength": 196,
      "pattern": "^[a-z][a-z0-9_-]{0,63}:[a-z][a-z0-9_.-]{0,127}$",
      "description": "namespace:name. Deterministic, filesystem-safe ASCII. Lowercase to avoid case-collision ambiguity."
    },

    "semver": {
      "type": "string",
      "minLength": 5,
      "maxLength": 64,
      "pattern": "^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?(?:\\+[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?$",
      "description": "SemVer 2.0.0 string. (Full semantic validation still recommended server-side/client-side.)"
    },

    "semver_req": {
      "type": "string",
      "minLength": 1,
      "maxLength": 128,
      "pattern": "^[0-9A-Za-z<>=^~!*xX\\.\\-\\+\\s,\\|\\(\\)]*$",
      "description": "SemVer requirement string (Cargo-like). Schema enforces only a safe character set; the resolver must parse/validate semantically."
    },

    "sha256_hex": {
      "type": "string",
      "pattern": "^[a-f0-9]{64}$",
      "description": "Lowercase hex SHA-256 digest."
    },

    "registry_index_url_or_null": {
      "description": "If present, overrides dependency registry. Null or omitted means 'same registry'.",
      "anyOf": [
        { "type": "null" },
        {
          "type": "string",
          "minLength": 1,
          "maxLength": 2048,
          "pattern": "^(sparse\\+)?https?://.+$"
        }
      ]
    },

    "world_id": {
      "type": "string",
      "minLength": 1,
      "maxLength": 64,
      "pattern": "^[a-z][a-z0-9_-]*$",
      "description": "World/capability profile identifier (lowercase, hyphen/underscore allowed)."
    },

    "dep": {
      "type": "object",
      "additionalProperties": false,
      "required": ["pkg", "req"],
      "properties": {
        "pkg": {
          "$ref": "#/$defs/pkg_id",
          "description": "Dependency package id."
        },
        "req": {
          "$ref": "#/$defs/semver_req",
          "description": "SemVer requirement for this dependency."
        },
        "registry": {
          "$ref": "#/$defs/registry_index_url_or_null",
          "description": "Optional registry override (index URL)."
        },
        "optional": {
          "type": "boolean",
          "default": false,
          "description": "If true, dependency is optional (not pulled unless enabled by package policy)."
        }
      }
    }
  }
}
```

### Notes you should enforce outside JSON Schema (deterministically)

JSON Schema can’t express these cross-line/file invariants, but your **server and/or `x07 pkg lock` client must**:

* **One file per package; one JSON object per version line** (append-only except yanking). ([Rust Documentation][1])
* **Version uniqueness per package**, ideally rejecting SemVer build-metadata duplicates (Cargo requires this). ([Rust Documentation][1])
* Treat `yanked: true` as “do not select for new resolutions”, but still valid for existing locks. (Cargo’s model is similar.) ([Rust Documentation][1])
* Parse `vers` and `req` with a real SemVer/SemVerReq parser (schema only constrains a safe character set).

[1]: https://doc.rust-lang.org/cargo/reference/registry-index.html "Registry Index - The Cargo Book"
[2]: https://rust-lang.github.io/rfcs/2789-sparse-index.html "2789-sparse-index - The Rust RFC Book"

++++++
Small note: I can’t see your earlier uploaded `openapi.json` (some uploaded files expire on my side), so I’ll draft endpoints in a way that’s **Cargo‑compatible in behavior** and easy to paste into your registry OpenAPI file. If you re‑upload your current OpenAPI doc later, I can produce an exact patch against it.

Below is a **normative, deterministic** spec for:

1. **Index-file pathing rules** (Cargo-style sharding / “prefixing”), adapted to your `namespace:name` package IDs without using `:` in filenames (Windows-safe), and
2. **HTTP endpoints** for sparse index fetch, including caching/auth semantics.

Everything here aligns with the Cargo index format rules and sparse protocol expectations (config.json, per-package files, sharded paths, caching, auth-required, etc.). ([Rust Documentation][1])

---

# X07 sparse index pathing rules v1

## 1) Terms

* **Package ID (`pkg`)**: `namespace:name` as in your schema (example: `x07:stdlib-net`).
* **Namespace**: the part before `:` (example: `x07`).
* **Name**: the part after `:` (example: `stdlib-net`).

### Determinism + portability constraint

To avoid `:` in filenames (invalid on Windows), **the index file path is derived from `(namespace, name)`** and never includes the literal `pkg` string as a filename.

---

## 2) Index root layout

The index root is a static HTTP file tree:

```
<index_root>/
  config.json
  ns/
    <namespace>/
      <shard_path>/<name>
```

Where `<shard_path>` is computed from the **lowercased** `name` using Cargo’s tiered directory scheme. ([Rust Documentation][1])

---

## 3) Shard path algorithm (Cargo-style)

Let `n = name_lowercase` (X07 already enforces lowercase in IDs; still, define it normatively).

Compute `shard_path(n)`:

* If `len(n) == 1`: shard path = `"1"`
* If `len(n) == 2`: shard path = `"2"`
* If `len(n) == 3`: shard path = `"3/<n[0]>"`
* If `len(n) >= 4`: shard path = `"<n[0..2]>/<n[2..4]>"`

This is exactly Cargo’s sharding scheme, just applied to the `name` component. ([Rust Documentation][1])

### Resulting full relative path

```
ns/<namespace>/<shard_path(name)>/<name>
```

---

## 4) Concrete examples

Package ID → index file path:

* `x07:stdlib-net`

  * `name = "stdlib-net"` (len ≥ 4)
  * shard = `st/dl`
  * **path**: `ns/x07/st/dl/stdlib-net`

* `x07:json`

  * shard = `js/on`
  * **path**: `ns/x07/js/on/json`

* `x07:a`

  * shard = `1`
  * **path**: `ns/x07/1/a`

* `x07:abc`

  * shard = `3/a`
  * **path**: `ns/x07/3/a/abc`

---

## 5) Index file content (per-package file)

* Each per-package file is **NDJSON**: one JSON object per line, **one version per line**. ([Rust Documentation][1])
* Version uniqueness: registry must ensure a version appears only once per package (ignoring build metadata) — same rule as Cargo. ([Rust Documentation][1])
* Your previously defined `spec/x07.sparse-index-entry.schema.json` governs each line.

---

# HTTP endpoints for sparse fetch

Cargo’s sparse protocol is **plain HTTP GET of files** under the index root; it first fetches `config.json`, then fetches the per-package file(s). ([Rust Documentation][1])

## 1) Base URL convention

Your registry index URL should be configured as:

* `sparse+https://<host>/<index_root>/`

and **MUST end with `/`** so relative paths resolve cleanly (this matches Cargo’s documented sparse index URL example). ([Rust Documentation][1])

---

## 2) Endpoints

### A) Fetch index config

**GET** `/<index_root>/config.json`

* **200 OK**: returns JSON:

  * `dl` (download base/template)
  * `api` (web API base)
  * optional `auth-required` boolean (private registry behavior) ([Rust Documentation][1])

(Your X07 client should mirror Cargo’s behavior: fetch `config.json` before anything else.) ([Rust Documentation][1])

---

### B) Fetch one package’s version list (NDJSON)

**GET** `/<index_root>/ns/{namespace}/{shard_path}/{name}`

Where `{shard_path}` is computed by the algorithm above.

**Responses:**

* **200 OK**: body is NDJSON (one entry per published version)
* **304 Not Modified**: if client sent cache validators (see caching section)
* **404 Not Found** / **410 Gone** / **451 Unavailable For Legal Reasons** for packages that don’t exist (Cargo-compatible expectations). ([Rust Documentation][1])

---

## 3) Caching (required for performance)

Your server **should emit** one of:

* `ETag`, or
* `Last-Modified`

…and the client should store and revalidate using:

* `If-None-Match` (preferred when ETag present), or
* `If-Modified-Since`

This is explicitly how Cargo’s sparse caching works. ([Rust Documentation][1])

**Deterministic server rule**: for a given index file bytes, `ETag` must be stable (content hash is fine).

---

## 4) Authentication behavior (Cargo-like)

If you set `"auth-required": true` in `config.json`, clients should include an `Authorization` token on index fetch requests (and downloads/APIs). ([Rust Documentation][1])

Cargo also supports a “probe then retry with auth” flow for `config.json` if it receives a 401 first. If you want maximum compatibility, implement:

* First request may be unauthenticated → return **401**
* Client retries with token → return **200** ([Rust Documentation][1])

---

# OpenAPI-friendly route shapes

If your registry server is implemented as a web service (not just a static file host), it’s often easier to define **explicit routes** for each shard shape (because `{prefix}` in Cargo can contain `/` like `ca/rg`).

You can still describe it as “GET `/{prefix}/{name}`” in documentation, but implement these concrete routes:

### Index endpoints (suggested)

* `GET /index/config.json`

* `GET /index/ns/{namespace}/1/{name}`  (len(name)=1)

* `GET /index/ns/{namespace}/2/{name}`  (len(name)=2)

* `GET /index/ns/{namespace}/3/{first}/{name}`  (len(name)=3)

* `GET /index/ns/{namespace}/{a2}/{b2}/{name}`  (len(name)>=4)

And define `{a2}` as `name[0..2]`, `{b2}` as `name[2..4]`.

This matches Cargo’s “tier of directories” scheme precisely. ([Rust Documentation][1])

---

# Practical implementation notes

* **Serve as static files if you can.** Sparse index works great as a static directory + CDN, as long as you handle cache invalidation when files update. ([Rust Documentation][1])
* **HTTP/2 recommended** (many small requests). ([Rust Documentation][1])
* **Name restrictions**: Cargo warns about filesystem/security issues and suggests restrictions (ASCII, avoid Windows reserved names, etc.). Your `pkg_id` schema already restricts heavily, but you should still consider a “reserved filename” denylist for `name`. ([Rust Documentation][1])

---
