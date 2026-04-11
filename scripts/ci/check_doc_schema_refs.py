#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Iterable


PROJECT_VERSION_RE = re.compile(r"\bx07\.project@\d+\.\d+\.\d+\b")
LOCK_VERSION_RE = re.compile(r"\bx07\.lock@\d+\.\d+\.\d+\b")
X07AST_VERSION_RE = re.compile(r"\bx07\.x07ast@\d+\.\d+\.\d+\b")

SUPPRESS_MARKER = "<!-- x07:allow-version-drift -->"


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def iter_files(root: Path, patterns: list[str]) -> Iterable[Path]:
    for pat in patterns:
        yield from sorted(root.glob(pat))


def load_versions(root: Path) -> dict[str, str]:
    path = root / "docs" / "_generated" / "versions.json"
    if not path.is_file():
        raise SystemExit(
            f"ERROR: missing {path.relative_to(root)} (run: python3 scripts/gen_versions_json.py --write)"
        )
    doc = json.loads(path.read_text(encoding="utf-8"))
    schemas = doc.get("schemas") or {}
    if not isinstance(schemas, dict):
        raise SystemExit(f"ERROR: {path.relative_to(root)}: schemas must be an object")

    def get(key: str) -> str:
        v = schemas.get(key)
        if not isinstance(v, str) or not v.strip():
            raise SystemExit(f"ERROR: {path.relative_to(root)}: missing schemas.{key}")
        return v.strip()

    return {
        "project": get("x07_project"),
        "lock": get("x07_lock"),
        "x07ast": get("x07_x07ast"),
    }


def _is_canonical_line(line_lower: str) -> bool:
    # Keep this conservative: only enforce in lines that are clearly stating the current choice.
    return any(
        key in line_lower
        for key in (
            "canonical",
            "new and actively maintained",
            "default lock schema",
            "current manifest line",
            "current schema line",
            "x07 init",
        )
    )


def check_file(path: Path, *, versions: dict[str, str]) -> list[str]:
    errs: list[str] = []
    text = path.read_text(encoding="utf-8")
    if SUPPRESS_MARKER in text:
        return errs

    for i, line in enumerate(text.splitlines(), start=1):
        line_lower = line.lower()
        if not _is_canonical_line(line_lower):
            continue

        proj = set(PROJECT_VERSION_RE.findall(line))
        lock = set(LOCK_VERSION_RE.findall(line))
        x07ast = set(X07AST_VERSION_RE.findall(line))

        if proj and versions["project"] not in proj:
            errs.append(
                f"{path.relative_to(repo_root())}:{i}: canonical docs reference {sorted(proj)} but expected {versions['project']}"
            )
        if lock and versions["lock"] not in lock:
            errs.append(
                f"{path.relative_to(repo_root())}:{i}: canonical docs reference {sorted(lock)} but expected {versions['lock']}"
            )
        if x07ast and versions["x07ast"] not in x07ast:
            errs.append(
                f"{path.relative_to(repo_root())}:{i}: canonical docs reference {sorted(x07ast)} but expected {versions['x07ast']}"
            )

    return errs


def main() -> int:
    root = repo_root()
    versions = load_versions(root)

    files: list[Path] = []
    files.extend(iter_files(root, ["docs/**/*.md"]))
    files.extend(iter_files(root, ["skills/pack/.agent/skills/**/SKILL.md"]))
    files.extend(iter_files(root, ["crates/x07up/assets/AGENT.template.md"]))
    files.extend(iter_files(root, ["README.md"]))

    errors: list[str] = []
    for f in files:
        if not f.is_file():
            continue
        errors.extend(check_file(f, versions=versions))

    if errors:
        print("ERROR: canonical docs/skills version references are out of date", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 2

    print("ok: docs/skills schema references match docs/_generated/versions.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

