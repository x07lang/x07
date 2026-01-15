Yes — **it’s worth adding multiprocessing**, but only if you frame it as an **OS‑world capability** with **strict policy + caps** and a **single canonical API**, not as “random fork/exec everywhere”.

If you do it that way, it becomes one of the highest‑leverage external capabilities for **100% agentic coding**: agents can parallelize CPU work, isolate risky work, and integrate with existing toolchains (git/cc/linkers/etc) without bloating your core runtime.

The biggest trap is implementing it the “obvious Unix way” (`fork()` from a multithreaded runtime). That’s a correctness and reliability footgun.

## The key technical constraint: avoid fork in a multithreaded runtime

On Linux/POSIX, **after `fork()` in a multithreaded program, the child may safely call only async‑signal‑safe functions until it calls `execve()`**. That rules out most of what a language runtime might do (allocation, locks, logging, etc.). ([man7.org][1])

So if X07 has any threads (scheduler thread, async runtime, background I/O, etc.), a naive fork-based spawn is brittle.

**Practical implication:** for “spawn a new program” semantics, prefer **`posix_spawn`/`posix_spawnp`** (Unix) and **CreateProcess** (Windows) style implementations, rather than forking your runtime and then trying to cleanly exec. This is exactly why many ecosystems steer toward spawn APIs which reduce the fork hazards; posix_spawn is often recommended in threaded contexts. ([LWN.net][2])

## What “multiprocessing” should mean for X07

There are two different needs that people call “multiprocessing”:

1. **Subprocess / command execution** (spawn another program, capture output, set env/cwd, etc.)
2. **Parallel compute workers** (run N worker processes, send them jobs, collect results)

For agentic coding, (1) is immediately useful and low ambiguity.
(2) is useful too, but you should implement it as a **library built on top of (1)** (with a single canonical “worker protocol”), not as a bunch of ad-hoc primitives.

## Recommendation: add it as OS‑only external module, with a 2‑layer API

### Layer A (v1): One canonical “run & capture” function

This eliminates 80% of real-world needs and is *agent-friendly*.

**Module:** `std.os.process` (run-os + run-os-sandboxed only)

**Canonical API:**

* `std.os.process.run_capture_v1(req_bytes, caps_bytes) -> ResultBytesV1`

Where:

* `req_bytes` is a **canonical request encoding** (argv list, env delta, cwd, stdin bytes)
* `caps_bytes` enforces **max_stdout_bytes, max_stderr_bytes, max_total_bytes, timeout_ms, max_procs, allowlist**, etc.
* `ResultBytesV1` returns either error code or `ProcRespV1` (exit status + stdout/stderr bytes + “killed/timeout” flags)

This is the X07 equivalent of Rust’s `std::process::Command` use cases (“spawn and get output”). ([Rust Documentation][3])

**Why this is agentic-friendly**

* No choices: one way to run a process.
* Easy to cap and sandbox deterministically.
* Easy to diagnose: structured response bytes → deterministic error codes.

### Layer B (v1.1): Streaming + handle-based spawn

Only after v1 is stable.

Add:

* `spawn_v1(req_bytes, caps_bytes) -> ResultBytesV1(handle)`
* `wait_v1(handle) -> ResultBytesV1(exit_status)`
* `stdin_write(handle, chunk) -> i32`
* `stdout_read(handle, max) -> bytes`
* `stderr_read(handle, max) -> bytes`
* `kill(handle, signal_or_code) -> i32`

This is for long-running tools and large outputs.

## Then “multiprocessing” proper becomes a library on top

Once Layer A is in, you can create a **single canonical worker pool**:

**Module:** `std.os.mproc.pool`

* `pool.new(worker_cmd_req, n, caps) -> pool_handle`
* `pool.map_bytes(pool, jobs_bytes) -> result_bytes`

  * deterministic “jobs encoding” (length-prefixed list)
  * deterministic “results order” (same order as jobs)
  * worker protocol is strict: length-prefixed stdin/stdout frames

This gives you true parallelism with:

* no shared memory (simpler and safer)
* deterministic aggregation semantics (the *result ordering* is deterministic even though execution timing isn’t)

## Sandboxing and policy must be first-class

Because you said X07 is for autonomous coding agents, **process spawning must never be “ambient”**.

In `run-os-sandboxed`:

* default: deny spawn or allow only a tiny allowlist
* policy decides:

  * allowed executables (hash or path allowlist)
  * allowed argv patterns (or no shell)
  * allowed cwd roots
  * env var allowlist
  * max concurrent children
  * rlimits / job object limits
  * timeouts

This mirrors your existing H3 philosophy (“opt-in, policy-driven, never used in deterministic eval”).

## Deterministic testing strategy (important for agentic reliability)

