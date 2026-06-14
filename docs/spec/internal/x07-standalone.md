Spec-ID: x07.spec.internal.x07-standalone@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# X07 as a standalone language

X07 can be used “standalone” by using the compiler + runners directly, pinning the compiler revision (and `lang_id`) plus `stdlib.lock`/dependency lockfiles as needed.

## Build/run pipeline

1. Write an X07 program as **x07AST JSON** (`*.x07.json`), the canonical LLM-first source format.
2. Format + lint + autofix deterministically with `x07` (machine-first tooling; stable JSON output + JSON Patch quickfixes).
3. Build the program to C with `x07 build`.
4. Compile and run the generated C as a native solver artifact with `x07 run` (it selects the deterministic or OS runner from the project world).

See `docs/spec/internal/x07-c-backend.md` for the solver ABI and execution details.

## Tooling loop (recommended)

For an autonomous agent workflow, treat `x07` as the contract-enforcer:

- Canonicalize JSON: `x07 fmt --input program.x07.json`
- Lint: `x07 lint --input program.x07.json`
- Apply safe fixes: `x07 fix --input program.x07.json`
- Apply LLM repair edits (RFC 6902): `x07 ast apply-patch --in program.x07.json --patch repair.patch.json --out program.x07.json`

## Projects (modules/packages)

For multi-module programs and pinned dependencies, use the project workflow:

- `x07 pkg lock --project <project/x07.json>`
- `x07 build --project <project/x07.json> --out <out.c>`
- `x07 run --project <project/x07.json> --input <case.bin>`

Details: `docs/spec/internal/modules-packages.md`.

## Standalone OS worlds

Standalone OS worlds are not used in deterministic suites:

- `run-os`
- `run-os-sandboxed` (policy restricted; see `schemas/run-os-policy.schema.json`)

Standalone OS worlds also include systems features:

- `unsafe` blocks, raw pointers, and `extern` C declarations/calls (world-gated; not available in `solve-*` worlds).

Use `x07-os-runner` to compile+run programs in these worlds.

## Freeze criteria (practical)

You can treat a given `lang_id` + compiler revision as a “release” when:

- A stable benchmark suite passes deterministically under `labs/scripts/bench/run_bench_suite.py`.
- The guide and prompts are stable enough to publish (LLM-first contracts: x07AST + x07Diag + JSON Patch).
- Any capability surfaces are world-scoped (`solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, `solve-full`) and covered by regression tests.
