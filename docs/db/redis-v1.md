# Redis adapter v1 — request blobs (`X7RO`/`X7RQ`/`X7RX` + `X7RV`)

Status: **implemented** in this repo via `os.db.redis.*` (native backend: `crates/x07-ext-db-redis-native/`).  
Worlds: **`run-os`**, **`run-os-sandboxed`**.

This document pins the **Redis v1 request formats** used by `std.db.redis`.

Shared contracts:

- `X7DB` responses and `X7DC` caps: [DB v1](db-v1.md)

## Builtins required (runner/toolchain)

Only available in standalone worlds (`run-os`, `run-os-sandboxed`):

- `os.db.redis.open_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.redis.cmd_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.redis.close_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)

The X07 wrappers are in:

- `packages/ext/x07-ext-db-redis/0.1.3/modules/std/db/redis.x07.json`
- `packages/ext/x07-ext-db-redis/0.1.3/modules/std/db/redis/spec.x07.json`
- `packages/ext/x07-ext-db-redis/0.1.3/modules/std/db/redis/argv.x07.json`

## `RedisOpenReqV1` (`X7RO`)

Produced by:

- `std.db.redis.spec.open_req_tcp_v1(host, port, user, pass, db, flags) -> bytes`
- `std.db.redis.spec.open_req_unix_v1(path, user, pass, db, flags) -> bytes`

The `X7RO` request encodes a tagged union:

- TCP: host/port
- Unix: path

All integers are **u32 little-endian**.

## `RedisCmdReqV1` (`X7RQ`)

Produced by: `std.db.redis.spec.cmd_req_v1(conn_id, argv_table, flags) -> bytes`

`argv_table` is a deterministic bytes table (`X7RV`, see below).

## `RedisCloseReqV1` (`X7RX`)

Produced by: `std.db.redis.spec.close_req_v1(conn_id, flags) -> bytes`

## `RedisArgvTableV1` (`X7RV`)

`std.db.redis.argv` provides helpers to build an argv table as bytes:

- `std.db.redis.argv.empty_v1() -> bytes`
- `std.db.redis.argv.from1_v1(a0) -> bytes`, `from2_v1(a0,a1) -> bytes`, ...
- `std.db.redis.argv.push_v1(argv, arg) -> bytes`

Each arg is raw bytes (UTF-8 recommended; not required).

## Response payload (Redis v1)

`os.db.redis.cmd_v1` returns an `X7DB` response.

On success, the `X7DB` OK payload is a DataModel **doc_ok** whose root value is a single DataModel value:

- `null` / `bool` / `number` / `string`
- `seq` (for arrays/sets)
- `map`

RESP3 mapping used by the native backend:

- RESP3 `null` → DataModel `null`
- RESP3 `bool` → DataModel `bool`
- RESP3 `number` → DataModel `number` (ASCII)
- RESP3 `string` / `blob` → DataModel `string` (raw bytes)
- RESP3 `array` → DataModel `seq`
- RESP3 `map` → DataModel `map` (string/number/bool keys only)
- RESP3 `set` → DataModel `seq` with items sorted (deterministic)

## Policy (run-os-sandboxed)

Sandboxed runs enforce the `db` section of `schemas/run-os-policy.schema.json`.

Minimum policy requirements for Redis TCP:

- `db.enabled = true`
- `db.drivers.redis = true`
- `db.net.allow_ports` contains the target port (e.g. `6379`)
- either `db.net.allow_dns` contains the hostname, or `db.net.allow_cidrs` contains the IP range

If you are connecting to local docker fixtures without TLS, set:

- `db.net.require_tls = false`
- `db.net.require_verify = false`