Even if “run-os” is inherently nondeterministic (timing), you can still have **deterministic tests**:

1. **Policy denial smoke tests**

   * In `run-os-sandboxed` default policy, `run_capture_v1` must return `ERR_POLICY_DENIED` deterministically.

2. **Fixture-backed “mock process world” for regression tests**

   * Add a *test-only runner mode* where `run_capture_v1` is backed by fixtures:

     * key = hash(req_bytes || caps_bytes)
     * value = recorded ProcRespV1 bytes
   * This makes “process-using programs” testable like your rr/kv fixtures.

## When it is NOT worth it

If your immediate roadmap is still dominated by:

* filling stdlib gaps,
* tightening ABI/types,
* stabilizing the agent repair loop,

…then implement **subprocess (Layer A)** first and delay “pool/true multiprocessing” until you have at least one workload that benefits (package builds, registry client, big parsing tasks, etc.).

But I wouldn’t skip subprocess support in a “general-purpose agent language” — it’s too useful.

## Concrete decision

**Worth it — yes, but do it as:**

* OS-only (`std.os.process`), never in solve-*.
* Implement spawn via **posix_spawn/CreateProcess** style paths, not fork-based runtime cloning (because fork + threads is restricted). ([man7.org][1])
* Start with `run_capture_v1` as the single canonical API; build pools on top.

---

[1]: https://man7.org/linux/man-pages/man2/fork.2.html?utm_source=chatgpt.com "fork(2) - Linux manual page"
[2]: https://lwn.net/Articles/674712/?utm_source=chatgpt.com "fork & multithreading"
[3]: https://doc.rust-lang.org/std/process/index.html?utm_source=chatgpt.com "std::process - Rust"
++++

Multiprocessing (spawning OS processes) **is worth adding** for 100% agentic coding *outside eval*, because it unlocks:

* calling existing system tools (compilers, linters, formatters, git, curl, etc.) as “skills” the agent can chain;
* isolating potentially-crashy work in a child process;
* parallelizing I/O-heavy workflows (with explicit caps + policy).

For implementation on Unix, **prefer `posix_spawn` / `posix_spawnp` over `fork`+`exec`** in a threaded runtime (which you already have because of G2 concurrency): `fork()` in a multithreaded process has sharp restrictions on what’s safe to do before `exec` and can deadlock if you touch non–async-signal-safe code. ([IBM][1])

Below are the **four concrete files** you asked for (drop-in). They’re designed to keep OS process APIs:

* **standalone-only** (never used in deterministic `solve-*` worlds),
* **policy-gated** under `run-os-sandboxed`,
* **fully absent / hard error** when compiling for any `solve-*` target (enforced in compiler/driver; not in this module stub).

---

## 1) `stdlib/std/<ver>/modules/std/os/process.x07.json`

> Replace `<ver>` with your current pinned stdlib version directory (e.g. `0.1.2`).

```lisp
; std.os.process — OS subprocess adapter (standalone-only, never used in solve-*).
;
; This module is only legal to import/compile under:
;   - world = run-os
;   - world = run-os-sandboxed (policy gated)
;
; Any attempt to import std.os.process in solve-* compile targets MUST be a hard compiler error.
;
;------------------------------------------------------------------------------
; ABI: ResultBytesV1 (returned bytes)
;
;   OK  : 0x01 | ProcRespV1
;   ERR : 0x00 | err_code_u32_le
;
; Error codes (u32):
;   1 = POLICY_DENIED
;   2 = INVALID_REQUEST
;   3 = SPAWN_FAILED
;   4 = TIMEOUT
;   5 = OUTPUT_LIMIT
;
;------------------------------------------------------------------------------
; ProcReqV1 encoding (req: bytes)
;
;   u8  ver = 1
;   u8  flags (bit0=clear_env, bit1=inherit_env; v1: flags=0 recommended)
;   u32 argv_count
;     repeat argv_count:
;       u32 arg_len
;       bytes arg
;   u32 env_count
;     repeat env_count:
;       u32 key_len, bytes key
;       u32 val_len, bytes val
;   u32 cwd_len (0 = no override)
;   bytes cwd
;   u32 stdin_len
;   bytes stdin
;
; NOTE: argv[0] is the executable path (no shell expansion). Shell execution is forbidden in sandboxed mode.
;
;------------------------------------------------------------------------------
; ProcCapsV1 encoding (caps: bytes)
;
;   u8  ver = 1
;   u32 max_stdout_bytes
;   u32 max_stderr_bytes
;   u32 timeout_ms
;   u32 max_total_bytes   (0 = max_stdout_bytes + max_stderr_bytes)
;
;------------------------------------------------------------------------------
; ProcRespV1 encoding (payload inside OK)
;
;   u8  ver = 1
;   u32 exit_code
;   u32 flags (bit0=timed_out, bit1=killed)
;   u32 stdout_len, stdout bytes
;   u32 stderr_len, stderr bytes
;
;------------------------------------------------------------------------------

(export std.os.process.run_capture_v1)

; Run a process and capture stdout/stderr (bounded by caps).
; Returns ResultBytesV1.
(defn std.os.process.run_capture_v1 ((req bytes) (caps bytes)) bytes
  (os.process.run_capture_v1 req caps))
```

