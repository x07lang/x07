# `x07crawl`

Reference implementation for **Roadmap App 2** in [`docs/agent/apps.md`](../../../agent/apps.md).

- Prompt: [`PROMPT.md`](PROMPT.md)
- Run: `mkdir -p out && x07 run --profile sandbox -- --mode replay --out out/crawl.json > out/crawl.json`
- Bundle: `x07 bundle --profile sandbox --out dist/x07crawl`
- Test: `x07 test --manifest tests/tests.json`

## Recording fixtures

Record HTTP responses into RR cassette files for offline replay testing:

```bash
# Record example.com
x07 rr record --cassette tests/fixtures/replay/rr/crawl_v1.rrbin \
  --op "std.crawl.fetch.http_get_v1" \
  "https://example.com/" "https://example.com/"

# Record robots.txt
x07 rr record --cassette tests/fixtures/replay/rr/robots_v1.rrbin \
  --op "std.crawl.fetch.http_get_v1" \
  "https://example.com/robots.txt" "https://example.com/robots.txt"
```
