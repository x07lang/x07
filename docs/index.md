# X07 (x07lang)

X07 is an **agent-first systems language**. It is built so coding agents can generate, modify, test, and repair software reliably, while still giving end users the things they actually care about: memory-safe defaults, speed, explicit concurrency, predictable deployment, and clear tooling.

Most languages optimize for human style flexibility. X07 optimizes for:

- **Canonical representations** so the same intent produces the same program shape.
- **LLM-oriented diagnostics** with stable error codes and machine-applicable fixes.
- **World-based capability modeling** so side effects stay explicit and reviewable.
- **Structured concurrency** so async work stays fast without turning into orphan-task chaos.

## The mental model

Think of X07 as two layers:

1. **A small, stable core** (compiler + runtime substrate)
2. **A growing ecosystem of libraries** (stdlib + external packages)

Programs are stored and exchanged in a **structured AST format** (x07AST JSON). Humans usually edit a pretty form (or generated templates), while agents operate on the structured form directly.

## Start here

- [Why X07 (trust story + evidence pack)](why-x07.md)
- [Formal verification & certification](toolchain/formal-verification.md)
- [Agent initial prompt (copy/paste)](getting-started/agent-initial-prompt.md)
- [Agent quickstart (learn X07 from scratch)](getting-started/agent-quickstart.md)
- [Install X07](getting-started/install.md)
- [Your first project](getting-started/first-project.md)
- [How “worlds” work](worlds/index.md)
- [The agent workflow](getting-started/agent-workflow.md)

## Ecosystem at a glance

The `x07` repo is the entrypoint, but the public ecosystem is intentionally split into focused repos:

- **MCP kit + official MCP server**: [`x07-mcp`](https://github.com/x07lang/x07-mcp) gives you templates for building MCP servers in X07 and ships the official `io.x07/x07lang-mcp` server for agent runtimes. Start with [MCP kit](toolchain/mcp-kit.md).
- **WASM, web UI, and device apps**: [`x07-wasm-backend`](https://github.com/x07lang/x07-wasm-backend), [`x07-web-ui`](https://github.com/x07lang/x07-web-ui), [`x07-device-host`](https://github.com/x07lang/x07-device-host), and [`x07-wasi`](https://github.com/x07lang/x07-wasi) cover browser UI, WASM services, packaged desktop/mobile apps, and WASI-facing contracts. Start with [WASM tooling](toolchain/wasm.md).
- **Lifecycle platform**: [`x07-platform`](https://github.com/x07lang/x07-platform) and [`x07-platform-contracts`](https://github.com/x07lang/x07-platform-contracts) cover sealed artifacts, deploy plans, incidents, regressions, and device release control. Start with [Platform for agents](agent/platform.md).
- **Packages and docs**: [`x07-registry`](https://github.com/x07lang/x07-registry) powers package publishing, [`x07-registry-web`](https://github.com/x07lang/x07-registry-web) serves [x07.io](https://x07.io), and [`x07-website`](https://github.com/x07lang/x07-website) serves [x07lang.org](https://x07lang.org).

## What makes X07 different?

### 1) One canonical way (agents don’t get “choice paralysis”)

Instead of 5 equivalent ways to read a file, split strings, build output, or handle errors, X07 aims for:

- one canonical API surface per capability,
- one canonical bytes encoding per data interchange,
- one canonical failure model per module (stable error code space).

This reduces “LLM confusion” and makes programs and patches more uniform.

### 2) Policy-gated OS execution

When you need real OS resources (real network, real disk, real time), use `run-os` or `run-os-sandboxed`.

`run-os-sandboxed` is governed by explicit policy files; X07 defaults to a VM boundary on supported platforms, but it is still not a hardened sandbox if you mount secrets or enable networking.

### 3) Production worlds are opt-in

When you need real OS resources (real network, real disk, real time), you switch to OS-backed worlds. Those worlds are **never used for deterministic evaluation**, and are governed by explicit policies.

## Why this matters in practice

- **For end users**: you get one language and one ecosystem for CLIs, MCP servers, web UI, packaged device apps, WASM services, package publishing, and lifecycle operations.
- **For teams**: the same contracts show up in docs, CLI reports, schema files, CI checks, and operational tooling.
- **For coding agents**: the language removes many of the ambiguities that make autonomous edits hard to trust in mainstream languages.

## Documentation map (human)

- Getting started:
  - [Agent initial prompt](getting-started/agent-initial-prompt.md)
  - [Agent quickstart](getting-started/agent-quickstart.md)
  - [Install](getting-started/install.md)
  - [First project](getting-started/first-project.md)
  - [Agent workflow](getting-started/agent-workflow.md)
- Language:
  - [Overview](language/overview.md)
  - [Syntax & x07AST](language/syntax-x07ast.md)
  - [Generics](language/generics.md)
  - [Types & memory model](language/types-memory.md)
  - [Concurrency & multiprocessing](language/concurrency-multiprocessing.md)
- Worlds:
  - [Worlds overview](worlds/index.md)
  - [OS worlds & sandbox policy](worlds/os-worlds.md)
- Toolchain:
  - [CLI](toolchain/cli.md)
  - [Generation pack](genpack/index.md)
  - [Benchmarks](toolchain/benchmarks.md)
  - [Running programs](toolchain/running-programs.md)
  - [Repair loop](toolchain/repair-loop.md)
  - [Diagnostics & repair](toolchain/diagnostics-repair.md)
  - [Testing](toolchain/testing.md)
  - [Property-based testing](toolchain/pbt.md)
  - [Formal verification & certification](toolchain/formal-verification.md)
  - [Review & trust artifacts](toolchain/review-trust.md)
- Guides:
  - [Building web services](guides/web-apps.md)
  - [Databases](guides/databases.md)
  - [Messaging and event streaming](guides/messaging.md)
  - [Provider-agnostic service runtimes and bindings](guides/provider-agnostic-services.md)
  - [Scaling, retry, and idempotency for services](guides/scaling-retry-idempotency.md)
- Packages:
  - [Workspaces, packages, lockfiles](packages/index.md)
  - [Publishing](packages/publishing.md)
- Libraries:
  - [Stdlib](libraries/stdlib.md)
  - [External packages](libraries/external.md)
  - [Design patterns for agent code](libraries/agentic-design.md)
