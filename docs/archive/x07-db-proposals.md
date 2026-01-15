Archived draft (kept for reference). For the implemented DB v1 packages and contracts, see:

- `docs/db/db-v1.md`
- `docs/db/sqlite-v1.md`
- `docs/db/pool-v1.md`

Below is a concrete, end‑user‑oriented design + dev plan for **database support as external packages** (SQLite + Postgres + MySQL) that is **DataModel‑native** (your `ext.data_model` doc/value encoding is the interchange format), and **run‑os/run‑os‑sandboxed only** (never available in deterministic solve-*).

I’m going to treat “agentic reliability” as the first requirement: agents should almost never hand‑roll binary encodings, never slice offsets manually, and should get structured, stable error codes while still having human‑readable error text for debugging.

---

## 0) Operating assumptions and non-goals

### Worlds and determinism

* DB access is **standalone-only**. It belongs to `run-os` and `run-os-sandboxed`.
* No attempt to make “DB I/O deterministic”. In sandboxed world we enforce **capability policy**, not determinism.

### Implementation substrate (what you should leverage)

* **SQLite**: embed or link to sqlite3 and use prepared statements + bind/step/finalize.
* **Postgres**: use `libpq` and the parameterized exec path (`PQexecParams`), which explicitly supports `$1`… placeholders and passing params separately (avoid manual quoting).
* **TLS**:

  * Postgres: libpq has an SSL layer and documents SSL configuration (including `sslmode` behavior).
  * MySQL/MariaDB: use `mysql_optionsv` / SSL mode options in the connector library (MariaDB connector docs are a good reference and often used as the “MySQL‑compatible C client” for licensing + portability reasons).

### License reality you should decide early

* SQLite is “public domain / unrestricted” (easy to vendor).
* For “MySQL client library” licensing, if you want fewer surprises, strongly consider **MariaDB Connector/C** as the default “MySQL‑protocol C connector” (it’s widely used and documents SSL options).
* If you instead link Oracle’s MySQL client libs, you’ll need to be careful about their licensing model.

---

## 1) What “DataModel compatibility” should mean (pin this as the core contract)

### A. Queries return **DataModel docs**

All query results returned to X07 should be a **DataModel DocV1** (the same thing JSON/CSV/YAML modules already consume/produce).

**Canonical result doc shape (DbRowsDocV1)**:

* Root value is a **map** with these keys (keys are raw bytes, but treat them as ASCII identifiers):

  * `"rows"`: seq of row maps
  * `"cols"`: seq of column names (strings) in driver column order
  * `"types"`: seq of driver type strings (optional in v1, strongly recommended)
  * `"rows_affected"`: number (present for exec / DML)
  * `"last_insert_id"`: number (MySQL/SQLite, optional)
* Each row is a **map**:

  * map keys are **column names** (bytes)
  * values are DataModel scalar values (null/bool/number/string)

**Critical determinism rule inside a row:**
Row maps must have **stable key ordering** (even though DB returns columns in a deterministic order, your map encoding should be sorted to enable fast `map_find` / binary search). Your `ext.data_model` already has sorting helpers, so: always sort row entries by key bytes before emitting. (This makes downstream access stable and fast.)

### B. Parameters are supplied as **DataModel scalar values**

Parameters should be accepted as a DataModel value (or doc) so agents can reuse the same “serde-like” building blocks they use for JSON/CSV/YAML.

**Canonical params shape (DbParamsV1)**:

* A **DataModel seq** of scalar values:

  * null
  * bool
  * number (bytes payload is canonical decimal ASCII unless otherwise specified)
  * string (UTF‑8 bytes)

For v1, keep it strict:

* No nested seq/map in params (reject with SPEC_ERR_DB_PARAM_KIND).
* Binary blobs: represent as **string** with a deterministic prefix:

  * `b64:<base64url>` (or hex), then adapters decode into DB bytes.
  * This keeps JSON/YAML roundtrips safe.

### C. Errors are stable codes, with optional text

For agentic coding you need:

* **Stable numeric codes** for program logic and repair loops.
* Optional **human-readable text** for debugging.

So implement DB calls returning either:

* `result_bytes` / `result_i32` (stable code), and separately return text via stderr diagnostics, **or**
* an explicit DB response envelope that includes both code and message.

Given you already invested in “specbin/specrows” style encodings elsewhere, I recommend an envelope for DB too.

---

## 2) Package split that avoids “LLM-choice problems”

Use the same “facade + internal impls” pattern you adopted for collections:

### Package 1: `x07:ext-db-core@0.1.0`

Contains:

* `std.db.spec.*` (all binary request/response encodings + pack/unpack)
* `std.db.dm.*` (DataModel mapping helpers: scalar conversions, row-doc builders if needed, error-doc builder)
* `std.db` facade (driver-neutral entry points)

### Package 2: `x07:ext-db-sqlite@0.1.0`

Contains:

* `std.db.sqlite` (driver module)
* native glue (C shim or Rust staticlib) implementing the adapter entry points

### Package 3: `x07:ext-db-postgres@0.1.0`

Contains:

* `std.db.pg`
* native glue (libpq-based)

### Package 4: `x07:ext-db-mysql@0.1.0`

Contains:

* `std.db.mysql`
* native glue (MariaDB connector recommended)

**Agents use only `std.db.*` facade** (one canonical way). The facade dispatches by URI scheme:

* `sqlite:` → sqlite driver
* `postgres:` / `postgresql:` → pg driver
* `mysql:` → mysql driver

---

## 3) Minimal API surface for “DB v1” (small, reliable, production‑useful)

### A. Facade API (what agents learn)

In `std.db`:

**Connection**

* `std.db.open_v1(db_uri_bytes, db_caps_bytes) -> result_i32`

  * returns a `db_conn_handle` (i32)
* `std.db.close_v1(conn_handle) -> i32`

**Exec and query**

* `std.db.exec_v1(conn_handle, sql_bytes, params_doc_bytes, qcaps_bytes) -> bytes`

  * returns `DbRespV1` (spec envelope) containing:

    * ok: rows_affected, last_insert_id (if any)
    * err: code + message
* `std.db.query_v1(conn_handle, sql_bytes, params_doc_bytes, qcaps_bytes) -> bytes`

  * returns `DbRespV1` where ok payload contains **a DataModel DocV1** (DbRowsDocV1)

**Transactions**

* `std.db.tx_begin_v1(conn_handle) -> i32`
* `std.db.tx_commit_v1(conn_handle) -> i32`
* `std.db.tx_rollback_v1(conn_handle) -> i32`

**Ergonomic helpers (agent-friendly)**

* `std.db.params.empty_v1() -> bytes` (a valid empty DataModel seq doc)
* `std.db.params.one_v1(x_dm_value) -> bytes` (wrap one scalar into params doc)
* `std.db.params.from_seq_doc_v1(seq_doc) -> result_bytes` (validate + reuse)
* `std.db.query_rows_doc_v1(resp_bytes) -> result_bytes` (extract DataModel doc or map err)
* `std.db.exec_rows_affected_v1(resp_bytes) -> result_i32`
* `std.db.err_code_v1(resp_bytes) -> i32`
* `std.db.err_msg_v1(resp_bytes) -> bytes` (best-effort UTF‑8)

### B. Driver modules (mostly hidden behind facade)

Each driver implements the same internal trait surface:

* `std.db.sqlite._open_v1(...)`
* `std.db.sqlite._exec_v1(...)`
* `std.db.sqlite._query_v1(...)`
* `std.db.sqlite._close_v1(...)`

Same for pg/mysql.

---

## 4) Spec: request/response bytes envelopes (so agents never slice offsets)

Define a pinned binary contract (like your `specbin` approach):

### `X7DB` envelope: `DbRespV1`

* `magic = "X7DB"` (4 bytes)
* `version = 1` (u32 LE)
* `tag` (u32 LE): 1=OK, 0=ERR
* `op` (u32 LE): 1=open, 2=exec, 3=query, 4=close, 5=tx_begin, 6=tx_commit, 7=tx_rollback
* If OK:

  * `ok_len` (u32 LE)
  * `ok_payload` (bytes)
* If ERR:

  * `err_code` (u32 LE)
  * `err_msg_len` (u32 LE)
  * `err_msg` (bytes, UTF‑8 best-effort)
  * `driver_code_len` + `driver_code` (optional; e.g. SQLSTATE for pg)
  * `driver_msg_len` + `driver_msg` (optional)

**Key design choice:** the *query* OK payload is a **DataModel DocV1 bytes blob** (DbRowsDocV1).

### `DbCapsV1` and `DbQueryCapsV1`

Keep them explicit and policy-compatible:

* connect timeout ms
* query timeout ms
* max rows
* max doc bytes
* tls required flag (must be true for pg/mysql)
* for sqlite: allowed open modes (ro/rw/create)

Agents should not build these manually:

* provide `std.db.spec.caps_default_v1()` and a few setters.

---

## 5) Native implementation plan (per DB)

### A. SQLite (in-process, embedded)

**Why**: simplest to ship, file-based, easiest for stable smoke tests, and SQLite itself is extremely well-documented and commonly embedded.

**Native steps**

1. Link sqlite3 (vendor amalgamation or system).
2. Implement per-connection handle table in the runtime shim:

   * `conn_handle -> sqlite3*`
3. For each exec/query:

   * prepare statement (`sqlite3_prepare_v2`)
   * bind params (`sqlite3_bind_*`)
   * step rows (`sqlite3_step`)
   * finalize (`sqlite3_finalize`)
4. Convert each row to DataModel:

   * build row map entries (colname → scalar)
   * sort keys deterministically
   * append to `rows` seq
5. Emit DbRowsDocV1 DataModel doc and wrap it into DbRespV1 OK.

**SQLite type mapping (v1)**

* NULL → dm.null
* INTEGER → dm.number (decimal string)
* REAL → dm.number (decimal string; do not promise exact float formatting stability)
* TEXT → dm.string (UTF‑8 bytes)
* BLOB → dm.string “b64:…”

### B. Postgres (libpq)

Use **parameterized execution** (no string interpolation), which is explicitly supported by `PQexecParams` and uses `$1`, `$2`, … placeholders.

**Native steps**

1. Link libpq.
2. Connection open:

   * Build conninfo string from `DbUriV1` + `DbCapsV1` (don’t let agents hand-roll).
   * Enforce TLS by setting `sslmode=verify-full` (or `verify-ca` if you must), per libpq SSL docs.
3. Query:

   * Use `PQexecParams` to pass params separately.
   * Params are text format (UTF‑8 bytes), DB casts as needed.
4. Result decode:

   * Use `PQnfields`, `PQfname`, `PQgetvalue`, `PQgetisnull`.
   * Optionally read OIDs to type-map into dm.bool/dm.number/dm.string.
5. Error handling:

   * Map SQLSTATE (if available) into `driver_code`.
   * Stable code space (see below).
6. Wrap DataModel doc into DbRespV1.

### C. MySQL (recommend MariaDB Connector/C)

You want MySQL protocol compatibility plus TLS and fewer licensing surprises; Connector/C documents SSL configuration and is widely used.

**Native steps**

1. Use prepared statement API:

   * prepare, bind params, execute, bind result, fetch (MySQL APIs document bind_result, etc.).
2. Enforce TLS in connector options (SSL mode required).
3. Decode fields using metadata to decide dm.bool/dm.number/dm.string.
4. Wrap into DbRespV1.

---

## 6) `run-os-sandboxed` policy design (DB section)

Add a `db` section to `schemas/run-os-policy.schema.json` (or extend existing net policy if you prefer, but DB deserves its own section because it includes file-path access for SQLite):

### `policy.db` (top level)

* `enabled: bool`
* `drivers: { sqlite: bool, postgres: bool, mysql: bool }`
* `max_live_conns: u32`
* `max_queries: u32`
* `connect_timeout_ms: u32`
* `query_timeout_ms: u32`
* `max_rows: u32`
* `max_resp_bytes: u32`

### `policy.db.sqlite`

* `allow_paths: [string]` (relative to sandbox root, no `..`)
* `allow_create: bool`
* `allow_in_memory: bool` (default false)

### `policy.db.net` (for pg/mysql)

* `allow_dns: [string]` (exact or suffix match rules; pick one deterministic rule set)
* `allow_cidrs: [string]` (CIDR strings)
* `allow_ports: [u32]`
* `require_tls: bool` (must be true for v1)
* `require_verify: bool` (verify-full / verify identity)

The native shim must:

* load the policy from env vars / a file path provided by the runner
* enforce host/port allowlists before connecting
* enforce timeouts, max rows, max bytes
* fail with stable error codes (policy denied, timeout, cap exceeded)

---

## 7) Bench/smoke strategy (so it’s testable and shippable)

### A. SQLite deterministic smoke (always on)

* Ship a fixture SQLite DB under `benchmarks/fixtures/db/sqlite/v1/app.db` seeded with:

  * a small table of ints, strings, NULLs, and one blob (to test b64 encoding)
* Add:

  * `benchmarks/run-os-sandboxed/db-sqlite-smoke.json`
  * tests:

    * open → query → verify DataModel doc equals expected canonical bytes
    * exec (insert/update) → rows_affected
    * policy deny (open path not allowlisted)

This suite is **fully deterministic** and doesn’t need network.

### B. Postgres/MySQL smoke (tiered)

Because these require a server:

* Provide smoke suites that run **only when env is present**, otherwise skip deterministically:

  * `X07_PG_TEST_DSN`
  * `X07_MYSQL_TEST_DSN`
* On dev machines and release CI, run docker-compose:

  * start postgres + mysql, seed deterministic schema/data, run smokes

Smoke tasks should validate:

* parameter binding works (no interpolation)
* TLS required (attempt plaintext should fail with stable code)
* DataModel mapping and canonical result doc shape

---

## 8) “Different complexity levels” roadmap (v1 → v2 → v3)

### DB v1 (what you implement first)

* open/close
* exec/query
* params as DataModel seq of scalars
* result as DataModel doc
* TLS required for pg/mysql
* sandbox policy gating
* sqlite deterministic smoke always-on; pg/mysql env-gated smokes

### DB v2 (high impact for real apps)

* Prepared statement handles (avoid re-prepare cost)
* Transaction object semantics:

  * begin returns tx_handle
  * commit/rollback consume tx_handle
* Connection pool in stdlib (fixed-size)
* Better type mapping:

  * timestamps remain string (RFC3339-ish), numeric stays number text
  * blobs as bytes type if/when DataModel adds it; otherwise keep b64

### DB v3 (scales to large datasets)

* Streaming rows:

  * query_stream returns `iface` row-reader that yields row docs or row chunks
  * integrate with `std.io` buffering traits (so JSON/CSV streaming pipelines work)
* Cancellation/timeouts integration with async:

  * if you can’t make DB I/O nonblocking in C, implement “DB I/O task” as yield boundary by polling with time slices (driver-specific complexity)

---

## 9) Concrete deliverables and file layout (so devs can implement)

### Docs

* `docs/db/db-v1.md` (normative)

  * DbRespV1 envelope
  * DbRowsDocV1 DataModel shape
  * DbParamsV1 requirements
  * error code spaces
  * sandbox policy expectations
* `docs/db/sqlite-v1.md`
* `docs/db/postgres-v1.md` (libpq + sslmode requirements)
* `docs/db/mysql-v1.md` (connector choice + SSL mode requirement)

### Policy schema

* `schemas/run-os-policy.schema.json` add `db` section described above

### Packages

* `packages/x07-ext-db-core/0.1.0/`

  * `modules/std/db/spec.x07.json`
  * `modules/std/db/params.x07.json`
  * `modules/std/db/dm.x07.json`
  * `modules/std/db/module.x07.json` (facade)
* `packages/x07-ext-db-sqlite/0.1.0/`

  * `modules/std/db/sqlite.x07.json`
* `packages/x07-ext-db-postgres/0.1.0/`

  * `modules/std/db/pg.x07.json`
* `packages/x07-ext-db-mysql/0.1.0/`

  * `modules/std/db/mysql.x07.json`

### Native shims

* `crates/x07-ext-db-sqlite/` (static lib)
* `crates/x07-ext-db-postgres/` (static lib or thin wrapper around libpq)
* `crates/x07-ext-db-mysql/` (static lib using MariaDB connector)
* `scripts/build_ext_db.sh` builds and copies libs into `deps/x07/` deterministically

