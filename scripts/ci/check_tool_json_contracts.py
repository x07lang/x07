#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


NATIVE_JSON_SCOPES = {
    "diag.explain",
    "doc",
    "fmt",
    "lint",
    "fix",
    "pkg.provides",
    "schema.derive",
    "sm.check",
    "test",
    "patch.apply",
}


def run_cmd(argv: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv,
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )


def parse_json_doc(stdout: str, context: str) -> dict:
    try:
        value = json.loads(stdout)
    except Exception as exc:  # noqa: BLE001
        raise SystemExit(f"{context}: stdout is not valid JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise SystemExit(f"{context}: expected JSON object")
    return value


def main() -> int:
    repo_root = Path(__file__).resolve().parents[2]
    find_bin = repo_root / "scripts" / "ci" / "find_x07.sh"
    x07_bin = os.environ.get("X07_BIN")
    if not x07_bin:
        proc = run_cmd([str(find_bin)], repo_root)
        if proc.returncode != 0:
            sys.stderr.write(proc.stderr)
            return proc.returncode or 2
        x07_bin = proc.stdout.strip()
    if not x07_bin:
        raise SystemExit("failed to resolve x07 binary path")

    specrows_proc = run_cmd([x07_bin, "--cli-specrows"], repo_root)
    if specrows_proc.returncode != 0:
        sys.stderr.write(specrows_proc.stderr)
        return specrows_proc.returncode or 2
    specrows = parse_json_doc(specrows_proc.stdout, "x07 --cli-specrows")
    rows = specrows.get("rows")
    if not isinstance(rows, list):
        raise SystemExit("x07 --cli-specrows: missing rows array")

    scopes: set[str] = set()
    for row in rows:
        if not isinstance(row, list) or len(row) < 2:
            continue
        path = row[0]
        kind = row[1]
        if not isinstance(path, str) or path == "root":
            continue
        if kind == "about":
            scopes.add(path)

    if not scopes:
        raise SystemExit("no command scopes discovered from --cli-specrows")

    for scope in sorted(scopes):
        argv = [x07_bin, *scope.split("."), "--json-schema"]
        proc = run_cmd(argv, repo_root)
        if proc.returncode != 0:
            raise SystemExit(
                f"{scope}: --json-schema failed with exit={proc.returncode}\n{proc.stderr}"
            )
        doc = parse_json_doc(proc.stdout, f"{scope} --json-schema")
        if "type" not in doc:
            raise SystemExit(f"{scope}: schema JSON missing type")

    for scope in sorted(scopes):
        if scope in NATIVE_JSON_SCOPES:
            continue
        argv = [x07_bin, *scope.split("."), "--json"]
        proc = run_cmd(argv, repo_root)
        if proc.stderr.strip():
            raise SystemExit(f"{scope}: --json emitted stderr: {proc.stderr.strip()}")
        doc = parse_json_doc(proc.stdout, f"{scope} --json")
        for key in ("schema_version", "command", "ok", "exit_code", "diagnostics", "result"):
            if key not in doc:
                raise SystemExit(f"{scope}: --json report missing key {key!r}")
        if not isinstance(doc["diagnostics"], list):
            raise SystemExit(f"{scope}: diagnostics must be an array")

    with tempfile.TemporaryDirectory(prefix="x07_tool_json_") as tmpdir:
        report_path = Path(tmpdir) / "report.json"
        proc = run_cmd(
            [
                x07_bin,
                "guide",
                "--json",
                "--report-out",
                str(report_path),
                "--quiet-json",
            ],
            repo_root,
        )
        if proc.returncode != 0:
            raise SystemExit(
                f"guide --json --report-out --quiet-json failed: {proc.stderr}"
            )
        if proc.stdout.strip():
            raise SystemExit("guide --quiet-json should not emit stdout")
        if not report_path.is_file():
            raise SystemExit("guide --quiet-json did not write --report-out")
        parse_json_doc(report_path.read_text(encoding="utf-8"), "guide --report-out")

    proc = run_cmd([x07_bin, "guide", "--jsonl"], repo_root)
    if proc.returncode != 0:
        raise SystemExit(f"guide --jsonl failed: {proc.stderr}")
    lines = [ln for ln in proc.stdout.splitlines() if ln.strip()]
    if not lines:
        raise SystemExit("guide --jsonl emitted no lines")
    for idx, line in enumerate(lines, start=1):
        try:
            json.loads(line)
        except Exception as exc:  # noqa: BLE001
            raise SystemExit(f"guide --jsonl line {idx} is not valid JSON: {exc}") from exc

    for schema_rel in ("spec/x07-tool.report.schema.json", "spec/x07.patchset.schema.json"):
        schema_path = repo_root / schema_rel
        if not schema_path.is_file():
            raise SystemExit(f"missing schema file: {schema_rel}")
        parse_json_doc(schema_path.read_text(encoding="utf-8"), schema_rel)

    print("ok: tool JSON contracts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
