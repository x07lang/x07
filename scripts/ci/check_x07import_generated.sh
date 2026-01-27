#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

manifest="${X07IMPORT_MANIFEST:-$root/ci/fixtures/x07import/manifest.json}"
if [[ ! -f "$manifest" ]]; then
  echo "error: x07import manifest not found: $manifest" >&2
  echo "hint: set X07IMPORT_MANIFEST=/abs/path/to/manifest.json" >&2
  exit 2
fi

cargo run -p x07import-cli -- batch --manifest "$manifest" --check

echo "ok: x07import outputs match $manifest"