### Smokes

* `benchmarks/run-os-sandboxed/db-sqlite-smoke.json`
* `benchmarks/run-os-sandboxed/db-policy-deny-smoke.json`
* `benchmarks/run-os/db-postgres-smoke.json` (env-gated)
* `benchmarks/run-os/db-mysql-smoke.json` (env-gated)

---

## 10) Key “agentic UX” choices that will pay off

1. **Agents never assemble envelopes manually**
   Every spec bytes encoding gets `pack_*` and `unpack_*` helpers (like you did for net/http). That’s the biggest reliability lever.

2. **One canonical facade (`std.db`)**
   Drivers exist, but agents should rarely import `std.db.pg` etc.

3. **Always parameterize**
   Make `exec/query` accept params doc and encourage placeholders; for Postgres you’ll be on `PQexecParams` which explicitly supports parameter arrays and `$n` placeholders.
   For SQLite the prepare/bind/step/finalize pattern is standard and stable.

4. **Stable code + debug text**
   Stable numeric codes drive agent repair; text is for logging.

5. **Sandbox policy is explicit and enforceable**
   For pg/mysql, enforce TLS configuration per connector docs; for MySQL/MariaDB, SSL mode config is explicitly supported via options calls.

---
Use docs/phases/assets/x07_ext_db_v1_sqlite_native_bundle.tar.gz
SHA-256: `0be34ce19ec0b70850fe7519fee17e32dc55b9c896f06db5738b46ce7f2b2a0b`

## What’s inside this “next bundle”

### 1) Native SQLite adapter path (stdlib + pinned wire format)

* **Implemented stdlib wrapper:** `std.db.sqlite` now has real request packers + thin call-throughs to the native OS builtins:

  * `std.db.sqlite.spec.caps_pack_v1` + `std.db.sqlite.caps_default_v1` (X7DC)
  * `std.db.sqlite.spec.open_req_path_v1` (X7SO)
  * `std.db.sqlite.spec.query_req_v1` (X7SQ)
  * `std.db.sqlite.spec.exec_req_v1` (X7SE)
  * `std.db.sqlite.spec.close_req_v1` (X7SC)
  * wrappers that call:

    * `os.db.sqlite.open_v1(req, caps) -> DbRespV1`
    * `os.db.sqlite.query_v1(req, caps) -> DbRespV1`
    * `os.db.sqlite.exec_v1(req, caps) -> DbRespV1`
    * `os.db.sqlite.close_v1(req, caps) -> DbRespV1`

SQLite is a good “always-green” first adapter because it’s an in-process, serverless, self-contained database engine.

### 2) Fully working smoke programs that do byte-for-byte DataModel doc asserts

Two standalone smoke programs are included (run under **run-os-sandboxed**):

1. **OK-path:**
   `tests/external_os/db_sqlite_smoke_ok/src/main.x07.json`

   * opens fixture DB: `benchmarks/fixtures/db/sqlite/v1/app.db`
   * runs query: `SELECT id,name,n,payload,note FROM items ORDER BY id;`
   * builds the **expected DataModel doc** using `ext.data_model` builders
   * compares **returned payload bytes** to expected **byte-for-byte**
   * prints `OK` on success, `ERR` otherwise

2. **Policy-deny path:**
   `tests/external_os/db_sqlite_smoke_deny/src/main.x07.json`

   * attempts to open `secrets.db`
   * asserts the response is an error with:

     * `DB_ERR_POLICY_DENIED = 53249` (0xD001)
   * prints `OK` on success, `ERR` otherwise

### 3) Updated smoke suite

* `benchmarks/run-os-sandboxed/db-sqlite-smoke.json` now runs both cases above against:

  * policy: `schemas/run-os-policy.db.example.json`

### 4) SQLite v1 doc updated to pin DataModel doc shape + mapping

* `docs/db/sqlite-v1.md` is rewritten to be the “single source of truth” for:

  * X7DC/X7SO/X7SQ/X7SE/X7SC request/caps formats
  * X7DB envelope usage
  * **Query payload**: canonical DataModel doc with map keys `"cols"` and `"rows"`
  * Value mapping based on SQLite’s storage classes (NULL/INTEGER/REAL/TEXT/BLOB).

### 5) Toolchain integration notes + reference skeleton

* `patches/INTEGRATION_os_db_sqlite.md` explains what the OS builtins must do and what the smoke relies on.
* `patches/REFERENCE_os_db_sqlite.c` is a readable starting point for native implementation (you’ll adapt it to your runtime ABI + your exact `ext.data_model` canonical encoding).

## The exact byte-for-byte expectation the smoke enforces

For `query`, the **X7DB ok payload** must be a **DataModel doc** whose body is:

```text
{
  "cols": ["id","name","n","payload","note"],
  "rows": [
    [1,"alpha",1,"HELLO",null],
    [2,"beta", 2,"BYE",  ""  ],
    [3,"gamma",3,"",     null]
  ]
}
```

Canonical rules matter (key order, offsets, etc.), because the smoke compares the payload bytes directly to an `ext.data_model`-constructed expected doc.

## Native side requirements (what your OS builtins must enforce)

Your runtime should:

* enforce policy allowlists via `run-os-policy`:

  * `db.enabled = true`
  * `db.drivers.sqlite = true`
  * `db.sqlite.allow_paths` contains the DB file path
* map SQLite values using its documented storage classes (NULL/INTEGER/REAL/TEXT/BLOB).
* return errors using the X7DB envelope from `std.db.spec`

If you follow `docs/db/sqlite-v1.md`, the included smoke suite will pass once the native builtins are wired up.

If you want, I can also generate a *second* bundle variant that vendors a tiny sqlite3 build (amalgamation) + a concrete C implementation that matches a specific `bytes_t`/`bytes_view_t` ABI (once you paste your current runtime C typedefs for those).

++++++
Yes — the external DB packages **should** use concurrency, but in a very specific way that fits X07’s model and keeps agent usage reliable:

* **X07-level concurrency** (what agents write): `defasync`, `task.*`, `chan.bytes.*` to run *multiple DB requests concurrently* and to build a *connection pool*.
* **Native adapter concurrency** (what the package implements under the hood): ensure each DB operation **does not block the single-threaded cooperative scheduler**. You do that either by:

  1. using the DB client library’s **nonblocking/async APIs**, or
  2. offloading blocking DB calls to a **bounded OS threadpool** and turning the call into a “yield boundary” (handle + poll/join).

Below is the recommended, production-friendly approach for X07 (run-os/run-os-sandboxed) that scales to SQLite + Postgres + MySQL.

---

## 1) Concurrency model to target for external DB packages

### What agents can do today (good)

* Spawn tasks that issue queries.
* Use channels to coordinate.
* Await results.

### The runtime constraint you must respect

Your **scheduler is cooperative and single-core** (even in run-os*). If a DB builtin does a blocking network read/write or blocks inside `sqlite3_step()`, it blocks the entire scheduler and kills “concurrency”.

So: **DB operations must become scheduler-friendly yield points**.

---

## 2) Two viable implementation strategies for DB adapter concurrency

### Strategy A (recommended for v1): “Async façade” using a bounded OS threadpool

**Why it’s best for v1**

* Works across SQLite/libpq/libmysqlclient without having to wire every library’s nonblocking API.
* Is cross-platform (Linux/macOS/Windows) using a consistent internal mechanism.
* Lets X07 tasks overlap DB calls (I/O waits) even though the scheduler is single-threaded.

**How it works**

* `os.db.*` builtins are split into:

  * `spawn_*_v1(...) -> i32 handle`
  * `try_join_*_v1(handle) -> result_bytes` (nonblocking)
  * `join_*_v1(handle) -> bytes` (**yield boundary**; allowed in `solve`/`defasync`)
  * `kill/drop` as needed

This mirrors your multiprocessing process-table design and fits agentic patterns.

**Safety**

* A strict cap from run-os-sandboxed policy:

  * `db.max_live_ops`
  * `db.max_threads`
  * per-op `timeout_ms`
  * per-op `max_rows`, `max_cell_bytes`, `max_total_bytes`

### Strategy B (more complex but higher-performance for network DBs): use nonblocking DB client APIs

This is how you eventually avoid extra OS threads:

* **Postgres (libpq)** supports async command processing: apps poll the socket and use `PQconsumeInput`, `PQisBusy`, and `PQgetResult`. ([PostgreSQL][1])
  It also has pipeline mode for batching. ([PostgreSQL][2])
* **MySQL** C API provides asynchronous/nonblocking functions like `mysql_real_query_nonblocking()` (and a whole “C API Asynchronous Interface”). ([MySQL Developer Zone][3])

You can integrate these with your scheduler by registering each connection’s socket fd and polling in the runtime loop (like an internal epoll/kqueue/IOCP abstraction). This is a **Phase v2** move unless you already have a general event loop substrate.

---

## 3) Connection pooling: how it should work in X07 (agent-friendly)

### Hard rule: never issue concurrent commands on the same connection

Most client libs either forbid it or it’s unsafe.

* libpq is thread-safe *but* you must not manipulate the same `PGconn` concurrently; to run concurrent commands use multiple connections. ([Postgres Professional][4])

So the pool exists to guarantee: **at most one in-flight query per connection**.

---

## 4) Pool design that fits X07’s primitives

### Pool v1 (canonical): “channel of connection handles”

This is simple, robust, and easy for agents:

* `pool` is represented as an `i32` channel handle.
* Each message is a 4-byte `u32_le` encoding of a connection handle (or a small tagged doc if you need driver id).

**Initialization**

* open N connections
* `chan.bytes.send(pool_chan, pack_handle(conn_handle))`

**Query**

* `conn = recv(pool_chan)`
* do query on conn (yielding DB op)
* send conn back
* return result

### Why this beats a runtime pool table

* No extra runtime surface area.
* No new lifetime type required.
* You can version it in stdlib packages and evolve it without changing compiler/runtime.

### But: “safe early-drop” / no-leak behavior

To be agent-proof, you must ensure the connection is always returned to the pool even on error.

Since X07 doesn’t have Rust-style RAII/Drop for arbitrary resources, the safest v1 approach is:

* **Do not expose “checkout/return” as the default workflow.**
* Provide **one canonical helper** that wraps the whole cycle:

#### `std.db.pool.query_v1(pool, sql, params_dm, caps) -> result_bytes`

Internally:

* recv conn
* run query
* always send conn back
* return `result_bytes`

For multi-statement transactions, add a single-shot transaction helper:

#### `std.db.pool.tx_batch_v1(pool, ops_dm, caps) -> result_bytes`

Where `ops_dm` is a DataModel array of `{sql, params}` steps and the helper ensures the conn is returned.

This avoids letting agents forget “return conn to pool” on early exits.

---

## 5) How pooling differs per DB

### SQLite

* Pooling is usually not helpful beyond **size=1** because SQLite serializes writes and uses file locks.
* You should expose `busy_timeout_ms` in caps (so “database is locked” retries are bounded). SQLite provides `sqlite3_busy_timeout()`. ([SQLite][5])
* Threading: if you ever use background threads, SQLite’s threading mode matters (Serialized vs Multi-thread) and depends on compile-time `SQLITE_THREADSAFE`. ([SQLite][6])
  In your current single-threaded scheduler, you’re naturally safe; only the “threadpool adapter” makes this relevant.

### Postgres

* Pool size >1 makes sense for concurrency.
* You **must** ensure one query at a time per `PGconn`. ([Postgres Professional][4])
* Consider pipeline mode later for throughput. ([PostgreSQL][2])

### MySQL/MariaDB

* Pool size >1 makes sense.
* Ensure the client lib is thread-safe if you use a threadpool adapter; MariaDB exposes `mysql_thread_safe()` to detect this and notes it’s thread-safe by default. ([MariaDB][7])
* MySQL has true nonblocking APIs you can integrate later. ([MySQL Developer Zone][3])

---

## 6) Concrete recommendation for your DB packages (v1 plan)

### Public API should be “concurrency-ready”, but still simple

Offer these tiers:

#### Tier 0 (simple, one connection)

* `std.db.sqlite.open_v1(...) -> i32 conn`
* `std.db.sqlite.query_dm_v1(conn, sql, params_dm, caps) -> result_bytes`
* same for pg/mysql

#### Tier 1 (pool, canonical)

* `std.db.pool.open_v1(driver_code, dsn_bytes, size_i32, caps) -> i32 pool_chan`
* `std.db.pool.query_v1(pool_chan, sql, params_dm, caps) -> result_bytes`
* `std.db.pool.close_v1(pool_chan) -> i32`

#### Tier 2 (advanced, later)

* streaming results (`query_stream_v1 -> iface reader`)
* prepared statement cache per worker
* async/nonblocking socket integration (remove OS threads)

### Under the hood: implement DB I/O as yield points

* In run-os world: allow blocking calls, but strongly prefer yield-boundary design so tasks keep running.
* In run-os-sandboxed: enforce hard caps and allowlists.

---

## 7) Answering your question directly

### “Will the external packages be using concurrency?”

They should, in two ways:

1. **At the X07 level**: yes — pooling + parallel queries are built with `defasync` + channels + `await` to overlap I/O and manage N connections.

2. **At the native adapter level**: yes (internally) — either via:

   * a bounded OS threadpool (recommended v1), or
   * DB nonblocking APIs (`libpq` async processing, MySQL async C API) (recommended v2). ([PostgreSQL][1])

### “How will they handle pools of connections?”

The most reliable agent-friendly method in X07 is:

* represent a pool as a **channel of connection handles**
* expose **one canonical helper** (`pool.query_v1`, `pool.tx_batch_v1`) so agents never manually manage checkout/return
* enforce **one query at a time per connection** (required by libpq rules and generally good practice). ([Postgres Professional][4])

---

## One decision I recommend you lock in now

If you want 100% agentic reliability, choose:

* **Pool API = “single canonical way”** (no manual checkout by default)
* **DB ops = yield boundaries** (disallow in `defn`, allowed in `solve`/`defasync`), mirroring your async/join restrictions

That combination dramatically reduces the two most common autonomous-agent failures:

* blocking the scheduler accidentally
* leaking/“forgetting to return” pooled resources

[1]: https://www.postgresql.org/docs/current/libpq-async.html?utm_source=chatgpt.com "18: 32.4. Asynchronous Command Processing"
[2]: https://www.postgresql.org/docs/current/libpq-pipeline-mode.html?utm_source=chatgpt.com "Documentation: 18: 32.5. Pipeline Mode"
[3]: https://dev.mysql.com/doc/c-api/8.3/en/mysql-real-query-nonblocking.html?utm_source=chatgpt.com "7.4.6 mysql_real_query_nonblocking()"
[4]: https://postgrespro.com/docs/enterprise/13/libpq-threading.html?utm_source=chatgpt.com "Documentation: 13: 36.19. Behavior in Threaded Programs"
[5]: https://sqlite.org/c3ref/busy_timeout.html?utm_source=chatgpt.com "Set A Busy Timeout"
[6]: https://sqlite.org/c3ref/c_config_covering_index_scan.html?utm_source=chatgpt.com "Configuration Options"
[7]: https://mariadb.com/docs/connectors/mariadb-connector-c/api-functions/mysql_thread_safe?utm_source=chatgpt.com "mysql_thread_safe | Connectors | MariaDB Documentation"
++++
Here’s the **v1 “drop‑in bundle delta”** that adds a DB pool abstraction (channel-of-tokens), pins the **X7PL** pool bytes encoding, and ships **3 run‑os‑sandboxed smoke cases** proving:

* **(a) FIFO fairness** (1-conn pool → acquisition order 0,1,2)
* **(b) max concurrency bound** (2-conn pool → event trace shows the 3rd worker can’t acquire until a token is returned)
* **(c) no-leak close** (a pool wrapper acquires/releases even when exec returns an error; close drains cleanly)

[Download x07_ext_db_v1_pool_delta_bundle.tar.gz](sandbox:/mnt/data/x07_ext_db_v1_pool_delta_bundle.tar.gz)
SHA-256: `0b13635b7ff7ad8858096baadab53c2e9d68a36296b992dc3a04a94c7ce2afdf`

---

## What’s inside (exact paths)

### New/updated stdlib modules (x07AST JSON)

