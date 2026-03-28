---
name: x07-test
description: Run project tests using the X07 test harness (x07 test), producing a machine-readable JSON report.
metadata:
  short-description: Run x07 tests
  version: 0.1.0
  kind: docs
---

# x07-test

This skill provides the single canonical way to run tests for an X07 project using the built-in test harness.

## Canonical command

- `x07 test --manifest tests/tests.json`

## Notes

- By default, `x07 test` prints JSON to stdout; use `--report-out <path>` to write a report file.
- Filter selection:
  - `x07 test --manifest tests/tests.json --filter <substr>`
  - `x07 test --manifest tests/tests.json --filter <id> --exact`
  - By default, filters that select 0 tests are an error; pass `--allow-empty` to treat this as success.
  - Use `--list` to print selected tests without running them.
- Progress output:
  - `x07 test --verbose` prints `test: <id>` lines to stderr while preserving the machine report on stdout.
- Fuel overrides:
  - Set `solve_fuel` on a test entry in `tests/tests.json` to override the per-test fuel cap.
- Runner traps:
  - When a runner traps, the report includes diagnostic `X07T_RUN_TRAP` with `details.trap` set to the decoded trap string.
- OS-world working directory:
  - For `run-os` / `run-os-sandboxed`, the runner process `cwd` is derived from the manifest directory when possible (sandboxed tests may choose a different `cwd` to satisfy policy read roots), so relative filesystem paths resolve from that `cwd`.
- Property-based tests (PBT):
  - `x07 test --pbt --manifest tests/tests.json` runs PBT entries only.
  - `x07 test --all --manifest tests/tests.json` runs unit + PBT.
  - `x07 test --pbt --pbt-repro <repro.json> --manifest tests/tests.json` replays exactly one counterexample.
  - If PBT finds a counterexample, use `x07 fix --from-pbt <repro.json> --write` to convert it into a deterministic regression test (wrapper module + manifest entry).
- `x07 test` resolves `stdlib.lock` by searching upward from the manifest directory, then upward from the `x07` executable location. Override with `--stdlib-lock <path>`.
- New projects created with `x07 init` include `tests/tests.json` plus a minimal `tests/smoke.x07.json`.
- Publishable package repos created with `x07 init --package` include `tests/tests.json` and publishable modules under `modules/` (no `tests/smoke.x07.json`).
- Tests under `tests/` can import your project modules via project module-root discovery (typically `src/` in projects, and `modules/` in package repos).
- World-gating is enforced at compile time. Keep unit tests small and pure (no filesystem/network I/O) unless you are explicitly writing OS-world integration tests and running them under the sandbox profile.

See also: https://x07lang.org/docs/toolchain/testing-by-example/
