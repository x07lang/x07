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

### Recording fixtures

To generate a minimal `solve-rr` fixture directory from a real HTTP response, use `x07 rr record`:

```bash
x07 rr record --out fixtures/rr example.com https://example.com
```

This writes:

- `fixtures/rr/index.json` (fixture index)
- `fixtures/rr/bodies/<sha256(key)>.bin` (raw response body bytes)

Your program can then use `std.rr.fetch(key)` (or `std.rr.send(key)` for streaming) in `solve-rr`, and the runner will resolve `key` through the fixture index.

## Seeded KV (solve-kv)

- the KV store is reset per case from a seeded dataset
- keys and values are bytes
- iteration is deterministic (if exposed)

## Why this matters for agentic coding

If an agent can’t reproduce failures exactly, it can’t repair reliably.

Fixture worlds create the “closed environment” needed for a robust autonomous loop.
