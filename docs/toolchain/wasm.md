# WASM toolchain

`x07 wasm` covers:

- build+run loops for **solve-pure** X07 programs as WASM modules
- **WASI 0.2 components** (HTTP + CLI runnable targets)
- hardening surfaces (toolchain pin validation, runtime budgets, and core-wasm HTTP reducers)

This surface is implemented by the `x07-wasm` tool (repo: `x07-wasm-backend`).

> **Status (2026-06):** core WASM modules and WASI components are the active surfaces. The web-ui, app-bundle, and device lanes were removed in the [2026-06 refocus](../roadmap.md) along with the archived `x07-web-ui` / `x07-device-host` repos.

## Delegation model

The core toolchain delegates WASM commands:

- `x07 wasm ...` delegates to `x07-wasm ...` on PATH.
- If `x07-wasm` is not installed/discoverable, delegated commands exit with code `2`.

## Install

Primary path:

```sh
x07up component add wasm
x07 wasm doctor --json
```

Fallbacks:

```sh
cargo install --locked x07-wasm --version <VERSION>
```

Use `cargo install --locked --git https://github.com/x07lang/x07-wasm-backend.git x07-wasm` only when you need unreleased development state from the repo.

The canonical runnable projects and CI scripts live in `x07-wasm-backend/examples/`.

Component builds also require additional tools on `PATH` (checked by `x07 wasm doctor`):

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

`x07 wasm build` delegates to `x07-wasm build` and selects a backend via the wasm profile’s `codegen_backend` field (or `--codegen-backend` override):

- `native_x07_wasm_v1` (default): calls `x07 build --emit-wasm ...` and skips `clang` / `wasm-ld`
- `c_toolchain_v1` (legacy): `x07 build --freestanding --emit-c-header ...` → `clang` → `wasm-ld`

In both cases it emits a wasm artifact manifest and a machine report.

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

## Components (WASI 0.2)

This section introduces a component pipeline:

- WIT registry: `arch/wit/index.x07wit.json` (vendored, pinned)
- Component profile registry: `arch/wasm/component/index.x07wasm.component.json`

Validate (offline, no external validators):

```sh
x07 wasm wit validate --json
x07 wasm component profile validate --json
```

Native targets (adapterless, no compose):

```sh
x07 wasm component build --project examples/http_echo/x07.json --emit http-native --json
x07 wasm component targets --component target/x07-wasm/component/http.component.wasm --wit wit/deps/wasi/http/0.2.8/proxy.wit --world proxy --json
x07 wasm serve --mode canary --component target/x07-wasm/component/http.component.wasm --request-body @examples/http_echo/tests/fixtures/request_body.bin --json
```

```sh
x07 wasm component build --project examples/solve_pure_echo/x07.json --emit cli-native --json
x07 wasm component targets --component target/x07-wasm/component/cli.component.wasm --wit wit/deps/wasi/cli/0.2.8/command.wit --world command --json
x07 wasm component run --component target/x07-wasm/component/cli.component.wasm --stdin examples/solve_pure_echo/tests/fixtures/in_hello.bin --stdout-out dist/stdout.bin --json
```

Composed path (adapters + `wac plug`):

```sh
x07 wasm component build --project examples/http_echo/x07.json --emit solve --json
x07 wasm component build --project examples/solve_pure_echo/x07.json --emit http-adapter --json
x07 wasm component compose --adapter http --solve target/x07-wasm/component/solve.component.wasm --out dist/app.http.component.wasm --json
x07 wasm component targets --component dist/app.http.component.wasm --wit wit/deps/wasi/http/0.2.8/proxy.wit --world proxy --json
```

Notes:

- `component build --emit all` builds `solve + http + cli` (composed). Build adapters explicitly via `http-adapter|cli-adapter`.
- Native HTTP components surface failures via response headers (`x-x07-diag-code`, optional `x-x07-diag-data-b64`). Native CLI components surface failures via stderr sentinel lines (`x07-diag-code: ...`).

Run (legacy composed artifacts):

```sh
x07 wasm serve --mode canary --component dist/app.http.component.wasm --request-body @examples/http_echo/tests/fixtures/request_body.bin --json
x07 wasm component run --component dist/app.cli.component.wasm --stdin examples/solve_pure_echo/tests/fixtures/in_hello.bin --stdout-out dist/stdout.bin --json
```

## Archived lanes (web-ui / device / app bundle)

The web-ui (`x07 wasm web-ui ...`), device bundle (`x07 wasm device ...`), and full-stack app-bundle (`x07 wasm app ...`) lanes — together with the `std-web-ui` package and the iOS/Android project generators — were removed in the [2026-06 refocus](../roadmap.md) along with the archived `x07-web-ui` and `x07-device-host` repos.

## Hardening

Toolchain pins as data (CI gate):

```sh
x07 wasm toolchain validate --profile arch/wasm/toolchain/profiles/toolchain_ci.json --json
```

Runtime limits can be overridden per command (all optional; defaults come from the selected wasm profile):

```sh
x07 wasm run --max-fuel 10000 --max-memory-bytes 67108864 --max-table-elements 10000 --max-wasm-stack-bytes 1048576 --json
```

Optional profile-level host runtime knobs:

- `runtime.instance_allocator`: `on_demand` (default) or `pooling`
- `runtime.cache_config`: path to a Wasmtime cache config file (loaded by the host)

Shipped WASM profiles include:

- `wasm_release_cached` (enables Wasmtime compilation cache via `arch/wasm/toolchain/wasmtime_cache.toml`)
- `wasm_release_pooling` (pooling allocator)

Core-wasm HTTP reducer contracts + loop:

```sh
x07 wasm http contracts validate --strict --json
```
