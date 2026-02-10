# `x07-api-gateway`

Reference implementation for **Roadmap App 1** in [`docs/agent/apps.md`](../../../agent/apps.md).

- Prompt: [`PROMPT.md`](PROMPT.md)
- Run: `x07 run --profile sandbox`
- Bundle: `x07 bundle --profile sandbox --out dist/x07-api-gateway`
- Test: `x07 test --manifest tests/tests.json`

## Recording fixtures

Record an upstream HTTP response into an RR cassette for solve-rr tests:

```bash
x07 rr record --cassette tests/fixtures/replay/rr/upstream_example.rrbin \
  --kind http \
  --op "std.net.http.client.get_v1" \
  "https://example.com/" "https://example.com/"
```

