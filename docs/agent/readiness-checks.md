# Agent readiness checks

This page contains the canonical “readiness prompts” used to validate that an agent can go from `x07 init` → deps → lockfile → run → test across the core toolchain, stdlib, and the external package ecosystem.

## Dependency conventions

- Prefer adding dependencies without pinning a version:
  - `x07 pkg add NAME --sync` (selects the latest non-yanked semver version from the index)
  - commit `x07.lock.json`
- If you need to pick a specific version:
  - `x07 pkg versions NAME`
  - `x07 pkg add NAME@VERSION --sync`
- If you need to undo a dependency:
  - `x07 pkg remove NAME --sync`

## Guardrails (brands + pipes)

- For structured bytes encodings, prefer branded bytes + validators over ad-hoc parsing. Schema-derived modules expose `meta.brands_v1`; use `std.brand.cast_view_v1` / `std.brand.cast_view_copy_v1` (see `x07 guide`).
- For streaming transforms, prefer `std.stream.pipe_v1` and `std.io.bufread` over manual loops (more predictable allocations; fewer borrow/ownership hazards).

If you hit:

```
unsupported schema_version: expected x07.x07ast@0.3.0 got "x07.x07ast@0.2.0"
```

it usually means you pinned an older package version. Upgrade to a newer package version.

---

### Prompt 1 — Core stdlib conformance and determinism harness

```text
You are a senior X07 coding agent. Create a repo/workspace named `x07-core-conformance` that serves as the “pure stdlib + core language” conformance harness.

GOAL
Build a single executable program (entrypoint `app.main`) that runs an internal battery of checks and prints ONE canonical JSON report summarizing pass/fail per stdlib module and per capability. The program must be deterministic: given the same input bytes, it produces identical output bytes.

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir myapp
cd myapp
x07 init

TOOLCHAIN + PROJECT SETUP
1) Pin toolchain to the latest.
2) Create `x07.json` with:
   - module_roots = ["src"]
   - default_profile = "sandbox"
   - profiles.sandbox.world = "run-os-sandboxed"
   - profiles.sandbox.policy = "policy/sandbox.base.json" (deny all net, deny all filesystem by default).
3) Generate a schema-valid base sandbox policy:
   - `x07 policy init --template worker --out policy/sandbox.base.json --mkdir-out`
4) Add dependencies:
   - ext-cli (for argv_v1 parsing)
5) Run `x07 pkg lock` and commit `x07.lock.json`.

FUNCTIONAL REQUIREMENTS
A) Exercise X07 core memory model patterns explicitly:
   - bytes vs bytes_view for scanning/splitting
   - vec_u8 builder patterns (with_capacity / extend_bytes / as_bytes)
   - option_* and result_* idioms (no sentinel “ERR strings”)
B) Exercise deterministic async concurrency (cooperative tasks) using `task.scope_v1` (no orphan tasks), e.g. a small async pipeline (map-reduce over byte chunks), and prove identical output across runs.
C) Exercise high-level primitives in a pure context:
   - `budget.scope_v1` around a hot loop (use a small nonzero cap and assert it does not trip)
   - `std.stream.pipe_v1` with a `std.stream.src.bytes_v1` source, a `std.stream.xf.split_lines_v1` stage, and a `std.stream.sink.collect_bytes_v1` sink

STDLIB COVERAGE (must call AT LEAST ONE export in each module listed below)
Implement one `check_<module>() -> result_i32` per module and call them from `app.main`, collecting results:
NOTE: `std.csv` is removed from the prod surface; for CSV parsing coverage, use `std.result.chain_sum_csv_i32`.
- std.bit (popcount_u32)
- std.bitset (new/set/test/intersection_count)
- std.btree_map (empty_u32_u32/put_u32_u32/len/get)
- std.btree_set (empty_u32/insert/contains/len)
- std.bytes (alloc/len/slice/concat/reverse/find_u8/count_u8/etc.)
- std.codec (read_u32_le/write_u32_le)
- std.deque_u32 (with_capacity/push_back/pop_front/front_or/len)
- std.fmt (u32_to_dec, s32_to_dec)
- std.hash (fnv1a32_bytes/mix32)
- std.hash_map (new/set/get/contains/len)
- std.hash_set (new/add/contains/len/emit or view_* APIs)
- std.heap_u32 (with_capacity/push/pop_min/min_or/len)
- std.io + std.io.bufread (read_all; bufread.new/fill/consume; read from ext.streams is NOT allowed here)
- std.json (canonicalize_small and extract_path_canon_or_err)
- std.lru_cache (new/put/peek/touch/len)
- std.map (word_freq_sorted_ascii)
- std.option (none/some/is_some/payload/unwrap_i32_le)
- std.parse (u32_dec, i32_status_le etc.)
- std.path (join/basename/extname)
- std.prng (lcg_next_u32 and x07rand32_v1_stream)
- std.regex-lite (find_literal/is_match_literal/count_matches_u32le)
- std.result (ok/err/is_ok/payload/unwrap_i32_le/chain_sum_csv_i32)
- std.set (unique_lines_sorted)
- std.slab (new/alloc/set/get/free/len/cap)
- std.slice (clamp/cmp_bytes)
- std.small_map (inc1_bytes_u32/get/put/remove/len)
- std.small_set (empty/insert/contains/len)
- std.test (assert_* and status_v1 helpers)
- std.text.ascii (tokenize_words_lower/split_lines_view/normalize_lines/etc.)
- std.text.slices (builder_* and view_at/validate)
- std.text.utf8 (is_valid/count_codepoints_or_neg1/validate_or_empty)
- std.u32 (read_le_at/write_le_at/pow2_ceil/push_le)
- std.vec (with_capacity/push/get/len/extend_bytes/as_bytes)

OUTPUT
- Output must be canonical JSON bytes (use std.json canonicalization where appropriate).
- Exit with nonzero if any check fails, and include a stable integer error code per failing module.

DELIVERABLES
- Repo with `src/` modules, `x07.json`, `x07.lock.json`, `policy/`, and a short README explaining how to run:
  - x07 run --profile sandbox -- <args>
  - x07 bundle --profile sandbox --out dist/core-conformance
```

