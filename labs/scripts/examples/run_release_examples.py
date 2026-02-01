#!/usr/bin/env python3
import argparse
import base64
import json
import os
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent.parent


def _decode_b64(b64_text: str) -> bytes:
    if not b64_text:
        return b""
    return base64.b64decode(b64_text.encode("ascii"))


def _try_utf8(data: bytes) -> Optional[str]:
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def _run(cmd: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )


def _ensure_release_fixtures(root: Path) -> None:
    fixtures_dir = root / "labs" / "examples" / "release" / "fixtures"
    artifact_path = fixtures_dir / "artifact_audit.tar.gz"
    zip_path = fixtures_dir / "zip_grep.zip"
    if artifact_path.exists() and zip_path.exists():
        return

    gen = root / "labs" / "scripts" / "examples" / "generate_release_fixtures.py"
    print("Generating labs/examples/release/fixtures (tar.gz + zip)...")
    proc = _run([sys.executable, str(gen)], cwd=root)
    if proc.returncode != 0:
        raise RuntimeError(
            "fixture generation failed:\n"
            f"cmd: {proc.args}\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}\n"
        )


def _host_runner_exe(root: Path, cargo_profile: str) -> Path:
    exe_name = "x07-host-runner.exe" if os.name == "nt" else "x07-host-runner"
    profile_dir = "release" if cargo_profile == "release" else "debug"
    exe = root / "target" / profile_dir / exe_name
    if exe.exists():
        return exe

    print(f"Building x07-host-runner ({cargo_profile})...")
    cmd = ["cargo", "build", "-p", "x07-host-runner"]
    if cargo_profile == "release":
        cmd.append("--release")
    proc = _run(cmd, cwd=root)
    if proc.returncode != 0:
        raise RuntimeError(
            "building x07-host-runner failed:\n"
            f"cmd: {proc.args}\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}\n"
        )
    if not exe.exists():
        raise RuntimeError(f"expected runner at {exe} after build")
    return exe


@dataclass(frozen=True)
class Example:
    name: str
    args: list[str]


def _examples(root: Path, input_dir_file: Path) -> list[Example]:
    release = root / "labs" / "examples" / "release"
    return [
        Example(
            name="artifact_audit",
            args=[
                "--project",
                str(release / "artifact_audit.x07project.json"),
                "--world",
                "solve-pure",
                "--input",
                str(release / "fixtures" / "artifact_audit.tar.gz"),
            ],
        ),
        Example(
            name="zip_grep",
            args=[
                "--project",
                str(release / "zip_grep.x07project.json"),
                "--world",
                "solve-pure",
                "--input",
                str(release / "fixtures" / "zip_grep.zip"),
            ],
        ),
        Example(
            name="full_pipeline",
            args=[
                "--project",
                str(release / "full_pipeline.x07project.json"),
                "--world",
                "solve-full",
                "--fixture-fs-dir",
                str(release / "full_pipeline" / "fixtures" / "fs"),
                "--fixture-rr-dir",
                str(release / "full_pipeline" / "fixtures" / "rr"),
                "--fixture-kv-dir",
                str(release / "full_pipeline" / "fixtures" / "kv"),
                "--fixture-kv-seed",
                "seed.json",
                "--input",
                str(input_dir_file),
            ],
        ),
    ]


