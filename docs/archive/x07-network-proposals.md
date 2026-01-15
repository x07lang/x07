Below is a design + development plan for a **production‑grade external networking package** for X07 that:

* works for **100% agentic users** (LLM coding agents),
* supports **multiple complexity tiers** (simple HTTP → streaming → raw TCP/UDP),
* stays **cleanly separated** from deterministic `solve-*` worlds,
* and is **portable across Linux/macOS/Windows**.

Implementation tracking: `docs/phases/x07-network-packages-v1-plan.md`.

In-tree status (v1):

- Agent surface: `packages/ext/x07-ext-net/0.1.0/modules/std/net/**` (`std.net.*`).
- Backend: `packages/ext/x07-ext-sockets-c/0.1.0/` (`ext.sockets.*`) + existing `ext-curl-c` for HTTP(S).
- Higher-level OS-world stack: `std.net.tls` (TLS-over-raw-TCP) and `std.net.http.server`.
- `iface` streaming integration: `std.net.tcp.stream_reader_v1` and `std.net.udp.recv_doc_reader_v1` for `std.io` / `std.io.bufread`.
- Normative bytes ABIs: `docs/net/net-v1.md` and `docs/net/errors-v1.md`.
- No `os.net.*` builtins were added; everything is implemented as external packages in OS worlds.
- Standalone runner support: `x07-os-runner --auto-ffi` compiles `ffi/*.c` and links required libs from `x07-package.json` metadata.

I’ll assume your current world split still holds:

* `solve-*`: deterministic, fixture-backed, **no real network**.
* `run-os` / `run-os-sandboxed`: real OS access, **network allowed only here**.

That separation should remain a hard invariant for production safety and for avoiding “evaluation determinism leaks”.

---

## 0) The core product requirement (what “networking package” must deliver)

A networking package for agentic coding needs:

1. **Small, canonical API surface** (agents can memorize it; no “choose among 10 ways”).
2. **Binary (bytes) contracts that are fully specified** (no implicit JSON parsing in the agent’s head).
3. **Streaming** as the default when payloads can be large.
4. **Async-first ergonomics** (but still usable synchronously).
5. **Hard caps** (max body bytes, timeouts, connection limits, concurrent requests).
6. **Policy gating** in `run-os-sandboxed` (host allowlists, port allowlists, DNS controls).
7. Cross-platform event readiness (Unix `poll`, Windows `WSAPoll`/`select`) for async integration. ([Man7][1])

---

## 1) Recommended architecture: “3 layers”

### Layer 1 — **OS adapter builtins** (tiny, stable, gated to run-os*)

This is the only part that must touch `x07c` runtime/C. Everything else is external packages.

**Goal:** expose a minimal, universal **socket stream** concept + DNS.

Why: if you do *only* a helper binary (curl/reqwest daemon), you get HTTP quickly but you don’t get “real networking programs” (custom protocols, TCP servers, etc.) without a second reinvention later.

#### Proposed OS builtins (run-os* only)

All return deterministic error-doc bytes (so agents can handle failure reliably).

**DNS**

* `os.net.dns_lookup_v1(name_bytes) -> bytes`
  returns `DnsDocV1` (see encodings below)

**TCP**

* `os.net.tcp_connect_v1(addr_bytes, caps_bytes) -> i32` (stream handle)
* `os.net.tcp_listen_v1(addr_bytes, caps_bytes) -> i32` (listener handle)
* `os.net.tcp_accept_v1(listener_handle, caps_bytes) -> i32` (stream handle)

**UDP (optional in v1, but plan it)**

* `os.net.udp_bind_v1(addr_bytes, caps_bytes) -> i32` (sock handle)
* `os.net.udp_sendto_v1(sock, addr_bytes, payload_bytes, caps_bytes) -> bytes` (ok/err)
* `os.net.udp_recvfrom_v1(sock, caps_bytes) -> bytes` (packet doc)

**Stream I/O** (for TCP streams; maybe also for TLS streams later)

* `os.net.stream_read_v1(stream_handle, max_bytes_i32, caps_bytes) -> bytes`
* `os.net.stream_write_v1(stream_handle, data_bytes, caps_bytes) -> bytes`
* `os.net.stream_close_v1(stream_handle) -> i32`
* `os.net.stream_drop_v1(stream_handle) -> i32` (free table slot; idempotent)

**Async integration (critical)**

* `os.net.stream_wait_v1(stream_handle, events_i32, timeout_ms_i32) -> i32`
  events: readable=1, writable=2, hangup=4.

Implementation basis:

* On Unix: nonblocking sockets + `poll` for readiness. ([Man7][1])
* For nonblocking connect: `connect()` can return `EINPROGRESS` and you detect completion by polling for writability and checking `SO_ERROR`. ([Man7][2])
* On Windows: use Winsock equivalents and `WSAPoll` (or `select` fallback). ([Microsoft Learn][3])

This yields the simplest portable “await readiness” primitive.

---

### Layer 2 — External package `x07:ext-net@0.1.0` (agent-facing API)

This is the package end-users will import.

It should present **one canonical way** to do:

* DNS lookup
* TCP client/server
* HTTP client
* streaming read/write

and it should hide all byte encoding details behind builders + getters.

Suggested module layout:

```
packages/x07-ext-net/0.1.0/
  package.json
  modules/
    std/net/codec.x07.json        # encoders/decoders for AddrV1, CapsV1, docs
    std/net/dns.x07.json          # dns.lookup(), parse DnsDocV1
    std/net/tcp.x07.json          # tcp.connect(), tcp.listen(), tcp.accept()
    std/net/io.x07.json           # net streams implement std.io Reader/Writer iface records
    std/net/http/spec.x07.json    # HttpReqV1 builders + HttpRespV1 getters
    std/net/http/client.x07.json  # http.fetch(), http.fetch_async()
    std/net/http/server.x07.json  # later: simple server on top of tcp + parser
```

**Rule:** Agents should rarely construct raw request bytes. They call builders like:

* `std.net.http.req_get_v1(url_bytes, headers_kv_bytes, caps_bytes) -> bytes`
* `std.net.http.req_post_v1(url_bytes, headers_kv_bytes, body_bytes, caps_bytes) -> bytes`

…and they decode responses via getters, not manual byte slicing:

* `std.net.http.resp_status_v1(resp_doc) -> i32`
* `std.net.http.resp_body_v1(resp_doc) -> bytes_view` (or bytes)
* `std.net.http.resp_header_get_v1(resp_doc, key_bytes) -> option_bytes`

This is how you keep the API “small enough to memorize”.

---

### Layer 3 — Optional “batteries” helpers (HTTP/TLS daemon), but keep the API stable

You can ship a helper binary for faster time-to-value:

* `deps/x07/x07-http-fetch` (Rust `reqwest` + `rustls`) for HTTPS client.
  Reqwest is explicitly an async HTTP client and is widely used; Rustls is a Rust TLS library focused on strong defaults. ([Docs.rs][4])

But: **don’t make the agent-facing API depend on whether it’s daemon-based or socket-based**.

Make `std.net.http.client.fetch_v1()` select an implementation:

* if TLS is requested: use helper fetcher initially
* if plain http://: can use native tcp+http1 parser

Over time you can replace the helper with native implementation without breaking user code.

For HTTP semantics / URL parsing correctness, pin to RFCs:

* URI syntax: RFC 3986 ([IETF Datatracker][5])
* HTTP semantics: RFC 9110 ([RFC Editor][6])

---

## 2) “Different complexity levels” roadmap (what users can build)

### Level A — Simple client scripts (fetch JSON, post forms)

Minimum viable “agent utility networking”:

* `http.fetch_v1(req) -> resp_doc`
* body cap + timeout
* deterministic error codes
* minimal headers support

### Level B — Concurrent clients (fan-out/fan-in)

Use X07’s async + cooperative scheduler:

* `http.fetch_async_v1(req) -> task_handle`
* `await` to gather results
* bounded concurrency helper: `http.pool_map_v1(reqs, max_in_flight)`

### Level C — Streaming (download large files, parse streaming JSON)

Requires:

* `http.open_v1(req) -> iface reader` (body stream)
* integrate with `std.io.bufread` to parse incrementally
* avoid copies (use views heavily)

### Level D — Custom protocols (raw TCP)

Expose `tcp.connect_stream_v1()` returning an `iface`:

* agents can write their own binary protocol parsers using views and `bufread`

### Level E — Servers (listen/accept)

Add:

* `tcp.listen_v1(addr)`
* `tcp.accept_v1(listener)`
* `http.server_v1` built atop tcp for simple HTTP servers (optional)

---

## 3) Bytes encodings (spec-first, agent-friendly)

You’ve already standardized “specbin-style” thinking for other packages. Do the same here.

### 3.1 Address encoding: `NetAddrV1`

One canonical encoding for DNS name + port and IP + port.

```
NetAddrV1:
  u8  tag
    1 = ipv4
    2 = ipv6
    3 = dns_name
  u16 port_be
  payload:
    tag=1: 4 bytes ipv4
    tag=2: 16 bytes ipv6
    tag=3: u16 name_len_be + name_bytes (utf8 or ascii)
```

Why `port_be`: conventional network order.

### 3.2 Caps encoding: `NetCapsV1`

Keep caps explicit and separate from policy:

```
NetCapsV1:
  u32 timeout_ms_le
  u32 max_read_bytes_le
  u32 max_write_bytes_le
  u32 max_total_bytes_le
  u32 flags_le   (bit0 = allow_ipv6, bit1 = allow_dns, bit2 = allow_tls, ...)
```

**Policy can further restrict** regardless of caps.

### 3.3 Stream I/O docs

Return docs instead of “magic i32” codes whenever it helps agents.

`WriteDocV1`:

```
u8 tag (1 ok, 0 err)
if ok:
  u32 bytes_written_le
else:
  u32 err_code_le
```

`ReadDocV1`:

```
u8 tag (1 ok, 0 err)
if ok:
  bytes payload
else:
  u32 err_code_le
```

### 3.4 DNS doc: `DnsDocV1`

```
u8 tag (1 ok, 0 err)
if ok:
  u16 count_be
  repeated NetAddrV1 (ip variants only, no dns_name)
else:
  u32 err_code_le
```

### 3.5 HTTP encoding (v1, small but complete)

Keep HTTP request encoding explicit; don’t ask agents to build textual HTTP.

`HttpReqV1`:

```
u8  version = 1
u8  method (1 GET, 2 POST, 3 PUT, 4 DELETE, 5 PATCH, 6 HEAD)
u8  flags  (bit0 follow_redirects, bit1 is_tls_required, ...)
u32 timeout_ms_le
u32 max_body_bytes_le
u16 url_len_be
url_bytes (UTF-8)
u16 hdr_count_be
repeat hdr:
  u16 k_len_be, k_bytes
  u16 v_len_be, v_bytes
u32 body_len_le
body_bytes
```

`HttpRespDocV1`:

```
u8 tag (1 ok, 0 err)
if ok:
  u16 status_be
  u16 hdr_count_be
  repeated (k_len_be, k_bytes, v_len_be, v_bytes)
  u32 body_len_le
  body_bytes
else:
  u32 err_code_le
  u16 msg_len_be
  msg_bytes (utf8, optional, bounded)
```

That’s enough for many agents without needing HTTP text knowledge, while still aligning with HTTP semantics. ([RFC Editor][6])

---

## 4) `run-os-sandboxed` policy additions (network)

Add a new `network` section to your policy schema (parallel to process policy).

Policy should allow:

* turning net on/off
* restricting outbound destinations
* restricting listen (server)
* restricting DNS
* restricting TLS (optional)

### Proposed policy shape (high level)

* `network.enabled: bool`
* `network.allow_dns: bool`
* `network.allow_outbound: bool`
* `network.allow_listen: bool`
* `network.allow_addrs: [ { kind, host_or_cidr, port_min, port_max } ]`
* `network.deny_addrs: [...]` (optional, evaluated before allow)
* `network.max_live_sockets: u32`
* `network.max_connects: u32`
* `network.max_total_bytes: u64` (or split per socket)
* `network.max_body_bytes_http: u32` (for helper-based http)

**Important:** do not rely on DNS name allowlists alone; allow IP CIDRs too. DNS resolution can return multiple IPs.

---

## 5) Cross-platform implementation plan (PR-sized milestones)

Below is a concrete, incremental plan that gets value early.

### NET‑01 — Spec + encodings (no code yet)

**Adds**

* `docs/net/net-v1.md` (normative)

  * NetAddrV1, NetCapsV1, Read/Write docs, DNS docs, HTTP req/resp docs
* `spec/x07net.schema.json` (optional; validates “specrows” if you use it)

**Gate**

* `scripts/check_contracts.py` validates the schema and a handful of example blobs.

### NET‑02 — Policy schema + runner wiring (no sockets yet)

**Adds**

* `schemas/run-os-policy.schema.json` additions under `network`
* `crates/x07-os-runner/src/policy.rs` parse/validate defaults
* Export env vars like:

  * `X07_OS_NET_ENABLED=0/1`
  * `X07_OS_NET_MAX_LIVE=...`
  * `X07_OS_NET_ALLOW_JSON=...` (or path to canonical JSON)

**Gate**

* `benchmarks/run-os-sandboxed/net-policy-smoke.json`:

  * deny-all by default
  * allow loopback only
  * invalid policy rejected deterministically

### NET‑03 — Minimal TCP client I/O builtins (Unix only first)

**Adds (x07c runtime)**

* socket table similar to process table
* `os.net.tcp_connect_v1`
* `os.net.stream_read_v1`
* `os.net.stream_write_v1`
* `os.net.stream_close_v1`
* `os.net.stream_drop_v1`

Unix implementation uses:

* nonblocking connect (handle EINPROGRESS) ([Man7][2])
* `poll` for readiness ([Man7][1])
* `recv`/`send` semantics ([The Open Group][7])

**Gate**

* `benchmarks/run-os/tcp-echo-smoke.json` (local loopback server helper)
* ensure timeouts work

### NET‑04 — Windows backend parity

**Adds**

* winsock init/shutdown
* use `WSAPoll` (or select) to implement `stream_wait_v1` ([Microsoft Learn][3])
* same error codes

**Gate**

* same smoke suite passes on Windows

### NET‑05 — External package `x07:ext-net@0.1.0` (TCP + DNS)

**Adds**

* `packages/x07-ext-net/0.1.0/...` modules:

  * `std.net.tcp.connect_v1(addr, caps) -> iface` (reader/writer iface)
  * `std.net.dns.lookup_v1(name, port, caps) -> result_bytes` (DnsDocV1)
  * `std.net.codec` encoders/decoders

**Gate**

* pure tests for codec enc/dec (no OS)
* run-os tests for connect+read+write

### NET‑06 — HTTP client v1 (fast unlock)

Pick one of these two implementation tracks:

**Track 6A (fastest): helper-per-request**

* Ship `deps/x07/x07-http-fetch` (Rust `reqwest` + `rustls`) ([Docs.rs][4])
* `std.net.http.fetch_v1(req_bytes) -> resp_doc_bytes` spawns helper using your process API

Pros: quickest HTTPS support.
Cons: higher overhead, no streaming, one process per request.

**Track 6B (native): HTTP/1.1 over tcp (no TLS yet)**

* implement HTTP/1.1 request serialization + response parse in X07
* aligns with RFC 9110 semantics ([RFC Editor][6])

Pros: no helper.
Cons: no TLS initially.

**My recommendation:** start with **6A**, but keep the encoding/API stable so you can add 6B later for plain HTTP and eventually TLS.

**Gate**

* `benchmarks/run-os/http-local-smoke.json` against a local stub server helper
* `benchmarks/run-os-sandboxed/http-policy-smoke.json` with allowlist host/port

### NET‑07 — Streaming HTTP (upgrade)

Once you have either:

* direct sockets in runtime, or
* streaming subprocess I/O,

add:

* `http.open_v1(req) -> iface` (body reader)
* `http.read_chunk_v1(reader, max) -> bytes_view/bytes`

**Gate**

* download 10MB from local stub server; mem assertions (avoid memcpy blowups)

### NET‑08 — Server capabilities (listen/accept)

Add:

* `tcp.listen_v1`, `tcp.accept_v1`
* minimal http server helper in package

**Gate**

* start server on loopback; client hits it; verify deterministic response

---

## 6) How this enables “different complexity programs”

With the above, agents can build:

* **Simple API clients** (HTTP GET/POST)
* **Parallel clients** (spawn multiple fetches with defasync and join)
* **Streaming parsers** (download large payloads and parse incrementally using views/bufread)
* **Custom protocols** (raw TCP)
* **Local microservices** (listen/accept + minimal HTTP server)
* **Cross-platform** (same X07 code; adapter changes in OS layer)

---

## 7) Key design choices to keep it agentic

### A) “One canonical return type”: always return a doc blob

Agents struggle when functions sometimes return sentinel codes and sometimes bytes.

Make everything return either:

* `bytes` doc with `tag=ok/err`, or
* `result_bytes` with canonical encodings

### B) Keep parsing out of user code: provide getters/builders

Agents should never need to memorize offsets. Provide:

* `*_build_v1(...) -> bytes`
* `*_get_*_v1(doc) -> ...`

### C) Make error codes stable and enumerable

Provide `docs/net/errors-v1.md` and `std.net.err.*` constants.

### D) Local stub servers for tests (never hit the real internet)

All “benchmarks/tests” must run on loopback with repo-shipped stubs to be stable across CI.

---

## 8) What I need from you (only if you want to lock decisions)