---

### Prompt 2 — Data model + full codec roundtrip laboratory

```text
You are an X07 coding agent building a “data interchange torture test” project named `x07-data-interop-lab`.

GOAL
Build an executable `interop.main` that:
1) accepts input bytes + argv_v1 args: --in-format and --out-dir
2) parses the input into ext.data_model
3) roundtrips the same logical data through ALL supported codecs, producing artifacts in out-dir
4) verifies semantic equivalence (or documented lossy behavior) and prints a canonical JSON report.

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir myapp
cd myapp
x07 init

DEPENDENCIES (add via x07 pkg add … --sync and lock)
- ext-data-model
- ext-json-rs
- ext-toml-rs
- ext-yaml-rs
- ext-xml-rs
- ext-ini-rs
- ext-csv-rs
- ext-cbor-rs
- ext-msgpack-rs
- ext-pb-rs
- ext-streams (stream readers)
- ext-cli (argv parsing)

REQUIREMENTS
A) Use ext.data_model as the canonical internal representation.
B) For each format, implement:
   - decode(bytes) -> data_model
   - encode(data_model) -> bytes
   - verify roundtrip invariants and emit per-format diagnostics
C) Enforce canonical encodings where the ecosystem provides them (use _v1 / canon modules where available).
D) Include at least one large/streaming test: read from a stream reader (ext.streams) and use std.io.bufread.

TESTS
- Provide a corpus folder `fixtures/` containing representative documents:
  - nested maps/arrays, unicode text, numbers, edge cases
  - include at least one case that triggers known differences (e.g., CSV typing ambiguity) and explicitly mark it “expected lossy”.
- Provide `tests/tests.json` with multiple entries that call `interop.tests.*` entrypoints under run-os-sandboxed with a policy that allows:
  - read-only access to fixtures/
  - write access only to a temp out-dir under repo (e.g., out/)
  - no network.

OUTPUT
- Write artifacts: out-dir/{json,toml,yaml,xml,ini,csv,cbor,msgpack,pb}/…
- Print one canonical JSON report summarizing:
  - per-format parse/encode success
  - equivalence checks
  - byte sizes and timing counters
```

---

### Prompt 3 — Crypto + compression + archive + diff integrity pipeline

