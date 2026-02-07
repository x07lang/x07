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
