#!/usr/bin/env python3
"""
CI gate for Genpack SDK diagnostics completeness.

Checks:
  1) catalog/genpack_error_codes.json is well-formed and canonical.
  2) No undocumented codes are referenced in SDK source trees.
  3) No documented codes are left unused in SDK source trees.
  4) Generated bindings are up-to-date.

No third-party dependencies.
Deterministic scanning (stable file iteration + stable error ordering).
Stable failure messages.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


CATALOG_SCHEMA_VERSION = "x07.genpack.error_codes@0.1.0"
CATALOG_REL = Path("catalog/genpack_error_codes.json")

GEN_PY_REL = Path("sdk/genpack-py/src/x07_genpack/error_codes.py")
GEN_TS_REL = Path("sdk/genpack-ts/src/errorCodes.ts")

SDK_SCAN_ROOTS = [
    Path("sdk/genpack-py/src"),
    Path("sdk/genpack-py/tests"),
    Path("sdk/genpack-ts/src"),
    Path("sdk/genpack-ts/test"),
]

CODE_RE = re.compile(r"\bX07_GENPACK_[EW]_[A-Z0-9_]+\b")


def _die(msg: str, code: int = 1) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    raise SystemExit(code)


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        _die(f"missing file: {path}")
    except json.JSONDecodeError as exc:
        _die(f"invalid JSON: {path}: {exc}")


def _read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        _die(f"missing file: {path}")
    except UnicodeDecodeError:
        _die(f"file is not valid UTF-8 text: {path}")


def _iter_files(root: Path, *, exts: set[str]) -> list[Path]:
    out: list[Path] = []
    if not root.exists():
        return out
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix in exts:
            out.append(path)
    return sorted(out)


def _load_catalog_codes(root: Path) -> set[str]:
    path = root / CATALOG_REL
    doc = _read_json(path)
    context = str(CATALOG_REL)

    if not isinstance(doc, dict):
        _die(f"{context}: expected JSON object")
    if doc.get("schema_version") != CATALOG_SCHEMA_VERSION:
        _die(f"{context}: unexpected schema_version: {doc.get('schema_version')!r}")

    codes = doc.get("codes")
    if not isinstance(codes, list):
        _die(f"{context}: codes must be an array")

    out: list[str] = []
    seen: set[str] = set()
    for index, entry in enumerate(codes):
        if not isinstance(entry, dict):
            _die(f"{context}: codes[{index}]: entry must be an object")

        code = entry.get("code")
        if not isinstance(code, str) or not code:
            _die(f"{context}: codes[{index}]: missing/invalid code")
        if code in seen:
            _die(f"{context}: duplicate code: {code}")
        seen.add(code)
        out.append(code)

        severity = entry.get("severity")
        if severity not in ("error", "warn"):
            _die(f"{context}: codes[{index}]: severity must be 'error' or 'warn' (got {severity!r})")

        summary = entry.get("summary")
        if not isinstance(summary, str) or not summary.strip():
            _die(f"{context}: codes[{index}]: summary must be non-empty")

        retryable = entry.get("retryable")
        if not isinstance(retryable, bool):
            _die(f"{context}: codes[{index}]: retryable must be boolean")

        hints = entry.get("hints")
        if not isinstance(hints, list) or any(not isinstance(x, str) for x in hints) or not hints:
            _die(f"{context}: codes[{index}]: hints must be a non-empty array of strings")

    if out != sorted(out):
        _die(f"{context}: codes must be sorted lexicographically by code")

    return set(out)


def _scan_used_codes(root: Path) -> set[str]:
    exclude_abs = {
        (root / GEN_PY_REL).resolve(),
        (root / GEN_TS_REL).resolve(),
    }

    used: set[str] = set()
    for rel_root in SDK_SCAN_ROOTS:
        scan_root = root / rel_root
        for path in _iter_files(scan_root, exts={".py", ".ts", ".tsx"}):
            if path.resolve() in exclude_abs:
                continue
            text = _read_text(path)
            for match in CODE_RE.finditer(text):
                used.add(match.group(0))
    return used


def _run_generator_check(root: Path) -> None:
    generator = root / "scripts" / "generate_genpack_error_codes_bindings.py"
    if not generator.is_file():
        _die("missing generator script: scripts/generate_genpack_error_codes_bindings.py")

    proc = subprocess.run(
        [sys.executable, str(generator), "--check"],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if proc.returncode != 0:
        stderr = proc.stderr.strip()
        stdout = proc.stdout.strip()
        msg = "genpack bindings generator check failed"
        if stderr:
            msg += f"\n\nstderr:\n{stderr}"
        if stdout:
            msg += f"\n\nstdout:\n{stdout}"
        _die(msg)


def main(argv: list[str]) -> int:
    if argv != ["--check"]:
        print("usage: check_genpack_error_codes.py --check", file=sys.stderr)
        return 2

    root = _repo_root()
    catalog_codes = _load_catalog_codes(root)
    used_codes = _scan_used_codes(root)

    undocumented = sorted(used_codes - catalog_codes)
    if undocumented:
        for code in undocumented:
            print(
                f"ERROR: undocumented genpack error code referenced in SDK sources: {code}",
                file=sys.stderr,
            )
        print(f"ERROR: update {CATALOG_REL} and re-run generator", file=sys.stderr)
        return 1

    unused = sorted(catalog_codes - used_codes)
    if unused:
        for code in unused:
            print(f"ERROR: catalog genpack error code is unused in SDK sources: {code}", file=sys.stderr)
        print("ERROR: remove stale codes from catalog or use them in SDK logic", file=sys.stderr)
        return 1

    _run_generator_check(root)
    print("ok: genpack error-codes catalog + bindings are complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
