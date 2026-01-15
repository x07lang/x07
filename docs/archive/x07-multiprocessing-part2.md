Below is a **developer-facing, production‑oriented implementation plan** to unblock **real multiprocessing** in X07 by upgrading the current **single blocking `run_capture_v1`** into a **handle‑based process subsystem** that:

* works **only in `run-os*` worlds** (never in `solve-*`),
* supports **multiple concurrent subprocesses** (true multi‑core parallelism via OS scheduling),
* integrates cleanly with your **existing cooperative async scheduler** (so you can `spawn` many workers and `join` them without blocking the whole program),
* is hardened by **policy + caps** (especially in `run-os-sandboxed`),
* and remains consistent with X07’s “LLM-first / canonical patterns” goal.

I’ll assume your current baseline is exactly what your report describes: `os.process.run_capture_v1(req,caps)->bytes` (POSIX `posix_spawn`, pipes, capture, allowlist policy, caps).

---

# 1) Target capability and constraints

## What “real multiprocessing” means in your architecture

* **solve-* worlds**: deterministic by design → keep **single‑core cooperative** only.
* **run-os / run-os-sandboxed**: non‑deterministic by design → allow **real OS processes** and thus multi-core parallelism.

The implementation should therefore:

* **Hard error** if any OS process builtins appear in any `solve-*` compilation.
* **Allow and fully support** handle-based process management in `run-os*`.

This is aligned with the “determinism split” you already have.

---

# 2) Public API: new builtins + canonical stdlib facade

You want to go from:

* **v1**: one blocking call `run_capture_v1(req,caps)->bytes`

to:

* **v2**: `spawn` returns a handle, `try_join` polls, `join` yields and completes.

## 2.1 Builtins (runtime + compiler)

Add these new builtins **standalone-only** (`run-os` + `run-os-sandboxed`):

### Process lifecycle

1. `os.process.spawn_capture_v1(req_bytes, caps_bytes) -> i32`

* returns `proc_handle` (opaque)

2. `os.process.try_join_capture_v1(proc_handle) -> option_bytes` *(recommended)*

* returns:

  * `none` if still running
  * `some(result_bytes)` if finished, where `result_bytes` uses the **same encoding as `run_capture_v1`**

3. `os.process.join_capture_v1(proc_handle) -> bytes`

* blocking/yielding join (allowed only in `solve` or `defasync`, not in `defn` — same rule as `task.join.bytes`)

4. `os.process.kill_v1(proc_handle, sig_i32) -> i32`

* returns 1 if signal sent, 0 otherwise (or error codes)

5. `os.process.drop_v1(proc_handle) -> i32`

* explicit close of handle; in sandboxed mode this should **kill/reap** if still running (see below)

### Why `try_join` should be nonblocking

On POSIX, you can poll child status using `waitpid(..., WNOHANG)`; if nothing has changed it returns **0** and does not block. ([man7.org][1])
That’s exactly what your scheduler needs.

## 2.2 Stdlib facade (what agents will call)

In `stdlib/std/<ver>/modules/std/os/process.x07.json` expose the canonical API:

* `std.os.process.spawn_capture(req,caps) -> i32`
* `std.os.process.try_join_capture(handle) -> option_bytes`
* `std.os.process.join_capture(handle) -> bytes`
* `std.os.process.run_capture(req,caps) -> bytes`
  (either calls v1 builtin for simplicity or implements as spawn+join for code reuse)

### Canonical “worker fan-out/fan-in” pattern (agent-friendly)

The single canonical way to use multiprocessing should be:

1. spawn N processes
2. poll/join them cooperatively
3. aggregate results

No alternative “modes” unless required.

---

# 3) Policy changes: make it production-safe

Your report says run-os-sandboxed already enforces allowlists and some caps. The missing production gates are **rate limits and concurrency limits**.

## 3.1 Update policy schema (JSON)

Add fields under `process`:

* `max_live` (max concurrent running processes)
* `max_spawns` (max total spawns during one program run)
* optionally:

  * `kill_on_drop` (default true in sandboxed)
  * `default_timeout_ms`
  * `max_stdout_bytes`, `max_stderr_bytes`, `max_total_output_bytes`

These prevent “spawn storms” and resource exhaustion.

## 3.2 Runner should compile policy into env vars

You’re already passing policy via env vars. Extend it with:

* `X07_OS_PROC_MAX_LIVE`
* `X07_OS_PROC_MAX_SPAWNS`
* (and optionally the output/timeout defaults)

## 3.3 Why you should stick to `posix_spawn`, not `fork`

If you ever link anything threaded (or even if the OS runner evolves), POSIX requires that after `fork()` in a multithreaded program the child should call only **async-signal-safe** operations until `exec`. ([The Open Group][2])
Using `posix_spawn()` avoids a whole class of “fork in multithreaded process” hazards and is explicitly intended as a standardized process creation mechanism. ([The Open Group][3])

---

# 4) Compiler work: add builtins, types, and world gating

This is mostly in `crates/x07c/src/c_emit.rs` (plus whatever builtin tables/type infer you have).

## 4.1 Add builtin signatures