---

## 2) `spec/x07.os.process.policy.schema.json`

This is the **single source of truth** for the sandbox policy file format. It’s intentionally small, deterministic, and allowlist-first.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://x07.io/spec/x07.os.process.policy.schema.json",
  "title": "X07 run-os-sandboxed process policy",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "process"],
  "properties": {
    "schema_version": {
      "const": "x07.os.process.policy@0.1.0"
    },
    "process": {
      "type": "object",
      "additionalProperties": false,
      "required": ["default_action", "deny_shell", "limits", "allow"],
      "properties": {
        "default_action": {
          "description": "Default action when no allow rule matches.",
          "type": "string",
          "enum": ["deny", "allow"]
        },
        "deny_shell": {
          "description": "Must be true in v1. Shell execution is forbidden.",
          "type": "boolean",
          "const": true,
          "default": true
        },
        "limits": {
          "description": "Global upper bounds; per-call caps cannot exceed these.",
          "type": "object",
          "additionalProperties": false,
          "required": [
            "max_concurrent_children",
            "timeout_ms_max",
            "max_stdout_bytes_max",
            "max_stderr_bytes_max",
            "max_total_bytes_max",
            "max_stdin_bytes_max",
            "max_env_entries_max",
            "max_arg_bytes_max"
          ],
          "properties": {
            "max_concurrent_children": { "type": "integer", "minimum": 0, "maximum": 64 },
            "timeout_ms_max": { "type": "integer", "minimum": 0, "maximum": 600000 },
            "max_stdout_bytes_max": { "type": "integer", "minimum": 0, "maximum": 16777216 },
            "max_stderr_bytes_max": { "type": "integer", "minimum": 0, "maximum": 16777216 },
            "max_total_bytes_max": { "type": "integer", "minimum": 0, "maximum": 33554432 },
            "max_stdin_bytes_max": { "type": "integer", "minimum": 0, "maximum": 16777216 },
            "max_env_entries_max": { "type": "integer", "minimum": 0, "maximum": 256 },
            "max_arg_bytes_max": { "type": "integer", "minimum": 0, "maximum": 1048576 }
          }
        },
        "allow": {
          "description": "Allowlist rules. First match wins (order matters).",
          "type": "array",
          "maxItems": 256,
          "items": { "$ref": "#/$defs/allow_rule" }
        }
      }
    }
  },
  "$defs": {
    "ident": {
      "type": "string",
      "pattern": "^[a-z0-9][a-z0-9._-]{0,63}$"
    },
    "exec_selector": {
      "description": "Select an executable by exact path or sha256. v1 recommends 'path'.",
      "oneOf": [
        {
          "type": "object",
          "additionalProperties": false,
          "required": ["kind", "path"],
          "properties": {
            "kind": { "const": "path" },
            "path": {
              "type": "string",
              "minLength": 1,
              "maxLength": 4096
            }
          }
        },
        {
          "type": "object",
          "additionalProperties": false,
          "required": ["kind", "sha256_hex"],
          "properties": {
            "kind": { "const": "sha256" },
            "sha256_hex": {
              "type": "string",
              "pattern": "^[0-9a-f]{64}$"
            }
          }
        }
      ]
    },
    "args_policy": {
      "description": "How argv is constrained (argv[0] is still validated separately by exec selector).",
      "type": "object",
      "additionalProperties": false,
      "required": ["mode"],
      "properties": {
        "mode": { "type": "string", "enum": ["any", "exact", "prefix"] },
        "exact": {
          "type": "array",
          "items": { "type": "string", "maxLength": 4096 },
          "maxItems": 128
        },
        "prefix": {
          "type": "array",
          "items": { "type": "string", "maxLength": 4096 },
          "maxItems": 128
        }
      },
      "allOf": [
        {
          "if": { "properties": { "mode": { "const": "exact" } } },
          "then": { "required": ["exact"] }
        },
        {
          "if": { "properties": { "mode": { "const": "prefix" } } },
          "then": { "required": ["prefix"] }
        }
      ]
    },
    "env_policy": {
      "type": "object",
      "additionalProperties": false,
      "required": ["inherit_allowlist", "set_allowlist", "denylist", "max_entries"],
      "properties": {
        "inherit_allowlist": {
          "description": "Only these variables may be inherited from the host environment.",
          "type": "array",
          "maxItems": 128,
          "items": { "type": "string", "pattern": "^[A-Z_][A-Z0-9_]{0,63}$" }
        },
        "set_allowlist": {
          "description": "Only these variable names may be set via ProcReqV1 env entries.",
          "type": "array",
          "maxItems": 128,
          "items": { "type": "string", "pattern": "^[A-Z_][A-Z0-9_]{0,63}$" }
        },
        "denylist": {
          "description": "Always forbidden regardless of inherit/set allowlists.",
          "type": "array",
          "maxItems": 128,
          "items": { "type": "string", "pattern": "^[A-Z_][A-Z0-9_]{0,63}$" }
        },
        "max_entries": { "type": "integer", "minimum": 0, "maximum": 256 }
      }
    },
    "allow_rule": {
      "type": "object",
      "additionalProperties": false,
      "required": ["id", "exec", "args", "cwd_roots", "env", "caps_max"],
      "properties": {
        "id": { "$ref": "#/$defs/ident" },
        "exec": { "$ref": "#/$defs/exec_selector" },
        "args": { "$ref": "#/$defs/args_policy" },
        "cwd_roots": {
          "description": "Allowed working directories (prefix match). Empty means 'no cwd override allowed'.",
          "type": "array",
          "maxItems": 32,
          "items": { "type": "string", "minLength": 1, "maxLength": 4096 }
        },
        "env": { "$ref": "#/$defs/env_policy" },
        "caps_max": {
          "description": "Upper bounds for ProcCapsV1 for this rule.",
          "type": "object",
          "additionalProperties": false,
          "required": ["timeout_ms", "max_stdout_bytes", "max_stderr_bytes", "max_total_bytes", "max_stdin_bytes"],
          "properties": {
            "timeout_ms": { "type": "integer", "minimum": 0, "maximum": 600000 },
            "max_stdout_bytes": { "type": "integer", "minimum": 0, "maximum": 16777216 },
            "max_stderr_bytes": { "type": "integer", "minimum": 0, "maximum": 16777216 },
            "max_total_bytes": { "type": "integer", "minimum": 0, "maximum": 33554432 },
            "max_stdin_bytes": { "type": "integer", "minimum": 0, "maximum": 16777216 }
          }
        }
      }
    }
  }
}
```

---

## 3) `benchmarks/run-os-sandboxed/proc-policy-smoke.json`

This suite **verifies policy denial is deterministic** (and happens *before* any real spawn). It expects your `run-os-sandboxed` world to load a policy that denies process execution by default.

```json
{
  "suite_id": "run-os-sandboxed/proc-policy-smoke@0.1.0",
  "world": "run-os-sandboxed",
  "tasks": [
    {
      "task_id": "proc/policy_denied_run_capture_v1",
      "description": "Policy smoke: calling std.os.process.run_capture_v1 must fail with POLICY_DENIED (err_code=1) under run-os-sandboxed default policy.\n\nInput encoding:\n  u32le req_len | req_bytes(ProcReqV1) | u32le caps_len | caps_bytes(ProcCapsV1)\n\nOutput encoding (ResultBytesV1):\n  ERR: 0x00 | err_code_u32_le\n\nThis suite assumes sandbox default_action=deny or an equivalent deny rule.\n",
      "assertions": {
        "capabilities_required": [
          "os.process.run_capture"
        ],
        "policy_required": true,
        "expected_error_code_u32": 1
      },
      "cases": [
        {
          "name": "deny_hello",
          "input_b64": "OQAAAAEAAQAAAB4AAABkZXBzL2V2b2xhbmcvZXZvbGFuZy1wcm9jLWVjaG8AAAAAAAAAAAUAAABoZWxsbxEAAAABQAAAAEAAAADoAwAAgAAAAA==",
          "expected_b64": "AAEAAAA="
        },
        {
          "name": "deny_empty_stdin",
          "input_b64": "NAAAAAEAAQAAAB4AAABkZXBzL2V2b2xhbmcvZXZvbGFuZy1wcm9jLWVjaG8AAAAAAAAAAAAAAAARAAAAAUAAAABAAAAA6AMAAIAAAAA=",
          "expected_b64": "AAEAAAA="
        }
      ]
    }
  ]
}
```

---

## 4) `benchmarks/run-os/proc-echo-smoke.json`

This suite **verifies the non-sandboxed OS world** can run a deterministic helper binary and capture stdout.

It assumes you ship/build a helper at **`deps/x07/x07-proc-echo`** that implements:

* read all stdin bytes
* write the same bytes to stdout
* exit code `0`

```json
{
  "suite_id": "run-os/proc-echo-smoke@0.1.0",
  "world": "run-os",
  "tasks": [
    {
      "task_id": "proc/run_capture_echo_v1",
      "description": "Run a tiny helper binary and capture stdout.\n\nHelper binary requirement:\n  deps/x07/x07-proc-echo\nBehavior:\n  stdout = stdin, stderr empty, exit_code=0.\n\nInput encoding:\n  u32le req_len | req_bytes(ProcReqV1) | u32le caps_len | caps_bytes(ProcCapsV1)\n\nOutput encoding (ResultBytesV1):\n  OK: 0x01 | ProcRespV1\nProcRespV1:\n  u8 ver=1 | u32 exit_code | u32 flags | u32 stdout_len | stdout | u32 stderr_len | stderr\n",
      "assertions": {
        "capabilities_required": [
          "os.process.run_capture"
        ],
        "requires_files": [
          "deps/x07/x07-proc-echo"
        ]
      },
      "cases": [
        {
          "name": "echo_empty",
          "input_b64": "NAAAAAEAAQAAAB4AAABkZXBzL2V2b2xhbmcvZXZvbGFuZy1wcm9jLWVjaG8AAAAAAAAAAAAAAAARAAAAAUAAAABAAAAA6AMAAIAAAAA=",
          "expected_b64": "AQEAAAAAAAAAAAAAAAAAAAAA"
        },
        {
          "name": "echo_abc",
          "input_b64": "NwAAAAEAAQAAAB4AAABkZXBzL2V2b2xhbmcvZXZvbGFuZy1wcm9jLWVjaG8AAAAAAAAAAAMAAABhYmMRAAAAAUAAAABAAAAA6AMAAIAAAAA=",
          "expected_b64": "AQEAAAAAAAAAAAMAAABhYmMAAAAA"
        },
        {
          "name": "echo_output_limit",
          "description": "Same request as echo_abc, but caps set max_stdout_bytes=2. Expected OUTPUT_LIMIT err_code=5 (no truncation).",
          "input_b64": "NwAAAAEAAQAAAB4AAABkZXBzL2V2b2xhbmcvZXZvbGFuZy1wcm9jLWVjaG8AAAAAAAAAAAMAAABhYmMRAAAAAQIAAABAAAAA6AMAAAAAAAA=",
          "expected_b64": "AAUAAAA="
        }
      ]
    }
  ]
}
```

---

## Notes you should bake into the runner/host implementation

These aren’t extra files you asked for, but they’re the “rules of the road” that make the above contracts work for agentic coding:

* **Spawn API implementation should avoid `fork` in a threaded runtime**; use `posix_spawn`/`CreateProcess`-style APIs and apply hard caps on stdin/stdout/stderr capture sizes. ([IBM][1])
* **Policy check order (sandboxed):** parse request → check policy allowlist → reject/allow → only then touch filesystem/spawn. This keeps denial deterministic and prevents “leaking existence” of arbitrary paths via side effects.
* **No-shell guarantee:** policy schema sets `deny_shell=true` with `const: true`; runner must reject any “shell mode” flags in request even in run-os-sandboxed.
* **No ambient solve-***: compiler/driver must hard-error if target is any `solve-*` world and the AST imports `std.os.*` (including `std.os.process`). This is your “evaluator isolation” rule made permanent.

[1]: https://www.ibm.com/docs/en/aix/7.3.0?topic=p-posix-spawn-posix-spawnp-subroutine&utm_source=chatgpt.com "posix_spawn or posix_spawnp Subroutine"
+++++
Below is a **drop-in helper crate** plus a **deterministic build/copy script** that installs the helper into `deps/x07/` exactly where your Phase H3 `run-os` benchmark expects it.

This design keeps builds deterministic in the practical sense (same inputs ⇒ same dependency graph, stable output path, stable perms, stable mtime) by using Cargo’s lockfile enforcement and a fixed target dir. Cargo’s `--locked` is specifically meant to assert the lockfile is honored for deterministic builds. ([Rust Documentation][1])

---

# 1) `crates/x07-proc-echo/` (tiny helper binary crate)

## `crates/x07-proc-echo/Cargo.toml`

```toml
[package]
name = "x07-proc-echo"
version = "0.1.0"
edition = "2021"
publish = false

