#!/usr/bin/env python3
"""Render the exact cold-start prompt a model receives for one (task, arm).

Implements the RUNBOOK prompting protocol: one shared template (the task prompt
+ the arm's bytes-in/bytes-out I/O contract) plus arm-specific minimal context.
For the two X07 arms the context is *exactly* `x07 guide` output and the
doc-tool usage line and nothing else, so the X07 arms are not advantaged by
extra hand-holding.

Usage:
  python3 render_prompt.py --task rot13 --arm python
  X07_BIN=~/.x07/bin/x07 python3 render_prompt.py --task rot13 --arm x07text
  python3 render_prompt.py --task rot13 --arm x07 --out prompts/x07/rot13.txt

The rendered prompt is what you paste into a fresh model session. Keep the
session cold (no prior repo context). Allow the model up to N=6 tool iterations
(x07 doc / x07 lint / x07 run) for the X07 arms; record attempt-1 and final.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent

ARMS = ("python", "rust", "x07", "x07text")

IO_CONTRACT = {
    "python": (
        "Write a single-file Python 3 program using only the standard library. "
        "It reads ALL bytes from stdin and writes the result bytes to stdout. "
        "Emit only the program source."
    ),
    "rust": (
        "Write a single-file Rust program (edition 2021, standard library only, "
        "compiled with `rustc -O`). It reads ALL bytes from stdin and writes the "
        "result bytes to stdout. Emit only the program source."
    ),
    "x07": (
        "Write a single X07 solve-pure program as one `*.x07.json` file "
        "(`x07.x07ast@0.8.0` schema, `\"kind\":\"entry\"`). The program receives "
        "the input bytes as the identifier `input` and must return the output "
        "bytes (its `solve` expression). Emit only the JSON program."
    ),
    "x07text": (
        "Write a single X07 solve-pure program authored as x07text (a `.x07t` "
        "file; it is converted to canonical x07AST via `x07 ast from-text`). The "
        "program receives the input bytes as the identifier `input` and must "
        "return the output bytes (its `solve` expression). Emit only the x07text source."
    ),
}

X07_DOC_LINE = (
    "Discover stdlib APIs with `x07 doc <module-or-symbol-or-keyword>` (a bare "
    "keyword lists matching modules), check with `x07 check`, and run with "
    "`x07 run --program <file> --world solve-pure --input-b64 <b64>`; you may use "
    "up to 6 tool iterations to reach a passing solution."
)


def x07_guide() -> str:
    x07_bin = os.environ.get("X07_BIN", "x07")
    try:
        proc = subprocess.run(
            [x07_bin, "guide"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60
        )
    except FileNotFoundError:
        raise SystemExit(
            f"X07 arm needs the guide: set X07_BIN to an x07 binary (got {x07_bin!r})"
        )
    if proc.returncode != 0:
        raise SystemExit(f"`{x07_bin} guide` failed: {proc.stderr.decode(errors='replace')[:300]}")
    return proc.stdout.decode()


def render(task: dict, arm: str) -> str:
    parts = [
        f"# Task: {task['title']}",
        "",
        task["prompt"],
        "",
        "## Contract",
        IO_CONTRACT[arm],
        "",
        "The solution is judged on exact bytes-out for every hidden test vector; "
        "match the specification precisely, including edge cases (empty input, "
        "single element, ties, trailing newlines).",
    ]
    if arm in ("x07", "x07text"):
        parts += [
            "",
            "## Tooling",
            X07_DOC_LINE,
            "",
            "## X07 language reference (`x07 guide`)",
            "",
            x07_guide().rstrip(),
        ]
    return "\n".join(parts) + "\n"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks", default=str(HERE / "tasks" / "tasks.json"))
    ap.add_argument("--task", required=True, help="task id")
    ap.add_argument("--arm", required=True, choices=ARMS)
    ap.add_argument("--out", default="", help="write to this file instead of stdout")
    args = ap.parse_args(argv)

    tasks = json.loads(Path(args.tasks).read_text())
    by_id = {t["id"]: t for t in tasks["tasks"]}
    if args.task not in by_id:
        raise SystemExit(f"unknown task: {args.task} (have {len(by_id)} tasks)")

    prompt = render(by_id[args.task], args.arm)
    if args.out:
        out = Path(args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(prompt)
        print(f"wrote {out} ({len(prompt)} chars)", file=sys.stderr)
    else:
        sys.stdout.write(prompt)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