---

[1]: https://man7.org/linux/man-pages/man2/poll.2.html?utm_source=chatgpt.com "poll(2) - Linux manual page"
[2]: https://man7.org/linux/man-pages/man2/connect.2.html?utm_source=chatgpt.com "connect(2) - Linux manual page"
[3]: https://learn.microsoft.com/en-us/windows/win32/api/winsock/nf-winsock-recv?utm_source=chatgpt.com "recv function (winsock.h) - Win32 apps"
[4]: https://docs.rs/reqwest/latest/reqwest/struct.Client.html?utm_source=chatgpt.com "Client in reqwest - Rust"
[5]: https://datatracker.ietf.org/doc/html/rfc3986?utm_source=chatgpt.com "RFC 3986 - Uniform Resource Identifier (URI): Generic ..."
[6]: https://www.rfc-editor.org/rfc/rfc9110.html?utm_source=chatgpt.com "RFC 9110: HTTP Semantics"
[7]: https://pubs.opengroup.org/onlinepubs/007904975/functions/recv.html?utm_source=chatgpt.com "recv"
++++
Use docs/phases/assets/x07_ext_net_v1_bundle.tar.gz

This tarball is **drop-in / ready-to-paste** and contains all four deliverables you asked for, plus the tiny helper binary skeleton needed by the smoke tests.

## What’s inside (repo-aligned)

### 1) Normative spec doc

* `docs/net/net-v1.md`

This is a draft-but-normative spec for **OS-world networking only** (hard-forbidden in `solve-*`). It defines:

* **Bytes contracts** (`NetAddrV1`, `NetCapsV1`, `ReadDocV1`, `WriteDocV1`, DNS list doc, HTTP req/resp docs)
* **Stable error codes**
* The **required determinism split** (compile-time hard errors for `solve-*`)
* Cross-platform portability guidance (POSIX readiness waiting via `poll()` and Windows via `WSAPoll()`), and notes on URL/HTTP semantics alignment (informational refs). ([IETF Datatracker][1])

### 2) `run-os-policy` network section (schema fragment)

* `schemas/run-os-policy.network.section.json`
* `schemas/README_network_policy.md`

This is the **network capability section** to merge into your existing `schemas/run-os-policy.schema.json`. It includes:

* `network.enabled`
* `allow_dns`, `allow_outbound`, `allow_listen`
* `allow_rules[]` + `deny_rules[]` with rule kinds (`cidr`, `ip`, `dns_exact`, `dns_suffix`)
* hard caps like `max_live_sockets`, `max_connects`, `max_total_bytes`, default timeouts

The README explains the **semantic rules** your validator must enforce beyond JSON Schema (deny-by-default, deny overrides allow, empty allow list = deny all, etc.).

### 3) External package skeleton

* `packages/x07-ext-net/0.1.0/package.json`
* `packages/x07-ext-net/0.1.0/modules/std/net/codec.x07.json`
* `packages/x07-ext-net/0.1.0/modules/std/net/dns.x07.json`
* `packages/x07-ext-net/0.1.0/modules/std/net/tcp.x07.json`
* `packages/x07-ext-net/0.1.0/modules/std/net/http/spec.x07.json`
* `packages/x07-ext-net/0.1.0/modules/std/net/http/client.x07.json`

Notes:

* `std.net.codec.*` is **concrete** (builds `NetAddrV1` + `NetCapsV1` deterministically).
* `std.net.dns`, `std.net.tcp`, `std.net.http.client` are **thin facades** over the planned OS builtins (`os.net.*`), consistent with the “stdlib written against traits / adapters bound per world” model.
* The HTTP builder `std.net.http.spec.req_get_v1(url, caps)` now produces a correctly-sized `HttpReqV1` with `hdr_count=0`, `body_len=0`, and appended `NetCapsV1`.

### 4) Cross-platform smoke suite JSON shapes (Linux/macOS/Windows)

* `benchmarks/run-os/net-smoke.json`
* `benchmarks/run-os-sandboxed/net-policy-smoke.json`

These are **copy/paste-ready JSON shapes** with:

* per-platform cases using `only_platforms: ["linux"|"macos"|"windows"]`
* helper binary path differences (`.exe` on Windows)
* sandboxed deny-by-default and allow-loopback examples

They assume your runner supports:

* `argv_utf8`
* `env` injection
* `policy_inline` for sandboxed runs
* `expect.exit_code` and `expect.stdout_utf8`

If your current runner uses slightly different keys, treat these files as the **canonical intent** and rename fields to match your existing harness.

### Included extra: tiny stub server helper binary skeleton

Because your smoke tests explicitly requested a helper binary, the bundle also includes:

* `crates/x07-net-stub-server/` (Rust crate)
* `scripts/build_os_helpers.sh` (build + copy to `deps/x07/`)

The stub server supports:

* `--mode tcp-echo` (default) or `--mode http`
* binds to the first free port in `--port-base..--port-max`
* optionally writes a chosen port to `--port-file`

This keeps CI reliable and avoids hitting public internet.

## API names and bytes ABI (as shipped in docs/net/net-v1.md)

### OS builtins (run-os* only; MUST hard-error in solve-*)

* `os.net.tcp_connect_v1(addr: bytes, caps: bytes) -> bytes`
* `os.net.tcp_listen_v1(addr: bytes, caps: bytes) -> bytes`
* `os.net.tcp_accept_v1(listener_handle: i32, caps: bytes) -> bytes`
* `os.net.stream_read_v1(stream_handle: i32, max_bytes: i32, caps: bytes) -> bytes`
* `os.net.stream_write_v1(stream_handle: i32, data: bytes, caps: bytes) -> bytes`
* `os.net.stream_shutdown_v1(stream_handle: i32, how: i32) -> i32`
* `os.net.stream_close_v1(stream_handle: i32) -> i32`
* `os.net.stream_wait_v1(stream_handle: i32, events: i32, timeout_ms: i32) -> i32`
* `os.net.dns_lookup_v1(name: bytes, port: i32, caps: bytes) -> bytes`
* optional: `os.net.http_fetch_v1(req: bytes) -> bytes`

Readiness waiting is explicitly modeled after POSIX `poll()` and Windows `WSAPoll()` behavior (bitmask events). ([Man7][2])

### Key bytes contracts

* `NetAddrV1` (tag + port_u32_le + payload)
* `NetCapsV1` (6×u32_le, fixed 24 bytes)
* `ErrorDocV1` (tag 0 + code + utf8 detail)
* `ReadDocV1` / `WriteDocV1`
* DNS list doc
* HTTP req/resp docs (RFC 3986/9110 alignment is informational; X07 defines its own internal ABI). ([IETF Datatracker][1])

## Small but important design choices (why this is LLM/agent-friendly)

* **All integers are u32_le** in internal net blobs (including port). This is intentionally “wasteful but simple” so agents don’t have to juggle u16/u64 edge cases.
* **No ambient side channels**: errors are always explicit `ErrorDocV1`, not “print and hope”.
* **Hard world gating**: prevents “accidental nondeterminism creep” into deterministic suites.
* **Policy-first**: `run-os-sandboxed` denies networking unless explicitly enabled and allowlisted.

## Next dev steps (what your engineers implement next)

1. Implement the `os.net.*` builtins in the OS runner runtime:

   * POSIX: `socket/connect/listen/accept`, nonblocking + readiness via `poll()`; connect-timeout via nonblocking connect + `poll()` and `getsockopt(SO_ERROR)` semantics. ([Man7][3])
   * Windows: Winsock, readiness via `WSAPoll()` (or `select()` if preferred). ([Microsoft Learn][4])

2. Enforce caps + policy:

   * policy deny ⇒ `EV_NET_ERR_POLICY_DENIED` (code `1`)
   * bytes caps must bound read/write/total; timeouts enforced even if program requests “infinite”.

3. Add the smoke test programs at the `program_path` locations referenced by the suites, using:

   * `std.os.process` to spawn `deps/x07/x07-net-stub-server[.exe]`
   * `std.net.tcp.*` wrappers to do the echo roundtrip
   * sandboxed suite to validate allow/deny

If you want, I can also generate the **two tiny smoke programs** (`tests/external_os/net_tcp_echo_roundtrip/src/main.x07.json` and `tests/external_os/net_policy_denies_by_default/src/main.x07.json`) in the same minimal x07AST JSON style you’re using now—so the suites become runnable immediately after you land the OS builtins.

[1]: https://datatracker.ietf.org/doc/html/rfc3986 "
            
                RFC 3986 - Uniform Resource Identifier (URI): Generic Syntax
            
        "
[2]: https://man7.org/linux/man-pages/man2/poll.2.html "poll(2) - Linux manual page"
[3]: https://man7.org/linux/man-pages/man2/connect.2.html "connect(2) - Linux manual page"
[4]: https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsapoll "WSAPoll function (winsock2.h) - Win32 apps | Microsoft Learn"
+++

Got it — with those answers, **Net v1 becomes “TCP servers + HTTPS-capable HTTP client + DNS allowlists (DNS + CIDR)”**.

### What changes vs the earlier draft bundle

* **Servers in v1:** keep `tcp_listen/accept` as first-class OS builtins + `std.net.tcp.*` wrappers.
* **HTTPS required in v1:** `os.net.http_fetch_v1` MUST support `https://...` and verify certs by default.

  * Recommended implementation is **libcurl** (portable HTTP(S) stack) which supports HTTPS by using a TLS backend (OpenSSL / Schannel / Secure Transport / rustls depending on the build).
* **Sandbox allowlists by DNS name AND CIDR:** the **normative** rule in the spec is:

  * for DNS connects, require **(name allow)** *and* **(resolved IP allow)** to mitigate DNS-rebind-like issues.

### Drop‑in bundle (updated for your answers)

It includes:

* `docs/net/net-v1.md` (normative, includes HTTPS requirement + policy semantics)
* `schemas/run-os-policy.network.section.schema.json` (DNS + CIDR + TLS allow_insecure toggle)
* `packages/x07-ext-net/0.1.0/...` (external package skeleton, stub modules)
* `crates/x07-net-stub-server/` (tiny helper server; supports `tcp-echo`, `http`, **`https`** using rustls)
* `benchmarks/run-os/net-smoke.json` + `benchmarks/run-os-sandboxed/net-policy-smoke.json` (copy/paste suite JSON shapes)

Use docs/phases/assets/x07_ext_net_v1_https_bundle.tar.gz

### Implementation notes the bundle “locks in”

* **TLS verify default ON**, with an escape hatch:

  * `NetCapsV1.flags` bit `NETCAP_TLS_INSECURE_OK`
  * only honored in `run-os-sandboxed` if `policy.network.tls.allow_insecure == true`
* **Windows readiness/polling:** if you implement nonblocking accept/read waits, Windows can use `WSAPoll` (available on Vista+). ([Microsoft Learn][1])
* **If you pick OpenSSL as a TLS backend:** OpenSSL 3.0+ is Apache-2.0 licensed, which can simplify licensing compared to older OpenSSL-era constraints.

[1]: https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsapoll "WSAPoll function (winsock2.h) - Win32 apps | Microsoft Learn"
++++
I replaced the `TODO` stubs with fully working **x07AST JSON** implementations for:

* **NetAddrV1** pack + unpack helpers (`std.net.codec.*`)
* **NetCapsV1** pack + unpack helpers (`std.net.codec.*`)
* **HttpReqV1** pack + unpack helpers (`std.net.http.spec.*`)
* plus tiny **Http header-table** builders so agents never hand-roll the binary header encoding.

Use docs/phases/assets/x07_ext_net_v1_https_bundle_impl.tar.gz.
## Encodings implemented

These are the exact encodings the helpers implement (and what your OS builtins should parse/emit):

### NetAddrV1 (bytes)

`tag_u8` + `port_u32_le` + payload

* `tag=1` IPv4 payload: 4 bytes `a,b,c,d`
* `tag=2` IPv6 payload: 16 bytes
* `tag=3` DNS payload: `name_len_u32_le` + `name_bytes`

### NetCapsV1 (bytes, fixed 24 bytes)

6× `u32_le`:

1. `connect_timeout_ms`
2. `io_timeout_ms`
3. `max_in_flight`
4. `max_body_bytes`
5. `tls_flags` (v1 uses `0` or `1`; `1` means insecure skip-verify)
6. `reserved` (0)

TLS “verify by default” is the expected default; an explicit skip-verify flag is intentionally opt-in. ([Curl][1])

### HttpReqV1 (bytes)

* `version_u8` (=1)
* `method_u8` (GET=1, POST=2, PUT=3, DELETE=4)
* `reserved_u16_le` (=0)
* `url_len_u32_le` + `url_bytes`
* `header_table_bytes` (starts with `header_count_u32_le`, then repeated `(k_len, k_bytes, v_len, v_bytes)`)
* `body_len_u32_le` + `body_bytes`
* `caps_len_u32_le` + `caps_bytes` (typically 24)

HTTP methods are standard; the “method code” is just your internal representation for those methods. ([RFC Editor][2])

---

## File 1: `packages/x07-ext-net/0.1.0/modules/std/net/codec.x07.json`

```json
[["export","std.net.codec.addr_ipv4_v1","std.net.codec.addr_ipv6_v1","std.net.codec.addr_dns_v1","std.net.codec.addr_tag_v1","std.net.codec.addr_port_v1","std.net.codec.addr_ipv4_bytes_v1","std.net.codec.addr_ipv6_bytes_v1","std.net.codec.addr_dns_name_v1","std.net.codec.caps_v1","std.net.codec.caps_connect_timeout_ms_v1","std.net.codec.caps_io_timeout_ms_v1","std.net.codec.caps_max_in_flight_v1","std.net.codec.caps_max_body_bytes_v1","std.net.codec.caps_tls_flags_v1","std.net.codec.caps_tls_insecure_skip_verify_v1"],["import","std.u32"],["defn","std.net.codec.addr_ipv4_v1",[["a","i32"],["b","i32"],["c","i32"],["d","i32"],["port","i32"]],"bytes",["begin",["let","out",["vec_u8.with_capacity",9]],["vec_u8.push","out",1],["set","out",["std.u32.push_le","out","port"]],["vec_u8.push","out","a"],["vec_u8.push","out","b"],["vec_u8.push","out","c"],["vec_u8.push","out","d"],["vec_u8.as_bytes","out"]]],["defn","std.net.codec.addr_ipv6_v1",[["ip16","bytes"],["port","i32"]],"bytes",["if",["=",["bytes.len","ip16"],16],["begin",["let","out",["vec_u8.with_capacity",21]],["vec_u8.push","out",2],["set","out",["std.u32.push_le","out","port"]],["set","out",["vec_u8.extend_bytes","out","ip16"]],["vec_u8.as_bytes","out"]],["bytes.alloc",0]]],["defn","std.net.codec.addr_dns_v1",[["name","bytes"],["port","i32"]],"bytes",["begin",["let","n",["bytes.len","name"]],["let","out",["vec_u8.with_capacity",["+",9,"n"]]],["vec_u8.push","out",3],["set","out",["std.u32.push_le","out","port"]],["set","out",["std.u32.push_le","out","n"]],["set","out",["vec_u8.extend_bytes","out","name"]],["vec_u8.as_bytes","out"]]],["defn","std.net.codec.addr_tag_v1",[["addr","bytes"]],"i32",["if",["<u",["bytes.len","addr"],1],0,["bytes.get_u8","addr",0]]],["defn","std.net.codec.addr_port_v1",[["addr","bytes"]],"i32",["if",["<u",["bytes.len","addr"],5],0,["std.u32.read_le_at","addr",1]]],["defn","std.net.codec.addr_ipv4_bytes_v1",[["addr","bytes"]],"bytes",["if",["=",["std.net.codec.addr_tag_v1","addr"],1],["if",["<u",["bytes.len","addr"],9],["bytes.alloc",0],["bytes.slice","addr",5,4]],["bytes.alloc",0]]],["defn","std.net.codec.addr_ipv6_bytes_v1",[["addr","bytes"]],"bytes",["if",["=",["std.net.codec.addr_tag_v1","addr"],2],["if",["<u",["bytes.len","addr"],21],["bytes.alloc",0],["bytes.slice","addr",5,16]],["bytes.alloc",0]]],["defn","std.net.codec.addr_dns_name_v1",[["addr","bytes"]],"bytes",["begin",["if",["!=",["std.net.codec.addr_tag_v1","addr"],3],["return",["bytes.alloc",0]],0],["if",["<u",["bytes.len","addr"],9],["return",["bytes.alloc",0]],0],["let","n",["std.u32.read_le_at","addr",5]],["if",["<u",["bytes.len","addr"],["+",9,"n"]],["bytes.alloc",0],["bytes.slice","addr",9,"n"]]]],["defn","std.net.codec.caps_v1",[["connect_timeout_ms","i32"],["io_timeout_ms","i32"],["max_in_flight","i32"],["max_body_bytes","i32"],["tls_insecure_skip_verify","i32"]],"bytes",["begin",["let","out",["bytes.alloc",24]],["set","out",["std.u32.write_le_at","out",0,"connect_timeout_ms"]],["set","out",["std.u32.write_le_at","out",4,"io_timeout_ms"]],["set","out",["std.u32.write_le_at","out",8,"max_in_flight"]],["set","out",["std.u32.write_le_at","out",12,"max_body_bytes"]],["let","flags",["if","tls_insecure_skip_verify",1,0]],["set","out",["std.u32.write_le_at","out",16,"flags"]],["set","out",["std.u32.write_le_at","out",20,0]],"out"]],["defn","std.net.codec.caps_connect_timeout_ms_v1",[["caps","bytes"]],"i32",["if",["<u",["bytes.len","caps"],4],0,["std.u32.read_le_at","caps",0]]],["defn","std.net.codec.caps_io_timeout_ms_v1",[["caps","bytes"]],"i32",["if",["<u",["bytes.len","caps"],8],0,["std.u32.read_le_at","caps",4]]],["defn","std.net.codec.caps_max_in_flight_v1",[["caps","bytes"]],"i32",["if",["<u",["bytes.len","caps"],12],0,["std.u32.read_le_at","caps",8]]],["defn","std.net.codec.caps_max_body_bytes_v1",[["caps","bytes"]],"i32",["if",["<u",["bytes.len","caps"],16],0,["std.u32.read_le_at","caps",12]]],["defn","std.net.codec.caps_tls_flags_v1",[["caps","bytes"]],"i32",["if",["<u",["bytes.len","caps"],20],0,["std.u32.read_le_at","caps",16]]],["defn","std.net.codec.caps_tls_insecure_skip_verify_v1",[["caps","bytes"]],"i32",["if",["=",["std.net.codec.caps_tls_flags_v1","caps"],0],0,1]]]
```

