#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


@dataclass(frozen=True)
class Semver:
    major: int
    minor: int
    patch: int

    @staticmethod
    def parse(s: str) -> "Semver | None":
        m = SEMVER_RE.match(s.strip())
        if not m:
            return None
        return Semver(int(m.group(1)), int(m.group(2)), int(m.group(3)))


def _repo_root() -> Path:
    # scripts/sync_ext_package_versions.py -> scripts -> repo_root
    return Path(__file__).resolve().parents[1]


def _read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _write_json(path: Path, obj: Any) -> None:
    path.write_text(json.dumps(obj, indent=2) + "\n", encoding="utf-8")


def _latest_ext_version(*, root: Path, pkg_name: str) -> str:
    pkg_dir = root / "packages" / "ext" / f"x07-{pkg_name}"
    if not pkg_dir.is_dir():
        raise ValueError(f"missing ext package dir: {pkg_dir.relative_to(root)}")

    versions: list[tuple[Semver, str]] = []
    for child in pkg_dir.iterdir():
        if not child.is_dir():
            continue
        v = Semver.parse(child.name)
        if v is None:
            continue
        versions.append((v, child.name))
    if not versions:
        raise ValueError(f"no semver versions under: {pkg_dir.relative_to(root)}")

    versions.sort(key=lambda it: (it[0].major, it[0].minor, it[0].patch))
    return versions[-1][1]


def _find_x07_bin(root: Path) -> Path:
    script = root / "scripts" / "ci" / "find_x07.sh"
    if not script.is_file():
        raise ValueError(f"missing helper: {script.relative_to(root)}")
    res = subprocess.run(
        [str(script)],
        cwd=str(root),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if res.returncode != 0:
        raise ValueError(f"find_x07.sh failed:\nstdout:\n{res.stdout}\nstderr:\n{res.stderr}")
    out = (res.stdout or "").strip()
    if not out:
        raise ValueError("find_x07.sh produced empty output")

    p = Path(out)
    if not p.is_absolute():
        p = root / p
    return p.resolve()


def _sync_capabilities_versions(*, root: Path, write: bool) -> list[str]:
    path = root / "catalog" / "capabilities.json"
    doc = _read_json(path)
    if not isinstance(doc, dict):
        raise ValueError("catalog/capabilities.json: expected JSON object")
    if doc.get("schema_version") != "x07.capabilities@0.1.0":
        raise ValueError(
            f"catalog/capabilities.json: unexpected schema_version: {doc.get('schema_version')!r}"
        )

    caps = doc.get("capabilities")
    if not isinstance(caps, list):
        raise ValueError("catalog/capabilities.json: capabilities must be an array")

    changes: list[str] = []

    def sync_pkg(obj: dict[str, Any], key: str) -> None:
        pkg = obj.get(key)
        if not isinstance(pkg, dict):
            return
        name = pkg.get("name")
        version = pkg.get("version")
        if not isinstance(name, str) or not name:
            return
        if not isinstance(version, str) or not version:
            return
        latest = _latest_ext_version(root=root, pkg_name=name)
        if latest != version:
            pkg["version"] = latest
            changes.append(f"{path.relative_to(root)}: {name}: {version} -> {latest}")

    for cap in caps:
        if not isinstance(cap, dict):
            continue
        sync_pkg(cap, "canonical")
        alts = cap.get("alternatives") or []
        if isinstance(alts, list):
            for alt in alts:
                if isinstance(alt, dict):
                    name = alt.get("name")
                    version = alt.get("version")
                    if not isinstance(name, str) or not name:
                        continue
                    if not isinstance(version, str) or not version:
                        continue
                    latest = _latest_ext_version(root=root, pkg_name=name)
                    if latest != version:
                        alt["version"] = latest
                        changes.append(
                            f"{path.relative_to(root)}: {name}: {version} -> {latest}"
                        )

    if changes and write:
        _write_json(path, doc)
    return changes


def _sync_cli_module_roots(*, root: Path, write: bool) -> list[str]:
    path = root / "crates" / "x07" / "src" / "cli.rs"
    text = path.read_text(encoding="utf-8")

    pkg_versions = {
        "ext-cli": _latest_ext_version(root=root, pkg_name="ext-cli"),
        "ext-data-model": _latest_ext_version(root=root, pkg_name="ext-data-model"),
        "ext-json-rs": _latest_ext_version(root=root, pkg_name="ext-json-rs"),
    }

    changes: list[str] = []
    updated = text
    for name, ver in pkg_versions.items():
        pkg_dir = f"x07-{name}"
        pat = re.compile(rf"(packages/ext/{re.escape(pkg_dir)}/)(\d+\.\d+\.\d+)(/modules)")

        def repl(m: re.Match[str]) -> str:
            old = m.group(2)
            if old != ver:
                changes.append(f"{path.relative_to(root)}: {name}: {old} -> {ver}")
            return f"{m.group(1)}{ver}{m.group(3)}"

        updated = pat.sub(repl, updated)

    if changes and write:
        path.write_text(updated, encoding="utf-8")
    return changes


def _iter_project_manifests(root: Path, rel_roots: Iterable[str]) -> Iterable[Path]:
    for rel in rel_roots:
        base = root / rel
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("x07.json")):
            yield path


