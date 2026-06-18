# Driver: running the decision-grade comparative eval

This kit turns `RUNBOOK.md` into a runnable experiment. It does NOT run the
models for you — you supply the three frontier models (different vendors,
cold context per task). The kit renders the prompts, defines the run layout,
judges every solution byte-exact, and applies the predeclared go/park verdict.

## What's in the kit

- `tasks/tasks.json` — 30 tasks, 10 per difficulty band (`band: a|b|c`).
  Regenerate with `python3 tasks/build_suite.py` (idempotent; `--check` in CI).
- `render_prompt.py` — the exact cold-start prompt for a `(task, arm)`.
- `runner.py` — byte-exact judge for python / rust / x07 / x07text solutions.
- `score.py` — aggregates a run and applies the decision rule → verdict + report.
- `solutions/reference/` — a 30-task Python baseline; use it to smoke-test the
  judge (`python3 runner.py --lang python --solutions solutions/reference` → 30/30).

## Prerequisites

```bash
export X07_BIN="$HOME/.x07/bin/x07"   # x07 >= 0.2.17 (keyword `x07 doc`, .x07t build input)
"$X07_BIN" --version                  # 0.2.17+
rustc --version                       # for the rust arm
python3 --version
# sanity-check the harness end-to-end before spending model budget:
X07_BIN="$X07_BIN" python3 runner.py --lang python --solutions solutions/reference
```

## Arms

| arm | ext | what the model emits |
|---|---|---|
| `python` | `.py` | single-file stdlib Python, stdin→stdout |
| `rust` | `.rs` | single-file `rustc -O` Rust, stdin→stdout |
| `x07` | `.x07.json` | x07AST JSON solve-pure program (`input` → solve bytes) |
| `x07text` | `.x07t` | x07text solve-pure program (converted via `x07 ast from-text`) |

The two X07 arms isolate how much of any gap is the JSON surface vs the
language. Python is the baseline the decision rule measures against; Rust is a
size/known-good control.

## Protocol (per RUNBOOK)

- 3 frontier models, different vendors. Fresh/cold session per `(task, arm)` —
  no repo warm-up; cold-start is the honest condition.
- Prompting: paste the output of `render_prompt.py`. For the X07 arms that is
  the task + I/O contract + the doc-tool line + `x07 guide`, and nothing else.
- Tool budget: X07 arms may use up to **N=6** iterations of
  `x07 doc` / `x07 check` / `x07 run` to repair. Baselines: emit then (optionally)
  repair within the same 6-iteration budget.
- Record, for every session: the **first emission** (`attempt1`) and the
  **final solution**, plus a `session.json` and the full transcript.

## Run layout

```
runs/<run_id>/
  <model>/<arm>/<task_id>.<ext>            # final solution     (judged for pass@6)
  <model>/<arm>/<task_id>.attempt1.<ext>   # first emission     (judged for pass@1)
  <model>/<arm>/<task_id>.session.json     # metrics (below)
  transcripts/<model>/<arm>/<task_id>.txt  # full transcript (the friction log)
```

`session.json` schema (all fields optional; richer data → richer report):

```json
{ "tool_iterations": 3, "tokens_in": 12000, "tokens_out": 900, "wall_ms": 41000 }
```

`runs/` is gitignored (large, model-specific). Keep transcripts — they are the
qualitative friction log RUNBOOK calls for.

## Step-by-step

1. Render every prompt (4 arms × 30 tasks = 120 files):

   ```bash
   for arm in python rust x07 x07text; do
     for t in $(python3 -c 'import json;print(" ".join(t["id"] for t in json.load(open("tasks/tasks.json"))["tasks"]))'); do
       X07_BIN="$X07_BIN" python3 render_prompt.py --task "$t" --arm "$arm" --out "prompts/$arm/$t.txt"
     done
   done
   ```

2. For each model × each prompt: run a cold session, save `attempt1`, final,
   `session.json`, and the transcript into the run layout above. (This is your
   model-harness; the kit is agnostic to how you call each vendor.)

3. Score and get the verdict:

   ```bash
   X07_BIN="$X07_BIN" python3 score.py --run runs/<run_id> \
     --out-json results/scaled-<run_id>.json --out-md results/scaled-<run_id>.md
   ```

## Predeclared decision rule (commit before looking at results)

`score.py` computes this automatically over bands (a)+(b):

- **bet_alive** — X07 (either arm) reaches ≥ 90% of Python's pass@6 on (a)+(b)
  AND median repair iterations ≤ Python's + 1. Prioritize RFC 0002, re-run.
- **keep_x07text_park_json** — X07 misses the bar but x07text materially beats
  the JSON arm. Keep x07text, park JSON-first authoring, proceed substrate-first.
- **park_direct_authoring** — X07 misses in both arms. X07 proceeds as a
  substrate (transpile target + verification + sandbox); language-surface work
  stops after RFC 0001.

Publish the report either way.

## Cost

~480 sessions (3 models × ~40 tasks × 4 arms; this suite is 30 tasks → ~360).
At ~30k tokens/session (X07 arms higher due to the guide) ≈ 11–15M tokens,
roughly $150–400 at mid-2026 frontier pricing, plus harness time.
