# Deterministic fixture worlds

Fixture worlds exist so agents can:

- run tests deterministically,
- reproduce bugs exactly,
- measure changes reliably.

## Key rules

- no ambient time
- no ambient network
- no ambient filesystem scanning
- all I/O goes through explicit, world-scoped builtins

## Deterministic filesystem (solve-fs)

- the runner mounts a fixture directory as `.` (read-only)
- path resolution is safe and deterministic
- directory listing order is canonicalized

## Request/response cassettes (solve-rr)

- request bytes map to response fixtures
- optional deterministic latency modeling via fixture metadata

## Seeded KV (solve-kv)

- the KV store is reset per case from a seeded dataset
- keys and values are bytes
- iteration is deterministic (if exposed)

## Why this matters for agentic coding

If an agent can’t reproduce failures exactly, it can’t repair reliably.

Fixture worlds create the “closed environment” needed for a robust autonomous loop.