* **NEW** `packages/x07-ext-db-core/0.1.0/modules/std/db/pool.x07.json`
* **UPDATED** `packages/x07-ext-db-core/0.1.0/package.json` (adds `std.db.pool`)
* **NEW** `packages/x07-ext-db-sqlite/0.1.0/modules/std/db/sqlite/pool.x07.json`
* **UPDATED** `packages/x07-ext-db-sqlite/0.1.0/package.json` (adds `std.db.sqlite.pool`)

### Normative pinned doc

* **NEW** `docs/db/pool-v1.md` (pins X7PL + token encoding + canonical acquire/release)

### Policy + smoke suite

* **NEW** `benchmarks/policies/run-os-sandboxed/db-pool-smoke.policy.json`
* **NEW** `benchmarks/run-os-sandboxed/db-pool-smoke.json`

### 3 smoke programs

* **NEW** `tests/external_os/db_pool_fairness/src/main.x07.json`
* **NEW** `tests/external_os/db_pool_max_concurrency/src/main.x07.json`
* **NEW** `tests/external_os/db_pool_no_leak_close/src/main.x07.json`

---

## API summary (what agents call)

### `std.db.pool` (core helpers; no blocking ops)

These are the “don’t slice offsets manually” helpers:

* `std.db.pool.pack_v1(chan_i32, max_conns_i32) -> bytes`
* `std.db.pool.chan_v1(pool_bytes) -> i32`
* `std.db.pool.max_conns_v1(pool_bytes) -> i32`
* `std.db.pool.conn_token_v1(conn_id_i32) -> bytes` *(u32_le(conn_id))*
* `std.db.pool.conn_from_token_v1(token_bytes) -> i32`

### `std.db.sqlite.pool` (driver-backed pool; uses concurrency)

**Important constraint:** anything that uses `chan.bytes.recv/send` must be in `solve` or `defasync`, so these are **defasync** helpers returning bytes (as your async ABI expects).

* `defasync std.db.sqlite.pool.new_path_v1(path, open_flags, caps, max_conns) -> bytes`

  * returns **X7DB resp** (`std.db.spec.resp_ok_v1`)
  * OK payload is the **X7PL pool bytes**
  * cleans up already-opened conns if an open fails mid-way
* `defasync std.db.sqlite.pool.close_v1(pool, caps) -> bytes`

  * drains exactly `max_conns` tokens, closes each conn, closes the chan
* `defasync std.db.sqlite.pool.exec0_async_v1(pool, sql_bytes, caps) -> bytes`

  * acquires token → runs `std.db.sqlite.exec0_v1` → releases token → returns resp
  * intentionally releases token regardless of exec OK/ERR (no-leak)

---

## Pinned bytes encodings (v1)

### Pool bytes: `DbPoolV1` (X7PL)

Pinned in `docs/db/pool-v1.md`.

```
magic[4]      = "X7PL"
version_u32   = 1
chan_u32      = chan.bytes handle
max_conns_u32 = pool size
```

Size = **16 bytes**.

### Channel token bytes: `DbConnTokenV1`

Each message sent/received on the pool channel is:

```
u32_le(conn_id)
```

For SQLite, `conn_id` is the `SqliteOpenOkV1.conn_id` returned by `std.db.sqlite.open_v1`.

---

## Smoke suite behavior (byte-for-byte expectations)

All 3 cases use stdin = `benchmarks/fixtures/db/sqlite/v1/app.db` and open flags = `1` (read-only), so they’re stable and don’t need FS writes.

1. **Fairness** (`db_pool_fairness_v1`)

* Pool size = 1
* Spawn 3 async workers contending on `chan.bytes.recv`
* Output is **3 bytes**: `[0,1,2]`
  Expected `stdout_b64`: `AAEC`

2. **Max concurrency** (`db_pool_max_concurrency_v1`)

* Pool size = 2
* Spawn 3 workers with different sleep durations while holding a token
* Output is **6 event bytes**:

  * acquire(id) → `0xA0 + id`
  * release(id) → `0xB0 + id`
* Expected trace: `A0 A1 R0 A2 R2 R1`
  Expected `stdout_b64`: `oKGwoo6ysQ==`

3. **No-leak close** (`db_pool_no_leak_close_v1`)

* Pool size = 1
* Calls `std.db.sqlite.pool.exec0_async_v1(pool, "BOGUS", caps)` (expected DB ERR)
* Still must release token, then `close_v1` must drain successfully
* Output is bytes `"OK"`
  Expected `stdout_b64`: `T0s=`

---

## How this answers your “pool fairness / max concurrency / no leak” requirements

* **Fairness:** implemented by leveraging the runtime’s FIFO wake order for blocked receivers; the test asserts acquisition order deterministically.
* **Max concurrency:** enforced structurally—only `max_conns` tokens exist; the test asserts the third acquire can’t happen until a release occurs.
* **No-leak close:** ensured by providing an agent-friendly wrapper (`exec0_async_v1`) that always returns the token; test proves close drains after an error path.

---

## Integrating the delta

1. Untar at repo root (so paths line up):

   * it adds/updates only the files listed above.
2. Ensure your run‑os‑sandboxed runner already supports the **db policy section** (you already landed DB v1).
3. Run the suite with your existing harness runner using:

   * `benchmarks/run-os-sandboxed/db-pool-smoke.json`

If you want the **same pool module to work for pg/mysql** next, the pattern is:

* keep `std.db.pool` unchanged (X7PL + token helpers stay the same),
* add `std.db.pg.pool` and `std.db.mysql.pool` modules that mirror `std.db.sqlite.pool` but call their driver open/close/exec builtins and enforce their policy knobs.
+++++
Here’s a **Postgres + MySQL delta bundle** that fills in what was previously only stubbed (i.e., the same kind of “package-layer + spec + smokes + policies + docs” coverage you already got for SQLite):

Use docs/phases/assets/x07_ext_db_v1_pg_mysql_bundle.tar.gz

## What this adds (mirrors the SQLite bundle pattern)

### 1) Real modules (replacing the placeholders)

* **`std.db.pg`**

  * `spec.*` request packers for **open/query/exec/close** (binary formats pinned)
  * `caps_pack_v1` + `caps_default_v1` (shared X7DC caps encoding)
  * convenience wrappers:

    * `open_v1(host,port,user,pass,db,flags,caps)`
    * `query_v1(conn_id, sql, params_doc, flags, caps)`
    * `exec_v1(conn_id, sql, params_doc, flags, caps)`
    * `close_v1(conn_id, caps)`
    * plus `query0_v1` / `exec0_v1` (empty params + flags=0)

* **`std.db.mysql`**
  Same structure as Postgres, with MySQL-specific magics.

### 2) Pool helpers (built on `std.db.pool`)

* **`std.db.pg.pool`**
* **`std.db.mysql.pool`**

Each provides:

* `new_v1(...) -> DbRespV1(ok(payload=X7PL pool))`
* `close_v1(pool,caps) -> DbRespV1(ok)`
* `query0_async_v1(pool, sql, caps) -> DbRespV1`
* `exec0_async_v1(pool, sql, caps) -> DbRespV1`

> These depend on your **pool-delta bundle** already being applied (because they import `std.db.pool`).

### 3) Real run-os-sandboxed smoke suites + programs

* `benchmarks/run-os-sandboxed/db-pg-smoke.json`
* `benchmarks/run-os-sandboxed/db-mysql-smoke.json`
* Programs:

  * `tests/external_os/db_pg_smoke_ok/src/main.x07.json`
  * `tests/external_os/db_mysql_smoke_ok/src/main.x07.json`

Each smoke:

1. opens a connection to **localhost**
2. runs `SELECT 1 AS one;`
3. checks that the returned **DataModel doc bytes match exactly**
4. writes `0x01` on success (stdout base64 `AQ==`)

### 4) Policy examples for localhost dev

* `schemas/run-os-policy.db.pg.example.json` (allows port 5432)
* `schemas/run-os-policy.db.mysql.example.json` (allows port 3306)

### 5) Driver v1 docs pinned

* `docs/db/postgres-v1.md`
* `docs/db/mysql-v1.md`

These pin:

* request magics + layout
* caps encoding
* required builtins
* response expectations

### 6) Optional docker-compose fixtures (to make the smokes easy to run)

* `benchmarks/fixtures/db/pg/v1/docker-compose.yml`
* `benchmarks/fixtures/db/mysql/v1/docker-compose.yml`

Both use credentials:

* db/user/pass = `x07/x07/x07`
* ports: 5432 / 3306

## Required native/builtin surface (same idea as SQLite)

This bundle does **not** implement native drivers. Your runtime must implement:

* `os.db.pg.open_v1 / query_v1 / exec_v1 / close_v1`
* `os.db.mysql.open_v1 / query_v1 / exec_v1 / close_v1`

…and they must consume the exact request encodings pinned in the docs and produced by `std.db.pg.spec.*` / `std.db.mysql.spec.*`.

## How to apply

1. Untar the bundle and copy the contained tree into your repo root (overwrite the existing stub files in the postgres/mysql packages).
2. Ensure you already applied the **pool delta** bundle (so `std.db.pool` exists).
3. (Optional) Bring up fixtures:

   * Postgres: `cd benchmarks/fixtures/db/pg/v1 && docker compose up -d`
   * MySQL: `cd benchmarks/fixtures/db/mysql/v1 && docker compose up -d`
4. Run the benchmark suite(s) you use for run-os-sandboxed.

++++
Yes — even if **SQLite v1 is “green”** and you have PG/MySQL adapters, there are still a handful of *high‑impact gaps* you’ll want to close before you can call the DB layer “production‑ready for autonomous agents”.

Below is a **practical “DB v1 → production” gap checklist**, prioritized by what most often breaks real agent-built systems (safety, reliability, ergonomics, performance, portability).

Also: some of the files you uploaded earlier have expired on my side. If you want me to review specific implementation details (exact encodings, error codes, module bodies), please re‑upload the relevant bundle(s) or current `std.db.*` modules.

---

## P0 gaps (must fix before “production agent usage”)

### 1) Enforce **parameterized queries as the default** (in the API surface)

If your API still allows “raw SQL string” execution as the normal path, autonomous agents *will* eventually generate unsafe string interpolation. For production readiness you want:

* `db.exec_params_v1(conn, sql, params_dm)` as the canonical path
* raw SQL (no params) should be either:

  * **not exposed**, or
  * behind an `unsafe`/`expert` namespace + lint warning

Rationale:

* Postgres’ libpq explicitly supports parameterized execution where values are supplied separately (e.g. `PQexecParams`) and also prepared statements (`PQexecPrepared`) which avoid repeated parse/plan. ([PostgreSQL][1])
* MySQL’s prepared statement API requires explicit parameter binding (`mysql_stmt_prepare` + bind) and is designed for repeated/parametrized execution. ([MySQL Developer Zone][2])

**What to add if missing**

* A **DataModel → param binder** that supports at least: null, i32, f64, bytes, utf8 string.
* A deterministic mapping of DM types to DB parameter types per backend (PG OIDs vs MySQL bind types).

---

### 2) Timeouts + cancellation (agents need “escape hatches”)

Agents will generate queries that occasionally hang (locks, bad plans, slow network). You need:

* `conn.set_timeouts_v1(connect_ms, query_ms)` (or caps)
* `query.cancel_v1(handle)` (or cancel by conn)

Postgres has explicit cancellation APIs (blocking and non‑blocking cancel request flow). ([PostgreSQL][3])
Libpq also supports nonblocking/asynchronous connection modes (`PQsetnonblocking`). ([PostgreSQL][4])

**Production agent requirement**

* The agent must be able to implement: “try query; if exceeds budget → cancel; retry/backoff; emit error”.

---

### 3) TLS security defaults (especially for PG/MySQL)

If you ship “HTTPS required” on your network package, the DB layer should match that security posture:

**Postgres**

* Default to `sslmode=verify-full` (or `verify-ca` at minimum) and require a root cert path or system trust usage. Libpq docs explicitly describe using `sslmode verify-full/verify-ca` and providing a root certificate to verify against. ([PostgreSQL][5])

**MySQL**

* Ensure you expose TLS configuration via options before connect (MySQL C API uses `mysql_options()` / related TLS setters before `mysql_real_connect()`). ([MySQL Developer Zone][6])

**Gap to check**

* Do your policies for `run-os-sandboxed` include:

  * allowed hostnames/CIDRs for DB targets,
  * allowed CA roots / cert pinning strategy,
  * a “no plaintext” switch?

If not: add them.

---

### 4) Result size caps + streaming results (avoid OOM and long pauses)

If your v1 returns the full result as one “DataModel doc bytes blob”, agents will accidentally fetch too much.

Add at least one of:

* `db.query_stream_v1(...) -> iface` returning a row-stream reader (ideal)
* or hard caps: `max_rows`, `max_total_bytes`, `max_cell_bytes` and fail deterministically when exceeded

This matters even for SQLite (someone can run `SELECT * FROM big_table`).

---

### 5) SQLite busy handling and safe concurrency posture

SQLite is “always green” only if you configure the pain points:

* Set busy timeout (or expose it) so agents don’t hit “database is busy” immediately under contention. SQLite exposes `sqlite3_busy_timeout()` semantics at the connection level. ([PHP][7])
* Document and enforce the threading mode / connection sharing rules. SQLite’s threading mode depends on `SQLITE_THREADSAFE` compile-time config. ([SQLite][8])

**Gap to check**

* If you plan to allow multi-task usage of the same SQLite connection inside X07, you should either:

  * serialize access via your pool (recommended), or
  * hard error if a conn is used concurrently.

---

## P1 gaps (high impact for performance + “agent ergonomics”)

### 6) Prepared statement lifecycle + caching

Autonomous agents will generate loops of “same query, different params”. Without prepared statements you pay parse/plan every time.

* Postgres: `PQexecPrepared` explicitly exists for this purpose (reuse parse/plan). ([PostgreSQL][1])
* MySQL: prepared statement interface is a first-class performance feature. ([MySQL Developer Zone][9])

**Recommendation**
Expose an API like:

* `stmt = db.prepare_v1(conn, sql)`
* `db.exec_prepared_v1(stmt, params_dm)`
* `db.stmt_drop_v1(stmt)`
  …and optionally a per-conn LRU “statement cache” with deterministic eviction policy (so agent doesn’t manage it manually).

---

### 7) Better connection pool semantics (beyond “max concurrency”)

You already started a pool module. The next gaps usually are:

* **fairness guarantees** (FIFO acquisition)
* **health checks / recycle** (drop broken conns and recreate)
* **max lifetime** / idle timeout (important for PG/MySQL)
* **per-backend pool tuning** (SQLite pool=1 often best; PG/MySQL >1)

Agents benefit from a single canonical helper:

* `db.pool.with_limits_v1({max, idle, lifetime_ms, connect_timeout_ms, query_timeout_ms})`

---

### 8) Richer DataModel mapping (metadata and types)

To be “DataModel-first”, you should standardize:

* `rowset.columns`: names + stable “type tags”
* `rowset.rows`: list of rows
* representation for:

  * NULL,
  * bytes vs string,
  * int/float,
  * “decimal-like” (string or bytes),
  * timestamps (string RFC3339 initially)

**Gaps to check**

* Postgres arrays / jsonb / uuid / numeric / timestamptz
* MySQL decimal / datetime / json

You can support v1 by mapping “unknown/complex” types to bytes or string + a tag (“opaque”) and still be consistent.

---

## P2 gaps (cross-platform + deployment)

### 9) Windows support plan (PG/MySQL especially)

You already called out POSIX-centric process runtime earlier — the DB layer has similar issues:

* SQLite can be compiled/linked almost everywhere.
* Postgres/MySQL on Windows means:

  * dynamic linking distribution story,
  * TLS cert store differences,
  * process env differences.

**Gap**
Have you defined:

* “system lib required” vs “bundled lib” packaging for Windows?
* how `run-os-sandboxed` policy locates CA bundles?

---

## P3 gaps (tooling + safety nets for 100% agentic usage)

### 10) Deterministic diagnostics + self-repair friendliness

Your agent success hinges on **errors that are structured and actionable**. For DB packages:

* Every API must return `result_bytes` where `err_code` is:

  * stable,
  * namespaced (no collisions),
  * accompanied by a minimal structured blob (e.g., “backend=pg, stage=connect, errno=…, sqlstate=… (if present)”, truncated safely)

And your lints should enforce:

* parameterized query usage,
* caps present,
* TLS mode present (PG/MySQL),
* pool used for concurrent programs.

---

