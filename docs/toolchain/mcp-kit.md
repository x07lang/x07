# MCP Kit

`x07-mcp` is the MCP server kit for X07. It ships templates, package modules, and a dedicated CLI for scaffold/check/bundle/conformance workflows.

Phase 4 adds:

- Streamable HTTP SSE (`POST /mcp` streaming and `GET /mcp` listen streams)
- progress tokens (`_meta.progressToken`) + `notifications/progress`
- explicit cancellation (`notifications/cancelled`)
- `resources/subscribe` / `resources/unsubscribe` + `notifications/resources/updated`
- deterministic HTTP+SSE RR fixtures (`*.http_sse.session.jsonl`)

## Delegation model

The core toolchain delegates MCP kit commands to `x07-mcp`:

- `x07 mcp ...` delegates to `x07-mcp ...` on PATH.
- `x07 init --template mcp-server|mcp-server-stdio|mcp-server-http` delegates scaffold generation to `x07-mcp scaffold init`.

If `x07-mcp` is not installed on PATH, delegated commands exit with code `2`.

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
- deterministic HTTP replay fixtures under `tests/.x07_rr/sessions/`

## Conformance workflow

Run MCP conformance against a running server:

```sh
x07 mcp conformance --url http://127.0.0.1:8080/mcp
```

Or spawn a reference server in the same command:

```sh
x07 mcp conformance \
  --baseline conformance/conformance-baseline.yml \
  --spawn postgres-mcp \
  --mode noauth
```

When `--url` is omitted with `--spawn`, the harness derives host/port/path from the selected server config.

Default run mode executes the Phase-4 regression scenarios:

- `server-initialize`
- `ping`
- `tools-list`
- `tools-call-with-progress`
- `resources-subscribe`
- `resources-unsubscribe`
- `server-sse-multiple-streams`
- `dns-rebinding-protection`

Use `--full-suite` to run the full active conformance suite.

Current policy: keep `conformance/conformance-baseline.yml` empty and fail CI on regressions.

## Registry and publish workflow

Build deterministic bundle artifacts:

```sh
x07 mcp bundle --mcpb --server-dir servers/postgres-mcp
```

Generate registry `server.json`:

```sh
x07 mcp registry gen \
  --in servers/postgres-mcp/x07.mcp.json \
  --out servers/postgres-mcp/dist/server.json \
  --mcpb servers/postgres-mcp/dist/postgres-mcp.mcpb
```

Validate publish inputs:

```sh
x07 mcp publish --dry-run \
  --server-json servers/postgres-mcp/dist/server.json \
  --mcpb servers/postgres-mcp/dist/postgres-mcp.mcpb
```

## Reference server set

The kit includes these reference servers:

- `github-mcp`
- `slack-mcp`
- `jira-mcp`
- `postgres-mcp`
- `redis-mcp`
- `s3-mcp`
- `kubernetes-mcp`
- `stripe-mcp`
- `smtp-mcp`
- `http-proxy-mcp`

## OAuth and replay in template tests

- OAuth fixture tokens (`TOKEN_OK`, `TOKEN_NO_SCOPE`) are defined in `config/mcp.oauth.json` for deterministic local tests.
- `tests/mcp_http_replay.x07.json` replays HTTP and HTTP+SSE cassettes to verify protocol/auth/streaming guardrails without live network dependencies.
