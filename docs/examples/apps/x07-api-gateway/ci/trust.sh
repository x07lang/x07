#!/usr/bin/env bash
# Emit CI trust report (budgets, capabilities, nondeterminism, SBOM).
# Usage: ci/trust.sh
# Outputs: target/trust/trust.json  target/trust/trust.html
set -euo pipefail

OUT_DIR="target/trust"
mkdir -p "$OUT_DIR"

x07 trust report \
  --project x07.json \
  --profile sandbox \
  --out "$OUT_DIR/trust.json" \
  --html-out "$OUT_DIR/trust.html" \
  --json=pretty

echo "trust report artifacts written to $OUT_DIR/"
