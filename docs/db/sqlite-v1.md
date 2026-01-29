# SQLite adapter v1 — request blobs (`X7SO`/`X7SQ`/`X7SE`/`X7SC`)

Status: **implemented** in this repo via `os.db.sqlite.*` (native backend: `crates/x07-ext-db-sqlite-native/`).  
Worlds: **`run-os`**, **`run-os-sandboxed`**.

This document pins the **SQLite v1 request formats** used by `std.db.sqlite`.

Shared contracts:

- `X7DB` responses and `X7DC` caps: [DB v1](db-v1.md)
- pool bytes and tokens: [std.db.pool v1](pool-v1.md)

## Builtins required (runner/toolchain)

Only available in standalone worlds (`run-os`, `run-os-sandboxed`):

- `os.db.sqlite.open_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.sqlite.query_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.sqlite.exec_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)
- `os.db.sqlite.close_v1(req: bytes, caps: bytes) -> bytes` (`X7DB`)

The X07 wrappers are in:

- `packages/ext/x07-ext-db-sqlite/0.1.6/modules/std/db/sqlite.x07.json`
- `packages/ext/x07-ext-db-sqlite/0.1.6/modules/std/db/sqlite/spec.x07.json`

## `SqliteOpenReqV1` (`X7SO`)

Produced by: `std.db.sqlite.spec.open_req_path_v1(path: bytes, flags: i32) -> bytes`

All integers are **u32 little-endian**.

- offset `0`: magic = `"X7SO"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `flags`
- offset `12`: `path_len`
- offset `16`: `path_bytes[path_len]`

Flags (bitset):

- bit `0`: `open_readonly` (`std.db.sqlite.spec.open_flag_readonly_v1() == 1`)
- bit `1`: `open_create` (`std.db.sqlite.spec.open_flag_create_v1() == 2`)
- all other bits must be `0` in v1

## `SqliteQueryReqV1` (`X7SQ`)

Produced by: `std.db.sqlite.spec.query_req_v1(conn_id, sql, params_doc, flags) -> bytes`

- offset `0`: magic = `"X7SQ"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `conn_id`
- offset `12`: `flags` (reserved; use `0` in v1)
- offset `16`: `sql_len`
- offset `20`: `sql_bytes[sql_len]`
- offset `20+sql_len`: `params_doc_len`
- offset `24+sql_len`: `params_doc_bytes[params_doc_len]` (DataModel doc; see [DB v1](db-v1.md))

## `SqliteExecReqV1` (`X7SE`)

Produced by: `std.db.sqlite.spec.exec_req_v1(conn_id, sql, params_doc, flags) -> bytes`

Same layout as `X7SQ`, but magic is `"X7SE"`.

## `SqliteCloseReqV1` (`X7SC`)

Produced by: `std.db.sqlite.spec.close_req_v1(conn_id) -> bytes`

- offset `0`: magic = `"X7SC"` (4 bytes)
- offset `4`: version = `1`
- offset `8`: `conn_id`

## Query result payload (SQLite v1)

`os.db.sqlite.query_v1` returns `X7DB` OK payload as a DataModel doc (from `ext.data_model`).

Root value is a map:

- `"cols"`: seq of column names (strings)
- `"rows"`: seq of rows, each row is a seq of values (scalars)

### Value mapping

SQLite values are mapped into DataModel scalars:

- `NULL` → `null`
- `INTEGER` → `number` (decimal ASCII)
- `REAL` → `number` (decimal ASCII)
- `TEXT` → `string` (raw UTF-8 bytes; validity is not required by DataModel)
- `BLOB` → `string` (raw bytes)

## Policy (run-os-sandboxed)

Sandboxed runs enforce the `db` section of `schemas/run-os-policy.schema.json`.

For SQLite, the minimum policy requirements are:

- `db.enabled = true`
- `db.drivers.sqlite = true`
- `db.sqlite.allow_paths` contains the DB path you are opening

Example policy used by the smoke suites:

- `tests/external_os/db_smoke_policy/run-os-policy.db_smoke.json`

## Smoke suites (SQLite)

- Runner: `scripts/ci/check_db_smoke.sh`
- Suites: `ci/suites/smoke/db-sqlite-os-sandboxed-smoke.json` and `ci/suites/smoke/db-sqlite-os-sandboxed-policy-deny-smoke.json`
