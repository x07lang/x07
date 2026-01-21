#!/usr/bin/env python3
from __future__ import annotations

import argparse
import difflib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence


"""
Repair corpus gate.

This gate is designed to be a release-quality verification that x07 can support
an autonomous “diagnose → quickfix → re-lint” loop deterministically.

Rules enforced per case:
- `x07 lint` on broken must return exit 1 and emit x07diag JSON
- expected diagnostic codes must be present
- at least one json_patch quickfix must exist (unless expect_quickfix=false)
- `x07 fix --write` must succeed
- `x07 fmt --check` must succeed after fix
- `x07 lint` must succeed after fix
- fixed output must match golden file byte-for-byte (unless --bless)
"""


@dataclass(frozen=True)
class Case:
    case_id: str
    world: str
    broken: Path
    fixed: Path
    expect_codes: List[str]
    expect_quickfix: bool


def _repo_root() -> Path:
    # scripts/ci/check_repair_corpus.py -> scripts/ci -> scripts -> repo_root
    return Path(__file__).resolve().parents[2]


def _resolve_x07_bin(root: Path, x07_override: Optional[str]) -> Path:
    env = x07_override or os.environ.get("X07_BIN", "")
    if env:
        p = Path(env)
        if p.is_file() and os.access(p, os.X_OK):
            return p
        raise SystemExit(f"ERROR: X07_BIN is set but not executable: {env}")

    find_rel = Path("scripts") / "ci" / "find_x07.sh"
    find = root / find_rel
    if not find.is_file():
        raise SystemExit(f"ERROR: missing helper: {find}")

    try:
        out = (
            subprocess.check_output(
                ["bash", find_rel.as_posix()],
                cwd=root,
                stderr=subprocess.STDOUT,
            )
            .decode("utf-8", errors="replace")
            .strip()
        )
    except subprocess.CalledProcessError as e:
        sys.stderr.write("ERROR: find_x07.sh failed\n")
        if e.output:
            sys.stderr.write(e.output.decode("utf-8", errors="replace"))
            sys.stderr.write("\n")
        raise SystemExit(1)
    p = (root / out).resolve() if not Path(out).is_absolute() else Path(out)
    if not p.is_file():
        raise SystemExit(f"ERROR: find_x07.sh returned non-file path: {out}")
    if not os.access(p, os.X_OK):
        raise SystemExit(f"ERROR: x07 binary is not executable: {p}")
    return p


def _run(
    cmd: Sequence[str],
    *,
    cwd: Path,
    expect_codes: Optional[Sequence[int]] = None,
) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        list(cmd),
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if expect_codes is not None and proc.returncode not in expect_codes:
        sys.stderr.write(f"ERROR: command failed: {' '.join(cmd)}\n")
        sys.stderr.write(f"exit: {proc.returncode}\n")
        if proc.stdout:
            sys.stderr.write(f"stdout:\n{proc.stdout}\n")
        if proc.stderr:
            sys.stderr.write(f"stderr:\n{proc.stderr}\n")
        raise SystemExit(1)
    return proc


def _parse_json(s: str) -> Any:
    s = s.strip()
    if not s:
        raise ValueError("empty JSON")
    return json.loads(s)


