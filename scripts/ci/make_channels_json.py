#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", required=True, help="Base URL of the local artifacts server (no trailing slash)")
    ap.add_argument("--out", required=True, type=Path)

    ap.add_argument("--tag", required=True, help="Version tag to publish in the manifest (e.g. v0.0.0-ci)")
    ap.add_argument("--target", required=True, help="Target triple key (e.g. x86_64-unknown-linux-gnu)")

    ap.add_argument("--toolchain-file", required=True, type=Path, help="Path to toolchain archive")
    ap.add_argument("--x07up-file", required=True, type=Path, help="Path to x07up archive")
    return ap.parse_args(argv)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def archive_format(path: Path) -> str:
    if path.name.endswith(".tar.gz"):
        return "tar.gz"
    if path.name.endswith(".zip"):
        return "zip"
    raise SystemExit(f"ERROR: unsupported archive extension: {path.name}")


def write_json(path: Path, doc: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def local_component_ref(*, base_url: str, name: str, version: str) -> dict[str, str]:
    return {
        "version": version,
        "tag": f"v{version}",
        "release_manifest_url": f"{base_url}/{name}",
        "release_manifest_sha256": "sha256:" + ("0" * 64),
    }


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    toolchain_path = args.toolchain_file.resolve()
    x07up_path = args.x07up_file.resolve()

    if not toolchain_path.is_file():
        raise SystemExit(f"ERROR: missing toolchain file: {toolchain_path}")
    if not x07up_path.is_file():
        raise SystemExit(f"ERROR: missing x07up file: {x07up_path}")
    if toolchain_path.parent != x07up_path.parent:
        raise SystemExit("ERROR: --toolchain-file and --x07up-file must be in the same directory")

    scripts_dir = Path(__file__).resolve().parent.parent
    build_script = scripts_dir / "build_channels_json.py"
    if not build_script.is_file():
        raise SystemExit(f"ERROR: build script not found: {build_script}")

    args.out.parent.mkdir(parents=True, exist_ok=True)

    subprocess.check_call(
        [
            sys.executable,
            str(build_script),
            "--tag",
            args.tag,
            "--dist",
            str(toolchain_path.parent),
            "--base-url",
            args.base_url,
            "--out",
            str(args.out),
        ]
    )

    version = args.tag.removeprefix("v")
    release_name = f"x07-{version}-release.json"
    release_path = toolchain_path.parent / release_name
    release_doc = {
        "schema_version": "x07.component.release@0.1.0",
        "component": "x07_core",
        "version": version,
        "tag": args.tag,
        "repo": "https://example.invalid/x07-local-smoke",
        "published_at_utc": "2026-03-05T00:00:00Z",
        "assets": [
            {
                "name": toolchain_path.name,
                "kind": "archive",
                "url": f"{args.base_url}/{toolchain_path.name}",
                "sha256": f"sha256:{sha256_file(toolchain_path)}",
                "bytes_len": toolchain_path.stat().st_size,
                "target": args.target,
            },
            {
                "name": x07up_path.name,
                "kind": "installer_archive",
                "url": f"{args.base_url}/{x07up_path.name}",
                "sha256": f"sha256:{sha256_file(x07up_path)}",
                "bytes_len": x07up_path.stat().st_size,
                "target": args.target,
            },
        ],
        "compatibility": {
            "x07_core": version,
        },
    }
    write_json(release_path, release_doc)

    stable_path = args.out.parent / "channels" / "stable.json"
    stable_doc = {
        "schema_version": "x07.release.bundle@0.1.0",
        "channel": "stable",
        "published_at_utc": "2026-03-05T00:20:00Z",
        "min_x07up_version": version,
        "x07_core": {
            "version": version,
            "tag": args.tag,
            "release_manifest_url": f"{args.base_url}/{release_name}",
            "release_manifest_sha256": f"sha256:{sha256_file(release_path)}",
        },
        "x07_wasm": local_component_ref(
            base_url=args.base_url,
            name="x07-wasm-placeholder-release.json",
            version=version,
        ),
        "x07_web_ui_host": local_component_ref(
            base_url=args.base_url,
            name="x07-web-ui-host-placeholder-release.json",
            version=version,
        ),
        "x07_device_host": local_component_ref(
            base_url=args.base_url,
            name="x07-device-host-placeholder-release.json",
            version=version,
        ),
        "packages": {
            "std_web_ui": version,
        },
    }
    write_json(stable_path, stable_doc)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv[1:]))
