# Guide: Databases (SQLite/Postgres/MySQL/Redis)

Database access is provided by OS-world-only external packages that expose `std.db.*` modules.

## Packages

Core interfaces (required):

- `ext-db-core@0.1.2` (`std.db`, pooling, specs, DataModel conversion)

Drivers (pick the ones you need):

- `ext-db-sqlite@0.1.2` (`std.db.sqlite`, `std.db.sqlite.pool`)
- `ext-db-postgres@0.1.2` (`std.db.postgres`, `std.db.postgres.pool`)
- `ext-db-mysql@0.1.2` (`std.db.mysql`, `std.db.mysql.pool`)
- `ext-db-redis@0.1.2` (`std.db.redis`, `std.db.redis.pool`)

## Add dependencies

Example (SQLite):

```bash
x07 pkg add ext-db-core@0.1.2 --sync
x07 pkg add ext-db-sqlite@0.1.2 --sync
```

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

## Pools and concurrency

- pools are deterministic wrappers around OS connections
- use async tasks for overlapping I/O
- use OS multiprocessing only for CPU-heavy workloads (not for DB I/O)
