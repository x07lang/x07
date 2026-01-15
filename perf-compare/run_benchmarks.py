#!/usr/bin/env python3
"""
Performance comparison benchmarks: X07 vs C vs Rust

Measures execution time, memory usage, compile time, and final build size
across three languages.
"""
from __future__ import annotations

import argparse
import json
import os
import random
import shutil
import statistics
import struct
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


_HOST_RUNNER_CACHE: dict[str, Path] = {}


def _x07_host_runner_prefix(host_runner: Path, cc_profile: str) -> list[str]:
    cmd = [str(host_runner)]
    if cc_profile != "default":
        cmd += ["--cc-profile", cc_profile]
    return cmd


def _ensure_host_runner(repo_root: Path) -> Path:
    key = str(repo_root)
    cached = _HOST_RUNNER_CACHE.get(key)
    if cached is not None and cached.is_file() and os.access(cached, os.X_OK):
        return cached

    exe_name = "x07-host-runner.exe" if sys.platform.startswith("win") else "x07-host-runner"
    exe = repo_root / "target" / "release" / exe_name
    subprocess.run(
        ["cargo", "build", "--release", "-p", "x07-host-runner"],
        cwd=str(repo_root),
        check=True,
        capture_output=True,
    )
    if not (exe.is_file() and os.access(exe, os.X_OK)):
        raise RuntimeError(f"x07-host-runner binary not found at {exe}")

    # Warm the binary once so benchmark timings don't include first-run cold-start overhead
    # (macOS in particular can be noticeably slower on the first exec after a build).
    subprocess.run(
        [str(exe), "--help"],
        cwd=str(repo_root),
        check=True,
        capture_output=True,
    )

    _HOST_RUNNER_CACHE[key] = exe
    return exe


def _time_bin() -> list[str] | None:
    p = Path("/usr/bin/time")
    if not (p.is_file() and os.access(p, os.X_OK)):
        return None
    if sys.platform == "darwin":
        return [str(p), "-l"]
    return [str(p), "-v"]


def _parse_time_max_rss_kb(stderr: bytes) -> int:
    txt = stderr.decode(errors="replace")

    # macOS (/usr/bin/time -l): max RSS in bytes.
    if sys.platform == "darwin":
        for line in txt.splitlines():
            if "maximum resident set size" not in line:
                continue
            parts = line.strip().split()
            if not parts:
                continue
            try:
                rss_bytes = int(parts[0])
                return rss_bytes // 1024
            except ValueError:
                continue
        return 0

    # GNU time (/usr/bin/time -v): max RSS in kbytes.
    for line in txt.splitlines():
        if "Maximum resident set size (kbytes):" not in line:
            continue
        _, v = line.split(":", 1)
        v = v.strip()
        try:
            return int(v)
        except ValueError:
            return 0

    return 0


def _run_with_optional_rss(
    cmd: list[str],
    input_data: bytes,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    measure_rss: bool = False,
) -> tuple[subprocess.CompletedProcess[bytes], int]:
    rss_kb = 0
    wrapped = cmd
    time_bin = _time_bin() if measure_rss else None
    if time_bin:
        wrapped = time_bin + cmd

    res = subprocess.run(
        wrapped,
        input=input_data,
        capture_output=True,
        cwd=str(cwd) if cwd else None,
        env=env,
    )

    if time_bin:
        rss_kb = _parse_time_max_rss_kb(res.stderr)

    return res, rss_kb


@dataclass
class BenchmarkResult:
    """Results from running a benchmark."""
    language: str
    benchmark: str
    times_ms: list[float] = field(default_factory=list)
    peak_rss_kb: int = 0
    build_size_bytes: int = 0
    output_bytes: bytes = b""
    compile_time_ms: float = 0.0
    success: bool = True
    error: str = ""

    @property
    def mean_time_ms(self) -> float:
        return statistics.mean(self.times_ms) if self.times_ms else 0.0

    @property
    def stddev_time_ms(self) -> float:
        return statistics.stdev(self.times_ms) if len(self.times_ms) > 1 else 0.0

    @property
    def min_time_ms(self) -> float:
        return min(self.times_ms) if self.times_ms else 0.0


@dataclass
class InputData:
    """Generated input data for a benchmark."""
    name: str
    data: bytes
    size_kb: float


