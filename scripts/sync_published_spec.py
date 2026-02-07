#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INTERNAL_SPEC_ROOT = ROOT / "labs" / "internal-docs" / "spec"
DOCS_SPEC_ROOT = ROOT / "docs" / "spec"
DOCS_SPEC_INTERNAL = DOCS_SPEC_ROOT / "internal"
DOCS_SPEC_ABI = DOCS_SPEC_ROOT / "abi"
DOCS_SPEC_SCHEMAS = DOCS_SPEC_ROOT / "schemas"
SPEC_SCHEMA_ROOT = ROOT / "spec"

SPEC_INDEX_PATH = DOCS_SPEC_ROOT / "spec-index.json"

META_APPLIES_TO = "toolchain >= v0.0.95"
META_VERSION = "0.1.0"

HEADER_RE = re.compile(
    r"^Spec-ID: .+\n"
    r"Status: .+\n"
    r"Applies-to: .+\n"
    r"Related schemas: .+\n\n",
    re.MULTILINE,
)

CONTRACT_DOC_DIRS = [
    ROOT / "docs" / "net",
    ROOT / "docs" / "db",
    ROOT / "docs" / "fs",
    ROOT / "docs" / "time",
    ROOT / "docs" / "math",
    ROOT / "docs" / "text",
    ROOT / "docs" / "os",
]


@dataclass(frozen=True)
class PlannedWrite:
    path: Path
    content: bytes


def read_text_normalized(path: Path) -> str:
    return normalize_text(path.read_text(encoding="utf-8"))


def normalize_text(text: str) -> str:
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    if not text.endswith("\n"):
        text += "\n"
    return text


def strip_existing_header(text: str) -> str:
    m = HEADER_RE.match(text)
    if not m:
        return text
    return text[m.end() :]


def rewrite_internal_links(text: str) -> str:
    text = text.replace("labs/internal-docs/spec/abi/", "docs/spec/abi/")
    text = text.replace("labs/internal-docs/spec/types/", "docs/spec/internal/types/")
    text = text.replace("labs/internal-docs/spec/", "docs/spec/internal/")
    return text


def spec_id_for_internal(rel: Path) -> str:
    slug = ".".join(rel.with_suffix("").parts).replace("_", "-")
    return f"x07.spec.internal.{slug}@{META_VERSION}"


def spec_id_for_abi(rel: Path) -> str:
    slug = ".".join(rel.with_suffix("").parts).replace("_", "-")
    return f"x07.spec.abi.{slug}@{META_VERSION}"


def spec_id_for_contract(rel: Path) -> str:
    slug = ".".join(rel.with_suffix("").parts).replace("_", "-")
    return f"x07.spec.contract.{slug}@{META_VERSION}"


def build_header(spec_id: str, status: str, related_schemas: list[str]) -> str:
    return (
        f"Spec-ID: {spec_id}\n"
        f"Status: {status}\n"
        f"Applies-to: {META_APPLIES_TO}\n"
        f"Related schemas: {json.dumps(related_schemas, separators=(', ', ': '))}\n\n"
    )


def title_from_markdown(content: str) -> str:
    for line in content.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return ""


def schema_version_from_doc(doc: dict) -> str:
    props = doc.get("properties")
    if not isinstance(props, dict):
        return ""
    schema_version = props.get("schema_version")
    if isinstance(schema_version, dict):
        const = schema_version.get("const")
        if isinstance(const, str):
            return const
    return ""


def planned_internal_docs() -> list[PlannedWrite]:
    writes: list[PlannedWrite] = []
    for src in sorted(INTERNAL_SPEC_ROOT.rglob("*.md")):
        rel = src.relative_to(INTERNAL_SPEC_ROOT)
        if rel.parts and rel.parts[0] == "abi":
            continue
        if rel == Path("language-guide.md"):
            continue
        raw = read_text_normalized(src)
        body = strip_existing_header(raw)
        body = rewrite_internal_links(body)
        spec_id = spec_id_for_internal(rel)
        content = build_header(spec_id, "draft", []) + body
        dst = DOCS_SPEC_INTERNAL / rel
        writes.append(PlannedWrite(path=dst, content=content.encode("utf-8")))
    return writes


def planned_abi_docs() -> list[PlannedWrite]:
    writes: list[PlannedWrite] = []
    abi_root = INTERNAL_SPEC_ROOT / "abi"
    for src in sorted(abi_root.rglob("*.md")):
        rel = src.relative_to(abi_root)
        raw = read_text_normalized(src)
        body = strip_existing_header(raw)
        body = rewrite_internal_links(body)
        spec_id = spec_id_for_abi(rel)
        content = build_header(spec_id, "stable", []) + body
        dst = DOCS_SPEC_ABI / rel
        writes.append(PlannedWrite(path=dst, content=content.encode("utf-8")))
    return writes


def planned_schema_copies() -> list[PlannedWrite]:
    writes: list[PlannedWrite] = []
    for src in sorted(SPEC_SCHEMA_ROOT.glob("*.schema.json")):
        content = normalize_text(src.read_text(encoding="utf-8")).encode("utf-8")
        dst = DOCS_SPEC_SCHEMAS / src.name
        writes.append(PlannedWrite(path=dst, content=content))
    return writes


