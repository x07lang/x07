# WASM (Phases 0–5)

Phase 0 adds a build+run loop for **solve-pure** X07 programs as WASM modules, without introducing a new compiler backend.
Phase 1 adds **WASI 0.2 components** (HTTP + CLI runnable targets) on top of Phase 0.
Phase 2 adds a **Web UI** loop (`web-ui build|serve|test`) on top of Phase 0/1.
Phase 3 adds a **full-stack app bundle** loop (`app build|serve|test`) that combines Phase 2 (frontend) and Phase 1 (backend).
Phase 4 adds **native backend targets** so `x07 wasm component build --emit http|cli` can produce runnable standard-world components without guest adapters and without a compose step.
Phase 5 adds **Track-1 hardening**: toolchain pin validation, host runtime budgets, deployable app packs, and a core-wasm HTTP reducer loop.

Phases 0–4 are implemented by the `x07-wasm` tool (repo: `x07-wasm-backend`).

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

Phase 2 (component+ESM builds) also uses:

- `node`
- `jco` (component transpile)

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

Phase 4 native targets (adapterless, no compose):

```sh
x07 wasm component build --project examples/http_echo/x07.json --emit http --json
x07 wasm component targets --component target/x07-wasm/component/http.component.wasm --wit wit/deps/wasi/http/0.2.8/proxy.wit --world proxy --json
x07 wasm serve --mode canary --component target/x07-wasm/component/http.component.wasm --request-body @examples/http_echo/tests/fixtures/request_body.bin --json
```

```sh
x07 wasm component build --project examples/solve_pure_echo/x07.json --emit cli --json
x07 wasm component targets --component target/x07-wasm/component/cli.component.wasm --wit wit/deps/wasi/cli/0.2.8/command.wit --world command --json
x07 wasm component run --component target/x07-wasm/component/cli.component.wasm --stdin examples/solve_pure_echo/tests/fixtures/in_hello.bin --stdout-out dist/stdout.bin --json
```

Legacy compose path (Phase 1 adapters + `wac plug`):

```sh
x07 wasm component build --project examples/http_echo/x07.json --emit solve --json
x07 wasm component build --project examples/solve_pure_echo/x07.json --emit http-adapter --json
x07 wasm component compose --adapter http --solve target/x07-wasm/component/solve.component.wasm --out dist/app.http.component.wasm --json
x07 wasm component targets --component dist/app.http.component.wasm --wit wit/deps/wasi/http/0.2.8/proxy.wit --world proxy --json
```

Notes:

- `component build --emit all` builds `solve + http + cli` (native). Build adapters explicitly via `http-adapter|cli-adapter`.
- Native HTTP components surface failures via response headers (`x-x07-diag-code`, optional `x-x07-diag-data-b64`). Native CLI components surface failures via stderr sentinel lines (`x07-diag-code: ...`).

Run (legacy composed artifacts):

```sh
x07 wasm serve --mode canary --component dist/app.http.component.wasm --request-body @examples/http_echo/tests/fixtures/request_body.bin --json
x07 wasm component run --component dist/app.cli.component.wasm --stdin examples/solve_pure_echo/tests/fixtures/in_hello.bin --stdout-out dist/stdout.bin --json
```

## Phase 2: web-ui (browser host)

Phase 2 adds a browser host loop for X07 reducers that consume `x07.web_ui.dispatch@0.1.0` and emit `x07.web_ui.frame@0.2.0` as UTF-8 JSON bytes.

The canonical `std-web-ui` package, browser host assets, and WIT contracts live in the `x07-web-ui` repo.

Validate contracts + profile registry (offline):

```sh
x07 wasm web-ui contracts validate --json
x07 wasm web-ui profile validate --json
```

Build + serve + test (example from `x07-web-ui`):

```sh
git clone https://github.com/x07lang/x07-web-ui.git
cd x07-web-ui

x07 wasm web-ui build --project examples/web_ui_counter/x07.json --profile web_ui_debug --out-dir dist --json
x07 wasm web-ui serve --dir dist --mode listen --strict-mime --json
x07 wasm web-ui test --dist-dir dist --case examples/web_ui_counter/tests/counter.trace.json --json
```

Component build (transpiled for the browser via `jco transpile`):

```sh
x07 wasm web-ui build --project examples/web_ui_counter/x07.json --profile web_ui_debug --out-dir dist --format component --json
```

Note: `web-ui build` emits `dist/wasm.profile.json` (the resolved wasm profile used for the build). `web-ui test` and replay tooling use it to apply Phase-5 runtime limits deterministically.

## Phase 3: app bundle (full stack)

Phase 3 introduces an app-bundle registry (`arch/app/*`) and a single closed loop:

- app profile → app build → app serve → app test → incident → regression

Validate (offline):

```sh
x07 wasm app contracts validate --json
x07 wasm app profile validate --json
```

Build + serve + test (example from `x07-wasm-backend`):

```sh
git clone https://github.com/x07lang/x07-wasm-backend.git
cd x07-wasm-backend

x07 wasm app build --profile app_dev --out-dir dist/app --clean --json
x07 wasm app serve --dir dist/app --mode smoke --strict-mime --json
x07 wasm app test --dir dist/app --trace examples/app_fullstack_hello/tests/trace_0001.json --json
```

## Phase 5: hardening

Toolchain pins as data (CI gate):

```sh
x07 wasm toolchain validate --profile arch/wasm/toolchain/profiles/toolchain_ci.json --json
```

Runtime limits can be overridden per command (all optional; defaults come from the selected wasm profile):

```sh
x07 wasm run --max-fuel 10000 --max-memory-bytes 67108864 --max-table-elements 10000 --max-wasm-stack-bytes 1048576 --json
```

App deploy artifacts:

```sh
x07 wasm app pack --bundle-manifest dist/app/app.bundle.json --out-dir dist/app.pack --profile-id app_dev --json
x07 wasm app verify --pack-manifest dist/app.pack/app.pack.json --json
```

Core-wasm HTTP reducer contracts + loop:

```sh
x07 wasm http contracts validate --strict --json
```