def _load_cases(corpus_path: Path) -> List[Case]:
    raw = json.loads(corpus_path.read_text(encoding="utf-8"))
    if not isinstance(raw, dict):
        raise SystemExit(f"ERROR: corpus must be a JSON object: {corpus_path}")

    default_world = str(raw.get("default_world", "solve-pure"))
    base_dir = corpus_path.parent

    cases_raw = raw.get("cases")
    if not isinstance(cases_raw, list) or not cases_raw:
        raise SystemExit(f"ERROR: corpus.cases must be a non-empty array: {corpus_path}")

    cases: List[Case] = []
    for i, c in enumerate(cases_raw):
        if not isinstance(c, dict):
            raise SystemExit(f"ERROR: cases[{i}] must be an object")
        case_id = str(c.get("id", "")).strip()
        if not case_id:
            raise SystemExit(f"ERROR: cases[{i}].id is required")

        rel_dir = str(c.get("dir", "")).strip()
        if not rel_dir:
            raise SystemExit(f"ERROR: cases[{i}].dir is required (relative to corpus folder)")
        case_dir = (base_dir / rel_dir).resolve()

        broken_rel = str(c.get("broken", "")).strip()
        fixed_rel = str(c.get("fixed", "")).strip()
        if not broken_rel or not fixed_rel:
            raise SystemExit(f"ERROR: cases[{case_id}]: broken and fixed are required")

        broken = (case_dir / broken_rel).resolve()
        fixed = (case_dir / fixed_rel).resolve()
        if not broken.is_file():
            raise SystemExit(f"ERROR: cases[{case_id}]: missing broken file: {broken}")
        if not fixed.is_file():
            raise SystemExit(f"ERROR: cases[{case_id}]: missing fixed file: {fixed}")

        world = str(c.get("world", default_world)).strip() or default_world
        expect_codes = c.get("expect_codes", [])
        if not isinstance(expect_codes, list):
            raise SystemExit(f"ERROR: cases[{case_id}].expect_codes must be an array")
        expect_codes_s = [str(x) for x in expect_codes if str(x).strip()]

        expect_quickfix = bool(c.get("expect_quickfix", True))

        cases.append(
            Case(
                case_id=case_id,
                world=world,
                broken=broken,
                fixed=fixed,
                expect_codes=expect_codes_s,
                expect_quickfix=expect_quickfix,
            )
        )

    return cases


