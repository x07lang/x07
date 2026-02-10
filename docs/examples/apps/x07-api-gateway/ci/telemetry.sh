#!/usr/bin/env bash
# Full CI telemetry pipeline: fmt, arch, test, pbt, trust.
# Usage: ci/telemetry.sh
# Outputs: target/telemetry/
set -euo pipefail

OUT_DIR="target/telemetry"
mkdir -p "$OUT_DIR"

echo "=== fmt ==="
x07 fmt --check --input src --input tests
echo "fmt ok"

echo "=== lint ==="
x07 lint --input src --input tests > "$OUT_DIR/lint.json"
echo "lint ok"

echo "=== arch check ==="
x07 arch check --json --report-out "$OUT_DIR/arch.json"
echo "arch ok"

echo "=== run (compile + execute) ==="
x07 run --json --report-out "$OUT_DIR/run.json"
echo "run ok"

echo "=== test ==="
x07 test --json --report-out "$OUT_DIR/test.json"
echo "test ok"

echo "=== test --repeat 2 (determinism gate) ==="
x07 test --repeat 2 --json --report-out "$OUT_DIR/test_repeat.json"
echo "determinism gate ok"

echo "=== test --pbt ==="
x07 test --pbt --json --report-out "$OUT_DIR/pbt.json"
echo "pbt ok"

echo "=== trust report ==="
mkdir -p "$OUT_DIR/trust"
x07 trust report --out "$OUT_DIR/trust/trust.json" --html-out "$OUT_DIR/trust/trust.html" --json=pretty
echo "trust ok"

echo ""
echo "all telemetry artifacts written to $OUT_DIR/"
