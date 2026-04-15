<picture>
  <source media="(prefers-color-scheme: dark)" srcset="branding/logo-full-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="branding/logo-full-light.png">
  <img alt="x07lang" src="branding/logo-full-light.png" height="80">
</picture>

# X07

The programming language designed for AI agents.

X07 is a language and toolchain for teams that want software generation, repair, review, and delivery to stay deterministic. The canonical source format is machine-readable `x07AST` JSON, diagnostics are structured, capabilities are explicit, and the toolchain is built to work cleanly with both coding agents and human reviewers.

> X07 is under active development. Tooling and APIs still move.

**Start here:** [Agent Quickstart](https://x07lang.org/docs/getting-started/agent-quickstart) · [Installer](https://x07lang.org/docs/getting-started/installer) · [Docs](https://x07lang.org) · [Roadmap](https://x07lang.org/docs/roadmap) · [Governance](https://x07lang.org/docs/governance) · [FAQ](https://x07lang.org/docs/faq) · [Package Registry](https://x07.io) · [Releases](https://github.com/x07lang/x07/releases) · [Support](SUPPORT.md) · [Discord](https://discord.gg/59xuEuPN47)

## What This Repo Is

This is the core X07 repo. It contains:

- the language CLI and compiler
- the standard library and schemas
- the package manager, test harness, and repair loop
- public verification and certification tooling
- the canonical docs source for `x07lang.org`

If you want to understand X07 itself, install the toolchain, or build software in X07, this is the repo to start with.

## Why X07

- **Machine-first source.** Canonical source files are `*.x07.json`, so agents patch structure instead of guessing at text edits.
- **Deterministic repair loop.** The toolchain emits stable diagnostics, quickfixes, and machine-readable reports.
- **Explicit capability model.** Side effects are opt-in through deterministic solve worlds or explicit OS worlds and sandbox policies.
- **One official path per task.** X07 avoids multiple equally-valid patterns that make generated code inconsistent and hard to review.
- **Native performance.** X07 compiles to optimized native code and is built for real workloads, not just toy examples. See [`x07lang/x07-perf-compare`](https://github.com/x07lang/x07-perf-compare) for benchmark snapshots.
- **Structured concurrency.** Task scopes and explicit budgets make async work easier to reason about for both humans and agents.

## Start Here

### Install the toolchain

macOS / Linux:

```bash
curl -fsSL https://x07lang.org/install.sh | sh -s -- --yes --channel stable
```

Windows is supported through WSL2. Full installer docs live at [x07lang.org/docs/getting-started/installer](https://x07lang.org/docs/getting-started/installer).

### Run your first program

```bash
mkdir myapp
cd myapp
x07 init
x07 run
```

For offline-first workflows, forbid network access during dependency hydration with `x07 run --offline` or `X07_OFFLINE=1` (and use `x07 pkg tree` to inspect the resolved lockfile closure).

`x07 run` is the canonical entrypoint for the agent loop. When you need explicit control over individual steps, the core commands are:

```bash
x07 fmt --input program.x07.json --write
x07 lint --input program.x07.json
x07 fix --input program.x07.json --write
x07 check --project x07.json --ast
x07 check --project x07.json
x07 ast apply-patch --in program.x07.json --patch patch.json --out program.x07.json --validate
```

### Spec-first workflows

XTAL (Phase A) and the `x07 gen` gate support spec-first and generated-artifact workflows. These flows assume you already have spec inputs and checked-in generated artifacts.

Docs:

- `docs/toolchain/xtal-phase-a.md`
- `docs/toolchain/generated-artifacts.md`

Core gates:

```bash
x07 xtal verify
x07 gen verify --index arch/gen/index.x07gen.json
```

### Use X07 with a coding agent

Start with the [Agent Quickstart](https://x07lang.org/docs/getting-started/agent-quickstart). If your runtime supports MCP, install the official `io.x07/x07lang-mcp` server from [`x07lang/x07-mcp`](https://github.com/x07lang/x07-mcp). That server exposes token-efficient editing, package, WASM, device, app, and platform tooling through structured contracts instead of shell scraping.

### Build beyond local CLIs

The core toolchain stays in this repo. The broader X07 stack extends into focused repos:

- [`x07lang/x07-mcp`](https://github.com/x07lang/x07-mcp) for MCP servers and the official X07 MCP server
- [`x07lang/x07-wasm-backend`](https://github.com/x07lang/x07-wasm-backend) for WASM modules, browser delivery, and app bundles
- [`x07lang/x07-web-ui`](https://github.com/x07lang/x07-web-ui) for reducer-style web UI contracts and browser host surfaces
- [`x07lang/x07-device-host`](https://github.com/x07lang/x07-device-host) for desktop and mobile WebView hosting
- [`x07lang/x07-platform`](https://github.com/x07lang/x07-platform) for workload delivery, release review, incidents, and lifecycle control

## Formal Verification And Certification

X07 exposes formal verification as a public toolchain surface, not a private experiment. The main commands are:

- `x07 verify` for coverage and proof generation
- `x07 prove check` for replaying proof objects against current source and obligations
- `x07 trust certify` for binding proof, test, boundary, capsule, and runtime evidence into a certificate

Start with [`docs/toolchain/formal-verification.md`](docs/toolchain/formal-verification.md) for the current proof model, constraints, and example flows.

If you want a first project in this area, use one of the built-in templates:

- `x07 init --template verified-core-pure`
- `x07 init --template trusted-sandbox-program`
- `x07 init --template trusted-network-service`
- `x07 init --template certified-capsule`
- `x07 init --template certified-network-capsule`

## Ecosystem Overview

The public X07 ecosystem is split into focused repos with clear boundaries:

- [`x07lang/x07`](https://github.com/x07lang/x07): language, CLI, compiler, stdlib, schemas, and canonical docs source
- [`x07lang/x07-rfcs`](https://github.com/x07lang/x07-rfcs): RFC process and design records for language, compatibility, release, and governance changes
- [`x07lang/x07-mcp`](https://github.com/x07lang/x07-mcp): MCP kit, templates, reference servers, and the official `io.x07/x07lang-mcp` server
- [`x07lang/x07-wasm-backend`](https://github.com/x07lang/x07-wasm-backend): WASM toolchain and app packaging
- [`x07lang/x07-web-ui`](https://github.com/x07lang/x07-web-ui): browser UI contracts and host surfaces
- [`x07lang/x07-device-host`](https://github.com/x07lang/x07-device-host): desktop and mobile WebView host
- [`x07lang/x07-platform`](https://github.com/x07lang/x07-platform): public runtime and lifecycle control plane
- [`x07lang/x07-registry`](https://github.com/x07lang/x07-registry): package registry backend
- [`x07lang/x07-registry-web`](https://github.com/x07lang/x07-registry-web): package registry UI at [x07.io](https://x07.io)
- [`x07lang/x07-website`](https://github.com/x07lang/x07-website): public docs site at [x07lang.org](https://x07lang.org)

Project governance is documented in [`GOVERNANCE.md`](GOVERNANCE.md) and
[`OWNERS.md`](OWNERS.md). The public 12-month plan lives in
[`docs/roadmap.md`](docs/roadmap.md).

## Repository Layout

```text
x07/
├── docs/           # Canonical docs source for x07lang.org
├── crates/         # Rust workspace
│   ├── x07c/           # Compiler (X07 -> C)
│   ├── x07-host-runner # Deterministic native runner
│   └── x07-os-runner   # OS-world runner backend
├── stdlib/         # Standard library
├── tests/          # Toolchain fixtures and harness suites
├── labs/           # Optional benchmarks, perf, fuzz, and evaluation tooling
└── scripts/        # Tooling and CI helpers
```

## Build From Source

Prerequisites: Rust toolchain, C compiler (`cc`), `clang`, and Python 3.

Full gate:

```bash
./scripts/ci/check_all.sh
```

Common individual commands:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -p x07 -- test --manifest tests/tests.json
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
