# Governance

X07 is currently founder-led and is formalizing open governance around the core
toolchain repo and the official companion repositories.

The canonical governance documents live in the repo root:

- `GOVERNANCE.md` for project scope, roles, and decision-making
- `OWNERS.md` for cross-repository ownership
- `governance/MAINTAINERS.md` for current maintainer and release authority
- `governance/DECISION-MAKING.md` for merge, RFC, and escalation rules

## Current project stage

X07 is a young project with one current core maintainer:

- Bohdan Vitomskyi (@webodik)

That transition state is explicit in the governance rules. Routine changes go through
public pull request review, while compatibility, release policy, governance changes, and
major breaking changes require core-maintainer approval.

## Official companion repositories

The governance scope covers these official companion repositories:

- `x07lang/x07-rfcs`
- `x07lang/x07-mcp`
- `x07lang/x07-wasm-backend`
- `x07lang/x07-web-ui`
- `x07lang/x07-device-host`
- `x07lang/x07-platform`
- `x07lang/x07-registry`
- `x07lang/x07-website`

## How decisions are made

- Routine changes are decided in public PR review.
- RFC-required changes follow the `x07-rfcs` process.
- When consensus cannot be reached, the core maintainers decide.
- While there is only one core maintainer, the founder acts as the temporary tie-break
  and final approver.

## Maintainer growth

The project roadmap includes moving from founder-led governance to multi-maintainer,
multi-organization governance.
See the public roadmap for the current milestones and growth metrics.
