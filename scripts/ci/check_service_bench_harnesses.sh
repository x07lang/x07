#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmp_dir}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

run_one() {
  local name="$1"
  local script="$2"
  local out="$3"
  BENCH_DRY=1 "${script}" --out "${out}" >/dev/null
  BENCH_OUT="${out}" BENCH_KIND="${name}" python3 - <<'PY'
import json
import os

path = os.environ["BENCH_OUT"]
kind = os.environ["BENCH_KIND"]
doc = json.load(open(path, "r", encoding="utf-8"))

for key in ["schema_version", "kind"]:
    if key not in doc:
        raise SystemExit(f"missing {key}: {path}")

if doc["kind"] != kind:
    raise SystemExit(f"wrong kind: {doc.get('kind')} (expected {kind})")

print("ok:", path)
PY
}

run_one "replicated-http" "./bench/replicated-http/run.sh" "${tmp_dir}/replicated-http.json"
run_one "partitioned-consumer" "./bench/partitioned-consumer/run.sh" "${tmp_dir}/partitioned-consumer.json"
run_one "burst-batch" "./bench/burst-batch/run.sh" "${tmp_dir}/burst-batch.json"

echo "ok: service bench harness dry smoke"
