Below is a concrete, production‑oriented plan to “finish” multiprocessing for **end‑users (coding agents)** across platforms, given your current state:

* solve‑* worlds stay **single‑core deterministic** (async scheduler + channels)
* run‑os / run‑os‑sandboxed are where you do **real OS multiprocessing** via subprocesses

I’ll focus on the *gaps you listed* and the *next changes that unlock real-world usage*.

---

## 0) Where you are today is the correct foundation

Your current design (handle‑based process table + builtins `spawn/try_join/join/kill/drop` + policy limits) is exactly the right “kernel” for OS multiprocessing in an LLM‑first language:

* It makes “parallelism” explicit (spawn OS processes).
* It keeps deterministic solve‑* worlds clean.
* It gives you policy enforcement hooks (max_live/max_spawns etc.).

The main remaining blockers are **platform support** and **ergonomics** (so agents can reliably build pools / pipelines without tripping on encoding or I/O edge cases).

---

## 1) Hard gap #1: Windows backend (required for “different platforms”)

Right now you’re POSIX‑centric; to be production‑ready you need a Windows backend for the same builtin surface.

### 1.1 Minimal Windows backend requirements

To match your current POSIX semantics, Windows needs:

1. **Spawn** with:

   * explicit `exe` path (no PATH search unless you add it intentionally)
   * argv list → command line construction rules (Windows differs)
   * env map (or explicit “inherit none” vs “inherit allowed”)
   * optional working directory (cwd)
2. **Pipe wiring** for stdin/stdout/stderr capture
3. **Join** / **try_join** / **kill** / **drop**
4. **Process tree cleanup** (so you don’t leak grandchildren)
5. **Policy enforcement** (`max_live`, `max_spawns`, allowlist execs)

### 1.2 Recommended Windows primitives

**CreateProcessW + STARTUPINFO + handle inheritance**
Microsoft’s reference pattern for redirected stdin/stdout/stderr uses `CreateProcess` with pipe handles placed in `STARTUPINFO` and `bInheritHandles=TRUE`. ([Microsoft Learn][1])

**Job Objects for “kill the whole tree” and hard limits**
Windows Job Objects are the standard way to manage a process group and kill everything when needed. The “kill on job close” mechanism is specifically designed for cleanup. ([Stack Overflow][2])

**Key policy mapping on Windows**:

* `process.max_live` ⇒ `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` (or enforce in your runtime + optionally in job limits)
* “kill tree” ⇒ assign process to job, then `TerminateJobObject` or “kill on job close” ([Microsoft Learn][3])

### 1.3 Pipe / streaming gotcha on Windows

If you later add streaming (you should), note: **anonymous pipes don’t support overlapped I/O** (async), which makes true nonblocking streaming tricky without threads or named pipes. Microsoft’s docs for anonymous pipes explicitly call out the limitation. ([Microsoft Learn][4])

**Practical recommendation for v1 Windows parity**:

* Keep “capture model” first (blocking reads internally), using worker threads to drain pipes if needed.
* Add “stream model” later with **named pipes** or “threaded pump” (see §2).

This keeps implementation sane and gets Windows support shipped.

---

## 2) Hard gap #2: “capture only” blocks real worker pools

You wrote it yourself: *“Capture model only: no streaming stdin/stdout/stderr APIs yet.”*

That’s the single biggest reason you can’t build a true multiprocessing pool / map‑reduce system cleanly.

### 2.1 Why capture-only is not enough

* Worker pools require:

  * spawn once
  * send tasks repeatedly (stdin streaming)
  * read responses repeatedly (stdout streaming)
  * poll multiple workers concurrently

Even if you spawn many short-lived processes, capture-only is still awkward and expensive.

### 2.2 Also: capture-only is a correctness risk (pipe deadlocks)

If you “wait for child exit” while not draining stdout/stderr, you can deadlock when the child fills the pipe buffer. Rust’s own `std::process` docs warn about this exact pattern. ([Rust Documentation][5])
Raymond Chen also describes the “pipes clog and everybody stops” failure mode. ([Microsoft for Developers][6])

So: even if you keep “capture”, **your join implementation must continuously drain pipes while waiting** (or do it on background threads).

### 2.3 Add a streaming subprocess API (v1)

Keep your existing `spawn_capture_v1` etc. for simple “run tool and get output”. Add a second, parallel API for pools:

#### New builtins (run-os* only)

* `os.process.spawn_piped_v1(req_bytes, caps_bytes) -> i32`

  * creates process + pipes
  * returns handle `h`

* `os.process.stdin_write_v1(h, chunk_bytes) -> i32`

  * deterministic return codes:

    * `1` ok
    * `0` closed/broken pipe
    * traps on policy violations (optional)

* `os.process.stdin_close_v1(h) -> i32`

* `os.process.stdout_read_v1(h, max_i32) -> bytes`

* `os.process.stderr_read_v1(h, max_i32) -> bytes`

  * returns empty bytes if no data available **and process still running**
  * returns empty bytes + `proc_state` query indicates finished (see below)

* `os.process.try_wait_v1(h) -> i32`

  * `0` still running
  * `1` exited (then exit info is available via `os.process.take_exit_v1`)

* `os.process.take_exit_v1(h) -> bytes`

  * returns a fixed encoding (see §3)

* `os.process.kill_v1(h, mode_i32) -> i32`

  * `mode=0` terminate (soft)
  * `mode=1` kill (hard)

