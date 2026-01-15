#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, NoReturn


def _die(msg: str, code: int = 2) -> NoReturn:
    print(msg, file=sys.stderr)
    raise SystemExit(code)


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        _die(f"ERROR: missing file: {path}")
    except UnicodeDecodeError as e:
        _die(f"ERROR: invalid UTF-8 in {path}: {e}")
    except json.JSONDecodeError as e:
        _die(f"ERROR: invalid JSON in {path}: {e}")


def _module_id_from_path(module_root: Path, file_path: Path) -> str:
    rel = file_path.relative_to(module_root)
    if not rel.name.endswith(".x07.json"):
        _die(f"ERROR: expected .x07.json module file: {file_path}")
    parts = list(rel.parts)
    parts[-1] = parts[-1][: -len(".x07.json")]
    return ".".join(parts)


def _discover_modules(module_root: Path) -> set[str]:
    return {
        _module_id_from_path(module_root, p)
        for p in module_root.rglob("*.x07.json")
        if p.is_file()
    }


def _normalize_modules(mods: Any, manifest_path: Path) -> list[str]:
    if not isinstance(mods, list):
        _die(f"ERROR: expected package.modules to be a list in {manifest_path}")
    out: list[str] = []
    for idx, m in enumerate(mods):
        if not isinstance(m, str):
            _die(f"ERROR: expected package.modules[{idx}] to be a string in {manifest_path}")
        out.append(m.strip())
    return out


def _check_manifest(manifest_path: Path) -> None:
    doc = _load_json(manifest_path)
    if not isinstance(doc, dict):
        _die(f"ERROR: expected JSON object in {manifest_path}")

    module_root_rel = doc.get("module_root")
    if not isinstance(module_root_rel, str) or not module_root_rel.strip():
        _die(f"ERROR: expected non-empty package.module_root in {manifest_path}")

    module_root = manifest_path.parent / module_root_rel
    if not module_root.exists():
        _die(f"ERROR: module_root does not exist for {manifest_path}: {module_root}")

    manifest_modules = _normalize_modules(doc.get("modules"), manifest_path)
    manifest_set = set(manifest_modules)
    if len(manifest_set) != len(manifest_modules):
        dups: set[str] = set()
        seen: set[str] = set()
        for m in manifest_modules:
            if m in seen:
                dups.add(m)
            seen.add(m)
        _die(f"ERROR: duplicate module IDs in {manifest_path}: {sorted(dups)}")

    disk_modules = _discover_modules(module_root)

    missing = sorted(disk_modules - manifest_set)
    extra = sorted(manifest_set - disk_modules)
    if missing or extra:
        lines = [f"ERROR: package manifest/module layout mismatch: {manifest_path}"]
        if missing:
            lines.append("  Missing from manifest (present on disk):")
            for m in missing:
                lines.append(f"    - {m}")
        if extra:
            lines.append("  Extra in manifest (missing on disk):")
            for m in extra:
                lines.append(f"    - {m}")
        _die("\n".join(lines))


def main() -> int:
    ap = argparse.ArgumentParser(description="Check that x07-package.json lists match module files.")
    ap.add_argument(
        "--root",
        default="stdlib",
        help="Directory to scan for x07-package.json files (default: stdlib)",
    )
    args = ap.parse_args()

    root = Path(args.root)
    if not root.is_absolute():
        root = _repo_root() / root
    if not root.exists():
        _die(f"ERROR: root does not exist: {root}")

    manifests = sorted(root.rglob("x07-package.json"))
    if not manifests:
        _die(f"ERROR: no x07-package.json files found under {root}")

    for m in manifests:
        _check_manifest(m)

    print(f"OK: {len(manifests)} package manifests match module files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
