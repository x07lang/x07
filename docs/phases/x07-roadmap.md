# X07 Roadmap (Native C backend) — Post‑G1/G2 + stdlib import pipeline

X07 targets an LLM-first language whose semantics are defined by a small, versioned set of core builtins plus a versioned stdlib, while the execution substrate stays fixed and deterministic.

## Pillars (don’t change across phases)

- **One backend**: programs compile to C and run as native executables (no alternate backends).
- **Deterministic evaluation**: fuel-bounded execution, fixed environment, and world-scoped capabilities.
- **x07lve the language, minimize trusted changes**:
  - Core builtin changes are explicit and versioned (language id + compiler/stdlib versions).
  - Trusted code (compiler/runner) only changes to: (a) add new capability worlds, (b) enforce safety/determinism gates,
    (c) expose carefully-versioned intrinsics/instrumentation.

## Worlds (capability profiles)

- `solve-pure`: pure bytes → bytes.
- `solve-fs`: deterministic read-only fixture filesystem (provided as `.`), with file reads via `["fs.read", ...]`.
- `solve-rr`: deterministic request/response fixtures (no real network).
- `solve-kv`: deterministic key/value store with seeded datasets (reset per case).
- `solve-full`: includes fs + rr + kv.

## Stdlib design principle (starts in Phase E)

- **Core intrinsics (trusted, tiny, fixed)**: only what must be implemented in the runtime/host for safety/perf/determinism.
- **Stdlib packages (X07 code, versioned)**: text parsing, file formats, request/response helpers, etc.
- **World adapters**:
  - In deterministic suites: stdlib I/O modules bind to deterministic fixtures.
  - In standalone: stdlib I/O modules may bind to real OS (explicit opt-in; not used by deterministic suites).

---

## Phase A — Stable deterministic substrate (foundation)

Goal: make evaluation stable enough that deltas are trustworthy.

Deliverables:
- Artifact gates for untrusted native executables.
- Fuel enforcement + hard limits + rlimits kill-switches.
- Fixture reproducibility for `solve-fs`.
- Regression smoke: phases A–H1 suites execute end-to-end on the C backend.

Exit criteria:
- Same candidate + same inputs run 10× ⇒ identical outputs and identical `fuel_used`.
- Suites run without infra errors.

---

## Phase B — Track B bootstrap (deterministic compiler + core builtins)

Goal: iterate on the language definition, not a VM kernel.

Deliverables:
- Fixed canonical source format: x07AST JSON (json-sexpr expressions).
- Stable core builtins surface with hard limits, versioned by a language id.
- Deterministic compiler: `crates/x07c` (x07AST JSON → self-contained C).
- Deterministic runner: `crates/x07-host-runner`.
- Cascade suite stages (parse/validate → compile sanity → benchmark suite).

Exit criteria:
- Phase B suite runs without infra failures.
- At least one stdlib/core-builtin iteration improves solve rate or fuel on Phase B.

---

## Phase C — Language core v1 (learnable, stable surface)

Goal: stabilize the surface so inner program synthesis is reliable.

Deliverables:
- x07AST JSON surface (json-sexpr expressions) + stable core builtins set.
- Compiler-generated guide (`guide_md`) documenting stable surface.
- Phase C suite.

Additions (essential pure stdlib surface v0):
- **`std.bytes`**: safe byte access, copy/concat, find, compare.
- **`std.codec`**: endian reads/writes, varints, hex/base64 helpers (pure).
- **`std.bit`**: bit ops, shifts, rotate/popcount helpers (pure).
- **`std.hash`**: deterministic non-crypto hashes for maps/sets and caching (pure).

Exit criteria:
- High first-try success on Phase C.
- Language/stdlib updates improve runtime/fuel without rename-only churn.

---

## Phase D — Algorithmic maturity (solve-pure ladder)

Goal: solve a wide range of algorithmic tasks with pure compute only.

Deliverables:
- Expand/maintain Phase D ladder.
- Grow pure builtins + stdlib for algorithmic breadth while keeping determinism.

Additions (pure text/formatting helpers that unblock reasoning):
- **`std.text.ascii`**: isdigit/isalpha, tolower/toupper, trim, `split_u8` / `split_lines_view` X7SL v1 slice lists (pure).
- **`std.text.utf8` (v0)**: validate UTF-8 and provide a minimal “decode next codepoint” primitive (pure).
- **`std.fmt` / `std.parse`**: parse integers, format integers, simple join (pure).

