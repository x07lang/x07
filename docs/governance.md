# Governance

X07’s primary goal is long-term reliability for autonomous agents.

That requires:

- stable specs and error codes,
- predictable upgrade paths,
- compatibility policies that are enforced in CI.

## Compatibility

- Pinned `_v1` contracts do not change.
- Breaking changes introduce `_v2` contracts.
- Package versions can add helpers and bug fixes without changing the pinned wire format.

See [Versioning policy](versioning-policy.md).

## Proposing changes

High-impact changes should include:

- a pinned spec doc update,
- smoke tests proving the new behavior,
- migration notes if needed.
