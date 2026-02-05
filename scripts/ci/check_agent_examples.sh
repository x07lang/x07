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

./scripts/ci/ensure_runners.sh

# Ensure the native ext-fs backend exists (required by OS-world examples).
./scripts/ci/ensure_ext_fs_backend.sh >/dev/null

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

extract_derived_policy_path() {
  local stdout_log="$1"
  local stderr_log="$2"
  local line
  line="$(grep -E '^x07 run: using derived policy ' "$stderr_log" | tail -n 1 || true)"
  if [[ -z "$line" ]]; then
    line="$(grep -E '^x07 run: using derived policy ' "$stdout_log" | tail -n 1 || true)"
  fi
  if [[ -z "$line" ]]; then
    die "expected derived policy path line in $stderr_log or $stdout_log"
  fi
  printf '%s' "${line#x07 run: using derived policy }"
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

seed_official_deps() {
  local work="$1"
  "$python_bin" - "$root" "$work" <<'PY'
import json
import shutil
import sys
from pathlib import Path

repo_root = Path(sys.argv[1]).resolve()
work = Path(sys.argv[2]).resolve()

doc = json.loads((work / "x07.json").read_text(encoding="utf-8"))
deps = doc.get("dependencies") or []
if not isinstance(deps, list):
    raise SystemExit("x07.json: dependencies must be an array")

for dep in deps:
    if not isinstance(dep, dict):
        raise SystemExit(f"x07.json: dependency must be an object: {dep!r}")
    name = dep.get("name")
    version = dep.get("version")
    rel_path = dep.get("path")
    if not isinstance(name, str) or not name:
        raise SystemExit(f"x07.json: dependency.name must be string: {dep!r}")
    if not isinstance(version, str) or not version:
        raise SystemExit(f"x07.json: dependency.version must be string: {dep!r}")
    if not isinstance(rel_path, str) or not rel_path:
        raise SystemExit(f"x07.json: dependency.path must be string: {dep!r}")

    dst = work / rel_path
    if dst.exists():
        if dst.is_dir():
            shutil.rmtree(dst)
        else:
            raise SystemExit(f"dependency path exists but is not a directory: {dst}")

    src = repo_root / "packages" / "ext" / f"x07-{name}" / version
    if not src.is_dir():
        raise SystemExit(f"missing official package dir for {name}@{version}: {src}")

    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(src, dst)
PY
}

unwrap_and_check_wrapped_report() {
  local name="$1"
  local wrapped_path="$2"
  local runner_out="$3"
  local want_runner="$4"   # "host" or "os" (or "" to skip)
  local want_world="$5"    # e.g. "run-os" (or "" to skip)
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

  # Keep agent examples deterministic: no network access during pkg lock checks.
  (cd "$work" && "$x07_bin" pkg lock --check --offline >/dev/null)
}

run_x07_run() {
  local name="$1"
  local work="$2"
  shift 2

  mkdir -p "$work/tmp"
  mkdir -p "$work/artifacts"

  local wrapped="$work/artifacts/run.report.json"
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

require_path "docs/examples/agent-gate"
require_path "ci/fixtures/repair-corpus"
require_path "ci/fixtures/www"

# ----------------------------
# Example 1: CLI parsing (newline payload)
# ----------------------------

echo "==> agent example: cli-newline (run-os)"

cli1_work="$tmp_dir/cli-newline"
copy_project "docs/examples/agent-gate/cli-newline" "$cli1_work"

seed_official_deps "$cli1_work"
pkg_lock_check "$cli1_work"
fmt_check_all "$cli1_work"
lint_check_one "$cli1_work" "run-os" "src/main.x07.json"

url_1="https://example.invalid/"
depth_1="2"
out_1="out/results.txt"
mkdir -p "$cli1_work/out"
mkdir -p "$cli1_work/tmp"
printf "%s\n%s\n%s\n" "$url_1" "$depth_1" "$out_1" >"$cli1_work/tmp/input.txt"

wrapped_1="$(run_x07_run "cli-newline" "$cli1_work" --profile os --input "tmp/input.txt")"
unwrap_and_check_wrapped_report "cli-newline" "$wrapped_1" "$cli1_work/tmp/runner.json" "os" "run-os" "false"

expected_1="url=${url_1}"$'\n'"depth=${depth_1}"$'\n'"out=${out_1}"$'\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "cli-newline" --path "$cli1_work/tmp/runner.json" --expect "$expected_1" >/dev/null

echo "ok: cli-newline"

# ----------------------------
# Example 2: CLI parsing (ext-cli + argv_v1 via `x07 run -- ...`)
# ----------------------------

echo "==> agent example: cli-ext-cli (run-os + ext-cli)"

cli2_work="$tmp_dir/cli-ext-cli"
copy_project "docs/examples/agent-gate/cli-ext-cli" "$cli2_work"

seed_official_deps "$cli2_work"
pkg_lock_check "$cli2_work"
fmt_check_all "$cli2_work"
lint_check_one "$cli2_work" "run-os" "src/main.x07.json"

url_2="https://example.invalid/"
depth_2="3"
out_2="out/results.txt"
mkdir -p "$cli2_work/out"

wrapped_2="$(run_x07_run "cli-ext-cli" "$cli2_work" --profile os -- tool --url "$url_2" --depth "$depth_2" --out "$out_2")"
unwrap_and_check_wrapped_report "cli-ext-cli" "$wrapped_2" "$cli2_work/tmp/runner.json" "os" "run-os" "true"

expected_2="url=${url_2}"$'\n'"depth=${depth_2}"$'\n'"out=${out_2}"$'\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "cli-ext-cli" --path "$cli2_work/tmp/runner.json" --expect "$expected_2" >/dev/null

echo "ok: cli-ext-cli"

# ----------------------------
# Example 3: Text utils (run-os + ext-text)
# ----------------------------

echo "==> agent example: text-utils (run-os + ext-text)"

text_work="$tmp_dir/text-utils"
copy_project "docs/examples/agent-gate/text-core/text-utils" "$text_work"

seed_official_deps "$text_work"
pkg_lock_check "$text_work"
fmt_check_all "$text_work"
lint_check_one "$text_work" "run-os" "src/main.x07.json"

wrapped_3="$(run_x07_run "text-utils" "$text_work" --profile os)"
unwrap_and_check_wrapped_report "text-utils" "$wrapped_3" "$text_work/tmp/runner.json" "os" "run-os" "true"

expected_3="hello|world"$'\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "text-utils" --path "$text_work/tmp/runner.json" --expect "$expected_3" >/dev/null

echo "ok: text-utils"

# ----------------------------
# Example 4: BigInt factorial (run-os + ext-bigint-rs)
# ----------------------------

echo "==> agent example: factorial-100 (run-os + ext-bigint-rs)"

bigint_work="$tmp_dir/factorial-100"
copy_project "docs/examples/agent-gate/math-bigint/factorial-100" "$bigint_work"

seed_official_deps "$bigint_work"
pkg_lock_check "$bigint_work"
fmt_check_all "$bigint_work"
lint_check_one "$bigint_work" "run-os" "src/main.x07.json"

wrapped_4="$(run_x07_run "factorial-100" "$bigint_work" --profile os)"
unwrap_and_check_wrapped_report "factorial-100" "$wrapped_4" "$bigint_work/tmp/runner.json" "os" "run-os" "true"

expected_4="ok"$'\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "factorial-100" --path "$bigint_work/tmp/runner.json" --expect "$expected_4" >/dev/null

echo "ok: factorial-100"

# ----------------------------
# Example 5: Decimal money formatting (run-os + ext-decimal-rs)
# ----------------------------

echo "==> agent example: money-format (run-os + ext-decimal-rs)"

decimal_work="$tmp_dir/money-format"
copy_project "docs/examples/agent-gate/math-decimal/money-format" "$decimal_work"

seed_official_deps "$decimal_work"
pkg_lock_check "$decimal_work"
fmt_check_all "$decimal_work"
lint_check_one "$decimal_work" "run-os" "src/main.x07.json"

wrapped_5="$(run_x07_run "money-format" "$decimal_work" --profile os)"
unwrap_and_check_wrapped_report "money-format" "$wrapped_5" "$decimal_work/tmp/runner.json" "os" "run-os" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "money-format" --path "$decimal_work/tmp/runner.json" --expect "$expected_4" >/dev/null

echo "ok: money-format"

# ----------------------------
# Example 6: Unicode normalize + casefold (run-os + ext-unicode-rs)
# ----------------------------

echo "==> agent example: normalize-casefold (run-os + ext-unicode-rs)"

unicode_work="$tmp_dir/normalize-casefold"
copy_project "docs/examples/agent-gate/text-unicode/normalize-casefold" "$unicode_work"

seed_official_deps "$unicode_work"
pkg_lock_check "$unicode_work"
fmt_check_all "$unicode_work"
lint_check_one "$unicode_work" "run-os" "src/main.x07.json"

wrapped_6="$(run_x07_run "normalize-casefold" "$unicode_work" --profile os)"
unwrap_and_check_wrapped_report "normalize-casefold" "$wrapped_6" "$unicode_work/tmp/runner.json" "os" "run-os" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "normalize-casefold" --path "$unicode_work/tmp/runner.json" --expect "$expected_4" >/dev/null

echo "ok: normalize-casefold"

# ----------------------------
# Example 7: CBOR roundtrip (run-os + ext-cbor-rs)
# ----------------------------

echo "==> agent example: cbor-roundtrip (run-os + ext-cbor-rs)"

cbor_work="$tmp_dir/data-cbor"
copy_project "docs/examples/agent-gate/data-cbor/roundtrip" "$cbor_work"

seed_official_deps "$cbor_work"
pkg_lock_check "$cbor_work"
fmt_check_all "$cbor_work"
lint_check_one "$cbor_work" "run-os" "src/main.x07.json"

wrapped_7="$(run_x07_run "data-cbor" "$cbor_work" --profile os)"
unwrap_and_check_wrapped_report "data-cbor" "$wrapped_7" "$cbor_work/tmp/runner.json" "os" "run-os" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "data-cbor" --path "$cbor_work/tmp/runner.json" --expect "$expected_4" >/dev/null

echo "ok: data-cbor"

# ----------------------------
# Example 8: MessagePack roundtrip (run-os + ext-msgpack-rs)
# ----------------------------

echo "==> agent example: msgpack-roundtrip (run-os + ext-msgpack-rs)"

msgpack_work="$tmp_dir/data-msgpack"
copy_project "docs/examples/agent-gate/data-msgpack/roundtrip" "$msgpack_work"

seed_official_deps "$msgpack_work"
pkg_lock_check "$msgpack_work"
fmt_check_all "$msgpack_work"
lint_check_one "$msgpack_work" "run-os" "src/main.x07.json"

wrapped_8="$(run_x07_run "data-msgpack" "$msgpack_work" --profile os)"
unwrap_and_check_wrapped_report "data-msgpack" "$wrapped_8" "$msgpack_work/tmp/runner.json" "os" "run-os" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "data-msgpack" --path "$msgpack_work/tmp/runner.json" --expect "$expected_4" >/dev/null

echo "ok: data-msgpack"

# ----------------------------
# Example 9: Checksums (run-os + ext-checksum-rs)
# ----------------------------

echo "==> agent example: checksum-smoke (run-os + ext-checksum-rs)"

checksum_work="$tmp_dir/checksum-fast"
copy_project "docs/examples/agent-gate/checksum-fast/smoke" "$checksum_work"

seed_official_deps "$checksum_work"
pkg_lock_check "$checksum_work"
fmt_check_all "$checksum_work"
lint_check_one "$checksum_work" "run-os" "src/main.x07.json"

wrapped_9="$(run_x07_run "checksum-fast" "$checksum_work" --profile os)"
unwrap_and_check_wrapped_report "checksum-fast" "$wrapped_9" "$checksum_work/tmp/runner.json" "os" "run-os" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "checksum-fast" --path "$checksum_work/tmp/runner.json" --expect "$expected_4" >/dev/null

echo "ok: checksum-fast"

# ----------------------------
# Example 10: Diff/patch apply (run-os + ext-diff-rs)
# ----------------------------

echo "==> agent example: diff-patch-apply (run-os + ext-diff-rs)"

diff_work="$tmp_dir/diff-patch"
copy_project "docs/examples/agent-gate/diff-patch/apply" "$diff_work"

seed_official_deps "$diff_work"
pkg_lock_check "$diff_work"
fmt_check_all "$diff_work"
lint_check_one "$diff_work" "run-os" "src/main.x07.json"

wrapped_10="$(run_x07_run "diff-patch" "$diff_work" --profile os)"
unwrap_and_check_wrapped_report "diff-patch" "$wrapped_10" "$diff_work/tmp/runner.json" "os" "run-os" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "diff-patch" --path "$diff_work/tmp/runner.json" --expect "$expected_4" >/dev/null

echo "ok: diff-patch"

# ----------------------------
# Example 11: zstd roundtrip (run-os + ext-compress-rs)
# ----------------------------

echo "==> agent example: compress-zstd (run-os + ext-compress-rs)"

zstd_work="$tmp_dir/compress-zstd"
copy_project "docs/examples/agent-gate/compress-zstd/roundtrip" "$zstd_work"

seed_official_deps "$zstd_work"
pkg_lock_check "$zstd_work"
fmt_check_all "$zstd_work"
lint_check_one "$zstd_work" "run-os" "src/main.x07.json"

wrapped_11="$(run_x07_run "compress-zstd" "$zstd_work" --profile os)"
unwrap_and_check_wrapped_report "compress-zstd" "$wrapped_11" "$zstd_work/tmp/runner.json" "os" "run-os" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "compress-zstd" --path "$zstd_work/tmp/runner.json" --expect "$expected_4" >/dev/null

echo "ok: compress-zstd"

# ----------------------------
# Example 12: OS-world glob + walk (run-os + ext-path-glob-rs)
# ----------------------------

echo "==> agent example: fs-globwalk (run-os + ext-path-glob-rs)"

globwalk_work="$tmp_dir/fs-globwalk"
copy_project "docs/examples/agent-gate/fs-globwalk/list-files" "$globwalk_work"

seed_official_deps "$globwalk_work"
pkg_lock_check "$globwalk_work"
fmt_check_all "$globwalk_work"
lint_check_one "$globwalk_work" "run-os" "src/main.x07.json"

wrapped_12="$(run_x07_run "fs-globwalk" "$globwalk_work" --profile os)"
unwrap_and_check_wrapped_report "fs-globwalk" "$wrapped_12" "$globwalk_work/tmp/runner.json" "os" "run-os" "true"

expected_12="a.txt"$'\n'"sub/c.txt"$'\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "fs-globwalk" --path "$globwalk_work/tmp/runner.json" --expect "$expected_12" >/dev/null

echo "ok: fs-globwalk"

# ----------------------------
# Example 13: WS/gRPC framing over loopback TCP (sandboxed OS world)
# ----------------------------

echo "==> agent example: protos-framing-loopback (run-os-sandboxed + allow-host sugar)"

proto_work="$tmp_dir/protos-framing-loopback"
copy_project "docs/examples/agent-gate/protos-framing-loopback" "$proto_work"

seed_official_deps "$proto_work"
pkg_lock_check "$proto_work"
"$x07_bin" policy init --template web-service --project "$proto_work/x07.json" --mkdir-out >/dev/null
fmt_check_all "$proto_work"
lint_check_one "$proto_work" "run-os-sandboxed" "src/main.x07.json"

wrapped_13="$(run_x07_run "protos-framing-loopback" "$proto_work" \
  --profile sandbox \
  --allow-host "127.0.0.1:18081" \
  --cpu-time-limit-seconds 60 \
)"
unwrap_and_check_wrapped_report "protos-framing-loopback" "$wrapped_13" "$proto_work/tmp/runner.json" "os" "run-os-sandboxed" "true"

expected_13='{"grpc":"ping","ok":true,"ws":"hello"}'$'\n'
"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "protos-framing-loopback" --path "$proto_work/tmp/runner.json" --expect "$expected_13" >/dev/null

echo "ok: protos-framing-loopback"

# ----------------------------
# Example 14: Web crawler against a local fixture site (sandboxed OS world)
# ----------------------------

echo "==> agent example: web-crawler-local (run-os-sandboxed + allow-host sugar)"

crawler_work="$tmp_dir/web-crawler-local"
copy_project "docs/examples/agent-gate/web-crawler-local" "$crawler_work"

seed_official_deps "$crawler_work"
pkg_lock_check "$crawler_work"
"$x07_bin" policy init --template http-client --project "$crawler_work/x07.json" --mkdir-out >/dev/null
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

# Wait for ready file. Some runners (notably macOS) can be slow to spin up Python.
for _i in $(seq 1 1000); do
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
  echo "--- local_http_server.py process ---" >&2
  ps -p "$SERVER_PID" -o pid=,ppid=,command= >&2 || true
  echo "--- server_ready path ---" >&2
  ls -l "$server_ready" >&2 || true
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
out_13="out/results.txt"
mkdir -p "$crawler_work/out"

wrapped_13="$(run_x07_run "web-crawler-local" "$crawler_work" \
  --profile sandbox \
  --allow-host "${host}:${port}" \
  --cpu-time-limit-seconds 60 \
  -- crawler --url "$base_url" --depth "2" --out "$out_13" \
)"
unwrap_and_check_wrapped_report "web-crawler-local" "$wrapped_13" "$crawler_work/tmp/runner.json" "os" "run-os-sandboxed" "true"

"$python_bin" "$root/scripts/ci/assert_run_os_ok.py" "web-crawler-local" --path "$crawler_work/tmp/runner.json" --expect "ok" >/dev/null

# Ensure derived policy was materialized (agent affordance semantics).
gen_dir="$crawler_work/.x07/policies/_generated"
[[ -d "$gen_dir" ]] || die "expected derived policy dir to exist: $gen_dir"
ls "$gen_dir"/*.json >/dev/null 2>&1 || die "expected a derived policy JSON under: $gen_dir"

# Assert allow-host was merged into the derived policy that x07 run actually used.
policy_allow="$(extract_derived_policy_path "$crawler_work/tmp/run.stdout" "$crawler_work/tmp/run.stderr")"
[[ -f "$policy_allow" ]] || die "derived policy path missing: $policy_allow"
"$python_bin" - "$policy_allow" "$host" "$port" <<'PY'
import json, sys
p=json.load(open(sys.argv[1], "r", encoding="utf-8"))
host=sys.argv[2].strip().lower()
port=int(sys.argv[3])
allow_hosts=(p.get("net") or {}).get("allow_hosts") or []
ok=False
for e in allow_hosts:
    if not isinstance(e, dict):
        continue
    if (e.get("host") or "").strip().lower() != host:
        continue
    ports=e.get("ports") or []
    if isinstance(ports, list) and port in ports:
        ok=True
        break
assert ok, (host, port, allow_hosts)
PY

# Now run with allow + deny for the same host, and ensure deny wins in the derived policy.
wrapped_13b="$(run_x07_run "web-crawler-local" "$crawler_work" \
  --profile sandbox \
  --allow-host "${host}:${port}" \
  --deny-host "${host}:${port}" \
  --cpu-time-limit-seconds 60 \
  -- crawler --url "$base_url" --depth "1" --out "$out_13" \
)"
unwrap_and_check_wrapped_report "web-crawler-local" "$wrapped_13b" "$crawler_work/tmp/runner_deny.json" "os" "run-os-sandboxed" "true"

policy_deny="$(extract_derived_policy_path "$crawler_work/tmp/run.stdout" "$crawler_work/tmp/run.stderr")"
[[ -f "$policy_deny" ]] || die "derived policy path missing: $policy_deny"
"$python_bin" - "$policy_deny" "$host" "$port" <<'PY'
import json, sys
p=json.load(open(sys.argv[1], "r", encoding="utf-8"))
host=sys.argv[2].strip().lower()
port=int(sys.argv[3])
allow_hosts=(p.get("net") or {}).get("allow_hosts") or []
for e in allow_hosts:
    if not isinstance(e, dict):
        continue
    if (e.get("host") or "").strip().lower() != host:
        continue
    ports=e.get("ports") or []
    if isinstance(ports, list) and port in ports:
        raise SystemExit((host, port, allow_hosts))
PY

# Compare produced outputs against golden fixtures.
require_path "$crawler_work/$out_13"
require_path "$crawler_work/$out_13.text"

expected_urls="$fixture_site/expected_urls.txt"
expected_text="$fixture_site/expected_text.txt"
require_path "$expected_urls"
require_path "$expected_text"

expected_urls_tmp="$crawler_work/tmp/expected_urls.actual_port.txt"
sed -e "s/{{PORT}}/${port}/g" -e "s/18080/${port}/g" "$expected_urls" >"$expected_urls_tmp"

diff -u "$expected_urls_tmp" "$crawler_work/$out_13" >/dev/null
diff -u "$expected_text" "$crawler_work/$out_13.text" >/dev/null

echo "ok: web-crawler-local"

echo
echo "ok: agent examples gate passed"
