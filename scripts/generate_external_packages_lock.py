#!/usr/bin/env python3
"""Generate/check external-packages.lock for X07 external packages."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from lockgen_common import (
    SEMVER_RE,
    die as _die,
    parse_x07import_meta as _parse_x07import_meta,
    resolve_x07import_source_path as _resolve_x07import_source_path,
    repo_root as _repo_root,
    sha256_file as _sha256_file,
    stable_canon as _stable_canon,
)


def _module_from_path(pkg_root: Path, file_path: Path) -> dict[str, Any]:
    """Extract module info from an x07.json file within a package."""
    rel = file_path.relative_to(pkg_root / "modules")
    parts = list(rel.parts)
    if not parts[-1].endswith(".x07.json"):
        _die(f"ERROR: expected .x07.json file: {file_path}")
    parts[-1] = parts[-1][: -len(".x07.json")]
    module_id = ".".join(parts)

    mod: dict[str, Any] = {
        "module_id": module_id,
        "path": str(file_path.relative_to(_repo_root()).as_posix()),
        "sha256": _sha256_file(file_path),
        "size_bytes": file_path.stat().st_size,
    }

    x07import_src = _parse_x07import_meta(file_path)
    if x07import_src is not None:
        src_path_str, header_sha = x07import_src
        src_path = _resolve_x07import_source_path(src_path_str)
        if src_path is None:
            _die(f"ERROR: x07import source missing for {file_path}: {src_path_str}")
        src_sha = _sha256_file(src_path)
        if header_sha is not None and header_sha != src_sha:
            _die(
                "ERROR: x07import source sha256 mismatch for "
                f"{file_path} (header={header_sha} computed={src_sha})"
            )
        mod["generated_by"] = "x07import"
        mod["source_path"] = src_path_str
        mod["source_sha256"] = src_sha

    return mod


def _package_from_dir(pkg_dir: Path) -> dict[str, Any]:
    """Extract package info from a package directory."""
    manifest_path = pkg_dir / "x07-package.json"
    if not manifest_path.exists():
        _die(f"ERROR: package manifest missing: {manifest_path}")

    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as e:
        _die(f"ERROR: failed to parse package manifest {manifest_path}: {e}")

    name = manifest.get("name")
    version = manifest.get("version")
    if not isinstance(name, str) or not isinstance(version, str):
        _die(f"ERROR: invalid package manifest {manifest_path}: missing name or version")
    if not SEMVER_RE.match(version):
        _die(f"ERROR: invalid version {version!r} in {manifest_path}")

    meta = manifest.get("meta", {})
    determinism_tier = meta.get("determinism_tier", "unknown")
    import_mode = meta.get("import_mode", "unknown")
    ffi_libs = meta.get("ffi_libs", [])

    modules_dir = pkg_dir / "modules"
    modules: list[dict[str, Any]] = []
    if modules_dir.exists():
        for mod_file in sorted(modules_dir.rglob("*.x07.json")):
            if mod_file.is_file():
                modules.append(_module_from_path(pkg_dir, mod_file))

    pkg: dict[str, Any] = {
        "name": name,
        "version": version,
        "path": str(pkg_dir.relative_to(_repo_root()).as_posix()),
        "determinism_tier": determinism_tier,
        "import_mode": import_mode,
        "package_manifest_sha256": _sha256_file(manifest_path),
        "modules": modules,
    }

    if ffi_libs:
        pkg["ffi_libs"] = ffi_libs

    return pkg


def _compute_lock(ext_root: Path) -> dict[str, Any]:
    ext_root = ext_root.resolve()
    if not ext_root.exists():
        _die(f"ERROR: external packages root not found: {ext_root}")

    packages: list[dict[str, Any]] = []
    for pkg_vendor in sorted(ext_root.iterdir()):
        if not pkg_vendor.is_dir():
            continue
        for version_dir in sorted(pkg_vendor.iterdir()):
            if not version_dir.is_dir():
                continue
            if not SEMVER_RE.match(version_dir.name):
                continue
            manifest = version_dir / "x07-package.json"
            if manifest.exists():
                packages.append(_package_from_dir(version_dir))

    packages.sort(key=lambda p: (p["name"], p["version"], p["path"]))

    seen: set[tuple[str, str]] = set()
    for p in packages:
        k = (str(p["name"]), str(p["version"]))
        if k in seen:
            _die(f"ERROR: duplicate package {p['name']}@{p['version']} (check directory layout)")
        seen.add(k)

    lock: dict[str, Any] = {
        "lock_version": 1,
        "format": "x07.external-packages.lock@0.1.0",
        "packages_root": str(ext_root.relative_to(_repo_root()).as_posix()),
        "packages": packages,
    }
    lock["packages_hash"] = hashlib.sha256(_stable_canon(packages).encode("utf-8")).hexdigest()
    return lock


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate/check X07 external-packages.lock.")
    ap.add_argument(
        "--packages-root",
        default="packages/ext",
        help="Path to external packages root (default: packages/ext)",
    )
    ap.add_argument(
        "--out",
        default="locks/external-packages.lock",
        help="Output lockfile path (default: locks/external-packages.lock)",
    )
    ap.add_argument(
        "--check",
        action="store_true",
        help="Check that existing lock matches; do not rewrite.",
    )
    ap.add_argument(
        "--write",
        action="store_true",
        help="Write the lockfile (default when not --check).",
    )
    args = ap.parse_args()

    ext_root = Path(args.packages_root)
    out_path = Path(args.out)

    if args.check and args.write:
        _die("ERROR: --check and --write are mutually exclusive")

    lock = _compute_lock(ext_root)
    out_text = json.dumps(lock, sort_keys=True, indent=2, ensure_ascii=False) + "\n"

    if args.check:
        if not out_path.exists():
            _die(f"ERROR: lockfile missing: {out_path}")
        cur = out_path.read_text(encoding="utf-8")
        if cur != out_text:
            _die(
                "ERROR: external-packages.lock is out of date.\n"
                f"  Run: python scripts/generate_external_packages_lock.py --packages-root {ext_root} --out {out_path} --write\n"
            )
        print(f"OK: {out_path} up to date ({len(lock['packages'])} packages)")
        return 0

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(out_text, encoding="utf-8")
    print(f"Wrote {out_path} ({len(lock['packages'])} packages, packages_hash={lock['packages_hash'][:12]})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
