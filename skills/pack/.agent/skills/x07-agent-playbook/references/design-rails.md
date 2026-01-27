# Agent-first design rails

These constraints are intended to keep autonomous agents convergent and reviews tractable.

## One canonical way

- Prefer one canonical encoding, one canonical error space, one canonical workflow for common tasks.
- If multiple approaches exist, pick one and make it the default in tooling and docs.

## Contracts over patterns

- Prefer machine-checkable contracts (schemas, diagnostics codes, budgets) over “pattern vocabulary”.
- Keep outputs machine-readable (JSON) so repair loops can be automated.

## Functional core, imperative shell

- Keep core logic (parsing/transform/encode) pure.
- Isolate effects (fs/net/process/time) behind world adapters, so behavior is capability-auditable.

## Ports and adapters

- Use the same core logic with different adapters (unsandboxed vs sandboxed execution) without rewriting the core.

## Repairability

- Make failures actionable: stable diagnostic codes, and structured patches (JSON Patch) when feasible.

## Budgets and policy

- Treat fuel/memory/output budgets and sandbox policy as part of the design contract.
