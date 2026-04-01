# MCP quality: Codespaces

Codespaces is the recommended zero-install evaluation path for the private alpha:

- run a minimal x07-native MCP server (`x07lang/x07-mcp`)
- verify it locally with `x07-mcp-test`

## Quickstart

1) Open a Codespace:

- `https://codespaces.new/x07lang/x07-mcp?quickstart=1`

2) Install the verifier from the repo root:

```sh
./scripts/dev/install_x07_mcp_test.sh
```

3) Build and run the minimal HTTP example:

```sh
cd examples/private-alpha-http-hello
x07 bundle --project x07.json --profile os --out out/mcp-router
x07 bundle --project x07.json --profile sandbox --program src/worker_main.x07.json --out out/mcp-worker
./out/mcp-router
```

4) Verify in another terminal:

```sh
x07-mcp-test conformance run --url "http://127.0.0.1:8314/mcp" --out out/conformance --machine json
x07-mcp-test replay record --url "http://127.0.0.1:8314/mcp" --out out/replay.session.json --machine json
x07-mcp-test replay verify --session out/replay.session.json --url "http://127.0.0.1:8314/mcp" --out out/replay-verify --machine json
```