---

## File 2: `packages/x07-ext-net/0.1.0/modules/std/net/http/spec.x07.json`

```json
[["export","std.net.http.spec.method_get","std.net.http.spec.method_post","std.net.http.spec.method_put","std.net.http.spec.method_delete","std.net.http.spec.headers_empty_v1","std.net.http.spec.headers1_v1","std.net.http.spec.headers_push_v1","std.net.http.spec.req_v1","std.net.http.spec.req_method_v1","std.net.http.spec.req_url_v1","std.net.http.spec.req_headers_v1","std.net.http.spec.req_body_v1","std.net.http.spec.req_caps_v1"],["import","std.u32"],["defn","std.net.http.spec.method_get",[],"i32",1],["defn","std.net.http.spec.method_post",[],"i32",2],["defn","std.net.http.spec.method_put",[],"i32",3],["defn","std.net.http.spec.method_delete",[],"i32",4],["defn","std.net.http.spec.headers_empty_v1",[],"bytes",["begin",["let","b",["bytes.alloc",4]],["std.u32.write_le_at","b",0,0]]],["defn","std.net.http.spec.headers1_v1",[["k","bytes"],["v","bytes"]],"bytes",["begin",["let","klen",["bytes.len","k"]],["let","vlen",["bytes.len","v"]],["let","out",["vec_u8.with_capacity",["+",12,["+", "klen","vlen"]]]],["set","out",["std.u32.push_le","out",1]],["set","out",["std.u32.push_le","out","klen"]],["set","out",["vec_u8.extend_bytes","out","k"]],["set","out",["std.u32.push_le","out","vlen"]],["set","out",["vec_u8.extend_bytes","out","v"]],["vec_u8.as_bytes","out"]]],["defn","std.net.http.spec.headers_push_v1",[["table","bytes"],["k","bytes"],["v","bytes"]],"bytes",["begin",["let","old_len",["bytes.len","table"]],["if",["<u","old_len",4],["return",["std.net.http.spec.headers1_v1","k","v"]],0],["let","old_count",["std.u32.read_le_at","table",0]],["let","klen",["bytes.len","k"]],["let","vlen",["bytes.len","v"]],["let","cap",["+", "old_len",["+",8,["+", "klen","vlen"]]]],["let","out",["vec_u8.with_capacity","cap"]],["set","out",["std.u32.push_le","out",["+", "old_count",1]]],["set","out",["vec_u8.extend_bytes_range","out","table",4,["-","old_len",4]]],["set","out",["std.u32.push_le","out","klen"]],["set","out",["vec_u8.extend_bytes","out","k"]],["set","out",["std.u32.push_le","out","vlen"]],["set","out",["vec_u8.extend_bytes","out","v"]],["vec_u8.as_bytes","out"]]],["defn","std.net.http.spec.req_v1",[["method_i32","i32"],["url_bytes","bytes"],["header_table_bytes","bytes"],["body_bytes","bytes"],["caps_bytes","bytes"]],"bytes",["begin",["let","url_len",["bytes.len","url_bytes"]],["let","hdr_len",["bytes.len","header_table_bytes"]],["let","body_len",["bytes.len","body_bytes"]],["let","caps_len",["bytes.len","caps_bytes"]],["let","cap",["+",16,["+", "url_len",["+", "hdr_len",["+", "body_len","caps_len"]]]]],["let","out",["vec_u8.with_capacity","cap"]],["vec_u8.push","out",1],["vec_u8.push","out","method_i32"],["vec_u8.push","out",0],["vec_u8.push","out",0],["set","out",["std.u32.push_le","out","url_len"]],["set","out",["vec_u8.extend_bytes","out","url_bytes"]],["set","out",["vec_u8.extend_bytes","out","header_table_bytes"]],["set","out",["std.u32.push_le","out","body_len"]],["set","out",["vec_u8.extend_bytes","out","body_bytes"]],["set","out",["std.u32.push_le","out","caps_len"]],["set","out",["vec_u8.extend_bytes","out","caps_bytes"]],["vec_u8.as_bytes","out"]]],["defn","std.net.http.spec.req_method_v1",[["req","bytes"]],"i32",["if",["<u",["bytes.len","req"],2],0,["bytes.get_u8","req",1]]],["defn","std.net.http.spec._headers_off_v1",[["req","bytes"]],"i32",["begin",["let","n",["bytes.len","req"]],["if",["<u","n",8],["return",0],0],["let","url_len",["std.u32.read_le_at","req",4]],["let","off",["+",8,"url_len"]],["if",["<u","n",["+", "off",4]],0,"off"]]],["defn","std.net.http.spec._headers_end_off_v1",[["req","bytes"]],"i32",["begin",["let","n",["bytes.len","req"]],["let","off",["std.net.http.spec._headers_off_v1","req"]],["if",["=","off",0],["return",0],0],["let","count",["std.u32.read_le_at","req","off"]],["let","cur",["+", "off",4]],["for","i",0,"count",["begin",["if",["<u","n",["+", "cur",4]],["return",0],0],["let","klen",["std.u32.read_le_at","req","cur"]],["set","cur",["+", "cur",4]],["if",["<u","n",["+", "cur","klen"]],["return",0],0],["set","cur",["+", "cur","klen"]],["if",["<u","n",["+", "cur",4]],["return",0],0],["let","vlen",["std.u32.read_le_at","req","cur"]],["set","cur",["+", "cur",4]],["if",["<u","n",["+", "cur","vlen"]],["return",0],0],["set","cur",["+", "cur","vlen"]],0]],"cur"]],["defn","std.net.http.spec.req_url_v1",[["req","bytes"]],"bytes",["begin",["let","n",["bytes.len","req"]],["if",["<u","n",8],["return",["bytes.alloc",0]],0],["let","url_len",["std.u32.read_le_at","req",4]],["if",["<u","n",["+",8,"url_len"]],["bytes.alloc",0],["bytes.slice","req",8,"url_len"]]]],["defn","std.net.http.spec.req_headers_v1",[["req","bytes"]],"bytes",["begin",["let","off",["std.net.http.spec._headers_off_v1","req"]],["let","end",["std.net.http.spec._headers_end_off_v1","req"]],["if",["or",["=","off",0],["=","end",0]],["return",["bytes.alloc",0]],0],["bytes.slice","req","off",["-","end","off"]]]],["defn","std.net.http.spec.req_body_v1",[["req","bytes"]],"bytes",["begin",["let","n",["bytes.len","req"]],["let","end",["std.net.http.spec._headers_end_off_v1","req"]],["if",["=","end",0],["return",["bytes.alloc",0]],0],["if",["<u","n",["+", "end",4]],["return",["bytes.alloc",0]],0],["let","body_len",["std.u32.read_le_at","req","end"]],["let","body_off",["+", "end",4]],["if",["<u","n",["+", "body_off","body_len"]],["bytes.alloc",0],["bytes.slice","req","body_off","body_len"]]]],["defn","std.net.http.spec.req_caps_v1",[["req","bytes"]],"bytes",["begin",["let","n",["bytes.len","req"]],["let","end",["std.net.http.spec._headers_end_off_v1","req"]],["if",["=","end",0],["return",["bytes.alloc",0]],0],["if",["<u","n",["+", "end",4]],["return",["bytes.alloc",0]],0],["let","body_len",["std.u32.read_le_at","req","end"]],["let","body_off",["+", "end",4]],["let","caps_len_off",["+", "body_off","body_len"]],["if",["<u","n",["+", "caps_len_off",4]],["return",["bytes.alloc",0]],0],["let","caps_len",["std.u32.read_le_at","req","caps_len_off"]],["let","caps_off",["+", "caps_len_off",4]],["if",["<u","n",["+", "caps_off","caps_len"]],["bytes.alloc",0],["bytes.slice","req","caps_off","caps_len"]]]]]
```

---

If you want, I can also add **one more tiny helper** that’s very agent-friendly:

* `std.net.http.spec.req_get_v1(url, headers_table, caps)`
* `std.net.http.spec.req_post_v1(url, headers_table, body, caps)`

…so agents don’t forget to pass empty bodies / correct method codes (the less “remembered structure” the better for autonomous agents).

[1]: https://curl.se/libcurl/c/CURLOPT_SSL_VERIFYPEER.html "CURLOPT_SSL_VERIFYPEER"
[2]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
++++

Here’s an **agent-friendly v1** addition that makes GET/POST request construction “non-forgettable”:

* `std.net.http.spec.req_get_v1(url, headers_table, caps)`
  → always uses **method GET** and a **zero-length body** (GET request bodies have no generally-defined semantics, so forcing empty is the safest default).
* `std.net.http.spec.req_post_v1(url, headers_table, body, caps)`
  → always uses **method POST**, body provided.

Both helpers also do a tiny robustness tweak: if `headers_table_bytes` is **shorter than 4 bytes** (missing the header_count u32), they treat it as `headers_empty_v1` to avoid “agent forgot to build headers table” footguns.

I also updated the normative doc (`docs/net/net-v1.md`) to mention these constructors and their behavior.

Some earlier uploaded files in this chat have expired on my side; for **this change** I didn’t need them, but if you want me to patch other previously-uploaded artifacts again, please re-upload those files.

## Drop-in bundle

Use docs/phases/assets/x07_ext_net_v1_https_bundle_impl3.tar.gz

---

## PR-style diff

### 1) `packages/x07-ext-net/0.1.0/modules/std/net/http/spec.x07.json`

```diff
--- a/packages/x07-ext-net/0.1.0/modules/std/net/http/spec.x07.json
+++ b/packages/x07-ext-net/0.1.0/modules/std/net/http/spec.x07.json
@@ -1,7 +1,9 @@
 [
   [
     "export",
     "std.net.http.spec.method_get",
     "std.net.http.spec.method_post",
+    "std.net.http.spec.req_get_v1",
+    "std.net.http.spec.req_post_v1",
     "std.net.http.spec.headers_empty_v1",
     "std.net.http.spec.headers1_v1",
     "std.net.http.spec.headers_push_v1",
@@ -171,6 +173,90 @@
       ]
     ]
   ],
+  [
+    "defn",
+    "std.net.http.spec.req_get_v1",
+    [
+      [
+        "url",
+        "bytes"
+      ],
+      [
+        "header_table",
+        "bytes"
+      ],
+      [
+        "caps",
+        "bytes"
+      ]
+    ],
+    "bytes",
+    [
+      "begin",
+      [
+        "let",
+        "ht",
+        [
+          "if",
+          [
+            "<u",
+            [
+              "bytes.len",
+              "header_table"
+            ],
+            4
+          ],
+          [
+            "std.net.http.spec.headers_empty_v1"
+          ],
+          "header_table"
+        ]
+      ],
+      [
+        "std.net.http.spec.req_v1",
+        [
+          "std.net.http.spec.method_get"
+        ],
+        "url",
+        "ht",
+        [
+          "bytes.alloc",
+          0
+        ],
+        "caps"
+      ]
+    ]
+  ],
+  [
+    "defn",
+    "std.net.http.spec.req_post_v1",
+    [
+      [
+        "url",
+        "bytes"
+      ],
+      [
+        "header_table",
+        "bytes"
+      ],
+      [
+        "body",
+        "bytes"
+      ],
+      [
+        "caps",
+        "bytes"
+      ]
+    ],
+    "bytes",
+    [
+      "begin",
+      [
+        "let",
+        "ht",
+        [
+          "if",
+          [
+            "<u",
+            [
+              "bytes.len",
+              "header_table"
+            ],
+            4
+          ],
+          [
+            "std.net.http.spec.headers_empty_v1"
+          ],
+          "header_table"
+        ]
+      ],
+      [
+        "std.net.http.spec.req_v1",
+        [
+          "std.net.http.spec.method_post"
+        ],
+        "url",
+        "ht",
+        "body",
+        "caps"
+      ]
+    ]
+  ],
   [
     "defn",
     "std.net.http.spec.req_v1",
     [
       [
         "method_i32",
         "i32"
```

### 2) `docs/net/net-v1.md`

```diff
--- a/docs/net/net-v1.md
+++ b/docs/net/net-v1.md
@@ -154,6 +154,16 @@ Encoding:
 - `caps_bytes_len_u32_le` (must be 24)
 - `NetCapsV1` bytes (24 bytes)

+Preferred constructors (agent-friendly):
+
+- `std.net.http.spec.req_get_v1(url_bytes, headers_table_bytes, caps_bytes)`
+  - Always uses method GET and an **empty body** (agents don’t need to remember method codes or pass an explicit empty body).
+  - If `headers_table_bytes` is shorter than 4 bytes, it is treated as `headers_empty_v1`.
+
+- `std.net.http.spec.req_post_v1(url_bytes, headers_table_bytes, body_bytes, caps_bytes)`
+  - Always uses method POST (method code handled internally).
+  - If `headers_table_bytes` is shorter than 4 bytes, it is treated as `headers_empty_v1`.
+
 ### HttpRespV1
```

---

## Notes for agent ergonomics

* **URL bytes** should be a valid URI/URL string per RFC 3986 (your `std.net.url` module can canonicalize / validate before building the request).
* Since you require **HTTPS in v1**, keep certificate verification enabled in your OS adapter (e.g., libcurl verifies peer certs and hostname by default; don’t default to “insecure”).
+++++++
You’re already past the *hardest* “agent reliability” cliff for networking: **agents don’t hand-roll binary encodings** anymore (you added pack/unpack + req_get/req_post helpers). What’s left to make the external net package feel “full-featured” (C/Rust-class) is mostly:

1. **completeness of the spec surface** (so nothing is “undefined/guessy”),
2. **streaming + concurrency integration** (so you can build real apps without “capture everything into RAM”),
3. **server-side HTTPS correctness**,
4. **cross-platform OS backends**, and
5. **policy/hardening** (so run-os-sandboxed is actually usable by agents).

Below is a prioritized gap list that’s concrete enough to drive PRs.

---

## P0 gaps: make the spec surface “closed” (no guessing)

### 1) Define `HttpRespV1` as a *first-class* spec blob (pack + unpack)

Right now you have request/caps/address covered. The next gap is: **responses must be equally rigid** so agents never “parse by vibes”.

Minimum `HttpRespV1` should include:

* `tag` (OK/ERR)
* `status_u16`
* `headers_table` (canonical encoding)
* **body**: either `bytes` (small) or a streaming handle reference (see P2)
* `err_code_u32` when tag=ERR (and a stable mapping from OS/lib errors to your codes)
* optional: `tls_peer` summary (verified? sni? alpn?), but keep this optional to avoid platform drift.

This becomes the “one true output” for `std.net.http.fetch_*` and for server request handlers.

### 2) Canonical `headers_table` rules (ordering, casing, duplicates)

To make agents reliable, headers must have:

* **canonical case-folding rule** (e.g., lower-case keys)
* stable ordering rule (“sorted by key then by insertion index” or similar)
* duplicates handling rule (either allow repeated keys as multiple pairs, or normalize into a joined value with `,` — pick one and freeze it)

Agents will otherwise produce inconsistent logic when reading/writing headers.

### 3) URL rules that match HTTP semantics (and avoid footguns)

Two pragmatic points you should encode into the spec helpers:

* **GET requests should default to empty body**; HTTP semantics for content in a GET request are not generally defined, so “agents accidentally attach a body” should be impossible with your helpers. ([RFC Editor][1])
* Normalize/validate URLs once (scheme/host/port/path) and emit deterministic errors.

### 4) TLS verification defaults must be “secure by default”

Whatever backend you use, you want the *spec* to strongly prefer:

* verify peer certificate
* verify host name

For example, libcurl exposes explicit switches for these (`CURLOPT_SSL_VERIFYPEER`, `CURLOPT_SSL_VERIFYHOST`) and host verification expects the certificate name to match the URL host. ([Curl][2])
If you’re using rustls anywhere in the stack, a nice property is that its **main API does not allow “turn off certificate verification”**, which makes your “agent-safe default” harder to accidentally bypass. ([Docs.rs][3])

So the gap here is not “add a flag”, it’s: **make insecure modes require explicit policy permission** (run-os-sandboxed) *and* require an explicit opt-in field in caps (double consent).

---

## P1 gaps: HTTP(S) client parity (what real programs need)

### 5) Redirects (policy-controlled) + safe method rewriting rules

Full-featured HTTP clients need redirects, but they must be controlled:

