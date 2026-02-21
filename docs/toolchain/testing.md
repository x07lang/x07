# Testing

X07 includes a built-in test harness designed for agents.

## Test command

- `x07 test` (unit tests; manifest entries where `pbt` is not set)
- `x07 test --pbt` (property-based tests only)
- `x07 test --all` (unit + property-based)

Each test declares an execution world (typically `run-os`).

Output is an `x07test` JSON report including:

- pass/fail summary
- failures with diagnostic codes
- failure classification (`failure_kind`, for example `contract_violation`)
- optional `mem_stats` / `sched_stats`

See also:

- [Test manifest](tests-manifest.md)
- [Testing by example](testing-by-example.md)
- [Property-based testing](pbt.md)
- [PBT repro → regression test](pbt-fix-from-repro.md)
- [Diagnostic codes](diagnostic-codes.md)
- [Benchmarks](benchmarks.md)

## Why X07 includes a built-in test harness

Agents need:

- a single canonical way to run tests
- a structured report
- stable failure semantics

If every project reinvents testing, agents become unreliable.

## Toolchain unit tests (Rust)

When changing compiler/toolchain internals, run:

- `cargo test -p x07c`

For minimal repro tests, add a grep-friendly tag:

- `REGRESSION: <id>` (for example `REGRESSION: x07.rfc.backlog.unit-tests@0.1.0`)

To find regression tests:

- `rg "REGRESSION:" crates/x07c`

## Compiler fuzzing (CI smoke gate)

The toolchain repo includes `cargo-fuzz` targets under `labs/fuzz/`. CI runs a short fuzz smoke gate (30 seconds per target) and uploads `labs/fuzz/artifacts/` on failure to enable local reproduction.

To run locally:

- `cargo install cargo-fuzz`
- `cargo +nightly fuzz run --fuzz-dir labs/fuzz parse_x07ast_json -- -max_total_time=30`
- `cargo +nightly fuzz run --fuzz-dir labs/fuzz parse_sexpr -- -max_total_time=30`
- `cargo +nightly fuzz run --fuzz-dir labs/fuzz compile_program_to_c -- -max_total_time=30`

To reproduce a CI crash input:

- `cargo +nightly fuzz run --fuzz-dir labs/fuzz <target> labs/fuzz/artifacts/<target>/crash-*`
