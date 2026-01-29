# MySQL adapter v1 — request blobs (`X7MO`/`X7MQ`/`X7ME`/`X7MC`)

Status: **implemented** in this repo via `os.db.mysql.*` (native backend: `crates/x07-ext-db-mysql-native/`).  
Worlds: **`run-os`**, **`run-os-sandboxed`**.

This document pins the **MySQL v1 request formats** used by `std.db.mysql`.

Shared contracts:

- `X7DB` responses and `X7DC` caps: [DB v1](db-v1.md)
- pool bytes and tokens: [std.db.pool v1](pool-v1.md) (for `std.db.mysql.pool`)

## Builtins required (runner/toolchain)

Only available in standalone worlds (`run-os`, `run-os-sandboxed`):

- `os.db.mysql.open_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.mysql.query_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.mysql.exec_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.mysql.close_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)

The X07 wrappers are in:

- `packages/ext/x07-ext-db-mysql/0.1.3/modules/std/db/mysql.x07.json`
- `packages/ext/x07-ext-db-mysql/0.1.3/modules/std/db/mysql/spec.x07.json`

## `MySqlOpenReqV1` (`X7MO`)

Produced by: `std.db.mysql.spec.open_req_v1(host, port, user, pass, db, flags) -> bytes`

All integers are **u32 little-endian**.

- offset `0`: magic = `"X7MO"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `flags` (reserved; use `std.db.mysql.spec.flags_none_v1()`)
- offset `12`: `host_len`
- offset `16`: `host_bytes[host_len]`
- next: `port` (u32; 1..65535)
- next: `user_len`, `user_bytes`
- next: `pass_len`, `pass_bytes`
- next: `db_len`, `db_bytes`

## `MySqlQueryReqV1` (`X7MQ`)

Produced by: `std.db.mysql.spec.query_req_v1(conn_id, sql, params_doc, flags) -> bytes`

- offset `0`: magic = `"X7MQ"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `conn_id`
- offset `12`: `flags` (reserved; use `0` in v1)
- offset `16`: `sql_len`
- offset `20`: `sql_bytes[sql_len]`
- offset `20+sql_len`: `params_doc_len`
- offset `24+sql_len`: `params_doc_bytes[params_doc_len]` (DataModel doc; see [DB v1](db-v1.md))

Parameter placeholder convention: `?` (positional).

## `MySqlExecReqV1` (`X7ME`)

Produced by: `std.db.mysql.spec.exec_req_v1(conn_id, sql, params_doc, flags) -> bytes`

Same layout as `X7MQ`, but magic is `"X7ME"`.

## `MySqlCloseReqV1` (`X7MC`)

Produced by: `std.db.mysql.spec.close_req_v1(conn_id) -> bytes`

- offset `0`: magic = `"X7MC"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `conn_id`

## Query result payload (MySQL v1)

`os.db.mysql.query_v1` returns `X7DB` OK payload as a DataModel doc (from `ext.data_model`).

Root value is a map:

- `"cols"`: seq of column names (strings)
- `"rows"`: seq of rows, each row is a seq of values (scalars)

### Value mapping

MySQL values are mapped into DataModel scalars:

- `NULL` → `null`
- integer types → `number` (decimal ASCII)
- float types → `number` (decimal ASCII)
- bytes/text types → `string` (raw bytes)
- other types → `string` (driver SQL literal format)

## Exec ok doc (MySQL v1)

`os.db.mysql.exec_v1` returns `X7DB` OK payload as a DataModel doc whose root value is a map:

- `"rows_affected"`: number (decimal ASCII)
- `"last_insert_id"`: number (decimal ASCII)

## Policy (run-os-sandboxed)

Sandboxed runs enforce the `db` section of `schemas/run-os-policy.schema.json`.

Minimum policy requirements for MySQL:

- `db.enabled = true`
- `db.drivers.mysql = true`
- `db.net.allow_ports` contains the target port (e.g. `3306`)
- either `db.net.allow_dns` contains the hostname, or `db.net.allow_cidrs` contains the IP range

If you are connecting to local docker fixtures without TLS, set:

- `db.net.require_tls = false`
- `db.net.require_verify = false`
