# DB v1 (external packages) — `X7DB` responses and `X7DC` caps

Status: **implemented** in this repo (SQLite, Postgres, MySQL, Redis).  
Worlds: **`run-os`**, **`run-os-sandboxed`** (never available in `solve-*` worlds).

This document pins the **v1 contract** shared across DB drivers:

- response envelope: `DbRespV1` (`X7DB`)
- per-call caps blob: `DbCapsV1` (`X7DC`)

The driver-specific request formats (`X7SO`, `X7SQ`, …) are documented per driver (for SQLite: [SQLite adapter v1](sqlite-v1.md)).

## Packages (reference)

- Core facade + spec: `packages/ext/x07-ext-db-core/0.1.6/`
- SQLite driver: `packages/ext/x07-ext-db-sqlite/0.1.6/`
- Postgres driver: `packages/ext/x07-ext-db-postgres/0.1.3/`
- MySQL driver: `packages/ext/x07-ext-db-mysql/0.1.3/`
- Redis driver: `packages/ext/x07-ext-db-redis/0.1.3/`

## `DbRespV1` envelope (`X7DB`)

All DB operations return a single `bytes` value encoded as `X7DB`.

All integers are **u32 little-endian**.

### Layout

Header:

- offset `0`: magic = `"X7DB"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: tag (`1` = OK, `0` = ERR)
- offset `12`: op (see operation codes below)

Payload:

- If `tag == 1` (OK):
  - offset `16`: `ok_len`
  - offset `20`: `ok_bytes[ok_len]`
- If `tag == 0` (ERR):
  - offset `16`: `err_code`
  - offset `20`: `msg_len`
  - offset `24`: `msg_bytes[msg_len]`

### Operation codes

Defined by `std.db.spec` (`packages/ext/x07-ext-db-core/0.1.6/modules/std/db/spec.x07.json`):

- `1`: `OPEN` (`std.db.spec.op_open_v1`)
- `2`: `EXEC` (`std.db.spec.op_exec_v1`)
- `3`: `QUERY` (`std.db.spec.op_query_v1`)
- `4`: `CLOSE` (`std.db.spec.op_close_v1`)
- `60`: `POOL_NEW` (`std.db.spec.op_pool_new_v1`)
- `61`: `POOL_CLOSE` (`std.db.spec.op_pool_close_v1`)

### OK payloads (v1)

- `OPEN` (`op=1`): `u32 conn_id` (driver-specific connection handle)
- `EXEC` (`op=2`): DataModel doc (see “Exec ok doc” below)
- `QUERY` (`op=3`): DataModel doc (see “Query rows doc” below)
- `CLOSE` (`op=4`): empty payload
- `POOL_NEW` (`op=60`): `DbPoolV1` bytes (`X7PL`, see [std.db.pool v1](pool-v1.md))
- `POOL_CLOSE` (`op=61`): empty payload

## `DbCapsV1` blob (`X7DC`)

Caps are passed as `bytes` to the OS builtins. The runtime should enforce `min(policy, caps)` (treating `0` as “use policy default”).

All integers are **u32 little-endian**.

### Layout (24 bytes)

- offset `0`: magic = `"X7DC"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `connect_timeout_ms` (0 = policy default)
- offset `12`: `query_timeout_ms` (0 = policy default)
- offset `16`: `max_rows` (0 = policy default)
- offset `20`: `max_resp_bytes` (0 = policy default)

Helpers:

- `std.db.spec.caps_pack_v1(connect_timeout_ms, query_timeout_ms, max_rows, max_resp_bytes) -> bytes`
- `std.db.sqlite.caps_default_v1() -> bytes` (all zeros)

## DataModel docs used by DB v1

DB v1 uses `ext.data_model` docs as payloads for `EXEC` and `QUERY`.

### Params doc (DbParamsV1)

`std.db.exec_v1` / `std.db.query_v1` accept `params_doc: bytes`:

