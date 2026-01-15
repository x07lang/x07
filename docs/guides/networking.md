# Guide: Networking (HTTP/TLS)

Networking is provided by the external `ext.net` package.

## Canonical approach

- Always construct requests through pack/unpack helpers:
  - `NetAddrV1`, `NetCapsV1`, `HttpReqV1`, `HttpRespV1`
- Never hand-roll bytes encodings.
- Always run OS networking in:
  - `run-os` for trusted environments
  - `run-os-sandboxed` with explicit policy allowlists

## Deterministic tests

Networking logic should be tested through:

- fixture cassettes (request/response replay), or
- pure request/response transforms.
