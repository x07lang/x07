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

# Ensure runners exist for x07 run (repo CI builds them earlier, but allow standalone execution).
if [[ ! -x "target/debug/x07-host-runner" && ! -x "target/release/x07-host-runner" ]]; then
  cargo build -p x07-host-runner -p x07-os-runner >/dev/null
fi

# Ensure the native ext-fs backend exists (required by OS-world examples that write files).
if [[ ! -f "deps/x07/include/x07_ext_fs_abi_v1.h" ]] || \
   [[ ! -f "deps/x07/libx07_ext_fs.a" && ! -f "deps/x07/x07_ext_fs.lib" ]]; then
  ./scripts/build_ext_fs.sh >/dev/null
fi

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    # Keep temp paths under the repo on Windows so both bash tools and native
    # Windows executables (python, x07.exe) agree on path semantics.
    mkdir -p "$root/tmp"
    tmp_dir="$(mktemp -d -p "$root/tmp" x07_agent_examples_XXXXXX)"
    ;;
  *)
    tmp_dir="$(mktemp -t x07_agent_examples_XXXXXX -d)"
    ;;
esac
cleanup() {
  # Best-effort cleanup of background processes (server).
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp_dir" || true
}
trap cleanup EXIT

die() {
  echo "ERROR: $*" >&2
  exit 1
}

require_path() {
  local p="$1"
  [[ -e "$p" ]] || die "missing required path: $p"
}

copy_project() {
  local src_rel="$1"
  local dst="$2"
  mkdir -p "$dst"
  cp -a "$root/$src_rel/." "$dst/"
}

unwrap_and_check_wrapped_report() {
  local name="$1"
  local wrapped_path="$2"
  local runner_out="$3"
  local want_runner="$4"   # "host" or "os" (or "" to skip)
  local want_world="$5"    # e.g. "solve-pure" (or "" to skip)
  local require_deps="$6"  # "true" or "false"

  "$python_bin" - "$name" "$wrapped_path" "$runner_out" "$want_runner" "$want_world" "$require_deps" <<'PY'
import json, sys
from pathlib import Path

name = sys.argv[1]
wrapped_path = Path(sys.argv[2])
runner_out = Path(sys.argv[3])
want_runner = sys.argv[4].strip()
want_world = sys.argv[5].strip()
require_deps = sys.argv[6].strip().lower() == "true"

doc = json.loads(wrapped_path.read_text(encoding="utf-8"))
sv = doc.get("schema_version")
if sv != "x07.run.report@0.1.0":
    raise SystemExit(f"{name}: expected schema_version x07.run.report@0.1.0, got {sv!r}")

if want_runner and doc.get("runner") != want_runner:
    raise SystemExit(f"{name}: expected runner {want_runner!r}, got {doc.get('runner')!r}")

if want_world and doc.get("world") != want_world:
    raise SystemExit(f"{name}: expected world {want_world!r}, got {doc.get('world')!r}")

target = doc.get("target") or {}
roots = target.get("resolved_module_roots") or []
if require_deps:
    ok = False
    for r in roots:
        if isinstance(r, str) and (".x07/deps/" in r or ".x07\\deps\\" in r):
            ok = True
            break
    if not ok:
        raise SystemExit(f"{name}: expected resolved_module_roots to include .x07/deps (lockfile module roots)")

runner_report = doc.get("report")
if not isinstance(runner_report, dict):
    raise SystemExit(f"{name}: wrapped.report must be an object")

runner_out.write_text(json.dumps(runner_report, indent=2) + "\n", encoding="utf-8")
PY
}

fmt_check_all() {
  local work="$1"
  (cd "$work" && find src -name '*.x07.json' -print0 | while IFS= read -r -d '' f; do
    "$x07_bin" fmt --input "$f" --check >/dev/null
  done)
}

lint_check_one() {
  local work="$1"
  local world="$2"
  local file_rel="$3"
  (cd "$work" && "$x07_bin" lint --input "$file_rel" --world "$world" >/dev/null)
}

