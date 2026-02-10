#!/usr/bin/env bash
set -euo pipefail

# CI: Telemetry - format, lint, test, pbt
# Saves artifacts to target/ci/telemetry/
OUT_DIR="target/ci/telemetry"
mkdir -p "$OUT_DIR"

echo "=== Telemetry: fmt ==="
x07 fmt --check --input src --input tests

echo "=== Telemetry: lint ==="
x07 lint --input src --input tests > "$OUT_DIR/lint.json"

echo "=== Telemetry: unit tests ==="
x07 test --json --report-out "$OUT_DIR/test.json" --manifest tests/tests.json

echo "=== Telemetry: all tests (including PBT + RR) ==="
x07 test --json --report-out "$OUT_DIR/test_all.json" --all --manifest tests/tests.json

echo "=== Telemetry complete. Artifacts in $OUT_DIR/ ==="
ls -la "$OUT_DIR/"