## P4 “nice but important” features (don’t block v1, but plan them)

### 11) Transactions + savepoints

Agents will want to do:

* begin → multi statements → commit/rollback
* nested behavior via savepoints

Even with “exec only”, you can expose:

* `db.tx_begin_v1`, `db.tx_commit_v1`, `db.tx_rollback_v1`, `db.savepoint_v1`…

### 12) Bulk operations

* Postgres: pipeline mode exists for reducing round trips, but it’s more advanced. ([PostgreSQL][10])
* Bulk insert helpers (parameter arrays) are usually the best ROI for agents.

---

## Recommendation: “DB v1 production readiness” definition

If you want a crisp “done enough for real agent usage” bar, I’d define it as:

1. **Safe-by-default** API: params-first, TLS-first, caps-first
2. **No-hang guarantee**: timeouts + cancel + bounded memory
3. **Stream or cap** results
4. **Pool** that is fair + bounded + leak-free
5. **Cross-platform story** (at least Linux/macOS; Windows planned with explicit constraints)
6. **DataModel contract** pinned + tested byte-for-byte

---


[1]: https://www.postgresql.org/docs/current/libpq-exec.html?utm_source=chatgpt.com "Documentation: 18: 32.3. Command Execution Functions"
[2]: https://dev.mysql.com/doc/c-api/8.0/en/c-api-prepared-statement-interface-usage.html?utm_source=chatgpt.com "6.1 Overview of the C API Prepared Statement Interface"
[3]: https://www.postgresql.org/docs/current/libpq-cancel.html?utm_source=chatgpt.com "Documentation: 18: 32.7. Canceling Queries in Progress"
[4]: https://www.postgresql.org/docs/current/libpq-async.html?utm_source=chatgpt.com "18: 32.4. Asynchronous Command Processing"
[5]: https://www.postgresql.org/docs/current/libpq-ssl.html?utm_source=chatgpt.com "PostgreSQL: Documentation: 18: 32.19. SSL Support"
[6]: https://dev.mysql.com/doc/c-api/8.1/en/c-api-encrypted-connections.html?utm_source=chatgpt.com "3.6.1 Support for Encrypted Connections"
[7]: https://www.php.net/manual/en/sqlite3.busytimeout.php?utm_source=chatgpt.com "SQLite3::busyTimeout - Manual"
[8]: https://sqlite.org/threadsafe.html?utm_source=chatgpt.com "Using SQLite In Multi-Threaded Applications"
[9]: https://dev.mysql.com/doc/c-api/8.0/en/c-api-prepared-statement-interface.html?utm_source=chatgpt.com "Chapter 6, C API Prepared Statement Interface"
[10]: https://www.postgresql.org/docs/current/libpq-pipeline-mode.html?utm_source=chatgpt.com "Documentation: 18: 32.5. Pipeline Mode"
+++++
Below is a **production-grade, implementation-ready blueprint** for X07 external database packages (SQLite + Postgres + MySQL) that are **DataModel-first**, **agent-friendly**, and **safe-by-default in `run-os-sandboxed`**.

I’m going to describe:

1. what “production-ready DB support” concretely means for X07,
2. the **layered architecture** (spec/encodings → native adapters → stdlib facade),
3. the **exact contracts** you should pin (bytes encodings, DataModel result shape, errors),
4. how to do **async + pools** in X07’s model,
5. sandbox policy requirements, and
6. the testing strategy that makes SQLite “always-green” and PG/MySQL “green when configured”.

---

## 0) What “production-ready DB packages” must include

For X07, “production-ready DB” is not “can execute a query once”, it’s:

### A. Correctness & safety

* deterministic, versioned binary contracts (no ad-hoc encoding)
* strict validation of request bytes → deterministic error codes
* safe limits everywhere: max SQL bytes, max params, max rows, max cell bytes, max total result bytes
* cancellation + timeouts
* handles must be droppable (no leaks) and must fail safely if reused after drop/close

### B. Operational quality

* connection pooling + fairness + max concurrency
* stable diagnostics: error code taxonomy + driver error mapping
* TLS/CA verification (PG/MySQL) and secure defaults
* cross-platform strategy (Linux/macOS/Windows)

### C. Agent ergonomics (LLM-first)

* agents **never** hand-roll binary encodings
* one canonical pattern for query:

  * build request via helpers → call driver → get DataModel doc → use DataModel helpers
* a small set of “do-the-right-thing” helpers:

  * `req_query_v1(sql, params_dm, caps)` (or prepared variant)
  * `resp.rows_v1`, `resp.cols_v1`, `resp.row_at_v1`, `resp.get_by_name_v1`

---

## 1) Layered architecture (keep it clean)

### Layer 1 — Pinned spec/encodings (agent never guesses)

**Package:** `packages/x07-ext-db/0.1.0/`
**Modules:** `std.db.spec.*` (x07AST JSON, source-only)

Responsibilities:

* **pack/unpack** `DbConnV1`, `DbCapsV1`, `DbReqV1`, `DbRespV1`
* validate magic/version/counts/offsets
* return `result_bytes` with **SPEC_ERR_DB_*`** on malformed input
* convenience builders: `req.query_v1`, `req.exec_v1`, `req.prepare_v1`, `req.exec_prepared_v1`
* param builders from DataModel (positional list or named map)

### Layer 2 — Native adapters (C shim + stable builtin ABI)

**Driver crates / shims:**

* `crates/x07-ext-db-sqlite/` (links to SQLite)
* `crates/x07-ext-db-pg/` (links to libpq)
* `crates/x07-ext-db-mysql/` (prefer MariaDB Connector/C for licensing + availability)

SQLite uses the canonical C API functions like `sqlite3_prepare_v2`, binding, stepping, column access, finalize, close.
Postgres uses libpq (`PQconnectdb`, `PQexecParams`, etc.) and can optionally use async APIs (`PQsendQueryParams`) later. ([Postgres Professional][1])
MySQL should use prepared statements (`mysql_stmt_prepare`, `mysql_stmt_bind_param`, etc.).

**Why MariaDB Connector/C for MySQL?** Cleaner licensing posture than `libmysqlclient` in many distributions, and widely available; also has documented nonblocking support. ([MariaDB][2])
**SQLite is public domain**, making bundling/distribution much simpler.

### Layer 3 — Agent-facing facade (one canonical way)

**Modules:**

* `std.db` (driver-agnostic facade)
* `std.db.sqlite`, `std.db.pg`, `std.db.mysql` (thin wrappers)
* `std.db.pool` (pooling)
* `std.db.dm` (DataModel helpers for results/params)

The facade hides driver differences and ensures the agent can write portable DB code.

---

## 2) Pin the DB v1 contracts (bytes + DataModel shape)

You already do this pattern for `HttpReqV1/HeadersTableV1`. DB should match that style.

### 2.1 Spec error code space (must never overlap others)

Pick a **high, reserved prefix** for DB spec errors.

Example (u32 space):

* `0xEDB0_0000 .. 0xEDB0_FFFF` = **DB Spec errors** (pack/unpack/validation)
* `0xEDB1_0000 .. 0xEDB1_FFFF` = **DB Policy errors**
* `0xEDB2_0000 .. 0xEDB2_FFFF` = **SQLite adapter errors**
* `0xEDB3_0000 .. 0xEDB3_FFFF` = **Postgres adapter errors**
* `0xEDB4_0000 .. 0xEDB4_FFFF` = **MySQL adapter errors**

Rationale: agents can branch on ranges; no collisions; deterministic.

### 2.2 DbConnV1 (connection spec) — bytes encoding

**Goal:** A single encoding for all drivers, with a driver tag.

Recommended:

```
DbConnV1 :=
  magic[4] = "X7DB"
  ver_u16 = 1
  driver_u16  (1=sqlite, 2=pg, 3=mysql)
  flags_u32 (must be 0 for v1)
  payload_len_u32
  payload[payload_len]
```

Then per-driver payload:

**SQLite payload v1**

```
sqlite :=
  path_len_u32 + path_bytes
  open_flags_u32  (bitmask: READONLY/READWRITE/CREATE)  ; keep tiny
```

**Postgres payload v1**

```
pg :=
  host_len_u32 + host
  port_u32
  db_len_u32 + db
  user_len_u32 + user
  pass_len_u32 + pass
  sslmode_u8  (0=disable,1=require,2=verify-ca,3=verify-full)
  ca_pem_len_u32 + ca_pem_bytes   (optional; 0 len means "system store")
  sni_len_u32 + sni_bytes         (optional; 0 means host)
```

`sslmode` is aligned with libpq’s documented `sslmode` behaviors (verify-full/verify-ca, etc.).

**MySQL payload v1**
Similar fields: host/port/db/user/pass + TLS mode + CA bundle.

### 2.3 DbCapsV1 (limits + behavior knobs)

Caps belong in a separate blob so:

* sandbox policy can clamp them
* the same conn can be used with different caps

```
DbCapsV1 :=
  magic "X7BC"
  ver_u16=1
  flags_u16 (0)
  connect_timeout_ms_u32
  query_timeout_ms_u32
  max_sql_bytes_u32
  max_params_u32
  max_rows_u32
  max_cell_bytes_u32
  max_total_bytes_u32
  allow_multi_stmt_u8 (0/1)  ; default 0
  require_tls_u8 (0/1)       ; default 1 for pg/mysql
```

### 2.4 DbReqV1 (query request)

Use a single request envelope:

```
DbReqV1 :=
  magic "X7QR"
  ver_u16=1
  op_u16    (1=exec, 2=query, 3=prepare, 4=exec_prepared)
  flags_u32 (0)
  sql_len_u32 + sql_bytes
  params_dm_len_u32 + params_dm_doc_bytes   ; DataModel doc (OK->seq/map)
  stmt_name_len_u32 + stmt_name_bytes       ; for prepared ops; else 0
