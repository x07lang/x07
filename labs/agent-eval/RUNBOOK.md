# Scaled comparative eval — runbook

Goal: measure whether agents produce correct programs more reliably/cheaply in
X07 than in baseline languages, with enough power to decide the
direct-authoring bet. This is the experiment the project has never run; do it
before further language-surface investment beyond RFC 0001/0002.

## Protocol

- Tasks: extend `tasks/tasks.json` to 30–50 tasks across three difficulty
  bands: (a) byte/text transforms, (b) data-structure logic (maps, sorting,
  graphs), (c) protocol/codec tasks (length-prefixed framing, checksums,
  JSON shaping). Keep every task bytes-in/bytes-out and deterministic so the
  same vectors judge every language.
- Languages: X07 (JSON), X07 (authored as x07text, converted via
  `x07 ast from-text`), Python, Rust. The x07text arm isolates how much of
  the gap is the JSON surface vs the language itself.
- Models: 3 frontier models from different vendors, fresh context per task
  (no warm-up on the repo — cold-start is the honest condition).
- Prompting: one shared template: task prompt + language-specific minimal
  context. For X07 arms include exactly: `x07 guide` output and the doc-tool
  usage line — nothing else. Agents may call `x07 doc`/`x07 lint`/`x07 run`
  up to N=6 tool iterations.
- Metrics per (task, language, model): pass@1, pass@6 (with repair
  iterations), tool calls used, total tokens in/out, wall-clock, solution
  bytes.
- Runs: 1 attempt per (task, lang, model) at temperature 0 equivalents; the
  iteration budget captures repair behavior; 3 models x ~40 tasks x 4 arms ≈
  480 sessions.

## Cost estimate

At ~30k tokens/session average (X07 arms higher due to guide+doc context):
~15M tokens ≈ $150–400 at mid-2026 frontier pricing, plus harness time.
One engineer-week including task authoring and report.

## Predeclared decision rule (commit to this before looking at results)

- If X07 (either arm) reaches >= 90% of Python's pass@6 on bands (a)+(b) AND
  median repair iterations <= Python's + 1: the direct-authoring bet stays
  alive; prioritize RFC 0002 (expressiveness floor) and re-run after.
- If X07 stays below that bar while the x07text arm materially beats the
  JSON arm: keep x07text, park JSON-first authoring guidance, still proceed
  substrate-first.
- If X07 misses the bar in both arms: park the direct-authoring bet entirely;
  X07 proceeds as a substrate (transpile target + verification + sandbox),
  and language-surface work stops after RFC 0001.

Publish the report either way — a negative result published honestly is
worth more credibility than another feature.

## Mechanics

The runnable kit for this protocol is built — see **`DRIVER.md`** for the
operator steps. Status of the pieces RUNBOOK called for:

- Tasks: `tasks/tasks.json` is at 30 tasks, 10 per band (`band: a|b|c`),
  regenerated + validated by `tasks/build_suite.py` (every vector is backed by
  a Python reference; `solutions/reference/` is the 30-task baseline).
- Runner: `runner.py` executes all four arms (python, rust, x07, x07text).
- Prompts: `render_prompt.py` renders the exact cold-start prompt per
  `(task, arm)` (X07 arms get `x07 guide` + the doc-tool line and nothing else).
- Scoring: `score.py` judges a run, computes pass@1 / pass@6 per band per arm,
  and applies the go/park rule above automatically → verdict + markdown report.
- Keep all model transcripts; they are the qualitative friction log.
- Results land in `results/` as JSON + a markdown report; cite
  `results/pilot-2026-06-12.md` as the pilot.

What remains is supplying the 3 cross-vendor models and driving them cold per
`DRIVER.md`; the kit is model-agnostic.
