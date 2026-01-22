#!/usr/bin/env python3
from __future__ import annotations

import argparse
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
    return 0


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv[1:]))