```text
You are an X07 coding agent building `x07-artifact-integrity-pipeline`.

GOAL
Build a production-style “artifact packer” tool with subcommands:
- pack: directory -> tar -> compress -> manifest
- verify: manifest + blob -> verify checksums/hashes/signatures -> unpack (optional)
- diff: two manifests -> compute patch_v1 and apply it

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir myapp
cd myapp
x07 init

DEPENDENCIES (lock them)
Pure-ish integrity + encoding:
- ext-crypto-rs
- ext-checksum-rs
- ext-base64-rs
- ext-hex-rs
- ext-byteorder-rs
- ext-u64-rs
- ext-uuid-rs
- ext-semver-rs
- ext-diff-rs (must use patch_v1 format)

Archiving/compression:
- ext-tar-rs
- ext-compress-rs (ext.compress/ext.zip/ext.zstd)

OS/FFI coverage (run-os only):
- ext-openssl-c (ed25519/rand/hash)
- ext-zlib-c (zlib bindings)

CLI:
- ext-cli

REQUIREMENTS
A) Produce a manifest JSON that includes:
   - semver for artifact schema versioning
   - uuid for artifact id
   - per-file metadata (path, size, sha256, crc32c, optional xxhash64)
   - archive format + compression parameters
   - base64 + hex representations for selected digests
B) Determinism: with fixed input files and fixed compression settings, output must be stable.
C) Implement “patching”:
   - given two manifests+blobs, compute a patch_v1 that transforms old->new (bytes and/or lines)
   - apply the patch_v1 and verify the resulting blob hash matches the target.
D) Demonstrate correct endian-safe encoding for binary headers using ext.byteorder and/or std.codec.

TESTS
- fixtures/ with a small directory tree (text + binary)
- tests that run in run-os-sandboxed with:
  - allow-read-root fixtures/
  - allow-write-root out/
  - deny all network
- verify includes both ext.crypto and ext.openssl hash paths, and includes zlib usage path.

DELIVERABLES
- `x07 bundle --profile sandbox` produces a normal CLI binary that can be run without framed I/O.
- Provide README commands to run pack/verify/diff.
```

---

### Prompt 4 — Sandboxed networking + HTTP + crawler/search + observability

```text
You are an X07 coding agent building `x07-sandbox-net-stack`.

GOAL
Build a single project that contains BOTH:
1) a local HTTP server (for deterministic integration testing)
2) a sandboxed crawler/client that fetches pages, respects robots.txt, parses sitemap, extracts links + visible text, and performs high-performance searches.

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir myapp
cd myapp
x07 init

DEPENDENCIES (lock them)
Networking stack (run-os only):
- ext-net (std.net.* modules including http client/server)
- ext-sockets-c (FFI backend)
- ext-curl-c (additional HTTP client via libcurl bindings)
URL + HTTP parsing:
- ext-url-rs (ext.url.* + http_types + httparse)
Content parsing/search:
- ext-html-lite-rs
- ext-robots-txt-rs
- ext-sitemap-rs
- ext-regex
- ext-aho-corasick-rs
- ext-memchr-rs
Text handling:
- ext-text
- ext-unicode-rs
Time + observability:
- ext-time-rs
- ext-log
- ext-tracing
CLI:
- ext-cli
Stdlib cross-coverage:
- must also use std.lru_cache, std.hash_map, std.rr (fetch/send) where appropriate

SANDBOX POLICY REQUIREMENTS
- Run everything under run-os-sandboxed.
- Allow network ONLY to 127.0.0.1:<port> (the local server you spin up).
- Deny all external hosts.
- Allow write only to out/ for reports.

FUNCTIONAL REQUIREMENTS
A) Server:
   - serve deterministic HTML pages + robots.txt + sitemap.xml
B) Crawler:
   - parse and enforce robots rules
   - parse sitemap and crawl discovered URLs
   - extract links + visible text from HTML
   - tokenize and normalize text, including unicode normalization/casefold tests
   - support pattern searches:
     - regex patterns
     - Aho–Corasick multi-pattern search
     - raw byte scanning via memchr
C) Observability:
   - emit structured logs and trace spans for each fetch/parse/search stage
   - include RFC3339 timestamps
D) Output:
   - canonical JSON report with:
     - fetched URLs
     - extracted links
     - search hits
     - timing + counters
     - policy info (what was allowed)

TESTS
- A test entrypoint that starts the server on an ephemeral port, runs the crawler against it, and asserts output stability.
```

---

### Prompt 5 — Filesystem + glob/walk + multi-DB ingestion + query engine

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
- store in sqlite using std.db + sqlite pool; schema migrations are part of the program.
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

---

### Prompt 6 — Record/replay + stream pipes + budgets (deterministic ingest)

