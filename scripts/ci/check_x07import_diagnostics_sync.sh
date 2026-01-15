#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

tmp="$(mktemp -t x07import_diagnostics_XXXXXX).md"
cleanup() { rm -f "$tmp"; }
trap cleanup EXIT

cargo run -p x07import-core --bin gen_diagnostics_md >"$tmp"

if [[ ! -f "docs/x07import/diagnostics.md" ]]; then
  echo "ERROR: missing docs/x07import/diagnostics.md" >&2
  exit 1
fi

if ! cmp -s "$tmp" "docs/x07import/diagnostics.md"; then
  echo "ERROR: docs/x07import/diagnostics.md is out of date." >&2
  echo "  Run: cargo run -p x07import-core --bin gen_diagnostics_md > docs/x07import/diagnostics.md" >&2
  diff -u "docs/x07import/diagnostics.md" "$tmp" || true
  exit 1
fi

echo "ok: x07import diagnostics.md up to date"

