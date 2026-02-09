from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        raise SystemExit(f"ERROR: parse {path}: {e}")


def _read_stdin_json() -> Any:
    try:
        return json.load(sys.stdin)
    except Exception as e:
        raise SystemExit(f"ERROR: parse stdin JSON: {e}")


def _as_int(v: Any) -> int:
    if isinstance(v, bool):
        return int(v)
    if isinstance(v, int):
        return v
    return 0


def _as_str(v: Any) -> str:
    return v if isinstance(v, str) else ""


def _unwrap_bench_report(report: Any) -> dict:
    if not isinstance(report, dict):
        raise SystemExit("ERROR: report must be JSON object")

    schema_version = _as_str(report.get("schema_version"))
    if schema_version == "x07.bench.report@0.1.0":
        return report

    if schema_version == "x07.tool.bench.eval.report@0.1.0":
        result = report.get("result")
        if isinstance(result, dict):
            stdout_json = result.get("stdout_json")
            if isinstance(stdout_json, dict):
                inner_sv = _as_str(stdout_json.get("schema_version"))
                if inner_sv == "x07.bench.report@0.1.0":
                    return stdout_json

        raise SystemExit("ERROR: tool report missing result.stdout_json bench report")

    raise SystemExit(f"ERROR: unsupported schema_version: {schema_version!r}")


def score_bench_report(report: dict) -> dict:
    report = _unwrap_bench_report(report)

    instances = report.get("instances", [])
    if not isinstance(instances, list):
        raise SystemExit("ERROR: report.instances must be array")

    summary = report.get("summary") if isinstance(report.get("summary"), dict) else {}
    duration_ms_total = _as_int(summary.get("duration_ms"))

    counts = {"resolved": 0, "unresolved": 0, "error": 0, "skipped": 0}
    compile_ok = 0
    compile_total = 0
    repair_iters_total = 0
    repair_ops_total = 0
    repair_count = 0

    for inst in instances:
        if not isinstance(inst, dict):
            continue
        status = _as_str(inst.get("status"))
        if status in counts:
            counts[status] += 1

        after_patch = inst.get("after_patch")
        if isinstance(after_patch, dict):
            compile_total += 1
            if _as_int(after_patch.get("compile_failures")) == 0:
                compile_ok += 1

        repair = inst.get("repair")
        if isinstance(repair, dict):
            repair_count += 1
            repair_iters_total += _as_int(repair.get("iterations"))
            repair_ops_total += _as_int(repair.get("applied_ops_count"))

    instances_total = len(instances)
    resolved = counts["resolved"]
    resolution_rate = (resolved / instances_total) if instances_total else 0.0
    first_pass_compile_ok_rate = (compile_ok / compile_total) if compile_total else 0.0
    avg_repair_iterations = (repair_iters_total / repair_count) if repair_count else 0.0
    avg_repair_applied_ops = (repair_ops_total / repair_count) if repair_count else 0.0
    avg_duration_ms_per_instance = (duration_ms_total / instances_total) if instances_total else 0.0

    tool = report.get("tool") if isinstance(report.get("tool"), dict) else {}
    tool_version = _as_str(tool.get("version"))

    suite = report.get("suite") if isinstance(report.get("suite"), dict) else {}
    suite_id = _as_str(suite.get("suite_id"))

    return {
        "schema_version": "x07.bench.score@0.1.0",
        "tool_version": tool_version,
        "suite_id": suite_id,
        "instances_total": instances_total,
        "resolved": resolved,
        "unresolved": counts["unresolved"],
        "errors": counts["error"],
        "skipped": counts["skipped"],
        "duration_ms_total": duration_ms_total,
        "avg_duration_ms_per_instance": avg_duration_ms_per_instance,
        "resolution_rate": resolution_rate,
        "first_pass_compile_ok_rate": first_pass_compile_ok_rate,
        "avg_repair_iterations": avg_repair_iterations,
        "avg_repair_applied_ops": avg_repair_applied_ops,
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--in", dest="in_path", type=Path, default=None, help="Bench report JSON path (default: stdin)")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    report = _read_json(args.in_path) if args.in_path is not None else _read_stdin_json()
    if not isinstance(report, dict):
        raise SystemExit("ERROR: report must be JSON object")

    scored = score_bench_report(report)
    sys.stdout.write(json.dumps(scored, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
