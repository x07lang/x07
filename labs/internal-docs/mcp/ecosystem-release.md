# MCP Kit ecosystem release checklist

This note tracks the MCP kit conformance + registry + packaging release policy.

## Pins

- MCP protocol version: `2025-11-25`
- Registry schema URL: `https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json`
- Conformance runner: `@modelcontextprotocol/conformance@0.1.13`
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

## Release checklist

1. Run `x07-mcp` checks (`./scripts/ci/check_all.sh` and reference server suites).
2. Build deterministic `.mcpb` artifact(s) and verify stable SHA-256 across repeat builds.
3. Generate `server.json` from `x07.mcp.json` and validate schema + non-schema constraints.
4. Run `x07 mcp publish --dry-run` for release artifacts.
5. Confirm docs and pin tables are synchronized across `x07-mcp` and `x07`.
