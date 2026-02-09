from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
SUITE_DIR = ROOT / "labs" / "x07bench" / "suites" / "core_v1"


@dataclass(frozen=True)
class PlannedWrite:
    path: Path
    content: bytes


def render_json(obj: object) -> bytes:
    return (json.dumps(obj, indent=2) + "\n").encode("utf-8")


def render_jsonl_row(obj: dict) -> str:
    return json.dumps(obj, separators=(",", ":"), ensure_ascii=False)


def module_assert_i32_eq(*, module_id: str, export: str, expr: object, expected: int) -> dict:
    return {
        "schema_version": "x07.x07ast@0.3.0",
        "kind": "module",
        "module_id": module_id,
        "imports": ["std.test"],
        "decls": [
            {"kind": "export", "names": [export]},
            {
                "kind": "defn",
                "name": export,
                "params": [],
                "result": "result_i32",
                "body": [
                    "begin",
                    ["let", "x", expr],
                    ["try", ["std.test.assert_i32_eq", "x", expected, ["std.test.code_assert_i32_eq"]]],
                    ["std.test.pass"],
                ],
            },
        ],
    }


def module_assert_true_eq(*, module_id: str, export: str, x_value: object, expected: object) -> dict:
    return {
        "schema_version": "x07.x07ast@0.3.0",
        "kind": "module",
        "module_id": module_id,
        "imports": ["std.test"],
        "decls": [
            {"kind": "export", "names": [export]},
            {
                "kind": "defn",
                "name": export,
                "params": [],
                "result": "result_i32",
                "body": [
                    "begin",
                    ["let", "x", x_value],
                    ["try", ["std.test.assert_true", ["=", "x", expected], ["std.test.code_assert_true"]]],
                    ["std.test.pass"],
                ],
            },
        ],
    }


def module_assert_true_eq_expr(*, module_id: str, export: str, expr: object, expected: object) -> dict:
    return {
        "schema_version": "x07.x07ast@0.3.0",
        "kind": "module",
        "module_id": module_id,
        "imports": ["std.test"],
        "decls": [
            {"kind": "export", "names": [export]},
            {
                "kind": "defn",
                "name": export,
                "params": [],
                "result": "result_i32",
                "body": [
                    "begin",
                    ["try", ["std.test.assert_true", ["=", expr, expected], ["std.test.code_assert_true"]]],
                    ["std.test.pass"],
                ],
            },
        ],
    }


def write_if_changed(path: Path, content: bytes, check: bool) -> bool:
    if path.exists() and path.read_bytes() == content:
        return False
    if check:
        return True
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    return True


