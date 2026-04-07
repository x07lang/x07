# Releases

This document defines what is included in an X07 release.

## Release authority

Releases are currently cut by the core maintainer. As additional maintainers are added,
release delegation will be documented in `OWNERS.md` and
`governance/MAINTAINERS.md`.

## Channels

- `stable`: recommended production channel
- `beta`: pre-release validation channel
- `nightly`: automated integration channel

## Release cadence

X07 uses a rolling stable release model with signed tags, published release manifests,
and channel metadata for installers and companion components.
`beta` and `nightly` are used to validate upcoming changes before they are promoted to
`stable`.

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

## Component release relationships

The `x07` release is the root compatibility anchor for the official companion repos:

- `x07-mcp` tracks the released core toolchain and companion package line.
- `x07-wasm-backend`, `x07-web-ui`, and `x07-device-host` publish component releases
  that declare compatibility with supported `x07` core versions.
- `x07-registry` validates the published toolchain and package surfaces that core and
  companion repos depend on.
- `x07-website` publishes the released docs and installer surfaces generated from `x07`.

Companion release delegation and compatibility ranges are documented in the owning repo
and mirrored through `OWNERS.md` and `governance/MAINTAINERS.md`.

## Supported versions

The project supports:

- the latest `stable` release
- the previous `stable` release (N-1)

Older versions may continue to work, but they are outside the actively supported window
unless a repo-specific policy says otherwise.

## Security patch releases

Security fixes are triaged under `SECURITY.md`.
When a supported version requires a security update, the core maintainer cuts a patch
release on the affected supported line, publishes updated manifests, and documents the
release in the public changelog or advisory surface.

## Verification

Before publishing a release, run:

- `./scripts/ci/check_all.sh`

Signing and provenance rules are defined in `docs/official-builds.md`.
Installer/bundle manifest generation rules are defined by `.github/workflows/release.yml`,
`.github/workflows/publish_channels.yml`, and `.github/workflows/nightly.yml`.
