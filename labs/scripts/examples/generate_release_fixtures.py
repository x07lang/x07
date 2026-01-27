#!/usr/bin/env python3
import gzip
import io
import json
import tarfile
import zipfile
from pathlib import Path


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent.parent


def _write_file(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def _add_tar_bytes(tf: tarfile.TarFile, name: str, data: bytes, mode: int = 0o644) -> None:
    info = tarfile.TarInfo(name=name)
    info.size = len(data)
    info.mtime = 0
    info.mode = mode
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    tf.addfile(info, io.BytesIO(data))


def _build_artifact_tar_gz() -> bytes:
    payload = bytes(range(256)) * 16
    manifest = {
        "name": "x07-release-fixture",
        "version": "0.1.0",
        "files": [
            {"path": "manifest.json", "len": None},
            {"path": "payload.bin", "len": len(payload)},
        ],
    }
    manifest_bytes = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode("utf-8")
    manifest["files"][0]["len"] = len(manifest_bytes)
    manifest_bytes = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode("utf-8")

    tar_buf = io.BytesIO()
    with tarfile.open(fileobj=tar_buf, mode="w") as tf:
        _add_tar_bytes(tf, "manifest.json", manifest_bytes)
        _add_tar_bytes(tf, "payload.bin", payload, mode=0o644)
        _add_tar_bytes(tf, "notes.txt", b"fixture_for_x07_release_testing\n", mode=0o644)

    gz_buf = io.BytesIO()
    with gzip.GzipFile(fileobj=gz_buf, mode="wb", mtime=0) as gz:
        gz.write(tar_buf.getvalue())
    return gz_buf.getvalue()


def _add_zip_bytes(
    zf: zipfile.ZipFile, name: str, data: bytes, mode: int = 0o644
) -> None:
    info = zipfile.ZipInfo(filename=name, date_time=(1980, 1, 1, 0, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    info.create_system = 3  # Unix
    info.external_attr = (mode & 0xFFFF) << 16
    zf.writestr(info, data)


def _build_zip_grep_zip() -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, mode="w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        _add_zip_bytes(zf, "alpha.txt", b"alpha\nTODO one\nTODO two\n")
        _add_zip_bytes(zf, "notes.md", b"# Notes\n\nNothing here.\nTODO: doc follow-up\n")
        _add_zip_bytes(zf, "binary.bin", bytes(range(32)), mode=0o644)
    return buf.getvalue()


def main() -> None:
    root = _repo_root()
    out_dir = root / "examples" / "release" / "fixtures"

    artifact_path = out_dir / "artifact_audit.tar.gz"
    zip_path = out_dir / "zip_grep.zip"

    _write_file(artifact_path, _build_artifact_tar_gz())
    _write_file(zip_path, _build_zip_grep_zip())

    print(f"Wrote {artifact_path.relative_to(root)}")
    print(f"Wrote {zip_path.relative_to(root)}")


if __name__ == "__main__":
    main()
