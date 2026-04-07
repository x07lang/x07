# Governance

## Scope

This document defines governance for the X07 project, centered on the `x07lang/x07`
repository and the official companion repositories listed below.

Official companion repositories:

- `x07lang/x07-rfcs`
- `x07lang/x07-mcp`
- `x07lang/x07-wasm-backend`
- `x07lang/x07-web-ui`
- `x07lang/x07-device-host`
- `x07lang/x07-platform`
- `x07lang/x07-registry`
- `x07lang/x07-website`

## Current project stage

X07 is currently founder-led and is formalizing open governance.

## Roles

### Project Maintainer

Project maintainers can approve and merge changes, cut releases where delegated, and
participate in governance decisions.

### Core Maintainer

Core maintainers are maintainers with cross-repository authority over language,
compatibility, release, and governance decisions.

### Contributors

Contributors may submit issues, pull requests, RFCs, docs, tests, examples, and
design feedback.

## Current maintainers

See `governance/MAINTAINERS.md`.

## Decision-making

- Routine changes are decided through pull request review.
- Changes requiring an RFC are governed by `x07-rfcs` and
  `governance/RFC-REQUIREMENTS.md`.
- Compatibility, release policy, governance changes, and major breaking changes
  require core-maintainer approval.
- When consensus cannot be reached, the core maintainers decide by majority vote.
- While there is only one core maintainer, the founder acts as the temporary
  tie-break and final approver.

## Becoming a maintainer

A contributor may be nominated as a maintainer when they have:

- sustained contribution history
- demonstrated review quality
- familiarity with X07 compatibility and release policy
- agreement to follow project governance and security processes

The nomination process is:

1. nomination by an existing maintainer
2. public discussion in an issue or PR
3. decision by the core maintainers
4. update of `governance/MAINTAINERS.md`, `OWNERS.md`, and CODEOWNERS

## Governance changes

Changes to governance require:

1. an RFC or governance PR
2. public discussion for at least 7 days
3. approval by the core maintainers

## Repo ownership and delegation

Cross-repository ownership is listed in `OWNERS.md`.
Repository-specific release delegation may be documented in each repo if needed.

## Transparency

Project planning, issues, RFCs, releases, and design records are maintained in
public repositories.
