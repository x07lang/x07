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
    tmp_dir="$(mktemp -d -t x07_xtal_improve_toy)"
    ;;
  *)
    tmp_dir="$(mktemp -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

cp -R "$fixture_dir" "$tmp_dir/xtal_improve_toy"

echo "[check] xtal_improve_toy: dev prechecks"
(
  cd "$tmp_dir/xtal_improve_toy"
  "$x07_bin" xtal dev --project x07.json --prechecks-only
)

echo "[check] xtal_improve_toy: improve"
(
  cd "$tmp_dir/xtal_improve_toy"
  "$x07_bin" xtal improve \
    --project x07.json \
    --input "target/xtal/violations/$incident_id/violation.json" \
    --out-dir "target/xtal/improve_ci"
)

summary_path="$tmp_dir/xtal_improve_toy/target/xtal/improve_ci/summary.json"
shadow_manifest="$tmp_dir/xtal_improve_toy/target/xtal/improve_ci/$incident_id/tests.shadow.json"
test -f "$summary_path"
test -f "$shadow_manifest"

echo "[check] xtal_improve_toy: improve --write"
(
  cd "$tmp_dir/xtal_improve_toy"
  "$x07_bin" xtal improve \
    --project x07.json \
    --input "target/xtal/violations/$incident_id/violation.json" \
    --out-dir "target/xtal/improve_ci_write" \
    --write
)

write_summary_path="$tmp_dir/xtal_improve_toy/target/xtal/improve_ci_write/summary.json"
write_shadow_manifest="$tmp_dir/xtal_improve_toy/target/xtal/improve_ci_write/$incident_id/tests.shadow.json"
test -f "$write_summary_path"
test -f "$write_shadow_manifest"

python3 - <<PY
import json
doc = json.load(open("$summary_path", "r", encoding="utf-8"))
assert doc.get("schema_version") == "x07.xtal.improve_summary@0.1.0"
assert doc.get("ok") is True

doc2 = json.load(open("$write_summary_path", "r", encoding="utf-8"))
assert doc2.get("schema_version") == "x07.xtal.improve_summary@0.1.0"
assert doc2.get("ok") is True
assert doc2.get("verify", {}).get("status") == "ok"
assert doc2.get("governance", {}).get("write_requested") is True
PY

printf 'OK %s\n' "$(basename "$0")"
