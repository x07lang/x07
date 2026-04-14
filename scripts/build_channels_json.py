#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
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
    ap.add_argument("--tag", required=True, help="Release tag (for example: v0.2.3)")
    ap.add_argument("--dist", type=Path, default=Path("dist"), help="Artifacts directory (default: dist)")
    ap.add_argument(
        "--base-url",
        default="",
        help="Base URL used for asset downloads (default: GitHub releases/download/<tag>)",
    )
    ap.add_argument("--out", type=Path, default=Path("dist/channels.json"), help="Output path (default: dist/channels.json)")
    ap.add_argument(
        "--channel-out-dir",
        type=Path,
        default=Path("dist/channels"),
        help="Directory for per-channel bundle manifests (default: dist/channels)",
    )
    ap.add_argument(
        "--bundle",
        type=Path,
        default=None,
        help="Optional x07.release.bundle@0.1.0 file to publish as channels/stable.json",
    )
    return ap.parse_args(argv)


def artifact_entry(*, path: Path, url: str, fmt: str) -> dict:
    return {
        "url": url,
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
        "format": fmt,
    }


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    tag = args.tag.strip()
    if not tag.startswith("v"):
        raise SystemExit(f"ERROR: --tag must start with 'v': {tag!r}")

    dist = args.dist.resolve()
    if not dist.is_dir():
        raise SystemExit(f"ERROR: dist dir not found: {dist}")

    base_url = args.base_url.strip().rstrip("/")
    if not base_url:
        base_url = f"https://github.com/x07lang/x07/releases/download/{tag}"

    published_at = iso_utc_now()
    updated_at = iso_utc_now()

    targets = [
        ("x86_64-unknown-linux-gnu", "tar.gz"),
        ("aarch64-unknown-linux-gnu", "tar.gz"),
        ("x86_64-apple-darwin", "tar.gz"),
        ("aarch64-apple-darwin", "tar.gz"),
        ("x86_64-pc-windows-msvc", "zip"),
    ]

    toolchain_assets: dict[str, dict] = {}
    x07up_assets: dict[str, dict] = {}

    for target, fmt in targets:
        toolchain_candidates = [
            f"x07-{tag}-{target}.{fmt}",
            f"x07-{tag.removeprefix('v')}-{target}.{fmt}",
        ]
        toolchain_path = None
        toolchain_name = None
        for candidate in toolchain_candidates:
            candidate_path = dist / candidate
            if candidate_path.is_file():
                toolchain_path = candidate_path
                toolchain_name = candidate
                break
        if toolchain_path is None or toolchain_name is None:
            toolchain_path = dist / toolchain_candidates[0]
            toolchain_name = toolchain_candidates[0]
        if toolchain_path.is_file():
            toolchain_assets[target] = artifact_entry(
                path=toolchain_path,
                url=f"{base_url}/{toolchain_name}",
                fmt=fmt,
            )

        x07up_name = f"x07up-{tag}-{target}.{fmt}"
        x07up_path = dist / x07up_name
        if x07up_path.is_file():
            x07up_assets[target] = artifact_entry(
                path=x07up_path,
                url=f"{base_url}/{x07up_name}",
                fmt=fmt,
            )

    if not toolchain_assets:
        raise SystemExit("ERROR: no toolchain assets found under dist/ (expected x07-<tag>-<target>.*)")
    if not x07up_assets:
        raise SystemExit("ERROR: no x07up assets found under dist/ (expected x07up-<tag>-<target>.*)")

    components: dict[str, dict] = {}
    docs_name = f"x07-docs-{tag}.tar.gz"
    docs_path = dist / docs_name
    if docs_path.is_file():
        components["docs"] = artifact_entry(path=docs_path, url=f"{base_url}/{docs_name}", fmt="tar.gz")

    skills_name = f"x07-skills-{tag}.tar.gz"
    skills_path = dist / skills_name
    if skills_path.is_file():
        components["skills"] = artifact_entry(path=skills_path, url=f"{base_url}/{skills_name}", fmt="tar.gz")

    toolchain_release: dict = {
        "published_at": published_at,
        "notes": "Stable release.",
        "assets": toolchain_assets,
        "min_required": {"x07up": tag},
    }
    if components:
        toolchain_release["components"] = components

    doc = {
        "schema_version": "x07.install.channels@0.1.0",
        "updated_at": updated_at,
        "channels": {"stable": {"toolchain": tag, "x07up": tag}},
        "toolchains": {tag: toolchain_release},
        "x07up": {tag: {"published_at": published_at, "assets": x07up_assets}},
    }

    out = args.out.resolve()
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"ok: wrote {out}")

    bundle_path = args.bundle
    if bundle_path is None:
        bundle_path = dist / "release" / f"x07-{tag.removeprefix('v')}-bundle.json"
    bundle_path = bundle_path.resolve()
    if bundle_path.is_file():
        channel_dir = args.channel_out_dir.resolve()
        channel_dir.mkdir(parents=True, exist_ok=True)
        stable_path = channel_dir / "stable.json"
        try:
            bundle_doc = json.loads(bundle_path.read_text(encoding="utf-8"))
        except Exception as e:
            raise SystemExit(f"ERROR: failed to parse bundle manifest {bundle_path}: {e}")
        if not isinstance(bundle_doc, dict):
            raise SystemExit(f"ERROR: bundle manifest must be a JSON object: {bundle_path}")
        if bundle_doc.get("schema_version") != "x07.release.bundle@0.1.0":
            raise SystemExit(
                f"ERROR: unexpected bundle schema_version: {bundle_doc.get('schema_version')!r}"
            )
        if bundle_doc.get("channel") != "stable":
            raise SystemExit(f"ERROR: bundle manifest is not stable: {bundle_path}")
        shutil.copy2(bundle_path, stable_path)
        print(f"ok: wrote {stable_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv[1:]))
