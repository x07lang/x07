# agent-eval: comparative agent benchmark harness

Measures whether coding agents produce correct programs in X07 as reliably as
in baseline languages, on identical bytes-in/bytes-out tasks judged by the
same vectors. This answers the question `x07 bench` does not: `x07 bench`
evaluates patches against X07 suites; agent-eval compares X07 against other
languages for fresh generation.

## Kit

- `tasks/tasks.json` — 30 tasks, 10 per difficulty band (`band: a|b|c`):
  (a) byte/text transforms, (b) data-structure logic, (c) protocol/codec.
- `tasks/build_suite.py` — regenerates the suite; every new task's vectors come
  from a Python reference snippet (correct by construction). `--check` in CI.
- `render_prompt.py` — the exact cold-start prompt for a `(task, arm)`
  (`--task <id> --arm {python,rust,x07,x07text}`). X07 arms embed `x07 guide`
  and the doc-tool line and nothing else.
- `runner.py` — stdlib-only, offline, deterministic judge for all four arms:
  `python3 runner.py --lang {python,rust,x07,x07text} --solutions <dir>`
  (X07 solutions run solve-pure via `X07_BIN`).
- `score.py` — aggregates a run, computes pass@1 / pass@6 per band per arm, and
  applies the RUNBOOK go/park decision rule → verdict + markdown report.
- `solutions/reference/` — a 30-task Python baseline; smoke-test the judge with
  `python3 runner.py --lang python --solutions solutions/reference` (→ 30/30).
- `solutions/<subject>/` — one solution per task per arm: `<task_id>.{py,rs}`,
  `<task_id>.x07.json`, or `<task_id>.x07t`.
- `results/` — checked-in run results. Start with `results/pilot-2026-06-12.md`.

## Running it

- `RUNBOOK.md` — the scaled 3-model protocol, cost estimate, and the predeclared
  go/park decision rule for the direct-authoring bet.
- `DRIVER.md` — the operator steps: render prompts → drive 3 cross-vendor models
  cold per task → score → verdict. The kit is model-agnostic; supply the models.

Status: harness, 30-task banded suite, prompt renderer, and scorer are built and
self-validated. What remains is supplying the cross-vendor models and driving
them cold (the decision-grade run the project has not yet executed).
