#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def repo_root() -> Path:
    # scripts/ci/check_registry_backlog.py -> scripts/ci -> scripts -> repo root
    return Path(__file__).resolve().parents[2]


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise ValueError(f"{path}: invalid JSON: {exc}") from exc


def expect(condition: bool, errors: list[str], message: str) -> None:
    if not condition:
        errors.append(message)


def expect_rel_path_exists(root: Path, rel: str, errors: list[str], kind: str) -> Path | None:
    if not isinstance(rel, str) or not rel.strip():
        errors.append(f"missing {kind} path")
        return None
    path = root / rel
    if not path.exists():
        errors.append(f"missing {kind} path: {rel}")
        return None
    return path


def check_backlog(backlog_path: Path) -> list[str]:
    root = repo_root()
    errors: list[str] = []

    try:
        backlog = read_json(backlog_path)
    except ValueError as exc:
        return [str(exc)]
    expect(isinstance(backlog, dict), errors, f"{backlog_path}: root must be an object")
    if errors:
        return errors

    schema_version = backlog.get("schema_version")
    expect(
        schema_version == "x07.planning.registry_aware_package_backlog@0.1.0",
        errors,
        f"{backlog_path}: schema_version mismatch: {schema_version!r}",
    )

    items = backlog.get("items")
    expect(isinstance(items, list), errors, f"{backlog_path}: items must be an array")
    if not isinstance(items, list):
        return errors

    seen_ids: set[str] = set()
    seen_capabilities: set[str] = set()
    seen_pkgver: set[tuple[str, str]] = set()

    for idx, item in enumerate(items):
        base = f"items[{idx}]"
        if not isinstance(item, dict):
            errors.append(f"{base}: must be an object")
            continue

        item_id = item.get("id")
        if not isinstance(item_id, str) or not item_id.strip():
            errors.append(f"{base}.id: must be a non-empty string")
        else:
            if item_id in seen_ids:
                errors.append(f"{base}.id: duplicate: {item_id}")
            seen_ids.add(item_id)

        cap_id = item.get("capability_id")
        if not isinstance(cap_id, str) or not cap_id.strip():
            errors.append(f"{base}.capability_id: must be a non-empty string")
        else:
            if cap_id in seen_capabilities:
                errors.append(f"{base}.capability_id: duplicate: {cap_id}")
            seen_capabilities.add(cap_id)

        pkg = item.get("package")
        if not isinstance(pkg, dict):
            errors.append(f"{base}.package: must be an object")
            continue

        pkg_name = pkg.get("name")
        pkg_version = pkg.get("version")
        if not isinstance(pkg_name, str) or not pkg_name.strip():
            errors.append(f"{base}.package.name: must be a non-empty string")
        if not isinstance(pkg_version, str) or not pkg_version.strip():
            errors.append(f"{base}.package.version: must be a non-empty string")
        if isinstance(pkg_name, str) and isinstance(pkg_version, str):
            key = (pkg_name, pkg_version)
            if key in seen_pkgver:
                errors.append(f"{base}.package: duplicate name/version: {pkg_name}@{pkg_version}")
            seen_pkgver.add(key)

        dod = item.get("definition_of_done")
        if not isinstance(dod, dict):
            errors.append(f"{base}.definition_of_done: must be an object")
            continue

        docs = dod.get("docs")
        if not isinstance(docs, dict):
            errors.append(f"{base}.definition_of_done.docs: must be an object")
        else:
            manifest_rel = docs.get("manifest_path")
            manifest_path = expect_rel_path_exists(
                root, manifest_rel, errors, f"{base}.definition_of_done.docs.manifest_path"
            )
            if manifest_path is not None and manifest_path.is_file():
                pkg_doc = read_json(manifest_path)
                actual_name = pkg_doc.get("name")
                actual_version = pkg_doc.get("version")
                if isinstance(pkg_name, str) and actual_name != pkg_name:
                    errors.append(
                        f"{base}: package.name mismatch vs manifest {manifest_rel}: expected {pkg_name!r}, got {actual_name!r}"
                    )
                if isinstance(pkg_version, str) and actual_version != pkg_version:
                    errors.append(
                        f"{base}: package.version mismatch vs manifest {manifest_rel}: expected {pkg_version!r}, got {actual_version!r}"
                    )

        example = dod.get("example")
        if not isinstance(example, dict):
            errors.append(f"{base}.definition_of_done.example: must be an object")
        else:
            ex_rel = example.get("path")
            ex_path = expect_rel_path_exists(root, ex_rel, errors, f"{base}.definition_of_done.example.path")
            if ex_path is not None and ex_path.is_dir():
                expect((ex_path / "x07.json").is_file(), errors, f"{ex_rel}: missing x07.json")
                expect((ex_path / "src").is_dir(), errors, f"{ex_rel}: missing src/")

        fixtures = dod.get("fixtures")
        if not isinstance(fixtures, list) or not fixtures:
            errors.append(f"{base}.definition_of_done.fixtures: must be a non-empty array")
            continue

        for fidx, fixture in enumerate(fixtures):
            fbase = f"{base}.definition_of_done.fixtures[{fidx}]"
            if not isinstance(fixture, dict):
                errors.append(f"{fbase}: must be an object")
                continue

            fix_rel = fixture.get("path")
            fix_path = expect_rel_path_exists(root, fix_rel, errors, f"{fbase}.path")
            if fix_path is None or not fix_path.is_dir():
                continue

            expect((fix_path / "prompt.md").is_file(), errors, f"{fix_rel}: missing prompt.md")
            expect((fix_path / "broken").is_dir(), errors, f"{fix_rel}: missing broken/")
            expect((fix_path / "expected").is_dir(), errors, f"{fix_rel}: missing expected/")
            expect((fix_path / "assert.sh").is_file(), errors, f"{fix_rel}: missing assert.sh")

            goldens = fixture.get("goldens")
            if not isinstance(goldens, list) or not goldens:
                errors.append(f"{fbase}.goldens: must be a non-empty array")
                continue
            for g in goldens:
                if not isinstance(g, str) or not g.strip():
                    errors.append(f"{fbase}.goldens: entries must be non-empty strings")
                    continue
                if not (fix_path / g).is_file():
                    errors.append(f"{fix_rel}: missing golden file: {g}")

    return errors


def main() -> int:
    p = argparse.ArgumentParser(description="Validate registry_aware_package_backlog.json (offline-only).")
    p.add_argument(
        "--backlog",
        default="catalog/registry_aware_package_backlog.json",
        help="Path to backlog JSON (repo-relative).",
    )
    p.add_argument("--check", action="store_true", help="Fail on any validation error.")
    args = p.parse_args()

    root = repo_root()
    backlog_path = (root / args.backlog).resolve()
    errors = check_backlog(backlog_path)

    if errors:
        for e in errors:
            print(f"ERROR: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
