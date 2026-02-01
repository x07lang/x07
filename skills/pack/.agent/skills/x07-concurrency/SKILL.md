---
name: x07-concurrency
description: Canonical concurrency patterns in X07 (defasync + task.* + chan.bytes.*), including sandbox constraints.
metadata:
  short-description: Async patterns
  version: 0.1.0
  kind: docs
---

# x07-concurrency

In OS worlds (`run-os*`), use subprocess spawning for multi-core parallelism. In `run-os-sandboxed`, process spawning must be enabled by policy (start from `x07 policy init --template worker-parallel` when needed). Thread-backed blocking operations are gated by `policy.threads` (for example, `threads.max_blocking = 0` disables blocking operations).

## Canonical patterns

- Use `task.scope_v1` for structured concurrency (no orphan tasks; slots/select are scope-owned).
- Use `defasync` + `task.*` + `chan.bytes.*` inside scopes to build deterministic async topologies.
- Keep `await` / `task.join.bytes` in `solve` expressions or inside `defasync` (they are not allowed inside `defn`).
- Keep OS effects (fs/net/process/time) isolated behind adapters and policies.
- Keep scheduling decisions explicit and data-driven (inputs → outputs), so behavior is reproducible.

For the built-in reference guide, use `x07 guide` and search for `defasync` / `task.` / `chan.bytes.`.
