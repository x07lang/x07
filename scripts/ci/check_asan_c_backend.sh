#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

if [[ -z "${X07_CC_ARGS:-}" ]]; then
  X07_CC_ARGS="-fsanitize=address,undefined -fno-omit-frame-pointer -g -O1"
  export X07_CC_ARGS
fi

cargo test -p x07-host-runner

echo "ok: C backend artifacts run clean under sanitizers"

