# Guide: Performance tuning

This guide summarizes the tuning knobs that matter in production and ties them back to the scale class your platform selects.

## Program-level tuning (solve worlds)

Deterministic solve worlds meter execution with a fuel-per-op model: every
executed operation costs fuel, so fuel is a deterministic proxy for CPU work.
Run reports include `fuel_used` and `heap_used` — compare those across program
variants instead of wall-clock time, which is noisy and machine-dependent.

- **Amortize compile overhead.** `x07 run` compiles the program and then
  executes it, so every invocation pays a fixed compile cost that is
  independent of the input. For repeated executions (test sweeps, batch
  inputs), compile once with `x07 build` or package a binary with `x07 bundle`
  and invoke the artifact instead.
- **Owned vs view discipline.** Helpers that take owned `bytes` consume their
  argument; repeated calls then force `view.to_bytes` copies. Prefer `std.*`
  helpers (they take `bytes_view`) and pass an explicit `["bytes.view", x]`
  for reads of owned locals, reserving owned `bytes` for data that is actually
  handed off.
- **vec_u8 accumulate-then-freeze.** Build output with
  `std.vec.with_capacity` + `std.vec.push` / `std.vec.extend_bytes` (amortized
  doubling growth), then freeze exactly once with `std.vec.as_bytes` (no
  copy). Handles are move-only: rebind the returned handle on every call and
  do not read the builder mid-accumulation.
- **Bytes doubling arena for random access.** When growing state must also be
  read at arbitrary offsets, keep an owned `bytes` arena: allocate with
  `std.bytes.alloc`, write in place with `bytes.set_u8` /
  `std.u32.write_le_at`, read with `std.codec.read_u32_le`, and when full
  allocate a doubled arena and copy the old contents across.
- **std.hash_map capacity planning.** Maps are fixed capacity:
  `std.hash_map.new(cap_pow2)` allocates a power-of-two slot table that never
  grows, and inserting a new key into a full table traps with `map_u32 full`.
  Size maps with `std.hash_map.with_capacity_u32(expected)` (at least 2x
  expected entries) so load stays at or below ~50% and linear probing stays
  short.
- **Profiling.** Set `X07_PROFILE=1` at compile time for minimal per-function
  profiling; see [Profiling](../toolchain/profiling.md).

## Canonical tuning checklist

Start from the scale class and tune in the smallest number of places. For the scale-class
definitions and the retry/idempotency rules that go with each one, see
[Guide: Scaling, retry, and idempotency for services](scaling-retry-idempotency.md) — this
section lists only the performance knobs.

### `replicated-http`

- concurrency limit per replica
- request body size caps + streaming
- batching where the downstream contract supports it
- connection pool sizing (DB/HTTP clients)
- explicit timeouts and cancellation

### `partitioned-consumer`

- partition key choice
- max in-flight per partition
- retry/backoff and dead-letter policy
- lag-based scaling signal (if supported by the runtime)

### `singleton-orchestrator`

- leader election timeouts
- reconciliation cadence
- backpressure for fan-out work

### `burst-batch`

- chunk size
- checkpoint frequency
- retry policy per step

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

- [Guide: Scaling, retry, and idempotency for services](scaling-retry-idempotency.md)
- [Guide: Kernel/shell in production](kernel-shell-production.md)
- [Benchmarks](../toolchain/benchmarks.md)

## Expert notes

When you need deeper profiling, prefer repeatable harnesses:

- produce machine-readable JSON outputs
- keep configuration explicit and version-controlled
- run in a “dry” mode in CI (no cluster required)

Minimal function profiling is available via `X07_PROFILE=1`:

- `docs/toolchain/profiling.md`

For end-to-end investigation, build a repeatable harness around `x07 run` JSON reports (`fuel_used`, `heap_used`) and keep its configuration version-controlled.
