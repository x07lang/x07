# About X07

X07 is built for a world where coding agents do most of the coding.

That changes what matters:

- Determinism beats “cleverness”
- Canonical encodings beat “flexibility”
- Machine-readable diagnostics beat “nice error strings”
- Stable boundaries beat ad-hoc conventions

Some high-level concepts that make those goals practical:

- [Streaming pipes](language/stream-pipes.md) (`std.stream.pipe_v1`) for deterministic, budgeted streaming composition
- [Branded bytes](language/types-memory.md#branded-bytes-typed-encodings) (`bytes@B`) for typed boundary encodings
- [Structured concurrency](language/concurrency-multiprocessing.md#structured-concurrency-taskscope_v1) (`task.scope_v1`) for “no orphan tasks”
- [Record/replay](worlds/record-replay.md) (`std.rr`) for OS → deterministic cassettes
- [Budget scopes](language/budget-scopes.md) (`budget.scope_v1`) for local cost contracts
- Contracts tooling: [architecture check](toolchain/arch-check.md), [schema derive](toolchain/schema-derive.md), [state machines](toolchain/state-machines.md)

X07 is open-source, with direction guided through:

- clear technical governance,
- strict compatibility policies for pinned specs,
- a small core toolchain and a larger ecosystem of packages.

## What X07 is *not*

- A sandbox for running hostile, untrusted code.
- A language that optimizes primarily for human hand-writing.
- A language that allows unlimited “styles” for the same task.

## What X07 is

- A deterministic substrate for autonomous repair loops.
- A practical C-backend systems language.
- A library ecosystem designed around pinned contracts.
