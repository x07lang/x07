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

x07c_bin="${X07C_BIN:-}"
if [[ -n "${x07c_bin}" ]]; then
  if is_executable "${x07c_bin}"; then
    echo "${x07c_bin}"
    exit 0
  fi
  echo "ERROR: X07C_BIN is set but not executable: ${x07c_bin}" >&2
  exit 2
fi

candidates=(
  "target/debug/x07c"
  "target/debug/x07c.exe"
  "target/release/x07c"
  "target/release/x07c.exe"
)

for p in "${candidates[@]}"; do
  if is_executable "$p"; then
    echo "$p"
    exit 0
  fi
done

if ! cargo build -p x07c >/dev/null 2>&1; then
  cargo build -p x07c
  exit 1
fi

for p in "${candidates[@]}"; do
  if is_executable "$p"; then
    echo "$p"
    exit 0
  fi
done

echo "ERROR: missing x07c binary (build with \`cargo build -p x07c\`)" >&2
exit 1
