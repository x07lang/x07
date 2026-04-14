#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def dump_pretty(doc: Any) -> bytes:
    out = json.dumps(doc, indent=2, sort_keys=False) + "\n"
    return out.encode("utf-8")


def load_versions(root: Path) -> dict[str, str]:
    versions_path = root / "docs" / "_generated" / "versions.json"
    if not versions_path.is_file():
        raise SystemExit(
            f"ERROR: missing {versions_path.relative_to(root)} (run: python3 scripts/gen_versions_json.py --write)"
        )
    doc = load_json(versions_path)
    if not isinstance(doc, dict):
        raise SystemExit(f"ERROR: {versions_path.relative_to(root)}: expected JSON object")

    schemas = doc.get("schemas")
    toolchain = doc.get("toolchain")
    pkg = doc.get("pkg")
    if not isinstance(schemas, dict) or not isinstance(toolchain, dict) or not isinstance(pkg, dict):
        raise SystemExit(f"ERROR: {versions_path.relative_to(root)}: invalid shape")

    def get_obj(obj: dict[str, Any], key: str) -> str:
        v = obj.get(key)
        if not isinstance(v, str) or not v.strip():
            raise SystemExit(f"ERROR: {versions_path.relative_to(root)}: missing {key}")
        return v.strip()

    return {
        "lock_schema": get_obj(schemas, "x07_lock"),
        "x07_version": get_obj(toolchain, "x07"),
        "x07c_version": get_obj(toolchain, "x07c"),
        "lang_id": get_obj(toolchain, "lang_id"),
        "compat_current": get_obj(toolchain, "compat_current"),
        "index_url": get_obj(pkg, "default_index_url"),
    }


def load_project_compat(lockfile_path: Path) -> str:
    project_path = lockfile_path.parent / "x07.json"
    if not project_path.is_file():
        return "current"
    doc = load_json(project_path)
    if not isinstance(doc, dict):
        raise SystemExit(f"{project_path}: expected JSON object")
    compat = doc.get("compat")
    if isinstance(compat, str) and compat.strip():
        return compat.strip()
    return "current"


def upgrade_lockfile(doc: Any, *, versions: dict[str, str], compat: str) -> Any:
    if not isinstance(doc, dict):
        raise SystemExit("lockfile must be a JSON object")
    if not compat.strip():
        raise SystemExit("compat must be non-empty")
    sv = doc.get("schema_version")

    deps = doc.get("dependencies")
    if not isinstance(deps, list):
        raise SystemExit("lockfile.dependencies must be an array")

    if sv == versions["lock_schema"]:
        return {
            "schema_version": versions["lock_schema"],
            "toolchain": {
                "x07_version": versions["x07_version"],
                "x07c_version": versions["x07c_version"],
                "lang_id": versions["lang_id"],
                "compat": compat,
            },
            "registry": {"index_url": versions["index_url"]},
            "dependencies": deps,
        }

    if sv != "x07.lock@0.3.0":
        raise SystemExit(
            f"unsupported lockfile schema_version: {sv!r} (expected x07.lock@0.3.0 or {versions['lock_schema']})"
        )

    return {
        "schema_version": versions["lock_schema"],
        "toolchain": {
            "x07_version": versions["x07_version"],
            "x07c_version": versions["x07c_version"],
            "lang_id": versions["lang_id"],
            "compat": compat,
        },
        "registry": {"index_url": versions["index_url"]},
        "dependencies": deps,
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--root",
        default="docs/examples",
        help="Directory containing docs examples (repo-relative).",
    )
    ap.add_argument("--check", action="store_true", help="Fail if any docs lockfile needs upgrade.")
    ap.add_argument("--write", action="store_true", help="Upgrade docs lockfiles in place.")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    if args.check == args.write:
        raise SystemExit("ERROR: set exactly one of --check or --write")

    root = repo_root()
    versions = load_versions(root)
    docs_root = (root / args.root).resolve()
    if not docs_root.is_dir():
        raise SystemExit(f"ERROR: missing docs examples dir: {docs_root.relative_to(root)}")

    lockfiles = sorted(docs_root.rglob("x07.lock.json"))
    if not lockfiles:
        print("ok: no docs lockfiles")
        return 0

    changed: list[Path] = []
    for path in lockfiles:
        try:
            doc = load_json(path)
        except Exception as ex:
            raise SystemExit(f"ERROR: {path.relative_to(root)}: invalid JSON: {ex}") from ex

        compat = load_project_compat(path)
        upgraded = upgrade_lockfile(doc, versions=versions, compat=compat)
        upgraded_bytes = dump_pretty(upgraded)
        existing_bytes = path.read_bytes()
        if upgraded_bytes != existing_bytes:
            changed.append(path)
            if args.write:
                path.write_bytes(upgraded_bytes)

    if args.check:
        if changed:
            for p in changed:
                print(f"ERROR: {p.relative_to(root)} is out of date (run --write)", file=sys.stderr)
            return 1
        print("ok: docs lockfiles are current")
        return 0

    if changed:
        print(f"ok: upgraded {len(changed)} docs lockfiles")
    else:
        print("ok: docs lockfiles already current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
