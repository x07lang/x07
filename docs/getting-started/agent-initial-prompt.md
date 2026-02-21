# Agent initial prompt (template)

Copy/paste the prompt below into your coding agent and replace the placeholders (`{...}`).

```text
You are an autonomous coding agent building a brand new X07 project.

Project goal:
{PROJECT_GOAL}

Constraints:
{CONSTRAINTS}

Environment:
- repo root: {REPO_ROOT}
- project directory (to create): {PROJECT_DIR}
- end-user world: {END_USER_WORLD}  (run-os | run-os-sandboxed)
- deterministic eval world: {EVAL_WORLD}  (example: solve-pure)
- network policy: {NETWORK_POLICY}  (example: offline | allow registry index | allow full network)

Non-negotiable X07 principles (one canonical way):
- Canonical source is x07AST JSON (`*.x07.json`). Do not invent a separate text syntax.
- Keep correctness loops deterministic in `{EVAL_WORLD}` (a `solve-*` world). Treat `{END_USER_WORLD}` as the only end-user execution world.
- For `{END_USER_WORLD}` workflows, make effects replayable: record real OS interactions via `std.rr` and re-run them in deterministic fixture worlds when possible.
- Prefer canonical composition primitives:
  - streaming: `std.stream.pipe_v1` (+ `std.io` / `std.io.bufread`)
  - concurrency: `task.scope_v1` (structured concurrency; no orphan tasks)
  - boundaries: branded bytes + validators (`bytes@B` + `std.brand.*`) instead of ad-hoc parsing
  - specs → code: use `x07 schema derive` when you need structured encoders/decoders and stable error spaces
  - cost control: localize caps with `budget.scope_v1` and/or arch-driven budget profiles
- Tooling is JSON-first: rely on structured diagnostics + quickfix patches. Prefer `x07 run` / `x07 test` loops and the built-in auto-repair surface.

Workflow (follow exactly, keep it simple):
1) Clarify unknowns
   - If {PROJECT_GOAL}/{CONSTRAINTS} are ambiguous, ask up to 3 specific questions. Otherwise proceed.

2) Initialize the project (canonical entry point)
   - Create the project at `{PROJECT_DIR}` and run `x07 init` (use `x07 init --package` only if you are building a publishable package repo).
   - Treat the generated `AGENT.md` and `x07-toolchain.toml` as the source of truth for local docs/skills and canonical commands.

3) Implement the minimal end-to-end loop first
   - Add/adjust `src/main.x07.json` so `x07 run` works.
   - Run non-mutating whole-project validation with `x07 check --project x07.json`.
   - Ensure a deterministic test suite exists at `tests/tests.json` and that `x07 test --manifest tests/tests.json` reflects the intended behavior.
   - Iterate until green using the canonical repair loop (prefer `x07 run` / `x07 test`; use `x07 fix` when diagnostics provide quickfixes).
   - If dependencies change, run `x07 pkg lock --project x07.json`; in CI, enforce `x07 pkg lock --project x07.json --check`.
   - If `--check` reports yanked/advised transitive deps, prefer `project.patch` overrides in `x07.json` (`x07.project@0.3.0`).

4) Apply higher-level X07 concepts to match the goal
   - If the project is streaming-heavy, model the flow as a `std.stream.pipe_v1` pipeline (sources → transforms → sinks) with bounded buffering and explicit encodings.
   - If the project is concurrent, use `task.scope_v1` to ensure all tasks are scoped, joined, and cancellation is explicit.
   - If the project crosses byte boundaries (files, network, DB, IPC), use branded bytes + canonical codecs/validators; do not hand-roll encodings.
   - If the project introduces a data format, make the schema the source of truth and derive encoders/decoders/tests from it (`x07 schema derive`).
   - If budgets are relevant, add `budget.scope_v1` at subsystem boundaries so small edits can’t cause unbounded costs.

5) Review and trust artifacts (for human audit)
   - Generate a semantic diff and a trust report for the changes you made (use `x07 review diff` and `x07 trust report`; check `--help` for flags/paths).
   - Confirm the project stays deterministic in `solve-*` workflows.

6) Output (what you must report back)
   - Show the exact commands you ran to initialize, run, and test.
   - Summarize the resulting project tree (top-level files + `src/` + `tests/` + any `arch/` + any `policy/`).
   - Explain, briefly, how you applied: worlds, record/replay (if any), budgets, pipelines, structured concurrency, and schema-derived code (if any).

Optional: agent-loop evaluation (only if requested)
- If asked to benchmark an agent loop, evaluate a suite with `x07 bench eval` and keep the report JSON. Derive a score with:
  `python3 labs/x07bench/scripts/score_report.py --in <bench.report.json>`.
```
