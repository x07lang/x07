#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, NoReturn


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _die(msg: str, code: int = 2) -> NoReturn:
    print(msg, file=sys.stderr)
    raise SystemExit(code)


def _unb64(s: str) -> bytes:
    try:
        return base64.b64decode(str(s or "").encode("ascii"), validate=True)
    except Exception as e:
        raise ValueError(f"invalid base64: {e!r}") from e


def _load_case_blob(case: dict[str, Any], suite_fixture_root: str | None, which: str) -> bytes:
    b64_key = f"{which}_b64"
    file_key = f"{which}_file"

    file_v = case.get(file_key)
    if file_v is not None:
        rel = str(file_v).strip()
        if not rel:
            return b""
        if suite_fixture_root is None:
            raise ValueError(f"{file_key} requires suite.fixture_root")
        rel_path = Path(rel)
        if rel_path.is_absolute() or any(p == ".." for p in rel_path.parts):
            raise ValueError(f"unsafe {file_key}: {rel!r}")
        p = _repo_root() / suite_fixture_root / rel_path
        return p.read_bytes()

    b64_v = case.get(b64_key)
    if b64_v is not None:
        return _unb64(str(b64_v))

    return b""


@dataclass(frozen=True)
class BenchCase:
    input_bytes: bytes
    expected_bytes: bytes
    name: str | None = None
    assertions: dict[str, Any] | None = None


@dataclass(frozen=True)
class BenchTask:
    task_id: str
    description: str
    cases: list[BenchCase]
    assertions: dict[str, Any] | None = None
    task_world: str | None = None


@dataclass(frozen=True)
class BenchSuite:
    suite_id: str
    world: str
    tasks: list[BenchTask]
    fixture_root: str | None = None
    fs_root: str | None = None
    fs_latency_index: str | None = None
    rr_index: str | None = None
    kv_seed: str | None = None
    requires_debug_borrow_checks: bool = False


@dataclass(frozen=True)
class BenchBundle:
    bundle_id: str
    pre_score_canaries: list[str]
    score_suites: list[str]
    debug_suites: list[str]


_ALLOWED_WORLDS = {
    "solve-pure",
    "solve-fs",
    "solve-rr",
    "solve-kv",
    "solve-full",
}

_PERF_BASELINE_SCHEMA = "x07.perf_baseline@0.1.0"
_BENCH_MODULE_ROOT_ENV = "X07_BENCH_MODULE_ROOT"


def _bench_module_roots() -> list[str]:
    raw = os.environ.get(_BENCH_MODULE_ROOT_ENV) or ""
    if not raw.strip():
        return []
    out: list[str] = []
    for part in raw.split(":"):
        s = str(part).strip()
        if s:
            out.append(s)
    return out


def _suite_rel_path(suite_path: Path) -> str:
    root = _repo_root().resolve()
    try:
        return str(suite_path.resolve().relative_to(root))
    except Exception:
        return str(suite_path)


def _load_perf_baseline(path: Path) -> dict[str, Any]:
    obj = json.loads(path.read_text())
    if not isinstance(obj, dict):
        raise ValueError(f"perf baseline JSON root is not an object: {path}")
    if str(obj.get("schema_version") or "").strip() != _PERF_BASELINE_SCHEMA:
        raise ValueError(
            f"perf baseline schema_version mismatch: expected {_PERF_BASELINE_SCHEMA!r} got {obj.get('schema_version')!r}"
        )
    suites = obj.get("suites")
    if not isinstance(suites, dict):
        raise ValueError(f"perf baseline suites must be an object: {path}")
    return obj


def _write_perf_baseline(path: Path, obj: dict[str, Any]) -> None:
    if path.parent:
        path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj, indent=2, sort_keys=True) + "\n")


def _load_suite_obj(obj: dict[str, Any]) -> BenchSuite:
    def opt_str_field(key: str) -> str | None:
        v = obj.get(key)
        if v is None:
            return None
        s = str(v)
        return s if s.strip() else None

    suite_id = str(obj.get("suite_id") or "").strip()
    if not suite_id:
        raise ValueError("suite_id is required")
    world = str(obj.get("world") or "").strip() or "solve-pure"
    if world not in _ALLOWED_WORLDS:
        raise ValueError(f"suite world {world!r} is not supported")

    fixture_root = opt_str_field("fixture_root")
    fs_root = opt_str_field("fs_root")
    fs_latency_index = opt_str_field("fs_latency_index")
    rr_index = opt_str_field("rr_index")
    kv_seed = opt_str_field("kv_seed")
    requires_debug_borrow_checks = bool(obj.get("requires_debug_borrow_checks") or False)

    tasks_raw = obj.get("tasks")
    if not isinstance(tasks_raw, list):
        raise ValueError("suite.tasks must be a list")

    tasks: list[BenchTask] = []
    for t in tasks_raw:
        if not isinstance(t, dict):
            continue

        task_id = str(t.get("task_id") or "").strip()
        description = str(t.get("description") or "")
        task_world = t.get("task_world")
        if task_world is not None:
            task_world = str(task_world).strip() or None
        if task_world is not None and task_world not in _ALLOWED_WORLDS:
            raise ValueError(f"task {task_id!r} has unsupported task_world {task_world!r}")

        assertions = t.get("assertions")
        if not isinstance(assertions, dict):
            assertions = None

        cases_raw = t.get("cases")
        if not task_id or not isinstance(cases_raw, list):
            continue

        cases: list[BenchCase] = []
        for c in cases_raw:
            if not isinstance(c, dict):
                continue
            inp = _load_case_blob(c, fixture_root, "input")
            exp = _load_case_blob(c, fixture_root, "expected")

            name = c.get("name")
            if name is not None:
                name = str(name)
                if not name.strip():
                    name = None

            case_assertions = c.get("assertions")
            if not isinstance(case_assertions, dict):
                case_assertions = None

            cases.append(
                BenchCase(
                    input_bytes=inp,
                    expected_bytes=exp,
                    name=name,
                    assertions=case_assertions,
                )
            )

        tasks.append(
            BenchTask(
                task_id=task_id,
                description=description,
                cases=cases,
                assertions=assertions,
                task_world=task_world,
            )
        )

    return BenchSuite(
        suite_id=suite_id,
        world=world,
        tasks=tasks,
        fixture_root=fixture_root,
        fs_root=fs_root,
        fs_latency_index=fs_latency_index,
        rr_index=rr_index,
        kv_seed=kv_seed,
        requires_debug_borrow_checks=requires_debug_borrow_checks,
    )


