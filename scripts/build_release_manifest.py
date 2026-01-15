from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Any


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def read_rust_toolchain_channel(path: Path) -> str:
    if not path.is_file():
        return "unknown"
    m = re.search(r'^\s*channel\s*=\s*"([^"]+)"\s*$', path.read_text(encoding="utf-8"), re.MULTILINE)
    if not m:
        return "unknown"
    return m.group(1)


def read_git_sha(root: Path) -> str:
    try:
        proc = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except FileNotFoundError:
        return ""

    if proc.returncode != 0:
        return ""
    return proc.stdout.strip()


def schema_version_from_schema(schema: dict[str, Any]) -> str | None:
    props = schema.get("properties")
    if not isinstance(props, dict):
        return None
    sv = props.get("schema_version")
    if not isinstance(sv, dict):
        return None
    const = sv.get("const")
    if isinstance(const, str):
        return const
    return None


def build_manifest(root: Path) -> dict[str, Any]:
    cargo_lock = root / "Cargo.lock"
    stdlib_lock = root / "stdlib.lock"
    stdlib_os_lock = root / "stdlib.os.lock"

    for p in [cargo_lock, stdlib_lock, stdlib_os_lock]:
        if not p.is_file():
            raise SystemExit(f"ERROR: missing required file: {p.relative_to(root)}")

    spec_dir = root / "spec"
    if not spec_dir.is_dir():
        raise SystemExit("ERROR: missing spec/ directory")

    schemas: list[dict[str, Any]] = []
    for path in sorted(spec_dir.glob("*.schema.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            raise SystemExit(f"ERROR: schema must be an object: {path.relative_to(root)}")
        schemas.append(
            {
                "path": str(path.relative_to(root)),
                "sha256": sha256_file(path),
                "schema_id": data.get("$id") if isinstance(data.get("$id"), str) else None,
                "schema_version": schema_version_from_schema(data),
            }
        )

    channel = read_rust_toolchain_channel(root / "rust-toolchain.toml")

    return {
        "schema_version": "x07.release-manifest@0.1.0",
        "git_sha": read_git_sha(root),
        "rust_toolchain": {"channel": channel},
        "lockfiles": {
            "cargo_lock": {"path": "Cargo.lock", "sha256": sha256_file(cargo_lock)},
            "stdlib_lock": {"path": "stdlib.lock", "sha256": sha256_file(stdlib_lock)},
            "stdlib_os_lock": {"path": "stdlib.os.lock", "sha256": sha256_file(stdlib_os_lock)},
        },
        "schemas": schemas,
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="dist/release-manifest.json", help="Output path (default: dist/release-manifest.json)")
    ap.add_argument("--check", action="store_true", help="Validate generation without writing files")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    root = Path(__file__).resolve().parents[1]

    schema_path = root / "dist" / "release-manifest.schema.json"
    if not schema_path.is_file():
        print("ERROR: missing dist/release-manifest.schema.json", file=sys.stderr)
        return 1
    try:
        schema_obj = json.loads(schema_path.read_text(encoding="utf-8"))
    except Exception as e:
        print(f"ERROR: dist/release-manifest.schema.json invalid JSON: {e}", file=sys.stderr)
        return 1
    if not isinstance(schema_obj, dict):
        print("ERROR: dist/release-manifest.schema.json must be a JSON object", file=sys.stderr)
        return 1

    manifest = build_manifest(root)
    rendered_a = json.dumps(manifest, sort_keys=True, indent=2) + "\n"
    rendered_b = json.dumps(build_manifest(root), sort_keys=True, indent=2) + "\n"
    if rendered_a != rendered_b:
        print("ERROR: release manifest generation is not deterministic", file=sys.stderr)
        return 3

    if args.check:
        print("ok: release manifest can be generated")
        return 0

    out_path = root / args.out
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(rendered_a, encoding="utf-8")
    print(f"ok: wrote {out_path.relative_to(root)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

