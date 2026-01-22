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


def _run_allow_fail(cmd: list[str], *, cwd: Path) -> tuple[int, str, str]:
    proc = subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    return proc.returncode, proc.stdout, proc.stderr


def _write_index_entry(*, index_root: Path, name: str, version: str, sha256: str) -> None:
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


def _write_dl_file(*, dl_root: Path, name: str, version: str, bytes_: bytes) -> None:
    dl_path = dl_root / name / version / "download"
    dl_path.parent.mkdir(parents=True, exist_ok=True)
    dl_path.write_bytes(bytes_)


def _pack_pkg(*, repo_root: Path, package_dir: Path, out_path: Path) -> str:
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
            str(package_dir),
            "--out",
            str(out_path),
        ],
        cwd=repo_root,
    )
    try:
        pack_report = json.loads(pack_out)
    except Exception as e:
        raise RuntimeError(f"pack output is not valid JSON: {e}\n\n{pack_out}")

    if not isinstance(pack_report, dict) or not pack_report.get("ok"):
        raise RuntimeError(f"pack report is not ok\n\n{pack_out}")

    sha256 = pack_report.get("result", {}).get("sha256", "")
    if not isinstance(sha256, str) or len(sha256) != 64:
        raise RuntimeError(f"pack report missing sha256\n\n{pack_out}")

    return sha256


