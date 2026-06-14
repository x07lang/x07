<picture>
  <source media="(prefers-color-scheme: dark)" srcset="branding/logo-full-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="branding/logo-full-light.png">
  <img alt="x07lang" src="branding/logo-full-light.png" height="80">
</picture>

# X07

The deterministic, certifiable execution substrate for agent-written software.

As code generation gets cheap, trust becomes the bottleneck: someone still has to decide whether generated code is safe to run. X07 is a language and toolchain built around that decision. Programs execute in deterministic solve worlds with record/replay, under explicit resource budgets and capability sandboxing. Diagnostics are structured, with quickfix coverage enforced over a 646-code catalog. Testing is spec-first (XTAL), and `x07 verify` / `x07 trust certify` bind proof, test, and runtime evidence into certificates a reviewer can check instead of re-reading source. The C backend gives native speed, fast compiles, and small binaries; a WASM target covers portable sandboxed execution.

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

The trust surface comes first:

- **Deterministic execution.** Correctness loops run in `solve-*` worlds; real OS interactions can be recorded into cassettes and replayed deterministically (`std.rr`).
- **Resource budgets.** Fuel, memory, and output budgets are explicit (`budget.scope_v1`, arch-driven profiles), so a small generated change cannot silently blow up costs.
- **Capability sandboxing.** Side effects are opt-in through explicit OS worlds and policy files; `run-os-sandboxed` defaults to a VM boundary on supported platforms.
- **Structured diagnostics.** A 646-code diagnostic catalog with quickfix coverage enforced as a CI gate, machine-readable failure reports, and did-you-mean suggestions on unknown symbols.
- **Spec-first testing.** XTAL drives verify/repair/certify loops from pinned specs, and `x07 gen verify` gates generated artifacts.
- **Proof-backed certification.** `x07 verify` produces proof and coverage artifacts; `x07 trust certify` binds proof, test, boundary, and runtime evidence into a reviewable certificate.
- **Native performance.** X07 compiles via C to optimized native code with fast compiles and small binaries; the WASM target covers portable sandboxed execution. See [`x07lang/x07-perf-compare`](https://github.com/x07lang/x07-perf-compare) for benchmark snapshots.

On authoring: agents and humans can write X07 directly, and the 2026-06 toolchain made that easier — the [x07text](docs/language/x07text.md) lossless text projection (`x07 ast to-text|from-text`, RFC 0001), behavioral summaries in `x07 doc`, and did-you-mean diagnostics. Direct authoring is still an explicitly gated bet, not a settled claim: the comparative eval in [`labs/agent-eval/`](labs/agent-eval/) (pilot complete; scaled protocol and predeclared decision rule in its RUNBOOK) decides whether deeper language investment (RFC 0002: records, enums + match, string, f64) proceeds. The results will be published either way.

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

XTAL and the `x07 gen` gate support spec-first and generated-artifact workflows. These flows assume you already have spec inputs and checked-in generated artifacts.

Docs:

- `docs/toolchain/xtal.md`
- `docs/toolchain/generated-artifacts.md`

Canonical gates:

```bash
x07 xtal dev
x07 xtal certify
x07 xtal ingest --input target/xtal/violations/<id>
x07 gen verify --index arch/gen/index.x07gen.json
```

Building blocks (advanced; used when you need to isolate a step):

```bash
x07 xtal verify
x07 xtal repair
x07 xtal improve
x07 xtal tasks run --input target/xtal/violations/<id>/violation.json
```

### Use X07 with a coding agent

Start with the [Agent Quickstart](https://x07lang.org/docs/getting-started/agent-quickstart). The authoring surface is agent-oriented end to end:

- canonical source is `x07AST` JSON, patched structurally (JSON Patch / quickfixes)
- [x07text](docs/language/x07text.md) is a lossless text projection for reading and authoring (`x07 ast to-text` / `x07 ast from-text`; conversion back emits canonical bytes)
- `x07 doc` returns behavioral summaries for stdlib exports, with fuzzy lookup
- unknown-symbol diagnostics carry did-you-mean suggestions, and `x07 run` failure reports embed structured diagnostics

If your runtime supports MCP, install the official `io.x07/x07lang-mcp` server from [`x07lang/x07-mcp`](https://github.com/x07lang/x07-mcp). It exposes token-efficient editing, package, and WASM tooling through structured contracts instead of shell scraping.

### Beyond this repo

The core toolchain stays in this repo. The active companion repos (2026-06 scope) are:

- [`x07lang/x07-mcp`](https://github.com/x07lang/x07-mcp) for the MCP kit and the official X07 MCP server
- [`x07lang/x07-registry`](https://github.com/x07lang/x07-registry) for the package registry backend
- [`x07lang/x07-wasm-backend`](https://github.com/x07lang/x07-wasm-backend) for WASM modules and WASI components
- [`x07lang/hardproof`](https://github.com/x07lang/hardproof) for standalone MCP server verification

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

The ecosystem was narrowed in 2026-06 to concentrate on the substrate bet (see [`docs/roadmap.md`](docs/roadmap.md)).

Active repos:

- [`x07lang/x07`](https://github.com/x07lang/x07): language, CLI, compiler, stdlib, schemas, verification/certification tooling, and canonical docs source
- [`x07lang/x07-mcp`](https://github.com/x07lang/x07-mcp): MCP kit, templates, reference servers, and the official `io.x07/x07lang-mcp` server
- [`x07lang/x07-registry`](https://github.com/x07lang/x07-registry): package registry backend
- [`x07lang/x07-wasm-backend`](https://github.com/x07lang/x07-wasm-backend): WASM toolchain (modules and WASI components)
- [`x07lang/hardproof`](https://github.com/x07lang/hardproof): standalone verifier CLI for MCP server quality and trust checks

Supporting repos:

- [`x07lang/x07-rfcs`](https://github.com/x07lang/x07-rfcs): RFC process and design records (RFC 0001 x07text, RFC 0002 expressiveness floor)
- [`x07lang/x07-website`](https://github.com/x07lang/x07-website): public docs site at [x07lang.org](https://x07lang.org)
- [`x07lang/x07-perf-compare`](https://github.com/x07lang/x07-perf-compare): reproducible cross-language benchmark snapshots

Maintenance mode (2026-06 scope cut; security and compatibility fixes only): `x07-studio`, `x07-forge`, `x07-crewops`, `x07-tactics`, `x07-device-host`, `x07-web-ui`, `x07-sentinel-reference-stack`, and the platform repos (`x07-platform`, `x07-platform-contracts`, `x07-platform-cloud`). The rationale and the conditions for reactivating them are in the roadmap.

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
├── labs/           # Optional benchmarks, perf, fuzz, and eval tooling (incl. labs/agent-eval)
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
