# X07 external packages: v1 supported set (production)

This doc defines the **v1 production supported set** of external packages shipped from this repo under `packages/ext/`.
All external packages are pinned + hashed in `locks/external-packages.lock`, but **not all packages are part of the supported surface**.

## Supported external packages (v1)

### Pure (solve-* capable)

These packages are deterministic and usable in `solve-*` and `run-os*` worlds.

**Core docs + errors**

- `ext-data-model` (`packages/ext/x07-ext-data-model/0.1.0/`) — `ext.data_model` (canonical bytes doc format for external parsers/decoders)
  - `ext.data_model.json.emit_canon` (`ext.data_model.json`) — emit canonical JSON bytes from an `ext.data_model` doc
  - `ext.data_model.toml.emit_canon` (`ext.data_model.toml`) — emit canonical TOML bytes from an `ext.data_model` doc (strict subset)
  - `ext.data_model.yaml.emit_canon` (`ext.data_model.yaml`) — emit canonical YAML bytes from an `ext.data_model` doc (YAML 1.2 JSON subset)
- `ext-error` (`packages/ext/x07-ext-error/0.1.0/`) — `ext.error.*` (error patterns: context/chain/fmt)

**Formats + schemas**

- `ext-url-rs` (`packages/ext/x07-ext-url-rs/0.1.0/`) — `ext.url.*`, `ext.http_types`, `ext.httparse`
- `ext-json-rs` (`packages/ext/x07-ext-json-rs/0.1.0/`) — `ext.json.pointer`, `ext.json.canon`, `ext.json.data_model` (complements `stdlib/std/0.1.1/modules/std/json.x07.json`)
- `ext-toml-rs` (`packages/ext/x07-ext-toml-rs/0.1.0/`) — `ext.toml`, `ext.toml.data_model`
- `ext-yaml-rs` (`packages/ext/x07-ext-yaml-rs/0.1.0/`) — `ext.yaml`, `ext.yaml.data_model`
- `ext-ini-rs` (`packages/ext/x07-ext-ini-rs/0.1.0/`) — `ext.ini`, `ext.ini.data_model`
- `ext-csv-rs` (`packages/ext/x07-ext-csv-rs/0.1.0/`) — `ext.csv`, `ext.csv.data_model`
- `ext-xml-rs` (`packages/ext/x07-ext-xml-rs/0.1.0/`) — `ext.xml`, `ext.xml.data_model`
- `ext-pb-rs` (`packages/ext/x07-ext-pb-rs/0.1.0/`) — `ext.pb.wire`, `ext.pb.data_model`

**Compression + archives**

- `ext-compress-rs` (`packages/ext/x07-ext-compress-rs/0.1.0/`) — `ext.compress` (inflate/zlib/gzip) + `ext.zip` (list/extract caps)
- `ext-tar-rs` (`packages/ext/x07-ext-tar-rs/0.1.0/`) — `ext.tar` (bounded lookup: `find_file_v1`)

**Crypto**

- `ext-crypto-rs` (`packages/ext/x07-ext-crypto-rs/0.1.0/`) — `ext.crypto` (`sha256`, `sha512`, `hmac_sha256`, `hkdf_sha256_*`, `eq_ct`)

**Time**

- `ext-time-rs` (`packages/ext/x07-ext-time-rs/0.1.0/`) — `ext.time.duration`, `ext.time.rfc3339`, `ext.time.civil`, `ext.time.instant`, `ext.time.tzdb` (pure) + `ext.time.os` (run-os* only)

**Math**

- `ext-math` (`packages/ext/x07-ext-math/0.1.0/`) — `std.math.*` (F64LE bytes + deterministic native backend)

**Primitives (encoding, identifiers, search/text)**

- `ext-u64-rs` (`packages/ext/x07-ext-u64-rs/0.1.0/`) — `ext.u64` (u64 helpers, LE encoding)
- `ext-byteorder-rs` (`packages/ext/x07-ext-byteorder-rs/0.1.0/`) — `ext.byteorder`
- `ext-base64-rs` (`packages/ext/x07-ext-base64-rs/0.1.0/`) — `ext.base64`
- `ext-hex-rs` (`packages/ext/x07-ext-hex-rs/0.1.0/`) — `ext.hex`
- `ext-uuid-rs` (`packages/ext/x07-ext-uuid-rs/0.1.0/`) — `ext.uuid`
- `ext-semver-rs` (`packages/ext/x07-ext-semver-rs/0.1.0/`) — `ext.semver`
- `ext-unicode-rs` (`packages/ext/x07-ext-unicode-rs/0.1.0/`) — `ext.unicode` (utf8 validity, encoding conversions, grapheme slices, basic NFKC)
- `ext-memchr-rs` (`packages/ext/x07-ext-memchr-rs/0.1.0/`) — `ext.memchr`
- `ext-aho-corasick-rs` (`packages/ext/x07-ext-aho-corasick-rs/0.1.0/`) — `ext.aho_corasick`
- `ext-regex` (`packages/ext/x07-ext-regex/0.2.0/`) — `ext.regex` (byte regex; native backend; contract: `docs/text/regex-v1.md`; build: `./scripts/build_ext_regex.sh`; smoke: `./scripts/ci/check_regex_smoke.sh`)

