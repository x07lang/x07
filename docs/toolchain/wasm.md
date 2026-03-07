# WASM (Phases 0–10)

Phase 0 adds a build+run loop for **solve-pure** X07 programs as WASM modules, without introducing a new compiler backend.
Phase 1 adds **WASI 0.2 components** (HTTP + CLI runnable targets) on top of Phase 0.
Phase 2 adds a **Web UI** loop (`web-ui build|serve|test`) on top of Phase 0/1.
Phase 3 adds a **full-stack app bundle** loop (`app build|serve|test`) that combines Phase 2 (frontend) and Phase 1 (backend).
Phase 4 adds **native backend targets** so `x07 wasm component build --emit http|cli` can produce runnable standard-world components without guest adapters and without a compose step.
Phase 5 adds **Track-1 hardening**: toolchain pin validation, host runtime budgets, deployable app packs, and a core-wasm HTTP reducer loop.
Phase 6 adds **operational contracts** (ops profiles, capabilities, policy), **SLO-as-code**, **deploy plan generation**, and **signed provenance** (DSSE + Ed25519).
Phase 7 adds a **native x07→wasm backend** so `solve-pure` wasm builds no longer require `clang` / `wasm-ld` by default.
Phase 8 adds **device bundles** for running `std.web_ui` reducers in a system WebView host (desktop + mobile), pinned to a host ABI hash.
Phase 9 adds a **system WebView host runner** and wires `device run` + `device package` for desktop.
Phase 10 adds **iOS/Android project generation** via `device package --target ios|android`.

Phases 0–10 are implemented by the `x07-wasm` tool (repo: `x07-wasm-backend`).

## Delegation model

The core toolchain delegates WASM commands:

- `x07 wasm ...` delegates to `x07-wasm ...` on PATH.
- If `x07-wasm` is not installed/discoverable, delegated commands exit with code `2`.

## Install

Primary path:

```sh
x07up component add wasm
x07up component add device-host
x07 wasm doctor --json
```

Fallbacks:

```sh
cargo install --locked x07-wasm --version <VERSION>
cargo install --locked x07-device-host-desktop --version <VERSION>
```

Use `cargo install --locked --git https://github.com/x07lang/x07-wasm-backend.git x07-wasm` only when you need unreleased development state from the repo.

Phase 1 also requires additional tools on `PATH` (checked by `x07 wasm doctor`):

- `wasm-tools`
- `wit-bindgen`
- `wac`
- `wasmtime`

Phase 2 (component+ESM builds) also uses:

- `node`
- `jco` (component transpile)

Note: Node is used for browser-targeted tooling (for example `jco transpile`), not as a secure WASI runtime. Node’s WASI APIs are not intended to be a security sandbox for untrusted code; use Wasmtime (the `x07-wasm` baseline) for untrusted execution.

Phase 9 (desktop host runner + packaging) also uses:

- `x07-device-host-desktop`

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
x07 wasm component build --project examples/http_echo/x07.json --emit http-native --json
x07 wasm component targets --component target/x07-wasm/component/http.component.wasm --wit wit/deps/wasi/http/0.2.8/proxy.wit --world proxy --json
x07 wasm serve --mode canary --component target/x07-wasm/component/http.component.wasm --request-body @examples/http_echo/tests/fixtures/request_body.bin --json
```

```sh
x07 wasm component build --project examples/solve_pure_echo/x07.json --emit cli-native --json
x07 wasm component targets --component target/x07-wasm/component/cli.component.wasm --wit wit/deps/wasi/cli/0.2.8/command.wit --world command --json
x07 wasm component run --component target/x07-wasm/component/cli.component.wasm --stdin examples/solve_pure_echo/tests/fixtures/in_hello.bin --stdout-out dist/stdout.bin --json
```

Composed path (Phase 1 adapters + `wac plug`):

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

## Phase 2: web-ui (browser host)

Phase 2 adds a browser host loop for X07 reducers that consume `x07.web_ui.dispatch@0.1.0` and emit `x07.web_ui.frame@0.2.0` as UTF-8 JSON bytes.

The canonical `std-web-ui` package, browser host assets, and WIT contracts live in the `x07-web-ui` repo. Install/update the package with the X07 package manager:

```sh
x07 pkg versions std-web-ui
x07 pkg add std-web-ui@0.1.9 --sync
```

If `std-web-ui` is locked under `.x07/deps/...` but missing on disk, `x07 test`, `x07 build`, `x07 run`, `x07 wasm web-ui build`, and `x07 wasm device build` auto-sync the package before they continue. Keep `x07 pkg lock --project x07.json --check` in CI when you want that state drift to fail hard instead.

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

Use `x07 wasm web-ui test` or `x07 test` for reducer-semantic coverage. Device bundle commands do not replace those feature-level checks.

Host entrypoint notes:

- `web-ui build` emits `dist/index.html` which loads `dist/bootstrap.js`.
- `dist/main.mjs` is a compatibility alias that imports `bootstrap.js`.
- Host HTML uses a strict CSP that allows WebAssembly via `'wasm-unsafe-eval'` (without enabling general `'unsafe-eval'`).

Component build (transpiled for the browser via `jco transpile`):

```sh
x07 wasm web-ui build --project examples/web_ui_counter/x07.json --profile web_ui_debug --out-dir dist --format component --json
```

Note: `web-ui build` emits `dist/wasm.profile.json` (the resolved wasm profile used for the build). `web-ui test` and replay tooling use it to apply Phase-5 runtime limits deterministically.

## Phase 8: device bundles (system WebView host)

Phase 8 introduces a device contract layer for running `std.web_ui` reducers in a system WebView host (desktop + mobile).

The device bundle format pins a host ABI hash (from the `x07-device-host` repo) so that device apps can reject incompatible hosts deterministically.

The host ABI is sealed as a snapshot contract in `x07-device-host` (`arch/host_abi/host_abi.snapshot.json`) and pinned into `x07-wasm-backend`. `device verify` compares a bundle’s `host.host_abi_hash` against the tool-pinned ABI hash and emits `X07WASM_DEVICE_BUNDLE_HOST_ABI_HASH_MISMATCH` (exit code 3) on mismatch. It does not require a repo-local vendored host ABI snapshot in the consumer project.

Validate contracts (offline):

```sh
x07 wasm device index validate --json
x07 wasm device profile validate --json
```

Build + verify a bundle:

```sh
x07 wasm device build --profile device_dev --out-dir dist/device --clean --json
x07 wasm device verify --dir dist/device --json
```

Bundle layout notes:

- The resolved device profile is embedded into the bundle under `profile/device.profile.json` and is digest-verified by `device verify`.
- `device verify` streams digests and enforces hard size caps to avoid unbounded reads (bundle manifest 8 MiB; bundle files 256 MiB).
- `web-ui build` and `device build` emit the canonical browser/WebView host assets from the tool-pinned `x07-web-ui` snapshot. Consumer repos do not need a local `vendor/x07-web-ui/` tree.
- Use reducer-level tests for semantics. Device commands cover bundle integrity, provenance, packaging, and host smoke execution.

Signed device provenance (DSSE + Ed25519):

```sh
x07 wasm device provenance attest --dir dist/device --signing-key <signing_key.b64> --out dist/device.provenance.dsse.json --json
x07 wasm device provenance verify --attestation dist/device.provenance.dsse.json --bundle-dir dist/device --trusted-public-key <public_key.b64> --json
```

## Phase 9: device run + package (desktop host)

Run a device bundle via the desktop host:

```sh
x07 wasm device run --bundle dist/device --target desktop --json
```

For a full desktop smoke loop, run:

```sh
x07 wasm device build --profile device_dev --out-dir dist/device --clean --json
x07 wasm device verify --dir dist/device --json
x07 wasm device package --bundle dist/device --target desktop --out-dir dist/device_package --json
x07 wasm device run --bundle dist/device --target desktop --headless-smoke --json
```

Testing boundary:

- Use `x07 wasm web-ui test` when you need deterministic reducer-level trace replay and feature assertions.
- Use `x07 wasm device build|verify|provenance|package|run` when you need bundle/package/provenance validation and desktop smoke coverage.

Package a device bundle into a desktop payload (writes `package.manifest.json`):

```sh
x07 wasm device package --bundle dist/device --target desktop --out-dir dist/device_package --json
```

## Phase 10: device package (iOS/Android project generation)

Generate an iOS project directory (no Xcode required for generation):

```sh
x07 wasm device package --bundle dist/device --target ios --out-dir dist/device_package_ios --json
```

Generate an Android project directory (no Gradle required for generation):

```sh
x07 wasm device package --bundle dist/device --target android --out-dir dist/device_package_android --json
```

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

For backends that need server-held JSON state across one `app serve` or `app test` session, set `backend.adapter` to `wasi_http_proxy_state_doc_v1`. The adapter passes a typed `{request, state}` document into the backend and persists the returned `state` for the next request in that replay session.

## Phase 5: hardening

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
- `wasm_web_ui_release_cached` (web-ui + compilation cache)
- `wasm_web_ui_release_pooling` (web-ui + pooling allocator)

App deploy artifacts:

```sh
x07 wasm app pack --bundle-manifest dist/app/app.bundle.json --out-dir dist/app.pack --profile-id app_dev --json
x07 wasm app verify --pack-manifest dist/app.pack/app.pack.json --json
```

Core-wasm HTTP reducer contracts + loop:

```sh
x07 wasm http contracts validate --strict --json
```

## Phase 6: ops + capabilities + policy + SLO + deploy plans + provenance

Phase 6 introduces machine-readable operational governance and deployment artifacts:

```sh
x07 wasm ops validate --profile arch/app/ops/ops_release.json --json
x07 wasm caps validate --profile arch/app/ops/caps_release.json --json
```

Policy cards (assertions + optional RFC-6902 JSON Patch):

```sh
x07 wasm policy validate --card arch/app/ops/policy_deploy_patch_id.json --json
```

SLO-as-code + offline evaluation:

```sh
x07 wasm slo validate --profile arch/slo/slo_min.json --json
x07 wasm slo eval --profile arch/slo/slo_min.json --metrics examples/app_min/tests/metrics_canary_ok.json --json
```

Deploy plan generation (writes `deploy.plan.json`; by default also emits Kubernetes YAMLs under `--out-dir`):

```sh
x07 wasm deploy plan --pack-manifest dist/app.pack/app.pack.json --ops arch/app/ops/ops_release.json --out-dir dist/deploy_plan --json
```

Plan-only mode (no Kubernetes YAML outputs):

```sh
x07 wasm deploy plan --pack-manifest dist/app.pack/app.pack.json --ops arch/app/ops/ops_release.json --emit-k8s false --out-dir dist/deploy_plan --json
```

Signed pack provenance (DSSE + Ed25519):

```sh
x07 wasm provenance attest --pack-manifest dist/app.pack/app.pack.json --ops arch/app/ops/ops_release.json --signing-key <signing_key.b64> --out dist/provenance.dsse.json --json
x07 wasm provenance verify --attestation dist/provenance.dsse.json --pack-dir dist/app.pack --trusted-public-key <public_key.b64> --json
```

Runtime enforcement via ops profiles:

- `x07 wasm serve --ops <ops.json>` applies capability enforcement to WASI 0.2 HTTP components.
- `x07 wasm http serve --ops <ops.json>` applies capability enforcement to the core-wasm HTTP reducer effects (for example `http.fetch` and `time.now`).
- `x07 wasm app serve --ops <ops.json>` applies capability enforcement to backend requests served via the in-proc component host.
- `x07 wasm app serve --mode canary --ops <ops.json>` includes an SLO decision (if the ops profile includes an SLO reference) under `result.stdout_json.canary.slo_decision`.

Record/replay evidence (clocks/random + secret delivery metadata):

- If caps use `clocks.mode=record` or `random.mode=record`, `x07 wasm serve` requires:
  - `--evidence-out <path>` (record), or
  - `--evidence-in <path>` (replay).
- Secrets are allowlisted via `caps.secrets.allow[]` and sourced from `.x07/secrets/<id>` or env `X07_SECRET_<ID>` (values are not recorded; only metadata is).

Provenance notes:

- Attestations include `predicate.x07.compatibility_hash` (matches `x07 wasm ops validate`).
- `x07 wasm provenance attest` fails closed (no DSSE envelope is written) if it encounters errors while resolving subjects, and writes DSSE output atomically (`*.tmp` then rename).
- `x07 wasm provenance verify` verifies the DSSE signature and then recomputes subject digests against `--pack-dir`.
- Verification commands stream digests and enforce hard size caps to avoid unbounded reads (pack manifest 8 MiB; pack files/subjects 256 MiB; DSSE attestation 16 MiB).
- `x07 wasm provenance attest` fails closed (does not write the DSSE output) when any subject path is unsafe and emits `X07WASM_PROVENANCE_SUBJECT_PATH_UNSAFE` (exit code 1).
