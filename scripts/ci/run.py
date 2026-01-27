from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPORT_SCHEMA_VERSION = "x07.ci-report@0.1.0"


@dataclass(frozen=True)
class Job:
    name: str
    cmd: list[str]


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def pick_python(root: Path) -> str:
    env = os.environ.get("X07_PYTHON")
    if env:
        return env
    venv = root / ".venv" / "bin" / "python"
    if venv.is_file() and os.access(venv, os.X_OK):
        return str(venv)
    return "python3"


def git_sha(root: Path) -> str:
    env = os.environ.get("GITHUB_SHA")
    if env:
        return env
    if not (root / ".git").exists():
        return "unknown"
    try:
        out = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True)
    except Exception:
        return "unknown"
    return out.strip() or "unknown"


def utc_timestamp() -> str:
    return datetime.now(timezone.utc).isoformat()


def classify_exit_code(code: int) -> str:
    if code == 0:
        return "pass"
    if code == 2:
        return "infra"
    if code == 3:
        return "nondeterminism"
    if code == 4:
        return "contract"
    return "fail"


def overall_exit_code(statuses: list[str]) -> int:
    if all(s == "pass" for s in statuses):
        return 0
    if any(s == "infra" for s in statuses):
        return 2
    if any(s == "contract" for s in statuses):
        return 4
    if any(s == "nondeterminism" for s in statuses):
        return 3
    return 1