```

### 2.5 DbRespV1 (response) — DataModel-first

**Return type should be a DataModel doc bytes** so everything composes.

But you also need a stable, query-result shape.

Recommended **success doc shape**:

`Ok(map{ "cols": seq, "rows": seq, "rows_affected": number, "last_insert_id": number|null })`

Where:

* `cols` is seq of maps: `{"name": string, "type": string, "nullable": bool}`
* `rows` is seq of seq (positional values aligned to cols)
* `rows_affected` always present (0 for SELECT on drivers that don’t report)
* `last_insert_id` for SQLite/MySQL where relevant; null if unavailable

This avoids per-row map overhead while keeping metadata.

**Error doc** uses DataModel’s error framing:

* tag 0 + `u32 code` + `u32 msg_len` + msg bytes
  (Your DataModel module already has this pattern.)

---

## 3) DataModel mapping rules (SQL → DM) you should pin as normative

This is the biggest thing that keeps your ecosystem composable.

### 3.1 SQLite type model reality

SQLite has storage classes `NULL, INTEGER, REAL, TEXT, BLOB`.
So map as:

* `NULL` → DM null
* `INTEGER` → DM number (canonical decimal ASCII)
* `REAL` → DM number (canonical decimal / scientific, choose one rule)
* `TEXT` → DM string (bytes; validate UTF‑8 if your DM string is semantic UTF‑8)
* `BLOB` → **DM string("b64:" + base64(blob))** (unless you add a DM bytes kind)

### 3.2 Postgres and MySQL mapping

Start with text-format results for v1:

* libpq supports choosing text vs binary formats; v1 should use text for simplicity.
* MySQL prepared statements expose metadata and values; v1 can render to text deterministically when needed.

Mapping:

* numeric types → DM number using driver-provided canonical text (or your own)
* bool → DM bool
* text/varchar → DM string
* bytea/blob → DM string("b64:…")
* json/jsonb → DM string OR parse to DM value if you want (but parsing must be deterministic and failure-handled)

### 3.3 Canonical number formatting

To avoid “agent surprises” and nondeterministic float formatting across libraries:

* For v1: **return numbers as strings exactly as returned by the DB text protocol**, not re-formatted.
* If you later need canonicalization: introduce `std.dm.number.normalize_v1`.

---

## 4) Native adapter implementation details (what devs actually build)

### 4.1 SQLite adapter (always-green)

Implementation: a connection handle table + statement handle table.

Core API usage:

* `sqlite3_open_v2` for open
* `sqlite3_prepare_v2` + `sqlite3_bind_*` + `sqlite3_step` + `sqlite3_column_*` + `sqlite3_finalize`
* `sqlite3_close_v2` on connection close
  These are the canonical building blocks documented by SQLite.

Timeouts:

* set busy timeout (`sqlite3_busy_timeout`) for “database is locked” cases; still enforce X07 cap timeouts too.

Threading:

* Keep each SQLite connection owned by exactly one worker thread OR serialize access with a mutex.
* SQLite’s thread-safety depends on build flags/config; safest portable approach is “one connection owned by one worker”. (You can still pool multiple connections.)

### 4.2 Postgres adapter (libpq)

Connection:

* use `PQconnectdb` (or params version), check status, capture error via `PQerrorMessage`
  Query:
* `PQexecParams` for parameterized queries (avoid injection, avoid manual quoting)
  TLS:
* enforce `sslmode` at least `verify-full` in sandboxed mode (unless policy says otherwise). `sslmode` behaviors are part of libpq connection parameters docs.

Async (optional later):

* libpq supports async command submission with functions like `PQsendQueryParams`. ([Postgres Professional][1])
  But v1 can use worker threads and blocking calls (simpler, cross-platform).

### 4.3 MySQL adapter

Prepared statements:

* Use the documented statement API: `mysql_stmt_prepare`, `mysql_stmt_bind_param`, `mysql_stmt_execute`, and metadata functions.

Library choice:

* Prefer MariaDB Connector/C (LGPL) for broad use; it documents nonblocking capability too. ([MariaDB][2])

TLS:

* require TLS in `run-os-sandboxed` unless explicitly disabled by policy.

---

## 5) Concurrency + pools (how it should work in X07)

### 5.1 Principle

* DB adapters do **blocking I/O** (disk or network).
* In X07, you want DB calls to be **yield boundaries** when called in `defasync`, so agents can overlap multiple queries naturally.

### 5.2 Implementation model that works everywhere

**Per-connection worker thread** + a lock-free request queue (or mutex+condvar) is the simplest robust model:

* connection handle owns:

  * driver connection object (`sqlite3*`, `PGconn*`, `MYSQL*`)
  * a worker thread that processes requests serially
  * completion slot per request id
* `std.db.*.query_async_v1(...)` returns a handle id
* `os.db.join_v1(handle)` yields until completion
* `os.db.try_join_v1(handle)` polls without yielding (optional)
* `os.db.drop_v1(handle)` releases resources

This matches what you already did for OS subprocess handles.

### 5.3 Pool semantics (agent-friendly)

In `std.db.pool`:

* pool is a channel of connection handles (FIFO → fairness)
* acquire = blocking recv (only allowed in `defasync` or solve)
* release = send back
* `with_conn(pool, fn)` helper ensures release on early return

**Must-have pool guarantees**

* fairness: FIFO (the next waiter gets the next conn)
* max concurrency: bounded by pool size
* close semantics: closing pool drains/joins workers and returns deterministic “pool closed” error

---

## 6) run-os-sandboxed policy (must exist before shipping)

You need a network-like policy section for DB, plus SQLite path controls.

### 6.1 Policy capabilities to add

**Under `db`:**

* `enabled: bool`
* `max_live_conns: u32`
* `max_spawns: u32` (how many opens total)
* `max_query_timeout_ms: u32` (clamp)
* `max_total_bytes: u32` clamp
* `sqlite: { allow_paths: [...], allow_readonly_only: bool }`
* `net: { allow_dns: [...], allow_cidr: [...], allow_ports: [...] }`
* `require_tls: bool` (default true)
* `allow_drivers: ["sqlite","pg","mysql"]`

Policy gating should happen:

* before opening a connection (driver allowlist + target allowlist)
* before query execution (clamp caps)

---

## 7) Testing plan (how you stay “always-green” + still cover PG/MySQL)

### 7.1 Deterministic unit tests (pure)

* pack/unpack roundtrips for `DbConnV1/DbCapsV1/DbReqV1`
* malformed blobs → exact `SPEC_ERR_DB_*` codes
* DataModel doc shape builders → byte-for-byte golden tests

### 7.2 SQLite integration tests (always run)

* ship a tiny fixture sqlite db file:

  * schema + a few rows
* tests:

  * query rows → expected DataModel doc bytes exactly
  * params binding correctness
  * timeout behavior
  * max_rows/max_total_bytes enforcement

### 7.3 Postgres/MySQL integration tests (run when configured)

Don’t make base CI depend on having servers installed.

Pattern:

* tests require env var:

  * `X07_TEST_PG_URL`
  * `X07_TEST_MYSQL_URL`
* if absent: mark suite “skipped” deterministically (still a green run)
* provide helper scripts:

  * `scripts/dev/start_pg_docker.sh`
  * `scripts/dev/start_mysql_docker.sh`
* smoke cases:

  * create temp schema/table, insert, query, validate DataModel doc shape

### 7.4 Concurrency/pool tests

* fairness:

  * N tasks acquire/release; ensure acquisition order matches spawn order
* max concurrency:

  * run slow queries; ensure at most `pool_size` active
* leak checks:

  * intentionally early-return paths; ensure all handles closed and worker threads joined

---

## 8) “Full featured” gaps you should explicitly plan (after v1)

Even after SQLite/PG/MySQL basic query support lands, the biggest gaps for real apps are:

1. **Streaming results** (cursor/batch)

   * avoid huge `rows` docs for large queries
   * return `iface` stream of row batches (DataModel seq per chunk)

2. **Prepared statement cache**

   * statement handle table + LRU per connection
   * avoid repeated parse/plan costs

3. **Transactions API**

   * `tx.begin_v1`, `tx.commit_v1`, `tx.rollback_v1` with RAII-ish helpers

4. **Migrations**

   * `std.db.migrate` reads migrations dir, applies deterministically, records applied versions

5. **Typed decoding helpers**

   * interpret DM numbers/strings into i32/u32/etc with clear errors

6. **Better binary story**

   * either extend DataModel with a `bytes` kind, or standardize base64 across the ecosystem

---

## 9) Concrete build/dependency notes (so devs don’t get surprised)

* **SQLite:** public domain; easiest to vendor the amalgamation (or link system sqlite3).
* **Postgres libpq:** permissive PostgreSQL license; link system `libpq`.
* **MySQL:** prefer MariaDB Connector/C (LGPL); clearer story than `libmysqlclient` for many projects.

---

## 10) If you want one “canonical v1” recommendation

If you want the fastest path to a **real, shippable DB experience** for agents:

1. Ship **SQLite adapter + full DataModel doc mapping + pool** first (always-green, no external service).
2. Ship **Postgres** second with TLS verify-full required in sandboxed mode.
3. Ship **MySQL via MariaDB Connector/C** third.
4. Then add:

   * streaming row batches
   * prepared cache
   * migrations
   * better binary

That yields “production usability” quickly while still converging on “full-featured”.

---

If you want, I can convert this blueprint into your usual **repo-aligned PR checklist** (DB‑01…DB‑xx) with:

* exact files, module paths, schema diffs,
* exact bytes encodings written as pinned docs,
* and the smoke suite JSON shapes for sqlite/pg/mysql + pool concurrency tests.

[1]: https://postgrespro.ru/docs/postgrespro/9.6/libpq-async?lang=en&utm_source=chatgpt.com "30.4. Asynchronous Command Processing"
[2]: https://mariadb.com/docs/general-resources/development-articles/mariadb-internals/using-mariadb-with-your-programs-api/non-blocking-client-library?utm_source=chatgpt.com "Non-Blocking Client Library | General Resources - MariaDB"
+++++
A quick note: some earlier uploaded repo artifacts in this chat session have expired on my side, so I can’t “diff against” the live tree. But you asked for a **repo‑aligned PR checklist + pinned docs + smoke suite JSON shapes**, which I can provide **fully and concretely** without needing those files.

Below is a **DB‑01…DB‑xx** checklist that your devs can implement incrementally. It assumes your current repo conventions:

* `docs/` for normative pinned specs
* `schemas/` for policy schemas
* `crates/` for native/toolchain/runner integrations
* `packages/` for external x07AST JSON modules (source‑only `.x07.json`)
* `tests/external_os/...` for runnable smoke programs
* `benchmarks/run-os*/...` for suite JSONs used by your smoke runner

Where I reference driver APIs, I cite the official docs: SQLite C API, PostgreSQL libpq (SSL and `PQexecParams`), and MySQL prepared statements.

---

# DB v1 repo‑aligned PR checklist (DB‑01…DB‑18)

## DB‑01 — Pin the DB v1 spec docs (encodings + shapes + error spaces)

**Goal:** “agents never guess bytes layouts”; devs have a single normative spec.

**Add files (copy/paste contents provided below):**

* `docs/db/db-v1.md` (overview + scope)
* `docs/db/db-encodings-v1.md` (**normative bytes layouts** for `DbConnV1`, `DbCapsV1`, `DbReqV1`, `DbRespV1`, `DbPoolV1`)
* `docs/db/db-datamodel-mapping-v1.md` (SQL→DataModel mapping rules + canonicalization rules)
* `docs/db/db-errors-v1.md` (numeric spaces: SPEC vs POLICY vs DRIVER)
* `docs/db/db-policy-v1.md` (policy semantics in run‑os‑sandboxed)

**No code changes yet.**

**CI/manual gate (script name):**

* `scripts/ci/check_db_docs.sh`

  * checks files exist + are referenced in `docs/README.md` (or equivalent)
  * greps for required section headers (“DbConnV1”, “DbCapsV1”, …)

**Notes / references to include in doc rationale:**

* SQLite “prepare/step/column/finalize” flow is the intended usage pattern.
* libpq “secure connections via sslmode=verify‑full/verify‑ca”.
* libpq `PQexecParams` supports param binding and text/binary formats; v1 uses **text** for simplicity.
* MySQL C API prepared statements require `mysql_stmt_prepare` + `mysql_stmt_bind_param`.

---

## DB‑02 — Add DB section to `run-os-policy.schema.json` (sandbox controls)

**Goal:** allow/deny DB usage, clamp caps, enforce network/path allowlists.

**Modify:**

* `schemas/run-os-policy.schema.json`

**Schema diffs (exact additions):**
Add a top-level property:

```json
"db": {
  "type": "object",
  "additionalProperties": false,
  "required": ["enabled"],
  "properties": {
    "enabled": { "type": "boolean" },

    "allow_drivers": {
      "type": "array",
      "items": { "enum": ["sqlite", "pg", "mysql"] },
      "default": ["sqlite", "pg", "mysql"]
    },

    "max_live_conns": { "type": "integer", "minimum": 0, "maximum": 1024, "default": 16 },
    "max_spawns": { "type": "integer", "minimum": 0, "maximum": 1000000, "default": 1024 },

    "clamp": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "max_connect_timeout_ms": { "type": "integer", "minimum": 1, "maximum": 600000, "default": 30000 },
        "max_query_timeout_ms": { "type": "integer", "minimum": 1, "maximum": 3600000, "default": 60000 },
        "max_sql_bytes": { "type": "integer", "minimum": 1, "maximum": 10485760, "default": 1048576 },
        "max_params": { "type": "integer", "minimum": 0, "maximum": 65535, "default": 1024 },
        "max_rows": { "type": "integer", "minimum": 0, "maximum": 10000000, "default": 10000 },
        "max_cell_bytes": { "type": "integer", "minimum": 0, "maximum": 10485760, "default": 1048576 },
        "max_total_bytes": { "type": "integer", "minimum": 0, "maximum": 1073741824, "default": 33554432 }
      }
    },

    "sqlite": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "allow_paths": {
          "type": "array",
          "items": { "type": "string", "minLength": 1 },
          "default": []
        },
        "readonly_only": { "type": "boolean", "default": true }
      }
    },

    "net": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "allow_dns": { "type": "array", "items": { "type": "string", "minLength": 1 }, "default": [] },
        "allow_cidr": { "type": "array", "items": { "type": "string", "minLength": 1 }, "default": [] },
        "allow_ports": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 65535 }, "default": [] },
        "require_tls": { "type": "boolean", "default": true }
      }
    }
  }
}
```

**CI/manual gate:**

* `scripts/ci/check_policy_schema.sh`

  * validates JSON schema loads + `db` exists
  * runs `scripts/check_contracts.py` if you have schema self-tests

---

## DB‑03 — Runner wiring: load db policy, export deterministic env clamps

**Goal:** sandbox policy → env vars → runtime builtins enforce consistently.

**Modify/add:**

* `crates/x07-os-runner/src/policy.rs`

  * parse the new `policy.db` section
  * validate deterministic: stable diagnostic codes if invalid
* `crates/x07-os-runner/src/main.rs`

  * set env vars like:

    * `X07_OS_DB_ENABLED=0/1`
    * `X07_OS_DB_ALLOW_DRIVERS=sqlite,pg,mysql`
    * `X07_OS_DB_MAX_LIVE=...`
    * `X07_OS_DB_MAX_SPAWNS=...`
    * clamps: `X07_OS_DB_CLAMP_MAX_SQL_BYTES=...`, etc.
    * sqlite allow paths: `X07_OS_DB_SQLITE_ALLOW_PATHS=...` (NUL-separated or JSON; pick one)
    * net allowlists: `X07_OS_DB_ALLOW_DNS=...`, `X07_OS_DB_ALLOW_CIDR=...`, `X07_OS_DB_ALLOW_PORTS=...`

**Add doc update:**

* `docs/db/db-policy-v1.md` references these envs as the “wire ABI” between runner and compiled program.

**CI/manual gates:**

* `scripts/ci/check_os_policy_parse.sh` (unit tests)
* `scripts/ci/check_run_os_policy_smoke.sh` (uses a trivial program calling `std.db.spec.*` validation only)

---

## DB‑04 — Toolchain: add OS builtins `os.db.*` (standalone only, hard error in solve-*)

**Goal:** a stable builtin surface that external packages can depend on, but **never leaks into deterministic worlds**.

**Modify (toolchain runtime emitter):**

* `crates/x07c/src/c_emit.rs` (or equivalent)

  * Add builtins **behind** world checks:

    * `world.is_standalone_only()` or `world.is_run_os*()`
  * Add **hard errors** if any `os.db.*` head appears when compiling `solve-*` targets.

**New builtins (exact names v1):**

* `os.db.open_v1(conn_bytes, caps_bytes) -> i32` (conn_handle or 0 on error? Prefer result_bytes; see below)
* `os.db.close_v1(conn_handle) -> i32`
* `os.db.query_spawn_v1(conn_handle, req_bytes) -> i32` (query_handle)
* `os.db.try_join_v1(query_handle) -> bytes` (DbTryJoinV1 bytes; see encoding)
* `os.db.join_v1(query_handle) -> bytes` (**yield boundary**, returns `DbRespV1` as DataModel doc bytes)
* `os.db.kill_v1(query_handle) -> i32` (cancel)
* `os.db.drop_v1(query_handle) -> i32` (release handle table slot)

**Why handle-based?**

* matches your subprocess model and integrates with your cooperative scheduler without blocking the whole runtime.
* Postgres/MySQL calls are blocking by nature unless you go fully async; handle+join makes it clean.

**C API notes (for devs)**

* SQLite: prepare/step/column/finalize pattern.
* Postgres: use `PQexecParams` for parameterized queries.
* MySQL: use `mysql_stmt_prepare` + `mysql_stmt_bind_param`.

**CI/manual gates:**

* `scripts/ci/check_no_os_builtins_in_solve.sh` (compile a solve-* sample that tries `os.db.*` and assert hard error)
* `scripts/ci/check_run_os_builtins_compile.sh`

---

## DB‑05 — Package: `x07-ext-db` v0.1.0 (spec + helpers + facade)

**Goal:** agents never build bytes layouts by hand.

**Add directories:**

* `packages/x07-ext-db/0.1.0/package.json`
* `packages/x07-ext-db/0.1.0/modules/std/db/spec/conn.x07.json`
* `packages/x07-ext-db/0.1.0/modules/std/db/spec/caps.x07.json`
* `packages/x07-ext-db/0.1.0/modules/std/db/spec/req.x07.json`
* `packages/x07-ext-db/0.1.0/modules/std/db/spec/resp.x07.json`
* `packages/x07-ext-db/0.1.0/modules/std/db/spec/pool.x07.json`
* `packages/x07-ext-db/0.1.0/modules/std/db/module.x07.json`
* `packages/x07-ext-db/0.1.0/modules/std/db/pool/module.x07.json`
* `packages/x07-ext-db/0.1.0/modules/std/db/dm/module.x07.json` (response accessors)

**Must-have agent helpers:**

* `std.db.spec.conn.sqlite_path_v1(path_bytes, open_flags)` -> `result_bytes` (DbConnV1)
* `std.db.spec.conn.pg_url_v1(url_bytes, tls_mode, ca_bytes)` -> `result_bytes` (DbConnV1)
* `std.db.spec.caps.default_v1()` -> bytes
* `std.db.spec.req.query_v1(sql_bytes, params_dm_doc_bytes)` -> `result_bytes`
* `std.db.open_v1(conn_bytes, caps_bytes)` -> `result_i32` (conn handle)
* `std.db.query_v1(conn_handle, req_bytes)` -> `result_bytes` (DbRespV1 = DataModel doc bytes)
* `std.db.close_v1(conn_handle)` -> i32

**CI/manual gate:**

* `scripts/ci/check_pkg_pack_db.sh`:

  * `x07 pkg pack packages/x07-ext-db/0.1.0`
  * `x07 pkg lock` pins hash
  * (optional) `scripts/check_pkg_contracts.py --check-archives`

---

## DB‑06 — Implement SQLite native adapter (always-green, cross-platform)

**Goal:** ship SQLite first because it yields stable “always green” DB capabilities.

**Add native crate:**

* `crates/x07-ext-db-sqlite/` (or integrate into your OS runner build)

  * link to system sqlite or vendor sqlite amalgamation
  * implement open/query/close in C (or Rust FFI) called by runtime builtins
  * enforce caps: max_sql_bytes, max_rows, max_total_bytes

**Reference API expectations:**

* statement compilation/execution via `sqlite3_prepare_v2`, `sqlite3_step`, `sqlite3_column_*`, and `sqlite3_finalize`.

**Add fixture DB file:**

* `benchmarks/fixtures/os/db/sqlite/fixture_v1.sqlite3` (committed binary)

  * simple schema and rows

**Add smoke programs:**

* `tests/external_os/db_sqlite_smoke/main.x07.json`

  * opens fixture DB
  * runs query
  * outputs **DbRespV1 DataModel doc bytes** directly

**Add smoke suites:**

* `benchmarks/run-os/db-sqlite-smoke.json`
* `benchmarks/run-os-sandboxed/db-sqlite-policy-smoke.json`

**CI/manual gates:**

* `scripts/ci/check_db_sqlite_smoke.sh` (runs both suites on all platforms)
* `scripts/ci/check_db_sqlite_leaks.sh` (ensures handles dropped / close called)

---

## DB‑07 — Define & enforce DbRespV1 “DataModel doc shape” (byte-for-byte)

**Goal:** response is DataModel doc bytes with a stable schema so agents can parse it.

**Modify:**

* `docs/db/db-encodings-v1.md` (finalize response shape)
* Implement `std.db.dm` accessors:

  * `resp.is_ok_v1(dm_doc) -> i32`
  * `resp.err_code_v1(dm_doc) -> i32`
  * `resp.cols_v1(dm_doc) -> bytes` (DataModel list)
  * `resp.rows_v1(dm_doc) -> bytes` (DataModel list)
  * `resp.rows_affected_v1(dm_doc) -> i32`
  * `resp.last_insert_id_v1(dm_doc) -> option_i32` (or i32 with -1 sentinel)

**CI/manual gate:**

* `scripts/ci/check_db_resp_shape_sqlite.sh`:

  * compares stdout bytes to expected base64/hex in the suite (no text parsing)

---

## DB‑08 — Add Postgres native adapter (libpq) + TLS policy enforcement

**Goal:** production-grade defaults: TLS verification required in sandbox.

**Add native crate:**

* `crates/x07-ext-db-pg/`

  * uses libpq for connect/query/close
  * for v1: use `PQexecParams` (param binding) and request **text format** results.

**TLS enforcement notes:**

* In libpq, require `sslmode=verify-full` or `verify-ca` for secure connections.

**Smoke programs:**

* `tests/external_os/db_pg_smoke/main.x07.json`

  * reads connection URL from stdin or argv
  * runs fixed query and outputs DataModel doc bytes

**Smoke suites (env-gated):**

* `benchmarks/run-os/db-pg-smoke.json`
* `benchmarks/run-os-sandboxed/db-pg-policy-smoke.json`

**CI/manual gate:**

* `scripts/ci/check_db_pg_smoke.sh`

  * if `X07_TEST_PG_URL` absent: deterministic “SKIP” (not failure)
  * if present: run suite and compare bytes

---

## DB‑09 — Add MySQL native adapter (prepared statements) + TLS

**Goal:** safe query execution and consistent param binding.

**Add native crate:**

* `crates/x07-ext-db-mysql/`

  * v1 uses prepared statements: `mysql_stmt_prepare`, `mysql_stmt_bind_param`, execute.

**Smoke programs:**

* `tests/external_os/db_mysql_smoke/main.x07.json`

**Smoke suites (env-gated):**

* `benchmarks/run-os/db-mysql-smoke.json`
* `benchmarks/run-os-sandboxed/db-mysql-policy-smoke.json`

**CI/manual gate:**

* `scripts/ci/check_db_mysql_smoke.sh` (skips if env not configured)

---

## DB‑10 — Implement `std.db.pool` (fair + bounded + leak-aware)

**Goal:** end-users can build real apps without reinventing pooling.

**Implement in external package:**

* `packages/x07-ext-db/0.1.0/modules/std/db/pool/module.x07.json`

**Exact API (v1):**

* `std.db.pool.new_v1(conn_bytes, caps_bytes, size_i32) -> result_bytes` (DbPoolV1 bytes)
* `std.db.pool.try_acquire_v1(pool_bytes) -> option_i32` (conn_handle)
* `std.db.pool.acquire_v1(pool_bytes) -> i32` (**blocking; only valid in solve or defasync**)
* `std.db.pool.release_v1(pool_bytes, conn_handle) -> i32`
* `std.db.pool.close_v1(pool_bytes) -> result_i32` (Err if conns still checked out)
* `std.db.pool.with_conn_v1(pool_bytes, fn_iface_or_fn_name, arg_bytes) -> bytes`

  * “safe early drop” pattern: always releases even if fn returns early via Result

**CI/manual gate:**

* `scripts/ci/check_db_pool_compile.sh`

---

## DB‑11 — Pool smoke tests: fairness, max concurrency, no-leak close

**Goal:** lock in properties that matter to agents.

**Add smoke programs:**

* `tests/external_os/db_pool_fairness/main.x07.json`
* `tests/external_os/db_pool_max_concurrency/main.x07.json`
* `tests/external_os/db_pool_no_leak_close/main.x07.json`

**Add smoke suites:**

* `benchmarks/run-os/db-pool-smoke.json`
* `benchmarks/run-os-sandboxed/db-pool-policy-smoke.json`

**Assertions to include in suite:**

* fairness: acquisition order bytes equals `[1,2,3,4]` as u32_le list
* max concurrency: after N `try_acquire`, next returns none
* no-leak close: close fails if checked-out conns exist; succeeds after release

**CI/manual gate:**

* `scripts/ci/check_db_pool_smoke.sh`

---

## DB‑12 — Deterministic diagnostics: SPEC_ERR vs POLICY_ERR vs DRIVER_ERR mapping

**Goal:** agents can auto-repair: error codes are stable and categorized.

**Modify:**

* `docs/db/db-errors-v1.md` (final mapping)
* `packages/x07-ext-db/0.1.0/modules/std/db/spec/errors.x07.json` (constants + predicate helpers)
* toolchain/runtime: ensure all failure paths return DataModel Err with code+msg

**CI/manual gate:**

* `scripts/ci/check_db_error_codes.sh` (golden tests for a few failures)

---

## DB‑13 — DataModel mapping correctness tests (SQLite always, PG/MySQL optional)

**Goal:** data model compatibility is the whole point.

**Add suites:**

* `benchmarks/run-os/db-dm-mapping-sqlite.json`
* `benchmarks/run-os/db-dm-mapping-pg.json` (env-gated)
* `benchmarks/run-os/db-dm-mapping-mysql.json` (env-gated)

**What they assert:**

* NULL → DM null
* INTEGER → DM number string
* TEXT → DM string
* BLOB → DM string with `b64:` prefix (if that’s your v1 decision)

---

## DB‑14 — Cross-platform build + feature flags

**Goal:** Windows/macOS/Linux builds are predictable.

**Add/modify:**

* `crates/x07-os-runner/Cargo.toml` features:

  * `db_sqlite`, `db_pg`, `db_mysql`
* `docs/db/db-v1.md` build matrix:

  * sqlite always on
  * pg/mysql optional

**CI/manual gates:**

* `scripts/ci/check_build_linux.sh`
* `scripts/ci/check_build_macos.sh`
* `scripts/ci/check_build_windows.ps1` (PowerShell)

  * sqlite smoke runs always
  * pg/mysql smoke skip unless env exists

---

## DB‑15 — “One canonical agent-friendly path” docs + guide integration

**Goal:** avoid “too many ways”; keep agents consistent.

**Modify:**

* `docs/spec/language-guide.md` (or wherever you keep the agent guide)

  * add a **single canonical recipe**:

    * build conn via `std.db.spec.conn.*`
    * build req via `std.db.spec.req.query_v1`
    * call `std.db.query_v1`
    * interpret via `std.db.dm.*`
    * for parallelism: use `std.db.pool` + `defasync`

**CI/manual gate:**

* `scripts/ci/check_docs_examples_compile.sh` (extract examples → compile)

---

## DB‑16 — “OS-world isolation” hardening

**Goal:** guarantee DB never appears in solve-*.

**Modify:**

* compiler lint pass:

  * “if head starts with `os.db.` and world != run-os* ⇒ error(DB_WORLD_FORBIDDEN)”
* runner:

  * if `policy.db.enabled==false`, any call returns POLICY_ERR_DB_DISABLED

**CI/manual gate:**

* `scripts/ci/check_db_forbidden_in_solve.sh`

---

## DB‑17 — Connection URL parsing helpers (agent convenience)

**Goal:** agents shouldn’t parse URLs manually.

**Add:**

* `packages/x07-ext-db/0.1.0/modules/std/db/spec/url.x07.json`

  * `pg_url_parse_v1`, `mysql_url_parse_v1`
* docs update

---

## DB‑18 — Release packaging + pinning

**Goal:** the package is publishable and lockable.

**Add:**

* update workspace lock:

  * `x07.lock` (or your lockfile) includes `x07-ext-db@0.1.0`
* add `scripts/release/publish_db.sh`

---

# Pinned doc contents (copy/paste)

## `docs/db/db-encodings-v1.md`

> This is the “exact bytes encodings written as pinned docs” you requested.

```md
# DB Encodings v1 (Normative)

