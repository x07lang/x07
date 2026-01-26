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

if [[ -n "${X07_BIN:-}" ]]; then
  x07_bin="${X07_BIN}"
  if [[ "$x07_bin" != /* ]]; then
    x07_bin="$root/$x07_bin"
  fi
else
  cargo build -q -p x07 -p x07-os-runner >/dev/null
  x07_bin="$root/target/debug/x07"
  if [[ ! -x "$x07_bin" && -f "$x07_bin.exe" ]]; then
    x07_bin="$x07_bin.exe"
  fi
fi

if [[ ! -x "$x07_bin" ]]; then
  echo "ERROR: x07 binary not found/executable at: $x07_bin" >&2
  exit 2
fi

# Temp workdir (Windows/MSYS2: keep under repo to avoid path issues).
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    mkdir -p "$root/tmp"
    tmp_dir="$(mktemp -d -p "$root/tmp" x07_threads_smoke_XXXXXX)"
    ;;
  *)
    tmp_dir="$(mktemp -t x07_threads_smoke_XXXXXX -d)"
    ;;
esac

keep_tmp="${X07_THREADS_SMOKE_KEEP_TMP:-}"
cleanup() {
  if [[ -z "$keep_tmp" ]]; then
    rm -rf "$tmp_dir" || true
  else
    echo "[threads-smoke] kept tmp dir: $tmp_dir"
  fi
}
trap cleanup EXIT

fixture_src="$root/ci/fixtures/concurrency/threads-policy-deny-blocking"
if [[ ! -d "$fixture_src" ]]; then
  echo "ERROR: missing concurrency fixture dir: $fixture_src" >&2
  exit 2
fi

fixture="$tmp_dir/threads-policy-deny-blocking"
cp -R "$fixture_src" "$fixture"
cd "$fixture"

# 1) Generate a schema-valid base policy.
"$x07_bin" policy init --template cli --project x07.json --emit report >"$tmp_dir/policy.init.report.json"

policy_path=".x07/policies/base/cli.sandbox.base.policy.json"
if [[ ! -f "$policy_path" ]]; then
  echo "ERROR: policy init did not create expected file: $policy_path" >&2
  exit 2
fi

# 2) Patch in threads limits (max_blocking=0) to force a predictable trap.
"$python_bin" - "$policy_path" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
policy = json.loads(path.read_text(encoding="utf-8"))

policy["threads"] = {
    "enabled": True,
    "max_workers": 1,
    "max_blocking": 0,
    "max_queue": 1024,
}

path.write_text(json.dumps(policy, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY

# Ensure the policy would otherwise allow the write (so a missing directory isn't the failure mode).
mkdir -p out

# 3) Run and capture the runner report JSON.
report_json="$tmp_dir/run.report.json"

set +e
"$x07_bin" run \
  --project x07.json \
  --profile sandbox \
  --report runner \
  --report-out "$report_json" \
  >/dev/null
rc="$?"
set -e

if [[ "$rc" -eq 0 ]]; then
  echo "ERROR: expected non-zero exit for threads.max_blocking=0, got rc=0" >&2
  echo "report:" >&2
  sed -n '1,200p' "$report_json" >&2 || true
  exit 2
fi

expected_trap="os.threads.blocking disabled by policy"

"$python_bin" - "$report_json" "$expected_trap" <<'PY'
import json
import sys
from pathlib import Path

doc = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
expected = sys.argv[2]

if "solve" in doc:
    solve = doc.get("solve") or {}
    ok = solve.get("ok")
    trap = solve.get("trap")
else:
    ok = doc.get("ok")
    trap = doc.get("trap")

if ok is not False:
    raise SystemExit(f"expected ok=false, got: {ok!r}")

if not trap:
    raise SystemExit("missing trap string")

if expected not in trap:
    raise SystemExit(f"trap mismatch:\n  got: {trap!r}\n  expected substring: {expected!r}")

print("ok: trap string matched")
PY

if [[ -f "out/deny.txt" ]]; then
  echo "ERROR: out/deny.txt was created but should not be (I/O must trap before writing)" >&2
  exit 2
fi

echo "ok: check_threads_smoke"
