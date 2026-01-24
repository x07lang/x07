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

## Packages

Core interfaces (required):

- `ext-db-core` (`std.db`, pooling, specs, DataModel conversion)

Drivers (pick the ones you need):

- `ext-db-sqlite` (`std.db.sqlite`, `std.db.sqlite.pool`)
- `ext-db-postgres` (`std.db.postgres`, `std.db.postgres.pool`)
- `ext-db-mysql` (`std.db.mysql`, `std.db.mysql.pool`)
- `ext-db-redis` (`std.db.redis`, `std.db.redis.pool`)

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

## Pools and concurrency

- pools are deterministic wrappers around OS connections
- use async tasks for overlapping I/O
- use OS multiprocessing only for CPU-heavy workloads (not for DB I/O)

## Expert appendix

- Add packages manually (advanced): pick NAME@VERSION from `/agent/latest/catalog/capabilities.json` and run `x07 pkg add NAME@VERSION --sync`.
- Debug runner behavior directly:
  - OS worlds: `x07-os-runner --project x07.json --world run-os`
