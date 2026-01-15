#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

profile="${1:-}"
shift || true

if [[ -z "${profile}" ]]; then
  echo "usage: ./scripts/ci/run.sh <dev|pr|nightly|release> [--strict]" >&2
  exit 2
fi

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi

exec "$python_bin" scripts/ci/run.py --profile "$profile" --progress "$@"
