<picture>
  <source media="(prefers-color-scheme: dark)" srcset="branding/logo-full-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="branding/logo-full-light.png">
  <img alt="x07lang" src="branding/logo-full-light.png" height="80">
</picture>

# The Language Designed for AI Agents

X07 is a programming language built from the ground up for **100% agentic coding**. Unlike traditional languages where humans write code and AI assists, X07 flips the model: AI agents generate, modify, test, and repair programs reliably—without needing a human to "massage" code.

> **X07 is under active development. APIs and tooling may change.**

**[Documentation](https://x07lang.org)** · [FAQ](https://x07lang.org/docs/faq) · [Releases](https://github.com/x07lang/x07/releases)

---

## Why X07?

Autonomous agents struggle with mainstream languages because of **multiple equivalent patterns**, **ambiguous diagnostics**, **nondeterministic test environments**, and **text-based patching on fragile syntax**. X07 makes these constraints first-class concerns:

### Machine-First Source Format

The canonical source is **x07AST JSON** (`*.x07.json`), not text files. Patches are structural ([RFC 6902 JSON Patch](https://datatracker.ietf.org/doc/html/rfc6902)), so agents apply changes mechanically—no parsing ambiguity, no whitespace surprises.

### Deterministic Execution

The primary execution model (`solve-*` worlds) is **resource-bounded and reproducible**. Agents can iterate through build → run → diff cycles without heisenbugs. Same input, same output, every time.

### Single Canonical Approach

One way to do each thing. No "should I use a for loop or map?" decisions. This eliminates the pattern confusion that plagues LLM-generated code in flexible languages.

### Machine-Readable Diagnostics

Errors are **structured identifiers with actionable fixes** designed for LLM consumption—not cryptic messages intended for humans to interpret.

### Explicit Capability Worlds

Side effects are opt-in. `solve-pure` is deterministic bytes → bytes. `run-os*` worlds enable real OS access. The boundary is explicit, not implicit.

---

## Quick Start

### Install

Download the latest release for your platform:

- **macOS:** `x07-<tag>-macOS.tar.gz`
- **Linux:** `x07-<tag>-Linux.tar.gz`
- **Windows:** `x07-<tag>-Windows.zip`

[Latest Release](https://github.com/x07lang/x07/releases/latest) · [All Releases](https://github.com/x07lang/x07/releases)

### Run a Program

```bash
x07-host-runner --program hello.x07.json --world solve-pure --input input.bin
```

### Agent Tooling

```bash
x07c fmt program.x07.json      # Format
x07c lint program.x07.json     # Lint
x07c fix program.x07.json      # Auto-fix issues
x07c apply-patch program.x07.json patch.json  # Apply RFC 6902 patch
```

---

## Capability Worlds

| World | Description |
|-------|-------------|
| `solve-pure` | Pure bytes → bytes, no I/O |
| `solve-fs` | Read-only fixture filesystem |
| `solve-rr` | Deterministic request/response (fixture-backed) |
| `solve-kv` | Deterministic key/value store |
| `solve-full` | fs + rr + kv combined |
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
│   └── x07-os-runner   # Standalone OS runner
├── stdlib/         # Standard library
├── benchmarks/     # Benchmark suites + fixtures
└── scripts/        # Tooling and CI scripts
```

## Related Repositories

- [`x07lang/x07`](https://github.com/x07lang/x07) — Toolchain + stdlib (this repo)
- [`x07lang/x07-website`](https://github.com/x07lang/x07-website) — x07lang.org
- [`x07lang/x07-registry`](https://github.com/x07lang/x07-registry) — Package registry
- [`x07lang/x07-index`](https://github.com/x07lang/x07-index) — Package index

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
