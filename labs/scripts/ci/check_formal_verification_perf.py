#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterable


ROOT = Path(__file__).resolve().parents[3]
BASELINE_SCHEMA_VERSION = "x07.formal_verification.perf_budget@0.1.0"


def detect_host_class() -> str:
    override = os.environ.get("X07_FORMAL_PERF_HOST_CLASS")
    if override:
        return override.strip()
    system = platform.system()
    if system == "Linux":
        vm_backend = (os.environ.get("X07_VM_BACKEND") or "").strip()
        if vm_backend in {"firecracker-ctr", "vz"}:
            return "selfhosted-kvm"
        return "hosted-linux"
    if system == "Darwin":
        vm_backend = (os.environ.get("X07_VM_BACKEND") or "").strip()
        if vm_backend == "vz":
            return "macos-vz"
        return "macos-local"
    return "unsupported"


def time_wrapper() -> list[str] | None:
    p = Path("/usr/bin/time")
    if not (p.is_file() and os.access(p, os.X_OK)):
        return None
    if platform.system() == "Darwin":
        return [str(p), "-l"]
    return [str(p), "-v"]


def parse_peak_rss_kb(stderr: bytes) -> int:
    txt = stderr.decode(errors="replace")
    if platform.system() == "Darwin":
        for line in txt.splitlines():
            if "maximum resident set size" not in line:
                continue
            parts = line.strip().split()
            if not parts:
                continue
            try:
                return int(parts[0]) // 1024
            except ValueError:
                continue
        return 0

    for line in txt.splitlines():
        if "Maximum resident set size (kbytes):" not in line:
            continue
        _, rhs = line.split(":", 1)
        rhs = rhs.strip()
        try:
            return int(rhs)
        except ValueError:
            return 0
    return 0


def path_bytes(path: Path) -> int:
    if not path.exists():
        return 0
    if path.is_file():
        return path.stat().st_size
    total = 0
    for child in path.rglob("*"):
        if child.is_file():
            total += child.stat().st_size
    return total


def render_cmd(cmd: list[str]) -> str:
    return " ".join(subprocess.list2cmdline([part]) for part in cmd)


@dataclass(frozen=True)
class RunMetrics:
    wall_ms: int
    peak_rss_kb: int
    artifacts: dict[str, int]
    artifact_bytes_total: int


@dataclass(frozen=True)
class Scenario:
    name: str
    description: str
    project_dir: Path
    host_classes: set[str]
    needs_solver: bool
    command: Callable[[Path, Path, Path], list[str]]
    artifact_paths: Callable[[Path], dict[str, Path]]
    prepare: Callable[[Path, Path, dict[str, str]], None] | None = None
    env_overrides: dict[str, str] | None = None


def require_solver_tools() -> None:
    missing: list[str] = []
    for exe in ("cbmc", "z3"):
        if shutil.which(exe) is None:
            missing.append(exe)
    if missing:
        raise RuntimeError(f"missing required solver tools on PATH: {', '.join(missing)}")


