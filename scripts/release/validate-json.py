#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import sys
from typing import Any


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Minimal JSON validator for release helpers.")
    ap.add_argument("--schema", required=True, help="Expected schema_version")
    ap.add_argument("--in", dest="in_path", required=True, type=pathlib.Path, help="Input JSON file")
    ap.add_argument("--require", action="append", default=[], help="Required top-level key")
    args = ap.parse_args(argv)

    in_path = args.in_path.resolve()
    if not in_path.is_file():
        print(f"--in is not a file: {in_path}", file=sys.stderr)
        return 2

    try:
        doc: Any = json.loads(in_path.read_text(encoding="utf-8"))
    except Exception as exc:
        print(f"failed to parse JSON {in_path}: {exc}", file=sys.stderr)
        return 2

    if not isinstance(doc, dict):
        print(f"JSON root must be an object: {in_path}", file=sys.stderr)
        return 2

    got_schema = doc.get("schema_version")
    if got_schema != args.schema:
        print(
            f"schema_version mismatch for {in_path}: expected {args.schema!r}, got {got_schema!r}",
            file=sys.stderr,
        )
        return 2

    for key in args.require:
        if key not in doc:
            print(f"missing required key {key!r} in {in_path}", file=sys.stderr)
            return 2

    sys.stdout.write(str(in_path) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
