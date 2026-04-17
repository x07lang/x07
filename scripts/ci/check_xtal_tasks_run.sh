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
    tmp_dir="$(mktemp -d -t x07_xtal_tasks_toy)"
    ;;
  *)
    tmp_dir="$(mktemp -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

cp -R "$fixture_dir" "$tmp_dir/xtal_tasks_toy"

echo "[check] xtal_tasks_toy: tasks run"
(
  cd "$tmp_dir/xtal_tasks_toy"
  "$x07_bin" xtal tasks run \
    --project x07.json \
    --input "target/xtal/violations/$incident_id/violation.json" \
    --out-dir "target/xtal/tasks_ci"
)

events_path="$tmp_dir/xtal_tasks_toy/target/xtal/events/$incident_id/events.jsonl"
test -f "$events_path"

python3 - <<PY
import json

try:
    import jsonschema
except Exception as exc:
    raise SystemExit(f"error: python jsonschema is required for this gate: {exc}")

schema_path = "$root/spec/x07.xtal.recovery_event@0.1.0.schema.json"
events_path = "$events_path"

schema = json.load(open(schema_path, "r", encoding="utf-8"))
validator = jsonschema.Draft202012Validator(schema)

events = []
for line in open(events_path, "r", encoding="utf-8"):
    line = line.strip()
    if not line:
        continue
    doc = json.loads(line)
    validator.validate(doc)
    events.append(doc)

kinds = [e.get("kind") for e in events]
assert "task_started_v1" in kinds, kinds
assert "task_finished_v1" in kinds, kinds

assert any(e.get("kind") == "task_started_v1" and e.get("task_id") == "noop_v1" for e in events)
assert any(e.get("kind") == "task_finished_v1" and e.get("task_id") == "noop_v1" for e in events)

start_idx = next(
    i
    for i, e in enumerate(events)
    if e.get("kind") == "task_started_v1" and e.get("task_id") == "noop_v1"
)
finish_idx = next(
    i
    for i, e in enumerate(events)
    if e.get("kind") == "task_finished_v1" and e.get("task_id") == "noop_v1"
)
assert start_idx < finish_idx, (start_idx, finish_idx, kinds)
PY

printf 'OK %s\n' "$(basename "$0")"