def _normalize_dep_path(*, dep_name: str, version: str) -> str:
    return f".x07/deps/{dep_name}/{version}"


def _sync_project_dependencies(*, root: Path, project_manifest: Path, write: bool) -> list[str]:
    doc = _read_json(project_manifest)
    if not isinstance(doc, dict):
        return []

    deps = doc.get("dependencies")
    if not isinstance(deps, list) or not deps:
        return []

    changes: list[str] = []
    for dep in deps:
        if not isinstance(dep, dict):
            continue
        name = dep.get("name")
        version = dep.get("version")
        if not isinstance(name, str) or not name.startswith("ext-"):
            continue
        if not isinstance(version, str) or not version:
            continue
        latest = _latest_ext_version(root=root, pkg_name=name)
        if latest == version:
            continue
        dep["version"] = latest
        dep["path"] = _normalize_dep_path(dep_name=name, version=latest)
        changes.append(f"{project_manifest.relative_to(root)}: {name}: {version} -> {latest}")

    if changes and write:
        _write_json(project_manifest, doc)
    return changes


def _seed_project_deps(*, root: Path, project_dir: Path, project_manifest: Path) -> None:
    doc = _read_json(project_manifest)
    deps = doc.get("dependencies") or []
    if not isinstance(deps, list):
        raise ValueError("x07.json: dependencies must be an array")

    for dep in deps:
        if not isinstance(dep, dict):
            raise ValueError(f"x07.json: dependency must be object: {dep!r}")
        name = dep.get("name")
        version = dep.get("version")
        rel_path = dep.get("path")
        if not isinstance(name, str) or not name:
            raise ValueError(f"x07.json: dependency.name must be string: {dep!r}")
        if not isinstance(version, str) or not version:
            raise ValueError(f"x07.json: dependency.version must be string: {dep!r}")
        if not isinstance(rel_path, str) or not rel_path:
            raise ValueError(f"x07.json: dependency.path must be string: {dep!r}")

        src = root / "packages" / "ext" / f"x07-{name}" / version
        if not src.is_dir():
            raise ValueError(f"missing official package dir for {name}@{version}: {src}")

        dst = (project_dir / rel_path).resolve()
        if project_dir.resolve() not in dst.parents and dst != project_dir.resolve():
            raise ValueError(
                f"{project_manifest.relative_to(root)}: dependency path escapes project dir: {rel_path}"
            )

        if dst.exists():
            if dst.is_dir():
                shutil.rmtree(dst)
            else:
                dst.unlink()
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(src, dst)


def _clean_project_state(project_dir: Path) -> None:
    deps_dir = project_dir / ".x07" / "deps"
    if deps_dir.exists():
        shutil.rmtree(deps_dir)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--roots",
        nargs="*",
        default=["examples/agent-gate", "ci/fixtures/agent-scenarios"],
        help="Relative directories to scan for x07.json files.",
    )
    ap.add_argument(
        "--write",
        action="store_true",
        help="Write changes (default is check-only).",
    )
    args = ap.parse_args(argv)

    root = _repo_root()
    write = bool(args.write)

    cap_changes = _sync_capabilities_versions(root=root, write=write)
    cli_changes = _sync_cli_module_roots(root=root, write=write)

    project_changes: list[str] = []
    changed_projects: list[Path] = []
    for manifest in _iter_project_manifests(root, args.roots):
        changes = _sync_project_dependencies(root=root, project_manifest=manifest, write=write)
        if changes:
            project_changes.extend(changes)
            changed_projects.append(manifest)

    all_changes = cap_changes + cli_changes + project_changes
    if all_changes and not write:
        for line in all_changes:
            print(line, file=sys.stderr)
        print(
            "ERROR: ext package versions are out of date (re-run with --write)",
            file=sys.stderr,
        )
        return 1

    if not write:
        return 0

    if changed_projects:
        x07_bin = _find_x07_bin(root)
        for manifest in changed_projects:
            project_dir = manifest.parent
            _clean_project_state(project_dir)
            _seed_project_deps(root=root, project_dir=project_dir, project_manifest=manifest)
            subprocess.check_call(
                [str(x07_bin), "pkg", "lock", "--project", str(manifest), "--offline"],
                cwd=str(project_dir),
            )
            _clean_project_state(project_dir)

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
