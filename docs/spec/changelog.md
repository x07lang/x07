# Spec Changelog

Spec-ID: x07.spec.changelog@0.1.0
Status: stable
Applies-to: toolchain >= v0.0.95
Related schemas: ["x07.spec.index@0.1.0"]

## v0.0.95

- Introduced published spec sync flow (`scripts/sync_published_spec.py`).
- Added published spec directories:
  - `docs/spec/internal/`
  - `docs/spec/abi/`
  - `docs/spec/schemas/`
- Added machine-readable spec manifest: `docs/spec/spec-index.json`.
- Added machine-readable doc report schema: `x07.doc.report@0.1.0`.
- Added semantic diff report schema: `x07.review.diff@0.1.0`.
- Added trust report schema: `x07.trust.report@0.1.0`.

## v0.1.2

- Added universal tool wrapper report schema: `x07.tool.report@0.1.0` (`spec/x07-tool.report.schema.json`).
- Added shared multi-file patchset schema: `x07.patchset@0.1.0` (`spec/x07.patchset.schema.json`).
- Added `x07 patch apply` command for deterministic repo-level patchset actuation.
- Added `scripts/ci/check_tool_json_contracts.py` gate and wired it into `scripts/ci/check_all.sh`.
