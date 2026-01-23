# Testing

X07 includes a deterministic test harness designed for agents.

For a step-by-step walkthrough with fixtures, see: [Testing by example](testing-by-example.md).

## What “deterministic tests” mean

- deterministic inputs
- deterministic fixture worlds
- deterministic budgets
- stable structured reports

## Test command

- `x07 test`

Output is an `x07test` JSON report including:

- pass/fail summary
- failures with diagnostic codes
- optional `mem_stats` / `sched_stats`

See also:

- [Test manifest](tests-manifest.md)
- [Testing by example](testing-by-example.md)
- [Diagnostic codes](diagnostic-codes.md)

## Why X07 includes a built-in test harness

Agents need:

- a single canonical way to run tests
- a structured report
- stable failure semantics

If every project reinvents testing, agents become unreliable.
