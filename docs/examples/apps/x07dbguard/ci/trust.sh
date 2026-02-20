#!/usr/bin/env bash
set -euo pipefail

# Emit trust artifacts (budgets, capabilities, nondeterminism, deterministic SBOM artifacts).
# Outputs: target/trust/trust.json  target/trust/trust.html  target/trust/trust.sbom.cdx.json

OUT_DIR="target/trust"
mkdir -p "$OUT_DIR"

x07 trust report \
  --project x07.json \
  --profile sandbox \
  --out "$OUT_DIR/trust.json" \
  --html-out "$OUT_DIR/trust.html" \
  --json=pretty

echo "trust report artifacts written to $OUT_DIR/"
