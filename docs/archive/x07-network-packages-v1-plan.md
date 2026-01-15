# External Networking Packages v1 — Development Plan (`x07:ext-net@0.1.0`)

This document is a **concrete, repo‑actionable plan** to deliver an agent‑facing networking surface as **external packages** (no X07 language/kernel changes unless explicitly required later).

Primary user-facing goal: **one canonical way** for agents to do networking in OS worlds via `std.net.*`.

---

## Status (in-tree)

Implemented and covered by the repo’s OS-world smoke gate:

- Package: `packages/ext/x07-ext-net/0.1.0/`
  - Modules: `std.net.err`, `std.net.codec`, `std.net.dns`, `std.net.tcp`, `std.net.udp`, `std.net.tls`, `std.net.io`, `std.net.http`, `std.net.http.spec`, `std.net.http.client`, `std.net.http.server`
- Backend package: `packages/ext/x07-ext-sockets-c/0.1.0/`
  - Modules: `ext.sockets._ffi`, `ext.sockets.net`
- Smoke program: `tests/external_os/net/src/main.x07.json`
- Smoke program: `tests/external_os/net_sockets/src/main.x07.json`
- Smoke program: `tests/external_os/net_sockets_policy_denied/src/main.x07.json`
- Smoke program: `tests/external_os/net_iface_stream/src/main.x07.json`
- Smoke program: `tests/external_os/net_tls/src/main.x07.json`
- Smoke program: `tests/external_os/net_http_server/src/main.x07.json`
- Sandboxed policy fixture: `tests/external_os/net/run-os-policy.file-etc-allow-ffi.json`
- Sandboxed policy fixture: `tests/external_os/net_sockets/run-os-policy.loopback-allow.json`
- CI gate: `./scripts/ci/check_external_packages_os_smoke.sh`
- Error codes doc: `docs/net/errors-v1.md`
- Sockets bytes ABIs doc: `docs/net/net-v1.md`

## Production readiness status (v1)

Resolved in-tree:

- `iface` streaming for TCP/UDP: `std.net.tcp.stream_reader_v1` and `std.net.udp.recv_doc_reader_v1` integrate with `std.io` / `std.io.bufread` (requires minimal runtime support for external `iface` reader vtables).
- Sandboxed DNS hardening: DNS-tag connect requires both `(name, port)` allowlisting and resolved-IP allowlisting (DNS rebinding mitigation); `allow_hosts.host` now supports exact IP, CIDR (`ip/prefix`), and IP range (`ip1-ip2`) patterns; schema updated.
- DNS connect semantics: DNS-tag TCP connect tries all `getaddrinfo` results with fallback.
- Higher-level stack: `std.net.tls` (TLS-over-raw-TCP streams) and `std.net.http.server` are implemented in `ext-net@0.1.0`.
- Cross-platform build/link: `x07-os-runner --auto-ffi` compiles `ffi/*.c` and links required libs from `x07-package.json` metadata (including Windows `ws2_32` for `ext-sockets-c` and Homebrew OpenSSL discovery on macOS).

Remaining for “production confidence”:

- Windows CI wiring is implemented via `.github/workflows/ci-windows.yml` (runs canaries + OS-world external packages smoke gate, including networking).

## Scope and invariants

- **Worlds**
  - Allowed: `run-os`, `run-os-sandboxed`
  - Forbidden: `solve-*` (no real network; determinism invariant)
- **No X07 surface changes by default**
  - Implement via external packages (`packages/ext/**`) + optional OS helper binaries (runner-side).
  - Exception: `iface` streaming for sockets required minimal runtime support for external `iface` reader vtables.
- **One canonical API per concept**
  - Agent-facing surface: `std.net.*` only.
  - Low-level/implementation packages may exist but are not “recommended surface”.
- **Memory model + placement rules**
  - Follow `docs/phases/x07-memory-model-v2.md` (views/owned bytes; no multiple competing patterns).
  - Follow `docs/dev/x07-policy.md` (OS access must be world-gated; keep TCB small).

---

## Current repo state (what we already have)

### Existing packages (relevant)

- `packages/ext/x07-ext-curl-c/0.1.0/`
  - `ext.curl.http` implements a **bounded** HTTP(S) client via libcurl.
  - Already enforces `run-os-sandboxed` policy using env vars set by `x07-os-runner` (`X07_OS_NET_*`).
  - Provides a stable, testable “implementation backend” for `std.net.http.*` without adding new builtins.
- `packages/ext/x07-ext-openssl-c/0.1.0/`
  - OS-world-only crypto (hash/rand/ed25519 verify). Not the network API, but related operationally (TLS ecosystem).
- `packages/ext/x07-ext-url-rs/0.1.0/`
  - Pure URL parsing/percent-encoding and HTTP message helpers (`ext.url.*`, `ext.http_types`, `ext.httparse`).
