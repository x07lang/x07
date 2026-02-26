# MCP Kit ecosystem release checklist

This note tracks the MCP kit conformance + registry + packaging release policy.

## Pins

- MCP protocol version: `2025-11-25`
- Registry schema URL: `https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json`
- Conformance runner: `@modelcontextprotocol/conformance@0.1.14`
- MCPB CLI: `@anthropic-ai/mcpb@2.1.2`

## Conformance baseline policy

- Keep expected-failure baseline minimal.
- Current baseline keys: _none_ (`server: []`, `client: []`).
- CI fails on regressions and also fails when baseline entries become stale.

## Phase 4 verification checks

- `POST /mcp` SSE flow:
  - prime event emitted first
  - progress notification emitted when `_meta.progressToken` is requested
  - final JSON-RPC response event emitted unless cancelled
- `GET /mcp` listen SSE flow:
  - subscription updates routed to listen stream only
  - no-broadcast routing between request/listen streams
- Resumption:
  - `Last-Event-ID` resumes from bounded stream buffer
  - replay does not cross stream keys
- Cancellation:
  - `notifications/cancelled` stops in-flight request
  - cancelled request produces no final response
- Origin enforcement:
  - invalid Origin on POST or GET returns `403`

## Phase 6 verification checks

- Task progress invariants:
  - task-augmented `tools/call` progress token remains valid for task lifetime
  - progress notifications stop after the task reaches a terminal state
  - progress notifications include related-task metadata
- Deterministic RR guardrails:
  - `hello_tasks_progress` replay fixture is enforced in CI
  - replay fails if progress continues after terminal

## Phase 7 verification checks

- Logging semantics:
  - `logging/setLevel` clamps + rejects invalid params deterministically
  - `notifications/message` emission is redacted, rate-limited, and replay-stable
- Observability outputs:
  - audit sink emits deterministic JSONL sidecars when enabled
  - metrics snapshot/export wiring is exercised in kit tests (when enabled)

## Phase 8 verification checks

- PRM (RFC9728):
  - insertion URL is served (derived from configured `oauth.resource`)
  - root alias is served only when `serve_root_alias=true`
  - response is `200` with `Content-Type: application/json` and `resource` matches config
- OAuth RS enforcement on `POST /mcp`:
  - `?access_token=...` is rejected with `400` (empty body)
  - missing Authorization returns `401` + `WWW-Authenticate` (resource metadata URL + scope)
  - insufficient scope returns `403` + `WWW-Authenticate ... error=\"insufficient_scope\"`
- Strict Streamable HTTP headers:
  - invalid Origin returns `403`
  - invalid/missing Accept returns `400`
  - invalid protocol version returns `400`
  - HTTP-level failures have empty bodies (status + headers only)
- RR sanitizer boundary:
  - redacts Authorization / Proxy-Authorization / Cookie / Set-Cookie
  - fails closed if token-like patterns survive sanitization

## Release checklist

1. Run `x07-mcp` checks (`./scripts/ci/check_all.sh` and reference server suites).
2. Build deterministic `.mcpb` artifact(s) and verify stable SHA-256 across repeat builds.
3. Generate `server.json` from `x07.mcp.json` and validate schema + non-schema constraints.
4. Run `x07 mcp publish --dry-run` for release artifacts.
5. Confirm docs and pin tables are synchronized across `x07-mcp` and `x07`.