* `os.process.drop_v1(h) -> i32`

  * always releases table slot
  * policy: in sandboxed mode, you can require “must kill before drop” if desired

#### Scheduler integration

Treat these as **yield boundaries** exactly like your join builtin:

* `stdout_read_v1`, `stderr_read_v1`, `stdin_write_v1`, `try_wait_v1`
  should “step scheduler” or at least not block forever.

Because run‑os is nondeterministic anyway, you can choose a pragmatic implementation (threads or poll/select).

### 2.4 Implementation strategy: POSIX vs Windows

#### POSIX

* Use `posix_spawnp` (optional) and `posix_spawn_file_actions_adddup2` for stdio redirection.
* Use nonblocking pipes + `poll`/`select` (or `epoll/kqueue`) to drain without deadlocking.

#### Windows

* Use `CreateProcessW` + redirected handles. ([Microsoft Learn][1])
* Use **Job Objects** to manage process tree. ([Microsoft Learn][3])
* For streaming:

  * easiest first: **dedicated reader threads** per stdout/stderr pipe that append to ring buffers (bounded by caps)
  * later: named pipes + overlapped I/O (more work; only needed if you want “fully async” without threads). ([Microsoft Learn][4])

---

## 3) Gap #3: “req/caps are bytes” is too low-level for agents

You already see it: agents must build `ProcReqV1/ProcCapsV1` manually.

For **LLM-first production**, you should provide:

* canonical builders (so agents don’t hand-assemble bytes)
* canonical decoders (so agents don’t guess tags/offsets)
* a strict “v1 encoding” doc that never changes

### 3.1 Add `std.os.process.req_v1` builder helpers

**Goal:** agents never hand-encode structs.

Examples (stdlib, not builtins):

* `std.os.process.req_v1.new(exe: bytes) -> vec_u8`
* `std.os.process.req_v1.arg(req: vec_u8, arg: bytes) -> vec_u8`
* `std.os.process.req_v1.env(req: vec_u8, key: bytes, val: bytes) -> vec_u8`
* `std.os.process.req_v1.stdin(req: vec_u8, stdin: bytes) -> vec_u8`
* `std.os.process.req_v1.finish(req: vec_u8) -> bytes`

Same for caps:

* `std.os.process.caps_v1.pack(max_stdout_bytes: i32, max_stderr_bytes: i32, timeout_ms: i32, max_total_bytes: i32) -> bytes`

### 3.2 Add stable doc decoders

Your builtin returns a “doc bytes blob” (ok/err with exit_code/stdout/stderr etc.).

Provide stable getters:

* `std.os.process.is_err(doc: bytes_view) -> i32`
* `std.os.process.err_code(doc: bytes_view) -> i32`
* `std.os.process.resp_exit_code(doc: bytes_view) -> i32`
* `std.os.process.resp_stdout(doc: bytes_view) -> bytes`
* `std.os.process.resp_stderr(doc: bytes_view) -> bytes`

Agents should treat the doc as opaque and only use these accessors.

---

## 4) Gap #4: “No pool library” ⇒ every user reinvents it

Once you have streaming, you should ship one canonical pool module.

### 4.1 `std.os.process.pool_v1` (high-level multiprocessing)

Provide “one right way” patterns:

* `pool.new(workers_i32, req_template_bytes, caps_bytes) -> pool`
* `pool.map_bytes(pool, inputs_bytes_u32le_list) -> outputs_bytes_u32le_list`
* `pool.close(pool) -> i32`

Where the worker protocol is fixed:

* stdin: length‑prefixed frames `[u32le len][payload...]`
* stdout: same framing for responses
* stderr: collected as debug log stream (optional)

This makes multiprocessing usable for agents without them reasoning about scheduling.

### 4.2 Keep it cross-platform

All pool complexity stays inside:

* the streaming subprocess builtins
* the pool stdlib module

So end-user code is identical on Linux/macOS/Windows.

---

## 5) Gap #5: Policy + security realism for production

Your current `run-os-sandboxed` is “policy allowlist + rlimits-ish”, which is fine for *trusted agent tooling*, but not for running random user code.

To move toward production readiness:

### 5.1 Extend policy schema in a compatible way

Add fields (still deterministic to parse; stable JSON):

* `process.allow_execs`: already
* `process.allow_exec_prefixes`: allow `deps/x07/` without enumerating every helper
* `process.allow_args_regex_lite`: **optional** but useful (careful: regex engines can be denial-of-service)
* `process.allow_env_keys`: allowlist env variables
* `process.allow_cwd_roots`: allowlist cwd roots
* `process.max_stdout_bytes`, `max_stderr_bytes`, `max_total_bytes`: enforce in runtime
* `process.max_runtime_ms`: enforce
* `process.kill_on_drop`: sandboxed default true (prevents orphan processes)

### 5.2 Make “capability leaks” impossible by construction

* In run‑os‑sandboxed, default `unsafe`/`extern` OFF unless explicitly enabled in policy (`language.allow_unsafe`, `language.allow_ffi`).
* Enforce this by compiling from source/project in `x07-os-runner` (do not accept precompiled `--artifact` in run-os-sandboxed).
* Otherwise a program can bypass your process policy anyway.

---

## 6) Gap #6: Cross-platform join semantics + “process tree” semantics must be explicit

Agents need a **single invariant**:

> “Killing or dropping a handle will not leave children running.”

On POSIX this means:

* put child into its own process group/session and kill the group
* or track descendants (hard)
  On Windows this means:
* job objects + kill-on-close / terminate job ([Microsoft Learn][3])

Write this invariant into docs and enforce it.

---

## 7) Optional but high leverage: use a cross-platform process library internally

If you want to reduce OS‑specific C runtime code, you have two viable internal strategies:

### Option A: keep your current “embedded C runtime” approach

* implement `_WIN32` backend alongside POSIX
* most control, no external deps
* but more maintenance

### Option B: link a small cross-platform runtime (libuv) in run-os builds

libuv gives you a unified process + pipes + event loop abstraction across Windows and Unix. ([Nikhil M.][7])
This can drastically lower the cost of “streaming subprocess I/O” on Windows.

Tradeoff: you introduce a dependency and an event loop runtime; but in run‑os worlds, nondeterminism is already accepted.

---

## 8) Concrete “next steps” priority order (what unblocks users fastest)

### P0 — Windows parity for capture API (1–2 PRs)

* Implement `spawn_capture_v1/try_join_capture_v1/join_capture_v1/kill/drop` on Windows
* Use Job Objects for cleanup/kill-tree ([Microsoft Learn][3])
* Use CreateProcess redirected handles ([Microsoft Learn][1])

### P1 — Make capture implementation deadlock-proof (POSIX + Windows)

* Ensure join drains stdout/stderr while waiting (poll or threads)
* Add tests for “child writes lots of output” (proves no deadlock)

  * this is exactly the kind of failure Rust warns about ([Rust Documentation][5])

### P2 — Add streaming subprocess API (piped) + std.io integration

* spawn_piped + stdin_write + stdout_read + try_wait + take_exit
* implement as yield boundaries (safe for defasync scheduler)
* on Windows, prefer threads first; anonymous pipe overlapped is limited ([Microsoft Learn][4])

### P3 — Add req/caps/doc builders and decoders in stdlib

* eliminates “bytes encoding” pain for agents

### P4 — Add `std.os.process.pool_v1`

* the actual user-facing unlock for multiprocessing workflows

### P5 — Harden sandbox policy + portable semantics

* ensure run‑os‑sandboxed can be used by default for agent tooling

---

## 9) What platforms will be “good enough” after this?

* **Linux**: fully workable, best support (posix + poll/epoll)
* **macOS**: workable with posix_spawn + kqueue/poll
* **Windows**: workable once CreateProcessW + Job Objects + pipe pumping exists

And your user-facing API stays the same everywhere.

---

## 10) The single most important principle for agent success

**Agents should never manipulate raw request/caps/doc bytes by hand.**

They should:

* call `std.os.process.req_v1.*`
* call `std.os.process.caps_v1.pack`
* decode via `std.os.process.is_err` / `std.os.process.err_code` / `std.os.process.resp_*`
* and use `std.os.process.pool_v1` for parallel workloads

That’s how you make multiprocessing “agentic” rather than “systems programming in bytes.”

---

[1]: https://learn.microsoft.com/en-us/windows/win32/procthread/creating-a-child-process-with-redirected-input-and-output?utm_source=chatgpt.com "Creating a Child Process with Redirected Input and Output"
[2]: https://stackoverflow.com/questions/1173342/terminate-a-process-tree-c-for-windows "Terminate a process tree (C for Windows) - Stack Overflow"
[3]: https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information "JOBOBJECT_BASIC_LIMIT_INFORMATION (winnt.h) - Win32 apps | Microsoft Learn"
[4]: https://learn.microsoft.com/en-us/windows/win32/ipc/anonymous-pipe-operations?utm_source=chatgpt.com "Anonymous Pipe Operations - Win32 apps"
[5]: https://doc.rust-lang.org/stable/std/process/struct.Stdio.html "Stdio in std::process - Rust"
[6]: https://devblogs.microsoft.com/oldnewthing/20230209-00/?p=107812 "A more direct and mistake-free way of creating a process in a job object - The Old New Thing"
[7]: https://nikhilm.github.io/uvbook/processes.html "Processes — An Introduction to libuv"
++++
Below is a repo‑aligned **PROC checklist** that turns “P0–P4” into **PR‑sized, shippable milestones** to make multiprocessing (via OS subprocesses) production‑ready across Linux/macOS/Windows—**without weakening the deterministic solve‑* model**.

I’m assuming your current state already has:

* builtins: `os.process.spawn_capture_v1 / try_join_capture_v1 / join_capture_v1 / kill_v1 / drop_v1`
* run‑os policy gating + `process.max_live` / `process.max_spawns`
* `std.os.process.*` wrappers except `join` (because join must remain a builtin yield boundary)

If any API name differs, keep the shape and adjust the literal names in one place (PROC‑01) so everything stays coherent.

---

## Process ABI v1 (freeze in PROC‑01)

The canonical bytes layouts for subprocess requests/caps/results are defined in `docs/os/process-abi-v1.md`.

Agents should not hand-encode these bytes. Use:

* `std.os.process.req_v1.*`
* `std.os.process.caps_v1.pack`
* decode via `std.os.process.is_err` / `std.os.process.err_code` / `std.os.process.resp_*`

---

## Sandbox policy schema additions (run‑os‑sandboxed)

Add these to `schemas/run-os-policy.schema.json` under `.process`:

* `max_exe_bytes` (int, default 4096)
* `max_args` (int, default 64)
* `max_arg_bytes` (int, default 4096)
* `max_env` (int, default 64)
* `max_env_key_bytes` (int, default 256)
* `max_env_val_bytes` (int, default 4096)
* `max_runtime_ms` (int, default 0; when non-zero, caps timeout is bounded and defaulted)
* `allow_cwd` (bool, default false)
* `allow_cwd_roots` (array<string>, default [])
* `allow_exec_prefixes` (array<string>, default [])
* `allow_args_regex_lite` (array<string>, default [])
  * If non-empty: every `argv[i]` for `i>=1` must full-match at least one pattern.
  * Pattern syntax (regex-lite): `.` matches any byte; `x*` repeats token; `\\` escapes a literal byte.
* `allow_env_keys` (array<string>, default [])

  * If empty, reject any env entries.
* `max_stdout_bytes`, `max_stderr_bytes`, `max_total_bytes`, `max_stdin_bytes`

  * These become the global ceilings for ProcCapsV1.
* `kill_on_drop` (bool, default true)

  * If handle dropped while running, terminate it.
* `kill_tree` (bool, default true)

  * Ensure child process tree is terminated (see Windows job objects / POSIX process groups).

On Windows, the right primitive for “kill tree” is a **Job Object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and the invariant “the job handle is not inherited by children”, otherwise closing your handle may not terminate the tree.

---

## PROC PR checklist (P0–P4)

### PROC‑01 — Freeze Process ABI v1 + builder/decoder helpers

**Goal (P3):** make req/caps/doc construction + decoding **LLM‑reliable** (no hand‑rolled offsets in every program).

**Adds**

* `docs/os/process-abi-v1.md` (normative)

  * contains the canonical ABI v1 layouts
  * contains stable `err_code` table
* `schemas/process-abi-v1.schema.json` *(optional but recommended)*
  JSON Schema for *metadata representations* (not for bytes), used by your tooling/tests.
* `stdlib/os/0.1.0/modules/std/os/process/req_v1.x07.json` *(new)*

  * **Req builder**

    * `std.os.process.req_v1.new(exe: bytes) -> vec_u8`
    * `std.os.process.req_v1.arg(req: vec_u8, arg: bytes) -> vec_u8`
    * `std.os.process.req_v1.env(req: vec_u8, key: bytes, val: bytes) -> vec_u8`
    * `std.os.process.req_v1.stdin(req: vec_u8, stdin: bytes) -> vec_u8`
    * `std.os.process.req_v1.finish(req: vec_u8) -> bytes`
* `stdlib/os/0.1.0/modules/std/os/process/caps_v1.x07.json` *(new)*

  * **Caps pack**

    * `std.os.process.caps_v1.pack(max_stdout_bytes: i32, max_stderr_bytes: i32, timeout_ms: i32, max_total_bytes: i32) -> bytes`
* `stdlib/os/0.1.0/modules/std/os/process.x07.json`

  * **Doc decode**

    * `std.os.process.is_err(doc: bytes_view) -> i32`
    * `std.os.process.err_code(doc: bytes_view) -> i32`
    * `std.os.process.resp_exit_code(doc: bytes_view) -> i32`
    * `std.os.process.resp_stdout(doc: bytes_view) -> bytes`
    * `std.os.process.resp_stderr(doc: bytes_view) -> bytes`
* `docs/spec/language-guide.md` updates: “canonical subprocess pattern”
* `crates/x07c/src/guide.rs` regeneration hook

**Touch**

* `stdlib/os/0.1.0/modules/std/os/process.x07.json`
  Re-export codec helpers or import them.
* `stdlib.lock` bump for `x07:os@0.1.0` if needed

**CI / scripts**

* `scripts/ci/check_process_abi_v1.sh`

  * runs unit tests that roundtrip pack→parse in X07 (pure) and in Rust test harness.

**Acceptance**

* Agents can spawn helper using only `std.os.process.*` + `os.process.join_capture_v1` without writing offsets.

---

### PROC‑02 — Windows parity for capture mode (spawn/join/kill/drop)

**Goal (P0):** `spawn_capture_v1` stack works on Windows with the same observable semantics as POSIX.

**Why this PR is non‑optional**

* Windows subprocess control is *not* posix_spawn; you need CreateProcess + handle inheritance and a “kill tree” primitive. Microsoft explicitly documents job objects and kill‑on‑close semantics.

**Implementation**

