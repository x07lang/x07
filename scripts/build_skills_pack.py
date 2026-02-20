from __future__ import annotations

import argparse
import hashlib
import io
import os
import tarfile
from pathlib import Path
import sys
import gzip
import json


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def iter_dirs_files(root: Path) -> tuple[list[Path], list[Path]]:
    dirs: list[Path] = []
    files: list[Path] = []
    for p in root.rglob("*"):
        if p.is_dir():
            dirs.append(p)
        elif p.is_file():
            files.append(p)
    dirs.sort(key=lambda p: p.relative_to(root).as_posix())
    files.sort(key=lambda p: p.relative_to(root).as_posix())
    return dirs, files


def add_dir(tar: tarfile.TarFile, arcname: str) -> None:
    ti = tarfile.TarInfo(name=arcname.rstrip("/") + "/")
    ti.type = tarfile.DIRTYPE
    ti.mode = 0o755
    ti.uid = 0
    ti.gid = 0
    ti.uname = ""
    ti.gname = ""
    ti.mtime = 0
    tar.addfile(ti)


def add_file(tar: tarfile.TarFile, path: Path, arcname: str) -> None:
    data = path.read_bytes()
    ti = tarfile.TarInfo(name=arcname)
    ti.type = tarfile.REGTYPE
    ti.mode = 0o644
    ti.uid = 0
    ti.gid = 0
    ti.uname = ""
    ti.gname = ""
    ti.mtime = 0
    ti.size = len(data)
    tar.addfile(ti, io.BytesIO(data))


def add_bytes(tar: tarfile.TarFile, data: bytes, arcname: str) -> None:
    ti = tarfile.TarInfo(name=arcname)
    ti.type = tarfile.REGTYPE
    ti.mode = 0o644
    ti.uid = 0
    ti.gid = 0
    ti.uname = ""
    ti.gname = ""
    ti.mtime = 0
    ti.size = len(data)
    tar.addfile(ti, io.BytesIO(data))


def skills_pack_meta_bytes(tag: str) -> bytes:
    toolchain_tag = tag.strip()
    toolchain_version = toolchain_tag[1:] if toolchain_tag.startswith("v") else toolchain_tag
    meta = {
        "schema_version": "x07.skills.pack-meta@0.1.0",
        "toolchain_tag": toolchain_tag,
        "toolchain_version": toolchain_version,
    }
    return (json.dumps(meta, sort_keys=True, indent=2) + "\n").encode("utf-8")


def build_skills_pack_bytes(root: Path, *, tag: str | None) -> bytes:
    skills_root = root / "skills" / "pack" / ".agent" / "skills"
    if not skills_root.is_dir():
        raise SystemExit("ERROR: missing skills/pack/.agent/skills/")

    dirs, files = iter_dirs_files(skills_root)

    out = io.BytesIO()
    with gzip.GzipFile(fileobj=out, mode="wb", mtime=0) as gz:
        with tarfile.open(fileobj=gz, mode="w") as tar:
            add_dir(tar, ".agent")
            add_dir(tar, ".agent/skills")
            for d in dirs:
                rel = d.relative_to(skills_root).as_posix()
                add_dir(tar, f".agent/skills/{rel}")
            for f in files:
                rel = f.relative_to(skills_root).as_posix()
                add_file(tar, f, f".agent/skills/{rel}")
            if tag is not None:
                add_bytes(tar, skills_pack_meta_bytes(tag), ".agent/skills/_pack_meta.json")
    return out.getvalue()


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="dist/x07-skills.tar.gz", help="Output path")
    ap.add_argument("--tag", default=None, help="Release tag (for example: v0.1.26)")
    ap.add_argument("--check", action="store_true", help="Validate determinism and (if present) output content")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    root = Path(__file__).resolve().parents[1]

    tag = args.tag.strip() if isinstance(args.tag, str) and args.tag.strip() else None

    a = build_skills_pack_bytes(root, tag=tag)
    b = build_skills_pack_bytes(root, tag=tag)
    if a != b:
        print("ERROR: skills pack generation is not deterministic", file=sys.stderr)
        return 3

    out_path = root / args.out
    if args.check:
        if out_path.exists():
            existing = out_path.read_bytes()
            if existing != a:
                print(f"ERROR: {out_path} is out of date", file=sys.stderr)
                return 1
        print("ok: skills pack can be generated")
        return 0

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_bytes(a)
    print(f"ok: wrote {out_path.relative_to(root)} (sha256={sha256_hex(a)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
