#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

need() {
  command -v "$1" >/dev/null 2>&1 || { echo "ERROR: missing tool: $1" >&2; exit 2; }
}

root="$(repo_root)"
cd "$root"

need cargo
need cargo-fuzz

if ! cargo +nightly --version >/dev/null 2>&1; then
  echo "ERROR: Rust nightly toolchain is required for fuzzing (try: rustup toolchain install nightly)" >&2
  exit 2
fi

cargo +nightly fuzz run --fuzz-dir labs/fuzz parse_x07ast_json -- -max_total_time=30
cargo +nightly fuzz run --fuzz-dir labs/fuzz parse_sexpr -- -max_total_time=30
cargo +nightly fuzz run --fuzz-dir labs/fuzz compile_program_to_c -- -max_total_time=30

echo "ok: fuzz smoke passed"