pkg_lock_check() {
  local work="$1"

  local args=(pkg lock --check)
  if [[ "${X07_AGENT_GATE_OFFLINE:-}" == "1" ]]; then
    args+=(--offline)
  fi
  (cd "$work" && "$x07_bin" "${args[@]}" >/dev/null)
}

run_x07_run() {
  local name="$1"
  local work="$2"
  shift 2

  mkdir -p "$work/tmp"

  local wrapped="$work/tmp/run.wrapped.json"
  local stdout_log="$work/tmp/run.stdout"
  local stderr_log="$work/tmp/run.stderr"

  set +e
  (cd "$work" && "$x07_bin" run --report wrapped --report-out "$wrapped" "$@" >"$stdout_log" 2>"$stderr_log")
  local code="$?"
  set -e

  if [[ "$code" -ne 0 ]]; then
    echo "ERROR: $name: x07 run failed (exit $code)" >&2
    echo "--- stderr ($stderr_log) ---" >&2
    cat "$stderr_log" >&2 || true
    echo "--- stdout ($stdout_log) ---" >&2
    cat "$stdout_log" >&2 || true
    if [[ -s "$wrapped" ]]; then
      echo "--- wrapped report ($wrapped) ---" >&2
      cat "$wrapped" >&2 || true
    fi
    exit 1
  fi

  echo "$wrapped"
}

# ----------------------------
# Fixtures + expected outputs
# ----------------------------

require_path "examples/agent-gate"
require_path "ci/fixtures/repair-corpus"
require_path "ci/fixtures/www"

# ----------------------------
# Example 1: CLI parsing (newline payload)
# ----------------------------

echo "==> agent example: cli-newline (solve-pure)"

cli1_work="$tmp_dir/cli-newline"
copy_project "examples/agent-gate/cli-newline" "$cli1_work"

pkg_lock_check "$cli1_work"
fmt_check_all "$cli1_work"
lint_check_one "$cli1_work" "solve-pure" "src/main.x07.json"

url_1="https://example.invalid/"
depth_1="2"
out_1="out/results.txt"
mkdir -p "$cli1_work/out"
mkdir -p "$cli1_work/tmp"
printf "%s\n%s\n%s\n" "$url_1" "$depth_1" "$out_1" >"$cli1_work/tmp/input.txt"

wrapped_1="$(run_x07_run "cli-newline" "$cli1_work" --profile test --input "tmp/input.txt")"
unwrap_and_check_wrapped_report "cli-newline" "$wrapped_1" "$cli1_work/tmp/runner.json" "host" "solve-pure" "false"

expected_1="url=${url_1}"$'\n'"depth=${depth_1}"$'\n'"out=${out_1}"$'\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "cli-newline" --path "$cli1_work/tmp/runner.json" --expect "$expected_1" >/dev/null

echo "ok: cli-newline"

# ----------------------------
# Example 2: CLI parsing (ext-cli + argv_v1 via `x07 run -- ...`)
# ----------------------------

echo "==> agent example: cli-ext-cli (solve-pure + ext-cli)"

cli2_work="$tmp_dir/cli-ext-cli"
copy_project "examples/agent-gate/cli-ext-cli" "$cli2_work"

pkg_lock_check "$cli2_work"
fmt_check_all "$cli2_work"
lint_check_one "$cli2_work" "solve-pure" "src/main.x07.json"

url_2="https://example.invalid/"
depth_2="3"
out_2="out/results.txt"
mkdir -p "$cli2_work/out"

wrapped_2="$(run_x07_run "cli-ext-cli" "$cli2_work" --profile test -- tool --url "$url_2" --depth "$depth_2" --out "$out_2")"
unwrap_and_check_wrapped_report "cli-ext-cli" "$wrapped_2" "$cli2_work/tmp/runner.json" "host" "solve-pure" "true"

