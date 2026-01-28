from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import subprocess
import sys


def infer_toolchain_repo(website_root: Path) -> Path | None:
    candidates = [
        website_root.parent,
        website_root.parent / "x07",
    ]
    for candidate in candidates:
        if not candidate.is_dir():
            continue
        if not (candidate / "Cargo.toml").is_file():
            continue
        if not (candidate / "docs").is_dir():
            continue
        return candidate.resolve()
    return None


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tag", required=True, help="Release tag (for example: v0.2.0)")
    ap.add_argument("--bundle", type=Path, required=True, help="Path to docs bundle tar.gz (from x07)")
    ap.add_argument(
        "--channels-json",
        type=Path,
        default=None,
        help="Path to channels.json to publish (default: <toolchain-repo>/dist/channels.json)",
    )
    ap.add_argument("--published-at-utc", default=None)
    ap.add_argument("--set-latest", action="store_true")
    ap.add_argument("--check", action="store_true", help="Validate without writing files")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    website_root = Path.cwd()

    def sync_file(*, src: Path, dst: Path) -> None:
        if not src.is_file():
            raise ValueError(f"missing source file: {src}")
        if args.check:
            if not dst.is_file():
                raise ValueError(f"[CHECK] missing: {dst.relative_to(website_root)}")
            if dst.read_bytes() != src.read_bytes():
                raise ValueError(f"[CHECK] out of date: {dst.relative_to(website_root)}")
            return
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)

    sync_script = website_root / "scripts" / "sync_from_bundle.py"
    if not sync_script.is_file():
        print(
            f"ERROR: expected x07-website checkout (missing {sync_script})",
            file=sys.stderr,
        )
        return 2

    tag = args.tag.strip()
    toolchain_version = tag.removeprefix("v")
    if toolchain_version == tag:
        print(f"ERROR: expected --tag like vX.Y.Z, got: {tag!r}", file=sys.stderr)
        return 2

    bundle_path = args.bundle.resolve()
    if not bundle_path.is_file():
        print(f"ERROR: docs bundle not found: {bundle_path}", file=sys.stderr)
        return 2

    toolchain_repo = infer_toolchain_repo(website_root)
    if toolchain_repo is None:
        print(
            "ERROR: unable to locate x07 toolchain repo (expected nested checkout or sibling ./x07)",
            file=sys.stderr,
        )
        return 2

    cmd = [
        sys.executable,
        str(sync_script),
        "--toolchain-version",
        toolchain_version,
        "--bundle",
        str(bundle_path),
        "--toolchain-repo",
        str(toolchain_repo),
    ]
    if args.published_at_utc is not None:
        cmd.extend(["--published-at-utc", str(args.published_at_utc)])
    if args.set_latest:
        cmd.append("--set-latest")
    if args.check:
        cmd.append("--check")

    subprocess.check_call(cmd)

    try:
        sync_file(
            src=toolchain_repo / "dist" / "install" / "install.sh",
            dst=website_root / "site" / "static" / "install.sh",
        )

        channels_path = args.channels_json
        if channels_path is None:
            channels_path = toolchain_repo / "dist" / "channels.json"
        sync_file(
            src=channels_path.resolve(),
            dst=website_root / "site" / "static" / "install" / "channels.json",
        )
    except ValueError as e:
        msg = str(e)
        print(f"ERROR: {msg}", file=sys.stderr)
        if args.check and msg.startswith("[CHECK]"):
            return 1
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