- `packages/ext/x07-ext-sockets-c/0.1.0/`
  - OS-world-only DNS/TCP/UDP primitives via a small C shim backend (`ext.sockets.*`).
  - Enforces `run-os-sandboxed` policy using env vars set by `x07-os-runner` (`X07_OS_NET_*`).
- `packages/ext/x07-ext-net/0.1.0/`
  - Agent-facing HTTP API: `std.net.http.*` (on top of `ext.curl.http`)
  - Agent-facing sockets API: `std.net.{codec,dns,tcp,tls,udp,io}` (on top of `ext.sockets.*`)
  - Agent-facing server helpers: `std.net.http.server` (on top of `std.net.tcp` + `ext.httparse`)
  - Pinned error codes: `std.net.err.*` (see `docs/net/errors-v1.md`)

### Existing runner policy surface

- `schemas/run-os-policy.schema.json` and `crates/x07-os-runner/src/policy.rs` define `policy.net`:
  - `enabled`, `allow_dns`, `allow_tcp`, `allow_udp`, `allow_hosts[{host, ports[]}]`
  - This is what `ext-curl-c` consumes today (via env vars).

---

## Input artifacts reviewed (phases assets)

These bundles are **design references** (not directly buildable code): they use a non‑x07AST JSON shape and contain placeholder programs / known typos (e.g. `header_table` vs `headers_table`).

We still use them to select the latest *intended* API/ABI.

### Bundles and “latest unique” selection

From `docs/phases/assets/x07_ext_net_v1_*`:

- Baseline bundle: `x07_ext_net_v1_https_bundle_hdrsort.tar.gz`
  - Contains the most complete *skeleton* set (codec + http client helpers + stream helpers).
- Spec patch: `x07_ext_net_v1_https_bundle_hdrdedupe.tar.gz`
  - Adds two key header-surface decisions:
    - `std.net.http.spec.headers_set_v1` (no-duplicates builder)
    - `std.net.http.spec.headers_canon_join_sorted_v1` (deterministic duplicate handling)

**Important:** do not copy bundle `.x07.json` directly into the repo. Instead, re-author as canonical x07AST JSON and validate via `x07 ast` (`docs/dev/x07-ast.md`).

---

## Decisions: package map (stay/remove/rename/merge)

### 1) Introduce one agent-facing package: `x07-ext-net`

Add:

- `packages/ext/x07-ext-net/0.1.0/` (new)
  - Provides `std.net.*` modules (agent-facing).
  - `determinism_tier`: `os-world-only`
  - `worlds_allowed`: `run-os`, `run-os-sandboxed`

### 2) Keep existing low-level packages for now (no duplication)

- Keep `x07-ext-curl-c` as the initial HTTP(S) backend for `std.net.http.client`.
  - Do **not** duplicate `curl_shim.c` in a second place.
- Keep `x07-ext-openssl-c` unchanged (separate concern: OS-world crypto).
- Keep `x07-ext-url-rs` unchanged (pure; useful in `solve-*` too).

### 3) Deprecation direction (after `ext-net` lands)

- Once `x07-ext-net` is usable and tested, update docs to recommend:
  - **Use**: `std.net.http.*`
  - **Avoid (low-level)**: `ext.curl.http.*` unless doing backend work.

No package deletions are part of this plan by default; removal/renames should be a separate explicit decision (breaks callers).

---

## Target v1 surface (what agents will use)

### HTTP (must-have v1)

- `std.net.http.spec`
  - canonical `HeadersTableV1` helpers (build + validate + canonicalize)
  - canonical `HttpReqV1` builders + getters
  - canonical `HttpCapsV1` helpers (curl-backed)
  - deterministic duplicate header handling (`headers_set_v1`, `headers_canon_join_sorted_v1`)
- `std.net.http.client`
  - `fetch_v1(req_doc: bytes) -> bytes` (resp_doc)
  - `get_v1(url: bytes, caps: bytes) -> bytes` (resp_doc) convenience
  - `post_v1(url: bytes, body: bytes, caps: bytes) -> bytes` convenience
  - `fetch_to_file_v1(req_doc: bytes, out_rel_path: bytes_view) -> bytes` (resp_doc; file-backed)
  - `get_to_file_v1(url: bytes, out_rel_path: bytes_view, caps: bytes) -> bytes` (resp_doc; file-backed)
  - `post_to_file_v1(url: bytes, body: bytes, out_rel_path: bytes_view, caps: bytes) -> bytes` (resp_doc; file-backed)
  - file-backed response helpers:
    - `resp_file_path_v1(resp_doc) -> bytes`
    - `resp_file_len_v1(resp_doc) -> i32`
    - `resp_file_reader_v1(resp_doc) -> iface`

