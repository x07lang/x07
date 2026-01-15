# x07CLI v1 — internal tooling borrows (separate plan)

This plan is intentionally **separate** from the external `ext.cli.*` package proposal:

- External plan: `docs/phases/x07-cli-v1-ext-cli-proposal.md`
- This doc: changes to internal Rust CLIs/tools to make agent workflows more reliable.

The separation keeps the external package contract stable while letting internal tools evolve without blocking `ext.cli` adoption.

---

## 1) Highest-leverage borrow: stable machine contracts everywhere

Goal: when an agent calls any internal tool, stdout is either:

- a single JSON object with a stable `schema_version`, or
- the requested raw artifact bytes (and nothing else).

Concretely:

- Avoid mixing `println!` JSON with trailing human strings like `"lint failed"`.
- Prefer a report that includes `ok`, `diagnostics`, and `exit_code` fields.

`x07 ast …` already trends in this direction; extend that consistency to other CLIs.

---

## 2) `x07-host-runner`: add module roots (enables ext-package benches)

Needed to run the semantic bundle’s CLI determinism suite against `ext.cli.*` solutions.

Proposal:

- Add `--module-root <DIR>` (repeatable) to `x07-host-runner`.
- Plumb through to `compile::CompileOptions.module_roots`.

Acceptance:

- Existing suites still pass with no `--module-root` provided.
- New suite `benchmarks/solve-pure/cli-v1-specrows-determinism.json` can compile a solution importing `ext.cli`.

---

## 3) Bench harness: allow module roots via env

Proposal:

- `scripts/bench/run_bench_suite.py` reads `X07_BENCH_MODULE_ROOT` as a `:`-separated list of roots and passes `--module-root` flags to `x07-host-runner`.

Why:

- avoids changing every suite file format
- keeps the suite format stable

---

## 4) `x07c`: converge on report schemas for fmt/lint/fix

Today:

- `x07c lint` prints JSON report but also emits a human error on failure.
- `x07c fmt` uses exit status + stderr strings.

Proposal:

- Add a `--report-json` flag to each subcommand that prints a single JSON object with:
  - `schema_version`
  - `ok`
  - `in`
  - `diagnostics_count` (and optionally diagnostics)

Keep existing output modes for humans if needed, but make “agent mode” one canonical JSON contract.

---

## 5) Internal CLI self-description (SpecRows-like)

Borrow the “SpecRows as a flat table” idea for internal tools:

- Add `--cli-specrows` (or `--specrows`) to `x07`, `x07c`, and runners to print their interface in `x07cli.specrows@0.1.0` format.

Agents can then:

- avoid scraping `--help`
- build correct invocations from structured data

This can be implemented by generating SpecRows from Clap metadata (Rust-side).

