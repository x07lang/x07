# Agent workflow (building software with autonomous coding agents)

X07 is designed around a simple reality:

> An agent that writes code also needs to **run checks**, **read diagnostics**, **apply patches**, and **prove invariants**—without human intervention.

So the workflow is built into the ecosystem:

- strict output contracts (JSON everywhere),
- deterministic tools,
- stable error codes,
- canonical file layouts.

New here? Start with [Agent quickstart](agent-quickstart.md) — it is the canonical on-ramp and defines the core agent loop. This page covers the project-shaping pieces an agent needs on top of that loop: the architecture contract file, the package → example → scenario map, and the boundary rules.

## The “architecture contract” file

Every serious X07 project should include:

- `AGENT.md`

This is a short, structured contract that tells the agent:

- what the system is,
- what constraints it must respect,
- what *must never change*,
- how to run tests and interpret failures.

Use the template in: [Agentic design patterns](../libraries/agentic-design.md#the-architecture-contract).

## The agent loop

The core agent loop (run → test → repair, with patches and migration) is defined once in [Agent quickstart](agent-quickstart.md#3-the-core-loop-run--test-auto-repair). Run it from the project root; this page assumes that loop is already in place.

Two project-level habits make the loop reliable for autonomous work:

- Read the task/spec + `AGENT.md` first, and use `x07 doc <symbol>` for behavioral summaries of stdlib exports before guessing at APIs.
- When OS access is required, prefer explicit profiles (for example `x07 run --profile os` / `x07 run --profile sandbox`) and run `x07 doctor` early to catch platform prerequisites.

If you want a good mental model for “AI-native engineering”, see OpenAI’s Codex guide on building AI-native engineering teams.

## Canonical by-example workflow (CI-gated, offline)

The `x07` repo includes two canonical, CI-gated “agentic” by-example surfaces:

- Agent-gate example projects under `docs/examples/agent-gate/` (runnable via `x07 run`, validated by `./scripts/ci/check_agent_examples.sh`)
- Agent-scenario fixtures under `ci/fixtures/agent-scenarios/` (validated by `./scripts/ci/check_agent_scenarios.sh`)

### Package → example → scenario map

Each capability below has one canonical package, one runnable example, and one or more scenario fixtures.

- `text.core` (canonical: `ext-text`)
  - Example: `docs/examples/agent-gate/text-core/text-utils/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/text-core/missing-dep/`
    - `ci/fixtures/agent-scenarios/text-core/semantics/`
- `math.bigint` (canonical: `ext-bigint-rs`)
  - Example: `docs/examples/agent-gate/math-bigint/factorial-100/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/math-bigint/missing-dep/`
    - `ci/fixtures/agent-scenarios/math-bigint/codec-v1/`
- `math.decimal` (canonical: `ext-decimal-rs`)
  - Example: `docs/examples/agent-gate/math-decimal/money-format/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/math-decimal/missing-dep/`
    - `ci/fixtures/agent-scenarios/math-decimal/scale-rounding/`
- `text.unicode` (canonical: `ext-unicode-rs`)
  - Example: `docs/examples/agent-gate/text-unicode/normalize-casefold/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/text-unicode/missing-dep/`
    - `ci/fixtures/agent-scenarios/text-unicode/normalize-casefold-segmentation/`
- `data.cbor` (canonical: `ext-cbor-rs`)
  - Example: `docs/examples/agent-gate/data-cbor/roundtrip/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/data-cbor/missing-dep/`
    - `ci/fixtures/agent-scenarios/data-cbor/canonical-encoding/`
- `data.msgpack` (canonical: `ext-msgpack-rs`)
  - Example: `docs/examples/agent-gate/data-msgpack/roundtrip/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/data-msgpack/missing-dep/`
    - `ci/fixtures/agent-scenarios/data-msgpack/encoding-vectors/`
- `checksum.fast` (canonical: `ext-checksum-rs`)
  - Example: `docs/examples/agent-gate/checksum-fast/smoke/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/checksum-fast/missing-dep/`
    - `ci/fixtures/agent-scenarios/checksum-fast/vectors/`
- `diff.patch` (canonical: `ext-diff-rs`)
  - Example: `docs/examples/agent-gate/diff-patch/apply/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/diff-patch/missing-dep/`
    - `ci/fixtures/agent-scenarios/diff-patch/patch-v1/`
- `compress.zstd` (canonical: `ext-compress-rs`)
  - Example: `docs/examples/agent-gate/compress-zstd/roundtrip/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/compress-zstd/missing-dep/`
    - `ci/fixtures/agent-scenarios/compress-zstd/roundtrip-vectors/`
- `fs.globwalk` (canonical: `ext-path-glob-rs`)
  - Example: `docs/examples/agent-gate/fs-globwalk/list-files/` (`x07 run`)
  - Scenarios:
    - `ci/fixtures/agent-scenarios/fs-globwalk/missing-dep/`
    - `ci/fixtures/agent-scenarios/fs-globwalk/deterministic-ordering/`

## Golden rule: keep boundaries explicit

Agent-written code is far more reliable if your system architecture makes boundaries explicit:

- pure logic separated from I/O,
- deterministic tests separated from OS worlds,
- adapters kept thin and declarative.

High-level primitives that make this “explicit boundaries” rule concrete:

- Use [Streaming pipes](../language/stream-pipes.md) to make streaming composition canonical and budgeted.
- Use [Branded bytes](../language/types-memory.md#branded-bytes-typed-encodings) to make boundary encodings typechecked.
- Use [Structured concurrency](../language/concurrency-multiprocessing.md#structured-concurrency-taskscope_v1) (`task.scope_v1`) to prevent orphan tasks.
- Use [Record/replay](../worlds/record-replay.md) to turn OS runs into deterministic cassettes.
- Use [Budget scopes](../language/budget-scopes.md) to localize performance/cost contracts.
- Enforce repo-level invariants with [`x07 arch check`](../toolchain/arch-check.md).

The “ports and adapters” (hexagonal) model is a good default because it keeps I/O at the edges and preserves a stable functional core. See Alistair Cockburn’s write-up for background.