def _load_bundle_obj(obj: dict[str, Any]) -> BenchBundle:
    bundle_id = str(obj.get("bundle_id") or "").strip()
    if not bundle_id:
        raise ValueError("bundle_id is required")

    def list_field(key: str) -> list[str]:
        raw = obj.get(key)
        if raw is None:
            return []
        if not isinstance(raw, list):
            raise ValueError(f"bundle.{key} must be a list")
        out: list[str] = []
        for item in raw:
            if item is None:
                continue
            s = str(item).strip()
            if s:
                out.append(s)
        return out

    pre_score_canaries = list_field("pre_score_canaries")
    score_suites = list_field("score_suites")
    debug_suites = list_field("debug_suites")
    if not score_suites:
        raise ValueError("bundle.score_suites must be non-empty")

    return BenchBundle(
        bundle_id=bundle_id,
        pre_score_canaries=pre_score_canaries,
        score_suites=score_suites,
        debug_suites=debug_suites,
    )


def _load_suite(path: Path) -> BenchSuite:
    obj = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(obj, dict):
        raise ValueError("suite root must be an object")
    return _load_suite_obj(obj)


def _load_bundle(path: Path) -> BenchBundle:
    obj = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(obj, dict):
        raise ValueError("bundle root must be an object")
    return _load_bundle_obj(obj)


def _is_bundle(path: Path) -> bool:
    try:
        obj = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return False
    if not isinstance(obj, dict):
        return False
    return "bundle_id" in obj and ("score_suites" in obj or "pre_score_canaries" in obj)


def _find_executable(candidates: Iterable[Path]) -> Path | None:
    for p in candidates:
        if p.is_file() and os.access(p, os.X_OK):
            return p
    return None


def _host_runner_bin() -> Path | None:
    env = os.environ.get("X07_HOST_RUNNER_BIN") or ""
    if env.strip():
        p = Path(env)
        if p.is_file() and os.access(p, os.X_OK):
            return p
        return None

    root = _repo_root()
    names = ["x07-host-runner"]
    if sys.platform.startswith("win"):
        names.append("x07-host-runner.exe")
    return _find_executable(
        [
            *(root / "target" / "debug" / name for name in names),
            *(root / "target" / "release" / name for name in names),
        ]
    )


