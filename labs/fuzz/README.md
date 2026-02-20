# X07 fuzzing targets

This folder contains `cargo-fuzz`/libFuzzer targets for parser + compiler components.

## Prereqs

- Install the cargo subcommand: `cargo install cargo-fuzz`
- Use a Rust nightly toolchain (required by `cargo-fuzz`).

## Run

From the toolchain repo root (`x07/`):

- `cargo +nightly fuzz run --fuzz-dir labs/fuzz parse_x07ast_json`
- `cargo +nightly fuzz run --fuzz-dir labs/fuzz parse_sexpr` (JSON expression form used inside x07AST; not legacy `*.sexpr` source)
- `cargo +nightly fuzz run --fuzz-dir labs/fuzz compile_program_to_c`

## CI

CI runs a bounded smoke fuzz gate (30 seconds per target) via `./scripts/ci/check_fuzz_smoke.sh`.

On failure, CI uploads `labs/fuzz/artifacts/` as a workflow artifact. To reproduce locally:

- `cargo +nightly fuzz run --fuzz-dir labs/fuzz <target> labs/fuzz/artifacts/<target>/crash-*`

## Notes

- These targets are meant as an offline gate (not deterministic, not part of `cargo test`).
- Inputs are size-capped to keep runs stable.
