#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  else
    python_bin="python3"
  fi
fi

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi
if [[ "$x07_bin" != /* ]]; then
  x07_bin="$root/$x07_bin"
fi

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    mkdir -p "$root/tmp"
    tmp_dir="$(mktemp -d -p "$root/tmp" x07_policy_templates_XXXXXX)"
    ;;
  *)
    tmp_dir="$(mktemp -t x07_policy_templates_XXXXXX -d)"
    ;;
esac

cleanup() {
  rm -rf "$tmp_dir" || true
}
trap cleanup EXIT

check_template() {
  local template="$1"
  local work="$tmp_dir/$template"
  mkdir -p "$work"

  (cd "$work" && "$x07_bin" init >/dev/null)

  local report1="$work/report1.json"
  local report2="$work/report2.json"

  (cd "$work" && "$x07_bin" policy init --template "$template" --project x07.json --emit report >"$report1")
  (cd "$work" && "$x07_bin" policy init --template "$template" --project x07.json --emit report >"$report2")

  "$python_bin" - "$template" "$report1" "$report2" <<'PY'
import json, sys
from pathlib import Path

template = sys.argv[1]
r1 = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
r2 = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))

def req(d, k):
    if k not in d:
        raise SystemExit(f"{template}: missing {k}")
    return d[k]

for i, r in enumerate((r1, r2), start=1):
    if req(r, "schema_version") != "x07.policy.init.report@0.1.0":
        raise SystemExit(f"{template}: report{i}: schema_version mismatch")
    if req(r, "template") != template:
        raise SystemExit(f"{template}: report{i}: template mismatch")
    if req(r, "status") not in ("created", "unchanged", "overwritten", "exists_different"):
        raise SystemExit(f"{template}: report{i}: unexpected status {r['status']!r}")

if r1["status"] != "created":
    raise SystemExit(f"{template}: first init expected status 'created', got {r1['status']!r}")
if r2["status"] != "unchanged":
    raise SystemExit(f"{template}: second init expected status 'unchanged', got {r2['status']!r}")
PY

  local policy_path
  policy_path="$work/.x07/policies/base/${template}.sandbox.base.policy.json"
  [[ -f "$policy_path" ]] || { echo "ERROR: $template: missing $policy_path" >&2; exit 1; }

  "$python_bin" - "$template" "$policy_path" <<'PY'
import json, sys
from pathlib import Path

template = sys.argv[1]
pol = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
if pol.get("schema_version") != "x07.run-os-policy@0.1.0":
    raise SystemExit(f"{template}: policy schema_version mismatch")
if not isinstance(pol.get("policy_id"), str) or not pol["policy_id"]:
    raise SystemExit(f"{template}: policy_id must be a non-empty string")
PY

  echo "ok: policy template $template"
}

for t in \
  cli \
  http-client \
  web-service \
  fs-tool \
  sqlite-app \
  postgres-client \
  worker \
  worker-parallel \
; do
  check_template "$t"
done

printf 'OK %s\n' "$(basename "$0")"
