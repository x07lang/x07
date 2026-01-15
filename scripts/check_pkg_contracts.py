from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def _file_url(path: Path) -> str:
    return path.resolve().as_uri()


def _index_rel_path(name: str) -> str:
    # Cargo-style sharding:
    # 1 -> 1/name
    # 2 -> 2/name
    # 3 -> 3/<first>/name
    # >=4 -> <first2>/<next2>/name
    if name != name.lower() or not name.isascii():
        raise ValueError(f"package name must be lowercase ASCII: {name!r}")
    if any(ch not in "abcdefghijklmnopqrstuvwxyz0123456789-_" for ch in name):
        raise ValueError(f"package name contains invalid chars: {name!r}")

    if len(name) == 1:
        shard = "1"
    elif len(name) == 2:
        shard = "2"
    elif len(name) == 3:
        shard = f"3/{name[0]}"
    else:
        shard = f"{name[0:2]}/{name[2:4]}"
    return f"{shard}/{name}"


def _run(cmd: list[str], *, cwd: Path) -> str:
    proc = subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed ({proc.returncode}): {' '.join(cmd)}\n\nstdout:\n{proc.stdout}\n\nstderr:\n{proc.stderr}"
        )
    return proc.stdout


def main(argv: list[str]) -> int:
    if argv != ["--check"]:
        print("usage: check_pkg_contracts.py --check", file=sys.stderr)
        return 2

    root = Path(__file__).resolve().parents[1]
    fixture_pkg = root / "tests" / "fixtures" / "pkg" / "hello_pkg" / "0.1.0"
    fixture_project = root / "tests" / "fixtures" / "pkg" / "hello_project"

    if not fixture_pkg.is_dir():
        print(f"ERROR: missing fixture package dir: {fixture_pkg}", file=sys.stderr)
        return 1
    if not fixture_project.is_dir():
        print(f"ERROR: missing fixture project dir: {fixture_project}", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory(prefix="x07_pkg_contracts_") as td:
        tmp = Path(td)

        # Create a local file-backed sparse index + dl tree.
        registry_root = tmp / "registry"
        index_root = registry_root / "index"
        dl_root = registry_root / "dl"
        index_root.mkdir(parents=True)
        dl_root.mkdir(parents=True)

        # Pack the fixture package twice and byte-compare.
        pkg_a = tmp / "hello_a.x07pkg"
        pkg_b = tmp / "hello_b.x07pkg"
        _run(
            [
                "cargo",
                "run",
                "-p",
                "x07",
                "--",
                "pkg",
                "pack",
                "--package",
                str(fixture_pkg),
                "--out",
                str(pkg_a),
            ],
            cwd=root,
        )
        _run(
            [
                "cargo",
                "run",
                "-p",
                "x07",
                "--",
                "pkg",
                "pack",
                "--package",
                str(fixture_pkg),
                "--out",
                str(pkg_b),
            ],
            cwd=root,
        )

        a_bytes = pkg_a.read_bytes()
        b_bytes = pkg_b.read_bytes()
        if a_bytes != b_bytes:
            print("ERROR: x07 pkg pack is not deterministic (byte mismatch)", file=sys.stderr)
            return 1

        pack_out = _run(
            [
                "cargo",
                "run",
                "-p",
                "x07",
                "--",
                "pkg",
                "pack",
                "--package",
                str(fixture_pkg),
                "--out",
                str(pkg_a),
            ],
            cwd=root,
        )
        try:
            pack_report = json.loads(pack_out)
        except Exception as e:
            print(f"ERROR: pack output is not valid JSON: {e}", file=sys.stderr)
            print(pack_out, file=sys.stderr)
            return 1

        if not isinstance(pack_report, dict) or not pack_report.get("ok"):
            print("ERROR: pack report is not ok", file=sys.stderr)
            print(pack_out, file=sys.stderr)
            return 1

        sha256 = pack_report.get("result", {}).get("sha256", "")
        if not isinstance(sha256, str) or len(sha256) != 64:
            print("ERROR: pack report missing sha256", file=sys.stderr)
            print(pack_out, file=sys.stderr)
            return 1

        # Write index config.json (Cargo-style fields).
        config = {
            "dl": _file_url(dl_root) + "/",
            "api": "http://localhost:8080/v1",
            "auth-required": False,
        }
        (index_root / "config.json").write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8")

        # Write per-package index entry file.
        name = "hello"
        version = "0.1.0"
        entry_path = index_root / _index_rel_path(name)
        entry_path.parent.mkdir(parents=True, exist_ok=True)
        entry = {
            "schema_version": "x07.index-entry@0.1.0",
            "name": name,
            "version": version,
            "cksum": sha256,
            "yanked": False,
        }
        entry_path.write_text(json.dumps(entry, separators=(",", ":")) + "\n", encoding="utf-8")

        # Write dl download file.
        dl_path = dl_root / name / version / "download"
        dl_path.parent.mkdir(parents=True, exist_ok=True)
        dl_path.write_bytes(a_bytes)

        # Copy the fixture project into temp so we don't pollute the repo.
        project_dir = tmp / "project"
        shutil.copytree(fixture_project, project_dir)
        lockfile = project_dir / "x07.lock.json"
        if lockfile.exists():
            lockfile.unlink()

        index_url = "sparse+" + _file_url(index_root) + "/"

        _run(
            [
                "cargo",
                "run",
                "-p",
                "x07",
                "--",
                "pkg",
                "lock",
                "--project",
                str(project_dir / "x07.json"),
                "--index",
                index_url,
            ],
            cwd=root,
        )
        _run(
            [
                "cargo",
                "run",
                "-p",
                "x07",
                "--",
                "pkg",
                "lock",
                "--project",
                str(project_dir / "x07.json"),
                "--index",
                index_url,
                "--check",
            ],
            cwd=root,
        )

        # Sanity: project build sees the dependency module.
        _run(
            [
                "cargo",
                "run",
                "-p",
                "x07c",
                "--",
                "build",
                "--project",
                str(project_dir / "x07.json"),
                "--out",
                str(tmp / "out.c"),
            ],
            cwd=root,
        )

    print("ok: pkg contracts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