expected_2="url=${url_2}"$'\n'"depth=${depth_2}"$'\n'"out=${out_2}"$'\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "cli-ext-cli" --path "$cli2_work/tmp/runner.json" --expect "$expected_2" >/dev/null

echo "ok: cli-ext-cli"

# ----------------------------
# Example 3: Web crawler against a local fixture site (sandboxed OS world)
# ----------------------------

echo "==> agent example: web-crawler-local (run-os-sandboxed + allow-host sugar)"

crawler_work="$tmp_dir/web-crawler-local"
copy_project "examples/agent-gate/web-crawler-local" "$crawler_work"

pkg_lock_check "$crawler_work"
fmt_check_all "$crawler_work"
lint_check_one "$crawler_work" "run-os-sandboxed" "src/main.x07.json"

fixture_site="$root/ci/fixtures/www/crawl_site_v1"
require_path "$fixture_site"
require_path "$root/scripts/ci/local_http_server.py"

server_ready="$tmp_dir/http_server.ready.json"
server_log="$tmp_dir/http_server.log"
set +e
"$python_bin" "$root/scripts/ci/local_http_server.py" \
  --root "$fixture_site" \
  --host 127.0.0.1 \
  --port 0 \
  --ready-json "$server_ready" \
  --quiet \
  >"$server_log" 2>&1 &
SERVER_PID="$!"
set -e

# Wait for ready file.
for _i in $(seq 1 200); do
  if [[ -s "$server_ready" ]]; then
    break
  fi
  if ! kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    echo "--- local_http_server.py log ---" >&2
    cat "$server_log" >&2 || true
    die "local_http_server.py exited before becoming ready"
  fi
  sleep 0.05
done
if [[ ! -s "$server_ready" ]]; then
  echo "--- local_http_server.py log ---" >&2
  cat "$server_log" >&2 || true
  die "local_http_server.py did not become ready (timeout)"
fi

host="$("$python_bin" - "$server_ready" <<'PY'
import json, sys
print(json.load(open(sys.argv[1], "r", encoding="utf-8"))["host"])
PY
)"
port="$("$python_bin" - "$server_ready" <<'PY'
import json, sys
print(json.load(open(sys.argv[1], "r", encoding="utf-8"))["port"])
PY
)"

base_url="http://${host}:${port}/"
out_3="out/results.txt"
mkdir -p "$crawler_work/out"

wrapped_3="$(run_x07_run "web-crawler-local" "$crawler_work" \
  --profile sandbox \
  --allow-host "${host}:${port}" \
  --cpu-time-limit-seconds 60 \
  -- crawler --url "$base_url" --depth "2" --out "$out_3" \
)"
unwrap_and_check_wrapped_report "web-crawler-local" "$wrapped_3" "$crawler_work/tmp/runner.json" "os" "run-os-sandboxed" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "web-crawler-local" --path "$crawler_work/tmp/runner.json" --expect "ok" >/dev/null

# Ensure derived policy was materialized (agent affordance semantics).
gen_dir="$crawler_work/.x07/policies/_generated"
[[ -d "$gen_dir" ]] || die "expected derived policy dir to exist: $gen_dir"
ls "$gen_dir"/*.json >/dev/null 2>&1 || die "expected a derived policy JSON under: $gen_dir"

# Compare produced outputs against golden fixtures.
require_path "$crawler_work/$out_3"
require_path "$crawler_work/$out_3.text"

expected_urls="$fixture_site/expected_urls.txt"
expected_text="$fixture_site/expected_text.txt"
require_path "$expected_urls"
require_path "$expected_text"

expected_urls_tmp="$crawler_work/tmp/expected_urls.actual_port.txt"
sed -e "s/{{PORT}}/${port}/g" -e "s/18080/${port}/g" "$expected_urls" >"$expected_urls_tmp"

diff -u "$expected_urls_tmp" "$crawler_work/$out_3" >/dev/null
diff -u "$expected_text" "$crawler_work/$out_3.text" >/dev/null

echo "ok: web-crawler-local"

echo
echo "ok: agent examples gate passed"
