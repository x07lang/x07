# WASM (Phase 0)

Phase 0 adds a build+run loop for **solve-pure** X07 programs as WASM modules, without introducing a new compiler backend.

Phase 0 is implemented by the `x07-wasm` tool (repo: `x07-wasm-backend`).

## Delegation model

The core toolchain delegates WASM commands:

- `x07 wasm ...` delegates to `x07-wasm ...` on PATH.
- If `x07-wasm` is not installed/discoverable, delegated commands exit with code `2`.

## Install

Install `x07-wasm` from the `x07-wasm-backend` repo:

```sh
cargo install --locked --git https://github.com/x07lang/x07-wasm-backend.git x07-wasm
```

## Profiles (contracts-as-data)

`x07-wasm` consumes a pinned profile registry by default:

- `arch/wasm/index.x07wasm.json`
- `arch/wasm/profiles/*.json`

Validate these files in CI:

```sh
x07 wasm profile validate --json
```

If you need to bypass the registry (e.g. experimentation), use `--profile-file`.

## Build

`x07 wasm build`:

- calls `x07 build --freestanding --emit-c-header …`
- compiles the emitted C to `wasm32` via `clang`
- links a reactor-style module via `wasm-ld --no-entry`
- emits a wasm artifact manifest and a machine report

Example:

```sh
x07 wasm build \
  --project ./x07.json \
  --profile wasm_release \
  --out dist/app.wasm \
  --artifact-out dist/app.wasm.manifest.json \
  --json
```

## Run

`x07 wasm run` instantiates the module under Wasmtime and calls `x07_solve_v2` using the WASM Basic C ABI **sret** convention.

On failures, it writes a deterministic incident bundle under `.x07-wasm/incidents/…` containing:

- `input.bin`
- `run.report.json`
- `wasm.manifest.json` (if discoverable next to the wasm path)

## Machine discovery

Agents should use:

- `x07 wasm --cli-specrows`
- `x07 wasm cli specrows check`
