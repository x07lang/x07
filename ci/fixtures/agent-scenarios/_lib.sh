#!/usr/bin/env bash
set -euo pipefail

die() {
  echo "ERROR: $*" >&2
  exit 1
}

require_env() {
  local name="$1"
  local v="${!name:-}"
  [[ -n "$v" ]] || die "missing required env var: $name"
}

require_path() {
  local p="$1"
  [[ -e "$p" ]] || die "missing required path: $p"
}

copy_tree() {
  local src="$1"
  local dst="$2"
  mkdir -p "$dst"
  cp -a "$src/." "$dst/"
}

rm_ephemeral() {
  local work="$1"
  rm -rf "$work/tmp" "$work/target" "$work/.x07" || true
}

fmt_check_all() {
  local work="$1"
  (cd "$work" && find src tests -name '*.x07.json' -print0 | while IFS= read -r -d '' f; do
    "$X07_BIN" fmt --input "$f" --check >/dev/null
  done)
}

fmt_write_all() {
  local work="$1"
  (cd "$work" && find src tests -name '*.x07.json' -print0 | while IFS= read -r -d '' f; do
    "$X07_BIN" fmt --input "$f" --write >/dev/null
  done)
}

lint_one() {
  local work="$1"
  local world="$2"
  local file_rel="$3"
  (cd "$work" && "$X07_BIN" lint --input "$file_rel" --world "$world")
}

fix_one() {
  local work="$1"
  local world="$2"
  local file_rel="$3"
  (cd "$work" && "$X07_BIN" fix --input "$file_rel" --world "$world" --write >/dev/null)
}

unwrap_wrapped_report() {
  local wrapped_path="$1"
  local runner_out="$2"
  "$X07_PYTHON" - "$wrapped_path" "$runner_out" <<'PY'
import json, sys
from pathlib import Path

wrapped = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
sv = wrapped.get("schema_version")
if sv != "x07.run.report@0.1.0":
    raise SystemExit(f"wrapped schema_version mismatch: {sv!r}")
report = wrapped.get("report")
if not isinstance(report, dict):
    raise SystemExit("wrapped.report must be an object")
Path(sys.argv[2]).write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
PY
}

run_wrapped() {
  local name="$1"
  local work="$2"
  shift 2

  mkdir -p "$work/tmp"
  local wrapped="$work/tmp/run.wrapped.json"
  local stdout_log="$work/tmp/run.stdout"
  local stderr_log="$work/tmp/run.stderr"

  set +e
  (cd "$work" && "$X07_BIN" run --project x07.json --report wrapped --report-out "$wrapped" "$@" >"$stdout_log" 2>"$stderr_log")
  local code="$?"
  set -e

  if [[ "$code" -ne 0 ]]; then
    echo "ERROR: $name: x07 run failed (exit $code)" >&2
    echo "--- stderr ($stderr_log) ---" >&2
    cat "$stderr_log" >&2 || true
    echo "--- stdout ($stdout_log) ---" >&2
    cat "$stdout_log" >&2 || true
    if [[ -s "$wrapped" ]]; then
      echo "--- wrapped report ($wrapped) ---" >&2
      cat "$wrapped" >&2 || true
    fi
    exit 1
  fi

  echo "$wrapped"
}

run_wrapped_allow_failure() {
  local name="$1"
  local work="$2"
  shift 2

  mkdir -p "$work/tmp"
  local wrapped="$work/tmp/run.wrapped.json"
  local stdout_log="$work/tmp/run.stdout"
  local stderr_log="$work/tmp/run.stderr"

  set +e
  (cd "$work" && "$X07_BIN" run --project x07.json --report wrapped --report-out "$wrapped" "$@" >"$stdout_log" 2>"$stderr_log")
  local code="$?"
  set -e

  echo "$wrapped"
  return "$code"
}

assert_wrapped_compile_error_contains() {
  local name="$1"
  local wrapped_path="$2"
  local needle="$3"
  "$X07_PYTHON" - "$name" "$wrapped_path" "$needle" <<'PY'
import json
import sys
from pathlib import Path

name = sys.argv[1]
wrapped_path = Path(sys.argv[2])
needle = sys.argv[3]

doc = json.loads(wrapped_path.read_text(encoding="utf-8"))
report = doc.get("report") or {}
compile_doc = report.get("compile") or {}
msg = compile_doc.get("compile_error") or ""
if not isinstance(msg, str):
    msg = str(msg)

if needle in msg:
    raise SystemExit(0)

raise SystemExit(f"{name}: compile_error missing {needle!r} (got {msg!r})")
PY
}

