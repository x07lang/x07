# Change propagation

This document defines how changes in `x07lang/x07` propagate to other repos.

## Releases

Toolchain releases are the primary synchronization point.

After a release:

- `x07lang/x07-index` is updated to publish new index entries (package ecosystem).
- `x07lang/x07-website` is updated to publish the new docs/contracts version.

Automation for cross-repo propagation is tracked in `docs/phases/x07-production-plan.md`.

## Current automation entrypoints

- `x07lang/x07/.github/workflows/release.yml` includes optional propagation jobs that run when `X07_BOT_TOKEN` is configured.
- `x07lang/x07/scripts/open_pr_website_update.py` updates `x07lang/x07-website/versions.json`.
- `x07lang/x07/scripts/open_pr_index_update.py` updates `x07lang/x07-index/index/config.json` (canonicalization / endpoint updates).
