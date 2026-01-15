# Agentic design patterns (for humans supervising autonomous coding agents)

This section is written for humans: the goal is to make it *easy* for agents to build correct software without inventing brittle architecture.

## The main idea: constrain the *boundaries*, not the *internals*

Agents are good at making wide edits and refactors.
They are bad at “guessing” invisible constraints.

So: **hard-enforce a small set of invariants**, and let the agent innovate inside them.

## High‑ROI “agent‑era rules”

These are unusually effective for autonomous reliability:

1. **No implicit global state**
   - Any state must be behind an explicit handle passed around.
   - This improves traceability and reduces “action at a distance”.

2. **Everything that crosses a module boundary has a pinned encoding doc**
   - If bytes flow across modules: pin a v1 encoding and provide accessors.
   - Never let modules invent ad-hoc structs/tuples for boundaries.

3. **Every module has a smoke harness**
   - A tiny deterministic test suite that proves core behavior and encodings.
   - Smoke harnesses run in fixture mode where possible.

4. **Architectural lints are first-class**
   - Imports/capabilities/budgets are checked by a deterministic linter.
   - This prevents dependency drift and “capability creep”.

5. **Refactor is expected**
   - Optimize for safe refactors, not “perfect architecture up front”.
   - Agents are strong at systematic edits—make those edits safe.

## Patterns that work well in X07

### 1) Pure core + adapters (imperative shell)

Keep business logic pure and deterministic, and put I/O behind adapters.

Benefits:

- deterministic tests in fixture worlds
- easy repair loop (failures are reproducible)
- OS integration is isolated

### 2) Ports & adapters (hexagonal)

Define small “ports” (interfaces) for fs/net/db/time, and implement:

- fixture adapters (deterministic)
- OS adapters (production)

This matches X07’s world model.

### 3) Architecture as data

For large projects, keep a small `arch/` folder:

- module graph
- budgets
- public API list (and encoding docs)
- forbidden dependencies

…and enforce it with a deterministic CI script.

## The architecture contract

Add `AGENTS.md` to every repo. Use this template:

---

# AGENTS.md (template)

## Purpose
One paragraph: what the system does.

## Non-goals
What it must not do.

## Invariants
- Determinism requirements
- Encoding requirements
- Error code requirements

## Module map (ports & adapters)
- core (pure)
- adapters (world-specific)
- entrypoints (thin glue)

## Tooling commands
- format: `x07 fmt --check ...`
- lint: `x07 lint ...`
- test: `x07 test ...`
- bench (optional): `x07 bench ...`

## Budgets / gates
- max allocs
- max memcpy
- timeouts
- network allowlists

## Forbidden changes
- do not change wire formats
- do not change error code spaces
- do not introduce new dependencies without lock update

---

## Further reading

See [References](../references.md).
