Spec-ID: x07.spec.internal.x07-core@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# X07 Core (Track B): build a compiler, not a VM

X07 is an experiment in discovering **programming languages optimized for LLMs to write**, not for humans to read.

The feedback loop is circular:

- A language is only “good” if an LLM can reliably write correct programs in it.
- You only learn what the LLM needs by observing successes/failures across many tasks.

This repo makes that loop explicit, deterministic, and measurable.

## Language and programs (Track B)

The core artifact is the compiler + its embedded language definition (versioned by `lang_id`).

In Track B:

- The language is defined by core built-ins plus built-in stdlib modules (no macro/rewrite layer).
- The compiler (`x07c`) turns the program into **C source**.
- The runner compiles the C source to a **native executable solver artifact**.
- The runner executes the solver deterministically with strict fuel + memory limits.

Seed language references:
- Guide snapshot: `docs/spec/language-guide.md`
- Default benchmark bundle: `labs/benchmarks/bundles/regression.json`
- Optional suite: `labs/benchmarks/solve-pure/phase4-holdout.json`

## Determinism and safety

To keep fitness stable and evaluation safe:

- Evaluation uses deterministic fuel-based halting for solver runs.
- Capability access is deny-by-default and world-scoped (`solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, `solve-full`).
- Solver artifacts run with a fixed environment (no inherited args/env) and Unix resource limits as kill-switches.

Implementation details: `docs/spec/internal/x07-c-backend.md`.

## Regression suites

Benchmark suites live under `labs/benchmarks/` and are executed against committed reference solutions in `labs/benchmarks/solutions/`.

To run the default regression bundle:

- `python3 labs/scripts/bench/run_bench_suite.py --suite labs/benchmarks/bundles/regression.json`
