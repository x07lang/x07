# Repositories

The `x07lang` GitHub org is split into focused repos so each part of the language story stays understandable. Together they form one public ecosystem around X07 as an agent-first programming language.

- `x07lang/x07`: the canonical toolchain repo. Compiler, runners, stdlib and package sources, schemas, specs, and the docs source for x07lang.org.
- `x07lang/x07-mcp`: the MCP kit plus the official `io.x07/x07lang-mcp` server for coding, package, WASM, web UI, device, and platform workflows from agent runtimes.
- `x07lang/x07-wasm-backend`: the WASM toolchain, host runners, app bundling, browser UI support, device packaging, and incident-to-regression workflows.
- `x07lang/x07-web-ui`: the official reducer-style web UI contracts, browser host, and `std.web_ui.*` packages.
- `x07lang/x07-device-host`: the desktop and mobile WebView host for packaged X07 device apps.
- `x07lang/x07-wasi`: canonical `std.wasi.*` packages for WASI-facing X07 programs.
- `x07lang/x07-platform-contracts`: authoritative public `lp.*` lifecycle contracts, schema indexes, and registry mirror helpers.
- `x07lang/x07-platform`: public workload/runtime control plane, `x07lp` CLI, adapters, fixtures, and platform docs.
- `x07lang/x07-registry`: the package registry backend.
- `x07lang/x07-registry-web`: the package registry UI at `x07.io`.
- `x07lang/x07-website`: the `x07lang.org` site built from released docs bundles.
- `x07lang/x07-perf-compare`: performance comparison harnesses and reproducible benchmark snapshots.
- `x07lang/x07-crewops`: a multi-role showcase app that demonstrates how the ecosystem fits together for a real product.

The `x07lang/x07-platform-cloud` repo is the private implementation repo for x07 Sentinel, the managed control layer built on the public platform split. `x07lang/x07-infra-private` holds operational infrastructure.
