# Guide: Performance tuning

This guide summarizes the tuning knobs that matter in production and ties them back to the scale class your platform selects.

## Canonical tuning checklist

Start from the scale class and tune in the smallest number of places:

### `replicated-http`

Primary knobs:

- concurrency limit per replica
- request body size caps + streaming
- batching where the downstream contract supports it
- connection pool sizing (DB/HTTP clients)

Default guidance:

- keep handlers stateless and bounded
- use explicit timeouts and cancellation
- record idempotency before acknowledging success upstream

### `partitioned-consumer`

Primary knobs:

- partition key choice
- max in-flight per partition
- retry/backoff and dead-letter policy
- lag-based scaling signal (if supported by the runtime)

Default guidance:

- treat delivery as at-least-once unless proven otherwise
- persist dedupe state before ack
- bound retries and surface incidents early

### `singleton-orchestrator`

Primary knobs:

- leader election timeouts
- reconciliation cadence
- backpressure for fan-out work

Default guidance:

- keep orchestration decisions deterministic in a kernel
- make side effects idempotent (retries are unavoidable)

### `burst-batch`

Primary knobs:

- chunk size
- checkpoint frequency
- retry policy per step

Default guidance:

- checkpoint after each irreversible effect
- design for crash/restart and replay safety

## What to measure

Pick a small set of metrics and keep them stable:

- latency (p50/p95/p99) for request work
- error rate
- throughput (rps / events per second)
- queue depth / lag (for consumers)

Always be able to answer:

- “what changed?”
- “is it safe to roll forward?”
- “is it safe to roll back?”

## Related docs

- `docs/guides/scaling-retry-idempotency.md`
- `docs/guides/kernel-shell-production.md`
- `docs/toolchain/benchmarks.md`

## Expert notes

When you need deeper profiling, prefer repeatable harnesses:

- produce machine-readable JSON outputs
- keep configuration explicit and version-controlled
- run in a “dry” mode in CI (no cluster required)

For end-to-end investigation, use the platform’s supported tooling instead of invoking runner binaries directly.
