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
        if (cur / "Cargo.toml").is_file() and (cur / "crates" / "x07-os-runner").is_dir():
            return cur
        if cur.parent == cur:
            break
        cur = cur.parent
    raise SystemExit("could not find repo root (expected Cargo.toml and crates/x07-os-runner/)")


def build_runner(root: Path) -> Path:
    candidates = [
        root / "target" / "debug" / ("x07-os-runner.exe" if os.name == "nt" else "x07-os-runner"),
        root / "target" / "release" / ("x07-os-runner.exe" if os.name == "nt" else "x07-os-runner"),
    ]
    for p in candidates:
        if p.is_file() and os.access(p, os.X_OK):
            return p
    proc = subprocess.run(
        ["cargo", "build", "-p", "x07-os-runner"],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or "cargo build -p x07-os-runner failed")
    for p in candidates:
        if p.is_file() and os.access(p, os.X_OK):
            return p
    raise RuntimeError("x07-os-runner binary missing after build")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument("--help", action="store_true")
    args, passthrough = ap.parse_known_args(argv)

    root = find_repo_root(Path(__file__).resolve())
    try:
        runner = build_runner(root)
    except Exception as e:
        out = {"ok": False, "runner_exit_code": 2, "error": str(e)}
        print(json.dumps(out, ensure_ascii=False, sort_keys=True))
        return 2

    cmd = [str(runner)] + (["--help"] if args.help else passthrough)
    proc = subprocess.run(cmd, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout = (proc.stdout or "").strip()
    stderr = (proc.stderr or "").strip()

    report: Any = None
    if stdout:
        try:
            report = json.loads(stdout)
        except Exception:
            report = None

    out = {
        "ok": proc.returncode == 0 and isinstance(report, dict) and bool(report.get("ok", True)),
        "runner_exit_code": proc.returncode,
        "report": report,
    }
    if not out["ok"] and stderr:
        out["stderr"] = stderr[-4000:]
    if report is None and stdout:
        out["stdout"] = stdout[-4000:]

    print(json.dumps(out, ensure_ascii=False, sort_keys=True))
    return 0 if out["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

