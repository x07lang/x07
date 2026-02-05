# Prompt 6 — Filesystem + glob/walk + multi-DB ingestion + query engine

```text
You are an X07 coding agent building `x07-db-fs-indexer`.

GOAL
Build a realistic “indexer” application that:
- discovers files via OS filesystem APIs + glob/walk helpers
- extracts structured metadata + content stats
- stores results in a database using std.db interfaces and drivers
- exposes a query CLI (and optionally a local HTTP query endpoint)

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir myapp
cd myapp
x07 init

DEPENDENCIES (lock them)
Filesystem + traversal (run-os only):
- ext-fs            (std.os.fs, std.os.fs.spec)
- ext-path-glob-rs  (std.os.fs.glob / walk / ignore / globwalk.tests)
- ext-walkdir-rs    (ext.walkdir)
- ext-glob-rs       (ext.glob wildcard matching)
Database core + drivers (run-os only):
- ext-db-core       (std.db.*, pool/spec/params/dm)
- ext-db-sqlite     (required for CI tests)
- ext-db-migrate    (REQUIRED: std.db.migrate)
- ext-db-postgres   (optional, feature-gated)
- ext-db-mysql      (optional, feature-gated)
- ext-db-redis      (optional, feature-gated)
Math + numeric edge cases:
- ext-math
- ext-bigint-rs
- ext-decimal-rs
Errors + CLI:
- ext-error
- ext-cli
- ext-cli-ux (deterministic table/jsonl output helpers)

STDLIB COVERAGE REQUIREMENTS (must explicitly exercise these “OS-edge” stdlib modules here)
- std.fs (list_dir/read/read_async/read_task)
- std.world.fs (read_file/read_file_async)
- std.kv (set/get and at least one async/task/stream variant)
- std.path (join/basename/extname)

SANDBOX POLICY REQUIREMENTS
- run-os-sandboxed
- allow-read-root: a fixtures/ tree and/or user-specified index root
- allow-write-root: out/ and a sqlite db file location
  - deny network by default; if postgres/mysql/redis are enabled, require explicit allow-host entries and document them.

FUNCTIONAL REQUIREMENTS
A) Indexing pipeline:
- walk file tree deterministically (sorted output)
- for each file: compute size, line count, and one stable hash/checksum (you may reuse std.hash or ext.checksum if you also include it)
- parse a couple numeric fields from text and store them as decimal and bigint to exercise those packages
- store in sqlite using std.db + sqlite pool; schema migrations are part of the program (use `std.db.migrate`).
B) Query CLI:
- queries by glob, extension, size ranges, numeric fields, and full-text substring matches (can reuse std.regex-lite or ext.regex if desired)
C) Error handling:
- all user-facing errors must include context chains (ext.error.*)
- stable exit codes

TESTS
- tests/tests.json with:
  - unit tests that index a small fixture tree
  - integration test that builds a sqlite db under out/ and runs queries
  - produce a canonical JSON report for each run that CI can diff.

DELIVERABLES
- `x07 bundle --profile sandbox` outputs a normal CLI binary `dist/indexer`.
- README with minimal commands to reproduce in CI.
```

