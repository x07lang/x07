#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

export X07_SANDBOX_BACKEND="${X07_SANDBOX_BACKEND:-os}"
export X07_I_ACCEPT_WEAKER_ISOLATION="${X07_I_ACCEPT_WEAKER_ISOLATION:-1}"

./scripts/ci/check_tools.sh >/dev/null
./scripts/ci/ensure_math_backend.sh >/dev/null
./scripts/ci/ensure_stream_xf_backend.sh >/dev/null

cargo build -q -p x07-os-runner >/dev/null

cargo run -p x07 -- test --manifest tests/tests.json --no-fail-fast --json=false

echo "ok: x07test smoke suite passed"
