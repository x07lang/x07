from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, NoReturn

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")


def die(msg: str, code: int = 2) -> NoReturn:
    print(msg, file=sys.stderr)
    raise SystemExit(code)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def stable_canon(obj: Any) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def parse_x07import_meta(module_path: Path) -> tuple[str, str | None] | None:
    try:
        doc = json.loads(module_path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None

    if not isinstance(doc, dict):
        return None
    meta = doc.get("meta")
    if not isinstance(meta, dict):
        return None
    if meta.get("generated_by") != "x07import":
        return None

    src_path = meta.get("source_path")
    if not isinstance(src_path, str) or not src_path.strip():
        return None
    sha = meta.get("source_sha256")
    if isinstance(sha, str) and sha.strip():
        return src_path.strip(), sha.strip()
    return src_path.strip(), None

