<picture>
  <source media="(prefers-color-scheme: dark)" srcset="branding/logo-full-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="branding/logo-full-light.png">
  <img alt="x07lang" src="branding/logo-full-light.png" height="80">
</picture>

# The Language Designed for AI Agents

X07 is a programming language built from the ground up for **100% agentic coding**. Unlike traditional languages where humans write code and AI assists, X07 flips the model: AI agents generate, modify, test, and repair programs reliably—without needing a human to "massage" code.

> **X07 is under active development. APIs and tooling may change.**

**[Documentation](https://x07lang.org)** · [FAQ](https://x07lang.org/docs/faq) · [Support](SUPPORT.md) · [Discord](https://discord.gg/59xuEuPN47) · [Email](mailto:support@x07lang.org) · [Releases](https://github.com/x07lang/x07/releases)

---

## Why X07?

X07 is the core repo and entrypoint for the whole ecosystem. It is the place to start if you want to understand the language, install the toolchain, or build agent-friendly software that can move from local development to browser, device, MCP, package registry, and lifecycle platform workflows without switching languages.

Autonomous agents struggle with mainstream languages because of **multiple equivalent patterns**, **ambiguous diagnostics**, **nondeterministic test environments**, and **text-based patching on fragile syntax**. X07 makes these constraints first-class concerns:

### Machine-First Source Format

The canonical source is **x07AST JSON** (`*.x07.json`), not text files. Patches are structural ([RFC 6902 JSON Patch](https://datatracker.ietf.org/doc/html/rfc6902)), so agents apply changes mechanically—no parsing ambiguity, no whitespace surprises.

### Memory-Safe Defaults

Normal X07 programs work with checked values, bytes, views, and structured data rather than raw pointer arithmetic. Unsafe pointer surfaces are explicit, limited, and outside the standard agent workflow, which keeps day-to-day code generation safer and easier to review.

### Deterministic Execution

X07’s tooling is designed for reproducible, machine-driven repair loops: stable error codes, structured reports, and explicit resource budgets.

### Single Canonical Approach

One way to do each thing. No "should I use a for loop or map?" decisions. This eliminates the pattern confusion that plagues LLM-generated code in flexible languages.

### Machine-Readable Diagnostics

Errors are **structured identifiers with actionable fixes** designed for LLM consumption—not cryptic messages intended for humans to interpret.

### Explicit Capability Worlds

Side effects are opt-in. Programs run in deterministic solve worlds or OS worlds, and sandboxing is explicit and policy-driven.

### Structured Concurrency

X07 gives agents one clear concurrency model: structured task scopes, explicit budgets, and deterministic fixture-world execution. That keeps async code fast enough for real workloads while staying reviewable and testable.

### High Performance

X07 compiles to optimized native code with competitive runtime performance. In the direct-binary benchmarks published in `x07lang/x07-perf-compare` (v0.0.3 snapshot), X07 matched or exceeded C/Rust execution times on the included workloads while compiling ~3x faster than C and ~6-7x faster than Rust. Binary sizes in that snapshot were comparable to C (~34 KiB).

See [`x07lang/x07-perf-compare`](https://github.com/x07lang/x07-perf-compare) for detailed benchmarks.

## Why End Users Care

- **Reliable memory model**: the default language surface avoids the raw-memory pitfalls that make systems code hard to trust.
- **Speed**: X07 targets native execution and competitive runtime performance.
- **Concurrency**: structured concurrency gives you parallel work without orphan-task chaos.
- **Predictable deployment story**: the same ecosystem covers local CLIs, MCP servers, web UI, device apps, WASM backends, package publishing, and lifecycle operations.
- **Simple mental model**: fewer equivalent ways to do the same thing means fewer hidden surprises in code reviews and maintenance.

## Why Coding Agents Work Reliably With X07

- **Canonical source and patching**: x07AST JSON plus JSON Patch avoids fragile text diffs.
- **Stable diagnostics**: agents can key off structured codes and quickfixes instead of guessing from prose.
- **Deterministic worlds and replay**: repair loops can stay reproducible until you explicitly opt into OS effects.
- **Capability boundaries**: tools, runners, and hosts all use explicit contracts for what code is allowed to do.
- **Single official path per capability**: fewer choices means less hallucinated glue code.

## Ecosystem Overview

X07 is not just a compiler. The public ecosystem is organized into focused repos with one consistent story:

- [`x07lang/x07`](https://github.com/x07lang/x07): the core language, compiler, CLI, stdlib sources, schemas, and canonical docs.
- [`x07lang/x07-mcp`](https://github.com/x07lang/x07-mcp): the MCP kit and the official `io.x07/x07lang-mcp` server for coding and operating X07 systems from agent runtimes.
- [`x07lang/x07-wasm-backend`](https://github.com/x07lang/x07-wasm-backend): the WASM toolchain for pure modules, full-stack app bundles, browser UI, and device packaging.
- [`x07lang/x07-web-ui`](https://github.com/x07lang/x07-web-ui): the official reducer-style web UI contracts and browser host.
- [`x07lang/x07-device-host`](https://github.com/x07lang/x07-device-host): the desktop and mobile WebView host that runs the same X07 UI reducer across platforms.
- [`x07lang/x07-wasi`](https://github.com/x07lang/x07-wasi): canonical `std.wasi.*` packages for WASI-facing X07 programs.
- [`x07lang/x07-platform`](https://github.com/x07lang/x07-platform): the lifecycle platform for sealed artifacts, deploy plans, incidents, regressions, and device releases.
- [`x07lang/x07-platform-contracts`](https://github.com/x07lang/x07-platform-contracts): the public `lp.*` contracts shared by the platform CLI, UI, APIs, and MCP tools.
- [`x07lang/x07-registry`](https://github.com/x07lang/x07-registry): the package registry backend.
- [`x07lang/x07-registry-web`](https://github.com/x07lang/x07-registry-web): the public package registry UI at [x07.io](https://x07.io).
- [`x07lang/x07-website`](https://github.com/x07lang/x07-website): the documentation website at [x07lang.org](https://x07lang.org).

For end users, that means one language with an official path for package distribution, WASM delivery, browser and device UI, MCP-based agent tooling, and production lifecycle control.

---

## Quick Start

### Install

The recommended installer is `x07up` (toolchain manager). It installs the toolchain under `~/.x07/`, configures `~/.x07/bin/` shims, and manages optional runtime components such as `x07-wasm` and `x07-device-host-desktop`.

macOS / Linux:

```bash
curl -fsSL https://x07lang.org/install.sh | sh -s -- --yes --channel stable
```

Windows (WSL2):

X07 is supported on Windows via WSL2 (Ubuntu recommended). In your WSL2 shell, run the macOS / Linux install command above.

Docs: https://x07lang.org/docs/getting-started/installer/

Advanced: toolchain archives are also available under https://github.com/x07lang/x07/releases

Add the WASM and device-host components when you need them:

```bash
x07up update
x07up component add wasm
x07up component add device-host
x07 wasm doctor --json
```

### Run a Program

```bash
mkdir myapp
cd myapp
x07 init
x07 run
```

### Agent Tooling

For the canonical agent loop, start with `x07 run` (auto-repair by default). Use the commands below when you need explicit control over individual repair steps.

```bash
x07 fmt --input program.x07.json --write
x07 lint --input program.x07.json
x07 fix --input program.x07.json --write
x07 ast apply-patch --in program.x07.json --patch patch.json --out program.x07.json --validate
```

### Official MCP server (for agent runtimes)

If your coding agent runtime supports MCP (Model Context Protocol), install and use the official X07 MCP server: `io.x07/x07lang-mcp`. It exposes token-efficient core editing tools plus capability-gated ecosystem tools such as `x07.ecosystem.status_v1`, `x07.pkg.provides_v1`, `x07.wasm.core_v1`, `x07.web_ui.exec_v1`, `x07.device.exec_v1`, `x07.app.exec_v1`, `lp.query_v1`, safe lifecycle control via `lp.control_v1`, and safe patch application for x07AST JSON.

For release-, incident-, and regression-aware clients, keep app-specific candidate/workspace state locally and consume operational truth from the official structured `lp.*` result contracts exposed through `x07lang-mcp`. The x07-side boundary is documented in [`docs/toolchain/mcp-kit.md`](docs/toolchain/mcp-kit.md) and [`docs/agent/platform.md`](docs/agent/platform.md).

For MCP server projects, prefer `x07 init --template mcp-server-http-tasks` when you want the task-aware HTTP/SSE scaffold; `x07` delegates that template to `x07-mcp` rather than re-implementing MCP kit logic locally.

Start here: https://x07lang.org/docs/getting-started/agent-quickstart/ (includes MCP setup).

---

## OS Worlds

| World | Description |
|-------|-------------|
| `run-os` | Real OS access (non-deterministic) |
| `run-os-sandboxed` | Policy-restricted OS access |

---

## Repository Layout

```
x07/
├── docs/           # End-user documentation (x07lang.org source)
├── crates/         # Rust workspace
│   ├── x07c/           # Compiler (X07 → C)
│   ├── x07-host-runner # Deterministic native runner
│   └── x07-os-runner   # OS-world runner backend (canonical entrypoint: `x07 run`)
├── stdlib/         # Standard library
├── ci/             # Release-blocking fixtures + suites
├── labs/           # Optional benchmarks, perf, fuzz, eval tooling
└── scripts/        # Tooling and CI scripts
```

## Key Links

- Docs: [x07lang.org](https://x07lang.org)
- Package registry: [x07.io](https://x07.io)
- MCP kit: [x07lang/x07-mcp](https://github.com/x07lang/x07-mcp)
- WASM tooling: [x07lang/x07-wasm-backend](https://github.com/x07lang/x07-wasm-backend)
- Lifecycle platform: [x07lang/x07-platform](https://github.com/x07lang/x07-platform)
- Official docs site repo: [x07lang/x07-website](https://github.com/x07lang/x07-website)
 
 ---

## Build from Source

Prerequisites: Rust toolchain, C compiler (`cc`), `clang`, Python 3

```bash
# Full CI check
./scripts/ci/check_all.sh

# Individual checks
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings

# Run test harness
cargo run -p x07 -- test --manifest tests/tests.json
```

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
