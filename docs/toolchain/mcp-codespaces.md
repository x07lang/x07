# MCP quality: Codespaces

Codespaces is the recommended zero-install evaluation path for the verifier:

- run a minimal x07-native MCP server (`x07lang/x07-mcp`)
- verify it locally with Hardproof (`hardproof`)

## Quickstart

1) Open a Codespace:

- `https://codespaces.new/x07lang/x07-mcp?quickstart=1`

2) Install the verifier from the repo root:

```sh
./scripts/dev/install_hardproof.sh
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
hardproof scan --url "http://127.0.0.1:8314/mcp" --out out/scan --format json
hardproof replay record --url "http://127.0.0.1:8314/mcp" --out out/replay.session.json --machine json
hardproof replay verify --session out/replay.session.json --url "http://127.0.0.1:8314/mcp" --out out/replay-verify --machine json
```
