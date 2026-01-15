# Guide: Databases (SQLite/Postgres/MySQL/Redis)

Database access is provided by the external `ext.db` family.

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
