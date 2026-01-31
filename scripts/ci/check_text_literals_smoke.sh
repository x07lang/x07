#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

./scripts/ci/check_tools.sh >/dev/null

X07_BIN="${X07_BIN:-$(./scripts/ci/find_x07.sh)}"

./scripts/ci/ensure_runners.sh

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
{
  "schema_version": "x07.x07ast@0.3.0",
  "kind": "entry",
  "module_id": "main",
  "imports": [],
  "decls": [],
  "solve": ["bytes.lit", "hello world\nline two"]
}
JSON

"$X07_BIN" run --project "$tmp/x07.json" --report wrapped --report-out "$tmp/run_report.json"

python3 - "$tmp/run_report.json" <<'PY'
import base64, json, sys

r = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert r.get("schema_version") == "x07.run.report@0.1.0", r.get("schema_version")
assert r.get("runner") == "host", r.get("runner")
rep = r.get("report") or {}
assert rep.get("schema_version") == "x07-host-runner.report@0.2.0", rep.get("schema_version")
assert rep.get("exit_code") == 0, rep.get("exit_code")
compile = rep.get("compile") or {}
assert compile.get("ok") is True, compile
assert compile.get("exit_status") == 0, compile.get("exit_status")
solve = rep.get("solve") or {}
assert solve.get("ok") is True, solve
assert solve.get("exit_status") == 0, solve.get("exit_status")
out = base64.b64decode(solve.get("solve_output_b64") or "")
assert out == b"hello world\nline two", out
print("ok: bytes.lit whitespace literal")
PY

echo "ok: check_text_literals_smoke"