def generate_input_data(benchmark: str, size_kb: int, seed: int = 42) -> InputData:
    """Generate input data for a specific benchmark."""
    random.seed(seed)
    size = size_kb * 1024

    if benchmark == "sum_bytes":
        data = bytes(random.randint(0, 255) for _ in range(size))
    elif benchmark == "word_count":
        words = ["the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog",
                 "hello", "world", "python", "rust", "code", "test", "benchmark"]
        out = bytearray()
        first = True
        while len(out) < size:
            if not first:
                out.append(32)
            out.extend(random.choice(words).encode())
            first = False
            if random.random() < 0.1:
                if len(out) >= size:
                    break
                out.append(32)
                out.append(10)
        data = bytes(out[:size])
    elif benchmark == "rle_encode":
        data_list = []
        while len(data_list) < size:
            byte_val = random.randint(0, 255)
            run_len = random.randint(1, min(50, size - len(data_list)))
            data_list.extend([byte_val] * run_len)
        data = bytes(data_list[:size])
    elif benchmark == "byte_freq":
        data = bytes(random.randint(0, 255) for _ in range(size))
    elif benchmark == "fibonacci":
        n = min(46, size_kb * 10)
        data = struct.pack("<I", n)
    elif benchmark == "regex_is_match" or benchmark == "regex_count":
        # Input format: 4 bytes (pat_len) + pattern + text
        pattern = b"[a-z]+"
        text_size = max(1, size - 4 - len(pattern))
        text = "".join(random.choices("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ ", k=text_size)).encode()
        data = struct.pack("<I", len(pattern)) + pattern + text
    elif benchmark == "regex_replace":
        # Input format: 4 bytes (pat_len) + 4 bytes (repl_len) + pattern + replacement + text
        pattern = b"[a-z]+"
        replacement = b"X"
        header_size = 4 + 4 + len(pattern) + len(replacement)
        text_size = max(1, size - header_size)
        text = "".join(random.choices("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ ", k=text_size)).encode()
        data = struct.pack("<I", len(pattern)) + struct.pack("<I", len(replacement)) + pattern + replacement + text
    else:
        data = bytes(random.randint(0, 255) for _ in range(size))

    return InputData(name=f"{benchmark}_{size_kb}kb", data=data, size_kb=len(data) / 1024)


class X07Runner:
    """Runner for X07 programs (via host runner)."""

    def __init__(self, repo_root: Path, cc_profile: str = "default"):
        self.repo_root = repo_root
        self.cc_profile = cc_profile
        self.host_runner = self._find_host_runner()

    def _find_host_runner(self) -> Path:
        return _ensure_host_runner(self.repo_root)

    def compile_and_run(
        self,
        program_path: Path,
        input_data: bytes,
        artifact_path: Path,
    ) -> tuple[bytes, dict[str, Any]]:
        """Compile and run an X07 program, returning output and metrics."""
        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as f:
            f.write(input_data)
            input_path = Path(f.name)

        try:
            cmd = _x07_host_runner_prefix(self.host_runner, self.cc_profile) + [
                "--program", str(program_path),
                "--world", "solve-pure",
                "--input", str(input_path),
                "--solve-fuel", "500000000",
                "--max-memory-bytes", str(256 * 1024 * 1024),
                "--compiled-out", str(artifact_path),
            ]

            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=str(self.repo_root),
            )

            if result.returncode != 0:
                raise RuntimeError(f"X07 runner failed: {result.stderr}")

            output = json.loads(result.stdout)
            if not output.get("solve", {}).get("ok"):
                trap = output.get("solve", {}).get("trap")
                raise RuntimeError(f"X07 execution failed: {trap}")

            import base64
            output_bytes = base64.b64decode(
                output.get("solve", {}).get("solve_output_b64", "")
            )

            return output_bytes, output

        finally:
            input_path.unlink(missing_ok=True)

    def run_cached(
        self,
        artifact_path: Path,
        input_data: bytes,
    ) -> tuple[bytes, dict[str, Any]]:
        """Run a pre-compiled X07 artifact."""
        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as f:
            f.write(input_data)
            input_path = Path(f.name)

        try:
            cmd = [
                str(self.host_runner),
                "--artifact", str(artifact_path),
                "--world", "solve-pure",
                "--input", str(input_path),
                "--solve-fuel", "500000000",
                "--max-memory-bytes", str(256 * 1024 * 1024),
            ]

            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=str(self.repo_root),
            )

            if result.returncode != 0:
                raise RuntimeError(f"X07 runner failed: {result.stderr}")

            output = json.loads(result.stdout)
            if not output.get("ok"):
                trap = output.get("trap")
                raise RuntimeError(f"X07 execution failed: {trap}")

            import base64
            output_bytes = base64.b64decode(output.get("solve_output_b64", ""))

            return output_bytes, output

        finally:
            input_path.unlink(missing_ok=True)


