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

# Doc flows should be runnable from a local toolchain build.
cargo build -p x07 -p x07up -p x07-host-runner -p x07-os-runner >/dev/null

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi
if [[ "$x07_bin" != /* ]]; then
  x07_bin="$root/$x07_bin"
fi

x07up_bin="${X07UP_BIN:-}"
if [[ -z "${x07up_bin}" ]]; then
  # Mirror scripts/ci/find_x07.sh behavior: prefer built binaries.
  if [[ -x "$root/target/debug/x07up" ]]; then
    x07up_bin="$root/target/debug/x07up"
  elif [[ -x "$root/target/release/x07up" ]]; then
    x07up_bin="$root/target/release/x07up"
  else
    x07up_bin="x07up"
  fi
fi

bin_dir="$(cd "$(dirname "$x07_bin")" && pwd)"
export PATH="$bin_dir:$PATH"

# Doc flows should not require a VM sandbox backend on macOS runners.
export X07_SANDBOX_BACKEND="${X07_SANDBOX_BACKEND:-os}"
export X07_I_ACCEPT_WEAKER_ISOLATION="${X07_I_ACCEPT_WEAKER_ISOLATION:-1}"
export X07_REQUIRE_SOLVERS="${X07_REQUIRE_SOLVERS:-0}"

./scripts/ci/ensure_runners.sh >/dev/null
./scripts/ci/ensure_ext_fs_backend.sh >/dev/null

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    mkdir -p "$root/tmp"
    tmp_dir="$(mktemp -d -p "$root/tmp" x07_doc_flows_XXXXXX)"
    ;;
  *)
    tmp_dir="$(mktemp -t x07_doc_flows_XXXXXX -d)"
    ;;
esac
cleanup() { rm -rf "$tmp_dir" || true; }
trap cleanup EXIT

unwrap_report() {
  local wrapped_path="$1"
  local runner_out="$2"
  "$python_bin" - "$wrapped_path" "$runner_out" <<'PY'
import json, sys
from pathlib import Path

wrapped = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if wrapped.get("schema_version","") != "x07.run.report@0.3.0":
    raise SystemExit("wrapped report schema_version mismatch")
report = wrapped.get("report")
if not isinstance(report, dict):
    raise SystemExit("wrapped.report must be an object")
Path(sys.argv[2]).write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
PY
}

echo "[check] x07up show --json"
"$x07up_bin" show --json | "$python_bin" -c 'import json,sys; json.load(sys.stdin)'

# ----------------------------
# Flow 1: minimal project (docs/getting-started/agent-quickstart.md)
# ----------------------------

echo "[check] doc flow: minimal project (x07 init -> fmt -> lint -> run -> test)"

min="$tmp_dir/min"
mkdir -p "$min"
(cd "$min" && "$x07_bin" init >/dev/null)

(cd "$min" && "$x07_bin" fmt --input src/main.x07.json --write >/dev/null)
(cd "$min" && "$x07_bin" lint --input src/main.x07.json >/dev/null)

runner_json="$min/runner.json"
(cd "$min" && "$x07_bin" run --offline --profile os --report runner >"$runner_json")
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "doc-flow:min" --path "$runner_json" --expect "" >/dev/null

(cd "$min" && "$x07_bin" test --manifest tests/tests.json >/dev/null)
echo "ok: doc-flow:min"

# ----------------------------
# Flow 2: CLI template (x07 init --template cli)
# ----------------------------

echo "[check] doc flow: cli template (x07 init --template cli -> x07 run -- ...)"

cli="$tmp_dir/cli"
mkdir -p "$cli"
(cd "$cli" && "$x07_bin" init --template cli >/dev/null)

(cd "$cli" && find src -name '*.x07.json' -print0 | while IFS= read -r -d '' f; do
  "$x07_bin" fmt --input "$f" --check >/dev/null
done)
(cd "$cli" && "$x07_bin" lint --input src/main.x07.json >/dev/null)

mkdir -p "$cli/tmp"
wrapped="$cli/tmp/run.wrapped.json"
(cd "$cli" && "$x07_bin" pkg lock --check --offline >/dev/null)
(cd "$cli" && "$x07_bin" run --offline --profile os --report wrapped --report-out "$wrapped" -- \
  tool --url https://example.invalid/ --depth 2 --out out/results.txt >/dev/null)

runner_out="$cli/tmp/runner.json"
unwrap_report "$wrapped" "$runner_out"

expected=$'url=https://example.invalid/\ndepth=2\nout=out/results.txt\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "doc-flow:cli" --path "$runner_out" --expect "$expected" >/dev/null
echo "ok: doc-flow:cli"

# ----------------------------
# Flow 3: JSON report template (x07 init --template json-report)
# ----------------------------

echo "[check] doc flow: json-report template (typed CLI -> DataModel -> JSON)"

json_report="$tmp_dir/json-report"
mkdir -p "$json_report"
(cd "$json_report" && "$x07_bin" init --template json-report >/dev/null)

(cd "$json_report" && "$x07_bin" pkg lock --check --offline >/dev/null)

mkdir -p "$json_report/tmp"
wrapped="$json_report/tmp/run.wrapped.json"
(cd "$json_report" && "$x07_bin" run --offline --profile os --report wrapped --report-out "$wrapped" -- \
  tool --count 3 --label hello --pretty false >/dev/null)

runner_out="$json_report/tmp/runner.json"
unwrap_report "$wrapped" "$runner_out"

expected=$'{"count":3,"label":"hello","pretty":false,"schema_version":"x07.example.json_report@0.1.0"}\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "doc-flow:json-report" --path "$runner_out" --expect "$expected" >/dev/null
echo "ok: doc-flow:json-report"

# ----------------------------
# Flow 4: FS tool (run-os-sandboxed + base policy)
# ----------------------------

echo "[check] doc flow: fs-tool template (sandboxed file I/O caps)"

fs_tool="$tmp_dir/fs-tool"
mkdir -p "$fs_tool"
(cd "$fs_tool" && "$x07_bin" init --template fs-tool >/dev/null)

mkdir -p "$fs_tool/src"
printf 'hello' >"$fs_tool/src/input.txt"

runner_json="$fs_tool/runner.json"
(cd "$fs_tool" && "$x07_bin" run --offline --report runner >"$runner_json")

expected=$'ok\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "doc-flow:fs-tool" --path "$runner_json" --expect "$expected" >/dev/null

test -f "$fs_tool/out/output.txt"
expected_path="$fs_tool/tmp/expected.txt"
mkdir -p "$fs_tool/tmp"
printf 'hello' >"$expected_path"
diff -u "$expected_path" "$fs_tool/out/output.txt" >/dev/null
echo "ok: doc-flow:fs-tool"

printf 'OK %s\n' "$(basename "$0")"
