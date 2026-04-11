#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


RUST_STR_CONST_RE = re.compile(r'^\s*pub const (?P<name>[A-Z0-9_]+): &str = (?P<expr>.+);\s*$')
TOML_SECTION_RE = re.compile(r"^\s*\[(?P<name>[^\]]+)\]\s*$")
TOML_VERSION_RE = re.compile(r'^\s*version\s*=\s*"(?P<version>[^"]+)"\s*$')
LANG_ID_RE = re.compile(r'^\s*pub const LANG_ID: &str = "(?P<id>[^"]+)";\s*$')
COMPAT_CURRENT_RE = re.compile(
    r"^\s*pub const CURRENT_VERSION: CompatVersion = CompatVersion::new\(\s*(?P<major>\d+)\s*,\s*(?P<minor>\d+)\s*\);\s*$"
)


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def read_cargo_package_version(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    in_package = False
    for raw in text.splitlines():
        m = TOML_SECTION_RE.match(raw)
        if m:
            in_package = m.group("name").strip() == "package"
            continue
        if not in_package:
            continue
        m = TOML_VERSION_RE.match(raw)
        if m:
            return m.group("version")
    raise SystemExit(f"ERROR: failed to read [package].version from {path.relative_to(repo_root())}")


def load_rust_str_consts(path: Path) -> dict[str, str]:
    consts: dict[str, str] = {}
    for raw in path.read_text(encoding="utf-8").splitlines():
        m = RUST_STR_CONST_RE.match(raw)
        if not m:
            continue
        name = m.group("name")
        expr = m.group("expr").strip()
        consts[name] = expr
    if not consts:
        raise SystemExit(f"ERROR: no Rust &str consts found in {path.relative_to(repo_root())}")
    return consts


def read_lang_id(path: Path) -> str:
    for raw in path.read_text(encoding="utf-8").splitlines():
        m = LANG_ID_RE.match(raw)
        if m:
            return m.group("id")
    raise SystemExit(f"ERROR: failed to read LANG_ID from {path.relative_to(repo_root())}")


def read_compat_current(path: Path) -> str:
    for raw in path.read_text(encoding="utf-8").splitlines():
        m = COMPAT_CURRENT_RE.match(raw)
        if not m:
            continue
        major = int(m.group("major"))
        minor = int(m.group("minor"))
        return f"{major}.{minor}"
    raise SystemExit(f"ERROR: failed to read CURRENT_VERSION from {path.relative_to(repo_root())}")


def resolve_rust_str_const(name: str, consts: dict[str, str]) -> str:
    resolving: set[str] = set()
    cache: dict[str, str] = {}

    def resolve_inner(k: str) -> str:
        if k in cache:
            return cache[k]
        if k in resolving:
            chain = " -> ".join([*resolving, k])
            raise SystemExit(f"ERROR: cycle in Rust const resolution: {chain}")
        expr = consts.get(k)
        if expr is None:
            raise SystemExit(f"ERROR: missing const {k!r} in Rust const set")

        resolving.add(k)
        out: str
        expr = expr.strip()
        if expr.startswith('"') and expr.endswith('"'):
            # X07 version strings are plain literals without escapes; keep parsing minimal.
            out = expr[1:-1]
        elif re.fullmatch(r"[A-Z0-9_]+", expr):
            out = resolve_inner(expr)
        else:
            raise SystemExit(f"ERROR: unsupported Rust const expr for {k}: {expr!r}")
        resolving.remove(k)
        cache[k] = out
        return out

    return resolve_inner(name)


def build_versions_doc(root: Path) -> dict[str, object]:
    contracts_rs = root / "crates" / "x07-contracts" / "src" / "lib.rs"
    consts = load_rust_str_consts(contracts_rs)

    def c(name: str) -> str:
        return resolve_rust_str_const(name, consts)

    return {
        "toolchain": {
            "x07": read_cargo_package_version(root / "crates" / "x07" / "Cargo.toml"),
            "x07c": read_cargo_package_version(root / "crates" / "x07c" / "Cargo.toml"),
            "x07up": read_cargo_package_version(root / "crates" / "x07up" / "Cargo.toml"),
            "lang_id": read_lang_id(root / "crates" / "x07c" / "src" / "language.rs"),
            "compat_current": read_compat_current(root / "crates" / "x07c" / "src" / "compat.rs"),
        },
        "schemas": {
            "x07_project": c("PROJECT_MANIFEST_SCHEMA_VERSION"),
            "x07_lock": c("PROJECT_LOCKFILE_SCHEMA_VERSION"),
            "x07_x07ast": c("X07AST_SCHEMA_VERSION"),
            "x07_x07diag": c("X07DIAG_SCHEMA_VERSION"),
            "x07_run_report": c("X07_RUN_REPORT_SCHEMA_VERSION"),
            "x07_doc_report": c("X07_DOC_REPORT_SCHEMA_VERSION"),
            "x07_verify_report": c("X07_VERIFY_REPORT_SCHEMA_VERSION"),
            "x07_agent_context": c("X07_AGENT_CONTEXT_SCHEMA_VERSION"),
        },
        "pkg": {
            "default_index_url": c("X07_PKG_DEFAULT_INDEX_URL"),
        },
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--out",
        default="docs/_generated/versions.json",
        help="Output path (repo-relative).",
    )
    ap.add_argument("--check", action="store_true", help="Fail if output is missing or out of date.")
    ap.add_argument("--write", action="store_true", help="Write the generated file.")
    ap.add_argument("--print", action="store_true", help="Print JSON to stdout (for debugging).")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    root = repo_root()

    doc = build_versions_doc(root)
    out_bytes = (json.dumps(doc, sort_keys=True, indent=2) + "\n").encode("utf-8")
    if args.print:
        sys.stdout.write(out_bytes.decode("utf-8"))

    out_path = (root / args.out).resolve()
    if args.check and args.write:
        raise SystemExit("ERROR: set at most one of --check or --write")
    if not args.check and not args.write and not args.print:
        raise SystemExit("ERROR: set one of --check, --write, or --print")

    if args.check:
        if not out_path.is_file():
            print(f"ERROR: missing {out_path.relative_to(root)} (run --write)", file=sys.stderr)
            return 1
        existing = out_path.read_bytes()
        if existing != out_bytes:
            print(f"ERROR: {out_path.relative_to(root)} is out of date (run --write)", file=sys.stderr)
            return 1
        print(f"ok: {out_path.relative_to(root)}")
        return 0

    if args.write:
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_bytes(out_bytes)
        print(f"ok: wrote {out_path.relative_to(root)}")
        return 0

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
