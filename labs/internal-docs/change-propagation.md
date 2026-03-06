# Change propagation

This document defines how changes in `x07lang/x07` propagate to other repos.

## Releases

Toolchain releases are the primary synchronization point.

After a release:

- `x07lang/x07-website` is updated to publish the new docs/contracts version.
- `x07lang/x07-registry-web` should mirror any newly public schemas that ship from sibling repos such as `x07-platform` so `https://x07.io/spec/...` stays aligned with the canonical repo copy.

## Current automation entrypoints

- `x07lang/x07/.github/workflows/release.yml` includes optional propagation jobs that run when `X07_BOT_TOKEN` is configured.
- `x07lang/x07/scripts/build_docs_bundle.py` builds a deterministic docs bundle (`dist/x07-docs-<tag>.tar.gz`).
- `x07lang/x07/scripts/open_pr_website_update.py` applies the release bundle to `x07lang/x07-website` (docs + agent) and updates `versions/toolchain_versions.json`.
