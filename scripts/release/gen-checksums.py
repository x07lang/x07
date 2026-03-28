#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import pathlib
import sys


def sha256_file(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while True:
            chunk = f.read(1024 * 1024)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def should_skip(path: pathlib.Path, out_path: pathlib.Path) -> bool:
    name = path.name
    if path.resolve() == out_path:
        return True
    if name.endswith(".tmp"):
        return True
    if name.endswith("-release.json"):
        return True
    if name.endswith("-bundle.json"):
        return True
    return False


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Generate SHA-256 checksums for release assets.")
    ap.add_argument("--in-dir", required=True, type=pathlib.Path, help="Release directory")
    ap.add_argument("--out", required=True, type=pathlib.Path, help="Output checksums file")
    args = ap.parse_args(argv)

    in_dir = args.in_dir.resolve()
    out_path = args.out.resolve()

    if not in_dir.is_dir():
        print(f"--in-dir is not a directory: {in_dir}", file=sys.stderr)
        return 2

    out_path.parent.mkdir(parents=True, exist_ok=True)

    files: list[pathlib.Path] = []
    for path in sorted(in_dir.iterdir(), key=lambda x: x.name):
        if not path.is_file():
            continue
        if should_skip(path, out_path):
            continue
        files.append(path)

    if not files:
        print(f"no release files found in {in_dir}", file=sys.stderr)
        return 1

    lines = [f"{sha256_file(path)}  {path.name}" for path in files]
    out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    sys.stdout.write(str(out_path) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
