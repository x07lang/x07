#!/usr/bin/env python3

import argparse
import base64
import json
import sys
from typing import Any, Dict, Optional, Tuple


def _b64(b64s: str) -> bytes:
    if not b64s:
        return b""
    return base64.b64decode(b64s.encode("ascii"), validate=False)


def _truncate(b: bytes, limit: int) -> bytes:
    if len(b) <= limit:
        return b
    return b[:limit] + b"\n...<truncated>...\n"


def _decode_utf8(b: bytes) -> str:
    if not b:
        return ""
    if len(b) % 2 == 0 and b.count(0) > (len(b) // 4):
        try:
            return b.decode("utf-16le", errors="replace")
        except Exception:
            pass
    return b.decode("utf-8", errors="replace")


def _extract(doc: Dict[str, Any]) -> Tuple[Optional[Dict[str, Any]], Dict[str, Any]]:
    if "solve" in doc:
        return (doc.get("compile") or {}), (doc.get("solve") or {})
    return None, doc


def _fmt_bytes(label: str, b64s: str, limit: int) -> str:
    raw = _truncate(_b64(b64s), limit)
    if not raw:
        return f"{label}: <empty>"
    return f"{label}:\n{_decode_utf8(raw)}"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("name")
    ap.add_argument("--path", help="Read JSON from this file (default: stdin)")
    ap.add_argument("--expect", default="ok", help="Expected solve output bytes (ASCII)")
    ap.add_argument("--stderr-limit", type=int, default=4000)
    ap.add_argument("--stdout-limit", type=int, default=2000)
    args = ap.parse_args()

    try:
        if args.path:
            with open(args.path, "r", encoding="utf-8") as f:
                doc = json.load(f)
        else:
            doc = json.load(sys.stdin)
    except Exception as e:
        print(f"ERROR: {args.name}: failed to read runner JSON: {e}", file=sys.stderr)
        return 2

    if not isinstance(doc, dict):
        print(f"ERROR: {args.name}: runner JSON is not an object", file=sys.stderr)
        return 2

    compile_doc, solve_doc = _extract(doc)
    if not isinstance(solve_doc, dict):
        solve_doc = {}

    got = _b64(str(solve_doc.get("solve_output_b64") or ""))
    want = args.expect.encode("ascii", errors="strict")
    if got == want:
        print(f"ok: {args.name}")
        return 0

    print(f"FAIL: {args.name}", file=sys.stderr)
    for k in ("mode", "world", "exit_code"):
        if k in doc:
            print(f"{k}: {doc.get(k)!r}", file=sys.stderr)

    if compile_doc is not None:
        print(f"compile.ok: {compile_doc.get('ok')!r}", file=sys.stderr)
        print(f"compile.exit_status: {compile_doc.get('exit_status')!r}", file=sys.stderr)
        if compile_doc.get("compile_error") is not None:
            print(f"compile.compile_error: {compile_doc.get('compile_error')!r}", file=sys.stderr)
        print(_fmt_bytes("compile.stdout", str(compile_doc.get("stdout_b64") or ""), args.stdout_limit), file=sys.stderr)
        print(_fmt_bytes("compile.stderr", str(compile_doc.get("stderr_b64") or ""), args.stderr_limit), file=sys.stderr)
    else:
        print(f"ok: {doc.get('ok')!r}", file=sys.stderr)
        print(f"exit_status: {doc.get('exit_status')!r}", file=sys.stderr)
        if doc.get("trap") is not None:
            print(f"trap: {doc.get('trap')!r}", file=sys.stderr)

    print(f"solve.ok: {solve_doc.get('ok')!r}", file=sys.stderr)
    print(f"solve.exit_status: {solve_doc.get('exit_status')!r}", file=sys.stderr)
    if solve_doc.get("trap") is not None:
        print(f"solve.trap: {solve_doc.get('trap')!r}", file=sys.stderr)
    print(_fmt_bytes("solve.stdout", str(solve_doc.get("stdout_b64") or ""), args.stdout_limit), file=sys.stderr)
    print(_fmt_bytes("solve.stderr", str(solve_doc.get("stderr_b64") or ""), args.stderr_limit), file=sys.stderr)
    print(f"solve.output: {got!r}", file=sys.stderr)
    print(f"solve.expected: {want!r}", file=sys.stderr)

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
