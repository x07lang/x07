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

Before using OS networking, verify native prerequisites (C compiler + curl/openssl linkability):

```bash
x07 doctor
```

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
   # If your project defines profiles (recommended):
   x07 run --profile os -- <your-cli-args...>

   # Advanced: provide input bytes directly (for fixtures)
   x07 run --profile os --input input.bin

   # Otherwise (explicit world selection):
   x07 run --project x07.json --world run-os -- <your-cli-args...>
   ```

System prerequisites depend on platform (for example, `libcurl` + `openssl` dev packages on Linux; Homebrew `openssl@3` on macOS).

## Sandboxed networking (explicit allowlists)

1. Generate a base policy (deny-by-default destinations):

   ```bash
   x07 policy init --template http-client
   ```

   Treat this as a starting point: review and edit the base policy for your project (filesystem roots, limits, env, etc.).

2. Run with explicit destinations (materializes a derived policy under `.x07/policies/_generated/`):

   ```bash
   x07 run --world run-os-sandboxed \
     --policy .x07/policies/base/http-client.sandbox.base.policy.json \
     --allow-host example.com:443 \
     -- <your-cli-args...>
   ```

If you edit a net-enabled base policy manually, keep `net.allow_dns: true` (required by the curl shim) and use `--allow-host` / `--deny-host` for destinations.

## Deterministic tests

Networking logic should be tested through:

- fixture cassettes (request/response replay), or
- pure request/response transforms.

See `examples/agent-gate/web-crawler-local` in the `x07` repo for an end-to-end sandboxed crawler that runs against a local fixture site (no public internet).
