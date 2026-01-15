# Phase E example project

This directory is a standalone X07 project that demonstrates:

- multi-module compilation via `(import ...)`
- a local path dependency package (with a lockfile)
- using `x07c build` to compile deterministically

## Files

- `examples/phaseE/x07.json`: project manifest
- `examples/phaseE/x07.lock.json`: pinned deps
- `examples/phaseE/src/main.x07.json`: entry module (solver body)
- `examples/phaseE/src/app/rle.x07.json`: project module
- `examples/phaseE/pkgs/appkit/0.1.0/`: dependency package

## Commands

From repo root:

- Regenerate lockfile: `cargo run -p x07c -- lock --project examples/phaseE/x07.json`
- Compile to C: `cargo run -p x07c -- build --project examples/phaseE/x07.json --out target/phaseE/example.c`
- Compile+run (native): `cargo run -p x07-host-runner -- --project examples/phaseE/x07.json --world solve-pure --input /dev/null`
