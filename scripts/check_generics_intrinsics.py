#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
GENERICS_RS = ROOT / "crates" / "x07c" / "src" / "generics.rs"
DOCS_GENERICS_MD = ROOT / "docs" / "language" / "generics.md"
DIAG_CATALOG = ROOT / "catalog" / "diagnostics.json"

BOUNDS_MARKER_BEGIN = "<!-- x07-generics-bounds:begin -->"
BOUNDS_MARKER_END = "<!-- x07-generics-bounds:end -->"

EXPECTED_BOUNDS: dict[str, list[str]] = {
    "any": ["*"],
    "bytes_like": ["bytes", "bytes_view"],
    "num_like": ["i32", "u32"],
    "hashable": ["i32", "u32"],
    "orderable": ["i32", "u32"],
}


def _die(msg: str, code: int = 1) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    raise SystemExit(code)


def _read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        _die(f"missing file: {path}")


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        _die(f"missing file: {path}")
    except json.JSONDecodeError as e:
        _die(f"invalid JSON: {path}: {e}")


def _extract_ty_intrinsics_from_rust(src: str) -> set[str]:
    # Keep this cheap and robust: treat string literals as the API.
    return set(re.findall(r'"(ty\.[a-z0-9_]+)"', src))


def _extract_ty_intrinsics_from_docs(src: str) -> set[str]:
    return set(re.findall(r"\bty\.[a-z0-9_]+\b", src))


def _extract_bounds_json_from_docs(src: str) -> dict[str, Any]:
    b = src.find(BOUNDS_MARKER_BEGIN)
    if b < 0:
        _die(f"{DOCS_GENERICS_MD}: missing bounds marker: {BOUNDS_MARKER_BEGIN}")
    e = src.find(BOUNDS_MARKER_END)
    if e < 0:
        _die(f"{DOCS_GENERICS_MD}: missing bounds marker: {BOUNDS_MARKER_END}")
    if e <= b:
        _die(f"{DOCS_GENERICS_MD}: invalid bounds marker ordering")

    window = src[b:e]
    blocks = re.findall(r"```json\s*(\{.*?\})\s*```", window, flags=re.DOTALL)
    if len(blocks) != 1:
        _die(f"{DOCS_GENERICS_MD}: expected exactly one ```json block between bounds markers")

    try:
        doc = json.loads(blocks[0])
    except json.JSONDecodeError as ex:
        _die(f"{DOCS_GENERICS_MD}: invalid bounds JSON between markers: {ex}")

    if not isinstance(doc, dict):
        _die(f"{DOCS_GENERICS_MD}: bounds JSON must be an object")
    return doc


def _extract_codes_from_source(src: str) -> set[str]:
    codes = set(re.findall(r"\b(X07-[A-Z0-9]+-[0-9]{4})\b", src))
    return {c for c in codes if c.startswith("X07-GENERICS-")}


def _catalog_codes(catalog_doc: dict[str, Any]) -> set[str]:
    entries = catalog_doc.get("entries")
    if not isinstance(entries, list):
        _die(f"{DIAG_CATALOG}: entries must be an array")
    out: set[str] = set()
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        code = entry.get("code")
        if isinstance(code, str) and code:
            out.add(code)
    return out


def main(argv: list[str]) -> int:
    if argv != ["--check"]:
        print("usage: check_generics_intrinsics.py --check", file=sys.stderr)
        return 2

    generics_rs = _read_text(GENERICS_RS)
    docs_md = _read_text(DOCS_GENERICS_MD)

    implemented_intrinsics = _extract_ty_intrinsics_from_rust(generics_rs)
    documented_intrinsics = _extract_ty_intrinsics_from_docs(docs_md)

    missing_in_docs = sorted(implemented_intrinsics.difference(documented_intrinsics))
    extra_in_docs = sorted(documented_intrinsics.difference(implemented_intrinsics))
    if missing_in_docs:
        _die(f"{DOCS_GENERICS_MD}: ty intrinsics implemented but not documented: {missing_in_docs}")
    if extra_in_docs:
        _die(f"{DOCS_GENERICS_MD}: ty intrinsics documented but not implemented: {extra_in_docs}")

    bounds_doc = _extract_bounds_json_from_docs(docs_md)
    bounds: dict[str, list[str]] = {}
    for k, v in bounds_doc.items():
        if not isinstance(k, str):
            continue
        if not isinstance(v, list) or not all(isinstance(x, str) for x in v):
            _die(f"{DOCS_GENERICS_MD}: bounds[{k!r}] must be an array of strings")
        bounds[k] = v

    if bounds != EXPECTED_BOUNDS:
        _die(f"{DOCS_GENERICS_MD}: bounds table mismatch: got={bounds!r} expected={EXPECTED_BOUNDS!r}")

    catalog = _read_json(DIAG_CATALOG)
    if not isinstance(catalog, dict):
        _die(f"{DIAG_CATALOG}: expected JSON object")
    catalog_codes = _catalog_codes(catalog)

    lint_rs = _read_text(ROOT / "crates" / "x07c" / "src" / "lint.rs")
    source_codes = sorted(_extract_codes_from_source(lint_rs))
    missing = [c for c in source_codes if c not in catalog_codes]
    if missing:
        _die(f"{DIAG_CATALOG}: missing diagnostic catalog entries for generics codes: {missing}")

    print("ok: generics intrinsics/bounds/docs/catalog are coherent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
