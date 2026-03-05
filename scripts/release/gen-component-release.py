#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import re
import sys
from typing import Any


COMPONENTS = {
    "x07_core",
    "x07_wasm",
    "x07_web_ui_host",
    "x07_device_host",
}

SEMVER_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")
TAG_RE = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")
UTC_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def sha256_file(path: pathlib.Path) -> tuple[str, int]:
    h = hashlib.sha256()
    n = 0
    with path.open("rb") as f:
        while True:
            chunk = f.read(1024 * 1024)
            if not chunk:
                break
            n += len(chunk)
            h.update(chunk)
    return h.hexdigest(), n


def read_json(path: pathlib.Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def parse_metadata_items(items: list[str]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for item in items:
        if "=" not in item:
            raise ValueError(f"--metadata must be KEY=VALUE, got: {item!r}")
        key, value = item.split("=", 1)
        key = key.strip()
        value = value.strip()
        if not key:
            raise ValueError(f"empty metadata key in: {item!r}")
        out[key] = value
    return out


def infer_asset_kind(component: str, name: str) -> str:
    if name.endswith("-checksums.txt"):
        return "checksums"
    if name.endswith("-attestations.jsonl"):
        return "attestations"
    if name.endswith("-release.json"):
        return "release_manifest"
    if component == "x07_core":
        if name.startswith("x07up-") and (name.endswith(".tar.gz") or name.endswith(".tar.xz") or name.endswith(".zip")):
            return "installer_archive"

    if component == "x07_web_ui_host":
        if name.startswith("x07-web-ui-host-") and name.endswith(".zip"):
            return "host_bundle"

    if component == "x07_device_host":
        if name.startswith("x07-device-host-mobile-templates-") and name.endswith(".zip"):
            return "templates"
        if name.startswith("x07-device-host-abi-") and name.endswith(".json"):
            return "abi_snapshot"

    if name.endswith(".tar.gz") or name.endswith(".tar.xz") or name.endswith(".zip"):
        return "archive"

    raise ValueError(f"cannot infer asset kind for file: {name}")


def infer_target(component: str, version: str, name: str) -> str | None:
    prefixes = {
        "x07_core": [f"x07-{version}-", f"x07-v{version}-"],
        "x07_wasm": [f"x07-wasm-{version}-"],
        "x07_device_host": [f"x07-device-host-desktop-{version}-"],
    }
    if component not in prefixes:
        prefixes = {}
    for prefix in prefixes.get(component, []):
        for suffix in (".tar.gz", ".tar.xz", ".zip"):
            if name.startswith(prefix) and name.endswith(suffix):
                return name[len(prefix) : -len(suffix)]
    if component == "x07_core":
        for prefix in (f"x07up-v{version}-", f"x07up-{version}-"):
            for suffix in (".tar.gz", ".tar.xz", ".zip"):
                if name.startswith(prefix) and name.endswith(suffix):
                    return name[len(prefix) : -len(suffix)]
    return None


def sort_assets(assets: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return sorted(assets, key=lambda asset: (str(asset.get("kind", "")), str(asset.get("name", ""))))


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Generate x07 component release manifest.")
    ap.add_argument("--schema", required=True)
    ap.add_argument("--component", required=True, choices=sorted(COMPONENTS))
    ap.add_argument("--version", required=True)
    ap.add_argument("--tag", required=True)
    ap.add_argument("--repo", required=True)
    ap.add_argument("--assets-dir", required=True, type=pathlib.Path)
    ap.add_argument("--compat-file", required=True, type=pathlib.Path)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    ap.add_argument("--published-at-utc", default=None)
    ap.add_argument("--metadata", action="append", default=[])
    ap.add_argument("--metadata-file", default=None, type=pathlib.Path)
    args = ap.parse_args(argv)

    if args.schema != "x07.component.release@0.1.0":
        print(f"unsupported --schema: {args.schema}", file=sys.stderr)
        return 2
    if not SEMVER_RE.fullmatch(args.version):
        print(f"invalid --version: {args.version}", file=sys.stderr)
        return 2
    if not TAG_RE.fullmatch(args.tag):
        print(f"invalid --tag: {args.tag}", file=sys.stderr)
        return 2

    published_at_utc = args.published_at_utc or utc_now()
    if not UTC_RE.fullmatch(published_at_utc):
        print(f"invalid --published-at-utc: {published_at_utc}", file=sys.stderr)
        return 2

    assets_dir = args.assets_dir.resolve()
    out_path = args.out.resolve()
    if not assets_dir.is_dir():
        print(f"--assets-dir is not a directory: {assets_dir}", file=sys.stderr)
        return 2

    compat_doc = read_json(args.compat_file.resolve())
    if not isinstance(compat_doc, dict):
        print("--compat-file must be a JSON object", file=sys.stderr)
        return 2
    if "x07_core" not in compat_doc:
        print("--compat-file missing x07_core", file=sys.stderr)
        return 2

    metadata: dict[str, Any] = {}
    if args.metadata_file is not None:
        metadata_doc = read_json(args.metadata_file.resolve())
        if not isinstance(metadata_doc, dict):
            print("--metadata-file must be a JSON object", file=sys.stderr)
            return 2
        metadata.update(metadata_doc)

    try:
        metadata.update(parse_metadata_items(args.metadata))
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 2

    repo = args.repo.rstrip("/")
    base_release_url = f"{repo}/releases/download/{args.tag}"

    out_path.parent.mkdir(parents=True, exist_ok=True)

    assets: list[dict[str, Any]] = []
    for path in sorted(assets_dir.iterdir(), key=lambda x: x.name):
        if not path.is_file():
            continue
        if path.suffix == ".tmp":
            continue
        if path.resolve() == out_path:
            continue
        try:
            kind = infer_asset_kind(args.component, path.name)
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        sha, bytes_len = sha256_file(path)
        asset: dict[str, Any] = {
            "name": path.name,
            "kind": kind,
            "url": f"{base_release_url}/{path.name}",
            "sha256": f"sha256:{sha}",
            "bytes_len": bytes_len,
        }
        target = infer_target(args.component, args.version, path.name)
        if target is not None:
            asset["target"] = target
        assets.append(asset)

    if not assets:
        print(f"no assets found in {assets_dir}", file=sys.stderr)
        return 1

    doc: dict[str, Any] = {
        "schema_version": args.schema,
        "component": args.component,
        "version": args.version,
        "tag": args.tag,
        "repo": repo,
        "published_at_utc": published_at_utc,
        "assets": sort_assets(assets),
        "compatibility": compat_doc,
    }
    if metadata:
        doc["metadata"] = metadata

    out_path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    sys.stdout.write(str(out_path) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