def x07_bin_path() -> Path:
    override = os.environ.get("X07_BIN")
    if override:
        return Path(override).expanduser().resolve()
    script = ROOT / "scripts" / "ci" / "find_x07.sh"
    res = subprocess.run(
        [str(script)],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    found = res.stdout.strip()
    if not found:
        raise RuntimeError("scripts/ci/find_x07.sh returned an empty path")
    path = Path(found)
    if not path.is_absolute():
        path = ROOT / path
    return path.resolve()


def command_env(x07_bin: Path) -> dict[str, str]:
    env = os.environ.copy()
    bin_dir = str(x07_bin.parent)
    old_path = env.get("PATH") or ""
    env["PATH"] = bin_dir if not old_path else f"{bin_dir}{os.pathsep}{old_path}"
    return env


def run_command_with_rss(*, cmd: list[str], cwd: Path, env: dict[str, str]) -> tuple[subprocess.CompletedProcess[bytes], int, int]:
    wrapped = cmd
    wrapper = time_wrapper()
    if wrapper is not None:
        wrapped = wrapper + cmd
    started = time.perf_counter()
    res = subprocess.run(
        wrapped,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    wall_ms = int((time.perf_counter() - started) * 1000.0)
    peak_rss_kb = parse_peak_rss_kb(res.stderr) if wrapper is not None else 0
    return res, wall_ms, peak_rss_kb


def copy_project_tree(src: Path, dst: Path) -> None:
    ignore = shutil.ignore_patterns(
        ".git",
        ".x07",
        "target",
        "node_modules",
        ".pytest_cache",
        "__pycache__",
    )
    shutil.copytree(src, dst, ignore=ignore)


def unique_artifact_bytes(paths: Iterable[Path]) -> int:
    files: set[Path] = set()
    for path in paths:
        if not path.exists():
            continue
        if path.is_file():
            files.add(path.resolve())
            continue
        for child in path.rglob("*"):
            if child.is_file():
                files.add(child.resolve())
    return sum(file_path.stat().st_size for file_path in files)


def fixture_dir() -> Path:
    return ROOT / "crates" / "x07" / "tests" / "fixtures" / "verified_core_fixture_v1"


def verified_core_example_dir() -> Path:
    return ROOT / "docs" / "examples" / "verified_core_pure_v1"


def trusted_sandbox_example_dir() -> Path:
    return ROOT / "docs" / "examples" / "trusted_sandbox_program_v1"


def verified_core_profile() -> Path:
    return ROOT / "arch" / "trust" / "profiles" / "verified_core_fixture_v1.json"


def make_scenarios(x07_bin: Path) -> dict[str, Scenario]:
    fixture = fixture_dir()
    pure_example = verified_core_example_dir()
    sandbox_example = trusted_sandbox_example_dir()
    fixture_profile = verified_core_profile()

    def coverage_cmd(_project_dir: Path, run_dir: Path, _prepared: Path) -> list[str]:
        return [
            str(x07_bin),
            "verify",
            "--coverage",
            "--project",
            "x07.json",
            "--entry",
            "fixture.main",
            "--report-out",
            str(run_dir / "coverage.report.json"),
            "--quiet-json",
        ]

    def coverage_artifacts(run_dir: Path) -> dict[str, Path]:
        return {
            "coverage_report_bytes": run_dir / "coverage.report.json",
        }

    def prove_cmd(_project_dir: Path, run_dir: Path, _prepared: Path) -> list[str]:
        return [
            str(x07_bin),
            "verify",
            "--prove",
            "--project",
            "x07.json",
            "--entry",
            "fixture.main",
            "--emit-proof",
            str(run_dir / "proof.json"),
            "--report-out",
            str(run_dir / "prove.report.json"),
            "--quiet-json",
        ]

    def prove_artifacts(run_dir: Path) -> dict[str, Path]:
        return {
            "proof_object_bytes": run_dir / "proof.json",
            "proof_check_report_bytes": run_dir / "proof.check.json",
            "proof_summary_bytes": run_dir / "verify.proof-summary.json",
            "proof_smt2_bytes": run_dir / "verify.smt2",
            "proof_solver_output_bytes": run_dir / "z3.out.txt",
            "verify_report_bytes": run_dir / "prove.report.json",
            "proof_bundle_bytes": run_dir,
        }

    def prove_check_prepare(project_dir: Path, prepared_dir: Path, env: dict[str, str]) -> None:
        prepared_dir.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [
                str(x07_bin),
                "verify",
                "--prove",
                "--project",
                "x07.json",
                "--entry",
                "fixture.main",
                "--emit-proof",
                str(prepared_dir / "proof.json"),
            ],
            cwd=project_dir,
            env=env,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def prove_check_cmd(_project_dir: Path, run_dir: Path, _prepared: Path) -> list[str]:
        return [
            str(x07_bin),
            "prove",
            "check",
            "--proof",
            str(run_dir / "proof.json"),
            "--report-out",
            str(run_dir / "provecheck.report.json"),
            "--quiet-json",
        ]

    def prove_check_artifacts(run_dir: Path) -> dict[str, Path]:
        return {
            "prove_check_report_bytes": run_dir / "provecheck.report.json",
        }

    def fixture_certify_cmd(_project_dir: Path, run_dir: Path, _prepared: Path) -> list[str]:
        return [
            str(x07_bin),
            "trust",
            "certify",
            "--project",
            "x07.json",
            "--profile",
            str(fixture_profile),
            "--entry",
            "fixture.main",
            "--out-dir",
            str(run_dir / "cert"),
            "--report-out",
            str(run_dir / "certify.report.json"),
            "--quiet-json",
        ]

    def certify_artifacts(run_dir: Path) -> dict[str, Path]:
        return {
            "certificate_bytes": run_dir / "cert" / "certificate.json",
            "summary_html_bytes": run_dir / "cert" / "summary.html",
            "prove_bundle_bytes": run_dir / "cert" / "prove",
            "cert_bundle_bytes": run_dir / "cert",
            "certify_report_bytes": run_dir / "certify.report.json",
        }

    def example_certify_cmd(_project_dir: Path, run_dir: Path, _prepared: Path) -> list[str]:
        return [
            str(x07_bin),
            "trust",
            "certify",
            "--project",
            "x07.json",
            "--profile",
            "arch/trust/profiles/verified_core_pure_v1.json",
            "--entry",
            "example.main",
            "--out-dir",
            str(run_dir / "cert"),
            "--report-out",
            str(run_dir / "certify.report.json"),
            "--quiet-json",
        ]

    def bundle_cmd(_project_dir: Path, run_dir: Path, _prepared: Path) -> list[str]:
        return [
            str(x07_bin),
            "bundle",
            "--project",
            "x07.json",
            "--out",
            str(run_dir / "example.bundle"),
            "--emit-attestation",
            str(run_dir / "compile.attest.json"),
        ]

    def bundle_artifacts(run_dir: Path) -> dict[str, Path]:
        return {
            "bundle_binary_bytes": run_dir / "example.bundle",
            "bundle_attestation_bytes": run_dir / "compile.attest.json",
        }

    def sandbox_certify_cmd(_project_dir: Path, run_dir: Path, _prepared: Path) -> list[str]:
        return [
            str(x07_bin),
            "trust",
            "certify",
            "--project",
            "x07.json",
            "--profile",
            "arch/trust/profiles/trusted_program_sandboxed_local_v1.json",
            "--entry",
            "example.main",
            "--out-dir",
            str(run_dir / "cert"),
            "--report-out",
            str(run_dir / "certify.report.json"),
            "--quiet-json",
        ]

    return {
        "verified_core_fixture.coverage": Scenario(
            name="verified_core_fixture.coverage",
            description="x07 verify --coverage on the strict verified-core fixture",
            project_dir=fixture,
            host_classes={"hosted-linux", "selfhosted-kvm", "macos-local", "macos-vz"},
            needs_solver=False,
            command=coverage_cmd,
            artifact_paths=coverage_artifacts,
            prepare=None,
        ),
        "verified_core_fixture.prove": Scenario(
            name="verified_core_fixture.prove",
            description="x07 verify --prove --emit-proof on the strict verified-core fixture",
            project_dir=fixture,
            host_classes={"hosted-linux", "selfhosted-kvm", "macos-local", "macos-vz"},
            needs_solver=True,
            command=prove_cmd,
            artifact_paths=prove_artifacts,
            prepare=None,
        ),
        "verified_core_fixture.prove_check": Scenario(
            name="verified_core_fixture.prove_check",
            description="x07 prove check on a pre-generated verified-core proof object",
            project_dir=fixture,
            host_classes={"hosted-linux", "selfhosted-kvm", "macos-local", "macos-vz"},
            needs_solver=True,
            command=prove_check_cmd,
            artifact_paths=prove_check_artifacts,
            prepare=prove_check_prepare,
        ),
        "verified_core_fixture.certify": Scenario(
            name="verified_core_fixture.certify",
            description="x07 trust certify on the strict verified-core fixture",
            project_dir=fixture,
            host_classes={"hosted-linux", "selfhosted-kvm", "macos-local", "macos-vz"},
            needs_solver=True,
            command=fixture_certify_cmd,
            artifact_paths=certify_artifacts,
            prepare=None,
        ),
        "verified_core_pure_example.certify": Scenario(
            name="verified_core_pure_example.certify",
            description="x07 trust certify on docs/examples/verified_core_pure_v1",
            project_dir=pure_example,
            host_classes={"hosted-linux", "selfhosted-kvm", "macos-local", "macos-vz"},
            needs_solver=True,
            command=example_certify_cmd,
            artifact_paths=certify_artifacts,
            prepare=None,
        ),
        "verified_core_pure_example.bundle_attestation": Scenario(
            name="verified_core_pure_example.bundle_attestation",
            description="x07 bundle --emit-attestation on docs/examples/verified_core_pure_v1",
            project_dir=pure_example,
            host_classes={"hosted-linux", "selfhosted-kvm", "macos-local", "macos-vz"},
            needs_solver=False,
            command=bundle_cmd,
            artifact_paths=bundle_artifacts,
            prepare=None,
        ),
        "trusted_sandbox_program.certify": Scenario(
            name="trusted_sandbox_program.certify",
            description="x07 trust certify on docs/examples/trusted_sandbox_program_v1",
            project_dir=sandbox_example,
            host_classes={"selfhosted-kvm", "macos-vz"},
            needs_solver=True,
            command=sandbox_certify_cmd,
            artifact_paths=certify_artifacts,
            prepare=None,
            env_overrides={
                "X07_SANDBOX_BACKEND": "vm",
                "X07_I_ACCEPT_WEAKER_ISOLATION": "0",
            },
        ),
    }


def load_json(path: Path) -> dict[str, Any]:
    obj = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(obj, dict):
        raise ValueError(f"expected JSON object at {path}")
    return obj


def median_int(values: list[int]) -> int:
    return int(statistics.median(values))


def scenario_enabled_for_host(doc: dict[str, Any], scenario_name: str, host_class: str) -> bool:
    scenario = doc.get("scenarios", {}).get(scenario_name)
    if not isinstance(scenario, dict):
        return False
    host_cfg = scenario.get("hosts", {}).get(host_class)
    if not isinstance(host_cfg, dict):
        return False
    return bool(host_cfg.get("enabled", False))


def scenario_runs_for_host(doc: dict[str, Any], scenario_name: str, host_class: str) -> int:
    scenario = doc["scenarios"][scenario_name]
    host_cfg = scenario["hosts"][host_class]
    runs = host_cfg.get("runs")
    if not isinstance(runs, int) or runs <= 0:
        raise ValueError(f"{scenario_name} host {host_class} is missing a positive runs count")
    return runs


def scenario_budgets(doc: dict[str, Any], scenario_name: str, host_class: str) -> dict[str, Any]:
    scenario = doc["scenarios"][scenario_name]
    host_cfg = scenario["hosts"][host_class]
    budgets = host_cfg.get("budgets")
    if not isinstance(budgets, dict):
        raise ValueError(f"{scenario_name} host {host_class} is missing budgets")
    out: dict[str, Any] = {}
    for key, value in budgets.items():
        metric = str(key)
        if isinstance(value, int) and value >= 0:
            out[metric] = {
                "budget": value,
            }
            continue
        if not isinstance(value, dict):
            raise ValueError(f"{scenario_name} host {host_class} budget {metric} must be an int or object")
        baseline = value.get("baseline")
        max_regression_pct = value.get("max_regression_pct")
        if not isinstance(baseline, int) or baseline < 0:
            raise ValueError(f"{scenario_name} host {host_class} budget {metric} baseline must be a non-negative int")
        if not isinstance(max_regression_pct, int) or max_regression_pct < 0:
            raise ValueError(
                f"{scenario_name} host {host_class} budget {metric} max_regression_pct must be a non-negative int"
            )
        out[metric] = {
            "baseline": baseline,
            "max_regression_pct": max_regression_pct,
            "budget": int(math.ceil(baseline * (100 + max_regression_pct) / 100.0)),
        }
    return out


def host_enforced(doc: dict[str, Any], host_class: str) -> bool:
    host_cfg = doc.get("host_classes", {}).get(host_class)
    if not isinstance(host_cfg, dict):
        return False
    return bool(host_cfg.get("enforce", False))


def run_scenario(
    *,
    scenario: Scenario,
    x07_bin: Path,
    host_class: str,
    runs: int,
) -> dict[str, Any]:
    if scenario.needs_solver:
        require_solver_tools()

    env = command_env(x07_bin)
    if scenario.env_overrides:
        env.update(scenario.env_overrides)
    per_run: list[dict[str, Any]] = []

    with tempfile.TemporaryDirectory(prefix=f"x07_formal_perf_{scenario.name.replace('.', '_')}_") as tmp_root_str:
        tmp_root = Path(tmp_root_str)
        prepared_dir = tmp_root / "prepared"
        prepared_workspace = tmp_root / "prepared-workspace"
        if scenario.prepare is not None:
            copy_project_tree(scenario.project_dir, prepared_workspace)
            scenario.prepare(prepared_workspace, prepared_dir, env)

        for idx in range(runs):
            run_dir = tmp_root / f"run-{idx + 1}"
            run_dir.mkdir(parents=True, exist_ok=True)
            project_dir = run_dir / "workspace"
            copy_project_tree(scenario.project_dir, project_dir)
            if prepared_dir.exists():
                for src in prepared_dir.rglob("*"):
                    rel = src.relative_to(prepared_dir)
                    dst = run_dir / rel
                    if src.is_dir():
                        dst.mkdir(parents=True, exist_ok=True)
                    elif src.is_file():
                        dst.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copy2(src, dst)

            cmd = scenario.command(project_dir, run_dir, prepared_dir)
            res, wall_ms, peak_rss_kb = run_command_with_rss(cmd=cmd, cwd=project_dir, env=env)
            if res.returncode != 0:
                raise RuntimeError(
                    "\n".join(
                        [
                            f"scenario {scenario.name!r} failed on host class {host_class}",
                            f"command: {render_cmd(cmd)}",
                            f"exit_code: {res.returncode}",
                            "stdout:",
                            res.stdout.decode(errors='replace'),
                            "stderr:",
                            res.stderr.decode(errors='replace'),
                        ]
                    )
                )

            artifacts = {
                key: path_bytes(path)
                for key, path in scenario.artifact_paths(run_dir).items()
            }
            artifact_bytes_total = unique_artifact_bytes(scenario.artifact_paths(run_dir).values())
            per_run.append(
                {
                    "run": idx + 1,
                    "wall_ms": wall_ms,
                    "peak_rss_kb": peak_rss_kb,
                    "artifact_bytes_total": artifact_bytes_total,
                    "artifacts": artifacts,
                }
            )

    wall_values = [int(run["wall_ms"]) for run in per_run]
    rss_values = [int(run["peak_rss_kb"]) for run in per_run]
    artifact_total_values = [int(run["artifact_bytes_total"]) for run in per_run]
    artifact_keys = sorted({key for run in per_run for key in run["artifacts"].keys()})
    medians = {
        "wall_ms": median_int(wall_values),
        "peak_rss_kb": median_int(rss_values),
        "artifact_bytes_total": median_int(artifact_total_values),
        "artifacts": {
            key: median_int([int(run["artifacts"].get(key, 0)) for run in per_run])
            for key in artifact_keys
        },
    }

    return {
        "name": scenario.name,
        "description": scenario.description,
        "runs": per_run,
        "median": medians,
    }


def compare_budget(*, name: str, observed: int, budget: int, label: str) -> str | None:
    if observed > budget:
        return f"{name}: {label} budget exceeded (observed={observed} budget={budget})"
    return None


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--baseline",
        default="labs/benchmarks/perf/formal_verification.json",
        help="Formal verification perf budget JSON",
    )
    ap.add_argument(
        "--report-out",
        default=None,
        help="Optional output path for the observed perf report",
    )
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    host_class = detect_host_class()
    baseline_path = (ROOT / args.baseline).resolve() if not Path(args.baseline).is_absolute() else Path(args.baseline).resolve()
    baseline = load_json(baseline_path)

    if baseline.get("schema_version") != BASELINE_SCHEMA_VERSION:
        raise SystemExit(
            f"ERROR: unexpected formal verification perf schema_version: {baseline.get('schema_version')!r}"
        )

    if host_class == "unsupported":
        print("ok: formal verification perf skipped (unsupported host)", file=sys.stderr)
        return 0

    x07_bin = x07_bin_path()
    scenarios = make_scenarios(x07_bin)

    enabled = [
        name
        for name, scenario in scenarios.items()
        if host_class in scenario.host_classes and scenario_enabled_for_host(baseline, name, host_class)
    ]
    if not enabled:
        print(f"ok: formal verification perf skipped (no enabled scenarios for {host_class})")
        return 0

    observed_results: list[dict[str, Any]] = []
    failures: list[str] = []
    for name in enabled:
        scenario = scenarios[name]
        runs = scenario_runs_for_host(baseline, name, host_class)
        print(f"[perf] {name} ({host_class}, runs={runs})")
        result = run_scenario(
            scenario=scenario,
            x07_bin=x07_bin,
            host_class=host_class,
            runs=runs,
        )
        budgets = scenario_budgets(baseline, name, host_class)
        median = result["median"]
        comparisons = {
            "wall_ms": {
                "observed": median["wall_ms"],
                **budgets.get("wall_ms", {}),
            },
            "peak_rss_kb": {
                "observed": median["peak_rss_kb"],
                **budgets.get("peak_rss_kb", {}),
            },
            "artifact_bytes_total": {
                "observed": median["artifact_bytes_total"],
                **budgets.get("artifact_bytes_total", {}),
            },
        }
        result["budgets"] = comparisons
        observed_results.append(result)

        maybe_failure = compare_budget(
            name=name,
            observed=int(median["wall_ms"]),
            budget=int(budgets["wall_ms"]["budget"]),
            label="wall_ms",
        )
        if maybe_failure:
            failures.append(maybe_failure)
        maybe_failure = compare_budget(
            name=name,
            observed=int(median["peak_rss_kb"]),
            budget=int(budgets["peak_rss_kb"]["budget"]),
            label="peak_rss_kb",
        )
        if maybe_failure:
            failures.append(maybe_failure)
        maybe_failure = compare_budget(
            name=name,
            observed=int(median["artifact_bytes_total"]),
            budget=int(budgets["artifact_bytes_total"]["budget"]),
            label="artifact_bytes_total",
        )
        if maybe_failure:
            failures.append(maybe_failure)

    enforced = host_enforced(baseline, host_class)
    report = {
        "schema_version": "x07.formal_verification.perf_report@0.1.0",
        "host_class": host_class,
        "enforced": enforced,
        "baseline_path": str(baseline_path),
        "results": observed_results,
        "ok": len(failures) == 0,
        "failures": failures,
    }

    if args.report_out:
        out_path = (ROOT / args.report_out).resolve() if not Path(args.report_out).is_absolute() else Path(args.report_out).resolve()
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"wrote: {out_path}")

    if failures and enforced:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    if failures:
        for failure in failures:
            print(f"WARN: {failure}", file=sys.stderr)
        print(f"ok: formal verification perf visibility report completed for {host_class}")
        return 0

    print(f"ok: formal verification perf budgets passed for {host_class}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
