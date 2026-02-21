# Agent workflow (how to build software with 100% autonomous coding agents)

X07 is designed around a simple reality:

> An agent that writes code also needs to **run checks**, **read diagnostics**, **apply patches**, and **prove invariants**—without human intervention.

So the workflow is built into the ecosystem:

- strict output contracts (JSON everywhere),
- deterministic tools,
- stable error codes,
- canonical file layouts.

## The “architecture contract” file

Every serious X07 project should include:

- `AGENT.md`

This is a short, structured contract that tells the agent:

- what the system is,
- what constraints it must respect,
- what *must never change*,
- how to run tests and interpret failures.

Use the template in: [Agentic design patterns](../libraries/agentic-design.md#the-architecture-contract).

## The minimal agent loop

An autonomous agent should follow a loop like:

1. Read task/spec + `AGENT.md`
2. Modify code **only through structured patches** (JSON Patch)
3. Iterate in deterministic worlds:
   - `x07 run` (auto-repair by default via `--repair=...`)
   - `x07 check --project x07.json` (non-mutating whole-project validation)
   - `x07 test`
   - optional: `x07 lint` for raw diagnostics (`x07diag`)
4. If it fails:
   - parse `x07diag` output
   - apply a suggested quickfix (`x07 fix`) or produce a new patch and apply it with `x07 ast apply-patch`
5. Repeat until green

See also: [Repair loop](../toolchain/repair-loop.md) and [Running programs](../toolchain/running-programs.md).

When dependencies change, update and verify the lockfile in the same loop:

- `x07 pkg lock --project x07.json`
- `x07 pkg lock --project x07.json --check` (CI gate)
- When the index can be consulted, `--check` also fails on yanked/advised deps unless explicitly allowed (`--allow-yanked` / `--allow-advisories`).
- For transitive dependency overrides, use `project.patch` in `x07.json` (canonical schema: `x07.project@0.3.0`; `x07.project@0.2.0` is legacy compatibility only).

If you want a good mental model for “AI-native engineering”, see OpenAI’s Codex guide on building AI-native engineering teams.

When OS access is required, prefer explicit profiles (for example `x07 run --profile os` / `x07 run --profile sandbox`) and run `x07 doctor` early to catch platform prerequisites.

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
