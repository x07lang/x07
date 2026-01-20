# Guide: Networking (HTTP/TLS)

Networking is provided by external packages and only works in OS worlds (`run-os` / `run-os-sandboxed`).

Packages:

- `ext-net` provides the agent-facing `std.net.*` modules (HTTP/TLS/DNS/TCP/UDP).
- `ext-curl-c` provides the `ext.curl.http` HTTP client used by `std.net.http.client`.
- `ext-sockets-c` provides the `ext.sockets.net` sockets adapter used by `std.net.tcp` / `std.net.udp`.
- `ext-openssl-c` provides TLS support used by `std.net.tls`.

X07 does not resolve transitive package dependencies yet, so add all required packages explicitly.

To discover available packages and versions, use the index catalog:

- https://registry.x07.io/index/catalog.json

## Canonical approach

- Always construct requests through pack/unpack helpers:
  - `NetAddrV1`, `NetCapsV1`, `HttpReqV1`, `HttpRespV1`
- Never hand-roll bytes encodings.
- Always run OS networking in:
  - `run-os` for trusted environments
  - `run-os-sandboxed` with explicit policy allowlists

## Quick start

1. Add packages to `x07.json`:

   ```bash
   x07 pkg add ext-net@<version>
   x07 pkg add ext-curl-c@<version>
   x07 pkg add ext-sockets-c@<version>
   x07 pkg add ext-openssl-c@<version>
   ```

2. Lock and fetch deps:

   ```bash
   x07 pkg lock
   ```

3. Run in an OS world (required). By default, `x07 run` enables automatic FFI wiring for OS worlds:

   ```bash
   x07 run --project x07.json --world run-os --input input.bin
   ```

System prerequisites depend on platform (for example, `libcurl` + `openssl` dev packages on Linux; Homebrew `openssl@3` on macOS).

## Deterministic tests

Networking logic should be tested through:

- fixture cassettes (request/response replay), or
- pure request/response transforms.
