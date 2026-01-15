#!/usr/bin/env python3
"""
Deterministic build+run wrapper for X07 (C backend).

This wrapper is intended for autonomous agents: it emits one JSON report to stdout.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import subprocess
import time
from pathlib import Path
from typing import Any, Dict, List, Tuple


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


def try_compile(
    x07c: str,
    program: Path,
    module_roots: List[Path],
    world: str,
    out_c: Path,
) -> Tuple[bool, Dict[str, Any]]:
    base = [x07c, "compile", "--program", str(program), "--world", world, "--out", str(out_c)]
    cmd: List[str] = base[:]
    for root in module_roots:
        cmd.extend(["--module-root", str(root)])

    try:
        p = subprocess.run(cmd, capture_output=True, text=True)
    except FileNotFoundError:
        return False, {
            "cmd": cmd,
            "exit_code": 127,
            "stdout": "",
            "stderr": f"x07c not found: {x07c}",
        }

    return (p.returncode == 0), {"cmd": cmd, "exit_code": p.returncode, "stdout": p.stdout, "stderr": p.stderr}


def cc_compile(cc: str, c_path: Path, exe_path: Path, cflags: List[str]) -> Tuple[bool, Dict[str, Any]]:
    cmd = [cc, "-std=c11", "-O2", "-o", str(exe_path), str(c_path)] + cflags
    try:
        p = subprocess.run(cmd, capture_output=True, text=True)
    except FileNotFoundError:
        return False, {"cmd": cmd, "exit_code": 127, "stdout": "", "stderr": f"C compiler not found: {cc}"}
    return (p.returncode == 0), {"cmd": cmd, "exit_code": p.returncode, "stdout": p.stdout, "stderr": p.stderr}


def run_exe(exe_path: Path, stdin_bytes: bytes, timeout_ms: int) -> Tuple[bool, Dict[str, Any]]:
    t0 = time.time()
    try:
        p = subprocess.run([str(exe_path)], input=stdin_bytes, capture_output=True, timeout=timeout_ms / 1000.0)
        dt = time.time() - t0
        return (p.returncode == 0), {
            "cmd": [str(exe_path)],
            "exit_code": p.returncode,
            "time_s": dt,
            "stdout_b64": base64.b64encode(p.stdout).decode("ascii"),
            "stderr": (p.stderr or b"").decode("utf-8", errors="replace")[:4000],
        }
    except subprocess.TimeoutExpired as e:
        dt = time.time() - t0
        return False, {
            "cmd": [str(exe_path)],
            "exit_code": 124,
            "time_s": dt,
            "stdout_b64": base64.b64encode(e.stdout or b"").decode("ascii"),
            "stderr": ("TIMEOUT\n" + ((e.stderr or b"").decode("utf-8", errors="replace")))[:4000],
        }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--program", default="src/main.x07.json")
    ap.add_argument("--module-root", action="append", default=["src"])
    ap.add_argument("--world", default=os.environ.get("X07_WORLD", "solve-pure"))
    ap.add_argument("--stdin", help="Optional input file passed to program stdin.")
    ap.add_argument("--out-dir", default="target/x07-build")
    ap.add_argument("--x07c", help="Path to x07c (default: scripts/ci/find_x07c.sh).")
    ap.add_argument("--cc", default=os.environ.get("X07_CC", os.environ.get("CC", "cc")))
    ap.add_argument("--cflags", default="", help="Extra flags, e.g. '-g -O0'.")
    ap.add_argument("--timeout-ms", type=int, default=5000)
    ap.add_argument("--write-stdout", help="Write raw stdout bytes to this path.")
    args = ap.parse_args()

    root = find_repo_root(Path(__file__).resolve())

    x07c = args.x07c or os.environ.get("X07C") or find_x07c(root)

    program = Path(args.program)
    if not program.is_absolute():
        program = root / program

    module_roots = [Path(p) for p in args.module_root]
    module_roots = [p if p.is_absolute() else (root / p) for p in module_roots]

    out_dir = Path(args.out_dir)
    if not out_dir.is_absolute():
        out_dir = root / out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    report: Dict[str, Any] = {"compile": {}, "cc": {}, "run": {}}

    if not program.exists():
        report["compile"] = {"ok": False, "exit_code": 2, "error": f"program not found: {str(program)}"}
        print(json.dumps(report, sort_keys=True))
        return 2

    c_path = out_dir / "program.c"
    exe_name = "program.exe" if os.name == "nt" else "program"
    exe_path = out_dir / exe_name

    ok, comp = try_compile(x07c, program, module_roots, args.world, c_path)
    report["compile"] = {
        "ok": ok,
        "exit_code": comp.get("exit_code"),
        "cmd": comp.get("cmd"),
        "stdout": (comp.get("stdout") or "")[-4000:],
        "stderr": (comp.get("stderr") or "")[-4000:],
        "c_path": str(c_path) if c_path.exists() else None,
    }
    if not ok:
        print(json.dumps(report, sort_keys=True))
        return 1

    ok2, cc_rep = cc_compile(args.cc, c_path, exe_path, args.cflags.split() if args.cflags.strip() else [])
    report["cc"] = {
        "ok": ok2,
        "exit_code": cc_rep.get("exit_code"),
        "cmd": cc_rep.get("cmd"),
        "stdout": (cc_rep.get("stdout") or "")[-4000:],
        "stderr": (cc_rep.get("stderr") or "")[-4000:],
        "exe_path": str(exe_path) if exe_path.exists() else None,
    }
    if not ok2:
        print(json.dumps(report, sort_keys=True))
        return 1

    stdin_bytes = b""
    if args.stdin:
        stdin_path = Path(args.stdin)
        if not stdin_path.is_absolute():
            stdin_path = root / stdin_path
        stdin_bytes = stdin_path.read_bytes()

    ok3, run_rep = run_exe(exe_path, stdin_bytes, args.timeout_ms)
    report["run"] = run_rep
    report["run"]["ok"] = ok3

    if args.write_stdout and "stdout_b64" in run_rep:
        out_bytes = base64.b64decode(run_rep["stdout_b64"].encode("ascii"))
        write_path = Path(args.write_stdout)
        if not write_path.is_absolute():
            write_path = root / write_path
        write_path.write_bytes(out_bytes)

    print(json.dumps(report, sort_keys=True))
    return 0 if (ok and ok2 and ok3) else 1


if __name__ == "__main__":
    raise SystemExit(main())