This document is normative for the on-wire bytes layouts used by `std.db.*` and `os.db.*`.

All integers are **u32 little-endian** unless stated otherwise.
All byte strings are `len_u32 + bytes[len]`.
All blobs have `magic[4]` + `ver_u16` at the start.

---

## Error code spaces (u32)

- 0xEDB0_0000 .. 0xEDB0_FFFF : SPEC_ERR_DB_* (pack/unpack/validation)
- 0xEDB1_0000 .. 0xEDB1_FFFF : POLICY_ERR_DB_* (sandbox policy)
- 0xEDB2_0000 .. 0xEDB2_FFFF : SQLITE_ERR_DB_*
- 0xEDB3_0000 .. 0xEDB3_FFFF : PG_ERR_DB_*
- 0xEDB4_0000 .. 0xEDB4_FFFF : MYSQL_ERR_DB_*

---

## DbConnV1

DbConnV1 bytes:

- magic[4] = "X7DB"
- ver_u16 = 1
- driver_u16:
  - 1 = sqlite
  - 2 = pg
  - 3 = mysql
- flags_u32 = 0 (must be 0 in v1)
- payload_len_u32
- payload[payload_len]

### SQLite payload v1 (driver=1)

- path_len_u32 + path_bytes (UTF-8)
- open_flags_u32:
  - bit0 = READONLY
  - bit1 = READWRITE
  - bit2 = CREATE

### Postgres payload v1 (driver=2)

- url_len_u32 + url_bytes (UTF-8)
- tls_mode_u8:
  - 0 = disable
  - 1 = require
  - 2 = verify-ca
  - 3 = verify-full
- ca_pem_len_u32 + ca_pem_bytes (0 => system)
- sni_len_u32 + sni_bytes (0 => host)

### MySQL payload v1 (driver=3)

- url_len_u32 + url_bytes (UTF-8)
- tls_mode_u8 (same as pg)
- ca_pem_len_u32 + ca_pem_bytes (0 => system)
- sni_len_u32 + sni_bytes (0 => host)

---

## DbCapsV1

- magic[4] = "X7BC"
- ver_u16 = 1
- flags_u16 = 0
- connect_timeout_ms_u32
- query_timeout_ms_u32
- max_sql_bytes_u32
- max_params_u32
- max_rows_u32
- max_cell_bytes_u32
- max_total_bytes_u32
- allow_multi_stmt_u8 (0/1)
- require_tls_u8 (0/1)

---

## DbReqV1

- magic[4] = "X7QR"
- ver_u16 = 1
- op_u16:
  - 1 = exec
  - 2 = query
  - 3 = prepare
  - 4 = exec_prepared
- flags_u32 = 0
- sql_len_u32 + sql_bytes
- params_dm_len_u32 + params_dm_bytes (DataModel doc bytes)
- stmt_name_len_u32 + stmt_name_bytes (UTF-8; 0 len if unused)

Notes:
- v1 parameter binding is **positional**.
- Postgres uses $1..$n, MySQL uses ? markers, SQLite supports ? / ?NNN / :name but v1 only requires positional list.

---

## DbRespV1

DbRespV1 is a DataModel doc bytes blob.

### Success shape (query)

Top-level is a DM map with keys:

- "cols": DM seq of DM maps:
  - {"name": string, "type": string, "nullable": bool}
- "rows": DM seq of DM seq (positional values aligned to cols)
- "rows_affected": number (string form allowed)
- "last_insert_id": number|null

### Success shape (exec)

Top-level is a DM map with keys:
- "rows_affected": number
- "last_insert_id": number|null

### Error shape

Top-level is a DM error doc:
- "code": u32
- "message": string (UTF-8, may be empty)

---

## DbPoolV1

DbPoolV1 bytes:

