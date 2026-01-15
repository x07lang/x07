#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

need() {
  command -v "$1" >/dev/null 2>&1 || { echo "ERROR: missing tool: $1" >&2; exit 1; }
}

need rsync
need find
need touch
need chmod
need python3

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <fixture_id> <source_dir>" >&2
  echo "Example: $0 my_fixture ./path/to/source_dir" >&2
  exit 2
fi

FIXTURE_ID="$1"
SRC_DIR="$2"

if [[ ! -d "$SRC_DIR" ]]; then
  echo "ERROR: source_dir does not exist or is not a directory: $SRC_DIR" >&2
  exit 2
fi

SRC_ABS="$(cd "$SRC_DIR" && pwd)"
DEST_DIR="$ROOT/benchmarks/fixtures/fs/$FIXTURE_ID"

if [[ -e "$DEST_DIR" ]]; then
  chmod -R u+w "$DEST_DIR" >/dev/null 2>&1 || true
fi
rm -rf "$DEST_DIR"
mkdir -p "$DEST_DIR"

rsync -a --delete \
  --exclude '.git/' \
  --exclude 'target/' \
  --exclude '__pycache__/' \
  --exclude '.DS_Store' \
  "$SRC_ABS"/ "$DEST_DIR"/

# Normalize mtimes to a fixed timestamp in UTC.
# 2000-01-01 00:00:00 UTC
( cd "$DEST_DIR" && TZ=UTC find . -exec touch -h -t 200001010000 {} + )

python3 - "$DEST_DIR" <<'PY'
from __future__ import annotations

import hashlib
import os
import pathlib
import sys

root = pathlib.Path(sys.argv[1])

def sha256_file(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()

files: list[pathlib.Path] = []
for dirpath, _, filenames in os.walk(root):
    for name in filenames:
        p = pathlib.Path(dirpath) / name
        rel = p.relative_to(root)
        if rel.as_posix() in {"MANIFEST.sha256", "SNAPSHOT.sha256"}:
            continue
        files.append(p)

files.sort(key=lambda p: p.relative_to(root).as_posix())

manifest_lines: list[str] = []
for p in files:
    rel = p.relative_to(root).as_posix()
    manifest_lines.append(f"{sha256_file(p)}  {rel}\n")

manifest_path = root / "MANIFEST.sha256"
manifest_path.write_text("".join(manifest_lines), encoding="utf-8")

snapshot_id = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
(root / "SNAPSHOT.sha256").write_text(snapshot_id + "\n", encoding="utf-8")
PY

( cd "$DEST_DIR" && TZ=UTC touch -h -t 200001010000 MANIFEST.sha256 SNAPSHOT.sha256 )
chmod -R a-w "$DEST_DIR"

echo "OK: snapshot created at $DEST_DIR"
echo "    snapshot id: $(cat "$DEST_DIR/SNAPSHOT.sha256")"
