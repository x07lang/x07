# Platform (x07lp)

The x07 Platform (x07-LP) is a contract-first lifecycle engine that consumes sealed artifacts (starting with `x07.app.pack@0.1.0`) and produces deterministic, machine-readable run + decision artifacts.

Repo: `x07lang/x07-platform`  
CLI: `x07lp`  
MCP tool namespace: `lp.*`

Phase A provides:

- `lp.*` JSON Schemas
- `x07lp` local CLI (`change`, `deploy`, and `schema` helpers)
- local content-addressed store (`--state-dir`)
- MCP router/worker skeleton using x07-mcp patterns