def collect_contract_entries() -> list[dict]:
    out: list[dict] = []
    for doc_dir in CONTRACT_DOC_DIRS:
        if not doc_dir.is_dir():
            continue
        for path in sorted(doc_dir.rglob("*.md")):
            rel = path.relative_to(ROOT / "docs")
            content = read_text_normalized(path)
            out.append(
                {
                    "kind": "contract",
                    "path": f"docs/{rel.as_posix()}",
                    "spec_id": spec_id_for_contract(rel),
                    "title": title_from_markdown(content),
                }
            )
    return out


def collect_markdown_entries() -> list[dict]:
    out: list[dict] = []
    for path in sorted(DOCS_SPEC_ROOT.rglob("*.md")):
        rel = path.relative_to(ROOT)
        content = read_text_normalized(path)
        spec_id = ""
        status = ""
        for line in content.splitlines():
            if line.startswith("Spec-ID: "):
                spec_id = line.split(": ", 1)[1].strip()
            elif line.startswith("Status: "):
                status = line.split(": ", 1)[1].strip()
            if spec_id and status:
                break
        out.append(
            {
                "kind": "markdown",
                "path": rel.as_posix(),
                "spec_id": spec_id,
                "status": status,
                "title": title_from_markdown(content),
            }
        )
    return out


def collect_schema_entries() -> list[dict]:
    out: list[dict] = []
    for path in sorted(DOCS_SPEC_SCHEMAS.glob("*.schema.json")):
        rel = path.relative_to(ROOT)
        doc = json.loads(path.read_text(encoding="utf-8"))
        schema_id = doc.get("$id", "")
        if not isinstance(schema_id, str):
            schema_id = ""
        out.append(
            {
                "kind": "schema",
                "path": rel.as_posix(),
                "schema_id": schema_id,
                "schema_version": schema_version_from_doc(doc),
                "title": str(doc.get("title", "")),
            }
        )
    return out


def build_spec_index() -> bytes:
    entries = collect_markdown_entries() + collect_schema_entries() + collect_contract_entries()
    entries.sort(key=lambda row: (str(row.get("kind", "")), str(row.get("path", ""))))
    doc = {"schema_version": "x07.spec.index@0.1.0", "entries": entries}
    return (json.dumps(doc, indent=2, sort_keys=True) + "\n").encode("utf-8")


def write_if_changed(path: Path, content: bytes, check: bool) -> bool:
    if path.exists():
        existing = path.read_bytes()
        if existing == content:
            return False
    if check:
        return True
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    return True


def remove_stale_files(root: Path, expected: set[Path], check: bool) -> list[Path]:
    removed: list[Path] = []
    if not root.is_dir():
        return removed
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        if path not in expected:
            removed.append(path)
    if check or not removed:
        return removed
    for path in removed:
        path.unlink()
    return removed


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser(description="Sync published spec docs and schemas.")
    ap.add_argument("--check", action="store_true", help="Fail if synced outputs would change.")
    return ap.parse_args()


def main() -> int:
    args = parse_args()

    if not INTERNAL_SPEC_ROOT.is_dir():
        print(f"ERROR: missing directory: {INTERNAL_SPEC_ROOT}", file=sys.stderr)
        return 2
    if not SPEC_SCHEMA_ROOT.is_dir():
        print(f"ERROR: missing directory: {SPEC_SCHEMA_ROOT}", file=sys.stderr)
        return 2

    planned = planned_internal_docs() + planned_abi_docs() + planned_schema_copies()
    expected_files = {pw.path for pw in planned}
    changed_paths: list[Path] = []
    for pw in planned:
        if write_if_changed(pw.path, pw.content, args.check):
            changed_paths.append(pw.path)

    stale_internal = remove_stale_files(DOCS_SPEC_INTERNAL, {p for p in expected_files if p.is_relative_to(DOCS_SPEC_INTERNAL)}, args.check)
    stale_abi = remove_stale_files(DOCS_SPEC_ABI, {p for p in expected_files if p.is_relative_to(DOCS_SPEC_ABI)}, args.check)
    stale_schemas = remove_stale_files(
        DOCS_SPEC_SCHEMAS,
        {p for p in expected_files if p.is_relative_to(DOCS_SPEC_SCHEMAS)},
        args.check,
    )

    if not args.check:
        DOCS_SPEC_ROOT.mkdir(parents=True, exist_ok=True)
    spec_index_bytes = build_spec_index()
    if write_if_changed(SPEC_INDEX_PATH, spec_index_bytes, args.check):
        changed_paths.append(SPEC_INDEX_PATH)

    stale_paths = stale_internal + stale_abi + stale_schemas
    if args.check and (changed_paths or stale_paths):
        print("ERROR: published spec docs are out of sync", file=sys.stderr)
        for path in changed_paths:
            print(f"  would update: {path.relative_to(ROOT).as_posix()}", file=sys.stderr)
        for path in stale_paths:
            print(f"  would remove: {path.relative_to(ROOT).as_posix()}", file=sys.stderr)
        print(
            "hint: python3 scripts/sync_published_spec.py",
            file=sys.stderr,
        )
        return 1

    if args.check:
        print("ok: published spec docs are in sync")
        return 0

    if stale_paths:
        for path in stale_paths:
            changed_paths.append(path)
    if changed_paths:
        changed_paths.sort()
        print(f"ok: synced {len(changed_paths)} files")
    else:
        print("ok: no changes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
