#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import re
import sys


PAT = re.compile(r"ext-[a-z0-9-]+@[0-9]")


def _repo_root() -> Path:
    # scripts/ci/check_doc_version_pins.py -> scripts/ci -> scripts -> repo_root
    return Path(__file__).resolve().parents[2]


def main() -> int:
    root = _repo_root()
    docs_dir = root / "docs"
    if not docs_dir.is_dir():
        print(f"ERROR: missing docs dir: {docs_dir.relative_to(root)}", file=sys.stderr)
        return 1

    rc = 0
    for path in sorted(docs_dir.rglob("*.md")):
        rel = path.relative_to(root)
        text = path.read_text(encoding="utf-8")
        for i, line in enumerate(text.splitlines(), start=1):
            if PAT.search(line):
                rc = 1
                print(
                    f"ERROR: {rel}:{i}: pinned ext package versions must not appear in docs (use NAME@VERSION placeholders and refer to the capability map / registry catalog)",
                    file=sys.stderr,
                )
                print(f"  {line.strip()}", file=sys.stderr)
    return rc


if __name__ == "__main__":
    raise SystemExit(main())

