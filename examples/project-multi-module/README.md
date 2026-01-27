# Example project: multi-module build

This directory is a standalone X07 project that demonstrates:

- multi-module compilation via `(import ...)`
- a local path dependency package (with a lockfile)
- using `x07c build` to compile deterministically

## Files

- `examples/project-multi-module/x07.json`: project manifest
- `examples/project-multi-module/x07.lock.json`: pinned deps
- `examples/project-multi-module/src/main.x07.json`: entry module (solver body)
- `examples/project-multi-module/src/app/rle.x07.json`: project module
- `examples/project-multi-module/pkgs/appkit/0.1.0/`: dependency package

## Commands

From repo root:

- Regenerate lockfile: `x07 pkg lock --project examples/project-multi-module/x07.json`
- Compile to C: `x07 build --project examples/project-multi-module/x07.json --out target/project-multi-module/example.c`
- Compile+run (deterministic): `x07 run --repair=off --project examples/project-multi-module/x07.json --world solve-pure --input /dev/null`
