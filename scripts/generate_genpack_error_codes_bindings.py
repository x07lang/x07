#!/usr/bin/env python3
"""
Generate Genpack SDK error-code bindings (Python + TypeScript) from the catalog.

- Input:  catalog/genpack_error_codes.json
- Output:
    sdk/genpack-py/src/x07_genpack/error_codes.py
    sdk/genpack-ts/src/errorCodes.ts

Modes:
  --check  : verify outputs are up-to-date (no writes)
  --write  : write outputs

No third-party dependencies.
Deterministic output (stable ordering, stable file content).
Stable failure messages.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


CATALOG_SCHEMA_VERSION = "x07.genpack.error_codes@0.1.0"

CATALOG_REL = Path("catalog/genpack_error_codes.json")
PY_OUT_REL = Path("sdk/genpack-py/src/x07_genpack/error_codes.py")
TS_OUT_REL = Path("sdk/genpack-ts/src/errorCodes.ts")


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


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


_CODE_RE = re.compile(r"^X07_GENPACK_[EW]_[A-Z0-9_]+$")


@dataclass(frozen=True)
class CodeEntry:
    code: str
    severity: str
    summary: str
    retryable: bool
    data_fields: tuple[str, ...]
    hints: tuple[str, ...]

    @staticmethod
    def from_obj(obj: Any, *, index: int, context: str) -> "CodeEntry":
        if not isinstance(obj, dict):
            _die(f"{context}: codes[{index}]: entry must be an object")

        code = obj.get("code")
        severity = obj.get("severity")
        summary = obj.get("summary")
        retryable = obj.get("retryable")
        data_fields = obj.get("data_fields", [])
        hints = obj.get("hints", [])

        if not isinstance(code, str) or not _CODE_RE.match(code):
            _die(f"{context}: codes[{index}]: invalid code: {code!r}")
        if severity not in ("error", "warn"):
            _die(f"{context}: codes[{index}]: severity must be 'error' or 'warn' (got {severity!r})")
        if not isinstance(summary, str) or not summary.strip():
            _die(f"{context}: codes[{index}]: summary must be a non-empty string")
        if not isinstance(retryable, bool):
            _die(f"{context}: codes[{index}]: retryable must be boolean")

        if not isinstance(data_fields, list) or any(not isinstance(x, str) for x in data_fields):
            _die(f"{context}: codes[{index}]: data_fields must be an array of strings")
        if not isinstance(hints, list) or any(not isinstance(x, str) for x in hints):
            _die(f"{context}: codes[{index}]: hints must be an array of strings")
        if not hints:
            _die(f"{context}: codes[{index}]: hints must be non-empty")

        normalized_data_fields = tuple(x.strip() for x in data_fields if x.strip())
        normalized_hints = tuple(x.strip() for x in hints if x.strip())
        if not normalized_hints:
            _die(f"{context}: codes[{index}]: hints must contain at least one non-empty hint")

        return CodeEntry(
            code=code.strip(),
            severity=severity,
            summary=summary.strip(),
            retryable=retryable,
            data_fields=normalized_data_fields,
            hints=normalized_hints,
        )


def _load_catalog(root: Path) -> list[CodeEntry]:
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

    entries: list[CodeEntry] = []
    for index, obj in enumerate(codes):
        entries.append(CodeEntry.from_obj(obj, index=index, context=context))

    code_names = [entry.code for entry in entries]
    if code_names != sorted(code_names):
        _die(f"{context}: codes must be sorted lexicographically by code")

    seen: set[str] = set()
    for code in code_names:
        if code in seen:
            _die(f"{context}: duplicate code: {code}")
        seen.add(code)

    return entries


def render_python(entries: list[CodeEntry]) -> str:
    lines: list[str] = []
    lines.append("# AUTO-GENERATED: DO NOT EDIT")
    lines.append("# generated by: scripts/generate_genpack_error_codes_bindings.py --write")
    lines.append("from __future__ import annotations")
    lines.append("")
    lines.append("from typing import Final, Literal")
    lines.append("")

    for entry in entries:
        lines.append(f'{entry.code}: Final[str] = "{entry.code}"')
    lines.append("")

    lines.append("ALL_CODES: Final[tuple[str, ...]] = (")
    for entry in entries:
        lines.append(f"    {entry.code},")
    lines.append(")")
    lines.append("")

    lines.append("GenpackErrorCode = Literal[")
    for entry in entries:
        lines.append(f'    "{entry.code}",')
    lines.append("]")
    lines.append("")

    lines.append("CODE_DOCS: Final[dict[str, dict[str, object]]] = {")
    for entry in entries:
        lines.append(f'    "{entry.code}": {{')
        lines.append(f'        "severity": "{entry.severity}",')
        lines.append(f'        "retryable": {str(entry.retryable)},')
        lines.append(f'        "summary": {json.dumps(entry.summary, ensure_ascii=False)},')
        lines.append(
            f'        "data_fields": {json.dumps(list(entry.data_fields), ensure_ascii=False, separators=(",", ":"))},'
        )
        lines.append(
            f'        "hints": {json.dumps(list(entry.hints), ensure_ascii=False, separators=(",", ":"))},'
        )
        lines.append("    },")
    lines.append("}")
    lines.append("")

    lines.append("__all__ = [")
    for entry in entries:
        lines.append(f'    "{entry.code}",')
    lines.append('    "ALL_CODES",')
    lines.append('    "GenpackErrorCode",')
    lines.append('    "CODE_DOCS",')
    lines.append("]")
    lines.append("")
    return "\n".join(lines)


def render_typescript(entries: list[CodeEntry]) -> str:
    lines: list[str] = []
    lines.append("/* AUTO-GENERATED: DO NOT EDIT */")
    lines.append("/* generated by: scripts/generate_genpack_error_codes_bindings.py --write */")
    lines.append("")

    for entry in entries:
        lines.append(f'export const {entry.code} = "{entry.code}" as const;')
    lines.append("")

    lines.append("export const ALL_CODES = [")
    for entry in entries:
        lines.append(f"  {entry.code},")
    lines.append("] as const;")
    lines.append("")

    lines.append("export type GenpackErrorCode = (typeof ALL_CODES)[number];")
    lines.append("")
    lines.append('export type GenpackErrorSeverity = "error" | "warn";')
    lines.append("")

    lines.append("export const CODE_DOCS: Record<GenpackErrorCode, {")
    lines.append("  severity: GenpackErrorSeverity;")
    lines.append("  retryable: boolean;")
    lines.append("  summary: string;")
    lines.append("  dataFields: string[];")
    lines.append("  hints: string[];")
    lines.append("}> = {")
    for entry in entries:
        lines.append(f"  {entry.code}: {{")
        lines.append(f'    severity: "{entry.severity}",')
        lines.append(f"    retryable: {str(entry.retryable).lower()},")
        lines.append(f"    summary: {json.dumps(entry.summary, ensure_ascii=False)},")
        lines.append(
            f"    dataFields: {json.dumps(list(entry.data_fields), ensure_ascii=False, separators=(',', ':'))},"
        )
        lines.append(f"    hints: {json.dumps(list(entry.hints), ensure_ascii=False, separators=(',', ':'))},")
        lines.append("  },")
    lines.append("};")
    lines.append("")

    return "\n".join(lines)


def _ensure_trailing_newline(text: str) -> str:
    return text if text.endswith("\n") else (text + "\n")


def _assert_deterministic(first: str, second: str, *, what: str) -> None:
    if first != second:
        _die(f"generator is not deterministic for {what}", code=3)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="generate_genpack_error_codes_bindings.py")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--check", action="store_true", help="Verify generated bindings are up-to-date")
    group.add_argument("--write", action="store_true", help="Write generated bindings")
    return parser.parse_args(argv)


def _check_file_matches(*, root: Path, rel: Path, expected: str) -> None:
    path = root / rel
    if not path.is_file():
        _die(f"missing generated file: {rel} (run: scripts/generate_genpack_error_codes_bindings.py --write)")
    actual = _read_text(path)
    if actual != expected:
        _die(f"generated file out of date: {rel} (run: scripts/generate_genpack_error_codes_bindings.py --write)")


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    root = _repo_root()

    entries = _load_catalog(root)

    py_first = _ensure_trailing_newline(render_python(entries))
    py_second = _ensure_trailing_newline(render_python(entries))
    _assert_deterministic(py_first, py_second, what="python")

    ts_first = _ensure_trailing_newline(render_typescript(entries))
    ts_second = _ensure_trailing_newline(render_typescript(entries))
    _assert_deterministic(ts_first, ts_second, what="typescript")

    if args.check:
        _check_file_matches(root=root, rel=PY_OUT_REL, expected=py_first)
        _check_file_matches(root=root, rel=TS_OUT_REL, expected=ts_first)
        print("ok: genpack error-code bindings are up-to-date")
        return 0

    _write_text(root / PY_OUT_REL, py_first)
    _write_text(root / TS_OUT_REL, ts_first)
    print(f"ok: wrote {PY_OUT_REL}")
    print(f"ok: wrote {TS_OUT_REL}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
