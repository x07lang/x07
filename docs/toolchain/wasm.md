# WASM (Phases 0–1)

Phase 0 adds a build+run loop for **solve-pure** X07 programs as WASM modules, without introducing a new compiler backend.
Phase 1 adds **WASI 0.2 components** (HTTP + CLI runnable targets) on top of Phase 0.

Phases 0–1 are implemented by the `x07-wasm` tool (repo: `x07-wasm-backend`).

## Delegation model

The core toolchain delegates WASM commands:

- `x07 wasm ...` delegates to `x07-wasm ...` on PATH.
- If `x07-wasm` is not installed/discoverable, delegated commands exit with code `2`.

## Install

Install `x07-wasm` from the `x07-wasm-backend` repo:

```sh
cargo install --locked --git https://github.com/x07lang/x07-wasm-backend.git x07-wasm
```

Phase 1 also requires additional tools on `PATH` (checked by `x07 wasm doctor`):

- `wasm-tools`
- `wit-bindgen`
- `wac`
- `wasmtime`

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

## Phase 1: components (WASI 0.2)

Phase 1 introduces a component pipeline:

- WIT registry: `arch/wit/index.x07wit.json` (vendored, pinned)
- Component profile registry: `arch/wasm/component/index.x07wasm.component.json`

Validate (offline, no external validators):

```sh
x07 wasm wit validate --json
x07 wasm component profile validate --json
```

Build + compose:

```sh
x07 wasm component build --project examples/http_echo/x07.json --emit all --json
x07 wasm component compose --adapter http --solve target/x07-wasm/component/solve.component.wasm --out dist/app.http.component.wasm --json
x07 wasm component targets --component dist/app.http.component.wasm --wit wit/deps/wasi/http/0.2.8/proxy.wit --world proxy --json
```

Run:

```sh
x07 wasm serve --mode canary --component dist/app.http.component.wasm --request-body @examples/http_echo/tests/fixtures/request_body.bin --json
x07 wasm component run --component dist/app.cli.component.wasm --stdin examples/solve_pure_echo/tests/fixtures/in_hello.bin --stdout-out dist/stdout.bin --json
```
