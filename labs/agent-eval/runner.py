#!/usr/bin/env python3
"""Agent-eval runner: execute candidate solutions against task vectors.

Deterministic, offline, stdlib-only. Solutions are judged purely on
bytes-out == expected for every vector.

Layout:
  tasks/tasks.json                  task suite (prompts + vectors)
  solutions/<subject>/<task_id>.py        python solution (stdin -> stdout)
  solutions/<subject>/<task_id>.x07.json  x07 solve-pure program (input -> solve bytes)

Usage:
  python3 runner.py --lang python --solutions solutions/claude-pilot --results results/out.json
  X07_BIN=/path/to/x07 python3 runner.py --lang x07 --solutions solutions/claude-pilot
"""

import argparse
import base64
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
TIMEOUT_SECONDS = 60


def run_python(solution: Path, input_bytes: bytes):
    proc = subprocess.run(
        [sys.executable, str(solution)],
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=TIMEOUT_SECONDS,
    )
    if proc.returncode != 0:
        return None, f"exit {proc.returncode}: {proc.stderr.decode(errors='replace')[:300]}"
    return proc.stdout, None


def run_x07(solution: Path, input_bytes: bytes):
    x07_bin = os.environ.get("X07_BIN", "x07")
    proc = subprocess.run(
        [
            x07_bin,
            "run",
            "--program",
            str(solution),
            "--world",
            "solve-pure",
            "--input-b64",
            base64.b64encode(input_bytes).decode(),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=TIMEOUT_SECONDS,
    )
    try:
        report = json.loads(proc.stdout.decode())
    except json.JSONDecodeError:
        return None, f"unparseable runner report: {proc.stdout[:200]!r}"
    if proc.returncode != 0:
        compile_error = (report.get("compile") or {}).get("compile_error")
        diags = (report.get("compile") or {}).get("diagnostics") or []
        codes = ",".join(d.get("code", "?") for d in diags[:3])
        return None, f"compile/run failed: {str(compile_error)[:200]} [{codes}]"
    solve = report.get("solve") or {}
    out_b64 = solve.get("solve_output_b64")
    if out_b64 is None:
        return None, "report has no solve_output_b64"
    return base64.b64decode(out_b64), None


def evaluate(lang: str, solutions_dir: Path, tasks: dict):
    runners = {"python": (run_python, ".py"), "x07": (run_x07, ".x07.json")}
    run_fn, ext = runners[lang]
    rows = []
    for task in tasks["tasks"]:
        task_id = task["id"]
        solution = solutions_dir / f"{task_id}{ext}"
        row = {
            "id": task_id,
            "status": "missing",
            "vectors_passed": 0,
            "vectors_total": len(task["vectors"]),
            "solution_bytes": None,
            "wall_ms": None,
            "first_failure": None,
        }
        if solution.is_file():
            row["solution_bytes"] = solution.stat().st_size
            started = time.monotonic()
            status = "pass"
            for i, vector in enumerate(task["vectors"]):
                input_bytes = vector["input"].encode()
                expected = vector["expected"].encode()
                try:
                    got, err = run_fn(solution, input_bytes)
                except subprocess.TimeoutExpired:
                    got, err = None, "timeout"
                if err is not None:
                    status = "error"
                    row["first_failure"] = {"vector": i, "error": err}
                    break
                if got != expected:
                    status = "fail"
                    row["first_failure"] = {
                        "vector": i,
                        "expected": expected.decode(errors="replace"),
                        "got": got.decode(errors="replace")[:200],
                    }
                    break
                row["vectors_passed"] += 1
            row["status"] = status
            row["wall_ms"] = int((time.monotonic() - started) * 1000)
        rows.append(row)

    passed = sum(1 for r in rows if r["status"] == "pass")
    return {
        "schema_version": "x07.agent-eval.results@0.1.0",
        "lang": lang,
        "solutions_dir": str(solutions_dir),
        "tasks": rows,
        "summary": {
            "passed": passed,
            "total": len(rows),
            "pass_rate": round(passed / len(rows), 4) if rows else 0.0,
        },
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks", default=str(HERE / "tasks" / "tasks.json"))
    ap.add_argument("--lang", choices=["python", "x07"], required=True)
    ap.add_argument("--solutions", required=True)
    ap.add_argument("--results", default="")
    args = ap.parse_args()

    tasks = json.loads(Path(args.tasks).read_text())
    results = evaluate(args.lang, Path(args.solutions), tasks)
    payload = json.dumps(results, indent=2, sort_keys=True) + "\n"
    if args.results:
        out = Path(args.results)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(payload)
    sys.stdout.write(payload)
    return 0 if results["summary"]["passed"] == results["summary"]["total"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
