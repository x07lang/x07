#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _find_x07_bin(root: Path) -> Path:
    override = os.environ.get("X07_BIN", "").strip()
    if override:
        p = Path(override)
        if p.is_file() and (os.name == "nt" or os.access(p, os.X_OK)):
            return p
        raise SystemExit(f"ERROR: X07_BIN is set but not executable: {override}")

    try:
        proc = subprocess.run(
            ["bash", "-c", "./scripts/ci/find_x07.sh"],
            cwd=str(root),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except FileNotFoundError:
        if os.name == "nt":
            raise SystemExit("ERROR: bash is required to run scripts/ci/find_x07.sh on Windows")
        proc = subprocess.run(
            [str(root / "scripts/ci/find_x07.sh")],
            cwd=str(root),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    if proc.returncode != 0:
        stdout = proc.stdout.rstrip()
        stderr = proc.stderr.rstrip()
        raise SystemExit(
            "ERROR: scripts/ci/find_x07.sh failed:\n"
            f"exit={proc.returncode}\n"
            f"stdout:\n{stdout if stdout else '<empty>'}\n"
            f"stderr:\n{stderr if stderr else '<empty>'}\n"
        )
    out = proc.stdout.strip()
    if not out:
        raise SystemExit("ERROR: scripts/ci/find_x07.sh produced empty output")
    if os.name == "nt" and len(out) >= 3 and out[0] == "/" and out[2] == "/" and out[1].isalpha():
        out = f"{out[1].upper()}:{out[2:]}".replace("/", "\\")
    p = (root / out).resolve() if not Path(out).is_absolute() else Path(out).resolve()
    if not p.is_file():
        raise SystemExit(f"ERROR: x07 binary not found: {p}")
    return p


def _run_json(*cmd: str, cwd: Path) -> tuple[int, Any]:
    proc = subprocess.run(
        list(cmd),
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        doc = json.loads(proc.stdout.strip() or "{}")
    except Exception as e:
        raise SystemExit(
            f"ERROR: failed to parse JSON stdout from {' '.join(cmd)} (exit={proc.returncode}): {e}\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}\n"
        )
    return proc.returncode, doc


def _assert_actionable_diag(doc: Any, *, want_code: str) -> None:
    if not isinstance(doc, dict):
        raise SystemExit(f"ERROR: lint output is not an object: {type(doc)}")
    if doc.get("schema_version") != "x07.x07diag@0.1.0":
        raise SystemExit(f"ERROR: lint output schema_version mismatch: {doc.get('schema_version')!r}")
    if doc.get("ok") is not False:
        raise SystemExit(f"ERROR: expected ok=false, got: {doc.get('ok')!r}")

    diagnostics = doc.get("diagnostics")
    if not isinstance(diagnostics, list):
        raise SystemExit("ERROR: lint output missing diagnostics[]")

    for d in diagnostics:
        if not isinstance(d, dict):
            continue
        if d.get("code") != want_code:
            continue

        loc = d.get("loc")
        if not isinstance(loc, dict) or loc.get("kind") != "x07ast" or not (loc.get("ptr") or "").strip():
            raise SystemExit(f"ERROR: {want_code}: expected loc.kind=x07ast with non-empty ptr, got: {loc!r}")

        notes = d.get("notes") or []
        if not isinstance(notes, list) or not any(isinstance(n, str) and "Suggested fix:" in n for n in notes):
            raise SystemExit(f"ERROR: {want_code}: expected notes[] containing 'Suggested fix:', got: {notes!r}")

        q = d.get("quickfix")
        if not isinstance(q, dict) or q.get("kind") != "json_patch" or not isinstance(q.get("patch"), list) or len(q["patch"]) == 0:
            raise SystemExit(f"ERROR: {want_code}: expected json_patch quickfix with non-empty patch[], got: {q!r}")

        return

    raise SystemExit(f"ERROR: lint output did not include expected code: {want_code}")


def main() -> int:
    root = _repo_root()
    x07 = _find_x07_bin(root)

    cases = [
        (
            "borrow_from_temporary",
            "X07-BORROW-0001",
            {
                "schema_version": "x07.x07ast@0.2.0",
                "kind": "entry",
                "module_id": "main",
                "imports": [],
                "decls": [],
                "solve": ["bytes.view", ["bytes.lit", "hello"]],
            },
        ),
        (
            "use_after_move_bytes_concat",
            "X07-MOVE-0001",
            {
                "schema_version": "x07.x07ast@0.2.0",
                "kind": "entry",
                "module_id": "main",
                "imports": [],
                "decls": [],
                "solve": [
                    "begin",
                    ["let", "b", ["bytes.lit", "hi"]],
                    ["bytes.concat", "b", "b"],
                ],
            },
        ),
    ]

    with tempfile.TemporaryDirectory(prefix="x07_diag_gate_") as td:
        tmp = Path(td)
        for case_id, want_code, program in cases:
            p = tmp / f"{case_id}.x07.json"
            p.write_text(json.dumps(program, separators=(",", ":")) + "\n", encoding="utf-8")

            rc, lint = _run_json(str(x07), "lint", "--world", "solve-pure", "--input", str(p), cwd=root)
            if rc != 1:
                raise SystemExit(f"ERROR: {case_id}: expected lint exit 1, got {rc} (doc={lint})")
            _assert_actionable_diag(lint, want_code=want_code)

            # Must be repairable via `x07 fix` (deterministic quickfix application).
            proc = subprocess.run(
                [str(x07), "fix", "--world", "solve-pure", "--write", "--input", str(p)],
                cwd=str(root),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            if proc.returncode != 0:
                raise SystemExit(
                    f"ERROR: {case_id}: x07 fix failed (exit={proc.returncode})\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}\n"
                )

            rc2, lint2 = _run_json(str(x07), "lint", "--world", "solve-pure", "--input", str(p), cwd=root)
            if rc2 != 0:
                raise SystemExit(f"ERROR: {case_id}: expected post-fix lint exit 0, got {rc2} (doc={lint2})")

    print("ok: check_diagnostics_actionable")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
