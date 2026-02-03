# External packages

External packages provide:

- OS integrations
- performance-critical native backends
- larger feature sets than stdlib should carry

External packages are distributed via the X07 registry:

- Human UI: https://x07.io/packages (JS app)
- Machine catalog: https://registry.x07.io/index/catalog.json

To use an external package from a project:

```bash
x07 pkg add <name>@<version> --sync
```

Notes:

- With `--sync`, `x07 pkg add` also runs `x07 pkg lock` to fetch deps and update `x07.lock.json`.
- `x07 pkg lock` defaults to the official registry index when fetching is required.
- Some packages may declare required helper packages via `meta.requires_packages`. When present, `x07 pkg lock` can add and fetch these transitive deps, but the canonical agent path is to use the capability map and templates so the dependency set is explicit.

Examples include:

- text utilities (`ext-text`) + Unicode helpers (`ext-unicode-rs`)
- BigInt (`ext-bigint-rs`) + Decimal (`ext-decimal-rs`)
- compact codecs: CBOR (`ext-cbor-rs`), MessagePack (`ext-msgpack-rs`)
- fast checksums: CRC32C / xxhash64 (`ext-checksum-rs`)
- deterministic diff + patch (`ext-diff-rs`)
- compression helpers: zstd framing (`ext-compress-rs`)
- archives (tar/tgz) + pinned extraction policies (`ext-archive-c`)
- OS-world glob + walk + ignore (`ext-path-glob-rs`)
- web routing + middleware + OpenAPI 3.1 (`ext-web-kit`)
- web crawling helpers (robots/sitemaps/urlnorm/scheduling + RR-friendly fetch) (`ext-web-crawl`)
- networking (HTTP/TLS, servers, client)
- modern protocol helpers (HTTP/2, WebSockets, gRPC) (`ext-net-protos-c`)
- databases (sqlite/pg/mysql/redis)
- deterministic migration plans (`ext-db-migrate`)
- filesystem utilities
- messaging core + RR format (`ext-msg-core`) and drivers (Kafka: `ext-msg-kafka-c`, AMQP: `ext-msg-amqp-c`)
- time + tzdb
- math (f64)
- regex (native implementation)
- observability (metrics snapshot + OpenMetrics + OTLP export) (`ext-obs`)
- auth helpers (Ed25519 + JWT) (`ext-auth-jwt`)
- CLI UX primitives (progress, tables, JSONL) (`ext-cli-ux`)

## Principle: one canonical binding per domain

External packages should not expose 3 competing ways to do the same thing.

Instead:

- define a pinned spec (bytes encoding)
- provide pack/unpack helpers
- keep native shims behind a stable ABI
