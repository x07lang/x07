# Concurrency & multiprocessing

X07 intentionally separates:

- **deterministic concurrency** (for tests and fixture worlds)
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

## OS multiprocessing (multi-core)

In OS worlds, X07 can spawn subprocesses:

- the OS schedules them across cores
- policies limit what can be spawned, how many, and resource bounds

This provides a safe default path to “real parallel work” without bringing nondeterminism into the deterministic test substrate.

Guideline:
- Keep core logic testable in fixture worlds.
- Use subprocess adapters at the edge for CPU-parallel work.
