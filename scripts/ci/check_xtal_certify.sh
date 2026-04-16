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
  echo "error: xtal certify fixture requires both cbmc and z3 on PATH" >&2
  exit 2
fi

fixture_dir="$root/tests/fixtures/xtal_certify_toy"

echo "[check] xtal_certify_toy: dev prechecks"
(
  cd "$fixture_dir"
  "$x07_bin" xtal dev --project x07.json
)

if [[ "$have_solvers" != "1" ]]; then
  echo "[check] xtal_certify_toy: certify skipped (cbmc/z3 unavailable)"
  printf 'OK %s\n' "$(basename "$0")"
  exit 0
fi

case "$(uname -s)" in
  Darwin)
    tmp_dir="$(mktemp -d -t x07_xtal_certify_toy)"
    ;;
  *)
    tmp_dir="$(mktemp -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

out_dir="$tmp_dir/cert"

echo "[check] xtal_certify_toy: certify"
(
  cd "$fixture_dir"
  "$x07_bin" xtal certify \
    --project x07.json \
    --all \
    --out-dir "$out_dir" \
    --no-prechecks
)

summary_path="$out_dir/summary.json"
test -f "$summary_path"

entry_dir="$out_dir/fixture.main"
test -f "$entry_dir/certificate.json"
test -f "$entry_dir/trust.report.json"

printf 'OK %s\n' "$(basename "$0")"
