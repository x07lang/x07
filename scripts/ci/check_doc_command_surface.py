#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Iterable


RUNNER_RE = re.compile(r"\\bx07-(?:os-runner|host-runner)\\b")
CODE_FENCE_RE = re.compile(r"^\\s*```")

# Where runner commands must NOT appear in fenced code blocks.
CANONICAL_MD_DIRS = [
    "docs/getting-started",
    "docs/worlds",
    "docs/guides",
    "docs/recipes",
]
CANONICAL_SKILLS = [
    "skills/pack/.codex/skills/x07-run/SKILL.md",
    "skills/pack/.codex/skills/x07-package/SKILL.md",
    "skills/pack/.codex/skills/x07-agent-playbook/SKILL.md",
]

# Where runner commands ARE allowed (expert tools).
EXPERT_MD_DIRS = [
    "docs/toolchain",
]
EXPERT_SKILLS_DIRS = [
    "skills/pack/.codex/skills/x07-os-run",
    "skills/pack/.codex/skills/x07-build-run",
]


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def iter_md_files(root: Path, rel_dirs: list[str]) -> Iterable[Path]:
    for d in rel_dirs:
        p = root / d
        if not p.is_dir():
            continue
        yield from sorted(p.rglob("*.md"))


def is_under_any(path: Path, roots: list[Path]) -> bool:
    for r in roots:
        try:
            path.relative_to(r)
            return True
        except ValueError:
            pass
    return False


def check_file(path: Path) -> list[str]:
    errs: list[str] = []
    lines = path.read_text(encoding="utf-8").splitlines()
    in_code = False
    buf: list[str] = []

    for i, line in enumerate(lines, start=1):
        if CODE_FENCE_RE.match(line):
            if in_code:
                block = "\n".join(buf)
                if RUNNER_RE.search(block):
                    errs.append(
                        f"{path}: runner command appears in fenced code block ending at line {i}"
                    )
                buf = []
                in_code = False
            else:
                in_code = True
            continue
        if in_code:
            buf.append(line)

    return errs


def main() -> int:
    root = repo_root()

    expert_roots = [root / d for d in EXPERT_MD_DIRS] + [root / d for d in EXPERT_SKILLS_DIRS]

    files: list[Path] = []
    files.extend(iter_md_files(root, CANONICAL_MD_DIRS))
    for s in CANONICAL_SKILLS:
        p = root / s
        if p.is_file():
            files.append(p)

    all_errs: list[str] = []
    for f in files:
        if is_under_any(f, expert_roots):
            continue
        all_errs.extend(check_file(f))

    if all_errs:
        print(
            "ERROR: canonical docs/skills contain direct runner commands in code blocks",
            file=sys.stderr,
        )
        for m in all_errs:
            print(f"  - {m}", file=sys.stderr)
        return 2

    print("ok: canonical docs/skills do not include x07-*-runner commands in fenced code blocks")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

