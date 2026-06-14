# FAQ

## Is X07 a general-purpose language?

Yes—via OS worlds it targets general-purpose usage (CLI tools, servers, automation).

Fixture worlds exist to make testing and repair deterministic, not to limit real-world capability.

## Why not just use Rust / Go / Python?

You can.
X07 exists because running agent-written code in mainstream ecosystems leaves the trust questions open:

- nondeterministic test environments,
- ambiguous diagnostics,
- implicit capabilities and unbounded resource use,
- review that depends on humans re-reading source.

X07 makes determinism, budgets, capability sandboxing, structured diagnostics, and proof-backed certification first-class. Whether it also beats those languages for *direct agent authoring* is an open, measured question — see the next entry.

## Is direct agent authoring proven?

No — it is an explicitly gated bet, and the project says so.

The comparative eval in `labs/agent-eval/` (toolchain repo) has agents solve identical bytes-in/bytes-out tasks in X07 and baseline languages, judged by the same vectors. The pilot (`labs/agent-eval/results/pilot-2026-06-12.md`) showed X07 behind Python on first-attempt correctness (4/6 vs 6/6 pass@1, converging by attempt 2) with roughly 4x larger solutions. The scaled run — protocol, cost estimate, and a predeclared go/park decision rule — is specified in `labs/agent-eval/RUNBOOK.md` and decides whether deeper language investment (RFC 0002: records, enums + match, string, f64) proceeds. The results get published either way.

The 2026-06 authoring improvements (x07text, `x07 doc` behavioral summaries, did-you-mean diagnostics) came directly out of the pilot's friction log.

## Why is x07text not the canonical format?

Because the toolchain's contracts operate on one canonical artifact: x07AST JSON.

JSON Patch and quickfixes, diagnostic JSON Pointers (`ptr=/...`), schema validation, and deterministic formatting (`x07 fmt` emits canonical JCS bytes) all target the JSON document. Making text canonical would reintroduce the parse/drift problems JSON-first sources were chosen to avoid, and would split every tool across two source formats.

x07text is instead a **lossless projection**: `x07 ast to-text` renders readable text, `x07 ast from-text` converts back to byte-identical canonical JSON, and a CI corpus round-trip gate keeps the mapping exact. You can author entirely in x07text and let conversion re-canonicalize the file. See [x07text](language/x07text.md) and RFC 0001 in `x07-rfcs`.

## What happened to the studio / device / web-ui repos?

They were archived in the 2026-06 scope cut: `x07-studio`, `x07-forge`, `x07-crewops`, `x07-tactics`, `x07-device-host`, `x07-web-ui`, `x07-sentinel-reference-stack`, and the platform repos (`x07-platform`, `x07-platform-contracts`, `x07-platform-cloud`). They are read-only on GitHub.

The reasoning: those surfaces duplicate mature mainstream ecosystems, multiply single-maintainer load, and none of them move the core bet — the deterministic, certifiable substrate. The active set is `x07`, `x07-mcp`, `x07-registry`, `x07-wasm-backend`, and `hardproof`. Reactivation conditions are in the [roadmap](roadmap.md).

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

x07AST supports `requires` / `ensures` / `invariant` on `defn`/`defasync` declarations (v0.5+), plus `loop_contracts` on `defn` (added in v0.6). The current schema is `x07.x07ast@0.8.0`; see [Syntax & x07AST](language/syntax-x07ast.md) for the per-version contract breakdown.

- Runtime contract violations show up in `x07 test` reports as `failure_kind: "contract_violation"` with a repro artifact path.
- For bounded proof artifacts, run `x07 verify --bmc|--smt --entry <sym>`.
- For certification-oriented proof artifacts, run `x07 verify --prove --entry <sym>` and `x07 verify --coverage --entry <sym>`.
- `x07 verify --prove` now honors `catalog/verify_primitives.json` when it builds the proof harness, so trusted imported helpers can stay outside the local proof closure while still being listed in coverage artifacts.

See: [Formal verification & certification](toolchain/formal-verification.md), [Syntax & x07AST](language/syntax-x07ast.md), and [CLI](toolchain/cli.md).

## How do I start a project that can be reviewed from a certificate instead of source?

Use:

- `x07 init --template verified-core-pure`
- `x07 init --template trusted-sandbox-program`
- `x07 init --template certified-capsule`

Then run:

- `x07 trust profile check --profile arch/trust/profiles/verified_core_pure_v1.json --project x07.json --entry example.main`
- `x07 trust certify --project x07.json --profile arch/trust/profiles/verified_core_pure_v1.json --entry example.main --out-dir target/cert`

For the sandboxed trust line, use `trusted_program_sandboxed_local_v1` on a host with a supported VM backend. For capsule-only certification, use `certified_capsule_v1`.

## How do humans review agent patches?

Use the toolchain’s semantic diff + trust report artifacts:

- `x07 review diff` (intent-level HTML diff)
- `x07 trust report` (budgets, worlds, nondeterminism summary)

See: [Review & trust artifacts](toolchain/review-trust.md).

## How do I integrate multiple subsystems (pipes + RR + budgets + contracts)?

Start from the readiness-check examples under `docs/examples/` — they combine real subsystems end-to-end. For the common patterns that trip up agents (bytes/view friction, assertion ownership, fixture paths), see [Agent patterns](recipes/agent-patterns.md).

## Why does my `bytes.view` fail with "requires an identifier"?

`bytes.view` takes a **named local**, not an arbitrary expression. Bind the bytes-producing expression to a local first:

```clojure
; x07text
(let raw (bytes.lit hello))
(bytes.view raw)
```

See [Agent patterns — bytes vs bytes_view](recipes/agent-patterns.md#1-bytes-vs-bytes_view) for the full set of ownership recipes.

## What does `x07 doc std.stream.pipe_v1` return?

`x07 doc std.stream.pipe_v1` prints built-in documentation for the special form, including its shape and a short example.

Use `x07 guide` and [Streaming pipes](language/stream-pipes.md) for the complete reference and composition guidance.