* `max_redirects`
* `allow_cross_origin_redirects` (default false in sandbox)
* rule for method rewriting (e.g., 301/302/303 semantics)

### 6) Proxy support (including HTTPS proxy TLS verification)

If you want “real network programs”, you’ll eventually need proxies:

* explicit proxy config in caps (NOT ambient env vars for sandbox world)
* HTTPS proxy certificate verification controls are distinct from the origin server; libcurl has separate “proxy verify peer” config. ([Curl][4])

### 7) Timeouts and retries that are explicit and bounded

Agents need a canonical retry strategy (or no retries):

* connect timeout
* total deadline
* max retries
* backoff (fixed or capped exponential)

All must be caps-driven, so behavior is auditable and sandboxable.

### 8) Connection reuse / keep-alive (optional in v1, important later)

Not required to be “correct”, but required for “not painfully slow”.
If you’re using curl, you’ll probably want a “client handle” that keeps connection pools.

---

## P2 gaps: Streaming (this is the big “full featured” unlock)

### 9) Response-body streaming (avoid capture-only)

Your current model sounds “capture returns final bytes doc”. That blocks:

* large downloads
* true pipelines (parse while reading)
* memory safety goals (peak_live_bytes explosions)

You already have `std.io` traits + buffering; so the missing piece is:

* `std.net.http.fetch_stream_v1(req, caps) -> iface` (reader)
* and a response header “prelude” encoding so you can get status/headers before body streaming begins.

For agent ergonomics:

* keep `fetch_v1 -> HttpRespV1` for small bodies,
* add `fetch_stream_v1 -> (HttpRespHeadV1 + iface)` for large bodies.

### 10) Request-body streaming (upload)

For POST/PUT with big bodies, you need:

* a writer or “body provider” interface
* a hard cap in policy/caps (so sandbox is safe)

Without this, agents will keep constructing huge `bytes` bodies.

---

## P3 gaps: HTTPS server support (your explicit requirement)

### 11) Listener + accept + per-conn handles

To truly “support servers”, you need OS builtins + stdlib wrappers for:

* bind/listen with addr (v4/v6), port
* accept connections (blocking or poll-based)
* close handle, drop handle

### 12) TLS server config (cert/key) and safe defaults

Servers + HTTPS means:

* load cert chain + private key
* choose TLS versions / ciphers (or a safe fixed set)
* SNI behavior
* optional client cert auth (mTLS) later

Sandbox policy must decide:

* which ports you can bind to
* whether binding to non-loopback is allowed
* whether you can load cert/key from filesystem and from which roots (or only allow bytes literal / embedded blobs)

### 13) HTTP request parsing and response writing rules

You need deterministic, spec’d behavior for:

* request line parsing
* header parsing and canonicalization
* body framing (content-length, chunked)

The exact HTTP semantics are defined by the HTTP core specs (HTTP/1.1, etc.). ([RFC Editor][1])
For v1 you can scope to HTTP/1.1 only; HTTP/2 can come later.

### 14) Concurrency integration (server shouldn’t freeze the runtime)

Even in run-os (nondeterministic), agents need:

* a canonical accept loop pattern
* spawn a task per connection (cooperative) **or** spawn subprocess workers for CPU-heavy work

But this only works well if network I/O can yield (streaming primitives + poll integration).

---

## P4 gaps: Cross-platform backends (Linux/macOS/Windows)

### 15) Windows socket backend (big missing gap if you’re POSIX-centric)

On Windows, you’ll need:

* WSAStartup/WSACleanup lifecycle
* socket APIs (Winsock)
* an event wait primitive (poll/select/IOCP)

`WSAPoll` exists on Windows (supported since Vista+), but you still need a careful abstraction layer to unify with POSIX polling. ([Microsoft Learn][5])

If you don’t implement a real Windows backend, your net package will not be “full featured” for production adoption.

### 16) TLS backend portability

The fastest route to “HTTPS everywhere” is usually:

* ship a known TLS stack consistently (e.g., rustls) **or**
* use platform TLS (SChannel / Secure Transport / OpenSSL) with a portability layer

Whichever you pick, the spec blobs must stay stable so agents don’t care.

---

## P5 gaps: Sandbox hardening (run-os-sandboxed usability)

Even if the world is “nondeterministic”, agents need **predictable policy failure modes**.

### 17) Policy: egress allowlists need *both* DNS and CIDR checks (you chose this)

Good call, but you still need to decide:

* how DNS resolves are handled (which resolver? system? DoH?)
* how many A/AAAA records are allowed
* what happens on multiple IPs (must all be allowed? any allowed?)

### 18) Ingress policy (servers)

You need policy controls for:

* `allow_listen` boolean
* allowed bind addresses (loopback-only default)
* allowed ports ranges
* max concurrent conns
* max bytes per conn

### 19) Explicit “unsafe/insecure TLS” gates

If you allow any “skip verify” mode, require:

* policy allow
* caps field allow
* and diagnostic logs marking the request as insecure

This prevents agents from silently shipping insecure networking.

---

## Optional “full-featured” items (don’t block v1, but matter later)

* WebSockets (common in modern systems)
* HTTP/2 (performance)
* UDP primitives (DNS, custom protocols)
* QUIC/HTTP/3 (future)

---

## A pragmatic definition of “full featured” for your v1 package

If you can do these *without hand-rolled bytes*:

1. HTTPS client: GET/POST + headers + redirects + timeouts + proxy (optional)
2. HTTPS server: listen + accept + parse request + write response
3. Streaming: download/upload via `iface` + `std.io.bufread`
4. Cross-platform: Linux + macOS + Windows smokes
5. Sandbox: explicit allowlists (DNS+CIDR) + bind rules + strict caps

…then your external networking package is “full featured enough” for agents to build real tools.

---
[1]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
[2]: https://curl.se/libcurl/c/CURLOPT_SSL_VERIFYPEER.html?utm_source=chatgpt.com "CURLOPT_SSL_VERIFYPEER explained"
[3]: https://docs.rs/rustls/latest/rustls/ "rustls - Rust"
[4]: https://curl.se/libcurl/c/CURLOPT_PROXY_SSL_VERIFYPEER.html?utm_source=chatgpt.com "CURLOPT_PROXY_SSL_VERIF..."
[5]: https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsapoll "WSAPoll function (winsock2.h) - Win32 apps | Microsoft Learn"
+++++
Here are the **highest‑impact gaps** still left for `x07:ext-net` to feel “full‑featured” for autonomous agents, and a **drop‑in bundle** that fills the top ones immediately.

## Highest‑impact remaining gaps

### P0 — Response contracts are underspecified / easy for agents to mis-handle

You already nailed `NetAddrV1/NetCapsV1/HttpReqV1`. What was missing is a **canonical HttpResp** contract and accessors so agents don’t “guess offsets”.

**Fix now:** add **HttpRespV1** (captured) + **HttpRespStreamV1** (stream handle) encodings + `pack/unpack` helpers.

### P1 — Header handling needs canonicalization rules

If agents can build semantically equivalent header sets into *different bytes*, you get:

* nondeterministic caching keys (if you hash request bytes)
* inconsistent policy matching
* “it works locally but not in sandbox” surprises

**Fix now:** canonicalize header **keys to ASCII lowercase** in the spec helpers.

(Stable sorting by header key is a **future improvement**; I didn’t add it yet because it’s heavier and you can ship v1 with deterministic “insertion order + canonical key folding”.)

### P2 — Streaming response bodies need an agent-friendly “read to end”

Even if you add a stream fetch builtin, agents still often get stuck writing loops.

**Fix now:** a single helper `std.net.stream.read_to_end_v1(handle, chunk_max, caps)` that does the boring loop correctly.

### Still missing after this bundle (next priorities)

These remain the next big unlocks for real apps:

* **Stable header sorting canonicalization** (order-insensitive canonical bytes)
* **Streaming upload** (request body streaming; chunked transfer; file→socket)
* **HTTP server API** (including TLS cert/key provisioning, listen/accept + HTTP parse)
* **WebSocket** (or at least raw TCP + TLS stream primitives good enough to build it)
* **Better TLS portability guidance** (backends differ; libcurl vs rustls tradeoffs). curl’s docs compare SSL backends and feature deltas, which is useful if you’re making cross‑platform choices. ([Curl][1])
  rustls is another option (TLS 1.2/1.3, client+server) if you want a Rust‑native TLS layer rather than libcurl. ([Docs.rs][2])

## Drop‑in bundle: fills P0 + P1 + P2

This bundle:

* **rewrites** `docs/net/net-v1.md` to be complete and normative (no tool-citations, no ellipses)
* updates `std.net.http.spec` with:

  * `HttpRespV1` + `HttpRespStreamV1` **encodings**
  * `resp_*` and `resp_stream_*` helpers
  * **header key canonicalization** (`headers_*_canon_v1`, `headers_canon_v1`)
  * updates `req_get_v1/req_post_v1` to canonicalize header keys automatically
* updates `std.net.http.client` to add:

  * `fetch_stream_v1`
  * `get_v1 / post_v1` wrappers that always go through spec helpers
* updates `std.net.stream` with:

  * `read_to_end_v1`

### Files included

```
x07_ext_net_v1_https_bundle_gapfill/
  docs/net/net-v1.md
  packages/x07-ext-net/0.1.0/modules/std/net/http/spec.x07.json
  packages/x07-ext-net/0.1.0/modules/std/net/http/client.x07.json
  packages/x07-ext-net/0.1.0/modules/std/net/stream.x07.json
  ... (keeps the rest of your previous skeleton unchanged)
```

Use docs/phases/assets/x07_ext_net_v1_https_bundle_gapfill.tar.gz.

## Quick “what changed” API summary

### New / updated helpers (agent‑facing)

**Headers canonicalization**

* `std.net.http.spec.headers1_canon_v1(key, val) -> bytes`
* `std.net.http.spec.headers_push_canon_v1(table, key, val) -> bytes`
* `std.net.http.spec.headers_canon_v1(table) -> bytes`
  (folds keys to ASCII lowercase; preserves insertion order)

**HTTP response (captured)**

* `std.net.http.spec.resp_v1(status, headers_table, body) -> bytes`
* `std.net.http.spec.resp_status_v1(resp) -> i32`
* `std.net.http.spec.resp_headers_v1(resp) -> bytes`
* `std.net.http.spec.resp_body_v1(resp) -> bytes`

**HTTP response (streamed)**

* `std.net.http.spec.resp_stream_v1(status, headers_table, body_len_hint, stream_handle) -> bytes`
* `std.net.http.spec.resp_stream_status_v1(doc) -> i32`
* `std.net.http.spec.resp_stream_headers_v1(doc) -> bytes`
* `std.net.http.spec.resp_stream_body_len_hint_v1(doc) -> i32`
* `std.net.http.spec.resp_stream_handle_v1(doc) -> i32`

**HTTP client**

* `std.net.http.client.fetch_stream_v1(req_bytes) -> bytes` (calls `os.net.http_fetch_stream_v1`)
* `std.net.http.client.get_v1(url, caps) -> bytes`
* `std.net.http.client.get_with_headers_v1(url, headers_table, caps) -> bytes`
* `std.net.http.client.post_v1(url, body, caps) -> bytes`
* `std.net.http.client.post_with_headers_v1(url, headers_table, body, caps) -> bytes`

**Stream quality-of-life**

* `std.net.stream.read_to_end_v1(handle, chunk_max, caps) -> bytes`

---

If you want the **next** drop‑in after this one, the best single upgrade is **order‑insensitive header canonicalization** (stable sort by key, then by value), because it makes caching + policy + diffs dramatically more reliable for agents.

[1]: https://curl.se/docs/ssl-compared.html "curl - SSL libraries compared"
[2]: https://docs.rs/rustls/latest/rustls/ "rustls - Rust"
+++++
Here’s the **next drop‑in bundle** that implements **order‑insensitive header canonicalization** (stable sort by **key, then value**) so agents don’t end up with different request bytes just because they built header tables in a different order.

Use docs/phases/assets/x07_ext_net_v1_https_bundle_hdrsort.tar.gz

## What this bundle changes

### 1) New API: sorted canonicalization for HeadersTableV1

Adds:

* `std.net.http.spec.headers_canon_sorted_v1(table: bytes) -> bytes`

Behavior:

* Lowercases header keys (HTTP field names are case-insensitive). ([RFC Editor][1])
* Produces a **canonical, order-insensitive** header table by sorting entries by:

  1. `key_bytes` (lexicographic), then
  2. `value_bytes` (lexicographic)

Sorting is deterministic and stable.

### 2) `req_get_v1` / `req_post_v1` now use the sorted canonicalization

Updates:

* `std.net.http.spec.req_get_v1(...)`
* `std.net.http.spec.req_post_v1(...)`

They now canonicalize the header table via `headers_canon_sorted_v1` so agents don’t need to remember to call it.

### 3) Docs updated to match

Updates `docs/net/net-v1.md` to document:

* `headers_canon_v1`: lowercase only, preserves insertion order
* `headers_canon_sorted_v1`: lowercase + **sorted canonical order** (recommended for hashing/policy stability)

And includes a warning about duplicate header lines: HTTP notes that **field order can be significant** when multiple field lines share a name and are combined. ([RFC Editor][1])
So the sorted canonicalization is intended for “agent stability / hashing / policy” use, and you should avoid relying on multi-line ordering semantics (prefer a single combined value when order matters).

## Files touched inside the bundle

* `docs/net/net-v1.md`
* `packages/x07-ext-net/0.1.0/modules/std/net/http/spec.x07.json`

## Notes on spec correctness vs agent-friendliness

* Lowercasing header keys is correct because field names are case-insensitive in HTTP. ([RFC Editor][1])
* Sorting **can** change semantics if you were relying on the order of repeated header field lines of the same name (HTTP says order can matter for interpretation when combined). ([RFC Editor][1])
  This is why the docs call it out explicitly: for agentic coding, the intended discipline is:

  * represent ordered header semantics inside a **single header value** (e.g., `"a, b, c"`), not as multiple header lines.

[1]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
+++++
Here’s the *next drop‑in bundle* that adds both:

* a strict **no‑duplicate builder**: `std.net.http.spec.headers_set_v1(...)`
* a deterministic **combine‑duplicates canonicalizer**: `std.net.http.spec.headers_canon_join_sorted_v1(...)`

This aligns with HTTP semantics where **field names are case‑insensitive** and multiple same‑name fields can be combined into a “combined field value” using commas (with the well‑known `set-cookie` exception).

Use docs/phases/assets/x07_ext_net_v1_https_bundle_hdrdedupe.tar.gz.
SHA‑256: `75593d6d136f5e7d95f693f43695b5223ba8487923c5faedebf902cf5cd5a8f2`

## What this bundle contains

It overwrites/updates exactly these paths:

1. `packages/x07-ext-net/0.1.0/modules/std/net/http/spec.x07.json`

Adds:

* `std.net.http.spec.headers_set_v1(table, key, val) -> bytes`

  * **lowercases** `key` (ASCII)
  * **removes** all existing entries for that key
  * **inserts exactly one** entry for that key (replacing the first occurrence position if present; else appends)
  * guarantees output has **no duplicate keys** (case‑insensitive)

* `std.net.http.spec.headers_canon_join_sorted_v1(table) -> bytes`

  * does `headers_canon_sorted_v1` (lowercase + stable sort by `(key, value)`)
  * then groups adjacent equal keys and joins values with **literal** `", "` (comma + single space)
  * guarantees output has **one field line per name** (for combinable headers)

Also updates:

* `std.net.http.spec.req_get_v1`
* `std.net.http.spec.req_post_v1`

…so they now canonicalize headers using **`headers_canon_join_sorted_v1`** instead of only `headers_canon_sorted_v1`.

2. `docs/net/net-v1.md`

Replaces the header semantics section with a clean, normative description of:

* the HeadersV1 encoding,
* case folding expectations,
* no‑duplicate builder semantics,
* join‑duplicates semantics,
* and the `set-cookie` caution.

## Quick usage patterns (agent-friendly)

### “I never want duplicates”

Use `headers_set_v1` repeatedly:

```json
["begin",
  ["let","h",["std.net.http.spec.headers_empty_v1"]],
  ["set","h",["std.net.http.spec.headers_set_v1","h",["bytes.lit","accept"],["bytes.lit","application/json"]]],
  ["set","h",["std.net.http.spec.headers_set_v1","h",["bytes.lit","user-agent"],["bytes.lit","x07-agent"]]],
  ["std.net.http.spec.req_get_v1",["bytes.lit","https://example.com/"],"h","caps"]]
```

### “I might add duplicates, but canonicalize to a single field line per name”

Use `headers_push_v1`, and rely on `req_*_v1` joining:

```json
["begin",
  ["let","h",["std.net.http.spec.headers_empty_v1"]],
  ["set","h",["std.net.http.spec.headers_push_v1","h",["bytes.lit","accept"],["bytes.lit","application/json"]]],
  ["set","h",["std.net.http.spec.headers_push_v1","h",["bytes.lit","accept"],["bytes.lit","text/plain"]]],
  ["std.net.http.spec.req_get_v1",["bytes.lit","https://example.com/"],"h","caps"]]
```

Result will encode `accept: application/json, text/plain` (deterministically sorted first, then joined).

## Why this is the “right” default

* HTTP header field names are **case-insensitive**, so lowercasing keys in the canonical encoding is safe and stabilizes hashing/policy matching.
* HTTP defines how **multiple same‑name fields can be combined** into a single combined value using commas, while also calling out important exceptions (notably `set-cookie`).

