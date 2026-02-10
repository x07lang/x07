#!/usr/bin/env bash
set -euo pipefail

# Emit trust artifacts (budgets, capabilities, nondeterminism, SBOM placeholders).
# Outputs: target/trust/trust.json  target/trust/trust.html

OUT_DIR="target/trust"
mkdir -p "$OUT_DIR"

x07 trust report \
  --project x07.json \
  --profile sandbox \
  --out "$OUT_DIR/trust.json" \
  --html-out "$OUT_DIR/trust.html" \
  --json=pretty

echo "trust report artifacts written to $OUT_DIR/"

