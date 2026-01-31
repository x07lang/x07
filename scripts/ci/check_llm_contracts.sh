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

"$python_bin" - <<'PY'
import json
from pathlib import Path

root = Path.cwd()

schemas = [
    root / "spec" / "x07ast.schema.json",
    root / "spec" / "x07diag.schema.json",
    root / "spec" / "x07patch.schema.json",
    root / "spec" / "x07test.schema.json",
    root / "spec" / "x07-run.report.schema.json",
    root / "spec" / "x07-project.schema.json",
    root / "spec" / "x07-lock.schema.json",
    root / "spec" / "x07-package.schema.json",
    root / "spec" / "x07-capabilities.schema.json",
    root / "spec" / "x07-website.package-index.schema.json",
]

for p in schemas:
    data = json.loads(p.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise SystemExit(f"schema must be a JSON object: {p}")

print("ok: schemas parse")
PY

tmp_dir="$(mktemp -t x07_contracts_XXXXXX -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

program="$tmp_dir/program.x07.json"
patch="$tmp_dir/patch.json"

cat >"$program" <<'JSON'
{
  "schema_version": "x07.x07ast@0.3.0",
  "kind": "entry",
  "module_id": "main",
  "imports": [],
  "decls": [],
  "solve": ["bytes.alloc", 0]
}
JSON

cat >"$patch" <<'JSON'
[
  {"op":"add","path":"/imports/-","value":"std.bytes"}
]
JSON

"$x07_bin" fmt --input "$program" --write >/dev/null
"$x07_bin" fmt --input "$program" --check >/dev/null
"$x07_bin" lint --input "$program" --world solve-pure >/dev/null
"$x07_bin" ast apply-patch --in "$program" --patch "$patch" --out "$program" --validate >/dev/null
"$x07_bin" fmt --input "$program" --check >/dev/null
"$x07_bin" lint --input "$program" --world solve-pure >/dev/null

echo "ok: llm contracts"
