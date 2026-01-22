#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def iso_utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", required=True, help="Base URL of the local artifacts server (no trailing slash)")
    ap.add_argument("--out", required=True, type=Path)

    ap.add_argument("--tag", required=True, help="Version tag to publish in the manifest (e.g. v0.0.0-ci)")
    ap.add_argument("--target", required=True, help="Target triple key (e.g. x86_64-unknown-linux-gnu)")

    ap.add_argument("--toolchain-file", required=True, type=Path, help="Path to toolchain archive")
    ap.add_argument("--x07up-file", required=True, type=Path, help="Path to x07up archive")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    base = args.base_url.rstrip("/")
    tag = args.tag.strip()
    if not tag:
        raise SystemExit("ERROR: --tag must be non-empty")

    toolchain_path = args.toolchain_file.resolve()
    x07up_path = args.x07up_file.resolve()
    if not toolchain_path.is_file():
        raise SystemExit(f"ERROR: missing toolchain file: {toolchain_path}")
    if not x07up_path.is_file():
        raise SystemExit(f"ERROR: missing x07up file: {x07up_path}")

    toolchain = {
        "url": f"{base}/{toolchain_path.name}",
        "sha256": sha256_file(toolchain_path),
        "size_bytes": toolchain_path.stat().st_size,
        "format": "tar.gz" if toolchain_path.name.endswith(".tar.gz") else "zip",
    }
    x07up = {
        "url": f"{base}/{x07up_path.name}",
        "sha256": sha256_file(x07up_path),
        "size_bytes": x07up_path.stat().st_size,
        "format": "tar.gz" if x07up_path.name.endswith(".tar.gz") else "zip",
    }

    doc = {
        "schema_version": "x07.install.channels@0.1.0",
        "updated_at": iso_utc_now(),
        "channels": {"stable": {"toolchain": tag, "x07up": tag}},
        "toolchains": {
            tag: {
                "published_at": iso_utc_now(),
                "notes": "CI local build",
                "assets": {args.target: toolchain},
                "min_required": {"x07up": tag},
            }
        },
        "x07up": {
            tag: {
                "published_at": iso_utc_now(),
                "notes": "CI local build",
                "assets": {args.target: x07up},
            }
        },
    }

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv[1:]))