**Observability (record encodings)**

- `ext-log` (`packages/ext/x07-ext-log/0.1.0/`) — `ext.log` (record encoding + formatting)
- `ext-tracing` (`packages/ext/x07-ext-tracing/0.1.0/`) — `ext.tracing` (span/event record encoding + formatting)

**Small utilities**

- `ext-streams` (`packages/ext/x07-ext-streams/0.1.0/`) — `ext.streams` (open deterministic readers from bytes/collections)

### OS-world-only (run-os / run-os-sandboxed)

These packages are standalone-only and must never be used in deterministic evaluation worlds.

- `ext-zlib-c` (`packages/ext/x07-ext-zlib-c/0.1.0/`) — `ext.zlib` (bounded inflate/zlib/gzip)
- `ext-openssl-c` (`packages/ext/x07-ext-openssl-c/0.1.0/`) — `ext.openssl.hash`, `ext.openssl.rand`, `ext.openssl.ed25519`
- `ext-curl-c` (`packages/ext/x07-ext-curl-c/0.1.0/`) — `ext.curl.http` (bounded HTTP; supports streaming body to file; exposes a file-backed `iface` reader via `resp_file_reader_v3`)
- `ext-sockets-c` (`packages/ext/x07-ext-sockets-c/0.1.0/`) — `ext.sockets.*` (DNS/TCP/UDP primitives; policy-gated; bytes ABIs: `docs/net/net-v1.md`)
- `ext-net` (`packages/ext/x07-ext-net/0.1.0/`) — agent-facing networking `std.net.*` (HTTP on `ext.curl.http`, DNS/TCP/UDP on `ext.sockets.*`; error codes: `docs/net/errors-v1.md`)
- `ext-fs` (`packages/ext/x07-ext-fs/0.1.0/`) — agent-facing filesystem `std.os.fs.*` (core ops: `docs/fs/fs-v1.md`)
- `ext-db-core` (`packages/ext/x07-ext-db-core/0.1.0/`) — agent-facing DB facade `std.db.*` + pinned contracts (`docs/db/db-v1.md`)
- `ext-db-sqlite` (`packages/ext/x07-ext-db-sqlite/0.1.0/`) — SQLite adapter `std.db.sqlite.*` + pool (`docs/db/sqlite-v1.md`, `docs/db/pool-v1.md`; smoke: `./scripts/ci/check_db_smoke.sh`)
- `ext-db-postgres` (`packages/ext/x07-ext-db-postgres/0.1.0/`) — Postgres adapter `std.db.pg.*` + pool (`docs/db/postgres-v1.md`, `docs/db/pool-v1.md`)
- `ext-db-mysql` (`packages/ext/x07-ext-db-mysql/0.1.0/`) — MySQL adapter `std.db.mysql.*` + pool (`docs/db/mysql-v1.md`, `docs/db/pool-v1.md`)
- `ext-db-redis` (`packages/ext/x07-ext-db-redis/0.1.0/`) — Redis adapter `std.db.redis.*` (`docs/db/redis-v1.md`)

## Not supported in v1 (present in-repo)

These are intentionally **not** part of the v1 production surface (can be kept for internal experiments).

- `ext-regex@0.1.0` (`packages/ext/x07-ext-regex/0.1.0/`) — legacy pure engine (superseded by `ext-regex@0.2.0`).

## Networking status (v1)

- `ext-net` includes TLS client streams (`std.net.tls`) and a minimal HTTP/1.1 server helper (`std.net.http.server`) (see `docs/phases/x07-network-packages-v1-plan.md`).

## Concrete change proposal (pre-v1)

### 1) Core async: add non-yielding `try_*` primitives

- Keep the compiler restriction: yielding ops (`task.join.bytes`, `chan.bytes.{send,recv}`) are only allowed in `solve` / `defasync`.
- Add non-yielding task/channel variants that are allowed in `defn`:
  - `task.is_finished` → `i32` (0/1)
  - `task.try_join.bytes` → `result_bytes` (err=1 not finished; err=2 canceled)
  - `chan.bytes.try_send` → `i32` status (0 full; 1 sent; 2 closed)
  - `chan.bytes.try_recv` → `result_bytes` (err=1 empty; err=2 closed)

### 2) Remaining high-leverage gaps (not solved by ext packages alone)

- HTTP server/networking primitives: deterministic evaluation worlds still need world adapters/runtime surface; HTTP parsing primitives exist via `ext.httparse`.
- Streaming HTTP as `iface` reader: `ext.curl.http.resp_file_reader_v3` opens the streamed-to-file body as an `iface` reader.
- Deterministic seeded PRNG: already in stdlib (`stdlib/std/0.1.1/modules/std/prng.x07.json`); remaining gaps are mostly ergonomics + higher-level APIs.
- `ext.data_model` emitters for “parse → transform → write”: JSON is covered via `ext.data_model.json.emit_canon`; TOML/YAML are covered via `ext.data_model.toml.emit_canon` and `ext.data_model.yaml.emit_canon`.
