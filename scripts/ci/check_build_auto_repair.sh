#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

./scripts/ci/check_tools.sh >/dev/null

X07_BIN="${X07_BIN:-$(./scripts/ci/find_x07.sh)}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/src"

cat >"$tmp/x07.json" <<'JSON'
{
  "schema_version": "x07.project@0.2.0",
  "world": "solve-pure",
  "entry": "src/main.x07.json",
  "module_roots": ["src"]
}
JSON

cat >"$tmp/x07.lock.json" <<'JSON'
{"schema_version":"x07.lock@0.2.0","dependencies":[]}
JSON

cat >"$tmp/src/main.x07.json" <<'JSON'
{"schema_version":"x07.x07ast@0.3.0","module_id":"main","kind":"entry","imports":[],"decls":[],"solve":["begin",["for","i",0,1,["let","x",0],["set","x",1],0],["bytes.alloc",0]]}
JSON

# Sanity: lint must fail pre-repair.
set +e
"$X07_BIN" lint --world solve-pure --input "$tmp/src/main.x07.json" >"$tmp/lint_before.json" 2>"$tmp/lint_before.err"
rc=$?
set -e
if [[ $rc -eq 0 ]]; then
  echo "ERROR: expected lint to fail before auto-repair" >&2
  exit 1
fi

# Expect: x07 build performs the canonical repair loop automatically and succeeds.
out_c="$tmp/out.c"
"$X07_BIN" build --project "$tmp/x07.json" --out "$out_c"

test -s "$out_c"

# Postconditions: the file must now be canonical + lint clean.
"$X07_BIN" fmt --check --input "$tmp/src/main.x07.json" >/dev/null
"$X07_BIN" lint --world solve-pure --input "$tmp/src/main.x07.json" >/dev/null

echo "ok: check_build_auto_repair"
