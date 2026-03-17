#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd
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

solutions_dir="${X07_BENCH_SOLUTIONS:-labs/benchmarks/solutions}"
suite_path="${X07_PERF_SUITE_BUNDLE:-labs/benchmarks/bundles/canaries.json}"
baseline_path="${X07_PERF_BASELINE:-labs/benchmarks/perf/canaries.json}"
formal_baseline_path="${X07_FORMAL_PERF_BASELINE:-labs/benchmarks/perf/formal_verification.json}"
formal_report_path="${X07_FORMAL_PERF_REPORT_OUT:-}"

if [[ "${X07_SKIP_SUITE_PERF:-0}" != "1" ]]; then
  if [[ ! -d "$solutions_dir" ]]; then
    echo "ERROR: missing solutions dir: $solutions_dir" >&2
    exit 1
  fi
  if [[ ! -f "$baseline_path" ]]; then
    echo "ERROR: missing perf baseline: $baseline_path" >&2
    exit 1
  fi

  "$python_bin" labs/scripts/bench/run_bench_suite.py \
    --suite "$suite_path" \
    --solutions "$solutions_dir" \
    --perf-baseline "$baseline_path"
else
  echo "ok: suite perf baseline skipped"
fi

if [[ "${X07_SKIP_FORMAL_PERF:-0}" != "1" ]]; then
  if [[ ! -f "$formal_baseline_path" ]]; then
    echo "ERROR: missing formal verification perf baseline: $formal_baseline_path" >&2
    exit 1
  fi
  formal_args=(labs/scripts/ci/check_formal_verification_perf.py --baseline "$formal_baseline_path")
  if [[ -n "$formal_report_path" ]]; then
    formal_args+=(--report-out "$formal_report_path")
  fi
  "$python_bin" "${formal_args[@]}"
else
  echo "ok: formal verification perf baseline skipped"
fi

echo "ok: perf baseline gate passed"
