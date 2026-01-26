# Concurrency & multiprocessing

X07 intentionally separates:

- **deterministic concurrency** (for tests and fixture worlds)
- **OS threads** (for OS-world blocking/I/O concurrency; policy-gated)
- **OS multiprocessing** (for production parallelism)

## Deterministic concurrency (async, single-core)

In fixture worlds, X07’s async system:

- is cooperative (tasks yield at known points)
- runs on a deterministic scheduler
- uses virtual time (ticks) instead of wall-clock time

This enables:
- deterministic pipelines,
- repeatable performance measurements,
- reproducible repair loops.

## OS threads (policy-gated)

In OS worlds, X07 can use threads for blocking and I/O-heavy work. In `run-os-sandboxed`, this is gated by policy.

The `threads` policy section controls thread-backed blocking operations. Setting:

- `threads.max_blocking = 0`

disables blocking operations and produces a stable trap:

- `os.threads.blocking disabled by policy`

## OS multiprocessing (multi-core)

In OS worlds, X07 can spawn subprocesses:

- the OS schedules them across cores
- policies limit what can be spawned, how many, and resource bounds

In `run-os-sandboxed`, process spawning must be explicitly enabled by policy. If your program needs to spawn helper processes for parallel work, start from:

- `x07 policy init --template worker-parallel`

If you don’t need process spawning, prefer the stricter `worker` template.

This provides a safe default path to “real parallel work” without bringing nondeterminism into the deterministic test substrate.

Guideline:
- Keep core logic testable in fixture worlds.
- Use subprocess adapters at the edge for CPU-parallel work.
