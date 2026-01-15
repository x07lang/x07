#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


def find_repo_root(start: Path) -> Path:
    cur = start.resolve()
    for _ in range(20):
        if (cur / "Cargo.toml").is_file() and (cur / "scripts" / "ci" / "find_x07c.sh").is_file():
            return cur
        if cur.parent == cur:
            break
        cur = cur.parent
    raise SystemExit("could not find repo root (expected Cargo.toml and scripts/ci/find_x07c.sh)")


def find_x07c(root: Path) -> str:
    proc = subprocess.run(
        ["bash", "-lc", "./scripts/ci/find_x07c.sh"],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or "find_x07c.sh failed")
    return proc.stdout.strip()


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--program")
    ap.add_argument("--project")
    ap.add_argument("--world", default=os.environ.get("X07_WORLD", "solve-pure"))
    ap.add_argument("--module-root", action="append", default=[])
    ap.add_argument("--out-c", required=True)
    ap.add_argument("--out-h", required=True)
    ap.add_argument("--x07c", help="Path to x07c (default: scripts/ci/find_x07c.sh).")
    args = ap.parse_args(argv)

    if (args.program is None) == (args.project is None):
        print("usage: gen_header.py (--program <file> | --project <x07.json>) --out-c <path> --out-h <path>", file=sys.stderr)
        return 2

    root = find_repo_root(Path(__file__).resolve())
    x07c = args.x07c or os.environ.get("X07C") or find_x07c(root)

    out_c = Path(args.out_c)
    out_h = Path(args.out_h)
    out_c.parent.mkdir(parents=True, exist_ok=True)
    out_h.parent.mkdir(parents=True, exist_ok=True)

    if args.project is not None:
        cmd = [
            x07c,
            "build",
            "--project",
            str(Path(args.project)),
            "--out",
            str(out_c),
            "--emit-c-header",
            str(out_h),
        ]
    else:
        cmd = [
            x07c,
            "compile",
            "--program",
            str(Path(args.program)),
            "--world",
            args.world,
            "--out",
            str(out_c),
            "--emit-c-header",
            str(out_h),
        ]
        for mr in args.module_root:
            cmd.extend(["--module-root", str(Path(mr))])

    proc = subprocess.run(cmd, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    ok = proc.returncode == 0 and out_c.is_file() and out_h.is_file()
    out: dict[str, Any] = {
        "ok": ok,
        "exit_code": proc.returncode,
        "cmd": cmd,
        "out_c": str(out_c),
        "out_h": str(out_h),
    }
    if not ok:
        out["stderr"] = (proc.stderr or "")[-4000:]
        out["stdout"] = (proc.stdout or "")[-4000:]
    print(json.dumps(out, ensure_ascii=False, sort_keys=True))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