### Shared error codes (must-have v1)

- `std.net.err`
  - `code_policy_denied_v1`, `code_invalid_req_v1`, `code_timeout_v1`, `code_too_large_v1`,
    `code_dns_v1`, `code_connect_v1`, `code_tls_v1`, `code_internal_v1`
- Normative doc: `docs/net/errors-v1.md`

### TCP/UDP/DNS (v1)

- `std.net.codec`
  - `NetAddrV1` and `NetCapsV1` builders + getters (agent-friendly; no offset guessing)
- `std.net.dns`
  - `lookup_v1(name, port, caps) -> bytes` (`DnsLookupDocV1`)
- `std.net.tcp`
  - `connect_v1(addr, caps) -> bytes` (`TcpConnectDocV1`)
  - `listen_v1(addr, caps) -> bytes` (`TcpListenDocV1`)
  - `accept_v1(listener_handle, caps) -> bytes` (`TcpAcceptDocV1`)
  - `stream_read_v1(stream_handle, max_bytes, caps) -> bytes` (`StreamReadDocV1`)
  - `stream_write_v1(stream_handle, data, caps) -> bytes` (`StreamWriteDocV1`)
  - `stream_wait_v1(stream_handle, events, timeout_ms) -> bytes` (`StreamWaitDocV1`)
- `std.net.udp`
  - `bind_v1(addr, caps) -> bytes` (`UdpBindDocV1`)
  - `sendto_v1(sock_handle, addr, payload, caps) -> bytes` (`UdpSendDocV1`)
  - `recvfrom_v1(sock_handle, max_bytes, caps) -> bytes` (`UdpRecvDocV1`)
- Normative bytes ABIs: `docs/net/net-v1.md`

## Implemented v1 bytes ABIs (pinned)

All multi-byte integers are `u32_le` encoded via `codec.{read,write}_u32_le` (represented as `i32`).

### `HeadersTableV1` (`std.net.http.spec`)

```
HeadersTableV1 :=
  count:u32_le
  repeated count times:
    name_len:u32_le
    name_bytes[name_len]          ; canonical key form uses ASCII-lowercase
    value_len:u32_le
    value_bytes[value_len]
```

Canonicalization (v1):
- `headers_set_v1(table, name, value)` lowercases `name` (ASCII), removes all existing entries for that name, and inserts exactly one entry (preserving the first occurrence position if present; otherwise appends).
- `headers_canon_join_sorted_v1(table)` lowercases names, sorts by `(name_bytes, value_bytes)`, then joins duplicate names with the literal separator `", "` (comma + space).

### `HttpCapsV1` (`std.net.http.spec`, curl-backed)

```
HttpCapsV1 :=
  ver:u32_le (=1)
  follow_location:u32_le
  timeout_s:u32_le
  max_redirects:u32_le
  max_header_bytes:u32_le
  max_headers:u32_le
  max_body_bytes:u32_le
```

- Header capture requires `max_header_bytes != 0` and `max_headers != 0` (curl shim behavior).
- In `run-os-sandboxed`, redirects are forbidden by policy (follow_location must be 0).

### `HttpReqV1` (`std.net.http.spec`)

```
HttpReqV1 :=
  ver:u32_le (=1)
  method:u32_le                  ; GET=1, POST=2
  url_len:u32_le
  url_bytes[url_len]
  headers_len:u32_le
  headers_bytes[headers_len]     ; HeadersTableV1
  body_len:u32_le
  body_bytes[body_len]
  caps_len:u32_le
  caps_bytes[caps_len]           ; HttpCapsV1 (if missing/invalid, client uses `caps_default_v1`)
```

### Response doc (`std.net.http.client.fetch_v1`)

- `fetch_v1` returns the `ext.curl.http` response doc without re-encoding; use `std.net.http.*` wrappers (`resp_is_err_v1`, `resp_status_v1`, `resp_body_v1`, …).
- For the backend doc format and error codes, see `packages/ext/x07-ext-curl-c/0.1.0/ffi/curl_shim.c`.
- Error code catalog (agent-facing): `docs/net/errors-v1.md` and `std.net.err.*`.

---

## Milestones (implementation steps + acceptance criteria)

### M1 — Create `x07-ext-net` package skeleton

Status: implemented.

Deliverables:
- `packages/ext/x07-ext-net/0.1.0/x07-package.json`
- Canonical x07AST modules created via `x07 ast init` + JSON Patch:
  - `modules/std/net/err.x07.json`
  - `modules/std/net/codec.x07.json`
  - `modules/std/net/dns.x07.json`
  - `modules/std/net/tcp.x07.json`
  - `modules/std/net/udp.x07.json`
  - `modules/std/net/io.x07.json`
  - `modules/std/net/http/spec.x07.json`
  - `modules/std/net/http/client.x07.json`
  - `modules/std/net/http.x07.json`

