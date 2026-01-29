# Networking error docs v1 (`std.net.err.*`)

This document pins the **v1 stable error doc format + codes** used by OS-world networking backends (currently `ext-curl-c` and `ext-sockets-c`) and surfaced via `ext-net` (`std.net.*`).

For the full networking bytes ABIs (TCP/UDP/DNS docs, NetAddrV1, NetCapsV1), see [Networking bytes ABIs v1](net-v1.md).

## Error doc binary layout

`NetErrDocV1` is a `bytes` blob:

- `u8 tag` (always `0` for error)
- `u32_le code` (one of the codes below)
- `u32_le reserved` (currently `0`)

`ext-curl-c` emits this layout; `ext-net` forwards it unchanged.

## Error codes (v1)

These codes are exposed as constants in `packages/ext/x07-ext-net/0.1.5/modules/std/net/err.x07.json`:

- `1` `std.net.err.code_policy_denied_v1`
- `2` `std.net.err.code_invalid_req_v1`
- `3` `std.net.err.code_timeout_v1`
- `4` `std.net.err.code_too_large_v1`
- `5` `std.net.err.code_dns_v1`
- `6` `std.net.err.code_connect_v1`
- `7` `std.net.err.code_tls_v1`
- `8` `std.net.err.code_internal_v1`

## Agent-friendly helpers (`std.net.err`)

- Build an error doc: `std.net.err.make_doc_v1(code)`
- Detect an error doc: `std.net.err.is_err_doc_v1(doc)`
- Read the code: `std.net.err.err_code_v1(doc)`

## Agent-friendly handling pattern (HTTP)

- Detect error: `std.net.http.resp_is_err_v1(resp_doc)`
- Read code: `std.net.http.resp_err_code_v1(resp_doc)`
- Compare against `std.net.err.*` constants to branch deterministically.