* `spawn_capture_v1`: `(bytes, bytes) -> i32`
* `try_join_capture_v1`: `(i32) -> option_bytes` (or bytes-encoded pending)
* `join_capture_v1`: `(i32) -> bytes`
* `kill_v1`: `(i32, i32) -> i32`
* `drop_v1`: `(i32) -> i32`

## 4.2 Hard gating by world

You want **compile-time hard errors** if any of these builtins appear in solve worlds.

Rule:

* If `world.is_solve_*`: forbid all `os.process.*`
* If `world.is_run_os*`: allow

This is a “production safety invariant” — it prevents accidental nondeterminism leaks into deterministic worlds.

## 4.3 Gating blocking ops inside `defn`

Keep your existing discipline:

* `join_capture_v1` is a **blocking/yielding boundary** → only in `solve` or `defasync`
* `try_join_capture_v1` is nonblocking → allowed in `defn`

This matches your current `task.join.bytes` vs `task.try_join.bytes` pattern and keeps “pure computation” functions from accidentally blocking.

---

# 5) Runtime C implementation: process table + nonblocking polling

This is the heart of “real multiprocessing”.

## 5.1 Add a process table in runtime state

Add a table of `rt_os_proc_t` entries in `ctx_t`, with:

* `gen` (generation counter)
* `state` (EMPTY/RUNNING/EXITED/FAILED)
* `pid`
* fds for `stdin/stdout/stderr`
* buffers for captured stdout/stderr
* counters + limits (stdout_bytes, stderr_bytes, total)
* timeouts and start timestamp (monotonic)
* flags: `joined_taken`, `kill_on_drop`
* optionally: `waiter list` (tasks waiting on this handle)

### Handle encoding (avoid stale handles)

Use `i32 handle = (gen << 16) | idx` (or similar) so:

* `idx` addresses table slot
* `gen` prevents use-after-free by rejecting stale handles

This is important for long-running agent workflows.

## 5.2 Spawn implementation (POSIX)

Spawn is:

1. parse `req_bytes` (reuse existing v1 format)
2. enforce policy:

   * allow_execs
   * max_live, max_spawns
3. create pipes
4. set fds `O_NONBLOCK` where needed
5. use `posix_spawn` with file-actions for `dup2` and close:

   * POSIX defines `posix_spawn` and `posix_spawnp` for process creation. ([The Open Group][3])
6. close child-ends in parent
7. store `pid + fds + buffers` into table slot
8. return handle

## 5.3 Nonblocking IO mechanics (stdout/stderr capture)

To support many concurrent children, do **nonblocking reads** from each stdout/stderr pipe.

Key behaviors you must handle:

* Pipes in nonblocking mode can return partial reads and EAGAIN.
* Nonblocking writes can return partial writes and EAGAIN if the pipe is full. ([man7.org][4])
* Use `poll()` to check readiness; beware POLLHUP semantics: you may see POLLHUP and still have buffered data to read; EOF (read=0) happens once drained. ([man7.org][5])

Implementation approach:

* In `rt_os_process_poll_all(ctx)`:

  * build `pollfd[]` for all running procs’ stdout/stderr fds
  * poll with timeout 0 (nonblocking)
  * for any readable fd:

    * read in a loop until EAGAIN or buffer cap hit
  * for stdin:

    * if request included stdin payload, write as much as possible; close stdin when done

## 5.4 Exit detection

Per process:

* call `waitpid(pid, &status, WNOHANG)`

  * returns `0` if still running ([man7.org][1])
  * returns `pid` when exited
* when exited:

  * keep draining stdout/stderr until EOF (or until buffers are capped)
  * then mark READY

## 5.5 Timeouts / output caps / kill semantics

Production readiness requires strict caps:

* If timeout exceeded:

  * kill (SIGKILL or policy-selected)
  * mark FAILED with deterministic error code (timeout)
* If output exceeds caps:

  * kill
  * mark FAILED (output_limit)
* If process denied by policy:

  * fail immediately (policy_denied)
* If spawn fails:

  * FAILED (spawn_failed)

---

# 6) Scheduler integration: make join a yield boundary

Right now you have cooperative scheduling. To avoid blocking the whole VM when waiting for a subprocess:

## 6.1 Add a new wait kind

* `WAIT_OS_PROC_JOIN(handle)`

## 6.2 `join_capture_v1` should not block in async contexts

In `defasync` lowering:

* `join_capture_v1` → set task state WAIT_OS_PROC_JOIN(handle) and yield
* scheduler calls `rt_os_process_poll_all()` each step
* once a proc transitions to READY:

  * wake waiters
  * next poll of task returns the result bytes

## 6.3 `join_capture_v1` in top-level `solve`

Top-level can “block” *cooperatively*:

* loop:

  * if try_join returns done → return
  * else `rt_sched_step()` and retry

This gives you the same semantics, while still allowing other tasks to run.

---

# 7) Drop semantics: don’t leak processes

This is a major production concern.

Rust `std::process::Child` intentionally does **not** kill on Drop (a dropped handle doesn’t stop the child) ([Rust Documentation][6])
…but async ecosystems often add “kill_on_drop” because it is safer in agentic/async flows. ([Docs.rs][7])