Exit criteria:
- Stable solve rate on Phase D and measurable Pareto improvements.

---

## Phase E — Modules, packages, and composable stdlib

Goal: move from one-off programs to reusable modules while keeping compilation deterministic and cheap.

Deliverables:
- Modules (imports/exports, namespaces, deterministic search paths).
  - No ambient filesystem scanning; no dependence on directory enumeration order.
- Packaging:
  - versioned package metadata + lockfile (reproducible builds)
- Composable stdlib:
  - prefer shipping stdlib as separately versioned packages over growing the runtime
  - explicit capability boundaries: pure stdlib is world-agnostic; I/O stdlib is world-scoped

Additions (stdlib packages that become “baseline” for later worlds):
- **Text**
  - `std.text` (ascii + utf8 v1), `std.regex-lite` (optional, deterministic subset)
  - `std.json` (deterministic parsing/printing), CSV helpers (optional)
- **Data structures**
  - `std.vec`/`std.slice` helpers, `std.small_map`/`std.small_set` (sorted packed), `std.hash_map`/`std.hash_set` (deterministic), `std.btree_map`/`std.btree_set` (ordered)
- **Error model**
  - `std.result`, `std.option`, deterministic error codes + formatting

Exit criteria:
- A non-trivial multi-module program compiles + runs deterministically with pinned inputs.

---

## Phase F — Capability worlds v1 + memory-aware evaluation (deterministic fixtures)

Goal: enable “real programs” (filesystem, request/response, key/value) while keeping runs comparable, safe, deterministic, and memory-selectable.

Deliverables:
- World expansion (end-to-end):
  - `solve-fs` + its suites
  - `solve-rr` (runner + fixtures)
  - `solve-kv` (runner + seeded store reset-per-case)
  - `solve-full` (fs + rr + kv)
- Deterministic I/O policies:
  - no ambient network/time
  - all I/O through explicit builtins, fixture-backed

Stdlib I/O modules (world-scoped packages):
- **Filesystem (`std.fs`, `std.path`)**
  - `fs.read_file(path)->bytes`
  - `fs.read_dir_sorted(path)->list<path>` (host must sort; never expose raw `readdir` order)
  - `path.join`, `path.normalize`, `path.basename`, `path.extname`
- **Request/Response (`std.rr`, `std.http-lite`)**
  - deterministic `rr.send_request(req_bytes)->resp_bytes` (fixture-backed)
  - helpers for encoding/decoding request/response formats and headers
- **Key/Value (`std.kv`)**
  - `kv.get(key)->bytes`, `kv.set(key,val)` (deterministic store)

Memory-aware evaluation & ergonomics:
- deterministic mem_stats emission + leak gates + mem_cost scoring
- vec_u8 primitives + mem benchmarks (rung1 + rung2)
- `bytes_view` + `view.*` are the zero-copy primitives in the C backend (Phase F-Mem2 is enabled)

Safety gates for the native C backend:
- sanitizer / hardened builds in CI/nightly and/or as a top-K candidate gate.

Exit criteria:
- `solve-fs`, `solve-rr`, `solve-kv` suites run end-to-end without infra errors.
- Fixture-backed behavior stable across repeated runs.
- Memory-aware scoring selects for fewer reallocs/copies without reducing solve_rate.

---

## Phase G — Memory model v2 + diagnostics, then concurrency/async (deterministic scheduling)

Goal: unlock zero-copy data flow safely, improve memory diagnostics, then add concurrency/async without nondeterminism.

Status:
- ✅ Phase G1 implemented
- ✅ Phase G2 implemented

### Phase G1 — Views + debug safety instrumentation (DONE)

Deliverables:
- **Slice/view type distinction** (zero-copy “views” as a distinct type from owning bytes)
  - views are “fat pointers” (ptr + length/metadata)
  - stdlib surface prefers views when possible (reduce memcpy overhead)
- Runtime borrow checking (debug only)
- Per-allocation tracking table (debug only)
- Upgrade `std.text` to use views heavily (split/trim/scan without copying)

Exit criteria:
- mem rung 2 shows consistent improvements:
  - memcpy overhead near output size
  - realloc_calls near zero for canonical builders
  - peak_live_bytes close to output + small slack

### Phase G2 — Concurrency and async (deterministic scheduling) (DONE)

