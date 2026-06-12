# X07 (x07lang)

X07 is the **deterministic, certifiable execution substrate for agent-written software**. As code generation gets cheap, trust becomes the bottleneck — so X07 makes generated code runnable: deterministic solve worlds with record/replay, explicit resource budgets, capability sandboxing, structured diagnostics with quickfix coverage, spec-first testing (XTAL), and proof-backed certification. It compiles via C to fast native binaries, with a WASM target for portable sandboxed execution.

Most languages optimize for human style flexibility. X07 optimizes for:

- **Canonical representations** so the same intent produces the same program shape.
- **Machine-readable diagnostics** with stable error codes, did-you-mean suggestions, and machine-applicable fixes.
- **World-based capability modeling** so side effects stay explicit and reviewable.
- **Evidence over review-by-reading**: budgets, replayable runs, trust reports, and certificates.

## The mental model

Think of X07 as two layers:

1. **A small, stable core** (compiler + runtime substrate)
2. **A growing ecosystem of libraries** (stdlib + external packages)

Programs are stored and exchanged in a **structured AST format** (x07AST JSON). Agents and humans can operate on the structured form directly, or read and author through [x07text](language/x07text.md) — a lossless text projection (`x07 ast to-text` / `from-text`) that always converts back to canonical JSON.

Direct authoring by agents is a supported surface and an explicitly gated bet: a comparative eval (`labs/agent-eval/` in the toolchain repo) with a predeclared decision rule determines whether deeper language-surface investment proceeds. See [Why X07](why-x07.md) for the honest status.

## Start here

### If you are a human evaluating X07

- [Why X07 (trust story + evidence pack)](why-x07.md)
- [Install X07](getting-started/install.md)
- [Your first project](getting-started/first-project.md)
- [Language overview](language/overview.md)
- [How “worlds” work](worlds/index.md)
- [MCP kit](toolchain/mcp-kit.md)
- [WASM tooling](toolchain/wasm.md)

### If you are driving through an agent

- [Agent initial prompt (copy/paste)](getting-started/agent-initial-prompt.md)
- [Agent quickstart (learn X07 from scratch)](getting-started/agent-quickstart.md)
- [The agent workflow](getting-started/agent-workflow.md)
- [Agent contracts](agent/contract.md)
- [Formal verification & certification](toolchain/formal-verification.md)

## Ecosystem at a glance (2026-06 scope)

The `x07` repo is the entrypoint. The active ecosystem was deliberately narrowed in 2026-06 to concentrate on the substrate bet (see the [roadmap](roadmap.md)):

- **Core toolchain**: [`x07`](https://github.com/x07lang/x07) — language, CLI, compiler, stdlib, verification/certification tooling, and the canonical docs source.
- **MCP kit + official MCP server**: [`x07-mcp`](https://github.com/x07lang/x07-mcp) gives you templates for building MCP servers in X07 and ships the official `io.x07/x07lang-mcp` server for agent runtimes. Start with [MCP kit](toolchain/mcp-kit.md).
- **WASM**: [`x07-wasm-backend`](https://github.com/x07lang/x07-wasm-backend) covers WASM modules and WASI components for portable sandboxed execution. Start with [WASM tooling](toolchain/wasm.md).
- **Packages and docs**: [`x07-registry`](https://github.com/x07lang/x07-registry) powers package publishing at [x07.io](https://x07.io), and [`x07-website`](https://github.com/x07lang/x07-website) serves [x07lang.org](https://x07lang.org).
- **MCP server verification**: [`hardproof`](https://github.com/x07lang/hardproof) is the standalone verifier CLI used by the [MCP quality](toolchain/mcp-quality.md) flow.

Former ecosystem surfaces — studio/IDE shells, web UI contracts, device hosts, and the lifecycle platform (`x07-studio`, `x07-forge`, `x07-crewops`, `x07-tactics`, `x07-device-host`, `x07-web-ui`, `x07-registry-web`, `x07-sentinel-reference-stack`, `x07-platform` + contracts/cloud) — are in maintenance mode under the 2026-06 scope cut: security and compatibility fixes only. See the [roadmap](roadmap.md) for the rationale and reactivation conditions.

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

- **For teams running agent-generated code**: deterministic runs, budgets, sandbox policies, and certificates turn “should we run this?” from a judgment call into a checkable artifact.
- **For end users**: one language and one ecosystem for CLIs, MCP servers, WASM services, and package publishing.
- **For coding agents**: the language removes many of the ambiguities that make autonomous edits hard to trust in mainstream languages.

## Documentation map (human)

- Getting started:
  - [Install](getting-started/install.md)
  - [First project](getting-started/first-project.md)
  - [Agent initial prompt](getting-started/agent-initial-prompt.md)
  - [Agent quickstart](getting-started/agent-quickstart.md)
  - [Agent workflow](getting-started/agent-workflow.md)
- Language:
  - [Overview](language/overview.md)
  - [Syntax & x07AST](language/syntax-x07ast.md)
  - [x07text projection](language/x07text.md)
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
  - [Migrating services to logical bindings](guides/service-binding-migration.md)
  - [Scaling, retry, and idempotency for services](guides/scaling-retry-idempotency.md)
- Packages:
  - [Workspaces, packages, lockfiles](packages/index.md)
  - [Publishing](packages/publishing.md)
- Libraries:
  - [Stdlib](libraries/stdlib.md)
  - [External packages](libraries/external.md)
  - [Design patterns for agent code](libraries/agentic-design.md)