def _diff_text(expected: str, got: str, *, expected_name: str, got_name: str) -> str:
    diff = difflib.unified_diff(
        expected.splitlines(keepends=True),
        got.splitlines(keepends=True),
        fromfile=expected_name,
        tofile=got_name,
    )
    lines = list(diff)
    max_lines = 2000
    if len(lines) > max_lines:
        head = lines[: max_lines // 2]
        tail = lines[-max_lines // 2 :]
        lines = head + ["\n... (diff truncated) ...\n"] + tail
    return "".join(lines)


def _assert_diag(case_id: str, diag: Any, *, expect_ok: bool) -> Dict[str, Any]:
    if not isinstance(diag, dict):
        raise SystemExit(f"ERROR: {case_id}: lint output is not a JSON object")

    schema_version = diag.get("schema_version", "")
    if schema_version != "x07.x07diag@0.1.0":
        raise SystemExit(
            f"ERROR: {case_id}: lint output schema_version must be 'x07.x07diag@0.1.0' (got {schema_version!r})"
        )

    ok = bool(diag.get("ok", True))
    if expect_ok and not ok:
        raise SystemExit(f"ERROR: {case_id}: expected ok=true but got ok=false")
    if not expect_ok and ok:
        raise SystemExit(f"ERROR: {case_id}: expected ok=false but got ok=true")

    diags = diag.get("diagnostics", None)
    if not isinstance(diags, list):
        raise SystemExit(f"ERROR: {case_id}: diagnostics must be an array")

    return diag


def _has_json_patch_quickfix(diag_doc: Dict[str, Any]) -> bool:
    diags = diag_doc.get("diagnostics") or []
    for d in diags:
        if not isinstance(d, dict):
            continue
        q = d.get("quickfix")
        if not isinstance(q, dict):
            continue
        if q.get("kind") != "json_patch":
            continue
        patch = q.get("patch")
        if isinstance(patch, list) and len(patch) > 0:
            return True
    return False


def _codes_present(diag_doc: Dict[str, Any]) -> List[str]:
    out: List[str] = []
    for d in diag_doc.get("diagnostics") or []:
        if isinstance(d, dict) and isinstance(d.get("code"), str):
            out.append(d["code"])
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--corpus",
        default="ci/fixtures/repair-corpus/corpus.json",
        help="Path to the repair corpus manifest JSON.",
    )
    ap.add_argument("--x07", default="", help="Override x07 binary path (or set X07_BIN).")
    ap.add_argument(
        "--bless",
        action="store_true",
        help="Update golden fixed files with the produced output (developer-only).",
    )
    ap.add_argument(
        "--keep-tmp",
        action="store_true",
        help="Keep temp directories for debugging (prints their paths).",
    )
    args = ap.parse_args()

    root = _repo_root()
    corpus_path = (root / args.corpus).resolve()
    if not corpus_path.is_file():
        raise SystemExit(f"ERROR: missing corpus manifest: {corpus_path}")

    x07_bin = _resolve_x07_bin(root, args.x07.strip() or None)

    cases = _load_cases(corpus_path)
    failures: List[str] = []

    outer_tmp = tempfile.mkdtemp(prefix="x07_repair_corpus_")
    try:
        for c in cases:
            case_tmp = Path(outer_tmp) / c.case_id
            case_tmp.mkdir(parents=True, exist_ok=True)

            work = case_tmp / "work.x07.json"
            shutil.copyfile(c.broken, work)

            # 1) lint broken
            lint = _run(
                [str(x07_bin), "lint", "--input", str(work), "--world", c.world],
                cwd=root,
                expect_codes=[0, 1],
            )
            if lint.returncode == 0:
                failures.append(f"{c.case_id}: expected lint to fail (exit 1), got 0")
                continue

            try:
                diag = _parse_json(lint.stdout)
            except Exception as e:
                failures.append(f"{c.case_id}: lint did not emit x07diag JSON on stdout: {e}")
                continue

            try:
                diag_doc = _assert_diag(c.case_id, diag, expect_ok=False)
            except SystemExit as e:
                failures.append(str(e))
                continue

            present = set(_codes_present(diag_doc))
            missing_codes = [code for code in c.expect_codes if code not in present]
            if missing_codes:
                failures.append(
                    f"{c.case_id}: missing expected diagnostic codes: {missing_codes} (present: {sorted(present)})"
                )
                continue

            if c.expect_quickfix and not _has_json_patch_quickfix(diag_doc):
                failures.append(f"{c.case_id}: expected at least one json_patch quickfix, found none")
                continue

            # 2) apply fixes
            _run(
                [str(x07_bin), "fix", "--input", str(work), "--world", c.world, "--write"],
                cwd=root,
                expect_codes=[0],
            )

            # 3) fmt check
            _run(
                [str(x07_bin), "fmt", "--input", str(work), "--check"],
                cwd=root,
                expect_codes=[0],
            )

            # 4) lint again
            lint2 = _run(
                [str(x07_bin), "lint", "--input", str(work), "--world", c.world],
                cwd=root,
                expect_codes=[0, 1],
            )
            if lint2.returncode != 0:
                failures.append(f"{c.case_id}: expected lint after fix to succeed, got exit {lint2.returncode}")
                continue

            try:
                diag2 = _parse_json(lint2.stdout)
            except Exception as e:
                failures.append(f"{c.case_id}: post-fix lint did not emit x07diag JSON: {e}")
                continue
            try:
                _assert_diag(c.case_id, diag2, expect_ok=True)
            except SystemExit as e:
                failures.append(str(e))
                continue

            # 5) compare to golden fixed file
            got_bytes = work.read_bytes()
            want_bytes = c.fixed.read_bytes()
            if got_bytes != want_bytes:
                if args.bless:
                    c.fixed.write_bytes(got_bytes)
                else:
                    try:
                        got_txt = got_bytes.decode("utf-8")
                        want_txt = want_bytes.decode("utf-8")
                        d = _diff_text(
                            want_txt,
                            got_txt,
                            expected_name=str(c.fixed),
                            got_name=str(work),
                        )
                        failures.append(f"{c.case_id}: output mismatch vs golden\n{d}")
                    except Exception:
                        failures.append(f"{c.case_id}: output mismatch vs golden (binary differs)")
                    continue

            print(f"ok: {c.case_id}")

    finally:
        if args.keep_tmp:
            print(f"tmp kept: {outer_tmp}")
        else:
            shutil.rmtree(outer_tmp, ignore_errors=True)

    if failures:
        sys.stderr.write("\n".join(failures) + "\n")
        return 1

    print(f"ok: repair corpus ({len(cases)} cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
