# Networking bytes ABIs v1 (`std.net.*`)

This document pins the **v1 stable bytes formats** used by the OS-world networking stack:

- Agent-facing modules: `packages/ext/x07-ext-net/0.1.5/modules/std/net/**` (`std.net.*`)
- Backend implementation: `packages/ext/x07-ext-sockets-c/0.1.5/` (`ext.sockets.*`)

All networking is **OS-world-only** (`run-os`, `run-os-sandboxed`). Deterministic `solve-*` worlds must not do real networking.

## Integer encoding

- All multi-byte integers are `u32_le` written/read via `codec.{write,read}_u32_le` (represented as `i32` in x07AST).
- All lengths are byte lengths.

## Error doc: `NetErrDocV1`

Pinned in [Networking error docs v1](errors-v1.md).

```
NetErrDocV1 :=
  tag:u8 (=0)
  code:u32_le
  reserved:u32_le (=0)
```

## Address: `NetAddrV1`

One canonical address encoding for IPv4 / IPv6 / DNS names.

```
NetAddrV1 :=
  ver:u32_le (=1)
  tag:u32_le
    1 = ipv4
    2 = ipv6
    3 = dns
  port:u32_le               ; 0..65535
  payload:
    tag=1: 4 bytes          ; ipv4 a,b,c,d
    tag=2: 16 bytes         ; ipv6
    tag=3:
      name_len:u32_le
      name_bytes[name_len]  ; UTF-8 (usually ASCII)
```

## Caps: `NetCapsV1`

Caps are **per-call** bounds (policy can further restrict in `run-os-sandboxed`).

```
NetCapsV1 :=
  ver:u32_le (=1)
  connect_timeout_ms:u32_le
  io_timeout_ms:u32_le
  max_read_bytes:u32_le
  max_write_bytes:u32_le
  reserved:u32_le (=0)
```

Notes:
- `0` values are treated as “use backend defaults”.
- `max_read_bytes` and `max_write_bytes` are enforced as upper bounds even if the caller passes a larger `max` or payload.

## DNS lookup doc: `DnsLookupDocV1`

Returned by `std.net.dns.lookup_v1`.

```
DnsLookupDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    count:u32_le
    repeated count times:
      addr_len:u32_le
      addr_bytes[addr_len]  ; NetAddrV1 (ip tags only)
```

## TCP docs

### `TcpConnectDocV1`

Returned by `std.net.tcp.connect_v1`.

```
TcpConnectDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    stream_handle:u32_le
```

### `TcpListenDocV1`

Returned by `std.net.tcp.listen_v1`.

```
TcpListenDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    listener_handle:u32_le
    bound_addr_len:u32_le
    bound_addr_bytes[bound_addr_len]  ; NetAddrV1 (ip tags only)
```

### `TcpAcceptDocV1`

Returned by `std.net.tcp.accept_v1`.

```
TcpAcceptDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    stream_handle:u32_le
    peer_addr_len:u32_le
    peer_addr_bytes[peer_addr_len]    ; NetAddrV1 (ip tags only)
```

### `StreamReadDocV1`

Returned by `std.net.tcp.stream_read_v1`.

```
StreamReadDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    payload_len:u32_le
    payload_bytes[payload_len]
```

### `StreamWriteDocV1`

Returned by `std.net.tcp.stream_write_v1`.
Also returned by `std.net.io.write_all_v1` (total bytes written).

```
StreamWriteDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    bytes_written:u32_le
```

### `StreamWaitDocV1`

Returned by `std.net.tcp.stream_wait_v1`.

```
StreamWaitDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    events:u32_le
```

Event bits (v1):
- `1` readable
- `2` writable
- `4` hangup

### Handle lifecycle (non-doc APIs)

These return `i32` status (`1` success, `0` failure/invalid handle):

- `std.net.tcp.stream_shutdown_v1(stream_handle, how)` where `how`: `0` read, `1` write, `2` both
- `std.net.tcp.stream_close_v1(stream_handle)`
- `std.net.tcp.stream_drop_v1(stream_handle)` (idempotent free of the slot)
- `std.net.tcp.listener_close_v1(listener_handle)`
- `std.net.tcp.listener_drop_v1(listener_handle)` (idempotent free of the slot)

## UDP docs

### `UdpBindDocV1`

Returned by `std.net.udp.bind_v1`.

```
UdpBindDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    sock_handle:u32_le
    bound_addr_len:u32_le
    bound_addr_bytes[bound_addr_len]  ; NetAddrV1 (ip tags only)
```

### `UdpSendDocV1`

Returned by `std.net.udp.sendto_v1`.

```
UdpSendDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    bytes_sent:u32_le
```

### `UdpRecvDocV1`

Returned by `std.net.udp.recvfrom_v1`.

```
UdpRecvDocV1 :=
  tag:u8
  if tag=0:
    NetErrDocV1
  else: ; tag=1
    ver:u8 (=1)
    reserved:u16 (=0)
    from_addr_len:u32_le
    from_addr_bytes[from_addr_len]    ; NetAddrV1 (ip tags only)
    payload_len:u32_le
    payload_bytes[payload_len]
```

## TLS (`std.net.tls`)

`std.net.tls` is an OS-world-only TLS client surface implemented by `ext-sockets-c` (OpenSSL-backed).

### TLS client config: `TlsClientCfgV1`

Produced by `std.net.tls.client_cfg_v1`.

```
TlsClientCfgV1 :=
  ver:u32_le (=1)
  verify_peer:u32_le            ; 0 or 1
  sni_len:u32_le                ; 0..1024
  sni_bytes[sni_len]            ; must not contain NUL
  reserved:u32_le (=0)
```

Notes:
- If `tls_cfg` is empty, the backend treats it as “defaults”: `verify_peer=1` and SNI is inferred from `NetAddrV1(dns)`; for IP addresses, pass an explicit SNI or set `verify_peer=0`.

### `std.net.tls.connect_v1` result doc

`std.net.tls.connect_v1(addr, tls_cfg, caps) -> bytes` returns a `TcpConnectDocV1` whose `stream_handle` refers to a TLS stream.
The handle is compatible with `std.net.tcp.stream_{read,write,wait,close,drop}_v1` and `std.net.tcp.stream_reader_v1`.

## `iface` streaming readers (`std.io` / `std.io.bufread` integration)

These helpers expose socket I/O as `iface` readers so agents can plug sockets into `std.io.read` and `std.io.bufread` without manual doc parsing.

### TCP/TLS stream reader: `std.net.tcp.stream_reader_v1`

`std.net.tcp.stream_reader_v1(stream_handle, caps) -> iface` returns a reader where:

- `std.io.read(reader, max) -> bytes` yields raw stream bytes.
- Empty bytes (`len=0`) means EOF (or a timeout/error that forces the stream closed); for structured errors, use doc-based `std.net.tcp.stream_read_v1`.
- Reads are bounded by `NetCapsV1` (`io_timeout_ms`, `max_read_bytes`).

### UDP recv-doc reader: `std.net.udp.recv_doc_reader_v1`

`std.net.udp.recv_doc_reader_v1(sock_handle, caps) -> iface` returns a reader where each `std.io.read(reader, max)` yields exactly one `UdpRecvDocV1` (or a `NetErrDocV1`).

Notes:
- This reader yields whole docs, not raw payload bytes. Use `std.net.udp.recv_payload_v1` / `std.net.udp.recv_from_addr_v1` on the returned doc.
- To allow IPv6 addresses plus payload, ensure `max >= 40 + payload_max` (the IPv6 header is larger than IPv4).
