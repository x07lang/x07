# X07 DB v1 (external packages) — implemented

Worlds: `run-os`, `run-os-sandboxed` (never available in `solve-*` worlds).

Normative contracts: `docs/db/db-v1.md` and driver docs under `docs/db/`.

This phase is implemented as **external packages** (under `packages/ext/`) plus native DB backends built as static libraries and linked into compiled programs (via staged artifacts under `deps/x07/`).

## What’s in the repo

- **Core package (driver-neutral facade):** `packages/ext/x07-ext-db-core/0.1.0/`
  - Modules: `std.db`, `std.db.spec`, `std.db.params`, `std.db.pool`, `std.db.dm`
- **SQLite package:** `packages/ext/x07-ext-db-sqlite/0.1.0/`
  - Modules: `std.db.sqlite`, `std.db.sqlite.spec`, `std.db.sqlite.pool`
- **Postgres package:** `packages/ext/x07-ext-db-postgres/0.1.0/`
  - Modules: `std.db.pg`, `std.db.pg.spec`, `std.db.pg.pool`
- **MySQL package:** `packages/ext/x07-ext-db-mysql/0.1.0/`
  - Modules: `std.db.mysql`, `std.db.mysql.spec`, `std.db.mysql.pool`
- **Redis package:** `packages/ext/x07-ext-db-redis/0.1.0/`
  - Modules: `std.db.redis`, `std.db.redis.spec`, `std.db.redis.argv`
- **Native DB backends:**
  - SQLite: `crates/x07-ext-db-sqlite-native/` (C ABI used by `os.db.sqlite.*`)
  - Postgres: `crates/x07-ext-db-pg-native/` (C ABI used by `os.db.pg.*`)
  - MySQL: `crates/x07-ext-db-mysql-native/` (C ABI used by `os.db.mysql.*`)
  - Redis: `crates/x07-ext-db-redis-native/` (C ABI used by `os.db.redis.*`)
  - Build + stage scripts:
    - `scripts/build_ext_db_sqlite.sh`
    - `scripts/build_ext_db_pg.sh`
    - `scripts/build_ext_db_mysql.sh`
    - `scripts/build_ext_db_redis.sh`
- **Policy schema:** `schemas/run-os-policy.schema.json` (includes the `db` section)
- **Smoke suites:** `scripts/ci/check_db_smoke.sh` (runs `benchmarks/smoke/db-*.json`)

## Canonical API (agent-facing)

The “single canonical way” to use SQL DB v1 is the `std.db` facade (envelope-first; no typed handles required).

- `std.db.open_v1(uri: bytes, caps: bytes_view) -> bytes` (DbRespV1)
  - Extract handle: `std.db.open_handle_v1(open_resp: bytes) -> i32`
- `std.db.query_v1(conn: i32, sql: bytes, params_doc: bytes, qcaps: bytes_view) -> bytes` (DbRespV1)
  - Extract rows doc: `std.db.query_rows_doc_v1(resp: bytes) -> bytes` (DataModel doc: ok payload or `doc_err_from_code`)
- `std.db.exec_v1(conn: i32, sql: bytes, params_doc: bytes, qcaps: bytes_view) -> bytes` (DbRespV1)
  - Extract rows affected: `std.db.exec_rows_affected_v1(resp: bytes) -> i32` (`-1` on error)
- `std.db.close_v1(conn: i32, caps: bytes_view) -> bytes` (DbRespV1)

Driver pools are:

- SQLite: `std.db.sqlite.pool`
- Postgres: `std.db.pg.pool`
- MySQL: `std.db.mysql.pool`

Redis uses a separate command API: `std.db.redis.*` (see `docs/db/redis-v1.md`).

## Contracts (pinned)

- Response + caps formats: `docs/db/db-v1.md` (`X7DB` / `X7DC`)
- SQLite request formats + query result doc: `docs/db/sqlite-v1.md` (`X7SO` / `X7SQ` / `X7SE` / `X7SC`)
- Postgres request formats: `docs/db/postgres-v1.md` (`X7PO` / `X7PQ` / `X7PE` / `X7PC`)
- MySQL request formats: `docs/db/mysql-v1.md` (`X7MO` / `X7MQ` / `X7ME` / `X7MC`)
- Redis request formats: `docs/db/redis-v1.md` (`X7RO` / `X7RQ` / `X7RX` + `X7RV`)
- Pool bytes and token formats: `docs/db/pool-v1.md` (`X7PL`)

## Reference bundles (assets)

These bundles were used to bootstrap DB v1 design/implementation; the in-repo source listed above is authoritative:

- `docs/phases/assets/x07_ext_db_v1_sqlite_native_bundle.tar.gz`
- `docs/phases/assets/x07_ext_db_v1_pool_delta_bundle.tar.gz`
- `docs/phases/assets/x07_ext_db_v1_pg_mysql_bundle.tar.gz`
- `docs/phases/assets/x07_ext_db_v1_redis_bundle.tar.gz`

## Run the DB smokes

- Run the DB smoke suites (builds + stages native DB backends used by the suites, then runs `run-os-sandboxed` suites):
  - `./scripts/ci/check_db_smoke.sh`

- Postgres/MySQL/Redis smokes require local services. Dev fixtures:
  - Postgres: `benchmarks/fixtures/db/pg/v1/`
  - MySQL: `benchmarks/fixtures/db/mysql/v1/`
  - Redis: `benchmarks/fixtures/db/redis/v1/`
  - Run with `X07_DB_NETWORK_SMOKE=1 ./scripts/ci/check_db_smoke.sh`

## x07AST editing workflow

All `*.x07.json` updates should be done via structured editing:

- `docs/dev/x07-ast.md` (`x07 ast canon` + RFC6902 patch + `x07 ast apply-patch --validate`)
