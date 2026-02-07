Spec-ID: x07.spec.internal.x07-standalone@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# X07 as a standalone language

X07 can be used “standalone” by using the compiler + runners directly, pinning the compiler revision (and `lang_id`) plus `stdlib.lock`/dependency lockfiles as needed.

## Build/run pipeline

1. Write an X07 program as **x07AST JSON** (`*.x07.json`), the canonical LLM-first source format.
2. Format + lint + autofix deterministically with `x07c` (machine-first tooling; stable JSON output + JSON Patch quickfixes).
3. Compile the program to C with `x07c`.
4. Compile and run the generated C as a native solver artifact:
   - Deterministic evaluation worlds: `x07-host-runner`
   - Standalone OS worlds: `x07-os-runner`

See `docs/spec/internal/x07-c-backend.md` for the solver ABI and execution details.

## Tooling loop (recommended)

For an autonomous agent workflow, treat `x07c` as the contract-enforcer:

- Canonicalize JSON: `cargo run -p x07c -- fmt --input program.x07.json --write`
- Lint (world-scoped): `cargo run -p x07c -- lint --input program.x07.json --world solve-pure`
- Apply safe fixes: `cargo run -p x07c -- fix --input program.x07.json --world solve-pure --write`
- Apply LLM repair edits (RFC 6902): `cargo run -p x07c -- apply-patch --program program.x07.json --patch repair.patch.json --out program.x07.json`

## Projects (modules/packages)

For multi-module programs and pinned dependencies, use the Phase E project workflow:

- `cargo run -p x07c -- lock --project <project/x07.json>`
- `cargo run -p x07c -- build --project <project/x07.json> --out <out.c>`
- `cargo run -p x07-host-runner -- --project <project/x07.json> --world solve-pure --input <case.bin>`

Details: `docs/spec/internal/modules-packages.md`.

## Standalone OS worlds (Phase H3)

Standalone OS worlds are not used in deterministic suites:

- `run-os`
- `run-os-sandboxed` (policy restricted; see `schemas/run-os-policy.schema.json`)

Phase H4 extends standalone-only worlds with systems features:

- `unsafe` blocks, raw pointers, and `extern` C declarations/calls (world-gated; not available in `solve-*` worlds).

Use `x07-os-runner` to compile+run programs in these worlds:

```bash
cargo run -p x07-os-runner -- \
  --program examples/h3/read_file_by_stdin.x07.json \
  --world run-os
```

## Freeze criteria (practical)

You can treat a given `lang_id` + compiler revision as a “release” when:

- A stable benchmark suite passes deterministically under `labs/scripts/bench/run_bench_suite.py`.
- The guide and prompts are stable enough to publish (LLM-first contracts: x07AST + x07Diag + JSON Patch).
- Any capability surfaces are world-scoped (`solve-pure`, `solve-fs`, `solve-rr`, `solve-kv`, `solve-full`) and covered by regression tests.
