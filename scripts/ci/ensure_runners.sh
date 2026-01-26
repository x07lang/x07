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

has_runner() {
  local name="$1"
  local candidates=(
    "target/debug/${name}"
    "target/debug/${name}.exe"
    "target/release/${name}"
    "target/release/${name}.exe"
  )

  for p in "${candidates[@]}"; do
    if is_executable "$p"; then
      return 0
    fi
  done
  return 1
}

if has_runner "x07-host-runner" && has_runner "x07-os-runner"; then
  exit 0
fi

if ! cargo build -p x07-host-runner -p x07-os-runner >/dev/null 2>&1; then
  cargo build -p x07-host-runner -p x07-os-runner
fi
