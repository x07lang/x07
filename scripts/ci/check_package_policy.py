#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


ALLOWED_VISIBILITY = {"canonical", "advanced", "experimental"}


def _repo_root() -> Path:
    # scripts/ci/check_package_policy.py -> scripts/ci -> scripts -> repo_root
    return Path(__file__).resolve().parents[2]


def _eprint(msg: str) -> None:
    print(msg, file=sys.stderr)


def _load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as ex:
        raise ValueError(f"{path}: invalid JSON: {ex}") from ex


def _iter_pkg_versions(packages_root: Path) -> list[Path]:
    out: list[Path] = []
    for vendor_dir in sorted(packages_root.iterdir(), key=lambda p: p.name):
        if not vendor_dir.is_dir():
            continue
        for ver_dir in sorted(vendor_dir.iterdir(), key=lambda p: p.name):
            if not ver_dir.is_dir():
                continue
            if not (ver_dir / "x07-package.json").is_file():
                continue
            out.append(ver_dir)
    return out


def _collect_capability_refs(doc: dict[str, Any]) -> tuple[set[tuple[str, str]], set[tuple[str, str]]]:
    canonical: set[tuple[str, str]] = set()
    any_ref: set[tuple[str, str]] = set()
    for cap in doc.get("capabilities", []) or []:
        if not isinstance(cap, dict):
            continue
        canon = cap.get("canonical")
        if isinstance(canon, dict):
            name = canon.get("name")
            version = canon.get("version")
            if isinstance(name, str) and isinstance(version, str):
                canonical.add((name, version))
                any_ref.add((name, version))
        for alt in cap.get("alternatives") or []:
            if not isinstance(alt, dict):
                continue
            name = alt.get("name")
            version = alt.get("version")
            if isinstance(name, str) and isinstance(version, str):
                any_ref.add((name, version))
    return canonical, any_ref


def main() -> int:
    root = _repo_root()

    policy_doc = root / "docs" / "project" / "package-policy.md"
    if not policy_doc.is_file():
        _eprint(f"ERROR: missing policy doc: {policy_doc.relative_to(root)}")
        return 2

    caps_path = root / "catalog" / "capabilities.json"
    try:
        caps_doc = _load_json(caps_path)
    except ValueError as ex:
        _eprint(f"ERROR: {ex}")
        return 2
    if not isinstance(caps_doc, dict):
        _eprint(f"ERROR: {caps_path.relative_to(root)}: root must be a JSON object")
        return 2

    caps_canonical, caps_any = _collect_capability_refs(caps_doc)

    packages_root = root / "packages" / "ext"
    if not packages_root.is_dir():
        _eprint(f"ERROR: missing packages dir: {packages_root.relative_to(root)}")
        return 2

    errs: list[str] = []

    # Hard no-duplicates rule: the same module_id must not be exported by multiple package names.
    module_to_pkg_names: dict[str, set[str]] = {}

    for ver_dir in _iter_pkg_versions(packages_root):
        manifest_path = ver_dir / "x07-package.json"
        try:
            manifest = _load_json(manifest_path)
        except ValueError as ex:
            errs.append(str(ex))
            continue

        if not isinstance(manifest, dict):
            errs.append(f"{manifest_path.relative_to(root)}: manifest must be a JSON object")
            continue

        pkg_name = manifest.get("name")
        if not isinstance(pkg_name, str) or not pkg_name:
            errs.append(f"{manifest_path.relative_to(root)}: missing/invalid name")
            continue

        modules = manifest.get("modules", [])
        if not isinstance(modules, list) or not all(isinstance(m, str) and m for m in modules):
            errs.append(f"{manifest_path.relative_to(root)}: modules must be an array of non-empty strings")
            continue

        for mid in modules:
            module_to_pkg_names.setdefault(mid, set()).add(pkg_name)

        meta = manifest.get("meta") or {}
        if not isinstance(meta, dict):
            errs.append(f"{manifest_path.relative_to(root)}: meta must be an object when present")
            continue

        visibility = meta.get("visibility")
        if visibility is None:
            continue
        if not isinstance(visibility, str) or visibility not in ALLOWED_VISIBILITY:
            errs.append(
                f"{manifest_path.relative_to(root)}: meta.visibility must be one of {sorted(ALLOWED_VISIBILITY)}"
            )
            continue

        version = manifest.get("version")
        if isinstance(version, str) and visibility == "canonical":
            if (pkg_name, version) not in caps_canonical:
                errs.append(
                    f"{manifest_path.relative_to(root)}: meta.visibility is canonical, but {pkg_name}@{version} is not a canonical entry in catalog/capabilities.json"
                )
        if isinstance(version, str) and (pkg_name, version) not in caps_any:
            errs.append(
                f"{manifest_path.relative_to(root)}: meta.visibility is set, but {pkg_name}@{version} is not referenced from catalog/capabilities.json"
            )

    for mid, names in sorted(module_to_pkg_names.items(), key=lambda kv: kv[0]):
        if len(names) <= 1:
            continue
        errs.append(
            f"module_id collision: {mid!r} is exported by multiple packages: {', '.join(sorted(names))}"
        )

    if errs:
        _eprint("ERROR: package policy checks failed")
        for e in errs:
            _eprint(f"  - {e}")
        return 2

    print("ok: package policy checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

