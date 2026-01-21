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

- `AGENTS.md`

This is a short, structured contract that tells the agent:

- what the system is,
- what constraints it must respect,
- what *must never change*,
- how to run tests and interpret failures.

Use the template in: [Agentic design patterns](../libraries/agentic-design.md#the-architecture-contract).

## The minimal agent loop

An autonomous agent should follow a loop like:

1. Read task/spec + `AGENTS.md`
2. Modify code **only through structured patches** (JSON Patch)
3. Run deterministic checks:
   - `x07 lint`
   - `x07 test`
   - optional: benchmark suites (for performance budgets)
4. If it fails:
   - parse `x07diag` output
   - apply a suggested quickfix (`x07 fix`), or produce a new patch and apply it with `x07 ast apply-patch`
5. Repeat until green

If you want a good mental model for “AI-native engineering”, see OpenAI’s Codex guide on building AI-native engineering teams.

When OS access is required, prefer explicit profiles (for example `x07 run --profile os` / `x07 run --profile sandbox`) and run `x07 doctor` early to catch platform prerequisites.

## Golden rule: keep boundaries explicit

Agent-written code is far more reliable if your system architecture makes boundaries explicit:

- pure logic separated from I/O,
- deterministic tests separated from OS worlds,
- adapters kept thin and declarative.

The “ports and adapters” (hexagonal) model is a good default because it keeps I/O at the edges and preserves a stable functional core. See Alistair Cockburn’s write-up for background.
