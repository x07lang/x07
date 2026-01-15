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


def iter_x07_files(paths: list[Path]) -> list[Path]:
    out: list[Path] = []
    for p in paths:
        if p.is_file():
            if p.name.endswith(".x07.json"):
                out.append(p)
            continue
        if p.is_dir():
            for fp in sorted(p.rglob("*.x07.json")):
                if fp.is_file():
                    out.append(fp)
    # Deterministic order and dedup.
    uniq: dict[str, Path] = {}
    for p in out:
        uniq[str(p.resolve())] = p
    return [uniq[k] for k in sorted(uniq.keys())]


def run_fmt(x07c: str, file_path: Path, mode: str) -> tuple[int, dict[str, Any]]:
    cmd = [x07c, "fmt", "--input", str(file_path), f"--{mode}", "--report-json"]
    proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if proc.returncode == 0:
        return 0, {}
    err = (proc.stderr or "").strip()
    out = (proc.stdout or "").strip()
    return proc.returncode, {"cmd": cmd, "stdout": out[-4000:], "stderr": err[-4000:]}


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--path", action="append", default=[], help="File or directory to scan (repeatable).")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--x07c", help="Path to x07c (default: scripts/ci/find_x07c.sh).")
    args = ap.parse_args(argv)

    if args.check == args.write:
        print("usage: format.py --path <path>... (--check | --write)", file=sys.stderr)
        return 2

    root = find_repo_root(Path(__file__).resolve())

    mode = "check" if args.check else "write"
    scan_paths = [Path(p) for p in (args.path or ["src", "tests", "benchmarks", "packages", "stdlib"])]
    files = iter_x07_files([root / p if not Path(p).is_absolute() else Path(p) for p in scan_paths])

    x07c = args.x07c or os.environ.get("X07C") or find_x07c(root)

    failed: list[dict[str, Any]] = []
    for f in files:
        code, info = run_fmt(x07c, f, mode)
        if code != 0:
            failed.append({"file": str(f), "exit_code": code, **info})

    out = {
        "ok": len(failed) == 0,
        "mode": mode,
        "files": [str(p) for p in files],
        "failed": failed,
    }
    print(json.dumps(out, ensure_ascii=False, sort_keys=True))
    return 0 if out["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

