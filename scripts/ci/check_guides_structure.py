#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import re
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


def _extract_summary_section(text: str, heading: str) -> str | None:
    lines = text.splitlines()
    start = None
    for i, ln in enumerate(lines):
        if ln.strip() == f"## {heading}":
            start = i + 1
            break
    if start is None:
        return None

    section: list[str] = []
    for ln in lines[start:]:
        if ln.strip().startswith("## "):
            break
        section.append(ln)
    return "\n".join(section)


def _summary_md_link_targets(text: str) -> set[str]:
    out: set[str] = set()
    for m in re.finditer(r"\]\(([^)]+)\)", text):
        target = m.group(1).strip()
        if "://" in target:
            continue
        target = target.split("#", 1)[0].strip()
        if target.startswith("./"):
            target = target[2:]
        if target.endswith(".md"):
            out.add(target)
    return out


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

    summary_path = root / "docs" / "SUMMARY.md"
    if not summary_path.is_file():
        return _fail(f"missing SUMMARY: {summary_path.relative_to(root)}")

    summary_text = summary_path.read_text(encoding="utf-8")
    guides_section = _extract_summary_section(summary_text, "Guides")
    if guides_section is None:
        rc = 1
        print(f"ERROR: {summary_path.relative_to(root)}: missing '## Guides' section", file=sys.stderr)
        guides_section = ""

    linked_guides = {
        p for p in _summary_md_link_targets(guides_section) if p.startswith("guides/")
    }
    expected_guides = {f"guides/{p.name}" for p in sorted(guides_dir.glob("*.md"))}

    missing = sorted(expected_guides - linked_guides)
    extra = sorted(linked_guides - expected_guides)
    for rel in missing:
        rc = 1
        print(
            f"ERROR: {summary_path.relative_to(root)}: missing guide link under ## Guides: {rel}",
            file=sys.stderr,
        )
    for rel in extra:
        rc = 1
        print(
            f"ERROR: {summary_path.relative_to(root)}: unexpected guide link under ## Guides: {rel}",
            file=sys.stderr,
        )

    return rc


if __name__ == "__main__":
    raise SystemExit(main())
