# Testing

X07 includes a built-in test harness designed for agents.

## Test command

- `x07 test`

Each test declares an execution world (typically `run-os`).

Output is an `x07test` JSON report including:

- pass/fail summary
- failures with diagnostic codes
- optional `mem_stats` / `sched_stats`

See also:

- [Test manifest](tests-manifest.md)
- [Testing by example](testing-by-example.md)
- [Diagnostic codes](diagnostic-codes.md)
- [Benchmarks](benchmarks.md)

## Why X07 includes a built-in test harness

Agents need:

- a single canonical way to run tests
- a structured report
- stable failure semantics

If every project reinvents testing, agents become unreliable.
