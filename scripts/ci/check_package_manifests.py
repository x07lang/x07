#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, Optional, Tuple


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def eprint(msg: str) -> None:
    print(msg, file=sys.stderr)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as ex:
        raise ValueError(f"{path}: invalid JSON: {ex}") from ex


def parse_semver(s: str) -> Optional[Tuple[int, int, int]]:
    parts = s.split(".")
    if len(parts) != 3:
        return None
    try:
        return (int(parts[0]), int(parts[1]), int(parts[2]))
    except Exception:
        return None


def validate_manifest(
    doc: Any, *, rel: str, expected_version: str, expected_name: str | None
) -> list[str]:
    errs: list[str] = []
    if not isinstance(doc, dict):
        return [f"{rel}: root must be a JSON object"]

    if doc.get("schema_version") != "x07.package@0.1.0":
        errs.append(f"{rel}: schema_version must be 'x07.package@0.1.0'")

    name = doc.get("name")
    if not isinstance(name, str) or not name:
        errs.append(f"{rel}: name must be a non-empty string")
    elif expected_name is not None and name != expected_name:
        errs.append(f"{rel}: name mismatch (expected {expected_name!r}, got {name!r})")

    version = doc.get("version")
    if not isinstance(version, str) or not version:
        errs.append(f"{rel}: version must be a non-empty string")
    elif version != expected_version:
        errs.append(f"{rel}: version mismatch (dir {expected_version!r} vs manifest {version!r})")

    module_root = doc.get("module_root")
    if not isinstance(module_root, str) or not module_root:
        errs.append(f"{rel}: module_root must be a non-empty string")

    modules = doc.get("modules")
    if not isinstance(modules, list) or not modules:
        errs.append(f"{rel}: modules must be a non-empty array")
    else:
        for i, m in enumerate(modules):
            if not isinstance(m, str) or not m:
                errs.append(f"{rel}: modules[{i}] must be a non-empty string")

    # description/docs are not required for ALL historical versions,
    # but are expected on the latest version (enforced separately).
    if "description" in doc and doc["description"] is not None and not isinstance(doc["description"], str):
        errs.append(f"{rel}: description must be a string if present")
    if "docs" in doc and doc["docs"] is not None and not isinstance(doc["docs"], str):
        errs.append(f"{rel}: docs must be a string if present")

    return errs


def main() -> int:
    root = repo_root()
    ext_root = root / "packages" / "ext"
    if not ext_root.is_dir():
        eprint(f"ERROR: missing {ext_root}")
        return 2

    all_errs: list[str] = []

    for pkg_dir in sorted([p for p in ext_root.iterdir() if p.is_dir()], key=lambda p: p.name):
        version_dirs = sorted([v for v in pkg_dir.iterdir() if v.is_dir()], key=lambda p: p.name)
        if not version_dirs:
            continue

        versions: list[tuple[tuple[int, int, int], Path]] = []
        for v in version_dirs:
            t = parse_semver(v.name)
            if t is None:
                continue
            versions.append((t, v))
        if not versions:
            continue
        versions.sort(key=lambda x: x[0])
        latest_dir = versions[-1][1]

        expected_name: str | None = None
        per_version_docs: dict[str, dict[str, Any]] = {}

        for _t, vdir in versions:
            manifest_path = vdir / "x07-package.json"
            rel = str(manifest_path.relative_to(root))
            if not manifest_path.is_file():
                all_errs.append(f"{rel}: missing file")
                continue
            try:
                doc = load_json(manifest_path)
            except ValueError as ex:
                all_errs.append(str(ex))
                continue

            if expected_name is None and isinstance(doc, dict) and isinstance(doc.get("name"), str) and doc["name"]:
                expected_name = doc["name"]

            all_errs.extend(
                validate_manifest(
                    doc,
                    rel=rel,
                    expected_version=vdir.name,
                    expected_name=expected_name,
                )
            )
            if isinstance(doc, dict):
                per_version_docs[vdir.name] = doc

        latest_doc = per_version_docs.get(latest_dir.name)
        if isinstance(latest_doc, dict):
            desc = latest_doc.get("description")
            docs = latest_doc.get("docs")
            if not isinstance(desc, str) or not desc.strip():
                all_errs.append(
                    f"{(latest_dir / 'x07-package.json').relative_to(root)}: latest version must have non-empty description"
                )
            if not isinstance(docs, str) or not docs.strip():
                all_errs.append(
                    f"{(latest_dir / 'x07-package.json').relative_to(root)}: latest version must have non-empty docs"
                )

    if all_errs:
        eprint("ERROR: package manifest validation failed")
        for m in all_errs:
            eprint(f"  - {m}")
        return 2

    print("ok: validated packages/ext manifests (and enforced docs/description on latest versions)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

