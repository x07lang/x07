#!/usr/bin/env python3
"""Deterministic semantic validator + canonicalizer for x07cli SpecRows.

- Input: x07cli.specrows@0.1.0 JSON (already schema-valid).
- Output:
  - diagnostics JSON on stdout (stable ordering)
  - optional canonicalized spec JSON (fmt)

This is intended as a CI gate and as an agent helper tool:
  * produce deterministic, single-source diagnostics codes
  * make spec formatting stable (canonical row ordering, implied defaults)

No third-party dependencies.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple


# ----------------------------
# Diagnostics (stable codes)
# ----------------------------

SEV_ERROR = "error"
SEV_WARN = "warn"

def diag(code: str, msg: str, *, severity: str = SEV_ERROR, scope: str = "", row_index: int = -1) -> Dict[str, Any]:
    return {
        "severity": severity,
        "code": code,
        "scope": scope,
        "row_index": row_index,
        "message": msg,
    }


# ----------------------------
# Spec helpers
# ----------------------------

def _is_empty(x: Any) -> bool:
    return x is None or x == ""

def _row_scope(row: List[Any]) -> str:
    return str(row[0]) if row else ""

def _row_kind(row: List[Any]) -> str:
    return str(row[1]) if len(row) > 1 else ""

def _row_get(row: List[Any], idx: int, default: Any = "") -> Any:
    return row[idx] if len(row) > idx else default


def _canon_sort_key_str(s: str) -> Tuple[int, str]:
    # Empty sorts last.
    return (1, "") if s == "" else (0, s)


# ----------------------------
# Semantic validation + canonicalization
# ----------------------------

@dataclass
class CanonResult:
    canon: Dict[str, Any]
    diagnostics: List[Dict[str, Any]]


def validate_and_canon(spec: Dict[str, Any]) -> CanonResult:
    diags: List[Dict[str, Any]] = []

    schema_version = spec.get("schema_version", "")
    if schema_version != "x07cli.specrows@0.1.0":
        diags.append(diag("ECLI_SCHEMA_VERSION", f"schema_version must be 'x07cli.specrows@0.1.0' (got {schema_version!r})"))

    rows = spec.get("rows")
    if not isinstance(rows, list):
        diags.append(diag("ECLI_ROWS_MISSING", "rows must be an array"))
        rows = []

    # Group rows by scope; keep original row_index
    by_scope: Dict[str, List[Tuple[int, List[Any]]]] = {}
    for i, row in enumerate(rows):
        if not isinstance(row, list) or len(row) < 2:
            diags.append(diag("ECLI_ROW_SHAPE", "row must be an array with at least [scope, kind, ...]", row_index=i))
            continue
        scope = _row_scope(row)
        by_scope.setdefault(scope, []).append((i, row))

    # Per-scope checks: uniqueness and constraints
    reserved_long_help = "--help"
    reserved_short_help = "-h"
    reserved_long_version = "--version"
    reserved_short_version = "-V"

    # Track help/version presence per scope
    have_help: Dict[str, bool] = {}
    have_version: Dict[str, bool] = {}

    # For canonicalization: build new rows list (scope by scope)
    canon_rows: List[List[Any]] = []

    # Deterministic scope iteration: sort scopes lexicographically, but keep root first if present.
    scopes = sorted(by_scope.keys())
    if "root" in scopes:
        scopes = ["root"] + [s for s in scopes if s != "root"]

    for scope in scopes:
        scoped = by_scope[scope]

        # Counters
        about_rows: List[Tuple[int, List[Any]]] = []
        help_rows: List[Tuple[int, List[Any]]] = []
        version_rows: List[Tuple[int, List[Any]]] = []
        flag_rows: List[Tuple[int, List[Any]]] = []
        opt_rows: List[Tuple[int, List[Any]]] = []
        arg_rows: List[Tuple[int, List[Any]]] = []

        # Uniqueness sets
        short_seen: Dict[str, int] = {}
        long_seen: Dict[str, int] = {}
        key_seen: Dict[str, int] = {}

        # First pass: categorize and validate basics
        for (row_index, row) in scoped:
            kind = _row_kind(row)

            if kind == "about":
                about_rows.append((row_index, row))
                continue
            if kind == "help":
                help_rows.append((row_index, row))
                continue
            if kind == "version":
                version_rows.append((row_index, row))
                continue
            if kind == "flag":
                flag_rows.append((row_index, row))
                continue
            if kind == "opt":
                opt_rows.append((row_index, row))
                continue
            if kind == "arg":
                arg_rows.append((row_index, row))
                continue

            diags.append(diag("ECLI_ROW_KIND_UNKNOWN", f"unknown row kind {kind!r}", scope=scope, row_index=row_index))

        # Dup row-type checks
        if len(about_rows) > 1:
            diags.append(diag("ECLI_ABOUT_DUP", "more than one about row in scope", scope=scope, row_index=about_rows[1][0]))
        if len(help_rows) > 1:
            diags.append(diag("ECLI_HELP_DUP", "more than one help row in scope", scope=scope, row_index=help_rows[1][0]))
        if len(version_rows) > 1:
            diags.append(diag("ECLI_VERSION_DUP", "more than one version row in scope", scope=scope, row_index=version_rows[1][0]))

        have_help[scope] = len(help_rows) > 0
        have_version[scope] = len(version_rows) > 0

        # Validate flag + opt rows have at least one name and populate uniqueness sets.
        def check_short_long(kind: str, row_index: int, short_opt: str, long_opt: str) -> None:
            if _is_empty(short_opt) and _is_empty(long_opt):
                diags.append(diag(
                    "ECLI_FLAG_NO_NAMES" if kind == "flag" else "ECLI_OPT_NO_NAMES",
                    f"{kind} row must provide at least one of shortOpt or longOpt",
                    scope=scope,
                    row_index=row_index,
                ))
                return

            # reserved checks: reserved opts cannot be used by non-help/version
            if kind not in ("help", "version"):
                if long_opt == reserved_long_help or short_opt == reserved_short_help:
                    diags.append(diag("ECLI_RESERVED_HELP_USED", f"{kind} row uses reserved help option", scope=scope, row_index=row_index))
                if long_opt == reserved_long_version or short_opt == reserved_short_version:
                    diags.append(diag("ECLI_RESERVED_VERSION_USED", f"{kind} row uses reserved version option", scope=scope, row_index=row_index))

            # uniqueness
            if short_opt and short_opt in short_seen:
                diags.append(diag("ECLI_DUP_SHORT", f"duplicate short option {short_opt}", scope=scope, row_index=row_index))
            elif short_opt:
                short_seen[short_opt] = row_index

            if long_opt and long_opt in long_seen:
                diags.append(diag("ECLI_DUP_LONG", f"duplicate long option {long_opt}", scope=scope, row_index=row_index))
            elif long_opt:
                long_seen[long_opt] = row_index

        # help rows: [scope,"help",short,long,desc]
        for (row_index, row) in help_rows:
            short_opt = str(_row_get(row, 2, ""))
            long_opt = str(_row_get(row, 3, ""))
            check_short_long("help", row_index, short_opt, long_opt)

        # version rows: same shape as help
        for (row_index, row) in version_rows:
            short_opt = str(_row_get(row, 2, ""))
            long_opt = str(_row_get(row, 3, ""))
            check_short_long("version", row_index, short_opt, long_opt)

        # flags: [scope,"flag",short,long,key,desc,(meta?)]
        for (row_index, row) in flag_rows:
            short_opt = str(_row_get(row, 2, ""))
            long_opt = str(_row_get(row, 3, ""))
            key = str(_row_get(row, 4, ""))
            check_short_long("flag", row_index, short_opt, long_opt)

            if key in key_seen:
                diags.append(diag("ECLI_DUP_KEY", f"duplicate key {key}", scope=scope, row_index=row_index))
            else:
                key_seen[key] = row_index

            meta = _row_get(row, 6, None)
            if isinstance(meta, dict) and "key" in meta and str(meta["key"]) != key:
                diags.append(diag("ECLI_META_KEY_MISMATCH", f"meta.key {meta['key']!r} does not match key {key!r}", scope=scope, row_index=row_index))

        # opts: [scope,"opt",short,long,key,value_kind,desc,(meta?)]
        allowed_value_kinds = {"STR","PATH","U32","I32","BYTES","BYTES_HEX"}
        for (row_index, row) in opt_rows:
            short_opt = str(_row_get(row, 2, ""))
            long_opt = str(_row_get(row, 3, ""))
            key = str(_row_get(row, 4, ""))
            value_kind = str(_row_get(row, 5, ""))

            check_short_long("opt", row_index, short_opt, long_opt)

            if key in key_seen:
                diags.append(diag("ECLI_DUP_KEY", f"duplicate key {key}", scope=scope, row_index=row_index))
            else:
                key_seen[key] = row_index

            if value_kind not in allowed_value_kinds:
                diags.append(diag("ECLI_OPT_VALUE_KIND_UNKNOWN", f"unknown value_kind {value_kind!r}", scope=scope, row_index=row_index))

            meta = _row_get(row, 7, None)
            if isinstance(meta, dict):
                if "key" in meta and str(meta["key"]) != key:
                    diags.append(diag("ECLI_META_KEY_MISMATCH", f"meta.key {meta['key']!r} does not match key {key!r}", scope=scope, row_index=row_index))

                # default validation (minimal, deterministic)
                if "default" in meta and value_kind in allowed_value_kinds:
                    default_val = meta.get("default")
                    if not _default_parse_ok(value_kind, default_val):
                        diags.append(diag("ECLI_OPT_DEFAULT_INVALID", f"default is not valid for {value_kind}", scope=scope, row_index=row_index))

        # args: [scope,"arg",POS_NAME,key,desc,(meta?)]
        # - enforce required-after-optional
        # - enforce multi last + single multi
        saw_optional = False
        saw_multi = False
        for pos, (row_index, row) in enumerate(arg_rows):
            # key uniqueness already checked with key_seen if we included args
            key = str(_row_get(row, 3, ""))
            if key in key_seen:
                diags.append(diag("ECLI_DUP_KEY", f"duplicate key {key}", scope=scope, row_index=row_index))
            else:
                key_seen[key] = row_index

            meta = _row_get(row, 5, None)
            required = True
            multiple = False
            if isinstance(meta, dict):
                required = bool(meta.get("required", True))
                multiple = bool(meta.get("multiple", False))

            if not required:
                saw_optional = True
            elif saw_optional:
                diags.append(diag("ECLI_ARG_REQUIRED_AFTER_OPTIONAL", "required arg appears after optional arg", scope=scope, row_index=row_index))

            if multiple:
                if saw_multi:
                    diags.append(diag("ECLI_ARG_MULTI_DUP", "more than one arg has multiple=true", scope=scope, row_index=row_index))
                saw_multi = True
                if pos != len(arg_rows) - 1:
                    diags.append(diag("ECLI_ARG_MULTI_NOT_LAST", "arg with multiple=true must be last", scope=scope, row_index=row_index))

        # Implied defaults insertion (help/version)
        # Insert help if missing
        def _insert_help_row() -> List[Any]:
            if reserved_long_help in long_seen:
                # reserved long used by non-help is already diagnosed; do not auto-insert a second
                return []
            short = reserved_short_help if reserved_short_help not in short_seen else ""
            return [scope, "help", short, reserved_long_help, "Show help"]

        def _insert_version_row() -> List[Any]:
            if reserved_long_version in long_seen:
                return []
            short = reserved_short_version if reserved_short_version not in short_seen else ""
            return ["root", "version", short, reserved_long_version, "Show version"]

        # Canonicalization: build ordered rows
        # about
        if about_rows:
            canon_rows.append(about_rows[0][1])

        # help (existing or implied)
        if help_rows:
            canon_rows.append(help_rows[0][1])
        else:
            hr = _insert_help_row()
            if hr:
                canon_rows.append(hr)

        # version
        if version_rows:
            canon_rows.append(version_rows[0][1])
        else:
            if scope == "root":
                vr = _insert_version_row()
                if vr:
                    canon_rows.append(vr)

        # flags + opts (sorted)
        def sort_flag(r: List[Any]) -> Tuple[Tuple[int,str], Tuple[int,str], str]:
            long_opt = str(_row_get(r, 3, ""))
            short_opt = str(_row_get(r, 2, ""))
            key = str(_row_get(r, 4, ""))
            return (_canon_sort_key_str(long_opt), _canon_sort_key_str(short_opt), key)

        def sort_opt(r: List[Any]) -> Tuple[Tuple[int,str], Tuple[int,str], str]:
            long_opt = str(_row_get(r, 3, ""))
            short_opt = str(_row_get(r, 2, ""))
            key = str(_row_get(r, 4, ""))
            return (_canon_sort_key_str(long_opt), _canon_sort_key_str(short_opt), key)

        for _, r in sorted(flag_rows, key=lambda it: sort_flag(it[1])):
            canon_rows.append(r)
        for _, r in sorted(opt_rows, key=lambda it: sort_opt(it[1])):
            canon_rows.append(r)

        # args keep original order
        for _, r in arg_rows:
            canon_rows.append(r)

    # Diagnostics stable ordering: severity (error first), then code, then scope, then row_index.
    def sev_rank(s: str) -> int:
        return 0 if s == SEV_ERROR else 1

    diags_sorted = sorted(diags, key=lambda d: (sev_rank(d.get("severity","error")), d.get("code",""), d.get("scope",""), d.get("row_index",-1)))

    # Canonical spec: keep original app object as-is (but stable JSON writer will sort keys)
    canon_spec = dict(spec)
    canon_spec["schema_version"] = "x07cli.specrows@0.1.0"
    canon_spec["rows"] = canon_rows

    return CanonResult(canon=canon_spec, diagnostics=diags_sorted)


def _default_parse_ok(value_kind: str, default_val: Any) -> bool:
    # Minimal deterministic checks. No locale. No floats.
    if default_val is None:
        return False

    if value_kind in ("STR","PATH","BYTES"):
        return isinstance(default_val, str)
    if value_kind == "BYTES_HEX":
        if not isinstance(default_val, str):
            return False
        s = default_val.strip()
        if len(s) % 2 != 0:
            return False
        for ch in s:
            if ch not in "0123456789abcdefABCDEF":
                return False
        return True
    if value_kind == "U32":
        if not isinstance(default_val, str):
            return False
        if not default_val.isdigit():
            return False
        # bounds are not enforced here (language is modulo 2^32 anyway)
        return True
    if value_kind == "I32":
        if not isinstance(default_val, str):
            return False
        s = default_val
        if s.startswith("-"):
            s = s[1:]
        return s.isdigit()
    return False


# ----------------------------
# CLI
# ----------------------------

def _read_json(path: str) -> Dict[str, Any]:
    with open(path, "rb") as f:
        return json.loads(f.read().decode("utf-8"))


def _write_json(path: Optional[str], obj: Any) -> None:
    data = json.dumps(obj, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    if path is None or path == "-":
        sys.stdout.write(data)
        sys.stdout.write("\n")
    else:
        with open(path, "wb") as f:
            f.write(data.encode("utf-8"))
            f.write(b"\n")


def main() -> int:
    ap = argparse.ArgumentParser(prog="check_x07cli_specrows_semantic.py")
    ap.add_argument("mode", choices=["check", "fmt"], help="check: validate; fmt: output canonical spec")
    ap.add_argument("spec_path", help="path to cli.specrows.json")
    ap.add_argument("--diag-out", default="-", help="where to write diagnostics JSON (default stdout)")
    ap.add_argument("--out", default="-", help="for fmt: write canonical JSON to this path (default stdout)")
    ap.add_argument("--in-place", action="store_true", help="for fmt: overwrite the input file")
    args = ap.parse_args()

    spec = _read_json(args.spec_path)
    res = validate_and_canon(spec)

    # Always emit diagnostics (deterministic)
    _write_json(args.diag_out, {"diagnostics": res.diagnostics})

    has_errors = any(d.get("severity") == SEV_ERROR for d in res.diagnostics)

    if args.mode == "fmt":
        out_path = args.spec_path if args.in_place else args.out
        _write_json(out_path, res.canon)

    return 1 if has_errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