class X07DirectRunner:
    """Runner for compiled X07 binaries (direct execution, no host runner overhead)."""

    def __init__(self, repo_root: Path, cc_profile: str = "default"):
        self.repo_root = repo_root
        self.cc_profile = cc_profile
        self.host_runner = self._find_host_runner()

    def _find_host_runner(self) -> Path:
        return _ensure_host_runner(self.repo_root)

    def compile(self, program_path: Path, artifact_path: Path) -> float:
        """Compile an X07 program to a native binary, returning compile time in ms."""
        start = time.perf_counter()
        cmd = _x07_host_runner_prefix(self.host_runner, self.cc_profile) + [
            "--program", str(program_path),
            "--world", "solve-pure",
            "--solve-fuel", "500000000",  # High fuel for actual execution
            "--max-memory-bytes", str(256 * 1024 * 1024),
            "--compiled-out", str(artifact_path),
            "--compile-only",
        ]

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            cwd=str(self.repo_root),
        )
        compile_time = (time.perf_counter() - start) * 1000

        try:
            output = json.loads(result.stdout)
        except json.JSONDecodeError:
            raise RuntimeError(f"X07 compilation failed: {result.stderr}")

        if not output.get("compile", {}).get("ok"):
            err = output.get("compile", {}).get("compile_error")
            raise RuntimeError(f"X07 compilation failed: {err}")

        # Verify the binary was created
        if not artifact_path.exists():
            raise RuntimeError(f"X07 compilation failed: binary not created")

        return compile_time

    def run_direct(self, binary_path: Path, input_data: bytes) -> tuple[bytes, float]:
        """Run a compiled X07 binary directly with length-prefixed I/O.

        Binary ABI:
        - Input: 4 bytes (u32_le length) + data
        - Output: 4 bytes (u32_le length) + data
        """
        # Prepare length-prefixed input
        input_len = len(input_data)
        prefixed_input = struct.pack("<I", input_len) + input_data

        start = time.perf_counter()
        result = subprocess.run(
            [str(binary_path)],
            input=prefixed_input,
            capture_output=True,
        )
        run_time = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            raise RuntimeError(f"X07 execution failed (exit {result.returncode})")

        # Parse length-prefixed output
        raw_output = result.stdout
        if len(raw_output) < 4:
            raise RuntimeError(f"X07 output too short: {len(raw_output)} bytes")

        out_len = struct.unpack("<I", raw_output[:4])[0]
        if len(raw_output) < 4 + out_len:
            raise RuntimeError(f"X07 output truncated: expected {out_len}, got {len(raw_output) - 4}")

        output_bytes = raw_output[4:4 + out_len]
        return output_bytes, run_time

    def run_direct_with_rss(self, binary_path: Path, input_data: bytes) -> tuple[bytes, int]:
        """Run a compiled X07 binary directly and return output plus peak RSS (KB)."""
        input_len = len(input_data)
        prefixed_input = struct.pack("<I", input_len) + input_data

        res, rss_kb = _run_with_optional_rss(
            [str(binary_path)], prefixed_input, measure_rss=True
        )
        if res.returncode != 0:
            raise RuntimeError(f"X07 execution failed (exit {res.returncode})")

        raw_output = res.stdout
        if len(raw_output) < 4:
            raise RuntimeError(f"X07 output too short: {len(raw_output)} bytes")

        out_len = struct.unpack("<I", raw_output[:4])[0]
        if len(raw_output) < 4 + out_len:
            raise RuntimeError(
                f"X07 output truncated: expected {out_len}, got {len(raw_output) - 4}"
            )

        output_bytes = raw_output[4 : 4 + out_len]
        return output_bytes, rss_kb


