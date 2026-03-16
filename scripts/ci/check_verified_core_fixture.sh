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
  echo "error: verified-core fixture certify requires both cbmc and z3 on PATH" >&2
  exit 2
fi

case "$(uname -s)" in
  Darwin)
    tmp_dir="$(mktemp -d -t x07_verified_core_fixture)"
    ;;
  *)
    tmp_dir="$(mktemp -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

fixture_dir="$root/crates/x07/tests/fixtures/verified_core_fixture_v1"
profile_path="$root/arch/trust/profiles/verified_core_fixture_v1.json"

echo "[check] verified_core_fixture_v1: profile check"
(
  cd "$fixture_dir"
  "$x07_bin" trust profile check \
    --project x07.json \
    --profile "$profile_path" \
    --entry fixture.main \
    >/dev/null
)

echo "[check] verified_core_fixture_v1: tests"
(
  cd "$fixture_dir"
  "$x07_bin" test --all --manifest tests/tests.json >/dev/null
)

if [[ "$have_solvers" != "1" ]]; then
  echo "[check] verified_core_fixture_v1: certify skipped (cbmc/z3 unavailable)"
  printf 'OK %s\n' "$(basename "$0")"
  exit 0
fi

echo "[check] verified_core_fixture_v1: certify"
(
  cd "$fixture_dir"
  "$x07_bin" trust certify \
    --project x07.json \
    --profile "$profile_path" \
    --entry fixture.main \
    --out-dir "$tmp_dir/cert" \
    >/dev/null
)

cert_path="$tmp_dir/cert/certificate.json"
test -f "$cert_path"

proof_paths_file="$tmp_dir/proof_paths.txt"
python3 - "$cert_path" "$proof_paths_file" <<'PY'
import json
import pathlib
import sys

cert_path = pathlib.Path(sys.argv[1])
proof_paths_path = pathlib.Path(sys.argv[2])
cert = json.loads(cert_path.read_text())

errors = []

entry = cert.get("entry")
operational_entry = cert.get("operational_entry_symbol")
if entry != operational_entry:
    errors.append(
        ("X07REL_ESURROGATE_ENTRY", f"entry {entry!r} must match operational_entry_symbol {operational_entry!r}")
    )

if cert.get("accepted_depends_on_bounded_proof"):
    errors.append(
        ("X07REL_EBOUNDED_PROOF", "certificate must not depend on bounded proof under the strict fixture")
    )

if cert.get("accepted_depends_on_dev_only_assumption"):
    errors.append(
        ("X07REL_EDEV_ONLY_ASSUMPTION", "certificate must not accept developer-only proof assumptions")
    )

if cert.get("imported_summary_inventory"):
    errors.append(
        ("X07REL_ECOVERAGE_ONLY_IMPORT", "strict fixture must not accept coverage-only imported summaries")
    )

proof_inventory = cert.get("proof_inventory") or []
if not proof_inventory:
    errors.append(("error", "strict fixture produced no proof inventory"))

proof_paths = []
for item in proof_inventory:
    proof_object = item.get("proof_object")
    proof_check = item.get("proof_check_report")
    if not proof_object or not proof_object.get("path"):
        errors.append(("error", f"missing proof object for {item.get('symbol')!r}"))
        continue
    if not proof_check or not proof_check.get("path"):
        errors.append(("error", f"missing proof check report for {item.get('symbol')!r}"))
    proof_paths.append(proof_object["path"])

if errors:
    for code, message in errors:
        print(f"{code}: {message}", file=sys.stderr)
    sys.exit(1)

proof_paths_path.write_text("\n".join(proof_paths) + ("\n" if proof_paths else ""))
PY

echo "[check] verified_core_fixture_v1: proof checks"
while IFS= read -r proof_path; do
  [[ -n "$proof_path" ]] || continue
  "$x07_bin" prove check --proof "$proof_path" >/dev/null
done <"$proof_paths_file"

surrogate_dir="$tmp_dir/surrogate"
cp -R "$fixture_dir" "$surrogate_dir"
python3 - "$surrogate_dir/x07.json" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
doc = json.loads(path.read_text())
doc["certification_entry_symbol"] = "fixture.surrogate"
path.write_text(json.dumps(doc, indent=2) + "\n")
PY

echo "[check] verified_core_fixture_v1: surrogate entry rejected"
surrogate_out="$tmp_dir/surrogate-certify.json"
if (
  cd "$surrogate_dir" && "$x07_bin" trust certify \
    --project x07.json \
    --profile "$profile_path" \
    --entry fixture.main \
    --out-dir "$tmp_dir/surrogate-cert" \
    >"$surrogate_out"
); then
  echo "X07REL_ESURROGATE_ENTRY: surrogate certification entry was accepted unexpectedly" >&2
  exit 1
fi
python3 - "$surrogate_out" <<'PY'
import json
import pathlib
import sys

report = json.loads(pathlib.Path(sys.argv[1]).read_text())
codes = {
    diag.get("code")
    for diag in report.get("diagnostics", [])
    if isinstance(diag, dict)
}
if "X07TC_ESURROGATE_ENTRY_FORBIDDEN" not in codes:
    print(
        "X07REL_ESURROGATE_ENTRY: missing X07TC_ESURROGATE_ENTRY_FORBIDDEN rejection diagnostic",
        file=sys.stderr,
    )
    sys.exit(1)
PY

printf 'OK %s\n' "$(basename "$0")"
