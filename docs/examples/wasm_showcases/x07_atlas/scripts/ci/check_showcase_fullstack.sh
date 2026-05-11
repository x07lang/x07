#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="$ROOT/build/showcase_fullstack"
REPORT_DIR="$BUILD_DIR/reports"
DIST_DIR="$ROOT/dist/showcase_fullstack"
APP_DEV_DIR="$DIST_DIR/app.atlas_dev"
APP_RELEASE_DIR="$DIST_DIR/app.atlas_release"
APP_BUDGET_DIR="$DIST_DIR/app.atlas_budget"
PACK_RELEASE_DIR="$DIST_DIR/pack.atlas_release"
DEPLOY_RELEASE_DIR="$DIST_DIR/deploy.atlas_release"

cd "$ROOT"

rm -rf "$BUILD_DIR" "$DIST_DIR"
mkdir -p "$REPORT_DIR" "$PACK_RELEASE_DIR" "$DEPLOY_RELEASE_DIR"

FAILURES=0
PYTHON="${PYTHON:-python3}"

note() {
  printf '%s\n' "$*"
}

mark_failure() {
  FAILURES=$((FAILURES + 1))
  note "FAIL: $*"
}

run_step() {
  local name="$1"
  shift
  note "==> $name"
  if ! "$@"; then
    mark_failure "$name"
    return 1
  fi
  return 0
}

run_step_allow_fail() {
  local name="$1"
  shift
  note "==> $name"
  if ! "$@"; then
    mark_failure "$name"
  fi
}

report_incident_dir() {
  local report_path="$1"
  if [ ! -f "$report_path" ]; then
    return 1
  fi
  "$PYTHON" - "$report_path" <<'PY'
import json
import pathlib
import sys

report = pathlib.Path(sys.argv[1])
doc = json.loads(report.read_text(encoding="utf-8"))
result = doc.get("result", {})
stdout_json = result.get("stdout_json", {}) if isinstance(result, dict) else {}
incident_dir = stdout_json.get("incident_dir")
if isinstance(incident_dir, str) and incident_dir:
    print(incident_dir)
PY
}

