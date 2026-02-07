#!/usr/bin/env python3
from __future__ import annotations

import argparse
import copy
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SPEC_DIR = REPO_ROOT / "spec"
BASE_SCHEMA_PATH = SPEC_DIR / "x07-tool.report.schema.json"
RUST_SCHEMA_MAP_PATH = REPO_ROOT / "crates" / "x07" / "src" / "tool_report_schemas.rs"

FIND_X07 = REPO_ROOT / "scripts" / "ci" / "find_x07.sh"

# Scopes with a dedicated native JSON report schema (tool wrapper schema is not used).
NATIVE_REPORT_SCOPES = {
    "doc",
}


@dataclass(frozen=True)
class PlannedWrite:
    path: Path
    content: bytes


def run_cmd(argv: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv,
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )


def resolve_x07_bin() -> str:
    x07_bin = os.environ.get("X07_BIN")
    if x07_bin:
        return x07_bin
    proc = run_cmd([str(FIND_X07)], REPO_ROOT)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(proc.returncode or 2)
    x07_bin = proc.stdout.strip()
    if not x07_bin:
        raise SystemExit("failed to resolve x07 binary path")
    return x07_bin


def discover_scopes(x07_bin: str) -> list[str]:
    proc = run_cmd([x07_bin, "--cli-specrows"], REPO_ROOT)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(proc.returncode or 2)
    doc = json.loads(proc.stdout)
    rows = doc.get("rows", [])
    scopes: set[str] = set()
    for row in rows:
        if not isinstance(row, list) or len(row) < 2:
            continue
        scope = row[0]
        kind = row[1]
        if not isinstance(scope, str) or scope == "root":
            continue
        if kind == "about":
            scopes.add(scope)
    out = sorted(scopes)
    if not out:
        raise SystemExit("no command scopes discovered from --cli-specrows")
    return out


def semver_from_base_schema(base_schema: dict) -> str:
    title = base_schema.get("title", "")
    if isinstance(title, str) and "@" in title:
        return title.split("@", 1)[1].strip()
    raise SystemExit(f"base schema title missing @semver: {title!r}")


def tool_schema_version(scope: str | None, semver: str) -> str:
    if scope is None:
        return f"x07.tool.root.report@{semver}"
    return f"x07.tool.{scope}.report@{semver}"


def tool_command_id(scope: str | None) -> str:
    if scope is None:
        return "x07"
    return f"x07.{scope}"


def tool_schema_filename(scope: str | None) -> str:
    if scope is None:
        return "x07-tool-root.report.schema.json"
    return f"x07-tool-{scope.replace('.', '-')}.report.schema.json"


def tool_schema_url(filename: str) -> str:
    return f"https://x07.io/spec/{filename}"


def build_tool_schema(base_schema: dict, scope: str | None, semver: str) -> dict:
    schema = copy.deepcopy(base_schema)
    filename = tool_schema_filename(scope)
    schema_version = tool_schema_version(scope, semver)
    command_id = tool_command_id(scope)

    schema["$id"] = tool_schema_url(filename)
    schema["title"] = schema_version

    props = schema.get("properties")
    if not isinstance(props, dict):
        raise SystemExit("base schema missing properties object")
    for key, const_value in (("schema_version", schema_version), ("command", command_id)):
        entry = props.get(key)
        if not isinstance(entry, dict):
            raise SystemExit(f"base schema properties.{key} must be an object")
        entry["const"] = const_value
        if "type" not in entry:
            entry["type"] = "string"

    return schema


def planned_tool_schema_writes(scopes: list[str]) -> list[PlannedWrite]:
    base_schema = json.loads(BASE_SCHEMA_PATH.read_text(encoding="utf-8"))
    semver = semver_from_base_schema(base_schema)

    planned: list[PlannedWrite] = []
    planned.append(
        PlannedWrite(
            path=SPEC_DIR / tool_schema_filename(None),
            content=(json.dumps(build_tool_schema(base_schema, None, semver), indent=2) + "\n").encode(
                "utf-8"
            ),
        )
    )
    for scope in scopes:
        if scope in NATIVE_REPORT_SCOPES:
            continue
        filename = tool_schema_filename(scope)
        planned.append(
            PlannedWrite(
                path=SPEC_DIR / filename,
                content=(
                    json.dumps(build_tool_schema(base_schema, scope, semver), indent=2) + "\n"
                ).encode("utf-8"),
            )
        )
    planned.append(PlannedWrite(path=RUST_SCHEMA_MAP_PATH, content=render_rust_schema_map(scopes)))
    return planned


def render_rust_schema_map(scopes: list[str]) -> bytes:
    lines: list[str] = []
    lines.append("use std::ffi::OsStr;")
    lines.append("")
    lines.append("pub(crate) fn tool_report_schema_bytes(scope: Option<&OsStr>) -> Option<&'static [u8]> {")
    lines.append("    match scope.and_then(|s| s.to_str()) {")
    lines.append(
        '        None => Some(include_bytes!("../../../spec/x07-tool-root.report.schema.json")),'
    )
    for scope in scopes:
        if scope in NATIVE_REPORT_SCOPES:
            continue
        filename = tool_schema_filename(scope)
        lines.append(
            f'        Some("{scope}") => Some(include_bytes!("../../../spec/{filename}")),'
        )
    lines.append("        _ => None,")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    return ("\n".join(lines)).encode("utf-8")


def remove_stale_tool_schemas(expected: set[Path], check: bool) -> list[Path]:
    removed: list[Path] = []
    for path in sorted(SPEC_DIR.glob("x07-tool-*.report.schema.json")):
        if path not in expected:
            removed.append(path)
    if check or not removed:
        return removed
    for path in removed:
        path.unlink()
    return removed


def write_if_changed(path: Path, content: bytes, check: bool) -> bool:
    if path.exists() and path.read_bytes() == content:
        return False
    if check:
        return True
    path.write_bytes(content)
    return True


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description="Generate per-scope x07 tool wrapper report schemas.")
    ap.add_argument("--check", action="store_true", help="Fail if outputs would change.")
    return ap.parse_args()


def main() -> int:
    args = parse_args()
    if not BASE_SCHEMA_PATH.is_file():
        print(f"ERROR: missing base schema: {BASE_SCHEMA_PATH}", file=sys.stderr)
        return 2

    x07_bin = resolve_x07_bin()
    scopes = discover_scopes(x07_bin)
    planned = planned_tool_schema_writes(scopes)
    expected_paths = {pw.path for pw in planned}

    changed: list[Path] = []
    for pw in planned:
        if write_if_changed(pw.path, pw.content, args.check):
            changed.append(pw.path)

    removed = remove_stale_tool_schemas(expected_paths, args.check)

    if args.check and (changed or removed):
        print("ERROR: tool wrapper report schemas are out of date", file=sys.stderr)
        for path in changed:
            print(f"  would update: {path.relative_to(REPO_ROOT)}", file=sys.stderr)
        for path in removed:
            print(f"  would remove: {path.relative_to(REPO_ROOT)}", file=sys.stderr)
        print("hint: python3 scripts/gen_tool_report_schemas.py", file=sys.stderr)
        return 1

    if args.check:
        print("ok: tool wrapper report schemas are in sync")
        return 0

    for path in removed:
        changed.append(path)
    if changed:
        changed.sort()
        print(f"ok: wrote {len(changed)} schema files")
    else:
        print("ok: no changes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