If you want, the next follow‑up bundle can add an even stricter API variant (e.g. `headers_insert_unique_v1(...) -> result_bytes` that returns an error code if the key already exists), but for autonomous agents `headers_set_v1` + `headers_canon_join_sorted_v1` tends to be the most “can’t mess it up” combination.
++++
Yes — even after the header sorting + dedupe helpers, there are still a few **high‑impact gaps** that matter if you want the external net package to feel “production‑ready for autonomous agents” (especially across platforms), while staying small and spec‑driven.

Below is a practical checklist of what’s still missing (or worth tightening), grouped by “must‑have for v1 production” vs “next tier”.

---

## Must‑have gaps to close for v1 “agent‑usable in production”

### 1) Response model parity: a canonical `HttpRespV1` (status + headers + body) with strict header semantics

You already made request encoding agent-friendly. The next big reliability win is to **make response handling equally spec‑shaped and non‑guessy**:

* Ensure response headers are returned in a canonical internal representation:

  * field names normalized (typically lowercase ASCII)
  * deterministic ordering (lexicographic)
  * deterministic duplicate handling:

    * “combine duplicates” into one field line **for fields that are list‑based**
    * but **never combine** `set-cookie` (it is explicitly special in practice/specs). ([RFC Editor][1])
* Make `HttpRespV1.unpack` return:

  * `status_u32`
  * `headers_table` (your table format)
  * `body_bytes` (or `body_reader_iface` if you add streaming later)
  * `err_code` for failures

Why this matters: HTTP explicitly allows intermediaries to merge multiple field lines of the same name into a comma‑separated value list for many headers, but not universally; relying on “whatever lib returns” makes agents brittle. ([RFC Editor][1])

### 2) URL parsing/normalization needs to be strict and predictable (and should be the *only* way agents build URLs)

Even if you already have `std.net.url`, for production you’ll want:

* RFC3986‑aligned parsing (scheme/authority/path/query/fragment), and a canonicalization policy you document and keep stable. ([Curl][2])
* Agent‑friendly constructors:

  * `url.parse_v1(bytes) -> Result<UrlV1, code>`
  * `url.join_v1(base, rel) -> Result<UrlV1, code>`
  * `url.to_bytes_v1(url) -> bytes` (canonical)

This reduces agent “almost correct” behavior around:

* percent encoding
* default ports
* absolute vs relative paths
* invalid control chars / spaces

### 3) TLS behavior must be explicit and policy‑checkable (not “whatever defaults do”)

Because HTTPS is required, your v1 contract should *force* agents into safe defaults and make insecure modes unrepresentable unless explicitly allowed by policy.

At minimum, make sure your caps include:

* `tls.verify_peer` (default true)
* `tls.verify_host` (default true)
* `tls.ca_bundle` selection rules (system store vs pinned path vs embedded bundle)
* optional `tls.server_name` override (rare, but needed for some SNI edge cases)

If you’re using libcurl under the hood, these map to well‑known knobs like peer/host verification and CA bundle configuration.

And in `run-os-sandboxed` you usually want:

* forbid disabling verification unless a policy flag allows it
* allow pinning CA bundle paths only via allowlisted files

### 4) Sandboxed policy must cover **servers** as well as clients

You said “servers in v1”, which implies you need policy controls for inbound operations too:

Outbound (client):

* allow by **DNS name allowlist AND CIDR allowlist**
* also include **port allowlist/ranges** (otherwise CIDR allowlists still allow surprising targets)
* enforce “resolve → connect” so DNS names can’t be used to sneak to disallowed IPs (DNS‑rebind style)

Inbound (server):

* allow listen addresses (e.g., `127.0.0.1` only by default)
* allow ports (explicit list or range)
* max concurrent conns / accept rate / total bytes caps

Without inbound policy, “HTTPS server support” becomes a sandbox escape vector in `run-os-sandboxed`.

### 5) Timeouts and cancellation must be first-class

For agentic coding, “it hung” is a top failure mode.

You want caps for:

* connect timeout
* request/overall timeout
* read timeout (or idle timeout)
* max header bytes and max body bytes

And you want a cancellation pathway:

* `http.cancel_v1(handle)` if you add streaming / async requests
* or ensure `spawn_capture` workers can be killed deterministically and error returns are stable

### 6) Redirect policy must be specified (even if you keep it minimal)

Agents will hit redirects constantly. You need deterministic, explicit behavior:

* allow/deny redirects
* max redirects
* whether headers are forwarded on redirect (never forward `Authorization` cross-host by default)
* whether method changes (e.g., POST→GET rules) are allowed

Even if you don’t implement all nuances initially, **your contract must say what happens**.

Also: avoid GET-with-body mistakes; HTTP semantics explicitly warn that request content on GET has no generally defined meaning and clients shouldn’t generate it. ([RFC Editor][1])
Your `req_get_v1` helper is exactly the right direction — extend this pattern.

---

## Next-tier gaps (not strictly required for v1, but big unlocks)

### 7) Streaming I/O for request/response bodies (integrate with `std.io` traits)

“Capture-only” is fine for small requests; it becomes painful for:

* downloads/uploads
* large JSON
* file proxying

A production‑friendly design is:

* `http.open_resp_body_v1(resp_doc) -> iface` (Reader)
* `http.send_stream_v1(req_doc, body_reader_iface) -> resp_doc`
* keep a hard `max_body_bytes` cap even in streaming mode (or “max_total_read_bytes” enforced by adapter)

### 8) HTTP/2 and WebSockets (explicitly planned, not accidental)

You don’t *need* HTTP/2 in v1, but you should decide whether you’re:

* explicitly HTTP/1.1 only
* or you allow the backend to negotiate HTTP/2

HTTP/2 is a major semantic/behavior shift (multiplexing, header compression). ([RFC Editor][3])
WebSockets similarly introduce framing + long-lived duplex connections. ([RFC Editor][4])

For agents, these should be **new, separate APIs** (`ws.*`, `http2.*`) rather than “sometimes this behaves differently”.

### 9) Cookies as a library feature (cookie jar), not ad-hoc header hacks

Agents will otherwise try to manually glue `cookie` headers and break:

* cookie scope rules
* multiple `set-cookie` handling
* persistence

If you add this:

* keep it explicit: `cookiejar.apply(req_headers)` and `cookiejar.update(resp_headers)`
* preserve `set-cookie` as multi-valued (don’t combine) ([RFC Editor][1])

### 10) Proxy support

If you want “real world” network programs:

* HTTP proxy / HTTPS proxy / NO_PROXY patterns
* explicit in caps (don’t silently read env in sandboxed mode)

### 11) Cross-platform reality: decide the portability contract

Right now you note “POSIX-centric”; networking needs the same clarity.

Options:

* **Use one portable helper binary** (`x07-netd`) invoked via your subprocess table on all OSes (fastest path to Windows parity).
* Or implement platform backends in the OS runner/runtime:

  * Windows: WinHTTP/Schannel or libcurl builds
  * macOS: SecureTransport / Network framework or curl+SecureTransport
  * Linux: OpenSSL/boringssl/rustls via helper

If you don’t standardize this, users will hit “works on Linux, fails on Windows”.

---

## The “one thing” I’d prioritize next

If you only do one more “gapfill bundle”, make it:

**`HttpRespV1` + response header canonicalization rules** (including `set-cookie` multi-value preservation) + explicit TLS defaults + redirect policy caps.

That one change tends to eliminate a huge class of “agent guessed wrong” behaviors and makes your net APIs symmetrical: **spec‑build request → send → spec‑unpack response**.

---

If you want, I can outline the **exact bytes encodings** for `HttpRespV1` (parallel to your `HttpReqV1`), plus the deterministic header merge algorithm that matches RFC9110’s guidance while keeping `set-cookie` as a multi-valued exception. ([RFC Editor][1])

[1]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
[2]: https://curl.se/libcurl/c/CURLOPT_SSL_VERIFYPEER.html?utm_source=chatgpt.com "CURLOPT_SSL_VERIFYPEER explained"
[3]: https://www.rfc-editor.org/info/rfc9113?utm_source=chatgpt.com "Information on RFC 9113"
[4]: https://www.rfc-editor.org/info/rfc6455?utm_source=chatgpt.com "Information on RFC 6455"
++++++

Below is a **normative v1** proposal for `HttpRespV1` that mirrors the “specbin-style” (`u32_le` header + length‑delimited payloads) you’ve been using for `HttpReqV1`, plus a **deterministic header merge algorithm** that follows RFC 9110’s “combine field lines” guidance while treating `set-cookie` as a special multi-valued exception.

---

## HttpRespV1 bytes encoding (v1)

### Endianness + primitives

* All integers are **unsigned `u32` little‑endian** unless stated otherwise.
* All variable data is length‑delimited (a preceding `u32_le length`, followed by that many bytes).
* All offsets below are **byte offsets** from the start of the `HttpRespV1` blob.

### Magic + version

This is primarily to prevent agents from accidentally passing “the wrong blob type” into an unpacker.

* `MAGIC_RESP = b"X7HR"` (4 bytes)
* `VERSION = 1`

### Tags

`tag_u32`:

* `0` = **ERR** (transport/policy error; no HTTP response semantics)
* `1` = **OK_BYTES** (complete response body captured inline)
* `2` = **OK_STREAM** (response body is a stream handle; bytes not inlined)

### Layout

Fixed header is **32 bytes**.

|           Offset | Field              | Type                     | Meaning                                                          |
| ---------------: | ------------------ | ------------------------ | ---------------------------------------------------------------- |
|                0 | `magic`            | 4 bytes                  | Must be `X7HR`                                                   |
|                4 | `version`          | `u32_le`                 | Must be `1`                                                      |
|                8 | `tag`              | `u32_le`                 | 0/1/2                                                            |
|               12 | `status`           | `u32_le`                 | HTTP status code (e.g. 200) when OK_*; `0` when ERR              |
|               16 | `err_code`         | `u32_le`                 | `0` when OK_*; nonzero when ERR                                  |
|               20 | `headers_len`      | `u32_le`                 | length in bytes of `HeadersTableV1`                              |
|               24 | `body_len_or_hint` | `u32_le`                 | if OK_BYTES: body length; if OK_STREAM: optional hint; if ERR: 0 |
|               28 | `stream_handle`    | `u32_le`                 | if OK_STREAM: nonzero handle; else 0                             |
|               32 | `headers_bytes`    | `headers_len` bytes      | `HeadersTableV1`                                                 |
| 32+`headers_len` | `body_bytes`       | `body_len_or_hint` bytes | present only if OK_BYTES                                         |

### Required invariants

**ERR (tag=0)**

* `status == 0`
* `headers_len == 0`
* `body_len_or_hint == 0`
* `stream_handle == 0`
* `err_code != 0`

**OK_BYTES (tag=1)**

* `err_code == 0`
* `stream_handle == 0`
* `headers_len >= 0`
* `body_len_or_hint` is exact body size
* body bytes are present inline

**OK_STREAM (tag=2)**

* `err_code == 0`
* `headers_len >= 0`
* `stream_handle != 0`
* `body_len_or_hint` is a **hint** only (may be 0 if unknown)
* no inline body bytes; body must be read through the stream API

### HeadersTableV1 (referenced payload)

(If you already have this defined, keep it unchanged; the important part is that response headers are delivered in a deterministic merged form.)

```
HeadersTableV1 :=
  u32_le count
  repeated count times:
    u32_le name_len
    bytes[name_len] name_bytes   ; header name (canonical lowercase)
    u32_le value_len
    bytes[value_len] value_bytes ; merged value (or per-line value for set-cookie)
```

---

## Deterministic header merge algorithm (RFC 9110-aligned, `set-cookie` exception)

### Why we must merge deterministically

HTTP headers can arrive as multiple field lines with the same name. RFC 9110 defines how repeated field lines represent a combined field value (comma-separated list semantics), and explicitly allows recipients to combine them back into one field line using comma + optional whitespace, recommending comma+SP for consistency. ([RFC Editor][1])

Field names are case-insensitive, so canonicalizing them to lowercase is standard practice. ([RFC Editor][1])

### Special case: `Set-Cookie`

RFC 9110 notes that `Set-Cookie` often appears across multiple field lines and **cannot** be combined into a single field value, so recipients should treat it as a special case. ([RFC Editor][1])
RFC 6265 is even more explicit: `Set-Cookie` header fields **must not be folded** into a single header field. ([IETF Datatracker][2])

### Algorithm inputs/outputs

**Input:** a raw list of header field lines as received from the OS HTTP stack / TLS library:
`[(name_bytes, value_bytes), ...]` (order matters)

**Output:** a canonical `HeadersTableV1` such that:

* header names are canonical lowercase,
* duplicate names are merged using RFC 9110 rules (comma + optional whitespace → choose comma+SP),
* **except** `set-cookie`, which remains multi-valued (kept as multiple entries).

### Canonicalization rules

#### 1) Normalize header names (case-insensitive → lowercase)

For each header line:

* Convert ASCII `A..Z` to `a..z` in `name_bytes` (bytewise).
* Do not attempt Unicode casefolding (HTTP field-name is `token` and effectively ASCII). RFC 9110 states field names are case-insensitive. ([RFC Editor][1])

Result: `name_lc`.

#### 2) Trim outer OWS on values (optional but recommended)

RFC 9110’s combination rule permits comma + optional whitespace and recommends comma+SP. ([RFC Editor][1])
To stabilize output:

* Strip **leading/trailing** spaces (`0x20`) and tabs (`0x09`) from each `value_bytes`.
* Do not touch interior bytes.

Call this `value_trim`.

#### 3) Stable sort by name (deterministic grouping)

To make output deterministic independent of platform header ordering quirks:

* **Stable-sort** all lines by `name_lc` lexicographically.
* Stability preserves within-name arrival order (so merge order respects the order guidance). ([RFC Editor][1])

#### 4) Merge duplicates per name

For each group of lines with the same `name_lc`:

* If `name_lc == b"set-cookie"`:

  * **Do not merge.**
  * Emit **one entry per line** in `HeadersTableV1` (same name repeated, different values).
  * Justification: `Set-Cookie` cannot be combined. ([IETF Datatracker][2])

* Else:

  * Merge values into a single value by concatenating in order with separator `b", "` (comma + SP).
  * This exactly implements RFC 9110’s allowed combination behavior and chooses comma+SP for consistency. ([RFC Editor][1])
  * Emit **one** entry in `HeadersTableV1`.

#### 5) Emit HeadersTableV1

* `count` is the number of emitted entries after merging (including multiple `set-cookie` entries).
* Each entry’s `name_bytes` is the canonical lowercase name.
* Each entry’s `value_bytes` is the merged value (or original per-line value for `set-cookie`).

### Pseudocode

```text
function headers_merge_rfc9110_v1(raw_lines):
  norm = []
  for (name, value) in raw_lines:
    name_lc = ascii_lower(name)
    value_trim = strip_ows(value)    ; strip SP/HTAB at ends
    norm.append((name_lc, value_trim))

  norm = stable_sort_by_name(norm)   ; lexicographic by name_lc

  out = []
  i = 0
  while i < len(norm):
    name = norm[i].name
    if name == "set-cookie":
      ; keep each line
      while i < len(norm) and norm[i].name == name:
        out.append((name, norm[i].value))
        i += 1
      continue

    ; merge group
    merged = norm[i].value
    i += 1
    while i < len(norm) and norm[i].name == name:
      merged = merged + ", " + norm[i].value
      i += 1
    out.append((name, merged))

  return encode_headers_table_v1(out)
```

---

## Where this logic should live

To keep agents from re-implementing tricky RFC details:

* **OS adapter (preferred):** the run‑os HTTP builtin should return `HttpRespV1` where `headers_bytes` are *already canonicalized* with the above algorithm.
* **Stdlib fallback (optional):** provide `std.net.http.headers.merge_rfc9110_v1(raw_headers_table)` so agent code can sanitize raw header tables from any source (but ideally it never needs to).

This way:

* the encoding is stable and “guess-free” for agents,
* the header semantics match RFC 9110’s guidance,
* `set-cookie` is safely handled as multi-valued.

[1]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
[2]: https://datatracker.ietf.org/doc/html/rfc6265 "
            
                RFC 6265 - HTTP State Management Mechanism
            
        "
++++++
Below is a **normative, agent‑friendly** spec for `HttpRespV1` that mirrors `HttpReqV1` (fixed header + length‑delimited payloads), plus the **exact accessor/helper functions** you should ship so agents never do manual offset slicing.

I’m also including the **deterministic header merge/canonicalization rule** that follows RFC 9110’s combined-field-value guidance, with the **`set-cookie` multi-valued exception** (RFC 9110 explicitly calls it out, and the cookie spec forbids combining it). ([RFC Editor][1])

---

## HttpRespV1 bytes encoding (normative)

### Overview

`HttpRespV1` is a single `bytes` blob:

* First **32 bytes** are a fixed header (`Header32`)
* Then **0–2 payload regions** (headers table, body bytes), depending on `tag`

This makes parsing **O(1)** for offsets and lengths and keeps agent code dead simple.

### Header32 layout (exact)

All integer fields are **u32 little‑endian** (but represented as `i32` in X07; treat values as unsigned).