resolve_path() {
  local path="$1"
  case "$path" in
    /*)
      printf '%s\n' "$path"
      ;;
    *)
      printf '%s\n' "$ROOT/$path"
      ;;
  esac
}

run_step_expect_failure() {
  local name="$1"
  local report_path="$2"
  shift 2
  note "==> $name"
  if "$@"; then
    mark_failure "$name (expected failure)"
    return 1
  fi
  local incident_dir
  incident_dir="$(report_incident_dir "$report_path" || true)"
  if [ -z "$incident_dir" ]; then
    mark_failure "$name (missing incident_dir)"
    return 1
  fi
  return 0
}

X07_BIN="${X07_BIN:-$(command -v x07 2>/dev/null || true)}"
X07_WASM_BIN="${X07_WASM_BIN:-$(command -v x07-wasm 2>/dev/null || true)}"

if [ -z "$X07_BIN" ] || [ -z "$X07_WASM_BIN" ]; then
  note "missing required binaries: x07 and/or x07-wasm"
  exit 1
fi

export X07_WORKSPACE_ROOT="$ROOT"

if [ -z "$X07_WASM_BIN" ] || [ ! -x "$X07_WASM_BIN" ]; then
  note "x07-wasm binary is not executable"
  exit 1
fi

run_step "x07 pkg lock atlas frontend" \
  "$X07_BIN" pkg lock --project "$ROOT/frontend/x07.json"

run_step "x07-wasm doctor" \
  "$X07_WASM_BIN" doctor --json --report-out "$REPORT_DIR/doctor.json" --quiet-json

run_step "x07-wasm app profile validate atlas_dev" \
  "$X07_WASM_BIN" app profile validate --profile-file "$ROOT/arch/app/profiles/atlas_dev.json" --json --report-out "$REPORT_DIR/app.profile.validate.json" --quiet-json
run_step "x07-wasm web-ui contracts validate" \
  "$X07_WASM_BIN" web-ui contracts validate --json --report-out "$REPORT_DIR/web-ui.contracts.validate.json" --quiet-json
run_step "x07-wasm http contracts validate" \
  "$X07_WASM_BIN" http contracts validate --json --report-out "$REPORT_DIR/http.contracts.validate.json" --quiet-json
run_step "x07-wasm caps validate" \
  "$X07_WASM_BIN" caps validate --profile "$ROOT/arch/app/ops/caps_release.json" --json --report-out "$REPORT_DIR/caps.validate.json" --quiet-json
run_step "x07-wasm ops validate" \
  "$X07_WASM_BIN" ops validate --json --report-out "$REPORT_DIR/ops.validate.json" --quiet-json
run_step "x07-wasm slo validate" \
  "$X07_WASM_BIN" slo validate --profile "$ROOT/arch/slo/slo_min.json" --json --report-out "$REPORT_DIR/slo.validate.json" --quiet-json

run_step "x07-wasm app build atlas_dev" \
  "$X07_WASM_BIN" app build --profile atlas_dev --clean --out-dir "$APP_DEV_DIR" --json --report-out "$REPORT_DIR/app.build.atlas_dev.json" --quiet-json
run_step "x07-wasm app build atlas_budget" \
  "$X07_WASM_BIN" app build --profile atlas_budget --clean --out-dir "$APP_BUDGET_DIR" --json --report-out "$REPORT_DIR/app.build.atlas_budget.json" --quiet-json
run_step "x07-wasm app build atlas_release" \
  "$X07_WASM_BIN" app build --profile atlas_release --clean --out-dir "$APP_RELEASE_DIR" --json --report-out "$REPORT_DIR/app.build.atlas_release.json" --quiet-json

run_step "x07-wasm app serve smoke atlas_dev" \
  "$X07_WASM_BIN" app serve --dir "$APP_DEV_DIR" --mode smoke --json --report-out "$REPORT_DIR/app.serve.smoke.json" --quiet-json

run_step "x07-wasm app test happy_path" \
  "$X07_WASM_BIN" app test --dir "$APP_DEV_DIR" --trace "$ROOT/tests/traces/happy_path.trace.json" --json --report-out "$REPORT_DIR/app.test.happy_path.json" --quiet-json
run_step "x07-wasm app test validation_error" \
  "$X07_WASM_BIN" app test --dir "$APP_DEV_DIR" --trace "$ROOT/tests/traces/validation_error.trace.json" --json --report-out "$REPORT_DIR/app.test.validation_error.json" --quiet-json
run_step_expect_failure "x07-wasm app test perf_budget" "$REPORT_DIR/app.test.perf_budget.json" \
  "$X07_WASM_BIN" app test --dir "$APP_BUDGET_DIR" --trace "$ROOT/tests/traces/perf_budget.trace.json" --json --report-out "$REPORT_DIR/app.test.perf_budget.json" --quiet-json
run_step_expect_failure "x07-wasm app test incident_seed" "$REPORT_DIR/app.test.incident_seed.json" \
  "$X07_WASM_BIN" app test --dir "$APP_DEV_DIR" --trace "$ROOT/tests/traces/incident_seed.trace.json" --json --report-out "$REPORT_DIR/app.test.incident_seed.json" --quiet-json

INCIDENT_DIR="$(report_incident_dir "$REPORT_DIR/app.test.incident_seed.json" || true)"
if [ -n "${INCIDENT_DIR:-}" ]; then
  INCIDENT_DIR="$(resolve_path "$INCIDENT_DIR")"
  run_step "x07-wasm app regress from-incident" \
    "$X07_WASM_BIN" app regress from-incident "$INCIDENT_DIR" --out-dir "$ROOT/tests/regress" --name atlas_incident --json --report-out "$REPORT_DIR/app.regress.from_incident.json" --quiet-json
  if [ -f "$ROOT/tests/regress/atlas_incident.trace.json" ]; then
    run_step "x07-wasm app test generated regression" \
      "$X07_WASM_BIN" app test --dir "$APP_DEV_DIR" --trace "$ROOT/tests/regress/atlas_incident.trace.json" --json --report-out "$REPORT_DIR/app.test.regress.atlas_incident.json" --quiet-json
  else
    mark_failure "generated regression trace missing"
  fi
else
  mark_failure "incident bundle missing under .x07-wasm/incidents/app"
fi

run_step "x07-wasm app pack atlas_release" \
  "$X07_WASM_BIN" app pack --bundle-manifest "$APP_RELEASE_DIR/app.bundle.json" --profile-id atlas_release --out-dir "$PACK_RELEASE_DIR" --json --report-out "$REPORT_DIR/app.pack.atlas_release.json" --quiet-json
run_step "x07-wasm app verify atlas_release" \
  "$X07_WASM_BIN" app verify --pack-manifest "$PACK_RELEASE_DIR/app.pack.json" --json --report-out "$REPORT_DIR/app.verify.atlas_release.json" --quiet-json
run_step "x07-wasm provenance attest atlas_release" \
  "$X07_WASM_BIN" provenance attest --pack-manifest "$PACK_RELEASE_DIR/app.pack.json" --ops "$ROOT/arch/app/ops/ops_release.json" --signing-key "$ROOT/arch/provenance/dev.ed25519.signing_key.b64" --out "$PACK_RELEASE_DIR/app.provenance.dsse.json" --json --report-out "$REPORT_DIR/provenance.attest.atlas_release.json" --quiet-json
run_step "x07-wasm provenance verify atlas_release" \
  "$X07_WASM_BIN" provenance verify --attestation "$PACK_RELEASE_DIR/app.provenance.dsse.json" --pack-dir "$PACK_RELEASE_DIR" --trusted-public-key "$ROOT/arch/provenance/dev.ed25519.public_key.b64" --json --report-out "$REPORT_DIR/provenance.verify.atlas_release.json" --quiet-json
run_step "x07-wasm deploy plan atlas_release" \
  "$X07_WASM_BIN" deploy plan --pack-manifest "$PACK_RELEASE_DIR/app.pack.json" --ops "$ROOT/arch/app/ops/ops_release.json" --out-dir "$DEPLOY_RELEASE_DIR" --json --report-out "$REPORT_DIR/deploy.plan.atlas_release.json" --quiet-json
run_step "x07-wasm slo eval atlas_canary_ok" \
  "$X07_WASM_BIN" slo eval --profile "$ROOT/arch/slo/slo_min.json" --metrics "$ROOT/tests/fixtures/metrics/atlas_canary_ok.json" --json --report-out "$REPORT_DIR/slo.eval.atlas_canary_ok.json" --quiet-json

if [ "$FAILURES" -ne 0 ]; then
  note "showcase gate finished with $FAILURES failure(s)"
  exit 1
fi

note "check_showcase_fullstack: PASS"
