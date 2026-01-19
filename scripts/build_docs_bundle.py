from __future__ import annotations

import argparse
import gzip
import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import sys


FIXED_MTIME = 946684800  # 2000-01-01T00:00:00Z


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def iter_docs_files(docs_root: Path) -> list[tuple[str, Path]]:
    files: list[tuple[str, Path]] = []
    for p in docs_root.rglob("*"):
        if not p.is_file():
            continue
        rel = p.relative_to(docs_root)
        if rel.name == ".DS_Store":
            continue
        if any(part.startswith("._") for part in rel.parts):
            continue
        rel_posix = rel.as_posix()
        files.append((rel_posix, p))
    files.sort(key=lambda e: e[0])
    return files


def write_docs_bundle(out_path: Path, docs_root: Path) -> None:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    files = iter_docs_files(docs_root)

    with out_path.open("wb") as f:
        with gzip.GzipFile(
            filename="",
            fileobj=f,
            mode="wb",
            compresslevel=9,
            mtime=0,
        ) as gz:
            with tarfile.open(fileobj=gz, mode="w|") as tf:
                for rel_posix, src in files:
                    if src.is_symlink():
                        raise SystemExit(f"ERROR: docs bundle does not support symlinks: {src}")
                    data = src.read_bytes()
                    info = tarfile.TarInfo(name=f"docs/{rel_posix}")
                    info.type = tarfile.REGTYPE
                    info.mode = 0o644
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    info.mtime = FIXED_MTIME
                    info.size = len(data)
                    tf.addfile(info, io.BytesIO(data))


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tag", required=True, help="Release tag (for example: v0.2.0)")
    ap.add_argument("--docs-dir", type=Path, default=Path("docs"), help="Docs directory (default: docs/)")
    ap.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Output path (default: dist/x07-docs-<tag>.tar.gz)",
    )
    ap.add_argument("--check", action="store_true", help="Fail if the bundle would change")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    root = Path(__file__).resolve().parents[1]

    docs_root = (root / args.docs_dir).resolve()
    if not docs_root.is_dir():
        print(f"ERROR: docs directory not found: {docs_root}", file=sys.stderr)
        return 2

    tag = args.tag.strip()
    if not tag:
        print("ERROR: --tag must be non-empty", file=sys.stderr)
        return 2

    out_path = args.out
    if out_path is None:
        out_path = root / "dist" / f"x07-docs-{tag}.tar.gz"
    else:
        out_path = (root / out_path).resolve() if not out_path.is_absolute() else out_path.resolve()

    if args.check:
        if not out_path.is_file():
            print(f"ERROR: bundle not found: {out_path}", file=sys.stderr)
            return 1
        before = sha256_file(out_path)
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp) / "bundle.tar.gz"
            write_docs_bundle(tmp_path, docs_root)
            after = sha256_file(tmp_path)
        if before != after:
            print(f"ERROR: {out_path} would change", file=sys.stderr)
            return 1
        print(f"ok: docs bundle up to date ({before})")
        return 0

    write_docs_bundle(out_path, docs_root)
    try:
        display_path = str(out_path.relative_to(root))
    except ValueError:
        display_path = str(out_path)
    print(f"ok: wrote {display_path} ({sha256_file(out_path)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
