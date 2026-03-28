#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import re
import sys
import urllib.request
from typing import Any


UTC_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")
SEMVER_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")
TAG_RE = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")
CHANNELS = {"stable", "beta", "nightly"}


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def sha256_file(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while True:
            chunk = f.read(1024 * 1024)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def sha256_bytes(data: bytes) -> str:
    h = hashlib.sha256()
    h.update(data)
    return h.hexdigest()


def read_json(path: pathlib.Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def fetch_release_manifest_sha256(url: str, expected_component: str) -> str:
    with urllib.request.urlopen(url) as resp:
        data = resp.read()
    doc = json.loads(data.decode("utf-8"))
    if not isinstance(doc, dict):
        raise ValueError(f"{expected_component} release manifest is not a JSON object")
    if doc.get("schema_version") != "x07.component.release@0.1.0":
        raise ValueError(f"{expected_component} release manifest has unexpected schema_version")
    if doc.get("component") != expected_component:
        raise ValueError(
            f"{expected_component} release manifest component mismatch: {doc.get('component')!r}"
        )
    return f"sha256:{sha256_bytes(data)}"


def validate_component_ref(name: str, obj: Any, expected_component: str) -> dict[str, Any]:
    if not isinstance(obj, dict):
        raise ValueError(f"{name} must be an object")
    required = ["version", "tag", "release_manifest_url"]
    for key in required:
        if key not in obj:
            raise ValueError(f"{name} missing key: {key}")
    version = obj.get("version")
    tag = obj.get("tag")
    if not isinstance(version, str) or not SEMVER_RE.fullmatch(version):
        raise ValueError(f"{name}.version must be semver")
    if not isinstance(tag, str) or not TAG_RE.fullmatch(tag):
        raise ValueError(f"{name}.tag must be a release tag")
    manifest_url = obj.get("release_manifest_url")
    if not isinstance(manifest_url, str) or not manifest_url.startswith("https://"):
        raise ValueError(f"{name}.release_manifest_url must be https://...")

    manifest_sha = obj.get("release_manifest_sha256")
    if manifest_sha == "sha256:" + ("0" * 64):
        manifest_sha = fetch_release_manifest_sha256(manifest_url, expected_component)
    elif not isinstance(manifest_sha, str) or not re.fullmatch(r"^sha256:[0-9a-f]{64}$", manifest_sha):
        manifest_sha = fetch_release_manifest_sha256(manifest_url, expected_component)

    out = dict(obj)
    out["release_manifest_sha256"] = manifest_sha
    return out


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Generate x07 channel bundle manifest.")
    ap.add_argument("--schema", required=True)
    ap.add_argument("--channel", required=True, choices=sorted(CHANNELS))
    ap.add_argument("--input", required=True, type=pathlib.Path)
    ap.add_argument("--core-manifest", required=True, type=pathlib.Path)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    ap.add_argument("--published-at-utc", default=None)
    args = ap.parse_args(argv)

    if args.schema != "x07.release.bundle@0.1.0":
        print(f"unsupported --schema: {args.schema}", file=sys.stderr)
        return 2

    in_doc = read_json(args.input.resolve())
    if not isinstance(in_doc, dict):
        print("--input must be a JSON object", file=sys.stderr)
        return 2

    core_path = args.core_manifest.resolve()
    core_doc = read_json(core_path)
    if not isinstance(core_doc, dict):
        print("--core-manifest must be a JSON object", file=sys.stderr)
        return 2
    if core_doc.get("schema_version") != "x07.component.release@0.1.0":
        print("--core-manifest is not an x07.component.release@0.1.0 document", file=sys.stderr)
        return 2
    if core_doc.get("component") != "x07_core":
        print("--core-manifest is not an x07_core release manifest", file=sys.stderr)
        return 2

    published_at_utc = args.published_at_utc or in_doc.get("published_at_utc") or utc_now()
    if not isinstance(published_at_utc, str) or not UTC_RE.fullmatch(published_at_utc):
        print(f"invalid published_at_utc: {published_at_utc!r}", file=sys.stderr)
        return 2

    min_x07up_version = in_doc.get("min_x07up_version")
    packages = in_doc.get("packages")
    if not isinstance(min_x07up_version, str) or not SEMVER_RE.fullmatch(min_x07up_version):
        print(f"invalid min_x07up_version: {min_x07up_version!r}", file=sys.stderr)
        return 2
    if not isinstance(packages, dict) or "std_web_ui" not in packages:
        print("input missing packages.std_web_ui", file=sys.stderr)
        return 2

    core_repo = str(core_doc["repo"]).rstrip("/")
    core_ref = {
        "version": core_doc["version"],
        "tag": core_doc["tag"],
        "release_manifest_url": f"{core_repo}/releases/download/{core_doc['tag']}/{core_path.name}",
        "release_manifest_sha256": f"sha256:{sha256_file(core_path)}",
    }

    try:
        wasm_ref = validate_component_ref("x07_wasm", in_doc["x07_wasm"], "x07_wasm")
        web_ui_host_ref = validate_component_ref("x07_web_ui_host", in_doc["x07_web_ui_host"], "x07_web_ui_host")
        device_host_ref = validate_component_ref("x07_device_host", in_doc["x07_device_host"], "x07_device_host")
    except KeyError as exc:
        print(f"input missing key: {exc.args[0]}", file=sys.stderr)
        return 2
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 2

    doc = {
        "schema_version": args.schema,
        "channel": args.channel,
        "published_at_utc": published_at_utc,
        "min_x07up_version": min_x07up_version,
        "x07_core": core_ref,
        "x07_wasm": wasm_ref,
        "x07_web_ui_host": web_ui_host_ref,
        "x07_device_host": device_host_ref,
        "packages": packages,
    }

    out_path = args.out.resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    sys.stdout.write(str(out_path) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
