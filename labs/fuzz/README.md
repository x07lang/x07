# X07 fuzzing targets

This folder contains `cargo-fuzz`/libFuzzer targets for parser + compiler components.

## Prereqs

- Install the cargo subcommand: `cargo install cargo-fuzz`
- Use a Rust nightly toolchain (required by `cargo-fuzz`).

## Run

From `fuzz/`:

- `cargo +nightly fuzz run parse_x07ast_json`
- `cargo +nightly fuzz run parse_sexpr` (JSON expression form used inside x07AST; not legacy `*.sexpr` source)
- `cargo +nightly fuzz run compile_program_to_c`

## Notes

- These targets are meant as an offline gate (not deterministic, not part of `cargo test`).
- Inputs are size-capped to keep runs stable.
