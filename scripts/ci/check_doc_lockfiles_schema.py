#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as ex:
        raise SystemExit(f"ERROR: {path.relative_to(repo_root())}: invalid JSON: {ex}") from ex


def load_lock_schema_version(root: Path) -> str:
    versions_path = root / "docs" / "_generated" / "versions.json"
    if not versions_path.is_file():
        raise SystemExit(
            f"ERROR: missing {versions_path.relative_to(root)} (run: python3 scripts/gen_versions_json.py --write)"
        )
    doc = load_json(versions_path)
    schemas = doc.get("schemas") if isinstance(doc, dict) else None
    if not isinstance(schemas, dict):
        raise SystemExit(f"ERROR: {versions_path.relative_to(root)}: schemas must be an object")
    lock = schemas.get("x07_lock")
    if not isinstance(lock, str) or not lock.strip():
        raise SystemExit(f"ERROR: {versions_path.relative_to(root)}: missing schemas.x07_lock")
    return lock.strip()


def check_lockfile(path: Path, *, want_schema: str) -> list[str]:
    errs: list[str] = []
    doc = load_json(path)
    if not isinstance(doc, dict):
        return [f"{path.relative_to(repo_root())}: lockfile must be a JSON object"]

    sv = (doc.get("schema_version") or "").strip() if isinstance(doc.get("schema_version"), str) else ""
    if sv != want_schema:
        errs.append(f"{path.relative_to(repo_root())}: schema_version is {sv!r}, expected {want_schema!r}")

    toolchain = doc.get("toolchain")
    if not isinstance(toolchain, dict):
        errs.append(f"{path.relative_to(repo_root())}: missing toolchain object (x07.lock@0.4.0+)")

    registry = doc.get("registry")
    if not isinstance(registry, dict) or not isinstance(registry.get("index_url"), str) or not registry.get("index_url"):
        errs.append(f"{path.relative_to(repo_root())}: missing registry.index_url (x07.lock@0.4.0+)")

    deps = doc.get("dependencies")
    if deps is not None and not isinstance(deps, list):
        errs.append(f"{path.relative_to(repo_root())}: dependencies must be an array when present")

    return errs


def main() -> int:
    root = repo_root()
    want_schema = load_lock_schema_version(root)

    docs_examples = root / "docs" / "examples"
    if not docs_examples.is_dir():
        print("ok: no docs/examples directory", file=sys.stderr)
        return 0

    lockfiles = sorted(docs_examples.rglob("x07.lock.json"))
    if not lockfiles:
        print("ok: no docs example lockfiles", file=sys.stderr)
        return 0

    errors: list[str] = []
    for lf in lockfiles:
        errors.extend(check_lockfile(lf, want_schema=want_schema))

    if errors:
        print("ERROR: docs example lockfiles are not on the current schema line", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 2

    print(f"ok: docs example lockfiles are {want_schema}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

