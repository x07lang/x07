# Spec Changelog

Spec-ID: x07.spec.changelog@0.1.0
Status: stable
Applies-to: toolchain >= v0.0.95
Related schemas: ["x07.spec.index@0.1.0"]

## v0.1.111

- Added `x07.project@0.5.0` and the frozen `spec/x07-project.v0.5.0.schema.json` public manifest line (adds `project.compat`).

## v0.1.81

- Added proof-only verification schemas:
  - `x07.verify.proof_summary@0.2.0`
  - `x07.verify.proof_object@0.2.0`
  - `x07.verify.proof_check.report@0.2.0`
- Added `x07.project@0.4.0` and the frozen `spec/x07-project.v0.4.0.schema.json` public manifest line, preserving the existing project world/run-profile controls and adding operational-entry certification fields.
- Bumped formal-verification and trust schemas:
  - `x07.verify.report@0.8.0`
  - `x07.verify.coverage@0.4.0`
  - `x07.verify.summary@0.2.0`
  - `x07.verify.primitives@0.2.0`
  - `x07.review.diff@0.5.0`
  - `x07.trust.profile@0.4.0`
  - `x07.trust.certificate@0.7.0`
- Added tool wrapper schemas for the public proof-check surface:
  - `x07.tool.prove.report@0.2.0`
  - `x07.tool.prove.check.report@0.2.0`
- Proof objects now record source-replay inputs, imported proof-summary digests, primitive-manifest digests, recursion metadata, and async scheduler-model digests so `x07 prove check` can semantically replay the proof.
- Proof-check reports now record replayed obligation and solver truth, and accepted certificates require proof-check success instead of only proof-check artifact presence.
- Certificates now expose explicit formal-verification scope so reviewers can distinguish operational-entry proof from capsule/runtime-backed trust posture.

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
