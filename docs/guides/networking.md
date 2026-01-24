# Guide: Networking (HTTP/TLS)

Networking is provided by external packages and only works in OS worlds (`run-os` / `run-os-sandboxed`).

## Canonical (template + x07 run)

Start from the HTTP client template (recommended):

```bash
x07 init --template http-client
```

Before using OS networking, verify native prerequisites (C compiler + curl/openssl linkability):

```bash
x07up doctor --json
```

Sandboxed run (deny-by-default + explicit allowlist):

```bash
x07 run --profile sandbox --allow-host example.com:443 -- <your-cli-args...>
```

Trusted environment (unsandboxed):

```bash
x07 run --profile os -- <your-cli-args...>
```

## Packages

Main package:

- `ext-net` provides the agent-facing `std.net.*` modules (HTTP/TLS/DNS/TCP/UDP).

`ext-net` requires several official helper packages:

  - `ext-curl-c` provides the `ext.curl.http` HTTP client used by `std.net.http.client`.
  - `ext-sockets-c` provides the `ext.sockets.net` sockets adapter used by `std.net.tcp` / `std.net.udp` / `std.net.tls` (links against system OpenSSL).
  - `ext-url-rs` provides HTTP parsing helpers used by `std.net.http.server`.

Some published package versions may omit `meta.requires_packages`, so do not rely on transitive auto-add for correctness. Use the capability map (or `x07 init --template http-client`) so the full canonical set is explicit.

To discover available packages and versions, use the registry catalog:

- https://registry.x07.io/index/catalog.json

## Canonical approach

- Always construct requests through pack/unpack helpers:
  - `NetAddrV1`, `NetCapsV1`, `HttpReqV1`, `HttpRespV1`
- Never hand-roll bytes encodings.
- Always run OS networking in:
  - `run-os` for trusted environments
  - `run-os-sandboxed` with explicit policy allowlists

## Quick start

1. Run in an OS world (required). By default, `x07 run` enables automatic FFI wiring for OS worlds:

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

## Expert appendix

- Add packages manually (advanced): pick NAME@VERSION from `/agent/latest/catalog/capabilities.json` and run `x07 pkg add NAME@VERSION --sync`.
- Debug runner behavior directly:
  - solve worlds: `x07-host-runner --project x07.json`
  - OS worlds: `x07-os-runner --project x07.json --world run-os`
