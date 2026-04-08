# MCP quality (verifier wedge)

Official MCP conformance is the baseline. Hardproof (`hardproof`) layers additional verification on top of that baseline so you can review and gate MCP servers with machine-readable evidence.

What Hardproof does:

- **Scan**: run deterministic verification across five dimensions (conformance, reliability, performance, security, trust) and emit a stable scan report (`scan.json`) plus an event stream (`scan.events.jsonl`).
- **CI gating**: run `hardproof ci` to enforce policies (minimum score, zero critical findings, per-dimension minimums, usage limits).
- **Replay**: record a small session cassette and replay it to detect behavior drift with deterministic diffs.
- **Trust + bundle**: verify registry metadata (`server.json`) and bundle integrity (`.mcpb`) as part of a scan (when inputs are provided) or via explicit subcommands.
- **Report export**: render a scan summary or export SARIF/HTML from an existing scan report.

What it does *not* do (yet):

- It does not replace the official MCP SDKs, Inspector, or Registry story.
- It does not make strong security claims from checks alone; it emits evidence you can review and enforce in your own workflows.

## Quickstart

1) Install `hardproof`: see [MCP quality: install](mcp-quality-install.md).

2) Run diagnostics:

```sh
hardproof doctor
hardproof doctor --machine json
```

3) Conformance:

```sh
hardproof scan --url "http://127.0.0.1:3000/mcp" --out out/scan --format json
```

Artifacts are written under `out/scan/`:

- `scan.json`
- `scan.events.jsonl`

4) Replay:

```sh
hardproof replay record --url "http://127.0.0.1:3000/mcp" --out out/replay.session.json --machine json
hardproof replay verify --session out/replay.session.json --url "http://127.0.0.1:3000/mcp" --out out/replay-verify --machine json
```

5) Trust and bundle verification (registry artifacts):

```sh
hardproof trust verify --server-json ./server.json --machine json
hardproof bundle verify --server-json ./server.json --mcpb ./bundle.mcpb --machine json
```

If you have both `server.json` and `.mcpb`, you can also include trust checks in the scan:

```sh
hardproof scan \
  --url "http://127.0.0.1:3000/mcp" \
  --server-json ./server.json \
  --mcpb ./bundle.mcpb \
  --out out/scan \
  --format json
```

Score semantics are explicit in the report:

- `score_mode=full`: `overall_score` is populated and the scan is eligible for a full score.
- `score_mode=partial`: `overall_score` stays `null`, `partial_score` remains machine-readable, and rich output withholds the primary score.

`hardproof ci` now fails on `score_mode=partial` by default. Use `--allow-partial-score` only when a partial gate is intentional.

## x07-native path (optional)

If you want to **build** an MCP server in X07 (not just verify one), use `x07lang/x07-mcp`.

The shortest zero-install path is Codespaces: see [MCP quality: Codespaces](mcp-codespaces.md).

## Hardproof as a reference x07 application

Hardproof is built as an x07-native application (CLI + contracts + deterministic evidence).
If you want a concrete example of x07 primitives working together, Hardproof uses:

- Streaming composition: `std.stream.pipe_v1`
- Structured concurrency: `task.scope_v1`
- Record/replay as first-class evidence (`std.rr` + replay cassettes)
- Deterministic CLI UX composition via `ext-cli-ux`, including `std.cli.panel.render_titled_v2`, `std.cli.bar.render_threshold_v2`, `std.cli.status.render_pill_v2`, `std.cli.layout.stack_v1`, `std.cli.layout.columns_auto_v2`, and `std.cli.events.render_progress_v2`
- Deterministic project/tooling checks: `x07 fmt`, `x07 pkg lock --check`, `x07 arch check`, `x07 test`, `x07 verify`
- Verification + review artifacts: `x07 prove check`, `x07 trust certify`, `x07 trust report`
