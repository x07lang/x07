# Deterministic fixture worlds

Fixture worlds exist so agents can:

- run tests deterministically,
- reproduce bugs exactly,
- measure changes reliably.

See also: [Fixture formats](fixture-formats.md).

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

- request/response interactions are replayed from cassette files (`*.rrbin`)
- deterministic latency modeling is supported via `latency_ticks` per entry

### Recording fixtures

To generate a minimal `solve-rr` fixture directory from a real HTTP response, use `x07 rr record`:

```bash
x07 rr record --cassette fixtures/rr/example.rrbin example.com https://example.com
```

This appends an entry to a cassette file under `fixtures/rr/` (see [Fixture formats](fixture-formats.md)).

In `solve-rr`, programs typically open cassettes via `std.rr.with_policy_v1` and then replay entries via `std.rr.next_v1` + `std.rr.entry_resp_v1`.

## Seeded KV (solve-kv)

- the KV store is reset per case from a seeded dataset
- keys and values are bytes
- iteration is deterministic (if exposed)

## Why this matters for agentic coding

If an agent can’t reproduce failures exactly, it can’t repair reliably.

Fixture worlds create the “closed environment” needed for a robust autonomous loop.
