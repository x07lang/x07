# agent-eval: comparative agent benchmark harness

Measures whether coding agents produce correct programs in X07 as reliably as
in baseline languages, on identical bytes-in/bytes-out tasks judged by the
same vectors. This answers the question `x07 bench` does not: `x07 bench`
evaluates patches against X07 suites; agent-eval compares X07 against other
languages for fresh generation.

- `tasks/tasks.json` — task suite (prompt + acceptance vectors).
- `runner.py` — stdlib-only, offline, deterministic executor:
  `python3 runner.py --lang {python,x07} --solutions <dir> [--results <json>]`
  (X07 solutions are solve-pure single-program files run via `X07_BIN`).
- `solutions/<subject>/` — one solution per task: `<task_id>.py` or
  `<task_id>.x07.json`.
- `results/` — checked-in run results. Start with
  `results/pilot-2026-06-12.md`.
- `RUNBOOK.md` — the scaled 3-model protocol, cost estimate, and the
  predeclared go/park decision rule for the direct-authoring bet.
