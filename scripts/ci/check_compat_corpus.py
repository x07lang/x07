#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, doc: Any) -> None:
    path.write_text(json.dumps(doc, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def run(
    cmd: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    timeout_s: int | None = None,
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
        timeout=timeout_s,
        check=False,
    )


def fail(cmd: list[str], proc: subprocess.CompletedProcess[str]) -> None:
    sys.stderr.write("ERROR: command failed\n")
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


def normalize_dep_path(path_s: str) -> str:
    return path_s.replace("\\", "/")


def apply_dependency_overrides(project_dir: Path, overrides: dict[str, str]) -> None:
    manifest_path = project_dir / "x07.json"
    doc = load_json(manifest_path)
    deps = doc.get("dependencies") or []
    if not isinstance(deps, list):
        raise SystemExit("x07.json: dependencies must be an array")

    missing = set(overrides.keys())
    for dep in deps:
        if not isinstance(dep, dict):
            raise SystemExit(f"x07.json: dependency must be an object: {dep!r}")
        name = dep.get("name")
        if not isinstance(name, str) or not name:
            raise SystemExit(f"x07.json: dependency.name must be a string: {dep!r}")
        if name not in overrides:
            continue
        missing.discard(name)

        new_version = overrides[name]
        old_version = dep.get("version")
        dep["version"] = new_version

        path_s = dep.get("path")
        if isinstance(path_s, str) and path_s:
            path_s = normalize_dep_path(path_s)
            parts = path_s.split("/")
            if parts and isinstance(old_version, str) and parts[-1] == old_version:
                parts[-1] = new_version
                dep["path"] = "/".join(parts)
            else:
                dep["path"] = f".x07/deps/{name}/{new_version}"
        else:
            dep["path"] = f".x07/deps/{name}/{new_version}"

    if missing:
        raise SystemExit(f"x07.json: dependency_overrides refers to missing deps: {sorted(missing)}")

    write_json(manifest_path, doc)


def seed_official_deps(repo: Path, project_dir: Path) -> None:
    doc = load_json(project_dir / "x07.json")
    deps = doc.get("dependencies") or []
    if not isinstance(deps, list):
        raise SystemExit("x07.json: dependencies must be an array")

    for dep in deps:
        if not isinstance(dep, dict):
            raise SystemExit(f"x07.json: dependency must be an object: {dep!r}")
        name = dep.get("name")
        version = dep.get("version")
        rel_path = dep.get("path")
        if not isinstance(name, str) or not name:
            raise SystemExit(f"x07.json: dependency.name must be string: {dep!r}")
        if not isinstance(version, str) or not version:
            raise SystemExit(f"x07.json: dependency.version must be string: {dep!r}")
        if not isinstance(rel_path, str) or not rel_path:
            raise SystemExit(f"x07.json: dependency.path must be string: {dep!r}")

        rel_path = normalize_dep_path(rel_path)
        dst = project_dir / rel_path
        if dst.exists():
            if dst.is_dir():
                shutil.rmtree(dst)
            else:
                raise SystemExit(f"dependency path exists but is not a directory: {dst}")

        src = repo / "packages" / "ext" / f"x07-{name}" / version
        if not src.is_dir():
            raise SystemExit(f"missing official package dir for {name}@{version}: {src}")

        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(src, dst)


def count_error_diags(report: Any) -> int:
    if not isinstance(report, dict):
        return 0
    diags = report.get("diagnostics")
    if not isinstance(diags, list):
        return 0
    n = 0
    for d in diags:
        if not isinstance(d, dict):
            continue
        if d.get("severity") == "error":
            n += 1
    return n


@dataclass(frozen=True)
class CorpusProject:
    id: str
    source: str
    run: bool
    dependency_overrides: dict[str, str]


@dataclass(frozen=True)
class FixCase:
    id: str
    input: str
    world: str
    min_fixable_rate: float


def parse_corpus(doc: Any) -> tuple[list[CorpusProject], list[FixCase]]:
    if not isinstance(doc, dict):
        raise SystemExit("corpus.json: expected an object")

    projects_raw = doc.get("projects")
    if not isinstance(projects_raw, list):
        raise SystemExit("corpus.json: projects must be an array")
    projects: list[CorpusProject] = []
    for item in projects_raw:
        if not isinstance(item, dict):
            raise SystemExit(f"corpus.json: project must be an object: {item!r}")
        pid = item.get("id")
        source = item.get("source")
        run_flag = item.get("run")
        overrides = item.get("dependency_overrides") or {}
        if not isinstance(pid, str) or not pid:
            raise SystemExit(f"corpus.json: project.id must be string: {item!r}")
        if not isinstance(source, str) or not source:
            raise SystemExit(f"corpus.json: project.source must be string: {item!r}")
        if not isinstance(run_flag, bool):
            raise SystemExit(f"corpus.json: project.run must be bool: {item!r}")
        if not isinstance(overrides, dict) or any(
            (not isinstance(k, str) or not isinstance(v, str)) for k, v in overrides.items()
        ):
            raise SystemExit(f"corpus.json: project.dependency_overrides must be object[str,str]: {item!r}")
        projects.append(
            CorpusProject(
                id=pid,
                source=source,
                run=run_flag,
                dependency_overrides=dict(overrides),
            )
        )

    fix_raw = doc.get("fix_cases") or []
    if not isinstance(fix_raw, list):
        raise SystemExit("corpus.json: fix_cases must be an array")
    fixes: list[FixCase] = []
    for item in fix_raw:
        if not isinstance(item, dict):
            raise SystemExit(f"corpus.json: fix case must be an object: {item!r}")
        fid = item.get("id")
        input_path = item.get("input")
        world = item.get("world")
        min_rate = item.get("min_fixable_rate")
        if not isinstance(fid, str) or not fid:
            raise SystemExit(f"corpus.json: fix_cases.id must be string: {item!r}")
        if not isinstance(input_path, str) or not input_path:
            raise SystemExit(f"corpus.json: fix_cases.input must be string: {item!r}")
        if not isinstance(world, str) or not world:
            raise SystemExit(f"corpus.json: fix_cases.world must be string: {item!r}")
        if not isinstance(min_rate, (int, float)):
            raise SystemExit(f"corpus.json: fix_cases.min_fixable_rate must be number: {item!r}")
        fixes.append(
            FixCase(
                id=fid,
                input=input_path,
                world=world,
                min_fixable_rate=float(min_rate),
            )
        )

    return projects, fixes


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the X07 compat corpus checks (Milestone M0).")
    parser.add_argument("--corpus", default="tests/compat_corpus/corpus.json", help="Path to corpus.json")
    parser.add_argument("--x07", required=True, help="Path to x07 binary")
    args = parser.parse_args()

    repo = repo_root()
    corpus_path = (repo / args.corpus).resolve()
    x07_bin = Path(args.x07).resolve()
    if not x07_bin.exists():
        raise SystemExit(f"missing x07 binary: {x07_bin}")
    if not corpus_path.is_file():
        raise SystemExit(f"missing corpus file: {corpus_path}")

    corpus_doc = load_json(corpus_path)
    projects, fix_cases = parse_corpus(corpus_doc)

    started = time.time()
    ok = True

    with tempfile.TemporaryDirectory(prefix="x07_compat_corpus_") as tmp:
        tmp_dir = Path(tmp).resolve()
        work_root = tmp_dir / "work"
        work_root.mkdir(parents=True, exist_ok=True)

        for proj in projects:
            t0 = time.time()
            src_dir = (repo / proj.source).resolve()
            if not src_dir.is_dir():
                raise SystemExit(f"missing project source dir: {src_dir} (from {proj.id})")
            dst_dir = work_root / proj.id
            if dst_dir.exists():
                shutil.rmtree(dst_dir)
            shutil.copytree(src_dir, dst_dir)

            if proj.dependency_overrides:
                apply_dependency_overrides(dst_dir, proj.dependency_overrides)
            seed_official_deps(repo, dst_dir)

            proc = run([str(x07_bin), "pkg", "lock", "--offline"], cwd=dst_dir)
            if proc.returncode != 0:
                ok = False
                sys.stderr.write(f"compat-corpus: project {proj.id}: pkg lock failed\n")
                fail([str(x07_bin), "pkg", "lock", "--offline"], proc)

            proc = run(
                [str(x07_bin), "check", "--project", "x07.json"],
                cwd=dst_dir,
            )
            if proc.returncode != 0:
                ok = False
                sys.stderr.write(f"compat-corpus: project {proj.id}: x07 check failed\n")
                fail([str(x07_bin), "check", "--project", "x07.json"], proc)

            if proj.run:
                proc = run([str(x07_bin), "run", "--project", "x07.json"], cwd=dst_dir, timeout_s=60)
                if proc.returncode != 0:
                    ok = False
                    sys.stderr.write(f"compat-corpus: project {proj.id}: x07 run failed\n")
                    fail([str(x07_bin), "run", "--project", "x07.json"], proc)

            dt_ms = int((time.time() - t0) * 1000)
            sys.stdout.write(f"ok: project {proj.id} ({dt_ms}ms)\n")

        for fix in fix_cases:
            t0 = time.time()
            src = (repo / fix.input).resolve()
            if not src.is_file():
                raise SystemExit(f"missing fix case input: {src} (from {fix.id})")
            case_dir = work_root / f"fixcase.{fix.id}"
            case_dir.mkdir(parents=True, exist_ok=True)
            input_copy = case_dir / "input.x07.json"
            fixed_path = case_dir / "fixed.x07.json"
            shutil.copy2(src, input_copy)

            lint0 = run(
                [str(x07_bin), "lint", "--world", fix.world, "--input", str(input_copy)],
                cwd=case_dir,
            )
            if lint0.returncode == 0:
                raise SystemExit(f"fix case {fix.id}: expected lint to fail before fix")
            try:
                report0 = json.loads(lint0.stdout) if lint0.stdout.strip() else {}
            except json.JSONDecodeError as err:
                raise SystemExit(f"fix case {fix.id}: failed to parse lint JSON: {err}")
            before_errors = count_error_diags(report0)
            if before_errors <= 0:
                raise SystemExit(f"fix case {fix.id}: expected >=1 error diag before fix")

            proc = run(
                [str(x07_bin), "fix", "--world", fix.world, "--input", str(input_copy)],
                cwd=case_dir,
            )
            if proc.returncode != 0:
                ok = False
                sys.stderr.write(f"compat-corpus: fix case {fix.id}: x07 fix failed\n")
                fail([str(x07_bin), "fix", "--world", fix.world, "--input", str(input_copy)], proc)
            fixed_path.write_text(proc.stdout, encoding="utf-8")

            lint1 = run(
                [str(x07_bin), "lint", "--world", fix.world, "--input", str(fixed_path)],
                cwd=case_dir,
            )
            try:
                report1 = json.loads(lint1.stdout) if lint1.stdout.strip() else {}
            except json.JSONDecodeError as err:
                raise SystemExit(f"fix case {fix.id}: failed to parse post-fix lint JSON: {err}")
            after_errors = count_error_diags(report1)
            fixable_rate = (before_errors - after_errors) / float(before_errors)
            if fixable_rate + 1e-9 < fix.min_fixable_rate:
                raise SystemExit(
                    f"fix case {fix.id}: fixable rate {fixable_rate:.3f} below minimum {fix.min_fixable_rate:.3f} "
                    f"(before_errors={before_errors}, after_errors={after_errors})"
                )

            dt_ms = int((time.time() - t0) * 1000)
            sys.stdout.write(
                f"ok: fixcase {fix.id} before_errors={before_errors} after_errors={after_errors} "
                f"fixable_rate={fixable_rate:.3f} ({dt_ms}ms)\n"
            )

    total_ms = int((time.time() - started) * 1000)
    sys.stdout.write(f"ok: compat corpus complete ({total_ms}ms)\n")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
