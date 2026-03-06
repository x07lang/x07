# Platform (x07lp)

The x07 Platform (x07-LP) is a contract-first lifecycle engine that consumes sealed artifacts (starting with `x07.app.pack@0.1.0`) and produces deterministic, machine-readable deployment, routing, and decision artifacts.

Repo: `x07lang/x07-platform`  
CLI: `x07lp`  
MCP tool namespace: `lp.*`

Current public surfaces:

- `lp.*` JSON Schemas, including `lp.deploy.execution@0.1.0`, `lp.deploy.execution.meta.local@0.1.0`, and `lp.deploy.query.result@0.1.0`
- `x07lp` local CLI for `change`, `deploy accept`, `deploy run`, `deploy query`, `deploy status`, `deploy stop`, `deploy rollback`, and `schema` helpers
- local content-addressed store and execution state rooted at `--state-dir`
- deterministic local deploy flow with candidate/stable slots, weighted routing, retry accounting, and promote/rollback decisions
- MCP tools `lp.deploy.accept`, `lp.deploy.run`, `lp.deploy.query`, `lp.deploy.status`, `lp.deploy.stop`, and `lp.deploy.rollback`

The local deploy executor consumes `x07.deploy.plan@0.2.0`, drives `x07-wasm deploy plan` and `x07-wasm slo eval` where needed, and persists machine-readable reports that can be queried by deployment id or by latest app/environment pair.