def jobs_for_profile(profile: str, root: Path) -> list[Job]:
    python_bin = pick_python(root)

    pr_jobs = [
        Job("tools", ["bash", "scripts/ci/check_tools.sh"]),
        Job("policy.governance", [python_bin, "scripts/check_governance_files.py"]),
        Job(
            "policy.trademarks",
            ["bash", "-c", "test -f TRADEMARKS.md"],
        ),
        Job(
            "policy.release-docs",
            [
                "bash",
                "-c",
                "test -f docs/releases.md && test -f docs/versioning.md && test -f docs/stability.md",
            ],
        ),
        Job("licenses", ["bash", "scripts/ci/check_licenses.sh"]),
        Job("release-manifest", [python_bin, "scripts/build_release_manifest.py", "--check"]),
        Job("cargo.fmt", ["cargo", "fmt", "--check"]),
        Job("cargo.test", ["cargo", "test"]),
        Job("cargo.clippy", ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"]),
        Job("pkg.contracts", [python_bin, "scripts/check_pkg_contracts.py", "--check"]),
        Job("skills", ["bash", "scripts/ci/check_skills.sh"]),
        Job("external-packages.lock", ["bash", "scripts/ci/check_external_packages_lock.sh"]),
        Job("canaries", ["bash", "scripts/ci/check_canaries.sh"]),
        Job("external-packages.os-smoke", ["bash", "scripts/ci/check_external_packages_os_smoke.sh"]),
    ]

    if profile == "dev":
        return [
            Job("tools", ["bash", "scripts/ci/check_tools.sh"]),
            Job("cargo.fmt", ["cargo", "fmt", "--check"]),
            Job("cargo.test", ["cargo", "test"]),
            Job("cargo.clippy", ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"]),
            Job("pkg.contracts", [python_bin, "scripts/check_pkg_contracts.py", "--check"]),
            Job("skills", ["bash", "scripts/ci/check_skills.sh"]),
        ]

    if profile == "pr":
        return pr_jobs

    if profile == "nightly":
        jobs = pr_jobs + [Job("asan.c-backend", ["bash", "scripts/ci/check_asan_c_backend.sh"])]

        suites = root / "labs" / "scripts" / "ci" / "check_suites_h1h2.sh"
        if suites.is_file():
            jobs.append(Job("labs.suites", ["bash", str(suites)]))

        perf = root / "labs" / "scripts" / "ci" / "check_perf_baseline.sh"
        if perf.is_file():
            jobs.append(Job("labs.perf-baseline", ["bash", str(perf)]))

        return jobs

    if profile == "release":
        return pr_jobs + [Job("asan.c-backend", ["bash", "scripts/ci/check_asan_c_backend.sh"])]

    raise ValueError(f"unknown profile: {profile!r}")


def write_json(path: Path, obj: Any, *, sort_keys: bool) -> None:
    if path.parent:
        path.parent.mkdir(parents=True, exist_ok=True)
    txt = json.dumps(obj, indent=2, sort_keys=sort_keys) + "\n"
    path.write_text(txt, encoding="utf-8")


def run_job(root: Path, job: Job, *, logs_dir: Path, jobs_dir: Path) -> dict[str, Any]:
    log_path = logs_dir / f"{job.name}.log"
    started = time.time()
    with log_path.open("wb") as log:
        proc = subprocess.run(job.cmd, cwd=root, stdout=log, stderr=subprocess.STDOUT)
    elapsed_ms = int((time.time() - started) * 1000)
    status = classify_exit_code(proc.returncode)

    job_report = {
        "name": job.name,
        "status": status,
        "exit_code": int(proc.returncode),
        "duration_ms": elapsed_ms,
        "cmd": job.cmd,
        "log_path": str(log_path),
    }
    job_report_path = jobs_dir / f"{job.name}.json"
    job_report["job_report_path"] = str(job_report_path)
    write_json(job_report_path, job_report, sort_keys=True)
    return job_report


def validate_report_strict(report: Any) -> None:
    if not isinstance(report, dict):
        raise ValueError("report must be a JSON object")

    allowed_top_keys = {
        "schema_version",
        "run_id",
        "profile",
        "timestamp",
        "git_sha",
        "ok",
        "exit_code",
        "artifacts_dir",
        "jobs_dir",
        "logs_dir",
        "summary_path",
        "jobs",
    }
    extra = set(report.keys()) - allowed_top_keys
    if extra:
        raise ValueError(f"report has unexpected keys: {sorted(extra)}")
    missing = allowed_top_keys - set(report.keys())
    if missing:
        raise ValueError(f"report is missing required keys: {sorted(missing)}")

    if report["schema_version"] != REPORT_SCHEMA_VERSION:
        raise ValueError(
            f"report.schema_version mismatch: expected {REPORT_SCHEMA_VERSION} got {report['schema_version']!r}"
        )
    if not isinstance(report["run_id"], str) or not report["run_id"]:
        raise ValueError("report.run_id must be a non-empty string")
    if report["profile"] not in ("dev", "pr", "nightly", "release"):
        raise ValueError("report.profile must be one of: dev, pr, nightly, release")
    for k in ("timestamp", "git_sha", "artifacts_dir", "jobs_dir", "logs_dir", "summary_path"):
        if not isinstance(report[k], str) or not report[k]:
            raise ValueError(f"report.{k} must be a non-empty string")
    if not isinstance(report["ok"], bool):
        raise ValueError("report.ok must be a boolean")
    if not isinstance(report["exit_code"], int):
        raise ValueError("report.exit_code must be an integer")
    if not isinstance(report["jobs"], list) or not report["jobs"]:
        raise ValueError("report.jobs must be a non-empty array")

    allowed_job_keys = {
        "name",
        "status",
        "exit_code",
        "duration_ms",
        "cmd",
        "log_path",
        "job_report_path",
    }
    allowed_status = {"pass", "fail", "infra", "nondeterminism", "contract"}
    for idx, job in enumerate(report["jobs"]):
        if not isinstance(job, dict):
            raise ValueError(f"report.jobs[{idx}] must be an object")
        extra_job = set(job.keys()) - allowed_job_keys
        if extra_job:
            raise ValueError(f"report.jobs[{idx}] has unexpected keys: {sorted(extra_job)}")
        missing_job = allowed_job_keys - set(job.keys())
        if missing_job:
            raise ValueError(f"report.jobs[{idx}] is missing keys: {sorted(missing_job)}")
        if not isinstance(job["name"], str) or not job["name"]:
            raise ValueError(f"report.jobs[{idx}].name must be a non-empty string")
        if job["status"] not in allowed_status:
            raise ValueError(f"report.jobs[{idx}].status must be one of {sorted(allowed_status)}")
        if not isinstance(job["exit_code"], int):
            raise ValueError(f"report.jobs[{idx}].exit_code must be an integer")
        if not isinstance(job["duration_ms"], int) or job["duration_ms"] < 0:
            raise ValueError(f"report.jobs[{idx}].duration_ms must be a non-negative integer")
        if not isinstance(job["cmd"], list) or not all(isinstance(s, str) for s in job["cmd"]):
            raise ValueError(f"report.jobs[{idx}].cmd must be an array of strings")
        for p in ("log_path", "job_report_path"):
            if not isinstance(job[p], str) or not job[p]:
                raise ValueError(f"report.jobs[{idx}].{p} must be a non-empty string")


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--profile", required=True, choices=["dev", "pr", "nightly", "release"])
    ap.add_argument("--strict", action="store_true", help="Validate the final JSON report shape")
    ap.add_argument("--progress", action="store_true", help="Print job progress to stderr")
    ap.add_argument(
        "--artifacts-dir",
        type=Path,
        default=None,
        help="Override artifacts output directory (default: artifacts/ci/<run_id>)",
    )
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    root = repo_root()

    run_id = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%SZ") + f"-{os.getpid()}"
    base_dir = args.artifacts_dir or (root / "artifacts" / "ci" / run_id)
    jobs_dir = base_dir / "jobs"
    logs_dir = base_dir / "logs"
    jobs_dir.mkdir(parents=True, exist_ok=True)
    logs_dir.mkdir(parents=True, exist_ok=True)

    jobs = jobs_for_profile(args.profile, root)

    results: list[dict[str, Any]] = []
    statuses: list[str] = []
    for job in jobs:
        if args.progress:
            print(f"==> {job.name}", file=sys.stderr)
        res = run_job(root, job, logs_dir=logs_dir, jobs_dir=jobs_dir)
        if args.progress:
            print(
                f"    {res['status']} (exit={res['exit_code']} duration_ms={res['duration_ms']})",
                file=sys.stderr,
            )
        results.append(res)
        statuses.append(res["status"])

    exit_code = overall_exit_code(statuses)
    report = {
        "schema_version": REPORT_SCHEMA_VERSION,
        "run_id": run_id,
        "profile": args.profile,
        "timestamp": utc_timestamp(),
        "git_sha": git_sha(root),
        "ok": exit_code == 0,
        "exit_code": exit_code,
        "artifacts_dir": str(base_dir),
        "jobs_dir": str(jobs_dir),
        "logs_dir": str(logs_dir),
        "summary_path": str(base_dir / "summary.json"),
        "jobs": results,
    }

    if args.strict:
        validate_report_strict(report)

    summary_path = base_dir / "summary.json"
    write_json(summary_path, report, sort_keys=True)

    print(json.dumps(report, indent=2, sort_keys=True))
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
