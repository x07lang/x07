#!/usr/bin/env python3
"""Score a comparative-eval run and apply the predeclared decision rule.

Consumes a run directory produced by driving models against rendered prompts
(see DRIVER.md), judges every solution byte-exact against the task vectors
(reusing runner.py), and computes per-arm / per-band metrics plus the
RUNBOOK go/park verdict for the direct-authoring bet.

Run layout (see DRIVER.md):
  runs/<run_id>/<model>/<arm>/<task_id>.<ext>            final solution (pass@N)
  runs/<run_id>/<model>/<arm>/<task_id>.attempt1.<ext>   first emission (pass@1); optional
  runs/<run_id>/<model>/<arm>/<task_id>.session.json     {tokens_in,tokens_out,tool_iterations,wall_ms}

ext: python=.py rust=.rs x07=.x07.json x07text=.x07t

Usage:
  X07_BIN=~/.x07/bin/x07 python3 score.py --run runs/2026-06-17 --out-json results/run.json --out-md results/run.md
"""
from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
from pathlib import Path

import runner  # run_python / run_rust / run_x07 / run_x07text live here

HERE = Path(__file__).resolve().parent

ARMS = ("python", "rust", "x07", "x07text")
EXT = {"python": ".py", "rust": ".rs", "x07": ".x07.json", "x07text": ".x07t"}
RUN_FN = {
    "python": runner.run_python,
    "rust": runner.run_rust,
    "x07": runner.run_x07,
    "x07text": runner.run_x07text,
}
# Decision rule operates on bands (a)+(b).
DECISION_BANDS = ("a", "b")
PASS_BAR = 0.90        # X07 must reach >= 90% of Python's pass@6 on a+b
ITER_SLACK = 1         # ...and median repair iters <= Python's + 1


def judge(solution: Path, arm: str, task: dict) -> str:
    """Return 'pass' | 'fail' | 'error' | 'missing' for one solution file."""
    if not solution.is_file():
        return "missing"
    run_fn = RUN_FN[arm]
    runner._BUILD_CACHE.pop(solution, None)  # don't cross-contaminate cached builds
    for vector in task["vectors"]:
        try:
            got, err = run_fn(solution, vector["input"].encode())
        except subprocess.TimeoutExpired:
            return "error"
        if err is not None:
            return "error"
        if got != vector["expected"].encode():
            return "fail"
    return "pass"


def load_session(path: Path) -> dict:
    if not path.is_file():
        return {}
    try:
        return json.loads(path.read_text())
    except (ValueError, OSError):
        return {}


def score_run(run_dir: Path, tasks: dict) -> dict:
    task_by_id = {t["id"]: t for t in tasks["tasks"]}
    models = sorted(p.name for p in run_dir.iterdir() if p.is_dir() and p.name != "transcripts")

    cells = []  # one per (model, arm, task)
    for model in models:
        for arm in ARMS:
            arm_dir = run_dir / model / arm
            if not arm_dir.is_dir():
                continue
            ext = EXT[arm]
            for task in tasks["tasks"]:
                tid = task["id"]
                final = arm_dir / f"{tid}{ext}"
                attempt1 = arm_dir / f"{tid}.attempt1{ext}"
                session = load_session(arm_dir / f"{tid}.session.json")
                final_status = judge(final, arm, task)
                # pass@1: first emission if recorded, else fall back to final.
                a1_path = attempt1 if attempt1.is_file() else final
                a1_status = judge(a1_path, arm, task)
                cells.append({
                    "model": model, "arm": arm, "task": tid, "band": task["band"],
                    "pass_final": final_status == "pass",
                    "pass_at_1": a1_status == "pass",
                    "final_status": final_status,
                    "tool_iterations": session.get("tool_iterations"),
                    "tokens_in": session.get("tokens_in"),
                    "tokens_out": session.get("tokens_out"),
                    "solution_bytes": final.stat().st_size if final.is_file() else None,
                })

    def rate(rows, key):
        rows = [r for r in rows if r["final_status"] != "missing"]
        return round(sum(1 for r in rows if r[key]) / len(rows), 4) if rows else None

    def med_iters(rows):
        xs = [r["tool_iterations"] for r in rows if isinstance(r["tool_iterations"], (int, float))]
        return round(statistics.median(xs), 2) if xs else None

    # Per-arm aggregate (across models).
    arms_summary = {}
    for arm in ARMS:
        arm_cells = [c for c in cells if c["arm"] == arm]
        present = [c for c in arm_cells if c["final_status"] != "missing"]
        ab = [c for c in arm_cells if c["band"] in DECISION_BANDS]
        bytes = [c["solution_bytes"] for c in present if c["solution_bytes"]]
        toks = [(c["tokens_in"] or 0) + (c["tokens_out"] or 0) for c in present
                if c["tokens_in"] is not None or c["tokens_out"] is not None]
        arms_summary[arm] = {
            "attempts": len(present),
            "pass_at_1": rate(arm_cells, "pass_at_1"),
            "pass_at_n": rate(arm_cells, "pass_final"),
            "pass_at_n_bands_ab": rate(ab, "pass_final"),
            "median_repair_iters_ab": med_iters(ab),
            "total_solution_bytes": sum(bytes) if bytes else None,
            "total_tokens": sum(toks) if toks else None,
            "by_band": {b: {"pass_at_1": rate([c for c in arm_cells if c["band"] == b], "pass_at_1"),
                            "pass_at_n": rate([c for c in arm_cells if c["band"] == b], "pass_final")}
                        for b in ("a", "b", "c")},
        }

    verdict = decide(arms_summary)
    return {
        "schema_version": "x07.agent-eval.scored@0.1.0",
        "run_dir": str(run_dir),
        "models": models,
        "task_count": len(tasks["tasks"]),
        "arms": arms_summary,
        "verdict": verdict,
        "cells": cells,
    }


