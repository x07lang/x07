# X07 external libraries: production-release gaps (x07AST)

This doc is a practical checklist of **which external libraries are still missing** (or too incomplete) to unlock smooth day-to-day X07 usage by developers, plus a snapshot of what already exists in this repo.

X07 source is **x07AST JSON only** (`*.x07.json`). Legacy S-expr (`*.sexpr`) is intentionally rejected by the toolchain.

## Tooling + format (non-negotiable)

- Create new modules/entries with `cargo run -p x07 -- ast init ...` (see `docs/dev/x07-ast.md`).
- Keep committed `*.x07.json` formatted with `target/debug/x07c fmt --input <file> --write` (CI enforces this via `scripts/check_x07_parens.py`).

## External packages already present (current repo state)

### Pure (solve-*) building blocks

- `packages/ext/x07-ext-u64-rs/0.1.0/`
  - `ext.u64`: u64 helpers (LE encoding as two i32 halves).
- `packages/ext/x07-ext-compress-rs/0.1.0/`
  - `ext.compress`: pure bounded decompression: `inflate_raw`, `zlib_decompress`, `gzip_decompress`.
  - `ext.zip`: zip read-only list/extract with caps (`list_names_v1`, `extract_file_v1`).
- `packages/ext/x07-ext-time-rs/0.1.0/`
  - `ext.time.rfc3339`: RFC3339 parse/format with deterministic bytes output, plus bracket tzid parsing (`Z[Etc/UTC]`), but **no tz database semantics**.
    - `parse_v2` / `format_v2`: i64 unix seconds (split into low+high u32 halves).
  - `ext.time.duration`: duration docs (u64 seconds + nanos) + arithmetic (`add_v1`, `sub_v1`).
- `packages/ext/x07-ext-crypto-rs/0.1.0/`
  - `ext.crypto`: `sha256`, `hmac_sha256`, `eq_ct`, `sha512`, `hkdf_sha256_*`.
- `packages/ext/x07-ext-pb-rs/0.1.0/`
  - `ext.pb.wire`: schema-less protobuf wire decode (`decode_v1`) and encode (`encode_v1`), plus `descriptor_set_to_schema_v1`.
  - `ext.pb.data_model`: schema-driven decode into `ext.data_model` docs (project-based tests require both module roots).
- `packages/ext/x07-ext-tar-rs/0.1.0/`
  - `ext.tar`: simple tar file lookup (`find_file_v1`) with strict size caps.

### OS-world-only adapters

- `packages/ext/x07-ext-curl-c/0.1.0/`
  - `ext.curl.http.request_v2`: bounded HTTP/file GET/POST via libcurl with response headers capture.
  - `ext.curl.http.req_*_to_file_v3`: stream body to a file (avoids buffering large responses in memory).
  - `run-os-sandboxed` hardening is enforced using `X07_OS_*` env provided by the runner (scheme gating, host allowlist, fs roots, no redirects).
- `packages/ext/x07-ext-zlib-c/0.1.0/`
  - `ext.zlib.inflate_raw`: bounded raw DEFLATE inflate.
  - `ext.zlib.gzip_decompress`: bounded gzip decompression.
- `packages/ext/x07-ext-openssl-c/0.1.0/`
  - `ext.openssl.ed25519.verify_v1`: signature verification.
  - `ext.openssl.rand.bytes_v1`: OS RNG bytes (standalone worlds only).

## Conventions to keep new packages consistent

### Error encoding (bytes “doc”)

Most fallible external APIs return a **bytes doc**:

- `OK`: `[1] + payload_bytes`
- `ERR`: `[0][err_code_u32_le][msg_len_u32_le=0]` (9 bytes total)

Modules typically expose helpers:

- `*.is_err(doc: bytes_view) -> i32`
- `*.err_code(doc: bytes_view) -> i32`
- `*.get_bytes(doc: bytes_view) -> bytes` (for ok payload)

### Resource bounding (determinism + safety)

Every parser/decoder that can expand data must take an explicit cap:

- `max_body_bytes` (HTTP)
- `max_size` / `max_out_bytes` (inflate/gzip, tar extract)
- `max_out_bytes` (protobuf wire decode output)

## Production-release checklist (status)

### 1) solve-* compression (big missing piece)

Covered by `packages/ext/x07-ext-compress-rs/0.1.0/` (`ext.compress.inflate_raw`, `ext.compress.zlib_decompress`, `ext.compress.gzip_decompress`).

### 2) Archives beyond tar “find”

Covered by `packages/ext/x07-ext-compress-rs/0.1.0/` (`ext.zip.list_names_v1`, `ext.zip.extract_file_v1`).

### 3) Crypto breadth for real tooling

Covered by:

- `packages/ext/x07-ext-crypto-rs/0.1.0/` (`sha512`, `hkdf_sha256_*`).
- `packages/ext/x07-ext-openssl-c/0.1.0/` (`ed25519.verify_v1`, `rand.bytes_v1`) (standalone worlds only).

### 4) HTTP client ergonomics + sandbox policy

Covered by `packages/ext/x07-ext-curl-c/0.1.0/`:

- response headers capture (`request_v2`, `resp_header_count`, `resp_header_get_v2`)
- redirect knobs (`REQ_FLAG_FOLLOWLOCATION`, denied under `run-os-sandboxed`)
- streaming-to-file (`req_get_to_file_v3`, `req_post_to_file_v3`)
- hardened `run-os-sandboxed` checks aligned with runner policy env (`X07_OS_*`)

Still missing if needed: a true `iface` streaming reader (vs streaming to a file).

### 5) Time/date completeness

Covered by `packages/ext/x07-ext-time-rs/0.1.0/`:

- `ext.time.rfc3339.parse_v2` / `ext.time.rfc3339.format_v2`: i64 unix seconds (low+high halves)
- `ext.time.duration`: duration docs + arithmetic

Still missing: (run-os) monotonic time + wall clock access via world adapters (world-gated).

### 6) Protobuf: encode + schema-driven decode (v2)

Covered by `packages/ext/x07-ext-pb-rs/0.1.0/`:

- `ext.pb.wire.encode_v1` and `ext.pb.wire.descriptor_set_to_schema_v1`
- `ext.pb.data_model.decode_descriptor_set_v2` (schema-driven decode into `ext.data_model`)
- 64-bit varint status helpers (`varint_u64_status_v1`)

## Testing note (current limitation)

`x07 test` accepts a single `--module-root`, so tests that need multiple external packages at once (e.g., `ext.pb.data_model` importing `ext.data_model`) require either:

- a project-based build (`x07c build --project ...`), or
- a merged module root in a dedicated test harness.
