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
if [[ ! -d "$solutions_dir" ]]; then
  echo "ERROR: missing solutions dir: $solutions_dir" >&2
  exit 1
fi

"$python_bin" labs/scripts/bench/run_bench_suite.py --suite labs/benchmarks/bundles/regression.json --solutions "$solutions_dir"

echo "ok: regression suites pass with reference solutions"