# Keep it tiny and dependency-free.
[dependencies]
```

If your workspace requires explicit membership (instead of globbing), add it to the root workspace members (Cargo workspaces are the standard mechanism here). ([Rust Documentation][2])

Example (only if you *don’t* already use a glob like `crates/*`):

```toml
# Cargo.toml (repo root)
[workspace]
members = [
  "crates/x07-proc-echo",
  # ...
]
```

## `crates/x07-proc-echo/src/main.rs`

This is a **streaming** echo (doesn’t buffer the entire stdin), which is safer for large inputs and is fully deterministic for a given stdin. It uses standard `Read`/`Write` traits. ([Rust Documentation][3])

```rust
use std::io::{self, Read, Write};

fn run() -> io::Result<()> {
    // Stream stdin -> stdout in fixed-size chunks.
    // This keeps memory usage bounded and avoids allocating the full input.
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = stdin.read(&mut buf)?;
        if n == 0 {
            break;
        }
        stdout.write_all(&buf[..n])?;
    }
    stdout.flush()?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        // Deterministic error surface:
        // - message format is stable
        // - exit code is non-zero
        eprintln!("x07-proc-echo: io error: {e}");
        std::process::exit(1);
    }
}
```

## (Optional) `crates/x07-proc-echo/README.md`

```md
# x07-proc-echo

Tiny helper binary used by `run-os` benchmarks.

Behavior:
- reads all stdin until EOF
- writes exactly the same bytes to stdout
- writes nothing to stderr on success
- exits 0 on success; 1 on I/O error
```

---

# 2) `scripts/build_os_helpers.sh` (deterministic build + install into `deps/x07/`)

This script:

* uses `cargo build --release` (release profile is the standard for `--release`) ([Rust Documentation][4])
* uses `--locked` to enforce the lockfile for deterministic dependency resolution ([Rust Documentation][1])
* uses a dedicated `CARGO_TARGET_DIR` so the binary path is stable and CI caching is easier
* copies with fixed permissions and a fixed mtime (useful if you later pack archives deterministically)

Create: `scripts/build_os_helpers.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

# build_os_helpers.sh
# Build standalone OS helper binaries and place them in deps/x07/ deterministically.
#
# Determinism properties:
# - Uses Cargo.lock enforcement (--locked) for stable dependency resolution.
# - Uses a fixed CARGO_TARGET_DIR so paths are stable.
# - Installs with stable mode bits.
# - Sets stable mtime on the installed artifact (useful for deterministic archives).

umask 022

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPS_DIR="${ROOT}/deps/x07"
TARGET_DIR="${ROOT}/target/os-helpers"

mkdir -p "${DEPS_DIR}"

echo "[os-helpers] Building x07-proc-echo (release, locked)…"
(
  cd "${ROOT}"
  CARGO_TARGET_DIR="${TARGET_DIR}" \
    cargo build -p x07-proc-echo --release --locked
)

BIN_SRC="${TARGET_DIR}/release/x07-proc-echo"
BIN_DST="${DEPS_DIR}/x07-proc-echo"

if [[ ! -f "${BIN_SRC}" ]]; then
  echo "[os-helpers] ERROR: build output not found: ${BIN_SRC}" >&2
  exit 1
fi

# Install with stable permissions.
# Use `install` when available (Linux/macOS).
if command -v install >/dev/null 2>&1; then
  install -m 0755 "${BIN_SRC}" "${BIN_DST}"
else
  cp -f "${BIN_SRC}" "${BIN_DST}"
  chmod 0755 "${BIN_DST}"
fi

# Set a stable timestamp (portable -t format works on GNU + BSD touch).
# This helps if later you pack deps/ into deterministic archives.
touch -t 200001010000.00 "${BIN_DST}" || true

# Write a checksum file for debugging / reproducible verification.
# Prefer sha256sum, fallback to shasum -a 256.
SUM_FILE="${BIN_DST}.sha256"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "${BIN_DST}" | awk '{print $1}' > "${SUM_FILE}"
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "${BIN_DST}" | awk '{print $1}' > "${SUM_FILE}"
else
  echo "[os-helpers] NOTE: no sha256 tool found; skipping checksum." >&2
  rm -f "${SUM_FILE}" || true
fi

echo "[os-helpers] Installed: ${BIN_DST}"
if [[ -f "${SUM_FILE}" ]]; then
  echo "[os-helpers] sha256: $(cat "${SUM_FILE}")"
fi
```

---

# 3) How this fits your `run-os/proc-echo-smoke.json`

Your benchmark expects the helper at:

* `deps/x07/x07-proc-echo`

So the harness (or your manual CI script) should run:

```bash
./scripts/build_os_helpers.sh
```

…before running `benchmarks/run-os/proc-echo-smoke.json`.

---

# 4) One small integration recommendation

Add a **tiny “preflight” check** in the runner for `run-os` benches:

* if the requested executable path is under `deps/x07/`, require that file exists and is executable
* emit a clear deterministic error (“MISSING_HELPER_BINARY”) if not

This turns “forgot to build helpers” into a crisp failure mode for agents.

[1]: https://doc.rust-lang.org/cargo/commands/cargo.html?utm_source=chatgpt.com "cargo - The Cargo Book"
[2]: https://doc.rust-lang.org/cargo/reference/workspaces.html?utm_source=chatgpt.com "Workspaces - The Cargo Book"
[3]: https://doc.rust-lang.org/std/io/trait.Read.html?utm_source=chatgpt.com "Read in std::io"
[4]: https://doc.rust-lang.org/book/ch14-01-release-profiles.html?utm_source=chatgpt.com "Customizing Builds with Release Profiles"

++++
Below is a **drop‑in contract** for the “missing helper binary” preflight that is:

* **deterministic** (same repo state ⇒ same error),
* **runner‑side** (not program output),
* **integrated into suite JSON assertions** (so CI fails crisply instead of “spawn failed” ambiguity),
* and **does not leak filesystem existence** in `run-os-sandboxed` (preflight only runs in `run-os`).

I’m also keeping the design consistent with your overall “LLM-first, agentic” goal: the runner returns a **single structured error** that the agent can react to (“run scripts/build_os_helpers.sh”).

---

## 1) Runner-side error codes (normative)

Reserve a tiny numeric range for runner “preflight/infrastructure” errors.

**`RunnerErrorCode` table (v1)**

| Code (string)                  | code_u32 | Meaning                                                            | Deterministic trigger   |
| ------------------------------ | -------: | ------------------------------------------------------------------ | ----------------------- |
| `MISSING_REQUIRED_FILE`        |     9001 | A suite/task/case declared a required repo file, but it is missing | `!path.exists()`        |
| `REQUIRED_FILE_NOT_EXECUTABLE` |     9002 | Required path exists but is not executable                         | `!is_executable(path)`  |
| `REQUIRED_FILE_NOT_REGULAR`    |     9003 | Required path exists but is not a regular file                     | `metadata.is_dir()` etc |

**Why this matters**: in a threaded runtime, spawning is usually implemented with `posix_spawn`/CreateProcess-style launch, but “spawn failed” can conflate many things. A preflight “missing helper” error makes failures actionable and reproducible. (Also, it’s generally safer to avoid complex `fork` flows in multithreaded programs; after `fork()` in a multithreaded process, the child must only call async-signal-safe functions until `exec`, which is one reason people prefer `posix_spawn` for “just run a program”.) ([The Open Group][1])

---

## 2) Runner JSON output shape (exact)

Add/extend the runner report JSON to include a **top-level status** and a **runner_error** object when status is `runner_error`.

### `RunnerReport` (fragment)

```json
{
  "status": "ok | compile_error | runtime_error | runner_error",
  "stdout_b64": "…",
  "stderr": "…",
  "mem_stats": { },
  "sched_stats": { },
  "runner_error": {
    "code": "MISSING_REQUIRED_FILE",
    "code_u32": 9001,
    "missing_paths": ["deps/x07/x07-proc-echo"],
    "message": "Required helper binary missing: deps/x07/x07-proc-echo",
    "hint": "Run ./scripts/build_os_helpers.sh"
  }
}
```

### Deterministic constraints

* `runner_error.message` and `hint` are **fixed templates** (no OS-dependent error strings).
* `missing_paths` is **sorted lexicographically** before emitting.

---

## 3) How it plugs into your benchmark assertion format (exact)

You already started using the pattern:

```json
"assertions": { "requires_files": ["deps/x07/x07-proc-echo"] }
```

Make it normative:

### A) Suite/task/case format

Allow `requires_files` at **task level** (applies to all cases) and optionally at **case level** (extra per-case requirements).

**Task-level:**

```json
{
  "task_id": "proc/run_capture_echo_v1",
  "assertions": {
    "requires_files": ["deps/x07/x07-proc-echo"]
  },
  "cases": [ ... ]
}
```

**Case-level (optional additive):**

```json
{
  "name": "echo_abc",
  "requires_files": ["deps/x07/x07-proc-echo"],
  "input_b64": "...",
  "expected_b64": "..."
}
```

### B) Runner behavior (exact)

When executing a case:

1. Collect `requires_files` = `task.assertions.requires_files ∪ case.requires_files`
2. **If world != `run-os`:**

   * **Do not perform these checks** (important for `run-os-sandboxed`, to avoid side-channel “file exists” leaks).
   * Missing helper should be irrelevant there because the policy must deny before any spawn attempt.
3. **If world == `run-os`:**

   * For each required file:

     * If missing → return `status="runner_error"` with `code= MISSING_REQUIRED_FILE (9001)`
     * Else if not regular file → `REQUIRED_FILE_NOT_REGULAR (9003)`
     * Else if not executable → `REQUIRED_FILE_NOT_EXECUTABLE (9002)`
   * Do **not** run compiler/executable if preflight fails.

### C) Evaluator / suite runner semantics

* If `status="runner_error"` and the case does **not** declare that this is expected → **suite fails** (CI configuration error).
* (Optional) If you want to test the preflight itself, allow:

  * `assertions.expect_runner_error_code_u32`
  * and then treat a matching runner_error as pass for that case.

Example optional assertion:

```json
"assertions": {
  "requires_files": ["deps/x07/x07-proc-echo"],
  "expect_runner_error_code_u32": 9001
}
```

(You probably won’t use this in normal suites; it’s for a dedicated “runner self-test” suite.)

---

## 4) “Exact” Rust implementation sketch (runner side)

This is intentionally small and deterministic.

### `RunnerErrorCode` enum

```rust
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunnerErrorCode {
    MissingRequiredFile,
    RequiredFileNotExecutable,
    RequiredFileNotRegular,
}

impl RunnerErrorCode {
    pub fn code_u32(self) -> u32 {
        match self {
            RunnerErrorCode::MissingRequiredFile => 9001,
            RunnerErrorCode::RequiredFileNotExecutable => 9002,
            RunnerErrorCode::RequiredFileNotRegular => 9003,
        }
    }
}
```

### Report payload

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RunnerError {
    pub code: RunnerErrorCode,
    pub code_u32: u32,
    pub missing_paths: Vec<String>,
    pub message: String,
    pub hint: String,
}
```

### Preflight function

```rust
fn preflight_requires_files_run_os(
    requires_files: &[String],
    repo_root: &std::path::Path,
) -> Result<(), RunnerError> {
    use std::fs;

    let mut missing: Vec<String> = Vec::new();
    let mut not_exec: Vec<String> = Vec::new();
    let mut not_regular: Vec<String> = Vec::new();

    for rel in requires_files {
        let rel_path = std::path::Path::new(rel);
        let abs = repo_root.join(rel_path);

        let md = match fs::metadata(&abs) {
            Ok(m) => m,
            Err(_) => { missing.push(rel.clone()); continue; }
        };

        if !md.is_file() {
            not_regular.push(rel.clone());
            continue;
        }

        // unix exec bit check
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = md.permissions().mode();
            if (mode & 0o111) == 0 {
                not_exec.push(rel.clone());
                continue;
            }
        }

        // windows: treat "exists + is_file" as ok (you can tighten later)
        #[cfg(windows)]
        {
            // no-op for now
        }
    }

    missing.sort();
    not_exec.sort();
    not_regular.sort();

    if !missing.is_empty() {
        return Err(RunnerError {
            code: RunnerErrorCode::MissingRequiredFile,
            code_u32: RunnerErrorCode::MissingRequiredFile.code_u32(),
            missing_paths: missing.clone(),
            message: format!("Required file(s) missing: {}", missing.join(", ")),
            hint: "Run ./scripts/build_os_helpers.sh".to_string(),
        });
    }

    if !not_regular.is_empty() {
        return Err(RunnerError {
            code: RunnerErrorCode::RequiredFileNotRegular,
            code_u32: RunnerErrorCode::RequiredFileNotRegular.code_u32(),
            missing_paths: not_regular.clone(),
            message: format!("Required path(s) not regular files: {}", not_regular.join(", ")),
            hint: "Fix deps layout; expected regular executable file(s).".to_string(),
        });
    }

    if !not_exec.is_empty() {
        return Err(RunnerError {
            code: RunnerErrorCode::RequiredFileNotExecutable,
            code_u32: RunnerErrorCode::RequiredFileNotExecutable.code_u32(),
            missing_paths: not_exec.clone(),
            message: format!("Required file(s) not executable: {}", not_exec.join(", ")),
            hint: "chmod +x the helper or rerun ./scripts/build_os_helpers.sh".to_string(),
        });
    }

    Ok(())
}
```

Then in your suite runner:

* if `world == run-os` call `preflight_requires_files_run_os(...)`
* if it returns Err → emit `RunnerReport { status: "runner_error", runner_error: Some(err), … }` and stop.

---

## 5) How this interacts with your two suites

* `benchmarks/run-os/proc-echo-smoke.json`

  * **should** include `assertions.requires_files: ["deps/x07/x07-proc-echo"]`
  * missing helper ⇒ deterministic runner_error 9001 with clear hint

* `benchmarks/run-os-sandboxed/proc-policy-smoke.json`

  * **should NOT** include `requires_files`
  * so you never leak “does helper exist?” in sandboxed mode (policy denial should happen first)

---

## One small “agentic” improvement

In addition to the runner error code, include a **machine-actionable hint field**:

```json
"hint_cmd": ["./scripts/build_os_helpers.sh"]
```

Agents can execute that as a “skill” step without guessing.

---

[1]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/fork.html?utm_source=chatgpt.com "fork"