class CRunner:
    """Runner for C programs."""

    def __init__(self, cc: str = "cc"):
        self.cc = cc

    def compile(self, source_path: Path, output_path: Path, optimize: bool = True) -> float:
        """Compile a C program, returning compile time in ms."""
        flags = ["-O3", "-march=native"] if optimize else ["-O0", "-g"]

        start = time.perf_counter()
        result = subprocess.run(
            [self.cc] + flags + ["-o", str(output_path), str(source_path)],
            capture_output=True,
            text=True,
        )
        compile_time = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            raise RuntimeError(f"C compilation failed: {result.stderr}")

        return compile_time

    def run(self, binary_path: Path, input_data: bytes) -> tuple[bytes, float]:
        """Run a compiled C program, returning output and time in ms."""
        start = time.perf_counter()
        result = subprocess.run(
            [str(binary_path)],
            input=input_data,
            capture_output=True,
        )
        run_time = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            raise RuntimeError(f"C execution failed: {result.stderr.decode()}")

        return result.stdout, run_time

    def run_with_rss(self, binary_path: Path, input_data: bytes) -> tuple[bytes, int]:
        """Run a compiled C program and return output plus peak RSS (KB)."""
        res, rss_kb = _run_with_optional_rss([str(binary_path)], input_data, measure_rss=True)
        if res.returncode != 0:
            raise RuntimeError(f"C execution failed: {res.stderr.decode(errors='replace')}")
        return res.stdout, rss_kb


class RustRunner:
    """Runner for Rust programs."""

    def __init__(self, rustc: str = "rustc"):
        self.rustc = rustc

    def compile(self, source_path: Path, output_path: Path, optimize: bool = True) -> float:
        """Compile a Rust program, returning compile time in ms."""
        flags = ["-C", "opt-level=3", "-C", "target-cpu=native"] if optimize else []

        start = time.perf_counter()
        result = subprocess.run(
            [self.rustc] + flags + ["-o", str(output_path), str(source_path)],
            capture_output=True,
            text=True,
        )
        compile_time = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            raise RuntimeError(f"Rust compilation failed: {result.stderr}")

        return compile_time

    def run(self, binary_path: Path, input_data: bytes) -> tuple[bytes, float]:
        """Run a compiled Rust program, returning output and time in ms."""
        start = time.perf_counter()
        result = subprocess.run(
            [str(binary_path)],
            input=input_data,
            capture_output=True,
        )
        run_time = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            raise RuntimeError(f"Rust execution failed: {result.stderr.decode()}")

        return result.stdout, run_time

    def run_with_rss(self, binary_path: Path, input_data: bytes) -> tuple[bytes, int]:
        """Run a compiled Rust program and return output plus peak RSS (KB)."""
        res, rss_kb = _run_with_optional_rss([str(binary_path)], input_data, measure_rss=True)
        if res.returncode != 0:
            raise RuntimeError(f"Rust execution failed: {res.stderr.decode(errors='replace')}")
        return res.stdout, rss_kb


class RustCargoRunner:
    """Runner for Rust programs using Cargo (for external dependencies)."""

    def compile(self, project_dir: Path, output_path: Path) -> float:
        """Compile a Cargo project, returning compile time in ms."""
        start = time.perf_counter()
        result = subprocess.run(
            ["cargo", "build", "--release"],
            capture_output=True,
            text=True,
            cwd=str(project_dir),
        )
        compile_time = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            raise RuntimeError(f"Cargo build failed: {result.stderr}")

        # Copy the binary to the output path
        binary_name = project_dir.name
        src_binary = project_dir / "target" / "release" / binary_name
        if not src_binary.exists():
            raise RuntimeError(f"Cargo build succeeded but binary not found: {src_binary}")

        import shutil
        shutil.copy2(src_binary, output_path)

        return compile_time

    def run(self, binary_path: Path, input_data: bytes) -> tuple[bytes, float]:
        """Run a compiled Rust program, returning output and time in ms."""
        start = time.perf_counter()
        result = subprocess.run(
            [str(binary_path)],
            input=input_data,
            capture_output=True,
        )
        run_time = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            raise RuntimeError(f"Rust execution failed: {result.stderr.decode()}")

        return result.stdout, run_time

    def run_with_rss(self, binary_path: Path, input_data: bytes) -> tuple[bytes, int]:
        """Run a compiled Rust program and return output plus peak RSS (KB)."""
        res, rss_kb = _run_with_optional_rss([str(binary_path)], input_data, measure_rss=True)
        if res.returncode != 0:
            raise RuntimeError(f"Rust execution failed: {res.stderr.decode(errors='replace')}")
        return res.stdout, rss_kb