Deliverables:
- async/await equivalents (LLM-friendly)
- deterministic scheduler, fuel attribution across tasks
- **I/O integration (still deterministic in eval)**:
  - `std.io` streaming traits (Read/Write style) for fs/rr/kv adapters
  - buffered readers/writers for text parsing
- Benchmarks: concurrent pipelines with deterministic replay
  - Covered by the H2 suite family (`benchmarks/solve-*/phaseH2-suite.json`).

Exit criteria:
- Concurrency benchmarks stable across repeated runs with identical outputs/metrics.

---

## Phase H — Full-feature language ramp (C/Rust-class capability)

Goal: reach a small-but-real systems language level while staying LLM-optimized, **without** breaking the deterministic evaluation substrate.

This phase is intentionally split to avoid mixing:
- (a) stdlib *production* (import/porting + tests),
- (b) language semantics (types/ABI),
- (c) OS integration (standalone worlds),
- (d) tooling/ecosystem.

### Phase H0 — Stdlib import pipeline (Rust/C → X07) (NEW)

Goal: speed up adding and maintaining X07 stdlib modules by translating from a restricted, deterministic subset of Rust and C, with differential tests.

Deliverables:
- `x07import` toolchain (repo-local, deterministic output; see `crates/x07import-cli/`, `crates/x07import-core/`):
  - Rust frontend: `syn::parse_file` → validate subset → lower to x07IR → emit `.x07.json` (x07AST JSON)
  - C frontend: parse via clang AST → validate subset → lower to x07IR → emit `.x07.json` (x07AST JSON)
  - single-sourced diagnostics catalog shared by validator + lowerer
- Deterministic codegen:
  - stable symbol naming, stable ordering, stable formatting (byte-identical output on regen)
- Differential tests:
  - each imported module ships a reference implementation (Rust/C) + corpus
  - CI runs reference vs generated X07 module and fails on mismatch
- Lockfile integration:
  - `stdlib.lock` pins stdlib packages + hashes (source hash + generated module hash)
  - canary gate requires lockfile validity before any scoring
- Import policy:
  - reject nondeterminism and unsupported constructs (time, randomness, unordered iteration, unsafe pointer tricks, etc.)
  - require deterministic maps/sets (stable iteration order) and deterministic hashing

Initial import targets (must succeed end-to-end):
- `std.text.ascii`
- `std.text.utf8`
- `std.parse` / `std.fmt` helpers
- `std.json` (parse + print subset)
- `std.net.url` (parse + canonicalize subset)
- `std.regex-lite` and `std.csv` (optional; deterministic subset)

Exit criteria:
- ≥5 modules imported end-to-end with zero manual edits post-import.
- Golden regen: running `x07import` twice produces byte-identical `.x07.json` output.
- Differential tests run in CI and are stable.

### Phase H1 — Type system v1 + stable ABI (struct/enum + interface records)

Goal: unlock C/Rust-class libraries while keeping evaluation deterministic and solver ergonomics high.

Deliverables:
- Core types & layouts:
  - `bool`, `u8`, `u32`, `u64` (or i32/i64 equivalents), `ptr` (opaque), `bytes`, `view`
  - `struct` + `enum` with explicit ABI-stable layouts
  - `Option<T>` (with null-pointer optimization where applicable)
  - `Result<T,E>` with deterministic error codes + propagation sugar
- Ownership/borrowing:
  - move semantics for owning values (Box/Vec/bytes)
  - borrowed views with compile-time rules (release) + runtime checks (debug)
  - make “view-first” the canonical stdlib style for parsing and scanning
- Polymorphism:
  - “interface records” / vtable-like ABI (no full trait solver initially)
  - monomorphization as the default for stdlib and user code (simple + fast)
- Compiler pipeline updates:
  - typed IR (and/or typed AST) with deterministic layout calculation
  - deterministic type checker with stable diagnostics

Benchmarks:
- A new `solve-pure` type/ABI suite:
  - struct packing + field access
  - enum tagging + match-like branching
  - Option/Result propagation patterns
  - view-heavy parsing with memcpy/realloc/peak-live assertions
- A debug-only suite that intentionally triggers borrow/move violations (must fail deterministically).

Exit criteria:
- The core Phase E baseline stdlib can be expressed with types (not “bytes-only hacks”).
- Release builds have near-zero overhead for borrowing compared to unsafe equivalents.
- Debug builds reliably catch the top “LLM mistake” patterns (use-after-move, stale view, double free).

### Phase H2 — Stdlib parity v1 (pure + deterministic fixture worlds)

