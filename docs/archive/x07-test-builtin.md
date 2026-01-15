# Phase: Built-in test harness (`x07 test`)

This phase adds a deterministic, manifest-driven test harness to X07’s toolchain to support agentic development loops (stable discovery, stable execution, strict JSON report).

## Status

Implemented (v1).

## Canonical artifacts

- Spec: `docs/spec/x07-testing-v1.md`
- Report schema: `spec/x07test.schema.json`
- CLI implementation: `crates/x07` (`x07 test`)
- Stdlib module: `stdlib/std/0.1.1/modules/std/test.x07.json` (embedded via `crates/x07c/src/builtin_modules.rs`)
- Smoke suite: `tests/tests.json`, `tests/smoke_pure.x07.json`, `tests/smoke_fs.x07.json`

## Notes (integration constraints)

- Function names must start with the containing module id; the stable numeric codes are exposed as `std.test.code_assert_*` (not `std.test.code.*`).
- Default `--module-root` is the manifest directory (so `tests/tests.json` resolves module `smoke_pure` from `tests/smoke_pure.x07.json`).
- `--jobs >1` requires `--no-fail-fast` (parallel runs are opt-in and stable-ordered).

## Quick usage

- Run the repo smoke suite: `cargo run -p x07 -- test --manifest tests/tests.json`
- Determinism gate: `cargo run -p x07 -- test --manifest tests/tests.json --repeat 3`
- List tests: `cargo run -p x07 -- test --manifest tests/tests.json --list`
