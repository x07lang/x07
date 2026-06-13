# Repositories

The `x07lang` GitHub org is split into focused repos. The active set was deliberately narrowed in 2026-06 to concentrate on X07 as the deterministic, certifiable execution substrate for agent-written software (see the [roadmap](../roadmap.md)).

## Active repos (2026-06 scope)

- `x07lang/x07`: the canonical toolchain repo. Compiler, runners, stdlib and package sources, schemas, specs, verification/certification tooling, and the docs source for x07lang.org.
- `x07lang/x07-mcp`: the MCP kit plus the official `io.x07/x07lang-mcp` server for driving the toolchain from agent runtimes.
- `x07lang/x07-registry`: the package registry backend.
- `x07lang/x07-wasm-backend`: the WASM toolchain (modules and WASI components) for portable sandboxed execution.
- `x07lang/hardproof`: the standalone verifier CLI for MCP server quality and trust checks.

## Supporting repos

- `x07lang/x07-rfcs`: the public RFC process and design-record repo (RFC 0001 x07text, RFC 0002 expressiveness floor).
- `x07lang/x07-website`: the `x07lang.org` site built from released docs bundles.
- `x07lang/x07-perf-compare`: performance comparison harnesses and reproducible benchmark snapshots.

## Archived (2026-06 scope cut)

These repos were archived in the 2026-06 refocus and are read-only on GitHub (rationale in the [roadmap](../roadmap.md)):

- `x07lang/x07-studio`: Studio/Forge shells and the Loom lifecycle daemon.
- `x07lang/x07-forge`: terminal shell over the XTAL loop.
- `x07lang/x07-crewops`: multi-role showcase app.
- `x07lang/x07-tactics`: tactics showcase.
- `x07lang/x07-web-ui`: reducer-style web UI contracts and browser host.
- `x07lang/x07-device-host`: desktop and mobile WebView host for packaged device apps.
- `x07lang/x07-registry-web`: the package registry UI at `x07.io`.
- `x07lang/x07-sentinel-reference-stack`: backend reference system for the former managed-platform offering.
- `x07lang/x07-platform` and `x07lang/x07-platform-contracts`: workload/runtime control plane and its public `lp.*` contracts.

The private `x07lang/x07-platform-cloud` (managed control layer) was archived alongside the platform repos. `x07lang/x07-infra-private` holds operational infrastructure.

## Governance scope

For governance purposes, the official companion repositories currently cited alongside `x07` are listed in `GOVERNANCE.md`; archived repos remain in governance scope as read-only history.
