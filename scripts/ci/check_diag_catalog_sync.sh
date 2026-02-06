#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

tmp_dir="$(mktemp -t x07_diag_catalog_XXXXXX -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

tmp_catalog="$tmp_dir/diagnostics.json"
tmp_docs="$tmp_dir/diagnostic-codes.md"
tmp_extracted="$tmp_dir/extracted_codes.json"
tmp_coverage="$tmp_dir/coverage.json"

cargo run -p x07 -- diag catalog \
  --catalog catalog/diagnostics.json \
  --format both \
  --out-json "$tmp_catalog" \
  --out-md "$tmp_docs" >/dev/null

if ! cmp -s "$tmp_catalog" "catalog/diagnostics.json"; then
  echo "ERROR: catalog/diagnostics.json is out of date." >&2
  echo "  Run: cargo run -p x07 -- diag catalog --catalog catalog/diagnostics.json --format both --out-json catalog/diagnostics.json --out-md docs/toolchain/diagnostic-codes.md" >&2
  diff -u "catalog/diagnostics.json" "$tmp_catalog" || true
  exit 1
fi

if ! cmp -s "$tmp_docs" "docs/toolchain/diagnostic-codes.md"; then
  echo "ERROR: docs/toolchain/diagnostic-codes.md is out of date." >&2
  echo "  Run: cargo run -p x07 -- diag catalog --catalog catalog/diagnostics.json --format both --out-json catalog/diagnostics.json --out-md docs/toolchain/diagnostic-codes.md" >&2
  diff -u "docs/toolchain/diagnostic-codes.md" "$tmp_docs" || true
  exit 1
fi

cargo run -p x07 -- diag check \
  --catalog catalog/diagnostics.json \
  --extracted-out "$tmp_extracted" >/dev/null

cargo run -p x07 -- diag coverage \
  --catalog catalog/diagnostics.json \
  --out "$tmp_coverage" \
  --min-coverage 0.90 \
  --severity error,warning >/dev/null

echo "ok: diagnostic catalog and coverage artifacts are in sync"