def _write_exported_defn_module(
    *, path: Path, module_id: str, imports: list[str], fn_name: str, body_expr: object
) -> None:
    doc = {
        "schema_version": "x07.x07ast@0.1.0",
        "kind": "module",
        "module_id": module_id,
        "imports": imports,
        "decls": [
            {"kind": "export", "names": [fn_name]},
            {"kind": "defn", "name": fn_name, "params": [], "result": "bytes", "body": body_expr},
        ],
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, separators=(",", ":")) + "\n", encoding="utf-8")


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
        sha_a = _pack_pkg(repo_root=root, package_dir=fixture_pkg, out_path=pkg_a)
        sha_b = _pack_pkg(repo_root=root, package_dir=fixture_pkg, out_path=pkg_b)

        a_bytes = pkg_a.read_bytes()
        b_bytes = pkg_b.read_bytes()
        if a_bytes != b_bytes:
            print("ERROR: x07 pkg pack is not deterministic (byte mismatch)", file=sys.stderr)
            return 1

        if sha_a != sha_b:
            print("ERROR: x07 pkg pack sha256 mismatch", file=sys.stderr)
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
        _write_index_entry(index_root=index_root, name=name, version=version, sha256=sha_a)

        # Write dl download file.
        _write_dl_file(dl_root=dl_root, name=name, version=version, bytes_=a_bytes)

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

        # Transitive dependency resolution: meta depends on base via package meta.
        base_pkg = tmp / "base_pkg"
        meta_pkg = tmp / "meta_pkg"
        base_modules = base_pkg / "modules"
        meta_modules = meta_pkg / "modules"

        _write_exported_defn_module(
            path=base_modules / "base" / "util.x07.json",
            module_id="base.util",
            imports=[],
            fn_name="base.util.answer",
            body_expr=["bytes.alloc", 0],
        )
        (base_pkg / "x07-package.json").write_text(
            json.dumps(
                {
                    "schema_version": "x07.package@0.1.0",
                    "name": "base",
                    "version": "0.1.0",
                    "module_root": "modules",
                    "modules": ["base.util"],
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )

        meta_module_doc = {
            "schema_version": "x07.x07ast@0.1.0",
            "kind": "module",
            "module_id": "meta.main",
            "imports": ["base.util"],
            "decls": [
                {"kind": "export", "names": ["meta.main.answer"]},
                {
                    "kind": "defn",
                    "name": "meta.main.answer",
                    "params": [],
                    "result": "bytes",
                    "body": ["base.util.answer"],
                },
            ],
        }
        (meta_modules / "meta").mkdir(parents=True, exist_ok=True)
        (meta_modules / "meta" / "main.x07.json").write_text(
            json.dumps(meta_module_doc, separators=(",", ":")) + "\n", encoding="utf-8"
        )
        (meta_pkg / "x07-package.json").write_text(
            json.dumps(
                {
                    "schema_version": "x07.package@0.1.0",
                    "name": "meta",
                    "version": "0.1.0",
                    "module_root": "modules",
                    "modules": ["meta.main"],
                    "meta": {"requires_packages": ["base@0.1.0"]},
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )

        base_pkg_archive = tmp / "base.x07pkg"
        meta_pkg_archive = tmp / "meta.x07pkg"
        base_sha = _pack_pkg(repo_root=root, package_dir=base_pkg, out_path=base_pkg_archive)
        meta_sha = _pack_pkg(repo_root=root, package_dir=meta_pkg, out_path=meta_pkg_archive)
        _write_index_entry(index_root=index_root, name="base", version="0.1.0", sha256=base_sha)
        _write_index_entry(index_root=index_root, name="meta", version="0.1.0", sha256=meta_sha)
        _write_dl_file(
            dl_root=dl_root, name="base", version="0.1.0", bytes_=base_pkg_archive.read_bytes()
        )
        _write_dl_file(
            dl_root=dl_root, name="meta", version="0.1.0", bytes_=meta_pkg_archive.read_bytes()
        )

        trans_project_dir = tmp / "transitive_project"
        (trans_project_dir / "src").mkdir(parents=True)
        trans_project_path = trans_project_dir / "x07.json"
        trans_project_path.write_text(
            json.dumps(
                {
                    "schema_version": "x07.project@0.2.0",
                    "world": "solve-pure",
                    "entry": "src/main.x07.json",
                    "module_roots": ["src"],
                    "dependencies": [
                        {"name": "meta", "version": "0.1.0", "path": ".x07/deps/meta/0.1.0"}
                    ],
                    "lockfile": "x07.lock.json",
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        (trans_project_dir / "src" / "main.x07.json").write_text(
            json.dumps(
                {
                    "schema_version": "x07.x07ast@0.1.0",
                    "kind": "entry",
                    "module_id": "main",
                    "imports": ["meta.main"],
                    "decls": [],
                    "solve": ["meta.main.answer"],
                },
                separators=(",", ":"),
            )
            + "\n",
            encoding="utf-8",
        )

        lockfile = trans_project_dir / "x07.lock.json"
        if lockfile.exists():
            lockfile.unlink()

        code, stdout, stderr = _run_allow_fail(
            [
                "cargo",
                "run",
                "-p",
                "x07",
                "--",
                "pkg",
                "lock",
                "--project",
                str(trans_project_path),
                "--index",
                index_url,
                "--check",
            ],
            cwd=root,
        )
        if code == 0:
            print("ERROR: expected x07 pkg lock --check to fail for missing transitive deps", file=sys.stderr)
            return 1
        try:
            report = json.loads(stdout)
        except Exception:
            print("ERROR: transitive pkg lock --check output is not valid JSON", file=sys.stderr)
            print(stdout, file=sys.stderr)
            print(stderr, file=sys.stderr)
            return 1
        if report.get("ok") is not False:
            print("ERROR: transitive pkg lock --check report.ok must be false", file=sys.stderr)
            print(stdout, file=sys.stderr)
            return 1
        if report.get("error", {}).get("code") != "X07PKG_TRANSITIVE_MISSING":
            print("ERROR: missing transitive deps should produce X07PKG_TRANSITIVE_MISSING", file=sys.stderr)
            print(stdout, file=sys.stderr)
            return 1

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
                str(trans_project_path),
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
                str(trans_project_path),
                "--index",
                index_url,
                "--check",
            ],
            cwd=root,
        )

        updated_project = json.loads(trans_project_path.read_text(encoding="utf-8"))
        dep_names = [d.get("name") for d in updated_project.get("dependencies", [])]
        if sorted(dep_names) != ["base", "meta"]:
            print("ERROR: transitive deps were not written into x07.json", file=sys.stderr)
            print(trans_project_path.read_text(encoding="utf-8"), file=sys.stderr)
            return 1

        _run(
            [
                "cargo",
                "run",
                "-p",
                "x07c",
                "--",
                "build",
                "--project",
                str(trans_project_path),
                "--out",
                str(tmp / "out_transitive.c"),
            ],
            cwd=root,
        )

    print("ok: pkg contracts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
