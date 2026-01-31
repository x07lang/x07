#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null
./scripts/ci/ensure_math_backend.sh >/dev/null

cargo run -p x07 -- test --manifest tests/tests.json --no-fail-fast --json=false

echo "ok: x07test smoke suite passed"
