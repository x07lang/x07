#!/usr/bin/env python3
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from official_deps import seed_official_deps


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def run(
    cmd: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    full_env = os.environ.copy()
    if env:
        full_env.update(env)
    return subprocess.run(
        cmd,
        cwd=str(cwd),
        env=full_env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def fail(label: str, cmd: list[str], proc: subprocess.CompletedProcess[str]) -> None:
    sys.stderr.write(f"ERROR: {label}\n")
    sys.stderr.write(f"  cmd: {' '.join(cmd)}\n")
    sys.stderr.write(f"  exit: {proc.returncode}\n")
    if proc.stdout:
        sys.stderr.write("--- stdout ---\n")
        sys.stderr.write(proc.stdout)
        if not proc.stdout.endswith("\n"):
            sys.stderr.write("\n")
    if proc.stderr:
        sys.stderr.write("--- stderr ---\n")
        sys.stderr.write(proc.stderr)
        if not proc.stderr.endswith("\n"):
            sys.stderr.write("\n")
    raise SystemExit(proc.returncode or 1)


def find_x07_bin(root: Path) -> Path:
    x07_bin = os.environ.get("X07_BIN", "").strip()
    if x07_bin:
        p = Path(x07_bin)
        return p if p.is_absolute() else (root / p)

    proc = run([str(root / "scripts" / "ci" / "find_x07.sh")], cwd=root)
    if proc.returncode != 0:
        fail("find x07 binary", [str(root / "scripts" / "ci" / "find_x07.sh")], proc)
    rel = (proc.stdout or "").strip()
    if not rel:
        raise SystemExit("find_x07.sh produced empty output")
    p = Path(rel)
    return p if p.is_absolute() else (root / p)


def find_doc_projects(root: Path) -> list[Path]:
    out: list[Path] = []
    base = root / "docs" / "examples"
    if not base.is_dir():
        return out
    for path in base.rglob("x07.json"):
        if ".x07" in path.parts:
            continue
        out.append(path.parent)
    out.sort()
    return out


def find_doc_standalone_examples(root: Path) -> list[Path]:
    base = root / "docs" / "examples"
    if not base.is_dir():
        return []
    out = sorted(base.glob("*.x07.json"))
    return [p for p in out if p.is_file()]


@dataclass(frozen=True)
class WorkItem:
    label: str
    work_dir: Path
    manifest_path: Path
    allow_network: bool
    check_lockfile: bool


def prepare_project_work(
    root: Path,
    tmp: Path,
    project_dir: Path,
    *,
    allow_network_if_missing: bool,
) -> WorkItem:
    rel = project_dir.relative_to(root)
    work_dir = tmp / rel
    if work_dir.exists():
        shutil.rmtree(work_dir)
    work_dir.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(
        project_dir,
        work_dir,
        ignore=shutil.ignore_patterns(".x07", "target", "dist", "artifacts", "node_modules"),
    )

    shutil.rmtree(work_dir / ".x07", ignore_errors=True)
    missing = seed_official_deps(root, work_dir, allow_missing=allow_network_if_missing)
    allow_network = bool(missing)

    return WorkItem(
        label=str(rel),
        work_dir=work_dir,
        manifest_path=work_dir / "x07.json",
        allow_network=allow_network,
        check_lockfile=True,
    )


def prepare_standalone_work(root: Path, tmp: Path, prog: Path) -> WorkItem:
    rel = prog.relative_to(root)
    stem = prog.name.rsplit(".", 2)[0]
    work_dir = tmp / "standalone" / stem
    if work_dir.exists():
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    (work_dir / "program.x07.json").write_text(
        prog.read_text(encoding="utf-8"), encoding="utf-8"
    )
    (work_dir / "x07.json").write_text(
        (
            "{\n"
            '  "schema_version": "x07.project@0.5.0",\n'
            '  "compat": "0.5",\n'
            '  "world": "solve-pure",\n'
            '  "entry": "program.x07.json",\n'
            '  "module_roots": ["."],\n'
            '  "dependencies": [],\n'
            '  "lockfile": "x07.lock.json"\n'
            "}\n"
        ),
        encoding="utf-8",
    )

    return WorkItem(
        label=str(rel),
        work_dir=work_dir,
        manifest_path=work_dir / "x07.json",
        allow_network=False,
        check_lockfile=False,
    )


def main() -> int:
    root = repo_root()
    x07_bin = find_x07_bin(root)
    if not x07_bin.exists():
        raise SystemExit(f"missing x07 binary at {x07_bin}")

    env_base = {
        "X07_SANDBOX_BACKEND": os.environ.get("X07_SANDBOX_BACKEND", "os"),
        "X07_I_ACCEPT_WEAKER_ISOLATION": os.environ.get("X07_I_ACCEPT_WEAKER_ISOLATION", "1"),
        "X07_REQUIRE_SOLVERS": os.environ.get("X07_REQUIRE_SOLVERS", "0"),
    }

    projects = find_doc_projects(root)
    standalone = find_doc_standalone_examples(root)
    if not projects and not standalone:
        print("ok: no docs/examples projects found")
        return 0

    with tempfile.TemporaryDirectory(prefix="x07_doc_examples_compile_") as tmp_s:
        tmp = Path(tmp_s)
        items: list[WorkItem] = []
        for project_dir in projects:
            items.append(
                prepare_project_work(
                    root, tmp, project_dir, allow_network_if_missing=True
                )
            )
        for prog in standalone:
            items.append(prepare_standalone_work(root, tmp, prog))

        for item in items:
            if item.check_lockfile:
                pkg_cmd = [
                    str(x07_bin),
                    "pkg",
                    "lock",
                    "--check",
                    "--project",
                    str(item.manifest_path),
                ]
                if not item.allow_network:
                    pkg_cmd.append("--offline")
                proc = run(pkg_cmd, cwd=item.work_dir, env=env_base)
                if proc.returncode != 0:
                    fail(f"{item.label}: pkg lock --check", pkg_cmd, proc)
            else:
                pkg_cmd = [
                    str(x07_bin),
                    "pkg",
                    "lock",
                    "--project",
                    str(item.manifest_path),
                    "--offline",
                ]
                proc = run(pkg_cmd, cwd=item.work_dir, env=env_base)
                if proc.returncode != 0:
                    fail(f"{item.label}: pkg lock", pkg_cmd, proc)

            check_cmd = [str(x07_bin), "check", "--project", str(item.manifest_path)]
            proc = run(check_cmd, cwd=item.work_dir, env=env_base)
            if proc.returncode != 0:
                fail(f"{item.label}: x07 check", check_cmd, proc)

    print("ok: docs examples compile")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
