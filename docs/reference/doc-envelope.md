# Doc Envelope Protocol

The X07 ecosystem commonly represents fallible outcomes as **bytes** that wrap either:

- an ok payload (bytes)
- an error payload (bytes)

This document defines the canonical **doc envelope** format so stdlib/ext packages can interoperate.

## Format

A doc envelope is a `bytes` value with the following layout:

- `tag` (1 byte)
  - `1` = ok
  - `0` = err
- `payload` (0 or more bytes)
  - ok payload: interpretation is defined by the producing API
  - err payload: interpretation is defined by the producing API, but see the recommended error payload below

Doc envelopes are **not** self-describing beyond the tag byte. The caller must know which API produced the doc and how to interpret its payload.

## Recommended Error Payload (v1)

For error docs (`tag=0`), stdlib uses this stable payload encoding:

- `code_u32le` (4 bytes, little-endian)
- `msg_len_u32le` (4 bytes, little-endian)
- `msg_bytes` (`msg_len_u32le` bytes, UTF-8 text or empty)

This encoding is used by helpers in `std.doc`:

- `std.doc.err_from_code_v1(code) -> bytes`
- `std.doc.err_v1(msg) -> bytes` (uses `code=0`)
- `std.doc.err_from_code_msg_v1(code, msg) -> bytes`
- `std.doc.error_code_v1(doc) -> i32`
- `std.doc.error_msg_v1(doc) -> bytes`

## Helpers

Use `std.doc` for construction and inspection:

- `std.doc.ok_v1(payload: bytes_view) -> bytes`
- `std.doc.is_ok_v1(doc: bytes_view) -> i32`
- `std.doc.payload_v1(doc: bytes_view) -> bytes`
- `std.doc.err_payload_v1(doc: bytes_view) -> bytes`

## Propagation Sugar: `try_doc`

The language provides `["try_doc", expr]` for doc envelopes:

- `expr` must evaluate to `bytes` or `bytes_view` representing a doc envelope
- if ok (`tag=1`): yields the payload as `bytes`
- if err (`tag=0` or invalid): returns the original doc from the enclosing `defn`

The enclosing `defn` return type must be `bytes` (the doc envelope shape).

