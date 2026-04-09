# MCP quality: GitHub Actions

You can run Hardproof (`hardproof`) in CI by downloading a prebuilt binary or using the Hardproof Scan GitHub Action.

At a minimum, a CI run usually does:

1) install `hardproof`
2) run `hardproof doctor` to validate prerequisites
3) start your MCP server (HTTP)
4) run `hardproof ci` and upload the scan report artifacts (`scan.json`, `scan.events.jsonl`)

Hardproof reports **score truth** explicitly:

- `score_mode=full` means `overall_score` is present and eligible as a full score.
- `score_mode=partial` keeps `overall_score=null` and provides `partial_score` plus `gating_reasons` (commonly because Trust inputs were not provided).

Token/context usage truth is explicit under `scan.json.usage_metrics.usage_mode`:

- `estimate` (deterministic estimates)
- `tokenizer_exact` (exact counts under a chosen tokenizer profile)
- `trace_observed` (observed counts from a real trace)
- `mixed` (per-metric mix of exact + observed)

## Minimal workflow sketch

This is intentionally a sketch (public beta). Wire the install and server start steps to match your repo.

```yaml
name: mcp-quality
on: [push, pull_request]
jobs:
  conformance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Start server
        run: |
          echo "Start your MCP server here"
      - name: Hardproof Scan (beta)
        uses: x07lang/hardproof/hardproof-scan@latest-beta
        with:
          url: http://127.0.0.1:3000/mcp
          version: latest-beta
          threshold: "80"
          full-suite: "false"
          sarif: "true"
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: conformance
          path: out/scan/
```
