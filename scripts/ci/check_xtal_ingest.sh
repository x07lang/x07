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

fixture_dir="$root/tests/fixtures/xtal_improve_toy"
violations_dir="$fixture_dir/target/xtal/violations"

incident_id=""
for p in "$violations_dir"/*; do
  if [[ -d "$p" ]]; then
    incident_id="$(basename "$p")"
    break
  fi
done
if [[ -z "$incident_id" ]]; then
  echo "error: no incident bundles found under $violations_dir" >&2
  exit 2
fi

case "$(uname -s)" in
  Darwin)
    tmp_dir="$(mktemp -d -t x07_xtal_ingest_toy)"
    ;;
  *)
    tmp_dir="$(mktemp -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

cp -R "$fixture_dir" "$tmp_dir/xtal_ingest_toy"

echo "[check] xtal_ingest_toy: ingest"
(
  cd "$tmp_dir/xtal_ingest_toy"
  "$x07_bin" xtal ingest \
    --project x07.json \
    --input "target/xtal/violations/$incident_id/violation.json" \
    --out-dir "target/xtal/ingest_ci" \
    --improve-out-dir "target/xtal/improve_ingest_ci"
)

ingest_summary_path="$tmp_dir/xtal_ingest_toy/target/xtal/ingest_ci/summary.json"
improve_summary_path="$tmp_dir/xtal_ingest_toy/target/xtal/improve_ingest_ci/summary.json"
test -f "$ingest_summary_path"
test -f "$improve_summary_path"

python3 - <<PY
import json

ingest = json.load(open("$ingest_summary_path", "r", encoding="utf-8"))
assert ingest.get("schema_version") == "x07.xtal.ingest_summary@0.1.0"
assert ingest.get("ok") is True

improve = json.load(open("$improve_summary_path", "r", encoding="utf-8"))
assert improve.get("schema_version") == "x07.xtal.improve_summary@0.1.0"
assert improve.get("ok") is True
PY

printf 'OK %s\n' "$(basename "$0")"

