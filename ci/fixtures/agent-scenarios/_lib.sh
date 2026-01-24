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
  rm -rf "$work/tmp" "$work/target" || true
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

diff_snapshot() {
  local expected_dir="$1"
  local work="$2"
  diff -ru "$expected_dir" "$work"
}
