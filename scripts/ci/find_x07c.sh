#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

x07c_bin="${X07C_BIN:-}"
if [[ -n "${x07c_bin}" ]]; then
  if [[ -x "${x07c_bin}" ]]; then
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
  if [[ -x "$p" ]]; then
    echo "$p"
    exit 0
  fi
done

cargo build -p x07c >/dev/null

for p in "${candidates[@]}"; do
  if [[ -x "$p" ]]; then
    echo "$p"
    exit 0
  fi
done

echo "ERROR: missing x07c binary (build with \`cargo build -p x07c\`)" >&2
exit 1
