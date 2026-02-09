# FAQ

## Is X07 a general-purpose language?

Yes—via OS worlds it targets general-purpose usage (CLI tools, servers, automation).

Fixture worlds exist to make testing and repair deterministic, not to limit real-world capability.

## Why not just use Rust / Go / Python?

You can.
X07 exists because autonomous agents struggle with:

- multiple equivalent patterns,
- ambiguous diagnostics,
- nondeterministic test environments,
- text-based patching on fragile syntax.

X07 makes those constraints first-class.

## Is X07 safe?

In deterministic worlds, X07 is designed to be safe-by-construction.
In OS worlds, safety depends on policies and host sandboxing.

## Is X07 fast?

X07 compiles to C and can call native shims for performance-critical workloads.

The main performance risk is agent-written O(n²) logic—so the ecosystem includes budgets, metrics, and canonical builders.

## What is the canonical way to do streaming transforms?

Use [Streaming pipes](language/stream-pipes.md) (`std.stream.pipe_v1`): they are deterministic, budgeted, and have a single canonical composition model (source → transducers → sink).

## How do I make OS-world behavior reproducible for agents?

Use [Record/replay](worlds/record-replay.md) (`std.rr`) to record real interactions into cassettes under `.x07_rr/`, then replay deterministically in `solve-rr`.

## How do I prevent “agent refactor” performance regressions?

Use [Budget scopes](language/budget-scopes.md) to localize resource contracts, and enforce repo-wide invariants with [`x07 arch check`](toolchain/arch-check.md).

## How do I run property-based tests?

Use the built-in test harness:

- `x07 test --pbt --manifest tests/tests.json` (property-based tests only)
- `x07 test --all --manifest tests/tests.json` (unit + property-based)

When a counterexample is found, convert it into a deterministic regression test:

- `x07 fix --from-pbt <path/to/repro.json> --write`

See: [Property-based testing](toolchain/pbt.md).

## How do I use function contracts and verification?

x07AST v0.5 supports `requires` / `ensures` / `invariant` on `defn`/`defasync` declarations.

- Runtime contract violations show up in `x07 test` reports as `failure_kind: "contract_violation"` with a repro artifact path.
- For bounded proof artifacts, run `x07 verify --bmc|--smt --entry <sym>` (subset supported in v0.1; see CLI docs).

See: [Syntax & x07AST](language/syntax-x07ast.md) and [CLI](toolchain/cli.md).

## How do humans review agent patches?

Use the toolchain’s semantic diff + trust report artifacts:

- `x07 review diff` (intent-level HTML diff)
- `x07 trust report` (budgets, worlds, nondeterminism summary)

See: [Review & trust artifacts](toolchain/review-trust.md).

## How do I integrate multiple subsystems (pipes + RR + budgets + contracts)?

Start from the readiness-check examples under `docs/examples/` — they combine real subsystems end-to-end. For the common patterns that trip up agents (bytes/view friction, assertion ownership, fixture paths), see [Agent patterns](recipes/agent-patterns.md).

## Why does my `bytes.view` fail with "requires an identifier"?

`bytes.view` takes a **named local**, not an arbitrary expression. Bind the bytes-producing expression to a local first:

```json
["let", "raw", ["bytes.lit", "hello"]],
["bytes.view", "raw"]
```

See [Agent patterns — bytes vs bytes_view](recipes/agent-patterns.md#1-bytes-vs-bytes_view) for the full set of ownership recipes.

## What does `x07 doc std.stream.pipe_v1` return?

`x07 doc std.stream.pipe_v1` prints built-in documentation for the special form, including its shape and a short example.

Use `x07 guide` and [Streaming pipes](language/stream-pipes.md) for the complete reference and composition guidance.