| Offset | Size | Field              | Meaning                                                                              |
| -----: | ---: | ------------------ | ------------------------------------------------------------------------------------ |
|      0 |    4 | `magic`            | ASCII bytes: `X7HR`                                                                  |
|      4 |    4 | `version`          | `1`                                                                                  |
|      8 |    4 | `tag`              | `0=ERR`, `1=OK_BYTES`, `2=OK_STREAM`                                                 |
|     12 |    4 | `status`           | HTTP status (100–599) if OK; `0` if ERR                                              |
|     16 |    4 | `err_code`         | `0` if OK; non‑zero if ERR                                                           |
|     20 |    4 | `headers_len`      | byte length of `HeadersTableV1` blob                                                 |
|     24 |    4 | `body_len_or_hint` | if OK_BYTES: body length; if OK_STREAM: body length hint or `0xFFFF_FFFF`; if ERR: 0 |
|     28 |    4 | `body_aux`         | if OK_STREAM: `stream_handle`; else 0                                                |

### Tag meanings and payload layout

#### tag = 0 (ERR)

* Header must satisfy:

  * `status == 0`
  * `err_code != 0`
  * `headers_len == 0`
  * `body_len_or_hint == 0`
  * `body_aux == 0`
* Payload: **none**
* Total doc length: **exactly 32 bytes**

`err_code` is a **deterministic numeric code** coming from the OS adapter / policy layer (timeout, TLS verify fail, policy denied, etc.).

#### tag = 1 (OK_BYTES)

* Header must satisfy:

  * `err_code == 0`
  * `headers_len` arbitrary (can be 0, but usually >0)
  * `body_len_or_hint` is the actual body byte length (can be 0)
  * `body_aux == 0`
* Payload layout:

  * `headers_blob` at `[32 .. 32+headers_len)`
  * `body_blob` at `[32+headers_len .. 32+headers_len+body_len_or_hint)`
* Total doc length: **exactly** `32 + headers_len + body_len_or_hint`

#### tag = 2 (OK_STREAM)

* Header must satisfy:

  * `err_code == 0`
  * `headers_len` arbitrary (can be 0)
  * `body_len_or_hint` is a hint (or `0xFFFF_FFFF` if unknown)
  * `body_aux` is a nonzero `stream_handle`
* Payload layout:

  * `headers_blob` at `[32 .. 32+headers_len)`
  * **no body bytes** in this doc
* Total doc length: **exactly** `32 + headers_len`

> Rationale: make “small control doc + streaming reader” possible without forcing giant captures.

---

## Headers canonicalization rule (deterministic, RFC-aligned)

This rule produces a deterministic `HeadersTableV1` from the OS/network stack.

### Required semantics

HTTP defines that repeated field names can be treated as a single “combined field value” by joining with commas, *but `Set-Cookie` is a well-known exception that must not be combined.* ([RFC Editor][1])

### Canonicalization algorithm (exact)

Input: list of raw `(name_bytes, value_bytes)` pairs from the OS adapter.

1. **Name normalization**

   * Convert header name to ASCII lowercase (`A–Z` → `a–z`).
   * Reject (drop) any name containing bytes outside visible ASCII token set (you can define “token” strictly if you want); dropped headers should increment a debug counter (optional).

2. **Value trimming**

   * Trim leading and trailing OWS (`SP` and `TAB`) from each field line value.

3. **Group by name**

   * Maintain per-name lists of values in the **original arrival order**.

4. **Merge duplicates**

   * If `name == "set-cookie"`: **do not merge**

     * Emit **one table row per value**, in original order.
   * Else: **merge into one row**

     * Join values with **`, `** (comma + single space), preserving original order of values in the group.
     * This matches the combined-field-value semantics and RFC guidance for recombining. ([RFC Editor][1])

5. **Deterministic row ordering**

   * Stable-sort rows by `name` lexicographically ascending.
   * Because it is stable, multiple `set-cookie` rows preserve arrival order.

This gives you:

* deterministic output across platforms/runtimes,
* semantics aligned with HTTP combined-field-value rules,
* correct `set-cookie` handling. ([IETF Datatracker][2])

---

## Helper functions to ship so agents never slice offsets

These are the **exact helper surfaces** I recommend adding to your `std.net.http.spec` module (or `std.net.http.resp` sub-namespace), so agents never compute offsets or lengths manually.

I’m naming them in the style you’ve been using: `std.net.http.spec.<thing>.<op>_v1`.

### A. Response “shape” helpers

All of these must be **total** (never trap). On malformed docs, they return safe defaults.

* `std.net.http.spec.resp.check_v1(doc: bytes) -> i32`

  * Returns `0` if valid `HttpRespV1`.
  * Returns a deterministic nonzero `SPEC_ERR_*` if invalid (bad magic, truncated, length mismatch, invalid headers blob, invalid tag invariants, etc.).
  * Agents should **always** call this once before accessing fields.

* `std.net.http.spec.resp.tag_v1(doc: bytes) -> i32`

  * Returns `0/1/2` if valid, else `0`.

* `std.net.http.spec.resp.is_err_v1(doc: bytes) -> i32`

  * `1` iff `check_v1==0` and `tag==0`, else `0`.

* `std.net.http.spec.resp.is_ok_bytes_v1(doc: bytes) -> i32`

  * `1` iff `check_v1==0` and `tag==1`, else `0`.

* `std.net.http.spec.resp.is_ok_stream_v1(doc: bytes) -> i32`

  * `1` iff `check_v1==0` and `tag==2`, else `0`.

### B. Scalar accessors

* `std.net.http.spec.resp.status_v1(doc: bytes) -> i32`

  * If `OK_*`: HTTP status
  * Else: `0`

* `std.net.http.spec.resp.err_code_v1(doc: bytes) -> i32`

  * If `ERR`: error code
  * If `OK_*`: `0`
  * If malformed: return a “bad doc” spec error code (or 0; but I strongly prefer a dedicated code so agents can surface “bad response doc” vs “HTTP error”).

* `std.net.http.spec.resp.body_len_hint_v1(doc: bytes) -> i32`

  * If `OK_BYTES`: exact body length
  * If `OK_STREAM`: hint or `0xFFFF_FFFF`
  * Else: `0`

* `std.net.http.spec.resp.stream_handle_v1(doc: bytes) -> i32`

  * If `OK_STREAM`: nonzero handle
  * Else: `0`

### C. Zero-copy payload accessors (preferred)

These return **views** so agents don’t accidentally copy large bodies/headers.

* `std.net.http.spec.resp.headers_view_v1(doc: bytes) -> bytes_view`

  * View over the embedded `HeadersTableV1` blob (or empty view).

* `std.net.http.spec.resp.body_view_v1(doc: bytes) -> bytes_view`

  * If `OK_BYTES`: view over embedded body bytes.
  * Else: empty view.

### D. Copying payload accessors (only if needed)

These copy into new owning `bytes`. (Useful if you need to store beyond a parent buffer’s lifetime, but most agent code shouldn’t.)

* `std.net.http.spec.resp.headers_bytes_v1(doc: bytes) -> bytes`
* `std.net.http.spec.resp.body_bytes_v1(doc: bytes) -> bytes`

### E. Header lookup helpers (so agents never parse `HeadersTableV1`)

These should internally:

1. extract the `HeadersTableV1` view, then
2. do name normalization, then
3. lookup/scan rows.

Minimal high-impact set:

* `std.net.http.spec.resp.header_get_v1(doc: bytes, name: bytes) -> bytes_view`

  * Returns the merged header value for `name` (case-insensitive lookup).
  * If missing: empty view.
  * If `name == "set-cookie"`: returns **the first** value (document this explicitly).

* `std.net.http.spec.resp.header_values_v1(doc: bytes, name: bytes) -> bytes`

  * Returns a `BytesListV1` of **all** values for that header name.
  * For ordinary headers after canonicalization: list has 0 or 1 elements.
  * For `set-cookie`: list can have multiple values.

If you already have a “bytes list” encoding elsewhere, reuse it. If not, define:

**BytesListV1 encoding (simple and deterministic)**

* `magic "X7BL"` (4)
* `version u32=1`
* `count u32`
* for each item: `len u32` + `bytes[len]`

### F. “One-liner” safe consumption helpers (very agent-friendly)

These reduce the common “check + branch + pick bytes” boilerplate:

* `std.net.http.spec.resp.ok_body_or_err_v1(doc: bytes) -> result_bytes`

  * `Ok(body_bytes_view_as_bytes_copy_or_view)` if `OK_BYTES`
  * `Err(err_code)` if `ERR`
  * `Err(SPEC_ERR_NOT_OK_BYTES)` if `OK_STREAM` (forces agent to handle streaming explicitly)
  * `Err(SPEC_ERR_BAD_DOC)` if malformed

* `std.net.http.spec.resp.ok_stream_or_err_v1(doc: bytes) -> result_i32`

  * `Ok(stream_handle)` if `OK_STREAM`
  * `Err(err_code)` if `ERR`
  * `Err(SPEC_ERR_NOT_STREAM)` if `OK_BYTES`
  * `Err(SPEC_ERR_BAD_DOC)` if malformed

These are *tiny*, but they massively reduce failure modes for autonomous agents.

---

## Why this matches RFC behavior (and why `set-cookie` is special)

* RFC 9110 defines “combined field value” behavior for repeated header names, and allows recombining repeated field lines by concatenating values with commas. ([RFC Editor][1])
* RFC 9110 explicitly notes the well-known exception: `Set-Cookie` cannot be combined; recipients should treat it specially. ([IETF Datatracker][2])
* The cookie spec (6265bis draft) is explicit that origin servers/intermediaries **must not combine** multiple `Set-Cookie` fields because commas are meaningful inside cookie syntax. ([IETF HTTP Working Group][3])

So your deterministic merge rule (“merge duplicates with comma+SP, except set-cookie”) is the least surprising and most interoperable.

---

## If you want one extra “agent-proofing” tweak

Add exactly one more helper:

* `std.net.http.spec.resp.debug_summary_v1(doc: bytes) -> bytes`

It returns a short, deterministic ASCII summary like:
`"ok_bytes status=200 hdrs=17 body=1024"` or `"err code=…"`.

This is **extremely useful** for autonomous agents to print diagnostics without learning encodings.

---

[1]: https://www.rfc-editor.org/rfc/rfc9110.html?utm_source=chatgpt.com "RFC 9110: HTTP Semantics"
[2]: https://datatracker.ietf.org/doc/html/rfc9110?utm_source=chatgpt.com "RFC 9110 - HTTP Semantics"
[3]: https://httpwg.org/http-extensions/draft-ietf-httpbis-rfc6265bis.html?utm_source=chatgpt.com "Cookies: HTTP State Management Mechanism"
++++
Below is a concrete, “freeze-able” v1 contract that (a) keeps **spec-layer** errors disjoint from OS/network/TLS errors, and (b) standardizes a **small, agent-friendly HeadersTableV1 API** so agents never slice offsets manually.

I’m also aligning the header behavior with RFC semantics: **field names are case-insensitive** ([RFC Editor][1]), repeated fields combine with comma separation ([RFC Editor][1]), but **Set-Cookie must not be folded** ([IETF Datatracker][2]).

---

## (a) `SPEC_ERR_*` numeric space (never overlaps OS/net error codes)

### Design goal

You have two “error producers”:

1. **OS/network adapter** (curl/openssl/OS): returns small integer error codes (e.g., libcurl `CURLE_*` codes start at 0 and then 1, …) ([Curl][3])
2. **Spec/encoding layer** (your pack/unpack helpers): should return **codes that can never collide** with (1), and stay stable forever.

### Normative rule (v1)

* **All OS/net/TLS/runtime adapter error codes MUST be `< SPEC_ERR_BASE`.**
* **All spec/encoding/validation errors MUST be `>= SPEC_ERR_BASE` and `< SPEC_ERR_LIMIT`.**

### Exact numeric range

* `SPEC_ERR_BASE = 2_000_000_000`
* `SPEC_ERR_LIMIT = 2_000_100_000`
  (100k codes reserved; all are `< 2_147_483_647`, so they remain representable as *positive* i32 decimals in X07.)

### Subranges (fixed)

Within `[2_000_000_000, 2_000_100_000)` reserve deterministic blocks:

| Range (inclusive start) | Range (exclusive end) | Owner                             |
| ----------------------: | --------------------: | --------------------------------- |
|           2_000_000_000 |         2_000_010_000 | Generic spec parsing errors       |
|           2_000_010_000 |         2_000_020_000 | `HeadersTableV1` errors           |
|           2_000_020_000 |         2_000_030_000 | `HttpReqV1` errors                |
|           2_000_030_000 |         2_000_040_000 | `HttpRespV1` errors               |
|           2_000_040_000 |         2_000_050_000 | `NetAddrV1`/`NetCapsV1` errors    |
|           2_000_050_000 |         2_000_100_000 | Reserved for future v1 spec types |

### Exact codes to standardize now (minimal but sufficient)

**Generic (2_000_000_000 block):**

* `SPEC_ERR_TRUNCATED = 2_000_000_001`
* `SPEC_ERR_BAD_MAGIC = 2_000_000_002`
* `SPEC_ERR_BAD_VERSION = 2_000_000_003`
* `SPEC_ERR_OVERFLOW = 2_000_000_004` (length sum overflow, count overflow, etc.)
* `SPEC_ERR_LIMIT_EXCEEDED = 2_000_000_005` (max headers bytes, max body bytes, etc.)

**HeadersTableV1 (2_000_010_000 block):**

* `SPEC_ERR_HEADERS_TRUNCATED = 2_000_010_001`
* `SPEC_ERR_HEADERS_BAD_MAGIC = 2_000_010_002`
* `SPEC_ERR_HEADERS_BAD_VERSION = 2_000_010_003`
* `SPEC_ERR_HEADERS_BAD_NAME = 2_000_010_004` (name not RFC token / not lowercase canonical form)
* `SPEC_ERR_HEADERS_NOT_SORTED = 2_000_010_005`
* `SPEC_ERR_HEADERS_DUP_KEY = 2_000_010_006` (duplicates present where forbidden by your canonicalizer)
* `SPEC_ERR_HEADERS_DUP_KEY_SET_COOKIE_ONLY = 2_000_010_007` (duplicates exist for a name other than `set-cookie`)

**HttpRespV1 (2_000_030_000 block):**

* `SPEC_ERR_RESP_TRUNCATED = 2_000_030_001`
* `SPEC_ERR_RESP_BAD_MAGIC = 2_000_030_002`
* `SPEC_ERR_RESP_BAD_VERSION = 2_000_030_003`
* `SPEC_ERR_RESP_BAD_STATUS = 2_000_030_004`
* `SPEC_ERR_RESP_HEADERS_INVALID = 2_000_030_005` (headers blob fails `HeadersTableV1` validation)

> Enforcement hook (strongly recommended): in the OS adapter / runner boundary, if an OS backend produces an error code `>= SPEC_ERR_BASE`, **remap** it to a deterministic adapter error (e.g. `NET_ERR_INTERNAL_RANGE_VIOLATION`) and include the original in an “opaque debug” field in `run-os` only. This guarantees the “never overlaps” invariant even if a backend library changes.

---

## (b) Minimal standardized `HeadersTableV1` helper functions

### Why a dedicated helper surface?

Because RFC semantics are subtle:

* header names are **case-insensitive** ([RFC Editor][1])
* repeated fields can be “combined” as comma-separated field-values ([RFC Editor][1])
* but **Set-Cookie** is a special case and should **not** be folded/combined ([IETF Datatracker][2])

So agents should never implement header merging / lookups themselves.

### Normative `HeadersTableV1` invariant

A `HeadersTableV1` blob MUST already be canonicalized:

1. **Name canonical form**: ASCII-lowercased field-name bytes (since case-insensitive) ([RFC Editor][1])
2. **Key order**: rows sorted lexicographically by `name_bytes` (bytewise).
3. **Duplicate rule**:

   * for all names **except** `set-cookie`, duplicates are **merged** into a single row (see merge algo below)
   * for `set-cookie`, duplicates are preserved as multiple rows (multi-valued) ([IETF Datatracker][2])

### Deterministic header merge algorithm (canonicalizer)

Input: list of `(name, value)` header lines in the order the backend provided.

1. Normalize `name_norm = ascii_lowercase(name)` (per RFC case-insensitivity) ([RFC Editor][1])
2. Group by `name_norm`. Preserve **value order within each group**.
3. For each group:

   * if `name_norm == "set-cookie"`: emit **one row per value** (no folding) ([IETF Datatracker][2])
   * else: emit **one row** with:

     * `value = join(values, ", ")` (comma + SP), consistent with the “combined field value” comma concatenation model ([RFC Editor][1])
4. Sort groups by `name_norm` ascending; emit rows.

This yields stable encodings and minimizes agent ambiguity.

---

## `HeadersTableV1` helper API v1 (exact names + behavior)

Put these under a single namespace (example):

* `std.net.http.headers.*`

### 1) `headers.count_v1`

**Signature**

* `std.net.http.headers.count_v1(headers_table: bytes) -> i32`

**Semantics**

* Returns number of rows `n`.
* If `headers_table` is malformed, returns `0` (and callers should treat this as “no headers”).

### 2) `headers.name_at_v1`

**Signature**

* `std.net.http.headers.name_at_v1(headers_table: bytes, idx: i32) -> bytes_view`

**Semantics**

* Returns view of the `idx`-th row name (0-based).
* If `idx < 0` or `idx >= count`, returns empty view.

### 3) `headers.value_at_v1`

**Signature**

* `std.net.http.headers.value_at_v1(headers_table: bytes, idx: i32) -> bytes_view`

**Semantics**

* Returns view of the `idx`-th row value (0-based).
* If OOB, returns empty view.

### 4) `headers.get_v1`