* `crates/x07c/src/c_emit.rs` (embedded C runtime)

  * `#if defined(_WIN32)` backend:

    * build `STARTUPINFOEXW` / `STARTUPINFOW` with redirected handles
    * create pipes for stdin/stdout/stderr
    * create a Job Object per spawned process (or reuse one per “run”)
    * set `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
    * ensure job handle is **NOT inheritable** to avoid leaked handles preventing termination
    * `kill_v1` → `TerminateJobObject` (tree) or `TerminateProcess` (single)
* `crates/x07-os-runner/` (if needed)

  * pass policy env vars identically on Windows
  * canonicalize allowlisted exe paths with `.exe`

**Important Windows constraints to bake into design**

* Windows uses a single command‑line string; parsing is typically via `CommandLineToArgvW` and has special backslash/quote behavior. You must implement a deterministic quoting function for argv generation.

**Policy additions (if not already present)**

* `process.kill_tree` (default true) must be honored on Windows via Job Objects.

**Smoke suites**

* `benchmarks/run-os/proc-capture-smoke.json` (shared)

  * include per‑case `platforms: [...]`
* `benchmarks/run-os/proc-capture-smoke.windows.json` (if you prefer OS‑split)

  * uses `deps/x07/x07-proc-echo.exe`

**Acceptance**

* On Windows, Linux, macOS: same X07 program spawns helper, captures stdout, exit_code OK.

---

### PROC‑03 — Deadlock‑proof capture join (POSIX + Windows)

**Goal (P1):** eliminate the classic “child blocks because stdout pipe buffer is full” failure mode.

This is a real production issue: if you “wait then read”, you can deadlock because the child can’t progress while the parent isn’t draining pipes. Rust even warns about this class of issues around stdio handling / waiting patterns.

**Implementation**

* POSIX runtime:

  * set stdout/stderr pipes **non‑blocking**
  * `poll`/`select` loop:

    * drain stdout/stderr while waiting for exit
    * enforce `max_*_bytes` caps while draining
    * if cap exceeded → kill (tree if policy) and return `ERR_OUTPUT_LIMIT`
* Windows runtime:

  * anonymous pipes don’t support overlapped I/O, so you either:

    * (A) use **threads** to drain stdout/stderr concurrently (recommended first)
    * or (B) use **named pipes with overlapped I/O** (more complex)
      Microsoft notes anonymous pipes aren’t overlapped.

**Adds**

* `proc_stats` (optional but very helpful):

  * `proc_spawn_calls`
  * `proc_join_calls`
  * `proc_kill_calls`
  * `proc_stdout_bytes`
  * `proc_stderr_bytes`
  * `proc_peak_live_handles`

**Smoke suites**

* `benchmarks/run-os/proc-capture-large-output-smoke.json`

  * helper emits >64KB stdout and stderr
  * asserts:

    * join succeeds
    * output length matches
    * no hang
* `benchmarks/run-os-sandboxed/proc-output-limit-smoke.json`

  * same helper, caps limit small
  * asserts `ERR_OUTPUT_LIMIT`

**Acceptance**

* No hangs on “large output” across Linux/macOS/Windows.

---

### PROC‑04 — Streaming subprocess API v1 (pipes) + std.io integration

**Goal (P2):** allow long‑lived workers (real multiprocessing) instead of “one process per job”.

#### Minimal builtin API (exact names)

Add these **standalone-only builtins** (hard error in solve-* builds):

* `os.process.spawn_piped_v1(req: bytes, caps: bytes) -> i32`
* `os.process.stdout_read_v1(h: i32, max: i32) -> bytes`
* `os.process.stderr_read_v1(h: i32, max: i32) -> bytes`
* `os.process.stdin_write_v1(h: i32, chunk: bytes) -> i32`
* `os.process.stdin_close_v1(h: i32) -> i32`
* `os.process.try_wait_v1(h: i32) -> i32`

  * returns `1` if exited, else `0`
* `os.process.join_exit_v1(h: i32) -> i32` *(yield boundary)*
* `os.process.take_exit_v1(h: i32) -> i32`

  * returns exit_code, or traps/err if not exited
* `os.process.drop_v1(h: i32) -> i32` (already exists)

#### Stdlib wrappers (agent-friendly)

In `stdlib/os/0.1.0/modules/std/os/process.x07.json` add:

* `std.os.process.spawn_piped_v1(req, caps) -> i32`
* `std.os.process.read_stdout_v1(h, max) -> bytes`
* `std.os.process.read_stderr_v1(h, max) -> bytes`
* `std.os.process.write_stdin_v1(h, chunk) -> i32`
* `std.os.process.close_stdin_v1(h) -> i32`
* `std.os.process.join_exit_v1(h) -> i32`
  **IMPORTANT:** implement this as a LangDef alias macro or as a special “builtin call wrapper” pattern, not a normal `defn`, so it stays a yield boundary.

#### Scheduler integration

* `join_exit_v1` must be treated like `os.process.join_capture_v1`: a yield boundary (allowed in solve/top-level or defasync; rejected in defn unless you choose to open that gate later).

#### Cross-platform backend

* POSIX: pipes + poll + nonblocking
* Windows: either

  * threads for read/write queues, or
  * named pipes overlapped (future)
    Start with threads for reliability.

**Smoke suites**

* `benchmarks/run-os/proc-stream-smoke.json`

  * uses a helper worker that reads frames and echoes them back
  * asserts:

    * multiple request/response rounds
    * no deadlock
    * correct byte framing

**Acceptance**

* You can keep N worker processes alive and exchange many messages without respawning.

---

### PROC‑05 — Worker pool stdlib (parallel_map) + cross-platform helper workers

**Goal (P4):** give agents **one canonical way** to do CPU parallelism via subprocess workers.

#### Worker protocol v1 (exact bytes)

Frame format (stdin/out):

* `u32_le id`
* `u32_le len`
* `len` bytes payload

Response frame:

* `u32_le id`
* `u32_le len`
* `len` bytes result

(Errors inside worker can be encoded as `len=0xFFFF_FFFF` + `u32_le err_code` next, but keep v1 simple.)

#### Stdlib module

Add `stdlib/os/0.1.0/modules/std/os/process_pool.x07.json` with:

* `std.os.process_pool.new_v1(req: bytes, caps: bytes, n_workers: i32) -> bytes`
  returns an opaque pool state blob (contains process handles + round-robin index + outstanding map)
* `std.os.process_pool.map_bytes_v1(pool: bytes, items: bytes) -> bytes`

  * `items` encoding: `u32 count` + repeated `bytes item`
  * output encoding: same, outputs aligned to inputs order
* `std.os.process_pool.close_v1(pool: bytes) -> i32`

  * kills/drops any remaining workers (policy.kill_on_drop respected)

You already have rich collections; the pool can store:

* `next_id` counter
* `pending` map: id → output slot
* per-worker inflight counters

#### Helper worker binaries (deps/x07/)

Add a tiny Rust helper (build per‑platform) if you don’t already have one:

* `crates/x07-proc-worker-frame-echo/`

  * reads frames, returns frames (maybe uppercases ASCII for test)

Update:

* `scripts/build_os_helpers.sh`

  * builds + copies `x07-proc-worker-frame-echo{.exe}` into `deps/x07/` deterministically

**Smoke suites**

* `benchmarks/run-os/proc-pool-smoke.json`

  * spawn pool with `n=2` or `n=4`
  * map 100 small items and check ordering
* `benchmarks/run-os-sandboxed/proc-pool-policy-smoke.json`

  * verify policy allowlist required

**Acceptance**

* End-user can do: “spawn pool once, map 1k jobs” and it works on all OSes.

---

## Concrete smoke suite set (Linux/macOS/Windows)

You asked for “concrete smoke suites for Linux/macOS/Windows.” Here’s a minimal set that covers the real gaps:

### 1) `benchmarks/run-os/proc-capture-smoke.json`

Cases:

* `spawn_capture_echo_small`
* `spawn_capture_echo_binary`
* `spawn_capture_exit_code`

### 2) `benchmarks/run-os/proc-capture-large-output-smoke.json`

Cases:

* `stdout_1mb_no_deadlock`
* `stderr_256kb_no_deadlock`

### 3) `benchmarks/run-os/proc-stream-smoke.json`

Cases:

* `frame_echo_roundtrip_10`
* `frame_echo_interleave_stdout_stderr` (optional)

### 4) `benchmarks/run-os/proc-pool-smoke.json`

Cases:

* `pool_map_100_ascii_upper` (worker uppercases)
* `pool_map_order_preserved`

### 5) `benchmarks/run-os-sandboxed/proc-policy-smoke.json`

Cases:

* `deny_unlisted_exec`
* `allow_listed_exec`
* `deny_env_key`
* `deny_cwd_outside_root`

### 6) `benchmarks/run-os-sandboxed/proc-limits-smoke.json`

Cases:

* `max_live_enforced`
* `max_spawns_enforced`
* `max_stdout_enforced`
* `timeout_enforced`

**Platform binding**

* Either:

  * keep single suite and add `platforms: ["windows"]` fields per case, or
  * maintain `*.windows.json` variants.

For agent-friendliness I recommend the **single suite with `platforms`**, and your runner filters by `std::env::consts::OS`.

---

## Notes that matter for correctness (why these choices)

* **Kill-tree on Windows:** Job Objects with `KILL_ON_JOB_CLOSE` are the standard way to guarantee child cleanup, but only if the job handle isn’t inherited/leaked.
* **Windows pipes & async:** anonymous pipes don’t support overlapped I/O, so don’t try to make them “pollable”. Use threads or named pipes for async streaming.
* **Command line quoting on Windows:** CreateProcess takes a command line string; `CommandLineToArgvW` parsing has special backslash rules, so you must implement deterministic quoting if you want `argv[]` to roundtrip reliably.
* **Deadlock-proof capture:** draining stdout/stderr concurrently is mandatory for large outputs; this is a well-known class of bug.
* **POSIX spawn plumbing:** you’ll keep using file actions (`dup2`/close) to wire pipes at spawn time.

---

## If you want one “single canonical user story” for agents

After PROC‑01..05, the agent‑friendly canonical patterns become:

1. **One-shot capture** (simple):

* build `req` via `std.os.process.req_v1.*`
* build `caps` via `std.os.process.caps_v1.pack`
* `h = std.os.process.spawn_capture_v1(req,caps)`
* `doc = os.process.join_capture_v1(h)`
* `std.os.process.drop_v1(h)`
* decode via `std.os.process.is_err` / `std.os.process.err_code` / `std.os.process.resp_*`

2. **Pool parallel_map** (fast):

* create pool once
* `std.os.process_pool.map_bytes_v1(pool, items_blob)`
* close pool

That’s exactly the kind of “one way to do it” that reduces LLM error rate.

---
+++++
Below are **copy/paste‑ready** smoke suite JSON files in the **same “suite_id/world/tasks/cases + per‑case assertions” style** as your existing phase suites, but extended with **run‑os / run‑os‑sandboxed** case fields:

* `program_path` (fixed program to compile+run for this task)
* `argv_utf8` (argv tokens, UTF‑8)
* `stdin_b64`
* `expected_exit_code`, `expected_stdout_b64`, `expected_stderr_b64`
* `policy_inline` (run‑os‑sandboxed only)
* `only_platforms` (optional; if your runner doesn’t support it yet, split into 3 files per OS)

I also made the outputs validate the **ProcDocV1 bytes ABI** directly by expecting the program to emit **raw ProcDocV1 bytes** to stdout.

---

## `benchmarks/run-os/proc-echo-smoke.json`

```json
{
  "suite_id": "run-os/proc-echo-smoke@0.1.0",
  "world": "run-os",
  "tasks": [
    {
      "task_id": "proc_spawn_join_capture_v1_doc_encoding",
      "description": "Spawns helper process, captures stdout/stderr, joins, and emits the raw ProcDocV1 bytes to stdout (tests doc ABI).",
      "program_path": "tests/external_os/process_spawn/src/main.x07.json",
      "assertions": {
        "max_wall_time_ms": 2000,
        "max_proc_spawns": 1,
        "max_proc_peak_live": 1
      },
      "cases": [
        {
          "name": "posix_hi",
          "only_platforms": [
            "linux",
            "macos"
          ],
          "argv_utf8": [
            "process_spawn",
            "--mode",
            "emit_doc_ok_hi"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AQAAAAACAAAAaGkAAAAA",
          "expected_stderr_b64": ""
        },
        {
          "name": "windows_hi",
          "only_platforms": [
            "windows"
          ],
          "argv_utf8": [
            "process_spawn.exe",
            "--mode",
            "emit_doc_ok_hi"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AQAAAAACAAAAaGkAAAAA",
          "expected_stderr_b64": ""
        }
      ]
    }
  ]
}
```

**What this asserts**

* join returns an **OK doc** with:

  * tag=1
  * exit_code=0
  * stdout_len=2, stdout=`"hi"`
  * stderr_len=0

---

## `benchmarks/run-os/proc-async-join-smoke.json`

```json
{
  "suite_id": "run-os/proc-async-join-smoke@0.1.0",
  "world": "run-os",
  "tasks": [
    {
      "task_id": "proc_spawn_two_join_async",
      "description": "Spawns two helper processes from defasync tasks, then uses os.process.join_capture_v1 as a yield boundary and emits concatenated child stdout.",
      "program_path": "tests/external_os/process_spawn_async_join/src/main.x07.json",
      "assertions": {
        "max_wall_time_ms": 3000,
        "max_proc_spawns": 2,
        "max_proc_peak_live": 2,
        "max_sched_yields": 100000
      },
      "cases": [
        {
          "name": "posix_ab",
          "only_platforms": [
            "linux",
            "macos"
          ],
          "argv_utf8": [
            "process_spawn_async_join",
            "--mode",
            "emit_concat_ab"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "YWI=",
          "expected_stderr_b64": ""
        },
        {
          "name": "windows_ab",
          "only_platforms": [
            "windows"
          ],
          "argv_utf8": [
            "process_spawn_async_join.exe",
            "--mode",
            "emit_concat_ab"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "YWI=",
          "expected_stderr_b64": ""
        }
      ]
    }
  ]
}
```

**What this asserts**

* You can use **async tasks** to manage multiple procs and `join_capture_v1` provides a yield boundary (doesn’t deadlock your cooperative scheduler integration).
* The program is expected to emit `b"ab"` (base64 `YWI=`).

---

## `benchmarks/run-os-sandboxed/proc-policy-smoke.json`

```json
{
  "suite_id": "run-os-sandboxed/proc-policy-smoke@0.1.0",
  "world": "run-os-sandboxed",
  "tasks": [
    {
      "task_id": "proc_policy_allow_deny_execs",
      "description": "Validates run-os-sandboxed policy gating: allowlisted exec succeeds; unlisted exec is denied with ProcDocV1 ERR(code=POLICY_DENIED). Program emits raw ProcDocV1 bytes.",
      "program_path": "tests/external_os/process_policy_smoke/src/main.x07.json",
      "assertions": {
        "max_wall_time_ms": 2000,
        "max_proc_spawns": 1,
        "max_proc_peak_live": 1
      },
      "cases": [
        {
          "name": "allow_posix",
          "only_platforms": [
            "linux",
            "macos"
          ],
          "policy_inline": {
            "schema_version": "x07.run-os-policy@0.1.0",
            "process": {
              "enabled": true,
              "allow_spawn": true,
              "allow_execs": [
                "deps/x07/x07-proc-echo"
              ],
              "max_live": 4,
              "max_spawns": 16,
              "max_runtime_ms": 1000,
              "max_stdout_bytes": 4096,
              "max_stderr_bytes": 4096,
              "max_total_output_bytes": 8192
            }
          },
          "argv_utf8": [
            "process_policy_smoke",
            "--mode",
            "emit_doc_ok_hi"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AQAAAAACAAAAaGkAAAAA",
          "expected_stderr_b64": ""
        },
        {
          "name": "deny_posix",
          "only_platforms": [
            "linux",
            "macos"
          ],
          "policy_inline": {
            "schema_version": "x07.run-os-policy@0.1.0",
            "process": {
              "enabled": true,
              "allow_spawn": true,
              "allow_execs": [
                "deps/x07/definitely-not-allowed"
              ],
              "max_live": 4,
              "max_spawns": 16,
              "max_runtime_ms": 1000,
              "max_stdout_bytes": 4096,
              "max_stderr_bytes": 4096,
              "max_total_output_bytes": 8192
            }
          },
          "argv_utf8": [
            "process_policy_smoke",
            "--mode",
            "emit_doc_policy_denied"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AAIAAAA=",
          "expected_stderr_b64": ""
        },
        {
          "name": "allow_windows",
          "only_platforms": [
            "windows"
          ],
          "policy_inline": {
            "schema_version": "x07.run-os-policy@0.1.0",
            "process": {
              "enabled": true,
              "allow_spawn": true,
              "allow_execs": [
                "deps/x07/x07-proc-echo.exe"
              ],
              "max_live": 4,
              "max_spawns": 16,
              "max_runtime_ms": 1000,
              "max_stdout_bytes": 4096,
              "max_stderr_bytes": 4096,
              "max_total_output_bytes": 8192
            }
          },
          "argv_utf8": [
            "process_policy_smoke.exe",
            "--mode",
            "emit_doc_ok_hi"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AQAAAAACAAAAaGkAAAAA",
          "expected_stderr_b64": ""
        },
        {
          "name": "deny_windows",
          "only_platforms": [
            "windows"
          ],
          "policy_inline": {
            "schema_version": "x07.run-os-policy@0.1.0",
            "process": {
              "enabled": true,
              "allow_spawn": true,
              "allow_execs": [
                "deps/x07/definitely-not-allowed.exe"
              ],
              "max_live": 4,
              "max_spawns": 16,
              "max_runtime_ms": 1000,
              "max_stdout_bytes": 4096,
              "max_stderr_bytes": 4096,
              "max_total_output_bytes": 8192
            }
          },
          "argv_utf8": [
            "process_policy_smoke.exe",
            "--mode",
            "emit_doc_policy_denied"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AAIAAAA=",
          "expected_stderr_b64": ""
        }
      ]
    }
  ]
}
```

**Notes**

* `expected_stdout_b64` for deny case is `ProcDocV1 ERR(code=2)` encoded as: `00 02 00 00 00` → base64 `AAIAAAA=`.
* If your implementation uses different numeric error codes, keep the *shape* and swap the expected bytes.

---

## `benchmarks/run-os-sandboxed/proc-limits-smoke.json`

```json
{
  "suite_id": "run-os-sandboxed/proc-limits-smoke@0.1.0",
  "world": "run-os-sandboxed",
  "tasks": [
    {
      "task_id": "proc_limits_max_live_and_timeout_and_output",
      "description": "Validates process caps: max_live, timeout, and output byte limits. Program emits raw ProcDocV1 bytes.",
      "program_path": "tests/external_os/process_spawn_max_live/src/main.x07.json",
      "assertions": {
        "max_wall_time_ms": 5000
      },
      "cases": [
        {
          "name": "max_live_posix",
          "only_platforms": [
            "linux",
            "macos"
          ],
          "policy_inline": {
            "schema_version": "x07.run-os-policy@0.1.0",
            "process": {
              "enabled": true,
              "allow_spawn": true,
              "allow_execs": [
                "deps/x07/x07-proc-echo"
              ],
              "max_live": 1,
              "max_spawns": 16,
              "max_runtime_ms": 1000,
              "max_stdout_bytes": 4096,
              "max_stderr_bytes": 4096,
              "max_total_output_bytes": 8192
            }
          },
          "argv_utf8": [
            "process_spawn_max_live",
            "--mode",
            "emit_doc_too_many_live"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AAYAAAA=",
          "expected_stderr_b64": ""
        },
        {
          "name": "timeout_posix",
          "only_platforms": [
            "linux",
            "macos"
          ],
          "policy_inline": {
            "schema_version": "x07.run-os-policy@0.1.0",
            "process": {
              "enabled": true,
              "allow_spawn": true,
              "allow_execs": [
                "deps/x07/x07-proc-echo"
              ],
              "max_live": 4,
              "max_spawns": 16,
              "max_runtime_ms": 10,
              "max_stdout_bytes": 4096,
              "max_stderr_bytes": 4096,
              "max_total_output_bytes": 8192
            }
          },
          "argv_utf8": [
            "process_spawn_max_live",
            "--mode",
            "emit_doc_timeout"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AAQAAAA=",
          "expected_stderr_b64": ""
        },
        {
          "name": "output_limit_posix",
          "only_platforms": [
            "linux",
            "macos"
          ],
          "policy_inline": {
            "schema_version": "x07.run-os-policy@0.1.0",
            "process": {
              "enabled": true,
              "allow_spawn": true,
              "allow_execs": [
                "deps/x07/x07-proc-echo"
              ],
              "max_live": 4,
              "max_spawns": 16,
              "max_runtime_ms": 1000,
              "max_stdout_bytes": 1,
              "max_stderr_bytes": 4096,
              "max_total_output_bytes": 8192
            }
          },
          "argv_utf8": [
            "process_spawn_max_live",
            "--mode",
            "emit_doc_output_limit"
          ],
          "stdin_b64": "",
          "expected_exit_code": 0,
          "expected_stdout_b64": "AAUAAAA=",
          "expected_stderr_b64": ""
        }
      ]
    }
  ]
}
```

**What these expect (by construction)**

* `AAYAAAA=` = `ProcDocV1 ERR(code=6)` (too many live handles)
* `AAQAAAA=` = `ProcDocV1 ERR(code=4)` (timeout)
* `AAUAAAA=` = `ProcDocV1 ERR(code=5)` (output limit)

Again: keep shape, adjust numeric codes if needed.

---

## If you don’t want `only_platforms` yet

If your benchmark runner doesn’t support `only_platforms`, the simplest “no-runner-change” approach is:

* keep the **exact same JSON shape**, but split into:

  * `proc-echo-smoke.linux.json`, `proc-echo-smoke.macos.json`, `proc-echo-smoke.windows.json`
  * same for policy/limits
* and run the correct one from your CI script based on `uname` / `$OS`.
