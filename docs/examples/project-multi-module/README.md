# Example project: multi-module build

This directory is a standalone X07 project that demonstrates:

- multi-module compilation via `(import ...)`
- a local path dependency package (with a lockfile)
- using `x07 build` to compile to C

## Files

- `docs/examples/project-multi-module/x07.json`: project manifest
- `docs/examples/project-multi-module/x07.lock.json`: pinned deps
- `docs/examples/project-multi-module/src/main.x07.json`: entry module (solver body)
- `docs/examples/project-multi-module/src/app/rle.x07.json`: project module
- `docs/examples/project-multi-module/pkgs/appkit/0.1.0/`: dependency package

## Commands

From repo root:

- Regenerate lockfile: `x07 pkg lock --project docs/examples/project-multi-module/x07.json`
- Compile to C: `x07 build --project docs/examples/project-multi-module/x07.json --out target/project-multi-module/example.c`
- Compile+run: `printf '' | x07 run --repair=off --project docs/examples/project-multi-module/x07.json --stdin`
