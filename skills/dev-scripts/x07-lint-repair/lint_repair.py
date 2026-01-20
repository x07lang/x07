#!/usr/bin/env python3
"""
Deterministic lint + JSON Patch apply wrapper for X07 x07AST JSON files.

Design goals:
- No network.
- Stable JSON output (single object printed to stdout).
- Patch application uses the canonical toolchain semantics (`x07 ast apply-patch`).
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


X07DIAG_SCHEMA_VERSION = "x07.x07diag@0.1.0"


def find_repo_root(start: Path) -> Path:
    cur = start.resolve()
    for _ in range(20):
        if (cur / "Cargo.toml").is_file() and (cur / "scripts" / "ci" / "find_x07.sh").is_file():
            return cur
        if cur.parent == cur:
            break
        cur = cur.parent
    raise SystemExit("could not find repo root (expected Cargo.toml and scripts/ci/find_x07.sh)")


def find_x07(root: Path) -> str:
    proc = subprocess.run(
        ["bash", "-lc", "./scripts/ci/find_x07.sh"],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or "find_x07.sh failed")
    return proc.stdout.strip()


def x07diag_error(code: str, stage: str, message: str, ptr: Optional[str] = None) -> Dict[str, Any]:
    diag: Dict[str, Any] = {
        "code": code,
        "severity": "error",
        "stage": stage,
        "message": message[:4096],
    }
    if ptr is not None:
        diag["loc"] = {"kind": "x07ast", "ptr": ptr}
    return {"schema_version": X07DIAG_SCHEMA_VERSION, "ok": False, "diagnostics": [diag], "meta": {}}


def run_x07_lint(x07: str, world: str, path: Path) -> Tuple[int, str, str]:
    cmd = [x07, "lint", "--input", str(path), "--world", world, "--report-json"]
    try:
        p = subprocess.run(cmd, capture_output=True, text=True)
    except FileNotFoundError:
        return 127, "", f"x07 not found: {x07}"
    return p.returncode, p.stdout, p.stderr


def tool_report_to_x07diag(tool_report: Dict[str, Any]) -> Dict[str, Any]:
    diagnostics = tool_report.get("diagnostics")
    ok = tool_report.get("ok")
    if not isinstance(diagnostics, list) or not isinstance(ok, bool):
        return x07diag_error("X07LINT_BAD_JSON", "lint", "x07 lint --report-json returned unexpected JSON")
    meta = {
        "tool": "x07",
        "tool_schema_version": tool_report.get("schema_version"),
        "tool_command": tool_report.get("command"),
        "tool_in": tool_report.get("in"),
        "tool_exit_code": tool_report.get("exit_code"),
    }
    return {"schema_version": X07DIAG_SCHEMA_VERSION, "ok": ok, "diagnostics": diagnostics, "meta": meta}


def apply_patch(x07: str, program: Path, patch: Path, out_path: Path) -> Tuple[int, str, str]:
    cmd = [x07, "ast", "apply-patch", "--in", str(program), "--patch", str(patch), "--out", str(out_path), "--validate"]
    try:
        p = subprocess.run(cmd, capture_output=True, text=True)
    except FileNotFoundError:
        return 127, "", f"x07 not found: {x07}"
    return p.returncode, p.stdout, p.stderr


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", required=True, help="Path to x07AST JSON file (e.g., src/main.x07.json).")
    ap.add_argument("--world", default=os.environ.get("X07_WORLD", "solve-pure"))
    ap.add_argument("--apply-patch", help="Path to JSON Patch file (RFC 6902) to apply to the x07AST.")
    ap.add_argument("--in-place", action="store_true", help="Write patched x07AST back to --file.")
    ap.add_argument("--x07", help="Path to x07 (default: scripts/ci/find_x07.sh).")
    args = ap.parse_args()

    root = find_repo_root(Path(__file__).resolve())
    x07 = args.x07 or os.environ.get("X07") or find_x07(root)

    file_path = Path(args.file)
    if not file_path.is_absolute():
        file_path = root / file_path
    out: Dict[str, Any] = {
        "ok": False,
        "lint_exit_code": None,
        "diagnostics": None,
        "applied_patch": False,
        "files_modified": [],
    }

    if not file_path.exists():
        out["diagnostics"] = x07diag_error(
            "X07FILE_NOT_FOUND",
            "parse",
            f"File does not exist: {str(file_path)}",
        )
        print(json.dumps(out, ensure_ascii=False, sort_keys=True))
        return 2

    lint_target = file_path

    if args.apply_patch:
        patch_path = Path(args.apply_patch)
        if not patch_path.is_absolute():
            patch_path = root / patch_path
        if not patch_path.exists():
            out["diagnostics"] = x07diag_error(
                "X07PATCH_NOT_FOUND",
                "rewrite",
                f"Patch file does not exist: {str(patch_path)}",
            )
            print(json.dumps(out, ensure_ascii=False, sort_keys=True))
            return 2

        if args.in_place:
            out_path = file_path
        else:
            tmp_dir = Path(tempfile.mkdtemp(prefix="x07_lint_repair_"))
            out_path = tmp_dir / file_path.name
            lint_target = out_path

        code, _stdout, stderr = apply_patch(x07, file_path, patch_path, out_path)
        if code != 0:
            out["diagnostics"] = x07diag_error(
                "X07PATCH_APPLY_FAILED",
                "rewrite",
                (stderr.strip() or f"x07 ast apply-patch failed with exit_code={code}")[:4096],
            )
            print(json.dumps(out, ensure_ascii=False, sort_keys=True))
            return 1

        out["applied_patch"] = True
        if args.in_place:
            out["files_modified"].append(str(file_path))

    exit_code, stdout, stderr = run_x07_lint(x07, args.world, lint_target)
    out["lint_exit_code"] = exit_code

    if stdout.strip():
        try:
            tool_report = json.loads(stdout)
        except Exception:
            out["diagnostics"] = x07diag_error(
                "X07LINT_NON_JSON",
                "lint",
                "x07 lint did not emit JSON. See stderr for details.",
            )
        else:
            if isinstance(tool_report, dict):
                out["diagnostics"] = tool_report_to_x07diag(tool_report)
            else:
                out["diagnostics"] = x07diag_error("X07LINT_BAD_JSON", "lint", "x07 lint --report-json returned non-object JSON")
    else:
        msg = stderr.strip() or f"x07 lint emitted empty stdout; exit_code={exit_code}"
        out["diagnostics"] = x07diag_error("X07LINT_EMPTY_STDOUT", "lint", msg)

    out["ok"] = bool(out.get("diagnostics", {}).get("ok") is True) and exit_code == 0

    print(json.dumps(out, ensure_ascii=False, sort_keys=True))
    return 0 if out["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