Goal: become productive for “real programs” inside deterministic worlds while preserving comparability and safety.

Deliverables:
- Pure stdlib coverage:
  - bytes/view utilities, text (ascii/utf8), parse/fmt, json, regex-lite, csv
  - deterministic map/set, deterministic PRNG (seeded)
- World-scoped stdlib coverage:
  - fs/path, rr/http-lite, kv/cache, io streams + buffering
- Benchmark ladder → “mini-app” tasks:
  - parse config → read fixture tree → fetch rr fixtures → cache in kv → emit report
  - streaming JSON parser with views (memcpy gating)
  - concurrent pipeline reading multiple fixture files and aggregating deterministically

Language maintenance strategy:
- x07lve core builtins intentionally and version them via the compiler language id (SemVer).
- x07lve stdlib via versioned packages, pinned by `stdlib.lock`.
- Gate candidates on a **bundle of canaries** across phases (A…G2 + H2 smoke) before any scoring.

Exit criteria:
- A non-trivial multi-module “mini app” passes in `solve-full` using only stdlib + world adapters.
- Adding a new stdlib module does not force per-phase retuning (canaries prevent regressions).

### Phase H3 — Standalone OS host adapters (opt-in, never used in eval)

Goal: run X07 as a general programming language outside deterministic evaluation, with an autonomous LLM agent as the primary user of the toolchain.

Status:
- ✅ Phase H3 implemented

Deliverables:
- New world(s) (standalone-only):
  - `run-os` (real filesystem, net, time, env, process)
  - `run-os-sandboxed` (capability-limited, policy-driven)
- OS-backed stdlib adapters:
  - `std.os.fs`, `std.os.net`, `std.os.process`, `std.os.env`, `std.os.time`
- Agent-first tooling surface (standalone + eval-compatible):
  - canonical sources are x07AST JSON (`*.x07.json`)
  - structured diagnostics (JSON) + patch-based edits (RFC 6902) for self-repair loops
  - machine-first formatting/linting/fixing as stable contracts (no human-centric syntax constraints)
- Human review translator (optional but recommended):
  - deterministic `x07c explain` that converts x07AST into a review-friendly representation (natural language or a stable debug view)
- Determinism separation:
  - deterministic suites never run OS worlds
  - stdlib modules can be written against `std.io` traits and bound to either fixtures or OS via world adapters

Exit criteria:
- The same program can be run in deterministic fixture worlds (tests) and OS world (real usage) with only binding changes.

### Phase H4 — Systems programming + ecosystem maturity (C/Rust-class)

Goal: reach “C/Rust-level” capability and ergonomics on the C backend, while keeping the LLM-first contracts stable enough for fully autonomous agents.

Deliverables:
- Systems features:
  - explicit `unsafe` region for raw pointer ops and FFI (compiler-gated by world capabilities)
  - C ABI interop: `extern` functions, header generation, linking to system libs
  - custom allocator ABI for kernels/embedded targets; freestanding builds where possible
  - collection iteration as streams: standardize “iterate” as `iface` readers (collection `open_read_*` adapters), with `emit_*` remaining the canonical materializers for stable bytes
- Toolchain:
  - formatter, linter, documentation generator (all agent-oriented: stable machine output, stable codes, deterministic ordering)
  - package publish/registry flow (start private/offline, grow later)
  - debugger-friendly output: source maps, stack traces, deterministic crash dumps in eval
- Agent workflow (end-to-end):
  - deterministic repair transcripts (diagnostics + patch + attempts) as first-class artifacts
  - a non-interactive “solve → lint/fix → compile/run → repair” loop suitable for autonomous CI agents
- Quality gates:
  - fuzzing of compiler + runtime
  - sanitizer builds in CI
  - performance baselines (compile + runtime regression tests)

Exit criteria:
- Build a small-but-real CLI tool and an HTTP client tool in X07 (standalone OS world),
  while preserving deterministic “fixture replay” tests for both in fixture worlds.

Status (implementation):
- ✅ `unsafe` blocks (standalone-only) and unsafe-required errors for unsafe ops
- ✅ Raw pointer types + core pointer/memory primitives (standalone-only)
- ✅ `extern` C function declarations + call lowering (standalone-only; `ffi` capability)
- ⬜ Header generation + link configuration in the project/runner workflow
- ⬜ Allocator ABI abstraction + freestanding compile profile
- ⬜ Debug source maps (`#line`) and deterministic backtraces for crashes
