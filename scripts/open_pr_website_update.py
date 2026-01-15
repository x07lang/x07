from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


WEBSITE_VERSIONS_SCHEMA_VERSION = "x07.website-versions@0.1.0"


@dataclass(frozen=True, order=True)
class SemverKey:
    major: int
    minor: int
    patch: int
    rest: str


SEMVER_RE = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)(.*)$")


def semver_key(v: str) -> SemverKey:
    m = SEMVER_RE.match(v.strip())
    if not m:
        return SemverKey(0, 0, 0, v)
    return SemverKey(int(m.group(1)), int(m.group(2)), int(m.group(3)), m.group(4))


def read_versions(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        raise SystemExit(f"ERROR: parse {path}: {e}")
    if not isinstance(data, dict):
        raise SystemExit(f"ERROR: {path} must be a JSON object")
    if data.get("schema_version") != WEBSITE_VERSIONS_SCHEMA_VERSION:
        raise SystemExit(f"ERROR: {path} schema_version mismatch")
    if not isinstance(data.get("versions"), list):
        raise SystemExit(f"ERROR: {path} versions must be an array")
    return data


def render_versions(data: dict[str, Any]) -> str:
    return json.dumps(data, sort_keys=True, indent=2) + "\n"


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--versions", type=Path, default=Path("versions.json"))
    ap.add_argument("--tag", required=True, help="Release tag (for example: v0.2.0)")
    ap.add_argument("--release-base-url", required=True, help="Base URL for release assets (no trailing slash required)")
    ap.add_argument("--check", action="store_true", help="Validate without writing files")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    versions_path: Path = args.versions

    tag = args.tag.strip()
    base = args.release_base_url.rstrip("/")

    data = read_versions(versions_path)

    entry = {
        "version": tag,
        "release_base_url": base,
        "release_manifest_url": f"{base}/release-manifest.json",
        "skills_pack_url": f"{base}/x07-skills-{tag}.tar.gz",
    }

    versions = data["versions"]
    assert isinstance(versions, list)

    out: list[dict[str, Any]] = []
    replaced = False
    for item in versions:
        if not isinstance(item, dict):
            continue
        if item.get("version") == tag:
            out.append(entry)
            replaced = True
        else:
            out.append(item)
    if not replaced:
        out.append(entry)

    out.sort(key=lambda v: semver_key(str(v.get("version", ""))))
    data["versions"] = out

    rendered = render_versions(data)
    if args.check:
        if versions_path.read_text(encoding="utf-8") != rendered:
            print(f"ERROR: {versions_path} would change", file=sys.stderr)
            return 1
        print("ok: versions.json up to date")
        return 0

    versions_path.write_text(rendered, encoding="utf-8")
    print(f"ok: updated {versions_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
