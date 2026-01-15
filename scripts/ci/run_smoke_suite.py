#!/usr/bin/env python3

import argparse
import base64
import json
import os
import subprocess
import sys
import tempfile
from typing import List, Optional


def _read_json(path: str) -> object:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def _b64decode(s: str) -> bytes:
    if not s:
        return b""
    return base64.b64decode(s)


def _run_case(
    *,
    runner: str,
    suite_path: str,
    case_name: str,
    program: str,
    world: str,
    module_roots: List[str],
    policy_json: Optional[str],
    input_bytes: bytes,
) -> bytes:
    tmp_path: str | None = None
    if input_bytes:
        fd, tmp_path = tempfile.mkstemp(prefix="x07_smoke_", suffix=".bin")
        try:
            os.write(fd, input_bytes)
        finally:
            os.close(fd)

    try:
        args: List[str] = [runner, "--program", program, "--world", world]
        for r in module_roots:
            args += ["--module-root", r]
        if policy_json is not None:
            args += ["--policy", policy_json]
        if tmp_path is not None:
            args += ["--input", tmp_path]

        proc = subprocess.run(
            args,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if proc.returncode != 0:
            raise SystemExit(
                f"{suite_path}:{case_name}: runner failed (exit={proc.returncode})\n"
                f"stderr:\n{proc.stderr}\nstdout:\n{proc.stdout}"
            )

        report = json.loads(proc.stdout)
        solve = report.get("solve") or {}
        got_b64 = solve.get("solve_output_b64") or ""
        return _b64decode(got_b64)
    finally:
        if tmp_path is not None:
            try:
                os.remove(tmp_path)
            except OSError:
                pass


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", required=True)
    parser.add_argument("--host-runner", required=True)
    parser.add_argument("--os-runner", required=True)
    parser.add_argument("--module-root", action="append", default=[])
    args = parser.parse_args()

    suite_path = args.suite
    suite = _read_json(suite_path)

    world = suite.get("world")
    program = suite.get("program_x07json")
    cases = suite.get("cases") or []

    if not isinstance(world, str) or not world:
        raise SystemExit(f"bad smoke suite world: {world!r}")
    if not isinstance(program, str) or not program:
        raise SystemExit(f"bad smoke suite program_x07json: {program!r}")
    if not isinstance(cases, list):
        raise SystemExit(f"bad smoke suite cases: {type(cases).__name__}")

    policy_json = suite.get("policy_json")
    if policy_json is not None and (not isinstance(policy_json, str) or not policy_json):
        raise SystemExit(f"bad smoke suite policy_json: {policy_json!r}")

    if world.startswith("solve-"):
        runner = args.host_runner
        if policy_json is not None:
            raise SystemExit(f"{suite_path}: policy_json is not supported for {world}")
    elif world.startswith("run-os"):
        runner = args.os_runner
        if world == "run-os-sandboxed" and policy_json is None:
            raise SystemExit(f"{suite_path}: run-os-sandboxed requires policy_json")
        if world != "run-os-sandboxed" and policy_json is not None:
            raise SystemExit(f"{suite_path}: policy_json is only supported for run-os-sandboxed")
    else:
        raise SystemExit(f"unsupported smoke suite world: {world!r}")

    module_roots: List[str] = args.module_root
    if not module_roots:
        raise SystemExit("missing --module-root (expected at least 1)")
    for r in module_roots:
        if not isinstance(r, str) or not r:
            raise SystemExit(f"bad --module-root: {r!r}")

    for case in cases:
        if not isinstance(case, dict):
            raise SystemExit(f"{suite_path}: case must be object, got: {type(case).__name__}")
        name = case.get("name") or "<unnamed>"
        input_b64 = case.get("input_b64") or ""
        expected_b64 = case.get("expected_b64") or ""

        if not isinstance(name, str):
            raise SystemExit(f"{suite_path}: case.name must be string, got: {name!r}")
        if not isinstance(input_b64, str):
            raise SystemExit(f"{suite_path}:{name}: input_b64 must be string")
        if not isinstance(expected_b64, str):
            raise SystemExit(f"{suite_path}:{name}: expected_b64 must be string")

        got = _run_case(
            runner=runner,
            suite_path=suite_path,
            case_name=name,
            program=program,
            world=world,
            module_roots=module_roots,
            policy_json=policy_json,
            input_bytes=_b64decode(input_b64),
        )
        expected = _b64decode(expected_b64)

        if got != expected:
            raise SystemExit(f"{suite_path}:{name}: expected {expected!r}, got {got!r}")

        print(f"ok: {suite_path}:{name}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