class X07ProjectRunner:
    """Runner for X07 projects (with package dependencies)."""

    def __init__(self, repo_root: Path, cc_profile: str = "default"):
        self.repo_root = repo_root
        self.cc_profile = cc_profile
        self.host_runner = _ensure_host_runner(repo_root)

    def compile(self, project_file: Path, artifact_path: Path) -> float:
        """Compile an X07 project to a native binary, returning compile time in ms."""
        start = time.perf_counter()
        cmd = _x07_host_runner_prefix(self.host_runner, self.cc_profile) + [
            "--project", str(project_file),
            "--world", "solve-pure",
            "--solve-fuel", "500000000",
            "--max-memory-bytes", str(256 * 1024 * 1024),
            "--compiled-out", str(artifact_path),
            "--compile-only",
        ]

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            cwd=str(self.repo_root),
        )
        compile_time = (time.perf_counter() - start) * 1000

        try:
            output = json.loads(result.stdout)
        except json.JSONDecodeError:
            raise RuntimeError(f"X07 project compilation failed: {result.stderr}")

        if not output.get("compile", {}).get("ok"):
            err = output.get("compile", {}).get("compile_error")
            raise RuntimeError(f"X07 project compilation failed: {err}")

        if not artifact_path.exists():
            raise RuntimeError(f"X07 project compilation failed: binary not created")

        return compile_time

    def run_direct(self, binary_path: Path, input_data: bytes) -> tuple[bytes, float]:
        """Run a compiled X07 project binary directly with length-prefixed I/O."""
        input_len = len(input_data)
        prefixed_input = struct.pack("<I", input_len) + input_data

        start = time.perf_counter()
        result = subprocess.run(
            [str(binary_path)],
            input=prefixed_input,
            capture_output=True,
        )
        run_time = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            raise RuntimeError(f"X07 execution failed (exit {result.returncode})")

        raw_output = result.stdout
        if len(raw_output) < 4:
            raise RuntimeError(f"X07 output too short: {len(raw_output)} bytes")

        out_len = struct.unpack("<I", raw_output[:4])[0]
        if len(raw_output) < 4 + out_len:
            raise RuntimeError(f"X07 output truncated: expected {out_len}, got {len(raw_output) - 4}")

        output_bytes = raw_output[4:4 + out_len]
        return output_bytes, run_time

    def run_direct_with_rss(self, binary_path: Path, input_data: bytes) -> tuple[bytes, int]:
        """Run a compiled X07 project binary directly and return output plus peak RSS (KB)."""
        input_len = len(input_data)
        prefixed_input = struct.pack("<I", input_len) + input_data

        res, rss_kb = _run_with_optional_rss(
            [str(binary_path)], prefixed_input, measure_rss=True
        )
        if res.returncode != 0:
            raise RuntimeError(f"X07 execution failed (exit {res.returncode})")

        raw_output = res.stdout
        if len(raw_output) < 4:
            raise RuntimeError(f"X07 output too short: {len(raw_output)} bytes")

        out_len = struct.unpack("<I", raw_output[:4])[0]
        if len(raw_output) < 4 + out_len:
            raise RuntimeError(
                f"X07 output truncated: expected {out_len}, got {len(raw_output) - 4}"
            )

        output_bytes = raw_output[4 : 4 + out_len]
        return output_bytes, rss_kb


