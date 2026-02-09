#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

step() {
  echo
  echo "==> $*"
}

need() {
  command -v "$1" >/dev/null 2>&1 || { echo "ERROR: missing tool: $1" >&2; exit 2; }
}

pick_python() {
  if [[ -n "${X07_PYTHON:-}" ]]; then
    echo "$X07_PYTHON"
    return
  fi
  if command -v python3 >/dev/null 2>&1; then
    echo "python3"
    return
  fi
  echo "python"
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux)
      case "$arch" in
        x86_64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) echo "unknown" ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64) echo "x86_64-apple-darwin" ;;
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        *) echo "unknown" ;;
      esac
      ;;
    *)
      echo "unknown"
      ;;
  esac
}

detect_platform_label() {
  case "$(uname -s)" in
    Linux) echo "Linux" ;;
    Darwin) echo "macOS" ;;
    *) echo "unknown" ;;
  esac
}

root="$(repo_root)"
cd "$root"

need bash
need tar
need cargo
need curl

python_bin="$(pick_python)"
need "$python_bin"

target="$(detect_target)"
platform="$(detect_platform_label)"
if [[ "$target" == "unknown" || "$platform" == "unknown" ]]; then
  echo "ERROR: unsupported host for installer smoke: target=$target platform=$platform" >&2
  exit 2
fi

tmp="$(mktemp -d)"
server_pid=""
cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

mode="${X07_INSTALL_SMOKE_MODE:-local}"

install_root="$tmp/x07root"
mkdir -p "$install_root"

channels_url=""
installer_path=""

if [[ "$mode" == "local" ]]; then
  step "build release binaries (including x07up)"
  cargo build --release -p x07 -p x07c -p x07-host-runner -p x07-os-runner -p x07import-cli -p x07up

  tag="v0.0.0-ci"
  artifacts="$tmp/artifacts"
  mkdir -p "$artifacts"

  step "package x07up archive"
  mkdir -p "$tmp/pkg/x07up"
  cp -f "target/release/x07up" "$tmp/pkg/x07up/x07up"
  chmod 0755 "$tmp/pkg/x07up/x07up"
  x07up_archive="$artifacts/x07up-${tag}-${target}.tar.gz"
  tar -czf "$x07up_archive" -C "$tmp/pkg/x07up" x07up

  step "package toolchain archive (CI minimal)"
  toolchain_archive="$artifacts/x07-${tag}-${target}.tar.gz"
  ./scripts/build_toolchain_tarball.sh --tag "$tag" --platform "$platform" --out "$toolchain_archive" --skip-native-backends

  step "start local artifacts server"
  server_json="$tmp/server.json"
  server_log="$tmp/server.log"
  "$python_bin" scripts/ci/local_http_server.py \
    --root "$artifacts" \
    --ready-json "$server_json" \
    --quiet \
    >"$server_log" 2>&1 &
  server_pid="$!"

  for _ in $(seq 1 200); do
    [[ -f "$server_json" ]] && break
    if ! kill -0 "$server_pid" >/dev/null 2>&1; then
      break
    fi
    sleep 0.05
  done
  if [[ ! -f "$server_json" ]]; then
    echo "ERROR: local server did not publish ready json" >&2
    if [[ -f "$server_log" ]]; then
      echo "--- local_http_server.py log ---" >&2
      tail -n 200 "$server_log" >&2 || true
    fi
    if ! kill -0 "$server_pid" >/dev/null 2>&1; then
      server_exit=0
      wait "$server_pid" || server_exit="$?"
      echo "ERROR: local server exited (status=$server_exit)" >&2
    fi
    exit 2
  fi
  if ! kill -0 "$server_pid" >/dev/null 2>&1; then
    echo "ERROR: local server exited early after publishing ready json" >&2
    if [[ -f "$server_log" ]]; then
      echo "--- local_http_server.py log ---" >&2
      tail -n 200 "$server_log" >&2 || true
    fi
    exit 2
  fi
  base_url="$("$python_bin" - "$server_json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
url = doc.get("url", "")
print(url.rstrip("/"))
PY
)"

  step "write local channels.json"
  "$python_bin" scripts/ci/make_channels_json.py \
    --base-url "$base_url" \
    --out "$artifacts/channels.json" \
    --tag "$tag" \
    --target "$target" \
    --toolchain-file "$toolchain_archive" \
    --x07up-file "$x07up_archive"

  channels_url="$base_url/channels.json"
  installer_path="$root/dist/install/install.sh"