**Signature**

* `std.net.http.headers.get_v1(headers_table: bytes, name: bytes) -> bytes_view`

**Semantics**

* Canonicalizes `name` by ASCII-lowercasing for lookup.
* Returns:

  * empty view if missing
  * otherwise, the (single) value row for that name
* Note: because duplicates are merged for all names except `set-cookie`, this is the canonical “normal header read”.

### 5) `headers.values_v1`

**Signature**

* `std.net.http.headers.values_v1(headers_table: bytes, name: bytes) -> bytes`

**Semantics**

* Returns a **new `HeadersTableV1` blob** containing *only* rows whose `name == name_norm` (post lowercase).

  * This keeps API small: you can reuse `count_v1/name_at_v1/value_at_v1` to iterate values.
* For most headers, this returns either:

  * `n=0` (missing), or
  * `n=1` (merged value)
* For `set-cookie`, this can return `n >= 0` (multi-valued).

> This design avoids introducing a second “values list” encoding just to iterate multi-values: agents keep using the same 5 helpers.

---

## Implementation notes that make agents more reliable

Even though you didn’t ask, these matter for “100% agentic” robustness:

1. **Make all helpers total** (never trap) and return “empty/0” on malformed bytes.
   Agent failure mode becomes “missing header” rather than “crash”.

2. **Expose one optional validator** (not required by your “minimal set”, but extremely useful):

   * `std.net.http.headers.check_v1(headers_table) -> i32` (1 ok / 0 bad)
   * If bad, expose a deterministic spec error:

     * `std.net.http.headers.err_code_v1(headers_table) -> i32` (one of `SPEC_ERR_HEADERS_*`)
       This is gold for automatic repair loops.

3. **Don’t allow duplicate-key states to exist** in the builder API:

   * your earlier `headers_set_v1` (“no duplicate keys”) builder is the right default
   * keep `set-cookie` as the one controlled exception

---

[1]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
[2]: https://datatracker.ietf.org/doc/html/rfc6265 "
            
                RFC 6265 - HTTP State Management Mechanism
            
        "
[3]: https://curl.se/libcurl/c/libcurl-errors.html "libcurl - Error Codes"
++++
Below is a **normative v1** encoding for `HeadersTableV1`, plus **agent-friendly pack/unpack contracts** that return `result_bytes` and use your `SPEC_ERR_HEADERS_*` space.

I’m explicitly aligning the “combine duplicates” rule with RFC 9110’s definition of the **combined field value** (comma-separated concatenation in order) ([RFC Editor][1]) and preserving **`set-cookie`** as a special multi-valued exception (must not be folded because commas have meaning inside Set-Cookie) ([IETF Datatracker][2]). Header names are **case-insensitive** in HTTP ([RFC Editor][1]), so we canonicalize names to ASCII-lowercase in the binary format.

---

# HeadersTableV1 binary encoding (X7HT/1)

## Byte order and scalars

* All integers are **unsigned u32 little-endian** (`u32_le`).
* All byte arrays are **length-prefixed** with `u32_le`.

## Top-level layout

`HeadersTableV1` is a single `bytes` blob:

```
offset  size  field
0       4     magic         = b"X7HT"  (0x45 0x56 0x48 0x54)
4       4     version_u32   = 1
8       4     flags_u32     = 0   (MUST be 0 in v1)
12      4     row_count_u32 = N
16      ...   rows[N]
```

### Row layout

Each row encodes one header field entry:

```
name_len_u32
name_bytes[name_len_u32]
value_len_u32
value_bytes[value_len_u32]
```

So, in bytes:

```
rows := for i in 0..N-1:
          u32_le(name_len)
          name_len bytes (name)
          u32_le(value_len)
          value_len bytes (value)
```

## Canonical invariants (MUST hold for HeadersTableV1)

These invariants are what make the format “agent-safe” and prevent bespoke parsing logic.

### 1) Canonical field-name form

Each `name_bytes` MUST be:

* ASCII-lowercase, because HTTP field names are case-insensitive ([RFC Editor][1])
* Non-empty (`name_len >= 1`)
* A conservative “token-like” subset:

  * allowed bytes: `a-z`, `0-9`, and `-` (hyphen)
  * (You can widen this later if needed, but v1 should be strict to avoid agent ambiguity.)

If any row violates this, the table is invalid.

### 2) Sorted by name (deterministic ordering)

Rows MUST be sorted by `name_bytes` ascending, using lexicographic byte ordering.

This means for consecutive rows `i` and `i+1`:

```
name[i] <= name[i+1]
```

### 3) Duplicate-name rule (merge policy baked into canonical form)

* For any name other than `set-cookie`:

  * duplicates MUST NOT exist in `HeadersTableV1`.
* For `set-cookie` only:

  * duplicates ARE allowed (multi-valued).
  * their relative order MUST reflect original arrival order (stable within the `set-cookie` group).

This follows HTTP guidance:

* recipients may combine repeated fields into a comma-separated combined field value ([RFC Editor][1])
* but Set-Cookie must not be folded/combined because comma has conflicting semantics ([IETF Datatracker][2])

### 4) Size limits (deterministic safety caps)

These are hard-coded v1 validation limits (pick values you’re comfortable pinning):

* `MAX_ROWS_V1 = 1024`
* `MAX_NAME_LEN_V1 = 64`
* `MAX_VALUE_LEN_V1 = 64 * 1024` (64KB)
* `MAX_TOTAL_BYTES_V1 = 256 * 1024` (256KB)

If any limit is exceeded, reject.

> Note: NetCaps can further cap `max_header_bytes`, but **this format’s own limits** are still important so malformed blobs can’t cause pathological allocations.

---

# Error codes used by headers helpers (SPEC_ERR_HEADERS_*)

These live in your reserved block `2_000_010_000 .. 2_000_020_000`. (You already have most of these; I’m adding a couple strictly necessary ones for flags/limits.)

* `SPEC_ERR_HEADERS_TRUNCATED = 2_000_010_001`
* `SPEC_ERR_HEADERS_BAD_MAGIC = 2_000_010_002`
* `SPEC_ERR_HEADERS_BAD_VERSION = 2_000_010_003`
* `SPEC_ERR_HEADERS_BAD_NAME = 2_000_010_004`
* `SPEC_ERR_HEADERS_NOT_SORTED = 2_000_010_005`
* `SPEC_ERR_HEADERS_DUP_KEY = 2_000_010_006`
* `SPEC_ERR_HEADERS_DUP_KEY_SET_COOKIE_ONLY = 2_000_010_007`
* `SPEC_ERR_HEADERS_BAD_FLAGS = 2_000_010_008`
* `SPEC_ERR_HEADERS_TOO_MANY_ROWS = 2_000_010_009`
* `SPEC_ERR_HEADERS_TOO_LARGE = 2_000_010_010`

---

# Pack/Unpack helpers (contracts)

These helpers ensure agents **never hand-roll** X7HT bytes and never guess how to canonicalize.

## Pack input: HeadersLinesV1 (X7HL/1)

To make `pack_v1` meaningful, it takes a “raw lines” encoding that allows duplicates and preserves input order.

`HeadersLinesV1` layout is identical to X7HT except magic differs and canonical invariants do not apply:

```
magic         = b"X7HL"
version_u32   = 1
flags_u32     = 0
row_count_u32 = N
rows[N]       = (name_len, name, value_len, value) repeated
```

Rules for X7HL:

* names may be mixed case; `pack_v1` lowercases them
* duplicates allowed
* order preserved (used to preserve stable value ordering)

## `std.net.http.headers.pack_v1`

**Signature**

* `std.net.http.headers.pack_v1(lines: bytes) -> result_bytes`

**Meaning**

* Parses `HeadersLinesV1` (`X7HL/1`).
* Validates basic shape (lengths, truncation, version/flags).
* Canonicalizes into `HeadersTableV1` (`X7HT/1`) using this deterministic algorithm:

### Canonicalization algorithm (normative)

Given X7HL rows in order `[(name_i, value_i)]`:

1. Normalize each header name:

   * `name_norm = ascii_lowercase(name_i)`
   * Validate `name_norm` matches v1 allowed set (`[a-z0-9-]+`), else `SPEC_ERR_HEADERS_BAD_NAME`.

2. Group rows by `name_norm`, preserving original order within each group.

3. For each group:

   * if `name_norm == b"set-cookie"`:

     * emit one X7HT row per value, in the same order (no merging) ([IETF Datatracker][2])
   * else:

     * merge values into a single value by concatenating with a comma byte `0x2C` between field-line values, in order:

       * `merged = v0 || "," || v1 || "," || ...`
     * This matches RFC 9110’s combined field value rule (comma-separated concatenation in order). ([RFC Editor][1])

4. Sort the resulting rows by `name_norm` ascending (lexicographic bytes).

5. Encode as X7HT/1 with `flags=0`.

6. Enforce X7HT limits (`MAX_*_V1`); if exceeded, `SPEC_ERR_HEADERS_TOO_LARGE` or `SPEC_ERR_HEADERS_TOO_MANY_ROWS`.

**Return**

* `Ok(headers_table_bytes)` on success
* `Err(SPEC_ERR_HEADERS_*)` on failure

## `std.net.http.headers.unpack_v1`

**Signature**

* `std.net.http.headers.unpack_v1(table: bytes) -> result_bytes`

**Meaning**

* Parses `HeadersTableV1` (`X7HT/1`) and strictly validates:

  * magic/version/flags
  * exact length (no truncation / no extra)
  * name validity
  * sorted order
  * duplicates rule (only `set-cookie` may repeat)
  * X7HT size limits

**Return**

* `Ok(table)` if valid (returns the same bytes)
* `Err(SPEC_ERR_HEADERS_*)` if invalid

> Important: `unpack_v1` is a **validator**. It does not “fix” the table. If you need canonicalization, go through `pack_v1` (X7HL → X7HT).

---

# Practical guidance for agents

* When building requests: construct X7HL using the provided builder helpers (your `headers_set_v1`, `headers_push_v1`, etc.), then call `headers.pack_v1` once to get X7HT.
* When reading responses: assume OS adapters provide X7HT already; use:

  * `headers.unpack_v1(resp_headers)` only as a debug/assert step
  * otherwise use the `HeadersTableV1` accessor helpers (`headers.get_v1`, `headers.values_v1`, etc.) so agents never touch offsets.

[1]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
[2]: https://datatracker.ietf.org/doc/html/rfc6265 "
            
                RFC 6265 - HTTP State Management Mechanism
            
        "
+++++
Below is the **normative “accessor semantics”** for `HeadersTableV1` (**X7HT/1**) in terms of the **row encoding** you pinned earlier, written so `headers.get_v1` can legitimately be implemented as a **binary search** (because the table is **sorted by canonical header name**).

This is meant to be **deterministic, total (never traps), and zero‑copy** where possible.

---

## Assumptions and invariants

These accessor semantics assume the input table is a **canonical X7HT/1** table produced by `headers.pack_v1` or validated/canonicalized by `headers.unpack_v1`.

Canonicalization aligns with HTTP guidance that header field names are **case-insensitive**, and that multiple field lines can be “combined” into one “combined field value” using commas **for fields where this is valid**, while `set-cookie` is a widely‑recognized exception (it must not be folded/combined because commas are meaningful in `Set-Cookie`). ([RFC Editor][1])

### X7HT/1 encoding recap (already pinned)

* `table` is a `bytes` blob:

  * `magic: 4 bytes` = ASCII `"X7HT"`
  * `version: u32_le` = `1`
  * `flags: u32_le` = `0`
  * `count: u32_le` = `N`
  * then `N` rows:

    * `name_len: u32_le`
    * `name_bytes[name_len]` (canonical header name)
    * `value_len: u32_le`
    * `value_bytes[value_len]`

### Canonical row constraints used by accessors

1. **Names are canonical**:

   * ASCII lowercase.
   * Must match your header-name validity rule (your chosen subset, or the full RFC token subset if you adopted it).
   * **Query names are normalized the same way** before lookup.
   * HTTP field names are case-insensitive. ([RFC Editor][1])
2. **Sorted by name**:

   * Rows are sorted ascending by `name_bytes` using **bytewise lexicographic order**.
   * All rows with identical `name_bytes` are **contiguous**.
3. **No duplicates**, except:

   * `name_bytes == b"set-cookie"` may appear **multiple times** (contiguous), preserving original order among those rows (stable tie-breaker).
   * This matches the general RFC guidance about combined field values vs. Set‑Cookie folding. ([RFC Editor][1])
4. **Structural validity**:

   * Length fields must not run past the end of `table`.
   * Counts/lengths must respect whatever caps you pinned for v1 (e.g., max rows, max name/value bytes). If you didn’t pin caps yet, define them once and reuse in pack/unpack + accessors.

### Totality rule (very important for agents)

All accessors below MUST be **non-throwing** / non-trapping. If the table is not valid X7HT/1, they behave as if the table had **zero rows**:

* `count_v1` returns `0`
* `name_at_v1` / `value_at_v1` return empty `bytes_view`
* `get_v1` returns empty `bytes_view`
* `values_v1` returns a valid empty X7HT/1 table (`count=0`)

This prevents “agent crashes” from malformed intermediate blobs.

---

## Shared helper definitions

### Canonical name normalization

**normalize_name_v1(name_bytes) -> (ok, canon_name_bytes_view_or_bytes)**

* Input: `name_bytes` (bytes or bytes_view).
* Output:

  * If every byte is allowed in your field-name charset:

    * lowercase ASCII A–Z → a–z
    * keep other allowed bytes unchanged
    * return `ok=1` + canonical name (view if no changes needed, otherwise a new bytes)
  * else:

    * return `ok=0`

> Note: RFC 9110’s registry guidance historically restricts header names to letters/digits/hyphen; if you’re enforcing a stricter subset for simplicity, that’s defensible. ([RFC Editor][1])

### Lexicographic compare

**cmp_bytes_lex(a_ptr,a_len, b_ptr,b_len) -> i32**

* Compare `a` and `b` byte-by-byte.
* Return:

  * `-1` if `a < b`
  * `0` if equal
  * `+1` if `a > b`
* If one is a prefix of the other, shorter is smaller.

This is the *sole* ordering used for sorting and for binary search comparisons.

---

## Row indexing (key to binary search)

Because rows are variable-length, random access to “row i” requires an offset index.

### build_row_offsets_v1(table) -> (ok, N, offsets[])

This helper is internal (not part of API) but defines the semantics you should implement.

* Parse the 16-byte header:

  * Validate magic/version/flags.
  * Read `N`.
  * If `N` exceeds cap → invalid.
* Allocate an offsets array of length `N+1` of u32 (or i32), where:

  * `offsets[i]` = byte offset (from table start) of row `i`’s `name_len` field
  * `offsets[N]` = end offset (first byte after final row)
* Fill offsets by **single linear scan**:

  * `p = 16`
  * For `i in 0..N-1`:

    * `offsets[i] = p`
    * Read `name_len` at `p`; advance `p`
    * Ensure `p + name_len <= table_len`; advance `p`
    * Read `value_len`; advance `p`
    * Ensure `p + value_len <= table_len`; advance `p`
  * `offsets[N] = p`
  * Require `p == table_len` **or** allow trailing bytes?

    * **Recommendation**: require `p == table_len` for canonical X7HT. If you allow trailing, specify it clearly; otherwise reject.
* While scanning, also enforce:

  * name validity (canonical already)
  * sortedness:

    * Compare each name to previous name; must be `>=` (non-decreasing)
    * If equal:

      * name must be `set-cookie` (exact canonical bytes)
      * and stability among equals is implicit (keep original order; scanning order is the stored order)

If any check fails → `ok=0`.

---

## Public accessor API semantics

These are the semantics you pin for `std.net.http.headers.*` helpers.

### 1) `headers.count_v1(table: bytes) -> i32`

**Semantics**

* Run `build_row_offsets_v1(table)`.
* If `ok=0`: return `0`.
* Else return `N` as i32.

**Rationale**

* This makes `for i 0 (headers.count_v1 t)` safe: if the table is truncated, you won’t get a bogus count and then crash on `name_at/value_at`.

---

### 2) `headers.name_at_v1(table: bytes, idx: i32) -> bytes_view`

**Semantics**

* If `idx < 0`: return empty view.
* Run `build_row_offsets_v1(table)`.
* If invalid: empty view.
* If `idx >= N`: empty view.
* Let `p = offsets[idx]`.
* Parse the row at `p`:

  * Read `name_len`, then `name_bytes`.
* Return a `bytes_view` referencing `table[name_start .. name_start+name_len]`.

**Guarantees**

* Returned view is **zero-copy** and points into the original `table`.
* It remains valid as long as `table` remains live.

---

### 3) `headers.value_at_v1(table: bytes, idx: i32) -> bytes_view`

**Semantics**

* Same indexing/validation rules as `name_at_v1`.
* Parse row at `offsets[idx]`, return view into its `value_bytes`.

---

### 4) `headers.get_v1(table: bytes, name: bytes) -> bytes_view`

Returns the **single effective value** for a header name.

**Semantics**

1. Normalize query:

   * `(ok, canon_name) = normalize_name_v1(name)`
   * If `ok=0`: return empty view
2. Parse table:

   * `(ok, N, offsets) = build_row_offsets_v1(table)`
   * If `ok=0`: return empty view
3. Binary search (lower_bound):

   * Find smallest index `lo` such that `row_name(lo) >= canon_name`
   * Use `cmp_bytes_lex(row_name_bytes, canon_name_bytes)` for comparisons.
