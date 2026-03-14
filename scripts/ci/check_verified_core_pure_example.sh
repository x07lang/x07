#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

cargo build -p x07 -p x07-host-runner >/dev/null

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi
if [[ "${x07_bin}" != /* ]]; then
  x07_bin="$root/$x07_bin"
fi

bin_dir="$(cd "$(dirname "$x07_bin")" && pwd)"
export PATH="$bin_dir:$PATH"

require_solvers="${X07_REQUIRE_SOLVERS:-0}"
have_cbmc=0
have_z3=0
if command -v cbmc >/dev/null 2>&1; then
  have_cbmc=1
fi
if command -v z3 >/dev/null 2>&1; then
  have_z3=1
fi
have_solvers=0
if [[ "$have_cbmc" == "1" && "$have_z3" == "1" ]]; then
  have_solvers=1
fi
if [[ "$require_solvers" == "1" && "$have_solvers" != "1" ]]; then
  echo "error: verified-core-pure certify requires both cbmc and z3 on PATH" >&2
  exit 2
fi

case "$(uname -s)" in
  Darwin)
    tmp_dir="$(mktemp -d -t x07_verified_core_pure)"
    ;;
  *)
    tmp_dir="$(mktemp -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

example_dir="$root/docs/examples/verified_core_pure_v1"
test -f "$example_dir/.github/workflows/certify.yml"

echo "[check] verified_core_pure_v1 docs example: profile check"
(
  cd "$example_dir"
  "$x07_bin" trust profile check \
    --project x07.json \
    --profile arch/trust/profiles/verified_core_pure_v1.json \
    --entry example.main \
    >/dev/null
)

echo "[check] verified_core_pure_v1 docs example: tests"
(
  cd "$example_dir"
  "$x07_bin" test --all --manifest tests/tests.json >/dev/null
)

if [[ "$have_solvers" == "1" ]]; then
  echo "[check] verified_core_pure_v1 docs example: certify"
  (
    cd "$example_dir"
    "$x07_bin" trust certify \
      --project x07.json \
      --profile arch/trust/profiles/verified_core_pure_v1.json \
      --entry example.main \
      --out-dir "$tmp_dir/example-cert" \
      >/dev/null
  )
  test -f "$tmp_dir/example-cert/certificate.json"
else
  echo "[check] verified_core_pure_v1 docs example: certify skipped (cbmc/z3 unavailable)"
fi

scaffold_dir="$tmp_dir/init"
mkdir -p "$scaffold_dir"

echo "[check] verified_core_pure_v1 template: init"
(
  cd "$scaffold_dir"
  "$x07_bin" init --template verified-core-pure >/dev/null
)
test -f "$scaffold_dir/.github/workflows/certify.yml"

echo "[check] verified_core_pure_v1 template: profile check"
(
  cd "$scaffold_dir"
  "$x07_bin" trust profile check \
    --project x07.json \
    --profile arch/trust/profiles/verified_core_pure_v1.json \
    --entry example.main \
    >/dev/null
)

echo "[check] verified_core_pure_v1 template: tests"
(
  cd "$scaffold_dir"
  "$x07_bin" test --all --manifest tests/tests.json >/dev/null
)

if [[ "$have_solvers" == "1" ]]; then
  echo "[check] verified_core_pure_v1 template: certify"
  (
    cd "$scaffold_dir"
    "$x07_bin" trust certify \
      --project x07.json \
      --profile arch/trust/profiles/verified_core_pure_v1.json \
      --entry example.main \
      --out-dir target/cert \
      >/dev/null
  )
  test -f "$scaffold_dir/target/cert/certificate.json"
else
  echo "[check] verified_core_pure_v1 template: certify skipped (cbmc/z3 unavailable)"
fi

printf 'OK %s\n' "$(basename "$0")"
