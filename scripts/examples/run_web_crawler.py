#!/usr/bin/env python3
import argparse
import base64
import json
import os
import subprocess
import sys
import time
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


def _os_runner_exe(root: Path, cargo_profile: str) -> Path:
    exe_name = "x07-os-runner.exe" if os.name == "nt" else "x07-os-runner"
    profile_dir = "release" if cargo_profile == "release" else "debug"
    exe = root / "target" / profile_dir / exe_name
    if exe.exists():
        return exe

    print(f"Building x07-os-runner ({cargo_profile})...")
    cmd = ["cargo", "build", "-p", "x07-os-runner"]
    if cargo_profile == "release":
        cmd.append("--release")
    proc = _run(cmd, cwd=root)
    if proc.returncode != 0:
        raise RuntimeError(
            "building x07-os-runner failed:\n"
            f"cmd: {proc.args}\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}\n"
        )
    if not exe.exists():
        raise RuntimeError(f"expected runner at {exe} after build")
    return exe


def _ensure_ext_fs_staged(root: Path) -> None:
    deps = root / "deps" / "x07"
    if (deps / "libx07_ext_fs.a").exists() or (deps / "x07_ext_fs.lib").exists():
        return

    if os.name == "nt":
        raise RuntimeError(
            "ext-fs backend is not staged under deps/x07; on Windows, build it via "
            "`bash scripts/build_ext_fs.sh` in WSL2 or stage the built .lib manually."
        )

    print("Staging ext-fs native backend (deps/x07/...)...")
    proc = _run(["bash", str(root / "scripts" / "build_ext_fs.sh")], cwd=root)
    if proc.returncode != 0:
        raise RuntimeError(
            "scripts/build_ext_fs.sh failed:\n"
            f"cmd: {proc.args}\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}\n"
        )


def _print_metrics(report: dict, wall_ms: float) -> None:
    compile = report.get("compile") if report.get("mode") != "run-os" else None
    solve = report.get("solve") if report.get("mode") != "run-os" else report

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
        )

    print(f"host: wall_ms={wall_ms:.2f}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--cargo-profile",
        choices=["debug", "release"],
        default="debug",
        help="Build x07-os-runner with this Cargo profile.",
    )
    parser.add_argument(
        "--config",
        type=Path,
        default=Path("examples/release/web_crawler/config.example.yaml"),
        help="Path to crawler YAML config (passed as --input to x07-os-runner).",
    )
    parser.add_argument("--compiled-out", type=Path, default=None)
    parser.add_argument("--raw-runner-json", action="store_true")
    args = parser.parse_args()

    root = _repo_root()
    _ensure_ext_fs_staged(root)
    runner = _os_runner_exe(root, args.cargo_profile)

    release = root / "examples" / "release"
    project = release / "web_crawler.x07project.json"
    if not project.exists():
        raise RuntimeError(f"missing {project}")
    config = (root / args.config).resolve() if not args.config.is_absolute() else args.config
    if not config.exists():
        raise RuntimeError(f"missing config file: {config}")

    cmd = [
        str(runner),
        "--project",
        str(project),
        "--world",
        "run-os",
        "--input",
        str(config),
        "--auto-ffi",
    ]
    if args.compiled_out is not None:
        cmd += ["--compiled-out", str(args.compiled_out)]

    print("Running:", " ".join(cmd))
    start = time.perf_counter()
    proc = _run(cmd, cwd=root)
    wall_ms = (time.perf_counter() - start) * 1000.0

    if proc.returncode != 0 and not proc.stdout.strip():
        raise RuntimeError(
            "x07-os-runner failed:\n"
            f"cmd: {proc.args}\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}\n"
        )

    report = json.loads(proc.stdout)
    if args.raw_runner_json:
        print(json.dumps(report, indent=2, sort_keys=True))
        return

    solve = report.get("solve") if report.get("mode") != "run-os" else report
    if isinstance(solve, dict):
        solve_out = _decode_b64(str(solve.get("solve_output_b64") or ""))
        stdout = _decode_b64(str(solve.get("stdout_b64") or ""))
        stderr = _decode_b64(str(solve.get("stderr_b64") or ""))

        if stdout:
            print("--- stdout ---")
            sys.stdout.write(_try_utf8(stdout) or repr(stdout))
            if _try_utf8(stdout) is not None and not stdout.endswith(b"\n"):
                print()
        if stderr:
            print("--- stderr ---")
            sys.stdout.write(_try_utf8(stderr) or repr(stderr))
            if _try_utf8(stderr) is not None and not stderr.endswith(b"\n"):
                print()

        print("--- solve output ---")
        txt = _try_utf8(solve_out)
        if txt is not None:
            sys.stdout.write(txt)
            if not solve_out.endswith(b"\n"):
                print()
        else:
            print(f"<non-utf8 solve output: {len(solve_out)} bytes>")

    _print_metrics(report, wall_ms)

    if proc.stderr.strip():
        print("--- x07-os-runner stderr ---")
        print(proc.stderr.rstrip())


if __name__ == "__main__":
    main()

