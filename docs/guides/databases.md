# Guide: Databases (SQLite/Postgres/MySQL/Redis)

Database access is provided by OS-world-only external packages that expose `std.db.*` modules.

## Canonical (template + x07 run)

Start from a DB template (recommended):

```bash
# Local file DB:
x07 init --template sqlite-app

# Remote DB client:
x07 init --template postgres-client
```

Then run in an OS world:

```bash
x07 run --profile sandbox -- <your-cli-args...>
```

Note: `x07 init --template sqlite-app` uses the canonical capability map and pins `ext-db-core@VERSION` and `ext-db-migrate@VERSION` (see `catalog/capabilities.json`). If your registry/index does not have those versions, init will fail; check the registry catalog (`https://registry.x07.io/index/catalog.json`, or `https://x07.io/packages`) and sync/publish the missing versions.

## Packages

Core interfaces (required):

- `ext-db-core` (`std.db`, pooling, specs, DataModel conversion)

Drivers (pick the ones you need):

- `ext-db-sqlite` (`std.db.sqlite`, `std.db.sqlite.pool`)
- `ext-db-postgres` (`std.db.pg`, `std.db.pg.pool`)
- `ext-db-mysql` (`std.db.mysql`, `std.db.mysql.pool`)
- `ext-db-redis` (`std.db.redis`, `std.db.redis.pool`)

Migrations + RR helpers:

- `ext-db-migrate` (`std.db.migrate`, `std.db.rr`) uses pinned repo-local plans under `arch/db/`.

To discover available packages and versions, use the registry catalog:

- https://registry.x07.io/index/catalog.json

## Worlds and sandbox policy

These packages are allowed only in:

- `run-os`
- `run-os-sandboxed`

For `run-os-sandboxed`, start from:

```bash
x07 policy init --template sqlite-app
```

Then ensure the SQLite database file path is under an allowed `write_roots` entry (for example under `out/`).

## DataModel-first

Database query results are returned as **DataModel** documents, so you can:

- convert between DB rows and JSON/YAML/CSV
- reuse formatting/validation tools across sources

## Canonical approach

- Create a connection (or pool) using `std.db.*` wrappers
- Send parameterized queries
- Decode results into DataModel
- Keep query-building deterministic

## Parameter binding (`params_doc`)

`std.db.*.exec_v1` and `std.db.*.query_v1` take `params_doc: bytes`, a **DataModel document**
whose root is a **sequence of DataModel values** (one value per SQL placeholder like `?`).

Helpers for common cases (from `std.db.params`):

- No params: `std.db.params.empty_v1()`
- 1 string param: `std.db.params.one_string_v1(bytes.lit("alice"))`
- 1 number param (decimal ASCII): `std.db.params.one_number_dec_v1(bytes.lit("42"))`
- 1 null param: `std.db.params.one_null_v1()`
- 1 bool param: `std.db.params.one_bool_v1(1)` (true) / `std.db.params.one_bool_v1(0)` (false)

For **multiple parameters**, build a DataModel sequence and wrap it as a doc:

```json
[
  "begin",
  ["let", "v_name", ["ext.data_model.value_string", ["bytes.view", ["bytes.lit", "alice"]]]],
  ["let", "v_age", ["ext.data_model.value_number", ["bytes.view", ["bytes.lit", "42"]]]],
  ["let", "elems", ["vec_u8.with_capacity", 0]],
  ["set", "elems", ["vec_u8.extend_bytes", "elems", ["codec.write_u32_le", 2]]],
  ["set", "elems", ["vec_u8.extend_bytes", "elems", ["codec.write_u32_le", ["bytes.len", "v_name"]]]],
  ["set", "elems", ["vec_u8.extend_bytes", "elems", "v_name"]],
  ["set", "elems", ["vec_u8.extend_bytes", "elems", ["codec.write_u32_le", ["bytes.len", "v_age"]]]],
  ["set", "elems", ["vec_u8.extend_bytes", "elems", "v_age"]],
  ["let", "elems_b", ["vec_u8.into_bytes", "elems"]],
  ["let", "seq_val", ["ext.data_model.value_seq_from_elems", ["bytes.view", "elems_b"]]],
  ["ext.data_model.doc_ok", ["bytes.view", "seq_val"]]
]
```

## Pools and concurrency

- pools are deterministic wrappers around OS connections
- use async tasks for overlapping I/O
- use OS multiprocessing only for CPU-heavy workloads (not for DB I/O)

## Expert appendix

- Add packages manually (advanced): pick NAME@VERSION from `/agent/latest/catalog/capabilities.json` and run `x07 pkg add NAME@VERSION --sync`.

## Minimal `arch/db/` example (copy/paste)

Contract files referenced by `std.db.migrate.*_from_arch_v1` live under `arch/db/`:

- `docs/examples/contracts_project/arch/db/index.x07db.json`
- `docs/examples/contracts_project/arch/db/migrations/app_v1.migrate.json`
- `docs/examples/contracts_project/arch/db/queries/app_v1.queries.json`

The example DB index references a budget profile; see:

- `docs/examples/contracts_project/arch/budgets/index.x07budgets.json`

## Reference contracts

- [DB v1 envelope + caps](../db/db-v1.md)
- Driver request formats: [SQLite](../db/sqlite-v1.md), [Postgres](../db/postgres-v1.md), [MySQL](../db/mysql-v1.md), [Redis](../db/redis-v1.md)
