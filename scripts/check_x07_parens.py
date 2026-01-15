#!/usr/bin/env python3
"""
CI guard: ensure all checked-in X07 sources are x07AST JSON (`*.x07.json`) and canonically formatted.

Design goals:
  - deterministic output (stable file order, stable diagnostics)
  - avoid re-implementing X07 formatting/parsing logic

Usage:
  python3 scripts/check_x07_parens.py
  python3 scripts/check_x07_parens.py stdlib benchmarks
  python3 scripts/check_x07_parens.py --glob 'stdlib/**/modules/**/*.x07.json'
"""

from __future__ import annotations

import argparse
import glob
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import List


DEFAULT_ROOTS = [
    "stdlib",
    "examples",
    "benchmarks/solutions",
    "tests",
]

SKIP_DIRS = {
    ".git",
    "target",
    "artifacts",
    "deps",
    "__pycache__",
}


@dataclass(frozen=True)
class ParenError:
    path: str
    code: str
    message: str

    def format(self) -> str:
        return f"{self.path}: {self.code} {self.message}"


def _should_skip_dir(dirpath: Path) -> bool:
    tail = dirpath.name
    if tail in SKIP_DIRS:
        return True
    rel = str(dirpath).replace(os.sep, "/")
    for s in SKIP_DIRS:
        if "/" in s and rel.endswith(s):
            return True
    return False


def iter_x07_json_files(paths: List[str], glob_pat: str | None) -> List[Path]:
    out: List[Path] = []

    if glob_pat:
        for p in sorted(glob.glob(glob_pat, recursive=True)):
            pp = Path(p)
            if pp.is_file() and pp.name.endswith(".x07.json"):
                out.append(pp)
        return out

    roots = paths if paths else DEFAULT_ROOTS
    for root in roots:
        rp = Path(root)
        if not rp.exists():
            continue
        if rp.is_file():
            if rp.name.endswith(".x07.json"):
                out.append(rp)
            continue

        for dirpath, dirnames, filenames in os.walk(rp):
            dp = Path(dirpath)
            if _should_skip_dir(dp):
                dirnames[:] = []
                continue

            dirnames[:] = sorted(dirnames)
            for fn in sorted(filenames):
                if fn.endswith(".x07.json"):
                    out.append(dp / fn)

    return sorted(out, key=lambda p: str(p).replace(os.sep, "/"))


def iter_non_json_sexpr_sources(paths: List[str]) -> List[Path]:
    roots = paths if paths else DEFAULT_ROOTS
    out: List[Path] = []

    for root in roots:
        rp = Path(root)
        if not rp.exists():
            continue
        if rp.is_file():
            if rp.name.endswith(".sexpr"):
                out.append(rp)
            continue

        for dirpath, dirnames, filenames in os.walk(rp):
            dp = Path(dirpath)
            if _should_skip_dir(dp):
                dirnames[:] = []
                continue

            dirnames[:] = sorted(dirnames)
            for fn in sorted(filenames):
                if fn.endswith(".sexpr"):
                    out.append(dp / fn)

    return sorted(out, key=lambda p: str(p).replace(os.sep, "/"))


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _x07c_bin() -> Path | None:
    # Prefer explicit override for CI.
    env = os.environ.get("X07C_BIN") or ""
    if env.strip():
        p = Path(env)
        if p.is_file() and os.access(p, os.X_OK):
            return p

    root = _repo_root()
    names = ["x07c"]
    if sys.platform.startswith("win"):
        names.append("x07c.exe")

    cand = [
        *(root / "target" / "debug" / name for name in names),
        *(root / "target" / "release" / name for name in names),
    ]
    for p in cand:
        if p.is_file() and os.access(p, os.X_OK):
            return p
    return None


def _ensure_x07c() -> Path:
    root = _repo_root()
    p = _x07c_bin()
    if p is not None:
        return p

    # Build once for all checks (avoid duplicated formatting logic in Python).
    res = subprocess.run(
        ["cargo", "build", "-p", "x07c"],
        cwd=str(root),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if res.returncode != 0:
        raise RuntimeError(
            "failed to build x07c:\n"
            f"stdout:\n{res.stdout}\n"
            f"stderr:\n{res.stderr}\n"
        )

    p = _x07c_bin()
    if p is None:
        raise RuntimeError("x07c binary not found after build")
    return p


def check_x07ast_format(x07c_bin: Path, path: Path) -> List[ParenError]:
    res = subprocess.run(
        [
            str(x07c_bin),
            "fmt",
            "--input",
            str(path),
            "--check",
        ],
        cwd=str(_repo_root()),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if res.returncode == 0:
        return []

    msg = (res.stderr or res.stdout).strip() or "format check failed"
    return [
        ParenError(
            path=str(path).replace(os.sep, "/"),
            code="X07FMT",
            message=msg,
        )
    ]


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Check X07 sources: forbid *.sexpr (legacy S-expr), enforce canonical *.x07.json formatting."
    )
    ap.add_argument("paths", nargs="*", help="Files/dirs to scan (default: stdlib/)")
    ap.add_argument(
        "--glob", dest="glob_pat", default=None, help="Glob pattern for .x07.json files"
    )
    args = ap.parse_args()

    files = iter_x07_json_files(args.paths, args.glob_pat)
    non_json = [] if args.glob_pat else iter_non_json_sexpr_sources(args.paths)
    if non_json:
        for p in non_json:
            rel = str(p).replace(os.sep, "/")
            print(f"{rel}: X07SRC non-JSON X07 source is forbidden; use *.x07.json", file=sys.stderr)
        print(
            f"check_x07_parens: FAIL ({len(non_json)} forbidden *.sexpr file(s))",
            file=sys.stderr,
        )
        return 1

    if not files:
        return 0

    try:
        x07c_bin = _ensure_x07c()
    except Exception as e:
        print(f"check_x07_parens: FAIL (cannot build x07c): {e}", file=sys.stderr)
        return 2

    all_errs: List[ParenError] = []
    for f in files:
        all_errs.extend(check_x07ast_format(x07c_bin, f))

    if all_errs:
        for e in all_errs:
            print(e.format(), file=sys.stderr)
        print(
            f"check_x07_parens: FAIL ({len(all_errs)} error(s) across {len(files)} file(s))",
            file=sys.stderr,
        )
        return 1

    print(f"check_x07_parens: OK ({len(files)} file(s))")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
