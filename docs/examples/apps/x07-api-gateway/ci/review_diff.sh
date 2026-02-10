#!/usr/bin/env bash
# Produce semantic (x07AST-level) diff report between two snapshots.
# Usage: ci/review_diff.sh <FROM_REF> <TO_REF>
#   FROM_REF / TO_REF are directory paths (or git worktree checkouts).
# Outputs: target/review/diff.html  target/review/diff.json
set -euo pipefail

FROM_REF="${1:?Usage: ci/review_diff.sh <FROM_REF> <TO_REF>}"
TO_REF="${2:?Usage: ci/review_diff.sh <FROM_REF> <TO_REF>}"

OUT_DIR="target/review"
mkdir -p "$OUT_DIR"

x07 review diff \
  --from "$FROM_REF" \
  --to "$TO_REF" \
  --html-out "$OUT_DIR/diff.html" \
  --json-out "$OUT_DIR/diff.json" \
  --json=pretty

echo "review diff artifacts written to $OUT_DIR/"
