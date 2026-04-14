# Packaging integrity (signing, reproducibility, semver checks)

This guide covers the integrity surfaces around X07 packages:

- registry signing + local verification
- deterministic packaging and lockfiles
- semver-compatible export changes

## Canonical workflow

### Package metadata requirements

Publishing to the public registry requires `x07-package.json` to include:

- `description` (non-empty string)
- `docs` (non-empty string; may be Markdown)
- `license` (non-empty string; SPDX expression recommended)
- `meta.x07c_compat` (required semver range for the minimum toolchain)

Example:

```jsonc
{
  "schema_version": "x07.package@0.1.0",
  "name": "acme-hello-demo",
  "version": "0.1.0",
  "description": "…",
  "license": "MIT OR Apache-2.0",
  "docs": "…",
  "module_root": "modules",
  "modules": ["ext.acme_hello_demo"],
  "meta": {
    "x07c_compat": ">=0.2.2, <0.3.0"
  }
}
```

### Verify signatures (`x07 pkg verify`)

Verify the latest non-yanked version:

```bash
x07 pkg verify NAME
```

Verify a specific version:

```bash
x07 pkg verify NAME@VERSION
```

If the index or package is unsigned, `x07 pkg verify` fails with an actionable error.

### Deterministic lockfiles

`x07 pkg lock` writes `x07.lock.json` deterministically for the same project inputs.

In CI, fail on drift without mutating the workspace:

```bash
x07 pkg lock --project x07.json --check
```

### Semver export checking (`x07 pkg check-semver`)

`x07 pkg check-semver` compares the exported symbols + signatures between two package directories and reports breaking changes:

```bash
x07 pkg check-semver --old path/to/old --new path/to/new
```

Breaking changes include:

- a module removed from `x07-package.json`
- an exported symbol removed from a module
- an exported symbol’s signature changed (params/result type)

### Runnable examples

See `docs/examples/packaging-integrity/` for offline examples of:

- `x07 pkg verify` against a signed `file://` sparse index fixture
- `x07 pkg check-semver` detecting an export/signature break

## Expert appendix

### Registry signing model

The sparse index (`/index/`) can advertise signing keys in `config.json`:

```jsonc
{
  "signing": {
    "kind": "ed25519",
    "public_keys": [{ "id": "…", "ed25519_pub": "…" }]
  }
}
```

When enabled, each index entry line includes a signature:

```jsonc
{
  "name": "pkg",
  "version": "0.1.0",
  "cksum": "<sha256 hex of the package archive bytes>",
  "signature": {
    "kind": "ed25519",
    "key_id": "…",
    "ed25519_sig": "…"
  }
}
```

The signature verifies a stable message:

```text
x07.pkg.sig.v1
name=<name>
version=<version>
sha256=<cksum>
```

### Do we need to republish packages?

No. The toolchain upgrade is compatible with existing packages:

- If an index entry has no signature, `x07 pkg verify` reports it as unsigned with an actionable error.
- Enabling signing only affects *new* publishes. If you want verification to succeed for historical packages too, do a server-side signature backfill (sign existing `name+version+cksum` and update sparse index entries) without re-uploading archives.
- If you want missing metadata (for example `license`) added to already-published versions, publish a new version; do not mutate an existing semver version.

### CI integration

`x07 pkg check-semver` exits with code 20 when it detects breaking changes, so it can be used as a CI gate.

Common pattern: compare the package directory at the PR base commit against the current workspace (exact checkout mechanics depend on your CI system):

```bash
x07 pkg check-semver --old /tmp/pkg-old --new .
```
