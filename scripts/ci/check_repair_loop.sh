#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi

tmp_dir="$(mktemp -t x07_repair_loop_XXXXXX -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

program="$tmp_dir/program.x07.json"
lint_report="$tmp_dir/lint.json"
patch="$tmp_dir/patch.json"

cat >"$program" <<'JSON'
{
  "schema_version": "x07.x07ast@0.3.0",
  "kind": "entry",
  "module_id": "main",
  "imports": ["std.os.proc"],
  "decls": [],
  "solve": ["bytes.alloc", 0]
}
JSON

set +e
lint_out="$("$x07_bin" lint --input "$program" --world solve-pure --json)"
lint_code="$?"
set -e

if [[ "$lint_code" -ne 1 ]]; then
  echo "ERROR: expected x07 lint to fail with exit code 1; got $lint_code" >&2
  echo "$lint_out" >&2
  exit 1
fi

printf '%s' "$lint_out" >"$lint_report"

"$python_bin" - "$lint_report" "$patch" <<'PY'
import json
import sys
from pathlib import Path

lint_path = Path(sys.argv[1])
patch_path = Path(sys.argv[2])

doc = json.loads(lint_path.read_text(encoding="utf-8"))
diagnostics = doc.get("diagnostics")
if not isinstance(diagnostics, list):
    raise SystemExit("ERROR: lint report missing diagnostics[]")

want = "X07-WORLD-OS-0001"
for d in diagnostics:
    if not isinstance(d, dict):
        continue
    if d.get("code") != want:
        continue
    q = d.get("quickfix")
    if not isinstance(q, dict):
        raise SystemExit(f"ERROR: {want} missing quickfix")
    if q.get("kind") != "json_patch":
        raise SystemExit(f"ERROR: {want} unexpected quickfix.kind: {q.get('kind')!r}")
    patch = q.get("patch")
    if not isinstance(patch, list) or not patch:
        raise SystemExit(f"ERROR: {want} missing quickfix.patch[]")
    patch_path.write_text(json.dumps(patch, indent=2) + "\n", encoding="utf-8")
    raise SystemExit(0)

raise SystemExit(f"ERROR: lint report did not include {want}")
PY

"$x07_bin" ast apply-patch --in "$program" --patch "$patch" --out "$program" --validate >/dev/null
"$x07_bin" lint --input "$program" --world solve-pure >/dev/null

echo "ok: repair loop"
