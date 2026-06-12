# About X07

X07 is built for a world where most code is generated — and where the scarce resource is trust in running it.

That changes what matters:

- Determinism beats “cleverness”
- Canonical encodings beat “flexibility”
- Machine-readable diagnostics beat “nice error strings”
- Evidence (proofs, budgets, replayable runs) beats review-by-reading
- Stable boundaries beat ad-hoc conventions

Some high-level concepts that make those goals practical:

- [Streaming pipes](language/stream-pipes.md) (`std.stream.pipe_v1`) for deterministic, budgeted streaming composition
- [Branded bytes](language/types-memory.md#branded-bytes-typed-encodings) (`bytes@B`) for typed boundary encodings
- [Structured concurrency](language/concurrency-multiprocessing.md#structured-concurrency-taskscope_v1) (`task.scope_v1`) for “no orphan tasks”
- [Record/replay](worlds/record-replay.md) (`std.rr`) for OS → deterministic cassettes
- [Budget scopes](language/budget-scopes.md) (`budget.scope_v1`) for local cost contracts
- Contracts tooling: [architecture check](toolchain/arch-check.md), [schema derive](toolchain/schema-derive.md), [state machines](toolchain/state-machines.md)
- Property-based testing: [PBT](toolchain/pbt.md) (`x07 test --pbt`) with repro → regression promotion (`x07 fix --from-pbt`)
- Certificate-first verification: [Formal verification & certification](toolchain/formal-verification.md) (`x07 verify --prove`, `x07 prove check`, `x07 trust certify`)
- Human review artifacts: [Review & trust](toolchain/review-trust.md) (`x07 review diff`, `x07 trust report`)

On authoring: agents and humans can write X07 directly — the [x07text](language/x07text.md) projection, `x07 doc` behavioral summaries, and did-you-mean diagnostics (all 2026-06) exist for exactly that. Whether direct authoring deserves deeper language investment is decided by a published comparative eval, not by assertion (see [Why X07](why-x07.md) and the [roadmap](roadmap.md)).

X07 is open-source, with direction guided through:

- clear technical governance,
- strict compatibility policies for pinned specs,
- a small core toolchain and a deliberately narrow set of active companion repos ([2026-06 scope](roadmap.md)).

## What X07 is *not*

- A sandbox for running hostile, untrusted code.
- A language that optimizes primarily for human hand-writing.
- A language that allows unlimited “styles” for the same task.
- A proven win for direct agent authoring — that claim is gated on the comparative eval in `labs/agent-eval/`.

## What X07 is

- A deterministic, certifiable execution substrate for agent-written software.
- A practical C-backend systems language.
- A library ecosystem designed around pinned contracts.
