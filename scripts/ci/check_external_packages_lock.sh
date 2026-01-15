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
  elif command -v python3 >/dev/null 2>&1; then
    python_bin="python3"
  else
    python_bin="python"
  fi
fi

"$python_bin" scripts/check_stdlib_package_manifests.py --root packages/ext
"$python_bin" scripts/generate_external_packages_lock.py --check

echo "ok: external packages lock up to date"
