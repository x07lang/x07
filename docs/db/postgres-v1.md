# Postgres adapter v1 — request blobs (`X7PO`/`X7PQ`/`X7PE`/`X7PC`)

Status: **implemented** in this repo via `os.db.pg.*` (native backend: `crates/x07-ext-db-pg-native/`).  
Worlds: **`run-os`**, **`run-os-sandboxed`**.

This document pins the **Postgres v1 request formats** used by `std.db.pg`.

Shared contracts:

- `X7DB` responses and `X7DC` caps: [DB v1](db-v1.md)
- pool bytes and tokens: [std.db.pool v1](pool-v1.md) (for `std.db.pg.pool`)

## Builtins required (runner/toolchain)

Only available in standalone worlds (`run-os`, `run-os-sandboxed`):

- `os.db.pg.open_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.pg.query_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.pg.exec_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.pg.close_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)

The X07 wrappers are in:

- `packages/ext/x07-ext-db-postgres/0.1.3/modules/std/db/pg.x07.json`
- `packages/ext/x07-ext-db-postgres/0.1.3/modules/std/db/pg/spec.x07.json`

## `PgOpenReqV1` (`X7PO`)

Produced by: `std.db.pg.spec.open_req_v1(host, port, user, pass, db, flags) -> bytes`

All integers are **u32 little-endian**.

- offset `0`: magic = `"X7PO"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `flags` (reserved; use `std.db.pg.spec.flags_none_v1()`)
- offset `12`: `host_len`
- offset `16`: `host_bytes[host_len]`
- next: `port` (u32; 1..65535)
- next: `user_len`, `user_bytes`
- next: `pass_len`, `pass_bytes`
- next: `db_len`, `db_bytes`

## `PgQueryReqV1` (`X7PQ`)

Produced by: `std.db.pg.spec.query_req_v1(conn_id, sql, params_doc, flags) -> bytes`

- offset `0`: magic = `"X7PQ"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `conn_id`
- offset `12`: `flags` (reserved; use `0` in v1)
- offset `16`: `sql_len`
- offset `20`: `sql_bytes[sql_len]`
- offset `20+sql_len`: `params_doc_len`
- offset `24+sql_len`: `params_doc_bytes[params_doc_len]` (DataModel doc; see [DB v1](db-v1.md))

Parameter placeholder convention: `$1`, `$2`, ...

## `PgExecReqV1` (`X7PE`)

Produced by: `std.db.pg.spec.exec_req_v1(conn_id, sql, params_doc, flags) -> bytes`

Same layout as `X7PQ`, but magic is `"X7PE"`.

## `PgCloseReqV1` (`X7PC`)

Produced by: `std.db.pg.spec.close_req_v1(conn_id) -> bytes`

- offset `0`: magic = `"X7PC"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `conn_id`

## Query result payload (Postgres v1)

`os.db.pg.query_v1` returns `X7DB` OK payload as a DataModel doc (from `ext.data_model`).

Root value is a map:

- `"cols"`: seq of column names (strings)
- `"rows"`: seq of rows, each row is a seq of values (scalars)

### Value mapping

Postgres values are mapped into DataModel scalars:

- `NULL` → `null`
- `BOOL` → `number` (`"1"` or `"0"`)
- `INT2/INT4/INT8` → `number` (decimal ASCII)
- `FLOAT4/FLOAT8` → `number` (decimal ASCII)
- `BYTEA` → `string` (raw bytes)
- other types → `string` (driver text format)

## Exec ok doc (Postgres v1)

`os.db.pg.exec_v1` returns `X7DB` OK payload as a DataModel doc whose root value is a map:

- `"rows_affected"`: number (decimal ASCII)
- `"last_insert_id"`: number (decimal ASCII; always `"0"` in the native backend)

## Policy (run-os-sandboxed)

Sandboxed runs enforce the `db` section of `schemas/run-os-policy.schema.json`.

Minimum policy requirements for Postgres:

- `db.enabled = true`
- `db.drivers.postgres = true`
- `db.net.allow_ports` contains the target port (e.g. `5432`)
- either `db.net.allow_dns` contains the hostname, or `db.net.allow_cidrs` contains the IP range

If you are connecting to local docker fixtures without TLS, set:

- `db.net.require_tls = false`
- `db.net.require_verify = false`