else
  channels_url="${X07_CHANNELS_URL:-https://x07lang.org/install/channels.json}"
  installer_path="${X07_INSTALLER_SH:-https://x07lang.org/install.sh}"
fi

step "run installer (mode=$mode)"
install_report="$tmp/install.report.json"

if [[ "$installer_path" == http* ]]; then
  curl -fsSL "$installer_path" | bash -s -- \
    --yes \
    --root "$install_root" \
    --channel stable \
    --channels-url "$channels_url" \
    --no-modify-path \
    --json \
    >"$install_report"
else
  if [[ ! -x "$installer_path" ]]; then
    echo "ERROR: installer not executable: $installer_path" >&2
    exit 2
  fi
  bash "$installer_path" \
    --yes \
    --root "$install_root" \
    --channel stable \
    --channels-url "$channels_url" \
    --no-modify-path \
    --json \
    >"$install_report"
fi

export PATH="$install_root/bin:$PATH"

step "smoke: x07up show"
x07up show --json >"$tmp/x07up.show.json"
"$python_bin" - "$tmp/x07up.show.json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
for k in ("schema_version","toolchains","active","channels"):
    if k not in doc:
        raise SystemExit(f"ERROR: x07up show missing key: {k}")
print("ok: x07up show shape")
PY

step "smoke: x07 help"
x07 --help >/dev/null

step "smoke: init+run (os profile)"
proj="$tmp/proj"
mkdir -p "$proj"
cd "$proj"
x07 init >/dev/null
test -f "$proj/AGENT.md"
test -f "$proj/x07-toolchain.toml"
test -f "$proj/.agent/skills/x07-agent-playbook/SKILL.md"

printf "hello" > input.bin
x07 run --profile os --input input.bin --report wrapped --report-out .x07/run.os.json >/dev/null

"$python_bin" - ".x07/run.os.json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
if doc.get("schema_version") != "x07.run.report@0.1.0":
    raise SystemExit("ERROR: wrapped report schema_version mismatch")
if doc.get("runner") != "os":
    raise SystemExit("ERROR: expected runner=os")
rep = doc.get("report") or {}
if rep.get("exit_code") not in (0, "0"):
    sys.stderr.write(json.dumps(doc, indent=2, sort_keys=True) + "\n")
    raise SystemExit("ERROR: os run exit_code != 0")
compile_ok = (rep.get("compile") or {}).get("ok")
solve_ok = (rep.get("solve") or {}).get("ok")
if compile_ok is not True or solve_ok is not True:
    sys.stderr.write(json.dumps(doc, indent=2, sort_keys=True) + "\n")
    raise SystemExit("ERROR: os run compile/solve not ok")
roots = doc.get("target", {}).get("resolved_module_roots") or []
if not any(r.replace("\\","/").endswith("/src") or r == "src" for r in roots):
    raise SystemExit("ERROR: expected src in resolved_module_roots")
if not any(r.replace("\\","/").endswith("stdlib/os/0.2.0/modules") for r in roots):
    raise SystemExit("ERROR: expected stdlib/os module root for os runner")
print("ok: os wrapped report ok")
PY

step "smoke: test harness baseline (stdlib.lock fallback)"
x07 test --manifest tests/tests.json >"$tmp/x07test.report.json"
"$python_bin" - "$tmp/x07test.report.json" <<'PY'
import json, sys
from pathlib import Path
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
if doc.get("schema_version") != "x07.x07test@0.3.0":
    raise SystemExit("ERROR: x07test schema_version mismatch")
summary = doc.get("summary") or {}
if summary.get("passed") != 1:
    raise SystemExit(f"ERROR: expected 1 passed test, got: {summary.get('passed')}")
stdlib_lock = (doc.get("invocation") or {}).get("stdlib_lock")
if not isinstance(stdlib_lock, str) or not stdlib_lock.endswith("stdlib.lock"):
    raise SystemExit("ERROR: missing invocation.stdlib_lock")
if not Path(stdlib_lock).is_file():
    raise SystemExit(f"ERROR: invocation.stdlib_lock does not exist: {stdlib_lock}")
print("ok: x07 test ok")
PY

echo
echo "ok: installer smoke passed"
