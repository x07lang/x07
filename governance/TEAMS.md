# Teams

This document defines the project teams and their responsibilities.

## Core team

Responsibilities:

- Owns the overall technical direction of X07.
- Approves RFCs and decides on stabilization and breaking changes.
- Owns release policy and the definition of "official" builds.
- Delegates responsibilities to other teams.

Membership: TBD.

## Toolchain team

Responsibilities:

- Compiler (`x07c`) and CLI (`x07`).
- Deterministic runners (`x07-host-runner`, `x07-os-runner`).
- Diagnostics, JSON contracts, schemas, and compatibility gates.

Membership: TBD.

## Stdlib team

Responsibilities:

- Stdlib modules (`stdlib/**`) and `stdlib.lock`.
- Determinism guarantees and performance baselines for builtins and stdlib.
- Compatibility across capability worlds.

Membership: TBD.

## Packages/registry team

Responsibilities:

- Package formats, lockfiles, registry protocol, and index format.
- `x07 pkg` client implementation and registry tooling.
- Operational policies for publishing (yank/un-yank, moderation primitives).

Membership: TBD.

## Security response team

Responsibilities:

- Security triage and coordinated disclosure handling.
- Advisories, patch releases, and supported-version policy.

Membership: TBD.