### Recommended X07 rule

* In `run-os-sandboxed`: `drop_v1` **must** kill+reap if still running.
* In `run-os`: either

  * kill-on-drop default true (safer for autonomous agents), or
  * optional `caps.kill_on_drop` flag, default true.

Also add **global cleanup**:

* at program exit, kill+reap all still-running children (especially critical in sandboxed mode).

---

# 8) Windows support plan (for true “production” reach)

Your current implementation is POSIX-centric. If you want “general adoption”, you’ll need a Windows backend eventually.

## 8.1 Use Job Objects

Windows Job Objects are the standard approach to manage process trees and enforce policies (including kill-on-close patterns). ([Microsoft Learn][8])

Windows plan:

* Create a job object for the X07 program
* Assign every spawned child to that job
* On timeout/output limit/program exit:

  * terminate job (kills subtree)

## 8.2 Pipe handling

Implement CreateProcess with inherited pipe handles (or use a cross-platform library like libuv later). Keep the same **req_bytes** encoding.

---

# 9) Tests and benchmark suites to prove “real multiprocessing” without flaky timing

Avoid timing-based assertions. Prefer semantic invariants.

## 9.1 `benchmarks/run-os/proc-echo-smoke.json`

* spawn N children of a helper binary (like your `x07-proc-echo`)
* join all
* assert stdout matches inputs, stderr empty, exit=0

## 9.2 `benchmarks/run-os-sandboxed/proc-policy-smoke.json`

* policy denies unknown exec → assert `policy_denied` code
* policy max_live=1:

  * spawn first → ok
  * spawn second without joining/dropping → `max_live` error

## 9.3 Concurrency proof test (no wall-clock)

* spawn 4 processes
* loop with `try_join`:

  * count completions
  * ensure you can progress and eventually collect all results
    This proves you can manage multiple concurrent children without blocking.

---

# 10) PR-sized implementation sequence (recommended)

Here’s how I’d stage it so each PR is reviewable and shippable.

## PR PROC‑01: Spec + policy schema + env vars

* Update `schemas/run-os-policy.schema.json`:

  * add `process.max_live`, `process.max_spawns`, maybe default limits
* Update OS runner:

  * parse new fields
  * export env vars
* Add tests for policy parsing.

## PR PROC‑02: Compiler builtins + gating

* Add builtin signatures in x07c
* Add world gating (hard error in solve-*)
* Add `defn` gating:

  * allow `try_join`
  * forbid `join`

## PR PROC‑03: Runtime process table (spawn + poll + try_join)

* Add `rt_os_proc_t` and handle encoding
* Implement:

  * `spawn_capture_v1`
  * `try_join_capture_v1`
  * `kill_v1`
  * `drop_v1`
* Keep join temporarily as simple blocking (for now).

## PR PROC‑04: Scheduler integration (yielding join)

* Add wait kind `WAIT_OS_PROC_JOIN`
* Implement:

  * `join_capture_v1` yield in defasync and solve
  * wake waiters when proc completes
* Add concurrency smoke suite (no timing).

## PR PROC‑05: Hardening

* Enforce output caps during polling
* Enforce timeouts using monotonic time
* Ensure kill+reap on cleanup
* Add more negative tests (bad handle, double join, etc.)

## PR PROC‑06: Windows roadmap (optional but “production”)

* Add compile-time stub errors on Windows until supported, **or** implement Job Object backend.
* Document OS portability and supported targets.

---

# 11) “Production readiness” checklist for multiprocessing

You’re production-ready (for agentic usage) when:

* ✅ Handle-based API exists and can run **many concurrent subprocesses**.
* ✅ `run-os-sandboxed` can enforce:

  * allow_execs
  * max_live
  * max_spawns
  * timeouts
  * output caps
* ✅ join is cooperative (does not block the whole runtime)
* ✅ drop semantics prevent orphan processes in sandboxed mode
* ✅ there are semantic smoke suites for:

  * correctness
  * policy denies
  * handle errors
* ✅ docs include one canonical “worker fan-out/fan-in” pattern.

---

[1]: https://man7.org/linux/man-pages/man2/wait.2.html?utm_source=chatgpt.com "wait(2) - Linux manual page"
[2]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fork.html?utm_source=chatgpt.com "fork"
[3]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_spawn.html?utm_source=chatgpt.com "posix_spawn"
[4]: https://man7.org/linux/man-pages/man7/pipe.7.html?utm_source=chatgpt.com "pipe(7) - Linux manual page"
[5]: https://man7.org/linux/man-pages/man2/poll.2.html?utm_source=chatgpt.com "poll(2) - Linux manual page"
[6]: https://doc.rust-lang.org/std/process/struct.Child.html?utm_source=chatgpt.com "Child in std::process"
[7]: https://docs.rs/tokio/latest/tokio/process/struct.Child.html?utm_source=chatgpt.com "Child in tokio::process - Rust"
[8]: https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects?utm_source=chatgpt.com "Job Objects - Win32 apps"