def planned_core_v1_writes() -> list[PlannedWrite]:
    def inst_ref(instance_id: str) -> dict:
        return {"id": instance_id, "path": f"instances/{instance_id}", "enabled": True}

    def tests_manifest(*, test_id: str, entry: str, world: str) -> dict:
        return {
            "schema_version": "x07.tests_manifest@0.1.0",
            "tests": [{"id": test_id, "world": world, "entry": entry, "expect": "pass"}],
        }

    def instance_json(*, instance_id: str, tags: list[str], test_id: str, notes: list[str]) -> dict:
        return {
            "schema_version": "x07.bench.instance@0.1.0",
            "instance_id": instance_id,
            "tags": tags,
            "world": "solve-pure",
            "problem_statement_path": "issue.md",
            "repo_path": "repo",
            "eval": {
                "kind": "x07test",
                "manifest": "tests/tests.json",
                "module_root": ["modules"],
                "stdlib_lock": "stdlib.lock",
                "filter": None,
                "exact": False,
                "repeat": 1,
                "jobs": 1,
                "keep_artifacts": False,
                "artifact_dir": "target/x07test",
                "no_fail_fast": False,
                "no_run": False,
                "verbose": False,
                "fail_to_pass": [test_id],
                "pass_to_pass": [],
            },
            "oracle": {"patch_kind": "x07-arch-patchset-json", "patch_path": "oracle.patchset.json"},
            "notes": notes,
        }

    def issue_md(*, instance_id: str, test_id: str) -> bytes:
        return (
            f"# {instance_id}\n\n"
            f"The test `{test_id}` is failing.\n\n"
            "Goal: apply a patch that makes the test pass.\n"
        ).encode("utf-8")

    def oracle_patchset(*, module_path: str, patch_ops: list[dict], note: str) -> dict:
        return {
            "schema_version": "x07.arch.patchset@0.1.0",
            "patches": [{"path": module_path, "patch": patch_ops, "note": note}],
        }

    specs: list[dict] = [
        {
            "instance_id": "std_math_0001",
            "tags": ["stdlib", "math", "bugfix"],
            "test_id": "bench/math_bug",
            "module_file": "bench_math_bug.x07.json",
            "module": module_assert_i32_eq(
                module_id="bench_math_bug",
                export="bench_math_bug.check_add",
                expr=["+", 2, 2],
                expected=5,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/2/1/2", "value": 4}],
            "oracle_note": "Fix expected sum constant",
            "notes": ["Fix off-by-one expected constant in std.test.assert_i32_eq."],
        },
        {
            "instance_id": "std_logic_0002",
            "tags": ["stdlib", "logic", "bugfix"],
            "test_id": "bench/logic_bug",
            "module_file": "bench_logic_bug.x07.json",
            "module": module_assert_true_eq(
                module_id="bench_logic_bug",
                export="bench_logic_bug.check_guard",
                x_value=1,
                expected=2,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/2/1/1/2", "value": 1}],
            "oracle_note": "Fix expected equality constant",
            "notes": ["Fix wrong comparison target in std.test.assert_true."],
        },
        {
            "instance_id": "std_math_0003",
            "tags": ["math", "operator", "bugfix"],
            "test_id": "bench/math_mul",
            "module_file": "bench_math_mul_bug.x07.json",
            "module": module_assert_i32_eq(
                module_id="bench_math_mul_bug",
                export="bench_math_mul_bug.check_mul",
                expr=["+", 3, 4],
                expected=12,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/1/2/0", "value": "*"}],
            "oracle_note": "Fix operator (+ -> *)",
            "notes": ["Replace `+` with `*` in the computation."],
        },
        {
            "instance_id": "std_math_0004",
            "tags": ["math", "nested", "bugfix"],
            "test_id": "bench/math_nested_mul",
            "module_file": "bench_math_nested_bug.x07.json",
            "module": module_assert_i32_eq(
                module_id="bench_math_nested_bug",
                export="bench_math_nested_bug.check_nested",
                expr=["*", ["+", 2, 3], 3],
                expected=20,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/1/2/2", "value": 4}],
            "oracle_note": "Fix multiplier constant",
            "notes": ["Fix the multiplier so (2+3)*4 = 20."],
        },
        {
            "instance_id": "std_math_0005",
            "tags": ["math", "constants", "bugfix"],
            "test_id": "bench/math_add_const",
            "module_file": "bench_math_add_const_bug.x07.json",
            "module": module_assert_i32_eq(
                module_id="bench_math_add_const_bug",
                export="bench_math_add_const_bug.check_add",
                expr=["+", 40, 1],
                expected=42,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/1/2/2", "value": 2}],
            "oracle_note": "Fix addend constant",
            "notes": ["Fix the constant so 40 + 2 = 42."],
        },
        {
            "instance_id": "std_math_0006",
            "tags": ["math", "constants", "bugfix"],
            "test_id": "bench/math_sub_const",
            "module_file": "bench_math_sub_const_bug.x07.json",
            "module": module_assert_i32_eq(
                module_id="bench_math_sub_const_bug",
                export="bench_math_sub_const_bug.check_sub",
                expr=["-", 10, 3],
                expected=8,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/1/2/2", "value": 2}],
            "oracle_note": "Fix subtraction constant",
            "notes": ["Fix the constant so 10 - 2 = 8."],
        },
        {
            "instance_id": "std_math_0007",
            "tags": ["math", "expected", "bugfix"],
            "test_id": "bench/math_expected",
            "module_file": "bench_math_expected_bug.x07.json",
            "module": module_assert_i32_eq(
                module_id="bench_math_expected_bug",
                export="bench_math_expected_bug.check_expected",
                expr=["*", 6, 7],
                expected=43,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/2/1/2", "value": 42}],
            "oracle_note": "Fix expected constant",
            "notes": ["Fix the expected value for 6*7."],
        },
        {
            "instance_id": "std_logic_0008",
            "tags": ["logic", "let", "bugfix"],
            "test_id": "bench/logic_let",
            "module_file": "bench_logic_let_bug.x07.json",
            "module": module_assert_true_eq(
                module_id="bench_logic_let_bug",
                export="bench_logic_let_bug.check_let",
                x_value=2,
                expected=1,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/1/2", "value": 1}],
            "oracle_note": "Fix let-bound constant",
            "notes": ["Fix the let-bound value so the equality holds."],
        },
        {
            "instance_id": "std_logic_0009",
            "tags": ["logic", "expected", "bugfix"],
            "test_id": "bench/logic_expected",
            "module_file": "bench_logic_expected_bug.x07.json",
            "module": module_assert_true_eq(
                module_id="bench_logic_expected_bug",
                export="bench_logic_expected_bug.check_eq",
                x_value=7,
                expected=8,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/2/1/1/2", "value": 7}],
            "oracle_note": "Fix expected equality constant",
            "notes": ["Fix the equality constant so the guard is true."],
        },
        {
            "instance_id": "std_logic_0010",
            "tags": ["logic", "expr", "bugfix"],
            "test_id": "bench/logic_expr_expected",
            "module_file": "bench_logic_expr_bug.x07.json",
            "module": module_assert_true_eq(
                module_id="bench_logic_expr_bug",
                export="bench_logic_expr_bug.check_expr",
                x_value=["+", 2, 2],
                expected=5,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/2/1/1/2", "value": 4}],
            "oracle_note": "Fix expected equality constant",
            "notes": ["Fix the expected value so (2+2) equals 4."],
        },
        {
            "instance_id": "std_logic_0011",
            "tags": ["logic", "expr", "bugfix"],
            "test_id": "bench/logic_expr2",
            "module_file": "bench_logic_expr2_bug.x07.json",
            "module": module_assert_true_eq_expr(
                module_id="bench_logic_expr2_bug",
                export="bench_logic_expr2_bug.check_expr2",
                expr=["+", 3, 4],
                expected=8,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/1/1/1/2", "value": 7}],
            "oracle_note": "Fix expected equality constant",
            "notes": ["Fix the expected value so (3+4) equals 7."],
        },
        {
            "instance_id": "std_math_0012",
            "tags": ["math", "nested", "bugfix"],
            "test_id": "bench/math_sum3",
            "module_file": "bench_math_sum3_bug.x07.json",
            "module": module_assert_i32_eq(
                module_id="bench_math_sum3_bug",
                export="bench_math_sum3_bug.check_sum3",
                expr=["+", ["+", 1, 2], 3],
                expected=7,
            ),
            "oracle_ops": [{"op": "replace", "path": "/decls/1/body/2/1/2", "value": 6}],
            "oracle_note": "Fix expected constant",
            "notes": ["Fix the expected sum for 1+2+3."],
        },
    ]

    suite_instances: list[dict] = []
    writes: list[PlannedWrite] = []

    for s in specs:
        instance_id = s["instance_id"]
        suite_instances.append(inst_ref(instance_id))

        module_file = s["module_file"]
        module_path = Path("repo") / "modules" / module_file

        instance_root = Path("instances") / instance_id

        writes.append(
            PlannedWrite(
                path=SUITE_DIR / instance_root / "instance.json",
                content=render_json(
                    instance_json(
                        instance_id=instance_id,
                        tags=s["tags"],
                        test_id=s["test_id"],
                        notes=s["notes"],
                    )
                ),
            )
        )
        writes.append(
            PlannedWrite(
                path=SUITE_DIR / instance_root / "issue.md",
                content=issue_md(instance_id=instance_id, test_id=s["test_id"]),
            )
        )
        writes.append(
            PlannedWrite(
                path=SUITE_DIR / instance_root / "repo" / "tests" / "tests.json",
                content=render_json(
                    tests_manifest(
                        test_id=s["test_id"],
                        entry=s["module"]["decls"][1]["name"],
                        world="solve-pure",
                    )
                ),
            )
        )
        writes.append(
            PlannedWrite(
                path=SUITE_DIR / instance_root / module_path,
                content=render_json(s["module"]),
            )
        )
        writes.append(
            PlannedWrite(
                path=SUITE_DIR / instance_root / "oracle.patchset.json",
                content=render_json(
                    oracle_patchset(
                        module_path=str(Path("modules") / module_file),
                        patch_ops=s["oracle_ops"],
                        note=s["oracle_note"],
                    )
                ),
            )
        )

    suite = {
        "schema_version": "x07.bench.suite@0.1.0",
        "suite_id": "core_v1",
        "description": "Seed X07 benchmark suite (v1) for patch-based agent correctness evaluation.",
        "instances": suite_instances,
        "defaults": {
            "world": "solve-pure",
            "repair_mode": "write",
            "jobs": 1,
            "keep_artifacts": False,
            "artifact_dir": "target/x07bench",
            "determinism_runs": 2,
        },
    }

    writes.append(PlannedWrite(path=SUITE_DIR / "suite.json", content=render_json(suite)))

    predictions_lines = []
    for inst in suite_instances:
        instance_id = inst["id"]
        predictions_lines.append(
            render_jsonl_row(
                {
                    "schema_version": "x07.bench.prediction@0.1.0",
                    "instance_id": instance_id,
                    "model_name_or_path": "oracle",
                    "patch_kind": "x07-arch-patchset-json",
                    "patch_path": f"instances/{instance_id}/oracle.patchset.json",
                }
            )
        )
    predictions = ("\n".join(predictions_lines) + "\n").encode("utf-8")
    writes.append(
        PlannedWrite(path=SUITE_DIR / "predictions.oracle.jsonl", content=predictions)
    )

    readme = (
        "# core_v1\n\n"
        "Seed benchmark suite for `x07 bench`.\n\n"
        "Run locally:\n\n"
        "```sh\n"
        "x07 bench list --suite labs/x07bench/suites/core_v1/suite.json --format text\n"
        "x07 bench validate --suite labs/x07bench/suites/core_v1/suite.json --artifact-dir target/x07bench --format text\n"
        "x07 bench eval --suite labs/x07bench/suites/core_v1/suite.json --predictions labs/x07bench/suites/core_v1/predictions.oracle.jsonl --artifact-dir target/x07bench --format text\n"
        "```\n\n"
        "Baselines (committed):\n\n"
        "- `baselines/oracle.report.json`\n"
        "- `baselines/oracle.score.json`\n\n"
        "Regenerate baselines:\n\n"
        "```sh\n"
        "x07 bench eval --suite labs/x07bench/suites/core_v1/suite.json \\\n"
        "  --predictions labs/x07bench/suites/core_v1/predictions.oracle.jsonl \\\n"
        "  --artifact-dir target/x07bench --format json --out labs/x07bench/suites/core_v1/baselines/oracle.report.json\n"
        "python3 labs/x07bench/scripts/score_report.py \\\n"
        "  --in labs/x07bench/suites/core_v1/baselines/oracle.report.json \\\n"
        "  > labs/x07bench/suites/core_v1/baselines/oracle.score.json\n"
        "```\n\n"
        "Each instance includes:\n\n"
        "- `issue.md`\n"
        "- `repo/` broken snapshot\n"
        "- `oracle.patchset.json` expected fix\n"
    ).encode("utf-8")
    writes.append(PlannedWrite(path=SUITE_DIR / "README.md", content=readme))

    return writes


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="fail if outputs are out of date")
    args = ap.parse_args(argv)

    writes = planned_core_v1_writes()
    changed = False
    for w in writes:
        if write_if_changed(w.path, w.content, args.check):
            changed = True

    if args.check:
        if changed:
            print(f"ERROR: {SUITE_DIR} is out of date (run without --check)", file=sys.stderr)
            return 1
        print("ok: x07bench core_v1 suite up to date")
        return 0

    print("ok: wrote x07bench core_v1 suite")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
