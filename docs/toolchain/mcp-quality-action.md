# MCP quality: GitHub Actions

You can run `x07-mcp-test` in CI by downloading a prebuilt binary and executing the verifier against your server in a workflow job.

At a minimum, a CI run usually does:

1) install `x07-mcp-test`
2) run `x07-mcp-test doctor` to validate prerequisites
3) start your MCP server (HTTP)
4) run conformance and upload artifacts (`summary.json`, `summary.junit.xml`, `summary.html`)

## Minimal workflow sketch

This is intentionally a sketch (private alpha). Wire the install and server start steps to match your repo.

```yaml
name: mcp-quality
on: [push, pull_request]
jobs:
  conformance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install x07-mcp-test
        run: |
          echo "Download x07-mcp-test from GitHub Releases and place on PATH"
      - name: Doctor
        run: x07-mcp-test doctor --machine json
      - name: Start server
        run: |
          echo "Start your MCP server here"
      - name: Conformance
        run: x07-mcp-test conformance run --url "http://127.0.0.1:3000/mcp" --out out/conformance --machine json
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: conformance
          path: out/conformance/
```

