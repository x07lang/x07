# MCP Kit ecosystem release checklist

This note tracks the MCP kit conformance + registry + packaging release policy.

## Pins

- MCP protocol version: `2025-11-25`
- Registry schema URL: `https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json`
- Conformance runner: `@modelcontextprotocol/conformance@0.1.13`
- MCPB CLI: `@anthropic-ai/mcpb@2.1.2`

## Conformance baseline policy

- Keep expected-failure baseline minimal.
- Current baseline keys:
  - `tools-call-with-progress`
  - `resources-subscribe`
- CI fails on regressions and also fails when baseline entries become stale.

## Release checklist

1. Run `x07-mcp` checks (`./scripts/ci/check_all.sh` and reference server suites).
2. Build deterministic `.mcpb` artifact(s) and verify stable SHA-256 across repeat builds.
3. Generate `server.json` from `x07.mcp.json` and validate schema + non-schema constraints.
4. Run `x07 mcp publish --dry-run` for release artifacts.
5. Confirm docs and pin tables are synchronized across `x07-mcp` and `x07`.
