#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

cargo run -p x07import-cli -- batch --manifest import_sources/manifest.json --check

echo "ok: x07import outputs match import_sources"