normalize_wrapped_report_to_golden() {
  local wrapped_path="$1"
  local out_path="$2"
  "$X07_PYTHON" - "$wrapped_path" "$out_path" <<'PY'
import json
import sys
from pathlib import Path

wrapped_path = Path(sys.argv[1])
out_path = Path(sys.argv[2])

doc = json.loads(wrapped_path.read_text(encoding="utf-8"))
runner = doc.get("runner")
world = doc.get("world")
report = doc.get("report") or {}

compile_doc = report.get("compile") if isinstance(report, dict) else None
solve_doc = report.get("solve") if isinstance(report, dict) else None

out = {
    "schema_version": "x07.run.golden@0.1.0",
    "runner": runner,
    "world": world,
    "mode": report.get("mode") if isinstance(report, dict) else None,
    "exit_code": report.get("exit_code") if isinstance(report, dict) else None,
    "compile_error": (compile_doc or {}).get("compile_error") if isinstance(compile_doc, dict) else None,
    "solve_output_b64": (solve_doc or {}).get("solve_output_b64") if isinstance(solve_doc, dict) else None,
}

out_path.write_text(json.dumps(out, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
}

normalize_x07test_report_to_golden() {
  local report_path="$1"
  local out_path="$2"
  "$X07_PYTHON" - "$report_path" "$out_path" <<'PY'
import json
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
out_path = Path(sys.argv[2])

doc = json.loads(report_path.read_text(encoding="utf-8"))
raw_summary = doc.get("summary") if isinstance(doc, dict) else None
tests = doc.get("tests") if isinstance(doc, dict) else None

summary = {}
if isinstance(raw_summary, dict):
    for k in (
        "passed",
        "failed",
        "skipped",
        "errors",
        "xfail_passed",
        "xfail_failed",
        "compile_failures",
        "run_failures",
    ):
        if k in raw_summary:
            summary[k] = raw_summary.get(k)

out = {
    "schema_version": "x07.test.golden@0.1.0",
    "summary": summary,
    "tests": [],
}
for t in tests or []:
    if not isinstance(t, dict):
        continue
    out["tests"].append(
        {
            "id": t.get("id"),
            "world": t.get("world"),
            "expect": t.get("expect"),
            "status": t.get("status"),
        }
    )

out_path.write_text(json.dumps(out, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
}

assert_json_golden_eq() {
  local name="$1"
  local got_path="$2"
  local golden_path="$3"
  "$X07_PYTHON" - "$name" "$got_path" "$golden_path" <<'PY'
import json
import sys
from pathlib import Path

name = sys.argv[1]
got_path = Path(sys.argv[2])
golden_path = Path(sys.argv[3])

got = json.loads(got_path.read_text(encoding="utf-8"))
golden = json.loads(golden_path.read_text(encoding="utf-8"))

got_s = json.dumps(got, indent=2, sort_keys=True) + "\n"
golden_s = json.dumps(golden, indent=2, sort_keys=True) + "\n"
if got_s == golden_s:
    raise SystemExit(0)

sys.stderr.write(f"ERROR: {name}: golden mismatch\\n")
sys.stderr.write(f"got: {got_path}\\n")
sys.stderr.write(f"golden: {golden_path}\\n")
raise SystemExit(1)
PY
}

assert_solve_output() {
  local name="$1"
  local runner_json="$2"
  local expected_ascii="$3"

  "$X07_PYTHON" "$X07_REPO_ROOT/scripts/ci/assert_run_os_ok.py" "$name" \
    --path "$runner_json" \
    --expect "$expected_ascii" \
    >/dev/null
}

run_tests() {
  local work="$1"
  mkdir -p "$work/tmp"
  local report="$work/tmp/x07test.report.json"
  set +e
  (cd "$work" && "$X07_BIN" test --manifest tests/tests.json --artifact-dir tmp/x07test >"$report")
  local code="$?"
  set -e
  if [[ "$code" -ne 0 ]]; then
    echo "ERROR: x07 test failed (exit $code)" >&2
    if [[ -s "$report" ]]; then
      echo "--- x07test report ($report) ---" >&2
      cat "$report" >&2 || true
    fi
    exit 1
  fi
}

run_tests_report() {
  local work="$1"
  mkdir -p "$work/tmp"
  local report="$work/tmp/x07test.report.json"
  set +e
  (cd "$work" && "$X07_BIN" test --manifest tests/tests.json --artifact-dir tmp/x07test >"$report")
  local code="$?"
  set -e
  if [[ "$code" -ne 0 ]]; then
    echo "ERROR: x07 test failed (exit $code)" >&2
    if [[ -s "$report" ]]; then
      echo "--- x07test report ($report) ---" >&2
      cat "$report" >&2 || true
    fi
    exit 1
  fi
  echo "$report"
}

diff_snapshot() {
  local expected_dir="$1"
  local work="$2"
  diff -ru "$expected_dir" "$work"
}
