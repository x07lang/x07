# Determinism (Phase A)

Deterministic evaluation depends on two invariants:

1. Execution is bounded by **fuel** (deterministic interruption).
2. The execution environment is fixed (no inherited args/env; no time or network access; world-scoped capabilities only).

Wall-clock timeouts are intentionally avoided in the runner because they introduce nondeterministic failure modes near the boundary.

## Runner

The deterministic runner is `crates/x07-host-runner`.

Key behaviors:

- Fuel is enforced inside generated code (`rt_fuel`).
- Memory is bounded by a fixed-capacity arena (`X07_MEM_CAP`).
- No inherited environment variables or CLI args.
- Filesystem access is deny-by-default; `solve-fs` provides a read-only fixture directory as `.` and exposes file reads via `["fs.read", ...]`.

## Phase G2 scheduler determinism

Phase G2 adds a deterministic, single-thread, cooperative scheduler:

- Concurrency is **virtual** (no OS threads); blocking points are explicit (`task.sleep`, I/O, channel waits).
- I/O uses fixture-backed latency indices and advances **virtual time** (`sched_stats.virtual_time_end`), not wall clock time.
- Solver artifacts emit `sched_stats.sched_trace_hash`, and Phase G2 suites can require replay runs with identical output + `fuel_used` + `sched_trace_hash`.
