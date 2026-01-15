#!/usr/bin/env python3
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
    repo_root as _repo_root,
    sha256_file as _sha256_file,
    stable_canon as _stable_canon,
)


def _package_from_path(std_root: Path, file_path: Path) -> dict[str, Any]:
    """
    std_root is expected to be stdlib/std

    Example:
      stdlib/std/0.1.1/modules/std/json.x07.json -> name=std.json, version=0.1.1
      stdlib/std/0.1.1/modules/std/text/utf8.x07.json -> name=std.text.utf8, version=0.1.1
    """

    rel = file_path.relative_to(std_root)
    parts = rel.parts
    if len(parts) < 2:
        _die(f"ERROR: unexpected stdlib layout for {file_path} (expected <ver>/.../*.x07.json)")

    version = parts[0]
    if not SEMVER_RE.match(version):
        _die(f"ERROR: invalid version directory {version!r} for {file_path}")

    if not file_path.name.endswith(".x07.json"):
        _die(f"ERROR: expected .x07.json file: {file_path}")

    segs = list(parts[1:])
    if segs and segs[0] == "modules":
        segs = segs[1:]
    if not segs[-1].endswith(".x07.json"):
        _die(f"ERROR: expected .x07.json file: {file_path}")
    segs[-1] = segs[-1][: -len(".x07.json")]
    name = ".".join(segs)

    pkg: dict[str, Any] = {
        "name": name,
        "version": version,
        "path": str(file_path.as_posix()),
        "relpath": str(file_path.relative_to(std_root.parent).as_posix()),
        "sha256": _sha256_file(file_path),
        "size_bytes": file_path.stat().st_size,
    }

    x07import_src = _parse_x07import_meta(file_path)
    if x07import_src is not None:
        src_path_str, header_sha = x07import_src
        src_path = Path(src_path_str)
        if not src_path.is_absolute():
            src_path = _repo_root() / src_path
        if not src_path.exists():
            _die(f"ERROR: x07import source missing for {file_path}: {src_path}")
        src_sha = _sha256_file(src_path)
        if header_sha is not None and header_sha != src_sha:
            _die(
                "ERROR: x07import source sha256 mismatch for "
                f"{file_path} (header={header_sha} computed={src_sha})"
            )
        pkg["generated_by"] = "x07import"
        pkg["source_path"] = src_path_str
        pkg["source_sha256"] = src_sha

    return pkg


def _compute_lock(std_root: Path) -> dict[str, Any]:
    if not std_root.exists():
        _die(f"ERROR: stdlib root not found: {std_root}")

    files = sorted([p for p in std_root.rglob("*.x07.json") if p.is_file()])
    pkgs = [_package_from_path(std_root, p) for p in files]

    pkgs.sort(key=lambda p: (p["name"], p["version"], p["path"]))

    seen: set[tuple[str, str]] = set()
    for p in pkgs:
        k = (str(p["name"]), str(p["version"]))
        if k in seen:
            _die(f"ERROR: duplicate package {p['name']}@{p['version']} (check file layout)")
        seen.add(k)

    lock: dict[str, Any] = {
        "lock_version": 1,
        "format": "x07.stdlib.lock@0.1.0",
        "stdlib_root": str(std_root.as_posix()),
        "packages": pkgs,
    }
    lock["stdlib_hash"] = hashlib.sha256(_stable_canon(pkgs).encode("utf-8")).hexdigest()
    return lock


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate/check X07 stdlib.lock.")
    ap.add_argument(
        "--stdlib-root",
        default="stdlib/std",
        help="Path to stdlib root (default: stdlib/std)",
    )
    ap.add_argument("--out", default="stdlib.lock", help="Output lockfile path (default: stdlib.lock)")
    ap.add_argument(
        "--check",
        action="store_true",
        help="Check that existing lock matches; do not rewrite.",
    )
    args = ap.parse_args()

    std_root = Path(args.stdlib_root)
    out_path = Path(args.out)

    lock = _compute_lock(std_root)
    out_text = json.dumps(lock, sort_keys=True, indent=2, ensure_ascii=False) + "\n"

    if args.check:
        if not out_path.exists():
            _die(f"ERROR: lockfile missing: {out_path}")
        cur = out_path.read_text(encoding="utf-8")
        if cur != out_text:
            _die(
                "ERROR: stdlib.lock is out of date.\n"
                f"  Run: python scripts/generate_stdlib_lock.py --stdlib-root {std_root} --out {out_path}\n"
            )
        print(f"OK: {out_path} up to date ({len(lock['packages'])} packages)")
        return 0

    out_path.write_text(out_text, encoding="utf-8")
    print(f"Wrote {out_path} ({len(lock['packages'])} packages, stdlib_hash={lock['stdlib_hash'][:12]})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
