#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

is_executable() {
  local path="$1"
  if [[ "$path" == *.exe ]]; then
    [[ -f "$path" ]]
  else
    [[ -x "$path" ]]
  fi
}

x07_bin="${X07_BIN:-}"
if [[ -n "${x07_bin}" ]]; then
  if is_executable "${x07_bin}"; then
    echo "${x07_bin}"
    exit 0
  fi
  echo "ERROR: X07_BIN is set but not executable: ${x07_bin}" >&2
  exit 2
fi

candidates=(
  "target/debug/x07"
  "target/debug/x07.exe"
  "target/release/x07"
  "target/release/x07.exe"
)

for p in "${candidates[@]}"; do
  if is_executable "$p"; then
    echo "$p"
    exit 0
  fi
done

if ! cargo build -p x07 >/dev/null 2>&1; then
  cargo build -p x07
  exit 1
fi

for p in "${candidates[@]}"; do
  if is_executable "$p"; then
    echo "$p"
    exit 0
  fi
done

echo "ERROR: missing x07 binary (build with \`cargo build -p x07\`)" >&2
exit 1
