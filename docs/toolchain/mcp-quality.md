# MCP quality (verifier wedge)

Official MCP conformance is the baseline. `x07-mcp-test` layers additional verification on top of that baseline so you can review and gate MCP servers with machine-readable evidence.

What `x07-mcp-test` does:

- **Conformance**: run the official MCP conformance suite and emit JSON/JUnit/HTML artifacts.
- **Replay**: record a small HTTP session cassette and replay it to detect behavior drift with deterministic diffs.
- **Trust**: validate required trust metadata in an MCP registry `server.json`.
- **Bundle**: validate that a `server.json` and `.mcpb` bundle are consistent (hash/metadata mismatch detection).

What it does *not* do (yet):

- It does not replace the official MCP SDKs, Inspector, or Registry story.
- It does not make strong security claims from checks alone; it emits evidence you can review and enforce in your own workflows.

## Quickstart

1) Install `x07-mcp-test`: see [MCP quality: install](mcp-quality-install.md).

2) Run diagnostics:

```sh
x07-mcp-test doctor
x07-mcp-test doctor --machine json
```

3) Conformance:

```sh
x07-mcp-test conformance run --url "http://127.0.0.1:3000/mcp" --out out/conformance --machine json
```

4) Replay:

```sh
x07-mcp-test replay record --url "http://127.0.0.1:3000/mcp" --out out/replay.session.json --machine json
x07-mcp-test replay verify --session out/replay.session.json --url "http://127.0.0.1:3000/mcp" --out out/replay-verify --machine json
```

5) Trust and bundle verification (registry artifacts):

```sh
x07-mcp-test trust verify --server-json ./server.json --machine json
x07-mcp-test bundle verify --server-json ./server.json --mcpb ./bundle.mcpb --machine json
```

## x07-native path (optional)

If you want to **build** an MCP server in X07 (not just verify one), use `x07lang/x07-mcp`.

The shortest zero-install path is Codespaces: see [MCP quality: Codespaces](mcp-codespaces.md).

