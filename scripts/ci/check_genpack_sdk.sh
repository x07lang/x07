#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

python_bin="${X07_PYTHON:-}"
if [[ -z "$python_bin" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi

command -v "$python_bin" >/dev/null 2>&1 || {
  echo "ERROR: missing python interpreter: $python_bin" >&2
  exit 1
}
command -v npm >/dev/null 2>&1 || {
  echo "ERROR: missing npm" >&2
  exit 1
}

"$python_bin" scripts/generate_genpack_error_codes_bindings.py --check
"$python_bin" scripts/check_genpack_error_codes.py --check

cargo build -p x07
export X07_BIN="$root/target/debug/x07"

# Python SDK integration tests
if [[ -x "$python_bin" ]]; then
  "$python_bin" -m pip install -e "sdk/genpack-py[dev]"
  "$python_bin" -m pytest -q sdk/genpack-py/tests
fi

# TypeScript SDK integration tests
npm ci --prefix sdk/genpack-ts
npm run lint --prefix sdk/genpack-ts
npm run test --prefix sdk/genpack-ts

echo "ok: genpack sdk checks passed"
