# Why X07

X07 is an agent-first systems language: it’s designed so autonomous coding agents can generate, modify, test, and repair programs reliably.

The core bet is simple:
**agents converge faster and more safely when the language and toolchain are built around deterministic workflows, canonical representations, and machine-checkable boundaries.**

## The trust story (what makes agent code reviewable)

- **Deterministic evaluation:** keep correctness loops in `solve-*` worlds; use OS worlds only with explicit intent.
- **Record/replay:** turn real OS interactions into deterministic cassettes you can re-run in fixture worlds.
- **Budgets:** local `budget.scope_v1` caps and arch-driven budget profiles prevent cost blowups from small agent edits.
- **Repair loops:** the toolchain surface is JSON-first (diagnostics + patches + reports), so “lint → fix → re-run” is machine-drivable.
- **Review artifacts:** semantic diff + trust report make changes auditable (world/capability deltas, budgets, nondeterminism flags).

## Evidence pack (public, reproducible)

### 1) Cross-language performance comparisons

For runtime/compile time/binary size comparisons (X07 vs C vs Rust vs Go), use:

- `x07lang/x07-perf-compare` (runs locally, verifies output equivalence)

### 2) Agent correctness benchmark harness (`x07 bench`)

`x07 bench` evaluates patch submissions against versioned benchmark suites with deterministic artifacts and a machine-readable report:

- Seed suite: `labs/x07bench/suites/core_v1/` (expanded; recommended)
- Docs: [Benchmarks](toolchain/benchmarks.md)

### 3) Diagnostic catalog + quickfix coverage gate

X07 tracks diagnostics as a catalog and enforces quickfix coverage as a CI gate:

- Catalog tooling: `x07 diag catalog`, `x07 diag check`, `x07 diag coverage`
- Rendered codes doc: [Diagnostic codes](toolchain/diagnostic-codes.md)

### 4) Agent-consumable spec + tool contracts

The toolchain exposes stable, machine-readable surfaces for agents:

- Schemas: `spec/*.schema.json`
- Offline docs + agent portal endpoints: [Agent contracts](agent/contract.md)
- Machine doc API: `x07 doc --json ...`

## How to evaluate locally (one path)

1. Start from [Agent quickstart](getting-started/agent-quickstart.md).
2. Run an end-to-end workflow on one of the reference projects under `docs/examples/`.
3. Run `x07 bench validate` / `x07 bench eval` on `labs/x07bench/suites/core_v1/`.
4. Run `x07-perf-compare` for cross-language perf + build size comparisons.