def _ensure_host_runner_bin() -> Path:
    p = _host_runner_bin()
    if p is not None:
        return p

    root = _repo_root()
    res = subprocess.run(
        ["cargo", "build", "-p", "x07-host-runner"],
        cwd=str(root),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if res.returncode != 0:
        _die(
            "failed to build x07-host-runner:\n"
            f"stdout:\n{res.stdout}\n"
            f"stderr:\n{res.stderr}\n",
            code=2,
        )

    p = _host_runner_bin()
    if p is None:
        _die("x07-host-runner binary not found after build", code=2)
    return p


def _split_solve_full_fixture_root_component_path(
    *, suite: BenchSuite, base_dir: Path, rel_path: str, component: str
) -> tuple[Path, str]:
    """
    Solve-full fixtures may use a single fixture_root with subdirs like:
      - fs/...
      - rr/...
      - kv/...
    while x07-host-runner expects separate --fixture-*-dir roots.
    """
    if suite.fixture_root is None:
        return base_dir, rel_path

    fixture_root = _repo_root() / suite.fixture_root
    if base_dir != fixture_root:
        return base_dir, rel_path

    p = Path(rel_path)
    parts = p.parts
    if len(parts) >= 2 and parts[0] == component:
        return base_dir / component, str(Path(*parts[1:]))

    return base_dir, rel_path


def _extend_runner_cmd_with_fixtures(*, cmd: list[str], suite: BenchSuite) -> None:
    root = _repo_root()

    if suite.world == "solve-pure":
        return

    if suite.fixture_root is None:
        raise ValueError(f"{suite.suite_id}: missing fixture_root for world {suite.world}")

    base_dir = root / suite.fixture_root

    if suite.world == "solve-fs":
        cmd.extend(["--fixture-fs-dir", str(base_dir)])
        if suite.fs_root:
            cmd.extend(["--fixture-fs-root", str(suite.fs_root)])
        if suite.fs_latency_index:
            cmd.extend(["--fixture-fs-latency-index", str(suite.fs_latency_index)])
        return

    if suite.world == "solve-rr":
        cmd.extend(["--fixture-rr-dir", str(base_dir)])
        if suite.rr_index:
            cmd.extend(["--fixture-rr-index", str(suite.rr_index)])
        return

    if suite.world == "solve-kv":
        cmd.extend(["--fixture-kv-dir", str(base_dir)])
        if suite.kv_seed:
            cmd.extend(["--fixture-kv-seed", str(suite.kv_seed)])
        return

    if suite.world == "solve-full":
        fixture_root = base_dir
        fs_dir = fixture_root
        rr_dir = fixture_root
        kv_dir = fixture_root

        fs_root = suite.fs_root
        fs_latency_index = suite.fs_latency_index
        rr_index = suite.rr_index
        kv_seed = suite.kv_seed

        if fs_root is not None:
            fs_dir, fs_root = _split_solve_full_fixture_root_component_path(
                suite=suite, base_dir=fixture_root, rel_path=fs_root, component="fs"
            )
        if fs_latency_index is not None:
            fs_dir2, fs_latency_index = _split_solve_full_fixture_root_component_path(
                suite=suite,
                base_dir=fixture_root,
                rel_path=fs_latency_index,
                component="fs",
            )
            if fs_dir2 != fs_dir:
                fs_dir = fs_dir2
        if rr_index is not None:
            rr_dir, rr_index = _split_solve_full_fixture_root_component_path(
                suite=suite, base_dir=fixture_root, rel_path=rr_index, component="rr"
            )
        if kv_seed is not None:
            kv_dir, kv_seed = _split_solve_full_fixture_root_component_path(
                suite=suite, base_dir=fixture_root, rel_path=kv_seed, component="kv"
            )

        cmd.extend(
            [
                "--fixture-fs-dir",
                str(fs_dir),
                "--fixture-rr-dir",
                str(rr_dir),
                "--fixture-kv-dir",
                str(kv_dir),
            ]
        )
        if fs_root:
            cmd.extend(["--fixture-fs-root", str(fs_root)])
        if fs_latency_index:
            cmd.extend(["--fixture-fs-latency-index", str(fs_latency_index)])
        if rr_index:
            cmd.extend(["--fixture-rr-index", str(rr_index)])
        if kv_seed:
            cmd.extend(["--fixture-kv-seed", str(kv_seed)])
        return

    raise ValueError(f"unsupported world: {suite.world}")


def _run_host_runner_json(host_runner: Path, args: list[str]) -> dict[str, Any]:
    res = subprocess.run(
        [str(host_runner), *args],
        cwd=str(_repo_root()),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    out = (res.stdout or "").strip()
    if not out.startswith("{"):
        raise RuntimeError(
            "host-runner produced non-JSON output:\n"
            f"exit={res.returncode}\n"
            f"stdout:\n{res.stdout}\n"
            f"stderr:\n{res.stderr}\n"
        )
    try:
        obj = json.loads(out)
    except Exception as e:
        raise RuntimeError(
            f"failed to parse host-runner JSON: {e!r}\nstdout:\n{out}\n"
        ) from e
    if not isinstance(obj, dict):
        raise RuntimeError("host-runner JSON root is not an object")
    return obj


def _solution_path(solutions_dir: Path, task_id: str) -> Path:
    cand = solutions_dir / Path(task_id + ".x07.json")
    if cand.is_file():
        return cand
    raise FileNotFoundError(f"missing solution for task {task_id!r}: {cand}")


def _merge_assertions(case: BenchCase, task: BenchTask) -> dict[str, Any]:
    out: dict[str, Any] = {}
    if isinstance(task.assertions, dict):
        out.update(task.assertions)
    if isinstance(case.assertions, dict):
        out.update(case.assertions)
    return out


def _require_int(obj: dict[str, Any], key: str, *, ctx: str) -> int:
    v = obj.get(key)
    if not isinstance(v, int):
        raise ValueError(f"{ctx}: missing {key}")
    return int(v)


def _require_nonempty_str(obj: dict[str, Any], key: str, *, ctx: str) -> str:
    v = obj.get(key)
    if not isinstance(v, str) or not v.strip():
        raise ValueError(f"{ctx}: missing {key}")
    return v.strip()


def _check_range(
    *,
    ctx: str,
    got: int,
    min_v: int | None,
    max_v: int | None,
    label: str,
) -> None:
    if min_v is not None and got < int(min_v):
        raise ValueError(f"{ctx}: {label} {got} < min {int(min_v)}")
    if max_v is not None and got > int(max_v):
        raise ValueError(f"{ctx}: {label} {got} > max {int(max_v)}")


def _enforce_assertions(
    *,
    ctx: str,
    solve_obj: dict[str, Any],
    assertions: dict[str, Any],
) -> None:
    mem_stats_required = bool(assertions.get("mem_stats_required") or False)
    leak_free_required = bool(assertions.get("leak_free_required") or False)
    debug_stats_required = bool(assertions.get("debug_stats_required") or False)
    sched_stats_required = bool(assertions.get("sched_stats_required") or False)
    replay_required = bool(assertions.get("replay_required") or False)

    if mem_stats_required:
        mem = solve_obj.get("mem_stats")
        if not isinstance(mem, dict):
            raise ValueError(f"{ctx}: mem_stats_required but missing mem_stats")

        live_allocs = int(mem.get("live_allocs") or 0)
        live_bytes = int(mem.get("live_bytes") or 0)
        if leak_free_required and (live_allocs != 0 or live_bytes != 0):
            raise ValueError(
                f"{ctx}: leak gate failed: live_allocs={live_allocs} live_bytes={live_bytes}"
            )

        realloc_calls = int(mem.get("realloc_calls") or 0)
        memcpy_bytes = int(mem.get("memcpy_bytes") or 0)
        peak_live_bytes = int(mem.get("peak_live_bytes") or 0)

        want_max_realloc = assertions.get("max_realloc_calls")
        if want_max_realloc is not None and realloc_calls > int(want_max_realloc):
            raise ValueError(
                f"{ctx}: realloc_calls {realloc_calls} > max {int(want_max_realloc)}"
            )

        want_max_memcpy = assertions.get("max_memcpy_bytes")
        if want_max_memcpy is not None and memcpy_bytes > int(want_max_memcpy):
            raise ValueError(
                f"{ctx}: memcpy_bytes {memcpy_bytes} > max {int(want_max_memcpy)}"
            )

        want_max_peak = assertions.get("max_peak_live_bytes")
        if want_max_peak is not None and peak_live_bytes > int(want_max_peak):
            raise ValueError(
                f"{ctx}: peak_live_bytes {peak_live_bytes} > max {int(want_max_peak)}"
            )

    if debug_stats_required:
        dbg = solve_obj.get("debug_stats")
        if not isinstance(dbg, dict):
            raise ValueError(f"{ctx}: debug_stats_required but missing debug_stats")
        borrow_violations = int(dbg.get("borrow_violations") or 0)
        want_max_borrow = assertions.get("max_borrow_violations")
        if want_max_borrow is not None and borrow_violations > int(want_max_borrow):
            raise ValueError(
                f"{ctx}: borrow_violations {borrow_violations} > max {int(want_max_borrow)}"
            )

    min_tasks_spawned = assertions.get("min_tasks_spawned")
    max_tasks_spawned = assertions.get("max_tasks_spawned")
    max_virtual_time_ticks = assertions.get("max_virtual_time_ticks")

    need_sched = (
        sched_stats_required
        or replay_required
        or min_tasks_spawned is not None
        or max_tasks_spawned is not None
        or max_virtual_time_ticks is not None
    )
    if need_sched:
        ss = solve_obj.get("sched_stats")
        if not isinstance(ss, dict):
            raise ValueError(f"{ctx}: missing sched_stats")

        tasks_spawned = _require_int(ss, "tasks_spawned", ctx=ctx)
        _check_range(
            ctx=ctx,
            got=tasks_spawned,
            min_v=int(min_tasks_spawned) if min_tasks_spawned is not None else None,
            max_v=int(max_tasks_spawned) if max_tasks_spawned is not None else None,
            label="tasks_spawned",
        )

        vt = _require_int(ss, "virtual_time_end", ctx=ctx)
        if max_virtual_time_ticks is not None and vt > int(max_virtual_time_ticks):
            raise ValueError(
                f"{ctx}: virtual_time_end {vt} > max {int(max_virtual_time_ticks)}"
            )

        _require_nonempty_str(ss, "sched_trace_hash", ctx=ctx)

    want_rr_req_min = assertions.get("min_rr_request_calls")
    want_rr_req_max = assertions.get("max_rr_request_calls")
    if want_rr_req_min is not None or want_rr_req_max is not None:
        got = solve_obj.get("rr_request_calls")
        if not isinstance(got, int):
            raise ValueError(f"{ctx}: missing rr_request_calls")
        _check_range(
            ctx=ctx,
            got=int(got),
            min_v=int(want_rr_req_min) if want_rr_req_min is not None else None,
            max_v=int(want_rr_req_max) if want_rr_req_max is not None else None,
            label="rr_request_calls",
        )

    want_min_read = assertions.get("min_fs_read_file_calls")
    want_max_read = assertions.get("max_fs_read_file_calls")
    if want_min_read is not None or want_max_read is not None:
        got = solve_obj.get("fs_read_file_calls")
        if not isinstance(got, int):
            raise ValueError(f"{ctx}: missing fs_read_file_calls")
        _check_range(
            ctx=ctx,
            got=int(got),
            min_v=int(want_min_read) if want_min_read is not None else None,
            max_v=int(want_max_read) if want_max_read is not None else None,
            label="fs_read_file_calls",
        )

    get_min = assertions.get("min_kv_get_calls")
    get_max = assertions.get("max_kv_get_calls")
    if get_min is not None or get_max is not None:
        got = solve_obj.get("kv_get_calls")
        if not isinstance(got, int):
            raise ValueError(f"{ctx}: missing kv_get_calls")
        _check_range(
            ctx=ctx,
            got=int(got),
            min_v=int(get_min) if get_min is not None else None,
            max_v=int(get_max) if get_max is not None else None,
            label="kv_get_calls",
        )

    set_min = assertions.get("min_kv_set_calls")
    set_max = assertions.get("max_kv_set_calls")
    if set_min is not None or set_max is not None:
        got = solve_obj.get("kv_set_calls")
        if not isinstance(got, int):
            raise ValueError(f"{ctx}: missing kv_set_calls")
        _check_range(
            ctx=ctx,
            got=int(got),
            min_v=int(set_min) if set_min is not None else None,
            max_v=int(set_max) if set_max is not None else None,
            label="kv_set_calls",
        )

    list_min = assertions.get("min_fs_list_dir_calls")
    list_max = assertions.get("max_fs_list_dir_calls")
    if list_min is not None or list_max is not None:
        got = solve_obj.get("fs_list_dir_calls")
        if not isinstance(got, int):
            raise ValueError(f"{ctx}: missing fs_list_dir_calls")
        _check_range(
            ctx=ctx,
            got=int(got),
            min_v=int(list_min) if list_min is not None else None,
            max_v=int(list_max) if list_max is not None else None,
            label="fs_list_dir_calls",
        )

    caps = assertions.get("capabilities_required")
    if isinstance(caps, list):
        for cap in caps:
            if not isinstance(cap, str):
                continue
            need = cap.strip()
            if not need:
                continue
            if need == "fs.read_file":
                got = solve_obj.get("fs_read_file_calls")
                if not isinstance(got, int):
                    raise ValueError(f"{ctx}: missing fs_read_file_calls for {need}")
                if got < 1:
                    raise ValueError(f"{ctx}: capability {need} requires fs_read_file_calls >= 1")
            elif need == "fs.list_dir":
                got = solve_obj.get("fs_list_dir_calls")
                if not isinstance(got, int):
                    raise ValueError(f"{ctx}: missing fs_list_dir_calls for {need}")
                if got < 1:
                    raise ValueError(f"{ctx}: capability {need} requires fs_list_dir_calls >= 1")
            elif need == "rr.fetch":
                got = solve_obj.get("rr_request_calls")
                if not isinstance(got, int):
                    raise ValueError(f"{ctx}: missing rr_request_calls for {need}")
                if got < 1:
                    raise ValueError(f"{ctx}: capability {need} requires rr_request_calls >= 1")
            elif need == "kv.get":
                got = solve_obj.get("kv_get_calls")
                if not isinstance(got, int):
                    raise ValueError(f"{ctx}: missing kv_get_calls for {need}")
                if got < 1:
                    raise ValueError(f"{ctx}: capability {need} requires kv_get_calls >= 1")
            elif need == "kv.set":
                got = solve_obj.get("kv_set_calls")
                if not isinstance(got, int):
                    raise ValueError(f"{ctx}: missing kv_set_calls for {need}")
                if got < 1:
                    raise ValueError(f"{ctx}: capability {need} requires kv_set_calls >= 1")


def _enforce_replay(
    *,
    host_runner: Path,
    suite: BenchSuite,
    world: str,
    artifact_path: Path,
    input_path: Path,
    out0: bytes,
    fuel0: int,
    sched_trace_hash0: str,
    solve_fuel: int,
    max_mem: int,
    replay_runs: int,
) -> None:
    if replay_runs <= 1:
        return

    for rep in range(1, replay_runs):
        cmd = [
            "--artifact",
            str(artifact_path),
            "--world",
            world,
            "--input",
            str(input_path),
            "--solve-fuel",
            str(int(solve_fuel)),
            "--max-memory-bytes",
            str(int(max_mem)),
        ]
        if suite.requires_debug_borrow_checks:
            cmd.append("--debug-borrow-checks")
        _extend_runner_cmd_with_fixtures(cmd=cmd, suite=suite)

        r = _run_host_runner_json(host_runner, cmd)
        if not bool(r.get("ok")):
            raise ValueError(f"replay{rep} failed")

        out_b = _unb64(str(r.get("solve_output_b64") or ""))
        if out_b != out0:
            raise ValueError(f"replay{rep} output mismatch")

        rfuel = r.get("fuel_used")
        if not isinstance(rfuel, int) or rfuel != int(fuel0):
            raise ValueError(f"replay{rep} fuel mismatch: got {rfuel} expected {fuel0}")

        rss = r.get("sched_stats")
        if not isinstance(rss, dict):
            raise ValueError(f"replay{rep} missing sched_stats")

        rth = rss.get("sched_trace_hash")
        if not isinstance(rth, str) or rth.strip() != sched_trace_hash0:
            raise ValueError(
                f"replay{rep} sched_trace_hash mismatch: got {rth!r} expected {sched_trace_hash0!r}"
            )


def _run_suite(
    *,
    host_runner: Path,
    suite_path: Path,
    suite: BenchSuite,
    solutions_dir: Path,
    solve_fuel: int,
    max_mem: int,
    perf_baseline_suite: dict[str, Any] | None,
    perf_out_suite: dict[str, Any] | None,
) -> tuple[int, int]:
    tasks = [t for t in suite.tasks if t.cases]
    tasks_total = len(tasks)
    tasks_ok = 0
    module_roots = _bench_module_roots()

    perf_baseline_tasks: dict[str, Any] | None = None
    if perf_baseline_suite is not None:
        v = perf_baseline_suite.get("tasks")
        if not isinstance(v, dict):
            print(
                f"{suite_path}: FAIL (perf) baseline suite missing tasks map",
                file=sys.stderr,
            )
            return 0, tasks_total

        baseline_ids = set(v.keys())
        expected_ids = {t.task_id for t in tasks}
        if baseline_ids != expected_ids:
            missing = sorted(expected_ids - baseline_ids)
            extra = sorted(baseline_ids - expected_ids)
            if missing:
                print(
                    f"{suite_path}: FAIL (perf) baseline missing tasks: {missing}",
                    file=sys.stderr,
                )
            if extra:
                print(
                    f"{suite_path}: FAIL (perf) baseline has extra tasks: {extra}",
                    file=sys.stderr,
                )
            return 0, tasks_total

        perf_baseline_tasks = v

    perf_out_tasks: dict[str, Any] | None = None
    if perf_out_suite is not None:
        v = perf_out_suite.get("tasks")
        if v is None:
            v = {}
            perf_out_suite["tasks"] = v
        if not isinstance(v, dict):
            print(
                f"{suite_path}: FAIL (perf) output suite tasks must be an object",
                file=sys.stderr,
            )
            return 0, tasks_total
        perf_out_tasks = v

    for task in tasks:

        task_world = task.task_world or suite.world

        solution = _solution_path(solutions_dir, task.task_id)

        with tempfile.TemporaryDirectory(prefix="x07_bench_task_") as tmp:
            tmp_dir = Path(tmp)
            artifact_path = tmp_dir / "solver.exe"

            case0 = task.cases[0]
            input0 = tmp_dir / "input0.bin"
            input0.write_bytes(case0.input_bytes)

            cmd = [
                "--program",
                str(solution),
                "--world",
                str(task_world),
                "--input",
                str(input0),
                "--solve-fuel",
                str(int(solve_fuel)),
                "--max-memory-bytes",
                str(int(max_mem)),
                "--compiled-out",
                str(artifact_path),
            ]
            for r in module_roots:
                cmd.extend(["--module-root", r])
            if suite.requires_debug_borrow_checks:
                cmd.append("--debug-borrow-checks")
            _extend_runner_cmd_with_fixtures(cmd=cmd, suite=suite)

            try:
                r0 = _run_host_runner_json(host_runner, cmd)
            except Exception as e:
                print(
                    f"{suite_path}: {task.task_id}: FAIL (runner error): {e}",
                    file=sys.stderr,
                )
                continue

            compile_obj = r0.get("compile")
            solve0 = r0.get("solve")
            if not isinstance(compile_obj, dict) or not bool(compile_obj.get("ok")):
                err = None
                if isinstance(compile_obj, dict):
                    err = compile_obj.get("compile_error")
                print(
                    f"{suite_path}: {task.task_id}: FAIL (compile) {err!r}",
                    file=sys.stderr,
                )
                continue
            if not isinstance(solve0, dict) or not bool(solve0.get("ok")):
                trap = solve0.get("trap") if isinstance(solve0, dict) else None
                print(
                    f"{suite_path}: {task.task_id}: FAIL (run) trap={trap!r}",
                    file=sys.stderr,
                )
                continue

            try:
                out0 = _unb64(str(solve0.get("solve_output_b64") or ""))
            except Exception as e:
                print(
                    f"{suite_path}: {task.task_id}: FAIL (bad output base64) {e!r}",
                    file=sys.stderr,
                )
                continue

            if out0 != case0.expected_bytes:
                print(
                    f"{suite_path}: {task.task_id}: FAIL (wrong output) case0={case0.name!r}",
                    file=sys.stderr,
                )
                continue

            try:
                _enforce_assertions(
                    ctx=f"{task.task_id}/case0",
                    solve_obj=solve0,
                    assertions=_merge_assertions(case0, task),
                )
            except Exception as e:
                print(
                    f"{suite_path}: {task.task_id}: FAIL (assertions) {e}",
                    file=sys.stderr,
                )
                continue

            assertions0 = _merge_assertions(case0, task)
            replay_required = bool(assertions0.get("replay_required") or False)
            replay_runs_v = assertions0.get("replay_runs")
            replay_runs = 1
            if replay_required:
                replay_runs = max(1, int(replay_runs_v) if replay_runs_v is not None else 2)

            if replay_required:
                fuel0 = solve0.get("fuel_used")
                ss = solve0.get("sched_stats")
                if not isinstance(fuel0, int) or not isinstance(ss, dict):
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (replay) missing fuel_used or sched_stats",
                        file=sys.stderr,
                    )
                    continue
                try:
                    sched_hash0 = _require_nonempty_str(
                        ss, "sched_trace_hash", ctx=f"{task.task_id}/case0"
                    )
                except Exception as e:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (replay) {e}",
                        file=sys.stderr,
                    )
                    continue

                try:
                    _enforce_replay(
                        host_runner=host_runner,
                        suite=suite,
                        world=task_world,
                        artifact_path=artifact_path,
                        input_path=input0,
                        out0=out0,
                        fuel0=int(fuel0),
                        sched_trace_hash0=sched_hash0,
                        solve_fuel=solve_fuel,
                        max_mem=max_mem,
                        replay_runs=replay_runs,
                    )
                except Exception as e:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (replay) {e}",
                        file=sys.stderr,
                    )
                    continue

            compile_fuel_used: int | None = None
            c_source_size: int | None = None
            perf_cases: list[dict[str, Any]] | None = None
            baseline_cases: list[Any] | None = None

            if perf_baseline_tasks is not None or perf_out_tasks is not None:
                compile_fuel_used = _require_int(
                    compile_obj, "fuel_used", ctx=f"{task.task_id}/compile"
                )
                c_source_size = _require_int(
                    compile_obj, "c_source_size", ctx=f"{task.task_id}/compile"
                )
                perf_cases = [
                    {
                        "name": case0.name,
                        "fuel_used": _require_int(
                            solve0, "fuel_used", ctx=f"{task.task_id}/case0"
                        ),
                        "heap_used": _require_int(
                            solve0, "heap_used", ctx=f"{task.task_id}/case0"
                        ),
                    }
                ]

            if perf_baseline_tasks is not None:
                baseline_task = perf_baseline_tasks.get(task.task_id)
                if not isinstance(baseline_task, dict):
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) baseline task is not an object",
                        file=sys.stderr,
                    )
                    continue

                if compile_fuel_used is None or c_source_size is None or perf_cases is None:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) internal error: missing metrics",
                        file=sys.stderr,
                    )
                    continue

                baseline_compile_fuel = _require_int(
                    baseline_task,
                    "compile_fuel_used",
                    ctx=f"{task.task_id}/perf_baseline",
                )
                baseline_c_source_size = _require_int(
                    baseline_task,
                    "c_source_size",
                    ctx=f"{task.task_id}/perf_baseline",
                )
                if compile_fuel_used > baseline_compile_fuel:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) compile fuel regression: got {compile_fuel_used} baseline {baseline_compile_fuel}",
                        file=sys.stderr,
                    )
                    continue
                if c_source_size > baseline_c_source_size:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) C size regression: got {c_source_size} baseline {baseline_c_source_size}",
                        file=sys.stderr,
                    )
                    continue

                baseline_cases_v = baseline_task.get("cases")
                if not isinstance(baseline_cases_v, list):
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) baseline cases must be a list",
                        file=sys.stderr,
                    )
                    continue
                if len(baseline_cases_v) != len(task.cases):
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) baseline case count mismatch: got {len(task.cases)} baseline {len(baseline_cases_v)}",
                        file=sys.stderr,
                    )
                    continue
                baseline_cases = baseline_cases_v

                bc0 = baseline_cases[0]
                if not isinstance(bc0, dict):
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) baseline case0 is not an object",
                        file=sys.stderr,
                    )
                    continue

                got0_fuel = _require_int(
                    perf_cases[0], "fuel_used", ctx=f"{task.task_id}/case0"
                )
                got0_heap = _require_int(
                    perf_cases[0], "heap_used", ctx=f"{task.task_id}/case0"
                )
                base0_fuel = _require_int(
                    bc0, "fuel_used", ctx=f"{task.task_id}/perf_baseline/case0"
                )
                base0_heap = _require_int(
                    bc0, "heap_used", ctx=f"{task.task_id}/perf_baseline/case0"
                )
                if got0_fuel > base0_fuel:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) case0 fuel regression: got {got0_fuel} baseline {base0_fuel}",
                        file=sys.stderr,
                    )
                    continue
                if got0_heap > base0_heap:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) case0 heap regression: got {got0_heap} baseline {base0_heap}",
                        file=sys.stderr,
                    )
                    continue

            other_ok = True
            for i, c in enumerate(task.cases[1:], start=1):
                inp = tmp_dir / f"input{i}.bin"
                inp.write_bytes(c.input_bytes)
                cmd_i = [
                    "--artifact",
                    str(artifact_path),
                    "--world",
                    str(task_world),
                    "--input",
                    str(inp),
                    "--solve-fuel",
                    str(int(solve_fuel)),
                    "--max-memory-bytes",
                    str(int(max_mem)),
                ]
                if suite.requires_debug_borrow_checks:
                    cmd_i.append("--debug-borrow-checks")
                _extend_runner_cmd_with_fixtures(cmd=cmd_i, suite=suite)

                try:
                    r = _run_host_runner_json(host_runner, cmd_i)
                except Exception as e:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (runner error) case{i} {e}",
                        file=sys.stderr,
                    )
                    other_ok = False
                    break
                if not bool(r.get("ok")):
                    trap = r.get("trap")
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (run) case{i} trap={trap!r}",
                        file=sys.stderr,
                    )
                    other_ok = False
                    break
                try:
                    out_b = _unb64(str(r.get("solve_output_b64") or ""))
                except Exception as e:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (bad output base64) case{i} {e!r}",
                        file=sys.stderr,
                    )
                    other_ok = False
                    break
                if out_b != c.expected_bytes:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (wrong output) case{i}={c.name!r}",
                        file=sys.stderr,
                    )
                    other_ok = False
                    break

                try:
                    _enforce_assertions(
                        ctx=f"{task.task_id}/case{i}",
                        solve_obj=r,
                        assertions=_merge_assertions(c, task),
                    )
                except Exception as e:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (assertions) case{i} {e}",
                        file=sys.stderr,
                    )
                    other_ok = False
                    break

                assertions_i = _merge_assertions(c, task)
                replay_required_i = bool(assertions_i.get("replay_required") or False)
                replay_runs_v_i = assertions_i.get("replay_runs")
                replay_runs_i = 1
                if replay_required_i:
                    replay_runs_i = max(
                        1, int(replay_runs_v_i) if replay_runs_v_i is not None else 2
                    )
                if replay_required_i:
                    fuel_i = r.get("fuel_used")
                    ss_i = r.get("sched_stats")
                    if not isinstance(fuel_i, int) or not isinstance(ss_i, dict):
                        print(
                            f"{suite_path}: {task.task_id}: FAIL (replay) case{i} missing fuel_used or sched_stats",
                            file=sys.stderr,
                        )
                        other_ok = False
                        break
                    try:
                        sched_hash_i = _require_nonempty_str(
                            ss_i, "sched_trace_hash", ctx=f"{task.task_id}/case{i}"
                        )
                    except Exception as e:
                        print(
                            f"{suite_path}: {task.task_id}: FAIL (replay) case{i} {e}",
                            file=sys.stderr,
                        )
                        other_ok = False
                        break
                    try:
                        _enforce_replay(
                            host_runner=host_runner,
                            suite=suite,
                            world=task_world,
                            artifact_path=artifact_path,
                            input_path=inp,
                            out0=out_b,
                            fuel0=int(fuel_i),
                            sched_trace_hash0=sched_hash_i,
                            solve_fuel=solve_fuel,
                            max_mem=max_mem,
                            replay_runs=replay_runs_i,
                        )
                    except Exception as e:
                        print(
                            f"{suite_path}: {task.task_id}: FAIL (replay) case{i} {e}",
                            file=sys.stderr,
                        )
                        other_ok = False
                        break

                if perf_cases is not None:
                    perf_case = {
                        "name": c.name,
                        "fuel_used": _require_int(
                            r, "fuel_used", ctx=f"{task.task_id}/case{i}"
                        ),
                        "heap_used": _require_int(
                            r, "heap_used", ctx=f"{task.task_id}/case{i}"
                        ),
                    }
                    perf_cases.append(perf_case)

                    if baseline_cases is not None:
                        bci = baseline_cases[i]
                        if not isinstance(bci, dict):
                            print(
                                f"{suite_path}: {task.task_id}: FAIL (perf) baseline case{i} is not an object",
                                file=sys.stderr,
                            )
                            other_ok = False
                            break
                        base_fuel = _require_int(
                            bci, "fuel_used", ctx=f"{task.task_id}/perf_baseline/case{i}"
                        )
                        base_heap = _require_int(
                            bci, "heap_used", ctx=f"{task.task_id}/perf_baseline/case{i}"
                        )
                        if perf_case["fuel_used"] > base_fuel:
                            print(
                                f"{suite_path}: {task.task_id}: FAIL (perf) case{i} fuel regression: got {perf_case['fuel_used']} baseline {base_fuel}",
                                file=sys.stderr,
                            )
                            other_ok = False
                            break
                        if perf_case["heap_used"] > base_heap:
                            print(
                                f"{suite_path}: {task.task_id}: FAIL (perf) case{i} heap regression: got {perf_case['heap_used']} baseline {base_heap}",
                                file=sys.stderr,
                            )
                            other_ok = False
                            break

            if not other_ok:
                continue

            if perf_out_tasks is not None:
                if compile_fuel_used is None or c_source_size is None or perf_cases is None:
                    print(
                        f"{suite_path}: {task.task_id}: FAIL (perf) internal error: missing metrics",
                        file=sys.stderr,
                    )
                    continue
                perf_out_tasks[task.task_id] = {
                    "compile_fuel_used": compile_fuel_used,
                    "c_source_size": c_source_size,
                    "cases": perf_cases,
                }

            tasks_ok += 1

    print(f"ok: {suite_path} tasks={tasks_ok}/{tasks_total}")
    return tasks_ok, tasks_total


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Run benchmark suites against committed reference solutions.")
    ap.add_argument("--suite", required=True, help="Suite JSON path or bundle JSON path.")
    ap.add_argument(
        "--solutions",
        default="benchmarks/solutions",
        help="Directory containing reference solutions by task_id.",
    )
    ap.add_argument("--solve-fuel", type=int, default=int(os.environ.get("X07_SOLVE_FUEL") or 50_000_000))
    ap.add_argument(
        "--max-memory-bytes",
        type=int,
        default=int(os.environ.get("X07_RUN_MAX_MEMORY_BYTES") or 64 * 1024 * 1024),
    )
    ap.add_argument(
        "--perf-baseline",
        help="Perf baseline JSON; fails if fuel/heap/C size exceed baseline.",
    )
    ap.add_argument(
        "--perf-baseline-out",
        help="Write perf baseline JSON for this run to the given path.",
    )
    args = ap.parse_args(argv)

    root = _repo_root()
    suite_path = (root / args.suite).resolve() if not Path(args.suite).is_absolute() else Path(args.suite)
    if not suite_path.is_file():
        _die(f"missing suite: {suite_path}", code=2)

    solutions_dir = (root / args.solutions).resolve() if not Path(args.solutions).is_absolute() else Path(args.solutions)
    if not solutions_dir.is_dir():
        _die(f"missing solutions dir: {solutions_dir}", code=2)

    perf_baseline: dict[str, Any] | None = None
    if args.perf_baseline:
        perf_path = (root / args.perf_baseline).resolve() if not Path(args.perf_baseline).is_absolute() else Path(args.perf_baseline)
        if not perf_path.is_file():
            _die(f"missing perf baseline: {perf_path}", code=2)
        perf_baseline = _load_perf_baseline(perf_path)

    perf_out: dict[str, Any] | None = None
    perf_out_path: Path | None = None
    if args.perf_baseline_out:
        perf_out_path = (root / args.perf_baseline_out).resolve() if not Path(args.perf_baseline_out).is_absolute() else Path(args.perf_baseline_out)
        perf_out = {"schema_version": _PERF_BASELINE_SCHEMA, "suites": {}}

    host_runner = _ensure_host_runner_bin()

    total_ok = 0
    total_tasks = 0

    if _is_bundle(suite_path):
        bundle = _load_bundle(suite_path)
        suite_paths: list[Path] = []
        for rel in bundle.pre_score_canaries + bundle.score_suites + bundle.debug_suites:
            p = root / rel
            if not p.is_file():
                _die(f"bundle references missing suite: {rel}", code=2)
            suite_paths.append(p)
    else:
        suite_paths = [suite_path]

    for sp in suite_paths:
        suite_rel = _suite_rel_path(sp)
        suite = _load_suite(sp)

        perf_baseline_suite = None
        if perf_baseline is not None:
            suites = perf_baseline.get("suites")
            if isinstance(suites, dict):
                perf_baseline_suite = suites.get(suite_rel)
            if not isinstance(perf_baseline_suite, dict):
                expected_tasks = len([t for t in suite.tasks if t.cases])
                print(
                    f"{sp}: FAIL (perf) baseline missing suite entry {suite_rel!r}",
                    file=sys.stderr,
                )
                total_ok += 0
                total_tasks += expected_tasks
                continue

        perf_out_suite = None
        if perf_out is not None:
            suites = perf_out.get("suites")
            if isinstance(suites, dict):
                perf_out_suite = {"tasks": {}}
                suites[suite_rel] = perf_out_suite

        ok, total = _run_suite(
            host_runner=host_runner,
            suite_path=sp,
            suite=suite,
            solutions_dir=solutions_dir,
            solve_fuel=int(args.solve_fuel),
            max_mem=int(args.max_memory_bytes),
            perf_baseline_suite=perf_baseline_suite,
            perf_out_suite=perf_out_suite,
        )
        total_ok += ok
        total_tasks += total

    if total_ok != total_tasks:
        if perf_out_path is not None:
            print("not writing perf baseline: run failed", file=sys.stderr)
        print(f"FAIL: tasks={total_ok}/{total_tasks}", file=sys.stderr)
        return 1

    if perf_out is not None and perf_out_path is not None:
        _write_perf_baseline(perf_out_path, perf_out)

    print(f"ok: all tasks passed ({total_ok})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
