#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import sys


def _repo_root() -> Path:
    # scripts/ci/check_guides_structure.py -> scripts/ci -> scripts -> repo_root
    return Path(__file__).resolve().parents[2]


def _fail(msg: str) -> int:
    print(f"ERROR: {msg}", file=sys.stderr)
    return 1


def _is_heading(line: str, prefix: str) -> bool:
    s = line.strip()
    return s.startswith(prefix) and (len(s) == len(prefix) or s[len(prefix)].isspace())


def main() -> int:
    root = _repo_root()
    guides_dir = root / "docs" / "guides"
    if not guides_dir.is_dir():
        return _fail(f"missing guides dir: {guides_dir.relative_to(root)}")

    rc = 0
    for path in sorted(guides_dir.glob("*.md")):
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()

        has_canonical = any(_is_heading(ln, "## Canonical") for ln in lines)
        has_expert = any(_is_heading(ln, "## Expert") for ln in lines)
        if not has_canonical:
            rc = 1
            print(
                f"ERROR: {path.relative_to(root)}: missing a '## Canonical ...' section",
                file=sys.stderr,
            )
        if not has_expert:
            rc = 1
            print(
                f"ERROR: {path.relative_to(root)}: missing a '## Expert ...' section",
                file=sys.stderr,
            )

        # Enforce that direct runner binaries only appear in the expert appendix.
        expert_idx = None
        for i, ln in enumerate(lines):
            if _is_heading(ln, "## Expert"):
                expert_idx = i
                break

        for i, ln in enumerate(lines):
            if "x07-host-runner" not in ln and "x07-os-runner" not in ln:
                continue
            if expert_idx is None or i < expert_idx:
                rc = 1
                print(
                    f"ERROR: {path.relative_to(root)}:{i+1}: direct runner usage must be under the Expert section",
                    file=sys.stderr,
                )

    return rc


if __name__ == "__main__":
    raise SystemExit(main())

