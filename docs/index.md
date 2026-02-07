# X07 (x07lang)

X07 is an **agent-first systems language**: it’s designed so autonomous coding agents can generate, modify, test, and repair programs reliably—without needing a human to “massage” code.

Most languages optimize for human ergonomics (expressiveness, many equivalent ways to write the same thing). X07 optimizes for:

- **Canonical representations** (bytes encodings, module layouts, request formats), so “the same intent” produces “the same shape”.
- **LLM-oriented diagnostics** (structured, code-stable error IDs + machine-applicable fixes).
- **World-based capability modeling** (especially for OS access), so agents can run safely with explicit intent.

## The mental model

Think of X07 as two layers:

1. **A small, stable core** (compiler + runtime substrate)
2. **A growing ecosystem of libraries** (stdlib + external packages)

Programs are stored and exchanged in a **structured AST format** (x07AST JSON). Humans usually edit a pretty form (or generated templates), while agents operate on the structured form directly.

## Start here

- [Agent quickstart (learn X07 from scratch)](getting-started/agent-quickstart.md)
- [Install X07](getting-started/install.md)
- [Your first project](getting-started/first-project.md)
- [How “worlds” work](worlds/index.md)
- [The agent workflow](getting-started/agent-workflow.md)

## What makes X07 different?

### 1) One canonical way (agents don’t get “choice paralysis”)

Instead of 5 equivalent ways to read a file, split strings, build output, or handle errors, X07 aims for:

- one canonical API surface per capability,
- one canonical bytes encoding per data interchange,
- one canonical failure model per module (stable error code space).

This reduces “LLM confusion” and makes programs and patches more uniform.

### 2) Policy-gated OS execution

When you need real OS resources (real network, real disk, real time), use `run-os` or `run-os-sandboxed`.

`run-os-sandboxed` is governed by explicit policy files (it is not a hardened sandbox).

### 3) Production worlds are opt-in

When you need real OS resources (real network, real disk, real time), you switch to OS-backed worlds. Those worlds are **never used for deterministic evaluation**, and are governed by explicit policies.

## Documentation map (human)

- Getting started:
  - [Agent quickstart](getting-started/agent-quickstart.md)
  - [Install](getting-started/install.md)
  - [First project](getting-started/first-project.md)
  - [Agent workflow](getting-started/agent-workflow.md)
- Language:
  - [Overview](language/overview.md)
  - [Syntax & x07AST](language/syntax-x07ast.md)
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
- Packages:
  - [Workspaces, packages, lockfiles](packages/index.md)
  - [Publishing](packages/publishing.md)
- Libraries:
  - [Stdlib](libraries/stdlib.md)
  - [External packages](libraries/external.md)
  - [Design patterns for agent code](libraries/agentic-design.md)
