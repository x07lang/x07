#!/usr/bin/env python3
"""Gate: documented diagnostic-catalog code counts must match the catalog.

Docs (e.g. docs/why-x07.md) state the diagnostic catalog size in prose. That
count is hand-written and silently drifts whenever catalog/diagnostics.json
changes. This check finds every such claim and asserts it equals the actual
number of entries in the catalog, so the count cannot rot unnoticed.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

# Phrasings that state the diagnostic-catalog code count, capturing the number:
#   "a 646-code diagnostic catalog"
#   "a catalog (646 codes)"
#   "diagnostic catalog of 646 codes"
_PATTERNS = (
    re.compile(r"(\d+)-code diagnostic catalog"),
    re.compile(r"diagnostic catalog[^.\n]*?\((\d+)\s+codes?\)"),
    re.compile(r"catalog\s*\((\d+)\s+codes?\)"),
    re.compile(r"diagnostic catalog of (\d+)\s+codes?"),
)


def _repo_root() -> Path:
    # scripts/ci/check_doc_diagnostic_count.py -> scripts/ci -> scripts -> repo_root
    return Path(__file__).resolve().parents[2]


def _catalog_count(root: Path) -> int:
    doc = json.loads((root / "catalog" / "diagnostics.json").read_text(encoding="utf-8"))
    entries = doc.get("entries")
    if not isinstance(entries, list):
        raise SystemExit("ERROR: catalog/diagnostics.json has no 'entries' array")
    return len(entries)


def main() -> int:
    root = _repo_root()
    expected = _catalog_count(root)

    docs_dir = root / "docs"
    if not docs_dir.is_dir():
        print(f"ERROR: missing docs dir: {docs_dir}", file=sys.stderr)
        return 1

    rc = 0
    found_any = False
    for path in sorted(docs_dir.rglob("*.md")):
        rel = path.relative_to(root)
        for i, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            for pat in _PATTERNS:
                for m in pat.finditer(line):
                    found_any = True
                    got = int(m.group(1))
                    if got != expected:
                        print(
                            f"ERROR: {rel}:{i}: documented diagnostic count {got} "
                            f"!= catalog entries {expected}\n  {line.strip()}",
                            file=sys.stderr,
                        )
                        rc = 1
    if not found_any:
        # The phrasing changed; the gate is no longer protecting anything.
        print(
            "ERROR: no documented diagnostic-catalog count found; update the "
            "patterns in check_doc_diagnostic_count.py or restore the doc claim",
            file=sys.stderr,
        )
        return 1
    if rc == 0:
        print(f"ok: documented diagnostic counts match catalog ({expected} codes)")
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