def run_benchmark(
    benchmark: str,
    input_data: InputData,
    repo_root: Path,
    tmp_dir: Path,
    iterations: int = 5,
    warmup: int = 1,
    direct_mode: bool = False,
    x07_cc_profile: str = "default",
) -> list[BenchmarkResult]:
    """Run a benchmark across all languages."""
    results = []

    perf_dir = repo_root / "perf-compare"
    c_runner = CRunner()
    rust_runner = RustRunner()

    x07_prog = perf_dir / "x07" / f"{benchmark}.x07.json"
    c_prog = perf_dir / "c" / f"{benchmark}.c"
    rust_prog = perf_dir / "rust" / f"{benchmark}.rs"

    # Check for project-based X07 (e.g., regex benchmarks)
    x07_project = None
    if benchmark.startswith("regex_"):
        entry_map = {
            "regex_is_match": "src/is_match.x07.json",
            "regex_count": "src/count.x07.json",
            "regex_replace": "src/replace.x07.json",
        }
        if benchmark in entry_map:
            project_dir = perf_dir / "projects" / "regex"
            project_file = project_dir / "x07.json"
            if project_file.exists():
                x07_project = (project_file, entry_map[benchmark])

    # Check for cargo-based Rust (e.g., regex benchmarks)
    rust_cargo_proj = perf_dir / "rust_cargo" / benchmark
    rust_cargo_exists = (rust_cargo_proj / "Cargo.toml").exists()

    reference_output = None

    # Priority: project-based X07 over single-file X07
    if x07_project is not None:
        project_file, entry = x07_project
        result = BenchmarkResult(language="X07", benchmark=benchmark)
        original_project_text = project_file.read_text()
        try:
            # Update project to use the right entry
            project_dir = project_file.parent
            project_data = json.loads(original_project_text)
            project_data["entry"] = entry
            project_file.write_text(json.dumps(project_data, indent=2))

            x07_runner = X07Runner(repo_root, cc_profile=x07_cc_profile)
            project_runner = X07ProjectRunner(repo_root, cc_profile=x07_cc_profile)
            artifact = tmp_dir / f"{benchmark}_x07"

            result.compile_time_ms = project_runner.compile(project_file, artifact)
            result.build_size_bytes = artifact.stat().st_size

            output, rss_kb = project_runner.run_direct_with_rss(artifact, input_data.data)
            result.output_bytes = output
            result.peak_rss_kb = rss_kb
            if reference_output is None:
                reference_output = output

            for _ in range(warmup):
                if direct_mode:
                    project_runner.run_direct(artifact, input_data.data)
                else:
                    x07_runner.run_cached(artifact, input_data.data)

            if direct_mode:
                for _ in range(iterations):
                    output, run_time = project_runner.run_direct(artifact, input_data.data)
                    result.times_ms.append(run_time)
            else:
                for _ in range(iterations):
                    start = time.perf_counter()
                    output, _metrics = x07_runner.run_cached(artifact, input_data.data)
                    result.times_ms.append((time.perf_counter() - start) * 1000)

            if reference_output is not None and output != reference_output:
                result.error = "Output mismatch with reference"

        except Exception as e:
            result.success = False
            result.error = str(e)
        finally:
            project_file.write_text(original_project_text)

        results.append(result)

    elif x07_prog.exists():
        result = BenchmarkResult(language="X07", benchmark=benchmark)
        try:
            x07_runner = X07Runner(repo_root, cc_profile=x07_cc_profile)
            direct_runner = X07DirectRunner(repo_root, cc_profile=x07_cc_profile)
            artifact = tmp_dir / f"{benchmark}_x07"

            result.compile_time_ms = direct_runner.compile(x07_prog, artifact)
            result.build_size_bytes = artifact.stat().st_size

            output, rss_kb = direct_runner.run_direct_with_rss(artifact, input_data.data)
            result.output_bytes = output
            result.peak_rss_kb = rss_kb
            if reference_output is None:
                reference_output = output

            for _ in range(warmup):
                if direct_mode:
                    direct_runner.run_direct(artifact, input_data.data)
                else:
                    x07_runner.run_cached(artifact, input_data.data)

            if direct_mode:
                for _ in range(iterations):
                    output, run_time = direct_runner.run_direct(artifact, input_data.data)
                    result.times_ms.append(run_time)
            else:
                for _ in range(iterations):
                    start = time.perf_counter()
                    output, _metrics = x07_runner.run_cached(artifact, input_data.data)
                    result.times_ms.append((time.perf_counter() - start) * 1000)

            if reference_output is not None and output != reference_output:
                result.error = "Output mismatch with reference"

        except Exception as e:
            result.success = False
            result.error = str(e)

        results.append(result)

    if c_prog.exists():
        result = BenchmarkResult(language="C", benchmark=benchmark)
        try:
            binary = tmp_dir / f"{benchmark}_c"
            result.compile_time_ms = c_runner.compile(c_prog, binary)
            result.build_size_bytes = binary.stat().st_size

            output, rss_kb = c_runner.run_with_rss(binary, input_data.data)
            result.peak_rss_kb = rss_kb
            result.output_bytes = output
            if reference_output is not None and output != reference_output:
                result.error = "Output mismatch with reference"

            for _ in range(warmup):
                c_runner.run(binary, input_data.data)

            for _ in range(iterations):
                output, run_time = c_runner.run(binary, input_data.data)
                result.times_ms.append(run_time)
            result.output_bytes = output
            if reference_output is not None and output != reference_output:
                result.error = "Output mismatch with reference"

        except Exception as e:
            result.success = False
            result.error = str(e)

        results.append(result)

    # Priority: cargo-based Rust over single-file Rust
    if rust_cargo_exists:
        result = BenchmarkResult(language="Rust", benchmark=benchmark)
        try:
            cargo_runner = RustCargoRunner()
            binary = tmp_dir / f"{benchmark}_rust"

            result.compile_time_ms = cargo_runner.compile(rust_cargo_proj, binary)
            result.build_size_bytes = binary.stat().st_size

            output, rss_kb = cargo_runner.run_with_rss(binary, input_data.data)
            result.peak_rss_kb = rss_kb
            result.output_bytes = output
            if reference_output is not None and output != reference_output:
                result.error = "Output mismatch with reference"

            for _ in range(warmup):
                cargo_runner.run(binary, input_data.data)

            for _ in range(iterations):
                output, run_time = cargo_runner.run(binary, input_data.data)
                result.times_ms.append(run_time)

        except Exception as e:
            result.success = False
            result.error = str(e)

        results.append(result)

    elif rust_prog.exists():
        result = BenchmarkResult(language="Rust", benchmark=benchmark)
        try:
            binary = tmp_dir / f"{benchmark}_rust"
            result.compile_time_ms = rust_runner.compile(rust_prog, binary)
            result.build_size_bytes = binary.stat().st_size

            output, rss_kb = rust_runner.run_with_rss(binary, input_data.data)
            result.peak_rss_kb = rss_kb
            result.output_bytes = output
            if reference_output is not None and output != reference_output:
                result.error = "Output mismatch with reference"

            for _ in range(warmup):
                rust_runner.run(binary, input_data.data)

            for _ in range(iterations):
                output, run_time = rust_runner.run(binary, input_data.data)
                result.times_ms.append(run_time)
            result.output_bytes = output
            if reference_output is not None and output != reference_output:
                result.error = "Output mismatch with reference"

        except Exception as e:
            result.success = False
            result.error = str(e)

        results.append(result)

    return results