- DataModel `doc_ok`
- root value: sequence
- element values: **scalars only** (`null`, `bool`, `number`, `string`)

Helpers live in `std.db.params` (`packages/ext/x07-ext-db-core/0.1.6/modules/std/db/params.x07.json`), for example:

- `std.db.params.empty_v1() -> bytes`
- `std.db.params.one_null_v1() -> bytes`
- `std.db.params.one_bool_v1(x_i32) -> bytes`
- `std.db.params.one_number_dec_v1(dec_ascii_bytes) -> bytes`
- `std.db.params.one_string_v1(utf8_bytes) -> bytes`

### Query rows doc (DbRowsDocV1)

For SQL drivers in this repo (SQLite/Postgres/MySQL), `QUERY` OK payload is a DataModel `doc_ok` whose root value is a map:

- key `"cols"`: sequence of column names (DataModel strings)
- key `"rows"`: sequence of rows, each row is a sequence of values (DataModel scalars)

See driver docs for exact mapping rules:

- SQLite: [SQLite adapter v1](sqlite-v1.md)
- Postgres: [Postgres adapter v1](postgres-v1.md)
- MySQL: [MySQL adapter v1](mysql-v1.md)

### Exec ok doc (DbExecOkDocV1)

For SQL drivers in this repo (SQLite/Postgres/MySQL), `EXEC` OK payload is a DataModel `doc_ok` whose root value is a map:

- key `"rows_affected"`: DataModel number (decimal ASCII)
- key `"last_insert_id"`: DataModel number (decimal ASCII)

## Error codes

DB v1 common error codes (stable in v1):

- `DB_ERR_POLICY_DENIED = 53249` (`0xD001`)
- `DB_ERR_BAD_REQ       = 53250` (`0xD002`)
- `DB_ERR_BAD_CONN      = 53251` (`0xD003`)
- `DB_ERR_TOO_LARGE     = 53760` (`0xD200`)

SQLite native backend error codes (stable in v1):

- `DB_ERR_SQLITE_OPEN   = 53504` (`0xD100`)
- `DB_ERR_SQLITE_PREP   = 53505` (`0xD101`)
- `DB_ERR_SQLITE_STEP   = 53506` (`0xD102`)

Postgres native backend error codes (stable in v1):

- `DB_ERR_PG_CONNECT = 53520` (`0xD110`)
- `DB_ERR_PG_QUERY   = 53521` (`0xD111`)
- `DB_ERR_PG_EXEC    = 53522` (`0xD112`)
- `DB_ERR_PG_TLS     = 53523` (`0xD113`)

MySQL native backend error codes (stable in v1):

- `DB_ERR_MYSQL_CONNECT = 53536` (`0xD120`)
- `DB_ERR_MYSQL_QUERY   = 53537` (`0xD121`)
- `DB_ERR_MYSQL_EXEC    = 53538` (`0xD122`)
- `DB_ERR_MYSQL_TLS     = 53539` (`0xD123`)

Redis native backend error codes (stable in v1):

- `DB_ERR_REDIS_CONNECT  = 53552` (`0xD130`)
- `DB_ERR_REDIS_CMD      = 53553` (`0xD131`)
- `DB_ERR_REDIS_PROTOCOL = 53554` (`0xD132`)
- `DB_ERR_REDIS_TLS      = 53555` (`0xD133`)
- `DB_ERR_REDIS_SERVER   = 53556` (`0xD134`)

## Accessors (agent-friendly)

Use `std.db.spec` helpers rather than slicing manually:

- `std.db.spec.resp_is_ok_v1(resp: bytes_view) -> i32`
- `std.db.spec.resp_err_code_v1(resp: bytes_view) -> i32`
- `std.db.spec.resp_err_msg_v1(resp: bytes_view) -> bytes`
- `std.db.spec.resp_ok_payload_v1(resp: bytes_view) -> bytes`

In `defasync` contexts, explicitly create views from owned `bytes` identifiers (`bytes.view`) before calling `bytes_view` APIs.