```text
You are an X07 coding agent building `x07-rr-pipes-smoke`.

GOAL
Build a single project that demonstrates an end-to-end deterministic ingest flow:
1) an OS-world run that records real interactions into an rr cassette
2) a fixture-world run that replays the same cassette and produces identical output bytes

The project must use:
- `std.rr.with_policy_v1` (structured rr scope)
- `std.stream.pipe_v1` (streaming composition)
- `budget.scope_from_arch_v1` (pinned local cost contracts)

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir x07-rr-pipes-smoke
cd x07-rr-pipes-smoke
x07 init

PROJECT SHAPE
1) Provide two run profiles in `x07.json`:
   - `os_record`: `run-os-sandboxed` with policy allowing loopback HTTP to a local server and write access to `.x07_rr/`.
   - `rr_replay`: `solve-rr` with rr cassette access and no network.
2) Provide a tiny local HTTP server entrypoint that serves a deterministic payload (for example: newline-delimited JSON or newline-delimited text).
3) Provide an ingest entrypoint that fetches from that local server and processes the response via `std.stream.pipe_v1`.

RR REQUIREMENTS
1) Put rr contracts under `arch/rr/`:
   - `arch/rr/index.x07rr.json` (`schema_version: "x07.arch.rr.index@0.1.0"`)
   - one policy under `arch/rr/policies/`
2) Use `std.rr.with_policy_v1` with:
   - `policy_id`: the policy you added to `arch/rr/index.x07rr.json`
   - `cassette_path`: under `.x07_rr/` (repo-local)
   - mode:
     - record in `os_record`
     - replay in `rr_replay`

PIPES REQUIREMENTS
1) Stream the response through a pipe:
   - source: either the rr streaming source (`std.stream.src.rr_send_v1`) or an OS-world reader source
   - chain: include at least `std.stream.xf.split_lines_v1`
   - sink: `std.stream.sink.collect_bytes_v1`
2) Do not read the entire response into memory as one bytes value before processing (the point is streaming).

BUDGET REQUIREMENTS
1) Put a budget profile under `arch/budgets/` and reference it by id:
   - `arch/budgets/index.x07budgets.json` (`schema_version: "x07.arch.budgets.index@0.1.0"`)
   - `arch/budgets/profiles/<id>.budget.json` (`schema_version: "x07.budget.profile@0.1.0"`)
2) Wrap the parsing/processing region in `budget.scope_from_arch_v1`.
3) The replay run must succeed under the pinned budget caps.

TESTS
- `x07 test` must run deterministically.
- Include at least one test that runs the replay path in `solve-rr` and compares output bytes exactly.

DELIVERABLES
- `x07 bundle --profile rr_replay` should produce a standalone binary that can replay from `.x07_rr/` and print the same report bytes.
```

---

### Prompt 7 — State machines as data + arch contracts (gen + drift + enforcement)

```text
You are an X07 coding agent building `x07-sm-arch-contracts-smoke`.

GOAL
Build a small “workflow engine” project where the core control-flow is defined as a state machine spec.
The toolchain must generate the implementation, and CI must detect drift.

The project must use:
- `x07 sm gen` (generate a deterministic step function + tests)
- `x07 arch check` with `contracts_v1.sm` enabled (validate the SM index + require generated outputs up to date)
- `budget.scope_from_arch_v1` for step-level caps (pinned under `arch/budgets/`)

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir x07-sm-arch-contracts-smoke
cd x07-sm-arch-contracts-smoke
x07 init

STATE MACHINE SPEC
1) Write an SM spec file (JSON) under `arch/sm/` that models a simple lifecycle:
   - states: `init`, `ready`, `running`, `done`, `error`
   - events: `start`, `tick`, `finish`, `fail`
   - transitions with stable error codes for invalid transitions
2) Run `x07 sm gen --input <spec> --out gen/sm --write`.
3) Run the generated tests via `x07 test --manifest gen/sm/tests.manifest.json`.

ARCH CONTRACTS
1) Create `arch/sm/index.x07sm.json` (`schema_version: "x07.arch.sm.index@0.1.0"`) referencing your spec file(s).
2) Create an `arch/manifest.x07arch.json` that enables `contracts_v1.sm` with:
   - `index_path` pointing at `arch/sm/index.x07sm.json`
   - `gen_dir` pointing at `gen/sm`
   - `require_gen_uptodate=true`
3) Run `x07 arch check --write-lock` and commit the lock.

BUDGETS
1) Define a budget profile under `arch/budgets/profiles/` for the generated `step_v1` hot path.
2) In your application entrypoint, wrap calls into the generated `step_v1` with `budget.scope_from_arch_v1`.

APPLICATION
1) Implement an entrypoint that:
   - accepts an event stream from input bytes (newline-delimited event names is fine)
   - feeds events into the generated machine
   - outputs a canonical report bytes value (including final state and counts)
2) The program must be deterministic and must run under `solve-pure` (no OS calls).

TESTS
- Include a deterministic test suite with multiple event streams (success path, invalid transition path, error path).
- CI must fail if:
  - generated outputs drift (`x07 sm gen --check`)
  - arch contracts drift (`x07 arch check` errors)
```
