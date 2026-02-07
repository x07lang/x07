#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
    from referencing import Registry, Resource
    from referencing.jsonschema import DRAFT202012
except Exception as exc:  # noqa: BLE001
    raise SystemExit(
        "missing python deps for tool JSON contract checks; install `jsonschema`"
    ) from exc


REPO_ROOT = Path(__file__).resolve().parents[2]
SPEC_DIR = REPO_ROOT / "spec"

X07DIAG_SCHEMA_PATH = SPEC_DIR / "x07diag.schema.json"
TOOL_EVENTS_SCHEMA_PATH = SPEC_DIR / "x07-tool.events.schema.json"

DOC_SCHEMA_PATH = SPEC_DIR / "x07-doc.report.schema.json"
TOOL_ROOT_SCHEMA_PATH = SPEC_DIR / "x07-tool-root.report.schema.json"


@dataclass(frozen=True)
class Scope:
    name: str
    argv: list[str]
    schema_path: Path
    schema_version: str


def run_cmd(argv: list[str], cwd: Path) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        argv,
        cwd=cwd,
        capture_output=True,
        check=False,
    )


def decode_utf8(data: bytes, context: str) -> str:
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise SystemExit(f"{context}: output is not utf-8: {exc}") from exc


def parse_json_doc_bytes(stdout: bytes, context: str) -> dict:
    try:
        value = json.loads(decode_utf8(stdout, context))
    except Exception as exc:  # noqa: BLE001
        raise SystemExit(f"{context}: stdout is not valid JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise SystemExit(f"{context}: expected JSON object")
    return value


def load_schema(path: Path) -> dict:
    doc = parse_json_doc_bytes(path.read_bytes(), str(path.relative_to(REPO_ROOT)))
    if not isinstance(doc.get("title"), str) or not doc["title"].strip():
        raise SystemExit(f"{path.relative_to(REPO_ROOT)}: missing schema title")
    if not isinstance(doc.get("$id"), str) or not doc["$id"].strip():
        raise SystemExit(f"{path.relative_to(REPO_ROOT)}: missing schema $id")
    return doc


def schema_version_const(schema: dict, context: str) -> str:
    props = schema.get("properties")
    if not isinstance(props, dict):
        raise SystemExit(f"{context}: schema missing properties object")
    entry = props.get("schema_version")
    if not isinstance(entry, dict):
        raise SystemExit(f"{context}: schema missing properties.schema_version object")
    const = entry.get("const")
    if not isinstance(const, str) or not const.strip():
        raise SystemExit(f"{context}: schema missing schema_version const")
    return const


def build_registry() -> Registry:
    if not X07DIAG_SCHEMA_PATH.is_file():
        raise SystemExit(f"missing schema file: {X07DIAG_SCHEMA_PATH.relative_to(REPO_ROOT)}")
    x07diag = load_schema(X07DIAG_SCHEMA_PATH)
    diag_resource = Resource.from_contents(x07diag, default_specification=DRAFT202012)

    reg = Registry()
    reg = reg.with_resource(x07diag["$id"], diag_resource)
    # Ensure x07diag is registered under common aliases (no network fetch in CI).
    reg = reg.with_resource("https://x07.io/spec/x07diag.schema.json", diag_resource)
    reg = reg.with_resource("x07diag.schema.json", diag_resource)
    return reg


def tool_wrapper_schema_path_for_scope(scope: str) -> Path:
    filename = f"x07-tool-{scope.replace('.', '-')}.report.schema.json"
    return SPEC_DIR / filename


def resolve_x07_bin() -> str:
    find_bin = REPO_ROOT / "scripts" / "ci" / "find_x07.sh"
    x07_bin = os.environ.get("X07_BIN")
    if not x07_bin:
        proc = run_cmd([str(find_bin)], REPO_ROOT)
        if proc.returncode != 0:
            sys.stderr.write(decode_utf8(proc.stderr, "find_x07.sh"))
            raise SystemExit(proc.returncode or 2)
        x07_bin = decode_utf8(proc.stdout, "find_x07.sh").strip()
    if not x07_bin:
        raise SystemExit("failed to resolve x07 binary path")
    return x07_bin


def load_cli_specrows(x07_bin: str) -> dict:
    proc = run_cmd([x07_bin, "--cli-specrows"], REPO_ROOT)
    if proc.returncode != 0:
        sys.stderr.write(decode_utf8(proc.stderr, "x07 --cli-specrows"))
        raise SystemExit(proc.returncode or 2)
    return parse_json_doc_bytes(proc.stdout, "x07 --cli-specrows")


def discover_scopes(specrows: dict) -> list[str]:
    rows = specrows.get("rows")
    if not isinstance(rows, list):
        raise SystemExit("x07 --cli-specrows: missing rows array")

    scopes: set[str] = set()
    for row in rows:
        if not isinstance(row, list) or len(row) < 2:
            continue
        scope = row[0]
        kind = row[1]
        if not isinstance(scope, str) or not isinstance(kind, str):
            continue
        if kind == "about" and scope != "root":
            scopes.add(scope)

    out = sorted(scopes)
    if not out:
        raise SystemExit("no command scopes discovered from --cli-specrows")
    return out


def has_long_opt(rows: list, scope: str, long_opt: str) -> bool:
    for row in rows:
        if not isinstance(row, list) or len(row) < 4:
            continue
        if row[0] != scope:
            continue
        kind = row[1]
        if kind not in {"flag", "opt"}:
            continue
        if row[3] == long_opt:
            return True
    return False


def assert_clap_surface(specrows: dict, scopes: list[str]) -> None:
    rows = specrows.get("rows")
    if not isinstance(rows, list):
        raise SystemExit("x07 --cli-specrows: missing rows array")

    required_long_opts = [
        "--json",
        "--jsonl",
        "--json-schema",
        "--json-schema-id",
        "--out",
        "--report-out",
        "--quiet-json",
    ]

    for scope in ["root", *scopes]:
        for opt in required_long_opts:
            if not has_long_opt(rows, scope, opt):
                raise SystemExit(f"{scope}: missing required machine flag in --cli-specrows: {opt}")


def list_expected_scopes(x07_bin: str, specrows: dict) -> list[Scope]:
    scopes = discover_scopes(specrows)

    out: list[Scope] = []

    root_schema = load_schema(TOOL_ROOT_SCHEMA_PATH)
    root_schema_version = schema_version_const(
        root_schema, str(TOOL_ROOT_SCHEMA_PATH.relative_to(REPO_ROOT))
    )
    out.append(
        Scope(
            name="root",
            argv=[],
            schema_path=TOOL_ROOT_SCHEMA_PATH,
            schema_version=root_schema_version,
        )
    )

    for scope in scopes:
        if scope == "doc":
            schema_path = DOC_SCHEMA_PATH
        else:
            schema_path = tool_wrapper_schema_path_for_scope(scope)
        schema = load_schema(schema_path)
        schema_version = schema_version_const(schema, str(schema_path.relative_to(REPO_ROOT)))
        out.append(
            Scope(
                name=scope,
                argv=scope.split("."),
                schema_path=schema_path,
                schema_version=schema_version,
            )
        )

    return out


def assert_stderr_empty(proc: subprocess.CompletedProcess[bytes], context: str) -> None:
    if proc.stderr.strip():
        raise SystemExit(f"{context}: unexpected stderr:\n{decode_utf8(proc.stderr, context)}")


def assert_bytes_equal(actual: bytes, expected: bytes, context: str) -> None:
    if actual != expected:
        raise SystemExit(f"{context}: output bytes do not match expected schema file")


def assert_schema_output_matches(x07_bin: str, scope: Scope) -> None:
    proc = run_cmd([x07_bin, *scope.argv, "--json-schema"], REPO_ROOT)
    if proc.returncode != 0:
        raise SystemExit(
            f"{scope.name}: --json-schema failed with exit={proc.returncode}\n"
            f"{decode_utf8(proc.stderr, scope.name)}"
        )
    assert_stderr_empty(proc, f"{scope.name} --json-schema")
    assert_bytes_equal(
        proc.stdout,
        scope.schema_path.read_bytes(),
        f"{scope.name} --json-schema",
    )


def assert_schema_id_matches(x07_bin: str, scope: Scope) -> None:
    proc = run_cmd([x07_bin, *scope.argv, "--json-schema-id"], REPO_ROOT)
    if proc.returncode != 0:
        raise SystemExit(
            f"{scope.name}: --json-schema-id failed with exit={proc.returncode}\n"
            f"{decode_utf8(proc.stderr, scope.name)}"
        )
    assert_stderr_empty(proc, f"{scope.name} --json-schema-id")
    expected = (scope.schema_version + "\n").encode("utf-8")
    if proc.stdout != expected:
        raise SystemExit(
            f"{scope.name}: --json-schema-id mismatch: "
            f"expected {scope.schema_version!r}, got {decode_utf8(proc.stdout, scope.name).strip()!r}"
        )


def assert_json_validates(registry: Registry, schema: dict, instance: dict, context: str) -> None:
    validator = Draft202012Validator(schema, registry=registry)
    errors = list(validator.iter_errors(instance))
    if errors:
        msg = errors[0]
        raise SystemExit(f"{context}: JSON does not validate: {msg.message}")


def assert_json_output_validates(
    x07_bin: str, registry: Registry, scope: Scope, argv: list[str]
) -> None:
    proc = run_cmd([x07_bin, *argv], REPO_ROOT)
    assert_stderr_empty(proc, " ".join(argv))
    doc = parse_json_doc_bytes(proc.stdout, " ".join(argv))
    schema = load_schema(scope.schema_path)
    assert_json_validates(registry, schema, doc, " ".join(argv))


def assert_jsonl_events_valid(x07_bin: str, registry: Registry) -> None:
    schema = load_schema(TOOL_EVENTS_SCHEMA_PATH)
    proc = run_cmd([x07_bin, "guide", "--jsonl", "--help"], REPO_ROOT)
    if proc.returncode != 0:
        raise SystemExit(f"guide --jsonl failed: {decode_utf8(proc.stderr, 'guide --jsonl')}")
    assert_stderr_empty(proc, "guide --jsonl")
    lines = [ln for ln in decode_utf8(proc.stdout, "guide --jsonl").splitlines() if ln.strip()]
    if not lines:
        raise SystemExit("guide --jsonl emitted no lines")
    for idx, line in enumerate(lines, start=1):
        try:
            ev = json.loads(line)
        except Exception as exc:  # noqa: BLE001
            raise SystemExit(f"guide --jsonl line {idx} is not valid JSON: {exc}") from exc
        if not isinstance(ev, dict):
            raise SystemExit(f"guide --jsonl line {idx}: expected JSON object")
        assert_json_validates(
            registry, schema, ev, f"guide --jsonl line {idx} (event={ev.get('event')!r})"
        )


def main() -> int:
    x07_bin = resolve_x07_bin()
    specrows = load_cli_specrows(x07_bin)
    scopes = discover_scopes(specrows)
    assert_clap_surface(specrows, scopes)

    # Ensure generated per-scope schemas exist and there are no stale leftovers.
    expected_scopes = list_expected_scopes(x07_bin, specrows)

    expected_tool_schema_files: set[Path] = set()
    expected_tool_schema_files.add(TOOL_ROOT_SCHEMA_PATH)
    for scope in expected_scopes:
        if scope.name not in {"root", "doc"}:
            expected_tool_schema_files.add(scope.schema_path)

    for path in expected_tool_schema_files:
        if not path.is_file():
            raise SystemExit(f"missing schema file: {path.relative_to(REPO_ROOT)}")

    stale = [
        p
        for p in sorted(SPEC_DIR.glob("x07-tool-*.report.schema.json"))
        if p not in expected_tool_schema_files
    ]
    if stale:
        msg = "\n".join(str(p.relative_to(REPO_ROOT)) for p in stale)
        raise SystemExit(f"stale tool wrapper schema files present:\n{msg}")

    registry = build_registry()

    # Validate schema discovery and IDs for every scope.
    for scope in expected_scopes:
        assert_schema_output_matches(x07_bin, scope)
        assert_schema_id_matches(x07_bin, scope)

    # Validate machine output shape for wrapper scopes (root + all scopes except doc).
    for scope in expected_scopes:
        if scope.name == "doc":
            continue
        assert_json_output_validates(
            x07_bin,
            registry,
            scope,
            [*scope.argv, "--json", "--help"],
        )

    # Validate native doc report and schema.
    doc_scope = next((s for s in expected_scopes if s.name == "doc"), None)
    if doc_scope is None:
        raise SystemExit("missing doc scope in --cli-specrows discovery")
    assert_json_output_validates(
        x07_bin,
        registry,
        doc_scope,
        ["doc", "--json", "bytes.view"],
    )

    # Validate --report-out/--quiet-json.
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
                "--help",
            ],
            REPO_ROOT,
        )
        if proc.returncode != 0:
            raise SystemExit(
                "guide --json --report-out --quiet-json failed: "
                f"{decode_utf8(proc.stderr, 'guide --quiet-json')}"
            )
        assert_stderr_empty(proc, "guide --quiet-json")
        if proc.stdout.strip():
            raise SystemExit("guide --quiet-json should not emit stdout")
        if not report_path.is_file():
            raise SystemExit("guide --quiet-json did not write --report-out")
        report_doc = parse_json_doc_bytes(report_path.read_bytes(), "guide --report-out")
        guide_scope = next((s for s in expected_scopes if s.name == "guide"), None)
        if guide_scope is None:
            raise SystemExit("missing guide scope in --cli-specrows discovery")
        assert_json_validates(
            registry, load_schema(guide_scope.schema_path), report_doc, "guide --report-out"
        )

    # Validate --out for a command with a primary stdout payload.
    with tempfile.TemporaryDirectory(prefix="x07_tool_out_") as tmpdir:
        guide_out = Path(tmpdir) / "guide.md"
        guide_scope = next((s for s in expected_scopes if s.name == "guide"), None)
        if guide_scope is None:
            raise SystemExit("missing guide scope in --cli-specrows discovery")
        assert_json_output_validates(
            x07_bin,
            registry,
            guide_scope,
            ["guide", "--json", "--out", str(guide_out)],
        )
        if not guide_out.is_file() or guide_out.stat().st_size == 0:
            raise SystemExit("guide --out did not write output file")

    # Validate JSONL streaming mode.
    proc = run_cmd([x07_bin, "--jsonl", "--json-schema"], REPO_ROOT)
    if proc.returncode != 0:
        raise SystemExit(f"--jsonl --json-schema failed: {decode_utf8(proc.stderr, 'jsonl')}")
    assert_stderr_empty(proc, "--jsonl --json-schema")
    assert_bytes_equal(proc.stdout, TOOL_EVENTS_SCHEMA_PATH.read_bytes(), "--jsonl --json-schema")
    assert_jsonl_events_valid(x07_bin, registry)

    # Validate base schema files are syntactically valid JSON.
    for schema_path in (
        SPEC_DIR / "x07-tool.report.schema.json",
        SPEC_DIR / "x07.patchset.schema.json",
        TOOL_EVENTS_SCHEMA_PATH,
    ):
        if not schema_path.is_file():
            raise SystemExit(f"missing schema file: {schema_path.relative_to(REPO_ROOT)}")
        load_schema(schema_path)

    print("ok: tool JSON contracts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