def decide(arms: dict) -> dict:
    """Apply the RUNBOOK predeclared go/park rule to the aggregated arms."""
    py = arms.get("python", {})
    py_pass = py.get("pass_at_n_bands_ab")
    py_iters = py.get("median_repair_iters_ab")
    if py_pass is None:
        return {"status": "insufficient_data", "reason": "no Python baseline on bands a+b"}

    bar = round(PASS_BAR * py_pass, 4)

    def arm_meets(arm):
        a = arms.get(arm, {})
        ap = a.get("pass_at_n_bands_ab")
        ai = a.get("median_repair_iters_ab")
        if ap is None:
            return False, f"{arm}: no data"
        ok_pass = ap >= bar
        ok_iter = (py_iters is None or ai is None) or (ai <= py_iters + ITER_SLACK)
        return (ok_pass and ok_iter), (
            f"{arm}: pass@6(a+b)={ap} vs bar {bar} ({'>=' if ok_pass else '<'}), "
            f"median_iters={ai} vs Python+{ITER_SLACK}={None if py_iters is None else py_iters+ITER_SLACK}"
        )

    x07_ok, x07_why = arm_meets("x07")
    x07t_ok, x07t_why = arm_meets("x07text")

    if x07_ok or x07t_ok:
        status = "bet_alive"
        action = "Direct-authoring bet stays alive; prioritize RFC 0002 and re-run."
    else:
        x = arms.get("x07", {}).get("pass_at_n_bands_ab")
        xt = arms.get("x07text", {}).get("pass_at_n_bands_ab")
        text_beats_json = (xt is not None and x is not None and xt > x)
        if text_beats_json:
            status = "keep_x07text_park_json"
            action = ("Keep x07text, park JSON-first authoring guidance, proceed "
                      "substrate-first.")
        else:
            status = "park_direct_authoring"
            action = ("Park the direct-authoring bet; X07 proceeds as a substrate "
                      "(transpile target + verification + sandbox); stop "
                      "language-surface work after RFC 0001.")
    return {
        "status": status,
        "action": action,
        "python_pass_at_n_bands_ab": py_pass,
        "bar_90pct": bar,
        "python_median_iters_ab": py_iters,
        "x07": x07_why,
        "x07text": x07t_why,
    }


def to_markdown(scored: dict) -> str:
    a = scored["arms"]
    lines = [
        f"# Comparative eval — scored ({scored['run_dir']})",
        "",
        f"Models: {', '.join(scored['models']) or '(none)'} · tasks: {scored['task_count']}",
        "",
        "| arm | attempts | pass@1 | pass@6 | pass@6 (a+b) | med repair iters (a+b) | total bytes |",
        "|---|---|---|---|---|---|---|",
    ]
    for arm in ARMS:
        s = a.get(arm, {})
        lines.append(
            f"| {arm} | {s.get('attempts', 0)} | {s.get('pass_at_1')} | {s.get('pass_at_n')} | "
            f"{s.get('pass_at_n_bands_ab')} | {s.get('median_repair_iters_ab')} | "
            f"{s.get('total_solution_bytes')} |"
        )
    v = scored["verdict"]
    lines += [
        "",
        "## Per-band pass@6",
        "",
        "| arm | band a | band b | band c |",
        "|---|---|---|---|",
    ]
    for arm in ARMS:
        bb = a.get(arm, {}).get("by_band", {})
        lines.append(
            f"| {arm} | {bb.get('a', {}).get('pass_at_n')} | "
            f"{bb.get('b', {}).get('pass_at_n')} | {bb.get('c', {}).get('pass_at_n')} |"
        )
    lines += [
        "",
        "## Verdict (predeclared decision rule)",
        "",
        f"**{v.get('status')}** — {v.get('action')}",
        "",
        f"- Python pass@6 on bands (a)+(b): {v.get('python_pass_at_n_bands_ab')} "
        f"(90% bar = {v.get('bar_90pct')}); Python median repair iters: {v.get('python_median_iters_ab')}",
        f"- {v.get('x07')}",
        f"- {v.get('x07text')}",
    ]
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True, help="run directory")
    ap.add_argument("--tasks", default=str(HERE / "tasks" / "tasks.json"))
    ap.add_argument("--out-json", default="")
    ap.add_argument("--out-md", default="")
    args = ap.parse_args(argv)

    run_dir = Path(args.run)
    if not run_dir.is_dir():
        raise SystemExit(f"run dir not found: {run_dir}")
    tasks = json.loads(Path(args.tasks).read_text())
    scored = score_run(run_dir, tasks)

    if args.out_json:
        Path(args.out_json).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out_json).write_text(json.dumps(scored, indent=2, sort_keys=True) + "\n")
    md = to_markdown(scored)
    if args.out_md:
        Path(args.out_md).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out_md).write_text(md)
    sys.stdout.write(md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
