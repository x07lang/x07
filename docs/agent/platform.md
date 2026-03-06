# Platform (x07lp)

The x07 Platform (x07-LP) is a contract-first lifecycle engine that consumes sealed artifacts (starting with `x07.app.pack@0.1.0`) and produces deterministic, machine-readable deployment, routing, decision, incident, and regression artifacts.

Repo: `x07lang/x07-platform`  
CLI: `x07lp`  
MCP tool namespace: `lp.*`

Current public surfaces:

- `lp.*` JSON Schemas, including:
  - deploy state and query contracts: `lp.deploy.execution@0.1.0`, `lp.deploy.execution.meta.local@0.1.0`, `lp.deploy.query.result@0.1.0`
  - incident and regression contracts: `lp.incident.bundle@0.1.0`, `lp.incident.bundle.meta.local@0.1.0`, `lp.incident.query.result@0.1.0`, `lp.regression.request@0.1.0`, `lp.regression.run.result@0.1.0`
  - control and inventory contracts: `lp.control.action.result@0.1.0`, `lp.app.list.result@0.1.0`
- `x07lp` local CLI for:
  - `change new|validate`
  - `deploy accept|run|query|status|pause|rerun|stop|rollback`
  - `incident list|get|capture`
  - `regress from-incident`
  - `app list|kill|unkill`
  - `platform kill|unkill`
  - `ui serve`
- local content-addressed store and execution state rooted at `--state-dir`
- deterministic local deploy flow with candidate/stable slots, weighted routing, retry accounting, signed manual control actions, incident capture, and regression generation
- MCP tools for deploy, incident, regression, app, and platform control flows
- a local Command Center surface served by `x07lpd`

The local deploy executor consumes `x07.deploy.plan@0.2.0`, drives `x07-wasm deploy plan`, `x07-wasm slo eval`, and `x07-wasm app regress from-incident` where needed, and persists machine-readable reports that can be queried by deployment id, incident id, or latest app/environment pair. The shared runtime path is implemented once in the Rust `x07lp-driver` so the CLI, MCP surface, and `x07lpd` daemon use the same execution and persistence logic.
