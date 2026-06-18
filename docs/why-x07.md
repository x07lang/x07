# Why X07

X07 is the deterministic, certifiable execution substrate for agent-written software.

The premise: as code generation gets cheap, the bottleneck moves to trust — deciding whether generated code is safe to run, and proving that decision to a reviewer. X07 is built around that bottleneck. Code that agents (or transpilers) produce runs sandboxed, budgeted, replayable, and provable.

## The trust story (what makes agent code runnable and reviewable)

- **Deterministic evaluation:** keep correctness loops in `solve-*` worlds; use OS worlds only with explicit intent.
- **Record/replay:** turn real OS interactions into deterministic cassettes you can re-run in fixture worlds.
- **Budgets:** local `budget.scope_v1` caps and arch-driven budget profiles prevent cost blowups from small agent edits.
- **Capability sandboxing:** side effects are opt-in through explicit OS worlds and policy files; `run-os-sandboxed` defaults to a VM boundary on supported platforms.
- **Structured diagnostics:** a 647-code diagnostic catalog with quickfix coverage enforced as a CI gate; the toolchain surface is JSON-first (diagnostics + patches + reports), so "lint → fix → re-run" is machine-drivable.
- **Spec-first testing:** XTAL drives verify/repair/certify loops from pinned specs ([XTAL](toolchain/xtal.md)).
- **Proof-backed certification:** `x07 verify` produces proof and coverage artifacts, `x07 prove check` replays them, and `x07 trust certify` binds proof, test, boundary, and runtime evidence into a certificate ([Formal verification & certification](toolchain/formal-verification.md)).
- **Review artifacts:** semantic diff + trust report make changes auditable (world/capability deltas, budgets, nondeterminism flags).

Performance is part of the substrate story: X07 compiles via C to native code with fast compiles and small binaries, and a WASM target covers portable sandboxed execution.

## The authoring story (honest status)

Agents and humans can author X07 directly, and the 2026-06 toolchain improved that surface:

- [x07text](language/x07text.md): a lossless text projection (`x07 ast to-text` / `x07 ast from-text`; RFC 0001). Canonical source stays x07AST JSON.
- behavioral summaries for stdlib exports in `x07 doc`, with fuzzy lookup
- did-you-mean suggestions on unknown symbols, and structured diagnostics in `x07 run` failure reports

Direct authoring is an explicitly gated bet, not a settled claim. The comparative eval in `labs/agent-eval/` — agents solving identical tasks in X07 vs Python/Rust — has a completed pilot (`labs/agent-eval/results/pilot-2026-06-12.md`) and a scaled protocol with a predeclared decision rule (`labs/agent-eval/RUNBOOK.md`). That run decides whether deeper language investment proceeds (RFC 0002: records, enums + match, string, f64) or X07 continues substrate-first. The project publishes the results either way.

## Evidence pack (public, reproducible)

### 1) Comparative agent eval (`labs/agent-eval/`)

Agents solve identical bytes-in/bytes-out tasks in X07 and baseline languages, judged by the same vectors. The pilot result and the scaled runbook (with its predeclared go/park decision rule) are checked in.

### 2) Cross-language performance comparisons

For runtime/compile time/binary size comparisons (X07 vs C vs Rust vs Go), use:

- `x07lang/x07-perf-compare` (runs locally, verifies output equivalence)

### 3) Agent correctness benchmark harness (`x07 bench`)

`x07 bench` evaluates patch submissions against versioned benchmark suites with deterministic artifacts and a machine-readable report:

- Seed suite: `labs/x07bench/suites/core_v1/` (expanded; recommended)
- Docs: [Benchmarks](toolchain/benchmarks.md)

### 4) Diagnostic catalog + quickfix coverage gate

X07 tracks diagnostics as a catalog (647 codes) and enforces quickfix coverage as a CI gate:

- Catalog tooling: `x07 diag catalog`, `x07 diag check`, `x07 diag coverage`
- Rendered codes doc: [Diagnostic codes](toolchain/diagnostic-codes.md)

### 5) Agent-consumable spec + tool contracts

The toolchain exposes stable, machine-readable surfaces for agents:

- Schemas: `spec/*.schema.json`
- Offline docs + agent portal endpoints: [Agent contracts](agent/contract.md)
- Machine doc API: `x07 doc --json ...`

## How to evaluate locally

1. Start with [Install](getting-started/install.md) and [Your first project](getting-started/first-project.md).
2. If you are operating through a coding agent, add [Agent quickstart](getting-started/agent-quickstart.md) and [The agent workflow](getting-started/agent-workflow.md).
3. Run an end-to-end workflow on one of the reference projects under `docs/examples/`.
4. Run `x07 bench validate` / `x07 bench eval` on `labs/x07bench/suites/core_v1/`.
5. Reproduce the agent-eval pilot with `labs/agent-eval/runner.py` (stdlib-only, offline).
6. Run `x07-perf-compare` for cross-language perf + build size comparisons.
