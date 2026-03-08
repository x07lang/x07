# Platform (x07lp)

The x07 Platform (x07-LP) is a contract-first lifecycle engine that consumes sealed artifacts (starting with `x07.app.pack@0.1.0`) and produces deterministic, machine-readable deployment, routing, decision, incident, and regression artifacts.

Repos: `x07lang/x07-platform-contracts`, `x07lang/x07-platform`, `x07lang/x07-platform-cloud`  
CLI: `x07lp`  
MCP tool namespace: `lp.*`

Current public surfaces:

- `x07-platform-contracts` as the source of truth for public `lp.*` JSON Schemas, including:
  - deploy state and query contracts: `lp.deploy.execution@0.1.0`, `lp.deploy.execution.meta.local@0.1.0`, `lp.deploy.query.result@0.1.0`
  - incident and regression contracts: `lp.incident.bundle@0.1.0`, `lp.incident.bundle.meta.local@0.1.0`, `lp.incident.query.result@0.1.0`, `lp.regression.request@0.1.0`, `lp.regression.run.result@0.1.0`
  - remote target and adapter contracts: `lp.target.profile@0.1.0`, `lp.target.list.result@0.1.0`, `lp.deploy.push.result@0.1.0`, `lp.deploy.remote.result@0.1.0`, `lp.remote.capabilities.response@0.1.0`, `lp.adapter.capabilities@0.1.0`, `lp.adapter.conformance.report@0.1.0`
  - control and inventory contracts: `lp.control.action.result@0.1.0`, `lp.app.list.result@0.1.0`
- `x07lp` local CLI for:
  - `change new|validate`
  - `deploy accept|run|query|status|pause|rerun|stop|rollback` with optional `--target <name>`
  - `target add|ls|inspect|use|rm`
  - `adapter conformance`
  - `incident list|get|capture` with optional `--target <name>`
  - `regress from-incident` with optional `--target <name>`
  - `app list|kill|unkill`
  - `platform kill|unkill`
  - `ui serve`
- local content-addressed store and execution state rooted at `--state-dir`
- deterministic local deploy flow with candidate/stable slots, weighted routing, retry accounting, signed manual control actions, incident capture, and regression generation
- self-hosted remote deploy parity in `x07-platform` using target profiles, remote CAS push, remote query/control flows, incident/regression parity, and the first OSS adapter set (`wasmcloud`, edge HTTP routing, OTLP HTTP telemetry, server-side secrets, OCI component registry)
- the supported wasm toolchain surface for platform flows is `x07 wasm app pack|app verify|deploy plan|slo eval|app regress from-incident`
- MCP tools for deploy, incident, regression, app, and platform control flows
- a self-hosted Command Center and remote control-plane surface served by `x07lpd`

Repository split:

- `x07-platform-contracts`: authoritative public platform schemas, examples, reason codes, and registry export helpers.
- `x07-platform`: public engine, CLI, MCP, self-hosted daemon, UI, adapters, fixtures, and CI.
- `x07-platform-cloud`: private hosted-product layer for identity, tenancy, metering, billing, managed services, and multi-tenant runtime concerns.

`x07-platform` consumes `x07-platform-contracts` through `contracts/`, and `x07-platform-cloud` consumes both `contracts/` and `platform/` without forking public engine code. The shared runtime path is implemented once in the Rust `x07lp-driver` so the CLI, MCP surface, and `x07lpd` daemon use the same execution and persistence logic across local and self-hosted remote targets.
