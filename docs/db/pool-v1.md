# `std.db.pool` v1 (`X7PL`)

This document pins the **v1** bytes encoding and intended usage pattern for database connection pools in X07.

The goals are:

- deterministic, agent-friendly handle passing (no hand-rolled binary slicing),
- compatibility with the existing DB envelope (`X7DB`) used by `std.db.*`,
- minimal surface area in v1.

## Design constraints

- Blocking scheduler boundaries (e.g. `chan.bytes.recv`, `chan.bytes.send`) are only allowed in `solve` or `defasync`.
- Therefore, pool acquire/release is expressed as direct channel operations plus small unpack helpers.

## Pool bytes encoding: `DbPoolV1` (`X7PL`)

A pool value is an opaque bytes blob that encodes:

- the underlying `chan.bytes.*` handle,
- the configured maximum number of connections (for deterministic draining/close).

All integers are **u32 little-endian**.

```
DbPoolV1 := {
  magic[4]      = b"X7PL"
  version_u32   = 1
  chan_u32      = <chan.bytes handle>
  max_conns_u32 = <max number of connections in pool>
}
```

Size: **16 bytes**.

### Helper accessors (stdlib)

- `std.db.pool.pack_v1(chan_i32, max_conns_i32) -> bytes`
- `std.db.pool.chan_v1(pool_bytes_view) -> i32`
- `std.db.pool.max_conns_v1(pool_bytes_view) -> i32`

## Connection token encoding: `DbConnTokenV1`

Pools are implemented as a `chan.bytes` whose messages are fixed-size **4 byte** tokens:

```
DbConnTokenV1 := u32_le(conn_id)
```

### Helper accessors (stdlib)

- `std.db.pool.conn_token_v1(conn_id_i32) -> bytes`
- `std.db.pool.conn_from_token_v1(token_bytes_view) -> i32`

## Canonical acquire/release pattern

Given `pool : bytes` (`X7PL`):

Acquire a connection:

1. `ch = std.db.pool.chan_v1(bytes.view(pool))`
2. `tok = chan.bytes.recv(ch)` (blocking; yield boundary)
3. `conn = std.db.pool.conn_from_token_v1(tok)` (token can be passed directly as `bytes_view`)

Release the same connection:

1. `chan.bytes.send(ch, tok)` (blocking; yield boundary)

Important: do not repack a token; send back the exact `tok` you received.

## Closing/draining a pool

The v1 recommended close algorithm is:

1. Receive exactly `max_conns` tokens from the pool channel.
2. Close each connection deterministically using the driver’s close function.
3. `chan.bytes.close(ch)`.

This ensures:

- you fail loudly (block) if a connection is leaked (never returned),
- you deterministically close all connections when everything is correct.

