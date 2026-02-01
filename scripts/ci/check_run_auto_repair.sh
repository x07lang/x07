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

python3 - "$tmp/lint_before.json" <<'PY'
import json, sys

doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert doc.get("schema_version") == "x07.x07diag@0.1.0", doc.get("schema_version")
assert doc.get("ok") is False, doc
codes = [d.get("code") for d in (doc.get("diagnostics") or []) if isinstance(d, dict)]
assert "X07-ARITY-FOR-0001" in codes, codes
PY

# Expect: x07 run performs the canonical repair loop automatically and succeeds.
"$X07_BIN" run --project "$tmp/x07.json" --report wrapped --report-out "$tmp/run_report.json"

# Postconditions: the file must now be canonical + lint clean.
"$X07_BIN" fmt --check --input "$tmp/src/main.x07.json" >/dev/null
"$X07_BIN" lint --world solve-pure --input "$tmp/src/main.x07.json" >/dev/null

python3 - "$tmp/run_report.json" "$tmp/src/main.x07.json" <<'PY'
import base64, json, sys

run_report_path, prog_path = sys.argv[1], sys.argv[2]

run_report = json.load(open(run_report_path, "r", encoding="utf-8"))
assert run_report.get("schema_version") == "x07.run.report@0.1.0", run_report.get("schema_version")
assert run_report.get("runner") == "host", run_report.get("runner")
assert run_report.get("world") == "solve-pure", run_report.get("world")

repair = run_report.get("repair")
assert isinstance(repair, dict), repair
assert repair.get("mode") in ("memory", "write"), repair
assert repair.get("last_lint_ok") is True, repair

runner = run_report.get("report")
assert isinstance(runner, dict), type(runner)
assert runner.get("schema_version") == "x07-host-runner.report@0.3.0", runner.get("schema_version")
assert runner.get("exit_code") == 0, runner.get("exit_code")

compile = runner.get("compile") or {}
assert compile.get("ok") is True, compile
assert compile.get("exit_status") == 0, compile.get("exit_status")

solve = runner.get("solve") or {}
assert solve.get("ok") is True, solve
assert solve.get("exit_status") == 0, solve.get("exit_status")
out = base64.b64decode(solve.get("solve_output_b64") or "")
assert out == b"", out

prog = json.load(open(prog_path, "r", encoding="utf-8"))
solve_expr = (prog.get("solve") or [])
assert isinstance(solve_expr, list) and solve_expr and solve_expr[0] == "begin", solve_expr
for_expr = solve_expr[1]
assert isinstance(for_expr, list) and for_expr and for_expr[0] == "for", for_expr
body = for_expr[4]
assert isinstance(body, list) and body and body[0] == "begin", body

print("ok: run auto-repair")
PY

echo "ok: check_run_auto_repair"
