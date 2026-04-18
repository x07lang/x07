#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any, Iterable


X07AST_SCHEMA_RE = re.compile(r'("schema_version"\s*:\s*")(?P<sv>x07\.x07ast@[^"]+)(")')


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def load_versions(root: Path) -> dict[str, str]:
    versions_path = root / "docs" / "_generated" / "versions.json"
    if not versions_path.is_file():
        raise SystemExit(
            f"ERROR: missing {versions_path.relative_to(root)} (run: python3 scripts/gen_versions_json.py --write)"
        )
    doc = load_json(versions_path)
    if not isinstance(doc, dict):
        raise SystemExit(f"ERROR: {versions_path.relative_to(root)}: expected JSON object")

    schemas = doc.get("schemas")
    if not isinstance(schemas, dict):
        raise SystemExit(f"ERROR: {versions_path.relative_to(root)}: invalid shape (schemas)")

    v = schemas.get("x07_x07ast")
    if not isinstance(v, str) or not v.strip():
        raise SystemExit(f"ERROR: {versions_path.relative_to(root)}: missing schemas.x07_x07ast")
    return {"x07ast_schema": v.strip()}


def is_hidden_rel(rel: Path) -> bool:
    return any(part.startswith(".") for part in rel.parts)

def is_generated_rel(rel: Path) -> bool:
    # docs/examples contains runnable projects. Tool outputs like `target/` and `dist/`
    # are intentionally ignored by docs checks (they are gitignored and can be huge).
    return any(part in {"target", "dist", "artifacts", "node_modules"} for part in rel.parts)


def iter_x07ast_files(examples_root: Path) -> Iterable[Path]:
    for path in sorted(examples_root.rglob("*.x07.json")):
        rel = path.relative_to(examples_root)
        if is_hidden_rel(rel):
            continue
        if is_generated_rel(rel):
            continue
        if not path.is_file():
            continue
        yield path


def rewrite_schema_version(text: str, *, want: str) -> tuple[str, str | None]:
    m = X07AST_SCHEMA_RE.search(text)
    if not m:
        return text, None
    have = m.group("sv")
    if have == want:
        return text, have
    def repl(m2: re.Match[str]) -> str:
        return f"{m2.group(1)}{want}{m2.group(3)}"

    out = X07AST_SCHEMA_RE.sub(repl, text, count=1)
    return out, have


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--root",
        default="docs/examples",
        help="Directory containing docs examples (repo-relative).",
    )
    ap.add_argument("--check", action="store_true", help="Fail if any docs x07AST file needs upgrade.")
    ap.add_argument("--write", action="store_true", help="Upgrade docs x07AST schema_version in place.")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    if args.check == args.write:
        raise SystemExit("ERROR: set exactly one of --check or --write")

    root = repo_root()
    versions = load_versions(root)
    want = versions["x07ast_schema"]
    examples_root = (root / args.root).resolve()
    if not examples_root.is_dir():
        raise SystemExit(f"ERROR: missing docs examples dir: {examples_root.relative_to(root)}")

    files = list(iter_x07ast_files(examples_root))
    if not files:
        print("ok: no docs x07AST files")
        return 0

    changed: list[Path] = []
    for path in files:
        rel = path.relative_to(root)
        text = path.read_text(encoding="utf-8")
        try:
            doc = json.loads(text)
        except Exception as ex:
            raise SystemExit(f"ERROR: {rel}: invalid JSON: {ex}") from ex
        if not isinstance(doc, dict):
            raise SystemExit(f"ERROR: {rel}: expected JSON object")
        sv = doc.get("schema_version")
        if not isinstance(sv, str) or not sv.strip():
            raise SystemExit(f"ERROR: {rel}: missing schema_version")
        if not sv.startswith("x07.x07ast@"):
            raise SystemExit(f"ERROR: {rel}: unexpected schema_version: {sv!r}")

        out, matched = rewrite_schema_version(text, want=want)
        if matched is None:
            raise SystemExit(f"ERROR: {rel}: could not locate schema_version token to rewrite")
        if out != text:
            try:
                rewritten = json.loads(out)
            except Exception as ex:
                raise SystemExit(f"ERROR: {rel}: rewrite produced invalid JSON: {ex}") from ex
            if not isinstance(rewritten, dict) or rewritten.get("schema_version") != want:
                raise SystemExit(f"ERROR: {rel}: rewrite produced unexpected schema_version")
            changed.append(path)
            if args.write:
                path.write_text(out, encoding="utf-8")

    if args.check:
        if changed:
            for p in changed:
                print(f"ERROR: {p.relative_to(root)} is out of date (run --write)", file=sys.stderr)
            return 1
        print("ok: docs x07AST schema_version is current")
        return 0

    if changed:
        print(f"ok: upgraded {len(changed)} docs x07AST files to {want}")
    else:
        print(f"ok: docs x07AST files already on {want}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