def print_results(
    all_results: dict[str, list[BenchmarkResult]],
    input_size_kb: int,
    direct_mode: bool = False,
    x07_cc_profile: str = "default",
) -> None:
    """Print benchmark results in a formatted table."""
    print()
    print("=" * 80)
    mode_str = "direct binary" if direct_mode else "host runner"
    print(
        f"Performance Benchmark Results (input size: {input_size_kb} KB, X07 mode: {mode_str}, cc-profile: {x07_cc_profile})"
    )
    print("=" * 80)
    print()

    for benchmark, results in all_results.items():
        print(f"Benchmark: {benchmark}")
        print("-" * 70)
        print(
            f"{'Language':<12} {'Mean (ms)':<12} {'Min (ms)':<12} {'StdDev':<10} "
            f"{'Compile (ms)':<12} {'Build (KiB)':<12} {'RSS (KiB)':<10} {'Status'}"
        )
        print("-" * 70)

        x07_time = None
        for r in results:
            if r.language == "X07" and r.success:
                x07_time = r.mean_time_ms
                break

        for r in results:
            status = "OK" if r.success else f"FAIL: {r.error[:30]}"
            if r.error and r.success:
                status = f"WARN: {r.error[:25]}"

            speedup = ""
            if x07_time and r.success and r.mean_time_ms > 0:
                ratio = x07_time / r.mean_time_ms
                if r.language != "X07":
                    speedup = f" ({ratio:.2f}x)"

            build_kib = r.build_size_bytes / 1024 if r.build_size_bytes else 0.0
            print(
                f"{r.language:<12} "
                f"{r.mean_time_ms:<12.2f} "
                f"{r.min_time_ms:<12.2f} "
                f"{r.stddev_time_ms:<10.2f} "
                f"{r.compile_time_ms:<12.1f} "
                f"{build_kib:<12.1f} "
                f"{r.peak_rss_kb:<10d} "
                f"{status}{speedup}"
            )

        print()

    print()
    print("Legend:")
    print("  - Mean/Min/StdDev: Execution time statistics over multiple runs")
    print("  - Compile: One-time compilation overhead")
    print("  - Build: Final executable size")
    print("  - RSS: Peak resident set size (one run)")
    print("  - Speedup (Nx): How many times faster than X07")
    print()


