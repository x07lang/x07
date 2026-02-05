from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class PackageSpec:
    name: str
    version: str

    @staticmethod
    def parse(raw: str) -> "PackageSpec | None":
        s = raw.strip()
        if not s or "@" not in s:
            return None
        name, version = s.split("@", 1)
        name = name.strip()
        version = version.strip()
        if not name or not version:
            return None
        return PackageSpec(name=name, version=version)


def _die(msg: str, code: int = 1) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    raise SystemExit(code)


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        _die(f"missing file: {path}")
    except json.JSONDecodeError as e:
        _die(f"invalid JSON: {path}: {e}")


def _repo_root() -> Path:
    # scripts/check_capabilities_coherence.py -> scripts -> repo root
    return Path(__file__).resolve().parents[1]


def _load_canonical_versions(*, capabilities_doc: dict[str, Any]) -> dict[str, str]:
    caps = capabilities_doc.get("capabilities")
    if not isinstance(caps, list):
        _die("catalog/capabilities.json: expected capabilities to be an array")

    out: dict[str, str] = {}
    for cap in caps:
        if not isinstance(cap, dict):
            continue
        canonical = cap.get("canonical")
        if not isinstance(canonical, dict):
            continue
        name = canonical.get("name")
        version = canonical.get("version")
        if not isinstance(name, str) or not isinstance(version, str):
            continue
        if not name or not version:
            continue
        if name in out and out[name] != version:
            _die(f"capabilities.json: canonical version mismatch for {name}: {out[name]} vs {version}")
        out[name] = version
    return out


def _pkg_manifest_path(*, root: Path, name: str, version: str) -> Path:
    if not name.startswith("ext-"):
        _die(f"unsupported canonical package (expected ext-*): {name}@{version}")
    return root / "packages" / "ext" / f"x07-{name}" / version / "x07-package.json"


def _load_requires_packages(*, pkg_manifest: dict[str, Any], context: str) -> list[PackageSpec]:
    meta = pkg_manifest.get("meta") or {}
    if not isinstance(meta, dict):
        return []
    reqs = meta.get("requires_packages") or []
    if not isinstance(reqs, list):
        _die(f"{context}: meta.requires_packages must be an array")

    out: list[PackageSpec] = []
    for raw in reqs:
        if not isinstance(raw, str):
            continue
        spec = PackageSpec.parse(raw)
        if spec is None:
            _die(f"{context}: invalid requires_packages entry: {raw!r}")
        out.append(spec)
    return out


def main(argv: list[str]) -> int:
    if argv != ["--check"]:
        print("usage: check_capabilities_coherence.py --check", file=sys.stderr)
        return 2

    root = _repo_root()
    cap_path = root / "catalog" / "capabilities.json"
    cap_doc = _read_json(cap_path)
    if not isinstance(cap_doc, dict):
        _die("catalog/capabilities.json: expected JSON object")
    if cap_doc.get("schema_version") != "x07.capabilities@0.1.0":
        _die(f"catalog/capabilities.json: unexpected schema_version: {cap_doc.get('schema_version')!r}")

    canonical_versions = _load_canonical_versions(capabilities_doc=cap_doc)

    errors: list[str] = []
    for name, version in sorted(canonical_versions.items()):
        manifest_path = _pkg_manifest_path(root=root, name=name, version=version)
        if not manifest_path.is_file():
            errors.append(f"missing local manifest for canonical package: {name}@{version}: {manifest_path}")
            continue

        pkg_doc = _read_json(manifest_path)
        if not isinstance(pkg_doc, dict):
            errors.append(f"{manifest_path}: expected JSON object")
            continue

        context = f"{name}@{version}"
        for dep in _load_requires_packages(pkg_manifest=pkg_doc, context=context):
            canon_dep_ver = canonical_versions.get(dep.name)
            if canon_dep_ver is None:
                continue
            if dep.version != canon_dep_ver:
                errors.append(
                    f"{context}: requires {dep.name}@{dep.version}, but capabilities pins {dep.name}@{canon_dep_ver}"
                )

    if errors:
        for e in errors:
            print(f"ERROR: {e}", file=sys.stderr)
        return 1

    print("ok: capabilities canonical packages are version-coherent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

