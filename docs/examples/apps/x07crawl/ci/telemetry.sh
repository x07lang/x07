#!/usr/bin/env bash
set -euo pipefail

echo "=== x07crawl telemetry ==="

TELEMETRY_DIR="target/telemetry"
mkdir -p "$TELEMETRY_DIR"

echo "--- fmt ---"
x07 fmt --check --input src --input tests
echo "fmt: ok"

echo "--- lint ---"
x07 lint --input src --input tests > "$TELEMETRY_DIR/lint.json"
echo "lint: ok"

echo "--- test ---"
x07 test --json --report-out "$TELEMETRY_DIR/test.json" --manifest tests/tests.json
echo "test: ok"

echo "--- pbt ---"
x07 test --json --report-out "$TELEMETRY_DIR/test_all.json" --all --manifest tests/tests.json
echo "test (all): ok"

echo "=== telemetry artifacts written to $TELEMETRY_DIR/ ==="
