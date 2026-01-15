#!/usr/bin/env bash
set -euo pipefail

need() {
  command -v "$1" >/dev/null 2>&1 || { echo "ERROR: missing tool: $1" >&2; exit 1; }
}

need cargo

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if command -v python3 >/dev/null 2>&1; then
    python_bin="python3"
  else
    python_bin="python"
  fi
fi
need "$python_bin"

cc_bin="${X07_CC:-cc}"
if [[ "$cc_bin" == */* ]]; then
  [[ -x "$cc_bin" ]] || { echo "ERROR: X07_CC is not executable: $cc_bin" >&2; exit 1; }
else
  need "$cc_bin"
fi

echo "cargo: $(cargo --version)"
echo "$python_bin: $("$python_bin" --version)"
echo "cc: $cc_bin"
"$cc_bin" --version | head -n 1
