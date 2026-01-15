# Releases

This document defines what is included in an X07 release.

## Release contents

An X07 release consists of:

- Toolchain binaries (`x07`, `x07c`) for supported platforms.
- Pinned stdlib inputs (`stdlib/**` and `stdlib.lock`).
- Canonical contracts and schemas (`spec/*.schema.json`).
- Release manifest (`dist/release-manifest.json`).

## Verification

Before publishing a release, run:

- `./scripts/ci/check_all.sh`

Signing and provenance rules are defined in `docs/official-builds.md`.
