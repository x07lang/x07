# Decision making

## Default rule: consensus

Most changes should be decided by consensus in PR review and issue discussion.
Consensus means reviewers have had a fair chance to object, substantial objections have
been addressed, and no maintainer has an unresolved blocking concern.

## When an RFC is required

Changes that require an RFC are listed in `governance/RFC-REQUIREMENTS.md`.

## Who can merge

- Maintainers may merge routine changes after the required review is present.
- Compatibility, release policy, governance, and major breaking changes require
  core-maintainer approval.
- CODEOWNERS and repository branch protection enforce the minimum review path for the
  default branch.

## Approval rules

- Non-RFC changes require at least one maintainer approval.
- RFC changes follow the process in `x07lang/x07-rfcs` and require core-maintainer
  approval.
- If a review is dismissed or changes are requested, the blocking state must be resolved
  before merge.

## Disagreements

If consensus cannot be reached:

1. Clarify the decision being made and list the options.
2. Document tradeoffs (correctness, determinism, stability, security, complexity).
3. The core maintainers make the final decision.

While there is only one core maintainer, the founder acts as the temporary tie-break
and final approver.

## Maintainer changes

Maintainers are added through the nomination process defined in `GOVERNANCE.md`.
A maintainer may be removed by core-maintainer decision when they are inactive for a
prolonged period, repeatedly violate project policy, or request removal.
Maintainer additions or removals must update `governance/MAINTAINERS.md`, `OWNERS.md`,
and the affected CODEOWNERS entries.

## Governance changes

Governance changes require:

1. an RFC or governance PR
2. at least 7 days of public discussion
3. approval by the core maintainers

## Releases

Releases are owned by the core maintainers and any delegated release maintainers.
Release artifacts and verification rules are defined in `docs/releases.md` and `docs/official-builds.md`.
