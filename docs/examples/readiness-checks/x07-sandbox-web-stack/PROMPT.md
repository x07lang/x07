# Prompt 5 — Sandboxed web stack (HTTP + crawler/search + WS/gRPC framing + observability)

```text
You are an X07 coding agent building `x07-sandbox-web-stack`.

GOAL
Build a single project that contains BOTH:
1) a local HTTP server (deterministic integration target)
2) a sandboxed crawler/client that fetches pages, respects robots.txt, parses sitemap, extracts links + visible text, and performs high-performance searches
3) a non-HTTP protocol coverage path via loopback TCP that exchanges:
   - WebSocket frames (framing helpers)
   - gRPC length-prefixed messages (message framing helpers)
4) observability outputs: canonical JSON report + OpenMetrics snapshot

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir x07-sandbox-web-stack
cd x07-sandbox-web-stack
x07 init --template web-service

DEPENDENCIES (lock them)
HTTP + crawling (run-os only):
- ext-net
- ext-sockets-c
- ext-curl-c
- ext-url-rs
- ext-web-kit
- ext-web-crawl
Modern protocols (run-os only):
- ext-net-protos-c (std.net.ws + std.net.grpc helpers)
Content parsing/search:
- ext-html-lite-rs
- ext-robots-txt-rs
- ext-sitemap-rs
- ext-regex
- ext-aho-corasick-rs
- ext-memchr-rs
Text handling:
- ext-text
- ext-unicode-rs
Observability:
- ext-log
- ext-tracing
- ext-obs
CLI:
- ext-cli
- ext-cli-ux
Note: x7sl “slice documents” are provided by stdlib `std.text.slices` (brand `std.text.slices.x7sl_v1`).

SANDBOX POLICY REQUIREMENTS
- Run everything under run-os-sandboxed.
- Use fixed loopback ports:
  - HTTP server: 127.0.0.1:18080
  - TCP proto service: 127.0.0.1:18081
- Allow network ONLY to those loopback ports (explicit ports only), for example:
  - `x07 run --profile sandbox --allow-host 127.0.0.1:18080,18081 -- ...`
- Deny all external hosts.
- Allow write only to out/ for reports and metrics snapshots.

FUNCTIONAL REQUIREMENTS
A) Server:
   - serve deterministic HTML pages + robots.txt + sitemap.xml
B) Crawler:
   - parse and enforce robots rules
   - parse sitemap and crawl discovered URLs
   - extract links + visible text from HTML
   - tokenize and normalize text, including unicode normalization/casefold tests
   - support pattern searches:
     - regex patterns
     - Aho–Corasick multi-pattern search
     - raw byte scanning via memchr
C) WS + gRPC framing over loopback TCP:
   - implement a tiny TCP protocol where:
     - client sends one WebSocket frame with an ASCII payload and receives a deterministic echo
     - client sends one gRPC msg-prefix frame with an ASCII payload and receives a deterministic echo
   - use `std.net.ws.frame_v1` / accessors and `std.net.grpc.msg_prefix_v1` / `std.net.grpc.msg_unprefix_v1` (framing only; no WS handshake / no gRPC service server).
D) Observability + output:
   - emit structured logs and trace spans for each fetch/parse/search stage
   - maintain metrics counters and export an OpenMetrics snapshot under out/
   - print one canonical JSON report with:
     - fetched URLs
     - extracted links
     - search hits
     - WS/gRPC transcripts
     - policy info (what was allowed)

TESTS
- A test entrypoint that uses the fixed ports above, runs the crawler against the local server, exercises the loopback WS/gRPC framing path, and asserts output stability.
```