Acceptance:
- `cargo run -p x07 -- ast validate --in <each module>` passes.
- `python3 scripts/generate_external_packages_lock.py --check` passes (after lock update in M4).

### M2 — Define and document bytes ABIs (agent-friendly, single canonical way)

Status: implemented above in **Implemented v1 bytes ABIs (pinned)**.

Deliverables:
- A pinned ABI section inside this doc (or a new pinned doc under `docs/phases/`) describing:
  - `HeadersTableV1` binary layout + canonicalization rules
  - `HttpCapsV1` (curl-backed: timeouts, body caps, header caps, redirect policy)
  - `HttpReqV1` layout (must include caps bytes so `fetch_v1(req)` has all knobs)
  - Response doc format (curl backend response doc) + error codes

Acceptance:
- Every field has an explicit endianness and bound.
- Every “invalid bytes” case maps to a deterministic error code, documented (see `docs/net/errors-v1.md`).

### M3 — Implement `std.net.http.spec` (pure logic; no `unsafe`)

Status: implemented.

Deliverables:
- `std.net.http.spec.headers_*_v1`:
  - `headers_empty_v1`, `headers_push_v1`, `headers_set_v1`
  - `headers_canon_join_sorted_v1` (latest bundle intent)
- `std.net.http.spec.caps_*_v1`:
  - `caps_v1`, `caps_default_v1`, plus getters
- `std.net.http.spec.req_*_v1`:
  - `req_get_v1`, `req_post_v1`, plus getters (`req_url_v1`, `req_caps_v1`, …)

Tooling requirements:
- Author via x07AST workflow (`docs/dev/x07-ast.md`): `ast init` → JSON Patch → `ast apply-patch --validate` → `ast canon`.

Acceptance:
- `run-os` smoke covers:
  - header canonicalization + duplicate handling edge cases
  - request encoding roundtrips (build → getters)

### M4 — Implement `std.net.http.client` using `ext.curl.http` backend

Status: implemented.

Approach (v1 backend):
- Parse `HttpReqV1` + `HttpCapsV1` from `req_doc`.
- Translate to `ext.curl.http` request bytes + options:
  - Use `ext.curl.http.request_v1(req_bytes, max_body_bytes)` as the single execution primitive.
  - Keep translation deterministic; no ambient env usage beyond policy gates already enforced by the shim.

Acceptance:
- `run-os` smoke test validates:
  - request executes via `ext.curl.http` backend
  - capture mode response is `resp_ver_v1 == 2` and body is non-empty (using `file:///etc/hosts` to avoid real network)
- `run-os-sandboxed` smoke test validates:
  - `file:///etc/hosts` works with `net.enabled=false`
  - file-backed fetch writes under `fs.write_roots` and can be read back via `resp_file_reader_v1`

### M5 — Repo integration: docs + lock + CI

Status: implemented.

Deliverables:
- Update `docs/phases/x07-external-packages-v1.md` to list `ext-net` under OS-world-only and to recommend `std.net.http.*` for agents.
- Update `locks/external-packages.lock` via:
  - `python3 scripts/generate_external_packages_lock.py --packages-root packages/ext --out locks/external-packages.lock`
- Add/extend CI smoke:
  - extend `scripts/ci/check_external_packages_os_smoke.sh` OR add `scripts/ci/check_external_packages_net_smoke.sh`

Acceptance:
- `python3 scripts/generate_external_packages_lock.py --check` passes.
- `./scripts/ci/check_external_packages_os_smoke.sh` still passes.

### M6 — TCP/UDP/DNS primitives without new builtins (C shim backend)

Status: implemented.

Two viable backend options that do not require X07 changes:

1) **C shim backend (recommended for performance and simplicity)**  
   Add a new OS-world-only package with `ffi/*.c` that implements:
   - DNS lookup (bounded, policy-checked)
   - TCP connect/listen/accept
   - stream read/write/close with deterministic error docs
   - UDP bind/send/recv

2) **Helper process backend (uses existing `os.process.*`)**  
   Spawn a bundled helper binary and proxy data via pipes.

Acceptance (if implemented):
- All operations are policy-gated in `run-os-sandboxed`.
- No unbounded memory growth; caps are enforced.

---

## Verification command checklist (for the implementing agent)

- x07AST validity + canonicalization:
  - `cargo run -p x07 -- ast validate --in <module.x07.json>`
  - `cargo run -p x07 -- ast canon --in <module.x07.json> --out <module.x07.json>`
- External package lock:
  - `python3 scripts/generate_external_packages_lock.py --check`
- OS package smoke:
  - `./scripts/ci/check_external_packages_os_smoke.sh`
- Rust hygiene (when Rust code is added/changed for helpers):
  - `cargo fmt --check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
