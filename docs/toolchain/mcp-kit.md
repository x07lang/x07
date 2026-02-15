# MCP Kit

`x07-mcp` is the MCP server kit for X07. It ships templates, package modules, and a dedicated CLI for scaffold/check/bundle/conformance workflows.

## Delegation model

The core toolchain delegates MCP workflows to `x07-mcp`:

- `x07 mcp ...` delegates directly to `x07-mcp ...`.
- `x07 init --template mcp-server|mcp-server-stdio|mcp-server-http` delegates scaffold generation to `x07-mcp scaffold init`.

If `x07-mcp` is not installed on PATH, delegation exits with code `2`.

## HTTP template quickstart

```sh
x07 init --template mcp-server-http --dir ./my-mcp-http
cd ./my-mcp-http
x07 pkg lock
x07 test --manifest tests/tests.json
```

The HTTP template includes:

- `config/mcp.server.json` (`x07.mcp.server_config@0.2.0`)
- `config/mcp.tools.json` (`x07.mcp.tools_manifest@0.2.0`)
- `config/mcp.oauth.json` (`x07.mcp.oauth@0.1.0`, with deterministic `test_static` dev tokens)
- deterministic HTTP replay fixtures under `tests/fixtures/rr/http/`

## OAuth and replay in template tests

- OAuth fixture tokens (`TOKEN_OK`, `TOKEN_NO_SCOPE`) are defined in `config/mcp.oauth.json` for deterministic local tests.
- `tests/mcp_http_replay.x07.json` replays HTTP cassettes to verify protocol/auth guardrails without live network dependencies.
