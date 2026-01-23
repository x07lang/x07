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


def _pkg_dir_name(pkg_name: str) -> str:
    return f"x07-{pkg_name}"


def validate_pkg_ref(*, root: Path, rel: str, ref: Any) -> list[str]:
    errs: list[str] = []
    if not isinstance(ref, dict):
        return [f"{rel}: package ref must be an object"]
    name = ref.get("name")
    version = ref.get("version")
    if not isinstance(name, str) or not name:
        errs.append(f"{rel}: package ref.name must be a non-empty string")
    if not isinstance(version, str) or parse_semver(version) is None:
        errs.append(f"{rel}: package ref.version must be semver (MAJOR.MINOR.PATCH)")
    if errs:
        return errs

    pkg_manifest_path = (
        root
        / "packages"
        / "ext"
        / _pkg_dir_name(name)
        / version
        / "x07-package.json"
    )
    if not pkg_manifest_path.is_file():
        errs.append(
            f"{rel}: referenced package not found: {pkg_manifest_path.relative_to(root)}"
        )
        return errs

    try:
        pkg_doc = load_json(pkg_manifest_path)
    except ValueError as ex:
        errs.append(f"{rel}: referenced package manifest invalid: {ex}")
        return errs

    if not isinstance(pkg_doc, dict):
        errs.append(
            f"{rel}: referenced package manifest must be object: {pkg_manifest_path.relative_to(root)}"
        )
        return errs
    if pkg_doc.get("schema_version") != "x07.package@0.1.0":
        errs.append(
            f"{rel}: referenced package schema_version mismatch: {pkg_manifest_path.relative_to(root)}"
        )
    if pkg_doc.get("name") != name:
        errs.append(
            f"{rel}: referenced package name mismatch: expected {name!r}, got {pkg_doc.get('name')!r}"
        )
    if pkg_doc.get("version") != version:
        errs.append(
            f"{rel}: referenced package version mismatch: expected {version!r}, got {pkg_doc.get('version')!r}"
        )
    return errs


def main() -> int:
    root = repo_root()
    catalog_path = root / "catalog" / "capabilities.json"
    if not catalog_path.is_file():
        eprint(f"ERROR: missing {catalog_path.relative_to(root)}")
        return 2

    try:
        doc = load_json(catalog_path)
    except ValueError as ex:
        eprint(f"ERROR: {ex}")
        return 2

    errs: list[str] = []
    if not isinstance(doc, dict):
        errs.append(f"{catalog_path.relative_to(root)}: root must be a JSON object")
    else:
        if doc.get("schema_version") != "x07.capabilities@0.1.0":
            errs.append(
                f"{catalog_path.relative_to(root)}: schema_version must be 'x07.capabilities@0.1.0'"
            )

        caps = doc.get("capabilities")
        if not isinstance(caps, list) or not caps:
            errs.append(f"{catalog_path.relative_to(root)}: capabilities must be a non-empty array")
            caps = []

        cap_ids: set[str] = set()
        for i, cap in enumerate(caps):
            rel = f"{catalog_path.relative_to(root)}: capabilities[{i}]"
            if not isinstance(cap, dict):
                errs.append(f"{rel}: must be an object")
                continue
            cid = cap.get("id")
            summary = cap.get("summary")
            canonical = cap.get("canonical")
            if not isinstance(cid, str) or not cid:
                errs.append(f"{rel}: id must be a non-empty string")
                continue
            if cid in cap_ids:
                errs.append(f"{rel}: duplicate id {cid!r}")
            cap_ids.add(cid)
            if not isinstance(summary, str) or not summary:
                errs.append(f"{rel}: summary must be a non-empty string")

            errs.extend(validate_pkg_ref(root=root, rel=f"{rel}.canonical", ref=canonical))

            alts = cap.get("alternatives")
            if alts is not None:
                if not isinstance(alts, list):
                    errs.append(f"{rel}: alternatives must be an array when present")
                else:
                    for j, alt in enumerate(alts):
                        errs.extend(
                            validate_pkg_ref(
                                root=root,
                                rel=f"{rel}.alternatives[{j}]",
                                ref=alt,
                            )
                        )

            worlds = cap.get("worlds")
            if worlds is not None and (
                not isinstance(worlds, list) or not all(isinstance(w, str) and w for w in worlds)
            ):
                errs.append(f"{rel}: worlds must be an array of non-empty strings when present")

            status = cap.get("status")
            if status is not None and status not in ("stable", "experimental", "deprecated"):
                errs.append(f"{rel}: status must be one of stable|experimental|deprecated when present")

            notes = cap.get("notes")
            if notes is not None and not isinstance(notes, str):
                errs.append(f"{rel}: notes must be a string when present")

        aliases = doc.get("aliases")
        if aliases is not None:
            if not isinstance(aliases, dict):
                errs.append(f"{catalog_path.relative_to(root)}: aliases must be an object when present")
            else:
                for k, v in aliases.items():
                    if not isinstance(k, str) or not k:
                        errs.append(f"{catalog_path.relative_to(root)}: aliases keys must be non-empty strings")
                        continue
                    if not isinstance(v, str) or not v:
                        errs.append(f"{catalog_path.relative_to(root)}: aliases[{k!r}] must be non-empty string")
                        continue
                    if v not in cap_ids:
                        errs.append(f"{catalog_path.relative_to(root)}: aliases[{k!r}] references unknown capability id {v!r}")

    if errs:
        eprint("ERROR: capabilities catalog validation failed")
        for m in errs:
            eprint(f"  - {m}")
        return 2

    print("ok: validated catalog/capabilities.json (and referenced packages exist in packages/ext)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

