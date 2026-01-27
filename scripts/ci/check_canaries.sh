#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

source ./scripts/ci/lib_ext_packages.sh

tmp_dir="$(mktemp -t x07_canaries_XXXXXX -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

run_quiet() {
  local name="$1"
  shift

  local log_path="$tmp_dir/${name}.log"

  echo "canary: $name"

  if "$@" >"$log_path" 2>&1; then
    return 0
  fi

  echo "ERROR: canary step failed: $name" >&2
  if command -v tail >/dev/null 2>&1; then
    tail -n 200 "$log_path" >&2 || true
  else
    cat "$log_path" >&2 || true
  fi

  return 1
}

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi

run_quiet "check_prod_surface_lean" "$python_bin" scripts/ci/check_prod_surface_lean.py

# Milestone 1: canonical repair loop is implicit in run/build.
run_quiet "check_run_auto_repair" ./scripts/ci/check_run_auto_repair.sh
run_quiet "check_build_auto_repair" ./scripts/ci/check_build_auto_repair.sh

# Milestone 2: whitespace-capable literals / authoring ergonomics.
run_quiet "check_text_literals_smoke" ./scripts/ci/check_text_literals_smoke.sh

# Milestone 3: actionable diagnostics (ptr + suggested fix + quickfix).
run_quiet "check_diagnostics_actionable" "$python_bin" scripts/ci/check_diagnostics_actionable.py

# Milestone 4: package/module discovery (provides).
run_quiet "check_pkg_provides_smoke" ./scripts/ci/check_pkg_provides_smoke.sh

# Milestone 5: concurrency + parallelism in os + sandbox worlds.
run_quiet "check_concurrency_parallelism_smoke" ./scripts/ci/check_concurrency_parallelism_smoke.sh

# Milestone 6: threads policy enforcement in sandbox.
run_quiet "check_threads_smoke" ./scripts/ci/check_threads_smoke.sh

export X07C_BIN
X07C_BIN="$(./scripts/ci/find_x07c.sh)"

run_quiet "check_x07_parens" "$python_bin" scripts/check_x07_parens.py
run_quiet "check_language_guide_sync" ./scripts/ci/check_language_guide_sync.sh
run_quiet "check_llm_contracts" ./scripts/ci/check_llm_contracts.sh
run_quiet "check_project_manifests" "$python_bin" scripts/ci/check_project_manifests.py
run_quiet "check_package_manifests" "$python_bin" scripts/ci/check_package_manifests.py
run_quiet "check_capabilities_catalog" "$python_bin" scripts/ci/check_capabilities_catalog.py
run_quiet "check_registry_backlog" "$python_bin" scripts/ci/check_registry_backlog.py --check
run_quiet "check_package_policy" "$python_bin" scripts/ci/check_package_policy.py
run_quiet "check_doc_command_surface" "$python_bin" scripts/ci/check_doc_command_surface.py
run_quiet "check_guides_structure" "$python_bin" scripts/ci/check_guides_structure.py
run_quiet "check_doc_version_pins" "$python_bin" scripts/ci/check_doc_version_pins.py
run_quiet "check_policy_templates" ./scripts/ci/check_policy_templates.sh
run_quiet "check_repair_corpus" "$python_bin" scripts/ci/check_repair_corpus.py
run_quiet "check_agent_scenarios" ./scripts/ci/check_agent_scenarios.sh
run_quiet "check_agent_examples" ./scripts/ci/check_agent_examples.sh
run_quiet "check_readme_commands" ./scripts/ci/check_readme_commands.sh
run_quiet "check_doc_flows" ./scripts/ci/check_doc_flows.sh
run_quiet "check_x07test_smoke" ./scripts/ci/check_x07test_smoke.sh
run_quiet "generate_stdlib_lock" "$python_bin" scripts/generate_stdlib_lock.py --check
run_quiet "generate_stdlib_os_lock" "$python_bin" scripts/generate_stdlib_lock.py --stdlib-root stdlib/os --out stdlib.os.lock --check
run_quiet "check_x07import_diagnostics_sync" ./scripts/ci/check_x07import_diagnostics_sync.sh
run_quiet "check_x07import_generated" ./scripts/ci/check_x07import_generated.sh

# Milestone 7: installer/docs contract stays consistent with release targets.
run_quiet "check_install_docs_targets" ./scripts/ci/check_install_docs_targets.sh

echo "ok: canary gate passed"
