# Guide: Web crawling and sitemaps

X07 supports deterministic crawler logic by splitting **pure scheduling/parse code** from **OS-bound fetching**:

- `ext-web-crawl` (pinned by `arch/crawl/`) provides:
  - robots.txt parsing (`std.crawl.robots`) per RFC 9309
  - sitemap parsing (`std.crawl.sitemap`)
  - URL canonicalization (`std.crawl.urlnorm`)
  - deterministic crawl scheduling (`std.crawl.schedule`)
- Fetch is OS-bound, but RR-friendly:
  - `std.crawl.fetch.replay_rr_v1(...)` (pure replay from an rr cassette entry)
  - `std.crawl.fetch.os.run_rr_v1(...)` / `std.crawl.fetch.os.run_rr_missing_v1(...)` (OS wrappers for record/replay)

## Canonical workflow

1. Keep crawl planning and parsing in pure modules (`solve-pure`).
2. Record real fetch results once under an rr policy like `crawl_rr_v1` and keep the cassette under `.x07_rr/`.
3. Replay fetch results deterministically in `solve-rr` using `std.crawl.fetch.replay_rr_v1`.

`x07 arch check` validates pinned contracts under `arch/crawl/**` and enforces the world split (no OS fetch imports in solve worlds).

## Adding the crawler package

Use the capability map (`web.crawl`) and sync the lockfile:

```bash
# Pick NAME@VERSION from /agent/latest/catalog/capabilities.json.
x07 pkg add NAME@VERSION --sync
```

## Expert (operations)

- Treat robots rules as a crawler behavior contract, not access control; apply your own auth and allowlists separately.
- Prefer `run-os-sandboxed` for fetch recording runs; keep cassettes deterministic and review sanitizers for any sensitive headers.