def _print_metrics(report: dict, wall_ms: float) -> None:
    compile = report.get("compile") if report.get("mode") != "solve" else None
    solve = report.get("solve") if report.get("mode") != "solve" else report

    if isinstance(compile, dict):
        print(
            "compile:",
            "ok=" + str(bool(compile.get("ok"))),
            "fuel_used=" + str(compile.get("fuel_used")),
            "c_source_size=" + str(compile.get("c_source_size")),
            "compiled_exe_size=" + str(compile.get("compiled_exe_size")),
        )

    if isinstance(solve, dict):
        mem_stats = solve.get("mem_stats") or {}
        sched_stats = solve.get("sched_stats") or {}
        print(
            "solve:",
            "ok=" + str(bool(solve.get("ok"))),
            "fuel_used=" + str(solve.get("fuel_used")),
            "heap_used=" + str(solve.get("heap_used")),
            "peak_live_bytes=" + str(mem_stats.get("peak_live_bytes")),
            "bytes_alloc_total=" + str(mem_stats.get("bytes_alloc_total")),
            "tasks_spawned=" + str(sched_stats.get("tasks_spawned")),
            "ctx_switches=" + str(sched_stats.get("ctx_switches")),
            "fs_read_file_calls=" + str(solve.get("fs_read_file_calls")),
            "fs_list_dir_calls=" + str(solve.get("fs_list_dir_calls")),
            "rr_open_calls=" + str(solve.get("rr_open_calls")),
            "rr_close_calls=" + str(solve.get("rr_close_calls")),
            "rr_next_calls=" + str(solve.get("rr_next_calls")),
            "rr_next_miss_calls=" + str(solve.get("rr_next_miss_calls")),
            "rr_append_calls=" + str(solve.get("rr_append_calls")),
            "kv_get_calls=" + str(solve.get("kv_get_calls")),
            "kv_set_calls=" + str(solve.get("kv_set_calls")),
        )

    print(f"host: wall_ms={wall_ms:.2f}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--cargo-profile",
        choices=["debug", "release"],
        default="debug",
        help="Build x07-host-runner with this Cargo profile.",
    )
    parser.add_argument("--all", action="store_true", help="Run all examples.")
    parser.add_argument("--artifact-audit", action="store_true")
    parser.add_argument("--zip-grep", action="store_true")
    parser.add_argument("--full-pipeline", action="store_true")
    parser.add_argument("--report-out", type=Path, default=None)
    parser.add_argument("--raw-runner-json", action="store_true")
    args = parser.parse_args()

    root = _repo_root()
    _ensure_release_fixtures(root)
    runner = _host_runner_exe(root, args.cargo_profile)

    wants_all = args.all or not any(
        [args.artifact_audit, args.zip_grep, args.full_pipeline]
    )

    with tempfile.TemporaryDirectory(prefix="x07-release-examples-") as td:
        input_dir_file = Path(td) / "full_pipeline.input"
        input_dir_file.write_bytes(b"data")

        results: list[dict] = []
        for ex in _examples(root, input_dir_file):
            if not wants_all and not getattr(args, ex.name):
                continue

            print(f"=== {ex.name} ===")
            start = time.perf_counter()
            proc = _run([str(runner), *ex.args], cwd=root)
            wall_ms = (time.perf_counter() - start) * 1000.0

            if proc.returncode != 0:
                raise RuntimeError(
                    f"x07-host-runner failed for {ex.name}:\n"
                    f"cmd: {proc.args}\n"
                    f"stdout:\n{proc.stdout}\n"
                    f"stderr:\n{proc.stderr}\n"
            )

            report = json.loads(proc.stdout)
            compact = report

            solve = compact.get("solve") if compact.get("mode") != "solve" else compact
            solve_output_b64 = (
                solve.get("solve_output_b64") if isinstance(solve, dict) else None
            )
            if isinstance(solve_output_b64, str):
                out_bytes = _decode_b64(solve_output_b64)
                out_text = _try_utf8(out_bytes)
                if out_text is None:
                    print("solve_output: (non-utf8) b64=" + solve_output_b64)
                else:
                    print(out_text.rstrip("\r\n"))

            _print_metrics(compact, wall_ms)

            if args.raw_runner_json:
                print(json.dumps(compact, indent=2, sort_keys=True))

            results.append(
                {
                    "example": ex.name,
                    "wall_ms": wall_ms,
                    "report": compact,
                }
            )

    if args.report_out is not None:
        args.report_out.parent.mkdir(parents=True, exist_ok=True)
        args.report_out.write_text(json.dumps(results, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