def print_summary_table(all_results: dict[str, list[BenchmarkResult]]) -> None:
    """Print a summary comparison table."""
    print()
    print("=" * 60)
    print("Summary: Relative Performance (X07 = 1.0x)")
    print("=" * 60)
    print()
    print(f"{'Benchmark':<20} {'X07':<12} {'C':<12} {'Rust':<12}")
    print("-" * 60)

    for benchmark, results in all_results.items():
        row = {"X07": "1.0x", "C": "N/A", "Rust": "N/A"}

        x07_time = None
        for r in results:
            if r.language == "X07" and r.success:
                x07_time = r.mean_time_ms
                break

        if x07_time and x07_time > 0:
            for r in results:
                if r.success and r.mean_time_ms > 0:
                    ratio = x07_time / r.mean_time_ms
                    row[r.language] = f"{ratio:.2f}x"

        print(f"{benchmark:<20} {row['X07']:<12} {row['C']:<12} {row['Rust']:<12}")

    print()


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Run performance comparison benchmarks")
    ap.add_argument(
        "--repo-root",
        type=Path,
        default=None,
        help="Path to X07 repo root (default: auto-detect)",
    )
    ap.add_argument("--size", type=int, default=100, help="Input size in KB (default: 100)")
    ap.add_argument("--iterations", type=int, default=5, help="Number of iterations (default: 5)")
    ap.add_argument("--warmup", type=int, default=2, help="Warmup iterations (default: 2)")
    ap.add_argument("--benchmarks", nargs="+", default=None,
                    help="Specific benchmarks to run (default: all)")
    ap.add_argument("--json", action="store_true", help="Output results as JSON")
    ap.add_argument("--direct", action="store_true",
                    help="Run X07 binaries directly (no host runner overhead)")
    ap.add_argument(
        "--x07-cc-profile",
        choices=["default", "size"],
        default=os.environ.get("X07_CC_PROFILE", "default").lower(),
        help="Pass through to x07-host-runner --cc-profile (default: default)",
    )
    args = ap.parse_args(argv)

    repo_root = args.repo_root or _repo_root()

    all_benchmarks = ["sum_bytes", "word_count", "rle_encode", "byte_freq", "fibonacci"]
    benchmarks = args.benchmarks if args.benchmarks else all_benchmarks

    all_results: dict[str, list[BenchmarkResult]] = {}

    with tempfile.TemporaryDirectory(prefix="perf_compare_") as tmp:
        tmp_dir = Path(tmp)

        for benchmark in benchmarks:
            print(f"Running benchmark: {benchmark}...", file=sys.stderr)

            input_data = generate_input_data(benchmark, args.size)

            results = run_benchmark(
                benchmark,
                input_data,
                repo_root,
                tmp_dir,
                iterations=args.iterations,
                warmup=args.warmup,
                direct_mode=args.direct,
                x07_cc_profile=args.x07_cc_profile,
            )

            all_results[benchmark] = results

    if args.json:
        output = {}
        for benchmark, results in all_results.items():
            output[benchmark] = [
                {
                    "language": r.language,
                    "mean_time_ms": r.mean_time_ms,
                    "min_time_ms": r.min_time_ms,
                    "stddev_time_ms": r.stddev_time_ms,
                    "compile_time_ms": r.compile_time_ms,
                    "build_size_bytes": r.build_size_bytes,
                    "peak_rss_kb": r.peak_rss_kb,
                    "success": r.success,
                    "error": r.error,
                    "x07_cc_profile": args.x07_cc_profile if r.language == "X07" else None,
                }
                for r in results
            ]
        print(json.dumps(output, indent=2))
    else:
        print_results(
            all_results,
            args.size,
            direct_mode=args.direct,
            x07_cc_profile=args.x07_cc_profile,
        )
        print_summary_table(all_results)

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
