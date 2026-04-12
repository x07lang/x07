from __future__ import annotations

import json
import shutil
from pathlib import Path
from typing import Any


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def normalize_dep_path(path_s: str) -> str:
    return path_s.replace("\\", "/")


def seed_official_deps(
    repo_root: Path,
    project_dir: Path,
    *,
    allow_missing: bool = False,
) -> list[tuple[str, str]]:
    doc = load_json(project_dir / "x07.json")
    deps = doc.get("dependencies") or []
    if not isinstance(deps, list):
        raise SystemExit("x07.json: dependencies must be an array")

    missing: list[tuple[str, str]] = []

    for dep in deps:
        if not isinstance(dep, dict):
            raise SystemExit(f"x07.json: dependency must be an object: {dep!r}")
        name = dep.get("name")
        version = dep.get("version")
        rel_path = dep.get("path")
        if not isinstance(name, str) or not name:
            raise SystemExit(f"x07.json: dependency.name must be string: {dep!r}")
        if not isinstance(version, str) or not version:
            raise SystemExit(f"x07.json: dependency.version must be string: {dep!r}")
        if not isinstance(rel_path, str) or not rel_path:
            raise SystemExit(f"x07.json: dependency.path must be string: {dep!r}")

        rel_path = normalize_dep_path(rel_path)
        if not rel_path.startswith(".x07/deps/"):
            continue

        dst = project_dir / rel_path
        if dst.exists():
            if dst.is_dir():
                shutil.rmtree(dst)
            else:
                raise SystemExit(f"dependency path exists but is not a directory: {dst}")

        src = repo_root / "packages" / "ext" / f"x07-{name}" / version
        if not src.is_dir():
            missing.append((name, version))
            if allow_missing:
                continue
            raise SystemExit(f"missing official package dir for {name}@{version}: {src}")

        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(src, dst)

    return missing
