# Releases

This document defines what is included in an X07 release.

## Release contents

An X07 release consists of:

- Toolchain archives (`x07-<version>-<target>.*`) for supported platforms.
- Installer archives (`x07up-v<version>-<target>.*`) for supported platforms.
- Pinned stdlib inputs (`stdlib/**` and `stdlib.lock`).
- Canonical contracts and schemas (`spec/*.schema.json`).
- Release manifest (`dist/release-manifest.json`).
- Core component release manifest (`dist/release/x07-<version>-release.json`).
- Blessed bundle manifest (`dist/release/x07-<version>-bundle.json`).
- Legacy bootstrap installer manifest (`dist/channels.json`) plus per-channel bundle manifests (`dist/channels/*.json`).

## Verification

Before publishing a release, run:

- `./scripts/ci/check_all.sh`

Signing and provenance rules are defined in `docs/official-builds.md`.
Installer/bundle manifest generation rules are defined by `.github/workflows/release.yml`,
`.github/workflows/publish_channels.yml`, and `.github/workflows/nightly.yml`.