4. If `lo == N`: not found → empty view.
5. If `row_name(lo) != canon_name`: not found → empty view.
6. If `canon_name == b"set-cookie"`:

   * Return **the value of the first matching row** (`value_at(lo)`).
   * (To get all cookies, use `values_v1`.)
7. Else (non-set-cookie):

   * Canonical X7HT guarantees at most one row for this name, so return `value_at(lo)`.

**Why this is RFC-aligned**

* RFC 9110 describes how multiple header field lines can be interpreted as a “combined field value” for many fields. Your canonicalization step is where that combining would happen; X7HT then stores a single row. ([RFC Editor][1])
* Set-Cookie folding is discouraged because commas are meaningful in the header value. ([IETF Datatracker][2])

---

### 5) `headers.values_v1(table: bytes, name: bytes) -> bytes`

Returns **all rows** for a header name as a **new X7HT/1 table** (typically used for `set-cookie`).

**Semantics**

1. Normalize name; if invalid → return empty X7HT (count=0).
2. Validate+index table via `build_row_offsets_v1`; if invalid → empty X7HT.
3. Use binary search to find range:

   * `first = lower_bound(name)`
   * `last  = upper_bound(name)` where upper_bound finds first index with `row_name > name`
4. If no matches → return empty X7HT.
5. Else:

   * Build a new X7HT table with:

     * same magic/version/flags
     * count = `last-first`
     * rows copied in order `i=first..last-1`:

       * copy `name_len + name_bytes + value_len + value_bytes` exactly
   * (You may either recompute name_len/value_len while copying or copy the raw fields; both are deterministic.)

**Canonicality of the returned table**

* It remains sorted (all names identical).
* For non-set-cookie names, it will have 0 or 1 row.
* For set-cookie, it may have >1 row and preserves order.

---

## Notes for implementers (stdlib authors), not agents

### Binary search implementation detail

The **semantic requirement** is that `get_v1` behaves like lower_bound on the sorted row names. The efficient, deterministic way is:

* Build `offsets[]` in one scan (`O(total_bytes)`), then binary search (`O(log N)` comparisons).

This avoids `O(N log N)` rescanning to reach midpoints.

### Constant-time comparison?

Not needed for header lookup (not a cryptographic secret). For crypto contexts you’d add dedicated constant-time helpers.

---

## Minimal “empty X7HT” constructor

Because accessors return empty X7HT when invalid/missing, define:

* `headers.empty_v1() -> bytes` = the 16-byte header with `count=0` and no rows.

This makes `values_v1` total and easy for agents to consume (they can safely loop `0..count_v1(values)`).

---

[1]: https://www.rfc-editor.org/rfc/rfc9110.html "RFC 9110: HTTP Semantics"
[2]: https://datatracker.ietf.org/doc/html/rfc6265 "
            
                RFC 6265 - HTTP State Management Mechanism
            
        "
++++Below is a **single pinned doc** you can drop in as `docs/net/headers-table-v1.md`, plus a **small internal helper module plan** centered around `std.net.http.headers._evht`.

---

## `docs/net/headers-table-v1.md`

```md
# HeadersTableV1 (X7HT/1) — Canonical HTTP header table encoding + accessors

Status: **Normative (v1 pinned)**  
Applies to: `std.net.http.headers` v1 helpers (pack/unpack + accessors)  
Primary goal: make header handling **deterministic**, **binary-searchable**, and **agent-friendly**
(agents should never hand-slice offsets).

This document defines:

1. The X7HT/1 bytes encoding for a header table (“HeadersTableV1”)
2. Canonicalization invariants (sorting, dedupe/merge rules)
3. Accessor semantics (total/non-trapping) enabling `O(log N)` lookup

---

## 1. Background / standards alignment (informative)

HTTP field names are **case-insensitive**, so X7HT canonicalizes names to a single stable form (lowercase).  
When a field name repeats, HTTP defines a combined field value as the ordered list of values joined by commas.
(We adopt this as the canonical merge rule for most headers.)  
`Set-Cookie` is a special case: servers should not fold multiple `Set-Cookie` field lines into one because commas
have semantics inside cookie syntax; X7HT preserves `set-cookie` as multi-valued.

---

## 2. X7HT/1 bytes encoding (normative)

All integers are unsigned **u32 little-endian** unless stated otherwise.

### 2.1. Top-level layout

`HeadersTableV1` is a single `bytes` blob:

```

offset  size   field
0       4      magic = ASCII "X7HT"
4       4      version = 1
8       4      flags = 0 (reserved; must be 0 in v1)
12      4      count = N (number of rows)
16      ...    rows[0..N-1]

```

### 2.2. Row encoding (repeated N times)

Each row is:

```

name_len   : u32
name_bytes : [name_len] bytes   // canonical header name (lowercase ASCII)
value_len  : u32
value_bytes: [value_len] bytes  // header value bytes (raw; see canonicalization rules below)

```

Constraints (v1):
- `magic` MUST match exactly.
- `version` MUST be 1.
- `flags` MUST be 0.
- `count` MUST be <= MAX_ROWS (implementation-defined cap; recommended 4096).
- Each row MUST fit fully within the blob (no overrun).
- `name_len` MUST be > 0 and <= MAX_NAME_BYTES (recommended 256).
- `value_len` MUST be <= MAX_VALUE_BYTES (recommended 1 MiB).
- Total blob MUST be <= MAX_TABLE_BYTES (implementation-defined cap; recommended 4 MiB).

(If your implementation chooses different caps, pin them in `docs/net/net-v1.md` once and reuse consistently.)

---

## 3. Canonicalization invariants (normative)

A canonical X7HT/1 blob MUST satisfy all of the following:

### 3.1. Name canonicalization

- Header names are stored as **ASCII lowercase**.
- Query names passed to accessors are normalized the same way.
- Name validity: `name_bytes` MUST satisfy your `token` subset for field names.
  Recommended minimal set for v1: ASCII letters/digits + `-` (and optionally `.`).
  (Exact charset is an implementation choice; it must be deterministic and pinned.)

### 3.2. Sorting

Rows MUST be sorted ascending by `name_bytes` using **lexicographic byte order**:
- Compare byte-by-byte; first differing byte determines ordering.
- If one is a prefix of the other, shorter is smaller.

All rows with the same `name_bytes` MUST be contiguous.

### 3.3. Duplicates & merge policy

Canonical X7HT/1 forbids duplicate names **except** for `set-cookie`.

- For `name != "set-cookie"`:
  - the table MUST contain at most one row for that name.
  - if duplicates occur at input, pack/canonicalize MUST merge them into one value
    using the “combined field value” rule (Section 3.4).

- For `name == "set-cookie"`:
  - multiple rows are allowed (multi-valued).
  - rows MUST remain contiguous.
  - relative order among `set-cookie` rows is preserved (stable).

### 3.4. Deterministic combined field value (merge algorithm)

When pack/canonicalize merges multiple values for the same name (except `set-cookie`), it MUST:

- preserve the original order of occurrences
- join values with the delimiter `", "` (comma + single SP)
- treat each occurrence’s `value_bytes` as opaque bytes (no trimming/OWS rewrite in v1)

So if values are `[v0, v1, v2]`, merged value is:
`v0 + b", " + v1 + b", " + v2`

(If you later add OWS trimming/canonicalization, that must be a v2 or a pinned flags bit.)

---

## 4. Accessor API semantics (normative)

All accessors are **total** and MUST NOT trap.
If `table` is not a valid canonical X7HT/1 blob, it is treated as an **empty table**.

This is a deliberate LLM/agent ergonomics rule:
malformed intermediate blobs should degrade safely, not crash the program.

### 4.1. Helper definitions used below

**normalize_name_v1(name_bytes) -> (ok, canon_name)**
- If `name_bytes` contains any invalid bytes per your name charset: return `ok=0`.
- Else return `ok=1` and `canon_name` where ASCII `A..Z` have been converted to `a..z`.
- If no bytes change, `canon_name` MAY be a view into the original input; otherwise it is new bytes.

**cmp_lex(a, b) -> {-1,0,+1}**
- Lexicographic byte compare of two byte sequences.

**lower_bound(name)**
- Smallest index `i` such that `row_name(i) >= name` under `cmp_lex`.
- Returns `N` if no such index.

**upper_bound(name)**
- Smallest index `i` such that `row_name(i) > name` under `cmp_lex`.
- Returns `N` if no such index.

Binary-search correctness depends on the sorting invariant (Section 3.2).

### 4.2. `headers.empty_v1() -> bytes`

Returns a valid empty X7HT/1 table:

- magic="X7HT"
- version=1
- flags=0
- count=0
- no rows

### 4.3. `headers.count_v1(table: bytes) -> i32`

- If `table` is invalid: return `0`.
- Else return `N` (row count).

### 4.4. `headers.name_at_v1(table: bytes, idx: i32) -> bytes_view`

- If `idx < 0`: empty view.
- If `table` invalid: empty view.
- If `idx >= N`: empty view.
- Else return a **view into the table** referencing the `name_bytes` of row `idx`.

### 4.5. `headers.value_at_v1(table: bytes, idx: i32) -> bytes_view`

Same indexing rules as `name_at_v1`, returning a view to the row’s `value_bytes`.

### 4.6. `headers.get_v1(table: bytes, name: bytes) -> bytes_view`

Returns the **single effective value** for `name`:

1. `(ok, canon_name) = normalize_name_v1(name)`
   - if `ok=0`: return empty view
2. If `table` invalid: return empty view
3. `i = lower_bound(canon_name)`
4. If `i == N` or `row_name(i) != canon_name`: return empty view
5. If `canon_name == b"set-cookie"`:
   - return `value_at_v1(table, i)` (the first cookie line)
   - (to get all cookie lines, use `headers.values_v1`)
6. Else:
   - return `value_at_v1(table, i)` (canonical table guarantees at most one)

### 4.7. `headers.values_v1(table: bytes, name: bytes) -> bytes`

Returns **all values** for `name` as a new X7HT/1 table:

1. Normalize `name`. If invalid -> return `headers.empty_v1()`.
2. If table invalid -> return `headers.empty_v1()`.
3. Find range:
   - `first = lower_bound(canon_name)`
   - `last  = upper_bound(canon_name)`
4. If no matches: return `headers.empty_v1()`.
5. Build a new X7HT/1 table containing rows `[first, last)` copied in order.

For non-`set-cookie` headers, `[first,last)` will be length 0 or 1.
For `set-cookie`, it may contain multiple rows, preserving their order.

---

## 5. Internal implementation guidance: `_evht` helper module (non-normative)

To avoid duplicating parsing/search logic in many helpers, implement a small internal module:

Module ID: `std.net.http.headers._evht`  
Visibility: internal (not listed in the public guide; not re-exported)

### 5.1. Offsets index encoding (OffsetsV1)

Because X7HT rows are variable-length, binary search needs an index.

Define an internal offsets blob:

```

OffsetsV1:
n: u32_le
offsets[0..n]: (n+1) u32_le entries

```

Where:
- `offsets[i]` = byte offset (from table start) of row i’s `name_len` field
- `offsets[n]` = end offset (first byte after last row)

### 5.2. Suggested internal helper functions

All helpers are total; on invalid input they behave as if `n=0`.

- `_evht.build_row_offsets_v1(table: bytes) -> bytes`
  - returns `OffsetsV1` blob, or empty bytes on invalid table

- `_evht.row_name_view_v1(table: bytes, off: bytes, idx: i32) -> bytes_view`
  - returns empty view on invalid/out-of-range

- `_evht.row_value_view_v1(table: bytes, off: bytes, idx: i32) -> bytes_view`
  - returns empty view on invalid/out-of-range

- `_evht.lower_bound_v1(table: bytes, off: bytes, canon_name: bytes) -> i32`
  - returns index in `[0..n]`

- `_evht.upper_bound_v1(table: bytes, off: bytes, canon_name: bytes) -> i32`
  - returns index in `[0..n]`

### 5.3. How public accessors should use `_evht`

Example (`headers.get_v1`):

1. normalize query name
2. `off = _evht.build_row_offsets_v1(table)`
3. `i = _evht.lower_bound_v1(table, off, canon_name)`
4. compare `row_name_view(i)` to `canon_name`
5. return `row_value_view(i)` or empty

This makes `get_v1` `O(total_table_bytes + log N)` worst-case per call, but with small header counts it is fast.
If you later need repeated lookup speed, you can expose a *cached index* API, but v1 keeps the surface minimal.

---

## 6. Versioning / future changes (normative)

- Any breaking change to:
  - the on-wire encoding
  - sorting/merge invariants
  - accessor totality behavior
  MUST increment `version` and/or introduce a new magic.

v1 reserves `flags` for future compatible behavior toggles; in v1 flags MUST be 0.
```

### Normative reference points used for the spec (for your reviewers)

* HTTP field names are case-insensitive, and combined field value is defined by concatenating repeated field line values separated by commas. ([RFC Editor][1])
* `Set-Cookie` should **not** be folded into a single comma-joined field line. ([IETF Datatracker][2])

---

## Small internal helper module plan (repo-aligned)

Here’s the “small helper module” plan in the repo style you’ve been using:

**Add internal module file**

* `stdlib/std/<ver>/modules/std/net/http/headers/_evht/module.x07.json`

  * Exports: none (internal)
  * Provides:

    * `std.net.http.headers._evht.build_row_offsets_v1`
    * `std.net.http.headers._evht.row_name_view_v1`
    * `std.net.http.headers._evht.row_value_view_v1`
    * `std.net.http.headers._evht.lower_bound_v1`
    * `std.net.http.headers._evht.upper_bound_v1`

**Update public headers module**

* `stdlib/std/<ver>/modules/std/net/http/headers/module.x07.json`

  * Public exports:

    * `headers.empty_v1`
    * `headers.count_v1`
    * `headers.name_at_v1`
    * `headers.value_at_v1`
    * `headers.get_v1`
    * `headers.values_v1`
    * plus your already-pinned `headers.pack_v1 / headers.unpack_v1`
  * Implementation should delegate parsing/search to `_evht` to keep the public module small and consistent.

**Test plan**

* Keep your existing pack/unpack + “no duplicate keys” / “combine duplicates except set-cookie” tests.
* Add 1–2 microtests:

  * `get_v1` on non-existent header returns empty view
  * `values_v1` on `set-cookie` returns X7HT with 2 rows, stable order
  * (Optional) randomized insertion order → `get_v1` still works due to sorting.

---
Here’s a drop‑in DB v1 bundle (docs + run‑os policy schema patch + package skeletons + smoke suite JSON shapes + a deterministic SQLite fixture DB) aligned to a **DataModel‑first** design.

Use docs/phases/assets/x07_ext_db_v1_bundle.tar.gz.

### What’s inside

* `docs/db/`

  * `db-v1.md` — pinned design: **DataModel params + DataModel result docs** + `DbRespV1 (X7DB/1)` envelope
  * `sqlite-v1.md` — prepared statements plan (prepare/bind/step/finalize)
  * `postgres-v1.md` — `PQexecParams` parameterized execution + TLS requirements (`sslmode=verify-full/verify-ca`)
  * `mysql-v1.md` — connector notes; MariaDB Connector/C options path (`mysql_optionsv`) + TLS references
* `schemas/`

  * `run-os-policy.db.section.json` — **DB policy schema fragment**
  * `run-os-policy.db.patch.json` — RFC6902 JSON Patch to add `/properties/db` to your existing policy schema
  * `run-os-policy.db.example.json` — minimal allowlist example
* `packages/`

  * `packages/x07-ext-db-core/0.1.0/`

    * `std.db.spec` **implemented**: X7DB/1 envelope builders + accessors
    * `std.db.params` **implemented**: DataModel params builders (empty / one value / one string / one number / null / bool) using `ext.data_model`
    * `std.db` facade **stubbed** (returns NOT_IMPLEMENTED until native adapters exist)
  * driver packages (`x07-ext-db-sqlite/postgres/mysql`) are skeleton stubs for now
* `benchmarks/`

  * `run-os-sandboxed/db-sqlite-smoke.json`
  * `run-os/db-pg-smoke.json` (env-gated via `X07_PG_TEST_DSN`)
  * `run-os/db-mysql-smoke.json` (env-gated via `X07_MYSQL_TEST_DSN`)
  * `fixtures/db/sqlite/v1/app.db` — deterministic SQLite file
* `tests/external_os/...` smoke program stubs (placeholders)

SQLite fixture + licensing note: SQLite is public domain, which is why it’s ideal for the “always-on deterministic smoke” tier.

### How to apply it to your repo

1. Untar and copy folders into your repo root.
2. Apply the schema patch:

   * merge `schemas/run-os-policy.db.section.json` under your policy schema’s `properties.db`, **or**
   * apply `schemas/run-os-policy.db.patch.json` with your existing patch tooling.
3. Pin packages as you do today (your `stdlib.lock` / package lock process).

### What you still need to implement (next PRs)

* Native adapters in `run-os/run-os-sandboxed`:

  * SQLite: prepared statements (`sqlite3_prepare_v2`, `sqlite3_bind_*`, `sqlite3_step`, `sqlite3_finalize`)
  * Postgres: use `PQexecParams` (parameterized `$1..$n`) + enforce TLS via `sslmode=verify-full`/`verify-ca`
  * MySQL: prepared statements + TLS via connector options (MariaDB Connector/C documents `mysql_optionsv`)
* Wire your native layer to replace the `std.db` facade stubs (right now they intentionally return NOT_IMPLEMENTED so the package can land without introducing unknown builtins).
