#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

source ./scripts/ci/lib_ext_packages.sh

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi

export X07C_BIN
X07C_BIN="$(./scripts/ci/find_x07c.sh)"

"$python_bin" scripts/check_x07_parens.py >/dev/null
./scripts/ci/check_language_guide_sync.sh >/dev/null
./scripts/ci/check_llm_contracts.sh >/dev/null
"$python_bin" scripts/ci/check_project_manifests.py >/dev/null
"$python_bin" scripts/ci/check_package_manifests.py >/dev/null
"$python_bin" scripts/ci/check_capabilities_catalog.py >/dev/null
"$python_bin" scripts/ci/check_doc_command_surface.py >/dev/null
"$python_bin" scripts/ci/check_repair_corpus.py >/dev/null
./scripts/ci/check_agent_examples.sh >/dev/null
./scripts/ci/check_readme_commands.sh >/dev/null
./scripts/ci/check_x07test_smoke.sh >/dev/null
"$python_bin" scripts/generate_stdlib_lock.py --check >/dev/null
"$python_bin" scripts/generate_stdlib_lock.py --stdlib-root stdlib/os --out stdlib.os.lock --check >/dev/null
./scripts/ci/check_x07import_diagnostics_sync.sh >/dev/null
./scripts/ci/check_x07import_generated.sh >/dev/null

solutions_dir="${X07_BENCH_SOLUTIONS:-benchmarks/solutions}"

if [[ ! -d "$solutions_dir" ]]; then
  echo "ERROR: missing solutions dir: $solutions_dir" >&2
  exit 1
fi

"$python_bin" scripts/bench/run_bench_suite.py --suite benchmarks/solve-pure/phaseH1-smoke.json --solutions "$solutions_dir"
"$python_bin" scripts/bench/run_bench_suite.py --suite benchmarks/solve-full/phaseH2-smoke.json --solutions "$solutions_dir"
"$python_bin" scripts/bench/run_bench_suite.py --suite benchmarks/solve-pure/emitters-v1-suite.json --solutions "$solutions_dir"
X07_BENCH_MODULE_ROOT="stdlib/std/0.1.1/modules:$(x07_ext_pkg_modules x07-ext-cli):$(x07_ext_pkg_modules x07-ext-data-model):$(x07_ext_pkg_modules x07-ext-json-rs)" \
  "$python_bin" scripts/bench/run_bench_suite.py --suite benchmarks/solve-pure/cli-v1-specrows-determinism.json --solutions "$solutions_dir"

echo "ok: canary gate passed"