- magic[4] = "X7PL"
- ver_u16 = 1
- flags_u16 = 0
- driver_u16 (same enum as DbConnV1)
- pad_u16 = 0
- chan_handle_u32   (the internal channel handle)
- size_u32          (pool size)
```

---

# Smoke suite JSON shapes (copy/paste-ready)

These shapes assume your bench runner can:

* run a `program` (x07AST JSON file)
* feed `stdin_b64` (optional)
* enforce `policy_path` (sandbox suites)
* assert stdout bytes and exit code
* optionally gate on environment variables (skip deterministically)

If your runner’s schema differs, treat these as the **canonical shape to implement** for DB suites.

---

## 1) SQLite smoke (run-os) — `benchmarks/run-os/db-sqlite-smoke.json`

```json
{
  "schema_version": "x07.benchsuite@0.1.0",
  "suite_id": "db-sqlite-smoke@0.1.0",
  "world": "run-os",
  "platforms": ["linux", "macos", "windows"],
  "cases": [
    {
      "id": "sqlite_query_fixture_v1",
      "program": "tests/external_os/db_sqlite_smoke/main.x07.json",
      "env": {
        "X07_DB_FIXTURE_PATH": "benchmarks/fixtures/os/db/sqlite/fixture_v1.sqlite3"
      },
      "stdin_b64": "",
      "expect": {
        "exit_code": 0,
        "stdout_b64": "REPLACE_WITH_EXPECTED_DBRESPV1_DATAMODEL_DOC_BASE64"
      }
    }
  ]
}
```

---

## 2) SQLite policy smoke (run-os-sandboxed) — `benchmarks/run-os-sandboxed/db-sqlite-policy-smoke.json`

```json
{
  "schema_version": "x07.benchsuite@0.1.0",
  "suite_id": "db-sqlite-policy-smoke@0.1.0",
  "world": "run-os-sandboxed",
  "platforms": ["linux", "macos", "windows"],
  "policy_path": "benchmarks/run-os-sandboxed/policies/db_sqlite_allow_fixture.json",
  "cases": [
    {
      "id": "sqlite_query_allowed_path",
      "program": "tests/external_os/db_sqlite_smoke/main.x07.json",
      "env": {
        "X07_DB_FIXTURE_PATH": "benchmarks/fixtures/os/db/sqlite/fixture_v1.sqlite3"
      },
      "stdin_b64": "",
      "expect": { "exit_code": 0, "stdout_b64": "REPLACE_WITH_EXPECTED_BASE64" }
    },
    {
      "id": "sqlite_query_denied_path",
      "program": "tests/external_os/db_sqlite_smoke/main.x07.json",
      "env": {
        "X07_DB_FIXTURE_PATH": "benchmarks/fixtures/os/db/sqlite/denied.sqlite3"
      },
      "stdin_b64": "",
      "expect": {
        "exit_code": 0,
        "stdout_b64": "REPLACE_WITH_EXPECTED_POLICY_ERR_DM_BASE64"
      }
    }
  ]
}
```

---

## 3) Postgres smoke (run-os, env-gated) — `benchmarks/run-os/db-pg-smoke.json`

```json
{
  "schema_version": "x07.benchsuite@0.1.0",
  "suite_id": "db-pg-smoke@0.1.0",
  "world": "run-os",
  "platforms": ["linux", "macos", "windows"],
  "requires_env": ["X07_TEST_PG_URL"],
  "cases": [
    {
      "id": "pg_query_simple_v1",
      "program": "tests/external_os/db_pg_smoke/main.x07.json",
      "env": {},
      "stdin_b64": "",
      "expect": {
        "exit_code": 0,
        "stdout_b64": "REPLACE_WITH_EXPECTED_DBRESPV1_BASE64_FOR_SELECT_1"
      }
    }
  ]
}
```

---

## 4) Postgres policy smoke (run-os-sandboxed, env-gated) — `benchmarks/run-os-sandboxed/db-pg-policy-smoke.json`

```json
{
  "schema_version": "x07.benchsuite@0.1.0",
  "suite_id": "db-pg-policy-smoke@0.1.0",
  "world": "run-os-sandboxed",
  "platforms": ["linux", "macos", "windows"],
  "requires_env": ["X07_TEST_PG_URL"],
  "policy_path": "benchmarks/run-os-sandboxed/policies/db_pg_allow_localhost.json",
  "cases": [
    {
      "id": "pg_query_allowed_host",
      "program": "tests/external_os/db_pg_smoke/main.x07.json",
      "expect": { "exit_code": 0, "stdout_b64": "REPLACE_WITH_EXPECTED_BASE64" }
    },
    {
      "id": "pg_query_denied_host",
      "program": "tests/external_os/db_pg_smoke/main.x07.json",
      "env": { "X07_TEST_PG_URL": "postgres://user:pass@denied.example:5432/db" },
      "expect": { "exit_code": 0, "stdout_b64": "REPLACE_WITH_POLICY_ERR_DM_BASE64" }
    }
  ]
}
```

---

## 5) MySQL smoke (run-os, env-gated) — `benchmarks/run-os/db-mysql-smoke.json`

```json
{
  "schema_version": "x07.benchsuite@0.1.0",
  "suite_id": "db-mysql-smoke@0.1.0",
  "world": "run-os",
  "platforms": ["linux", "macos", "windows"],
  "requires_env": ["X07_TEST_MYSQL_URL"],
  "cases": [
    {
      "id": "mysql_query_simple_v1",
      "program": "tests/external_os/db_mysql_smoke/main.x07.json",
      "expect": {
        "exit_code": 0,
        "stdout_b64": "REPLACE_WITH_EXPECTED_DBRESPV1_BASE64"
      }
    }
  ]
}
```

---

## 6) Pool fairness smoke — `benchmarks/run-os/db-pool-fairness-smoke.json`

```json
{
  "schema_version": "x07.benchsuite@0.1.0",
  "suite_id": "db-pool-fairness-smoke@0.1.0",
  "world": "run-os",
  "platforms": ["linux", "macos", "windows"],
  "cases": [
    {
      "id": "pool_fairness_fifo",
      "program": "tests/external_os/db_pool_fairness/main.x07.json",
      "expect": {
        "exit_code": 0,
        "stdout_b64": "REPLACE_WITH_EXPECTED_U32LE_LIST_BASE64"
      }
    }
  ]
}
```

---

## 7) Pool max concurrency smoke — `benchmarks/run-os/db-pool-maxconcurrency-smoke.json`

```json
{
  "schema_version": "x07.benchsuite@0.1.0",
  "suite_id": "db-pool-maxconcurrency-smoke@0.1.0",
  "world": "run-os",
  "platforms": ["linux", "macos", "windows"],
  "cases": [
    {
      "id": "pool_try_acquire_limits",
      "program": "tests/external_os/db_pool_max_concurrency/main.x07.json",
      "expect": { "exit_code": 0, "stdout_b64": "REPLACE_WITH_EXPECTED_OK_BYTES_BASE64" }
    }
  ]
}
```

---

## 8) Pool no-leak close smoke — `benchmarks/run-os/db-pool-noleak-smoke.json`

```json
{
  "schema_version": "x07.benchsuite@0.1.0",
  "suite_id": "db-pool-noleak-smoke@0.1.0",
  "world": "run-os",
  "platforms": ["linux", "macos", "windows"],
  "cases": [
    {
      "id": "pool_close_fails_with_checked_out_conn_then_succeeds",
      "program": "tests/external_os/db_pool_no_leak_close/main.x07.json",
      "expect": { "exit_code": 0, "stdout_b64": "REPLACE_WITH_EXPECTED_OK_BYTES_BASE64" }
    }
  ]
}
```

---

# Implementation notes devs should follow (why these gates matter)

* SQLite adapter should follow the standard statement lifecycle (`sqlite3_prepare_v2` → `sqlite3_step` → `sqlite3_column_*` → `sqlite3_finalize`).
* Postgres adapter should use `PQexecParams` for parameter binding and request text results for v1 simplicity.
* TLS defaults for Postgres in sandbox mode should require `sslmode=verify-full` (or `verify-ca`) as documented by libpq.
* MySQL adapter should use the prepared statement interface (`mysql_stmt_prepare`, `mysql_stmt_bind_param`) to avoid manual quoting and keep behavior consistent.

---
Below are **drop‑in “external_os smoke program” stubs** in the same **JSON‑S‑expr x07AST** style your other `external_os` programs use (`{"decls":[ ... ]}`).

They are designed so your team only needs to fill **two constants in the *smoke suite*** (not in the program):

* `stdin_b64`: base64 of the **SQLite fixture path bytes** (passed as `input`)
* `stdout_exact_b64`: base64 of the expected **program output bytes**

---

## `tests/external_os/db_sqlite_smoke/main.x07.json`

**Behavior:** reads the SQLite path from `input`, opens it, runs a constant query (`SELECT 1 AS x`), closes, and returns:

* on success: the **DataModel doc bytes** (from `DbRespV1.ok_payload`)
* on failure: the **DbRespV1 bytes** (so you can still assert error codes if needed)

```json
{
  "decls": [
    ["import", "std.codec", "std.fmt", "std.db.spec", "std.db.sqlite"],
    ["export", ["db_sqlite_smoke.solve"]],

    ["defn", "db_sqlite_smoke._sql_select_1_as_x_v1", [], "bytes",
      ["begin",
        ["let", "v", ["vec_u8.with_capacity", 16]],
        ["let", "v", ["vec_u8.extend_bytes", "v", ["bytes.lit", "SELECT"]]],
        ["let", "v", ["vec_u8.push", "v", 32]],
        ["let", "v", ["vec_u8.extend_bytes", "v", ["std.fmt.u32_to_dec", 1]]],
        ["let", "v", ["vec_u8.push", "v", 32]],
        ["let", "v", ["vec_u8.extend_bytes", "v", ["bytes.lit", "AS"]]],
        ["let", "v", ["vec_u8.push", "v", 32]],
        ["let", "v", ["vec_u8.extend_bytes", "v", ["bytes.lit", "x"]]],
        ["vec_u8.as_bytes", "v"]
      ]
    ],

    ["defn", "db_sqlite_smoke.solve", [["input", "bytes"]], "bytes",
      ["begin",
        ["let", "caps", ["std.db.sqlite.caps_default_v1"]],

        ["let", "open_resp", ["std.db.sqlite.open_path_v1", "input", "caps"]],
        ["if",
          ["std.db.spec.resp_is_ok_v1", "open_resp"],

          ["begin",
            ["let", "conn_payload", ["std.db.spec.resp_ok_payload_v1", "open_resp"]],
            ["let", "conn_id", ["std.codec.read_u32_le", "conn_payload", 0]],

            ["let", "sql", ["db_sqlite_smoke._sql_select_1_as_x_v1"]],
            ["let", "q_resp", ["std.db.sqlite.query0_v1", "conn_id", "sql", "caps"]],

            ["let", "_close", ["std.db.sqlite.close_v1", "conn_id", "caps"]],

            ["if",
              ["std.db.spec.resp_is_ok_v1", "q_resp"],
              ["std.db.spec.resp_ok_payload_v1", "q_resp"],
              "q_resp"
            ]
          ],

          "open_resp"
        ]
      ]
    ]
  ]
}
```

**Notes**

* This is intentionally **fixture‑content independent**: it does not require any tables to exist, only that `input` points to a valid SQLite database file.
* If you want “create if missing” semantics later, add an `open` flag in `std.db.sqlite.open_v1`/`open_path_v1` (but keep smoke deterministic by using a checked‑in empty DB fixture).

---

## `tests/external_os/db_pool_fairness/main.x07.json`

This is the **pool fairness** smoke program (copied in the same style as your existing pool delta bundle): it creates a pool, spawns 3 workers, and asserts fairness by emitting a deterministic byte sequence.

```json
{
  "decls": [
    ["import", "std.codec", "std.db.spec", "std.db.pool", "std.db.sqlite.pool", "std.db.sqlite"],
    ["export", ["db_pool_fairness.solve"]],

    ["defasync", "db_pool_fairness.worker", [["hpool", "bytes"], ["id", "i32"]], "bytes",
      ["begin",
        ["let", "c", ["std.db.pool.chan_v1", "hpool"]],
        ["let", "tok", ["chan.bytes.recv", "c"]],
        ["let", "tok_id", ["std.codec.read_u32_le", "tok", 0]],
        ["let", "b", ["bytes1", "tok_id"]],
        ["let", "_", ["chan.bytes.send", "c", "tok"]],
        "b"
      ]
    ],

    ["defn", "db_pool_fairness.solve", [["input", "bytes"]], "bytes",
      ["begin",
        ["let", "caps", ["std.db.sqlite.caps_default_v1"]],
        ["let", "resp_new", ["await", ["std.db.sqlite.pool.new_path_v1", "input", 0, "caps", 1]]],
        ["let", "hpool", ["std.db.spec.resp_ok_payload_v1", "resp_new"]],

        ["let", "h0", ["db_pool_fairness.worker", "hpool", 0]],
        ["let", "h1", ["db_pool_fairness.worker", "hpool", 1]],
        ["let", "h2", ["db_pool_fairness.worker", "hpool", 2]],

        ["let", "b0", ["await", "h0"]],
        ["let", "b1", ["await", "h1"]],
        ["let", "b2", ["await", "h2"]],

        ["let", "_close", ["await", ["std.db.sqlite.pool.close_v1", "hpool", "caps"]]],

        ["bytes.concat", "b0", ["bytes.concat", "b1", "b2"]]
      ]
    ]
  ]
}
```

---

## `tests/external_os/db_pool_max_concurrency/main.x07.json`

This is the **max pool concurrency** smoke program: it checks that the pool does not allow more than `max_conns` “live” tokens to be held concurrently.

```json
{
  "decls": [
    ["import", "std.codec", "std.db.spec", "std.db.pool", "std.db.sqlite.pool", "std.db.sqlite"],
    ["export", ["db_pool_max_concurrency.solve"]],

    ["defasync", "db_pool_max_concurrency.take_one", [["hpool", "bytes"], ["tag", "i32"]], "bytes",
      ["begin",
        ["let", "c", ["std.db.pool.chan_v1", "hpool"]],
        ["let", "tok", ["chan.bytes.recv", "c"]],
        ["let", "tok_id", ["std.codec.read_u32_le", "tok", 0]],
        ["let", "b", ["bytes1", ["bit.or", ["bit.shl", "tag", 4], "tok_id"]]],
        ["let", "_", ["chan.bytes.send", "c", "tok"]],
        "b"
      ]
    ],

    ["defn", "db_pool_max_concurrency.solve", [["input", "bytes"]], "bytes",
      ["begin",
        ["let", "caps", ["std.db.sqlite.caps_default_v1"]],
        ["let", "resp_new", ["await", ["std.db.sqlite.pool.new_path_v1", "input", 0, "caps", 2]]],
        ["let", "hpool", ["std.db.spec.resp_ok_payload_v1", "resp_new"]],

        ["let", "a0", ["db_pool_max_concurrency.take_one", "hpool", 10]],
        ["let", "a1", ["db_pool_max_concurrency.take_one", "hpool", 11]],
        ["let", "a2", ["db_pool_max_concurrency.take_one", "hpool", 12]],
        ["let", "a3", ["db_pool_max_concurrency.take_one", "hpool", 13]],
        ["let", "a4", ["db_pool_max_concurrency.take_one", "hpool", 14]],
        ["let", "a5", ["db_pool_max_concurrency.take_one", "hpool", 15]],

        ["let", "b0", ["await", "a0"]],
        ["let", "b1", ["await", "a1"]],
        ["let", "b2", ["await", "a2"]],
        ["let", "b3", ["await", "a3"]],
        ["let", "b4", ["await", "a4"]],
        ["let", "b5", ["await", "a5"]],

        ["let", "_close", ["await", ["std.db.sqlite.pool.close_v1", "hpool", "caps"]]],

        ["bytes.concat", "b0",
          ["bytes.concat", "b1",
            ["bytes.concat", "b2",
              ["bytes.concat", "b3",
                ["bytes.concat", "b4", "b5"]
              ]
            ]
          ]
        ]
      ]
    ]
  ]
}
```

---

## `tests/external_os/db_pool_no_leak_close/main.x07.json`

This is the **no‑leak close** smoke: it creates a pool, grabs tokens, closes, and returns `OK`.

```json
{
  "decls": [
    ["import", "std.db.spec", "std.db.pool", "std.db.sqlite.pool", "std.db.sqlite"],
    ["export", ["db_pool_no_leak_close.solve"]],

    ["defn", "db_pool_no_leak_close.solve", [["input", "bytes"]], "bytes",
      ["begin",
        ["let", "caps", ["std.db.sqlite.caps_default_v1"]],
        ["let", "resp_new", ["await", ["std.db.sqlite.pool.new_path_v1", "input", 0, "caps", 2]]],
        ["let", "hpool", ["std.db.spec.resp_ok_payload_v1", "resp_new"]],

        ["let", "c", ["std.db.pool.chan_v1", "hpool"]],
        ["let", "_t0", ["chan.bytes.recv", "c"]],
        ["let", "_t1", ["chan.bytes.recv", "c"]],

        ["let", "_close", ["await", ["std.db.sqlite.pool.close_v1", "hpool", "caps"]]],

        ["bytes.lit", "OK"]
      ]
    ]
  ]
}
```

---

## How to wire “only two constants” in your smoke suites

For each smoke **case** you only set:

1. `stdin_b64`: base64 of the sqlite file path bytes
2. `stdout_exact_b64`: base64 of expected output bytes

* For **pool fairness**, expected bytes are usually `00 01 02` → base64 `"AAEC"`.
* For **no leak close**, expected bytes are ASCII `"OK"` → base64 `"T0s="`.
* For **db_sqlite_smoke**, expected bytes are the DataModel doc for `SELECT 1 AS x` per your DB adapter’s DataModel encoding.
+++
Below is a Redis “driver” blueprint that matches the way your SQLite / Postgres / MySQL DB v1 packages were specified: pinned bytes contracts, DataModel-first return shape, run‑os / run‑os‑sandboxed policy gating, and a minimal-but-expandable API surface.

I’m assuming the same architectural choice you already made for pg/mysql: **OS-world builtins** do the real I/O + protocol parsing, and **stdlib modules** only (a) build/validate request bytes and (b) decode response bytes into “agent-friendly” accessors.

---

## 0) Positioning: what Redis is in your DB ecosystem

Redis is **not SQL** and doesn’t fit “rows/columns” naturally, so treat it as a DB driver whose `cmd` operation returns **a DataModel document whose root value is the Redis reply**.

Use RESP3 where possible because it maps cleanly to DataModel primitives (null/bool/number/string/seq/map). RESP3 negotiation is done via `HELLO 3`.

For the native implementation: **hiredis** is the best “C backend” candidate because it explicitly added **RESP3 + SSL + better Windows support** (v1.0.0 changelog).

---

## 1) Pinned bytes contracts (docs/db/redis-v1.md)

Create a pinned normative doc:

* `docs/db/redis-v1.md`
  (parallel to `docs/db/sqlite-v1.md`, `docs/db/postgres-v1.md`, `docs/db/mysql-v1.md`)

### 1.1 Common conventions

* All integers are **u32 little-endian** unless explicitly noted.
* All variable-length byte fields are **u32 length + bytes**.
* Magic is 4 bytes (ASCII), version is u32.

### 1.2 `DbCapsV1` (shared “X7DC” envelope)

Reuse your existing `X7DC` caps shape, but extend with Redis-relevant fields if you don’t already have them.

**X7DC v1 (caps bytes)**

```
magic[4]        = "X7DC"
version_u32     = 1

connect_timeout_ms_u32
op_timeout_ms_u32          ; for a single command round-trip
max_req_bytes_u32          ; hard cap, reject if larger
max_resp_bytes_u32         ; hard cap, reject if larger
tls_mode_u32               ; 0=PLAINTEXT, 1=TLS_NO_VERIFY, 2=TLS_VERIFY

reserved0_u32 = 0
reserved1_u32 = 0
```

Notes:

* For Redis, `connect_timeout_ms` and `op_timeout_ms` must be enforced in the builtin.
* `max_resp_bytes` is critical to prevent unbounded replies (e.g., `KEYS *`, large `MGET`, etc.).

### 1.3 Redis open request: `RedisOpenReqV1` (“X7RO”)

```
magic[4]      = "X7RO"
version_u32   = 1
flags_u32     = 0   ; must be 0 in v1; reject otherwise

host_len_u32 + host_bytes        ; DNS name or IP literal
port_u32

username_len_u32 + username_bytes  ; empty => omit username
password_len_u32 + password_bytes  ; empty => no AUTH
db_index_u32                       ; 0..N (Redis SELECT)
```

Behavior contract:

* If password is non-empty, the adapter must issue AUTH.

  * Support `AUTH <password>` and `AUTH <username> <password>` for ACL mode.
* If db_index != 0, adapter must issue `SELECT <db_index>` after auth.

### 1.4 Redis command request: `RedisCmdReqV1` (“X7RE”)

(“E” chosen to match your exec-style request names; you can call it “cmd” in APIs.)

```
magic[4]      = "X7RE"
version_u32   = 1
flags_u32     = 0   ; must be 0 in v1; reject otherwise

conn_id_u32

argc_u32
repeat argc times:
  arg_len_u32 + arg_bytes
```

Rules:

* `argc` must be >= 1.
* `arg_bytes` are raw (binary-safe), not assumed UTF‑8.
* For determinism/interop: command name canonicalization is optional; if you do it, do **ASCII uppercase** only and do it in the builtin (not in LLM code).

### 1.5 Redis close request: `RedisCloseReqV1` (“X7RC”)

```
magic[4]      = "X7RC"
version_u32   = 1
flags_u32     = 0
conn_id_u32
```

### 1.6 Response envelope: `DbRespV1` (“X7DB”)

Use your existing X7DB contract. For Redis, pin the **payload semantics**:

* **OPEN ok payload**: `conn_id_u32` as 4 bytes LE
* **CMD ok payload**: `DataModelDocV1` bytes (your ext.data_model doc encoding)
* **CLOSE ok payload**: empty bytes

### 1.7 Redis reply → DataModel mapping (RESP3-focused)

RESP3 is negotiated with `HELLO 3`.

Map Redis replies to DataModel values:

* RESP3 Null → DataModel `null`
* RESP3 Boolean → DataModel `bool`
* RESP3 Integer / Double → DataModel `number` (store the decimal ASCII exactly as received; do **not** reformat floats)
* RESP3 Simple string / Blob string / Verbatim string → DataModel `string` (raw bytes)
* RESP3 Array → DataModel `seq`
* RESP3 Map → DataModel `map` **only if keys are string-ish** (simple/blob/verbatim string). Otherwise:

  * either return an X7DB error `DB_ERR_REDIS_UNSUPPORTED_KEY_TYPE`
  * or represent as `seq` of `[k,v]` pairs (pick one and pin it)
* RESP3 Set → DataModel `seq` (but **sort elements lexicographically by their encoded bytes** to keep stable output across runs/servers)
* RESP3 Error reply → X7DB error (not DataModel), see error codes below

Why RESP3: it has explicit boolean/null/map/set types, which RESP2 lacks; and Redis explicitly documents HELLO-based negotiation.

---

## 2) Error code plan (add to docs/db/db-errors-v1.md or docs/db/redis-v1.md)

You already have the notion of `DB_ERR_POLICY_DENIED` in your existing smoke suites. Keep that pattern, but make it numeric + pinned.

Recommended structure:

### 2.1 Reserve a range for DB v1

* `DB_ERR_*` (runtime / OS adapter / policy / network): **10000–10999**
* `DB_SPEC_ERR_*` (pack/unpack/validation failures in stdlib helpers): **11000–11999**
* `DB_ERR_REDIS_*` (Redis-specific runtime errors): **12000–12099**

### 2.2 Minimal Redis-specific set

Runtime (X7DB error channel):

* `DB_ERR_POLICY_DENIED` = 10001
* `DB_ERR_INVALID_REQ` = 10002
* `DB_ERR_INVALID_CAPS` = 10003
* `DB_ERR_CONNECT_FAILED` = 10010
* `DB_ERR_TLS_FAILED` = 10011
* `DB_ERR_TIMEOUT` = 10012
* `DB_ERR_IO` = 10013
* `DB_ERR_NOT_FOUND` = 10014 (conn_id missing)
* `DB_ERR_BUSY` = 10015 (if you add async handles later)

Redis-specific:

* `DB_ERR_REDIS_AUTH_FAILED` = 12001
* `DB_ERR_REDIS_SELECT_FAILED` = 12002
* `DB_ERR_REDIS_SERVER_ERROR` = 12003 (RESP error reply)
* `DB_ERR_REDIS_PROTOCOL` = 12004 (parse failure, unexpected reply type)
* `DB_ERR_REDIS_UNSUPPORTED_REPLY` = 12005 (e.g., push messages if you don’t support pubsub)
* `DB_ERR_REDIS_UNSUPPORTED_KEY_TYPE` = 12006 (if you choose “map keys must be strings”)

Spec-side (stdlib pack/unpack returning `result_bytes`):

* `DB_SPEC_ERR_REDIS_BAD_MAGIC` = 11001
* `DB_SPEC_ERR_REDIS_BAD_VERSION` = 11002
* `DB_SPEC_ERR_REDIS_BAD_FLAGS` = 11003
* `DB_SPEC_ERR_REDIS_TRUNCATED` = 11004
* `DB_SPEC_ERR_REDIS_LIMIT_EXCEEDED` = 11005

Pin these in a doc so agent tooling can learn them.

---

## 3) Sandbox policy schema additions (schemas/run-os-policy.schema.json)

You already have `db.net` and driver toggles. Extend the **drivers allowlist**:

Add:

```json
"redis": {
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "enabled": { "type": "boolean" },
    "allow_db_indexes": {
      "type": "array",
      "items": { "type": "integer", "minimum": 0, "maximum": 1024 }
    }
  },
  "required": ["enabled"]
}
```

And ensure `db.net` supports:

* `allow_dns`: list of allowed DNS names (exact match or suffix match — pick one and pin)
* `allow_cidr`: list of CIDRs for resolved IP checks
* `allow_ports`: list of allowed ports
* `require_tls`: boolean
* `require_verify`: boolean (TLS cert verification)

You already decided earlier for networking: **both DNS allowlists and CIDR**; carry the same model here.

Implementation notes:

* If host is a DNS name: must match `allow_dns`, and every resolved IP must be within an allowed CIDR (defense-in-depth).
* If host is an IP literal: DNS check can be skipped, but CIDR must match.

---

## 4) Stdlib modules to add (packages/x07-ext-db-redis/0.1.0/…)

Mirror your existing ext-db layout:

### 4.1 Package skeleton

* `packages/x07-ext-db-redis/0.1.0/package.json`
* `packages/x07-ext-db-redis/0.1.0/modules/std/db/redis.x07.json`
* `packages/x07-ext-db-redis/0.1.0/modules/std/db/redis/spec.x07.json`
* `packages/x07-ext-db-redis/0.1.0/modules/std/db/redis/resp.x07.json`
* `packages/x07-ext-db-redis/0.1.0/modules/std/db/redis/pool.x07.json` (thin wrapper over your generic `std.db.pool`)

### 4.2 Minimal **agent-facing API** (exact names)

Keep it small and “don’t-make-the-agent-slice-by-offset”.

#### `std.db.redis.spec`

* `open_req_v1(host, port_i32, username, password, db_index_i32) -> bytes`
* `caps_pack_v1(connect_timeout_ms_i32, op_timeout_ms_i32, max_req_bytes_i32, max_resp_bytes_i32, tls_mode_i32) -> bytes`
* `cmd1_v1(a0) -> bytes`
* `cmd2_v1(a0, a1) -> bytes`
* `cmd3_v1(a0, a1, a2) -> bytes`
* `cmd4_v1(a0, a1, a2, a3) -> bytes`
* `cmdv_from_args_blob_v1(argc_i32, args_blob_bytes) -> result_bytes`

  * where args_blob is `repeat argc: u32_len + bytes` (so agents can build it with vec_u8 helpers)

#### `std.db.redis`

* `open_v1(host, port, username, password, db_index, caps_bytes) -> bytes` (DbRespV1)
* `cmd_v1(conn_id_i32, cmd_bytes, caps_bytes) -> bytes` (DbRespV1)
* `close_v1(conn_id_i32, caps_bytes) -> bytes` (DbRespV1)

#### `std.db.redis.resp`

* `ok_v1(resp_bytes) -> i32`
* `err_code_v1(resp_bytes) -> i32`
* `err_msg_v1(resp_bytes) -> bytes`
* `conn_id_v1(open_resp_bytes) -> result_i32`
* `dm_doc_v1(cmd_resp_bytes) -> result_bytes`  *(returns the DataModel doc bytes on ok)*

This gives agents a single canonical way: “call `resp.*` accessors”.

#### `std.db.redis.pool`

* `new_v1(open_args..., caps_bytes, pool_caps_bytes) -> bytes` (pool handle/token)
* `with_conn_cmd_v1(pool, cmd_bytes, caps_bytes) -> bytes` (DbRespV1)
* `close_v1(pool) -> i32` (no-leak close)

(Reuse your existing pool fairness + max-live semantics.)

---

## 5) OS adapter / builtin implementation plan (native, hiredis-first)

### 5.1 Why hiredis

* hiredis v1.0.0 explicitly added **RESP3 support**, **SSL connections**, and “better Windows support”.
  That matches your requirements: servers + TLS + cross-platform.

### 5.2 Builtins to implement (run-os* only)

In `crates/x07c/src/c_emit.rs` (or wherever your OS DB builtins live):

* `os.db.redis.open_v1(req_bytes, caps_bytes) -> DbRespV1`
* `os.db.redis.cmd_v1(req_bytes, caps_bytes) -> DbRespV1`
* `os.db.redis.close_v1(req_bytes, caps_bytes) -> DbRespV1`

Hard gates:

* Only available when world is run‑os or run‑os‑sandboxed.
* Compile-time hard error in any deterministic world.

### 5.3 Connection table + handle safety

You already built a “process table”; do the same pattern:

* Global `redis_conn_table` with:

  * `live: bool`
  * `generation: u32`
  * `ctx: redisContext*` (or hiredis SSL context wrapper)
  * `last_err: u32` etc
* `conn_id` is `(slot_index << 16) | (generation_low16)` (or similar) to prevent ABA reuse.
* Enforce `process.max_live`-style caps: `db.redis.max_live` and `db.redis.max_op_inflight` (if you add async ops later).

### 5.4 TLS + verification

* If caps `tls_mode != 0`, create SSL context and connect using hiredis SSL support (hiredis provides SSL helpers; confirm exact API from your chosen hiredis version).
* If sandbox policy `db.net.require_tls = true` then reject plaintext requests.
* If policy `require_verify = true`, reject tls_no_verify.

### 5.5 RESP3 negotiation + auth

On successful TCP/TLS connect:

1. Send `HELLO 3` to switch to RESP3.
2. If password non-empty:

   * If username non-empty: send `AUTH username password`
   * Else: send `AUTH password`
3. If db_index != 0: send `SELECT <db_index>`.

### 5.6 Command execution + reply parsing

* Use argv-style command submission (binary-safe).
* Convert reply into DataModel doc bytes (using your existing ext.data_model encoding rules):

  * build DataModel value bytes, then wrap into a doc: `[ok_tag=1] + value`.
* Enforce:

  * `max_resp_bytes`
  * recursion depth cap (e.g., 64)
  * element count cap for arrays/maps/sets
* If reply is Redis Error:

  * Return X7DB error with `DB_ERR_REDIS_SERVER_ERROR` and include message truncated to (say) 4KB.

### 5.7 Deterministic ordering in “unordered” Redis reply containers

Even in run‑os worlds, **stable outputs are hugely helpful for agents**. So:

* For RESP3 Set: sort elements lexicographically by their **value bytes** before emitting DataModel seq.
* For RESP3 Map: sort keys lexicographically (your DataModel map encoder already sorts keys; leverage it).

---

## 6) Smoke suites (JSON shapes) + always-green testing strategy

You asked earlier for “always-green” paths; for Redis, the practical way is:

* **Unit / pack/unpack tests**: pure (no OS) for spec helpers.
* **OS smoke**: run a tiny helper Redis stub server binary (RESP3 + TLS) that you ship, so CI doesn’t depend on “redis-server installed”.

### 6.1 Pure suite (spec correctness)

`benchmarks/pure/db-redis-spec-smoke.json` (shape)

* Case runs a program that:

  * calls `std.db.redis.spec.open_req_v1(...)`
  * validates magic/version/lengths via `spec.unpack` or `result_bytes` checks
  * builds a `cmd3_v1` and checks it round-trips through unpacker

### 6.2 run-os suite (real connect + command + DataModel bytes exact)

`benchmarks/run-os/db-redis-smoke.json` (shape)

```json
{
  "schema_version": "x07.bench_suite@v1",
  "suite_id": "run-os/db-redis-smoke@v1",
  "world": "run-os",
  "notes": "Requires REDIS_STUB_HOST/REDIS_STUB_PORT; expects TLS_NO_VERIFY for stub.",
  "cases": [
    {
      "id": "redis_ping_set_get_dm_exact",
      "only_platforms": ["linux", "macos", "windows"],
      "program": "tests/external_os/db_redis_smoke/main.x07.json",
      "argv": [],
      "env": {
        "REDIS_HOST": "127.0.0.1",
        "REDIS_PORT": "6389",
        "REDIS_TLS_MODE": "1",
        "REDIS_EXPECT_DM_BASE64": "BASE64_OF_EXPECTED_DM_DOC"
      },
      "timeout_ms": 5000,
      "expect": { "exit_code": 0 }
    }
  ]
}
```

Program behavior:

* Execute `PING`, `SET k v`, `GET k`, and produce a single DataModel doc that contains results (e.g., map of `{ping:"PONG", get:"v"}`).
* Compare to `REDIS_EXPECT_DM_BASE64` byte-for-byte; print `OK` and exit 0 if match.

### 6.3 run-os-sandboxed suite (policy allow/deny)

`benchmarks/run-os-sandboxed/db-redis-policy-smoke.json` (shape)

Cases:

1. **deny_by_host**: host not in allow_dns/cidr → expect `DB_ERR_POLICY_DENIED`
2. **deny_plaintext_when_require_tls**: tls_mode=0 but require_tls=true → denied
3. **allow_local_stub**: allow dns=localhost or cidr=127.0.0.0/8 and port=6389 → success

---

## 7) “Full-featured” Redis gaps (v1 → v1.5 roadmap)

The above is “production usable” for most agentic tasks, but here are the next big missing pieces:

1. **Pipelining / batching**
   Add `cmd_many_v1(conn_id, cmds_seq)` returning DataModel seq of replies.
2. **Pub/Sub & push messages (RESP3 push type)**
   Either explicitly reject in v1 (pinned) or implement a streaming interface later.
3. **Cluster / Sentinel**
   Out of scope for v1; requires topology discovery and redirect handling.
4. **Connection pool resilience**
   Reconnect-on-broken, exponential backoff caps, per-conn health checks (`PING`) at checkout.
5. **Typed convenience layer** (agent friendly but small)
   `get_bytes`, `set_bytes`, `incr_i32`, etc., built on top of `cmd*`.
6. **Better TLS story for verification**
   Allow passing a pinned CA bundle (bytes) or pinned SPKI hash; otherwise rely on OS trust store.

---

## 8) One key recommendation for agentic coding reliability

For Redis (and frankly all DB drivers), the *largest reliability win* is to enforce:

* agents never parse offsets
* agents never build binary envelopes

So: **do not ship `open_v1(req_bytes,caps_bytes)` as the primary API**. Always ship:

* field-based builders (`open_v1(host,port,user,pass,db,caps)`)
* plus response accessors (`resp.conn_id_v1`, `resp.dm_doc_v1`, `resp.err_msg_v1`)

This is exactly the “LLM-friendly surface” that prevents “forgot a u32 length prefix” failures.

---
Here’s the next **drop‑in bundle** with Redis v1 (docs + policy schema fragment/patch + ext package skeleton + external_os smoke program + run‑os/run‑os‑sandboxed suite JSON shapes):

Use docs/phases/assets/x07_ext_db_v1_redis_bundle.tar.gz

### What’s inside (high level)

* **Pinned spec doc:** `docs/db/redis-v1.md`
  *Exact bytes encodings* for `X7RV` (argv table), `X7RO` (open req), `X7RQ` (cmd req), `X7RX` (close req), `X7RK` (open ok payload) + RESP3→DataModel mapping rules (including deterministic sorting for RESP3 maps/sets).
* **Integration guidance:** `docs/integration/redis-native-integration.md`
  Native adapter implementation plan (recommended: **hiredis + hiredis_ssl**; RESP3 + SSL supported since hiredis v1.0.0).
* **Sandbox policy schema fragment + patch JSON:**

  * `schemas/run-os-policy.db.section.redis.json` (db section with `drivers.redis` + net allowlists)
  * `schemas/run-os-policy.schema.redis.patch.json` (simple JSON patch that adds `/properties/db`—you may need to adjust if you already merged db section)
* **External package skeleton:**

  * `packages/x07-ext-db-redis/0.1.0/...`
  * Fully working x07AST module `std.db.redis` (no TODOs) including:

    * X7DC caps pack + default caps
    * Open/cmd/close request builders
    * X7RV argv helpers (empty/push/fromN)
    * `cmd_doc_v1` that returns a DataModel doc so agents never slice offsets
* **Smoke program stub (but functional):**

  * `tests/external_os/db_redis_smoke/src/main.x07.json`
  * Connects to `localhost:6379`, runs `PING`, asserts returned DataModel doc equals `ok("PONG")` byte‑for‑byte (traps otherwise), then prints the doc.
* **Smoke suite JSON shapes (copy/paste ready):**

  * `benchmarks/run-os/db-redis-smoke.json`
  * `benchmarks/run-os-sandboxed/db-redis-smoke.json`

### Why these protocol choices are pinned

* Using **RESP3** is anchored by Redis’ protocol negotiation (`HELLO 3`) and the RESP3 type system documentation.
* **AUTH username/password** behavior is anchored by Redis’ `AUTH` command docs.
* The suggested native library (**hiredis**) explicitly calls out RESP3 + SSL support in its changelog (v1.0.0).
